use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use draugr_time_core::Storage;
use draugr_time_daemon::{api::router, Shared};
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

#[tokio::test]
async fn computer_state_and_diagnostic_evidence_are_available_via_local_api() {
    use draugr_time_daemon::Tracker;
    use draugr_time_watchers::{MockWatcher, WatcherError};
    let shared = Shared::new(Storage::open_in_memory().unwrap(), "test-token".into(), 300);
    let mut tracker = Tracker::new(shared.clone());
    let watcher = MockWatcher {
        window: Err(WatcherError::PermissionDenied("window title")),
        idle: Ok(0),
    };
    for ts in [1000, 2000] {
        tracker.poll_once(ts, &watcher, &watcher).unwrap();
    }
    let app = router(shared);
    for path in ["/status", "/events?bucket=computer&from=0&to=3000"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header("Authorization", "Bearer test-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let value: Value = serde_json::from_slice(&body).unwrap();
        let computer = if path == "/status" {
            &value["computer"]
        } else {
            &value[0]["data"]
        };
        assert_eq!(computer["machine"], "awake");
        assert_eq!(computer["activity"], "active");
        assert_eq!(computer["recording"], "unobserved");
        assert!(computer["reason"].as_str().unwrap().contains("window"));
        assert_eq!(computer["source"], "running process");
    }
}
