-- Phase 5 (IMP-P5-001–017): DEYPAY's authoritative double-entry ledger.
--
-- Owner decisions of 2026-11-26 are binding here: exact integer accounting
-- units, independent USDC/USDT, NGN for settlement only, separate available and
-- reserved customer liability accounts, immutable postings, compensating
-- corrections, idempotent accounting events and evidence-based reservation
-- release. The proposed chart of accounts (ledger spec §7.2) is an operational
-- subledger; statutory mappings remain subject to Finance approval (DEC-015).
--
-- Privilege model (infrastructure/postgres/ledger-grants.psql):
--   * Tables belong to the migration owner. No runtime role receives DML.
--   * Every write runs inside a SECURITY DEFINER function owned by the NOLOGIN
--     role deypay_ledger_owner.
--   * deypay_ledger_poster (workflows), deypay_ledger_controller (finance
--     operations) and deypay_ledger_reader (reporting) receive EXECUTE only on
--     their functions. deypay_app receives nothing.
--
-- Error codes raised by ledger functions (mapped in deypay-infrastructure):
--   LE001 unbalanced journal        LE002 insufficient funds
--   LE003 account not postable      LE004 idempotency conflict
--   LE005 invalid reversal          LE006 invalid input
--   LE007 reservation state         LE008 unknown account/record
--   LE009 customer not eligible     LE010 asset on hold
--   LE011 asset mismatch            LE012 posting not permitted
--   LE013 approval required/denied

CREATE SCHEMA ledger;
CREATE SCHEMA operations;

-- IMP-P5-002: owner-approved accounting precision. Token precision is stored
-- separately per network (wallets.asset_networks.token_decimals).
ALTER TABLE wallets.assets
    ADD CONSTRAINT approved_accounting_precision CHECK (
        accounting_precision <= 18
        AND (symbol NOT IN ('USDC','USDT') OR accounting_precision = 6)
        AND (symbol <> 'NGN' OR accounting_precision = 2));

-- Administrative permissions used by the maker-checker controls.
INSERT INTO audit.admin_permissions(permission_code, description) VALUES
    ('LEDGER_CORRECTION_REQUEST', 'Propose a ledger reversal or adjustment'),
    ('LEDGER_CORRECTION_APPROVE', 'Approve, reject or execute another person''s ledger correction'),
    ('LEDGER_ACCOUNT_CONTROL', 'Open platform accounts, freeze, close or lift ledger holds'),
    ('RECONCILIATION_OPERATE', 'Run reconciliations and investigate discrepancies')
ON CONFLICT (permission_code) DO NOTHING;

---------------------------------------------------------------------------
-- IMP-P5-003: chart of accounts and account instances
---------------------------------------------------------------------------
CREATE TABLE ledger.chart_accounts (
    code text PRIMARY KEY CHECK (code ~ '^[0-9]{4}$'),
    name text NOT NULL CHECK (btrim(name) <> ''),
    account_type text NOT NULL CHECK (account_type IN
        ('ASSET','LIABILITY','REVENUE','EXPENSE','EQUITY','CLEARING','SUSPENSE')),
    normal_balance text NOT NULL CHECK (normal_balance IN ('DEBIT','CREDIT')),
    negative_allowed boolean NOT NULL,
    customer_role text CHECK (customer_role IN ('AVAILABLE','RESERVED')),
    -- NULL means the account may carry any asset (for example the actual fee asset).
    asset_symbols text[] CHECK (asset_symbols IS NULL OR cardinality(asset_symbols) > 0),
    reconciliation_role text CHECK (reconciliation_role IN ('CUSTOMER_LIABILITY','CUSTOMER_BACKING')),
    CHECK (account_type NOT IN ('ASSET','EXPENSE') OR normal_balance = 'DEBIT'),
    CHECK (account_type NOT IN ('LIABILITY','REVENUE','EQUITY') OR normal_balance = 'CREDIT'),
    CHECK (customer_role IS NULL OR (account_type = 'LIABILITY' AND NOT negative_allowed
        AND cardinality(asset_symbols) = 1 AND reconciliation_role = 'CUSTOMER_LIABILITY')),
    UNIQUE (code, account_type, normal_balance, negative_allowed)
);

INSERT INTO ledger.chart_accounts
    (code, name, account_type, normal_balance, negative_allowed, customer_role, asset_symbols, reconciliation_role) VALUES
    ('1100','Custody USDC Assets','ASSET','DEBIT',false,NULL,'{USDC}','CUSTOMER_BACKING'),
    ('1110','Custody USDT Assets','ASSET','DEBIT',false,NULL,'{USDT}','CUSTOMER_BACKING'),
    ('1120','Stablecoin Provider Receivables','ASSET','DEBIT',false,NULL,'{USDC,USDT}','CUSTOMER_BACKING'),
    ('1130','NGN Settlement Bank Assets','ASSET','DEBIT',false,NULL,'{NGN}',NULL),
    ('1140','NGN Provider Settlement Receivables','ASSET','DEBIT',false,NULL,'{NGN}',NULL),
    ('1150','Treasury Stablecoin Assets','ASSET','DEBIT',false,NULL,'{USDC,USDT}',NULL),
    ('1160','Treasury NGN Assets','ASSET','DEBIT',false,NULL,'{NGN}',NULL),
    ('2100','Customer Available USDC','LIABILITY','CREDIT',false,'AVAILABLE','{USDC}','CUSTOMER_LIABILITY'),
    ('2110','Customer Reserved USDC','LIABILITY','CREDIT',false,'RESERVED','{USDC}','CUSTOMER_LIABILITY'),
    ('2120','Customer Available USDT','LIABILITY','CREDIT',false,'AVAILABLE','{USDT}','CUSTOMER_LIABILITY'),
    ('2130','Customer Reserved USDT','LIABILITY','CREDIT',false,'RESERVED','{USDT}','CUSTOMER_LIABILITY'),
    ('2140','Pending Customer Deposit Obligations','LIABILITY','CREDIT',false,NULL,'{USDC,USDT}','CUSTOMER_LIABILITY'),
    ('2150','Pending Customer Refund Obligations','LIABILITY','CREDIT',false,NULL,'{USDC,USDT}','CUSTOMER_LIABILITY'),
    ('2160','NGN Payout Settlement Obligations','LIABILITY','CREDIT',false,NULL,'{NGN}',NULL),
    ('4100','Transfer Fee Revenue','REVENUE','CREDIT',false,NULL,'{USDC,USDT}',NULL),
    ('4110','Withdrawal Fee Revenue','REVENUE','CREDIT',false,NULL,'{USDC,USDT}',NULL),
    ('4120','Conversion Revenue','REVENUE','CREDIT',false,NULL,'{USDC,USDT,NGN}',NULL),
    ('4130','Bank Payment Fee Revenue','REVENUE','CREDIT',false,NULL,'{USDC,USDT,NGN}',NULL),
    ('5100','Blockchain Network Expenses','EXPENSE','DEBIT',false,NULL,NULL,NULL),
    ('5110','Provider Processing Expenses','EXPENSE','DEBIT',false,NULL,NULL,NULL),
    ('5120','Conversion Expenses','EXPENSE','DEBIT',false,NULL,NULL,NULL),
    ('5130','Settlement Expenses','EXPENSE','DEBIT',false,NULL,'{NGN}',NULL),
    ('6100','Stablecoin Conversion Clearing','CLEARING','DEBIT',true,NULL,'{USDC,USDT}',NULL),
    ('6110','NGN Conversion Clearing','CLEARING','DEBIT',true,NULL,'{NGN}',NULL),
    ('6120','Bank Payout Clearing','CLEARING','DEBIT',true,NULL,'{NGN}',NULL),
    ('6130','External Withdrawal Clearing','CLEARING','DEBIT',true,NULL,'{USDC,USDT}',NULL),
    ('6140','Unidentified Deposit Suspense','SUSPENSE','CREDIT',true,NULL,'{USDC,USDT}',NULL),
    ('6150','Reconciliation Suspense','SUSPENSE','DEBIT',true,NULL,NULL,NULL);

CREATE TRIGGER immutable_chart BEFORE UPDATE OR DELETE OR TRUNCATE ON ledger.chart_accounts
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();

CREATE TABLE ledger.accounts (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    chart_code text NOT NULL,
    account_type text NOT NULL,
    normal_balance text NOT NULL,
    negative_allowed boolean NOT NULL,
    asset_id uuid NOT NULL REFERENCES wallets.assets,
    owner_type text NOT NULL CHECK (owner_type IN ('CUSTOMER','PLATFORM')),
    owner_id uuid REFERENCES identity.users,
    wallet_profile_id uuid,
    customer_role text CHECK (customer_role IN ('AVAILABLE','RESERVED')),
    instance_key text NOT NULL DEFAULT 'primary' CHECK (instance_key ~ '^[a-z0-9][a-z0-9._-]{0,63}$'),
    status text NOT NULL DEFAULT 'OPEN' CHECK (status IN ('OPEN','FROZEN','CLOSED')),
    created_at timestamptz NOT NULL DEFAULT now(),
    status_changed_at timestamptz,
    FOREIGN KEY (chart_code, account_type, normal_balance, negative_allowed)
        REFERENCES ledger.chart_accounts(code, account_type, normal_balance, negative_allowed),
    FOREIGN KEY (wallet_profile_id, asset_id) REFERENCES wallets.wallet_profiles(id, asset_id),
    CHECK ((owner_type = 'CUSTOMER') = (owner_id IS NOT NULL)),
    CHECK ((owner_type = 'CUSTOMER') = (wallet_profile_id IS NOT NULL)),
    CHECK ((owner_type = 'CUSTOMER') = (customer_role IS NOT NULL)),
    CHECK (owner_type = 'PLATFORM' OR instance_key = 'primary'),
    UNIQUE (id, asset_id),
    UNIQUE (id, normal_balance, negative_allowed)
);
CREATE UNIQUE INDEX customer_account_role ON ledger.accounts(wallet_profile_id, customer_role)
    WHERE owner_type = 'CUSTOMER';
CREATE UNIQUE INDEX platform_account_instance ON ledger.accounts(chart_code, asset_id, instance_key)
    WHERE owner_type = 'PLATFORM';
CREATE INDEX accounts_owner ON ledger.accounts(owner_id, asset_id) WHERE owner_type = 'CUSTOMER';

-- The chart decides what an account may be; only the status can change later.
CREATE FUNCTION ledger.guard_account() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    chart ledger.chart_accounts%ROWTYPE;
    symbol text;
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'ledger accounts cannot be deleted' USING ERRCODE = '55000';
    END IF;
    IF TG_OP = 'UPDATE' THEN
        IF (NEW.id, NEW.chart_code, NEW.account_type, NEW.normal_balance, NEW.negative_allowed, NEW.asset_id,
            NEW.owner_type, NEW.owner_id, NEW.wallet_profile_id, NEW.customer_role, NEW.instance_key, NEW.created_at)
           IS DISTINCT FROM
           (OLD.id, OLD.chart_code, OLD.account_type, OLD.normal_balance, OLD.negative_allowed, OLD.asset_id,
            OLD.owner_type, OLD.owner_id, OLD.wallet_profile_id, OLD.customer_role, OLD.instance_key, OLD.created_at) THEN
            RAISE EXCEPTION 'only a ledger account''s status can change' USING ERRCODE = '55000';
        END IF;
        IF OLD.status = 'CLOSED' AND NEW.status <> 'CLOSED' THEN
            RAISE EXCEPTION 'a closed ledger account cannot reopen' USING ERRCODE = '55000';
        END IF;
        RETURN NEW;
    END IF;
    SELECT * INTO chart FROM ledger.chart_accounts c WHERE c.code = NEW.chart_code;
    SELECT a.symbol INTO symbol FROM wallets.assets a WHERE a.id = NEW.asset_id;
    IF chart.customer_role IS DISTINCT FROM NEW.customer_role THEN
        RAISE EXCEPTION 'account role does not match chart account %', NEW.chart_code USING ERRCODE = '23514';
    END IF;
    IF chart.asset_symbols IS NOT NULL AND NOT (symbol = ANY (chart.asset_symbols)) THEN
        RAISE EXCEPTION 'chart account % does not carry %', NEW.chart_code, symbol USING ERRCODE = '23514';
    END IF;
    IF NEW.owner_type = 'CUSTOMER' AND NOT EXISTS (
        SELECT 1 FROM wallets.wallet_profiles w
        WHERE w.id = NEW.wallet_profile_id AND w.user_id = NEW.owner_id AND w.asset_id = NEW.asset_id) THEN
        RAISE EXCEPTION 'customer account must belong to the wallet profile owner' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER guard_account BEFORE INSERT OR UPDATE OR DELETE ON ledger.accounts
    FOR EACH ROW EXECUTE FUNCTION ledger.guard_account();
CREATE TRIGGER no_account_truncate BEFORE TRUNCATE ON ledger.accounts
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();

---------------------------------------------------------------------------
-- IMP-P5-012: balance projections (normal-side signed balance)
---------------------------------------------------------------------------
CREATE TABLE ledger.balances (
    account_id uuid PRIMARY KEY,
    normal_balance text NOT NULL,
    negative_allowed boolean NOT NULL,
    debit_total numeric NOT NULL DEFAULT 0 CHECK (debit_total >= 0 AND debit_total = trunc(debit_total)),
    credit_total numeric NOT NULL DEFAULT 0 CHECK (credit_total >= 0 AND credit_total = trunc(credit_total)),
    balance_minor numeric NOT NULL DEFAULT 0,
    version bigint NOT NULL DEFAULT 0 CHECK (version >= 0),
    last_journal_id uuid,
    updated_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (account_id, normal_balance, negative_allowed)
        REFERENCES ledger.accounts(id, normal_balance, negative_allowed),
    CHECK (balance_minor = CASE normal_balance WHEN 'CREDIT' THEN credit_total - debit_total
        ELSE debit_total - credit_total END),
    -- The database itself refuses an overdrawn customer or asset account.
    CHECK (negative_allowed OR balance_minor >= 0),
    CHECK (abs(balance_minor) < 1e38)
);

