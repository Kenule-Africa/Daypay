use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use deypay_api::email_verification::{Policy, routes};
use deypay_infrastructure::{
    MIGRATOR,
    email_delivery::{Delivery, EmailDelivery, Message, MockDelivery, SendResult},
    email_verification::VerificationWorker,
};
use serde_json::{Value, json};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use uuid::Uuid;

const BASE: &str = "http://127.0.0.1:8000/verify-email";
async fn role(pool: &PgPool, statement: &'static str) -> sqlx::Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(4)
        .after_connect(move |conn, _| {
            Box::pin(async move {
                sqlx::query(statement).execute(conn).await?;
                Ok(())
            })
        })
        .connect_with((*pool.connect_options()).clone())
        .await
}
async fn setup(pool: &PgPool) -> sqlx::Result<(Router, PgPool)> {
    for grants in [
        include_str!("../../../../infrastructure/postgres/registration-sandbox.psql"),
        include_str!("../../../../infrastructure/postgres/phase-two-grants.psql"),
        include_str!("../../../../infrastructure/postgres/email-verification-api-grants.psql"),
    ] {
        sqlx::raw_sql(grants).execute(pool).await?;
    }
    let name: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(pool)
        .await?;
    let grants =
        include_str!("../../../../infrastructure/postgres/email-verification-worker-grants.psql")
            .replace(":\"DBNAME\"", &format!("\"{name}\""))
            .replace(":\"worker_role\"", "pg_read_all_settings");
    sqlx::raw_sql(&grants).execute(pool).await?;
    let api = role(pool, "SET ROLE deypay_app").await?;
    assert!(
        sqlx::query("SELECT integrations.dispatch_verification_email(5)")
            .execute(&api)
            .await
            .is_err()
    );
    let worker = role(pool, "SET ROLE pg_read_all_settings").await?;
    assert!(
        sqlx::query(
            "SELECT identity.consume_verification_email(gen_random_uuid(),'foo','sandbox',1)"
        )
        .execute(&worker)
        .await
        .is_err()
    );
    Ok((
        routes(Some(Policy {
            scope: "sandbox",
            per_minute: 120,
            cooldown_seconds: 60,
            resends_per_hour: 2,
            guesses: 3,
        }))
        .with_state(api),
        worker,
    ))
}
async fn register(pool: &PgPool, email: &str, name: &str) -> sqlx::Result<()> {
    sqlx::query("SELECT identity.register_customer($1,'+2348012345678',$2,'$argon2id$test-only','Test','Customer','NG','1.0','1.0',true,true)")
        .bind(email).bind(name).execute(pool).await?;
    Ok(())
}
async fn post(app: &Router, path: &str, body: Value) -> (StatusCode, String) {
    let r = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/auth/{path}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.headers()[header::CACHE_CONTROL], "no-store");
    assert!(r.headers().get(header::SET_COOKIE).is_none());
    (
        r.status(),
        String::from_utf8(to_bytes(r.into_body(), 4096).await.unwrap().to_vec()).unwrap(),
    )
}
fn token(text: &str) -> &str {
    text.lines()
        .find_map(|line| line.strip_prefix(&format!("{BASE}#")))
        .unwrap()
}
async fn verify(app: &Router, token: &str) -> StatusCode {
    let (code, body) = post(app, "verify-email", json!({"token":token})).await;
    assert!(!body.contains(token));
    code
}
async fn age_requests(pool: &PgPool) -> sqlx::Result<()> {
    sqlx::query("UPDATE integrations.outbox_events SET created_at=created_at-interval '61 seconds' WHERE event_type='EMAIL_VERIFICATION_REQUESTED'").execute(pool).await?;
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn registration_delivery_then_concurrent_confirmation(pool: PgPool) -> sqlx::Result<()> {
    let (app, worker_db) = setup(&pool).await?;
    register(&pool, "ada@example.test", "ada").await?;
    sqlx::query("SELECT integrations.dispatch_verification_email(5)")
        .execute(&worker_db)
        .await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM integrations.claim_verification_email('published',900,30)"
        )
        .fetch_one(&worker_db)
        .await?,
        0
    );
    let inbox = Arc::new(MockDelivery::default());
    let worker = VerificationWorker::sandbox(inbox.clone(), [11; 32], BASE).unwrap();
    let (a, b) = tokio::join!(worker.tick(&worker_db), worker.tick(&worker_db));
    assert_ne!(a?, b?);
    let (id, text) = inbox.last_message().unwrap();
    let bearer = token(&text);
    assert!(bearer.starts_with(&id.to_string()));
    let snapshot:(bool,bool,String)=sqlx::query_as("SELECT u.email_verified_at IS NULL,j.status='SUCCEEDED', concat_ws(' ',c.code_hash,j.payload,e.payload,a.metadata) FROM identity.users u JOIN identity.verification_challenges c ON c.user_id=u.id JOIN integrations.jobs j ON j.id=c.id JOIN integrations.outbox_events e ON e.id=j.id JOIN audit.audit_events a ON a.correlation_id=j.id")
        .fetch_one(&pool).await?;
    assert!(snapshot.0 && snapshot.1 && !snapshot.2.contains(bearer.split_once('.').unwrap().1));
    let (a, b) = tokio::join!(verify(&app, bearer), verify(&app, bearer));
    assert!([a, b].contains(&StatusCode::OK) && [a, b].contains(&StatusCode::UNPROCESSABLE_ENTITY));
    assert_eq!(verify(&app, bearer).await, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(sqlx::query_scalar::<_,bool>("SELECT email_verified_at IS NOT NULL AND phone_verified_at IS NULL AND status='CONTACT_UNVERIFIED'
      AND NOT EXISTS(SELECT FROM identity.sessions) AND NOT EXISTS(SELECT FROM wallets.wallet_profiles)
      AND NOT EXISTS(SELECT FROM payments.transactions) AND NOT EXISTS(SELECT FROM compliance.kyc_cases)
      AND (SELECT count(*) FROM audit.audit_events WHERE action='EMAIL_VERIFIED')=1 FROM identity.users").fetch_one(&pool).await?);
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn resend_cooldown_limit_supersession_and_guess_budget(pool: PgPool) -> sqlx::Result<()> {
    let (app, db) = setup(&pool).await?;
    let inbox = Arc::new(MockDelivery::default());
    let worker = VerificationWorker::sandbox(inbox.clone(), [12; 32], BASE).unwrap();
    register(&pool, "one@example.test", "one").await?;
    worker.tick(&db).await?;
    let (id, first) = inbox.last_message().unwrap();
    let first = token(&first);
    let path = "resend-verification";
    let unknown = post(&app, path, json!({"email":"unknown@example.test"})).await;
    assert_eq!(unknown.0, StatusCode::ACCEPTED);
    let request = json!({"email":"ONE@example.test"});
    assert_eq!(unknown, post(&app, path, request.clone()).await);
    assert!(!worker.tick(&db).await?);
    age_requests(&pool).await?;
    let (a, b) = tokio::join!(
        post(&app, path, request.clone()),
        post(&app, path, request.clone())
    );
    assert_eq!(a, unknown);
    assert_eq!(b, unknown);
    assert_eq!(verify(&app, first).await, StatusCode::UNPROCESSABLE_ENTITY);
    worker.tick(&db).await?;
    let (new_id, next) = inbox.last_message().unwrap();
    assert_ne!(id, new_id);
    age_requests(&pool).await?;
    assert_eq!(unknown, post(&app, path, request).await);
    assert!(!worker.tick(&db).await?);
    for _ in 0..3 {
        assert_eq!(
            verify(&app, &format!("{new_id}.{}", "00".repeat(32))).await,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    assert_eq!(
        verify(&app, token(&next)).await,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(sqlx::query_scalar::<_,i32>("SELECT attempt_count FROM identity.verification_challenges WHERE id=$1 AND revoked_at IS NOT NULL")
        .bind(new_id).fetch_one(&pool).await?,3);
    Ok(())
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn binding_expiry_and_distributed_admission(pool: PgPool) -> sqlx::Result<()> {
    let (app, db) = setup(&pool).await?;
    let inbox = Arc::new(MockDelivery::default());
    let worker = VerificationWorker::sandbox(inbox.clone(), [22; 32], BASE).unwrap();
    register(&pool, "a@example.test", "a").await?;
    worker.tick(&db).await?;
    let (aid, first) = inbox.last_message().unwrap();
    register(&pool, "b@example.test", "b").await?;
    worker.tick(&db).await?;
    let (_, second) = inbox.last_message().unwrap();
    let crossed = format!("{aid}.{}", token(&second).split_once('.').unwrap().1);
    assert_eq!(
        verify(&app, &crossed).await,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    sqlx::query(
        "UPDATE identity.users SET email='changed@example.test' WHERE email='a@example.test'",
    )
    .execute(&pool)
    .await?;
    assert_eq!(
        verify(&app, token(&first)).await,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    // Seed an already-expired challenge without changing its immutable timestamps.
    let expired=sqlx::query_scalar::<_,Uuid>("INSERT INTO identity.verification_challenges(user_id,challenge_type,destination,code_hash,created_at,expires_at)
      SELECT id,'EMAIL_VERIFICATION',email,encode(sha256(decode(repeat('aa',32),'hex')),'hex'),now()-interval '2 hours',now()-interval '1 hour'
      FROM identity.users WHERE email='b@example.test' RETURNING id").fetch_one(&pool).await?;
    assert_eq!(
        verify(&app, &format!("{expired}.{}", "aa".repeat(32))).await,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        verify(&app, "malformed").await,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        post(
            &app,
            "resend-verification",
            json!({"email":"x@example.test\r\nBcc: z"})
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let small = routes(Some(Policy {
        scope: "sandbox",
        per_minute: 1,
        cooldown_seconds: 1,
        resends_per_hour: 1,
        guesses: 1,
    }))
    .with_state(pool.clone());
    assert_eq!(
        post(
            &small,
            "resend-verification",
            json!({"email":"b@example.test"})
        )
        .await
        .0,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert!(
        sqlx::query_scalar::<_, bool>(
            "SELECT bool_and(email_verified_at IS NULL) FROM identity.users"
        )
        .fetch_one(&pool)
        .await?
    );
    Ok(())
}

#[derive(Default)]
struct Flaky(Mutex<Vec<(Uuid, String)>>);
impl EmailDelivery for Flaky {
    fn send<'a>(&'a self, id: Uuid, msg: &'a Message) -> SendResult<'a> {
        Box::pin(async move {
            let mut seen = self.0.lock().unwrap();
            seen.push((id, serde_json::to_string(msg).unwrap()));
            match seen.len() {
                1 => Delivery::RateLimited(35),
                2 => Delivery::Transient,
                _ => Delivery::Accepted(Some(Uuid::from_u128(7))),
            }
        })
    }
}
#[sqlx::test(migrator = "MIGRATOR")]
async fn delivery_retry_recovery_and_payload_stability(pool: PgPool) -> sqlx::Result<()> {
    let (_, db) = setup(&pool).await?;
    let adapter = Arc::new(Flaky::default());
    let worker = VerificationWorker::sandbox(adapter.clone(), [42; 32], BASE).unwrap();
    register(&pool, "retry@example.test", "retry").await?;
    worker.tick(&db).await?;
    assert!(sqlx::query_scalar::<_,bool>("SELECT status='PENDING' AND attempts=1 AND last_error='RATE_LIMITED' AND available_at>=now()+interval '30 seconds' FROM integrations.jobs").fetch_one(&pool).await?);
    assert!(!worker.tick(&db).await?);
    for _ in 0..2 {
        sqlx::query("UPDATE integrations.jobs SET available_at=now()")
            .execute(&pool)
            .await?;
        worker.tick(&db).await?;
    }
    assert!(
        sqlx::query_scalar::<_, bool>(
            "SELECT status='SUCCEEDED' AND attempts=3 AND (payload->>'provider_message_id')::uuid=$1 FROM integrations.jobs"
        )
        .bind(Uuid::from_u128(7))
        .fetch_one(&pool)
        .await?
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM identity.verification_challenges")
            .fetch_one(&pool)
            .await?,
        1
    );
    let seen = adapter.0.lock().unwrap();
    assert_eq!(seen.len(), 3);
    assert!(seen.windows(2).all(|w| w[0] == w[1]));
    Ok(())
}
#[sqlx::test(migrator = "MIGRATOR")]
async fn reclaim_after_worker_crash_and_reject_a_changed_retry(pool: PgPool) -> sqlx::Result<()> {
    let (_, db) = setup(&pool).await?;
    register(&pool, "crash@example.test", "crash").await?;
    let adapter = Arc::new(Flaky::default());
    let first = VerificationWorker::sandbox(adapter.clone(), [20; 32], BASE).unwrap();
    first.tick(&db).await?;
    // Recreate a worker crash after claim, with no provider acknowledgement recorded.
    let stale_lease = sqlx::query_scalar::<_, Uuid>(
        "UPDATE integrations.jobs SET status='RUNNING',available_at=now(),
      lease_token=gen_random_uuid(),locked_until=now()-interval '1 second' RETURNING lease_token",
    )
    .fetch_one(&pool)
    .await?;
    let changed = VerificationWorker::sandbox(adapter.clone(), [21; 32], BASE).unwrap();
    assert!(changed.tick(&db).await?);
    let id=sqlx::query_scalar::<_,Uuid>("SELECT id FROM integrations.jobs WHERE status='DEAD_LETTER' AND last_error='MESSAGE_CHANGED'").fetch_one(&pool).await?;
    assert!(
        !sqlx::query_scalar::<_, bool>(
            "SELECT integrations.finish_verification_email($1,$2,'ACCEPTED',0,NULL)"
        )
        .bind(id)
        .bind(stale_lease)
        .fetch_one(&db)
        .await?
    );
    assert_eq!(adapter.0.lock().unwrap().len(), 1);
    Ok(())
}
