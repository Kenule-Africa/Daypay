-- Customer KYC case writes (IMP-P4-005 prerequisite, approved by the repository owner).
-- The runtime role receives EXECUTE on these two fixed operations only. It holds no
-- INSERT or UPDATE on compliance.kyc_cases, so it cannot write a provider or reviewer
-- status such as APPROVED. Provider and reviewer transitions need their own reviewed
-- write paths.
--
-- Each successful write appends its audit event in the same transaction, as the
-- registration and email-verification operations do, so a case change cannot commit
-- without its evidence. Wider KYC audit (reviewer, provider and access events,
-- retention) remains IMP-P4-008.

-- Opens the customer's first case in DRAFT and returns it, or returns no row when the
-- customer already has a case in any status. Second cases, re-verification and
-- resubmission after rejection are undecided policy, so they are refused here.
CREATE FUNCTION compliance.open_first_kyc_case(p_user_id uuid)
RETURNS TABLE (case_id uuid, owner_id uuid, case_status text)
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
BEGIN
    -- Under a transaction-wide snapshot the existence check below could miss a case
    -- committed while this call waited for the lock.
    IF current_setting('transaction_isolation') <> 'read committed' THEN
        RAISE EXCEPTION USING ERRCODE='0A000', MESSAGE='KYC case creation requires READ COMMITTED';
    END IF;
    -- Serializes first-case requests for one customer until commit or rollback. A hash
    -- collision only serializes two customers' requests; it never admits a duplicate.
    PERFORM pg_advisory_xact_lock(hashtextextended('compliance.kyc_cases.user_id:' || p_user_id::text, 0));
    IF EXISTS (SELECT FROM compliance.kyc_cases k WHERE k.user_id = p_user_id) THEN
        RETURN;
    END IF;
    INSERT INTO compliance.kyc_cases AS k (user_id, status) VALUES (p_user_id, 'DRAFT')
        RETURNING k.id, k.user_id, k.status INTO case_id, owner_id, case_status;
    INSERT INTO audit.audit_events(actor_type, actor_id, action, resource_type, resource_id)
        VALUES ('CUSTOMER', p_user_id, 'KYC_CASE_OPENED', 'KYC_CASE', case_id);
    RETURN NEXT;
END;
$$;

-- Moves the customer's own case from p_from to SUBMITTED only while it still has
-- p_from, and reports whether it did. Only customer submission is possible here.
-- submitted_at records the first submission (DRAFT); a resubmission after an
-- information request keeps it, which preserves reviewed_at >= submitted_at. Every
-- submission, first or repeated, has its own audit event.
CREATE FUNCTION compliance.submit_kyc_case(p_user_id uuid, p_case_id uuid, p_from text)
RETURNS boolean
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE
    submitted boolean;
BEGIN
    IF p_from IS NULL OR p_from NOT IN ('DRAFT','INFORMATION_REQUESTED') THEN
        RAISE EXCEPTION USING ERRCODE='23514', MESSAGE='Not a customer KYC submission';
    END IF;
    UPDATE compliance.kyc_cases k SET status='SUBMITTED',
        submitted_at=CASE WHEN p_from = 'DRAFT' THEN coalesce(k.submitted_at, now()) ELSE k.submitted_at END,
        updated_at=greatest(k.updated_at, now())
        WHERE k.id = p_case_id AND k.user_id = p_user_id AND k.status = p_from;
    submitted := FOUND;
    IF submitted THEN
        INSERT INTO audit.audit_events(actor_type, actor_id, action, resource_type, resource_id, metadata)
            VALUES ('CUSTOMER', p_user_id, 'KYC_CASE_SUBMITTED', 'KYC_CASE', p_case_id,
                    jsonb_build_object('from_status', p_from, 'to_status', 'SUBMITTED'));
    END IF;
    RETURN submitted;
END;
$$;

REVOKE ALL ON FUNCTION compliance.open_first_kyc_case(uuid),
    compliance.submit_kyc_case(uuid,uuid,text) FROM PUBLIC;