CREATE FUNCTION ledger.open_balance() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
BEGIN
    INSERT INTO ledger.balances(account_id, normal_balance, negative_allowed)
        VALUES (NEW.id, NEW.normal_balance, NEW.negative_allowed);
    RETURN NULL;
END;
$$;
CREATE TRIGGER open_balance AFTER INSERT ON ledger.accounts
    FOR EACH ROW EXECUTE FUNCTION ledger.open_balance();
CREATE TRIGGER no_balance_delete BEFORE DELETE OR TRUNCATE ON ledger.balances
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();

---------------------------------------------------------------------------
-- IMP-P5-004/005/013: accounting events, journals and entries
---------------------------------------------------------------------------
CREATE TABLE ledger.accounting_events (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    event_type text NOT NULL CHECK (event_type ~ '^[A-Z][A-Z0-9_]{2,63}$'),
    economic_event_key text NOT NULL CHECK (length(economic_event_key) BETWEEN 1 AND 256
        AND economic_event_key !~ '[[:cntrl:]]'),
    posting_purpose text NOT NULL CHECK (posting_purpose ~ '^[A-Z][A-Z0-9_:.-]{0,63}$'),
    request_hash bytea NOT NULL CHECK (length(request_hash) = 32),
    source_transaction_id uuid REFERENCES payments.transactions,
    actor_type text NOT NULL CHECK (actor_type IN ('SYSTEM','SERVICE','CUSTOMER','ADMIN')),
    actor_id uuid,
    correlation_id uuid,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (economic_event_key, posting_purpose)
);

CREATE SEQUENCE ledger.journal_sequence AS bigint MINVALUE 1 NO CYCLE;

CREATE TABLE ledger.journals (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    posting_sequence bigint NOT NULL UNIQUE,
    journal_reference text NOT NULL UNIQUE CHECK (journal_reference ~ '^JNL-[0-9]{12,}$'),
    accounting_event_id uuid NOT NULL UNIQUE REFERENCES ledger.accounting_events,
    event_type text NOT NULL,
    source_transaction_id uuid REFERENCES payments.transactions,
    status text NOT NULL DEFAULT 'POSTED' CHECK (status = 'POSTED'),
    reversal_of_journal_id uuid UNIQUE REFERENCES ledger.journals,
    entry_count integer NOT NULL CHECK (entry_count BETWEEN 2 AND 64),
    description text CHECK (description IS NULL OR (length(description) <= 500 AND description !~ '[[:cntrl:]]')),
    posted_at timestamptz NOT NULL DEFAULT now(),
    CHECK (reversal_of_journal_id IS DISTINCT FROM id)
);
CREATE INDEX journals_source_transaction ON ledger.journals(source_transaction_id)
    WHERE source_transaction_id IS NOT NULL;
CREATE INDEX journals_posted ON ledger.journals(posted_at, posting_sequence);

CREATE TABLE ledger.entries (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    journal_id uuid NOT NULL REFERENCES ledger.journals,
    line_number smallint NOT NULL CHECK (line_number BETWEEN 1 AND 64),
    account_id uuid NOT NULL,
    asset_id uuid NOT NULL,
    direction text NOT NULL CHECK (direction IN ('DEBIT','CREDIT')),
    amount_minor payments.minor_units NOT NULL CHECK (amount_minor > 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (account_id, asset_id) REFERENCES ledger.accounts(id, asset_id),
    UNIQUE (journal_id, account_id),
    UNIQUE (journal_id, line_number)
);
CREATE INDEX entries_account ON ledger.entries(account_id, journal_id);

-- IMP-P5-006: no journal can commit unless it has its declared entries and
-- balances independently for every asset. The check runs at commit, after all
-- entries exist, for any writer (including the definer functions).
-- SECURITY DEFINER because deferred checks fire at commit under the caller's role.
CREATE FUNCTION ledger.assert_journal_balanced() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    target uuid;
    declared integer;
    actual integer;
BEGIN
    IF TG_TABLE_NAME = 'journals' THEN
        target := NEW.id;
    ELSE
        target := NEW.journal_id;
    END IF;
    SELECT j.entry_count INTO declared FROM ledger.journals j WHERE j.id = target;
    SELECT count(*) INTO actual FROM ledger.entries e WHERE e.journal_id = target;
    IF declared IS NULL OR actual <> declared OR actual < 2 THEN
        RAISE EXCEPTION 'journal % has % of % entries', target, actual, declared USING ERRCODE = 'LE001';
    END IF;
    IF EXISTS (SELECT 1 FROM ledger.entries e WHERE e.journal_id = target GROUP BY e.asset_id
               HAVING sum(CASE e.direction WHEN 'DEBIT' THEN e.amount_minor ELSE -e.amount_minor END) <> 0) THEN
        RAISE EXCEPTION 'journal % does not balance for each asset', target USING ERRCODE = 'LE001';
    END IF;
    RETURN NULL;
END;
$$;
CREATE CONSTRAINT TRIGGER journal_balanced AFTER INSERT ON ledger.journals
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION ledger.assert_journal_balanced();
CREATE CONSTRAINT TRIGGER entry_balanced AFTER INSERT ON ledger.entries
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION ledger.assert_journal_balanced();

-- Posted history is append-only for every role.
CREATE TRIGGER immutable_events BEFORE UPDATE OR DELETE OR TRUNCATE ON ledger.accounting_events
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();
CREATE TRIGGER immutable_journals BEFORE UPDATE OR DELETE OR TRUNCATE ON ledger.journals
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();
CREATE TRIGGER immutable_entries BEFORE UPDATE OR DELETE OR TRUNCATE ON ledger.entries
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();

---------------------------------------------------------------------------
-- IMP-P5-010/011: reservations
---------------------------------------------------------------------------
CREATE TABLE ledger.reservations (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    operation_key text NOT NULL UNIQUE CHECK (length(operation_key) BETWEEN 1 AND 256
        AND operation_key !~ '[[:cntrl:]]'),
    transaction_id uuid REFERENCES payments.transactions,
    user_id uuid NOT NULL REFERENCES identity.users,
    asset_id uuid NOT NULL,
    available_account_id uuid NOT NULL,
    reserved_account_id uuid NOT NULL,
    amount_minor payments.minor_units NOT NULL CHECK (amount_minor > 0),
    consumed_minor numeric NOT NULL DEFAULT 0,
    released_minor numeric NOT NULL DEFAULT 0,
    remaining_minor numeric NOT NULL,
    status text NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE','SETTLED','RELEASED')),
    execution_state text NOT NULL DEFAULT 'NOT_SUBMITTED'
        CHECK (execution_state IN ('NOT_SUBMITTED','SUBMITTED','UNKNOWN','EXECUTED')),
    reserve_journal_id uuid NOT NULL UNIQUE REFERENCES ledger.journals,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    released_at timestamptz,
    settled_at timestamptz,
    FOREIGN KEY (available_account_id, asset_id) REFERENCES ledger.accounts(id, asset_id),
    FOREIGN KEY (reserved_account_id, asset_id) REFERENCES ledger.accounts(id, asset_id),
    CHECK (consumed_minor >= 0 AND consumed_minor = trunc(consumed_minor)),
    CHECK (released_minor >= 0 AND released_minor = trunc(released_minor)),
    CHECK (remaining_minor >= 0 AND remaining_minor = trunc(remaining_minor)),
    CHECK (consumed_minor + released_minor + remaining_minor = amount_minor),
    CHECK ((status = 'ACTIVE') = (remaining_minor > 0)),
    CHECK (status <> 'RELEASED' OR consumed_minor = 0),
    CHECK (status <> 'SETTLED' OR consumed_minor > 0),
    CHECK ((released_at IS NOT NULL) = (released_minor > 0)),
    CHECK ((settled_at IS NOT NULL) = (consumed_minor > 0)),
    CHECK (updated_at >= created_at)
);
CREATE INDEX reservations_active ON ledger.reservations(reserved_account_id) WHERE status = 'ACTIVE';
CREATE INDEX reservations_transaction ON ledger.reservations(transaction_id) WHERE transaction_id IS NOT NULL;
CREATE TRIGGER no_reservation_delete BEFORE DELETE OR TRUNCATE ON ledger.reservations
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();

CREATE TABLE ledger.reservation_movements (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    reservation_id uuid NOT NULL REFERENCES ledger.reservations,
    journal_id uuid NOT NULL UNIQUE REFERENCES ledger.journals,
    movement text NOT NULL CHECK (movement IN ('RESERVE','CONSUME','RELEASE')),
    amount_minor payments.minor_units NOT NULL CHECK (amount_minor > 0),
    evidence_type text CHECK (evidence_type IN
        ('VERIFIED_SETTLEMENT','DEFINITIVE_NON_EXECUTION','PERMITTED_CANCELLATION','SETTLEMENT_REMAINDER')),
    evidence_reference text CHECK (evidence_reference IS NULL OR (length(evidence_reference) BETWEEN 1 AND 512
        AND evidence_reference !~ '[[:cntrl:]]')),
    created_at timestamptz NOT NULL DEFAULT now(),
    CHECK ((movement = 'RESERVE') = (evidence_type IS NULL)),
    CHECK ((movement = 'RESERVE') = (evidence_reference IS NULL)),
    CHECK (movement <> 'CONSUME' OR evidence_type = 'VERIFIED_SETTLEMENT'),
    CHECK (movement <> 'RELEASE' OR evidence_type <> 'VERIFIED_SETTLEMENT')
);
CREATE INDEX reservation_movements_reservation ON ledger.reservation_movements(reservation_id, created_at);
CREATE TRIGGER immutable_reservation_movements BEFORE UPDATE OR DELETE OR TRUNCATE ON ledger.reservation_movements
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();

CREATE TABLE ledger.reservation_execution_history (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    reservation_id uuid NOT NULL REFERENCES ledger.reservations,
    previous_state text NOT NULL,
    new_state text NOT NULL,
    evidence_reference text NOT NULL CHECK (length(evidence_reference) BETWEEN 1 AND 512
        AND evidence_reference !~ '[[:cntrl:]]'),
    recorded_at timestamptz NOT NULL DEFAULT now()
);
CREATE TRIGGER immutable_execution_history BEFORE UPDATE OR DELETE OR TRUNCATE ON ledger.reservation_execution_history
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();

---------------------------------------------------------------------------
-- IMP-P5-014: corrections (maker-checker), recovery cases and asset holds
---------------------------------------------------------------------------
CREATE TABLE ledger.recovery_cases (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    original_journal_id uuid NOT NULL REFERENCES ledger.journals,
    reason text NOT NULL CHECK (btrim(reason) <> ''),
    shortfall jsonb NOT NULL CHECK (jsonb_typeof(shortfall) = 'array' AND jsonb_array_length(shortfall) > 0),
    status text NOT NULL DEFAULT 'OPEN' CHECK (status IN ('OPEN','RESOLVED')),
    resolution_journal_id uuid REFERENCES ledger.journals,
    resolution_notes text,
    resolved_by uuid REFERENCES identity.users,
    created_at timestamptz NOT NULL DEFAULT now(),
    resolved_at timestamptz,
    CHECK ((status = 'RESOLVED') = (resolved_at IS NOT NULL)),
    CHECK ((status = 'RESOLVED') = (resolution_notes IS NOT NULL AND btrim(resolution_notes) <> '')),
    CHECK ((status = 'RESOLVED') = (resolved_by IS NOT NULL))
);
CREATE TRIGGER no_recovery_delete BEFORE DELETE OR TRUNCATE ON ledger.recovery_cases
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();

CREATE TABLE ledger.correction_requests (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    correction_type text NOT NULL CHECK (correction_type IN ('REVERSAL','ADJUSTMENT')),
    target_journal_id uuid REFERENCES ledger.journals,
    lines jsonb CHECK (lines IS NULL OR jsonb_typeof(lines) = 'array'),
    reason text NOT NULL CHECK (btrim(reason) <> '' AND length(reason) <= 1000),
    evidence_reference text NOT NULL CHECK (length(evidence_reference) BETWEEN 1 AND 512
        AND evidence_reference !~ '[[:cntrl:]]'),
    requested_by uuid NOT NULL REFERENCES identity.users,
    decided_by uuid REFERENCES identity.users,
    decision_reason text,
    status text NOT NULL DEFAULT 'PENDING'
        CHECK (status IN ('PENDING','APPROVED','REJECTED','EXECUTED','RECOVERY_REQUIRED')),
    result_journal_id uuid UNIQUE REFERENCES ledger.journals,
    recovery_case_id uuid UNIQUE REFERENCES ledger.recovery_cases,
    executed_by uuid REFERENCES identity.users,
    created_at timestamptz NOT NULL DEFAULT now(),
    decided_at timestamptz,
    executed_at timestamptz,
    -- Maker-checker: nobody approves their own correction.
    CHECK (decided_by IS DISTINCT FROM requested_by),
    CHECK ((correction_type = 'REVERSAL') = (target_journal_id IS NOT NULL)),
    CHECK ((correction_type = 'ADJUSTMENT') = (lines IS NOT NULL)),
    CHECK ((status = 'PENDING') = (decided_by IS NULL)),
    CHECK ((status = 'PENDING') = (decided_at IS NULL)),
    CHECK (status = 'PENDING' OR (decision_reason IS NOT NULL AND btrim(decision_reason) <> '')),
    CHECK ((status = 'EXECUTED') = (result_journal_id IS NOT NULL)),
    CHECK ((status = 'RECOVERY_REQUIRED') = (recovery_case_id IS NOT NULL)),
    CHECK ((status IN ('EXECUTED','RECOVERY_REQUIRED')) = (executed_at IS NOT NULL)),
    CHECK ((executed_at IS NOT NULL) = (executed_by IS NOT NULL))
);
CREATE UNIQUE INDEX one_open_reversal ON ledger.correction_requests(target_journal_id)
    WHERE correction_type = 'REVERSAL' AND status IN ('PENDING','APPROVED','EXECUTED','RECOVERY_REQUIRED');
CREATE TRIGGER no_correction_delete BEFORE DELETE OR TRUNCATE ON ledger.correction_requests
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();

CREATE TABLE ledger.asset_holds (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    asset_id uuid NOT NULL REFERENCES wallets.assets,
    reason text NOT NULL CHECK (btrim(reason) <> ''),
    reconciliation_case_id uuid,
    active boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT now(),
    lifted_at timestamptz,
    lifted_by uuid REFERENCES identity.users,
    lift_reason text,
    CHECK (active = (lifted_at IS NULL)),
    CHECK (active = (lifted_by IS NULL)),
    CHECK (active OR (lift_reason IS NOT NULL AND btrim(lift_reason) <> ''))
);
CREATE INDEX asset_holds_active ON ledger.asset_holds(asset_id) WHERE active;
CREATE TRIGGER no_hold_delete BEFORE DELETE OR TRUNCATE ON ledger.asset_holds
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();

---------------------------------------------------------------------------
-- IMP-P5-015: reconciliation foundations
---------------------------------------------------------------------------
CREATE TABLE operations.reconciliation_runs (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    provider_id uuid REFERENCES integrations.providers,
    reconciliation_type text NOT NULL CHECK (reconciliation_type IN
        ('LEDGER_INTEGRITY','CUSTOMER_LIABILITY','PROVIDER_BALANCE','PROVIDER_TRANSACTIONS','SETTLEMENT')),
    period_start timestamptz NOT NULL,
    period_end timestamptz NOT NULL,
    status text NOT NULL DEFAULT 'RUNNING'
        CHECK (status IN ('RUNNING','COMPLETED','COMPLETED_WITH_DISCREPANCIES','FAILED')),
    started_by uuid NOT NULL REFERENCES identity.users,
    started_at timestamptz NOT NULL DEFAULT now(),
    completed_at timestamptz,
    CHECK (period_end > period_start),
    CHECK ((status = 'RUNNING') = (completed_at IS NULL))
);
CREATE INDEX reconciliation_runs_period ON operations.reconciliation_runs(provider_id, period_start, period_end);
CREATE TRIGGER no_run_delete BEFORE DELETE OR TRUNCATE ON operations.reconciliation_runs
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();

CREATE TABLE operations.reconciliation_items (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    reconciliation_run_id uuid NOT NULL REFERENCES operations.reconciliation_runs,
    internal_transaction_id uuid REFERENCES payments.transactions,
    journal_id uuid REFERENCES ledger.journals,
    ledger_account_id uuid REFERENCES ledger.accounts,
    provider_operation_id uuid REFERENCES integrations.provider_operations,
    external_evidence_reference text CHECK (external_evidence_reference IS NULL
        OR (length(external_evidence_reference) BETWEEN 1 AND 512 AND external_evidence_reference !~ '[[:cntrl:]]')),
    evidence_sha256 bytea CHECK (evidence_sha256 IS NULL OR length(evidence_sha256) = 32),
    evidence_observed_at timestamptz,
    observed_asset_id uuid REFERENCES wallets.assets,
    observed_amount_minor numeric CHECK (observed_amount_minor IS NULL OR observed_amount_minor = trunc(observed_amount_minor)),
    expected_amount_minor numeric CHECK (expected_amount_minor IS NULL OR expected_amount_minor = trunc(expected_amount_minor)),
    match_status text NOT NULL CHECK (match_status IN ('MATCHED','MISSING_INTERNAL','MISSING_EXTERNAL',
        'AMOUNT_MISMATCH','ASSET_MISMATCH','DUPLICATE','SHORTFALL','SURPLUS','INTEGRITY_VIOLATION')),
    discrepancy_type text,
    details jsonb NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(details) = 'object'),
    created_at timestamptz NOT NULL DEFAULT now(),
    CHECK ((match_status = 'MATCHED') = (discrepancy_type IS NULL))
);
CREATE INDEX reconciliation_items_run ON operations.reconciliation_items(reconciliation_run_id, match_status);
CREATE INDEX reconciliation_items_reference ON operations.reconciliation_items(reconciliation_run_id, external_evidence_reference)
    WHERE external_evidence_reference IS NOT NULL;
