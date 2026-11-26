-- IMP-P3-008 password recovery and the remaining IMP-P3-009 username
-- availability check. Forward-only: earlier migrations are unchanged. Numbered
-- after IMP-P3-012's 202611251201 so fresh and upgraded databases end with the
-- same auth_acl_ok(); the public endpoints use that migration's admission.
--
-- Recovery follows the email-verification design. An outbox intent is bound
-- to the address digest. A leased worker derives the link secret and stores
-- only its SHA-256 digest, and PostgreSQL controls expiry, guesses and single
-- use. The credential write and the revocation of every session run inside
-- one deypay_auth_owner function that only the isolated verifier can call,
-- with a signed, fresh assertion. Owner decisions of 2026-11-25: links go only
-- to verified addresses of accounts in the active release's allowed states,
-- live 30 minutes by default, and a newer request supersedes older links.

CREATE INDEX password_reset_intents_by_customer ON integrations.outbox_events(aggregate_id,created_at DESC,id)
    WHERE event_type='PASSWORD_RESET_REQUESTED' AND aggregate_type='CUSTOMER';
CREATE INDEX password_reset_challenges_by_customer ON identity.verification_challenges(user_id)
    WHERE challenge_type='PASSWORD_RESET' AND consumed_at IS NULL AND revoked_at IS NULL;
CREATE INDEX password_reset_email_retry_after ON integrations.jobs(available_at)
    WHERE job_type='RESET_PASSWORD_EMAIL' AND last_error='RATE_LIMITED';

-- Each recovery intent records the digest of the address it was requested for.
CREATE TRIGGER bind_password_reset_intent BEFORE INSERT ON integrations.outbox_events FOR EACH ROW
    WHEN (NEW.event_type='PASSWORD_RESET_REQUESTED' AND NEW.aggregate_type='CUSTOMER')
    EXECUTE FUNCTION integrations.bind_verification_email();

-- Internal: executable only by deypay_auth_owner (auth-grants.psql), from
-- identity.auth_request_reset after the release check. Applies the per-account
-- cooldown and hourly cap, and supersedes outstanding reset links.
CREATE FUNCTION identity.enqueue_password_reset(customer uuid, allowed text[], cooldown integer, hourly integer)
RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE stamp timestamptz; recent bigint; latest timestamptz; event_id uuid;
BEGIN
    IF cooldown NOT BETWEEN 1 AND 86400 OR hourly NOT BETWEEN 1 AND 100 OR cardinality(allowed)=0 THEN RETURN; END IF;
    PERFORM FROM identity.users u WHERE u.id=customer AND u.email_verified_at IS NOT NULL
        AND u.status=ANY(allowed) FOR NO KEY UPDATE;
    IF NOT FOUND THEN RETURN; END IF;
    stamp := clock_timestamp();
    SELECT count(*) FILTER(WHERE created_at > stamp-interval '1 hour'),max(created_at)
      INTO recent,latest FROM integrations.outbox_events WHERE aggregate_id=customer AND aggregate_type='CUSTOMER'
      AND event_type='PASSWORD_RESET_REQUESTED' AND created_at>=stamp-interval '1 day';
    IF latest>stamp-make_interval(secs=>cooldown) OR recent>=hourly THEN RETURN; END IF;
    UPDATE identity.verification_challenges SET revoked_at=stamp WHERE user_id=customer
      AND challenge_type='PASSWORD_RESET' AND consumed_at IS NULL AND revoked_at IS NULL;
    INSERT INTO integrations.outbox_events(aggregate_id,aggregate_type,event_type,payload,created_at)
      VALUES(customer,'CUSTOMER','PASSWORD_RESET_REQUESTED','{}',stamp) RETURNING id INTO event_id;
    INSERT INTO audit.audit_events(actor_type,action,resource_type,resource_id,correlation_id)
      VALUES('SYSTEM','PASSWORD_RESET_REQUESTED','CUSTOMER',customer,event_id);
END $$;

-- Internal: executable only by deypay_auth_owner, from identity.auth_reset_password.
-- Consumes one reset link and replaces the credential. A wrong secret spends one
-- guess; the last guess revokes the link. Returns the customer, or NULL.
CREATE FUNCTION identity.redeem_password_reset(selector uuid, digest_hex text, new_hash text,
    max_guesses integer, allowed text[])
