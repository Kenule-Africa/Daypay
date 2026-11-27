-- Phase 6 (IMP-P6-001–010): wallet and asset management, built on the Phase 2
-- wallet tables and the Phase 5 ledger.
--
-- Trust model (infrastructure/postgres/wallet-roles.psql, wallet-grants.psql):
--   * Registries (assets, networks, token contracts, providers and provider
--     capabilities) are written only by the database administrator under change
--     control. Every insert and update is kept in wallets.registry_changes.
--   * Customer wallet reads, wallet initialization and deposit-address requests
--     run in SECURITY DEFINER functions owned by the NOLOGIN deypay_wallet_owner.
--     deypay_app executes them for the authenticated customer and gains no table
--     or ledger privilege. Balances come only from ledger.customer_balance.
--   * Provider provisioning results and deposit-address verification are recorded
--     only by deypay_wallet_provisioner (provider-integration workers).
--   * Administrative wallet status changes run as deypay_ledger_controller for the
--     operator bound to the session's login, who must hold WALLET_STATUS_CONTROL.
--
-- A wallet existing, or an asset being enabled, never permits moving funds: the
-- ledger's eligibility checks and capability activations still decide that.
--
-- Error codes:
--   WA001 customer not eligible for wallets   WA002 invalid input
--   WA003 invalid status transition           WA004 unknown wallet or record
--   WA005 unsupported asset or network        WA006 wallet or account restricted
--   WA007 deposits not enabled                WA008 no approved provider
--   WA009 provisioning state conflict         WA010 address rejected
--   WA011 wallet not empty                    WA012 KYC approval required

INSERT INTO audit.admin_permissions(permission_code, description) VALUES
    ('WALLET_STATUS_CONTROL', 'Restrict, suspend, reactivate or close customer wallets')
ON CONFLICT (permission_code) DO NOTHING;

---------------------------------------------------------------------------
-- Registry change history
---------------------------------------------------------------------------
CREATE TABLE wallets.registry_changes (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    registry text NOT NULL,
    record_id uuid NOT NULL,
    operation text NOT NULL CHECK (operation IN ('INSERT','UPDATE')),
    previous jsonb,
    current jsonb NOT NULL,
    changed_by text NOT NULL DEFAULT session_user,
    changed_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    CHECK ((operation = 'UPDATE') = (previous IS NOT NULL))
);
CREATE INDEX registry_changes_record ON wallets.registry_changes(registry, record_id, changed_at);
CREATE TRIGGER immutable_registry_changes BEFORE UPDATE OR DELETE OR TRUNCATE ON wallets.registry_changes
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();

CREATE FUNCTION wallets.record_registry_change() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
BEGIN
    INSERT INTO wallets.registry_changes(registry, record_id, operation, previous, current)
    VALUES (TG_TABLE_SCHEMA || '.' || TG_TABLE_NAME, NEW.id, TG_OP,
        CASE WHEN TG_OP = 'UPDATE' THEN to_jsonb(OLD) END, to_jsonb(NEW));
    RETURN NULL;
END;
$$;

-- IMP-P6-002/003/007: canonical on-chain address for a chain family, or NULL
-- when the text is not a valid address. EVM addresses are stored lowercase;
-- Solana addresses must decode to a 32-byte public key.
CREATE FUNCTION wallets.base58_byte_length(p_text text) RETURNS integer
LANGUAGE plpgsql IMMUTABLE SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    alphabet constant text := '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
    accumulated numeric := 0;
    leading_zeros integer := 0;
    in_prefix boolean := true;
    digit integer;
    bytes integer := 0;
BEGIN
    IF p_text IS NULL OR p_text = '' OR length(p_text) > 64 THEN
        RETURN NULL;
    END IF;
    FOR i IN 1 .. length(p_text) LOOP
        digit := strpos(alphabet, substr(p_text, i, 1)) - 1;
        IF digit < 0 THEN
            RETURN NULL;
        END IF;
        IF in_prefix AND digit = 0 THEN
            leading_zeros := leading_zeros + 1;
        ELSE
            in_prefix := false;
        END IF;
        accumulated := accumulated * 58 + digit;
    END LOOP;
    WHILE accumulated > 0 LOOP
        accumulated := div(accumulated, 256);
        bytes := bytes + 1;
    END LOOP;
    RETURN leading_zeros + bytes;
END;
$$;

CREATE FUNCTION wallets.canonical_address(p_family text, p_address text) RETURNS text
LANGUAGE plpgsql IMMUTABLE SET search_path = pg_catalog, pg_temp AS $$
BEGIN
    IF p_family = 'EVM' AND p_address ~ '^0x[0-9a-fA-F]{40}$' AND p_address !~ '^0x0{40}$' THEN
        RETURN lower(p_address);
    ELSIF p_family = 'SOLANA' AND p_address ~ '^[1-9A-HJ-NP-Za-km-z]{32,44}$'
          AND wallets.base58_byte_length(p_address) = 32 THEN
        RETURN p_address;
    END IF;
    RETURN NULL;
END;
$$;

CREATE FUNCTION wallets.printable(p_text text, p_max integer) RETURNS boolean
LANGUAGE sql IMMUTABLE SET search_path = pg_catalog, pg_temp AS $$
    SELECT p_text IS NULL OR (btrim(p_text) <> '' AND length(p_text) <= p_max AND p_text !~ '[[:cntrl:]]');
$$;

---------------------------------------------------------------------------
-- IMP-P6-001: asset registry
---------------------------------------------------------------------------
-- `enabled` means approved for use in DEYPAY records (wallet profiles may
-- exist). It never permits customer movements; those need ledger activations.
ALTER TABLE wallets.assets
    ADD COLUMN approval_reference text,
    ADD COLUMN suspended_at timestamptz,
    ADD COLUMN suspension_reason text,
    ADD COLUMN updated_at timestamptz NOT NULL DEFAULT now(),
    ADD CONSTRAINT approved_asset_type CHECK (asset_type IN ('STABLECOIN','FIAT')),
    ADD CONSTRAINT asset_symbol_format CHECK (symbol ~ '^[A-Z][A-Z0-9]{1,11}$'),
    ADD CONSTRAINT asset_name_present CHECK (btrim(name) <> '' AND length(name) <= 100),
    ADD CONSTRAINT asset_approval_reference CHECK ((approval_reference IS NULL OR (btrim(approval_reference) <> '' AND length(approval_reference) <= 256 AND approval_reference !~ '[[:cntrl:]]'))),
    ADD CONSTRAINT enabled_asset_is_approved CHECK (NOT enabled OR approval_reference IS NOT NULL),
    ADD CONSTRAINT asset_suspension CHECK ((suspended_at IS NULL) = (suspension_reason IS NULL)
        AND (suspension_reason IS NULL OR (btrim(suspension_reason) <> '' AND length(suspension_reason) <= 500 AND suspension_reason !~ '[[:cntrl:]]')));

-- An enabled asset must have ledger accounts in the Phase 5 chart.
CREATE FUNCTION wallets.guard_asset() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, pg_temp AS $$
BEGIN
    IF TG_OP = 'UPDATE' AND (NEW.id, NEW.asset_type, NEW.created_at) IS DISTINCT FROM (OLD.id, OLD.asset_type, OLD.created_at) THEN
        RAISE EXCEPTION 'asset identity is immutable' USING ERRCODE = '55000';
    END IF;
    IF NEW.enabled AND NOT EXISTS (SELECT 1 FROM ledger.chart_accounts c WHERE NEW.symbol = ANY (c.asset_symbols)) THEN
        RAISE EXCEPTION 'asset % has no accounts in the ledger chart', NEW.symbol USING ERRCODE = 'WA005';
    END IF;
    NEW.updated_at := now();
    RETURN NEW;
END;
$$;
CREATE TRIGGER guard_asset BEFORE INSERT OR UPDATE ON wallets.assets
    FOR EACH ROW EXECUTE FUNCTION wallets.guard_asset();
CREATE TRIGGER asset_history AFTER INSERT OR UPDATE ON wallets.assets
    FOR EACH ROW EXECUTE FUNCTION wallets.record_registry_change();
CREATE TRIGGER no_asset_delete BEFORE DELETE OR TRUNCATE ON wallets.assets
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();

-- Owner decision of 2026-11-26 (DECISIONS: Owner-approved financial ledger
-- architecture): USDC and USDT are the customer wallet assets with six
-- accounting decimals; NGN (two decimals) is for funding, treasury and
-- settlement only and has no customer wallet.
INSERT INTO wallets.assets(symbol, name, asset_type, accounting_precision, enabled, approval_reference) VALUES
    ('USDC', 'USD Coin', 'STABLECOIN', 6, true, 'DECISIONS · Owner-approved financial ledger architecture · 2026-11-26'),
    ('USDT', 'Tether USD', 'STABLECOIN', 6, true, 'DECISIONS · Owner-approved financial ledger architecture · 2026-11-26'),
    ('NGN', 'Nigerian Naira', 'FIAT', 2, true, 'DECISIONS · Owner-approved financial ledger architecture · 2026-11-26')