CREATE TRIGGER immutable_reconciliation_items BEFORE UPDATE OR DELETE OR TRUNCATE ON operations.reconciliation_items
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();

CREATE TABLE operations.reconciliation_cases (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    reconciliation_item_id uuid NOT NULL UNIQUE REFERENCES operations.reconciliation_items,
    severity text NOT NULL CHECK (severity IN ('LOW','MEDIUM','HIGH','CRITICAL')),
    status text NOT NULL DEFAULT 'OPEN' CHECK (status IN ('OPEN','INVESTIGATING','RESOLVED')),
    impact_asset_id uuid REFERENCES wallets.assets,
    financial_impact_minor numeric CHECK (financial_impact_minor IS NULL OR financial_impact_minor = trunc(financial_impact_minor)),
    assigned_to uuid REFERENCES identity.users,
    resolution_notes text,
    resolution_evidence text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    resolved_at timestamptz,
    CHECK ((status = 'RESOLVED') = (resolved_at IS NOT NULL)),
    CHECK (status <> 'RESOLVED' OR (btrim(coalesce(resolution_notes,'')) <> '' AND btrim(coalesce(resolution_evidence,'')) <> ''))
);
CREATE INDEX reconciliation_cases_open ON operations.reconciliation_cases(severity, created_at) WHERE status <> 'RESOLVED';
CREATE TRIGGER no_case_delete BEFORE DELETE OR TRUNCATE ON operations.reconciliation_cases
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();
ALTER TABLE ledger.asset_holds ADD FOREIGN KEY (reconciliation_case_id) REFERENCES operations.reconciliation_cases;

CREATE TABLE operations.reconciliation_case_events (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id uuid NOT NULL REFERENCES operations.reconciliation_cases,
    actor_id uuid REFERENCES identity.users,
    previous_status text,
    new_status text NOT NULL,
    note text NOT NULL CHECK (btrim(note) <> ''),
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX reconciliation_case_events_case ON operations.reconciliation_case_events(case_id, created_at);
CREATE TRIGGER immutable_case_events BEFORE UPDATE OR DELETE OR TRUNCATE ON operations.reconciliation_case_events
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();

---------------------------------------------------------------------------
-- Internal helpers (owner-only; never granted)
---------------------------------------------------------------------------
-- IMP-P5-001: positive integer accounting units within the 38-digit range,
-- written without sign, exponent, fraction or leading zeros.
CREATE FUNCTION ledger.parse_amount(p_amount text) RETURNS numeric
LANGUAGE plpgsql IMMUTABLE SET search_path = pg_catalog, pg_temp AS $$
BEGIN
    IF p_amount IS NULL OR p_amount !~ '^[1-9][0-9]{0,37}$' THEN
        RAISE EXCEPTION 'amount must be a positive whole number of accounting units' USING ERRCODE = 'LE006';
    END IF;
    RETURN p_amount::numeric;
END;
$$;

-- Unambiguous request fingerprint for idempotent replay.
CREATE FUNCTION ledger.fingerprint(VARIADIC p_parts text[]) RETURNS bytea
LANGUAGE sql IMMUTABLE SET search_path = pg_catalog, pg_temp AS $$
    SELECT sha256(convert_to(to_jsonb(p_parts)::text, 'UTF8'));
$$;

CREATE FUNCTION ledger.require_key(p_value text, p_label text) RETURNS void
LANGUAGE plpgsql IMMUTABLE SET search_path = pg_catalog, pg_temp AS $$
BEGIN
    IF p_value IS NULL OR length(p_value) NOT BETWEEN 1 AND 256 OR p_value ~ '[[:cntrl:]]' THEN
        RAISE EXCEPTION '% must be 1–256 printable characters', p_label USING ERRCODE = 'LE006';
    END IF;
END;
$$;

CREATE FUNCTION ledger.require_actor(p_actor_type text) RETURNS void
LANGUAGE plpgsql IMMUTABLE SET search_path = pg_catalog, pg_temp AS $$
BEGIN
    IF p_actor_type IS NULL OR p_actor_type NOT IN ('SYSTEM','SERVICE','CUSTOMER','ADMIN') THEN
        RAISE EXCEPTION 'unknown actor type' USING ERRCODE = 'LE006';
    END IF;
END;
$$;

-- IMP-P5-013: claims an accounting event identity. Waits for any concurrent
-- claim of the same identity, then returns the journal already posted for an
-- identical request, NULL when unclaimed, or raises on a conflicting reuse.
CREATE FUNCTION ledger.claim_event(p_key text, p_purpose text, p_hash bytea) RETURNS uuid
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    stored bytea;
    posted uuid;
BEGIN
    PERFORM ledger.require_key(p_key, 'economic event key');
    IF p_purpose IS NULL OR p_purpose !~ '^[A-Z][A-Z0-9_:.-]{0,63}$' THEN
        RAISE EXCEPTION 'invalid posting purpose' USING ERRCODE = 'LE006';
    END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended('deypay.ledger.event|' || p_purpose || '|' || p_key, 0));
    SELECT e.request_hash, j.id INTO stored, posted
    FROM ledger.accounting_events e JOIN ledger.journals j ON j.accounting_event_id = e.id
    WHERE e.economic_event_key = p_key AND e.posting_purpose = p_purpose;
    IF NOT FOUND THEN
        RETURN NULL;
    END IF;
    IF stored <> p_hash THEN
        RAISE EXCEPTION 'accounting event %/% was already posted with a different request', p_purpose, p_key
            USING ERRCODE = 'LE004';
    END IF;
    RETURN posted;
END;
$$;

-- IMP-P5-007/016: the single posting path. Locks every affected balance in
-- account-id order (a global lock order, so concurrent journals cannot
-- deadlock), validates accounts, statuses, permissions and funds, then writes
-- the event, journal, entries, projections and outbox record together.
--
-- p_mode decides which customer movements are permitted:
--   SYSTEM            no customer debits and no reserved-account movement
--   CUSTOMER_TRANSFER customer available debits (eligibility checked by caller)
--   RESERVATION       reserved-account movements (reservation functions only)
--   CORRECTION        approved corrections; never reserved accounts
CREATE FUNCTION ledger.apply_journal(
    p_mode text, p_event_type text, p_key text, p_purpose text, p_hash bytea,
    p_source_transaction uuid, p_reversal_of uuid,
    p_accounts uuid[], p_assets uuid[], p_directions text[], p_amounts numeric[],
    p_description text, p_actor_type text, p_actor_id uuid, p_correlation uuid,
    p_report_shortfall boolean)
RETURNS TABLE(journal_id uuid, accounting_event_id uuid, posted_at timestamptz, replayed boolean, shortfall jsonb)
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp SET lock_timeout = '10s' AS $$
#variable_conflict use_column
DECLARE
    lines integer := cardinality(p_accounts);
    existing uuid;
    new_event uuid;
    new_journal uuid;
    new_posted timestamptz;
    sequence_number bigint;
    missing jsonb := '[]'::jsonb;
    line record;
    decreases boolean;
    projected numeric;
