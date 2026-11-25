-- Do not allow a retired verifier key to be installed in another release.
CREATE UNIQUE INDEX auth_release_key_once ON identity.auth_policy (sha256(signing_key));

-- Retirement may be requested by the policy administrator. Authoritative
-- revocation events must still originate from the protected database identity.
ALTER FUNCTION identity.auth_retire() SECURITY DEFINER;

CREATE FUNCTION identity.auth_owner_ok() RETURNS boolean LANGUAGE sql STABLE SET search_path=pg_catalog,pg_temp AS $$
    SELECT EXISTS (SELECT FROM pg_roles owner
        WHERE owner.oid=(SELECT relowner FROM pg_class WHERE oid='identity.auth_policy'::regclass)
        AND owner.rolname='deypay_auth_owner' AND NOT (owner.rolcanlogin OR owner.rolsuper OR owner.rolcreaterole
            OR owner.rolcreatedb OR owner.rolbypassrls OR owner.rolreplication))
    AND NOT EXISTS (SELECT FROM pg_roles service CROSS JOIN pg_roles privileged
        WHERE service.rolname IN ('deypay_app','deypay_auth','deypay_auth_owner')
        AND (privileged.rolsuper OR privileged.rolname IN ('pg_read_all_data','pg_write_all_data',
            'pg_read_server_files','pg_write_server_files','pg_execute_server_program','pg_signal_backend'))
        AND pg_has_role(service.oid,privileged.oid,'MEMBER'))
    AND NOT EXISTS (SELECT FROM pg_proc p CROSS JOIN LATERAL aclexplode(p.proacl) a
        WHERE p.pronamespace='identity'::regnamespace AND (left(p.proname,5)='auth_' OR p.proname='protect_auth_audit')
        AND a.grantee<>p.proowner AND a.grantee NOT IN ('deypay_app'::regrole,'deypay_auth'::regrole))
$$;
REVOKE ALL ON FUNCTION identity.auth_owner_ok() FROM PUBLIC;

CREATE OR REPLACE FUNCTION identity.auth_ready(r uuid) RETURNS boolean LANGUAGE sql STABLE SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
    SELECT EXISTS (SELECT FROM identity.auth_policy WHERE id=r AND enabled) AND identity.auth_acl_ok() AND identity.auth_owner_ok()
$$;
CREATE OR REPLACE FUNCTION identity.auth_reader_ready(r uuid, env text, web_origin text) RETURNS boolean LANGUAGE sql STABLE
SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
    SELECT EXISTS (SELECT FROM identity.auth_policy WHERE id=r AND enabled AND environment=env AND origin=web_origin)
        AND identity.auth_acl_ok() AND identity.auth_owner_ok()
$$;
CREATE OR REPLACE FUNCTION identity.auth_configuration(r uuid,env text,nonce text) RETURNS TABLE(origin text,idle_seconds integer,
    absolute_seconds integer,memory_kib integer,passes integer,lanes integer,slots integer,key_check bytea)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
    SELECT p.origin,p.idle_seconds,p.absolute_seconds,p.memory_kib,p.passes,p.lanes,p.slots,
        auth_crypto.hmac(convert_to('ready'||chr(10)||p.id::text||chr(10)||nonce,'UTF8'),p.signing_key,'sha256')
    FROM identity.auth_policy p WHERE p.id=r AND p.enabled AND p.environment=env AND nonce ~ '^[a-f0-9]{64}$'
        AND identity.auth_acl_ok() AND identity.auth_owner_ok()
$$;
