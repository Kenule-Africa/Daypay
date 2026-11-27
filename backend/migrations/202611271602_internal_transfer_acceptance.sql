-- Phase 7: preserve the accepted response for idempotent retries and evaluate
-- authorization lifetime after lock waits. Existing signatures, role grants and
-- activation gates carry forward. No customer capability or policy is enabled.

-- The earlier branch stored only a subset of the result. Preserve those keys
-- and capture the permitted participant projection once for existing records.
-- Display names edited before this upgrade cannot be recovered from those rows.
WITH legacy AS (
    SELECT r.id, to_jsonb(v) AS snapshot
    FROM payments.idempotency_records r
    CROSS JOIN LATERAL payments.participant_transfers(r.user_id, r.transaction_id, NULL, 1) v
    WHERE r.operation_type = 'INTERNAL_TRANSFER' AND r.status = 'COMPLETED'
      AND r.transaction_id IS NOT NULL AND NOT (r.response_body ? 'reference')
)
UPDATE payments.idempotency_records r SET response_body = r.response_body || legacy.snapshot
FROM legacy WHERE r.id = legacy.id;

CREATE OR REPLACE FUNCTION payments.authorize_internal_transfer(p_customer uuid, p_session_digest text, p_recipient uuid,
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
    -- Serialize replacement of open challenges even when there is no row to lock.
    PERFORM pg_advisory_xact_lock(hashtextextended('deypay.payments.challenge|' || session::text, 0));
    PERFORM payments.active_session(p_customer, p_session_digest);
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

CREATE OR REPLACE FUNCTION payments.confirm_transfer_authorization(p_customer uuid, p_session_digest text, p_challenge uuid,
    p_proof_digest text) RETURNS text
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp SET lock_timeout = '10s' AS $$
DECLARE
    session uuid := payments.active_session(p_customer, p_session_digest);
    challenge identity.verification_challenges%ROWTYPE;
    stamp timestamptz;
    expiry timestamptz;
    proof uuid;
BEGIN
    IF p_proof_digest IS NULL OR p_proof_digest !~ '^[a-f0-9]{64}$' THEN
        RAISE EXCEPTION 'invalid proof digest' USING ERRCODE = 'TR001';
    END IF;
    SELECT * INTO challenge FROM identity.verification_challenges c
    WHERE c.id = p_challenge AND c.user_id = p_customer AND c.operation_type = 'INTERNAL_TRANSFER'
    FOR UPDATE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'this authorization is no longer valid' USING ERRCODE = 'TR009';
    END IF;
    -- Check time and session validity after acquiring the challenge lock.
    SELECT least(s.expires_at, s.absolute_expires_at, clock_timestamp() + interval '2 minutes') INTO expiry
    FROM identity.sessions s WHERE s.id = session AND s.token_hash = p_session_digest
        AND s.revoked_at IS NULL AND NOT s.refresh_required;
    stamp := clock_timestamp();
    IF expiry IS NULL OR expiry <= stamp THEN
        RAISE EXCEPTION 'session is not active' USING ERRCODE = 'TR015';
    END IF;
    IF challenge.session_id IS DISTINCT FROM session OR challenge.consumed_at IS NOT NULL
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

CREATE OR REPLACE FUNCTION payments.execute_internal_transfer(p_customer uuid, p_session_digest text, p_idempotency_key text,
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
    stamp timestamptz;
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
    session := payments.active_session(p_customer, p_session_digest);
    SELECT * INTO previous FROM payments.idempotency_records r
    WHERE r.user_id = p_customer AND r.operation_type = 'INTERNAL_TRANSFER' AND r.idempotency_key = p_idempotency_key;
    IF FOUND THEN
        IF previous.request_hash <> fingerprint THEN
            RAISE EXCEPTION 'this idempotency key was used for a different transfer' USING ERRCODE = 'TR010';
        END IF;
        RETURN QUERY SELECT true, v.* FROM jsonb_to_record(previous.response_body) AS v(
            transaction_id uuid, reference text, transaction_type text, status text, direction text,
            asset text, accounting_precision smallint, amount_minor text, fee_minor text, description text,
            counterparty_id uuid, counterparty_username text, counterparty_display_name text,
            created_at text, completed_at text);
        RETURN;
    END IF;

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
       OR proof.consumed_at IS NOT NULL OR proof.revoked_at IS NOT NULL OR clock_timestamp() >= proof.expires_at
       OR proof.binding_version <> 1
       OR proof.binding_digest <> payments.transfer_binding(p_customer, p_recipient, terms.asset_id, terms.amount, 0,
           p_description) THEN
        RAISE EXCEPTION 'this authorization is no longer valid' USING ERRCODE = 'TR009';
    END IF;
    PERFORM payments.recipient_identity(p_recipient);
    PERFORM payments.require_transfer_eligibility(p_customer, p_recipient, terms.asset_id);

    -- Eligibility may have waited for a wallet lock. Validate the session and proof at consumption.
    PERFORM payments.active_session(p_customer, p_session_digest);
    stamp := clock_timestamp();
    IF stamp >= proof.expires_at THEN
        RAISE EXCEPTION 'this authorization is no longer valid' USING ERRCODE = 'TR009';
    END IF;
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
    INSERT INTO integrations.outbox_events(aggregate_type, aggregate_id, event_type, payload)
    VALUES ('payments.transaction', opened, 'INTERNAL_TRANSFER_COMPLETED', jsonb_build_object(
        'transaction_id', opened, 'sender_user_id', p_customer, 'recipient_user_id', p_recipient,
        'asset', terms.symbol, 'amount_minor', terms.amount::text, 'journal_id', posted.journal_id));
    INSERT INTO audit.audit_events(actor_type, actor_id, action, resource_type, resource_id, metadata)
    VALUES ('CUSTOMER', p_customer, 'INTERNAL_TRANSFER_COMPLETED', 'TRANSACTION', opened,
        jsonb_build_object('session_id', session, 'authorization_proof_id', proof.id,
            'journal_id', posted.journal_id, 'recipient_id', p_recipient));
    -- Project once: the first caller and every retry see the same accepted result.
    RETURN QUERY
    WITH result AS MATERIALIZED (
        SELECT v.* FROM payments.participant_transfers(p_customer, opened, NULL, 1) v
    ), saved AS (
        INSERT INTO payments.idempotency_records(user_id, operation_type, idempotency_key, request_hash,
            transaction_id, response_status, response_body, status, created_at, expires_at)
        SELECT p_customer, 'INTERNAL_TRANSFER', p_idempotency_key, fingerprint, opened, 201,
            to_jsonb(v), 'COMPLETED', stamp, stamp + interval '90 days' FROM result v
        RETURNING payments.idempotency_records.transaction_id
    )
    SELECT false, v.* FROM result v JOIN saved USING (transaction_id);
    IF NOT FOUND THEN
        RAISE EXCEPTION 'completed transfer has no participant result';
    END IF;
END;
$$;