BEGIN
    IF p_mode NOT IN ('SYSTEM','CUSTOMER_TRANSFER','RESERVATION','CORRECTION') THEN
        RAISE EXCEPTION 'unknown posting mode' USING ERRCODE = 'LE012';
    END IF;
    existing := ledger.claim_event(p_key, p_purpose, p_hash);
    IF existing IS NOT NULL THEN
        RETURN QUERY SELECT j.id, j.accounting_event_id, j.posted_at, true, NULL::jsonb
            FROM ledger.journals j WHERE j.id = existing;
        RETURN;
    END IF;
    IF p_event_type IS NULL OR p_event_type !~ '^[A-Z][A-Z0-9_]{2,63}$' THEN
        RAISE EXCEPTION 'invalid accounting event type' USING ERRCODE = 'LE006';
    END IF;
    PERFORM ledger.require_actor(p_actor_type);
    IF p_description IS NOT NULL AND (length(p_description) > 500 OR p_description ~ '[[:cntrl:]]') THEN
        RAISE EXCEPTION 'description is too long or contains control characters' USING ERRCODE = 'LE006';
    END IF;
    IF lines IS NULL OR lines < 2 OR lines > 64
       OR cardinality(p_assets) IS DISTINCT FROM lines
       OR cardinality(p_directions) IS DISTINCT FROM lines
       OR cardinality(p_amounts) IS DISTINCT FROM lines THEN
        RAISE EXCEPTION 'a journal needs 2–64 aligned lines' USING ERRCODE = 'LE006';
    END IF;
    FOR i IN 1 .. lines LOOP
        IF p_accounts[i] IS NULL OR p_assets[i] IS NULL OR p_directions[i] IS NULL
           OR p_directions[i] NOT IN ('DEBIT','CREDIT') OR p_amounts[i] IS NULL
           OR p_amounts[i] <= 0 OR p_amounts[i] <> trunc(p_amounts[i]) OR p_amounts[i] >= 1e38 THEN
            RAISE EXCEPTION 'line % is incomplete or has an invalid amount', i USING ERRCODE = 'LE006';
        END IF;
    END LOOP;
    IF (SELECT count(DISTINCT a) FROM unnest(p_accounts) AS a) <> lines THEN
        RAISE EXCEPTION 'an account may appear only once in a journal' USING ERRCODE = 'LE006';
    END IF;
    IF EXISTS (SELECT 1 FROM unnest(p_assets, p_directions, p_amounts) AS l(asset, dir, amount)
               GROUP BY l.asset HAVING sum(CASE l.dir WHEN 'DEBIT' THEN l.amount ELSE -l.amount END) <> 0) THEN
        RAISE EXCEPTION 'journal does not balance for each asset' USING ERRCODE = 'LE001';
    END IF;

    PERFORM 1 FROM ledger.balances b WHERE b.account_id = ANY (p_accounts) ORDER BY b.account_id FOR UPDATE;

    FOR line IN
        SELECT l.account, l.asset, l.dir, l.amount, a.id AS found, a.asset_id, a.status, a.owner_type,
               a.customer_role, a.normal_balance, a.negative_allowed, b.balance_minor
        FROM unnest(p_accounts, p_assets, p_directions, p_amounts) AS l(account, asset, dir, amount)
        LEFT JOIN ledger.accounts a ON a.id = l.account
        LEFT JOIN ledger.balances b ON b.account_id = l.account
        ORDER BY l.account
    LOOP
        IF line.found IS NULL THEN
            RAISE EXCEPTION 'unknown ledger account %', line.account USING ERRCODE = 'LE008';
        END IF;
        IF line.asset_id <> line.asset THEN
            RAISE EXCEPTION 'ledger account % does not carry the line asset', line.account USING ERRCODE = 'LE011';
        END IF;
        decreases := (line.normal_balance = 'CREDIT') = (line.dir = 'DEBIT');
        IF line.status = 'CLOSED' OR (line.status = 'FROZEN' AND decreases AND p_mode <> 'CORRECTION') THEN
            RAISE EXCEPTION 'ledger account % is %', line.account, lower(line.status) USING ERRCODE = 'LE003';
        END IF;
        IF line.owner_type = 'CUSTOMER' THEN
            IF line.customer_role = 'RESERVED' AND p_mode <> 'RESERVATION' THEN
                RAISE EXCEPTION 'customer reserved balances change only through reservations' USING ERRCODE = 'LE012';
            END IF;
            IF decreases AND p_mode = 'SYSTEM' THEN
                RAISE EXCEPTION 'system postings cannot reduce customer funds' USING ERRCODE = 'LE012';
            END IF;
        END IF;
        projected := line.balance_minor + CASE WHEN decreases THEN -line.amount ELSE line.amount END;
        IF abs(projected) >= 1e38 THEN
            RAISE EXCEPTION 'ledger account % would exceed the accounting range', line.account USING ERRCODE = 'LE006';
        END IF;
        IF NOT line.negative_allowed AND projected < 0 THEN
            IF p_report_shortfall THEN
                missing := missing || jsonb_build_object('account_id', line.account, 'asset_id', line.asset,
                    'required_minor', line.amount::text, 'balance_minor', line.balance_minor::text,
                    'shortfall_minor', (-projected)::text);
            ELSE
                RAISE EXCEPTION 'insufficient funds in ledger account %', line.account USING ERRCODE = 'LE002';
            END IF;
        END IF;
    END LOOP;
    IF jsonb_array_length(missing) > 0 THEN
        RETURN QUERY SELECT NULL::uuid, NULL::uuid, NULL::timestamptz, false, missing;
        RETURN;
    END IF;

    INSERT INTO ledger.accounting_events(event_type, economic_event_key, posting_purpose, request_hash,
        source_transaction_id, actor_type, actor_id, correlation_id)
    VALUES (p_event_type, p_key, p_purpose, p_hash, p_source_transaction, p_actor_type, p_actor_id, p_correlation)
    RETURNING id INTO new_event;
    sequence_number := nextval('ledger.journal_sequence');
    INSERT INTO ledger.journals(posting_sequence, journal_reference, accounting_event_id, event_type,
        source_transaction_id, reversal_of_journal_id, entry_count, description)
    VALUES (sequence_number, 'JNL-' || lpad(sequence_number::text, 12, '0'), new_event, p_event_type,
        p_source_transaction, p_reversal_of, lines, p_description)
    RETURNING id, posted_at INTO new_journal, new_posted;
    INSERT INTO ledger.entries(journal_id, line_number, account_id, asset_id, direction, amount_minor)
    SELECT new_journal, l.n, l.account, l.asset, l.dir, l.amount
    FROM unnest(p_accounts, p_assets, p_directions, p_amounts) WITH ORDINALITY AS l(account, asset, dir, amount, n);
    UPDATE ledger.balances b SET
        debit_total = b.debit_total + CASE WHEN l.dir = 'DEBIT' THEN l.amount ELSE 0 END,
        credit_total = b.credit_total + CASE WHEN l.dir = 'CREDIT' THEN l.amount ELSE 0 END,
        balance_minor = b.balance_minor + CASE WHEN (b.normal_balance = 'CREDIT') = (l.dir = 'CREDIT')
            THEN l.amount ELSE -l.amount END,
        version = b.version + 1,
        last_journal_id = new_journal,
        updated_at = now()
    FROM unnest(p_accounts, p_directions, p_amounts) AS l(account, dir, amount)
    WHERE b.account_id = l.account;
    INSERT INTO integrations.outbox_events(aggregate_type, aggregate_id, event_type, payload)
    VALUES ('ledger.journal', new_journal, 'LEDGER_JOURNAL_POSTED', jsonb_build_object(
        'journal_id', new_journal, 'accounting_event_id', new_event, 'event_type', p_event_type,
        'posting_sequence', sequence_number, 'source_transaction_id', p_source_transaction));
    RETURN QUERY SELECT new_journal, new_event, new_posted, false, NULL::jsonb;
END;
$$;

CREATE FUNCTION ledger.customer_accounts(p_user uuid, p_asset uuid)
RETURNS TABLE(wallet_profile_id uuid, available_account_id uuid, reserved_account_id uuid)
LANGUAGE plpgsql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_column
BEGIN
    RETURN QUERY
    SELECT w.id, av.id, rs.id
    FROM wallets.wallet_profiles w
    JOIN ledger.accounts av ON av.wallet_profile_id = w.id AND av.owner_type = 'CUSTOMER' AND av.customer_role = 'AVAILABLE'
    JOIN ledger.accounts rs ON rs.wallet_profile_id = w.id AND rs.owner_type = 'CUSTOMER' AND rs.customer_role = 'RESERVED'
    WHERE w.user_id = p_user AND w.asset_id = p_asset;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'customer has no ledger accounts for this asset' USING ERRCODE = 'LE008';
    END IF;
END;
$$;

-- Fail-closed minimum for spending customer funds. Workflow eligibility
-- (limits, screening, step-up) is enforced by the application gate in addition.
CREATE FUNCTION ledger.require_spend_eligibility(p_user uuid, p_wallet uuid, p_asset uuid) RETURNS void
LANGUAGE plpgsql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM identity.users u WHERE u.id = p_user AND u.status = 'KYC_APPROVED') THEN
        RAISE EXCEPTION 'customer is not approved for financial activity' USING ERRCODE = 'LE009';
    END IF;
    IF EXISTS (SELECT 1 FROM compliance.account_restrictions r WHERE r.user_id = p_user AND r.active) THEN
        RAISE EXCEPTION 'customer account is restricted' USING ERRCODE = 'LE009';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM wallets.wallet_profiles w WHERE w.id = p_wallet AND w.status = 'ACTIVE') THEN
        RAISE EXCEPTION 'customer wallet is not active' USING ERRCODE = 'LE009';
    END IF;
    IF EXISTS (SELECT 1 FROM ledger.asset_holds h WHERE h.asset_id = p_asset AND h.active) THEN
        RAISE EXCEPTION 'this asset is on hold pending reconciliation' USING ERRCODE = 'LE010';
    END IF;
END;
$$;

CREATE FUNCTION ledger.holds_permission(p_user uuid, p_permission text) RETURNS boolean
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
    SELECT EXISTS (
        SELECT 1 FROM audit.admin_assignments s
        JOIN audit.role_permissions rp ON rp.role_id = s.role_id
        JOIN audit.admin_permissions p ON p.id = rp.permission_id
        JOIN identity.users u ON u.id = s.user_id
        WHERE s.user_id = p_user AND s.revoked_at IS NULL AND p.permission_code = p_permission
          AND u.status NOT IN ('SUSPENDED','CLOSED'));
$$;

CREATE FUNCTION ledger.require_permission(p_user uuid, p_permission text) RETURNS void
LANGUAGE plpgsql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
BEGIN
    IF p_user IS NULL OR NOT ledger.holds_permission(p_user, p_permission) THEN
        RAISE EXCEPTION 'permission % is required', p_permission USING ERRCODE = 'LE013';
    END IF;
END;
$$;

CREATE FUNCTION ledger.audit(p_actor uuid, p_action text, p_resource_type text, p_resource uuid,
    p_reason text, p_metadata jsonb) RETURNS void
LANGUAGE sql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
    INSERT INTO audit.audit_events(actor_type, actor_id, action, resource_type, resource_id, reason, metadata)
    VALUES (CASE WHEN p_actor IS NULL THEN 'SYSTEM' ELSE 'ADMIN' END, p_actor, p_action, p_resource_type,
        p_resource, p_reason, coalesce(p_metadata, '{}'::jsonb));
$$;

---------------------------------------------------------------------------
-- Public posting interface (deypay_ledger_poster)
---------------------------------------------------------------------------

-- IMP-P5-008/009: a wallet profile's available and reserved liability
-- accounts. Opening accounts creates no value and posts no journal.
CREATE FUNCTION ledger.open_customer_accounts(p_wallet_profile uuid)
RETURNS TABLE(available_account_id uuid, reserved_account_id uuid)
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_column
DECLARE
    wallet record;
BEGIN
    SELECT w.id, w.user_id, w.asset_id, w.asset_symbol INTO wallet
    FROM wallets.wallet_profiles w WHERE w.id = p_wallet_profile;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'unknown wallet profile' USING ERRCODE = 'LE008';
    END IF;
    INSERT INTO ledger.accounts(chart_code, account_type, normal_balance, negative_allowed, asset_id,
        owner_type, owner_id, wallet_profile_id, customer_role)
    SELECT c.code, c.account_type, c.normal_balance, c.negative_allowed, wallet.asset_id,
        'CUSTOMER', wallet.user_id, wallet.id, c.customer_role
    FROM ledger.chart_accounts c
    WHERE c.customer_role IS NOT NULL AND c.asset_symbols = ARRAY[wallet.asset_symbol]
    ON CONFLICT (wallet_profile_id, customer_role) WHERE owner_type = 'CUSTOMER' DO NOTHING;
    RETURN QUERY
    SELECT av.id, rs.id FROM ledger.accounts av, ledger.accounts rs
    WHERE av.wallet_profile_id = wallet.id AND av.customer_role = 'AVAILABLE' AND av.owner_type = 'CUSTOMER'
      AND rs.wallet_profile_id = wallet.id AND rs.customer_role = 'RESERVED' AND rs.owner_type = 'CUSTOMER';
    IF NOT FOUND THEN
        RAISE EXCEPTION 'no customer chart accounts exist for %', wallet.asset_symbol USING ERRCODE = 'LE008';
    END IF;
END;
$$;

-- Workflow postings such as recognized deposits and settlements. They may
-- credit customers but can never debit customer funds or touch reservations.
CREATE FUNCTION ledger.post_journal(
    p_event_type text, p_economic_key text, p_purpose text, p_source_transaction uuid,
    p_accounts uuid[], p_assets uuid[], p_directions text[], p_amounts text[], p_description text,
    p_actor_type text, p_actor_id uuid, p_correlation uuid)
RETURNS TABLE(journal_id uuid, accounting_event_id uuid, posted_at timestamptz, replayed boolean)
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_column
DECLARE
    amounts numeric[];
    request bytea;
BEGIN
    IF p_amounts IS NULL OR cardinality(p_amounts) = 0 THEN
        RAISE EXCEPTION 'a journal needs 2–64 aligned lines' USING ERRCODE = 'LE006';
    END IF;
    -- These purposes identify reservation, transfer and correction postings.
    IF p_purpose IN ('RESERVE','TRANSFER','REVERSAL','ADJUSTMENT') THEN
        RAISE EXCEPTION 'posting purpose % is reserved', p_purpose USING ERRCODE = 'LE012';
    END IF;
    SELECT array_agg(ledger.parse_amount(t.a) ORDER BY t.n) INTO amounts
    FROM unnest(p_amounts) WITH ORDINALITY AS t(a, n);
    request := ledger.fingerprint('post_journal', p_event_type, coalesce(p_source_transaction::text, ''),
        coalesce(p_description, ''),
        (SELECT coalesce(jsonb_agg(jsonb_build_array(l.account, l.asset, l.dir, l.amount) ORDER BY l.account, l.dir), '[]')::text
         FROM unnest(p_accounts, p_assets, p_directions, p_amounts) AS l(account, asset, dir, amount)));
    RETURN QUERY
    SELECT r.journal_id, r.accounting_event_id, r.posted_at, r.replayed
    FROM ledger.apply_journal('SYSTEM', p_event_type, p_economic_key, p_purpose, request, p_source_transaction,
        NULL, p_accounts, p_assets, p_directions, amounts, p_description, p_actor_type, p_actor_id,
        p_correlation, false) r;
END;
$$;

-- Customer-to-customer transfer of available funds (ledger spec §14).
CREATE FUNCTION ledger.transfer_between_customers(
    p_operation_key text, p_sender uuid, p_recipient uuid, p_asset uuid, p_amount text,
    p_source_transaction uuid, p_actor_type text, p_actor_id uuid, p_correlation uuid)
