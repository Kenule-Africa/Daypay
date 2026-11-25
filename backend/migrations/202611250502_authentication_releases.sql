-- Replace the sandbox authority in one SQLx transaction. No release is activated here.
DROP TRIGGER invalidate_changed_sandbox_identity ON identity.users;
DROP FUNCTION identity.invalidate_changed_login_identity(), identity.sandbox_login_credential(text),
    identity.issue_sandbox_session(uuid,text,text,text,text,integer,integer),
    identity.validate_sandbox_session(text,text,integer), identity.record_login_rejection(uuid),
    identity.lock_sandbox_login_customer(uuid,text);

CREATE SCHEMA auth_crypto;
REVOKE ALL ON SCHEMA auth_crypto FROM PUBLIC;
CREATE EXTENSION IF NOT EXISTS pgcrypto WITH SCHEMA auth_crypto;
DO $$ BEGIN
    IF to_regprocedure('auth_crypto.hmac(bytea,bytea,text)') IS NULL THEN
        RAISE EXCEPTION 'Provision pgcrypto in the restricted auth_crypto schema before enabling authentication';
    END IF;
END $$;

CREATE TABLE identity.auth_policy (
    id uuid PRIMARY KEY, enabled boolean NOT NULL DEFAULT false,
    environment text NOT NULL CHECK (environment IN ('local','test','staging','production')),
    origin text NOT NULL CHECK (origin LIKE 'https://%'),
    security_ref text NOT NULL CHECK (btrim(security_ref)<>''),
    product_ref text NOT NULL CHECK (btrim(product_ref)<>''),
    audit_ref text NOT NULL CHECK (btrim(audit_ref)<>''),
    signing_key bytea NOT NULL CHECK (octet_length(signing_key)=32),
    allowed_states text[] NOT NULL CHECK (cardinality(allowed_states)>0 AND array_position(allowed_states,NULL) IS NULL
        AND allowed_states <@ ARRAY['CONTACT_VERIFIED','KYC_REQUIRED','KYC_PENDING','KYC_APPROVED']),
    idle_seconds integer NOT NULL CHECK (idle_seconds>0),
    absolute_seconds integer NOT NULL CHECK (absolute_seconds>=idle_seconds),
    total_limit integer NOT NULL CHECK (total_limit>0),
    identifier_limit integer NOT NULL CHECK (identifier_limit BETWEEN 1 AND total_limit),
    memory_kib integer NOT NULL CHECK (memory_kib>0), passes integer NOT NULL CHECK (passes>0),
    lanes integer NOT NULL CHECK (lanes>0 AND memory_kib::bigint>=8::bigint*lanes),
    slots integer NOT NULL CHECK (slots>0),
    assertion_seconds integer NOT NULL CHECK (assertion_seconds BETWEEN 1 AND 60)
);
CREATE UNIQUE INDEX one_active_auth_release ON identity.auth_policy(enabled) WHERE enabled;
ALTER TABLE identity.sessions ADD COLUMN auth_release uuid REFERENCES identity.auth_policy,
    ADD COLUMN refresh_required boolean NOT NULL DEFAULT false;
CREATE INDEX auth_active_sessions ON identity.sessions(user_id,auth_release) WHERE revoked_at IS NULL;

-- The old cookies lack release binding. Retain their rows and audit the cutoff.
WITH retired AS (UPDATE identity.sessions SET revoked_at=greatest(created_at,clock_timestamp())
    WHERE revoked_at IS NULL RETURNING id,user_id)
INSERT INTO audit.audit_events(actor_type,actor_id,action,resource_type,resource_id,reason)
SELECT 'SYSTEM',user_id,'SESSION_REVOKED','SESSION',id,'AUTHORITY_UPGRADE' FROM retired;

