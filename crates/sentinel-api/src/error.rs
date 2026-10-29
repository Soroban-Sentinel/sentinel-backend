use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

pub struct ApiError(anyhow::Error);

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct BadRequest(String);

impl ApiError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self(anyhow::Error::new(BadRequest(message.into())))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = if let Some(error) = self.0.downcast_ref::<BadRequest>() {
            (StatusCode::BAD_REQUEST, error.to_string())
        } else if matches!(
            self.0.downcast_ref::<sqlx::Error>(),
            Some(sqlx::Error::RowNotFound)
        ) {
            (StatusCode::NOT_FOUND, "Resource not found".to_owned())
        } else {
            tracing::error!(error = ?self.0, "API request failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_owned(),
            )
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(e: E) -> Self {
        Self(e.into())
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    #[tokio::test]
    async fn errors_have_correct_status_and_public_message() {
        let cases = [
            (
                ApiError::bad_request("Invalid config"),
                StatusCode::BAD_REQUEST,
                "Invalid config",
            ),
            (
                sqlx::Error::RowNotFound.into(),
                StatusCode::NOT_FOUND,
                "Resource not found",
            ),
            (
                anyhow::anyhow!("private database details").into(),
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error",
            ),
        ];
        for (error, status, message) in cases {
            let response = error.into_response();
            assert_eq!(response.status(), status);
            let body = to_bytes(response.into_body(), 1024).await.unwrap();
            let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(body, json!({ "error": message }));
        }
    }
}
