use super::PgSessions;
use deypay_application::{
    login::{ActiveSession, RevokedSession, SessionControls},
    passwords::CredentialError,
};
use uuid::Uuid;

impl SessionControls for PgSessions {
    async fn logout(&self, digest: &str) -> Result<(), CredentialError> {
        let (issued, tag) = self.proof("logout", &[digest]);
        sqlx::query("SELECT identity.auth_logout($1,$2,$3,$4)")
            .bind(self.release)
            .bind(digest)
            .bind(issued)
            .bind(tag)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(super::session_error)
    }

    async fn list_sessions(&self, digest: &str) -> Result<Vec<ActiveSession>, CredentialError> {
        let (issued, tag) = self.proof("list_sessions", &[digest]);
        let rows: Vec<(Uuid, bool, String, String)> = sqlx::query_as(
            r#"SELECT session_id,is_current,
                to_char(created_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"'),
                to_char(expires_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS.US"Z"')
                FROM identity.auth_list_sessions($1,$2,$3,$4)"#,
        )
        .bind(self.release)
        .bind(digest)
        .bind(issued)
        .bind(tag)
        .fetch_all(&self.pool)
        .await
        .map_err(super::session_error)?;
        if !rows.iter().any(|(_, current, _, _)| *current) {
            return Err(CredentialError::Rejected);
        }
        Ok(rows
            .into_iter()
            .map(
                |(session_id, is_current, created_at, expires_at)| ActiveSession {
                    session_id,
                    is_current,
                    created_at,
                    expires_at,
                },
            )
            .collect())
    }

    async fn revoke_session(
        &self,
        digest: &str,
        target: Option<Uuid>,
    ) -> Result<RevokedSession, CredentialError> {
        let (issued, tag) = self.proof(
            "revoke_session",
            &[digest, &target.map(|id| id.to_string()).unwrap_or_default()],
        );
        let outcome: Option<String> =
            sqlx::query_scalar("SELECT identity.auth_revoke_session($1,$2,$3,$4,$5)")
                .bind(self.release)
                .bind(digest)
                .bind(target)
                .bind(issued)
                .bind(tag)
                .fetch_one(&self.pool)
                .await
                .map_err(super::session_error)?;
        match outcome.as_deref() {
            Some("CURRENT") => Ok(RevokedSession::Current),
            Some("OTHER") => Ok(RevokedSession::Other),
            Some("ABSENT") => Ok(RevokedSession::Absent),
            None => Err(CredentialError::Rejected),
            _ => Err(CredentialError::Unavailable),
        }
    }
}
