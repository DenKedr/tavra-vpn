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
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    let response = app
        .clone()
        .oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).expect("API must return typed JSON"),
    )
}
#[tokio::test]
async fn inventory_crud_preserves_domain_assignment_boundary_and_audit() {
    let url = std::env::var("TAVRA_TEST_DATABASE_URL").expect("isolated database required");
    let state = AppState::connect(&url, "bootstrap-inventory-integration-secret-0001".into())
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
    let app = router(state.clone());
    let (_,login)=call(&app,"POST","/v1/admin/bootstrap",None,json!({"bootstrap_secret":"bootstrap-inventory-integration-secret-0001","admin_secret":"inventory-admin-key-01234567890123456789"})).await;
    let token = login["token"].as_str().unwrap();
    assert_eq!(
        call(
            &app,
            "POST",
            "/v1/domains",
            None,
            json!({"name":"example.com"})
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
        json!({"name":"Node A","host":"127.0.0.1","ssh_port":22,"username":"root"}),
    )
    .await;
    let server_path = format!("/v1/servers/{}", server["id"].as_str().unwrap());
    let (status, edited) = call(
        &app,
        "PATCH",
        &server_path,
        Some(token),
        json!({"name":"Node B","ssh_port":2222}),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "server inventory PATCH missing: {edited}"
    );
    assert_eq!(edited["name"], "Node B");
    assert_eq!(edited["ssh_port"], 2222);
    assert_eq!(edited["host"], "127.0.0.1");
    assert_eq!(edited["status"], "not_provisioned");
    assert_eq!(
        call(
            &app,
            "PATCH",
            &server_path,
            Some(token),
            json!({"host":"https://u:p@example.com"})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &app,
            "PATCH",
            &server_path,
            Some(token),
            json!({"name":null})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, domain) = call(
        &app,
        "POST",
        "/v1/domains",
        Some(token),
        json!({"name":"ПРИМЕР.РФ.","server_id":server["id"]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{domain}");
    assert_eq!(domain["name"], "xn--e1afmkfd.xn--p1ai");
    assert_eq!(domain["verification_status"], "unverified");
    assert_eq!(domain["server_id"], server["id"]);
    let domain_path = format!("/v1/domains/{}", domain["id"].as_str().unwrap());
    assert_eq!(
        call(
            &app,
            "PATCH",
            &domain_path,
            Some(token),
            json!({"name":null})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/v1/domains",
            Some(token),
            json!({"name":"xn--e1afmkfd.xn--p1ai"})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/v1/domains",
            Some(token),
            json!({"name":"other.example","server_id":uuid::Uuid::new_v4()})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    for invalid in [
        "127.0.0.1",
        "::1",
        "127.1",
        "0x7f000001",
        "*.example.com",
        "https://example.com",
        "user:password@example.com",
        "bad..example",
        "_tcp.example.com",
        "-bad.example",
        "localhost",
        "example.com/path",
        "example.com:443",
        "percent%2eexample.com",
        " bad.example",
        "bad.example ",
    ] {
        assert_eq!(
            call(
                &app,
                "POST",
                "/v1/domains",
                Some(token),
                json!({"name":invalid})
            )
            .await
            .0,
            StatusCode::BAD_REQUEST,
            "accepted {invalid}"
        );
    }
    let (status, renamed) = call(
        &app,
        "PATCH",
        &domain_path,
        Some(token),
        json!({"name":"NEW.Example.COM","server_id":null}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(renamed["name"], "new.example.com");
    assert!(renamed["server_id"].is_null());
    assert_eq!(
        call(
            &app,
            "PATCH",
            &domain_path,
            Some(token),
            json!({"server_id":server["id"]})
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, removed) = call(&app, "DELETE", &server_path, Some(token), json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(removed["deleted"], true);
    assert_eq!(removed["remote_uninstall_performed"], false);
    assert_eq!(removed["unassigned_domain_ids"], json!([domain["id"]]));
    let (_, domains) = call(&app, "GET", "/v1/domains", Some(token), json!({})).await;
    assert_eq!(domains.as_array().unwrap().len(), 1);
    assert!(domains[0]["server_id"].is_null());
    assert_eq!(domains[0]["verification_status"], "unverified");
    let reconnect = router(AppState::connect(&url, String::new()).await.unwrap());
    let (_, domains) = call(&reconnect, "GET", "/v1/domains", Some(token), json!({})).await;
    assert_eq!(domains[0]["name"], "new.example.com");
    assert_eq!(
        call(&app, "DELETE", &domain_path, Some(token), json!({}))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&app, "DELETE", &domain_path, Some(token), json!({}))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let server_id = uuid::Uuid::parse_str(server["id"].as_str().unwrap()).unwrap();
    let domain_id = uuid::Uuid::parse_str(domain["id"].as_str().unwrap()).unwrap();
    let actions: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM inventory_audit WHERE entity_id=$1 ORDER BY sequence",
    )
    .bind(server_id)
    .fetch_all(&state.pool)
    .await
    .unwrap();
    assert_eq!(actions, ["created", "updated", "deleted"]);
    let actions: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM inventory_audit WHERE entity_id=$1 ORDER BY sequence",
    )
    .bind(domain_id)
    .fetch_all(&state.pool)
    .await
    .unwrap();
    assert_eq!(
        actions,
        [
            "created",
            "updated",
            "updated",
            "server_unassigned",
            "deleted"
        ]
    );
    let before: Value = sqlx::query_scalar(
        "SELECT before_state FROM inventory_audit WHERE entity_id=$1 AND action='deleted'",
    )
    .bind(server_id)
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert_eq!(before["name"], "Node B");
    let (_, race_server) = call(
        &app,
        "POST",
        "/v1/servers",
        Some(token),
        json!({"name":"Race","host":"::1","ssh_port":22,"username":"admin"}),
    )
    .await;
    let (_, race_domain) = call(
        &app,
        "POST",
        "/v1/domains",
        Some(token),
        json!({"name":"race.example.com","server_id":race_server["id"]}),
    )
    .await;
    let race_server_path = format!("/v1/servers/{}", race_server["id"].as_str().unwrap());
    let race_domain_path = format!("/v1/domains/{}", race_domain["id"].as_str().unwrap());
    let (removed, reassigned) = tokio::join!(
        call(&app, "DELETE", &race_server_path, Some(token), json!({})),
        call(
            &app,
            "PATCH",
            &race_domain_path,
            Some(token),
            json!({"server_id":race_server["id"]})
        )
    );
    assert_eq!(removed.0, StatusCode::OK);
    assert!([StatusCode::OK, StatusCode::BAD_REQUEST].contains(&reassigned.0));
    let (_, after_race) = call(&app, "GET", "/v1/domains", Some(token), json!({})).await;
    assert!(after_race[0]["server_id"].is_null());
    let (first, second) = tokio::join!(
        call(
            &app,
            "POST",
            "/v1/domains",
            Some(token),
            json!({"name":"same.example.com"})
        ),
        call(
            &app,
            "POST",
            "/v1/domains",
            Some(token),
            json!({"name":"SAME.EXAMPLE.COM."})
        )
    );
    assert_eq!(
        [first.0, second.0]
            .iter()
            .filter(|&&v| v == StatusCode::OK)
            .count(),
        1
    );
    assert_eq!(
        [first.0, second.0]
            .iter()
            .filter(|&&v| v == StatusCode::CONFLICT)
            .count(),
        1
    );
}
