-- Phase 7 (IMP-P7-001–010): internal transfers between DEYPAY customers.
--
-- Builds on the Phase 5 ledger and Phase 6 wallets:
--   * ledger.transfer_between_customers posts the one balanced journal
--     (sender available debit, recipient available credit) after its own
--     CUSTOMER_TRANSFER activation, ceiling, spend-eligibility and funds checks;
--   * wallets.restrictions and wallets.customer_eligible decide who may send
--     and receive; wallets.transfer_wallets (below) creates missing wallets and
--     locks both wallet rows for the transfer's transaction.
-- The business record uses the Phase 2 tables payments.transactions,
-- payments.transaction_status_history and payments.idempotency_records; the
-- customer's authorization uses identity.verification_challenges and
-- identity.transaction_proofs (API §§16, 20, 28; security SEC-TXN-001–007;
-- database §§8.5, 15, 18, 31; ledger §§14, 25.1).
--
-- Privilege model (infrastructure/postgres/payments-grants.psql):
--   * Every payments function is owned by the NOLOGIN role deypay_payments_owner.
--     It reaches the ledger only through transfer_between_customers and
--     asset_controls, and the wallets only through customer_eligible and
--     transfer_wallets. It writes no ledger or wallet table.
--   * deypay_app executes the seven customer functions, always with the
--     authenticated customer's identifier (and, for authorizations and
--     transfers, the digest of that customer's session cookie). It gains no
--     table write.
--
-- Fail-closed gates this migration does not lift:
--   * ledger.capability_activations CUSTOMER_TRANSFER per asset (with its
--     approved per-operation ceiling). None is seeded.
--   * The application FinancialGate: NoApprovedPolicy refuses every customer
--     movement before the database is asked.
--   * payments.recipient_lookup_policy: recipient lookups are refused until the
--     database administrator records the approved per-customer budget.
--
-- Error codes (mapped in deypay-infrastructure):
--   TR001 invalid request            TR002 invalid amount
--   TR003 unsupported asset          TR004 unknown recipient
--   TR005 recipient cannot receive   TR006 sender and recipient are the same
--   TR007 KYC approval required      TR008 sender restricted
--   TR009 authorization invalid      TR010 idempotency key reused
--   TR011 transfers not enabled      TR012 asset unavailable
--   TR013 lookup budget exhausted    TR014 lookups not enabled
--   TR015 session not active

---------------------------------------------------------------------------
-- IMP-P7-007: the internal transfer record (database §18)
---------------------------------------------------------------------------
-- One row per completed transfer, written in the same transaction as its
-- ledger journal. A refused transfer leaves no row, no journal and no
-- consumed authorization. DEC-010 (fee schedule) is open, so the fee is zero
-- and no fee is posted (ledger §14.6).
CREATE TABLE payments.internal_transfers (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    transaction_id uuid NOT NULL UNIQUE,
    sender_user_id uuid NOT NULL,
    recipient_user_id uuid NOT NULL,
    asset_id uuid NOT NULL REFERENCES wallets.assets,
    amount_minor payments.minor_units NOT NULL CHECK (amount_minor > 0),
    fee_minor payments.minor_units NOT NULL DEFAULT 0 CHECK (fee_minor = 0),
    status payments.workflow_status NOT NULL
        CHECK (status IN ('CREATED','VALIDATED','AUTHORIZED','COMPLETED','REJECTED','FAILED')),
    ledger_journal_id uuid UNIQUE REFERENCES ledger.journals,
    authorization_proof_id uuid NOT NULL UNIQUE REFERENCES identity.transaction_proofs,
    created_at timestamptz NOT NULL DEFAULT now(),
    completed_at timestamptz,
    FOREIGN KEY (sender_user_id, transaction_id) REFERENCES payments.transactions(user_id, id),
    FOREIGN KEY (sender_user_id, asset_id) REFERENCES wallets.wallet_profiles(user_id, asset_id),
    FOREIGN KEY (recipient_user_id, asset_id) REFERENCES wallets.wallet_profiles(user_id, asset_id),
    CHECK (sender_user_id <> recipient_user_id),
    CHECK ((status = 'COMPLETED') = (ledger_journal_id IS NOT NULL)),
    CHECK ((status = 'COMPLETED') = (completed_at IS NOT NULL)),
    CHECK (completed_at >= created_at)
);
CREATE INDEX internal_transfers_sender ON payments.internal_transfers(sender_user_id, created_at DESC, transaction_id);
CREATE INDEX internal_transfers_recipient ON payments.internal_transfers(recipient_user_id, created_at DESC, transaction_id);
CREATE TRIGGER immutable_internal_transfer BEFORE UPDATE OR DELETE OR TRUNCATE ON payments.internal_transfers
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();

