//! API errors in the game server's wire shape: `{error, reason?}`, where
//! `error` is a localizable message key.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use game_core::msg::Msg;

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub reason: Option<&'static str>,
    pub message: Msg,
}

pub type ApiResult<T> = Result<T, ApiError>;

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<Msg>) -> Self {
        Self {
            status,
            reason: None,
            message: message.into(),
        }
    }

    pub fn bad(message: impl Into<Msg>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "err.unauthorized")
    }

    pub fn not_found(message: impl Into<Msg>) -> Self {
        Self::new(StatusCode::NOT_FOUND, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            Msg::new("err.internal").text("detail", message.into()),
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = serde_json::json!({
            "error": self.message,
            "reason": self.reason,
        });
        (self.status, Json(body)).into_response()
    }
}
