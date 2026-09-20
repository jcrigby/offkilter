//! HTTP and WebSocket routes.
//!
//! Signing in is optional. Documents created anonymously have no owner and
//! are open to everyone on the server; documents created while signed in
//! belong to that account and are visible only to the owner and the people
//! it was shared with.

use crate::auth::{clear_cookie, session_cookie, AuthError, CurrentUser, User, UserStore};
use crate::live::{ClientMessage, DocHub, ServerMessage};
use crate::store::{DocMeta, DocStore, ShareRole};
use crate::teams::{TeamError, TeamShare, TeamStore};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, DefaultBodyLimit, FromRef, Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tower_http::services::{ServeDir, ServeFile};

/// Largest request body and WebSocket message accepted: a document as JSON.
const MAX_BODY_BYTES: usize = 64 * 1024 * 1024;

/// Sign-in attempts allowed per client address in each window.
const AUTH_ATTEMPTS_PER_WINDOW: u32 = 10;
const AUTH_WINDOW: Duration = Duration::from_secs(60);

/// A fixed-window counter per client address for the sign-in endpoints,
/// so passwords cannot be guessed at line rate.
#[derive(Clone, Default)]
pub struct RateLimiter {
    windows: Arc<Mutex<HashMap<String, (Instant, u32)>>>,
}

impl RateLimiter {
    /// Records an attempt from `key` and says whether it is within the limit.
    pub fn allow(&self, key: &str) -> bool {
        let mut w = self.windows.lock().unwrap();
        let now = Instant::now();
        if w.len() > 10_000 {
            w.retain(|_, (start, _)| now.duration_since(*start) < AUTH_WINDOW);
        }
        let entry = w.entry(key.to_string()).or_insert((now, 0));
        if now.duration_since(entry.0) >= AUTH_WINDOW {
            *entry = (now, 0);
        }
        entry.1 += 1;
        entry.1 <= AUTH_ATTEMPTS_PER_WINDOW
    }
}

#[derive(Clone)]
pub struct AppState {
    hub: DocHub,
    users: UserStore,
    teams: TeamStore,
    limiter: RateLimiter,
}

impl FromRef<AppState> for TeamStore {
    fn from_ref(s: &AppState) -> TeamStore {
        s.teams.clone()
    }
}

impl FromRef<AppState> for RateLimiter {
    fn from_ref(s: &AppState) -> RateLimiter {
        s.limiter.clone()
    }
}

impl FromRef<AppState> for DocHub {
    fn from_ref(s: &AppState) -> DocHub {
        s.hub.clone()
    }
}

impl FromRef<AppState> for UserStore {
    fn from_ref(s: &AppState) -> UserStore {
        s.users.clone()
    }
}

pub fn router(
    store: DocStore,
    users: UserStore,
    teams: TeamStore,
    static_dir: Option<PathBuf>,
) -> Router {
    let state = AppState {
        hub: DocHub::new(store),
        users,
        teams,
        limiter: RateLimiter::default(),
    };
    let api = Router::new()
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me))
        .route("/docs", get(list_docs).post(create_doc))
        .route("/docs/{id}", get(get_doc).put(put_doc).delete(delete_doc))
        .route("/docs/{id}/meta", get(get_meta))
        .route("/docs/{id}/share", post(share_doc))
        .route("/docs/{id}/share/{user}", delete(unshare_doc))
        .route("/docs/{id}/invites", post(create_invite))
        .route("/docs/{id}/invites/{token}", delete(revoke_invite))
        .route("/docs/{id}/invites/{token}/accept", post(accept_invite))
        .route("/docs/{id}/share-team", post(share_team))
        .route("/docs/{id}/share-team/{team}", delete(unshare_team))
        .route("/teams", get(list_teams).post(create_team))
        .route("/teams/{id}", delete(delete_team))
        .route("/teams/{id}/members", post(add_team_member))
        .route("/teams/{id}/members/{user}", delete(remove_team_member))
        .route("/docs/{id}/ws", get(ws_upgrade))
        .route("/docs/{id}/versions", get(list_versions).post(save_version))
        .route("/docs/{id}/versions/{vid}", get(get_version))
        .route("/docs/{id}/versions/{vid}/restore", post(restore_version))
        .route("/docs/{id}/branch", post(branch_doc))
        .route("/docs/{id}/check", get(check_doc))
        .route("/docs/{id}/export/stl", get(export_stl))
        .route("/health", get(|| async { "ok" }))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(state);
    let mut app = Router::new().nest("/api", api);
    if let Some(dir) = static_dir {
        let index = dir.join("index.html");
        app = app.fallback_service(ServeDir::new(dir).not_found_service(ServeFile::new(index)));
    }
    app.layer(tower_http::cors::CorsLayer::permissive())
        .layer(axum::middleware::from_fn(security_headers))
}

/// Response headers that stop content sniffing, framing by other sites and
/// referrer leaks. The app is a same-origin bundle, so nothing here is
/// restrictive for it.
async fn security_headers(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    h.insert("x-frame-options", HeaderValue::from_static("DENY"));
    h.insert("referrer-policy", HeaderValue::from_static("same-origin"));
    res
}

/// The client's address when the server was started with connection info,
/// as a rate-limiting key; "local" otherwise (tests).
struct ClientKey(String);

impl<S: Send + Sync> axum::extract::FromRequestParts<S> for ClientKey {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        Ok(ClientKey(
            parts
                .extensions
                .get::<ConnectInfo<SocketAddr>>()
                .map(|a| a.0.ip().to_string())
                .unwrap_or_else(|| "local".into()),
        ))
    }
}

// ---------------------------------------------------------------- accounts

#[derive(Deserialize)]
struct Credentials {
    name: String,
    password: String,
}

