use crate::Shared;
use axum::{
    extract::{DefaultBodyLimit, Path, Query, Request, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use dagdatime_core::{Collection, StorageError};
use serde::Deserialize;
use serde_json::{json, Value};

type ApiResult = Result<Json<Value>, (StatusCode, Json<Value>)>;
fn error(status: StatusCode, message: impl ToString) -> (StatusCode, Json<Value>) {
    (status, Json(json!({"error":message.to_string()})))
}
fn storage_error(error_value: StorageError) -> (StatusCode, Json<Value>) {
    match error_value {
        StorageError::NotFound => error(StatusCode::NOT_FOUND, error_value),
        StorageError::Sqlite(rusqlite_error) => error(StatusCode::BAD_REQUEST, rusqlite_error),
        _ => error(StatusCode::BAD_REQUEST, error_value),
    }
}
fn collection(name: &str) -> Result<Collection, (StatusCode, Json<Value>)> {
    Collection::parse(name).ok_or_else(|| error(StatusCode::NOT_FOUND, "unknown collection"))
}

pub fn router(shared: Shared) -> Router {
    Router::new()
        .route("/status", get(status))
        .route("/events", get(events))
        .route("/pause", post(pause))
        .route("/resume", post(resume))
        .route("/shutdown", post(shutdown))
        .route("/{collection}", get(list).post(create))
        .route("/{collection}/{id}", get(one).put(update).delete(delete))
        .layer(DefaultBodyLimit::max(65_536))
        .layer(middleware::from_fn_with_state(shared.clone(), authorize))
        .with_state(shared)
}

async fn authorize(State(shared): State<Shared>, request: Request, next: Next) -> Response {
    let expected = format!("Bearer {}", shared.token);
    let authorized = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
        == Some(expected.as_str());
    let origin_allowed = request.headers().get(header::ORIGIN).is_none();
    if !authorized || !origin_allowed {
        return error(StatusCode::UNAUTHORIZED, "local API authorization required").into_response();
    }
    next.run(request).await
}
async fn status(State(shared): State<Shared>) -> Json<Value> {
    Json(serde_json::to_value(shared.status()).unwrap())
}

#[derive(Deserialize)]
struct EventsQuery {
    bucket: Option<String>,
    from: i64,
    to: i64,
}
async fn events(State(shared): State<Shared>, Query(query): Query<EventsQuery>) -> ApiResult {
    let events = shared
        .storage
        .lock()
        .unwrap()
        .events(query.bucket.as_deref(), query.from, query.to)
        .map_err(storage_error)?;
    Ok(Json(json!(events)))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pause {
    duration_ms: Option<i64>,
}
async fn pause(State(shared): State<Shared>, Json(body): Json<Pause>) -> ApiResult {
    shared
        .pause(crate::now_ms(), body.duration_ms)
        .map_err(|message| error(StatusCode::BAD_REQUEST, message))?;
    Ok(Json(json!(shared.status())))
}
async fn resume(State(shared): State<Shared>) -> Json<Value> {
    shared.resume();
    Json(json!(shared.status()))
}
async fn shutdown(State(shared): State<Shared>) -> Json<Value> {
    shared.control.lock().unwrap().shutdown = true;
    Json(json!({"ok":true}))
}
async fn list(State(shared): State<Shared>, Path(name): Path<String>) -> ApiResult {
    Ok(Json(json!(shared
        .storage
        .lock()
        .unwrap()
        .list(collection(&name)?)
        .map_err(storage_error)?)))
}
async fn one(State(shared): State<Shared>, Path((name, id)): Path<(String, i64)>) -> ApiResult {
    Ok(Json(
        shared
            .storage
            .lock()
            .unwrap()
            .get(collection(&name)?, id)
            .map_err(storage_error)?,
    ))
}
async fn create(
    State(shared): State<Shared>,
    Path(name): Path<String>,
    Json(body): Json<Value>,
) -> ApiResult {
    Ok(Json(
        shared
            .storage
            .lock()
            .unwrap()
            .put(collection(&name)?, None, body)
            .map_err(storage_error)?,
    ))
}
async fn update(
    State(shared): State<Shared>,
    Path((name, id)): Path<(String, i64)>,
    Json(body): Json<Value>,
) -> ApiResult {
    Ok(Json(
        shared
            .storage
            .lock()
            .unwrap()
            .put(collection(&name)?, Some(id), body)
            .map_err(storage_error)?,
    ))
}
async fn delete(State(shared): State<Shared>, Path((name, id)): Path<(String, i64)>) -> ApiResult {
    if !shared
        .storage
        .lock()
        .unwrap()
        .delete(collection(&name)?, id)
        .map_err(storage_error)?
    {
        return Err(error(StatusCode::NOT_FOUND, "record not found"));
    }
    Ok(Json(json!({"ok":true})))
}
