-- The outbox keeps the destination binding, never the address or a bearer credential.
CREATE INDEX email_intents_by_customer ON integrations.outbox_events(aggregate_id,created_at DESC,id)
    WHERE event_type='EMAIL_VERIFICATION_REQUESTED' AND aggregate_type='CUSTOMER';
CREATE INDEX email_challenges_by_customer ON identity.verification_challenges(user_id)
    WHERE challenge_type='EMAIL_VERIFICATION' AND consumed_at IS NULL AND revoked_at IS NULL;
CREATE INDEX verification_email_retry_after ON integrations.jobs(available_at)
    WHERE job_type='VERIFY_EMAIL' AND last_error='RATE_LIMITED';

CREATE FUNCTION integrations.bind_verification_email() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
BEGIN
    SELECT jsonb_build_object('address_digest',encode(sha256(convert_to(u.email,'UTF8')),'hex'))
      INTO STRICT NEW.payload FROM identity.users u WHERE u.id=NEW.aggregate_id;
    RETURN NEW;
END $$;
REVOKE ALL ON FUNCTION integrations.bind_verification_email() FROM PUBLIC;
CREATE TRIGGER bind_email_intent BEFORE INSERT ON integrations.outbox_events FOR EACH ROW
    WHEN (NEW.event_type='EMAIL_VERIFICATION_REQUESTED' AND NEW.aggregate_type='CUSTOMER')
    EXECUTE FUNCTION integrations.bind_verification_email();
-- Adopt only still-unverified legacy registration intents whose address has never been updated.
UPDATE integrations.outbox_events e SET payload=jsonb_build_object('address_digest',encode(sha256(convert_to(u.email,'UTF8')),'hex'))
 FROM identity.users u WHERE e.aggregate_id=u.id AND e.aggregate_type='CUSTOMER'
 AND e.event_type='EMAIL_VERIFICATION_REQUESTED' AND e.status='PENDING'
 AND u.email_verified_at IS NULL AND u.created_at=u.updated_at AND e.payload='{}';

CREATE TABLE identity.email_verification_traffic (
    operation text PRIMARY KEY CHECK(operation IN ('resend','verify')),
    minute timestamptz NOT NULL, hits integer NOT NULL CHECK(hits>0)
);
CREATE FUNCTION identity.admit_email_request(op text, cap integer) RETURNS boolean
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE bucket timestamptz := date_trunc('minute',clock_timestamp());
BEGIN
    IF cap NOT BETWEEN 1 AND 100000 THEN RETURN false; END IF;
    INSERT INTO identity.email_verification_traffic AS t VALUES(op,bucket,1)
      ON CONFLICT(operation) DO UPDATE SET minute=excluded.minute,
      hits=CASE WHEN t.minute=excluded.minute THEN t.hits+1 ELSE 1 END
      WHERE t.minute<>excluded.minute OR t.hits<cap;
    RETURN FOUND;
END $$;

CREATE VIEW customers.email_verification_eligibility AS
 SELECT evidence.user_id,release.scope FROM customers.registration_consent_evidence evidence
 JOIN customers.registration_legal_releases release ON release.id=evidence.release_id;

CREATE FUNCTION identity.request_verification_email(address text, wanted_scope text, cooldown integer, hourly integer)
RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE customer uuid; stamp timestamptz; recent bigint; latest timestamptz; event_id uuid;
BEGIN
    IF cooldown NOT BETWEEN 1 AND 86400 OR hourly NOT BETWEEN 1 AND 100 THEN RETURN; END IF;
    SELECT u.id INTO customer FROM identity.users u JOIN customers.email_verification_eligibility r ON r.user_id=u.id
      WHERE u.email=lower(btrim(address)) AND r.scope=wanted_scope AND u.email_verified_at IS NULL
       AND u.status IN ('REGISTERED','CONTACT_UNVERIFIED') FOR UPDATE OF u;
    IF NOT FOUND THEN RETURN; END IF;
    stamp := clock_timestamp();
    SELECT count(*) FILTER(WHERE created_at > stamp-interval '1 hour'),max(created_at)
      INTO recent,latest FROM integrations.outbox_events WHERE aggregate_id=customer AND aggregate_type='CUSTOMER'
      AND event_type='EMAIL_VERIFICATION_REQUESTED' AND created_at>=stamp-interval '1 day';
    IF latest>stamp-make_interval(secs=>cooldown) OR recent>=hourly THEN RETURN; END IF;
    UPDATE identity.verification_challenges SET revoked_at=stamp WHERE user_id=customer
      AND challenge_type='EMAIL_VERIFICATION' AND consumed_at IS NULL AND revoked_at IS NULL;
    INSERT INTO integrations.outbox_events(aggregate_id,aggregate_type,event_type,payload,created_at)
      VALUES(customer,'CUSTOMER','EMAIL_VERIFICATION_REQUESTED','{}',stamp) RETURNING id INTO event_id;
    INSERT INTO audit.audit_events(actor_type,action,resource_type,resource_id,correlation_id)
      VALUES('SYSTEM','EMAIL_VERIFICATION_REQUESTED','CUSTOMER',customer,event_id);
