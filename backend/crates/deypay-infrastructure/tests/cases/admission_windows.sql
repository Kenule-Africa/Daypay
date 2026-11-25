-- Disposable PostgreSQL test; each scenario starts with fresh counters.
INSERT INTO identity.admission_policy(operation,total_limit,subject_limit)
    VALUES ('forgot_password',3,1), ('reset_password',2,1), ('username_availability',2,2);

INSERT INTO identity.email_verification_traffic(operation,minute,hits)
    SELECT operation,clock_timestamp()+interval '1 day',0 FROM identity.admission_policy;

DO $$
BEGIN
    IF NOT identity.admit_public_request('forgot_password',NULL)
       OR NOT identity.admit_public_request('forgot_password','unknown@example.test')
       OR identity.admit_public_request('forgot_password','unknown@example.test') THEN
        RAISE EXCEPTION 'Expected uniform subject saturation without an account';
    END IF;
    IF NOT identity.admit_public_request('username_availability','unknown')
       OR NOT identity.admit_public_request('username_availability','unknown')
       OR identity.admit_public_request('username_availability','unknown') THEN
        RAISE EXCEPTION 'Independent operation policy was not enforced';
    END IF;
    IF NOT identity.admit_public_request('reset_password','selector-a')
       OR NOT identity.admit_public_request('reset_password','selector-b')
       OR identity.admit_public_request('reset_password','selector-c') THEN
        RAISE EXCEPTION 'Subject stage must bound cardinality independently';
    END IF;

    UPDATE identity.email_verification_traffic SET minute=minute + interval '1 hour' WHERE operation='reset_password';
    IF identity.admit_public_request('reset_password','selector-c') THEN
        RAISE EXCEPTION 'A backward clock jump reopened a spent quota';
    END IF;
    UPDATE identity.email_verification_traffic SET minute=clock_timestamp() - interval '2 minutes'
        WHERE operation='reset_password';
    IF NOT identity.admit_public_request('reset_password','selector-c') THEN
        RAISE EXCEPTION 'New window must admit a fresh subject';
    END IF;
    IF (SELECT count(*) FROM identity.admission_subject_window WHERE operation='reset_password') <> 1 THEN
        RAISE EXCEPTION 'Rollover must reclaim previous subject buckets';
    END IF;
END $$;