ON CONFLICT (symbol) DO NOTHING;

---------------------------------------------------------------------------
-- IMP-P6-002: blockchain network registry
---------------------------------------------------------------------------
-- No network is seeded: each is added, with its evidence, once its support is
-- confirmed for DEYPAY's account (PROVIDER-CAPABILITIES.md).
ALTER TABLE wallets.networks
    ALTER COLUMN chain_identifier SET NOT NULL,
    ADD COLUMN deposits_enabled boolean NOT NULL DEFAULT false,
    ADD COLUMN withdrawals_enabled boolean NOT NULL DEFAULT false,
    ADD COLUMN memo_required boolean NOT NULL DEFAULT false,
    ADD COLUMN required_confirmations integer,
    ADD COLUMN confirmation_policy_reference text,
    ADD COLUMN evidence_reference text,
    ADD COLUMN updated_at timestamptz NOT NULL DEFAULT now(),
    ADD CONSTRAINT network_code_format CHECK (code ~ '^[a-z][a-z0-9-]{1,31}$'),
    ADD CONSTRAINT network_name_present CHECK (btrim(name) <> '' AND length(name) <= 100),
    ADD CONSTRAINT supported_chain_family CHECK (chain_family IN ('EVM','SOLANA')),
    ADD CONSTRAINT chain_identifier_format CHECK (CASE chain_family
        WHEN 'EVM' THEN chain_identifier ~ '^[1-9][0-9]{0,18}$'
        WHEN 'SOLANA' THEN chain_identifier ~ '^[1-9A-HJ-NP-Za-km-z]{32,44}$' END),
    ADD CONSTRAINT network_confirmations CHECK (required_confirmations IS NULL OR required_confirmations BETWEEN 1 AND 100000),
    ADD CONSTRAINT network_references CHECK ((confirmation_policy_reference IS NULL OR (btrim(confirmation_policy_reference) <> '' AND length(confirmation_policy_reference) <= 256 AND confirmation_policy_reference !~ '[[:cntrl:]]'))
        AND (evidence_reference IS NULL OR (btrim(evidence_reference) <> '' AND length(evidence_reference) <= 256 AND evidence_reference !~ '[[:cntrl:]]'))),
    ADD CONSTRAINT enabled_network_has_evidence CHECK (NOT enabled OR evidence_reference IS NOT NULL),
    ADD CONSTRAINT network_flows_need_enabled_network CHECK (enabled OR NOT (deposits_enabled OR withdrawals_enabled)),
    ADD CONSTRAINT deposits_need_confirmation_rule CHECK (NOT deposits_enabled
        OR required_confirmations IS NOT NULL OR confirmation_policy_reference IS NOT NULL),
    ADD CONSTRAINT one_network_identity UNIQUE (chain_family, chain_identifier);

CREATE FUNCTION wallets.guard_network() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, pg_temp AS $$
BEGIN
    IF TG_OP = 'UPDATE' AND (NEW.id, NEW.code, NEW.chain_family, NEW.chain_identifier, NEW.created_at)
       IS DISTINCT FROM (OLD.id, OLD.code, OLD.chain_family, OLD.chain_identifier, OLD.created_at) THEN
        RAISE EXCEPTION 'network identity is immutable' USING ERRCODE = '55000';
    END IF;
    NEW.updated_at := now();
    RETURN NEW;
END;
$$;
CREATE TRIGGER guard_network BEFORE INSERT OR UPDATE ON wallets.networks
    FOR EACH ROW EXECUTE FUNCTION wallets.guard_network();
CREATE TRIGGER network_history AFTER INSERT OR UPDATE ON wallets.networks
    FOR EACH ROW EXECUTE FUNCTION wallets.record_registry_change();
CREATE TRIGGER no_network_delete BEFORE DELETE OR TRUNCATE ON wallets.networks
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();

---------------------------------------------------------------------------
-- IMP-P6-003: token contract registry (wallets.asset_networks)
---------------------------------------------------------------------------
-- A mapping's identity (asset, network, contract, decimals) never changes; a
-- token migration retires the old mapping and adds a new one. Only a verified
-- mapping can be enabled, and at most one per asset and network.
ALTER TABLE wallets.asset_networks
    ALTER COLUMN token_contract SET NOT NULL,
    ADD COLUMN status text NOT NULL DEFAULT 'DISABLED',
    ADD COLUMN verification_status text NOT NULL DEFAULT 'UNVERIFIED',
    ADD COLUMN evidence_reference text,
    ADD COLUMN verified_at timestamptz,
    ADD COLUMN updated_at timestamptz NOT NULL DEFAULT now(),
    ADD CONSTRAINT token_contract_status CHECK (status IN ('DISABLED','ENABLED','RETIRED')),
    ADD CONSTRAINT token_contract_verification CHECK (verification_status IN ('UNVERIFIED','VERIFIED','REJECTED')),
    ADD CONSTRAINT token_decimals_range CHECK (token_decimals BETWEEN 0 AND 36),
    ADD CONSTRAINT token_contract_evidence CHECK ((evidence_reference IS NULL OR (btrim(evidence_reference) <> '' AND length(evidence_reference) <= 256 AND evidence_reference !~ '[[:cntrl:]]'))),
    ADD CONSTRAINT verified_contract CHECK ((verification_status = 'VERIFIED') = (verified_at IS NOT NULL)
        AND (verification_status <> 'VERIFIED' OR evidence_reference IS NOT NULL)),
    ADD CONSTRAINT enabled_contract_is_verified CHECK (status <> 'ENABLED' OR verification_status = 'VERIFIED'),
    ADD CONSTRAINT flows_need_enabled_contract CHECK (status = 'ENABLED' OR NOT (deposit_enabled OR withdrawal_enabled)),
    ADD CONSTRAINT asset_network_of_network UNIQUE (id, network_id);
CREATE UNIQUE INDEX one_mapping_per_contract ON wallets.asset_networks(network_id, token_contract);
CREATE UNIQUE INDEX one_enabled_contract ON wallets.asset_networks(asset_id, network_id) WHERE status = 'ENABLED';

CREATE FUNCTION wallets.guard_asset_network() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    family text;
    asset_type text;
    canonical text;
BEGIN
    IF TG_OP = 'UPDATE' THEN
        IF (NEW.id, NEW.asset_id, NEW.network_id, NEW.token_contract, NEW.token_decimals, NEW.created_at)
           IS DISTINCT FROM (OLD.id, OLD.asset_id, OLD.network_id, OLD.token_contract, OLD.token_decimals, OLD.created_at) THEN
            RAISE EXCEPTION 'a token contract mapping is immutable; retire it and add a new one' USING ERRCODE = '55000';
        END IF;
        IF OLD.status = 'RETIRED' AND NEW.status <> 'RETIRED' THEN
            RAISE EXCEPTION 'a retired token contract cannot return' USING ERRCODE = '55000';
        END IF;
        IF OLD.verification_status IN ('VERIFIED','REJECTED') AND NEW.verification_status <> OLD.verification_status THEN
            RAISE EXCEPTION 'a token contract verification decision is final' USING ERRCODE = '55000';
        END IF;
    ELSE
        SELECT n.chain_family INTO family FROM wallets.networks n WHERE n.id = NEW.network_id;
        SELECT a.asset_type INTO asset_type FROM wallets.assets a WHERE a.id = NEW.asset_id;
        IF asset_type IS DISTINCT FROM 'STABLECOIN' THEN
            RAISE EXCEPTION 'only stablecoins have network token contracts' USING ERRCODE = 'WA005';
        END IF;
        canonical := wallets.canonical_address(family, NEW.token_contract);
        IF canonical IS NULL THEN
            RAISE EXCEPTION 'token contract is not a valid % address', family USING ERRCODE = 'WA002';
        END IF;
        NEW.token_contract := canonical;
    END IF;
    IF NEW.status = 'ENABLED' AND (TG_OP = 'INSERT' OR OLD.status <> 'ENABLED') AND NOT EXISTS (
        SELECT 1 FROM wallets.networks n, wallets.assets a
        WHERE n.id = NEW.network_id AND n.enabled AND a.id = NEW.asset_id AND a.enabled AND a.suspended_at IS NULL) THEN
        RAISE EXCEPTION 'enable the asset and network before their token contract' USING ERRCODE = 'WA005';
    END IF;
    NEW.updated_at := now();
    RETURN NEW;
END;
$$;
CREATE TRIGGER guard_asset_network BEFORE INSERT OR UPDATE ON wallets.asset_networks
    FOR EACH ROW EXECUTE FUNCTION wallets.guard_asset_network();
CREATE TRIGGER asset_network_history AFTER INSERT OR UPDATE ON wallets.asset_networks
    FOR EACH ROW EXECUTE FUNCTION wallets.record_registry_change();
CREATE TRIGGER no_asset_network_delete BEFORE DELETE OR TRUNCATE ON wallets.asset_networks
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();