END $$;

CREATE FUNCTION identity.consume_verification_email(selector uuid, digest_hex text, wanted_scope text, max_guesses integer)
RETURNS boolean LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE u identity.users; c identity.verification_challenges; stamp timestamptz;
BEGIN
    IF max_guesses NOT BETWEEN 1 AND 50 OR digest_hex !~ '^[a-f0-9]{64}$' THEN RETURN false; END IF;
    SELECT owner.* INTO u FROM identity.users owner JOIN identity.verification_challenges challenge ON challenge.user_id=owner.id
      JOIN customers.email_verification_eligibility r ON r.user_id=owner.id WHERE challenge.id=selector
      AND r.scope=wanted_scope AND challenge.challenge_type='EMAIL_VERIFICATION'
      AND owner.email_verified_at IS NULL AND owner.status IN ('REGISTERED','CONTACT_UNVERIFIED') FOR UPDATE OF owner;
    IF NOT FOUND THEN RETURN false; END IF;
    SELECT * INTO c FROM identity.verification_challenges WHERE id=selector FOR UPDATE;
    stamp := clock_timestamp();
    IF c.destination IS DISTINCT FROM u.email OR c.consumed_at IS NOT NULL OR c.revoked_at IS NOT NULL
       OR stamp>=c.expires_at OR c.attempt_count>=max_guesses THEN RETURN false; END IF;
    IF c.code_hash IS DISTINCT FROM digest_hex THEN
        UPDATE identity.verification_challenges SET attempt_count=attempt_count+1,
          revoked_at=CASE WHEN attempt_count+1>=max_guesses THEN stamp END WHERE id=selector;
        IF c.attempt_count+1>=max_guesses THEN
            INSERT INTO audit.audit_events(actor_type,action,resource_type,resource_id,correlation_id)
              VALUES('SYSTEM','EMAIL_VERIFICATION_ATTEMPTS_EXHAUSTED','CUSTOMER',u.id,selector);
        END IF;
        RETURN false;
    END IF;
    UPDATE identity.verification_challenges SET consumed_at=stamp WHERE id=selector;
    UPDATE identity.users SET email_verified_at=stamp,updated_at=stamp WHERE id=u.id;
    INSERT INTO audit.audit_events(actor_type,actor_id,action,resource_type,resource_id,correlation_id)
      VALUES('CUSTOMER',u.id,'EMAIL_VERIFIED','CUSTOMER',u.id,selector);
    RETURN true;
END $$;

-- A published outbox event means it has a durable job. Only the leased worker contacts Resend.
CREATE FUNCTION integrations.dispatch_verification_email(max_tries integer) RETURNS void
LANGUAGE sql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
    WITH picked AS (SELECT id FROM integrations.outbox_events WHERE status='PENDING'
      AND aggregate_type='CUSTOMER' AND event_type='EMAIL_VERIFICATION_REQUESTED'
      AND max_tries BETWEEN 1 AND 20 ORDER BY created_at,id LIMIT 1 FOR UPDATE SKIP LOCKED),
    published AS (UPDATE integrations.outbox_events e SET status='PUBLISHED',published_at=clock_timestamp()
      FROM picked WHERE e.id=picked.id RETURNING e.id)
    INSERT INTO integrations.jobs(id,job_type,execution_key,outbox_event_id,payload,max_attempts,available_at)
      SELECT id,'VERIFY_EMAIL',id::text,id,'{}',max_tries,clock_timestamp() FROM published;
$$;

CREATE FUNCTION integrations.claim_verification_email(wanted_scope text, ttl integer, lease_seconds integer)
RETURNS TABLE(message_id uuid, lease uuid, customer uuid, address text, expiry bigint, attempt integer)
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE j integrations.jobs; e integrations.outbox_events; u identity.users; c identity.verification_challenges;
    stamp timestamptz; until_epoch bigint; failure text;
