DO $$
DECLARE
    provider uuid; owner_id uuid; tx uuid; op uuid; fact uuid;
BEGIN
    INSERT INTO identity.users(email,status) VALUES('events@example.test','REGISTERED') RETURNING id INTO owner_id;
    INSERT INTO payments.transactions(user_id,public_reference,transaction_type,status)
        VALUES(owner_id,'fixture-event','BANK_PAYMENT','CREATED') RETURNING id INTO tx;
    INSERT INTO integrations.providers(provider_code,provider_name,provider_type)
        VALUES('FIXTURE','fixture','TEST') RETURNING id INTO provider;
    INSERT INTO integrations.provider_operations(provider_id,provider_account_scope,is_financial,transaction_id,operation_type,leg,
        authorized_attempt,idempotency_reference,status) VALUES(provider,'account',true,tx,'PAYOUT','payout',1,'submission','CREATED')
        RETURNING id INTO op;
    BEGIN
        INSERT INTO integrations.provider_operations(provider_id,provider_account_scope,is_financial,transaction_id,operation_type,leg,
            authorized_attempt,idempotency_reference,status) VALUES(provider,'other',true,tx,'PAYOUT','payout',1,'second','CREATED');
        RAISE EXCEPTION 'same authorized payout submitted twice';
    EXCEPTION WHEN unique_violation THEN NULL; END;
    BEGIN
        UPDATE integrations.provider_operations SET idempotency_reference='replacement' WHERE id=op;
        RAISE EXCEPTION 'provider submission identity changed';
    EXCEPTION WHEN object_not_in_prerequisite_state THEN NULL; END;
    INSERT INTO integrations.economic_events(provider_id,provider_account_scope,economic_reference,leg,provider_operation_id,verified_at)
        VALUES(provider,'account','external settlement','payout',op,now()) RETURNING id INTO fact;
    INSERT INTO integrations.event_observations(economic_event_id,source,evidence_reference)
        VALUES(fact,'POLL','ref-a'),(fact,'STATEMENT','ref-b');
    BEGIN
        INSERT INTO integrations.economic_events(provider_id,provider_account_scope,economic_reference,leg,provider_operation_id,verified_at)
            VALUES(provider,'account','external settlement','payout',op,now());
        RAISE EXCEPTION 'duplicate settlement accepted';
    EXCEPTION WHEN unique_violation THEN NULL; END;
    INSERT INTO integrations.webhook_events(provider_id,provider_account_scope,external_event_id,event_type,payload_hash,signature_verified,processing_status)
        VALUES(provider,'account','event','fixture','digest',false,'REJECTED'),
              (provider,'account','event','fixture','digest',true,'PENDING'),
              (provider,'other','event','fixture','digest',true,'PENDING');
    BEGIN
        INSERT INTO integrations.webhook_events(provider_id,provider_account_scope,external_event_id,event_type,payload_hash,signature_verified,processing_status)
            VALUES(provider,'account','event','fixture','other digest',true,'PENDING');
        RAISE EXCEPTION 'verified callback replay accepted';
    EXCEPTION WHEN unique_violation THEN NULL; END;
    BEGIN
        INSERT INTO integrations.webhook_events(provider_id,provider_account_scope,event_type,payload_hash,signature_verified,processing_status)
            VALUES(provider,'account','fixture','digest',true,'PENDING');
        RAISE EXCEPTION 'payload hash alone made work eligible';
    EXCEPTION WHEN check_violation THEN NULL; END;
    -- A failed second write must roll the outbox insert back as well.
    BEGIN
        INSERT INTO integrations.outbox_events(aggregate_type,aggregate_id,event_type,payload) VALUES('fixture',tx,'test','{}');
        INSERT INTO integrations.jobs(job_type,execution_key,payload,status,max_attempts,available_at)
            VALUES('test','key','{}','RUNNING',3,now());
        RAISE EXCEPTION 'unleased running job accepted';
    EXCEPTION WHEN check_violation THEN NULL; END;
    IF EXISTS (SELECT FROM integrations.outbox_events WHERE aggregate_id=tx) THEN RAISE EXCEPTION 'partial outbox commit'; END IF;
    INSERT INTO audit.audit_events(actor_type,action,resource_type) VALUES('SYSTEM','TEST','fixture');
    BEGIN
        TRUNCATE audit.audit_events;
        RAISE EXCEPTION 'audit history truncated';
    EXCEPTION WHEN object_not_in_prerequisite_state THEN NULL; END;
    BEGIN
        UPDATE audit.audit_events SET action='REWRITE';
        RAISE EXCEPTION 'audit history edited';
    EXCEPTION WHEN object_not_in_prerequisite_state THEN NULL; END;
    BEGIN
        INSERT INTO audit.approval_requests(action_type,target_reference,requested_by,approved_by,status,reason,decided_at)
            VALUES('test','resource',owner_id,owner_id,'APPROVED','fixture',now());
        RAISE EXCEPTION 'self-approval accepted';
    EXCEPTION WHEN check_violation THEN NULL; END;
END;
$$;
