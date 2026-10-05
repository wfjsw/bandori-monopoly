//! API errors: an HTTP status, an optional machine-readable reason code, and the
//! player-facing message as a localizable [`Msg`] (the client renders it).

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use game_core::msg::Msg;
use game_core::net;

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub reason: Option<&'static str>,
    pub message: Msg,
}

pub type ApiResult<T> = Result<T, ApiError>;

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<Msg>) -> Self {
        Self { status, reason: None, message: message.into() }
    }

    /// 400 -- the request is understood but not allowed right now.
    pub fn bad(message: impl Into<Msg>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "err.unauthorized")
    }

    pub fn not_found(message: impl Into<Msg>) -> Self {
        Self::new(StatusCode::NOT_FOUND, message)
    }

    pub fn forbidden(message: impl Into<Msg>) -> Self {
        Self::new(StatusCode::FORBIDDEN, message)
    }

    /// Joining was refused (`NetProtocol.Reject*`), with the original message.
    pub fn reject(code: &'static str) -> Self {
        let status = if code == net::reject::PASSWORD { StatusCode::FORBIDDEN } else { StatusCode::CONFLICT };
        Self { status, reason: Some(code), message: net::describe(code) }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = serde_json::json!({ "error": self.message, "reason": self.reason });
        (self.status, Json(body)).into_response()
    }
}