BEGIN
    IF ttl NOT BETWEEN 60 AND 3600 OR lease_seconds NOT BETWEEN 10 AND 180 THEN RETURN; END IF;
    -- A provider rate limit covers the sender, including other queued messages.
    IF EXISTS (SELECT FROM integrations.jobs WHERE job_type='VERIFY_EMAIL' AND
        last_error='RATE_LIMITED' AND available_at>clock_timestamp()) THEN RETURN; END IF;
    SELECT * INTO j FROM integrations.jobs WHERE job_type='VERIFY_EMAIL' AND
      ((status='PENDING' AND available_at<=clock_timestamp()) OR (status='RUNNING' AND locked_until<=clock_timestamp()))
      AND NOT EXISTS (SELECT FROM integrations.outbox_events intent JOIN customers.email_verification_eligibility eligibility
          ON eligibility.user_id=intent.aggregate_id WHERE intent.id=jobs.outbox_event_id AND eligibility.scope<>wanted_scope)
      ORDER BY available_at,id LIMIT 1 FOR UPDATE SKIP LOCKED;
    IF NOT FOUND THEN RETURN; END IF;
    SELECT * INTO STRICT e FROM integrations.outbox_events WHERE id=j.outbox_event_id;
    SELECT owner.* INTO u FROM identity.users owner JOIN customers.email_verification_eligibility r ON r.user_id=owner.id
      WHERE owner.id=e.aggregate_id AND r.scope=wanted_scope FOR UPDATE OF owner;
    SELECT * INTO c FROM identity.verification_challenges WHERE id=e.id;
    stamp := clock_timestamp();
    until_epoch := coalesce((j.payload->>'expiry')::bigint, floor(extract(epoch FROM stamp))::bigint+ttl);
    IF j.attempts>=j.max_attempts THEN failure:='RETRY_EXHAUSTED';
    ELSIF u.id IS NULL OR u.status NOT IN ('REGISTERED','CONTACT_UNVERIFIED') OR u.email_verified_at IS NOT NULL
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
CREATE FUNCTION integrations.prepare_verification_email(msg uuid, ownership uuid, digest_hex text, fingerprint text)
RETURNS boolean LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE j integrations.jobs; u identity.users;
BEGIN
    SELECT * INTO j FROM integrations.jobs WHERE id=msg AND job_type='VERIFY_EMAIL' AND status='RUNNING'
      AND lease_token=ownership FOR UPDATE;
    IF NOT FOUND OR digest_hex !~ '^[0-9a-f]{64}$' OR fingerprint !~ '^[0-9a-f]{64}$' THEN RETURN false; END IF;
    IF j.payload ? 'fingerprint' THEN RETURN j.payload->>'fingerprint'=fingerprint; END IF;
    SELECT owner.* INTO STRICT u FROM identity.users owner JOIN integrations.outbox_events e ON e.aggregate_id=owner.id WHERE e.id=msg;
    INSERT INTO identity.verification_challenges(id,user_id,challenge_type,destination,code_hash,expires_at)
      VALUES(msg,u.id,'EMAIL_VERIFICATION',u.email,digest_hex,to_timestamp((j.payload->>'expiry')::bigint));
    UPDATE integrations.jobs SET payload=payload||jsonb_build_object('fingerprint',fingerprint) WHERE id=msg;
    RETURN true;
END $$;

CREATE FUNCTION integrations.finish_verification_email(msg uuid, ownership uuid, outcome text, delay_seconds bigint, receipt uuid)
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
      WHERE id=msg AND job_type='VERIFY_EMAIL' AND status='RUNNING' AND lease_token=ownership RETURNING * INTO j;
    IF NOT FOUND THEN RETURN false; END IF;
    IF j.status IN ('SUCCEEDED','DEAD_LETTER') THEN
        INSERT INTO audit.audit_events(actor_type,action,resource_type,resource_id,correlation_id)
          VALUES('SYSTEM',CASE WHEN j.status='SUCCEEDED' THEN 'VERIFICATION_EMAIL_ACCEPTED' ELSE 'VERIFICATION_EMAIL_FAILED' END,
          'EMAIL_DELIVERY',msg,msg);
    END IF;
    RETURN true;
END $$;

REVOKE ALL ON FUNCTION identity.admit_email_request(text,integer),
 identity.request_verification_email(text,text,integer,integer),identity.consume_verification_email(uuid,text,text,integer),
 integrations.dispatch_verification_email(integer),integrations.claim_verification_email(text,integer,integer),
 integrations.prepare_verification_email(uuid,uuid,text,text),integrations.finish_verification_email(uuid,uuid,text,bigint,uuid) FROM PUBLIC;

ALTER FUNCTION identity.admit_email_request(text,integer) STRICT;
ALTER FUNCTION identity.request_verification_email(text,text,integer,integer) STRICT;
ALTER FUNCTION identity.consume_verification_email(uuid,text,text,integer) STRICT;
ALTER FUNCTION integrations.dispatch_verification_email(integer) STRICT;
ALTER FUNCTION integrations.claim_verification_email(text,integer,integer) STRICT;
ALTER FUNCTION integrations.prepare_verification_email(uuid,uuid,text,text) STRICT;
