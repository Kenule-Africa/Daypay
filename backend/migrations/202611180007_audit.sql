CREATE TABLE audit.admin_roles (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), role_code text NOT NULL UNIQUE, description text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE audit.admin_permissions (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), permission_code text NOT NULL UNIQUE, description text NOT NULL
);
CREATE TABLE audit.role_permissions (
    role_id uuid NOT NULL REFERENCES audit.admin_roles, permission_id uuid NOT NULL REFERENCES audit.admin_permissions,
    PRIMARY KEY (role_id,permission_id)
);
CREATE TABLE audit.admin_assignments (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), user_id uuid NOT NULL REFERENCES identity.users,
    role_id uuid NOT NULL REFERENCES audit.admin_roles, assigned_at timestamptz NOT NULL DEFAULT now(), revoked_at timestamptz,
    CHECK (revoked_at >= assigned_at)
);
CREATE UNIQUE INDEX admin_current_assignment ON audit.admin_assignments(user_id,role_id) WHERE revoked_at IS NULL;
CREATE TABLE audit.audit_events (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), actor_type text NOT NULL, actor_id uuid, action text NOT NULL,
    resource_type text NOT NULL, resource_id uuid, reason text, correlation_id uuid,
    metadata jsonb NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(metadata) = 'object'),
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX audit_resource_history ON audit.audit_events(resource_type,resource_id,created_at);
CREATE TRIGGER immutable_audit BEFORE UPDATE OR DELETE OR TRUNCATE ON audit.audit_events
    FOR EACH STATEMENT EXECUTE FUNCTION audit.reject_mutation();
CREATE TABLE audit.approval_requests (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(), action_type text NOT NULL, target_reference text NOT NULL,
    requested_by uuid NOT NULL REFERENCES identity.users, approved_by uuid REFERENCES identity.users,
    status text NOT NULL CHECK (status IN ('PENDING','APPROVED','REJECTED')),
    reason text NOT NULL CHECK (btrim(reason) <> ''), created_at timestamptz NOT NULL DEFAULT now(), decided_at timestamptz,
    CHECK (approved_by IS DISTINCT FROM requested_by),
    CHECK ((status='PENDING') = (approved_by IS NULL) AND (status='PENDING') = (decided_at IS NULL)),
    CHECK (decided_at >= created_at)
);
