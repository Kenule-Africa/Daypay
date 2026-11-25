use super::{BrowserAuth, failure, reply};
use crate::sessions::{expired_session_cookie, request_token};
use axum::{
    Router,
    extract::{Path, Request, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use deypay_application::{
    login::{RevokedSession, SessionControls},
    passwords::CredentialVerifier,
};
use deypay_infrastructure::sessions::SessionToken;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

impl<R: SessionControls + 'static, V: CredentialVerifier + Send + 'static> BrowserAuth<R, V> {
    pub(super) fn control_routes() -> Router<Arc<Self>> {
        Router::new()
            .route("/api/v1/auth/logout", post(Self::logout))
            .route("/api/v1/auth/sessions", get(Self::list))
            .route("/api/v1/auth/sessions/{session_id}", delete(Self::revoke))
    }
    fn control_cookie(&self, req: &Request) -> Result<Option<SessionToken>, StatusCode> {
        if !self.guard.origin.permits(req.method(), req.headers()) {
            return Err(StatusCode::FORBIDDEN);
        }
        request_token(req.headers()).map_err(|_| StatusCode::UNAUTHORIZED)
    }
    async fn logout(State(auth): State<Arc<Self>>, req: Request) -> Response {
        match auth.control_cookie(&req) {
            Ok(Some(token)) => match auth.guard.store.logout(&token.digest()).await {
                Ok(()) => cleared(),
                Err(e) => failure(e),
            },
            Ok(None) => cleared(),
            Err(status) => reply(status),
        }
    }
    async fn list(State(auth): State<Arc<Self>>, req: Request) -> Response {
        let token = match auth.control_cookie(&req) {
            Ok(Some(token)) => token,
            Ok(None) => return reply(StatusCode::UNAUTHORIZED),
            Err(status) => return reply(status),
        };
        match auth.guard.store.list_sessions(&token.digest()).await {
            Ok(sessions) => {
                let records: Vec<_> = sessions
                    .into_iter()
                    .map(|s| {
                        json!({"session_id":s.session_id.to_string(),
                    "is_current":s.is_current,"created_at":s.created_at,"expires_at":s.expires_at})
                    })
                    .collect();
                (
                    [
                        (header::CONTENT_TYPE, "application/json"),
                        (header::CACHE_CONTROL, "no-store"),
                    ],
                    json!({"data": records}).to_string(),
                )
                    .into_response()
            }
            Err(e) => failure(e),
        }
    }
    async fn revoke(
        State(auth): State<Arc<Self>>,
        Path(target): Path<String>,
        req: Request,
    ) -> Response {
        let token = match auth.control_cookie(&req) {
            Ok(Some(token)) => token,
            Ok(None) => return reply(StatusCode::UNAUTHORIZED),
            Err(status) => return reply(status),
        };
        let Ok(id) = Uuid::parse_str(&target) else {
            return reply(StatusCode::BAD_REQUEST);
        };
        match auth.guard.store.revoke_session(&token.digest(), id).await {
            Ok(RevokedSession::Current) => cleared(),
            Ok(RevokedSession::Other) => reply(StatusCode::OK),
            Ok(RevokedSession::Absent) => reply(StatusCode::NOT_FOUND),
            Err(e) => failure(e),
        }
    }
}

fn cleared() -> Response {
    let mut response = reply(StatusCode::OK);
    response
        .headers_mut()
        .insert(header::SET_COOKIE, expired_session_cookie());
    response
}
