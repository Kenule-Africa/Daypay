CREATE DOMAIN payments.workflow_status AS text CHECK (VALUE IN (
    'CREATED','VALIDATED','AUTHORIZED','DETECTED','CONFIRMING','SCREENING','CREDITED','QUOTED','RESERVED',
    'SUBMITTED','PROCESSING','COMPLETED','BENEFICIARY_VERIFIED','CONVERTING','PAYOUT_PROCESSING',
    'AWAITING_FUNDS','PENDING_RECONCILIATION','UNDER_REVIEW','REFUND_PROCESSING','REFUNDED','REORG_REVIEW',
    'REVERSED','REJECTED','FAILED','CANCELLED','EXPIRED'));
CREATE TABLE payments.transactions (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), public_reference text NOT NULL UNIQUE CHECK (public_reference <> ''),
    user_id uuid NOT NULL REFERENCES identity.users,
    transaction_type text NOT NULL CHECK (transaction_type IN ('DEPOSIT','WITHDRAWAL','INTERNAL_TRANSFER','BANK_PAYMENT',
        'BANK_WITHDRAWAL','FUNDING_ORDER','CONVERSION','ADJUSTMENT','REVERSAL')),
    status payments.workflow_status NOT NULL, source_asset_id uuid REFERENCES wallets.assets,
    destination_asset_id uuid REFERENCES wallets.assets, fee_asset_id uuid REFERENCES wallets.assets,
    source_amount_minor payments.minor_units CHECK (source_amount_minor > 0),
    destination_amount_minor payments.minor_units CHECK (destination_amount_minor > 0),
    fee_amount_minor payments.minor_units, description text,
    created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now(), completed_at timestamptz,
    CHECK (source_amount_minor IS NULL OR source_asset_id IS NOT NULL),
    CHECK (destination_amount_minor IS NULL OR destination_asset_id IS NOT NULL),
    CHECK (fee_amount_minor IS NULL OR fee_asset_id IS NOT NULL),
    CHECK (updated_at >= created_at AND completed_at >= created_at), UNIQUE (user_id,id)
);
CREATE INDEX transactions_user_time ON payments.transactions(user_id,created_at DESC,id);
CREATE INDEX transactions_status ON payments.transactions(status);
ALTER TABLE identity.transaction_proofs ADD FOREIGN KEY (user_id,accepted_transaction_id)
    REFERENCES payments.transactions(user_id,id);
ALTER TABLE compliance.risk_cases ADD FOREIGN KEY (user_id,transaction_id) REFERENCES payments.transactions(user_id,id);
CREATE TABLE payments.transaction_status_history (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), transaction_id uuid NOT NULL REFERENCES payments.transactions,
    previous_status payments.workflow_status, new_status payments.workflow_status NOT NULL, reason_code text,
    actor_type text NOT NULL, actor_id uuid, created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX transaction_history_time ON payments.transaction_status_history(transaction_id,created_at);
CREATE TRIGGER immutable_history BEFORE UPDATE OR DELETE OR TRUNCATE ON payments.transaction_status_history
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();
CREATE TABLE payments.idempotency_records (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), user_id uuid NOT NULL REFERENCES identity.users,
    operation_type text NOT NULL, idempotency_key text NOT NULL CHECK (idempotency_key <> ''),
    request_hash text NOT NULL CHECK (request_hash <> ''), transaction_id uuid,
    response_status integer CHECK (response_status BETWEEN 100 AND 599), response_body jsonb, status text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(), expires_at timestamptz NOT NULL CHECK (expires_at > created_at),
    UNIQUE (user_id,operation_type,idempotency_key),
    FOREIGN KEY (user_id,transaction_id) REFERENCES payments.transactions(user_id,id)
);
CREATE TABLE compliance.transaction_limits (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), scope_type text NOT NULL, scope_id uuid, transaction_type text NOT NULL,
    asset_id uuid REFERENCES wallets.assets, period text NOT NULL, limit_amount payments.minor_units NOT NULL,
    enabled boolean NOT NULL DEFAULT false, created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE compliance.limit_usage (
    limit_rule_id uuid NOT NULL REFERENCES compliance.transaction_limits, scope_id uuid NOT NULL, window_start timestamptz NOT NULL,
    transaction_id uuid NOT NULL REFERENCES payments.transactions, asset_id uuid NOT NULL REFERENCES wallets.assets,
    amount_minor payments.minor_units NOT NULL, released_at timestamptz,
    PRIMARY KEY (limit_rule_id,scope_id,window_start,transaction_id)
);