---------------------------------------------------------------------------
-- IMP-P6-008: provider lifecycle and capability register
---------------------------------------------------------------------------
-- DEYPAY owns routing: a provider is selected for a capability only when it is
-- APPROVED and enabled, and its capability for the asset/network is
-- PRODUCTION_ENABLED and available. Nothing is production-enabled here.
ALTER TABLE integrations.providers
    ADD COLUMN lifecycle_status text NOT NULL DEFAULT 'CANDIDATE',
    ADD COLUMN updated_at timestamptz NOT NULL DEFAULT now(),
    ADD CONSTRAINT provider_lifecycle CHECK (lifecycle_status IN ('CANDIDATE','APPROVED','SUSPENDED','RETIRED')),
    ADD CONSTRAINT provider_code_format CHECK (provider_code ~ '^[A-Z][A-Z0-9_]{1,31}$'),
    ADD CONSTRAINT enabled_provider_is_approved CHECK (NOT enabled OR lifecycle_status = 'APPROVED');

CREATE FUNCTION wallets.guard_provider() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, pg_temp AS $$
BEGIN
    IF TG_OP = 'UPDATE' AND (NEW.id, NEW.provider_code, NEW.created_at) IS DISTINCT FROM (OLD.id, OLD.provider_code, OLD.created_at) THEN
        RAISE EXCEPTION 'provider identity is immutable' USING ERRCODE = '55000';
    END IF;
    NEW.updated_at := now();
    RETURN NEW;
END;
$$;
CREATE TRIGGER guard_provider BEFORE INSERT OR UPDATE ON integrations.providers
    FOR EACH ROW EXECUTE FUNCTION wallets.guard_provider();
CREATE TRIGGER provider_history AFTER INSERT OR UPDATE ON integrations.providers
    FOR EACH ROW EXECUTE FUNCTION wallets.record_registry_change();

-- Owner decision (DECISIONS: provider strategy): Nuvion is the first intended
-- infrastructure provider and ImportaPay a retained alternative. Both are
-- candidates only: not approved, not enabled, with no capability recorded.
INSERT INTO integrations.providers(provider_code, provider_name, provider_type, enabled) VALUES
    ('NUVION', 'Nuvion', 'INFRASTRUCTURE', false),
    ('IMPORTAPAY', 'ImportaPay', 'INFRASTRUCTURE', false)
ON CONFLICT (provider_code) DO NOTHING;

-- The Phase 2 capability register (integrations.provider_capabilities) already
-- scopes a provider capability by service, asset, network, corridor, country
-- and environment. Phase 6 adds the verification stage, its evidence, routing
-- priority and operational health. A row with a NULL asset or network covers
-- all of them; a more specific row overrides it.
ALTER TABLE integrations.provider_capabilities
    ADD COLUMN verification_level text NOT NULL DEFAULT 'PENDING_CONFIRMATION',
    ADD COLUMN evidence_reference text,
    ADD COLUMN routing_priority integer NOT NULL DEFAULT 100,
    ADD COLUMN operational boolean NOT NULL DEFAULT true,
    ADD COLUMN created_at timestamptz NOT NULL DEFAULT now(),
    ADD COLUMN updated_at timestamptz NOT NULL DEFAULT now(),
    ADD CONSTRAINT capability_service_format CHECK (service ~ '^[A-Z][A-Z0-9_]{2,63}$'),
    ADD CONSTRAINT capability_environment CHECK (environment IN ('production','sandbox')),
    ADD CONSTRAINT capability_verification_level CHECK (verification_level IN ('PENDING_CONFIRMATION','DOCUMENTED',
        'TECHNICALLY_VERIFIED','SANDBOX_TESTED','CONTRACTUALLY_APPROVED','PRODUCTION_ENABLED','UNSUPPORTED')),
    ADD CONSTRAINT capability_evidence CHECK ((evidence_reference IS NULL OR (btrim(evidence_reference) <> ''
        AND length(evidence_reference) <= 256 AND evidence_reference !~ '[[:cntrl:]]'))
        AND (verification_level IN ('PENDING_CONFIRMATION','UNSUPPORTED') OR evidence_reference IS NOT NULL)),
    ADD CONSTRAINT production_approval_is_enabled_level CHECK (production_approved = (verification_level = 'PRODUCTION_ENABLED')),
    ADD CONSTRAINT unsupported_is_unavailable CHECK (verification_level <> 'UNSUPPORTED' OR NOT available),
    ADD CONSTRAINT capability_routing_priority CHECK (routing_priority BETWEEN 1 AND 1000);
CREATE FUNCTION wallets.guard_capability() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, pg_temp AS $$
BEGIN
    IF TG_OP = 'UPDATE' AND (NEW.id, NEW.provider_id, NEW.service, NEW.asset_id, NEW.network_id, NEW.currency_corridor,
            NEW.country_code, NEW.environment, NEW.created_at)
       IS DISTINCT FROM (OLD.id, OLD.provider_id, OLD.service, OLD.asset_id, OLD.network_id, OLD.currency_corridor,
            OLD.country_code, OLD.environment, OLD.created_at) THEN
        RAISE EXCEPTION 'capability identity is immutable' USING ERRCODE = '55000';
    END IF;
    NEW.updated_at := now();
    RETURN NEW;
END;
$$;
CREATE TRIGGER guard_capability BEFORE INSERT OR UPDATE ON integrations.provider_capabilities
    FOR EACH ROW EXECUTE FUNCTION wallets.guard_capability();
CREATE TRIGGER capability_history AFTER INSERT OR UPDATE ON integrations.provider_capabilities
    FOR EACH ROW EXECUTE FUNCTION wallets.record_registry_change();
CREATE TRIGGER no_capability_delete BEFORE DELETE OR TRUNCATE ON integrations.provider_capabilities
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();

---------------------------------------------------------------------------
-- IMP-P6-004/005: customer wallet profiles and status
---------------------------------------------------------------------------
-- provisioning_status is the internal lifecycle (PENDING until the ledger
-- accounts exist, then COMPLETE). Deposit-address provisioning is separate.
ALTER TABLE wallets.wallet_profiles
    ADD COLUMN status_reason text,
    ADD COLUMN status_actor_type text NOT NULL DEFAULT 'SYSTEM',
    ADD COLUMN status_actor_id uuid,
    ADD COLUMN status_changed_at timestamptz NOT NULL DEFAULT now(),
    ADD COLUMN closed_at timestamptz,
    ADD CONSTRAINT wallet_provisioning_status CHECK (provisioning_status IN ('PENDING','COMPLETE')),
    ADD CONSTRAINT provisioning_wallet CHECK ((status = 'PROVISIONING') = (provisioning_status = 'PENDING')),
    ADD CONSTRAINT closed_wallet CHECK ((status = 'CLOSED') = (closed_at IS NOT NULL)),
    ADD CONSTRAINT wallet_status_reason CHECK ((status_reason IS NULL OR (btrim(status_reason) <> '' AND length(status_reason) <= 500 AND status_reason !~ '[[:cntrl:]]'))),
    ADD CONSTRAINT wallet_status_actor CHECK (status_actor_type IN ('SYSTEM','ADMIN')
        AND (status_actor_type = 'ADMIN') = (status_actor_id IS NOT NULL));

