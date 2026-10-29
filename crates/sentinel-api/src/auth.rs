use crate::state::AppState;
use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};

pub async fn require_token(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let supplied = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    let authorized = supplied.is_some_and(|value| {
        let expected = state.api_token.as_bytes();
        let supplied = value.as_bytes();
        if supplied.len() != expected.len() {
            return false;
        }
        supplied
            .iter()
            .zip(expected)
            .fold(0_u8, |diff, (a, b)| diff | (a ^ b))
            == 0
    });
    if !authorized {
        return (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Bearer")],
            Json(serde_json::json!({"error": "A valid bearer token is required"})),
        )
            .into_response();
    }
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, routing::get, Router};
    use tower::ServiceExt;
    #[tokio::test]
    async fn rejects_missing_and_invalid_credentials() {
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .connect_lazy("sqlite::memory:")
            .unwrap();
        let state = AppState::new(db, "/tmp".into(), "a".repeat(32));
        let app = Router::new()
            .route("/private", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn_with_state(state, require_token));
        for token in [
            None,
            Some("Bearer wrong".to_owned()),
            Some(format!("Bearer {}", "a".repeat(32))),
        ] {
            let mut request = Request::builder().uri("/private");
            if let Some(value) = &token {
                request = request.header(header::AUTHORIZATION, value);
            }
            let response = app
                .clone()
                .oneshot(request.body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                if token.is_some_and(|t| t.ends_with(&"a".repeat(32))) {
                    StatusCode::OK
                } else {
                    StatusCode::UNAUTHORIZED
                }
            );
        }
    }
}