RETURNS TABLE(journal_id uuid, accounting_event_id uuid, posted_at timestamptz, replayed boolean)
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_column
DECLARE
    amount numeric := ledger.parse_amount(p_amount);
    sender record;
    recipient record;
    request bytea;
    existing uuid;
BEGIN
    IF p_sender IS NULL OR p_recipient IS NULL OR p_asset IS NULL OR p_sender = p_recipient THEN
        RAISE EXCEPTION 'a transfer needs two different customers and an asset' USING ERRCODE = 'LE006';
    END IF;
    request := ledger.fingerprint('transfer', p_sender::text, p_recipient::text, p_asset::text, amount::text,
        coalesce(p_source_transaction::text, ''));
    existing := ledger.claim_event(p_operation_key, 'TRANSFER', request);
    IF existing IS NOT NULL THEN
        RETURN QUERY SELECT j.id, j.accounting_event_id, j.posted_at, true FROM ledger.journals j WHERE j.id = existing;
        RETURN;
    END IF;
    SELECT * INTO sender FROM ledger.customer_accounts(p_sender, p_asset);
    SELECT * INTO recipient FROM ledger.customer_accounts(p_recipient, p_asset);
    PERFORM ledger.require_spend_eligibility(p_sender, sender.wallet_profile_id, p_asset);
    IF NOT EXISTS (SELECT 1 FROM identity.users u JOIN wallets.wallet_profiles w ON w.user_id = u.id
                   WHERE u.id = p_recipient AND w.id = recipient.wallet_profile_id
                     AND u.status NOT IN ('SUSPENDED','CLOSED') AND w.status NOT IN ('SUSPENDED','CLOSED')) THEN
        RAISE EXCEPTION 'recipient cannot receive funds' USING ERRCODE = 'LE009';
    END IF;
    RETURN QUERY
    SELECT r.journal_id, r.accounting_event_id, r.posted_at, r.replayed
    FROM ledger.apply_journal('CUSTOMER_TRANSFER', 'INTERNAL_TRANSFER_EXECUTED', p_operation_key, 'TRANSFER', request,
        p_source_transaction, NULL, ARRAY[sender.available_account_id, recipient.available_account_id],
        ARRAY[p_asset, p_asset], ARRAY['DEBIT','CREDIT'], ARRAY[amount, amount], NULL,
        p_actor_type, p_actor_id, p_correlation, false) r;
END;
$$;

-- IMP-P5-010: moves funds from available to reserved for one operation.
CREATE FUNCTION ledger.reserve_funds(
    p_operation_key text, p_user uuid, p_asset uuid, p_amount text, p_transaction uuid, p_event_type text,
    p_actor_type text, p_actor_id uuid, p_correlation uuid)
RETURNS TABLE(reservation_id uuid, journal_id uuid, posted_at timestamptz, replayed boolean)
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_column
DECLARE
    amount numeric := ledger.parse_amount(p_amount);
    accounts record;
    request bytea;
    existing uuid;
    posted record;
    created uuid;
BEGIN
    IF p_user IS NULL OR p_asset IS NULL THEN
        RAISE EXCEPTION 'a reservation needs a customer and an asset' USING ERRCODE = 'LE006';
    END IF;
    request := ledger.fingerprint('reserve', p_operation_key, p_user::text, p_asset::text, amount::text,
        coalesce(p_transaction::text, ''), coalesce(p_event_type, ''));
    existing := ledger.claim_event(p_operation_key, 'RESERVE', request);
    IF existing IS NOT NULL THEN
        RETURN QUERY SELECT r.id, r.reserve_journal_id, j.posted_at, true
            FROM ledger.reservations r JOIN ledger.journals j ON j.id = r.reserve_journal_id
            WHERE r.reserve_journal_id = existing;
        RETURN;
    END IF;
    SELECT * INTO accounts FROM ledger.customer_accounts(p_user, p_asset);
    PERFORM ledger.require_spend_eligibility(p_user, accounts.wallet_profile_id, p_asset);
    SELECT * INTO posted FROM ledger.apply_journal('RESERVATION', p_event_type, p_operation_key, 'RESERVE', request,
        p_transaction, NULL, ARRAY[accounts.available_account_id, accounts.reserved_account_id],
        ARRAY[p_asset, p_asset], ARRAY['DEBIT','CREDIT'], ARRAY[amount, amount], NULL,
        p_actor_type, p_actor_id, p_correlation, false);
    INSERT INTO ledger.reservations(operation_key, transaction_id, user_id, asset_id, available_account_id,
        reserved_account_id, amount_minor, remaining_minor, reserve_journal_id)
    VALUES (p_operation_key, p_transaction, p_user, p_asset, accounts.available_account_id,
        accounts.reserved_account_id, amount, amount, posted.journal_id)
    RETURNING id INTO created;
    INSERT INTO ledger.reservation_movements(reservation_id, journal_id, movement, amount_minor)
    VALUES (created, posted.journal_id, 'RESERVE', amount);
    RETURN QUERY SELECT created, posted.journal_id, posted.posted_at, false;
END;
$$;

-- Records the external execution status of a reserved operation. Only
-- definitive evidence may later release funds once execution was attempted.
CREATE FUNCTION ledger.record_execution_state(p_reservation uuid, p_state text, p_evidence_reference text)
RETURNS text
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    current_state text;
    current_status text;
BEGIN
    IF p_evidence_reference IS NULL OR length(p_evidence_reference) NOT BETWEEN 1 AND 512
       OR p_evidence_reference ~ '[[:cntrl:]]' THEN
        RAISE EXCEPTION 'execution evidence reference is required' USING ERRCODE = 'LE006';
    END IF;
    SELECT r.execution_state, r.status INTO current_state, current_status
    FROM ledger.reservations r WHERE r.id = p_reservation FOR UPDATE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'unknown reservation' USING ERRCODE = 'LE008';
    END IF;
    IF current_state = p_state THEN
        RETURN current_state;
    END IF;
    IF current_status <> 'ACTIVE' OR NOT (
        (current_state = 'NOT_SUBMITTED' AND p_state IN ('SUBMITTED','UNKNOWN','EXECUTED'))
        OR (current_state = 'SUBMITTED' AND p_state IN ('UNKNOWN','EXECUTED'))
        OR (current_state = 'UNKNOWN' AND p_state IN ('SUBMITTED','EXECUTED'))) THEN
        RAISE EXCEPTION 'execution state cannot change from % to %', current_state, p_state USING ERRCODE = 'LE007';
    END IF;
    UPDATE ledger.reservations r SET execution_state = p_state, updated_at = now() WHERE r.id = p_reservation;
    INSERT INTO ledger.reservation_execution_history(reservation_id, previous_state, new_state, evidence_reference)
    VALUES (p_reservation, current_state, p_state, p_evidence_reference);
    RETURN p_state;
END;
$$;

-- Settles reserved funds against verified external settlement: debits the
-- reserved liability and credits custody, revenue or clearing accounts.
CREATE FUNCTION ledger.consume_reservation(
    p_reservation uuid, p_economic_key text, p_purpose text, p_event_type text,
    p_accounts uuid[], p_amounts text[], p_evidence_reference text,
    p_actor_type text, p_actor_id uuid, p_correlation uuid)
RETURNS TABLE(journal_id uuid, accounting_event_id uuid, posted_at timestamptz, replayed boolean)
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_column
DECLARE
    reservation record;
    amounts numeric[];
    total numeric;
    credits integer := cardinality(p_accounts);
    request bytea;
    existing uuid;
    posted record;
BEGIN
    IF credits IS NULL OR credits < 1 OR credits > 63 OR cardinality(p_amounts) IS DISTINCT FROM credits THEN
        RAISE EXCEPTION 'settlement needs 1–63 aligned credit lines' USING ERRCODE = 'LE006';
    END IF;
    IF p_evidence_reference IS NULL OR length(p_evidence_reference) NOT BETWEEN 1 AND 512
       OR p_evidence_reference ~ '[[:cntrl:]]' THEN
        RAISE EXCEPTION 'settlement evidence reference is required' USING ERRCODE = 'LE006';
    END IF;
    SELECT array_agg(ledger.parse_amount(t.a) ORDER BY t.n), sum(ledger.parse_amount(t.a)) INTO amounts, total
    FROM unnest(p_amounts) WITH ORDINALITY AS t(a, n);
    SELECT * INTO reservation FROM ledger.reservations r WHERE r.id = p_reservation FOR UPDATE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'unknown reservation' USING ERRCODE = 'LE008';
    END IF;
    request := ledger.fingerprint('consume', p_reservation::text, coalesce(p_event_type, ''), p_evidence_reference,
        (SELECT jsonb_agg(jsonb_build_array(l.account, l.amount) ORDER BY l.account)::text
         FROM unnest(p_accounts, p_amounts) AS l(account, amount)));
    existing := ledger.claim_event(p_economic_key, p_purpose, request);
    IF existing IS NOT NULL THEN
        RETURN QUERY SELECT j.id, j.accounting_event_id, j.posted_at, true FROM ledger.journals j WHERE j.id = existing;
        RETURN;
    END IF;
    IF reservation.status <> 'ACTIVE' THEN
        RAISE EXCEPTION 'reservation is %', lower(reservation.status) USING ERRCODE = 'LE007';
    END IF;
    IF total > reservation.remaining_minor THEN
        RAISE EXCEPTION 'settlement exceeds the remaining reservation' USING ERRCODE = 'LE007';
    END IF;
    IF EXISTS (SELECT 1 FROM ledger.accounts a WHERE a.id = ANY (p_accounts) AND a.owner_type = 'CUSTOMER') THEN
        RAISE EXCEPTION 'reserved funds settle to platform accounts only' USING ERRCODE = 'LE012';
    END IF;
    SELECT * INTO posted FROM ledger.apply_journal('RESERVATION', p_event_type, p_economic_key, p_purpose, request,
        reservation.transaction_id, NULL, array_prepend(reservation.reserved_account_id, p_accounts),
        array_fill(reservation.asset_id, ARRAY[credits + 1]),
        array_prepend('DEBIT'::text, array_fill('CREDIT'::text, ARRAY[credits])), array_prepend(total, amounts), NULL,
        p_actor_type, p_actor_id, p_correlation, false);
    UPDATE ledger.reservations r SET
        consumed_minor = r.consumed_minor + total,
        remaining_minor = r.remaining_minor - total,
        status = CASE WHEN r.remaining_minor - total = 0 THEN 'SETTLED' ELSE 'ACTIVE' END,
        execution_state = 'EXECUTED',
        settled_at = now(),
        updated_at = now()
    WHERE r.id = p_reservation;
    IF reservation.execution_state <> 'EXECUTED' THEN
        INSERT INTO ledger.reservation_execution_history(reservation_id, previous_state, new_state, evidence_reference)
        VALUES (p_reservation, reservation.execution_state, 'EXECUTED', p_evidence_reference);
    END IF;
    INSERT INTO ledger.reservation_movements(reservation_id, journal_id, movement, amount_minor, evidence_type, evidence_reference)
    VALUES (p_reservation, posted.journal_id, 'CONSUME', total, 'VERIFIED_SETTLEMENT', p_evidence_reference);
    RETURN QUERY SELECT posted.journal_id, posted.accounting_event_id, posted.posted_at, false;
END;
$$;

-- IMP-P5-011: returns reserved funds to available only on evidence that the
-- external operation did not and will not execute, a cancellation before any
-- submission, or the unneeded remainder after verified settlement. An
-- uncertain outcome (SUBMITTED or UNKNOWN) never qualifies on its own.
CREATE FUNCTION ledger.release_reservation(
    p_reservation uuid, p_economic_key text, p_purpose text, p_amount text,
    p_evidence_type text, p_evidence_reference text, p_actor_type text, p_actor_id uuid, p_correlation uuid)
RETURNS TABLE(journal_id uuid, accounting_event_id uuid, posted_at timestamptz, replayed boolean)
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_column
DECLARE
    amount numeric := ledger.parse_amount(p_amount);
    reservation record;
    request bytea;
    existing uuid;
    posted record;
BEGIN
    IF p_evidence_type IS NULL
       OR p_evidence_type NOT IN ('DEFINITIVE_NON_EXECUTION','PERMITTED_CANCELLATION','SETTLEMENT_REMAINDER') THEN
        RAISE EXCEPTION 'unknown release evidence type' USING ERRCODE = 'LE006';
    END IF;
    IF p_evidence_reference IS NULL OR length(p_evidence_reference) NOT BETWEEN 1 AND 512
       OR p_evidence_reference ~ '[[:cntrl:]]' THEN
        RAISE EXCEPTION 'release evidence reference is required' USING ERRCODE = 'LE006';
    END IF;
    SELECT * INTO reservation FROM ledger.reservations r WHERE r.id = p_reservation FOR UPDATE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'unknown reservation' USING ERRCODE = 'LE008';
    END IF;
    request := ledger.fingerprint('release', p_reservation::text, amount::text, p_evidence_type, p_evidence_reference);
    existing := ledger.claim_event(p_economic_key, p_purpose, request);
    IF existing IS NOT NULL THEN
        RETURN QUERY SELECT j.id, j.accounting_event_id, j.posted_at, true FROM ledger.journals j WHERE j.id = existing;
        RETURN;
    END IF;
    IF reservation.status <> 'ACTIVE' THEN
        RAISE EXCEPTION 'reservation is %', lower(reservation.status) USING ERRCODE = 'LE007';
    END IF;
    IF amount > reservation.remaining_minor THEN
        RAISE EXCEPTION 'release exceeds the remaining reservation' USING ERRCODE = 'LE007';
    END IF;
    IF NOT (
        (p_evidence_type = 'PERMITTED_CANCELLATION' AND reservation.execution_state = 'NOT_SUBMITTED')
        OR (p_evidence_type = 'DEFINITIVE_NON_EXECUTION' AND reservation.execution_state <> 'EXECUTED')
        OR (p_evidence_type = 'SETTLEMENT_REMAINDER' AND reservation.consumed_minor > 0)) THEN
        RAISE EXCEPTION 'release is not permitted while execution is %', lower(reservation.execution_state)
            USING ERRCODE = 'LE007';
    END IF;
    SELECT * INTO posted FROM ledger.apply_journal('RESERVATION', 'RESERVATION_RELEASED', p_economic_key, p_purpose,
        request, reservation.transaction_id, NULL,
        ARRAY[reservation.reserved_account_id, reservation.available_account_id],
        ARRAY[reservation.asset_id, reservation.asset_id], ARRAY['DEBIT','CREDIT'], ARRAY[amount, amount], NULL,
        p_actor_type, p_actor_id, p_correlation, false);
    UPDATE ledger.reservations r SET
        released_minor = r.released_minor + amount,
        remaining_minor = r.remaining_minor - amount,
        status = CASE WHEN r.remaining_minor - amount > 0 THEN 'ACTIVE'
                      WHEN r.consumed_minor > 0 THEN 'SETTLED' ELSE 'RELEASED' END,
        released_at = now(),
        updated_at = now()
    WHERE r.id = p_reservation;
    INSERT INTO ledger.reservation_movements(reservation_id, journal_id, movement, amount_minor, evidence_type, evidence_reference)
    VALUES (p_reservation, posted.journal_id, 'RELEASE', amount, p_evidence_type, p_evidence_reference);
    RETURN QUERY SELECT posted.journal_id, posted.accounting_event_id, posted.posted_at, false;
