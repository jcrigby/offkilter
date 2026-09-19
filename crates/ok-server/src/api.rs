//! HTTP and WebSocket routes.

use crate::live::{ClientMessage, DocHub, ServerMessage};
use crate::store::DocStore;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::path::PathBuf;
use tower_http::services::{ServeDir, ServeFile};

pub fn router(store: DocStore, static_dir: Option<PathBuf>) -> Router {
    let hub = DocHub::new(store);
    let api = Router::new()
        .route("/docs", get(list_docs).post(create_doc))
        .route("/docs/{id}", get(get_doc).put(put_doc).delete(delete_doc))
        .route("/docs/{id}/ws", get(ws_upgrade))
        .route("/docs/{id}/versions", get(list_versions).post(save_version))
        .route("/docs/{id}/versions/{vid}", get(get_version))
        .route(
            "/docs/{id}/versions/{vid}/restore",
            axum::routing::post(restore_version),
        )
        .route("/health", get(|| async { "ok" }))
        .with_state(hub);
    let mut app = Router::new().nest("/api", api);
    if let Some(dir) = static_dir {
        let index = dir.join("index.html");
        app = app.fallback_service(ServeDir::new(dir).not_found_service(ServeFile::new(index)));
    }
    app.layer(tower_http::cors::CorsLayer::permissive())
}

async fn list_docs(State(hub): State<DocHub>) -> impl IntoResponse {
    match hub.store().list() {
        Ok(list) => Json(list).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
struct CreateDoc {
    name: String,
    #[serde(default)]
    json: Option<String>,
}

async fn create_doc(State(hub): State<DocHub>, Json(body): Json<CreateDoc>) -> impl IntoResponse {
    let json = match body.json {
        Some(j) => match ok_model::PartStudio::from_json(&j) {
            Ok(_) => j,
            Err(e) => {
                return (StatusCode::BAD_REQUEST, format!("invalid document: {e}")).into_response()
            }
        },
        None => ok_model::PartStudio::new(body.name.clone()).to_json(),
    };
    match hub.store().create(&body.name, &json) {
        Ok(meta) => (StatusCode::CREATED, Json(meta)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_doc(State(hub): State<DocHub>, Path(id): Path<String>) -> impl IntoResponse {
    match hub.store().read(&id) {
        Some(json) => (
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            json,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn put_doc(
    State(hub): State<DocHub>,
    Path(id): Path<String>,
    body: String,
) -> impl IntoResponse {
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

async fn delete_doc(State(hub): State<DocHub>, Path(id): Path<String>) -> impl IntoResponse {
    hub.evict(&id);
    match hub.store().delete(&id) {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::NOT_FOUND,
    }
}

async fn list_versions(State(hub): State<DocHub>, Path(id): Path<String>) -> impl IntoResponse {
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
    Path(id): Path<String>,
    Json(body): Json<SaveVersion>,
) -> impl IntoResponse {
    match hub.store().save_version(&id, &body.name) {
        Ok(meta) => (StatusCode::CREATED, Json(meta)).into_response(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => StatusCode::NOT_FOUND.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_version(
    State(hub): State<DocHub>,
    Path((id, vid)): Path<(String, String)>,
) -> impl IntoResponse {
    match hub.store().read_version(&id, &vid) {
        Some(json) => (
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            json,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Restores a version: connected clients receive a replace_document op.
async fn restore_version(
    State(hub): State<DocHub>,
    Path((id, vid)): Path<(String, String)>,
) -> impl IntoResponse {
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
    Path(id): Path<String>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    match hub.get(&id) {
        Some(doc) => ws
            .on_upgrade(move |socket| handle_socket(socket, hub, doc))
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn handle_socket(socket: WebSocket, hub: DocHub, doc: std::sync::Arc<crate::live::LiveDoc>) {
    let (mut sink, mut stream) = socket.split();
    // First message may be a hello with a display name.
    let mut name: Option<String> = None;
    let first = stream.next().await;
    let mut pending_after_hello: Option<ClientMessage> = None;
    if let Some(Ok(Message::Text(text))) = &first {
        match serde_json::from_str::<ClientMessage>(text) {
            Ok(ClientMessage::Hello { name: n }) => name = n,
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

    #[tokio::test]
    async fn create_list_get_put_delete() {
        let app = router(temp_store(), None);
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
    async fn two_clients_see_each_others_ops() {
        use tokio_tungstenite::tungstenite::Message as WsMessage;
        let store = temp_store();
        let meta = store
            .create("shared", &ok_model::PartStudio::new("shared").to_json())
            .unwrap();
        let app = router(store, None);
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
