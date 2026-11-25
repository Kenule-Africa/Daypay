-- Keep one predecessor per session for logout requests racing a token rotation.
-- Validation, refresh and targeted controls still accept only the current digest.
ALTER TABLE identity.sessions ADD COLUMN logout_previous_digest text,
    ADD COLUMN logout_rotated_at timestamptz;
CREATE INDEX sessions_logout_predecessor ON identity.sessions(auth_release,logout_previous_digest)
    WHERE logout_previous_digest IS NOT NULL AND revoked_at IS NULL;

CREATE FUNCTION identity.auth_track_rotation() RETURNS trigger LANGUAGE plpgsql SET search_path=pg_catalog,pg_temp AS $$
BEGIN
    NEW.logout_previous_digest:=OLD.token_hash;
    NEW.logout_rotated_at:=clock_timestamp();
    RETURN NEW;
END $$;
CREATE TRIGGER remember_logout_predecessor BEFORE UPDATE OF token_hash ON identity.sessions
FOR EACH ROW WHEN (OLD.token_hash IS DISTINCT FROM NEW.token_hash) EXECUTE FUNCTION identity.auth_track_rotation();

-- Private helper. Check the deadlines after acquiring the row lock. Only one
-- caller can perform the active-to-revoked transition and write the audit event.
CREATE FUNCTION identity.auth_close_owned(r uuid, customer uuid, target uuid, cause text)
RETURNS boolean LANGUAGE plpgsql SET search_path=pg_catalog,pg_temp AS $$
DECLARE selected identity.sessions; stamp timestamptz;
BEGIN
    SELECT s.* INTO selected FROM identity.sessions s
        WHERE s.id=target AND s.user_id=customer AND s.auth_release=r FOR UPDATE;
    IF NOT FOUND THEN RETURN false; END IF;
    stamp:=clock_timestamp();
    IF selected.revoked_at IS NOT NULL OR selected.created_at>stamp OR
        least(selected.expires_at,selected.absolute_expires_at)<=stamp THEN RETURN false; END IF;
    UPDATE identity.sessions SET revoked_at=stamp WHERE id=selected.id;
    INSERT INTO audit.audit_events(actor_type,actor_id,action,resource_type,resource_id,reason)
        VALUES('CUSTOMER',customer,'SESSION_REVOKED','SESSION',selected.id,cause);
    RETURN true;
END $$;

CREATE FUNCTION identity.auth_logout(r uuid, digest text, issued bigint, tag bytea)
RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE p identity.auth_policy; customer uuid; target uuid;
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
    IF NOT FOUND THEN RETURN; END IF;
    PERFORM FROM identity.users WHERE id=customer FOR NO KEY UPDATE;
    PERFORM identity.auth_close_owned(r,customer,target,'CUSTOMER_LOGOUT');
    PERFORM identity.auth_check(p,'logout',issued,ARRAY[digest],tag);
END $$;

CREATE FUNCTION identity.auth_list_sessions(r uuid, digest text, issued bigint, tag bytea)
RETURNS TABLE(session_id uuid,is_current boolean,created_at timestamptz,expires_at timestamptz)
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE p identity.auth_policy; customer uuid; stamp timestamptz;
BEGIN
    p:=identity.auth_active(r);
    PERFORM identity.auth_check(p,'list_sessions',issued,ARRAY[digest],tag);
    SELECT a.customer_id INTO customer FROM identity.auth_session(p,digest,NULL) a;
    IF NOT FOUND THEN RETURN; END IF;
    stamp:=clock_timestamp();
    RETURN QUERY SELECT s.id,s.token_hash=digest,s.created_at,least(s.expires_at,s.absolute_expires_at)
        FROM identity.sessions s WHERE s.auth_release=r AND s.user_id=customer AND s.revoked_at IS NULL
        AND s.created_at<=stamp AND least(s.expires_at,s.absolute_expires_at)>stamp
        ORDER BY s.created_at DESC,s.id;
END $$;