CREATE TABLE wallets.wallet_status_history (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_profile_id uuid NOT NULL REFERENCES wallets.wallet_profiles,
    previous_status text,
    new_status text NOT NULL,
    reason text,
    actor_type text NOT NULL,
    actor_id uuid,
    database_login text NOT NULL DEFAULT session_user,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX wallet_status_history_wallet ON wallets.wallet_status_history(wallet_profile_id, created_at);
CREATE TRIGGER immutable_wallet_status_history BEFORE UPDATE OR DELETE OR TRUNCATE ON wallets.wallet_status_history
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();

-- Allowed transitions. PROVISIONING becomes ACTIVE only once the ledger
-- accounts exist; CLOSED is final; a closed wallet stays in history.
CREATE FUNCTION wallets.guard_wallet_profile() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
BEGIN
    IF TG_OP = 'UPDATE' THEN
        IF (NEW.id, NEW.user_id, NEW.asset_id, NEW.asset_symbol, NEW.created_at)
           IS DISTINCT FROM (OLD.id, OLD.user_id, OLD.asset_id, OLD.asset_symbol, OLD.created_at) THEN
            RAISE EXCEPTION 'wallet identity is immutable' USING ERRCODE = '55000';
        END IF;
        IF OLD.provisioning_status = 'COMPLETE' AND NEW.provisioning_status <> 'COMPLETE' THEN
            RAISE EXCEPTION 'a provisioned wallet cannot return to provisioning' USING ERRCODE = 'WA003';
        END IF;
        IF NEW.status IS DISTINCT FROM OLD.status THEN
            IF NOT ((OLD.status = 'PROVISIONING' AND NEW.status = 'ACTIVE')
                    OR (OLD.status IN ('ACTIVE','RESTRICTED','SUSPENDED')
                        AND NEW.status IN ('ACTIVE','RESTRICTED','SUSPENDED','CLOSED'))) THEN
                RAISE EXCEPTION 'wallet status cannot change from % to %', OLD.status, NEW.status USING ERRCODE = 'WA003';
            END IF;
            NEW.status_changed_at := now();
            NEW.closed_at := CASE WHEN NEW.status = 'CLOSED' THEN now() END;
        END IF;
        NEW.updated_at := now();
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER guard_wallet_profile BEFORE UPDATE ON wallets.wallet_profiles
    FOR EACH ROW EXECUTE FUNCTION wallets.guard_wallet_profile();
CREATE TRIGGER no_wallet_delete BEFORE DELETE OR TRUNCATE ON wallets.wallet_profiles
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();

CREATE FUNCTION wallets.record_wallet_status() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
BEGIN
    IF TG_OP = 'INSERT' OR NEW.status IS DISTINCT FROM OLD.status THEN
        INSERT INTO wallets.wallet_status_history(wallet_profile_id, previous_status, new_status, reason, actor_type, actor_id)
        VALUES (NEW.id, CASE WHEN TG_OP = 'UPDATE' THEN OLD.status END, NEW.status, NEW.status_reason,
            NEW.status_actor_type, NEW.status_actor_id);
    END IF;
    RETURN NULL;
END;
$$;
CREATE TRIGGER wallet_status_history AFTER INSERT OR UPDATE ON wallets.wallet_profiles
    FOR EACH ROW EXECUTE FUNCTION wallets.record_wallet_status();

---------------------------------------------------------------------------
-- IMP-P6-007/008: deposit-address provisioning requests and addresses
---------------------------------------------------------------------------
-- A request is DEYPAY's durable identity for one provisioning attempt. Its
-- reference is the idempotency key sent to the provider. SUBMITTED and UNKNOWN
-- are resolved by looking up the same request with the same provider, never by
-- submitting again or through another provider.
ALTER TABLE wallets.wallet_provisioning_requests
    ADD COLUMN asset_network_id uuid NOT NULL REFERENCES wallets.asset_networks,
    ADD COLUMN external_reference text,
    ADD COLUMN submitted_at timestamptz,
    ADD COLUMN completed_at timestamptz,
    ADD CONSTRAINT provisioning_status CHECK (status IN
        ('PENDING','SUBMITTED','UNKNOWN','PROVISIONED','FAILED','UNSUPPORTED')),
    ADD CONSTRAINT provisioning_reference CHECK (request_reference ~ '^deypay-addr-[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'),
    ADD CONSTRAINT provisioning_external_reference CHECK ((external_reference IS NULL OR (btrim(external_reference) <> '' AND length(external_reference) <= 512 AND external_reference !~ '[[:cntrl:]]'))),
    ADD CONSTRAINT provisioning_error_code CHECK (last_error_code IS NULL OR last_error_code ~ '^[A-Z][A-Z0-9_]{1,63}$'),
    ADD CONSTRAINT provisioning_completion CHECK ((status IN ('PROVISIONED','FAILED','UNSUPPORTED')) = (completed_at IS NOT NULL)),
    ADD CONSTRAINT provisioning_submission CHECK (status NOT IN ('SUBMITTED','UNKNOWN','PROVISIONED') OR submitted_at IS NOT NULL);
CREATE UNIQUE INDEX provisioning_request_reference ON wallets.wallet_provisioning_requests(request_reference);
CREATE UNIQUE INDEX one_open_provisioning ON wallets.wallet_provisioning_requests(wallet_profile_id, asset_network_id)
    WHERE status IN ('PENDING','SUBMITTED','UNKNOWN');

CREATE FUNCTION wallets.guard_provisioning_request() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, pg_temp AS $$
BEGIN
    IF (NEW.id, NEW.wallet_profile_id, NEW.provider_id, NEW.request_reference, NEW.asset_network_id, NEW.created_at)
       IS DISTINCT FROM (OLD.id, OLD.wallet_profile_id, OLD.provider_id, OLD.request_reference, OLD.asset_network_id, OLD.created_at) THEN
        RAISE EXCEPTION 'a provisioning request keeps its provider, wallet and reference' USING ERRCODE = '55000';
    END IF;
    IF OLD.external_reference IS NOT NULL AND NEW.external_reference IS DISTINCT FROM OLD.external_reference THEN
        RAISE EXCEPTION 'a provider reference cannot change' USING ERRCODE = '55000';
    END IF;
    IF NEW.status IS DISTINCT FROM OLD.status AND NOT (
        (OLD.status = 'PENDING' AND NEW.status IN ('SUBMITTED','FAILED','UNSUPPORTED'))
        OR (OLD.status = 'SUBMITTED' AND NEW.status IN ('PENDING','UNKNOWN','PROVISIONED','FAILED','UNSUPPORTED'))
        OR (OLD.status = 'UNKNOWN' AND NEW.status IN ('SUBMITTED','PROVISIONED','FAILED'))) THEN
        RAISE EXCEPTION 'provisioning cannot change from % to %', OLD.status, NEW.status USING ERRCODE = 'WA009';
    END IF;
    NEW.updated_at := now();
    RETURN NEW;
END;
$$;
CREATE TRIGGER guard_provisioning_request BEFORE UPDATE ON wallets.wallet_provisioning_requests
    FOR EACH ROW EXECUTE FUNCTION wallets.guard_provisioning_request();
CREATE TRIGGER no_provisioning_delete BEFORE DELETE OR TRUNCATE ON wallets.wallet_provisioning_requests
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();

-- One on-chain address (with its memo) on a network belongs to one customer
-- and one provider for ever, but may serve several of that customer's tokens
-- (design §12.2). Addresses start unverified and are never shown until
-- verified; replacing one means retiring it and recording another.
ALTER TABLE wallets.wallet_addresses
    ADD COLUMN network_id uuid NOT NULL,
    ADD COLUMN provisioning_request_id uuid REFERENCES wallets.wallet_provisioning_requests,
    ADD COLUMN verification_status text NOT NULL DEFAULT 'UNVERIFIED',
    ADD COLUMN evidence_reference text,
    ADD COLUMN evidence_sha256 bytea,
    ADD COLUMN verified_at timestamptz,
    ADD COLUMN activated_at timestamptz,
    ADD COLUMN retirement_reason text,
    ADD COLUMN updated_at timestamptz NOT NULL DEFAULT now(),
    ADD CONSTRAINT address_network FOREIGN KEY (asset_network_id, network_id)
        REFERENCES wallets.asset_networks(id, network_id),
    ADD CONSTRAINT address_status CHECK (status IN ('PENDING_VERIFICATION','ACTIVE','SUSPENDED','RETIRED','REJECTED')),
    ADD CONSTRAINT address_verification CHECK (verification_status IN ('UNVERIFIED','VERIFIED','REJECTED')),
    ADD CONSTRAINT address_evidence CHECK ((evidence_reference IS NULL OR (btrim(evidence_reference) <> '' AND length(evidence_reference) <= 512 AND evidence_reference !~ '[[:cntrl:]]'))
        AND (evidence_sha256 IS NULL OR length(evidence_sha256) = 32)),
    ADD CONSTRAINT verified_address CHECK ((verification_status = 'VERIFIED') = (verified_at IS NOT NULL)
        AND (verification_status <> 'VERIFIED' OR (evidence_reference IS NOT NULL AND evidence_sha256 IS NOT NULL))),
    ADD CONSTRAINT live_address_is_verified CHECK (status NOT IN ('ACTIVE','SUSPENDED')
        OR (verification_status = 'VERIFIED' AND activated_at IS NOT NULL)),
    ADD CONSTRAINT rejected_address CHECK ((status = 'REJECTED') = (verification_status = 'REJECTED')),
    ADD CONSTRAINT retired_address CHECK ((status = 'RETIRED') = (retired_at IS NOT NULL)
        AND (status <> 'RETIRED' OR retirement_reason IS NOT NULL) AND (retirement_reason IS NULL OR (btrim(retirement_reason) <> '' AND length(retirement_reason) <= 500 AND retirement_reason !~ '[[:cntrl:]]'))),
    ADD CONSTRAINT address_memo CHECK ((memo_tag IS NULL OR (btrim(memo_tag) <> '' AND length(memo_tag) <= 128 AND memo_tag !~ '[[:cntrl:]]'))),
    ADD CONSTRAINT address_external_reference CHECK ((external_wallet_reference IS NULL OR (btrim(external_wallet_reference) <> '' AND length(external_wallet_reference) <= 512 AND external_wallet_reference !~ '[[:cntrl:]]')));
CREATE UNIQUE INDEX one_current_address ON wallets.wallet_addresses(wallet_profile_id, asset_network_id)
    WHERE status IN ('PENDING_VERIFICATION','ACTIVE','SUSPENDED');
CREATE INDEX wallet_addresses_onchain ON wallets.wallet_addresses(network_id, address);

CREATE FUNCTION wallets.guard_wallet_address() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    network record;
    canonical text;
    holder uuid;
BEGIN
    IF TG_OP = 'UPDATE' THEN
        IF (NEW.id, NEW.wallet_profile_id, NEW.asset_id, NEW.asset_network_id, NEW.network_id, NEW.provider_id,
            NEW.address, NEW.memo_tag, NEW.external_wallet_reference, NEW.provisioning_request_id, NEW.created_at)
           IS DISTINCT FROM
           (OLD.id, OLD.wallet_profile_id, OLD.asset_id, OLD.asset_network_id, OLD.network_id, OLD.provider_id,
            OLD.address, OLD.memo_tag, OLD.external_wallet_reference, OLD.provisioning_request_id, OLD.created_at) THEN
            RAISE EXCEPTION 'a deposit address is never replaced in place; retire it and record another'
                USING ERRCODE = '55000';
        END IF;
        IF NEW.status IS DISTINCT FROM OLD.status AND NOT (
            (OLD.status = 'PENDING_VERIFICATION' AND NEW.status IN ('ACTIVE','REJECTED','RETIRED'))
            OR (OLD.status = 'ACTIVE' AND NEW.status IN ('SUSPENDED','RETIRED'))
            OR (OLD.status = 'SUSPENDED' AND NEW.status IN ('ACTIVE','RETIRED'))) THEN
            RAISE EXCEPTION 'deposit address cannot change from % to %', OLD.status, NEW.status USING ERRCODE = 'WA009';
        END IF;
        NEW.updated_at := now();
        RETURN NEW;
    END IF;
    SELECT n.chain_family, n.memo_required INTO network FROM wallets.networks n WHERE n.id = NEW.network_id;
    canonical := wallets.canonical_address(network.chain_family, NEW.address);
    IF canonical IS NULL THEN
        RAISE EXCEPTION 'not a valid % address', network.chain_family USING ERRCODE = 'WA010';
    END IF;
    IF network.memo_required AND NEW.memo_tag IS NULL THEN
        RAISE EXCEPTION 'this network requires a memo or tag' USING ERRCODE = 'WA010';
    END IF;
    IF NEW.status <> 'PENDING_VERIFICATION' OR NEW.verification_status <> 'UNVERIFIED' THEN
        RAISE EXCEPTION 'a new deposit address starts unverified' USING ERRCODE = 'WA010';
    END IF;
    NEW.address := canonical;
    SELECT w.user_id INTO holder FROM wallets.wallet_profiles w WHERE w.id = NEW.wallet_profile_id;
    PERFORM pg_advisory_xact_lock(hashtextextended('deypay.wallets.address|' || NEW.network_id::text || '|'
        || canonical || '|' || coalesce(NEW.memo_tag, ''), 0));
    IF EXISTS (SELECT 1 FROM wallets.wallet_addresses a JOIN wallets.wallet_profiles w ON w.id = a.wallet_profile_id
               WHERE a.network_id = NEW.network_id AND a.address = canonical
                 AND a.memo_tag IS NOT DISTINCT FROM NEW.memo_tag
                 AND (w.user_id <> holder OR a.provider_id <> NEW.provider_id)) THEN
        RAISE EXCEPTION 'this address belongs to another customer or provider' USING ERRCODE = 'WA010';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER guard_wallet_address BEFORE INSERT OR UPDATE ON wallets.wallet_addresses
    FOR EACH ROW EXECUTE FUNCTION wallets.guard_wallet_address();
CREATE TRIGGER no_address_delete BEFORE DELETE OR TRUNCATE ON wallets.wallet_addresses
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();

---------------------------------------------------------------------------
-- Ledger read interfaces for the wallet module (owned by deypay_ledger_owner
-- after ledger-grants.psql)
---------------------------------------------------------------------------
-- Hold and activation state of one asset, without exposing the ledger tables.
CREATE FUNCTION ledger.asset_controls(p_asset uuid) RETURNS TABLE(on_hold boolean, activated text[])
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
    SELECT EXISTS (SELECT 1 FROM ledger.asset_holds h WHERE h.asset_id = p_asset AND h.active),
           coalesce((SELECT array_agg(c.capability ORDER BY c.capability) FROM ledger.capability_activations c
                     WHERE c.asset_id = p_asset AND c.deactivated_at IS NULL), '{}'::text[]);
$$;

-- The Phase 5 operator binding, callable by other modules' definer functions:
-- returns the session's authenticated operator if it is the stated one and
-- holds the permission, else raises LE016/LE013.
CREATE FUNCTION ledger.authenticated_operator(p_claimed uuid, p_permission text) RETURNS uuid
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
    SELECT ledger.authorize_operator(p_claimed, p_permission);
$$;
REVOKE ALL ON FUNCTION ledger.asset_controls(uuid), ledger.authenticated_operator(uuid,text) FROM PUBLIC;

---------------------------------------------------------------------------
-- Internal helpers (owned by deypay_wallet_owner; never granted)
---------------------------------------------------------------------------
CREATE FUNCTION wallets.utc(p_time timestamptz) RETURNS text
LANGUAGE sql STABLE SET search_path = pg_catalog, pg_temp AS $$
    SELECT to_char(p_time AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"');
$$;

-- Customers who may hold a session under the authentication release
-- (CONTACT_VERIFIED through KYC_APPROVED with a verified email) receive wallets.
CREATE FUNCTION wallets.customer_eligible(p_customer uuid) RETURNS boolean
LANGUAGE sql STABLE SET search_path = pg_catalog, pg_temp AS $$
    SELECT EXISTS (SELECT 1 FROM identity.users u WHERE u.id = p_customer AND u.email_verified_at IS NOT NULL
                   AND u.status IN ('CONTACT_VERIFIED','KYC_REQUIRED','KYC_PENDING','KYC_APPROVED'));
$$;

-- IMP-P6-009: everything that currently limits a wallet. An empty array means
-- the database would allow customer spending; the application gate must agree.
CREATE FUNCTION wallets.restrictions(p_wallet uuid) RETURNS text[]
LANGUAGE sql STABLE SET search_path = pg_catalog, pg_temp AS $$
    SELECT array_remove(ARRAY[
        CASE w.status WHEN 'PROVISIONING' THEN 'WALLET_PROVISIONING' WHEN 'RESTRICTED' THEN 'WALLET_RESTRICTED'
                      WHEN 'SUSPENDED' THEN 'WALLET_SUSPENDED' WHEN 'CLOSED' THEN 'WALLET_CLOSED' END,
        CASE WHEN u.status IN ('RESTRICTED','SUSPENDED','CLOSED','KYC_REJECTED')
                  OR EXISTS (SELECT 1 FROM compliance.account_restrictions r WHERE r.user_id = u.id AND r.active)
             THEN 'ACCOUNT_RESTRICTED' END,
        CASE WHEN u.status <> 'KYC_APPROVED' THEN 'KYC_REQUIRED' END,
        CASE WHEN NOT a.enabled OR a.suspended_at IS NOT NULL THEN 'ASSET_UNAVAILABLE' END,
        CASE WHEN c.on_hold THEN 'ASSET_ON_HOLD' END,
        CASE WHEN NOT (c.activated @> ARRAY['CUSTOMER_TRANSFER'] OR c.activated @> ARRAY['CUSTOMER_RESERVATION'])
             THEN 'FINANCIAL_ACTIVITY_NOT_ENABLED' END
    ], NULL)
    FROM wallets.wallet_profiles w
    JOIN identity.users u ON u.id = w.user_id
    JOIN wallets.assets a ON a.id = w.asset_id
    CROSS JOIN LATERAL ledger.asset_controls(w.asset_id) c
    WHERE w.id = p_wallet;
$$;

-- The provider DEYPAY routes a new deposit-address request to, if any: the
-- most specific production capability row per provider decides, and it must
-- be production-enabled, approved, available and operational, for an
-- approved and enabled provider.
CREATE FUNCTION wallets.select_address_provider(p_asset_network uuid)
RETURNS TABLE(provider_id uuid, provider_code text)
LANGUAGE sql STABLE SET search_path = pg_catalog, pg_temp AS $$
    SELECT s.provider_id, s.provider_code FROM (
        SELECT DISTINCT ON (p.id) p.id AS provider_id, p.provider_code, c.verification_level, c.production_approved,
               c.available, c.operational, c.routing_priority, p.enabled, p.lifecycle_status
        FROM wallets.asset_networks an
        JOIN integrations.provider_capabilities c
          ON c.service = 'DEPOSIT_ADDRESS_PROVISIONING' AND c.environment = 'production'
         AND c.currency_corridor IS NULL AND c.country_code IS NULL
         AND (c.asset_id = an.asset_id OR c.asset_id IS NULL) AND (c.network_id = an.network_id OR c.network_id IS NULL)
        JOIN integrations.providers p ON p.id = c.provider_id
        WHERE an.id = p_asset_network
        ORDER BY p.id, (c.asset_id IS NULL)::integer + (c.network_id IS NULL)::integer) s
    WHERE s.verification_level = 'PRODUCTION_ENABLED' AND s.production_approved AND s.available AND s.operational
      AND s.enabled AND s.lifecycle_status = 'APPROVED'
    ORDER BY s.routing_priority, s.provider_code
    LIMIT 1;
$$;

-- Supported asset networks of one wallet, with what the customer may do on each.
CREATE FUNCTION wallets.wallet_network_view(p_wallet uuid)
RETURNS TABLE(asset_network_id uuid, network_code text, network_name text, chain_family text,
    token_contract text, token_decimals smallint, memo_required boolean, required_confirmations integer,
    deposits_enabled boolean, deposit_available boolean, withdrawal_available boolean,
    address_status text, address text, memo text)
LANGUAGE sql STABLE SET search_path = pg_catalog, pg_temp AS $$
    WITH wallet AS (
        SELECT w.id, w.asset_id, w.status, r.restrictions,
               w.status IN ('ACTIVE','RESTRICTED')
               AND NOT r.restrictions && ARRAY['ACCOUNT_RESTRICTED','KYC_REQUIRED','ASSET_UNAVAILABLE','ASSET_ON_HOLD']
               AND c.activated @> ARRAY['CUSTOMER_CREDIT'] AS can_receive
        FROM wallets.wallet_profiles w
        CROSS JOIN LATERAL (SELECT wallets.restrictions(w.id) AS restrictions) r
        CROSS JOIN LATERAL ledger.asset_controls(w.asset_id) c
        WHERE w.id = p_wallet)
    SELECT an.id, n.code, n.name, n.chain_family, an.token_contract::text, an.token_decimals, n.memo_required,
           n.required_confirmations, (n.deposits_enabled AND an.deposit_enabled),
           f.receivable AND (live.address IS NOT NULL OR EXISTS (SELECT 1 FROM wallets.select_address_provider(an.id))),
           cardinality(v.restrictions) = 0 AND n.withdrawals_enabled AND an.withdrawal_enabled,
           -- A verified address is shown only while the wallet may receive on it.
           CASE WHEN live.address IS NOT NULL AND f.receivable THEN 'ACTIVE'
                WHEN live.address IS NOT NULL THEN 'HIDDEN'
                WHEN open.id IS NOT NULL THEN 'PENDING'
                ELSE 'NONE' END,
           CASE WHEN f.receivable THEN live.address END,
           CASE WHEN f.receivable THEN live.memo_tag END
    FROM wallet v
    JOIN wallets.asset_networks an ON an.asset_id = v.asset_id AND an.status = 'ENABLED'
    JOIN wallets.networks n ON n.id = an.network_id AND n.enabled
    CROSS JOIN LATERAL (SELECT v.can_receive AND n.deposits_enabled AND an.deposit_enabled AS receivable) f
    LEFT JOIN LATERAL (SELECT a.address, a.memo_tag FROM wallets.wallet_addresses a
                       WHERE a.wallet_profile_id = v.id AND a.asset_network_id = an.id AND a.status = 'ACTIVE'
                         AND a.verification_status = 'VERIFIED') live ON true
    -- In progress: an open request, or an assigned address awaiting verification.
    LEFT JOIN LATERAL (SELECT r.id FROM wallets.wallet_provisioning_requests r
                       WHERE r.wallet_profile_id = v.id AND r.asset_network_id = an.id
                         AND r.status IN ('PENDING','SUBMITTED','UNKNOWN')
                       UNION ALL
                       SELECT a.id FROM wallets.wallet_addresses a
                       WHERE a.wallet_profile_id = v.id AND a.asset_network_id = an.id
                         AND a.status = 'PENDING_VERIFICATION'
                       LIMIT 1) open ON true
    ORDER BY n.code;
$$;

CREATE FUNCTION wallets.owned_wallet(p_customer uuid, p_wallet uuid) RETURNS wallets.wallet_profiles
LANGUAGE plpgsql STABLE SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    wallet wallets.wallet_profiles%ROWTYPE;
BEGIN
    SELECT * INTO wallet FROM wallets.wallet_profiles w WHERE w.id = p_wallet AND w.user_id = p_customer;
    IF NOT FOUND OR p_customer IS NULL THEN
        RAISE EXCEPTION 'unknown wallet' USING ERRCODE = 'WA004';
    END IF;
    RETURN wallet;
END;
$$;

CREATE FUNCTION wallets.audit(p_actor_type text, p_actor uuid, p_action text, p_resource uuid, p_reason text,
    p_metadata jsonb) RETURNS void
LANGUAGE sql SET search_path = pg_catalog, pg_temp AS $$
    INSERT INTO audit.audit_events(actor_type, actor_id, action, resource_type, resource_id, reason, metadata)
    VALUES (p_actor_type, p_actor, p_action, 'wallets.wallet_profile', p_resource, p_reason,
        coalesce(p_metadata, '{}'::jsonb) || jsonb_build_object('database_login', session_user::text));
$$;

---------------------------------------------------------------------------
-- Customer interface (deypay_app; the customer is the authenticated one)
---------------------------------------------------------------------------
-- IMP-P6-004: creates the USDC and USDT wallet profiles and their zero-balance
-- ledger accounts in one transaction. Idempotent and safe under concurrency;
-- a failure leaves nothing behind and a retry completes the work. Creates no value.
CREATE FUNCTION wallets.initialize_customer_wallets(p_customer uuid) RETURNS integer
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp SET lock_timeout = '10s' AS $$
DECLARE
    pending record;
    provisioned integer := 0;
BEGIN
    IF NOT wallets.customer_eligible(p_customer) THEN
        RAISE EXCEPTION 'customer is not eligible for wallet profiles' USING ERRCODE = 'WA001';
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM wallets.assets a
        WHERE a.asset_type = 'STABLECOIN' AND a.symbol IN ('USDC','USDT') AND a.enabled AND a.suspended_at IS NULL
          AND NOT EXISTS (SELECT 1 FROM wallets.wallet_profiles w WHERE w.user_id = p_customer AND w.asset_id = a.id))
       AND NOT EXISTS (SELECT 1 FROM wallets.wallet_profiles w
                       WHERE w.user_id = p_customer AND w.provisioning_status = 'PENDING') THEN
        RETURN 0;
    END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended('deypay.wallets.initialize|' || p_customer::text, 0));
    INSERT INTO wallets.wallet_profiles(user_id, asset_id, asset_symbol, status, provisioning_status, status_reason)
    SELECT p_customer, a.id, a.symbol, 'PROVISIONING', 'PENDING', 'Wallet profile created'
    FROM wallets.assets a
    WHERE a.asset_type = 'STABLECOIN' AND a.symbol IN ('USDC','USDT') AND a.enabled AND a.suspended_at IS NULL
    ON CONFLICT (user_id, asset_id) DO NOTHING;
    FOR pending IN SELECT w.id, w.asset_symbol FROM wallets.wallet_profiles w
                   WHERE w.user_id = p_customer AND w.provisioning_status = 'PENDING' ORDER BY w.id FOR UPDATE LOOP
        PERFORM ledger.open_customer_accounts(pending.id);
        UPDATE wallets.wallet_profiles w SET status = 'ACTIVE', provisioning_status = 'COMPLETE',
            status_reason = 'Ledger accounts opened', status_actor_type = 'SYSTEM', status_actor_id = NULL
        WHERE w.id = pending.id;
        PERFORM wallets.audit('SYSTEM', NULL, 'WALLET_PROVISIONED', pending.id, NULL,
            jsonb_build_object('asset', pending.asset_symbol, 'customer_id', p_customer));
        provisioned := provisioned + 1;
    END LOOP;
    RETURN provisioned;
