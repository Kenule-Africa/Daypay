-- The email counters survive the upgrade. Every replica serializes on the
-- same operation row and samples the database clock after obtaining the lock.
ALTER TABLE identity.email_verification_traffic
    DROP CONSTRAINT email_verification_traffic_operation_check,
    DROP CONSTRAINT email_verification_traffic_hits_check,
    ALTER COLUMN hits TYPE bigint,
    ADD CHECK (hits >= 0),
    ADD COLUMN window_cap bigint,
    ADD COLUMN named integer NOT NULL DEFAULT 0 CHECK (named >= 0),
    ADD CHECK (operation IN ('resend','verify','register','forgot_password','reset_password',
                             'username_availability','refresh','logout','session_list','session_revoke','logout_noop','session_revoke_noop'));

CREATE TABLE identity.admission_policy (
    operation text PRIMARY KEY CHECK (operation IN ('forgot_password','reset_password','username_availability',
                                                    'refresh','logout','session_list','session_revoke')),
    total_limit integer NOT NULL CHECK (total_limit > 0),
    subject_limit integer CHECK (subject_limit BETWEEN 1 AND total_limit),
    subject_key bytea NOT NULL DEFAULT auth_crypto.gen_random_bytes(32) CHECK (octet_length(subject_key) = 32),
    CHECK ((operation IN ('forgot_password','reset_password','username_availability')) = (subject_limit IS NOT NULL))
);

CREATE TABLE identity.admission_subject_window (
    operation text NOT NULL REFERENCES identity.email_verification_traffic(operation),
    subject_digest bytea NOT NULL CHECK (octet_length(subject_digest) = 32),
    hits integer NOT NULL CHECK (hits > 0),
    PRIMARY KEY (operation, subject_digest)
);

-- Private counter engine used only by the fixed, execute-only entry points.
CREATE FUNCTION identity.take_admission(op text, cap bigint, subject_hash bytea, subject_cap integer)
RETURNS boolean LANGUAGE plpgsql SET search_path=pg_catalog,pg_temp AS $$
DECLARE
    w identity.email_verification_traffic;
    bucket timestamptz;
BEGIN
    IF cap IS NULL OR cap < 1 THEN RAISE EXCEPTION 'Admission unavailable'; END IF;
    INSERT INTO identity.email_verification_traffic(operation,minute,hits)
        VALUES(op,date_trunc('minute',clock_timestamp()),0) ON CONFLICT DO NOTHING;
    SELECT * INTO STRICT w FROM identity.email_verification_traffic WHERE operation=op FOR UPDATE;
    bucket := date_trunc('minute',clock_timestamp());
    IF bucket > w.minute THEN
        DELETE FROM identity.admission_subject_window WHERE operation=op;
        UPDATE identity.email_verification_traffic SET minute=bucket,hits=0,named=0,window_cap=cap
            WHERE operation=op RETURNING * INTO w;
    ELSIF w.window_cap IS NULL OR cap < w.window_cap THEN
        -- A replica with a looser configuration cannot reopen the current window.
        UPDATE identity.email_verification_traffic SET window_cap=cap WHERE operation=op RETURNING * INTO w;
    END IF;
    IF subject_hash IS NULL THEN
        IF w.hits >= w.window_cap THEN RETURN false; END IF;
        UPDATE identity.email_verification_traffic SET hits=hits+1 WHERE operation=op;
    ELSE
        IF w.named >= w.window_cap THEN RETURN false; END IF;
        INSERT INTO identity.admission_subject_window AS s(operation,subject_digest,hits)
            VALUES(op,subject_hash,1) ON CONFLICT(operation,subject_digest) DO UPDATE
            SET hits=s.hits+1 WHERE s.hits<subject_cap;
        IF NOT FOUND THEN RETURN false; END IF;
        UPDATE identity.email_verification_traffic SET named=named+1 WHERE operation=op;
    END IF;
    RETURN true;
END $$;