RETURNS uuid LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE u identity.users; c identity.verification_challenges; stamp timestamptz;
BEGIN
    IF max_guesses NOT BETWEEN 1 AND 50 OR digest_hex !~ '^[a-f0-9]{64}$' OR left(new_hash,10)<>'$argon2id$'
       OR octet_length(new_hash)>128 OR cardinality(allowed)=0 THEN RETURN NULL; END IF;
    SELECT owner.* INTO u FROM identity.users owner JOIN identity.verification_challenges challenge
      ON challenge.user_id=owner.id WHERE challenge.id=selector AND challenge.challenge_type='PASSWORD_RESET'
      AND owner.email_verified_at IS NOT NULL AND owner.status=ANY(allowed) FOR NO KEY UPDATE OF owner;
    IF NOT FOUND THEN RETURN NULL; END IF;
    SELECT * INTO c FROM identity.verification_challenges WHERE id=selector FOR UPDATE;
    stamp := clock_timestamp();
    IF c.destination IS DISTINCT FROM u.email OR c.consumed_at IS NOT NULL OR c.revoked_at IS NOT NULL
       OR stamp>=c.expires_at OR c.attempt_count>=max_guesses THEN RETURN NULL; END IF;
    IF c.code_hash IS DISTINCT FROM digest_hex THEN
        UPDATE identity.verification_challenges SET attempt_count=attempt_count+1,
          revoked_at=CASE WHEN attempt_count+1>=max_guesses THEN stamp END WHERE id=selector;
        IF c.attempt_count+1>=max_guesses THEN
            INSERT INTO audit.audit_events(actor_type,action,resource_type,resource_id,correlation_id)
              VALUES('SYSTEM','PASSWORD_RESET_ATTEMPTS_EXHAUSTED','CUSTOMER',u.id,selector);
        END IF;
        RETURN NULL;
    END IF;
    UPDATE identity.password_credentials SET password_hash=new_hash,password_updated_at=stamp WHERE user_id=u.id;
    IF NOT FOUND THEN RETURN NULL; END IF;
    UPDATE identity.verification_challenges SET consumed_at=stamp WHERE id=selector;
    -- SEC-REC-004: recovery ends every other reset link and any outstanding
    -- financial challenge as well as the sessions revoked by the caller.
    UPDATE identity.verification_challenges SET revoked_at=stamp WHERE user_id=u.id AND id<>selector
      AND consumed_at IS NULL AND revoked_at IS NULL AND (challenge_type='PASSWORD_RESET' OR is_financial);
    INSERT INTO audit.audit_events(actor_type,actor_id,action,resource_type,resource_id,correlation_id)
      VALUES('CUSTOMER',u.id,'PASSWORD_RESET_COMPLETED','CUSTOMER',u.id,selector);
    RETURN u.id;
END $$;

-- Verifier capability: queue a reset email for an eligible verified address.
-- The outcome is not returned, so callers cannot learn whether an account exists.
CREATE FUNCTION identity.auth_request_reset(r uuid, address text, cooldown integer, hourly integer,
    issued bigint, tag bytea) RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE p identity.auth_policy; customer uuid;
BEGIN
    p:=identity.auth_active(r);
    PERFORM identity.auth_check(p,'request_reset',issued,
        ARRAY[identity.auth_hex(address),cooldown::text,hourly::text],tag);
    IF octet_length(address)>512 THEN RETURN; END IF;
    SELECT u.id INTO customer FROM identity.users u WHERE u.email=lower(btrim(address))
        AND u.email_verified_at IS NOT NULL AND u.status=ANY(p.allowed_states);
    IF FOUND THEN
        PERFORM identity.enqueue_password_reset(customer,p.allowed_states,cooldown,hourly);
    END IF;
END $$;

-- Verifier capability: redeem a reset link with a new Argon2id hash. The
-- credential write and the revocation of every session commit together.
CREATE FUNCTION identity.auth_reset_password(r uuid, selector uuid, digest text, new_hash text, guesses integer,
    issued bigint, tag bytea) RETURNS boolean LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE p identity.auth_policy; fields text[]; customer uuid;
BEGIN
    p:=identity.auth_active(r);
    fields:=ARRAY[selector::text,digest,identity.auth_hex(new_hash),guesses::text];
    PERFORM identity.auth_check(p,'reset_password',issued,fields,tag);
    customer:=identity.redeem_password_reset(selector,digest,new_hash,guesses,p.allowed_states);
    IF customer IS NULL THEN RETURN false; END IF;
    -- Recheck freshness after every potentially blocking lock, before committing effects.
    PERFORM identity.auth_check(p,'reset_password',issued,fields,tag);
    WITH retired AS (UPDATE identity.sessions SET revoked_at=greatest(created_at,clock_timestamp())
        WHERE user_id=customer AND revoked_at IS NULL RETURNING id)
    INSERT INTO audit.audit_events(actor_type,actor_id,action,resource_type,resource_id,reason)
        SELECT 'CUSTOMER',customer,'SESSION_REVOKED','SESSION',id,'PASSWORD_RESET' FROM retired;
    RETURN true;