END;
$$;

-- IMP-P6-006: the customer's wallets (or one of them) with ledger balances.
-- Balances are NULL only while a wallet's ledger accounts are being opened.
CREATE FUNCTION wallets.customer_wallets(p_customer uuid, p_wallet uuid)
RETURNS TABLE(wallet_id uuid, asset text, asset_name text, accounting_precision smallint, status text,
    provisioning_status text, created_at text, updated_at text, available_minor text, reserved_minor text,
    total_minor text, restrictions text[], deposit_networks integer)
LANGUAGE plpgsql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_column
BEGIN
    IF p_customer IS NULL THEN
        RAISE EXCEPTION 'a customer is required' USING ERRCODE = 'WA002';
    END IF;
    RETURN QUERY
    SELECT w.id, w.asset_symbol, a.name, a.accounting_precision, w.status, w.provisioning_status,
           wallets.utc(w.created_at), wallets.utc(w.updated_at), b.available_minor, b.reserved_minor, b.total_minor,
           wallets.restrictions(w.id),
           (SELECT count(*)::integer FROM wallets.wallet_network_view(w.id) n WHERE n.deposit_available)
    FROM wallets.wallet_profiles w
    JOIN wallets.assets a ON a.id = w.asset_id
    LEFT JOIN LATERAL ledger.customer_balance(w.user_id, w.asset_id) b ON w.provisioning_status = 'COMPLETE'
    WHERE w.user_id = p_customer AND (p_wallet IS NULL OR w.id = p_wallet)
    ORDER BY w.asset_symbol;
