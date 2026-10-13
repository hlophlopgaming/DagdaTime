use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use dagdatime_core::Storage;
use dagdatime_daemon::{api::router, Shared};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

#[tokio::test]
async fn authentication_crud_and_pause_status() {
    let shared = Shared::new(Storage::open_in_memory().unwrap(), "token".into(), 300);
    let app = router(shared);
    let unauth = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauth.status(), StatusCode::UNAUTHORIZED);
    async fn call(app: axum::Router, method: &str, path: &str, data: Value) -> (StatusCode, Value) {
        let response = app
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("authorization", "Bearer token")
                    .header("content-type", "application/json")
                    .body(Body::from(data.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let data =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        (status, data)
    }
    let (status, tag) = call(app.clone(), "POST", "/tags", json!({"name":"Work"})).await;
    assert_eq!(status, StatusCode::OK);
    let path = format!("/tags/{}", tag["id"]);
    assert_eq!(
        call(app.clone(), "GET", &path, json!(null)).await.1["name"],
        "Work"
    );
    assert_eq!(
        call(app.clone(), "PUT", &path, json!({"name":"Client"}))
            .await
            .1["name"],
        "Client"
    );
    assert_eq!(
        call(app.clone(), "POST", "/pause", json!({"duration_ms":60_000}))
            .await
            .1["state"],
        "paused"
    );
    assert_eq!(
        call(app.clone(), "GET", "/status", json!(null)).await.1["state"],
        "paused"
    );
    assert_eq!(
        call(app.clone(), "POST", "/resume", json!(null)).await.1["state"],
        "starting"
    );
    assert_eq!(
        call(app.clone(), "DELETE", &path, json!(null)).await.0,
        StatusCode::OK
    );
    assert_eq!(
        call(app.clone(), "GET", &path, json!(null)).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(app.clone(), "GET", "/events?from=10&to=1", json!(null))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let response = app
        .oneshot(
            Request::builder()
                .uri("/status")
                .header("authorization", "Bearer token")
                .header("origin", "https://evil.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
