use deypay_infrastructure::{
    MIGRATOR,
    rate_limit::{AdmissionError, AuthOperation::*, admit, admit_subject},
};
use sqlx::{PgPool, postgres::PgPoolOptions};

#[sqlx::test(migrator = "MIGRATOR")]
async fn admission_window_boundaries(pool: PgPool) -> sqlx::Result<()> {
    sqlx::raw_sql(include_str!("../cases/admission_windows.sql"))
        .execute(&pool)
        .await?;
    Ok(())
}

async fn api_pool(admin: &PgPool) -> sqlx::Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(4)
        .after_connect(|db, _| {
            Box::pin(async move {
                sqlx::query("SET ROLE deypay_app").execute(db).await?;
                Ok(())
            })
        })
        .connect_with((*admin.connect_options()).clone())
        .await
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn independent_api_pools_enforce_private_policy_and_fail_closed(
    pool: PgPool,
) -> sqlx::Result<()> {
    sqlx::raw_sql(include_str!(
        "../../../../../infrastructure/postgres/public-admission-grants.psql"
    ))
    .execute(&pool)
    .await?;
    let first = api_pool(&pool).await?;
    let other = api_pool(&pool).await?;
    assert_eq!(
        admit(&first, ForgotPassword).await,
        Err(AdmissionError::Unavailable)
    );
    sqlx::query(
        "INSERT INTO identity.admission_policy(operation,total_limit,subject_limit) VALUES
        ('forgot_password',1,1),('username_availability',3,1),('reset_password',3,1)",
    )
    .execute(&pool)
    .await?;
    sqlx::raw_sql(include_str!("../cases/pin_admission_windows.sql"))
        .execute(&pool)
        .await?;
    let (a, b) = tokio::join!(admit(&first, ForgotPassword), admit(&other, ForgotPassword));
    assert!(matches!(
        (a, b),
        (Ok(()), Err(AdmissionError::Limited)) | (Err(AdmissionError::Limited), Ok(()))
    ));
    let (a, b) = tokio::join!(
        admit_subject(&first, ForgotPassword, " Ada@Example.test "),
        admit_subject(&other, ForgotPassword, "ada@example.test")
    );
    assert!(matches!(
        (a, b),
        (Ok(()), Err(AdmissionError::Limited)) | (Err(AdmissionError::Limited), Ok(()))
    ));
    assert_eq!(
        admit_subject(&first, UsernameAvailability, "@ALICE").await,
        Ok(())
    );
    assert_eq!(
        admit_subject(&other, UsernameAvailability, "alice").await,
        Err(AdmissionError::Limited)
    );
    assert_eq!(admit_subject(&first, ResetPassword, "Case_A").await, Ok(()));
    assert_eq!(
        admit_subject(&other, ResetPassword, "Case_A").await,
        Err(AdmissionError::Limited)
    );
    assert_eq!(admit_subject(&other, ResetPassword, "case_a").await, Ok(()));
    assert_eq!(
        admit_subject(&first, ResetPassword, "selector.secret").await,
        Err(AdmissionError::Unavailable)
    );
    let denied: bool = sqlx::query_scalar("SELECT NOT has_any_column_privilege(current_user,'identity.admission_policy','SELECT,UPDATE,INSERT')
        AND NOT has_any_column_privilege(current_user,'identity.admission_subject_window','SELECT,UPDATE,INSERT')
        AND NOT has_any_column_privilege(current_user,'identity.email_verification_traffic','SELECT,UPDATE,INSERT')
        AND NOT has_function_privilege(current_user,'identity.take_admission(text,bigint,bytea,integer)','EXECUTE')
        AND NOT has_function_privilege(current_user,'identity.require_session_admission(text,boolean)','EXECUTE')
        AND NOT has_function_privilege('deypay_auth','identity.admit_public_request(text,text)','EXECUTE')").fetch_one(&first).await?;
    assert!(denied);
    let keyed: bool = sqlx::query_scalar("SELECT s.subject_digest=auth_crypto.hmac(convert_to('ada@example.test','UTF8'),p.subject_key,'sha256')
        FROM identity.admission_subject_window s JOIN identity.admission_policy p USING(operation) WHERE operation='forgot_password'").fetch_one(&pool).await?;
    assert!(keyed);
    sqlx::query(
        "REVOKE EXECUTE ON FUNCTION identity.admit_public_request(text,text) FROM deypay_app",
    )
    .execute(&pool)
    .await?;
    assert_eq!(
        admit(&other, ResetPassword).await,
        Err(AdmissionError::Unavailable)
    );
    first.close().await;
    assert_eq!(
        admit_subject(&first, ResetPassword, "selector").await,
        Err(AdmissionError::Unavailable)
    );
    Ok(())
}
