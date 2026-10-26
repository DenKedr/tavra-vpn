use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tavra_control::{AppState, router};
use tower::ServiceExt;

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut r = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(t) = token {
        r = r.header("authorization", format!("Bearer {t}"));
    }
    let response = app
        .clone()
        .oneshot(r.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

// A dedicated empty PostgreSQL database is mandatory. Never silently skip database tests.
#[tokio::test]
async fn persistent_authorization_and_device_policy() {
    let url =
        std::env::var("TAVRA_TEST_DATABASE_URL").expect("set isolated TAVRA_TEST_DATABASE_URL");
    let state = AppState::connect(&url, "bootstrap-secret-for-integration-test-0001".into())
        .await
        .unwrap();
    sqlx::query(
        "TRUNCATE admins, sessions, users, subscriptions, challenges, devices, servers CASCADE",
    )
    .execute(&state.pool)
    .await
    .unwrap();
    sqlx::query("UPDATE security_clock SET high_water=0")
        .execute(&state.pool)
        .await
        .unwrap();
    let test_time = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(state.now()));
    let state = state.with_clock({
        let time = test_time.clone();
        move || time.load(std::sync::atomic::Ordering::SeqCst)
    });
    let app = router(state.clone());
    assert_eq!(
        call(&app, "GET", "/v1/users", None, json!({})).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (status, session) = call(&app,"POST","/v1/admin/bootstrap",None,json!({"bootstrap_secret":"bootstrap-secret-for-integration-test-0001","admin_secret":"persistent-admin-secret-01234567890123456789"})).await;
    assert_eq!(status, StatusCode::OK);
    let token = session["token"].as_str().unwrap();
    assert_eq!(call(&app,"POST","/v1/admin/bootstrap",None,json!({"bootstrap_secret":"bootstrap-secret-for-integration-test-0001","admin_secret":"another-secret-012345678901234567890"})).await.0, StatusCode::CONFLICT);
    let (_, user) = call(
        &app,
        "POST",
        "/v1/users",
        Some(token),
        json!({"first_name":"Denis","last_name":"","phone":"","email":""}),
    )
    .await;
    assert!(user["id"].is_string(), "{user}");
    let (_, sub) = call(
        &app,
        "POST",
        "/v1/subscriptions",
        Some(token),
        json!({"user_id":user["id"],"expires_at":0,"device_limit":1}),
    )
    .await;
    let invite = sub["invitation"].as_str().unwrap();
    use ed25519_dalek::{Signer, SigningKey};
    let key = SigningKey::from_bytes(&[7; 32]);
    let pk = hex::encode(key.verifying_key().to_bytes());
    let (_, ch) = call(
        &app,
        "POST",
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
    let proof = json!({"challenge_id":ch["challenge_id"],"public_key":pk,"signature":hex::encode(key.sign(payload.as_bytes()).to_bytes())});
    let (status, device) = call(&app, "POST", "/v1/enroll", None, proof.clone()).await;
    assert_eq!(status, StatusCode::OK, "{device}");
    assert_eq!(
        call(&app, "POST", "/v1/enroll", None, proof).await.0,
        StatusCode::UNAUTHORIZED
    );
    // Fresh challenges permit idempotent registration but never permit a second key.
    let (_, retry_ch) = call(
        &app,
        "POST",
        "/v1/enroll/challenge",
        None,
        json!({"invitation":invite,"public_key":pk}),
    )
    .await;
    let retry_payload = format!(
        "tavra-enroll-v1\n{}\n{}\n{}",
        retry_ch["challenge_id"].as_str().unwrap(),
        retry_ch["nonce"].as_str().unwrap(),
        pk
    );
    let (_, retry_device) = call(&app,"POST","/v1/enroll",None,json!({"challenge_id":retry_ch["challenge_id"],"public_key":pk,"signature":hex::encode(key.sign(retry_payload.as_bytes()).to_bytes())})).await;
    assert_eq!(device["device_id"], retry_device["device_id"]);
    let key2 = SigningKey::from_bytes(&[9; 32]);
    let pk2 = hex::encode(key2.verifying_key().to_bytes());
    let (_, ch2) = call(
        &app,
        "POST",
        "/v1/enroll/challenge",
        None,
        json!({"invitation":invite,"public_key":pk2}),
    )
    .await;
    let payload2 = format!(
        "tavra-enroll-v1\n{}\n{}\n{}",
        ch2["challenge_id"].as_str().unwrap(),
        ch2["nonce"].as_str().unwrap(),
        pk2
    );
    assert_eq!(call(&app,"POST","/v1/enroll",None,json!({"challenge_id":ch2["challenge_id"],"public_key":pk2,"signature":hex::encode(key2.sign(payload2.as_bytes()).to_bytes())})).await.0,StatusCode::CONFLICT);
    assert_eq!(
        call(
            &app,
            "POST",
            "/v1/users",
            Some(token),
            json!({"first_name":"x".repeat(121),"last_name":"","email":"","phone":""})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/v1/admin/session",
            None,
            json!({"admin_secret":"wrong-key-012345678901234567890123456"})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&app, "GET", "/v1/devices", None, json!({})).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &app,
            "DELETE",
            "/v1/users/not-a-uuid",
            Some(token),
            json!({})
        )
        .await
        .1["error"]["code"],
        "invalid_input"
    );
    let (_, race_sub) = call(
        &app,
        "POST",
        "/v1/subscriptions",
        Some(token),
        json!({"user_id":user["id"],"expires_at":0,"device_limit":1}),
    )
    .await;
    let p1 = proof_for(&app, race_sub["invitation"].as_str().unwrap(), 11).await;
    let p2 = proof_for(&app, race_sub["invitation"].as_str().unwrap(), 12).await;
    let (r1, r2) = tokio::join!(
        call(&app, "POST", "/v1/enroll", None, p1),
        call(&app, "POST", "/v1/enroll", None, p2)
    );
    let statuses = [r1.0, r2.0];
    assert_eq!(statuses.iter().filter(|&&v| v == StatusCode::OK).count(), 1);
    assert_eq!(
        statuses
            .iter()
            .filter(|&&v| v == StatusCode::CONFLICT)
            .count(),
        1
    );
    let (_, expired_sub) = call(
        &app,
        "POST",
        "/v1/subscriptions",
        Some(token),
        json!({"user_id":user["id"],"expires_at":1,"device_limit":0}),
    )
    .await;
    assert_eq!(
        call(
            &app,
            "POST",
            "/v1/enroll/challenge",
            None,
            json!({"invitation":expired_sub["invitation"],"public_key":pk})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let (_, unlimited) = call(
        &app,
        "POST",
        "/v1/subscriptions",
        Some(token),
        json!({"user_id":user["id"],"expires_at":0,"device_limit":0}),
    )
    .await;
    let unlimited_invite = unlimited["invitation"].as_str().unwrap();
    for seed in [13, 14] {
        let p = proof_for(&app, unlimited_invite, seed).await;
        assert_eq!(
            call(&app, "POST", "/v1/enroll", None, p).await.0,
            StatusCode::OK
        );
    }
    let stale = proof_for(&app, unlimited_invite, 15).await;
    let future = router(state.clone().with_clock({
        let now = state.now();
        move || now + 121
    }));
    assert_eq!(
        call(&future, "POST", "/v1/enroll", None, stale).await.0,
        StatusCode::UNAUTHORIZED
    );
    test_time.fetch_add(121, std::sync::atomic::Ordering::SeqCst);
    let stale = proof_for(&app, unlimited_invite, 16).await;
    assert_eq!(
        call(
            &app,
            "POST",
            &format!(
                "/v1/subscriptions/{}/rotate",
                unlimited["id"].as_str().unwrap()
            ),
            Some(token),
            json!({})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&app, "POST", "/v1/enroll", None, stale).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/v1/enroll/challenge",
            None,
            json!({"invitation":unlimited_invite,"public_key":pk})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &app,
            "PATCH",
            &format!("/v1/users/{}", user["id"].as_str().unwrap()),
            Some(token),
            json!({"status":"blocked"})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/v1/enroll/challenge",
            None,
            json!({"invitation":invite,"public_key":pk})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &app,
            "PATCH",
            &format!("/v1/users/{}", user["id"].as_str().unwrap()),
            Some(token),
            json!({"status":"active"})
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, fresh) = call(
        &app,
        "POST",
        "/v1/subscriptions",
        Some(token),
        json!({"user_id":user["id"],"expires_at":0,"device_limit":0}),
    )
    .await;
    let fresh_invite = fresh["invitation"].as_str().unwrap();
    let mut invalid = proof_for(&app, fresh_invite, 21).await;
    let original = invalid.clone();
    invalid["signature"] = json!("00".repeat(64));
    assert_eq!(
        call(&app, "POST", "/v1/enroll", None, invalid).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&app, "POST", "/v1/enroll", None, original).await.0,
        StatusCode::UNAUTHORIZED
    );
    let original = proof_for(&app, fresh_invite, 22).await;
    let (_, registered) = call(&app, "POST", "/v1/enroll", None, original).await;
    assert_eq!(
        call(
            &app,
            "DELETE",
            &format!("/v1/devices/{}", registered["device_id"].as_str().unwrap()),
            Some(token),
            json!({})
        )
        .await
        .0,
        StatusCode::OK
    );
    let retry = proof_for(&app, fresh_invite, 22).await;
    assert_eq!(
        call(&app, "POST", "/v1/enroll", None, retry).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(
            &app,
            "PATCH",
            &format!("/v1/subscriptions/{}", fresh["id"].as_str().unwrap()),
            Some(token),
            json!({"status":"blocked"})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/v1/enroll/challenge",
            None,
            json!({"invitation":fresh_invite,"public_key":pk})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let (_, server) = call(
        &app,
        "POST",
        "/v1/servers",
        Some(token),
        json!({"name":"Inventory","host":"127.0.0.1","ssh_port":22,"username":"root"}),
    )
    .await;
    assert_eq!(server["status"], "not_provisioned");
    let (_, overview) = call(&app, "GET", "/v1/overview", Some(token), json!({})).await;
    assert_eq!(overview["users"], 1);
    assert_eq!(overview["servers"], 1);
    let stored: Vec<u8> =
        sqlx::query_scalar("SELECT invitation_hash FROM subscriptions WHERE id=$1")
            .bind(uuid::Uuid::parse_str(fresh["id"].as_str().unwrap()).unwrap())
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert_eq!(stored, tavra_control::auth::hash(fresh_invite));
    let restarted = router(
        AppState::connect(&url, "different-bootstrap-secret-012345678901".into())
            .await
            .unwrap()
            .with_clock({
                let time = test_time.clone();
                move || time.load(std::sync::atomic::Ordering::SeqCst)
            }),
    );
    let (status, login) = call(
        &restarted,
        "POST",
        "/v1/admin/session",
        None,
        json!({"admin_secret":"persistent-admin-secret-01234567890123456789"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(login["token"].is_string());
    let (_, list) = call(
        &restarted,
        "GET",
        "/v1/subscriptions",
        Some(token),
        json!({}),
    )
    .await;
    assert_eq!(list.as_array().unwrap().len(), 5);
    assert!(!list.to_string().contains(invite));
    assert_eq!(
        call(
            &app,
            "POST",
            &format!("/v1/subscriptions/{}/revoke", sub["id"].as_str().unwrap()),
            Some(token),
            json!({})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/v1/enroll/challenge",
            None,
            json!({"invitation":invite,"public_key":pk})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let dead_session = router(state.clone().with_clock({
        let now = state.now();
        move || now + 9 * 3600
    }));
    assert_eq!(
        call(&dead_session, "GET", "/v1/users", Some(token), json!({}))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

async fn proof_for(app: &Router, invitation: &str, seed: u8) -> Value {
    use ed25519_dalek::{Signer, SigningKey};
    let key = SigningKey::from_bytes(&[seed; 32]);
    let pk = hex::encode(key.verifying_key().to_bytes());
    let (status, ch) = call(
        app,
        "POST",
        "/v1/enroll/challenge",
        None,
        json!({"invitation":invitation,"public_key":pk}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{ch}");
    let payload = format!(
        "tavra-enroll-v1\n{}\n{}\n{}",
        ch["challenge_id"].as_str().unwrap(),
        ch["nonce"].as_str().unwrap(),
        pk
    );
    json!({"challenge_id":ch["challenge_id"],"public_key":pk,"signature":hex::encode(key.sign(payload.as_bytes()).to_bytes())})
}
