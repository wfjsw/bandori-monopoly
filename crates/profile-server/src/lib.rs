//! The user profile server.
//!
//! It is **not an identity provider**. There is no password, no OAuth and no
//! external account: a player types a display name and gets an account. A real
//! IdP (Authentik / Zitadel / ...) can be placed in front later; the session
//! ticket is the seam it would plug into. Player state and the game-server
//! directory live here, apart from any game server:
//!
//! | Method | Path | Body | Response |
//! |---|---|---|---|
//! | GET    | `/api/health` | | `{service, version}` |
//! | POST   | `/api/session` | `{player, persona?, cnId?}` | `{token, player, account}` + cookie |
//! | GET    | `/api/session` | | `{token, player, account}` |
//! | POST   | `/api/logout` | | `{ok}` |
//! | GET    | `/api/profile` | | `{profile}` (`profile` is the opaque client document) |
//! | PUT    | `/api/profile` | `{profile}` | `{ok}` |
//! | GET    | `/api/servers` | | `GameServer[]` |
//! | POST   | `/api/servers` | `{name, url, note?}` | `GameServer` |
//! | DELETE | `/api/servers/{id}` | | `{ok}` |
//! | GET    | `/api/uploads` | | `Upload[]` |
//! | POST   | `/api/uploads?name=&kind=` | raw bytes | `Upload` |
//! | DELETE | `/api/uploads/{id}` | | `{ok}` |
//! | GET    | `/uploads/{id}/{file}` | | the bytes |
//!
//! Sessions are stateless HMAC tickets (`profile-token`), scoped to **this**
//! service. A game server never sees one: the client opens its own session on
//! each game server by display name. The name is the only identifier shared
//! between the two services.
//!
//! Uploads carry game data (a rule build, tables, a ruleset archive) so a
//! player can hand a server owner something to install, or share it with
//! other players. They are stored opaquely and served back at `/uploads/`.
//!
//! The profile document is opaque: this service stores and returns it, the
//! client owns its shape. The web client keeps its working copy in
//! `localStorage` and syncs here, so a different browser can restore it.

pub mod auth;
pub mod error;
pub mod store;

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use game_core::msg::Msg;
use game_core::net;
use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

use crate::auth::{Auth, COOKIE};
use crate::error::{ApiError, ApiResult};
use crate::store::{random_hex, GameServer};

pub use crate::store::Store;

/// Cap on the opaque profile document (decks, history and rewards).
pub const MAX_PROFILE: usize = 1_048_576;

/// Cap on one game-data upload (builds ship wasm and tables).
pub const MAX_UPLOAD: usize = 64 * 1024 * 1024;

pub struct App {
    pub store: Store,
    pub secret: Vec<u8>,
    pub started: u64,
}

pub fn now() -> u64 {
    profile_token::now()
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SessionReq {
    pub player: String,
    pub persona: String,
    pub cn_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    pub id: String,
    pub name: String,
    pub persona: String,
    pub cn_id: String,
    pub has_profile: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub token: String,
    pub player: String,
    pub account: AccountView,
}

fn view(token: String, a: &store::Account) -> SessionView {
    SessionView {
        token,
        player: a.name.clone(),
        account: AccountView {
            id: a.id.clone(),
            name: a.name.clone(),
            persona: a.persona.clone(),
            cn_id: a.cn_id.clone(),
            has_profile: a.profile.is_some(),
        },
    }
}

type S = State<Arc<App>>;

pub async fn health(State(app): S) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "service": "profile",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime": now().saturating_sub(app.started),
    }))
}

/// `POST /api/session` -- sign in by display name, creating the account on
/// first use (there is no password; the id, not the name, is the identity).
pub async fn create_session(State(app): S, Json(req): Json<SessionReq>) -> ApiResult<impl IntoResponse> {
    let player = net::clean_name(&req.player);
    if player.is_empty() {
        return Err(ApiError::bad("err.name_required"));
    }
    let account = app
        .store
        .ensure_account(&player, &req.persona, &req.cn_id, now())?;
    let token = profile_token::mint_now(&app.secret, &account.id, &account.name, &account.persona, &account.cn_id)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let body = view(token.clone(), &account);
    let ttl = profile_token::DEFAULT_TTL_SECS;
    let cookie = format!("{COOKIE}={token}; HttpOnly; SameSite=Lax; Path=/; Max-Age={ttl}");
    Ok(([(header::SET_COOKIE, cookie)], Json(body)))
}