END;
$$;

CREATE FUNCTION wallets.customer_wallet_networks(p_customer uuid, p_wallet uuid)
RETURNS TABLE(network_code text, network_name text, chain_family text, token_contract text,
    token_decimals smallint, memo_required boolean, required_confirmations integer, deposits_enabled boolean,
    deposit_available boolean, withdrawal_available boolean, address_status text, address text, memo text)
LANGUAGE plpgsql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_column
DECLARE
    wallet wallets.wallet_profiles%ROWTYPE := wallets.owned_wallet(p_customer, p_wallet);
BEGIN
    RETURN QUERY
    SELECT v.network_code, v.network_name, v.chain_family, v.token_contract, v.token_decimals, v.memo_required,
           v.required_confirmations, v.deposits_enabled, v.deposit_available, v.withdrawal_available,
           v.address_status, v.address, v.memo
    FROM wallets.wallet_network_view(wallet.id) v;
END;
$$;

-- IMP-P6-002/003: enabled networks and their enabled token mappings. A named
-- asset must be an enabled customer wallet asset.
CREATE FUNCTION wallets.supported_networks(p_asset text)
RETURNS TABLE(network_code text, network_name text, chain_family text, asset text, token_contract text,
    token_decimals smallint, memo_required boolean, required_confirmations integer, deposits boolean, withdrawals boolean)