END $$;

-- Preserve every existing ownership, search-path and privilege check. Expand the
-- verifier's allowlist for the two recovery capabilities and keep both runtime
-- roles away from the internal recovery helpers.
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
            OR has_schema_privilege(who,'identity','CREATE') OR has_schema_privilege(who,'auth_crypto','CREATE')
            OR has_function_privilege(who,'identity.enqueue_password_reset(uuid,text[],integer,integer)','EXECUTE')
            OR has_function_privilege(who,'identity.redeem_password_reset(uuid,text,text,integer,text[])','EXECUTE'))
    AND to_regprocedure('identity.auth_logout(uuid,text,bigint,bytea)') IS NOT NULL
    AND to_regprocedure('identity.auth_list_sessions(uuid,text,bigint,bytea)') IS NOT NULL
    AND to_regprocedure('identity.auth_revoke_session(uuid,text,uuid,bigint,bytea)') IS NOT NULL
    AND to_regprocedure('identity.auth_request_reset(uuid,text,integer,integer,bigint,bytea)') IS NOT NULL
    AND to_regprocedure('identity.auth_reset_password(uuid,uuid,text,text,integer,bigint,bytea)') IS NOT NULL
    AND (SELECT bool_and(
        pr.proowner=(SELECT relowner FROM pg_class WHERE oid='identity.auth_policy'::regclass)
        AND pr.proconfig @> ARRAY['search_path=pg_catalog, pg_temp']
        AND NOT EXISTS (SELECT FROM aclexplode(coalesce(pr.proacl,acldefault('f',pr.proowner))) a
            WHERE a.grantee=0 AND a.privilege_type='EXECUTE')
        AND has_function_privilege('deypay_auth',pr.oid,'EXECUTE') =
            (pr.proname IN ('auth_admit','auth_ready','auth_configuration','auth_credential','auth_reject','auth_issue',
                'auth_validate','auth_logout','auth_list_sessions','auth_revoke_session','auth_request_reset','auth_reset_password'))
        AND has_function_privilege('deypay_app',pr.oid,'EXECUTE') = (pr.proname IN ('auth_reader_ready','auth_principal')))
        FROM pg_proc pr WHERE pr.pronamespace='identity'::regnamespace
        AND (left(pr.proname,5)='auth_' OR pr.proname='protect_auth_audit'))
$$;

-- Leased worker operations for reset emails, mirroring the verification email
-- functions. Only the worker contacts the provider; only digests are stored.
CREATE FUNCTION integrations.dispatch_password_reset_email(max_tries integer) RETURNS void
LANGUAGE sql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
    WITH picked AS (SELECT id FROM integrations.outbox_events WHERE status='PENDING'
      AND aggregate_type='CUSTOMER' AND event_type='PASSWORD_RESET_REQUESTED'
      AND max_tries BETWEEN 1 AND 20 ORDER BY created_at,id LIMIT 1 FOR UPDATE SKIP LOCKED),
    published AS (UPDATE integrations.outbox_events e SET status='PUBLISHED',published_at=clock_timestamp()
      FROM picked WHERE e.id=picked.id RETURNING e.id)
    INSERT INTO integrations.jobs(id,job_type,execution_key,outbox_event_id,payload,max_attempts,available_at)
      SELECT id,'RESET_PASSWORD_EMAIL',id::text,id,'{}',max_tries,clock_timestamp() FROM published;
$$;

CREATE FUNCTION integrations.claim_password_reset_email(wanted_scope text, ttl integer, lease_seconds integer)
RETURNS TABLE(message_id uuid, lease uuid, customer uuid, address text, expiry bigint, attempt integer)
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE j integrations.jobs; e integrations.outbox_events; u identity.users; c identity.verification_challenges;
    stamp timestamptz; until_epoch bigint; failure text;
