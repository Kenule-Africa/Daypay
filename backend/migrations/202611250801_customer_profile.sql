-- IMP-P3-009 / IMP-P3-010. The owner approved 202611250601 pending Codex's
-- sequencing; renumbered after the merged 202611250701 session controls.

-- Owner-approved username format (2026-11-25): ASCII letters, digits, underscore
-- and period, 3-30 characters, no leading, trailing or doubled period. Explicit
-- character lists keep the rule independent of database collation, and lower()
-- folds ASCII identically in every locale. NOT VALID leaves existing rows as
-- they are; every new active claim must comply, and retirement stays possible.
ALTER TABLE customers.usernames ADD CONSTRAINT usernames_approved_format CHECK (
    NOT is_active OR (
        username ~ '^@?[0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_.]{3,30}$'
        AND username !~ '^@?\.|\.$|\.\.'
    )
) NOT VALID;

-- IMP-P3-010. The runtime receives only the fixed operations below; it has no
-- direct read or write access to profile, preference or image rows. Every
-- operation is scoped to the customer UUID supplied by the authenticated API
-- boundary, and every change appends its audit event in the same transaction.

-- Owner-approved defaults (2026-11-25) for optional notifications. Security
-- and service messages do not depend on them. Phone discovery and marketing
-- stay opt-in.
ALTER TABLE customers.preferences
    ALTER COLUMN email_notifications SET DEFAULT true,
    ALTER COLUMN push_notifications SET DEFAULT false;

-- One current, re-encoded image per customer, stored apart from KYC documents.
-- Owner-approved contract (2026-11-25): JPEG output, longest side <= 512 px,
-- at most 150 KB. The version changes on every replacement and is the ETag.
CREATE TABLE customers.profile_images (
    user_id uuid PRIMARY KEY REFERENCES identity.users,
    version uuid NOT NULL UNIQUE,
    content_type text NOT NULL CHECK (content_type = 'image/jpeg'),
    width integer NOT NULL CHECK (width BETWEEN 1 AND 512),
    height integer NOT NULL CHECK (height BETWEEN 1 AND 512),
    content bytea NOT NULL CHECK (
        octet_length(content) BETWEEN 4 AND 153600
        AND substring(content FROM 1 FOR 3) = '\xffd8ff'::bytea
        AND substring(content FROM octet_length(content) - 1) = '\xffd9'::bytea
    ),
    updated_at timestamptz NOT NULL DEFAULT now()
);
-- The JPEG payload is already compressed.
ALTER TABLE customers.profile_images ALTER COLUMN content SET STORAGE EXTERNAL;