CREATE TABLE identity.auth_window (
    release uuid PRIMARY KEY REFERENCES identity.auth_policy, minute timestamptz NOT NULL,
    total integer NOT NULL DEFAULT 0, named integer NOT NULL DEFAULT 0
);
CREATE TABLE identity.auth_identifier_window (
    release uuid NOT NULL REFERENCES identity.auth_window, identifier bytea NOT NULL,
    hits integer NOT NULL DEFAULT 1, PRIMARY KEY(release,identifier)
);
-- One random 256-bit session digest doubles as the signed issuance nonce.
-- Receipts survive session rotation, revocation or deletion until their proof expires.
CREATE TABLE identity.auth_receipt (
    release uuid NOT NULL REFERENCES identity.auth_policy, nonce text NOT NULL,
    valid_until timestamptz NOT NULL, PRIMARY KEY(release,nonce)
);
CREATE INDEX auth_receipt_expiry ON identity.auth_receipt(valid_until);

CREATE FUNCTION identity.auth_retire() RETURNS trigger LANGUAGE plpgsql
SET search_path=pg_catalog,pg_temp AS $$
BEGIN
    IF TG_OP='DELETE' OR NOT OLD.enabled OR NEW.enabled OR
        (to_jsonb(OLD)-'enabled') IS DISTINCT FROM (to_jsonb(NEW)-'enabled') THEN
        RAISE EXCEPTION 'Install a new authentication release';
    END IF;
    WITH retired AS (UPDATE identity.sessions SET revoked_at=greatest(created_at,clock_timestamp())
        WHERE auth_release=OLD.id AND revoked_at IS NULL RETURNING id,user_id)
    INSERT INTO audit.audit_events(actor_type,actor_id,action,resource_type,resource_id,reason)
        SELECT 'SYSTEM',user_id,'SESSION_REVOKED','SESSION',id,'RELEASE_RETIRED' FROM retired;
    DELETE FROM identity.auth_identifier_window WHERE release=OLD.id;
    DELETE FROM identity.auth_window WHERE release=OLD.id;
    DELETE FROM identity.auth_receipt WHERE release=OLD.id;
    RETURN NEW;
END $$;
CREATE TRIGGER immutable_auth_policy BEFORE UPDATE OR DELETE ON identity.auth_policy
FOR EACH ROW EXECUTE FUNCTION identity.auth_retire();

-- Invoker security deliberately preserves the effective identity of the outer operation.
CREATE FUNCTION identity.protect_auth_audit() RETURNS trigger LANGUAGE plpgsql
SET search_path=pg_catalog,pg_temp AS $$
BEGIN
    IF current_user <> pg_get_userbyid((SELECT relowner FROM pg_class WHERE oid='identity.auth_policy'::regclass)) THEN
        RAISE EXCEPTION 'Protected authentication event' USING ERRCODE='42501';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER protected_auth_events BEFORE INSERT ON audit.audit_events FOR EACH ROW
WHEN (NEW.action IN ('LOGIN_SUCCEEDED','LOGIN_REJECTED','SESSION_ISSUED','SESSION_REFRESHED','SESSION_REVOKED'))
EXECUTE FUNCTION identity.protect_auth_audit();

CREATE FUNCTION identity.auth_active(r uuid) RETURNS identity.auth_policy LANGUAGE plpgsql
SET search_path=pg_catalog,pg_temp AS $$
DECLARE p identity.auth_policy;
BEGIN
    SELECT * INTO STRICT p FROM identity.auth_policy WHERE id=r AND enabled FOR SHARE;
    RETURN p;
