use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};
use tavra_control::{AppState, router};
use tower::ServiceExt;
async fn call(app: &Router, path: &str, token: Option<&str>, body: Value) -> (StatusCode, Value) {
    let mut r = Request::builder()
        .method(
            if (path == "/v1/users" && body.is_null()) || path == "/v1/health" {
                "GET"
            } else if path.starts_with("/v1/users/") {
                "PATCH"
            } else {
                "POST"
            },
        )
        .uri(path)
        .header("content-type", "application/json");
    if let Some(t) = token {
        r = r.header("authorization", format!("Bearer {t}"));
    }
    let r = app
        .clone()
        .oneshot(r.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = r.status();
    let bytes = r.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}
#[tokio::test]
async fn rollback_and_user_block_never_revive_credentials() {
    let url = std::env::var("TAVRA_TEST_DATABASE_URL").expect("isolated test database required");
    let state = AppState::connect(&url, "bootstrap-secret-for-integration-test-0001".into())
        .await
        .unwrap();
    sqlx::query("TRUNCATE admins,sessions,users,subscriptions,challenges,devices,servers CASCADE")
        .execute(&state.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE security_clock SET high_water=0")
        .execute(&state.pool)
        .await
        .unwrap();
    let time = Arc::new(AtomicI64::new(state.now()));
    let base = time.load(Ordering::SeqCst);
    let state = state.with_clock({
        let time = time.clone();
        move || time.load(Ordering::SeqCst)
    });
    let app = router(state.clone());
    let (_,login)=call(&app,"/v1/admin/bootstrap",None,json!({"bootstrap_secret":"bootstrap-secret-for-integration-test-0001","admin_secret":"persistent-admin-secret-01234567890123456789"})).await;
    let token = login["token"].as_str().unwrap();
    let initial_generation: i64 =
        sqlx::query_scalar("SELECT generation FROM sessions WHERE token_hash=$1")
            .bind(tavra_control::auth::hash(token))
            .fetch_one(&state.pool)
            .await
            .unwrap();
    let (_, user) = call(
        &app,
        "/v1/users",
        Some(token),
        json!({"first_name":"Clock"}),
    )
    .await;
    let (_, sub) = call(
        &app,
        "/v1/subscriptions",
        Some(token),
        json!({"user_id":user["id"],"expires_at":0,"device_limit":0}),
    )
    .await;
    let proof = proof_for(&app, sub["invitation"].as_str().unwrap()).await;
    let user_path = format!("/v1/users/{}", user["id"].as_str().unwrap());
    assert_eq!(
        call(&app, &user_path, Some(token), json!({"status":"blocked"}))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&app, &user_path, Some(token), json!({"status":"active"}))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&app, "/v1/enroll", None, proof).await.0,
        StatusCode::UNAUTHORIZED,
        "block/unblock must destroy pending proof"
    );
    let proof = proof_for(&app, sub["invitation"].as_str().unwrap()).await;
    time.store(base + 121, Ordering::SeqCst);
    assert_eq!(
        call(&app, "/v1/enroll", None, proof.clone()).await.0,
        StatusCode::UNAUTHORIZED
    );
    time.store(base, Ordering::SeqCst);
    let (status, error) = call(&app, "/v1/enroll", None, proof.clone()).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(error["error"]["code"], "clock_untrusted");
    // A new AppState (process restart) must retain the durable rollback boundary.
    let restarted = router(
        AppState::connect(&url, String::new())
            .await
            .unwrap()
            .with_clock(move || base),
    );
    assert_eq!(
        call(&restarted, "/v1/users", Some(token), Value::Null)
            .await
            .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    // Recover by moving the trustworthy wall clock to/past persisted watermark.
    time.store(base + 122, Ordering::SeqCst);
    let (_, login) = call(
        &app,
        "/v1/admin/session",
        None,
        json!({"admin_secret":"persistent-admin-secret-01234567890123456789"}),
    )
    .await;
    let new_token = login["token"].as_str().unwrap();
    sqlx::query("INSERT INTO sessions(token_hash,expires_at,generation) VALUES($1,$2,$3) ON CONFLICT DO NOTHING")
        .bind(tavra_control::auth::hash(token))
        .bind(base + 999999)
        .bind(initial_generation)
        .execute(&state.pool)
        .await
        .unwrap();
    assert_eq!(
        call(&app, "/v1/users", Some(token), Value::Null).await.0,
        StatusCode::UNAUTHORIZED,
        "in-flight issuance from revoked time generation must not survive"
    );
    assert_eq!(
        call(&app, "/v1/enroll", None, proof).await.0,
        StatusCode::UNAUTHORIZED
    );
    time.store(base + 9 * 3600, Ordering::SeqCst);
    assert_eq!(
        call(&app, "/v1/users", Some(new_token), Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    time.store(base + 122, Ordering::SeqCst);
    assert_eq!(
        call(&app, "/v1/users", Some(new_token), Value::Null)
            .await
            .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    time.store(base + 9 * 3600 + 1, Ordering::SeqCst);
    assert_eq!(
        call(&app, "/v1/users", Some(new_token), Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    // A clock held fixed is not treated as safe max(now,watermark) forever.
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    let (status, error) = call(
        &app,
        "/v1/admin/session",
        None,
        json!({"admin_secret":"persistent-admin-secret-01234567890123456789"}),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(error["error"]["code"], "clock_untrusted");
    assert_eq!(
        call(&app, "/v1/health", None, Value::Null).await.0,
        StatusCode::OK
    );
    time.store(-1, Ordering::SeqCst);
    assert_eq!(
        call(&app, "/v1/users", Some(token), Value::Null).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
}
async fn proof_for(app: &Router, invite: &str) -> Value {
    use ed25519_dalek::{Signer, SigningKey};
    let key = SigningKey::from_bytes(&[42; 32]);
    let pk = hex::encode(key.verifying_key().to_bytes());
    let (_, ch) = call(
        app,
        "/v1/enroll/challenge",
        None,
        json!({"invitation":invite,"public_key":pk}),
    )
    .await;
    let payload = format!(
        "tavra-enroll-v1\n{}\n{}\n{}",
        ch["challenge_id"].as_str().unwrap(),
        ch["nonce"].as_str().unwrap(),
        pk
    );
    json!({"challenge_id":ch["challenge_id"],"public_key":pk,"signature":hex::encode(key.sign(payload.as_bytes()).to_bytes())})
}