END;
$$;

---------------------------------------------------------------------------
-- Read interface (poster, controller and reader)
---------------------------------------------------------------------------
CREATE FUNCTION ledger.account_balance(p_account uuid)
RETURNS TABLE(asset_id uuid, normal_balance text, balance_minor text, version bigint, status text)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
    SELECT a.asset_id, b.normal_balance, b.balance_minor::text, b.version, a.status
    FROM ledger.balances b JOIN ledger.accounts a ON a.id = b.account_id WHERE b.account_id = p_account;
$$;

-- Available and reserved are separate liabilities; total = available + reserved.
CREATE FUNCTION ledger.customer_balance(p_user uuid, p_asset uuid)
RETURNS TABLE(available_minor text, reserved_minor text, total_minor text)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
    SELECT av.balance_minor::text, rs.balance_minor::text, (av.balance_minor + rs.balance_minor)::text
    FROM wallets.wallet_profiles w
    JOIN ledger.accounts a1 ON a1.wallet_profile_id = w.id AND a1.owner_type = 'CUSTOMER' AND a1.customer_role = 'AVAILABLE'
    JOIN ledger.balances av ON av.account_id = a1.id
    JOIN ledger.accounts a2 ON a2.wallet_profile_id = w.id AND a2.owner_type = 'CUSTOMER' AND a2.customer_role = 'RESERVED'
    JOIN ledger.balances rs ON rs.account_id = a2.id
    WHERE w.user_id = p_user AND w.asset_id = p_asset;
$$;

CREATE FUNCTION ledger.reservation_state(p_reservation uuid)
RETURNS TABLE(operation_key text, user_id uuid, asset_id uuid, amount_minor text, consumed_minor text,
    released_minor text, remaining_minor text, status text, execution_state text)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
    SELECT r.operation_key, r.user_id, r.asset_id, r.amount_minor::text, r.consumed_minor::text,
        r.released_minor::text, r.remaining_minor::text, r.status, r.execution_state
    FROM ledger.reservations r WHERE r.id = p_reservation;
$$;

-- Active reservations whose outcome needs attention, oldest first.
CREATE FUNCTION ledger.outstanding_reservations(p_older_than interval)
RETURNS TABLE(reservation_id uuid, operation_key text, asset_id uuid, remaining_minor text,
    execution_state text, created_at timestamptz)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
    SELECT r.id, r.operation_key, r.asset_id, r.remaining_minor::text, r.execution_state, r.created_at
    FROM ledger.reservations r
    WHERE r.status = 'ACTIVE' AND r.created_at <= now() - coalesce(p_older_than, '0'::interval)
    ORDER BY r.created_at, r.id;
$$;

-- IMP-P5-012: balances rebuilt from posted entries alone.
CREATE FUNCTION ledger.reconstructed_balances()
RETURNS TABLE(account_id uuid, debit_total numeric, credit_total numeric, balance_minor numeric)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
    SELECT a.id,
        coalesce(sum(e.amount_minor) FILTER (WHERE e.direction = 'DEBIT'), 0),
        coalesce(sum(e.amount_minor) FILTER (WHERE e.direction = 'CREDIT'), 0),
        CASE a.normal_balance
            WHEN 'CREDIT' THEN coalesce(sum(e.amount_minor) FILTER (WHERE e.direction = 'CREDIT'), 0)
                - coalesce(sum(e.amount_minor) FILTER (WHERE e.direction = 'DEBIT'), 0)
            ELSE coalesce(sum(e.amount_minor) FILTER (WHERE e.direction = 'DEBIT'), 0)
                - coalesce(sum(e.amount_minor) FILTER (WHERE e.direction = 'CREDIT'), 0) END
    FROM ledger.accounts a LEFT JOIN ledger.entries e ON e.account_id = a.id
    GROUP BY a.id, a.normal_balance;
$$;

-- Every way the books could disagree with themselves. An empty result is the
-- evidence that projections, journals and reservations are consistent.
CREATE FUNCTION ledger.integrity_violations()
RETURNS TABLE(kind text, account_id uuid, journal_id uuid, reservation_id uuid, detail jsonb)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
    SELECT 'PROJECTION_DRIFT', b.account_id, NULL::uuid, NULL::uuid,
        jsonb_build_object('projected_minor', b.balance_minor::text, 'reconstructed_minor', r.balance_minor::text)
    FROM ledger.balances b JOIN ledger.reconstructed_balances() r ON r.account_id = b.account_id
    WHERE b.balance_minor <> r.balance_minor OR b.debit_total <> r.debit_total OR b.credit_total <> r.credit_total
    UNION ALL
    SELECT 'UNBALANCED_JOURNAL', NULL, e.journal_id, NULL, jsonb_build_object('asset_id', e.asset_id)
    FROM ledger.entries e GROUP BY e.journal_id, e.asset_id
    HAVING sum(CASE e.direction WHEN 'DEBIT' THEN e.amount_minor ELSE -e.amount_minor END) <> 0
    UNION ALL
    SELECT 'ENTRY_COUNT', NULL, j.id, NULL, jsonb_build_object('declared', j.entry_count, 'actual', count(e.id))
    FROM ledger.journals j LEFT JOIN ledger.entries e ON e.journal_id = j.id
    GROUP BY j.id, j.entry_count HAVING count(e.id) <> j.entry_count
    UNION ALL
    SELECT 'RESERVED_BALANCE_MISMATCH', a.id, NULL, NULL,
        jsonb_build_object('reserved_balance_minor', b.balance_minor::text,
            'active_reservations_minor', coalesce(sum(r.remaining_minor), 0)::text)
    FROM ledger.accounts a JOIN ledger.balances b ON b.account_id = a.id
    LEFT JOIN ledger.reservations r ON r.reserved_account_id = a.id AND r.status = 'ACTIVE'
    WHERE a.owner_type = 'CUSTOMER' AND a.customer_role = 'RESERVED'
    GROUP BY a.id, b.balance_minor HAVING b.balance_minor <> coalesce(sum(r.remaining_minor), 0)
    UNION ALL
    SELECT 'RESERVATION_MOVEMENTS', NULL, NULL, r.id,
        jsonb_build_object('consumed_minor', r.consumed_minor::text, 'released_minor', r.released_minor::text)
    FROM ledger.reservations r
    LEFT JOIN ledger.reservation_movements m ON m.reservation_id = r.id
    GROUP BY r.id, r.amount_minor, r.consumed_minor, r.released_minor
    HAVING coalesce(sum(m.amount_minor) FILTER (WHERE m.movement = 'RESERVE'), 0) <> r.amount_minor
        OR coalesce(sum(m.amount_minor) FILTER (WHERE m.movement = 'CONSUME'), 0) <> r.consumed_minor
        OR coalesce(sum(m.amount_minor) FILTER (WHERE m.movement = 'RELEASE'), 0) <> r.released_minor
    UNION ALL
    SELECT 'NEGATIVE_BALANCE', b.account_id, NULL, NULL, jsonb_build_object('balance_minor', b.balance_minor::text)
    FROM ledger.balances b WHERE NOT b.negative_allowed AND b.balance_minor < 0;
$$;

---------------------------------------------------------------------------
-- Controller interface (deypay_ledger_controller)
---------------------------------------------------------------------------
CREATE FUNCTION ledger.open_platform_account(p_chart_code text, p_asset uuid, p_instance_key text, p_actor uuid)
RETURNS uuid
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    chart ledger.chart_accounts%ROWTYPE;
    opened uuid;
BEGIN
    PERFORM ledger.require_permission(p_actor, 'LEDGER_ACCOUNT_CONTROL');
    SELECT * INTO chart FROM ledger.chart_accounts c WHERE c.code = p_chart_code;
    IF NOT FOUND OR chart.customer_role IS NOT NULL THEN
        RAISE EXCEPTION 'unknown platform chart account %', p_chart_code USING ERRCODE = 'LE008';
    END IF;
    INSERT INTO ledger.accounts(chart_code, account_type, normal_balance, negative_allowed, asset_id,
        owner_type, instance_key)
    VALUES (chart.code, chart.account_type, chart.normal_balance, chart.negative_allowed, p_asset,
        'PLATFORM', coalesce(p_instance_key, 'primary'))
    ON CONFLICT (chart_code, asset_id, instance_key) WHERE owner_type = 'PLATFORM' DO NOTHING
    RETURNING id INTO opened;
    IF opened IS NULL THEN
        SELECT a.id INTO opened FROM ledger.accounts a
        WHERE a.owner_type = 'PLATFORM' AND a.chart_code = chart.code AND a.asset_id = p_asset
          AND a.instance_key = coalesce(p_instance_key, 'primary');
    ELSE
        PERFORM ledger.audit(p_actor, 'LEDGER_ACCOUNT_OPENED', 'ledger.account', opened, NULL,
            jsonb_build_object('chart_code', chart.code, 'asset_id', p_asset));
    END IF;
    RETURN opened;
END;
$$;

CREATE FUNCTION ledger.set_account_status(p_account uuid, p_status text, p_reason text, p_actor uuid)
RETURNS text
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp SET lock_timeout = '10s' AS $$
DECLARE
    current_status text;
    current_balance numeric;
BEGIN
    PERFORM ledger.require_permission(p_actor, 'LEDGER_ACCOUNT_CONTROL');
    IF p_status NOT IN ('OPEN','FROZEN','CLOSED') OR p_reason IS NULL OR btrim(p_reason) = '' THEN
        RAISE EXCEPTION 'a valid status and reason are required' USING ERRCODE = 'LE006';
    END IF;
    SELECT b.balance_minor INTO current_balance FROM ledger.balances b WHERE b.account_id = p_account FOR UPDATE;
    SELECT a.status INTO current_status FROM ledger.accounts a WHERE a.id = p_account;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'unknown ledger account' USING ERRCODE = 'LE008';
    END IF;
    IF current_status = p_status THEN
        RETURN p_status;
    END IF;
    IF current_status = 'CLOSED' THEN
        RAISE EXCEPTION 'a closed ledger account cannot reopen' USING ERRCODE = 'LE003';
    END IF;
    IF p_status = 'CLOSED' AND (current_balance <> 0 OR EXISTS (
        SELECT 1 FROM ledger.reservations r
        WHERE (r.available_account_id = p_account OR r.reserved_account_id = p_account) AND r.status = 'ACTIVE')) THEN
        RAISE EXCEPTION 'only an empty account without active reservations can close' USING ERRCODE = 'LE003';
    END IF;
    UPDATE ledger.accounts a SET status = p_status, status_changed_at = now() WHERE a.id = p_account;
    PERFORM ledger.audit(p_actor, 'LEDGER_ACCOUNT_STATUS_CHANGED', 'ledger.account', p_account, p_reason,
        jsonb_build_object('previous_status', current_status, 'new_status', p_status));
    RETURN p_status;
END;
$$;

-- Maker: proposes a reversal of a posted journal or a balanced adjustment.
CREATE FUNCTION ledger.request_correction(p_type text, p_target_journal uuid, p_lines jsonb, p_reason text,
    p_evidence_reference text, p_requested_by uuid)
RETURNS uuid
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    target record;
    created uuid;