CREATE OR REPLACE FUNCTION identity.admit_email_request(op text, cap integer) RETURNS boolean
LANGUAGE plpgsql STRICT SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
BEGIN
    IF op NOT IN ('verify','resend') OR cap NOT BETWEEN 1 AND 100000 THEN RETURN false; END IF;
    RETURN identity.take_admission(op,cap,NULL,NULL);
END $$;

CREATE FUNCTION identity.admit_registration_request(cap bigint) RETURNS boolean
LANGUAGE sql STRICT SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
    SELECT identity.take_admission('register',cap,NULL,NULL)
$$;

CREATE FUNCTION identity.admit_public_request(op text, subject text) RETURNS boolean
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE p identity.admission_policy; digest bytea;
BEGIN
    IF op IS NULL OR op NOT IN ('forgot_password','reset_password','username_availability') THEN
        RAISE EXCEPTION 'Admission unavailable';
    END IF;
    SELECT * INTO STRICT p FROM identity.admission_policy WHERE operation=op FOR SHARE;
    IF subject IS NOT NULL THEN
        IF octet_length(subject) NOT BETWEEN 1 AND 512 THEN RAISE EXCEPTION 'Invalid admission subject'; END IF;
        IF op='forgot_password' THEN subject:=lower(btrim(subject)); END IF;
        digest := auth_crypto.hmac(convert_to(subject,'UTF8'),p.subject_key,'sha256');
    END IF;
    RETURN identity.take_admission(op,p.total_limit,digest,p.subject_limit);
END $$;

CREATE FUNCTION identity.require_session_admission(op text, effective boolean) RETURNS void
LANGUAGE plpgsql STRICT SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE cap integer; lane text;
BEGIN
    IF op NOT IN ('refresh','logout','session_list','session_revoke') THEN RAISE EXCEPTION 'Admission unavailable'; END IF;
    SELECT total_limit INTO STRICT cap FROM identity.admission_policy WHERE operation=op FOR SHARE;
    -- Failed and repeated attempts cannot spend the capacity reserved for closing
    -- a live session. Both lanes use the same operator-configured ceiling.
    lane := CASE WHEN NOT effective AND op IN ('logout','session_revoke') THEN op || '_noop' ELSE op END;
    IF NOT identity.take_admission(lane,cap,NULL,NULL) THEN
        RAISE EXCEPTION 'Admission exhausted' USING ERRCODE='DP429';
    END IF;
END $$;

REVOKE ALL ON TABLE identity.admission_policy,identity.admission_subject_window FROM PUBLIC;
REVOKE ALL ON FUNCTION identity.take_admission(text,bigint,bytea,integer),
    identity.admit_registration_request(bigint),identity.admit_public_request(text,text),
    identity.require_session_admission(text,boolean) FROM PUBLIC;

-- Admission and the signed operation commit together. A limit error rolls back
-- any token rotation, revocation and associated audit event in the same call.
CREATE OR REPLACE FUNCTION identity.auth_validate(r uuid,digest text,replacement text,issued bigint,tag bytea)
RETURNS TABLE(customer_id uuid,account_status text) LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE p identity.auth_policy;
BEGIN
    p:=identity.auth_active(r);
    PERFORM identity.auth_check(p,'validate',issued,ARRAY[digest,coalesce(replacement,'')],tag);
    IF replacement IS NOT NULL THEN PERFORM identity.require_session_admission('refresh',true); END IF;
    RETURN QUERY SELECT * FROM identity.auth_session(p,digest,replacement);
END $$;

