CREATE TABLE integrations.provider_accounts (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), provider_id uuid NOT NULL REFERENCES integrations.providers,
    internal_owner_type text NOT NULL, internal_owner_id uuid NOT NULL, external_account_reference varchar(512) NOT NULL,
    status text NOT NULL, created_at timestamptz NOT NULL DEFAULT now(), UNIQUE(provider_id,external_account_reference)
);
CREATE TABLE integrations.provider_capabilities (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), provider_id uuid NOT NULL REFERENCES integrations.providers,
    service text NOT NULL, asset_id uuid REFERENCES wallets.assets, network_id uuid REFERENCES wallets.networks,
    currency_corridor text, country_code text, environment text NOT NULL,
    available boolean NOT NULL DEFAULT false, production_approved boolean NOT NULL DEFAULT false,
    CHECK (NOT production_approved OR available),
    UNIQUE NULLS NOT DISTINCT(provider_id,service,asset_id,network_id,currency_corridor,country_code,environment)
);
CREATE TABLE integrations.provider_operations (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), provider_id uuid NOT NULL REFERENCES integrations.providers,
    provider_account_scope varchar(512) NOT NULL CHECK (provider_account_scope <> ''),
    transaction_id uuid REFERENCES payments.transactions, is_financial boolean NOT NULL,
    operation_type text NOT NULL, leg text NOT NULL CHECK (leg <> ''), authorized_attempt integer NOT NULL CHECK (authorized_attempt > 0),
    external_reference varchar(512), idempotency_reference varchar(512) NOT NULL CHECK (idempotency_reference <> ''),
    status text NOT NULL CHECK (btrim(status) <> ''), last_error_code text,
    created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (NOT is_financial OR transaction_id IS NOT NULL),
    UNIQUE (provider_id,provider_account_scope,idempotency_reference), UNIQUE (transaction_id,leg,authorized_attempt),
    UNIQUE (id,provider_id,provider_account_scope,leg)
);
CREATE TRIGGER immutable_submission_identity BEFORE UPDATE ON integrations.provider_operations FOR EACH ROW
    WHEN ((OLD.provider_id,OLD.provider_account_scope,OLD.transaction_id,OLD.is_financial,OLD.leg,
           OLD.operation_type,OLD.authorized_attempt,OLD.idempotency_reference) IS DISTINCT FROM
          (NEW.provider_id,NEW.provider_account_scope,NEW.transaction_id,NEW.is_financial,NEW.leg,
           NEW.operation_type,NEW.authorized_attempt,NEW.idempotency_reference))
    EXECUTE FUNCTION audit.reject_mutation();
CREATE INDEX provider_operations_lookup ON integrations.provider_operations(provider_id,provider_account_scope,external_reference)
    WHERE external_reference IS NOT NULL;

-- An observation is not a second economic event; provider chain-transfer claims remain deferred to the deposit model.
CREATE TABLE integrations.economic_events (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), provider_id uuid NOT NULL REFERENCES integrations.providers,
    provider_account_scope varchar(512) NOT NULL, economic_reference varchar(512) NOT NULL CHECK (economic_reference <> ''),
    leg text NOT NULL, provider_operation_id uuid NOT NULL, verified_at timestamptz NOT NULL,
    FOREIGN KEY (provider_operation_id,provider_id,provider_account_scope,leg)
        REFERENCES integrations.provider_operations(id,provider_id,provider_account_scope,leg),
    UNIQUE (provider_id,provider_account_scope,economic_reference,leg)
);
CREATE TABLE integrations.event_observations (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), economic_event_id uuid NOT NULL REFERENCES integrations.economic_events,
    source text NOT NULL CHECK (source IN ('WEBHOOK','POLL','STATEMENT')),
    evidence_reference varchar(512) NOT NULL, observed_at timestamptz NOT NULL DEFAULT now()
);
CREATE TRIGGER immutable_observations BEFORE UPDATE OR DELETE OR TRUNCATE ON integrations.event_observations
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();
CREATE TABLE integrations.webhook_events (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), provider_id uuid NOT NULL REFERENCES integrations.providers,
    provider_account_scope varchar(512) NOT NULL CHECK (provider_account_scope <> ''),
    external_event_id varchar(512) CHECK (external_event_id <> ''), approved_fallback_key varchar(512) CHECK (approved_fallback_key <> ''),
    event_type text NOT NULL, payload_reference text, payload_hash text NOT NULL CHECK (payload_hash <> ''),
    signature_verified boolean NOT NULL DEFAULT false,
    processing_status text NOT NULL CHECK (processing_status IN ('RECEIVED','PENDING','PROCESSED','FAILED','UNDER_REVIEW','REJECTED')),
    received_at timestamptz NOT NULL DEFAULT now(), processed_at timestamptz,
    CHECK (processing_status NOT IN ('PENDING','PROCESSED') OR
        (signature_verified AND (external_event_id IS NOT NULL OR approved_fallback_key IS NOT NULL))),
    CHECK ((processing_status = 'PROCESSED') = (processed_at IS NOT NULL) AND processed_at >= received_at)
);
CREATE UNIQUE INDEX webhook_verified_id ON integrations.webhook_events(provider_id,provider_account_scope,external_event_id)
    WHERE signature_verified AND external_event_id IS NOT NULL;
CREATE UNIQUE INDEX webhook_verified_fallback ON integrations.webhook_events(provider_id,provider_account_scope,approved_fallback_key)
    WHERE signature_verified AND approved_fallback_key IS NOT NULL;
CREATE TABLE integrations.outbox_events (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), aggregate_type text NOT NULL, aggregate_id uuid NOT NULL,
    event_type text NOT NULL, payload jsonb NOT NULL, status text NOT NULL DEFAULT 'PENDING' CHECK (status IN ('PENDING','PUBLISHED')),
    created_at timestamptz NOT NULL DEFAULT now(), published_at timestamptz,
    CHECK ((status = 'PUBLISHED') = (published_at IS NOT NULL))
);
CREATE INDEX outbox_pending ON integrations.outbox_events(created_at,id) WHERE status='PENDING';
CREATE TABLE integrations.jobs (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), job_type text NOT NULL, execution_key text NOT NULL CHECK (execution_key <> ''),
    outbox_event_id uuid REFERENCES integrations.outbox_events, payload jsonb NOT NULL,
    status text NOT NULL DEFAULT 'PENDING' CHECK (status IN ('PENDING','RUNNING','SUCCEEDED','DEAD_LETTER')),
    attempts integer NOT NULL DEFAULT 0, max_attempts integer NOT NULL CHECK (max_attempts > 0),
    available_at timestamptz NOT NULL, lease_token uuid, locked_until timestamptz,
    last_error text, created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (job_type,execution_key), CHECK (attempts BETWEEN 0 AND max_attempts),
    CHECK ((status='RUNNING') = (locked_until IS NOT NULL) AND (status='RUNNING') = (lease_token IS NOT NULL)),
    CHECK (status <> 'RUNNING' OR attempts > 0)
);
CREATE INDEX jobs_pending ON integrations.jobs(available_at,id) WHERE status='PENDING';
CREATE INDEX jobs_expired_leases ON integrations.jobs(locked_until,id) WHERE status='RUNNING';
CREATE TRIGGER immutable_economic_events BEFORE UPDATE OR DELETE OR TRUNCATE ON integrations.economic_events
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();