BEGIN
    PERFORM ledger.require_permission(p_requested_by, 'LEDGER_CORRECTION_REQUEST');
    IF p_type = 'REVERSAL' THEN
        IF p_lines IS NOT NULL THEN
            RAISE EXCEPTION 'a reversal mirrors the original journal and takes no lines' USING ERRCODE = 'LE006';
        END IF;
        SELECT j.id, j.reversal_of_journal_id INTO target FROM ledger.journals j WHERE j.id = p_target_journal;
        IF NOT FOUND THEN
            RAISE EXCEPTION 'unknown journal' USING ERRCODE = 'LE008';
        END IF;
        IF target.reversal_of_journal_id IS NOT NULL THEN
            RAISE EXCEPTION 'a reversal cannot itself be reversed; post an adjustment' USING ERRCODE = 'LE005';
        END IF;
        IF EXISTS (SELECT 1 FROM ledger.journals j WHERE j.reversal_of_journal_id = p_target_journal) THEN
            RAISE EXCEPTION 'journal is already reversed' USING ERRCODE = 'LE005';
        END IF;
        IF EXISTS (SELECT 1 FROM ledger.entries e JOIN ledger.accounts a ON a.id = e.account_id
                   WHERE e.journal_id = p_target_journal AND a.owner_type = 'CUSTOMER' AND a.customer_role = 'RESERVED') THEN
            RAISE EXCEPTION 'reservation journals are corrected by releasing or settling the reservation'
                USING ERRCODE = 'LE005';
        END IF;
    ELSIF p_type = 'ADJUSTMENT' THEN
        IF p_target_journal IS NOT NULL OR p_lines IS NULL OR jsonb_typeof(p_lines) <> 'array'
           OR jsonb_array_length(p_lines) NOT BETWEEN 2 AND 64 THEN
            RAISE EXCEPTION 'an adjustment needs 2–64 lines' USING ERRCODE = 'LE006';
        END IF;
        PERFORM ledger.parse_amount(l->>'amount_minor'), (l->>'account_id')::uuid, (l->>'asset_id')::uuid
        FROM jsonb_array_elements(p_lines) AS l;
        IF EXISTS (SELECT 1 FROM jsonb_array_elements(p_lines) AS l
                   WHERE l->>'account_id' IS NULL OR l->>'asset_id' IS NULL) THEN
            RAISE EXCEPTION 'adjustment lines need an account and asset' USING ERRCODE = 'LE006';
        END IF;
        IF EXISTS (SELECT 1 FROM jsonb_array_elements(p_lines) AS l
                   WHERE (l->>'direction') IS DISTINCT FROM 'DEBIT' AND (l->>'direction') IS DISTINCT FROM 'CREDIT') THEN
            RAISE EXCEPTION 'adjustment lines need a DEBIT or CREDIT direction' USING ERRCODE = 'LE006';
        END IF;
        IF EXISTS (SELECT 1 FROM jsonb_array_elements(p_lines) AS l
                   GROUP BY l->>'asset_id'
                   HAVING sum(CASE l->>'direction' WHEN 'DEBIT' THEN (l->>'amount_minor')::numeric
                                                   ELSE -(l->>'amount_minor')::numeric END) <> 0) THEN
            RAISE EXCEPTION 'adjustment does not balance for each asset' USING ERRCODE = 'LE001';
        END IF;
    ELSE
        RAISE EXCEPTION 'unknown correction type' USING ERRCODE = 'LE006';
    END IF;
    INSERT INTO ledger.correction_requests(correction_type, target_journal_id, lines, reason, evidence_reference, requested_by)
    VALUES (p_type, p_target_journal, p_lines, p_reason, p_evidence_reference, p_requested_by)
    RETURNING id INTO created;
    PERFORM ledger.audit(p_requested_by, 'LEDGER_CORRECTION_REQUESTED', 'ledger.correction_request', created, p_reason,
        jsonb_build_object('correction_type', p_type, 'target_journal_id', p_target_journal,
            'evidence_reference', p_evidence_reference));
    RETURN created;
END;
$$;

-- Checker: a different, authorized person approves or rejects.
CREATE FUNCTION ledger.decide_correction(p_request uuid, p_decided_by uuid, p_approve boolean, p_reason text)
RETURNS text
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    request record;
    outcome text := CASE WHEN p_approve THEN 'APPROVED' ELSE 'REJECTED' END;
BEGIN
    SELECT * INTO request FROM ledger.correction_requests c WHERE c.id = p_request FOR UPDATE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'unknown correction request' USING ERRCODE = 'LE008';
    END IF;
    IF request.status <> 'PENDING' THEN
        RAISE EXCEPTION 'correction request is already %', lower(request.status) USING ERRCODE = 'LE013';
    END IF;
    IF p_decided_by IS NULL OR p_decided_by = request.requested_by THEN
        RAISE EXCEPTION 'the requester cannot decide their own correction' USING ERRCODE = 'LE013';
    END IF;
    PERFORM ledger.require_permission(p_decided_by, 'LEDGER_CORRECTION_APPROVE');
    IF p_approve IS NULL OR p_reason IS NULL OR btrim(p_reason) = '' THEN
        RAISE EXCEPTION 'a decision and reason are required' USING ERRCODE = 'LE006';
    END IF;
    UPDATE ledger.correction_requests c SET status = outcome, decided_by = p_decided_by,
        decision_reason = p_reason, decided_at = now()
    WHERE c.id = p_request;
    PERFORM ledger.audit(p_decided_by, 'LEDGER_CORRECTION_' || outcome, 'ledger.correction_request', p_request,
        p_reason, jsonb_build_object('requested_by', request.requested_by));
    RETURN outcome;
END;
$$;

-- Posts an approved correction exactly once. A reversal that would overdraw
-- an account opens a recovery case instead of debiting unavailable funds.
CREATE FUNCTION ledger.execute_correction(p_request uuid, p_executed_by uuid)
RETURNS TABLE(status text, journal_id uuid, recovery_case_id uuid)
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_column
DECLARE
    request record;
    original record;
    accounts uuid[];
    assets uuid[];
    directions text[];
    amounts numeric[];
    posted record;
    opened uuid;
BEGIN
    SELECT * INTO request FROM ledger.correction_requests c WHERE c.id = p_request FOR UPDATE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'unknown correction request' USING ERRCODE = 'LE008';
    END IF;
    IF request.status IN ('EXECUTED','RECOVERY_REQUIRED') THEN
        RETURN QUERY SELECT request.status, request.result_journal_id, request.recovery_case_id;
        RETURN;
    END IF;
    IF request.status <> 'APPROVED' THEN
        RAISE EXCEPTION 'correction request is %', lower(request.status) USING ERRCODE = 'LE013';
    END IF;
    IF NOT (ledger.holds_permission(p_executed_by, 'LEDGER_CORRECTION_APPROVE')
            OR ledger.holds_permission(p_executed_by, 'LEDGER_CORRECTION_REQUEST')) THEN
        RAISE EXCEPTION 'permission to execute corrections is required' USING ERRCODE = 'LE013';
    END IF;
    IF request.correction_type = 'REVERSAL' THEN
        SELECT j.id, j.source_transaction_id INTO original FROM ledger.journals j WHERE j.id = request.target_journal_id;
        IF EXISTS (SELECT 1 FROM ledger.journals j WHERE j.reversal_of_journal_id = original.id) THEN
            RAISE EXCEPTION 'journal is already reversed' USING ERRCODE = 'LE005';
        END IF;
        SELECT array_agg(e.account_id ORDER BY e.line_number), array_agg(e.asset_id ORDER BY e.line_number),
               array_agg(CASE e.direction WHEN 'DEBIT' THEN 'CREDIT' ELSE 'DEBIT' END ORDER BY e.line_number),
               array_agg(e.amount_minor::numeric ORDER BY e.line_number)
        INTO accounts, assets, directions, amounts
        FROM ledger.entries e WHERE e.journal_id = original.id;
        SELECT * INTO posted FROM ledger.apply_journal('CORRECTION', 'JOURNAL_REVERSED', 'correction:' || p_request,
            'REVERSAL', ledger.fingerprint('reversal', p_request::text, original.id::text), original.source_transaction_id,
            original.id, accounts, assets, directions, amounts,
            left(regexp_replace(request.reason, '[[:cntrl:]]', ' ', 'g'), 500), 'ADMIN', p_executed_by, NULL, true);
        IF posted.shortfall IS NOT NULL THEN
            INSERT INTO ledger.recovery_cases(original_journal_id, reason, shortfall)
            VALUES (original.id, request.reason, posted.shortfall)
            RETURNING id INTO opened;
            UPDATE ledger.correction_requests c SET status = 'RECOVERY_REQUIRED', recovery_case_id = opened,
                executed_by = p_executed_by, executed_at = now()
            WHERE c.id = p_request;
            PERFORM ledger.audit(p_executed_by, 'LEDGER_RECOVERY_CASE_OPENED', 'ledger.recovery_case', opened,
                request.reason, jsonb_build_object('correction_request_id', p_request, 'shortfall', posted.shortfall));
            RETURN QUERY SELECT 'RECOVERY_REQUIRED'::text, NULL::uuid, opened;
            RETURN;
        END IF;
    ELSE
        SELECT array_agg((l->>'account_id')::uuid ORDER BY n), array_agg((l->>'asset_id')::uuid ORDER BY n),
               array_agg(l->>'direction' ORDER BY n), array_agg(ledger.parse_amount(l->>'amount_minor') ORDER BY n)
        INTO accounts, assets, directions, amounts
        FROM jsonb_array_elements(request.lines) WITH ORDINALITY AS t(l, n);
        SELECT * INTO posted FROM ledger.apply_journal('CORRECTION', 'LEDGER_ADJUSTMENT', 'correction:' || p_request,
            'ADJUSTMENT', ledger.fingerprint('adjustment', p_request::text, request.lines::text), NULL, NULL,
            accounts, assets, directions, amounts, left(regexp_replace(request.reason, '[[:cntrl:]]', ' ', 'g'), 500),
            'ADMIN', p_executed_by, NULL, false);
    END IF;
    UPDATE ledger.correction_requests c SET status = 'EXECUTED', result_journal_id = posted.journal_id,
        executed_by = p_executed_by, executed_at = now()
    WHERE c.id = p_request;
    PERFORM ledger.audit(p_executed_by, 'LEDGER_CORRECTION_EXECUTED', 'ledger.correction_request', p_request,
        request.reason, jsonb_build_object('journal_id', posted.journal_id));
    RETURN QUERY SELECT 'EXECUTED'::text, posted.journal_id, NULL::uuid;
END;
$$;

CREATE FUNCTION ledger.resolve_recovery_case(p_case uuid, p_resolution_journal uuid, p_notes text, p_actor uuid)
RETURNS text
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    current_status text;
BEGIN
    PERFORM ledger.require_permission(p_actor, 'LEDGER_CORRECTION_APPROVE');
    SELECT c.status INTO current_status FROM ledger.recovery_cases c WHERE c.id = p_case FOR UPDATE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'unknown recovery case' USING ERRCODE = 'LE008';
    END IF;
    IF current_status <> 'OPEN' THEN
        RAISE EXCEPTION 'recovery case is already resolved' USING ERRCODE = 'LE007';
    END IF;
    IF p_notes IS NULL OR btrim(p_notes) = '' THEN
        RAISE EXCEPTION 'resolution notes are required' USING ERRCODE = 'LE006';
    END IF;
    IF p_resolution_journal IS NOT NULL AND NOT EXISTS (SELECT 1 FROM ledger.journals j WHERE j.id = p_resolution_journal) THEN
        RAISE EXCEPTION 'unknown resolution journal' USING ERRCODE = 'LE008';
    END IF;
    UPDATE ledger.recovery_cases c SET status = 'RESOLVED', resolution_journal_id = p_resolution_journal,
        resolution_notes = p_notes, resolved_by = p_actor, resolved_at = now()
    WHERE c.id = p_case;
    PERFORM ledger.audit(p_actor, 'LEDGER_RECOVERY_CASE_RESOLVED', 'ledger.recovery_case', p_case, p_notes,
        jsonb_build_object('resolution_journal_id', p_resolution_journal));
    RETURN 'RESOLVED';
END;
$$;

CREATE FUNCTION ledger.lift_asset_hold(p_hold uuid, p_reason text, p_actor uuid) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    hold record;
BEGIN
    PERFORM ledger.require_permission(p_actor, 'LEDGER_ACCOUNT_CONTROL');
    SELECT * INTO hold FROM ledger.asset_holds h WHERE h.id = p_hold FOR UPDATE;
    IF NOT FOUND OR NOT hold.active THEN
        RAISE EXCEPTION 'no active hold' USING ERRCODE = 'LE008';
    END IF;
    IF p_reason IS NULL OR btrim(p_reason) = '' THEN
        RAISE EXCEPTION 'a reason is required' USING ERRCODE = 'LE006';
    END IF;
    IF hold.reconciliation_case_id IS NOT NULL AND EXISTS (
        SELECT 1 FROM operations.reconciliation_cases c WHERE c.id = hold.reconciliation_case_id AND c.status <> 'RESOLVED') THEN
        RAISE EXCEPTION 'resolve the reconciliation case before lifting its hold' USING ERRCODE = 'LE007';
    END IF;
    UPDATE ledger.asset_holds h SET active = false, lifted_at = now(), lifted_by = p_actor, lift_reason = p_reason
    WHERE h.id = p_hold;
    PERFORM ledger.audit(p_actor, 'LEDGER_ASSET_HOLD_LIFTED', 'ledger.asset_hold', p_hold, p_reason,
        jsonb_build_object('asset_id', hold.asset_id));
END;
$$;

---------------------------------------------------------------------------
-- Reconciliation interface (deypay_ledger_controller)
---------------------------------------------------------------------------
CREATE FUNCTION operations.start_reconciliation(p_type text, p_provider uuid, p_period_start timestamptz,
    p_period_end timestamptz, p_actor uuid)
RETURNS uuid
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    started uuid;
BEGIN
    PERFORM ledger.require_permission(p_actor, 'RECONCILIATION_OPERATE');
    INSERT INTO operations.reconciliation_runs(reconciliation_type, provider_id, period_start, period_end, started_by)
    VALUES (p_type, p_provider, p_period_start, p_period_end, p_actor)
    RETURNING id INTO started;
    RETURN started;
END;
$$;

CREATE FUNCTION operations.running(p_run uuid) RETURNS operations.reconciliation_runs
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    run operations.reconciliation_runs%ROWTYPE;
BEGIN
    SELECT * INTO run FROM operations.reconciliation_runs r WHERE r.id = p_run FOR UPDATE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'unknown reconciliation run' USING ERRCODE = 'LE008';
    END IF;
    IF run.status <> 'RUNNING' THEN
        RAISE EXCEPTION 'reconciliation run is complete' USING ERRCODE = 'LE007';
    END IF;
    RETURN run;
