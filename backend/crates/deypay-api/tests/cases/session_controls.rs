use super::super::bytes;
use super::*;
use axum::http::Method;
use serde_json::Value;

const LOGOUT: &str = "/api/v1/auth/logout";
const SESSIONS: &str = "/api/v1/auth/sessions";
const CLEAR: &str = "__Host-deypay_session=; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=0";

async fn control(
    app: &Router,
    method: Method,
    path: &str,
    cookie: Option<&str>,
    origin: Option<&str>,
) -> Response {
    let mut req = Request::builder().method(method).uri(path);
    if let Some(cookie) = cookie {
        req = req.header(header::COOKIE, cookie);
    }
    if let Some(origin) = origin {
        req = req.header(header::ORIGIN, origin);
    }
    call(app, req.body(Body::empty()).unwrap()).await
}
async fn logout(app: &Router, cookie: Option<&str>) -> Response {
    control(app, Method::POST, LOGOUT, cookie, Some(ORIGIN)).await
}
async fn list(app: &Router, cookie: &str) -> Vec<Value> {
    let res = control(app, Method::GET, SESSIONS, Some(cookie), None).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()[header::CACHE_CONTROL], "no-store");
    let body: Value = serde_json::from_slice(&bytes(res).await).unwrap();
    assert_eq!(body.as_object().unwrap().len(), 1);
    body["data"].as_array().unwrap().clone()
}
async fn delete(app: &Router, cookie: &str, id: Uuid) -> Response {
    control(
        app,
        Method::DELETE,
        &format!("{SESSIONS}/{id}"),
        Some(cookie),
        Some(ORIGIN),
    )
    .await
}
async fn acknowledged(res: Response, clear: bool) {
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers()
            .get(header::SET_COOKIE)
            .map(|h| h.to_str().unwrap()),
        clear.then_some(CLEAR)
    );
    assert_eq!(res.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(bytes(res).await.as_ref(), br#"{"data":{}}"#);
}
async fn session_id(pool: &PgPool, cookie: &str) -> sqlx::Result<Uuid> {
    let token = SessionToken::from_cookie(cookie.split_once('=').unwrap().1).unwrap();
    sqlx::query_scalar("SELECT id FROM identity.sessions WHERE token_hash=$1")
        .bind(token.digest())
        .fetch_one(pool)
        .await
}
async fn active_cookie(app: &Router, name: &str) -> String {
    pair(&login(app, &format!("{name}@example.test"), PW, None).await)
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn logout_is_authoritative_origin_checked_and_idempotent(pool: PgPool) -> sqlx::Result<()> {
    let first = router(runtime(&pool).await?, hasher());
    let second = router(auth_pool(&pool).await?, hasher());
    let customer = seed(&pool, "ada").await?;
    let cookie = active_cookie(&first, "ada").await;
    let sid = session_id(&pool, &cookie).await?;
    let survivor = active_cookie(&first, "ada").await;
    for origin in [None, Some("https://elsewhere.test")] {
        for supplied in [None, Some(cookie.as_str())] {
            let forbidden = control(&first, Method::POST, LOGOUT, supplied, origin).await;
            assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
            assert!(forbidden.headers().get(header::SET_COOKIE).is_none());
        }
    }
    acknowledged(logout(&first, None).await, true).await;
    rejected(logout(&first, Some("__Host-deypay_session=malformed")).await).await;
    let duplicate = format!("{cookie}; {cookie}");
    rejected(logout(&first, Some(&duplicate)).await).await;
    assert_eq!(
        principal(&second, Some(&cookie)).await.status(),
        StatusCode::OK
    );
    acknowledged(logout(&second, Some(&cookie)).await, true).await;
    acknowledged(logout(&first, Some(&cookie)).await, true).await;
    rejected(principal(&first, Some(&cookie)).await).await;
    rejected(call(&second, post(REFRESH, String::new(), Some(&cookie))).await).await;
    assert_eq!(
        principal(&first, Some(&survivor)).await.status(),
        StatusCode::OK
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit.audit_events WHERE resource_id=$1 AND actor_id=$2
        AND actor_type='CUSTOMER' AND action='SESSION_REVOKED' AND reason='CUSTOMER_LOGOUT'",
    )
    .bind(sid)
    .bind(customer)
    .fetch_one(&pool)
    .await?;
    assert_eq!(count, 1);
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn lists_only_owned_active_metadata_and_revokes_current_or_other(
    pool: PgPool,
) -> sqlx::Result<()> {
    let first = router(runtime(&pool).await?, hasher());
    let replica = router(auth_pool(&pool).await?, hasher());
    seed(&pool, "ada").await?;
    seed(&pool, "ben").await?;
    let caller = active_cookie(&first, "ada").await;
    let other = active_cookie(&first, "ada").await;
    let expired = active_cookie(&first, "ada").await;
    let foreign = active_cookie(&first, "ben").await;
    let caller_id = session_id(&pool, &caller).await?;
    let other_id = session_id(&pool, &other).await?;
    let expired_id = session_id(&pool, &expired).await?;
    let foreign_id = session_id(&pool, &foreign).await?;
    // Let the stored idle deadline pass on the database clock.
    sqlx::query("UPDATE identity.sessions SET expires_at=clock_timestamp()+interval '10 milliseconds' WHERE id=$1")
        .bind(expired_id).execute(&pool).await?;
    sqlx::query("SELECT pg_sleep(0.02)").execute(&pool).await?;
    for supplied in [
        None,
        Some("__Host-deypay_session=bad"),
        Some(expired.as_str()),
    ] {
        rejected(control(&replica, Method::GET, SESSIONS, supplied, None).await).await;
        rejected(
            control(
                &replica,
                Method::DELETE,
                &format!("{SESSIONS}/{other_id}"),
                supplied,
                Some(ORIGIN),
            )
            .await,
        )
        .await;
    }
    let records = list(&replica, &caller).await;
    assert_eq!(records.len(), 2);
    for (id, current) in [(caller_id, true), (other_id, false)] {
        let record = records
            .iter()
            .find(|v| v["session_id"] == id.to_string())
            .unwrap();
        assert_eq!(record.as_object().unwrap().len(), 4);
        assert_eq!(record["is_current"], current);
        let created = record["created_at"].as_str().unwrap();
        let expires = record["expires_at"].as_str().unwrap();
        assert!(created.ends_with('Z') && expires.ends_with('Z'));
        assert!(sqlx::query_scalar::<_,bool>("SELECT created_at=$2::text::timestamptz AND expires_at=$3::text::timestamptz FROM identity.sessions WHERE id=$1")
            .bind(id).bind(created).bind(expires).fetch_one(&pool).await?);
    }
    let mut absent = None;
    for id in [foreign_id, Uuid::nil(), expired_id] {
        let res = delete(&replica, &caller, id).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        assert!(res.headers().get(header::SET_COOKIE).is_none());
        let body = bytes(res).await;
        assert_eq!(absent.get_or_insert_with(|| body.clone()), &body);
    }
    for origin in [None, Some("https://cross.example.test")] {
        assert_eq!(
            control(
                &first,
                Method::DELETE,
                &format!("{SESSIONS}/{other_id}"),
                Some(&caller),
                origin
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
    }
    acknowledged(logout(&replica, Some(&expired)).await, true).await;
    acknowledged(delete(&first, &caller, other_id).await, false).await;
    rejected(call(&replica, post(REFRESH, String::new(), Some(&other))).await).await;
    assert_eq!(
        delete(&replica, &caller, other_id).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(list(&replica, &caller).await.len(), 1);
    acknowledged(delete(&replica, &caller, caller_id).await, true).await;
    rejected(principal(&first, Some(&caller)).await).await;
    rejected(delete(&first, &caller, caller_id).await).await;
    assert_eq!(
        principal(&first, Some(&foreign)).await.status(),
        StatusCode::OK
    );
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn refresh_and_logout_on_different_replicas_cannot_leave_a_usable_token(
    pool: PgPool,
) -> sqlx::Result<()> {
    let a = router(runtime(&pool).await?, hasher());
    let b = router(auth_pool(&pool).await?, hasher());
    seed(&pool, "ada").await?;
    let old = active_cookie(&a, "ada").await;
    let rotated = pair(&call(&b, post(REFRESH, String::new(), Some(&old))).await);
    // Refresh won. Its predecessor cannot authorize a list, DELETE or refresh.
    let id = session_id(&pool, &rotated).await?;
    rejected(control(&a, Method::GET, SESSIONS, Some(&old), None).await).await;
    rejected(delete(&a, &old, id).await).await;
    rejected(call(&b, post(REFRESH, String::new(), Some(&old))).await).await;
    acknowledged(logout(&a, Some(&old)).await, true).await;
    rejected(principal(&b, Some(&rotated)).await).await;
    let cookie = active_cookie(&a, "ada").await;
    let (ended, refreshed) = tokio::join!(
        logout(&a, Some(&cookie)),
        call(&b, post(REFRESH, String::new(), Some(&cookie)))
    );
    acknowledged(ended, true).await;
    if refreshed.status() == StatusCode::OK {
        let replacement = pair(&refreshed);
        rejected(principal(&a, Some(&replacement)).await).await;
        rejected(call(&b, post(REFRESH, String::new(), Some(&replacement))).await).await;
    } else {
        rejected(refreshed).await;
    }
    rejected(principal(&b, Some(&cookie)).await).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM identity.sessions WHERE revoked_at IS NULL"
        )
        .fetch_one(&pool)
        .await?,
        0
    );
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn concurrent_revokes_and_refresh_commit_exactly_one_revocation(
    pool: PgPool,
) -> sqlx::Result<()> {
    let a = router(runtime(&pool).await?, hasher());
    let b = router(auth_pool(&pool).await?, hasher());
    seed(&pool, "ada").await?;
    let caller = active_cookie(&a, "ada").await;
    let other = active_cookie(&b, "ada").await;
    let id = session_id(&pool, &other).await?;
    let (one, two, refresh) = tokio::join!(
        delete(&a, &caller, id),
        delete(&b, &caller, id),
        call(&b, post(REFRESH, String::new(), Some(&other)))
    );
    let mut statuses = [one.status(), two.status()];
    statuses.sort();
    assert_eq!(statuses, [StatusCode::OK, StatusCode::NOT_FOUND]);
    if refresh.status() == StatusCode::OK {
        rejected(principal(&a, Some(&pair(&refresh))).await).await;
    } else {
        rejected(refresh).await;
    }
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM audit.audit_events WHERE resource_id=$1 AND action='SESSION_REVOKED'")
        .bind(id).fetch_one(&pool).await?,1);
    rejected(principal(&b, Some(&other)).await).await;
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn failed_audit_and_unavailable_store_never_acknowledge_revocation(
    pool: PgPool,
) -> sqlx::Result<()> {
    let connection = runtime(&pool).await?;
    let app = router(connection.clone(), hasher());
    let observer = router(auth_pool(&pool).await?, hasher());
    seed(&pool, "ada").await?;
    let caller = active_cookie(&app, "ada").await;
    let other = active_cookie(&app, "ada").await;
    let id = session_id(&pool, &other).await?;
    sqlx::raw_sql(
        "CREATE FUNCTION audit.test_fail_controls() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN RAISE EXCEPTION 'synthetic audit outage'; END $$;
        CREATE TRIGGER test_fail_controls BEFORE INSERT ON audit.audit_events FOR EACH ROW
        WHEN (NEW.action='SESSION_REVOKED') EXECUTE FUNCTION audit.test_fail_controls();",
    )
    .execute(&pool)
    .await?;
    for res in [
        logout(&app, Some(&caller)).await,
        delete(&app, &caller, id).await,
    ] {
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(res.headers().get(header::SET_COOKIE).is_none());
        assert!(!String::from_utf8_lossy(&bytes(res).await).contains("audit"));
    }
    assert_eq!(
        principal(&observer, Some(&caller)).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        principal(&observer, Some(&other)).await.status(),
        StatusCode::OK
    );
    connection.close().await;
    for res in [
        logout(&app, Some(&caller)).await,
        delete(&app, &caller, id).await,
        control(&app, Method::GET, SESSIONS, Some(&caller), None).await,
    ] {
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(res.headers().get(header::SET_COOKIE).is_none());
    }
    assert_eq!(list(&observer, &caller).await.len(), 2);
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn session_controls_require_verifier_grants_and_purpose_bound_proofs(
    pool: PgPool,
) -> sqlx::Result<()> {
    let verifier = runtime(&pool).await?;
    assert!(store(verifier.clone()).ready().await);
    let signatures = [
        "identity.auth_logout(uuid,text,bigint,bytea)",
        "identity.auth_list_sessions(uuid,text,bigint,bytea)",
        "identity.auth_revoke_session(uuid,text,uuid,bigint,bytea)",
    ];
    for signature in signatures {
        assert!(
            sqlx::query_scalar::<_, bool>(
                "SELECT has_function_privilege('deypay_auth',$1,'EXECUTE')
            AND NOT has_function_privilege('deypay_app',$1,'EXECUTE')"
            )
            .bind(signature)
            .fetch_one(&pool)
            .await?
        );
    }
    for role in ["deypay_auth", "deypay_app"] {
        assert!(sqlx::query_scalar::<_,bool>("SELECT NOT has_function_privilege($1,'identity.auth_close_owned(uuid,uuid,uuid,text)','EXECUTE')
            AND NOT has_column_privilege($1,'identity.sessions','logout_previous_digest','SELECT')
            AND NOT has_any_column_privilege($1,'identity.sessions','UPDATE')").bind(role).fetch_one(&pool).await?);
    }
    let app = router(verifier.clone(), hasher());
    seed(&pool, "ada").await?;
    let cookie = active_cookie(&app, "ada").await;
    let digest = SessionToken::from_cookie(cookie.split_once('=').unwrap().1)
        .unwrap()
        .digest();
    let sid = session_id(&pool, &cookie).await?;
    let release = RELEASE.parse::<Uuid>().unwrap();
    let bad=sqlx::query("SELECT identity.auth_revoke_session($1,$2,$3,floor(extract(epoch FROM clock_timestamp()))::bigint,$4)")
        .bind(release).bind(digest).bind(sid).bind(vec![0u8;32]).execute(&verifier).await.unwrap_err();
    assert_eq!(
        bad.as_database_error().and_then(|e| e.code()).as_deref(),
        Some("28000")
    );
    assert_eq!(
        principal(&app, Some(&cookie)).await.status(),
        StatusCode::OK
    );
    sqlx::query(
        "REVOKE EXECUTE ON FUNCTION identity.auth_logout(uuid,text,bigint,bytea) FROM deypay_auth",
    )
    .execute(&pool)
    .await?;
    assert!(!store(verifier).ready().await);
    assert_eq!(
        logout(&app, Some(&cookie)).await.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    Ok(())
}