BEGIN
    IF ttl NOT BETWEEN 60 AND 3600 OR lease_seconds NOT BETWEEN 10 AND 180 THEN RETURN; END IF;
    -- Verification and reset emails share the sender and its provider rate limit.
    IF EXISTS (SELECT FROM integrations.jobs WHERE job_type='RESET_PASSWORD_EMAIL' AND
        last_error='RATE_LIMITED' AND available_at>clock_timestamp())
       OR EXISTS (SELECT FROM integrations.jobs WHERE job_type='VERIFY_EMAIL' AND
        last_error='RATE_LIMITED' AND available_at>clock_timestamp()) THEN RETURN; END IF;
    SELECT * INTO j FROM integrations.jobs WHERE job_type='RESET_PASSWORD_EMAIL' AND
      ((status='PENDING' AND available_at<=clock_timestamp()) OR (status='RUNNING' AND locked_until<=clock_timestamp()))
      AND NOT EXISTS (SELECT FROM integrations.outbox_events intent JOIN customers.email_verification_eligibility eligibility
          ON eligibility.user_id=intent.aggregate_id WHERE intent.id=jobs.outbox_event_id AND eligibility.scope<>wanted_scope)
      ORDER BY available_at,id LIMIT 1 FOR UPDATE SKIP LOCKED;
    IF NOT FOUND THEN RETURN; END IF;
    SELECT * INTO STRICT e FROM integrations.outbox_events WHERE id=j.outbox_event_id;
    SELECT owner.* INTO u FROM identity.users owner JOIN customers.email_verification_eligibility r ON r.user_id=owner.id
      WHERE owner.id=e.aggregate_id AND r.scope=wanted_scope FOR NO KEY UPDATE OF owner;
    SELECT * INTO c FROM identity.verification_challenges WHERE id=e.id;
    stamp := clock_timestamp();
    until_epoch := coalesce((j.payload->>'expiry')::bigint, floor(extract(epoch FROM stamp))::bigint+ttl);
    IF j.attempts>=j.max_attempts THEN failure:='RETRY_EXHAUSTED';
    ELSIF u.id IS NULL OR u.email_verified_at IS NULL
      OR u.status NOT IN ('CONTACT_VERIFIED','KYC_REQUIRED','KYC_PENDING','KYC_APPROVED')
      OR e.payload->>'address_digest' IS DISTINCT FROM encode(sha256(convert_to(u.email,'UTF8')),'hex')
      OR c.consumed_at IS NOT NULL OR c.revoked_at IS NOT NULL OR until_epoch<=extract(epoch FROM stamp)
      OR EXISTS (SELECT FROM integrations.outbox_events newer WHERE newer.aggregate_type='CUSTOMER'
        AND newer.aggregate_id=e.aggregate_id AND newer.event_type=e.event_type
        AND (newer.created_at,newer.id)>(e.created_at,e.id)) THEN failure:='INELIGIBLE';
    END IF;
    IF failure IS NOT NULL THEN
        UPDATE integrations.jobs SET status='DEAD_LETTER',last_error=failure,lease_token=NULL,locked_until=NULL WHERE id=j.id;
        RETURN;
    END IF;
    UPDATE integrations.jobs SET status='RUNNING',attempts=attempts+1,lease_token=gen_random_uuid(),
      locked_until=stamp+make_interval(secs=>lease_seconds),payload=payload||jsonb_build_object('expiry',until_epoch)
      WHERE id=j.id RETURNING * INTO j;
    RETURN QUERY SELECT j.id,j.lease_token,u.id,u.email,until_epoch,j.attempts;
END $$;

-- Called in the claim transaction before I/O. Only hashes and the expiry survive a crash.
CREATE FUNCTION integrations.prepare_password_reset_email(msg uuid, ownership uuid, digest_hex text, fingerprint text)
RETURNS boolean LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE j integrations.jobs; u identity.users;
BEGIN
    SELECT * INTO j FROM integrations.jobs WHERE id=msg AND job_type='RESET_PASSWORD_EMAIL' AND status='RUNNING'
      AND lease_token=ownership FOR UPDATE;
    IF NOT FOUND OR digest_hex !~ '^[0-9a-f]{64}$' OR fingerprint !~ '^[0-9a-f]{64}$' THEN RETURN false; END IF;
    IF j.payload ? 'fingerprint' THEN RETURN j.payload->>'fingerprint'=fingerprint; END IF;
    SELECT owner.* INTO STRICT u FROM identity.users owner JOIN integrations.outbox_events e ON e.aggregate_id=owner.id WHERE e.id=msg;
    INSERT INTO identity.verification_challenges(id,user_id,challenge_type,destination,code_hash,expires_at)
      VALUES(msg,u.id,'PASSWORD_RESET',u.email,digest_hex,to_timestamp((j.payload->>'expiry')::bigint));
    UPDATE integrations.jobs SET payload=payload||jsonb_build_object('fingerprint',fingerprint) WHERE id=msg;
    RETURN true;