END;
$$;

-- Records one reconciliation result and, for any mismatch, an open case.
CREATE FUNCTION operations.record_item(p_run uuid, p_status text, p_discrepancy text, p_severity text,
    p_account uuid, p_journal uuid, p_reference text, p_evidence bytea, p_observed_at timestamptz,
    p_asset uuid, p_observed numeric, p_expected numeric, p_details jsonb)
RETURNS uuid
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    item uuid;
    opened uuid;
BEGIN
    INSERT INTO operations.reconciliation_items(reconciliation_run_id, journal_id, ledger_account_id,
        external_evidence_reference, evidence_sha256, evidence_observed_at, observed_asset_id,
        observed_amount_minor, expected_amount_minor, match_status, discrepancy_type, details)
    VALUES (p_run, p_journal, p_account, p_reference, p_evidence, p_observed_at, p_asset, p_observed, p_expected,
        p_status, CASE WHEN p_status = 'MATCHED' THEN NULL ELSE coalesce(p_discrepancy, p_status) END,
        coalesce(p_details, '{}'::jsonb))
    RETURNING id INTO item;
    IF p_status <> 'MATCHED' THEN
        INSERT INTO operations.reconciliation_cases(reconciliation_item_id, severity, impact_asset_id, financial_impact_minor)
        VALUES (item, p_severity, p_asset,
            CASE WHEN p_observed IS NOT NULL AND p_expected IS NOT NULL THEN p_observed - p_expected END)
        RETURNING id INTO opened;
        INSERT INTO operations.reconciliation_case_events(case_id, previous_status, new_status, note)
        VALUES (opened, NULL, 'OPEN', 'Opened by reconciliation: ' || coalesce(p_discrepancy, p_status));
    END IF;
    RETURN item;
END;
$$;

-- Internal accounting reconciliation: projections, journals and reservations.
CREATE FUNCTION operations.reconcile_ledger_integrity(p_run uuid) RETURNS integer
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    run operations.reconciliation_runs%ROWTYPE := operations.running(p_run);
    violation record;
    found_count integer := 0;
BEGIN
    FOR violation IN SELECT * FROM ledger.integrity_violations() LOOP
        found_count := found_count + 1;
        PERFORM operations.record_item(run.id, 'INTEGRITY_VIOLATION', violation.kind, 'CRITICAL',
            violation.account_id, violation.journal_id, NULL, NULL, now(), NULL, NULL, NULL,
            violation.detail || jsonb_build_object('reservation_id', violation.reservation_id));
    END LOOP;
    IF found_count = 0 THEN
        PERFORM operations.record_item(run.id, 'MATCHED', NULL, NULL, NULL, NULL, NULL, NULL, now(), NULL, NULL, NULL,
            jsonb_build_object('check', 'LEDGER_INTEGRITY', 'journals', (SELECT count(*) FROM ledger.journals)));
    END IF;
    RETURN found_count;
END;
$$;

-- Customer liabilities against custody and permitted settlement positions for
-- one asset (ledger spec §30.3). A shortfall is critical and places the asset
-- on hold, so customer spending stops until the case is resolved.
CREATE FUNCTION operations.reconcile_customer_liabilities(p_run uuid, p_asset uuid) RETURNS text
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    run operations.reconciliation_runs%ROWTYPE := operations.running(p_run);
    liabilities numeric;
    backing numeric;
    outcome text;
    item uuid;
    opened uuid;
BEGIN
    SELECT coalesce(sum(b.balance_minor) FILTER (WHERE c.reconciliation_role = 'CUSTOMER_LIABILITY'), 0),
           coalesce(sum(b.balance_minor) FILTER (WHERE c.reconciliation_role = 'CUSTOMER_BACKING'), 0)
    INTO liabilities, backing
    FROM ledger.accounts a JOIN ledger.chart_accounts c ON c.code = a.chart_code
    JOIN ledger.balances b ON b.account_id = a.id
    WHERE a.asset_id = p_asset;
    outcome := CASE WHEN backing = liabilities THEN 'MATCHED' WHEN backing < liabilities THEN 'SHORTFALL' ELSE 'SURPLUS' END;
    item := operations.record_item(run.id, outcome, outcome, CASE outcome WHEN 'SHORTFALL' THEN 'CRITICAL' ELSE 'MEDIUM' END,
        NULL, NULL, NULL, NULL, now(), p_asset, backing, liabilities,
        jsonb_build_object('check', 'CUSTOMER_LIABILITY', 'customer_liabilities_minor', liabilities::text,
            'backing_minor', backing::text));
    IF outcome = 'SHORTFALL' THEN
        SELECT c.id INTO opened FROM operations.reconciliation_cases c WHERE c.reconciliation_item_id = item;
        INSERT INTO ledger.asset_holds(asset_id, reason, reconciliation_case_id)
        VALUES (p_asset, 'Customer liabilities exceed custody backing', opened);
    END IF;
    RETURN outcome;
END;
$$;

-- External balance evidence (statement, custody report) against one ledger account.
CREATE FUNCTION operations.record_balance_evidence(p_run uuid, p_account uuid, p_observed_minor text,
    p_evidence_reference text, p_evidence_sha256 bytea, p_observed_at timestamptz)
RETURNS text
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    run operations.reconciliation_runs%ROWTYPE := operations.running(p_run);
    observed numeric;
    account record;
    outcome text;
BEGIN
    IF p_observed_minor IS NULL OR p_observed_minor !~ '^(0|[1-9][0-9]{0,37})$' THEN
        RAISE EXCEPTION 'observed balance must be a whole number of accounting units' USING ERRCODE = 'LE006';
    END IF;
    IF p_evidence_reference IS NULL OR p_evidence_sha256 IS NULL OR p_observed_at IS NULL THEN
        RAISE EXCEPTION 'balance evidence needs a reference, digest and observation time' USING ERRCODE = 'LE006';
    END IF;
    observed := p_observed_minor::numeric;
    SELECT a.asset_id, b.balance_minor INTO account
    FROM ledger.accounts a JOIN ledger.balances b ON b.account_id = a.id WHERE a.id = p_account;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'unknown ledger account' USING ERRCODE = 'LE008';
    END IF;
    outcome := CASE WHEN observed = account.balance_minor THEN 'MATCHED' ELSE 'AMOUNT_MISMATCH' END;
    PERFORM operations.record_item(run.id, outcome, 'BALANCE_MISMATCH', 'HIGH', p_account, NULL, p_evidence_reference,
        p_evidence_sha256, p_observed_at, account.asset_id, observed, account.balance_minor,
        jsonb_build_object('check', 'EXTERNAL_BALANCE'));
    RETURN outcome;
END;
$$;

-- One external transaction line (provider statement or chain transfer) against
-- the journals posted for the same economic event on one ledger account.
CREATE FUNCTION operations.record_transaction_evidence(p_run uuid, p_external_reference text, p_account uuid,
    p_asset uuid, p_direction text, p_amount_minor text, p_evidence_reference text, p_evidence_sha256 bytea,
    p_observed_at timestamptz)
RETURNS text
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    run operations.reconciliation_runs%ROWTYPE := operations.running(p_run);
    observed numeric := ledger.parse_amount(p_amount_minor);
    account_asset uuid;
    booked numeric;
    booked_journal uuid;
    outcome text;
BEGIN
    PERFORM ledger.require_key(p_external_reference, 'external reference');
    IF p_direction NOT IN ('DEBIT','CREDIT') OR p_evidence_reference IS NULL OR p_evidence_sha256 IS NULL
       OR p_observed_at IS NULL THEN
        RAISE EXCEPTION 'transaction evidence needs a direction, reference, digest and observation time'
            USING ERRCODE = 'LE006';
    END IF;
    SELECT a.asset_id INTO account_asset FROM ledger.accounts a WHERE a.id = p_account;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'unknown ledger account' USING ERRCODE = 'LE008';
    END IF;
    IF EXISTS (SELECT 1 FROM operations.reconciliation_items i
               WHERE i.reconciliation_run_id = run.id AND i.external_evidence_reference = p_external_reference) THEN
        outcome := 'DUPLICATE';
    ELSIF account_asset <> p_asset THEN
        outcome := 'ASSET_MISMATCH';
    ELSE
        SELECT sum(e.amount_minor), min(e.journal_id::text)::uuid INTO booked, booked_journal
        FROM ledger.accounting_events ev JOIN ledger.journals j ON j.accounting_event_id = ev.id
        JOIN ledger.entries e ON e.journal_id = j.id
        WHERE ev.economic_event_key = p_external_reference AND e.account_id = p_account AND e.direction = p_direction;
        outcome := CASE WHEN booked IS NULL THEN 'MISSING_INTERNAL' WHEN booked = observed THEN 'MATCHED'
                        ELSE 'AMOUNT_MISMATCH' END;
    END IF;
    PERFORM operations.record_item(run.id, outcome, outcome,
        CASE outcome WHEN 'DUPLICATE' THEN 'CRITICAL' ELSE 'HIGH' END, p_account, booked_journal,
        p_external_reference, p_evidence_sha256, p_observed_at, p_asset, observed, booked,
        jsonb_build_object('check', 'EXTERNAL_TRANSACTION', 'evidence_reference', p_evidence_reference,
            'direction', p_direction));
    RETURN outcome;
END;
$$;

-- Closes a run. Journals of the expected event types on the account within the
-- period that no external evidence matched are recorded as MISSING_EXTERNAL.
CREATE FUNCTION operations.complete_reconciliation(p_run uuid, p_expected_event_types text[], p_account uuid)
RETURNS text
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    run operations.reconciliation_runs%ROWTYPE := operations.running(p_run);
    unmatched record;
    final_status text;
BEGIN
    IF p_account IS NOT NULL AND p_expected_event_types IS NOT NULL THEN
        FOR unmatched IN
            SELECT j.id AS journal, ev.economic_event_key AS reference, e.asset_id AS asset, e.amount_minor AS amount
            FROM ledger.journals j JOIN ledger.accounting_events ev ON ev.id = j.accounting_event_id
            JOIN ledger.entries e ON e.journal_id = j.id AND e.account_id = p_account
            WHERE j.event_type = ANY (p_expected_event_types)
              AND j.posted_at >= run.period_start AND j.posted_at < run.period_end
              AND NOT EXISTS (SELECT 1 FROM operations.reconciliation_items i
                              WHERE i.reconciliation_run_id = run.id AND i.external_evidence_reference = ev.economic_event_key)
        LOOP
            PERFORM operations.record_item(run.id, 'MISSING_EXTERNAL', 'MISSING_EXTERNAL', 'HIGH', p_account,
                unmatched.journal, NULL, NULL, NULL, unmatched.asset, NULL, unmatched.amount,
                jsonb_build_object('check', 'EXTERNAL_TRANSACTION', 'economic_event_key', unmatched.reference));
        END LOOP;
    END IF;
    final_status := CASE WHEN EXISTS (SELECT 1 FROM operations.reconciliation_items i
                                      WHERE i.reconciliation_run_id = run.id AND i.match_status <> 'MATCHED')
                         THEN 'COMPLETED_WITH_DISCREPANCIES' ELSE 'COMPLETED' END;
    UPDATE operations.reconciliation_runs r SET status = final_status, completed_at = now() WHERE r.id = run.id;
    RETURN final_status;
END;
$$;

-- Investigation history. Cases cannot be closed without notes and evidence.
CREATE FUNCTION operations.update_case(p_case uuid, p_status text, p_assigned_to uuid, p_note text,
    p_resolution_evidence text, p_actor uuid)
RETURNS text
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    current_status text;
BEGIN
    PERFORM ledger.require_permission(p_actor, 'RECONCILIATION_OPERATE');
    IF p_status NOT IN ('OPEN','INVESTIGATING','RESOLVED') OR p_note IS NULL OR btrim(p_note) = '' THEN
        RAISE EXCEPTION 'a valid status and note are required' USING ERRCODE = 'LE006';
    END IF;
    IF p_status = 'RESOLVED' AND (p_resolution_evidence IS NULL OR btrim(p_resolution_evidence) = '') THEN
        RAISE EXCEPTION 'resolving a case requires resolution evidence' USING ERRCODE = 'LE006';
    END IF;
    SELECT c.status INTO current_status FROM operations.reconciliation_cases c WHERE c.id = p_case FOR UPDATE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'unknown reconciliation case' USING ERRCODE = 'LE008';
    END IF;
    UPDATE operations.reconciliation_cases c SET
        status = p_status,
        assigned_to = coalesce(p_assigned_to, c.assigned_to),
        resolution_notes = CASE WHEN p_status = 'RESOLVED' THEN p_note ELSE c.resolution_notes END,
        resolution_evidence = CASE WHEN p_status = 'RESOLVED' THEN p_resolution_evidence ELSE c.resolution_evidence END,
        resolved_at = CASE WHEN p_status = 'RESOLVED' THEN now() END,
        updated_at = now()
    WHERE c.id = p_case;
    INSERT INTO operations.reconciliation_case_events(case_id, actor_id, previous_status, new_status, note)
    VALUES (p_case, p_actor, current_status, p_status, p_note);
    RETURN p_status;
END;
$$;

-- PostgreSQL grants EXECUTE to PUBLIC by default; ledger functions are granted
-- explicitly by infrastructure/postgres/ledger-grants.psql.
REVOKE ALL ON ALL FUNCTIONS IN SCHEMA ledger FROM PUBLIC;
REVOKE ALL ON ALL FUNCTIONS IN SCHEMA operations FROM PUBLIC;
REVOKE ALL ON SCHEMA ledger, operations FROM PUBLIC;