/// `GET /api/session` -- the identity behind the presented ticket.
pub async fn get_session(State(app): S, Auth(claims): Auth) -> ApiResult<Json<SessionView>> {
    let account = app.store.account(&claims.sub).ok_or_else(ApiError::unauthorized)?;
    let token = profile_token::mint_now(&app.secret, &account.id, &account.name, &account.persona, &account.cn_id)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(view(token, &account)))
}

pub async fn logout() -> impl IntoResponse {
    let cookie = format!("{COOKIE}=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0");
    ([(header::SET_COOKIE, cookie)], Json(serde_json::json!({ "ok": true })))
}

#[derive(Debug, Serialize)]
pub struct ProfileView {
    pub profile: Option<String>,
}

pub async fn get_profile(State(app): S, Auth(claims): Auth) -> ApiResult<Json<ProfileView>> {
    let a = app.store.account(&claims.sub).ok_or_else(ApiError::unauthorized)?;
    Ok(Json(ProfileView { profile: a.profile }))
}

#[derive(Debug, Deserialize)]
pub struct ProfileReq {
    pub profile: String,
}

pub async fn put_profile(State(app): S, Auth(claims): Auth, Json(req): Json<ProfileReq>) -> ApiResult<Json<serde_json::Value>> {
    if req.profile.len() > MAX_PROFILE {
        return Err(ApiError::bad("err.profile.too_large"));
    }
    app.store.set_profile(&claims.sub, req.profile)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn list_servers(State(app): S) -> Json<Vec<GameServer>> {
    Json(app.store.servers())
}

#[derive(Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NewServer {
    pub name: String,
    pub url: String,
    pub note: String,
}

impl Default for NewServer {
    fn default() -> Self {
        Self { name: String::new(), url: String::new(), note: String::new() }
    }
}

/// Mutations require a session (a free, name-only one): the directory is
/// shared, so anonymous clients must not be able to wipe or inject entries.
pub async fn add_server(
    State(app): S,
    Auth(_claims): Auth,
    Json(req): Json<NewServer>,
) -> ApiResult<Json<GameServer>> {
    if req.name.trim().is_empty() {
        return Err(ApiError::bad(Msg::new("err.server.name")));
    }
    Ok(Json(app.store.add_server(&req.name, &req.url, &req.note)?))
}

