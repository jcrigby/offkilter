//! HTTP and WebSocket routes.
//!
//! Signing in is optional. Documents created anonymously have no owner and
//! are open to everyone on the server; documents created while signed in
//! belong to that account and are visible only to the owner and the people
//! it was shared with.

use crate::auth::{clear_cookie, session_cookie, AuthError, CurrentUser, User, UserStore};
use crate::live::{ClientMessage, DocHub, ServerMessage};
use crate::store::{DocMeta, DocStore};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{FromRef, Path, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::path::PathBuf;
use tower_http::services::{ServeDir, ServeFile};

#[derive(Clone)]
pub struct AppState {
    hub: DocHub,
    users: UserStore,
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

pub fn router(store: DocStore, users: UserStore, static_dir: Option<PathBuf>) -> Router {
    let state = AppState {
        hub: DocHub::new(store),
        users,
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
        .route("/docs/{id}/ws", get(ws_upgrade))
        .route("/docs/{id}/versions", get(list_versions).post(save_version))
        .route("/docs/{id}/versions/{vid}", get(get_version))
        .route("/docs/{id}/versions/{vid}/restore", post(restore_version))
        .route("/health", get(|| async { "ok" }))
        .with_state(state);
    let mut app = Router::new().nest("/api", api);
    if let Some(dir) = static_dir {
        let index = dir.join("index.html");
        app = app.fallback_service(ServeDir::new(dir).not_found_service(ServeFile::new(index)));
    }
    app.layer(tower_http::cors::CorsLayer::permissive())
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

async fn register(State(users): State<UserStore>, Json(c): Json<Credentials>) -> impl IntoResponse {
    signed_in(users.register(&c.name, &c.password))
}

async fn login(State(users): State<UserStore>, Json(c): Json<Credentials>) -> impl IntoResponse {
    signed_in(users.login(&c.name, &c.password))
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
        Some(j) => match ok_model::PartStudio::from_json(&j) {
            Ok(_) => j,
            Err(e) => {
                return (StatusCode::BAD_REQUEST, format!("invalid document: {e}")).into_response()
            }
        },
        None => ok_model::PartStudio::new(body.name.clone()).to_json(),
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
        Ok(meta) => Json(meta).into_response(),
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

async fn put_doc(
    State(hub): State<DocHub>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<String>,
    body: String,
) -> impl IntoResponse {
    if let Err(code) = accessible(&hub, &id, user.as_ref()) {
        return code.into_response();
    }
    let studio = match ok_model::PartStudio::from_json(&body) {
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
    match state
        .hub
        .store()
        .share(&id, crate::auth::UserInfo::from(&target))
    {
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
    if let Err(code) = accessible(&hub, &id, user.as_ref()) {
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
    if let Err(code) = accessible(&hub, &id, user.as_ref()) {
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
    if let Err(code) = accessible(&hub, &id, user.as_ref()) {
        return code.into_response();
    }
    match hub.get(&id) {
        Some(doc) => ws
            .on_upgrade(move |socket| handle_socket(socket, hub, doc, user))
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn handle_socket(
    socket: WebSocket,
    hub: DocHub,
    doc: std::sync::Arc<crate::live::LiveDoc>,
    user: Option<User>,
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
    let (client, welcome) = doc.join(name);
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
            ClientMessage::Op { op, id, base } => match doc.apply(client, id, op, base) {
                Ok(_) => None,
                Err(message) => Some(ServerMessage::Error { message, id }),
            },
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
    async fn accounts_own_and_share_documents() {
        let app = router(temp_store(), temp_users(), None);
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
        let app = router(temp_store(), temp_users(), None);
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
        let studio = ok_model::PartStudio::from_json(&json).unwrap();
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
                    .body(Body::from(ok_model::PartStudio::new("empty").to_json()))
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
            ok_model::PartStudio::from_json(&json)
                .unwrap()
                .features()
                .len(),
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
                &ok_model::PartStudio::new("mine").to_json(),
                Some(crate::auth::UserInfo::from(&owner)),
            )
            .unwrap();
        let app = router(store, users, None);
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
            .create(
                "shared",
                &ok_model::PartStudio::new("shared").to_json(),
                None,
            )
            .unwrap();
        let app = router(store, temp_users(), None);
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
        a.send(WsMessage::Text(r#"{"type":"op","id":7,"base":1048576,"op":{"type":"add_sketch","plane":{"type":"standard","base":"top"},"name":null}}"#.into())).await.unwrap();
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
            r#"{"type":"op","id":8,"op":{"type":"delete_feature","id":999}}"#.into(),
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
        let studio = ok_model::PartStudio::from_json(snap["doc"].as_str().unwrap()).unwrap();
        assert_eq!(studio.features().len(), 1);
        assert_eq!(
            studio.features()[0].id.0,
            1048576,
            "id allocated from the client's base"
        );
    }
}
