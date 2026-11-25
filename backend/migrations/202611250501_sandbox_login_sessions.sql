-- Customer login is opt-in for sandbox registrations. Production approvals are separate.
CREATE FUNCTION identity.sandbox_login_credential(address text)
RETURNS TABLE(customer_id uuid, password_hash text)
LANGUAGE sql STABLE STRICT SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
    SELECT u.id, c.password_hash FROM identity.users u
    JOIN identity.password_credentials c ON c.user_id=u.id
    WHERE u.email=lower(btrim(address)) AND u.status='CONTACT_VERIFIED' AND u.email_verified_at IS NOT NULL
      AND EXISTS (SELECT FROM customers.email_verification_eligibility e WHERE e.user_id=u.id AND e.scope='sandbox')
$$;

-- Lock the authoritative user before a session row. A concurrent status change
-- either completes first and denies access, or waits for this operation to commit.
CREATE FUNCTION identity.lock_sandbox_login_customer(customer uuid, address text)
RETURNS boolean LANGUAGE plpgsql SET search_path=pg_catalog,pg_temp AS $$
BEGIN
    PERFORM FROM identity.users u WHERE u.id=customer AND u.status='CONTACT_VERIFIED'
        AND u.email_verified_at IS NOT NULL AND (address IS NULL OR u.email=lower(btrim(address)))
        AND EXISTS (SELECT FROM customers.email_verification_eligibility e WHERE e.user_id=u.id AND e.scope='sandbox')
        FOR SHARE OF u;
    RETURN FOUND;
END $$;

CREATE FUNCTION identity.record_login_rejection(customer uuid) RETURNS void
LANGUAGE sql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
    INSERT INTO audit.audit_events(actor_type,action,resource_type,resource_id)
    VALUES ('SYSTEM','LOGIN_REJECTED','CUSTOMER',customer)
$$;

CREATE FUNCTION identity.issue_sandbox_session(customer uuid, address text, checked_phc text,
    new_digest text, previous_digest text, idle_seconds integer, absolute_seconds integer)
RETURNS boolean LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE stamp timestamptz; session_id uuid;
BEGIN
    IF new_digest IS NULL OR new_digest !~ '^[0-9a-f]{64}$' OR new_digest=previous_digest
       OR (previous_digest IS NOT NULL AND previous_digest !~ '^[0-9a-f]{64}$')
       OR idle_seconds IS NULL OR idle_seconds<1 OR absolute_seconds IS NULL OR absolute_seconds<idle_seconds
       OR address IS NULL THEN
        RAISE EXCEPTION 'Invalid session request' USING ERRCODE='22023';
    END IF;
    IF NOT identity.lock_sandbox_login_customer(customer,address) THEN
        PERFORM identity.record_login_rejection(customer);
        RETURN false;
    END IF;
    PERFORM FROM identity.password_credentials c WHERE c.user_id=customer AND c.password_hash=checked_phc FOR SHARE;
    IF NOT FOUND THEN
        PERFORM identity.record_login_rejection(customer);
        RETURN false;
    END IF;
    -- Even on a shared browser the supplied previous session cannot survive a new login.
    UPDATE identity.sessions SET revoked_at=greatest(created_at,clock_timestamp())
    WHERE token_hash=previous_digest AND revoked_at IS NULL
      AND EXISTS (SELECT FROM customers.email_verification_eligibility e WHERE e.user_id=identity.sessions.user_id AND e.scope='sandbox');
    stamp := clock_timestamp();
    INSERT INTO identity.sessions(user_id,token_hash,created_at,last_activity_at,expires_at,absolute_expires_at)
    VALUES (customer,new_digest,stamp,stamp,stamp+make_interval(secs=>idle_seconds),stamp+make_interval(secs=>absolute_seconds))
    RETURNING id INTO session_id;
    INSERT INTO audit.audit_events(actor_type,actor_id,action,resource_type,resource_id)
    VALUES ('CUSTOMER',customer,'LOGIN_SUCCEEDED','SESSION',session_id);
    RETURN true;
END $$;

CREATE FUNCTION identity.validate_sandbox_session(current_digest text, replacement_digest text, idle_seconds integer)
RETURNS uuid LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE customer uuid; selected identity.sessions%ROWTYPE; stamp timestamptz;
BEGIN
    IF current_digest IS NULL OR current_digest !~ '^[0-9a-f]{64}$' OR idle_seconds IS NULL OR idle_seconds<1
       OR (replacement_digest IS NOT NULL AND (replacement_digest=current_digest OR replacement_digest !~ '^[0-9a-f]{64}$')) THEN
        RETURN NULL;
    END IF;
    SELECT s.user_id INTO customer FROM identity.sessions s WHERE s.token_hash=current_digest;
    IF NOT FOUND OR NOT identity.lock_sandbox_login_customer(customer,NULL) THEN RETURN NULL; END IF;
    SELECT s.* INTO selected FROM identity.sessions s WHERE s.token_hash=current_digest AND s.user_id=customer FOR UPDATE;
    IF NOT FOUND THEN RETURN NULL; END IF;
    -- Check wall time *after* obtaining the lock. A blocked request cannot use its earlier arrival time.
    stamp := clock_timestamp();
    IF selected.revoked_at IS NOT NULL OR selected.created_at>stamp OR selected.expires_at<=stamp
       OR selected.absolute_expires_at<=stamp THEN RETURN NULL; END IF;
    UPDATE identity.sessions SET token_hash=coalesce(replacement_digest,current_digest), last_activity_at=stamp,
        expires_at=least(selected.absolute_expires_at,stamp+make_interval(secs=>idle_seconds)) WHERE id=selected.id;
    IF replacement_digest IS NOT NULL THEN
        INSERT INTO audit.audit_events(actor_type,actor_id,action,resource_type,resource_id)
        VALUES ('CUSTOMER',customer,'SESSION_REFRESHED','SESSION',selected.id);
    END IF;
    RETURN customer;
END $$;

-- A previously issued sandbox cookie must not regain access if eligibility later returns.
CREATE FUNCTION identity.invalidate_changed_login_identity() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
BEGIN
    IF EXISTS (SELECT FROM customers.email_verification_eligibility e WHERE e.user_id=NEW.id AND e.scope='sandbox') THEN
        UPDATE identity.sessions SET revoked_at=greatest(created_at,clock_timestamp()) WHERE user_id=NEW.id AND revoked_at IS NULL;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER invalidate_changed_sandbox_identity AFTER UPDATE ON identity.users
FOR EACH ROW WHEN ((OLD.status,OLD.email,OLD.email_verified_at) IS DISTINCT FROM (NEW.status,NEW.email,NEW.email_verified_at))
EXECUTE FUNCTION identity.invalidate_changed_login_identity();

REVOKE ALL ON FUNCTION identity.sandbox_login_credential(text), identity.lock_sandbox_login_customer(uuid,text),
    identity.record_login_rejection(uuid), identity.issue_sandbox_session(uuid,text,text,text,text,integer,integer),
    identity.validate_sandbox_session(text,text,integer), identity.invalidate_changed_login_identity() FROM PUBLIC;