CREATE FUNCTION identity.auth_revoke_session(r uuid, digest text, target uuid, issued bigint, tag bytea)
RETURNS text LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE p identity.auth_policy; customer uuid; caller uuid; fields text[]:=ARRAY[digest,target::text];
BEGIN
    p:=identity.auth_active(r);
    PERFORM identity.auth_check(p,'revoke_session',issued,fields,tag);
    SELECT s.user_id,s.id INTO customer,caller FROM identity.sessions s
        WHERE s.auth_release=r AND s.token_hash=digest;
    IF NOT FOUND THEN RETURN NULL; END IF;
    -- Serialize same-owner control mutations before locking the caller and
    -- target. Reciprocal revocations cannot deadlock or authorize a stale caller.
    PERFORM FROM identity.users WHERE id=customer FOR NO KEY UPDATE;
    -- Validate the caller after any wait on the target, including an expiry
    -- while another transaction was already updating that session.
    PERFORM FROM identity.sessions s WHERE s.id=target AND s.user_id=customer AND s.auth_release=r FOR UPDATE;
    PERFORM FROM identity.auth_session(p,digest,NULL);
    IF NOT FOUND THEN RETURN NULL; END IF;
    IF NOT identity.auth_close_owned(r,customer,target,'CUSTOMER_REVOKED') THEN RETURN 'ABSENT'; END IF;
    PERFORM identity.auth_check(p,'revoke_session',issued,fields,tag);
    RETURN CASE WHEN target=caller THEN 'CURRENT' ELSE 'OTHER' END;
END $$;
REVOKE ALL ON FUNCTION identity.auth_track_rotation(),identity.auth_close_owned(uuid,uuid,uuid,text),
    identity.auth_logout(uuid,text,bigint,bytea),identity.auth_list_sessions(uuid,text,bigint,bytea),
    identity.auth_revoke_session(uuid,text,uuid,bigint,bytea) FROM PUBLIC;

-- Preserve the existing ownership, search-path and effective privilege checks,
-- expanding the verifier's explicit allowlist only for the three public controls.
CREATE OR REPLACE FUNCTION identity.auth_acl_ok() RETURNS boolean LANGUAGE sql STABLE SET search_path=pg_catalog,pg_temp AS $$
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
            OR has_column_privilege(who,'identity.sessions','logout_previous_digest','SELECT')
            OR has_any_column_privilege(who,'identity.sessions','INSERT,UPDATE')
            OR has_table_privilege(who,'identity.sessions','DELETE,TRUNCATE,TRIGGER')
            OR has_any_column_privilege(who,'identity.users','UPDATE')
            OR has_schema_privilege(who,'identity','CREATE') OR has_schema_privilege(who,'auth_crypto','CREATE'))
    AND to_regprocedure('identity.auth_logout(uuid,text,bigint,bytea)') IS NOT NULL
    AND to_regprocedure('identity.auth_list_sessions(uuid,text,bigint,bytea)') IS NOT NULL
    AND to_regprocedure('identity.auth_revoke_session(uuid,text,uuid,bigint,bytea)') IS NOT NULL
    AND (SELECT bool_and(
        pr.proowner=(SELECT relowner FROM pg_class WHERE oid='identity.auth_policy'::regclass)
        AND pr.proconfig @> ARRAY['search_path=pg_catalog, pg_temp']
        AND NOT EXISTS (SELECT FROM aclexplode(coalesce(pr.proacl,acldefault('f',pr.proowner))) a
            WHERE a.grantee=0 AND a.privilege_type='EXECUTE')
        AND has_function_privilege('deypay_auth',pr.oid,'EXECUTE') =
            (pr.proname IN ('auth_admit','auth_ready','auth_configuration','auth_credential','auth_reject','auth_issue','auth_validate','auth_logout','auth_list_sessions','auth_revoke_session'))
        AND has_function_privilege('deypay_app',pr.oid,'EXECUTE') = (pr.proname IN ('auth_reader_ready','auth_principal')))
        FROM pg_proc pr WHERE pr.pronamespace='identity'::regnamespace
        AND (left(pr.proname,5)='auth_' OR pr.proname='protect_auth_audit'))
$$;
