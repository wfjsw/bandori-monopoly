//! Session extraction from `Authorization: Bearer`, the `bm_profile` cookie,
//! or `?token=` (tools, and the browser's EventSource elsewhere).

use std::sync::Arc;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;

use crate::error::ApiError;
use crate::App;
use profile_token::Claims;

pub const COOKIE: &str = "bm_profile";

pub struct Auth(pub Claims);

fn token_from(parts: &Parts) -> Option<String> {
    if let Some(v) = parts
        .headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        if let Some(t) = v.strip_prefix("Bearer ") {
            return Some(t.trim().to_string());
        }
    }
    for v in parts.headers.get_all(axum::http::header::COOKIE) {
        let Ok(v) = v.to_str() else { continue };
        for kv in v.split(';') {
            if let Some(t) = kv.trim().strip_prefix(&format!("{COOKIE}=")) {
                return Some(t.to_string());
            }
        }
    }
    parts.uri.query()?.split('&').find_map(|kv| kv.strip_prefix("token=")).map(str::to_string)
}

impl FromRequestParts<Arc<App>> for Auth {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, app: &Arc<App>) -> Result<Self, Self::Rejection> {
        let token = token_from(parts).ok_or_else(ApiError::unauthorized)?;
        let claims = profile_token::verify(&app.secret, &token).map_err(|_| ApiError::unauthorized())?;
        Ok(Auth(claims))
    }
}
