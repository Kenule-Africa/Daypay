CREATE TABLE identity.users (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    email text NOT NULL CHECK (email = lower(btrim(email)) AND email <> '') UNIQUE,
    email_verified_at timestamptz,
    phone_e164 text CHECK (phone_e164 ~ '^\+[1-9][0-9]{1,14}$'),
    phone_verified_at timestamptz,
    status text NOT NULL CHECK (status IN ('REGISTERED','CONTACT_UNVERIFIED','CONTACT_VERIFIED',
        'KYC_REQUIRED','KYC_PENDING','KYC_APPROVED','KYC_REJECTED','RESTRICTED','SUSPENDED','CLOSED')),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    closed_at timestamptz,
    CHECK (updated_at >= created_at AND closed_at >= created_at),
    CHECK (phone_verified_at IS NULL OR (phone_e164 IS NOT NULL AND phone_verified_at >= created_at)),
    CHECK (email_verified_at >= created_at),
    CHECK ((status = 'CLOSED') = (closed_at IS NOT NULL))
);
CREATE UNIQUE INDEX users_verified_phone ON identity.users(phone_e164) WHERE phone_verified_at IS NOT NULL;

CREATE TABLE customers.addresses (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), user_id uuid NOT NULL REFERENCES identity.users,
    address_type text NOT NULL, line_1 text NOT NULL, line_2 text, city text NOT NULL,
    state_region text, postal_code text, country_code text NOT NULL CHECK (country_code ~ '^[A-Z]{2}$'),
    created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (user_id, id)
);
CREATE TABLE customers.profiles (
    user_id uuid PRIMARY KEY REFERENCES identity.users, first_name text NOT NULL, last_name text NOT NULL,
    date_of_birth date, country_code text NOT NULL CHECK (country_code ~ '^[A-Z]{2}$'),
    residential_address_id uuid, preferred_currency text,
    created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (user_id, residential_address_id) REFERENCES customers.addresses(user_id, id)
);
-- Policy owners supply reserved handles; a single key serializes reservation and active claims.
CREATE TABLE customers.username_keys (
    normalized_username text PRIMARY KEY,
    reserved boolean NOT NULL DEFAULT false,
    UNIQUE (normalized_username,reserved)
);
CREATE FUNCTION customers.register_username_key() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO customers.username_keys(normalized_username)
        VALUES (lower(ltrim(NEW.username,'@'))) ON CONFLICT (normalized_username) DO NOTHING;
    RETURN NEW;
END;
$$;
CREATE TABLE customers.usernames (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), user_id uuid NOT NULL REFERENCES identity.users,
    username text NOT NULL CHECK (username ~ '^@?[^@[:space:]]+$'),
    normalized_username text GENERATED ALWAYS AS (lower(ltrim(username,'@'))) STORED,
    active_reservation_marker boolean GENERATED ALWAYS AS (CASE WHEN is_active THEN false END) STORED,
    is_active boolean NOT NULL DEFAULT true, created_at timestamptz NOT NULL DEFAULT now(), retired_at timestamptz,
    FOREIGN KEY (normalized_username,active_reservation_marker) REFERENCES customers.username_keys(normalized_username,reserved),
    CHECK (is_active = (retired_at IS NULL) AND retired_at >= created_at)
);
CREATE TRIGGER register_key BEFORE INSERT OR UPDATE OF username ON customers.usernames
    FOR EACH ROW EXECUTE FUNCTION customers.register_username_key();
CREATE UNIQUE INDEX usernames_active_key ON customers.usernames(normalized_username) WHERE is_active;
CREATE UNIQUE INDEX usernames_active_user ON customers.usernames(user_id) WHERE is_active;
CREATE TABLE customers.preferences (
    user_id uuid PRIMARY KEY REFERENCES identity.users, phone_discoverable boolean NOT NULL DEFAULT false,
    email_notifications boolean NOT NULL, push_notifications boolean NOT NULL,
    marketing_consent boolean NOT NULL DEFAULT false, updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE identity.password_credentials (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), user_id uuid UNIQUE NOT NULL REFERENCES identity.users,
    password_hash text NOT NULL CHECK (password_hash LIKE '$argon2id$%'),
    password_updated_at timestamptz NOT NULL DEFAULT now(), created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE identity.devices (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), user_id uuid NOT NULL REFERENCES identity.users,
    device_name text NOT NULL, device_fingerprint_hash text, last_seen_at timestamptz NOT NULL DEFAULT now(),
    trusted_at timestamptz, created_at timestamptz NOT NULL DEFAULT now(), UNIQUE (user_id,id)
);
CREATE TABLE identity.sessions (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), user_id uuid NOT NULL REFERENCES identity.users,
    token_hash text NOT NULL UNIQUE CHECK (token_hash <> ''), device_id uuid, ip_address inet, user_agent text,
    created_at timestamptz NOT NULL DEFAULT now(), last_activity_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL, absolute_expires_at timestamptz NOT NULL, revoked_at timestamptz,
    UNIQUE (user_id,id), FOREIGN KEY (user_id,device_id) REFERENCES identity.devices(user_id,id),
    CHECK (last_activity_at >= created_at AND expires_at > created_at AND expires_at <= absolute_expires_at),
    CHECK (revoked_at >= created_at)
);
CREATE FUNCTION identity.reject_auth_rewrite() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'authenticated identity, binding or consumption is immutable' USING ERRCODE = '23514';
END;
$$;
CREATE TRIGGER preserve_session_lifetime BEFORE UPDATE ON identity.sessions FOR EACH ROW
    WHEN ((OLD.user_id,OLD.created_at,OLD.absolute_expires_at) IS DISTINCT FROM (NEW.user_id,NEW.created_at,NEW.absolute_expires_at))
    EXECUTE FUNCTION identity.reject_auth_rewrite();