-- A completed internal transfer's business record never changes or disappears.
CREATE FUNCTION payments.guard_transfer_transaction() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, pg_temp AS $$
BEGIN
    IF OLD.transaction_type = 'INTERNAL_TRANSFER' AND (TG_OP = 'DELETE' OR OLD.status = 'COMPLETED') THEN
        RAISE EXCEPTION 'a completed internal transfer is immutable' USING ERRCODE = '55000';
    END IF;
    RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END;
$$;
CREATE TRIGGER guard_transfer_transaction BEFORE UPDATE OR DELETE ON payments.transactions
    FOR EACH ROW EXECUTE FUNCTION payments.guard_transfer_transaction();

---------------------------------------------------------------------------
-- IMP-P7-002: recipient lookup budget (API §§16.4, 41)
---------------------------------------------------------------------------
-- Written only by the database administrator after Security approves the
-- values. Without a row every recipient lookup is refused.
CREATE TABLE payments.recipient_lookup_policy (
    id boolean PRIMARY KEY DEFAULT true CHECK (id),
    window_seconds integer NOT NULL CHECK (window_seconds BETWEEN 60 AND 86400),
    lookups_per_window integer NOT NULL CHECK (lookups_per_window BETWEEN 1 AND 10000),
    approval_reference text NOT NULL CHECK (length(approval_reference) BETWEEN 1 AND 256
        AND approval_reference !~ '[[:cntrl:]]'),
    recorded_by text NOT NULL DEFAULT session_user,
    recorded_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE payments.recipient_lookup_usage (
    user_id uuid NOT NULL REFERENCES identity.users,
    window_start timestamptz NOT NULL,
    lookups integer NOT NULL CHECK (lookups > 0),
    PRIMARY KEY (user_id, window_start)
);

---------------------------------------------------------------------------
-- Wallet support for transfers (owned by deypay_wallet_owner with the other
-- wallet functions; executed only by deypay_payments_owner)
---------------------------------------------------------------------------
-- The sender's and recipient's wallets for one asset, created when missing
-- for an eligible customer and locked FOR SHARE in id order until the
-- transfer's transaction ends. A concurrent status change (set_wallet_status
-- locks the wallet FOR UPDATE) therefore commits before these checks or waits
-- until the transfer has committed. NULLs mean the wallet does not exist.
CREATE FUNCTION wallets.transfer_wallets(p_sender uuid, p_recipient uuid, p_asset uuid)
RETURNS TABLE(sender_wallet uuid, sender_status text, sender_restrictions text[],
    recipient_wallet uuid, recipient_status text, recipient_restrictions text[])
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp SET lock_timeout = '10s' AS $$
BEGIN
    IF wallets.customer_eligible(p_sender) THEN
        PERFORM wallets.initialize_customer_wallets(p_sender);
    END IF;
    IF wallets.customer_eligible(p_recipient) THEN
        PERFORM wallets.initialize_customer_wallets(p_recipient);
    END IF;
    PERFORM 1 FROM wallets.wallet_profiles w
    WHERE w.user_id IN (p_sender, p_recipient) AND w.asset_id = p_asset
    ORDER BY w.id FOR SHARE;
    RETURN QUERY
    SELECT s.id, s.status, CASE WHEN s.id IS NOT NULL THEN wallets.restrictions(s.id) END,
           r.id, r.status, CASE WHEN r.id IS NOT NULL THEN wallets.restrictions(r.id) END
    FROM (SELECT 1) AS one
    LEFT JOIN wallets.wallet_profiles s ON s.user_id = p_sender AND s.asset_id = p_asset
    LEFT JOIN wallets.wallet_profiles r ON r.user_id = p_recipient AND r.asset_id = p_asset;
END;
$$;
REVOKE ALL ON FUNCTION wallets.transfer_wallets(uuid,uuid,uuid) FROM PUBLIC;