CREATE OR REPLACE FUNCTION identity.auth_list_sessions(r uuid, digest text, issued bigint, tag bytea)
RETURNS TABLE(session_id uuid,is_current boolean,created_at timestamptz,expires_at timestamptz)
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE p identity.auth_policy; customer uuid; stamp timestamptz;
BEGIN
    p:=identity.auth_active(r);
    PERFORM identity.auth_check(p,'list_sessions',issued,ARRAY[digest],tag);
    PERFORM identity.require_session_admission('session_list',true);
    SELECT a.customer_id INTO customer FROM identity.auth_session(p,digest,NULL) a;
    IF NOT FOUND THEN RETURN; END IF;
    stamp:=clock_timestamp();
    RETURN QUERY SELECT s.id,s.token_hash=digest,s.created_at,least(s.expires_at,s.absolute_expires_at)
        FROM identity.sessions s WHERE s.auth_release=r AND s.user_id=customer AND s.revoked_at IS NULL
        AND s.created_at<=stamp AND least(s.expires_at,s.absolute_expires_at)>stamp
        ORDER BY s.created_at DESC,s.id;
END $$;

CREATE OR REPLACE FUNCTION identity.auth_logout(r uuid, digest text, issued bigint, tag bytea)
RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE p identity.auth_policy; customer uuid; target uuid; closed boolean;
BEGIN
    p:=identity.auth_active(r);
    PERFORM identity.auth_check(p,'logout',issued,ARRAY[digest],tag);
    -- Resolve the session identity before waiting behind a refresh. If the
    -- refresh already committed, its predecessor has logout-only authority for
    -- at most the release's existing in-flight verifier assertion window.
    SELECT s.user_id,s.id INTO customer,target FROM identity.sessions s
        WHERE s.auth_release=r AND s.revoked_at IS NULL AND
        (s.token_hash=digest OR (s.logout_previous_digest=digest AND
            s.logout_rotated_at>=clock_timestamp()-make_interval(secs=>p.assertion_seconds)));
    IF NOT FOUND THEN
        PERFORM identity.require_session_admission('logout',false);
        RETURN;
    END IF;
    PERFORM FROM identity.users WHERE id=customer FOR NO KEY UPDATE;
    closed:=identity.auth_close_owned(r,customer,target,'CUSTOMER_LOGOUT');
    PERFORM identity.require_session_admission('logout',closed);
    PERFORM identity.auth_check(p,'logout',issued,ARRAY[digest],tag);
END $$;

CREATE OR REPLACE FUNCTION identity.auth_revoke_session(r uuid, digest text, target uuid, issued bigint, tag bytea)
RETURNS text LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE p identity.auth_policy; customer uuid; caller uuid; fields text[]:=ARRAY[digest,coalesce(target::text,'')];
BEGIN
    p:=identity.auth_active(r);
    PERFORM identity.auth_check(p,'revoke_session',issued,fields,tag);
    SELECT s.user_id,s.id INTO customer,caller FROM identity.sessions s
        WHERE s.auth_release=r AND s.token_hash=digest;
    IF NOT FOUND THEN
        PERFORM identity.require_session_admission('session_revoke',false);
        RETURN NULL;
    END IF;
    -- Serialize same-owner control mutations before locking the caller and
    -- target. Reciprocal revocations cannot deadlock or authorize a stale caller.
    PERFORM FROM identity.users WHERE id=customer FOR NO KEY UPDATE;
    -- Validate the caller after any wait on the target, including an expiry
    -- while another transaction was already updating that session.
    PERFORM FROM identity.sessions s WHERE s.id=target AND s.user_id=customer AND s.auth_release=r FOR UPDATE;
    PERFORM FROM identity.auth_session(p,digest,NULL);
    IF NOT FOUND THEN
        PERFORM identity.require_session_admission('session_revoke',false);
        RETURN NULL;
    END IF;
    IF NOT identity.auth_close_owned(r,customer,target,'CUSTOMER_REVOKED') THEN
        PERFORM identity.require_session_admission('session_revoke',false);
        RETURN 'ABSENT';
    END IF;
    PERFORM identity.require_session_admission('session_revoke',true);
    PERFORM identity.auth_check(p,'revoke_session',issued,fields,tag);
    RETURN CASE WHEN target=caller THEN 'CURRENT' ELSE 'OTHER' END;
END $$;