END $$;
CREATE FUNCTION identity.auth_hex(value text) RETURNS text LANGUAGE sql IMMUTABLE STRICT
SET search_path=pg_catalog,pg_temp AS $$ SELECT encode(sha256(convert_to(value,'UTF8')),'hex') $$;
CREATE FUNCTION identity.auth_check(p identity.auth_policy, purpose text, issued bigint, fields text[], tag bytea)
RETURNS void LANGUAGE plpgsql SET search_path=pg_catalog,pg_temp AS $$
DECLARE message text; expected bytea;
BEGIN
    IF issued IS NULL OR abs(extract(epoch FROM clock_timestamp())-issued::numeric)>p.assertion_seconds
        OR tag IS NULL OR octet_length(tag)<>32 OR array_position(fields,NULL) IS NOT NULL THEN
        RAISE EXCEPTION 'Invalid authentication authority' USING ERRCODE='28000';
    END IF;
    message:=array_to_string(ARRAY['deypay-auth-v1',purpose,p.id::text,issued::text]||fields,chr(10));
    expected:=auth_crypto.hmac(convert_to(message,'UTF8'),p.signing_key,'sha256');
    IF sha256(expected)<>sha256(tag) THEN
        RAISE EXCEPTION 'Invalid authentication authority' USING ERRCODE='28000';
    END IF;
END $$;

CREATE FUNCTION identity.auth_admit(r uuid, address text) RETURNS boolean LANGUAGE plpgsql
SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE p identity.auth_policy; w identity.auth_window; bucket timestamptz; k bytea;
BEGIN
    p:=identity.auth_active(r);
    INSERT INTO identity.auth_window(release,minute) VALUES(r,date_trunc('minute',clock_timestamp())) ON CONFLICT DO NOTHING;
    SELECT * INTO STRICT w FROM identity.auth_window WHERE release=r FOR UPDATE;
    bucket:=date_trunc('minute',clock_timestamp());
    IF bucket>w.minute THEN
        DELETE FROM identity.auth_identifier_window WHERE release=r;
        DELETE FROM identity.auth_receipt WHERE valid_until<clock_timestamp();
        UPDATE identity.auth_window SET minute=bucket,total=0,named=0 WHERE release=r RETURNING * INTO w;
    END IF;
    IF address IS NULL THEN
        IF w.total>=p.total_limit THEN RETURN false; END IF;
        UPDATE identity.auth_window SET total=total+1 WHERE release=r;
    ELSE
        IF octet_length(address)>512 OR w.named>=p.total_limit THEN RETURN false; END IF;
        k:=auth_crypto.hmac(convert_to('identifier:'||lower(btrim(address)),'UTF8'),p.signing_key,'sha256');
        INSERT INTO identity.auth_identifier_window AS i(release,identifier) VALUES(r,k)
        ON CONFLICT(release,identifier) DO UPDATE SET hits=i.hits+1 WHERE i.hits<p.identifier_limit;
        IF NOT FOUND THEN RETURN false; END IF;
        UPDATE identity.auth_window SET named=named+1 WHERE release=r;
    END IF;
    RETURN true;
END $$;

CREATE FUNCTION identity.auth_credential(r uuid,address text,issued bigint,tag bytea)
RETURNS TABLE(customer_id uuid,password_hash text) LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE p identity.auth_policy;
BEGIN
    p:=identity.auth_active(r);
    PERFORM identity.auth_check(p,'lookup',issued,ARRAY[identity.auth_hex(address)],tag);
    RETURN QUERY SELECT u.id,c.password_hash FROM identity.users u JOIN identity.password_credentials c ON c.user_id=u.id
        WHERE u.email=lower(btrim(address)) AND u.status=ANY(p.allowed_states)
        AND u.email_verified_at IS NOT NULL AND octet_length(c.password_hash)<=128;
END $$;
CREATE FUNCTION identity.auth_reject(r uuid,customer uuid,issued bigint,tag bytea) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
BEGIN
    PERFORM identity.auth_check(identity.auth_active(r),'reject',issued,ARRAY[coalesce(customer::text,'')],tag);
    INSERT INTO audit.audit_events(actor_type,action,resource_type,resource_id)
    VALUES('SYSTEM','LOGIN_REJECTED','CUSTOMER',customer);
END $$;

CREATE FUNCTION identity.auth_issue(r uuid,customer uuid,address text,fingerprint text,new_digest text,previous text,
    issued bigint,tag bytea) RETURNS boolean LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE p identity.auth_policy; fields text[]; stamp timestamptz; sid uuid;