---------------------------------------------------------------------------
-- Internal helpers (owned by deypay_payments_owner; never granted)
---------------------------------------------------------------------------
CREATE FUNCTION payments.utc(p_time timestamptz) RETURNS text
LANGUAGE sql STABLE SET search_path = pg_catalog, pg_temp AS $$
    SELECT to_char(p_time AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"');
$$;

-- The customer's live session whose cookie digest the API presents. The
-- session identifier stays stable when the cookie rotates.
CREATE FUNCTION payments.active_session(p_customer uuid, p_digest text) RETURNS uuid
LANGUAGE plpgsql STABLE SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    live uuid;
BEGIN
    IF p_customer IS NOT NULL AND p_digest ~ '^[a-f0-9]{64}$' THEN
        SELECT s.id INTO live FROM identity.sessions s
        WHERE s.user_id = p_customer AND s.token_hash = p_digest AND s.revoked_at IS NULL
          AND NOT s.refresh_required AND s.expires_at > clock_timestamp()
          AND s.absolute_expires_at > clock_timestamp();
    END IF;
    IF live IS NULL THEN
        RAISE EXCEPTION 'session is not active' USING ERRCODE = 'TR015';
    END IF;
    RETURN live;
END;
$$;

-- Charges one recipient lookup to the customer's current window.
CREATE FUNCTION payments.charge_lookup(p_customer uuid) RETURNS void
LANGUAGE plpgsql SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    policy payments.recipient_lookup_policy%ROWTYPE;
    bucket timestamptz;
    used integer;
BEGIN
    SELECT * INTO policy FROM payments.recipient_lookup_policy;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'recipient lookups are not enabled' USING ERRCODE = 'TR014';
    END IF;
    bucket := to_timestamp(floor(extract(epoch FROM clock_timestamp()) / policy.window_seconds)
        * policy.window_seconds);
    INSERT INTO payments.recipient_lookup_usage AS u(user_id, window_start, lookups)
    VALUES (p_customer, bucket, 1)
    ON CONFLICT (user_id, window_start) DO UPDATE SET lookups = u.lookups + 1
    RETURNING u.lookups INTO used;
    IF used > policy.lookups_per_window THEN
        RAISE EXCEPTION 'recipient lookup limit reached' USING ERRCODE = 'TR013';
    END IF;
    DELETE FROM payments.recipient_lookup_usage u
    WHERE u.user_id = p_customer AND u.window_start < bucket - interval '1 day';
END;
$$;

-- Whether a customer may currently receive internal transfers at all:
-- wallet-eligible, KYC-approved and not restricted (engineering default
-- pending owner confirmation; DECISIONS: Phase 7).
CREATE FUNCTION payments.can_receive(p_user uuid) RETURNS boolean
LANGUAGE sql STABLE SET search_path = pg_catalog, pg_temp AS $$
    SELECT wallets.customer_eligible(p_user)
       AND EXISTS (SELECT 1 FROM identity.users u WHERE u.id = p_user AND u.status = 'KYC_APPROVED')
       AND NOT EXISTS (SELECT 1 FROM compliance.account_restrictions r WHERE r.user_id = p_user AND r.active);
$$;

-- The recipient's permitted identity details (API §16.4): username and
-- profile name, never contact, KYC, balance or compliance data.
CREATE FUNCTION payments.recipient_identity(p_recipient uuid)
RETURNS TABLE(username text, display_name text)
LANGUAGE plpgsql STABLE SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_column
BEGIN
    RETURN QUERY
    SELECT ltrim(n.username, '@'), nullif(btrim(concat_ws(' ', p.first_name, p.last_name)), '')
    FROM identity.users u
    LEFT JOIN customers.usernames n ON n.user_id = u.id AND n.is_active
    LEFT JOIN customers.profiles p ON p.user_id = u.id
    WHERE u.id = p_recipient AND wallets.customer_eligible(u.id);
    IF NOT FOUND THEN
        RAISE EXCEPTION 'unknown recipient' USING ERRCODE = 'TR004';
    END IF;
END;
$$;

-- IMP-P7-003: validates the transfer terms the customer submitted. Amounts
-- arrive in accounting units; the API has already parsed the decimal string
-- exactly at the asset's precision.
CREATE FUNCTION payments.transfer_request(p_customer uuid, p_recipient uuid, p_asset text, p_amount_minor text,
    p_description text)
RETURNS TABLE(asset_id uuid, symbol text, accounting_precision smallint, amount numeric)
LANGUAGE plpgsql STABLE SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_column
DECLARE
    asset record;
BEGIN
    IF p_customer IS NULL OR p_recipient IS NULL THEN
        RAISE EXCEPTION 'a sender and recipient are required' USING ERRCODE = 'TR001';
    END IF;
    IF p_description IS NOT NULL AND (p_description <> btrim(p_description)
        OR length(p_description) NOT BETWEEN 1 AND 140 OR p_description ~ '[[:cntrl:]]') THEN
        RAISE EXCEPTION 'a description is 1–140 printable characters' USING ERRCODE = 'TR001';
    END IF;
    SELECT a.id, a.symbol, a.accounting_precision INTO asset
    FROM wallets.assets a WHERE a.symbol = p_asset AND a.symbol IN ('USDC','USDT');
    IF NOT FOUND THEN
        RAISE EXCEPTION 'unsupported asset' USING ERRCODE = 'TR003';
    END IF;
    IF p_amount_minor IS NULL OR p_amount_minor !~ '^[1-9][0-9]{0,37}$' THEN
        RAISE EXCEPTION 'amount must be a positive whole number of accounting units' USING ERRCODE = 'TR002';
    END IF;
    IF p_recipient = p_customer THEN
        RAISE EXCEPTION 'a customer cannot transfer to themselves' USING ERRCODE = 'TR006';
    END IF;
    RETURN QUERY SELECT asset.id, asset.symbol, asset.accounting_precision, p_amount_minor::numeric;
END;
$$;

-- SEC-TXN-002 binding: customer, operation type, asset, amount, immutable
-- recipient identifier, fee and description. Version 1.
CREATE FUNCTION payments.transfer_binding(p_customer uuid, p_recipient uuid, p_asset uuid, p_amount numeric,
    p_fee numeric, p_description text) RETURNS text
LANGUAGE sql IMMUTABLE SET search_path = pg_catalog, pg_temp AS $$
    SELECT encode(sha256(convert_to(jsonb_build_array('INTERNAL_TRANSFER', 1, p_customer::text, p_recipient::text,
        p_asset::text, p_amount::text, p_fee::text, coalesce(p_description, ''))::text, 'UTF8')), 'hex');
$$;

-- IMP-P7-004: everything that must hold for this sender to send this asset to
-- this recipient now. Returns the two locked wallets.
CREATE FUNCTION payments.require_transfer_eligibility(p_sender uuid, p_recipient uuid, p_asset uuid)
RETURNS TABLE(sender_wallet uuid, recipient_wallet uuid)
LANGUAGE plpgsql SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_column
DECLARE
    state record;
BEGIN
    IF NOT (SELECT c.activated @> ARRAY['CUSTOMER_TRANSFER'] FROM ledger.asset_controls(p_asset) c) THEN
        RAISE EXCEPTION 'internal transfers are not enabled for this asset' USING ERRCODE = 'TR011';
    END IF;
    IF NOT wallets.customer_eligible(p_sender) THEN
        RAISE EXCEPTION 'this account cannot send funds' USING ERRCODE = 'TR008';
    END IF;
    SELECT * INTO state FROM wallets.transfer_wallets(p_sender, p_recipient, p_asset);
    IF state.sender_wallet IS NULL
       OR state.sender_restrictions && ARRAY['ASSET_UNAVAILABLE','ASSET_ON_HOLD'] THEN
        RAISE EXCEPTION 'this asset is temporarily unavailable' USING ERRCODE = 'TR012';
    END IF;
    IF state.sender_restrictions @> ARRAY['ACCOUNT_RESTRICTED'] OR state.sender_status <> 'ACTIVE' THEN
        RAISE EXCEPTION 'this account cannot send funds' USING ERRCODE = 'TR008';
    END IF;
    IF state.sender_restrictions @> ARRAY['KYC_REQUIRED'] THEN
        RAISE EXCEPTION 'identity verification is required to send funds' USING ERRCODE = 'TR007';
    END IF;
    -- A RESTRICTED wallet still receives; SUSPENDED, CLOSED and PROVISIONING do not.
    IF NOT payments.can_receive(p_recipient) OR state.recipient_wallet IS NULL
       OR state.recipient_status NOT IN ('ACTIVE','RESTRICTED')
       OR state.recipient_restrictions && ARRAY['ACCOUNT_RESTRICTED','KYC_REQUIRED','ASSET_UNAVAILABLE','ASSET_ON_HOLD'] THEN
        RAISE EXCEPTION 'the recipient cannot receive this transfer' USING ERRCODE = 'TR005';
    END IF;
    RETURN QUERY SELECT state.sender_wallet, state.recipient_wallet;
END;
$$;

-- Public transaction references are random, never sequential.
CREATE FUNCTION payments.new_reference() RETURNS text
LANGUAGE sql VOLATILE SET search_path = pg_catalog, pg_temp AS $$
    SELECT 'DP-' || upper(substr(replace(gen_random_uuid()::text, '-', ''), 1, 12)
        || substr(replace(gen_random_uuid()::text, '-', ''), 1, 4));
$$;

-- IMP-P7-009: a customer's internal transfers, as sender or recipient, with
-- the direction and counterparty relative to that customer (API §28.5,
-- database §18.3). Filters: one transaction, or a page before a cursor.
CREATE FUNCTION payments.participant_transfers(p_customer uuid, p_transaction uuid, p_cursor uuid, p_limit integer)
RETURNS TABLE(transaction_id uuid, reference text, transaction_type text, status text, direction text, asset text,
    accounting_precision smallint, amount_minor text, fee_minor text, description text, counterparty_id uuid,
    counterparty_username text, counterparty_display_name text, created_at text, completed_at text)
LANGUAGE sql STABLE SET search_path = pg_catalog, pg_temp AS $$
    WITH anchor AS (
        SELECT it.created_at, it.transaction_id FROM payments.internal_transfers it
        WHERE it.transaction_id = p_cursor
          AND (it.sender_user_id = p_customer OR it.recipient_user_id = p_customer))
    SELECT t.id, t.public_reference, t.transaction_type, t.status::text,
           CASE WHEN it.sender_user_id = p_customer THEN 'DEBIT' ELSE 'CREDIT' END,
           a.symbol, a.accounting_precision, it.amount_minor::text,
           CASE WHEN it.sender_user_id = p_customer THEN it.fee_minor ELSE 0 END::text,
           t.description, other.id,
           (SELECT ltrim(n.username, '@') FROM customers.usernames n WHERE n.user_id = other.id AND n.is_active),
           (SELECT nullif(btrim(concat_ws(' ', p.first_name, p.last_name)), '')
            FROM customers.profiles p WHERE p.user_id = other.id),
           payments.utc(t.created_at), payments.utc(t.completed_at)
    FROM payments.internal_transfers it
    JOIN payments.transactions t ON t.id = it.transaction_id
    JOIN wallets.assets a ON a.id = it.asset_id
    JOIN identity.users other ON other.id = CASE WHEN it.sender_user_id = p_customer
                                                 THEN it.recipient_user_id ELSE it.sender_user_id END
    WHERE (it.sender_user_id = p_customer OR it.recipient_user_id = p_customer)
      AND (p_transaction IS NULL OR it.transaction_id = p_transaction)
      AND (p_cursor IS NULL OR (it.created_at, it.transaction_id) < (SELECT x.created_at, x.transaction_id FROM anchor x))
    ORDER BY it.created_at DESC, it.transaction_id DESC
    LIMIT least(greatest(coalesce(p_limit, 20), 1), 100);
$$;

---------------------------------------------------------------------------
-- Customer interface (deypay_app)
---------------------------------------------------------------------------
-- IMP-P7-001/002: resolves an exact username to the recipient's immutable
-- identifier (API §16.2). Authenticated customers only, charged to their
-- lookup budget (a lookup that finds nobody is charged too). No partial
-- matching or listing. An unknown, retired or ineligible username returns no
-- row. can_receive is false for the caller themselves and for anyone who
-- cannot receive now, without saying why.
CREATE FUNCTION payments.resolve_recipient(p_customer uuid, p_username text)
RETURNS TABLE(recipient_id uuid, username text, display_name text, can_receive boolean)
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_column
DECLARE
    hit record;
BEGIN
    IF p_customer IS NULL THEN
        RAISE EXCEPTION 'a customer is required' USING ERRCODE = 'TR001';
    END IF;
    PERFORM payments.charge_lookup(p_customer);
    IF p_username IS NULL OR p_username !~ '^@?[0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_.]{3,30}$'
       OR p_username ~ '^@?\.|\.$|\.\.' THEN
        RETURN;
    END IF;
    SELECT n.user_id, ltrim(n.username, '@') AS username,
           nullif(btrim(concat_ws(' ', p.first_name, p.last_name)), '') AS display_name INTO hit
    FROM customers.usernames n
    LEFT JOIN customers.profiles p ON p.user_id = n.user_id
    WHERE n.normalized_username = lower(ltrim(p_username, '@')) AND n.is_active
      AND wallets.customer_eligible(n.user_id);
    IF NOT FOUND THEN
        RETURN;
    END IF;
    RETURN QUERY SELECT hit.user_id, hit.username, hit.display_name,
        hit.user_id <> p_customer AND payments.can_receive(hit.user_id);
END;
$$;

-- The transfer assets: the customer wallet stablecoins and their precision.
CREATE FUNCTION payments.transfer_assets()
RETURNS TABLE(asset_id uuid, symbol text, accounting_precision smallint)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
    SELECT a.id, a.symbol, a.accounting_precision FROM wallets.assets a
    WHERE a.symbol IN ('USDC','USDT') ORDER BY a.symbol;
$$;

-- IMP-P7-006 step 1 (API §20.6): validates the exact terms and eligibility
-- and opens a confirmation challenge bound to them and to this session.
-- Opening one revokes the session's earlier open transfer challenges.
-- Verification is explicit confirmation of the displayed terms: no step-up
-- method is approved (DEC-002), and no risk policy defines high-risk
-- transfers (DEC-011). Challenge lifetime: 5 minutes (engineering default).
-- No code is issued, so code_hash holds a marker that no digest can equal.
CREATE FUNCTION payments.authorize_internal_transfer(p_customer uuid, p_session_digest text, p_recipient uuid,
    p_asset text, p_amount_minor text, p_description text)
RETURNS TABLE(challenge_id uuid, expires_at text, recipient_id uuid, recipient_username text,
    recipient_display_name text, asset text, accounting_precision smallint, amount_minor text, fee_minor text,
    description text, verification_method text)
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp SET lock_timeout = '10s' AS $$
#variable_conflict use_column
DECLARE
    session uuid := payments.active_session(p_customer, p_session_digest);
    terms record;
    person record;
    expiry timestamptz := clock_timestamp() + interval '5 minutes';
    opened uuid;
BEGIN
    SELECT * INTO terms FROM payments.transfer_request(p_customer, p_recipient, p_asset, p_amount_minor, p_description);
    SELECT * INTO person FROM payments.recipient_identity(p_recipient);
    PERFORM payments.require_transfer_eligibility(p_customer, p_recipient, terms.asset_id);
    UPDATE identity.verification_challenges c SET revoked_at = clock_timestamp()
    WHERE c.user_id = p_customer AND c.session_id = session AND c.operation_type = 'INTERNAL_TRANSFER'
      AND c.consumed_at IS NULL AND c.revoked_at IS NULL;
    INSERT INTO identity.verification_challenges(user_id, challenge_type, code_hash, session_id, operation_type,
        binding_version, binding_digest, expires_at)
    VALUES (p_customer, 'TRANSACTION_CONFIRMATION', 'none:explicit-confirmation', session, 'INTERNAL_TRANSFER', 1,
        payments.transfer_binding(p_customer, p_recipient, terms.asset_id, terms.amount, 0, p_description), expiry)
    RETURNING id INTO opened;
    INSERT INTO audit.audit_events(actor_type, actor_id, action, resource_type, resource_id, metadata)
    VALUES ('CUSTOMER', p_customer, 'TRANSACTION_AUTHORIZATION_REQUESTED', 'AUTHORIZATION_CHALLENGE', opened,
        jsonb_build_object('operation_type', 'INTERNAL_TRANSFER', 'session_id', session));
    RETURN QUERY SELECT opened, payments.utc(expiry), p_recipient, person.username, person.display_name,
        terms.symbol, terms.accounting_precision, terms.amount::text, '0'::text, p_description, 'CONFIRMATION'::text;
END;
$$;

-- IMP-P7-006 step 2: the customer confirms the displayed terms in the same
-- session before the challenge expires; the challenge completes once and a
-- single-use proof is recorded. Only the proof's SHA-256 digest is stored; the
-- API generates the proof and returns it once. Proof lifetime: 2 minutes
-- (engineering default), never beyond the session.
CREATE FUNCTION payments.confirm_transfer_authorization(p_customer uuid, p_session_digest text, p_challenge uuid,
    p_proof_digest text) RETURNS text
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp SET lock_timeout = '10s' AS $$
DECLARE
    session uuid := payments.active_session(p_customer, p_session_digest);
    challenge identity.verification_challenges%ROWTYPE;
    stamp timestamptz := clock_timestamp();
    expiry timestamptz := clock_timestamp() + interval '2 minutes';
    proof uuid;
BEGIN
    IF p_proof_digest IS NULL OR p_proof_digest !~ '^[a-f0-9]{64}$' THEN
        RAISE EXCEPTION 'invalid proof digest' USING ERRCODE = 'TR001';
    END IF;
    SELECT * INTO challenge FROM identity.verification_challenges c
    WHERE c.id = p_challenge AND c.user_id = p_customer AND c.operation_type = 'INTERNAL_TRANSFER'
    FOR UPDATE;
    IF NOT FOUND OR challenge.session_id <> session OR challenge.consumed_at IS NOT NULL
       OR challenge.revoked_at IS NOT NULL OR stamp >= challenge.expires_at THEN
        RAISE EXCEPTION 'this authorization is no longer valid' USING ERRCODE = 'TR009';
    END IF;
    UPDATE identity.verification_challenges c SET consumed_at = stamp WHERE c.id = challenge.id;
    INSERT INTO identity.transaction_proofs(user_id, challenge_id, proof_hash, expires_at, challenge_completed_at)
    VALUES (p_customer, challenge.id, p_proof_digest, expiry, stamp)
    RETURNING id INTO proof;
    INSERT INTO audit.audit_events(actor_type, actor_id, action, resource_type, resource_id, metadata)
    VALUES ('CUSTOMER', p_customer, 'TRANSACTION_AUTHORIZED', 'AUTHORIZATION_CHALLENGE', challenge.id,
        jsonb_build_object('operation_type', 'INTERNAL_TRANSFER', 'session_id', session, 'proof_id', proof,
            'binding_digest', challenge.binding_digest));
    RETURN payments.utc(expiry);
END;
$$;

-- IMP-P7-003–008: executes an authorized transfer in one transaction.
--   1. Validates the terms, then the idempotency key: an identical earlier
--      request returns its transfer (replayed); a different request under the
--      same key is refused (TR010). Concurrent requests with one key queue.
--   2. Verifies the single-use proof: this customer and session, unexpired,
--      unconsumed, unrevoked and bound to exactly these terms.
--   3. Rechecks activation, eligibility and both wallets, which stay locked.
--   4. Records the transaction (CREATED → VALIDATED → AUTHORIZED), consumes the
--      proof, posts the ledger journal (funds, ceiling and accounts are checked
--      there), records the transfer, completes the transaction, stores the
--      idempotent result and queues the notification event for both parties.
-- Any refusal rolls everything back: no record, journal or consumed proof.
CREATE FUNCTION payments.execute_internal_transfer(p_customer uuid, p_session_digest text, p_idempotency_key text,
    p_proof_digest text, p_recipient uuid, p_asset text, p_amount_minor text, p_description text)
RETURNS TABLE(replayed boolean, transaction_id uuid, reference text, transaction_type text, status text,
    direction text, asset text, accounting_precision smallint, amount_minor text, fee_minor text, description text,
    counterparty_id uuid, counterparty_username text, counterparty_display_name text, created_at text,
    completed_at text)
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp SET lock_timeout = '10s' AS $$
#variable_conflict use_column
DECLARE
    terms record;
    fingerprint text;
    previous payments.idempotency_records%ROWTYPE;
    session uuid;
    proof record;
    opened uuid;
    posted record;
    stamp timestamptz := clock_timestamp();
    finished timestamptz;
BEGIN
    SELECT * INTO terms FROM payments.transfer_request(p_customer, p_recipient, p_asset, p_amount_minor, p_description);
    IF p_idempotency_key IS NULL OR p_idempotency_key !~ '^[A-Za-z0-9_.:-]{8,128}$' THEN
        RAISE EXCEPTION 'an idempotency key is 8–128 letters, digits or _.:-' USING ERRCODE = 'TR001';
    END IF;
    fingerprint := encode(sha256(convert_to(jsonb_build_array(p_recipient::text, terms.symbol, terms.amount::text,
        coalesce(p_description, ''))::text, 'UTF8')), 'hex');
    PERFORM pg_advisory_xact_lock(hashtextextended('deypay.payments.idempotency|INTERNAL_TRANSFER|'
        || p_customer::text || '|' || p_idempotency_key, 0));
    SELECT * INTO previous FROM payments.idempotency_records r
    WHERE r.user_id = p_customer AND r.operation_type = 'INTERNAL_TRANSFER' AND r.idempotency_key = p_idempotency_key;
    IF FOUND THEN
        IF previous.request_hash <> fingerprint THEN
            RAISE EXCEPTION 'this idempotency key was used for a different transfer' USING ERRCODE = 'TR010';
        END IF;
        RETURN QUERY SELECT true, v.* FROM payments.participant_transfers(p_customer, previous.transaction_id, NULL, 1) v;
        RETURN;
    END IF;

    session := payments.active_session(p_customer, p_session_digest);
    IF p_proof_digest IS NULL OR p_proof_digest !~ '^[a-f0-9]{64}$' THEN
        RAISE EXCEPTION 'this authorization is no longer valid' USING ERRCODE = 'TR009';
    END IF;
    SELECT p.id, p.expires_at, p.consumed_at, p.revoked_at, c.session_id, c.operation_type, c.binding_version,
           c.binding_digest INTO proof
    FROM identity.transaction_proofs p
    JOIN identity.verification_challenges c ON c.id = p.challenge_id AND c.user_id = p.user_id
    WHERE p.proof_hash = p_proof_digest AND p.user_id = p_customer
    FOR UPDATE OF p;
    IF NOT FOUND OR proof.operation_type IS DISTINCT FROM 'INTERNAL_TRANSFER' OR proof.session_id <> session
       OR proof.consumed_at IS NOT NULL OR proof.revoked_at IS NOT NULL OR stamp >= proof.expires_at
       OR proof.binding_version <> 1
       OR proof.binding_digest <> payments.transfer_binding(p_customer, p_recipient, terms.asset_id, terms.amount, 0,
           p_description) THEN
        RAISE EXCEPTION 'this authorization is no longer valid' USING ERRCODE = 'TR009';
    END IF;
    PERFORM payments.recipient_identity(p_recipient);
    PERFORM payments.require_transfer_eligibility(p_customer, p_recipient, terms.asset_id);

    INSERT INTO payments.transactions(public_reference, user_id, transaction_type, status, source_asset_id,
        destination_asset_id, fee_asset_id, source_amount_minor, destination_amount_minor, fee_amount_minor,
        description, created_at, updated_at)
    VALUES (payments.new_reference(), p_customer, 'INTERNAL_TRANSFER', 'AUTHORIZED', terms.asset_id, terms.asset_id,
        terms.asset_id, terms.amount, terms.amount, 0, p_description, stamp, stamp)
    RETURNING id INTO opened;
    INSERT INTO payments.transaction_status_history(transaction_id, previous_status, new_status, reason_code,
        actor_type, actor_id, created_at)
    VALUES (opened, NULL, 'CREATED', NULL, 'CUSTOMER', p_customer, stamp),
           (opened, 'CREATED', 'VALIDATED', 'TERMS_VALIDATED', 'CUSTOMER', p_customer, stamp),
           (opened, 'VALIDATED', 'AUTHORIZED', 'PROOF_ACCEPTED', 'CUSTOMER', p_customer, stamp);
    UPDATE identity.transaction_proofs p SET consumed_at = stamp, accepted_transaction_id = opened
    WHERE p.id = proof.id;

    SELECT * INTO posted FROM ledger.transfer_between_customers('internal-transfer:' || opened::text, p_customer,
        p_recipient, terms.asset_id, terms.amount::text, opened, 'CUSTOMER', p_customer, NULL);
    finished := clock_timestamp();
    INSERT INTO payments.internal_transfers(transaction_id, sender_user_id, recipient_user_id, asset_id, amount_minor,
        fee_minor, status, ledger_journal_id, authorization_proof_id, created_at, completed_at)
    VALUES (opened, p_customer, p_recipient, terms.asset_id, terms.amount, 0, 'COMPLETED', posted.journal_id,
        proof.id, stamp, finished);
    UPDATE payments.transactions t SET status = 'COMPLETED', completed_at = finished, updated_at = finished
    WHERE t.id = opened;
    INSERT INTO payments.transaction_status_history(transaction_id, previous_status, new_status, reason_code,
        actor_type, actor_id, created_at)
    VALUES (opened, 'AUTHORIZED', 'COMPLETED', 'LEDGER_POSTED', 'SYSTEM', NULL, finished);
    INSERT INTO payments.idempotency_records(user_id, operation_type, idempotency_key, request_hash, transaction_id,
        response_status, response_body, status, created_at, expires_at)
    VALUES (p_customer, 'INTERNAL_TRANSFER', p_idempotency_key, fingerprint, opened, 201,
        jsonb_build_object('transaction_id', opened, 'status', 'COMPLETED', 'asset', terms.symbol,
            'amount_minor', terms.amount::text),
        'COMPLETED', stamp, stamp + interval '90 days');
    INSERT INTO integrations.outbox_events(aggregate_type, aggregate_id, event_type, payload)
    VALUES ('payments.transaction', opened, 'INTERNAL_TRANSFER_COMPLETED', jsonb_build_object(
        'transaction_id', opened, 'sender_user_id', p_customer, 'recipient_user_id', p_recipient,
        'asset', terms.symbol, 'amount_minor', terms.amount::text, 'journal_id', posted.journal_id));
    INSERT INTO audit.audit_events(actor_type, actor_id, action, resource_type, resource_id, metadata)
    VALUES ('CUSTOMER', p_customer, 'INTERNAL_TRANSFER_COMPLETED', 'TRANSACTION', opened,
        jsonb_build_object('session_id', session, 'authorization_proof_id', proof.id,
            'journal_id', posted.journal_id, 'recipient_id', p_recipient));
    RETURN QUERY SELECT false, v.* FROM payments.participant_transfers(p_customer, opened, NULL, 1) v;
END;
$$;

-- IMP-P7-009: the customer's transfer history, newest first, after an
-- optional cursor (the last transaction of the previous page).
CREATE FUNCTION payments.customer_transactions(p_customer uuid, p_cursor uuid, p_limit integer)
RETURNS TABLE(transaction_id uuid, reference text, transaction_type text, status text, direction text, asset text,
    accounting_precision smallint, amount_minor text, fee_minor text, description text, counterparty_id uuid,
    counterparty_username text, counterparty_display_name text, created_at text, completed_at text)
LANGUAGE plpgsql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
BEGIN
    IF p_customer IS NULL OR p_limit IS NULL OR p_limit NOT BETWEEN 1 AND 100 THEN
        RAISE EXCEPTION 'invalid page request' USING ERRCODE = 'TR001';
    END IF;
    IF p_cursor IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM payments.internal_transfers it WHERE it.transaction_id = p_cursor
          AND (it.sender_user_id = p_customer OR it.recipient_user_id = p_customer)) THEN
        RAISE EXCEPTION 'unknown cursor' USING ERRCODE = 'TR001';
    END IF;
    RETURN QUERY SELECT * FROM payments.participant_transfers(p_customer, NULL, p_cursor, p_limit);
END;
$$;

-- One transaction, for its sender or recipient only.
CREATE FUNCTION payments.customer_transaction(p_customer uuid, p_transaction uuid)
RETURNS TABLE(transaction_id uuid, reference text, transaction_type text, status text, direction text, asset text,
    accounting_precision smallint, amount_minor text, fee_minor text, description text, counterparty_id uuid,
    counterparty_username text, counterparty_display_name text, created_at text, completed_at text)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
    SELECT * FROM payments.participant_transfers(p_customer, p_transaction, NULL, 1)
    WHERE p_customer IS NOT NULL AND p_transaction IS NOT NULL;
$$;

REVOKE ALL ON ALL FUNCTIONS IN SCHEMA payments FROM PUBLIC;
