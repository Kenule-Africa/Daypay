CREATE OR REPLACE FUNCTION identity.register_customer(
    p_email text, p_phone text, p_username text, p_password_hash text,
    p_first_name text, p_last_name text, p_country_code text,
    p_terms_version text, p_privacy_version text, p_accept_terms boolean, p_ack_privacy boolean
) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    new_user uuid;
    request_id uuid := gen_random_uuid();
    selected customers.registration_legal_releases%ROWTYPE;
BEGIN
    SELECT r.* INTO selected FROM customers.current_registration_release c
        JOIN customers.registration_legal_releases r ON r.id=c.release_id
        WHERE c.scope='sandbox' FOR SHARE OF c;
    IF NOT FOUND THEN RAISE EXCEPTION USING ERRCODE='DP503', MESSAGE='Registration unavailable'; END IF;
    IF p_phone IS NULL OR p_accept_terms IS NOT TRUE OR p_ack_privacy IS NOT TRUE
       OR selected.terms_version IS DISTINCT FROM p_terms_version
       OR selected.privacy_version IS DISTINCT FROM p_privacy_version THEN
        RAISE EXCEPTION USING ERRCODE='DP422', MESSAGE='Invalid registration request';
    END IF;
    IF EXISTS (SELECT FROM identity.users WHERE phone_e164 = p_phone AND phone_verified_at IS NOT NULL) THEN
        RETURN;
    END IF;
    INSERT INTO identity.users (email, phone_e164, status)
        VALUES (lower(btrim(p_email)), p_phone, 'CONTACT_UNVERIFIED')
        ON CONFLICT (email) DO NOTHING RETURNING id INTO new_user;
    IF new_user IS NULL THEN RETURN; END IF;

    -- The Phase 2 key table, partial unique index and reservation FK serialize
    -- racing claims. The exception rolls back this entire registration.
    BEGIN
        INSERT INTO customers.usernames (user_id, username) VALUES (new_user, p_username);
    EXCEPTION WHEN unique_violation OR foreign_key_violation THEN
        RAISE EXCEPTION USING ERRCODE = 'DP001', MESSAGE = 'registration conflict';
    END;
    INSERT INTO customers.profiles (user_id, first_name, last_name, country_code)
        VALUES (new_user, p_first_name, p_last_name, p_country_code);
    INSERT INTO identity.password_credentials (user_id, password_hash) VALUES (new_user, p_password_hash);
    INSERT INTO customers.registration_consents (user_id, terms_version, privacy_version, accepted_at)
        VALUES (new_user, p_terms_version, p_privacy_version, statement_timestamp());
    INSERT INTO customers.registration_consent_evidence
        (user_id,terms_version,privacy_version,release_id,terms_accepted,privacy_acknowledged,request_id,channel)
        VALUES (new_user,p_terms_version,p_privacy_version,selected.id,p_accept_terms,p_ack_privacy,request_id,'REGISTRATION_API');
    INSERT INTO audit.audit_events (actor_type, actor_id, action, resource_type, resource_id, correlation_id)
        VALUES ('CUSTOMER', new_user, 'CUSTOMER_REGISTERED', 'CUSTOMER', new_user, request_id);
    INSERT INTO integrations.outbox_events (aggregate_type, aggregate_id, event_type, payload)
        VALUES ('CUSTOMER', new_user, 'EMAIL_VERIFICATION_REQUESTED', '{}');
    RETURN;
EXCEPTION WHEN SQLSTATE 'DP001' THEN
    RETURN;
END;
$$;

-- The verification routine sets this timestamp only after consuming a valid
-- email proof. Keep contact progression atomic with that write, including for
-- callers that began using the previous routine during an upgrade.
CREATE FUNCTION identity.promote_email_verified_contact() RETURNS trigger
LANGUAGE plpgsql SET search_path=pg_catalog,pg_temp AS $$
BEGIN
    NEW.status := 'CONTACT_VERIFIED';
    RETURN NEW;
END $$;
REVOKE ALL ON FUNCTION identity.promote_email_verified_contact() FROM PUBLIC;

CREATE TRIGGER mvp_email_contact_verified BEFORE UPDATE OF email_verified_at ON identity.users
    FOR EACH ROW WHEN (OLD.email_verified_at IS NULL AND NEW.email_verified_at IS NOT NULL
                       AND NEW.status IN ('REGISTERED','CONTACT_UNVERIFIED'))
    EXECUTE FUNCTION identity.promote_email_verified_contact();

-- The generic outbox writer also observes the MVP gate. Existing phone intents
-- keep their original payload, timestamps and status. A future release can drop
-- this constraint when it introduces the approved phone verification flow.
ALTER TABLE integrations.outbox_events ADD CONSTRAINT mvp_phone_verification_deferred
    CHECK (event_type <> 'PHONE_VERIFICATION_REQUESTED') NOT VALID;

-- Reconcile only contact states. Keep both proof timestamps, KYC decisions
-- and restricted accounts intact. Record the reason at migration time.
WITH aligned AS (
    UPDATE identity.users
    SET status=CASE WHEN email_verified_at IS NULL THEN 'CONTACT_UNVERIFIED' ELSE 'CONTACT_VERIFIED' END,
        updated_at=statement_timestamp()
    WHERE (email_verified_at IS NOT NULL AND status IN ('REGISTERED','CONTACT_UNVERIFIED'))
       OR (email_verified_at IS NULL AND status='CONTACT_VERIFIED')
    RETURNING id,status
)
INSERT INTO audit.audit_events(actor_type,action,resource_type,resource_id,reason)
    SELECT 'SYSTEM','MVP_CONTACT_STATUS_ALIGNED','CUSTOMER',id,
           CASE WHEN status='CONTACT_VERIFIED' THEN 'Previously verified email' ELSE 'Email confirmation required' END
    FROM aligned;

ALTER TABLE identity.users ADD CONSTRAINT mvp_contact_verified_requires_email
    CHECK (status <> 'CONTACT_VERIFIED' OR email_verified_at IS NOT NULL);