BEGIN
    p:=identity.auth_active(r);
    fields:=ARRAY[customer::text,identity.auth_hex(address),fingerprint,new_digest,coalesce(previous,'')];
    PERFORM identity.auth_check(p,'issue',issued,fields,tag);
    IF new_digest !~ '^[a-f0-9]{64}$' OR fingerprint !~ '^[a-f0-9]{64}$'
        OR (previous IS NOT NULL AND (previous=new_digest OR previous !~ '^[a-f0-9]{64}$')) THEN RETURN false; END IF;
    PERFORM FROM identity.users u WHERE u.id=customer AND u.email=lower(btrim(address))
        AND u.email_verified_at IS NOT NULL AND u.status=ANY(p.allowed_states) FOR SHARE;
    IF FOUND THEN
        PERFORM FROM identity.password_credentials c WHERE c.user_id=customer
            AND octet_length(c.password_hash)<=128 AND identity.auth_hex(c.password_hash)=fingerprint FOR SHARE;
    END IF;
    IF NOT FOUND THEN
        INSERT INTO audit.audit_events(actor_type,action,resource_type,resource_id)
            VALUES('SYSTEM','LOGIN_REJECTED','CUSTOMER',customer);
        RETURN false;
    END IF;
    INSERT INTO identity.auth_receipt VALUES(r,new_digest,to_timestamp(issued)+make_interval(secs=>p.assertion_seconds))
        ON CONFLICT DO NOTHING;
    IF NOT FOUND THEN RETURN false; END IF;
    WITH retired AS (UPDATE identity.sessions SET revoked_at=greatest(created_at,clock_timestamp())
        WHERE token_hash=previous AND revoked_at IS NULL RETURNING id,user_id)
    INSERT INTO audit.audit_events(actor_type,actor_id,action,resource_type,resource_id,reason)
        SELECT 'SYSTEM',user_id,'SESSION_REVOKED','SESSION',id,'LOGIN_REPLACED' FROM retired;
    -- Recheck freshness after every potentially blocking lock, before committing effects.
    PERFORM identity.auth_check(p,'issue',issued,fields,tag);
    stamp:=clock_timestamp();
    INSERT INTO identity.sessions(auth_release,user_id,token_hash,created_at,last_activity_at,expires_at,absolute_expires_at)
        VALUES(r,customer,new_digest,stamp,stamp,stamp+make_interval(secs=>p.idle_seconds),
        stamp+make_interval(secs=>p.absolute_seconds)) RETURNING id INTO sid;
    INSERT INTO audit.audit_events(actor_type,actor_id,action,resource_type,resource_id)
        VALUES('CUSTOMER',customer,'LOGIN_SUCCEEDED','SESSION',sid),('CUSTOMER',customer,'SESSION_ISSUED','SESSION',sid);
    RETURN true;
END $$;

CREATE FUNCTION identity.auth_session(p identity.auth_policy, digest text, replacement text)
RETURNS TABLE(customer_id uuid,account_status text) LANGUAGE plpgsql SET search_path=pg_catalog,pg_temp AS $$
DECLARE customer uuid; state text; selected identity.sessions; stamp timestamptz;
BEGIN
    IF digest IS NULL OR digest !~ '^[a-f0-9]{64}$' OR (replacement IS NOT NULL AND
        (replacement=digest OR replacement !~ '^[a-f0-9]{64}$')) THEN RETURN; END IF;
    SELECT s.user_id INTO customer FROM identity.sessions s WHERE s.auth_release=p.id AND s.token_hash=digest;
    IF NOT FOUND THEN RETURN; END IF;
    SELECT u.status INTO state FROM identity.users u WHERE u.id=customer AND u.email_verified_at IS NOT NULL
        AND u.status=ANY(p.allowed_states) FOR SHARE;
    IF NOT FOUND THEN RETURN; END IF;
    SELECT s.* INTO selected FROM identity.sessions s WHERE s.auth_release=p.id AND s.user_id=customer
        AND s.token_hash=digest FOR UPDATE;
    IF NOT FOUND THEN RETURN; END IF;
    stamp:=clock_timestamp();
    IF selected.revoked_at IS NOT NULL OR selected.created_at>stamp OR selected.expires_at<=stamp
        OR selected.absolute_expires_at<=stamp OR (selected.refresh_required AND replacement IS NULL) THEN RETURN; END IF;
    UPDATE identity.sessions SET token_hash=coalesce(replacement,digest),refresh_required=false,last_activity_at=stamp,
        expires_at=least(selected.absolute_expires_at,stamp+make_interval(secs=>p.idle_seconds)) WHERE id=selected.id;
    IF replacement IS NOT NULL THEN
        INSERT INTO audit.audit_events(actor_type,actor_id,action,resource_type,resource_id)
        VALUES('CUSTOMER',customer,'SESSION_REFRESHED','SESSION',selected.id);
    END IF;
    RETURN QUERY SELECT customer,state;