CREATE FUNCTION customers.utc_text(stamp timestamptz) RETURNS text
LANGUAGE sql STABLE SET search_path = pg_catalog, pg_temp AS $$
    SELECT to_char(stamp AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"')
$$;

-- A write needs an account that may hold a customer session. The shared row
-- lock orders the write against a concurrent account status change.
CREATE FUNCTION customers.lock_profile_owner(p_user uuid) RETURNS text
LANGUAGE plpgsql SET search_path = pg_catalog, pg_temp AS $$
DECLARE state text;
BEGIN
    SELECT u.status INTO state FROM identity.users u
        WHERE u.id = p_user AND u.email_verified_at IS NOT NULL
          AND u.status IN ('CONTACT_VERIFIED','KYC_REQUIRED','KYC_PENDING','KYC_APPROVED')
        FOR SHARE;
    RETURN state;
END $$;

-- Owner decision (2026-11-25): names stay editable until identity
-- verification starts, so verified identity cannot drift.
CREATE FUNCTION customers.names_editable(p_user uuid, p_status text) RETURNS boolean
LANGUAGE sql STABLE SET search_path = pg_catalog, pg_temp AS $$
    SELECT p_status IN ('CONTACT_VERIFIED','KYC_REQUIRED')
       AND NOT EXISTS (SELECT FROM compliance.kyc_cases k WHERE k.user_id = p_user AND k.status <> 'DRAFT')
$$;

-- Mirrors the API rule: trimmed, 1-100 characters, no control, format,
-- separator or non-space whitespace characters.
CREATE FUNCTION customers.valid_profile_name(p_name text) RETURNS boolean
LANGUAGE sql IMMUTABLE SET search_path = pg_catalog, pg_temp AS $$
    SELECT p_name = btrim(p_name) AND char_length(p_name) BETWEEN 1 AND 100
       AND p_name !~ '[\x01-\x1f\x7f-\xa0­؀-؅؜۝܏ ᠎ -‏ -  -⁤⁦-⁯　﻿￹-￻]'
$$;

CREATE FUNCTION customers.customer_profile(p_user uuid)
RETURNS TABLE (username text, first_name text, last_name text, email text, email_verified boolean,
    phone text, phone_verified boolean, country_code text, account_status text, names_editable boolean,
    image_version uuid, image_width integer, image_height integer, image_updated_at text,
    created_at text, updated_at text)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
    SELECT ltrim(n.username, '@'), p.first_name, p.last_name, u.email, u.email_verified_at IS NOT NULL,
        u.phone_e164, u.phone_verified_at IS NOT NULL, p.country_code, u.status,
        customers.names_editable(u.id, u.status),
        i.version, i.width, i.height, customers.utc_text(i.updated_at),
        customers.utc_text(u.created_at), customers.utc_text(p.updated_at)
    FROM identity.users u
    JOIN customers.profiles p ON p.user_id = u.id
    LEFT JOIN customers.usernames n ON n.user_id = u.id AND n.is_active
    LEFT JOIN customers.profile_images i ON i.user_id = u.id
    WHERE u.id = p_user
$$;

-- NULL leaves a name unchanged. Returns 'updated', 'locked' or 'ineligible'.
CREATE FUNCTION customers.rename_customer(p_user uuid, p_first_name text, p_last_name text) RETURNS text
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    state text;
    before customers.profiles%ROWTYPE;
    changed text[] := '{}';
BEGIN
    IF num_nonnulls(p_first_name, p_last_name) = 0
       OR NOT coalesce(customers.valid_profile_name(p_first_name), true)
       OR NOT coalesce(customers.valid_profile_name(p_last_name), true) THEN
        RAISE EXCEPTION 'Invalid profile name' USING ERRCODE = '22023';
    END IF;
    state := customers.lock_profile_owner(p_user);
    IF state IS NULL THEN RETURN 'ineligible'; END IF;
    -- A concurrent KYC submission updates its case row, so it orders after this check.
    PERFORM FROM compliance.kyc_cases k WHERE k.user_id = p_user FOR SHARE;
    IF NOT customers.names_editable(p_user, state) THEN RETURN 'locked'; END IF;
    SELECT * INTO before FROM customers.profiles p WHERE p.user_id = p_user FOR UPDATE;
    IF NOT FOUND THEN RETURN 'ineligible'; END IF;
    IF p_first_name IS DISTINCT FROM before.first_name AND p_first_name IS NOT NULL THEN
        changed := changed || 'first_name'::text;
    END IF;
    IF p_last_name IS DISTINCT FROM before.last_name AND p_last_name IS NOT NULL THEN
        changed := changed || 'last_name'::text;
    END IF;
    IF cardinality(changed) > 0 THEN
        UPDATE customers.profiles p
            SET first_name = coalesce(p_first_name, p.first_name), last_name = coalesce(p_last_name, p.last_name),
                updated_at = greatest(p.updated_at, now())
            WHERE p.user_id = p_user;
        -- Field names only: the audit trail does not copy personal data.
        INSERT INTO audit.audit_events(actor_type, actor_id, action, resource_type, resource_id, metadata)
            VALUES ('CUSTOMER', p_user, 'PROFILE_NAME_CHANGED', 'CUSTOMER', p_user,
                    jsonb_build_object('fields', to_jsonb(changed)));
    END IF;
    RETURN 'updated';
END $$;

CREATE FUNCTION customers.customer_preferences(p_user uuid)
RETURNS TABLE (phone_discoverable boolean, email_notifications boolean, push_notifications boolean,
    marketing_consent boolean, updated_at text)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
    -- The same values as the column defaults for a customer without a row.
    SELECT coalesce(p.phone_discoverable, false), coalesce(p.email_notifications, true),
        coalesce(p.push_notifications, false), coalesce(p.marketing_consent, false),
        customers.utc_text(p.updated_at)
    FROM identity.users u LEFT JOIN customers.preferences p ON p.user_id = u.id
    WHERE u.id = p_user
$$;

-- NULL leaves a preference unchanged. Returns 'updated', 'phone_unverified' or 'ineligible'.
CREATE FUNCTION customers.update_customer_preferences(p_user uuid, p_phone_discoverable boolean,
    p_email_notifications boolean, p_push_notifications boolean, p_marketing_consent boolean) RETURNS text
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    phone_verified boolean;
    before customers.preferences%ROWTYPE;
    changes jsonb := '{}';
    wording text;
BEGIN
    IF num_nonnulls(p_phone_discoverable, p_email_notifications, p_push_notifications, p_marketing_consent) = 0 THEN
        RAISE EXCEPTION 'No preference supplied' USING ERRCODE = '22023';
    END IF;
    IF customers.lock_profile_owner(p_user) IS NULL THEN RETURN 'ineligible'; END IF;
    SELECT u.phone_verified_at IS NOT NULL INTO phone_verified FROM identity.users u WHERE u.id = p_user;
    -- API section 16.5: discovery requires a verified phone. MVP phones are unverified.
    IF p_phone_discoverable AND NOT phone_verified THEN RETURN 'phone_unverified'; END IF;
    INSERT INTO customers.preferences(user_id) VALUES (p_user) ON CONFLICT (user_id) DO NOTHING;
    SELECT * INTO before FROM customers.preferences p WHERE p.user_id = p_user FOR UPDATE;
    IF p_phone_discoverable IS DISTINCT FROM before.phone_discoverable AND p_phone_discoverable IS NOT NULL THEN
        changes := changes || jsonb_build_object('phone_discoverable', p_phone_discoverable);
    END IF;
    IF p_email_notifications IS DISTINCT FROM before.email_notifications AND p_email_notifications IS NOT NULL THEN
        changes := changes || jsonb_build_object('email_notifications', p_email_notifications);
    END IF;
    IF p_push_notifications IS DISTINCT FROM before.push_notifications AND p_push_notifications IS NOT NULL THEN
        changes := changes || jsonb_build_object('push_notifications', p_push_notifications);
    END IF;
    IF p_marketing_consent IS DISTINCT FROM before.marketing_consent AND p_marketing_consent IS NOT NULL THEN
        -- Owner decision (2026-11-25): marketing consent may be granted or
        -- withdrawn here. The event cites the DEP-LEG-003 wording release the
        -- customer registered under; legal review of its sufficiency is open.
        SELECT r.wording_version INTO wording FROM customers.registration_consent_evidence e
            JOIN customers.registration_legal_releases r ON r.id = e.release_id WHERE e.user_id = p_user;
        changes := changes || jsonb_build_object('marketing_consent', p_marketing_consent,
            'consent_wording', jsonb_build_object('document_id', 'DEP-LEG-003', 'version', wording));
    END IF;
    IF changes = '{}' THEN RETURN 'updated'; END IF;
    UPDATE customers.preferences p SET
        phone_discoverable = coalesce(p_phone_discoverable, p.phone_discoverable),
        email_notifications = coalesce(p_email_notifications, p.email_notifications),
        push_notifications = coalesce(p_push_notifications, p.push_notifications),
        marketing_consent = coalesce(p_marketing_consent, p.marketing_consent),
        updated_at = greatest(p.updated_at, now())
        WHERE p.user_id = p_user;
    INSERT INTO audit.audit_events(actor_type, actor_id, action, resource_type, resource_id, metadata)
        VALUES ('CUSTOMER', p_user, 'PREFERENCES_CHANGED', 'CUSTOMER', p_user, changes);
    RETURN 'updated';
END $$;

-- Image writes for one customer are serialized so that the audit event names
-- the exact version each replacement superseded.
CREATE FUNCTION customers.lock_profile_image(p_user uuid) RETURNS void
LANGUAGE sql SET search_path = pg_catalog, pg_temp AS $$
    SELECT pg_advisory_xact_lock(hashtextextended('customers.profile_images:' || p_user::text, 0))
$$;

-- Stores the API's re-encoded output. Returns no row for an ineligible account.
CREATE FUNCTION customers.store_profile_image(p_user uuid, p_content bytea, p_width integer, p_height integer)
RETURNS TABLE (version uuid, width integer, height integer, updated_at text)
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_column
DECLARE
    previous uuid;
    stored customers.profile_images%ROWTYPE;
BEGIN
    IF customers.lock_profile_owner(p_user) IS NULL THEN RETURN; END IF;
    PERFORM customers.lock_profile_image(p_user);
    SELECT i.version INTO previous FROM customers.profile_images i WHERE i.user_id = p_user;
    INSERT INTO customers.profile_images AS i (user_id, version, content_type, width, height, content, updated_at)
        VALUES (p_user, gen_random_uuid(), 'image/jpeg', p_width, p_height, p_content, now())
        ON CONFLICT (user_id) DO UPDATE SET version = excluded.version, width = excluded.width,
            height = excluded.height, content = excluded.content,
            updated_at = greatest(i.updated_at, excluded.updated_at)
        RETURNING i.* INTO stored;
    INSERT INTO audit.audit_events(actor_type, actor_id, action, resource_type, resource_id, metadata)
        VALUES ('CUSTOMER', p_user, 'PROFILE_IMAGE_STORED', 'PROFILE_IMAGE', stored.version,
                jsonb_build_object('replaced_version', previous, 'bytes', octet_length(p_content),
                                   'width', p_width, 'height', p_height));
    RETURN QUERY SELECT stored.version, stored.width, stored.height, customers.utc_text(stored.updated_at);
END $$;

-- Omits the bytes when the caller already holds the current version.
CREATE FUNCTION customers.profile_image(p_user uuid, p_known_version uuid)
RETURNS TABLE (version uuid, content bytea)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
    SELECT i.version, CASE WHEN i.version IS DISTINCT FROM p_known_version THEN i.content END
    FROM customers.profile_images i WHERE i.user_id = p_user
$$;

-- Returns 'removed', 'absent' or 'ineligible'.
CREATE FUNCTION customers.remove_profile_image(p_user uuid) RETURNS text
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE removed uuid;
BEGIN
    IF customers.lock_profile_owner(p_user) IS NULL THEN RETURN 'ineligible'; END IF;
    PERFORM customers.lock_profile_image(p_user);
    DELETE FROM customers.profile_images i WHERE i.user_id = p_user RETURNING i.version INTO removed;
    IF removed IS NULL THEN RETURN 'absent'; END IF;
    INSERT INTO audit.audit_events(actor_type, actor_id, action, resource_type, resource_id)
        VALUES ('CUSTOMER', p_user, 'PROFILE_IMAGE_REMOVED', 'PROFILE_IMAGE', removed);
    RETURN 'removed';
END $$;

REVOKE ALL ON FUNCTION customers.utc_text(timestamptz), customers.lock_profile_owner(uuid),
    customers.names_editable(uuid,text), customers.valid_profile_name(text),
    customers.customer_profile(uuid), customers.rename_customer(uuid,text,text),
    customers.customer_preferences(uuid),
    customers.update_customer_preferences(uuid,boolean,boolean,boolean,boolean),
    customers.lock_profile_image(uuid),
    customers.store_profile_image(uuid,bytea,integer,integer),
    customers.profile_image(uuid,uuid), customers.remove_profile_image(uuid) FROM PUBLIC;
