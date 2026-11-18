-- Reject misspelled reservation keys during policy import. Both policy and
-- customer claims must use the same Phase 2 canonical representation.
ALTER TABLE customers.username_keys ADD CONSTRAINT username_keys_canonical
    CHECK (normalized_username = lower(normalized_username)
           AND normalized_username ~ '^[^@[:space:]]+$');

CREATE TABLE customers.registration_consents (
    user_id uuid PRIMARY KEY REFERENCES identity.users,
    terms_version text NOT NULL CHECK (btrim(terms_version) <> ''),
    privacy_version text NOT NULL CHECK (btrim(privacy_version) <> ''),
    accepted_at timestamptz NOT NULL,
    recorded_at timestamptz NOT NULL DEFAULT now() CHECK (accepted_at <= recorded_at)
);
CREATE TRIGGER immutable_registration_consent BEFORE UPDATE OR DELETE OR TRUNCATE
    ON customers.registration_consents FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();

-- Registration is the only public operation with this write shape. Its caller
-- cannot choose an account status, a verification flag, or any financial record.
CREATE FUNCTION identity.register_customer(
    p_email text, p_phone text, p_username text, p_password_hash text,
    p_first_name text, p_last_name text, p_country_code text,
    p_terms_version text, p_privacy_version text, p_accepted_at_unix bigint
) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    new_user uuid;
BEGIN
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
        VALUES (new_user, p_terms_version, p_privacy_version, to_timestamp(p_accepted_at_unix));
    INSERT INTO audit.audit_events (actor_type, actor_id, action, resource_type, resource_id)
        VALUES ('CUSTOMER', new_user, 'CUSTOMER_REGISTERED', 'CUSTOMER', new_user);
    INSERT INTO integrations.outbox_events (aggregate_type, aggregate_id, event_type, payload)
        VALUES ('CUSTOMER', new_user, 'EMAIL_VERIFICATION_REQUESTED', '{}'),
               ('CUSTOMER', new_user, 'PHONE_VERIFICATION_REQUESTED', '{}');
    RETURN;
EXCEPTION WHEN SQLSTATE 'DP001' THEN
    RETURN;
END;
$$;
REVOKE ALL ON FUNCTION identity.register_customer(text,text,text,text,text,text,text,text,text,bigint) FROM PUBLIC;