END $$;

CREATE FUNCTION identity.auth_validate(r uuid,digest text,replacement text,issued bigint,tag bytea)
RETURNS TABLE(customer_id uuid,account_status text) LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE p identity.auth_policy;
BEGIN
    p:=identity.auth_active(r);
    PERFORM identity.auth_check(p,'validate',issued,ARRAY[digest,coalesce(replacement,'')],tag);
    RETURN QUERY SELECT * FROM identity.auth_session(p,digest,replacement);
END $$;

-- General API capability: a known live bearer digest can only be validated.
CREATE FUNCTION identity.auth_principal(r uuid,env text,web_origin text,digest text)
RETURNS TABLE(customer_id uuid,account_status text) LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE p identity.auth_policy;
BEGIN
    p:=identity.auth_active(r);
    IF (p.environment,p.origin) IS DISTINCT FROM (env,web_origin) THEN RAISE EXCEPTION 'Authentication policy mismatch'; END IF;
    RETURN QUERY SELECT * FROM identity.auth_session(p,digest,NULL);
END $$;

CREATE FUNCTION identity.auth_identity_changed() RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
BEGIN
    WITH retired AS (UPDATE identity.sessions s SET revoked_at=greatest(s.created_at,clock_timestamp())
        FROM identity.auth_policy p WHERE s.user_id=NEW.id AND s.revoked_at IS NULL AND p.id=s.auth_release
        AND ((OLD.email,OLD.email_verified_at) IS DISTINCT FROM (NEW.email,NEW.email_verified_at) OR NOT (NEW.status=ANY(p.allowed_states)))
        RETURNING s.id)
    INSERT INTO audit.audit_events(actor_type,actor_id,action,resource_type,resource_id,reason)
        SELECT 'SYSTEM',NEW.id,'SESSION_REVOKED','SESSION',id,'IDENTITY_CHANGED' FROM retired;
    IF OLD.status<>NEW.status THEN
        UPDATE identity.sessions SET refresh_required=true WHERE user_id=NEW.id AND revoked_at IS NULL;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER auth_identity_change AFTER UPDATE ON identity.users FOR EACH ROW
WHEN ((OLD.status,OLD.email,OLD.email_verified_at) IS DISTINCT FROM (NEW.status,NEW.email,NEW.email_verified_at))
EXECUTE FUNCTION identity.auth_identity_changed();

