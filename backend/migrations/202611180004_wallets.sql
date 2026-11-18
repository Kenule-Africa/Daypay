CREATE TABLE wallets.assets (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), symbol text NOT NULL UNIQUE CHECK (symbol ~ '^[A-Z0-9]+$'),
    name text NOT NULL, asset_type text NOT NULL, accounting_precision smallint NOT NULL CHECK (accounting_precision >= 0),
    enabled boolean NOT NULL DEFAULT false, created_at timestamptz NOT NULL DEFAULT now(), UNIQUE (id,symbol)
);
-- Once amounts exist, silently changing their unit would change their meaning.
CREATE FUNCTION wallets.preserve_asset_unit() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.accounting_precision <> OLD.accounting_precision OR NEW.symbol <> OLD.symbol THEN
        RAISE EXCEPTION 'asset unit is immutable' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER preserve_asset_unit BEFORE UPDATE ON wallets.assets
    FOR EACH ROW EXECUTE FUNCTION wallets.preserve_asset_unit();
CREATE TABLE wallets.networks (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), code text NOT NULL UNIQUE, name text NOT NULL,
    chain_family text NOT NULL, chain_identifier varchar(512), enabled boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE wallets.asset_networks (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), asset_id uuid NOT NULL REFERENCES wallets.assets,
    network_id uuid NOT NULL REFERENCES wallets.networks, token_contract varchar(512),
    token_decimals smallint NOT NULL CHECK (token_decimals >= 0), deposit_enabled boolean NOT NULL DEFAULT false,
    withdrawal_enabled boolean NOT NULL DEFAULT false, created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE NULLS NOT DISTINCT (asset_id,network_id,token_contract), UNIQUE (id,asset_id)
);
CREATE TABLE wallets.wallet_profiles (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), user_id uuid NOT NULL REFERENCES identity.users,
    asset_id uuid NOT NULL, asset_symbol text NOT NULL CHECK (asset_symbol IN ('USDC','USDT')),
    status text NOT NULL CHECK (status IN ('ACTIVE','RESTRICTED','PROVISIONING','SUSPENDED','CLOSED')),
    provisioning_status text NOT NULL CHECK (btrim(provisioning_status) <> ''),
    created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now(),
    FOREIGN KEY (asset_id,asset_symbol) REFERENCES wallets.assets(id,symbol),
    UNIQUE (user_id,asset_id), UNIQUE (id,asset_id)
);
CREATE TABLE wallets.wallet_addresses (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), wallet_profile_id uuid NOT NULL, asset_id uuid NOT NULL,
    asset_network_id uuid NOT NULL, provider_id uuid NOT NULL REFERENCES integrations.providers,
    address text NOT NULL CHECK (address <> ''), memo_tag text, external_wallet_reference varchar(512),
    status text NOT NULL, created_at timestamptz NOT NULL DEFAULT now(), retired_at timestamptz,
    FOREIGN KEY (wallet_profile_id,asset_id) REFERENCES wallets.wallet_profiles(id,asset_id),
    FOREIGN KEY (asset_network_id,asset_id) REFERENCES wallets.asset_networks(id,asset_id)
);
CREATE INDEX wallet_addresses_lookup ON wallets.wallet_addresses(asset_network_id,address,memo_tag);
CREATE INDEX wallet_addresses_provider_ref ON wallets.wallet_addresses(provider_id,external_wallet_reference)
    WHERE external_wallet_reference IS NOT NULL;
CREATE TABLE wallets.wallet_provisioning_requests (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), wallet_profile_id uuid NOT NULL REFERENCES wallets.wallet_profiles,
    provider_id uuid NOT NULL REFERENCES integrations.providers, request_reference varchar(512) NOT NULL,
    status text NOT NULL, attempt_count integer NOT NULL DEFAULT 0 CHECK (attempt_count >= 0), last_error_code text,
    created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE(provider_id,request_reference)
);
