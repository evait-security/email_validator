//! API server module.
//!
//! Provides an HTTP API for email validation via axum.
//! Three endpoints: `POST /validate`, `GET /validate`, `GET /health`.

use axum::{
    Router,
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use crate::Method;
use crate::precheck;
use crate::validation;
use crate::validation::ValidationResult;

// ── Request / Response types ───────────────────────────────────────

/// POST /validate body
#[derive(Deserialize)]
struct ValidateRequest {
    emails: Vec<String>,
    #[serde(default)]
    method: Option<Method>,
    #[serde(default)]
    disable_wildcard: Option<bool>,
}

/// GET /validate query params
#[derive(Deserialize)]
struct SingleValidateQuery {
    email: String,
    #[serde(default)]
    method: Option<Method>,
    #[serde(default)]
    disable_wildcard: Option<bool>,
}

/// GET /health response
#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
}

/// Error response (400, 422, etc.)
#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

// ── Shared state ───────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    verbose: bool,
}

// ── Server entry point ─────────────────────────────────────────────

/// Start the HTTP API server.
pub async fn run(bind_addr: &str, verbose: bool) {
    let state = AppState { verbose };

    let app = Router::new()
        .route("/validate", post(validate_batch).get(validate_single))
        .route("/health", get(health))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(bind_addr).await.unwrap();
    eprintln!("API server listening on http://{bind_addr}");
    axum::serve(listener, app).await.unwrap();
}

// ── Handlers ──────────────────────────────────────────────────────

async fn validate_batch(
    State(state): State<AppState>,
    Json(request): Json<ValidateRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    // 1. Leere Liste? → 400
    if request.emails.is_empty() {
        return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse {
            error: "email list is empty".into(),
        })));
    }

    // 2. Batch-Limit? → 400 (SOFORT, vor jeglicher Verarbeitung!)
    if request.emails.len() > 1000 {
        return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse {
            error: "too many emails, max 1000".into(),
        })));
    }

    // 3. Deduplizieren (case-insensitive, consistent with ingestion.rs)
    let mut emails = request.emails;
    emails = emails.into_iter().map(|e| e.to_lowercase()).collect();
    emails.sort();
    emails.dedup();

    // 4. Defaults
    let method = request.method.unwrap_or(Method::Smtp);
    let disable_wildcard = request.disable_wildcard.unwrap_or(false);
    let is_quiet = !state.verbose;

    // 5-6. Pipeline
    let wildcard_domains = precheck::run(method, disable_wildcard, state.verbose, &emails, is_quiet, Some(25)).await;
    let results = validation::run(method, disable_wildcard, &emails, &wildcard_domains, is_quiet, Some(25)).await;

    Ok(Json(build_response(&results)))
}

async fn validate_single(
    State(state): State<AppState>,
    Query(query): Query<SingleValidateQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let email = query.email.trim();
    if email.is_empty() {
        return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse {
            error: "email parameter is required".into(),
        })));
    }

    let method = query.method.unwrap_or(Method::Smtp);
    let disable_wildcard = query.disable_wildcard.unwrap_or(false);
    let is_quiet = !state.verbose;
    let emails = vec![email.to_string()];

    let wildcard_domains = precheck::run(method, disable_wildcard, state.verbose, &emails, is_quiet, Some(25)).await;
    let results = validation::run(method, disable_wildcard, &emails, &wildcard_domains, is_quiet, Some(25)).await;

    Ok(Json(build_response(&results)))
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".into(),
        version: env!("CARGO_PKG_VERSION").into(),
    })
}

// ── Helpers ────────────────────────────────────────────────────────

fn build_response(results: &[ValidationResult]) -> serde_json::Value {
    let valid_count = results.iter().filter(|r| r.valid).count();
    let invalid_count = results.len() - valid_count;
    let catch_all_count = results.iter().filter(|r| r.catch_all).count();

    json!({
        "total": results.len(),
        "valid_count": valid_count,
        "invalid_count": invalid_count,
        "catch_all_count": catch_all_count,
        "results": results,
    })
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    /// Helper: build the test app
    fn test_app() -> Router {
        let state = AppState { verbose: false };
        Router::new()
            .route("/validate", post(validate_batch).get(validate_single))
            .route("/health", get(health))
            .with_state(state)
    }

    #[tokio::test]
    async fn test_health() {
        let app = test_app();
        let req = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
        assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
    }

    #[tokio::test]
    async fn test_validate_single_regex() {
        let app = test_app();
        let req = Request::builder()
            .uri("/validate?email=test@example.com&method=regex")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["total"], 1);
        assert_eq!(json["valid_count"], 1);
        assert_eq!(json["results"][0]["email"], "test@example.com");
        assert_eq!(json["results"][0]["valid"], true);
    }

    #[tokio::test]
    async fn test_validate_single_missing_email() {
        let app = test_app();
        let req = Request::builder()
            .uri("/validate")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_validate_batch_empty() {
        let app = test_app();
        let req = Request::builder()
            .uri("/validate")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"emails":[]}"#))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "email list is empty");
    }

    #[tokio::test]
    async fn test_validate_batch_too_many() {
        let app = test_app();
        let emails: Vec<String> = (0..1001).map(|i| format!("x{i}@y.com")).collect();
        let body_str = serde_json::json!({"emails": emails}).to_string();
        let req = Request::builder()
            .uri("/validate")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(body_str))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_validate_batch_ok() {
        let app = test_app();
        let req = Request::builder()
            .uri("/validate")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"emails":["a@b.de","c@d.com"],"method":"regex"}"#))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["total"], 2);
        assert_eq!(json["valid_count"], 2);
    }

    #[tokio::test]
    async fn test_validate_batch_malformed_json() {
        let app = test_app();
        let req = Request::builder()
            .uri("/validate")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from("not json"))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_validate_batch_regex_flags_invalid_emails() {
        // Verifies that bogus emails are marked valid: false
        let app = test_app();
        let req = Request::builder()
            .uri("/validate")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"emails":["a@b.de","bogus","c@d.com"],"method":"regex"}"#))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["total"], 3);
        assert_eq!(json["valid_count"], 2);
        assert_eq!(json["invalid_count"], 1);

        let results = json["results"].as_array().unwrap();
        let valid_emails: Vec<&str> = results.iter()
            .filter(|r| r["valid"].as_bool().unwrap())
            .map(|r| r["email"].as_str().unwrap())
            .collect();
        assert_eq!(valid_emails, vec!["a@b.de", "c@d.com"]);
    }

    #[tokio::test]
    async fn test_validate_single_regex_invalid_email() {
        let app = test_app();
        let req = Request::builder()
            .uri("/validate?email=bogus&method=regex")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["total"], 1);
        assert_eq!(json["valid_count"], 0);
        assert_eq!(json["invalid_count"], 1);
        assert_eq!(json["results"][0]["valid"], false);
    }

    #[tokio::test]
    async fn test_validate_batch_deduplication() {
        // Duplicates should be collapsed, valid_count should not double-count
        let app = test_app();
        let req = Request::builder()
            .uri("/validate")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"emails":["a@b.de","a@b.de","A@B.DE"],"method":"regex"}"#))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["total"], 1, "duplicates should be collapsed");
        assert_eq!(json["valid_count"], 1);
    }
}