CREATE FUNCTION identity.auth_acl_ok() RETURNS boolean LANGUAGE sql STABLE SET search_path=pg_catalog,pg_temp AS $$
    SELECT (SELECT count(*)=2 AND bool_and(NOT(rolsuper OR rolcreaterole OR rolcreatedb OR rolreplication OR rolbypassrls))
        FROM pg_roles WHERE rolname IN ('deypay_auth','deypay_app'))
    AND NOT pg_has_role('deypay_app','deypay_auth','MEMBER')
    AND NOT pg_has_role('deypay_auth','deypay_app','MEMBER')
    AND NOT EXISTS (SELECT FROM (VALUES ('deypay_app'),('deypay_auth')) roles(who)
        CROSS JOIN (VALUES ('identity.auth_policy'),('identity.auth_window'),('identity.auth_identifier_window'),
            ('identity.auth_receipt'),('identity.password_credentials')) tables(rel)
        WHERE has_any_column_privilege(who,rel,'SELECT,INSERT,UPDATE')
           OR has_table_privilege(who,rel,'DELETE,TRUNCATE,TRIGGER'))
    AND NOT EXISTS (SELECT FROM (VALUES ('deypay_app'),('deypay_auth')) roles(who)
        WHERE has_column_privilege(who,'identity.sessions','token_hash','SELECT')
            OR has_any_column_privilege(who,'identity.sessions','INSERT,UPDATE')
            OR has_table_privilege(who,'identity.sessions','DELETE,TRUNCATE,TRIGGER')
            OR has_any_column_privilege(who,'identity.users','UPDATE')
            OR has_schema_privilege(who,'identity','CREATE') OR has_schema_privilege(who,'auth_crypto','CREATE'))
    AND (SELECT bool_and(
        pr.proowner=(SELECT relowner FROM pg_class WHERE oid='identity.auth_policy'::regclass)
        AND pr.proconfig @> ARRAY['search_path=pg_catalog, pg_temp']
        AND NOT EXISTS (SELECT FROM aclexplode(coalesce(pr.proacl,acldefault('f',pr.proowner))) a
            WHERE a.grantee=0 AND a.privilege_type='EXECUTE')
        AND has_function_privilege('deypay_auth',pr.oid,'EXECUTE') =
            (pr.proname IN ('auth_admit','auth_ready','auth_configuration','auth_credential','auth_reject','auth_issue','auth_validate'))
        AND has_function_privilege('deypay_app',pr.oid,'EXECUTE') = (pr.proname IN ('auth_reader_ready','auth_principal')))
        FROM pg_proc pr WHERE pr.pronamespace='identity'::regnamespace
        AND (left(pr.proname,5)='auth_' OR pr.proname='protect_auth_audit'))
$$;

CREATE FUNCTION identity.auth_ready(r uuid) RETURNS boolean LANGUAGE sql STABLE SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
    SELECT EXISTS (SELECT FROM identity.auth_policy WHERE id=r AND enabled) AND identity.auth_acl_ok()
$$;
CREATE FUNCTION identity.auth_reader_ready(r uuid, env text, web_origin text) RETURNS boolean LANGUAGE sql STABLE
SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
    SELECT EXISTS (SELECT FROM identity.auth_policy WHERE id=r AND enabled AND environment=env AND origin=web_origin)
        AND identity.auth_acl_ok()
$$;
CREATE FUNCTION identity.auth_configuration(r uuid,env text,nonce text) RETURNS TABLE(origin text,idle_seconds integer,
    absolute_seconds integer,memory_kib integer,passes integer,lanes integer,slots integer,key_check bytea)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
    SELECT p.origin,p.idle_seconds,p.absolute_seconds,p.memory_kib,p.passes,p.lanes,p.slots,
        auth_crypto.hmac(convert_to('ready'||chr(10)||p.id::text||chr(10)||nonce,'UTF8'),p.signing_key,'sha256')
    FROM identity.auth_policy p WHERE p.id=r AND p.enabled AND p.environment=env AND nonce ~ '^[a-f0-9]{64}$'
        AND identity.auth_acl_ok()
$$;

DO $$ DECLARE routine regprocedure;
BEGIN
    FOR routine IN SELECT p.oid::regprocedure FROM pg_proc p WHERE p.pronamespace='identity'::regnamespace
        AND (left(p.proname,5)='auth_' OR p.proname='protect_auth_audit')
    LOOP
        EXECUTE format('REVOKE ALL ON FUNCTION %s FROM PUBLIC',routine);
    END LOOP;
END $$;