pub async fn remove_server(
    State(app): S,
    Auth(_claims): Auth,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    if !app.store.remove_server(&id)? {
        return Err(ApiError::not_found("err.server.unknown"));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadView {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub size: u64,
    pub sha256: String,
    pub owner: String,
    pub created: u64,
    /// Path relative to this server, e.g. `/uploads/<id>/<name>`.
    pub url: String,
    pub download: String,
}

fn upload_view(u: store::Upload) -> UploadView {
    UploadView {
        url: format!("/uploads/{}/{}", u.id, u.name),
        download: format!("/api/uploads/{}/{}", u.id, u.name),
        id: u.id,
        name: u.name,
        kind: u.kind,
        size: u.size,
        sha256: u.sha256,
        owner: u.owner,
        created: u.created,
    }
}

pub async fn list_uploads(State(app): S) -> Json<Vec<UploadView>> {
    Json(app.store.uploads().into_iter().map(upload_view).collect())
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct UploadQuery {
    pub name: String,
    pub kind: String,
}

impl Default for UploadQuery {
    fn default() -> Self {
        Self { name: String::new(), kind: String::new() }
    }
}

/// `POST /api/uploads?name=build.tar.gz&kind=ruleset` -- raw body. The name is
/// only a label; the server cleans it before using it as a path component.
pub async fn put_upload(
    State(app): S,
    Auth(claims): Auth,
    Query(q): Query<UploadQuery>,
    body: Bytes,
) -> ApiResult<Json<UploadView>> {
    if body.is_empty() {
        return Err(ApiError::bad("err.upload.empty"));
    }
    if body.len() > MAX_UPLOAD {
        return Err(ApiError::bad("err.upload.too_large"));
    }
    let u = app.store.put_upload(&q.name, &q.kind, &claims.sub, &body)?;
    Ok(Json(upload_view(u)))
}

pub async fn remove_upload(
    State(app): S,
    Auth(claims): Auth,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    if !app.store.remove_upload(&id, &claims.sub)? {
        return Err(ApiError::not_found("err.upload.unknown"));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// `GET /uploads/{id}/{file}` -- the stored bytes. Public so a game server
/// owner can fetch a shared build without an account.
pub async fn serve_upload(State(app): S, Path((id, file)): Path<(String, String)>) -> Response {
    // `{id}` is a path component. Real ids are hex; `..` is deliberately
    // allowed and resolves to the data directory itself (`uploads/../…`).
    if id.is_empty() || id == "." || id.contains(['/', '\\']) {
        return StatusCode::NOT_FOUND.into_response();
    }
    if file.split('/').any(|seg| seg.is_empty() || seg == "." || seg == "..") {
        return StatusCode::NOT_FOUND.into_response();
    }
    let path = app.store.upload_dir(&id).join(&file);
    match tokio::fs::read(&path).await {
        Ok(bytes) => {
            let mime = mime_for(&file);
            (
                [
                    (header::CONTENT_TYPE, mime),
                    (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
                ],
                bytes,
            )
                .into_response()
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// `GET /api/uploads/{id}/{file}` -- the same bytes with a friendly name.
pub async fn download_upload(State(app): S, Path((id, file)): Path<(String, String)>) -> Response {
    serve_upload(State(app), Path((id, file))).await
}

fn mime_for(file: &str) -> &'static str {
    match file.rsplit('.').next().unwrap_or("") {
        "wasm" => "application/wasm",
        "json" => "application/json",
        "bin" => "application/octet-stream",
        "gz" => "application/gzip",
        "zip" => "application/zip",
        "txt" | "md" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

pub fn router(app: Arc<App>, static_dir: Option<PathBuf>, game_data: Option<PathBuf>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);
    let mut router = Router::new()
        .route("/api/health", get(health))
        .route("/api/session", post(create_session).get(get_session))
        .route("/api/logout", post(logout))
        .route("/api/profile", get(get_profile).put(put_profile))
        .route("/api/servers", get(list_servers).post(add_server))
        .route("/api/servers/{id}", delete(remove_server))
        .route("/api/uploads", get(list_uploads).post(put_upload))
        .route("/api/uploads/{id}", delete(remove_upload))
        .route("/api/uploads/{id}/{file}", get(download_upload))
        .route("/uploads/{id}/{file}", get(serve_upload))
        .layer(DefaultBodyLimit::max(MAX_UPLOAD + 4096))
        .layer(cors)
        .with_state(app);
    // The SPA served here needs game tables for its first paint (the menu
    // renders from the roster / board before a game server is picked). These
    // are read-only copies; matches always run on the chosen game server.
    if let Some(dir) = game_data {
        router = router.nest_service("/data", ServeDir::new(dir));
    }
    if let Some(dir) = static_dir {
        let index = get(spa_index).with_state(dir.join("index.html"));
        router = router.fallback_service(ServeDir::new(dir).fallback(index));
    }
    router
}

async fn spa_index(State(index): State<PathBuf>, uri: Uri) -> Response {
    let path = uri.path();
    let file_like = path.rsplit('/').next().is_some_and(|last| last.contains('.'));
    if file_like || path.starts_with("/api/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    match tokio::fs::read(&index).await {
        Ok(html) => (
            [
                (header::CONTENT_TYPE, "text/html; charset=utf-8"),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            html,
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// A random secret for a single-process deployment.
pub fn random_secret() -> String {
    random_hex(32)
}