END $$;

CREATE FUNCTION integrations.finish_password_reset_email(msg uuid, ownership uuid, outcome text, delay_seconds bigint, receipt uuid)
RETURNS boolean LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE j integrations.jobs;
BEGIN
    IF outcome IS NULL OR delay_seconds IS NULL OR (receipt IS NOT NULL AND outcome <> 'ACCEPTED')
      OR outcome NOT IN ('ACCEPTED','TRANSIENT','RATE_LIMITED','PERMANENT','MESSAGE_CHANGED','INVALID_ADDRESS')
      OR delay_seconds NOT BETWEEN 0 AND 86400 THEN RETURN false; END IF;
    UPDATE integrations.jobs SET
      status=CASE WHEN outcome='ACCEPTED' THEN 'SUCCEEDED'
        WHEN outcome IN ('TRANSIENT','RATE_LIMITED') AND attempts<max_attempts
         AND (payload->>'expiry')::bigint>extract(epoch FROM clock_timestamp())+delay_seconds THEN 'PENDING'
        ELSE 'DEAD_LETTER' END,
      available_at=clock_timestamp()+make_interval(secs=>delay_seconds::integer),lease_token=NULL,locked_until=NULL,
      payload=CASE WHEN receipt IS NULL THEN payload ELSE payload||jsonb_build_object('provider_message_id',receipt) END,
      last_error=CASE WHEN outcome<>'ACCEPTED' THEN outcome END
      WHERE id=msg AND job_type='RESET_PASSWORD_EMAIL' AND status='RUNNING' AND lease_token=ownership RETURNING * INTO j;
    IF NOT FOUND THEN RETURN false; END IF;
    IF j.status IN ('SUCCEEDED','DEAD_LETTER') THEN
        INSERT INTO audit.audit_events(actor_type,action,resource_type,resource_id,correlation_id)
          VALUES('SYSTEM',CASE WHEN j.status='SUCCEEDED' THEN 'PASSWORD_RESET_EMAIL_ACCEPTED' ELSE 'PASSWORD_RESET_EMAIL_FAILED' END,
          'EMAIL_DELIVERY',msg,msg);
    END IF;
    RETURN true;
END $$;

-- IMP-P3-009: advisory availability for the approved handle format. A reserved
-- or actively claimed key is unavailable; the two cases are indistinguishable.
-- Registration's unique keys remain the authority.
CREATE FUNCTION customers.username_available(handle text) RETURNS boolean
LANGUAGE sql STABLE STRICT SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
    SELECT handle ~ '^@?[0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_.]{3,30}$'
        AND handle !~ '^@?\.|\.$|\.\.'
        AND NOT EXISTS (SELECT FROM customers.username_keys k
            WHERE k.normalized_username=lower(ltrim(handle,'@')) AND k.reserved)
        AND NOT EXISTS (SELECT FROM customers.usernames n
            WHERE n.normalized_username=lower(ltrim(handle,'@')) AND n.is_active)
$$;

ALTER FUNCTION identity.enqueue_password_reset(uuid,text[],integer,integer) STRICT;
ALTER FUNCTION identity.redeem_password_reset(uuid,text,text,integer,text[]) STRICT;
ALTER FUNCTION integrations.dispatch_password_reset_email(integer) STRICT;
ALTER FUNCTION integrations.claim_password_reset_email(text,integer,integer) STRICT;
ALTER FUNCTION integrations.prepare_password_reset_email(uuid,uuid,text,text) STRICT;

REVOKE ALL ON FUNCTION identity.enqueue_password_reset(uuid,text[],integer,integer),
    identity.redeem_password_reset(uuid,text,text,integer,text[]),
    identity.auth_request_reset(uuid,text,integer,integer,bigint,bytea),
    identity.auth_reset_password(uuid,uuid,text,text,integer,bigint,bytea),
    integrations.dispatch_password_reset_email(integer),
    integrations.claim_password_reset_email(text,integer,integer),
    integrations.prepare_password_reset_email(uuid,uuid,text,text),
    integrations.finish_password_reset_email(uuid,uuid,text,bigint,uuid),
    customers.username_available(text) FROM PUBLIC;
