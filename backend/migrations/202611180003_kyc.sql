-- The provider catalog precedes KYC results and wallet references.
CREATE TABLE integrations.providers (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), provider_code text NOT NULL UNIQUE,
    provider_name text NOT NULL, provider_type text NOT NULL, enabled boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE compliance.kyc_cases (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), user_id uuid NOT NULL REFERENCES identity.users,
    status text NOT NULL CHECK (status IN ('DRAFT','SUBMITTED','PENDING_PROVIDER','MANUAL_REVIEW',
        'INFORMATION_REQUESTED','APPROVED','REJECTED','EXPIRED')),
    risk_tier text, submitted_at timestamptz, reviewed_at timestamptz,
    reviewer_id uuid REFERENCES identity.users, rejection_reason_code text,
    created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (reviewed_at >= submitted_at AND updated_at >= created_at)
);
CREATE INDEX kyc_cases_customer ON compliance.kyc_cases(user_id,created_at);
CREATE TABLE compliance.kyc_documents (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), kyc_case_id uuid NOT NULL REFERENCES compliance.kyc_cases,
    document_type text NOT NULL, storage_reference text NOT NULL CHECK (storage_reference <> ''),
    verification_status text NOT NULL, created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE compliance.verification_results (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), kyc_case_id uuid NOT NULL REFERENCES compliance.kyc_cases,
    provider_id uuid NOT NULL REFERENCES integrations.providers, external_reference varchar(512),
    result_status text NOT NULL, result_summary jsonb NOT NULL CHECK (jsonb_typeof(result_summary) = 'object'),
    verified_at timestamptz, created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE compliance.risk_cases (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), user_id uuid NOT NULL REFERENCES identity.users,
    transaction_id uuid, case_type text NOT NULL, severity text NOT NULL, status text NOT NULL,
    assigned_to uuid REFERENCES identity.users, created_at timestamptz NOT NULL DEFAULT now(),
    resolved_at timestamptz CHECK (resolved_at >= created_at)
);
CREATE TABLE compliance.account_restrictions (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), user_id uuid NOT NULL REFERENCES identity.users,
    restriction_type text NOT NULL, reason_code text NOT NULL, active boolean NOT NULL,
    created_by uuid REFERENCES identity.users, created_at timestamptz NOT NULL DEFAULT now(), lifted_at timestamptz,
    CHECK (active = (lifted_at IS NULL) AND lifted_at >= created_at)
);
