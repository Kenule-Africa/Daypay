use axum::http::{HeaderMap, HeaderValue, Method, Uri, header};
use deypay_infrastructure::sessions::SessionToken;

const COOKIE: &str = "__Host-deypay_session";

/// Exact HTTPS origin supplied by trusted deployment configuration. Request
/// Host, Forwarded and customer-supplied identity headers are never used.
pub struct BrowserOrigin(HeaderValue);

impl BrowserOrigin {
    pub fn parse(value: &str) -> Option<Self> {
        let uri: Uri = value.parse().ok()?;
        let authority = uri.authority()?;
        if uri.scheme_str() != Some("https")
            || authority.host().is_empty()
            || authority.as_str().contains('@')
            || value != format!("https://{authority}")
        {
            return None;
        }
        Some(Self(HeaderValue::from_str(value).ok()?))
    }

    pub fn permits(&self, method: &Method, headers: &HeaderMap) -> bool {
        if matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
            return true;
        }
        let mut origins = headers.get_all(header::ORIGIN).iter();
        origins.next() == Some(&self.0) && origins.next().is_none()
    }
}

/// Reject ambiguous cookies even when supplied in separate Cookie fields.
pub fn request_token(headers: &HeaderMap) -> Result<Option<SessionToken>, &'static str> {
    let mut token = None;
    for field in headers.get_all(header::COOKIE) {
        for item in field.to_str().map_err(|_| "Invalid session")?.split(';') {
            let item = item.trim();
            if item == COOKIE {
                return Err("Invalid session");
            }
            if let Some((COOKIE, value)) = item.split_once('=') {
                if token.is_some() {
                    return Err("Invalid session");
                }
                token = Some(SessionToken::from_cookie(value).ok_or("Invalid session")?);
            }
        }
    }
    Ok(token)
}

pub fn session_cookie(token: &SessionToken) -> HeaderValue {
    let mut value = HeaderValue::from_str(&format!(
        "{COOKIE}={}; Path=/; Secure; HttpOnly; SameSite=Lax",
        token.cookie_value().as_str()
    ))
    .expect("session token uses lowercase hex");
    value.set_sensitive(true);
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_changes_require_one_exact_configured_origin() {
        let origin = BrowserOrigin::parse("https://app.example.test").unwrap();
        let mut headers = HeaderMap::new();
        assert!(origin.permits(&Method::GET, &headers));
        assert!(!origin.permits(&Method::POST, &headers));
        for invalid in [
            "null",
            "https://app.example.test.other",
            "https://other.test",
        ] {
            headers.insert(header::ORIGIN, HeaderValue::from_static(invalid));
            assert!(!origin.permits(&Method::POST, &headers));
        }
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://app.example.test"),
        );
        assert!(origin.permits(&Method::PATCH, &headers));
        headers.append(
            header::ORIGIN,
            HeaderValue::from_static("https://app.example.test"),
        );
        assert!(!origin.permits(&Method::DELETE, &headers));
        for invalid in [
            "http://app.example.test",
            "https://user@app.example.test",
            "https://app.example.test/path",
        ] {
            assert!(BrowserOrigin::parse(invalid).is_none());
        }
    }

    #[test]
    fn only_one_well_formed_host_cookie_is_accepted() {
        let token = SessionToken::generate().unwrap();
        let set_cookie = session_cookie(&token);
        assert!(set_cookie.is_sensitive());
        let (pair, attrs) = set_cookie.to_str().unwrap().split_once("; ").unwrap();
        assert_eq!(attrs, "Path=/; Secure; HttpOnly; SameSite=Lax");
        let mut headers = HeaderMap::new();
        assert!(request_token(&headers).unwrap().is_none());
        headers.append(header::COOKIE, HeaderValue::from_str(pair).unwrap());
        assert!(request_token(&headers).unwrap().unwrap().digest() == token.digest());
        headers.append(header::COOKIE, HeaderValue::from_str(pair).unwrap());
        assert!(request_token(&headers).is_err());
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("__Host-deypay_session=bad"),
        );
        assert!(request_token(&headers).is_err());
    }
}