LANGUAGE plpgsql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_column
BEGIN
    IF p_asset IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM wallets.assets a WHERE a.symbol = p_asset AND a.asset_type = 'STABLECOIN'
          AND a.symbol IN ('USDC','USDT') AND a.enabled AND a.suspended_at IS NULL) THEN
        RAISE EXCEPTION 'unsupported asset' USING ERRCODE = 'WA005';
    END IF;
    RETURN QUERY
    SELECT n.code, n.name, n.chain_family, a.symbol, an.token_contract::text, an.token_decimals, n.memo_required,
           n.required_confirmations, n.deposits_enabled AND an.deposit_enabled,
           n.withdrawals_enabled AND an.withdrawal_enabled
    FROM wallets.networks n
    JOIN wallets.asset_networks an ON an.network_id = n.id AND an.status = 'ENABLED'
    JOIN wallets.assets a ON a.id = an.asset_id AND a.enabled AND a.suspended_at IS NULL
    WHERE n.enabled AND (p_asset IS NULL OR a.symbol = p_asset)
    ORDER BY n.code, a.symbol;
END;
$$;

-- IMP-P6-007/008: returns the verified deposit address, the open request, or
-- opens a durable request with the provider DEYPAY routes to. Fails closed
-- when deposits are not enabled, the customer is not eligible, or no approved
-- provider capability exists. Never contacts a provider itself.
CREATE FUNCTION wallets.request_deposit_address(p_customer uuid, p_wallet uuid, p_network text)
RETURNS TABLE(status text, address text, memo text, created boolean)
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp SET lock_timeout = '10s' AS $$
#variable_conflict use_column
DECLARE
    wallet wallets.wallet_profiles%ROWTYPE := wallets.owned_wallet(p_customer, p_wallet);
    limits text[];
    network record;
    provider record;
    opened uuid;
BEGIN
    PERFORM 1 FROM wallets.wallet_profiles w WHERE w.id = wallet.id FOR UPDATE;
    SELECT * INTO network FROM wallets.wallet_network_view(wallet.id) v WHERE v.network_code = p_network;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'network is not supported for this wallet' USING ERRCODE = 'WA005';
    END IF;
    limits := wallets.restrictions(wallet.id);
    IF wallet.status NOT IN ('ACTIVE','RESTRICTED')
       OR limits && ARRAY['ACCOUNT_RESTRICTED','ASSET_UNAVAILABLE','ASSET_ON_HOLD'] THEN
        RAISE EXCEPTION 'this wallet cannot receive deposits' USING ERRCODE = 'WA006';
    END IF;
    IF limits @> ARRAY['KYC_REQUIRED'] THEN
        RAISE EXCEPTION 'deposit addresses require KYC approval' USING ERRCODE = 'WA012';
    END IF;
    IF network.address_status = 'ACTIVE' AND network.address IS NOT NULL THEN
        RETURN QUERY SELECT 'ACTIVE'::text, network.address, network.memo, false;
        RETURN;
    END IF;
    IF network.address_status = 'PENDING' THEN
        RETURN QUERY SELECT 'PENDING'::text, NULL::text, NULL::text, false;
        RETURN;
    END IF;
    IF NOT network.deposits_enabled OR NOT (SELECT c.activated @> ARRAY['CUSTOMER_CREDIT']
                                             FROM ledger.asset_controls(wallet.asset_id) c) THEN
        RAISE EXCEPTION 'deposits are not enabled for this asset and network' USING ERRCODE = 'WA007';
    END IF;
    SELECT * INTO provider FROM wallets.select_address_provider(network.asset_network_id);
    IF NOT FOUND THEN
        RAISE EXCEPTION 'no approved provider can provision this address' USING ERRCODE = 'WA008';
    END IF;
    INSERT INTO wallets.wallet_provisioning_requests(wallet_profile_id, provider_id, request_reference, status,
        asset_network_id)
    VALUES (wallet.id, provider.provider_id, 'deypay-addr-' || gen_random_uuid()::text, 'PENDING',
        network.asset_network_id)
    RETURNING id INTO opened;
    PERFORM wallets.audit('CUSTOMER', p_customer, 'DEPOSIT_ADDRESS_REQUESTED', wallet.id, NULL,
        jsonb_build_object('network', p_network, 'provider', provider.provider_code, 'request_id', opened));
    RETURN QUERY SELECT 'PENDING'::text, NULL::text, NULL::text, true;
END;
$$;

---------------------------------------------------------------------------
-- Provider-integration interface (deypay_wallet_provisioner)
---------------------------------------------------------------------------
-- Requests needing provider work, oldest first. SUBMITTED and UNKNOWN requests
-- need a lookup with their original provider, whatever routing says now.
CREATE FUNCTION wallets.provisioning_work(p_limit integer)
RETURNS TABLE(request_id uuid, request_reference text, status text, provider_code text, wallet_id uuid,
    asset text, network_code text, chain_family text, token_contract text, external_reference text,
    attempt_count integer)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
    SELECT r.id, r.request_reference::text, r.status, p.provider_code, w.id, w.asset_symbol, n.code, n.chain_family,
           an.token_contract::text, r.external_reference, r.attempt_count
    FROM wallets.wallet_provisioning_requests r
    JOIN integrations.providers p ON p.id = r.provider_id
    JOIN wallets.wallet_profiles w ON w.id = r.wallet_profile_id
    JOIN wallets.asset_networks an ON an.id = r.asset_network_id
    JOIN wallets.networks n ON n.id = an.network_id
    WHERE r.status IN ('PENDING','SUBMITTED','UNKNOWN')
    ORDER BY r.created_at, r.id
    LIMIT least(greatest(coalesce(p_limit, 1), 1), 100);
$$;

-- Marks a request SUBMITTED before the provider is called (returns SUBMIT), or
-- tells the caller to look up an earlier attempt (LOOKUP). A pending request
-- whose provider is no longer selectable fails without being submitted.
CREATE FUNCTION wallets.begin_provisioning(p_request uuid) RETURNS text
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    request wallets.wallet_provisioning_requests%ROWTYPE;
BEGIN
    SELECT * INTO request FROM wallets.wallet_provisioning_requests r WHERE r.id = p_request FOR UPDATE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'unknown provisioning request' USING ERRCODE = 'WA004';
    END IF;
    IF request.status IN ('SUBMITTED','UNKNOWN') THEN
        RETURN 'LOOKUP';
    END IF;
    IF request.status <> 'PENDING' THEN
        RETURN 'DONE';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM wallets.select_address_provider(request.asset_network_id) s
                   WHERE s.provider_id = request.provider_id) THEN
        UPDATE wallets.wallet_provisioning_requests r SET status = 'FAILED', last_error_code = 'PROVIDER_NOT_SELECTABLE',
            completed_at = now() WHERE r.id = p_request;
        RETURN 'DONE';
    END IF;
    UPDATE wallets.wallet_provisioning_requests r SET status = 'SUBMITTED', attempt_count = r.attempt_count + 1,
        submitted_at = now(), last_error_code = NULL
    WHERE r.id = p_request;
    RETURN 'SUBMIT';
END;
$$;

-- Records the normalized provider result. PROVISIONED stores the address as
-- PENDING_VERIFICATION; it becomes visible only after verify_deposit_address.
-- Returns the address record for PROVISIONED, else NULL. Repeating the stored
-- final result is a no-op; a contradicting one is refused.
CREATE FUNCTION wallets.record_provisioning_result(p_request uuid, p_result text, p_external_reference text,
    p_address text, p_memo text, p_error_code text) RETURNS uuid
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    request wallets.wallet_provisioning_requests%ROWTYPE;
    mapping record;
    recorded uuid;