fn auth_status(e: &AuthError) -> StatusCode {
    match e {
        AuthError::BadName | AuthError::BadPassword => StatusCode::BAD_REQUEST,
        AuthError::Taken => StatusCode::CONFLICT,
        AuthError::Denied => StatusCode::UNAUTHORIZED,
        AuthError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn signed_in(result: Result<(User, String), AuthError>) -> axum::response::Response {
    match result {
        Ok((user, tok)) => (
            [(header::SET_COOKIE, session_cookie(&tok))],
            Json(crate::auth::UserInfo::from(&user)),
        )
            .into_response(),
        Err(e) => (auth_status(&e), e.to_string()).into_response(),
    }
}

async fn register(
    State(users): State<UserStore>,
    State(limiter): State<RateLimiter>,
    ClientKey(key): ClientKey,
    Json(c): Json<Credentials>,
) -> axum::response::Response {
    if !limiter.allow(&key) {
        return too_many_attempts();
    }
    signed_in(users.register(&c.name, &c.password)).into_response()
}

async fn login(
    State(users): State<UserStore>,
    State(limiter): State<RateLimiter>,
    ClientKey(key): ClientKey,
    Json(c): Json<Credentials>,
) -> axum::response::Response {
    if !limiter.allow(&key) {
        return too_many_attempts();
    }
    signed_in(users.login(&c.name, &c.password)).into_response()
}

fn too_many_attempts() -> axum::response::Response {
    (
        StatusCode::TOO_MANY_REQUESTS,
        [(header::RETRY_AFTER, AUTH_WINDOW.as_secs().to_string())],
        "too many sign-in attempts; try again in a minute",
    )
        .into_response()
}

async fn logout(State(users): State<UserStore>, req: axum::extract::Request) -> impl IntoResponse {
    let (parts, _) = req.into_parts();
    if let Some(tok) = crate::auth::token_from_parts(&parts) {
        users.logout(&tok);
    }
    (
        [(header::SET_COOKIE, clear_cookie())],
        StatusCode::NO_CONTENT,
    )
}

async fn me(CurrentUser(user): CurrentUser) -> impl IntoResponse {
    match user {
        Some(u) => Json(crate::auth::UserInfo::from(&u)).into_response(),
        None => StatusCode::UNAUTHORIZED.into_response(),
    }
}

// ---------------------------------------------------------------- documents

/// The document's metadata if `user` may open it; 404 hides documents the
/// user cannot see, 403 marks ones they know of but may not manage.
fn accessible(hub: &DocHub, id: &str, user: Option<&User>) -> Result<DocMeta, StatusCode> {
    let meta = hub.store().meta(id).ok_or(StatusCode::NOT_FOUND)?;
    if meta.can_access(user) {
        Ok(meta)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// The document when `user` may change it; 403 for read-only viewers.
fn editable(hub: &DocHub, id: &str, user: Option<&User>) -> Result<DocMeta, StatusCode> {
    let meta = accessible(hub, id, user)?;
    if meta.can_edit(user) {
        Ok(meta)
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

fn manageable(hub: &DocHub, id: &str, user: Option<&User>) -> Result<DocMeta, StatusCode> {
    let meta = accessible(hub, id, user)?;
    if meta.can_manage(user) {
        Ok(meta)
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

async fn list_docs(State(hub): State<DocHub>, CurrentUser(user): CurrentUser) -> impl IntoResponse {
    match hub.store().list() {
        Ok(list) => Json(
            list.into_iter()
                .filter(|m| m.can_access(user.as_ref()))
                .map(|m| m.visible_to(user.as_ref()))
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
struct CreateDoc {
    name: String,
    #[serde(default)]
    json: Option<String>,
}

async fn create_doc(
    State(hub): State<DocHub>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<CreateDoc>,
) -> impl IntoResponse {
    let json = match body.json {
        Some(j) => match ok_model::Document::from_json(&j) {
            Ok(_) => j,
            Err(e) => {
                return (StatusCode::BAD_REQUEST, format!("invalid document: {e}")).into_response()
            }
        },
        None => ok_model::Document::new(body.name.clone()).to_json(),
    };
    let owner = user.as_ref().map(crate::auth::UserInfo::from);
    match hub.store().create(&body.name, &json, owner) {
        Ok(meta) => (StatusCode::CREATED, Json(meta)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_meta(
    State(hub): State<DocHub>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match accessible(&hub, &id, user.as_ref()) {
        Ok(meta) => Json(meta.visible_to(user.as_ref())).into_response(),
        Err(code) => code.into_response(),
    }
}

async fn get_doc(
    State(hub): State<DocHub>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if let Err(code) = accessible(&hub, &id, user.as_ref()) {
        return code.into_response();
    }
    match hub.store().read(&id) {
        Some(json) => ([(header::CONTENT_TYPE, "application/json")], json).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[derive(Deserialize)]
struct BranchDoc {
    #[serde(default)]
    name: Option<String>,
    /// A saved version to branch from; the current state when absent.
    #[serde(default)]
    version: Option<String>,
}

/// Copies a document (as it is now, or as a saved version) into a new
/// document owned by the requester, remembering where it came from.
/// Anyone who can read the source may branch it.
async fn branch_doc(
    State(hub): State<DocHub>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    Json(body): Json<BranchDoc>,
) -> impl IntoResponse {
    let source = match accessible(&hub, &id, user.as_ref()) {
        Ok(m) => m,
        Err(code) => return code.into_response(),
    };
    let store = hub.store();
    let (json, version_name) = match &body.version {
        Some(v) => match store.read_version(&id, v) {
            Some(json) => {
                let name = store
                    .list_versions(&id)
                    .ok()
                    .and_then(|l| l.into_iter().find(|m| &m.id == v))
                    .map(|m| m.name);
                (json, name)
            }
            None => return (StatusCode::NOT_FOUND, "no such version").into_response(),
        },
        None => match current_json(&hub, &id) {
            Some(json) => (json, None),
            None => return StatusCode::NOT_FOUND.into_response(),
        },
    };
    let name = body.name.clone().unwrap_or_else(|| match &version_name {
        Some(v) => format!("{} ({v})", source.name),
        None => format!("{} (branch)", source.name),
    });
    let owner = user.as_ref().map(crate::auth::UserInfo::from);
    let meta = match store.create(&name, &json, owner) {
        Ok(m) => m,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    let origin = crate::store::BranchOrigin {
        doc: source.id.clone(),
        doc_name: source.name.clone(),
        version: body.version.clone(),
        version_name,
    };
    match store.set_parent(&meta.id, origin) {
        Ok(meta) => (StatusCode::CREATED, Json(meta)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// The document as it is right now: the live copy when one is open,
/// else the stored one.
fn current_json(hub: &DocHub, id: &str) -> Option<String> {
    match hub.get(id) {
        Some(doc) => match doc.snapshot() {
            ServerMessage::Snapshot { doc, .. } => Some(doc),
            _ => None,
        },
        None => hub.store().read(id),
    }
}

#[derive(Serialize)]
struct CheckBody {
    name: String,
    source: u32,
    volume: f64,
    area: f64,
    faces: usize,
    bounds: Option<(ok_math::Vec3, ok_math::Vec3)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    material: Option<ok_model::Material>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mass_g: Option<f64>,
}

#[derive(Serialize)]
struct CheckTab {
    id: u32,
    name: String,
    kind: &'static str,
    bodies: Vec<CheckBody>,
    /// Feature (or instance / mate) errors as "name: message".
    errors: Vec<String>,
}

#[derive(Serialize)]
struct CheckReport {
    name: String,
    ok: bool,
    tabs: Vec<CheckTab>,
}

/// Regenerates every tab of a document on the server and reports its
/// bodies and errors: a headless validation for scripts and CI.
fn check_document(json: &str) -> Result<CheckReport, String> {
    let mut doc = ok_model::Document::from_json(json).map_err(|e| e.to_string())?;
    let ids: Vec<(ok_model::TabId, String, &'static str)> = doc
        .tabs
        .iter()
        .map(|t| {
            let name = match &t.kind {
                ok_model::TabKind::PartStudio(p) => p.name.clone(),
                ok_model::TabKind::Assembly(a) => a.name.clone(),
            };
            (t.id, name, t.kind_name())
        })
        .collect();
    let mut tabs = Vec::new();
    let body_of = |b: &ok_model::Body| CheckBody {
        name: b.name.clone(),
        source: b.source.0,
        volume: b.solid.volume(),
        area: b.solid.surface_area(),
        faces: b.solid.faces.len(),
        bounds: b.solid.bounds(),
        material: b.material.clone(),
        mass_g: b.material.as_ref().map(|m| m.mass_g(b.solid.volume())),
    };
    for (id, name, kind) in ids {
        let (bodies, errors) = if kind == "assembly" {
            match doc.regenerate_assembly(id) {
                Ok(r) => {
                    let mut errors: Vec<String> = r
                        .instance_errors
                        .iter()
                        .map(|(i, e)| format!("instance {}: {e}", i.0))
                        .collect();
                    errors.extend(
                        r.mate_errors
                            .iter()
                            .map(|(m, e)| format!("mate {}: {e}", m.0)),
                    );
                    (r.bodies.iter().map(body_of).collect(), errors)
                }
                Err(e) => (Vec::new(), vec![e.to_string()]),
            }
        } else {
            match doc.regenerate_studio(id, None) {
                Ok(r) => {
                    let names: std::collections::BTreeMap<u32, String> = doc
                        .tab(id)
                        .and_then(|t| match &t.kind {
                            ok_model::TabKind::PartStudio(p) => Some(
                                p.features()
                                    .iter()
                                    .map(|f| (f.id.0, f.name.clone()))
                                    .collect(),
                            ),
                            _ => None,
                        })
                        .unwrap_or_default();
                    let errors = r
                        .errors()
                        .map(|(f, e)| {
                            format!(
                                "{}: {e}",
                                names
                                    .get(&f.0)
                                    .cloned()
                                    .unwrap_or_else(|| format!("feature {}", f.0))
                            )
                        })
                        .collect();
                    (r.bodies.iter().map(body_of).collect(), errors)
                }
                Err(e) => (Vec::new(), vec![e.to_string()]),
            }
        };
        tabs.push(CheckTab {
            id: id.0,
            name,
            kind,
            bodies,
            errors,
        });
    }
    Ok(CheckReport {
        name: doc.name.clone(),
        ok: tabs.iter().all(|t| t.errors.is_empty()),
        tabs,
    })
}

async fn check_doc(
    State(hub): State<DocHub>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if let Err(code) = accessible(&hub, &id, user.as_ref()) {
        return code.into_response();
    }
    let Some(json) = current_json(&hub, &id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match tokio::task::spawn_blocking(move || check_document(&json)).await {
        Ok(Ok(report)) => Json(report).into_response(),
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, e).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
struct ExportQuery {
    /// Tab id; the first tab when absent.
    tab: Option<u32>,
}

/// Binary STL of every body of a tab, regenerated on the server.
fn stl_of(json: &str, tab: Option<u32>) -> Result<Vec<u8>, String> {
    let mut doc = ok_model::Document::from_json(json).map_err(|e| e.to_string())?;
    let id = match tab {
        Some(t) => ok_model::TabId(t),
        None => doc
            .tabs
            .first()
            .map(|t| t.id)
            .ok_or("document has no tabs")?,
    };
    let kind = doc.tab(id).map(|t| t.kind_name()).ok_or("no such tab")?;
    let meshes: Vec<ok_mesh::TriMesh> = if kind == "assembly" {
        doc.regenerate_assembly(id)
            .map_err(|e| e.to_string())?
            .bodies
            .iter()
            .map(|b| b.mesh.clone())
            .collect()
    } else {
        doc.regenerate_studio(id, None)
            .map_err(|e| e.to_string())?
            .bodies
            .iter()
            .map(|b| b.mesh.clone())
            .collect()
    };
    let count: usize = meshes.iter().map(|m| m.triangle_count()).sum();
    let mut out = Vec::with_capacity(84 + count * 50);
    let mut header = format!("offkilter tab {}", id.0).into_bytes();
    header.resize(80, 0);
    out.extend_from_slice(&header);
    out.extend_from_slice(&(count as u32).to_le_bytes());
    for m in &meshes {
        for tri in m.indices.chunks_exact(3) {
            let p = |i: u32| {
                let k = i as usize * 3;
                [m.positions[k], m.positions[k + 1], m.positions[k + 2]]
            };
            let (a, b, c) = (p(tri[0]), p(tri[1]), p(tri[2]));
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            let n = if len > 0.0 {
                [n[0] / len, n[1] / len, n[2] / len]
            } else {
                [0.0, 0.0, 0.0]
            };
            for f in n.iter().chain(a.iter()).chain(b.iter()).chain(c.iter()) {
                out.extend_from_slice(&f.to_le_bytes());
            }
            out.extend_from_slice(&[0, 0]);
        }
    }
    Ok(out)
}

async fn export_stl(
    State(hub): State<DocHub>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    axum::extract::Query(q): axum::extract::Query<ExportQuery>,
) -> impl IntoResponse {
    if let Err(code) = accessible(&hub, &id, user.as_ref()) {
        return code.into_response();
    }
    let Some(json) = current_json(&hub, &id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match tokio::task::spawn_blocking(move || stl_of(&json, q.tab)).await {
        Ok(Ok(bytes)) => (
            [
                (header::CONTENT_TYPE, "model/stl".to_string()),
                (
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{id}.stl\""),
                ),
            ],
            bytes,
        )
            .into_response(),
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, e).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn put_doc(
    State(hub): State<DocHub>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    body: String,
) -> impl IntoResponse {
    if let Err(code) = editable(&hub, &id, user.as_ref()) {
        return code.into_response();
    }
    let studio = match ok_model::Document::from_json(&body) {
        Ok(s) => s,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, format!("invalid document: {e}")).into_response()
        }
    };
    match hub.store().write(&id, &body, Some(&studio.name)) {
        Ok(()) => {
            hub.evict(&id);
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn delete_doc(
    State(hub): State<DocHub>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if let Err(code) = manageable(&hub, &id, user.as_ref()) {
        return code;
    }
    hub.evict(&id);
    match hub.store().delete(&id) {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::NOT_FOUND,
    }
}

#[derive(Deserialize)]
struct ShareDoc {
    name: String,
    /// "editor" (default) or "viewer" (read-only).
    #[serde(default)]
    role: Option<ShareRole>,
}

async fn share_doc(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    Json(body): Json<ShareDoc>,
) -> impl IntoResponse {
    if let Err(code) = manageable(&state.hub, &id, user.as_ref()) {
        return code.into_response();
    }
    let Some(target) = state.users.user_by_name(&body.name) else {
        return (StatusCode::NOT_FOUND, "no such user").into_response();
    };
    match state.hub.store().share(
        &id,
        crate::auth::UserInfo::from(&target),
        body.role.unwrap_or(ShareRole::Editor),
    ) {
        Ok(meta) => Json(meta).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn unshare_doc(
    State(hub): State<DocHub>,
    CurrentUser(user): CurrentUser,
    Path((id, target)): Path<(String, String)>,
) -> impl IntoResponse {
    if let Err(code) = manageable(&hub, &id, user.as_ref()) {
        return code.into_response();
    }
    match hub.store().unshare(&id, &target) {
        Ok(meta) => Json(meta).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

fn team_error(e: TeamError) -> axum::response::Response {
    let code = match e {
        TeamError::BadName => StatusCode::BAD_REQUEST,
        TeamError::NotFound => StatusCode::NOT_FOUND,
        TeamError::NotOwner => StatusCode::FORBIDDEN,
        TeamError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (code, e.to_string()).into_response()
}

async fn list_teams(
    State(teams): State<TeamStore>,
    CurrentUser(user): CurrentUser,
) -> impl IntoResponse {
    match user {
        Some(u) => Json(teams.for_user(&u.id)).into_response(),
        None => (StatusCode::UNAUTHORIZED, "sign in to use teams").into_response(),
    }
}

#[derive(Deserialize)]
struct CreateTeam {
    name: String,
}

async fn create_team(
    State(teams): State<TeamStore>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<CreateTeam>,
) -> impl IntoResponse {
    let Some(user) = user else {
        return (StatusCode::UNAUTHORIZED, "sign in to create a team").into_response();
    };
    match teams.create(&body.name, &user) {
        Ok(t) => (StatusCode::CREATED, Json(t)).into_response(),
        Err(e) => team_error(e),
    }
}

async fn delete_team(
    State(teams): State<TeamStore>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(user) = user else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    match teams.delete(&id, &user) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => team_error(e),
    }
}

#[derive(Deserialize)]
struct AddMember {
    name: String,
}

async fn add_team_member(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    Json(body): Json<AddMember>,
) -> impl IntoResponse {
    let Some(user) = user else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(member) = state.users.user_by_name(&body.name) else {
        return (StatusCode::NOT_FOUND, "no such user").into_response();
    };
    match state
        .teams
        .add_member(&id, &user, crate::auth::UserInfo::from(&member))
    {
        Ok(t) => Json(t).into_response(),
        Err(e) => team_error(e),
    }
}

async fn remove_team_member(
    State(teams): State<TeamStore>,
    CurrentUser(user): CurrentUser,
    Path((id, member)): Path<(String, String)>,
) -> impl IntoResponse {
    let Some(user) = user else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    match teams.remove_member(&id, &user, &member) {
        Ok(t) => Json(t).into_response(),
        Err(e) => team_error(e),
    }
}

#[derive(Deserialize)]
struct ShareTeam {
    team: String,
    #[serde(default)]
    role: Option<ShareRole>,
}

/// Shares a document with a team the owner belongs to.
async fn share_team(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    Json(body): Json<ShareTeam>,
) -> impl IntoResponse {
    if let Err(code) = manageable(&state.hub, &id, user.as_ref()) {
        return code.into_response();
    }
    let Some(team) = state.teams.team(&body.team) else {
        return (StatusCode::NOT_FOUND, "no such team").into_response();
    };
    if !user.as_ref().is_some_and(|u| team.has_member(&u.id)) {
        return (StatusCode::FORBIDDEN, "you are not in that team").into_response();
    }
    let share = TeamShare {
        id: team.id,
        name: team.name,
        role: body.role.unwrap_or(ShareRole::Editor),
    };
    match state.hub.store().share_team(&id, share) {
        Ok(meta) => Json(meta).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn unshare_team(
    State(hub): State<DocHub>,
    CurrentUser(user): CurrentUser,
    Path((id, team)): Path<(String, String)>,
) -> impl IntoResponse {
    if let Err(code) = manageable(&hub, &id, user.as_ref()) {
        return code.into_response();
    }
    match hub.store().unshare_team(&id, &team) {
        Ok(meta) => Json(meta).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
struct CreateInvite {
    #[serde(default)]
    role: Option<ShareRole>,
}

async fn create_invite(
    State(hub): State<DocHub>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    Json(body): Json<CreateInvite>,
) -> impl IntoResponse {
    if let Err(code) = manageable(&hub, &id, user.as_ref()) {
        return code.into_response();
    }
    match hub
        .store()
        .create_invite(&id, body.role.unwrap_or(ShareRole::Editor))
    {
        Ok(invite) => (StatusCode::CREATED, Json(invite)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn revoke_invite(
    State(hub): State<DocHub>,
    CurrentUser(user): CurrentUser,
    Path((id, token)): Path<(String, String)>,
) -> impl IntoResponse {
    if let Err(code) = manageable(&hub, &id, user.as_ref()) {
        return code.into_response();
    }
    match hub.store().revoke_invite(&id, &token) {
        Ok(meta) => Json(meta).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// A signed-in account joins a document through an invitation link. The
/// document need not be visible to it beforehand; a wrong token is a 404
/// like a document that does not exist.
async fn accept_invite(
    State(hub): State<DocHub>,
    CurrentUser(user): CurrentUser,
    Path((id, token)): Path<(String, String)>,
) -> impl IntoResponse {
    let Some(user) = user else {
        return (StatusCode::UNAUTHORIZED, "sign in to accept an invitation").into_response();
    };
    if hub.store().meta(&id).is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }
    match hub
        .store()
        .accept_invite(&id, &token, crate::auth::UserInfo::from(&user))
    {
        Ok(meta) => Json(meta.visible_to(Some(&user))).into_response(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            (StatusCode::NOT_FOUND, e.to_string()).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn list_versions(
    State(hub): State<DocHub>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if let Err(code) = accessible(&hub, &id, user.as_ref()) {
        return code.into_response();
    }
    match hub.store().list_versions(&id) {
        Ok(v) => Json(v).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
struct SaveVersion {
    name: String,
}

async fn save_version(
    State(hub): State<DocHub>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    Json(body): Json<SaveVersion>,
) -> impl IntoResponse {
    if let Err(code) = editable(&hub, &id, user.as_ref()) {
        return code.into_response();
    }
    match hub.store().save_version(&id, &body.name) {
        Ok(v) => (StatusCode::CREATED, Json(v)).into_response(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_version(
    State(hub): State<DocHub>,
    CurrentUser(user): CurrentUser,
    Path((id, vid)): Path<(String, String)>,
) -> impl IntoResponse {
    if let Err(code) = accessible(&hub, &id, user.as_ref()) {
        return code.into_response();
    }
    match hub.store().read_version(&id, &vid) {
        Some(json) => ([(header::CONTENT_TYPE, "application/json")], json).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn restore_version(
    State(hub): State<DocHub>,
    CurrentUser(user): CurrentUser,
    Path((id, vid)): Path<(String, String)>,
) -> impl IntoResponse {
    if let Err(code) = editable(&hub, &id, user.as_ref()) {
        return code.into_response();
    }
    let Some(json) = hub.store().read_version(&id, &vid) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match hub.get(&id) {
        Some(doc) => match doc.replace(&json) {
            Ok(()) => StatusCode::NO_CONTENT.into_response(),
            Err(e) => (StatusCode::BAD_REQUEST, e).into_response(),
        },
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn ws_upgrade(
    State(hub): State<DocHub>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let meta = match accessible(&hub, &id, user.as_ref()) {
        Ok(m) => m,
        Err(code) => return code.into_response(),
    };
    let read_only = !meta.can_edit(user.as_ref());
    match hub.get(&id) {
        Some(doc) => ws
            .max_message_size(MAX_BODY_BYTES)
            .max_frame_size(MAX_BODY_BYTES)
            .on_upgrade(move |socket| handle_socket(socket, hub, doc, user, read_only))
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn handle_socket(
    socket: WebSocket,
    hub: DocHub,
    doc: std::sync::Arc<crate::live::LiveDoc>,
    user: Option<User>,
    read_only: bool,
) {
    let (mut sink, mut stream) = socket.split();
    // First message may be a hello with a display name; a signed-in user
    // is always shown under their account name.
    let mut name: Option<String> = user.map(|u| u.name);
    let first = stream.next().await;
    let mut pending_after_hello: Option<ClientMessage> = None;
    if let Some(Ok(Message::Text(text))) = &first {
        match serde_json::from_str::<ClientMessage>(text) {
            Ok(ClientMessage::Hello { name: n }) => name = name.or(n),
            Ok(other) => pending_after_hello = Some(other),
            Err(_) => {}
        }
    } else if first.is_none() {
        return;
    }
    // Subscribe before joining so this client also sees its own presence update.
    let mut rx = doc.tx.subscribe();
    let (client, welcome) = doc.join(name, read_only);
    if sink
        .send(Message::Text(
            serde_json::to_string(&welcome).unwrap().into(),
        ))
        .await
        .is_err()
    {
        doc.leave(client);
        hub.release(&doc.id);
        return;
    }
    let doc_id = doc.id.clone();

    // Outgoing: broadcast messages to this client.
    let (out_tx, mut out_rx) = tokio::sync::mpsc::channel::<ServerMessage>(64);
    let writer = tokio::spawn(async move {
        loop {
            tokio::select! {
                Ok(msg) = rx.recv() => {
                    if sink.send(Message::Text(serde_json::to_string(&msg).unwrap().into())).await.is_err() {
                        break;
                    }
                }
                Some(msg) = out_rx.recv() => {
                    if sink.send(Message::Text(serde_json::to_string(&msg).unwrap().into())).await.is_err() {
                        break;
                    }
                }
                else => break,
            }
        }
    });

    let handle = |msg: ClientMessage, doc: &crate::live::LiveDoc| -> Option<ServerMessage> {
        match msg {
            ClientMessage::Hello { .. } => None,
            ClientMessage::Op { op, id, base } => {
                if read_only {
                    return Some(ServerMessage::Error {
                        message: "this document is shared with you read-only".into(),
                        id,
                    });
                }
                match doc.apply(client, id, op, base) {
                    Ok(_) => None,
                    Err(message) => Some(ServerMessage::Error { message, id }),
                }
            }
            ClientMessage::Snapshot => Some(doc.snapshot()),
        }
    };
    if let Some(msg) = pending_after_hello {
        if let Some(reply) = handle(msg, &doc) {
            let _ = out_tx.send(reply).await;
        }
    }
    while let Some(Ok(msg)) = stream.next().await {
        match msg {
            Message::Text(text) => {
                if let Ok(parsed) = serde_json::from_str::<ClientMessage>(&text) {
                    if let Some(reply) = handle(parsed, &doc) {
                        let _ = out_tx.send(reply).await;
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
    writer.abort();
    doc.leave(client);
    hub.release(&doc_id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn temp_store() -> DocStore {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("ok-server-test-{}-{nanos}-{n}", std::process::id()));
        DocStore::open(&dir).unwrap()
    }

    fn temp_teams() -> TeamStore {
        let dir = std::env::temp_dir().join(format!(
            "ok-server-teams-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        TeamStore::open(&dir).unwrap()
    }

    fn temp_users() -> UserStore {
        let dir = std::env::temp_dir().join(format!(
            "ok-server-users-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        UserStore::open(&dir).unwrap()
    }

    /// Sends a JSON request, returning status, headers and body text.
    async fn call(
        app: &Router,
        method: &str,
        path: &str,
        body: Option<&str>,
        cookie: Option<&str>,
    ) -> (StatusCode, axum::http::HeaderMap, String) {
        let mut req = Request::builder().method(method).uri(path);
        if body.is_some() {
            req = req.header("content-type", "application/json");
        }
        if let Some(c) = cookie {
            req = req.header("cookie", c);
        }
        let req = req
            .body(
                body.map(|b| Body::from(b.to_string()))
                    .unwrap_or_else(Body::empty),
            )
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let headers = res.headers().clone();
        let text = String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec())
            .unwrap();
        (status, headers, text)
    }

    fn cookie_of(headers: &axum::http::HeaderMap) -> String {
        let set = headers.get("set-cookie").unwrap().to_str().unwrap();
        set.split(';').next().unwrap().to_string()
    }

    #[tokio::test]
    async fn sign_in_attempts_are_rate_limited_and_responses_carry_security_headers() {
        let app = router(temp_store(), temp_users(), temp_teams(), None);
        let (status, headers, _) = call(&app, "GET", "/api/health", None, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
        assert_eq!(headers.get("x-frame-options").unwrap(), "DENY");
        let creds = r#"{"name":"nobody","password":"wrong-password"}"#;
        let mut statuses = Vec::new();
        for _ in 0..AUTH_ATTEMPTS_PER_WINDOW + 2 {
            let (status, headers, _) =
                call(&app, "POST", "/api/auth/login", Some(creds), None).await;
            if status == StatusCode::TOO_MANY_REQUESTS {
                assert!(headers.get("retry-after").is_some());
            }
            statuses.push(status);
        }
        let limited = statuses
            .iter()
            .filter(|s| **s == StatusCode::TOO_MANY_REQUESTS)
            .count();
        assert_eq!(limited, 2, "{statuses:?}");
        assert!(statuses[..AUTH_ATTEMPTS_PER_WINDOW as usize]
            .iter()
            .all(|s| *s == StatusCode::UNAUTHORIZED));
    }

    #[tokio::test]
    async fn viewers_can_read_but_not_write() {
        let users = temp_users();
        let app = router(temp_store(), users.clone(), temp_teams(), None);
        let (_, owner_tok) = users.register("olive", "olives-password").unwrap();
        let (_, viewer_tok) = users.register("vic", "vics-password").unwrap();
        let (owner, viewer) = (
            format!("ok_session={owner_tok}"),
            format!("ok_session={viewer_tok}"),
        );
        let (status, _, text) = call(
            &app,
            "POST",
            "/api/docs",
            Some(r#"{"name":"plans"}"#),
            Some(&owner),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{text}");
        let id = serde_json::from_str::<serde_json::Value>(&text).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        // Share read-only.
        let (status, _, text) = call(
            &app,
            "POST",
            &format!("/api/docs/{id}/share"),
            Some(r#"{"name":"vic","role":"viewer"}"#),
            Some(&owner),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{text}");
        let meta: DocMeta = serde_json::from_str(&text).unwrap();
        assert_eq!(meta.viewers.len(), 1);
        assert!(meta.collaborators.is_empty());
        // The viewer sees and reads the document but cannot replace it.
        let (status, _, _) = call(&app, "GET", "/api/docs", None, Some(&viewer)).await;
        assert_eq!(status, StatusCode::OK);
        let (status, _, doc) =
            call(&app, "GET", &format!("/api/docs/{id}"), None, Some(&viewer)).await;
        assert_eq!(status, StatusCode::OK);
        let (status, _, _) = call(
            &app,
            "PUT",
            &format!("/api/docs/{id}"),
            Some(&doc),
            Some(&viewer),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, _, _) = call(
            &app,
            "POST",
            &format!("/api/docs/{id}/versions"),
            Some(r#"{"name":"v1"}"#),
            Some(&viewer),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        // Promoting to editor moves the account between the lists.
        let (status, _, text) = call(
            &app,
            "POST",
            &format!("/api/docs/{id}/share"),
            Some(r#"{"name":"vic","role":"editor"}"#),
            Some(&owner),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{text}");
        let meta: DocMeta = serde_json::from_str(&text).unwrap();
        assert!(meta.viewers.is_empty());
        assert_eq!(meta.collaborators.len(), 1);
        let (status, _, _) = call(
            &app,
            "PUT",
            &format!("/api/docs/{id}"),
            Some(&doc),
            Some(&viewer),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn check_and_stl_export_regenerate_on_the_server() {
        let app = router(temp_store(), temp_users(), temp_teams(), None);
        // A 10 x 20 x 5 block built through ops, as a document JSON.
        let mut ps = ok_model::PartStudio::new("block");
        let sketch = ps
            .apply(ok_model::Op::AddSketch {
                plane: ok_model::PlaneRef::standard(ok_model::StandardPlane::Top),
                name: None,
            })
            .unwrap()
            .feature
            .unwrap();
        ps.apply(ok_model::Op::Sketch {
            id: sketch,
            op: ok_model::SketchOp::AddRectangle {
                a: ok_math::Vec2::ZERO,
                b: ok_math::Vec2::new(10.0, 20.0),
            },
        })
        .unwrap();
        ps.apply(ok_model::Op::AddExtrude {
            sketch,
            profiles: ok_model::ProfileSelection::All,
            depth: 5.0,
            direction: ok_model::ExtrudeDirection::Normal,
            end: ok_model::ExtrudeEnd::Blind,
            op: ok_model::BodyOp::New,
            name: Some("Block".into()),
        })
        .unwrap();
        let mut doc = ok_model::Document::new("headless");
        // A new document already holds one empty part studio tab: fill it.
        doc.tabs[0].kind = ok_model::TabKind::PartStudio(ps);
        let body = serde_json::json!({ "name": "headless", "json": doc.to_json() }).to_string();
        let (status, _, text) = call(&app, "POST", "/api/docs", Some(&body), None).await;
        assert_eq!(status, StatusCode::CREATED, "{text}");
        let id = serde_json::from_str::<serde_json::Value>(&text).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        let (status, _, text) =
            call(&app, "GET", &format!("/api/docs/{id}/check"), None, None).await;
        assert_eq!(status, StatusCode::OK, "{text}");
        let report: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(report["ok"], true, "{text}");
        assert_eq!(report["tabs"][0]["bodies"][0]["name"], "Part 1", "{text}");
        assert!((report["tabs"][0]["bodies"][0]["volume"].as_f64().unwrap() - 1000.0).abs() < 1e-9);
        let (status, headers, bytes) =
            call_bytes(&app, &format!("/api/docs/{id}/export/stl?tab=1"), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::CONTENT_TYPE], "model/stl");
        let count = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
        assert_eq!(count, 12);
        assert_eq!(bytes.len(), 84 + 50 * count);
        let (status, _, _) =
            call_bytes(&app, &format!("/api/docs/{id}/export/stl?tab=9"), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    /// Sends a GET, returning status, headers and raw bytes.
    async fn call_bytes(
        app: &Router,
        path: &str,
        cookie: Option<&str>,
    ) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
        let mut req = Request::builder().method("GET").uri(path);
        if let Some(c) = cookie {
            req = req.header("cookie", c);
        }
        let req = req.body(Body::empty()).unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let headers = res.headers().clone();
        let bytes = res.into_body().collect().await.unwrap().to_bytes().to_vec();
        (status, headers, bytes)
    }

    #[tokio::test]
    async fn branching_copies_a_document_or_one_of_its_versions() {
        let users = temp_users();
        let app = router(temp_store(), users.clone(), temp_teams(), None);
        let (_, tok) = users.register("bea", "beas-password").unwrap();
        let bea = format!("ok_session={tok}");
        let doc = ok_model::Document::new("origin").to_json();
        let body = serde_json::json!({ "name": "origin", "json": doc }).to_string();
        let (_, _, text) = call(&app, "POST", "/api/docs", Some(&body), Some(&bea)).await;
        let id = serde_json::from_str::<serde_json::Value>(&text).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        // Save a version, then change the document (rename it through a PUT).
        let (status, _, text) = call(
            &app,
            "POST",
            &format!("/api/docs/{id}/versions"),
            Some(r#"{"name":"v1"}"#),
            Some(&bea),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{text}");
        let vid = serde_json::from_str::<serde_json::Value>(&text).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        let mut changed = ok_model::Document::new("origin");
        changed.name = "changed".into();
        let (status, _, _) = call(
            &app,
            "PUT",
            &format!("/api/docs/{id}"),
            Some(&changed.to_json()),
            Some(&bea),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        // Branch from the version: the copy holds the old state and names its origin.
        let (status, _, text) = call(
            &app,
            "POST",
            &format!("/api/docs/{id}/branch"),
            Some(&format!(r#"{{"version":"{vid}"}}"#)),
            Some(&bea),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{text}");
        let meta: DocMeta = serde_json::from_str(&text).unwrap();
        assert_eq!(meta.name, "changed (v1)");
        let parent = meta.parent.unwrap();
        assert_eq!(parent.doc, id);
        assert_eq!(parent.version_name.as_deref(), Some("v1"));
        let (_, _, json) = call(
            &app,
            "GET",
            &format!("/api/docs/{}", meta.id),
            None,
            Some(&bea),
        )
        .await;
        assert_eq!(ok_model::Document::from_json(&json).unwrap().name, "origin");
        // Branch from the current state, with a name.
        let (status, _, text) = call(
            &app,
            "POST",
            &format!("/api/docs/{id}/branch"),
            Some(r#"{"name":"experiment"}"#),
            Some(&bea),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{text}");
        let meta: DocMeta = serde_json::from_str(&text).unwrap();
        assert_eq!(meta.name, "experiment");
        let (_, _, json) = call(
            &app,
            "GET",
            &format!("/api/docs/{}", meta.id),
            None,
            Some(&bea),
        )
        .await;
        assert_eq!(
            ok_model::Document::from_json(&json).unwrap().name,
            "changed"
        );
        // Strangers cannot branch a private document.
        let (_, tok2) = users.register("cal", "cals-password").unwrap();
        let (status, _, _) = call(
            &app,
            "POST",
            &format!("/api/docs/{id}/branch"),
            Some("{}"),
            Some(&format!("ok_session={tok2}")),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn teams_share_documents_with_every_member() {
        let users = temp_users();
        let teams = temp_teams();
        let app = router(temp_store(), users.clone(), teams.clone(), None);
        let (_, owner_tok) = users.register("olive", "olives-password").unwrap();
        let (_, mate_tok) = users.register("mia", "mias-password").unwrap();
        let (owner, mate) = (
            format!("ok_session={owner_tok}"),
            format!("ok_session={mate_tok}"),
        );
        // Olive makes a team and adds Mia; Mia cannot add anyone.
        let (status, _, text) = call(
            &app,
            "POST",
            "/api/teams",
            Some(r#"{"name":"Design"}"#),
            Some(&owner),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{text}");
        let team: crate::teams::Team = serde_json::from_str(&text).unwrap();
        let (status, _, text) = call(
            &app,
            "POST",
            &format!("/api/teams/{}/members", team.id),
            Some(r#"{"name":"mia"}"#),
            Some(&owner),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{text}");
        let (status, _, _) = call(
            &app,
            "POST",
            &format!("/api/teams/{}/members", team.id),
            Some(r#"{"name":"olive"}"#),
            Some(&mate),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (_, _, text) = call(&app, "GET", "/api/teams", None, Some(&mate)).await;
        assert!(text.contains("Design"));
        // A document shared with the team read-only: Mia can read, not write.
        let (_, _, text) = call(
            &app,
            "POST",
            "/api/docs",
            Some(r#"{"name":"plans"}"#),
            Some(&owner),
        )
        .await;
        let id = serde_json::from_str::<serde_json::Value>(&text).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        let (status, _, _) = call(&app, "GET", &format!("/api/docs/{id}"), None, Some(&mate)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _, text) = call(
            &app,
            "POST",
            &format!("/api/docs/{id}/share-team"),
            Some(&format!(r#"{{"team":"{}","role":"viewer"}}"#, team.id)),
            Some(&owner),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{text}");
        let (status, _, doc) =
            call(&app, "GET", &format!("/api/docs/{id}"), None, Some(&mate)).await;
        assert_eq!(status, StatusCode::OK);
        let (status, _, _) = call(
            &app,
            "PUT",
            &format!("/api/docs/{id}"),
            Some(&doc),
            Some(&mate),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        // As editors the team can write; leaving the team ends access.
        let (status, _, _) = call(
            &app,
            "POST",
            &format!("/api/docs/{id}/share-team"),
            Some(&format!(r#"{{"team":"{}","role":"editor"}}"#, team.id)),
            Some(&owner),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, _, _) = call(
            &app,
            "PUT",
            &format!("/api/docs/{id}"),
            Some(&doc),
            Some(&mate),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let mia_id = users.user_by_name("mia").unwrap().id;
        let (status, _, _) = call(
            &app,
            "DELETE",
            &format!("/api/teams/{}/members/{mia_id}", team.id),
            None,
            Some(&owner),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, _, _) = call(&app, "GET", &format!("/api/docs/{id}"), None, Some(&mate)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        // Unsharing the team and deleting it are the owner's alone.
        let (status, _, _) = call(
            &app,
            "DELETE",
            &format!("/api/docs/{id}/share-team/{}", team.id),
            None,
            Some(&owner),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, _, _) = call(
            &app,
            "DELETE",
            &format!("/api/teams/{}", team.id),
            None,
            Some(&mate),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, _, _) = call(
            &app,
            "DELETE",
            &format!("/api/teams/{}", team.id),
            None,
            Some(&owner),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn invitation_links_join_accounts_in_the_invited_role() {
        let users = temp_users();
        let app = router(temp_store(), users.clone(), temp_teams(), None);
        let (_, owner_tok) = users.register("olive", "olives-password").unwrap();
        let (_, guest_tok) = users.register("gus", "guss-password").unwrap();
        let (owner, guest) = (
            format!("ok_session={owner_tok}"),
            format!("ok_session={guest_tok}"),
        );
        let (_, _, text) = call(
            &app,
            "POST",
            "/api/docs",
            Some(r#"{"name":"plans"}"#),
            Some(&owner),
        )
        .await;
        let id = serde_json::from_str::<serde_json::Value>(&text).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        // Only the owner may create links; the guest cannot even see the document.
        let (status, _, _) = call(
            &app,
            "POST",
            &format!("/api/docs/{id}/invites"),
            Some(r#"{"role":"viewer"}"#),
            Some(&guest),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _, text) = call(
            &app,
            "POST",
            &format!("/api/docs/{id}/invites"),
            Some(r#"{"role":"viewer"}"#),
            Some(&owner),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{text}");
        let invite: crate::store::Invite = serde_json::from_str(&text).unwrap();
        assert_eq!(invite.token.len(), 64);
        // Anonymous callers are told to sign in; a wrong token is a 404.
        let (status, _, _) = call(
            &app,
            "POST",
            &format!("/api/docs/{id}/invites/{}/accept", invite.token),
            None,
            None,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, _, _) = call(
            &app,
            "POST",
            &format!("/api/docs/{id}/invites/deadbeef/accept"),
            None,
            Some(&guest),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        // The guest joins as a viewer and never sees the token list.
        let (status, _, text) = call(
            &app,
            "POST",
            &format!("/api/docs/{id}/invites/{}/accept", invite.token),
            None,
            Some(&guest),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{text}");
        let meta: DocMeta = serde_json::from_str(&text).unwrap();
        assert_eq!(meta.viewers.len(), 1);
        assert!(meta.invites.is_empty(), "{text}");
        let (_, _, text) = call(&app, "GET", "/api/docs", None, Some(&guest)).await;
        assert!(!text.contains(&invite.token));
        let (_, _, text) = call(&app, "GET", "/api/docs", None, Some(&owner)).await;
        assert!(text.contains(&invite.token));
        // An editor link promotes; revoking it stops further joins but keeps members.
        let (_, _, text) = call(
            &app,
            "POST",
            &format!("/api/docs/{id}/invites"),
            Some(r#"{}"#),
            Some(&owner),
        )
        .await;
        let editor: crate::store::Invite = serde_json::from_str(&text).unwrap();
        assert_eq!(editor.role, ShareRole::Editor);
        let (status, _, text) = call(
            &app,
            "POST",
            &format!("/api/docs/{id}/invites/{}/accept", editor.token),
            None,
            Some(&guest),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{text}");
        let meta: DocMeta = serde_json::from_str(&text).unwrap();
        assert!(meta.viewers.is_empty());
        assert_eq!(meta.collaborators.len(), 1);
        let (status, _, text) = call(
            &app,
            "DELETE",
            &format!("/api/docs/{id}/invites/{}", editor.token),
            None,
            Some(&owner),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{text}");
        let meta: DocMeta = serde_json::from_str(&text).unwrap();
        assert_eq!(meta.invites.len(), 1);
        assert_eq!(meta.collaborators.len(), 1);
        let (status, _, _) = call(
            &app,
            "POST",
            &format!("/api/docs/{id}/invites/{}/accept", editor.token),
            None,
            Some(&guest),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn accounts_own_and_share_documents() {
        let app = router(temp_store(), temp_users(), temp_teams(), None);
        // Anonymous: no session.
        let (st, _, _) = call(&app, "GET", "/api/auth/me", None, None).await;
        assert_eq!(st, StatusCode::UNAUTHORIZED);
        // Register alice and bob.
        let (st, h, text) = call(
            &app,
            "POST",
            "/api/auth/register",
            Some(r#"{"name":"alice","password":"hunter2hunter2"}"#),
            None,
        )
        .await;
        assert_eq!(st, StatusCode::OK, "{text}");
        let alice = cookie_of(&h);
        let (st, _, _) = call(
            &app,
            "POST",
            "/api/auth/register",
            Some(r#"{"name":"alice","password":"hunter2hunter2"}"#),
            None,
        )
        .await;
        assert_eq!(st, StatusCode::CONFLICT);
        let (_, h, _) = call(
            &app,
            "POST",
            "/api/auth/register",
            Some(r#"{"name":"bob","password":"bobs-password"}"#),
            None,
        )
        .await;
        let bob = cookie_of(&h);
        let (st, _, text) = call(&app, "GET", "/api/auth/me", None, Some(&alice)).await;
        assert_eq!(st, StatusCode::OK);
        assert!(text.contains(r#""name":"alice""#), "{text}");

        // Alice creates a document: she owns it, bob and anonymous cannot see it.
        let (st, _, text) = call(
            &app,
            "POST",
            "/api/docs",
            Some(r#"{"name":"secret"}"#),
            Some(&alice),
        )
        .await;
        assert_eq!(st, StatusCode::CREATED);
        let meta: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(meta["owner"]["name"], "alice");
        let id = meta["id"].as_str().unwrap().to_string();
        let (st, _, _) = call(&app, "GET", &format!("/api/docs/{id}"), None, None).await;
        assert_eq!(st, StatusCode::NOT_FOUND);
        let (st, _, _) = call(&app, "GET", &format!("/api/docs/{id}"), None, Some(&bob)).await;
        assert_eq!(st, StatusCode::NOT_FOUND);
        let (_, _, text) = call(&app, "GET", "/api/docs", None, Some(&bob)).await;
        assert_eq!(text, "[]");

        // Bob cannot share it; alice shares it with bob, who can then open but not delete it.
        let (st, _, _) = call(
            &app,
            "POST",
            &format!("/api/docs/{id}/share"),
            Some(r#"{"name":"bob"}"#),
            Some(&bob),
        )
        .await;
        assert_eq!(st, StatusCode::NOT_FOUND);
        let (st, _, text) = call(
            &app,
            "POST",
            &format!("/api/docs/{id}/share"),
            Some(r#"{"name":"BOB"}"#),
            Some(&alice),
        )
        .await;
        assert_eq!(st, StatusCode::OK, "{text}");
        let meta: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(meta["collaborators"][0]["name"], "bob");
        let bob_id = meta["collaborators"][0]["id"].as_str().unwrap().to_string();
        let (st, _, _) = call(&app, "GET", &format!("/api/docs/{id}"), None, Some(&bob)).await;
        assert_eq!(st, StatusCode::OK);
        let (st, _, _) = call(&app, "DELETE", &format!("/api/docs/{id}"), None, Some(&bob)).await;
        assert_eq!(st, StatusCode::FORBIDDEN);
        let (st, _, _) = call(
            &app,
            "POST",
            &format!("/api/docs/{id}/share"),
            Some(r#"{"name":"nobody"}"#),
            Some(&alice),
        )
        .await;
        assert_eq!(st, StatusCode::NOT_FOUND);
        // Unshare: bob loses access.
        let (st, _, _) = call(
            &app,
            "DELETE",
            &format!("/api/docs/{id}/share/{bob_id}"),
            None,
            Some(&alice),
        )
        .await;
        assert_eq!(st, StatusCode::OK);
        let (st, _, _) = call(&app, "GET", &format!("/api/docs/{id}"), None, Some(&bob)).await;
        assert_eq!(st, StatusCode::NOT_FOUND);

        // Anonymous documents stay open to everyone, signed in or not.
        let (st, _, text) = call(&app, "POST", "/api/docs", Some(r#"{"name":"open"}"#), None).await;
        assert_eq!(st, StatusCode::CREATED);
        let open: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(open.get("owner").is_none());
        let oid = open["id"].as_str().unwrap();
        let (st, _, _) = call(&app, "GET", &format!("/api/docs/{oid}"), None, Some(&bob)).await;
        assert_eq!(st, StatusCode::OK);

        // Logout invalidates the session; a wrong password is refused.
        let (st, h, _) = call(&app, "POST", "/api/auth/logout", None, Some(&alice)).await;
        assert_eq!(st, StatusCode::NO_CONTENT);
        assert!(h
            .get("set-cookie")
            .unwrap()
            .to_str()
            .unwrap()
            .contains("Max-Age=0"));
        let (st, _, _) = call(&app, "GET", "/api/auth/me", None, Some(&alice)).await;
        assert_eq!(st, StatusCode::UNAUTHORIZED);
        let (st, _, _) = call(
            &app,
            "POST",
            "/api/auth/login",
            Some(r#"{"name":"alice","password":"wrong-password"}"#),
            None,
        )
        .await;
        assert_eq!(st, StatusCode::UNAUTHORIZED);
        let (st, h, _) = call(
            &app,
            "POST",
            "/api/auth/login",
            Some(r#"{"name":"alice","password":"hunter2hunter2"}"#),
            None,
        )
        .await;
        assert_eq!(st, StatusCode::OK);
        let alice2 = cookie_of(&h);
        let (st, _, _) = call(&app, "GET", &format!("/api/docs/{id}"), None, Some(&alice2)).await;
        assert_eq!(st, StatusCode::OK);
    }

    #[tokio::test]
    async fn create_list_get_put_delete() {
        let app = router(temp_store(), temp_users(), temp_teams(), None);
        let res = app
            .clone()
            .oneshot(
                Request::post("/api/docs")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"bracket"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
        let meta: serde_json::Value =
            serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap();
        let id = meta["id"].as_str().unwrap().to_string();

        let res = app
            .clone()
            .oneshot(Request::get("/api/docs").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let list: Vec<serde_json::Value> =
            serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0]["name"], "bracket");

        let res = app
            .clone()
            .oneshot(
                Request::get(format!("/api/docs/{id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let json = String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec())
            .unwrap();
        let studio = ok_model::Document::from_json(&json).unwrap();
        assert_eq!(studio.name, "bracket");

        let demo = ok_model::PartStudio::demo().to_json();
        let res = app
            .clone()
            .oneshot(
                Request::put(format!("/api/docs/{id}"))
                    .body(Body::from(demo))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);
        let res = app
            .clone()
            .oneshot(
                Request::put(format!("/api/docs/{id}"))
                    .body(Body::from("nope"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);

        // Versions: save, list, read, restore.
        let res = app
            .clone()
            .oneshot(
                Request::post(format!("/api/docs/{id}/versions"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"v1"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
        let vmeta: serde_json::Value =
            serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap();
        let vid = vmeta["id"].as_str().unwrap().to_string();
        let res = app
            .clone()
            .oneshot(
                Request::get(format!("/api/docs/{id}/versions"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let versions: Vec<serde_json::Value> =
            serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0]["name"], "v1");
        let res = app
            .clone()
            .oneshot(
                Request::get(format!("/api/docs/{id}/versions/{vid}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        // Overwrite the document with an empty one, then restore v1 (the demo).
        let res = app
            .clone()
            .oneshot(
                Request::put(format!("/api/docs/{id}"))
                    .body(Body::from(ok_model::Document::new("empty").to_json()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);
        let res = app
            .clone()
            .oneshot(
                Request::post(format!("/api/docs/{id}/versions/{vid}/restore"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);
        let res = app
            .clone()
            .oneshot(
                Request::get(format!("/api/docs/{id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let json = String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec())
            .unwrap();
        assert_eq!(
            {
                let d = ok_model::Document::from_json(&json).unwrap();
                d.studio(d.first_studio().unwrap())
                    .unwrap()
                    .features()
                    .len()
            },
            6
        );

        let res = app
            .clone()
            .oneshot(
                Request::delete(format!("/api/docs/{id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);
        let res = app
            .clone()
            .oneshot(
                Request::get(format!("/api/docs/{id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn websocket_requires_access_and_uses_account_names() {
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        use tokio_tungstenite::tungstenite::Message as WsMessage;
        let store = temp_store();
        let users = temp_users();
        let (owner, tok) = users.register("carol", "carols-password").unwrap();
        let meta = store
            .create(
                "mine",
                &ok_model::Document::new("mine").to_json(),
                Some(crate::auth::UserInfo::from(&owner)),
            )
            .unwrap();
        let app = router(store, users, temp_teams(), None);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let url = format!("ws://{addr}/api/docs/{}/ws", meta.id);
        // Anonymous: refused.
        let err = tokio_tungstenite::connect_async(&url).await.err().unwrap();
        assert!(err.to_string().contains("404"), "{err}");
        // The owner connects and is listed under the account name, whatever
        // the hello says.
        let mut req = url.into_client_request().unwrap();
        req.headers_mut().insert(
            "cookie",
            format!("{}={tok}", crate::auth::COOKIE).parse().unwrap(),
        );
        let (mut ws, _) = tokio_tungstenite::connect_async(req).await.unwrap();
        ws.send(WsMessage::Text(
            r#"{"type":"hello","name":"someone"}"#.into(),
        ))
        .await
        .unwrap();
        let welcome: serde_json::Value =
            serde_json::from_str(&ws.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
        assert_eq!(welcome["type"], "welcome");
        let presence: serde_json::Value =
            serde_json::from_str(&ws.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
        assert_eq!(presence["names"], serde_json::json!(["carol"]));
    }

    #[tokio::test]
    async fn two_clients_see_each_others_ops() {
        use tokio_tungstenite::tungstenite::Message as WsMessage;
        let store = temp_store();
        let meta = store
            .create("shared", &ok_model::Document::new("shared").to_json(), None)
            .unwrap();
        let app = router(store, temp_users(), temp_teams(), None);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let url = format!("ws://{addr}/api/docs/{}/ws", meta.id);

        let (mut a, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
        a.send(WsMessage::Text(r#"{"type":"hello","name":"alice"}"#.into()))
            .await
            .unwrap();
        let welcome_a: serde_json::Value =
            serde_json::from_str(&a.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
        assert_eq!(welcome_a["type"], "welcome");
        assert_eq!(welcome_a["prefix"], 1);
        // Alice's own presence update (1 online).
        let presence_a: serde_json::Value =
            serde_json::from_str(&a.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
        assert_eq!(presence_a["clients"], 1);

        let (mut b, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
        b.send(WsMessage::Text(r#"{"type":"hello","name":"bob"}"#.into()))
            .await
            .unwrap();
        let welcome_b: serde_json::Value =
            serde_json::from_str(&b.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
        assert_eq!(welcome_b["clients"], 2);
        assert_eq!(welcome_b["prefix"], 2);
        // Alice is told about Bob joining; Bob's own presence follows his welcome.
        let presence: serde_json::Value =
            serde_json::from_str(&a.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
        assert_eq!(presence["type"], "presence");
        assert_eq!(presence["clients"], 2);
        let presence_b: serde_json::Value =
            serde_json::from_str(&b.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
        assert_eq!(presence_b["type"], "presence");

        // Alice adds a sketch; both receive the op with seq 1.
        a.send(WsMessage::Text(r#"{"type":"op","id":7,"base":1048576,"op":{"type":"studio","tab":1,"op":{"type":"add_sketch","plane":{"type":"standard","base":"top"},"name":null}}}"#.into())).await.unwrap();
        let mut got_a = None;
        let mut got_b = None;
        for _ in 0..4 {
            if got_a.is_none() {
                let m: serde_json::Value =
                    serde_json::from_str(&a.next().await.unwrap().unwrap().into_text().unwrap())
                        .unwrap();
                if m["type"] == "op" {
                    got_a = Some(m);
                }
            }
            if got_b.is_none() {
                let m: serde_json::Value =
                    serde_json::from_str(&b.next().await.unwrap().unwrap().into_text().unwrap())
                        .unwrap();
                if m["type"] == "op" {
                    got_b = Some(m);
                }
            }
            if got_a.is_some() && got_b.is_some() {
                break;
            }
        }
        let (ma, mb) = (got_a.unwrap(), got_b.unwrap());
        assert_eq!(ma["seq"], 1);
        assert_eq!(mb["seq"], 1);
        assert_eq!(mb["id"], 7);
        assert_eq!(mb["from"], welcome_a["client"]);
        assert_eq!(mb["base"], 1048576);
        assert_eq!(mb["hash"].as_str().unwrap().len(), 16);

        // A bad op is rejected for the sender only.
        b.send(WsMessage::Text(
            r#"{"type":"op","id":8,"op":{"type":"studio","tab":1,"op":{"type":"delete_feature","id":999}}}"#.into(),
        ))
        .await
        .unwrap();
        let err: serde_json::Value =
            serde_json::from_str(&b.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
        assert_eq!(err["type"], "error");
        assert_eq!(err["id"], 8);

        // Snapshot reflects the applied op.
        b.send(WsMessage::Text(r#"{"type":"snapshot"}"#.into()))
            .await
            .unwrap();
        let snap: serde_json::Value =
            serde_json::from_str(&b.next().await.unwrap().unwrap().into_text().unwrap()).unwrap();
        assert_eq!(snap["type"], "snapshot");
        let doc = ok_model::Document::from_json(snap["doc"].as_str().unwrap()).unwrap();
        let studio = doc.studio(doc.first_studio().unwrap()).unwrap();
        assert_eq!(studio.features().len(), 1);
        assert_eq!(
            studio.features()[0].id.0,
            1048576,
            "id allocated from the client's base"
        );
    }
}
