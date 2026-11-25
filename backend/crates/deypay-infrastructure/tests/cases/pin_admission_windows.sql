-- Disposable fixtures only. Keep quota assertions deterministic even when
-- concurrent clients straddle a wall-clock minute. Rollover tests move the row explicitly.
INSERT INTO identity.email_verification_traffic(operation,minute,hits)
    SELECT op,clock_timestamp()+interval '1 day',0
    FROM (SELECT operation AS op FROM identity.admission_policy
          UNION VALUES ('register'),('logout_noop'),('session_revoke_noop')) AS scopes
    ON CONFLICT(operation) DO UPDATE SET minute=excluded.minute,hits=0,named=0,window_cap=NULL;
