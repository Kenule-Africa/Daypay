DO $$
DECLARE
    customer uuid;
    other_customer uuid;
    device uuid;
    case_id uuid;
    session_key uuid;
    contact_challenge uuid;
BEGIN
    INSERT INTO identity.users(email,status) VALUES ('owner@example.test','REGISTERED') RETURNING id INTO customer;
    INSERT INTO identity.users(email,status) VALUES ('other@example.test','REGISTERED') RETURNING id INTO other_customer;
    BEGIN
        INSERT INTO identity.users(email,status) VALUES ('owner@example.test','REGISTERED');
        RAISE EXCEPTION 'duplicate email accepted';
    EXCEPTION WHEN unique_violation THEN NULL; END;
    INSERT INTO customers.usernames(user_id,username) VALUES (customer,'Handle');
    BEGIN
        INSERT INTO customers.usernames(user_id,username) VALUES (other_customer,'@handle');
        RAISE EXCEPTION 'duplicate active handle accepted';
    EXCEPTION WHEN unique_violation THEN NULL; END;
    INSERT INTO customers.username_keys(normalized_username,reserved) VALUES ('reserved_fixture',true);
    BEGIN
        INSERT INTO customers.usernames(user_id,username) VALUES (other_customer,'@reserved_fixture');
        RAISE EXCEPTION 'reserved handle accepted';
    EXCEPTION WHEN foreign_key_violation THEN NULL; END;
    INSERT INTO identity.devices(user_id,device_name) VALUES (customer,'test') RETURNING id INTO device;
    BEGIN
        INSERT INTO identity.sessions(user_id,device_id,token_hash,expires_at,absolute_expires_at)
            VALUES (other_customer,device,'synthetic digest',now()+interval '1 hour',now()+interval '1 day');
        RAISE EXCEPTION 'cross-customer device accepted';
    EXCEPTION WHEN foreign_key_violation THEN NULL; END;
    BEGIN
        INSERT INTO identity.password_credentials(user_id,password_hash) VALUES (customer,'plaintext');
        RAISE EXCEPTION 'plaintext credential accepted';
    EXCEPTION WHEN check_violation THEN NULL; END;
    INSERT INTO identity.sessions(user_id,device_id,token_hash,expires_at,absolute_expires_at)
        VALUES(customer,device,'owner-test-digest',now()+interval '1 hour',now()+interval '1 day') RETURNING id INTO session_key;
    BEGIN
        UPDATE identity.sessions SET absolute_expires_at=absolute_expires_at+interval '1 day' WHERE id=session_key;
        RAISE EXCEPTION 'refresh extended the absolute session lifetime';
    EXCEPTION WHEN check_violation THEN NULL; END;
    INSERT INTO identity.verification_challenges(user_id,challenge_type,code_hash,expires_at,consumed_at)
        VALUES(customer,'CONTACT','synthetic',now()+interval '1 hour',now()) RETURNING id INTO contact_challenge;
    BEGIN
        INSERT INTO identity.transaction_proofs(user_id,challenge_id,proof_hash,expires_at,challenge_completed_at)
            VALUES(customer,contact_challenge,'fake-proof',now()+interval '1 minute',now());
        RAISE EXCEPTION 'contact verification created a financial proof';
    EXCEPTION WHEN foreign_key_violation THEN NULL; END;
    BEGIN
        UPDATE identity.verification_challenges SET operation_type='TRANSFER',session_id=session_key,
            binding_digest='changed',binding_version=1 WHERE id=contact_challenge;
        RAISE EXCEPTION 'completed challenge rebound';
    EXCEPTION WHEN check_violation THEN NULL; END;
    INSERT INTO compliance.kyc_cases(user_id,status) VALUES (customer,'DRAFT') RETURNING id INTO case_id;
    INSERT INTO compliance.kyc_documents(kyc_case_id,document_type,storage_reference,verification_status)
        VALUES (case_id,'synthetic','fixture/key','PENDING');
    BEGIN
        UPDATE compliance.kyc_cases SET status='UNREVIEWED_APPROVAL' WHERE id=case_id;
        RAISE EXCEPTION 'invalid KYC state accepted';
    EXCEPTION WHEN check_violation THEN NULL; END;
END;
$$;