BEGIN
    IF p_result IS NULL OR p_result NOT IN ('ACCEPTED','PROVISIONED','FAILED','UNKNOWN','NOT_RECEIVED','UNSUPPORTED') THEN
        RAISE EXCEPTION 'unknown provisioning result' USING ERRCODE = 'WA002';
    END IF;
    SELECT * INTO request FROM wallets.wallet_provisioning_requests r WHERE r.id = p_request FOR UPDATE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'unknown provisioning request' USING ERRCODE = 'WA004';
    END IF;
    IF request.status IN ('PROVISIONED','FAILED','UNSUPPORTED') THEN
        IF request.status = (CASE p_result WHEN 'PROVISIONED' THEN 'PROVISIONED' WHEN 'FAILED' THEN 'FAILED'
                                           WHEN 'UNSUPPORTED' THEN 'UNSUPPORTED' END) THEN
            SELECT a.id INTO recorded FROM wallets.wallet_addresses a WHERE a.provisioning_request_id = p_request;
            RETURN recorded;
        END IF;
        RAISE EXCEPTION 'provisioning request is already %', lower(request.status) USING ERRCODE = 'WA009';
    END IF;
    IF request.status = 'PENDING' THEN
        RAISE EXCEPTION 'begin the provisioning attempt before recording its result' USING ERRCODE = 'WA009';
    END IF;
    IF p_result = 'NOT_RECEIVED' AND request.status <> 'SUBMITTED' THEN
        RAISE EXCEPTION 'an attempt with an unknown outcome cannot become not received' USING ERRCODE = 'WA009';
    END IF;
    IF p_result = 'PROVISIONED' THEN
        SELECT an.asset_id, an.network_id INTO mapping FROM wallets.asset_networks an WHERE an.id = request.asset_network_id;
        INSERT INTO wallets.wallet_addresses(wallet_profile_id, asset_id, asset_network_id, network_id, provider_id,
            address, memo_tag, external_wallet_reference, status, provisioning_request_id)
        VALUES (request.wallet_profile_id, mapping.asset_id, request.asset_network_id, mapping.network_id,
            request.provider_id, p_address, p_memo, coalesce(p_external_reference, request.external_reference),
            'PENDING_VERIFICATION', p_request)
        RETURNING id INTO recorded;
    END IF;
    UPDATE wallets.wallet_provisioning_requests r SET
        status = CASE p_result WHEN 'ACCEPTED' THEN 'SUBMITTED' WHEN 'NOT_RECEIVED' THEN 'PENDING' ELSE p_result END,
        external_reference = coalesce(r.external_reference, p_external_reference),
        last_error_code = CASE WHEN p_result IN ('FAILED','UNKNOWN','NOT_RECEIVED','UNSUPPORTED')
                               THEN coalesce(p_error_code, p_result) END,
        submitted_at = CASE WHEN p_result = 'NOT_RECEIVED' THEN NULL ELSE r.submitted_at END,
        completed_at = CASE WHEN p_result IN ('PROVISIONED','FAILED','UNSUPPORTED') THEN now() END
    WHERE r.id = p_request;
    RETURN recorded;
END;
$$;

-- Verification against authenticated provider data that the address is the
-- one assigned to this wallet. Only a verified address becomes ACTIVE.
CREATE FUNCTION wallets.verify_deposit_address(p_address uuid, p_evidence_reference text, p_evidence_sha256 bytea)
RETURNS text
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    current_status text;
BEGIN
    IF p_evidence_reference IS NULL OR NOT wallets.printable(p_evidence_reference, 512)
       OR p_evidence_sha256 IS NULL OR length(p_evidence_sha256) <> 32 THEN
        RAISE EXCEPTION 'verification needs an evidence reference and SHA-256 digest' USING ERRCODE = 'WA002';
    END IF;
    SELECT a.status INTO current_status FROM wallets.wallet_addresses a WHERE a.id = p_address FOR UPDATE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'unknown deposit address' USING ERRCODE = 'WA004';
    END IF;
    IF current_status = 'ACTIVE' THEN
        RETURN current_status;
    END IF;
    IF current_status <> 'PENDING_VERIFICATION' THEN
        RAISE EXCEPTION 'deposit address is %', lower(current_status) USING ERRCODE = 'WA009';
    END IF;
    UPDATE wallets.wallet_addresses a SET status = 'ACTIVE', verification_status = 'VERIFIED', verified_at = now(),
        activated_at = now(), evidence_reference = p_evidence_reference, evidence_sha256 = p_evidence_sha256
    WHERE a.id = p_address;
    RETURN 'ACTIVE';
END;
$$;

-- An address that failed verification is never shown and never reused.
CREATE FUNCTION wallets.reject_deposit_address(p_address uuid, p_reason text) RETURNS text
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
BEGIN
    IF p_reason IS NULL OR NOT wallets.printable(p_reason, 500) THEN
        RAISE EXCEPTION 'a reason is required' USING ERRCODE = 'WA002';
    END IF;
    UPDATE wallets.wallet_addresses a SET status = 'REJECTED', verification_status = 'REJECTED',
        retirement_reason = p_reason
    WHERE a.id = p_address AND a.status = 'PENDING_VERIFICATION';
    IF NOT FOUND THEN
        RAISE EXCEPTION 'only an unverified address can be rejected' USING ERRCODE = 'WA009';
    END IF;
    RETURN 'REJECTED';
END;
$$;

-- Retires an address (provider change or provider notice). The record stays,
-- so deposits that still arrive there remain attributable to the customer.
CREATE FUNCTION wallets.retire_deposit_address(p_address uuid, p_reason text) RETURNS text
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
BEGIN
    IF p_reason IS NULL OR NOT wallets.printable(p_reason, 500) THEN
        RAISE EXCEPTION 'a reason is required' USING ERRCODE = 'WA002';
    END IF;
    UPDATE wallets.wallet_addresses a SET status = 'RETIRED', retired_at = now(), retirement_reason = p_reason
    WHERE a.id = p_address AND a.status IN ('PENDING_VERIFICATION','ACTIVE','SUSPENDED');
    IF NOT FOUND THEN
        IF EXISTS (SELECT 1 FROM wallets.wallet_addresses a WHERE a.id = p_address AND a.status = 'RETIRED') THEN
            RETURN 'RETIRED';
        END IF;
        RAISE EXCEPTION 'unknown or rejected deposit address' USING ERRCODE = 'WA009';
    END IF;
    RETURN 'RETIRED';
END;
$$;

---------------------------------------------------------------------------
-- Administrative interface (deypay_ledger_controller)
---------------------------------------------------------------------------
-- IMP-P6-005: restrict, suspend, reactivate or close a wallet for the
-- authenticated operator holding WALLET_STATUS_CONTROL. Closing requires a
-- zero ledger balance; the ledger accounts and history remain.
CREATE FUNCTION wallets.set_wallet_status(p_wallet uuid, p_status text, p_reason text, p_actor uuid) RETURNS text
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp SET lock_timeout = '10s' AS $$
DECLARE
    acting uuid := ledger.authenticated_operator(p_actor, 'WALLET_STATUS_CONTROL');
    wallet wallets.wallet_profiles%ROWTYPE;
    total text;
BEGIN
    IF p_status IS NULL OR p_status NOT IN ('ACTIVE','RESTRICTED','SUSPENDED','CLOSED')
       OR p_reason IS NULL OR NOT wallets.printable(p_reason, 500) THEN
        RAISE EXCEPTION 'a valid status and reason are required' USING ERRCODE = 'WA002';
    END IF;
    SELECT * INTO wallet FROM wallets.wallet_profiles w WHERE w.id = p_wallet FOR UPDATE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'unknown wallet' USING ERRCODE = 'WA004';
    END IF;
    IF wallet.status = p_status THEN
        RETURN p_status;
    END IF;
    IF wallet.status = 'PROVISIONING' THEN
        RAISE EXCEPTION 'a wallet still being provisioned cannot change status' USING ERRCODE = 'WA003';
    END IF;
    IF p_status = 'CLOSED' THEN
        SELECT b.total_minor INTO total FROM ledger.customer_balance(wallet.user_id, wallet.asset_id) b;
        IF total IS DISTINCT FROM '0' THEN
            RAISE EXCEPTION 'only an empty wallet can close' USING ERRCODE = 'WA011';
        END IF;
    END IF;
    UPDATE wallets.wallet_profiles w SET status = p_status, status_reason = p_reason, status_actor_type = 'ADMIN',
        status_actor_id = acting
    WHERE w.id = p_wallet;
    PERFORM wallets.audit('ADMIN', acting, 'WALLET_STATUS_CHANGED', p_wallet, p_reason,
        jsonb_build_object('previous_status', wallet.status, 'new_status', p_status));
    RETURN p_status;
END;
$$;

-- Every wallet routine pins its search path, including the Phase 2 trigger.
ALTER FUNCTION wallets.preserve_asset_unit() SET search_path = pg_catalog, pg_temp;

-- PostgreSQL grants EXECUTE to PUBLIC by default; wallet functions are granted
-- explicitly by infrastructure/postgres/wallet-grants.psql.
REVOKE ALL ON ALL FUNCTIONS IN SCHEMA wallets FROM PUBLIC;