CREATE INDEX sessions_active_user ON identity.sessions(user_id) WHERE revoked_at IS NULL;
CREATE INDEX sessions_expiry ON identity.sessions(expires_at);
CREATE TABLE identity.verification_challenges (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), user_id uuid NOT NULL REFERENCES identity.users,
    challenge_type text NOT NULL, destination text, code_hash text NOT NULL CHECK (code_hash <> ''),
    session_id uuid, operation_type text, binding_version integer, binding_digest text,
    created_at timestamptz NOT NULL DEFAULT now(), expires_at timestamptz NOT NULL,
    consumed_at timestamptz, revoked_at timestamptz, attempt_count integer NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    FOREIGN KEY (user_id,session_id) REFERENCES identity.sessions(user_id,id),
    is_financial boolean GENERATED ALWAYS AS (operation_type IS NOT NULL) STORED,
    UNIQUE (user_id,id,is_financial,consumed_at), CHECK (expires_at > created_at AND consumed_at >= created_at AND consumed_at < expires_at),
    CHECK ((operation_type IS NULL AND binding_version IS NULL AND binding_digest IS NULL)
        OR (operation_type IS NOT NULL AND session_id IS NOT NULL AND binding_version IS NOT NULL
        AND binding_version > 0 AND binding_digest IS NOT NULL AND binding_digest <> ''))
);
CREATE TABLE identity.transaction_proofs (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), user_id uuid NOT NULL REFERENCES identity.users,
    challenge_id uuid NOT NULL UNIQUE, proof_hash text NOT NULL UNIQUE CHECK (proof_hash <> ''),
    created_at timestamptz NOT NULL DEFAULT now(), expires_at timestamptz NOT NULL, revoked_at timestamptz,
    consumed_at timestamptz, accepted_transaction_id uuid UNIQUE,
    financial_challenge boolean NOT NULL DEFAULT true CHECK (financial_challenge), challenge_completed_at timestamptz NOT NULL,
    FOREIGN KEY (user_id,challenge_id,financial_challenge,challenge_completed_at)
        REFERENCES identity.verification_challenges(user_id,id,is_financial,consumed_at),
    CHECK (expires_at > created_at AND consumed_at >= created_at AND consumed_at < expires_at),
    CHECK ((consumed_at IS NULL) = (accepted_transaction_id IS NULL))
);
CREATE TRIGGER immutable_challenge_binding BEFORE UPDATE ON identity.verification_challenges FOR EACH ROW
    WHEN ((OLD.user_id,OLD.session_id,OLD.challenge_type,OLD.operation_type,OLD.binding_version,OLD.binding_digest,OLD.expires_at)
        IS DISTINCT FROM (NEW.user_id,NEW.session_id,NEW.challenge_type,NEW.operation_type,NEW.binding_version,NEW.binding_digest,NEW.expires_at)
        OR (OLD.consumed_at IS NOT NULL AND OLD.consumed_at IS DISTINCT FROM NEW.consumed_at)
        OR (OLD.revoked_at IS NOT NULL AND OLD.revoked_at IS DISTINCT FROM NEW.revoked_at))
    EXECUTE FUNCTION identity.reject_auth_rewrite();
CREATE TRIGGER immutable_transaction_proof BEFORE UPDATE ON identity.transaction_proofs FOR EACH ROW
    WHEN ((OLD.user_id,OLD.challenge_id,OLD.proof_hash,OLD.expires_at) IS DISTINCT FROM
          (NEW.user_id,NEW.challenge_id,NEW.proof_hash,NEW.expires_at)
        OR (OLD.consumed_at IS NOT NULL AND (OLD.consumed_at,OLD.accepted_transaction_id) IS DISTINCT FROM
          (NEW.consumed_at,NEW.accepted_transaction_id))
        OR (OLD.revoked_at IS NOT NULL AND OLD.revoked_at IS DISTINCT FROM NEW.revoked_at))
    EXECUTE FUNCTION identity.reject_auth_rewrite();
