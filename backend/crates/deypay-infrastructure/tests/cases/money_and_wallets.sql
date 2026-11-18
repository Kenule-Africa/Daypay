DO $$
DECLARE
    customer uuid;
    usd uuid;
    ngn uuid;
    tx uuid;
    stored numeric;
BEGIN
    INSERT INTO identity.users(email,status) VALUES ('money@example.test','REGISTERED') RETURNING id INTO customer;
    INSERT INTO wallets.assets(symbol,name,asset_type,accounting_precision) VALUES ('USDC','test USDC','STABLECOIN',6) RETURNING id INTO usd;
    INSERT INTO wallets.assets(symbol,name,asset_type,accounting_precision) VALUES ('NGN','test NGN','FIAT',2) RETURNING id INTO ngn;
    INSERT INTO wallets.wallet_profiles(user_id,asset_id,asset_symbol,status,provisioning_status)
        VALUES(customer,usd,'USDC','PROVISIONING','PENDING');
    BEGIN
        INSERT INTO wallets.wallet_profiles(user_id,asset_id,asset_symbol,status,provisioning_status)
            VALUES(customer,ngn,'NGN','PROVISIONING','PENDING');
        RAISE EXCEPTION 'NGN customer wallet accepted';
    EXCEPTION WHEN check_violation THEN NULL; END;
    BEGIN
        INSERT INTO wallets.wallet_profiles(user_id,asset_id,asset_symbol,status,provisioning_status)
            VALUES(customer,ngn,'USDT','PROVISIONING','PENDING');
        RAISE EXCEPTION 'asset symbol substitution accepted';
    EXCEPTION WHEN foreign_key_violation THEN NULL; END;
    INSERT INTO payments.transactions(user_id,public_reference,transaction_type,status,source_asset_id,source_amount_minor)
        VALUES(customer,'fixture-T','INTERNAL_TRANSFER','CREATED',usd,99999999999999999999999999999999999999)
        RETURNING id,source_amount_minor INTO tx,stored;
    IF stored <> 99999999999999999999999999999999999999 THEN RAISE EXCEPTION 'precision lost'; END IF;
    BEGIN
        UPDATE payments.transactions SET source_amount_minor=0.5 WHERE id=tx;
        RAISE EXCEPTION 'fractional minor units rounded';
    EXCEPTION WHEN check_violation THEN NULL; END;
    BEGIN
        UPDATE payments.transactions SET fee_amount_minor=1 WHERE id=tx;
        RAISE EXCEPTION 'assetless fee accepted';
    EXCEPTION WHEN check_violation THEN NULL; END;
    BEGIN
        UPDATE payments.transactions SET source_amount_minor=1e38 WHERE id=tx;
        RAISE EXCEPTION 'oversize amount accepted';
    EXCEPTION WHEN check_violation THEN NULL; END;
    BEGIN
        UPDATE payments.transactions SET source_amount_minor='NaN' WHERE id=tx;
        RAISE EXCEPTION 'nonfinite amount accepted';
    EXCEPTION WHEN check_violation THEN NULL; END;
    BEGIN
        UPDATE wallets.assets SET accounting_precision=2 WHERE id=usd;
        RAISE EXCEPTION 'unit of stored amount changed';
    EXCEPTION WHEN check_violation THEN NULL; END;
    INSERT INTO payments.transaction_status_history(transaction_id,new_status,actor_type) VALUES(tx,'CREATED','SYSTEM');
    BEGIN
        DELETE FROM payments.transaction_status_history WHERE transaction_id=tx;
        RAISE EXCEPTION 'status evidence deleted';
    EXCEPTION WHEN object_not_in_prerequisite_state THEN NULL; END;
    INSERT INTO payments.idempotency_records(user_id,operation_type,idempotency_key,request_hash,status,expires_at,transaction_id)
        VALUES(customer,'TRANSFER','fixture-key','digest','ACCEPTED',now()+interval '1 day',tx);
    BEGIN
        INSERT INTO payments.idempotency_records(user_id,operation_type,idempotency_key,request_hash,status,expires_at)
            VALUES(customer,'TRANSFER','fixture-key','other','PENDING',now()+interval '1 day');
        RAISE EXCEPTION 'duplicate financial request accepted';
    EXCEPTION WHEN unique_violation THEN NULL; END;
END;
$$;
