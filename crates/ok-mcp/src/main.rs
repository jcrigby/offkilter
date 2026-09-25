//! `ok-mcp`: a Model Context Protocol server (JSON-RPC over stdio) that
//! lets a language model drive offkilter documents.
//!
//! Two backends:
//!
//! - `--server http://localhost:8080` edits documents on a running
//!   `ok-server`, so every op shows up live in the browser and the model
//!   and a person can work on one document together. `--cookie` passes a
//!   session cookie (`ok_session=...`) or `--login name:password` signs in.
//! - `--file part.okpart` works on a local document file with the kernel
//!   embedded; no server needed.
//!
//! Tools: `offkilter_reference` (the op catalogue), `list_documents`,
//! `create_document`, `open_document`, `report`, `apply`, `screenshot`,
//! `import`, `export`, `document_url`. See docs/MCP.md.

use serde_json::{json, Value};
use std::io::{BufRead, Write};

const REFERENCE: &str = include_str!("../../../docs/OPS.md");
const PROTOCOL: &str = "2024-11-05";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let backend = match Backend::from_args(&args) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("ok-mcp: {e}");
            eprintln!("usage: ok-mcp --server URL [--cookie ok_session=TOKEN | --login NAME:PASSWORD] [--doc ID]");
            eprintln!("       ok-mcp --file PATH.okpart");
            std::process::exit(2);
        }
    };
    let mut server = Server::new(backend);
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = server.handle_line(&line) {
            let _ = writeln!(out, "{response}");
            let _ = out.flush();
        }
    }
}

// ---------------------------------------------------------------------------
// Backends

enum Backend {
    Server {
        agent: ureq::Agent,
        base: String,
        cookie: Option<String>,
    },
    Local {
        path: std::path::PathBuf,
        doc: ok_model::Document,
    },
}

impl Backend {
    fn from_args(args: &[String]) -> Result<(Backend, Option<String>), String> {
        let mut server = None;
        let mut file = None;
        let mut cookie = None;
        let mut login = None;
        let mut doc = None;
        let mut i = 0;
        while i < args.len() {
            let next = |i: usize| -> Result<String, String> {
                args.get(i + 1)
                    .cloned()
                    .ok_or_else(|| format!("{} needs a value", args[i]))
            };
            match args[i].as_str() {
                "--server" => server = Some(next(i)?),
                "--file" => file = Some(next(i)?),
                "--cookie" => cookie = Some(next(i)?),
                "--login" => login = Some(next(i)?),
                "--doc" => doc = Some(next(i)?),
                other => return Err(format!("unknown argument {other}")),
            }
            i += 2;
        }
        match (server, file) {
            (Some(base), None) => {
                let agent: ureq::Agent = ureq::Agent::config_builder()
                    .http_status_as_error(false)
                    .build()
                    .into();
                let base = base.trim_end_matches('/').to_string();
                let mut backend = Backend::Server {
                    agent,
                    base,
                    cookie,
                };
                if let Some(login) = login {
                    let (name, password) =
                        login.split_once(':').ok_or("--login takes NAME:PASSWORD")?;
                    backend.sign_in(name, password)?;
                }
                Ok((backend, doc))
            }
            (None, Some(path)) => {
                let path = std::path::PathBuf::from(path);
                let doc = if path.exists() {
                    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
                    ok_model::Document::from_json(&text).map_err(|e| e.to_string())?
                } else {
                    let name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("document")
                        .to_string();
                    ok_model::Document::new(name)
                };
                Ok((Backend::Local { path, doc }, None))
            }
            (None, None) => Err("give --server URL or --file PATH".into()),
            (Some(_), Some(_)) => Err("give either --server or --file, not both".into()),
        }
    }

    fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
    ) -> Result<(u16, String), String> {
        let Backend::Server {
            agent,
            base,
            cookie,
        } = self
        else {
            return Err("not connected to a server".into());
        };
        let url = format!("{base}/api{path}");
        let mut response = match (method, body) {
            ("GET", _) => {
                let mut req = agent.get(&url);
                if let Some(c) = cookie {
                    req = req.header("cookie", c);
                }
                req.call()
            }
            ("POST", body) => {
                let mut req = agent.post(&url).header("content-type", "application/json");
                if let Some(c) = cookie {
                    req = req.header("cookie", c);
                }
                req.send(body.unwrap_or(json!({})).to_string().as_bytes())
            }
            _ => return Err(format!("unsupported method {method}")),
        }
        .map_err(|e| format!("{method} {url}: {e}"))?;
        let status = response.status().as_u16();
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| e.to_string())?;
        Ok((status, text))
    }

    fn request_bytes(&self, path: &str) -> Result<(u16, Vec<u8>), String> {
        let Backend::Server {
            agent,
            base,
            cookie,
        } = self
        else {
            return Err("not connected to a server".into());
        };
        let url = format!("{base}/api{path}");
        let mut req = agent.get(&url);
        if let Some(c) = cookie {
            req = req.header("cookie", c);
        }
        let mut response = req.call().map_err(|e| format!("GET {url}: {e}"))?;
        let status = response.status().as_u16();
        let bytes = response
            .body_mut()
            .read_to_vec()
            .map_err(|e| e.to_string())?;
        Ok((status, bytes))
    }

    fn sign_in(&mut self, name: &str, password: &str) -> Result<(), String> {
        let Backend::Server {
            agent,
            base,
            cookie,
        } = self
        else {
            return Ok(());
        };
        let url = format!("{base}/api/login");
        let body = json!({ "name": name, "password": password }).to_string();
        let response = agent
            .post(&url)
            .header("content-type", "application/json")
            .send(body.as_bytes())
            .map_err(|e| format!("login: {e}"))?;
        if response.status().as_u16() >= 300 {
            return Err(format!("login failed with status {}", response.status()));
        }
        let set = response
            .headers()
            .get("set-cookie")
            .and_then(|v| v.to_str().ok())
            .ok_or("login gave no session cookie")?;
        let session = set.split(';').next().unwrap_or("").trim().to_string();
        *cookie = Some(session);
        Ok(())
    }

    fn expect_ok(status: u16, text: String) -> Result<String, String> {
        if (200..300).contains(&status) {
            Ok(text)
        } else {
            Err(format!("server said {status}: {text}"))
        }
    }

    fn save_local(&self) -> Result<(), String> {
        if let Backend::Local { path, doc } = self {
            std::fs::write(path, doc.to_json()).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    // ---- operations shared by the tools

    fn list(&self) -> Result<Value, String> {
        match self {
            Backend::Server { .. } => {
                let (s, t) = self.request("GET", "/docs", None)?;
                serde_json::from_str(&Self::expect_ok(s, t)?).map_err(|e| e.to_string())
            }
            Backend::Local { path, doc } => Ok(json!([{
                "id": path.display().to_string(),
                "name": doc.name,
                "tabs": tabs_of(doc),
            }])),
        }
    }

    fn create(&mut self, name: &str) -> Result<(String, Value), String> {
        match self {
            Backend::Server { .. } => {
                let json = ok_model::Document::new(name).to_json();
                let (s, t) =
                    self.request("POST", "/docs", Some(json!({ "name": name, "json": json })))?;
                let meta: Value =
                    serde_json::from_str(&Self::expect_ok(s, t)?).map_err(|e| e.to_string())?;
                let id = meta["id"].as_str().unwrap_or("").to_string();
                Ok((id, meta))
            }
            Backend::Local { path, doc } => {
                *doc = ok_model::Document::new(name);
                let file = path.with_file_name(format!("{}.okpart", safe_name(name)));
                *path = file.clone();
                self.save_local()?;
                Ok((
                    file.display().to_string(),
                    json!({ "path": file.display().to_string() }),
                ))
            }
        }
    }

    fn document(&self, id: &str) -> Result<ok_model::Document, String> {
        match self {
            Backend::Server { .. } => {
                let (s, t) = self.request("GET", &format!("/docs/{id}"), None)?;
                ok_model::Document::from_json(&Self::expect_ok(s, t)?).map_err(|e| e.to_string())
            }
            Backend::Local { doc, .. } => Ok(doc.clone()),
        }
    }

    /// The tab's report, built by the kernel from the current document
    /// (the server's own `/report` gives the same to other scripts).
    fn report(&self, id: &str, tab: Option<u32>) -> Result<ok_model::TabReport, String> {
        let mut doc = self.document(id)?;
        let tab = pick_tab(&doc, tab)?;
        doc.describe(tab)
    }

    fn apply(&mut self, id: &str, ops: Vec<Value>) -> Result<Value, String> {
        match self {
            Backend::Server { .. } => {
                let (s, t) = self.request(
                    "POST",
                    &format!("/docs/{id}/ops"),
                    Some(json!({ "ops": ops })),
                )?;
                serde_json::from_str(&Self::expect_ok(s, t)?).map_err(|e| e.to_string())
            }
            Backend::Local { doc, .. } => {
                let mut results = Vec::new();
                let mut error = None;
                for (i, op) in ops.into_iter().enumerate() {
                    match doc.apply_json_with_base(&op.to_string(), None) {
                        Ok(r) => results.push(serde_json::to_value(r).unwrap_or_default()),
                        Err(e) => {
                            error = Some(format!("op {i} failed: {e}"));
                            break;
                        }
                    }
                }
                self.save_local()?;
                let mut out = json!({ "results": results });
                if let Some(e) = error {
                    out["error"] = Value::String(e);
                }
                Ok(out)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn export(
        &self,
        id: &str,
        tab: Option<u32>,
        format: &str,
        view: ok_render::View,
        hidden: bool,
        body: Option<&str>,
        sheet: &ok_sheet::Options,
    ) -> Result<Vec<u8>, String> {
        match self {
            Backend::Server { .. } => {
                let mut query: Vec<String> = tab.map(|t| format!("tab={t}")).into_iter().collect();
                if format == "dxf" {
                    query.push(format!("view={}", view_name(view)));
                    query.push(format!("hidden={hidden}"));
                }
                if format == "pdf" {
                    query.push(format!("views={}", urlencode(&sheet.views.join(","))));
                    query.push(format!("sheet={}", sheet.sheet.name()));
                    query.push(format!("parts={}", sheet.parts));
                    if !sheet.note.is_empty() {
                        query.push(format!("note={}", urlencode(&sheet.note)));
                    }
                }
                if let Some(b) = body {
                    query.push(format!("body={}", urlencode(b)));
                }
                let query = if query.is_empty() {
                    String::new()
                } else {
                    format!("?{}", query.join("&"))
                };
                let (s, bytes) =
                    self.request_bytes(&format!("/docs/{id}/export/{format}{query}"))?;
                if (200..300).contains(&s) {
                    Ok(bytes)
                } else {
                    Err(format!(
                        "server said {s}: {}",
                        String::from_utf8_lossy(&bytes)
                    ))
                }
            }
            Backend::Local { doc, .. } => {
                let mut doc = doc.clone();
                let tab = pick_tab(&doc, tab)?;
                if format == "dxf" {
                    return ok_render::view_dxf(&mut doc, tab, view, hidden)
                        .map(String::into_bytes);
                }
                if format == "pdf" {
                    return ok_sheet::drawing_pdf(&mut doc, tab, sheet);
                }
                let kind = doc.tab(tab).map(|t| t.kind_name()).ok_or("no such tab")?;
                let mut bodies = if kind == "assembly" {
                    doc.regenerate_assembly(tab)
                        .map_err(|e| e.to_string())?
                        .bodies
                } else {
                    doc.regenerate_studio(tab, None)
                        .map_err(|e| e.to_string())?
                        .bodies
                };
                if let Some(want) = body {
                    let index: Option<usize> = want.parse().ok();
                    bodies = bodies
                        .into_iter()
                        .enumerate()
                        .filter(|(i, b)| b.name == want || index == Some(*i))
                        .map(|(_, b)| b)
                        .collect();
                    if bodies.is_empty() {
                        return Err(format!("no body {want:?} in the tab"));
                    }
                }
                match format {
                    "stl" => {
                        let meshes: Vec<ok_mesh::TriMesh> =
                            bodies.iter().map(|b| b.mesh.clone()).collect();
                        Ok(ok_mesh::to_stl(
                            &meshes,
                            &format!("offkilter tab {}", tab.0),
                        ))
                    }
                    "step" => {
                        let solids: Vec<(&str, &ok_brep::Solid)> =
                            bodies.iter().map(|b| (b.name.as_str(), &b.solid)).collect();
                        Ok(ok_step::write_step(&solids, &doc.name).into_bytes())
                    }
                    other => Err(format!("unknown format {other}; use stl, step, dxf or pdf")),
                }
            }
        }
    }

    /// A PNG of the tab, rendered by the server or by the embedded kernel.
    fn screenshot(
        &self,
        id: &str,
        tab: Option<u32>,
        options: &ok_render::Options,
    ) -> Result<Vec<u8>, String> {
        match self {
            Backend::Server { .. } => {
                let mut query = vec![
                    format!("width={}", options.width),
                    format!("height={}", options.height),
                    format!("edges={}", options.edges),
                ];
                if let Some(t) = tab {
                    query.push(format!("tab={t}"));
                }
                query.push(format!("view={}", view_name(options.view)));
                if let Some(s) = &options.section {
                    query.push(format!("section={}", section_name(s)));
                }
                let (s, bytes) =
                    self.request_bytes(&format!("/docs/{id}/screenshot?{}", query.join("&")))?;
                if (200..300).contains(&s) {
                    Ok(bytes)
                } else {
                    Err(format!(
                        "server said {s}: {}",
                        String::from_utf8_lossy(&bytes)
                    ))
                }
            }
            Backend::Local { doc, .. } => {
                let mut doc = doc.clone();
                let tab = pick_tab(&doc, tab)?;
                ok_render::screenshot(&mut doc, tab, options)
            }
        }
    }

    fn url(&self, id: &str) -> Option<String> {
        match self {
            Backend::Server { base, .. } => Some(format!("{base}/?doc={id}")),
            Backend::Local { .. } => None,
        }
    }
}

type Neighbours = std::collections::HashMap<(u32, u32, u32), [u32; ok_model::NEAR]>;

/// Adds `near` to every face reference in an op that lacks it, from the
/// report's references (matched by feature, local and piece number).
fn attach_neighbours(value: &mut Value, neighbours: &Neighbours) {
    match value {
        Value::Object(map) => {
            let is_ref = map.get("feature").is_some_and(|f| f.is_u64())
                && map.get("local").is_some_and(|l| l.is_u64())
                && map
                    .keys()
                    .all(|k| matches!(k.as_str(), "feature" | "local" | "part" | "near"));
            if is_ref && !map.contains_key("near") {
                let feature = map["feature"].as_u64().unwrap() as u32;
                let local = map["local"].as_u64().unwrap() as u32;
                let part = map.get("part").and_then(|p| p.as_u64()).unwrap_or(0) as u32;
                if let Some(near) = neighbours.get(&(feature, local, part)) {
                    if !ok_model::no_near(near) {
                        map.insert("near".into(), json!(near));
                    }
                }
                return;
            }
            for v in map.values_mut() {
                attach_neighbours(v, neighbours);
            }
        }
        Value::Array(items) => {
            for v in items {
                attach_neighbours(v, neighbours);
            }
        }
        _ => {}
    }
}

/// Percent-encodes a query value (names with spaces, say).
fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn view_name(view: ok_render::View) -> String {
    match view {
        ok_render::View::Top => "top".into(),
        ok_render::View::Front => "front".into(),
        ok_render::View::Right => "right".into(),
        ok_render::View::Iso => "iso".into(),
        ok_render::View::Direction(d) => format!("{},{},{}", d.x, d.y, d.z),
    }
}

fn section_name(s: &ok_render::Section) -> String {
    let axis = if s.axis.x != 0.0 {
        "x"
    } else if s.axis.y != 0.0 {
        "y"
    } else {
        "z"
    };
    format!("{axis}:{}{}", s.offset, if s.flip { ":flip" } else { "" })
}

/// Standard base64 (RFC 4648) with padding, for image content.
fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16
            | (chunk.get(1).copied().unwrap_or(0) as u32) << 8
            | chunk.get(2).copied().unwrap_or(0) as u32;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// What a tool call produced: text, or an image with a caption.
enum ToolOut {
    Text(String),
    Image { png: Vec<u8>, caption: String },
}

impl From<String> for ToolOut {
    fn from(text: String) -> Self {
        ToolOut::Text(text)
    }
}

fn tabs_of(doc: &ok_model::Document) -> Value {
    Value::Array(
        doc.tabs
            .iter()
            .map(|t| json!({ "id": t.id.0, "name": t.name(), "kind": t.kind_name() }))
            .collect(),
    )
}

fn pick_tab(doc: &ok_model::Document, tab: Option<u32>) -> Result<ok_model::TabId, String> {
    match tab {
        Some(t) => {
            let id = ok_model::TabId(t);
            doc.tab(id).map(|_| id).ok_or_else(|| format!("no tab {t}"))
        }
        None => doc
            .tabs
            .first()
            .map(|t| t.id)
            .ok_or_else(|| "document has no tabs".into()),
    }
}

fn safe_name(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    if s.is_empty() {
        "document".into()
    } else {
        s
    }
}

// ---------------------------------------------------------------------------
// The MCP server

struct Server {
    backend: Backend,
    /// The document tools act on when a call names none.
    current: Option<String>,
}

const DOC_OPS: &[&str] = &[
    "add_part_studio",
    "add_assembly",
    "rename_tab",
    "delete_tab",
    "insert_tab",
    "rename_document",
    "set_drawing_dimensions",
    "studio",
    "assembly",
    "replace_document",
];
const ASSEMBLY_OPS: &[&str] = &[
    "add_instance",
    "remove_instance",
    "set_instance",
    "add_mate",
    "set_mate",
    "remove_mate",
    "restore",
];

impl Server {
    fn new((backend, current): (Backend, Option<String>)) -> Server {
        let current = current.or_else(|| match &backend {
            Backend::Local { path, .. } => Some(path.display().to_string()),
            Backend::Server { .. } => None,
        });
        Server { backend, current }
    }

    /// Handles one JSON-RPC line; notifications get no response.
    fn handle_line(&mut self, line: &str) -> Option<String> {
        let request: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                return Some(
                    json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": format!("parse error: {e}") } })
                        .to_string(),
                )
            }
        };
        let id = request.get("id").cloned();
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = request.get("params").cloned().unwrap_or(json!({}));
        let result = self.dispatch(method, &params);
        let id = id?; // a notification
        let response = match result {
            Ok(v) => json!({ "jsonrpc": "2.0", "id": id, "result": v }),
            Err((code, message)) => {
                json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
            }
        };
        Some(response.to_string())
    }

    fn dispatch(&mut self, method: &str, params: &Value) -> Result<Value, (i64, String)> {
        match method {
            "initialize" => Ok(json!({
                "protocolVersion": PROTOCOL,
                "capabilities": { "tools": {}, "resources": {} },
                "serverInfo": { "name": "offkilter", "version": env!("CARGO_PKG_VERSION") },
                "instructions": "Parametric CAD. Call offkilter_reference first to learn the op format, then create_document, apply ops in small batches, and read the report between steps; faces are named by the references the report lists."
            })),
            "ping" => Ok(json!({})),
            "notifications/initialized" | "notifications/cancelled" => Ok(Value::Null),
            "tools/list" => Ok(json!({ "tools": tool_list() })),
            "tools/call" => {
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                Ok(match self.call_tool(name, &args) {
                    Ok(ToolOut::Text(text)) => {
                        json!({ "content": [{ "type": "text", "text": text }] })
                    }
                    Ok(ToolOut::Image { png, caption }) => json!({ "content": [
                        { "type": "image", "data": base64(&png), "mimeType": "image/png" },
                        { "type": "text", "text": caption }
                    ] }),
                    Err(e) => {
                        json!({ "content": [{ "type": "text", "text": e }], "isError": true })
                    }
                })
            }
            "resources/list" => Ok(json!({ "resources": [{
                "uri": "offkilter://reference/ops",
                "name": "offkilter op reference",
                "mimeType": "text/markdown",
                "description": "How to write document ops: envelopes, feature and sketch ops, references."
            }] })),
            "resources/read" => {
                let uri = params.get("uri").and_then(|u| u.as_str()).unwrap_or("");
                if uri == "offkilter://reference/ops" {
                    Ok(
                        json!({ "contents": [{ "uri": uri, "mimeType": "text/markdown", "text": REFERENCE }] }),
                    )
                } else {
                    Err((-32602, format!("unknown resource {uri}")))
                }
            }
            "prompts/list" => Ok(json!({ "prompts": [] })),
            other => Err((-32601, format!("method not found: {other}"))),
        }
    }

    fn doc_id(&self, args: &Value) -> Result<String, String> {
        args.get("doc")
            .and_then(|d| d.as_str())
            .map(str::to_string)
            .or_else(|| self.current.clone())
            .ok_or_else(|| {
                "no document: call create_document or open_document first, or pass doc".into()
            })
    }

    fn call_tool(&mut self, name: &str, args: &Value) -> Result<ToolOut, String> {
        self.call_tool_inner(name, args)
    }

    fn call_tool_inner(&mut self, name: &str, args: &Value) -> Result<ToolOut, String> {
        let tab = args.get("tab").and_then(|t| t.as_u64()).map(|t| t as u32);
        if name == "screenshot" {
            let id = self.doc_id(args)?;
            let text = |key: &str| args.get(key).and_then(|v| v.as_str()).unwrap_or("");
            let size = |key: &str, default: usize| {
                args.get(key)
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(default)
            };
            let options = ok_render::Options {
                width: size("width", 640),
                height: size("height", 480),
                view: ok_render::View::parse(text("view"))?,
                section: match text("section") {
                    "" => None,
                    s => Some(ok_render::Section::parse(s)?),
                },
                ..ok_render::Options::default()
            };
            let png = self.backend.screenshot(&id, tab, &options)?;
            let mut caption = format!(
                "{} view of the tab, {}x{}{}",
                view_name(options.view),
                options.width,
                options.height,
                options
                    .section
                    .as_ref()
                    .map(|s| format!(", sectioned at {}", section_name(s)))
                    .unwrap_or_default()
            );
            if let Some(path) = args.get("path").and_then(|p| p.as_str()) {
                std::fs::write(path, &png).map_err(|e| format!("could not write {path}: {e}"))?;
                caption.push_str(&format!("; written to {path}"));
            }
            return Ok(ToolOut::Image { png, caption });
        }
        if name == "measure_photo" {
            let path = args
                .get("path")
                .and_then(|p| p.as_str())
                .ok_or("measure_photo needs path: the photograph")?;
            let size = args
                .get("sheet")
                .and_then(|s| s.as_str())
                .map(ok_photo::SheetSize::parse)
                .transpose()?;
            let reference = args
                .get("reference")
                .and_then(|s| s.as_str())
                .map(ok_photo::Reference::parse)
                .transpose()?;
            let bytes = std::fs::read(path).map_err(|e| format!("could not read {path}: {e}"))?;
            let m = ok_photo::measure(&bytes, size, reference.as_ref())?;
            let mut caption = match m.sheet {
                Some(size) => format!(
                    "{} sheet ({}): the four marks found ({:.2} mm per photo pixel, fit {:.1} px). Rectified at {} px/mm, the origin mark at pixel ({:.0}, {:.0}), x right, y up; the grid drawn is 10 mm, heavier every 50.\n",
                    size.name(),
                    if m.sheet_read {
                        "read from the dots by the origin mark"
                    } else {
                        "as given; no size code seen"
                    },
                    m.mm_per_pixel,
                    m.residual,
                    m.picture_scale,
                    m.picture_origin[0],
                    m.picture_origin[1]
                ),
                None => format!(
                    "No sheet marks: a flat scan, measured from the rule alone at {:.4} mm per pixel. The frame is the picture's own, origin at its bottom-left corner at pixel ({:.0}, {:.0}) of the returned picture, x right, y up; the grid drawn is 10 mm, heavier every 50.\n",
                    m.mm_per_pixel, m.picture_origin[0], m.picture_origin[1]
                ),
            };
            if let Some(r) = &m.rule {
                caption.push_str(&format!(
                    "Rule: {} ticks over {:.0} mm read at {:.3} px/mm; the millimetre edge was {}.\n",
                    r.ticks, r.length, r.px_per_mm, r.edge
                ));
            }
            match &m.calibration {
                Some(c) => caption.push_str(&format!(
                    "Reference: {} at ({:.0}, {:.0}) measured {:.2} mm for {:.2}, so the sheet was printed at {:.1} %; every size below is corrected by that, and the drawn grid is true millimetres.\n",
                    c.reference,
                    (c.bbox[0] + c.bbox[2]) / 2.0,
                    (c.bbox[1] + c.bbox[3]) / 2.0,
                    c.measured,
                    c.nominal,
                    c.factor * 100.0
                )),
                None if m.sheet.is_some() => caption.push_str(
                    "No reference given: sizes trust the print being at 100 % (check the sheet's 100 mm bar), or name one with reference.\n",
                ),
                None => {}
            }
            if m.parts.is_empty() {
                caption.push_str("Nothing dark enough to be a part lies on the grid.\n");
            }
            for (k, p) in m.parts.iter().enumerate() {
                let [x0, y0, x1, y1] = p.bbox;
                caption.push_str(&format!(
                    "Part {}: {:.1} x {:.1} mm, from ({:.1}, {:.1}) to ({:.1}, {:.1}), area {:.0} mm2, centroid ({:.1}, {:.1}), outline of {} points",
                    k + 1,
                    x1 - x0,
                    y1 - y0,
                    x0,
                    y0,
                    x1,
                    y1,
                    p.area,
                    p.centroid[0],
                    p.centroid[1],
                    p.outline.len()
                ));
                if p.circularity > 0.85 {
                    caption.push_str(&format!("; round, {:.1} mm across", p.diameter));
                }
                for h in &p.holes {
                    caption.push_str(&format!(
                        "; hole {:.1} mm across at ({:.1}, {:.1}){}",
                        h.diameter,
                        h.centre[0],
                        h.centre[1],
                        if h.circularity > 0.85 { ", round" } else { "" }
                    ));
                }
                caption.push('\n');
            }
            caption.push_str("These are top-face silhouettes; anything with height is shifted by parallax unless the camera looked straight down. For a size that matters, ask for a caliper reading and name the part and feature as this picture shows them.");
            if let Some(out) = args.get("out").and_then(|p| p.as_str()) {
                std::fs::write(out, &m.picture)
                    .map_err(|e| format!("could not write {out}: {e}"))?;
                caption.push_str(&format!("\nRectified picture written to {out}."));
            }
            if args
                .get("sketch")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                let id = self.doc_id(args)?;
                let doc = self.backend.document(&id)?;
                let tab = pick_tab(&doc, tab)?;
                let studio = |op: Value| json!({ "type": "studio", "tab": tab.0, "op": op });
                let made = self.backend.apply(
                    &id,
                    vec![studio(json!({
                        "type": "add_sketch",
                        "plane": { "type": "standard", "base": "top", "offset": 0 },
                        "name": "Photo outlines"
                    }))],
                )?;
                let sketch = made
                    .pointer("/results/0/studio/feature")
                    .and_then(|v| v.as_u64())
                    .ok_or("the sketch was not created")?;
                let mut ops = Vec::new();
                for p in &m.parts {
                    if p.circularity > 0.85 {
                        ops.push(studio(json!({ "type": "sketch", "id": sketch, "op": {
                            "type": "add_circle",
                            "center": { "x": p.centroid[0], "y": p.centroid[1] },
                            "radius": p.diameter / 2.0
                        }})));
                    }
                    let n = if p.circularity > 0.85 {
                        0
                    } else {
                        p.outline.len()
                    };
                    for (i, a) in p.outline.iter().take(n).enumerate() {
                        let b = p.outline[(i + 1) % n];
                        ops.push(studio(json!({ "type": "sketch", "id": sketch, "op": {
                            "type": "add_line",
                            "a": { "x": a[0], "y": a[1] },
                            "b": { "x": b[0], "y": b[1] }
                        }})));
                    }
                    for h in &p.holes {
                        ops.push(studio(json!({ "type": "sketch", "id": sketch, "op": {
                            "type": "add_circle",
                            "center": { "x": h.centre[0], "y": h.centre[1] },
                            "radius": h.diameter / 2.0
                        }})));
                    }
                }
                let count = ops.len();
                self.backend.apply(&id, ops)?;
                caption.push_str(&format!(
                    "\nSketch feature {sketch} on tab {} of the document holds the outlines: {count} lines and circles in sheet millimetres.",
                    tab.0
                ));
            }
            return Ok(ToolOut::Image {
                png: m.picture,
                caption,
            });
        }
        Ok(ToolOut::Text(match name {
            "offkilter_reference" => Ok(REFERENCE.to_string()),
            "measuring_sheet" => {
                let path = args
                    .get("path")
                    .and_then(|p| p.as_str())
                    .ok_or("measuring_sheet needs path: where to write the PDF")?;
                let size = ok_photo::SheetSize::parse(
                    args.get("sheet")
                        .and_then(|s| s.as_str())
                        .unwrap_or("Letter"),
                )?;
                let pdf = ok_photo::sheet_pdf(size);
                std::fs::write(path, &pdf).map_err(|e| format!("could not write {path}: {e}"))?;
                let (lx, ly) = ok_photo::span(size);
                Ok(format!(
                    "wrote the {} measuring sheet to {path}: print at 100 % (the bar on it is 100 mm), lay parts on the grid, photograph it straight down with all four corner marks in the picture, then measure_photo. The marks are {lx:.0} x {ly:.0} mm apart; the double-ringed one is the origin, and the dots beside it tell measure_photo which sheet this is.",
                    size.name()
                ))
            }
            "list_documents" => {
                let list = self.backend.list()?;
                Ok(serde_json::to_string_pretty(&list).unwrap_or_default())
            }
            "create_document" => {
                let name = args
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("Untitled");
                let (id, meta) = self.backend.create(name)?;
                self.current = Some(id.clone());
                let mut out = json!({ "doc": id, "tabs": [{ "id": 1, "name": "Part Studio 1", "kind": "part_studio" }], "meta": meta });
                if let Some(url) = self.backend.url(&id) {
                    out["url"] = Value::String(url);
                }
                Ok(serde_json::to_string_pretty(&out).unwrap_or_default())
            }
            "open_document" => {
                let id = args
                    .get("doc")
                    .and_then(|d| d.as_str())
                    .ok_or("open_document needs doc")?
                    .to_string();
                let doc = self.backend.document(&id)?;
                self.current = Some(id.clone());
                let mut out = json!({ "doc": id, "name": doc.name, "tabs": tabs_of(&doc) });
                if let Some(url) = self.backend.url(&id) {
                    out["url"] = Value::String(url);
                }
                Ok(serde_json::to_string_pretty(&out).unwrap_or_default())
            }
            "report" => {
                let id = self.doc_id(args)?;
                let report = self.backend.report(&id, tab)?;
                let detail = args
                    .get("detail")
                    .and_then(|d| d.as_str())
                    .unwrap_or("summary");
                if detail == "full" {
                    serde_json::to_string_pretty(&report).map_err(|e| e.to_string())
                } else {
                    Ok(summarize(&report))
                }
            }
            "apply" => {
                let id = self.doc_id(args)?;
                let ops = args
                    .get("ops")
                    .and_then(|o| o.as_array())
                    .cloned()
                    .ok_or("apply needs ops: an array of ops")?;
                let mut doc = self.backend.document(&id)?;
                let default_tab = pick_tab(&doc, tab)?;
                // Face references a model writes from the summary name a
                // piece by number; the report knows each piece's
                // neighbours, which let the reference follow the piece
                // through later edits, so they are attached here.
                let neighbours: Neighbours = doc
                    .describe(default_tab)
                    .map(|r| {
                        r.bodies
                            .iter()
                            .flat_map(|b| b.faces.iter())
                            .map(|f| {
                                let r = f.reference;
                                ((r.feature.0, r.local, r.part.unwrap_or(0)), r.near)
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let wrapped: Vec<Value> = ops
                    .into_iter()
                    .map(|op| {
                        let mut op = wrap_op(op, default_tab, &doc);
                        attach_neighbours(&mut op, &neighbours);
                        op
                    })
                    .collect();
                let outcome = self.backend.apply(&id, wrapped)?;
                let mut text = String::new();
                let results = outcome
                    .get("results")
                    .and_then(|r| r.as_array())
                    .cloned()
                    .unwrap_or_default();
                for (i, r) in results.iter().enumerate() {
                    let mut made = Vec::new();
                    if let Some(f) = r.pointer("/studio/feature").and_then(|v| v.as_u64()) {
                        made.push(format!("feature {f}"));
                    }
                    if let Some(e) = r.pointer("/studio/entities").and_then(|v| v.as_array()) {
                        if !e.is_empty() {
                            made.push(format!(
                                "entities {}",
                                e.iter()
                                    .map(|x| x.to_string())
                                    .collect::<Vec<_>>()
                                    .join(",")
                            ));
                        }
                    }
                    if let Some(c) = r.pointer("/studio/constraint").and_then(|v| v.as_u64()) {
                        made.push(format!("constraint {c}"));
                    }
                    if made.is_empty() {
                        if let Some(t) = r.get("tab").and_then(|v| v.as_u64()) {
                            made.push(format!("tab {t}"));
                        }
                    }
                    if let Some(x) = r.get("instance").and_then(|v| v.as_u64()) {
                        made.push(format!("instance {x}"));
                    }
                    if let Some(m) = r.get("mate").and_then(|v| v.as_u64()) {
                        made.push(format!("mate {m}"));
                    }
                    text.push_str(&format!(
                        "op {i}: ok{}\n",
                        if made.is_empty() {
                            String::new()
                        } else {
                            format!(" ({})", made.join(", "))
                        }
                    ));
                }
                if let Some(e) = outcome.get("error").and_then(|e| e.as_str()) {
                    text.push_str(&format!("ERROR: {e}\n"));
                }
                // What the document looks like now, so mistakes show at once.
                match self.backend.report(&id, Some(default_tab.0)) {
                    Ok(report) => {
                        text.push('\n');
                        text.push_str(&summarize(&report));
                    }
                    Err(e) => text.push_str(&format!("\nreport failed: {e}\n")),
                }
                Ok(text)
            }
            "import" => {
                let id = self.doc_id(args)?;
                let path = args
                    .get("path")
                    .and_then(|p| p.as_str())
                    .ok_or("import needs path: an STL, OBJ or STEP file")?;
                let bytes =
                    std::fs::read(path).map_err(|e| format!("could not read {path}: {e}"))?;
                let stem = std::path::Path::new(path)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Import".into());
                let ext = std::path::Path::new(path)
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                let name = args
                    .get("name")
                    .and_then(|n| n.as_str())
                    .map(str::to_string);
                // (name, vertices, triangles) per body.
                let bodies: Vec<(String, Vec<ok_math::Vec3>, Vec<[u32; 3]>)> = match ext.as_str() {
                    "stl" => {
                        let m = ok_mesh::from_stl(&bytes)?;
                        vec![(name.clone().unwrap_or(stem), m.vertices, m.triangles)]
                    }
                    "obj" => {
                        let m = ok_mesh::from_obj(&String::from_utf8_lossy(&bytes))?;
                        vec![(name.clone().unwrap_or(stem), m.vertices, m.triangles)]
                    }
                    "step" | "stp" => ok_step::read_step(&String::from_utf8_lossy(&bytes))?
                        .into_iter()
                        .map(|b| (b.name, b.vertices, b.triangles))
                        .collect(),
                    other => {
                        return Err(format!(
                            "unknown file type .{other}; use .stl, .obj, .step or .stp"
                        ))
                    }
                };
                let doc = self.backend.document(&id)?;
                let tab_id = pick_tab(&doc, tab)?;
                let ops: Vec<Value> = bodies
                    .iter()
                    .map(|(n, v, t)| {
                        json!({ "type": "studio", "tab": tab_id.0, "op": {
                            "type": "add_mesh",
                            "vertices": v.iter().map(|p| json!({ "x": p.x, "y": p.y, "z": p.z })).collect::<Vec<_>>(),
                            "triangles": t,
                            "name": n,
                        } })
                    })
                    .collect();
                let outcome = self.backend.apply(&id, ops)?;
                let mut text = format!(
                    "imported {} bod{} from {path}: {}",
                    bodies.len(),
                    if bodies.len() == 1 { "y" } else { "ies" },
                    bodies
                        .iter()
                        .map(|(n, v, t)| format!(
                            "{n} ({} vertices, {} triangles)",
                            v.len(),
                            t.len()
                        ))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                if let Some(e) = outcome.get("error").and_then(|e| e.as_str()) {
                    text.push_str(&format!("\nERROR: {e}"));
                }
                match self.backend.report(&id, Some(tab_id.0)) {
                    Ok(r) => {
                        text.push('\n');
                        text.push_str(&summarize(&r));
                    }
                    Err(e) => text.push_str(&format!("\nreport failed: {e}\n")),
                }
                Ok(text)
            }
            "export" => {
                let id = self.doc_id(args)?;
                let format = args
                    .get("format")
                    .and_then(|f| f.as_str())
                    .unwrap_or("stl")
                    .to_lowercase();
                let path = args
                    .get("path")
                    .and_then(|p| p.as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("offkilter-export.{format}"));
                let view = ok_render::View::parse(
                    args.get("view").and_then(|v| v.as_str()).unwrap_or("top"),
                )?;
                let hidden = args
                    .get("hidden")
                    .and_then(|h| h.as_bool())
                    .unwrap_or(false);
                let body = args.get("body").and_then(|b| match b {
                    Value::String(s) => Some(s.clone()),
                    Value::Number(n) => Some(n.to_string()),
                    _ => None,
                });
                let mut sheet = ok_sheet::Options {
                    sheet: ok_sheet::SheetSize::parse(
                        args.get("sheet").and_then(|s| s.as_str()).unwrap_or(""),
                    )?,
                    parts: args.get("parts").and_then(|p| p.as_bool()).unwrap_or(true),
                    note: args
                        .get("note")
                        .and_then(|n| n.as_str())
                        .unwrap_or("")
                        .to_string(),
                    hidden: args.get("hidden").and_then(|h| h.as_bool()),
                    ..ok_sheet::Options::default()
                };
                match args.get("views") {
                    Some(Value::Array(list)) => {
                        sheet.views = list
                            .iter()
                            .filter_map(|v| v.as_str())
                            .map(|s| s.trim().to_lowercase())
                            .collect();
                    }
                    Some(Value::String(s)) => {
                        sheet.views = s
                            .split(',')
                            .map(|v| v.trim().to_lowercase())
                            .filter(|v| !v.is_empty())
                            .collect();
                    }
                    _ => {}
                }
                if sheet.views.is_empty() {
                    return Err("views must name at least one of front, top, right, iso".into());
                }
                let bytes = self.backend.export(
                    &id,
                    tab,
                    &format,
                    view,
                    hidden,
                    body.as_deref(),
                    &sheet,
                )?;
                std::fs::write(&path, &bytes)
                    .map_err(|e| format!("could not write {path}: {e}"))?;
                Ok(format!(
                    "wrote {} bytes of {} to {path}{}",
                    bytes.len(),
                    format.to_uppercase(),
                    if format == "dxf" {
                        format!(" (view {}, 1:1 mm)", view_name(view))
                    } else if format == "pdf" {
                        format!(
                            " ({} sheet, views {})",
                            sheet.sheet.name(),
                            sheet.views.join(", ")
                        )
                    } else {
                        String::new()
                    }
                ))
            }
            "document_url" => {
                let id = self.doc_id(args)?;
                self.backend
                    .url(&id)
                    .ok_or_else(|| "no server: the document is a local file".into())
            }
            other => Err(format!("unknown tool {other}")),
        }?))
    }
}

/// Puts the envelope on an op that lacks one: studio ops and sketch ops
/// go to the tab given (or the first part studio), assembly ops to the
/// tab given (or the first assembly).
fn wrap_op(op: Value, default_tab: ok_model::TabId, doc: &ok_model::Document) -> Value {
    let kind = op.get("type").and_then(|t| t.as_str()).unwrap_or("");
    if DOC_OPS.contains(&kind) {
        return op;
    }
    if ASSEMBLY_OPS.contains(&kind) {
        let tab = if doc.tab(default_tab).map(|t| t.kind_name()) == Some("assembly") {
            default_tab
        } else {
            doc.tabs
                .iter()
                .find(|t| t.kind_name() == "assembly")
                .map(|t| t.id)
                .unwrap_or(default_tab)
        };
        return json!({ "type": "assembly", "tab": tab.0, "op": op });
    }
    let tab = if doc.tab(default_tab).map(|t| t.kind_name()) == Some("part_studio") {
        default_tab
    } else {
        doc.tabs
            .iter()
            .find(|t| t.kind_name() == "part_studio")
            .map(|t| t.id)
            .unwrap_or(default_tab)
    };
    json!({ "type": "studio", "tab": tab.0, "op": op })
}

fn v3(v: &ok_math::Vec3) -> String {
    format!("({:.3}, {:.3}, {:.3})", v.x, v.y, v.z)
}

/// A compact, readable account of a report: features and errors, sketch
/// state, bodies with their faces and cylinders.
fn summarize(r: &ok_model::TabReport) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "Tab {} \"{}\" ({}): {}\n",
        r.tab.0,
        r.name,
        r.kind,
        if r.ok { "ok" } else { "ERRORS" }
    ));
    for e in &r.errors {
        s.push_str(&format!("  error: {e}\n"));
    }
    if !r.features.is_empty() {
        s.push_str("Features:\n");
        for f in &r.features {
            s.push_str(&format!(
                "  {} {} [{}]{}{}{}\n",
                f.id.0,
                f.name,
                f.kind,
                if f.suppressed { " suppressed" } else { "" },
                f.value.map(|v| format!(" = {v}")).unwrap_or_default(),
                f.error
                    .as_ref()
                    .map(|e| format!(" ERROR: {e}"))
                    .unwrap_or_default()
            ));
        }
    }
    for sk in &r.sketches {
        let entities = sk.entities.as_array().map(|a| a.len()).unwrap_or(0);
        let constraints = sk.constraints.as_array().map(|a| a.len()).unwrap_or(0);
        s.push_str(&format!(
            "Sketch {} \"{}\": {} entities, {} constraints, {:?}, dof {}, {} closed region{}\n",
            sk.feature.0,
            sk.name,
            entities,
            constraints,
            sk.solve.status,
            sk.solve.dof,
            sk.regions,
            if sk.regions == 1 { "" } else { "s" }
        ));
        if let Some(list) = sk.entities.as_array() {
            for e in list.iter().take(60) {
                let id = e.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                let kind = e.get("type").and_then(|v| v.as_str()).unwrap_or("?");
                let detail = match kind {
                    "point" => e
                        .get("pos")
                        .map(|p| format!(" at ({}, {})", p["x"], p["y"]))
                        .unwrap_or_default(),
                    "line" => format!(" points {} -> {}", e["start"], e["end"]),
                    "circle" => format!(" centre point {} r {}", e["center"], e["radius"]),
                    "arc" => format!(
                        " centre {} from {} to {}",
                        e["center"], e["start"], e["end"]
                    ),
                    _ => String::new(),
                };
                s.push_str(&format!("    entity {id} {kind}{detail}\n"));
            }
            if list.len() > 60 {
                s.push_str(&format!("    … {} more (detail: full)\n", list.len() - 60));
            }
        }
    }
    if !r.variables.is_empty() {
        s.push_str(&format!(
            "Variables: {}\n",
            r.variables
                .iter()
                .map(|(k, v)| format!("#{k} = {v}"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    for b in &r.bodies {
        s.push_str(&format!(
            "Body {} \"{}\" (from feature {}): volume {:.3} mm³, area {:.3} mm², bounds {}{}\n",
            b.index,
            b.name,
            b.source.0,
            b.volume,
            b.area,
            b.bounds
                .map(|(lo, hi)| format!("{} to {}", v3(&lo), v3(&hi)))
                .unwrap_or_else(|| "none".into()),
            b.material
                .as_ref()
                .map(|m| format!(", material {}", m.name))
                .unwrap_or_default()
        ));
        for f in &b.faces {
            s.push_str(&format!(
                "  face ref {{feature: {}, local: {}, part: {}}} {} normal {} centre {} area {:.3}{}\n",
                f.reference.feature.0,
                f.reference.local,
                f.reference.part.unwrap_or(0),
                f.surface,
                v3(&f.normal),
                v3(&f.centroid),
                f.area,
                f.facets.map(|n| format!(" ({n} facets)")).unwrap_or_default()
            ));
        }
        for c in &b.cylinders {
            s.push_str(&format!(
                "  cylinder {} r {:.3} axis {} through {} ref {{feature: {}, local: {}, part: {}}}\n",
                if c.hole { "hole" } else { "boss" },
                c.radius,
                v3(&c.axis),
                v3(&c.origin),
                c.reference.feature.0,
                c.reference.local,
                c.reference.part.unwrap_or(0)
            ));
        }
    }
    for i in &r.instances {
        s.push_str(&format!(
            "Instance {} \"{}\" of tab {} body {}{}{}{}\n",
            i.id.0,
            i.name,
            i.studio.0,
            i.body,
            if i.fixed { " (fixed)" } else { "" },
            i.placed
                .map(|p| format!(" at {} rotated {}", v3(&p.position), v3(&p.rotation)))
                .unwrap_or_default(),
            i.error
                .as_ref()
                .map(|e| format!(" ERROR: {e}"))
                .unwrap_or_default()
        ));
    }
    for m in &r.mates {
        s.push_str(&format!(
            "Mate {} \"{}\" {:?}{}\n",
            m.id.0,
            m.name,
            m.kind,
            m.error
                .as_ref()
                .map(|e| format!(" ERROR: {e}"))
                .unwrap_or_default()
        ));
    }
    s
}

fn tool_list() -> Value {
    let doc_prop = json!({ "type": "string", "description": "Document id (server) or path (local); defaults to the current document." });
    let tab_prop =
        json!({ "type": "integer", "description": "Tab id; defaults to the first tab." });
    json!([
        {
            "name": "offkilter_reference",
            "description": "The op reference: how to write sketch, feature and assembly ops, the envelopes, and how faces and edges are referenced. Read it before building anything.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "list_documents",
            "description": "Lists the documents on the server (or the local file).",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "create_document",
            "description": "Creates a new document with one part studio (tab 1) and makes it current. Returns its id and, on a server, the browser URL.",
            "inputSchema": { "type": "object", "properties": { "name": { "type": "string" } }, "required": ["name"] }
        },
        {
            "name": "open_document",
            "description": "Makes an existing document current and lists its tabs.",
            "inputSchema": { "type": "object", "properties": { "doc": doc_prop }, "required": ["doc"] }
        },
        {
            "name": "report",
            "description": "Regenerates a tab and reports it: features with ids and errors, sketches with solver state and entity ids, bodies with volume, bounds and every face's reference (for sketches on faces, holes, fillets, shells). detail 'full' returns the raw JSON with all sketch entities and constraints.",
            "inputSchema": { "type": "object", "properties": { "doc": doc_prop, "tab": tab_prop, "detail": { "type": "string", "enum": ["summary", "full"] } } }
        },
        {
            "name": "apply",
            "description": "Applies ops in order (see offkilter_reference). Bare studio, sketch and assembly ops are wrapped for the tab. Stops at the first failing op, keeps the earlier ones, and returns each op's result (new feature and entity ids) followed by the tab's report.",
            "inputSchema": { "type": "object", "properties": { "doc": doc_prop, "tab": tab_prop, "ops": { "type": "array", "items": { "type": "object" } } }, "required": ["ops"] }
        },
        {
            "name": "screenshot",
            "description": "A PNG of the tab's bodies from a standard view (top, front, right, iso) or an x,y,z eye direction, rendered without a browser; optionally sectioned by an axis-aligned plane (section 'z:10' keeps z >= 10, 'z:10:flip' the other side, cut faces hatched; the iso eye looks from +x,+y,+z, so a section at x:0 shows the kept half's outside and 'x:0:flip' its cut faces). Look at it after building something to check it is what was meant. width and height default to 640x480; path also writes the file.",
            "inputSchema": { "type": "object", "properties": { "doc": doc_prop, "tab": tab_prop, "view": { "type": "string" }, "section": { "type": "string" }, "width": { "type": "integer" }, "height": { "type": "integer" }, "path": { "type": "string" } } }
        },
        {
            "name": "import",
            "description": "Imports an STL, OBJ or STEP (.step/.stp) file from a path as mesh bodies of the tab: STEP solids come in faceted (planes and cylinders; other surfaces are refused by name), scaled to millimetres. Returns the tab's report.",
            "inputSchema": { "type": "object", "properties": { "doc": doc_prop, "tab": tab_prop, "path": { "type": "string" }, "name": { "type": "string", "description": "Body name (STL and OBJ; STEP bodies keep their own names)." } }, "required": ["path"] }
        },
        {
            "name": "measuring_sheet",
            "description": "Writes the printable measuring sheet as a PDF: a light 10 mm grid with four bullseye marks in the corners (the origin's double-ringed, with dots beside it that encode the sheet size) and a 100 mm bar to check the print. Print it at 100 %, lay parts on it, photograph it straight down with all four marks in view, and measure_photo reads sizes off the picture.",
            "inputSchema": { "type": "object", "properties": { "path": { "type": "string" }, "sheet": { "type": "string", "description": "Letter (default), A4, A3, A2 or Tabloid" } }, "required": ["path"] }
        },
        {
            "name": "measure_photo",
            "description": "Measures a photograph (JPEG or PNG) of parts lying on the printed measuring sheet: finds the four marks, squares the picture up onto the sheet's millimetres, and reports every dark shape on the grid as a part with its bounding box, area, centroid, outline and any holes (with their diameters), all in millimetres from the origin mark, x right, y up. Returns the squared-up picture with the grid and the parts drawn on it, so a feature can be named by where it sits; `out` also writes that picture. Good to a fraction of a millimetre on flat things photographed straight down; heights shift edges by parallax, so ask for a caliper reading for anything that matters and use the picture to say which. `sketch: true` adds a sketch on the current document's tab (doc, tab) with the outlines as lines and the holes as circles, ready to extrude. The sheet size is read from the dots by the origin mark; `sheet` is the fallback for a print without them. A print is rarely exactly 100 %: `reference` names a thing of known size on the sheet, and the print scale is read from it and every size corrected: 'rule' for a steel rule with millimetre graduations (its ticks are read; an inch edge is told apart), 'bars 10' for a forensic photo scale with alternating 10 mm black and white bars, 'disc 24.26' or a US coin by name (quarter, nickel, dime, penny) for a round object. A phone picture in which the sheet fills the frame resolves the ticks. With 'rule' the sheet is optional: a flatbed scan of parts and a rule, no sheet, is measured in the picture's own frame (origin bottom-left).",
            "inputSchema": { "type": "object", "properties": { "path": { "type": "string" }, "sheet": { "type": "string", "description": "Fallback when the picture carries no size code: A4, A3, A2, Letter or Tabloid" }, "reference": { "type": "string", "description": "A known size in the picture: 'rule' (a steel rule's mm graduations; works without the sheet in a flat scan), 'bars <mm>' (a photo scale's alternating blocks), 'disc <mm>', or quarter, nickel, dime, penny" }, "out": { "type": "string" }, "sketch": { "type": "boolean" }, "doc": doc_prop, "tab": tab_prop }, "required": ["path"] }
        },
        {
            "name": "export",
            "description": "Writes a tab's bodies as STL or STEP to a file path; or as DXF: the bodies' visible edges seen from `view` (top by default; the same names as screenshot) at 1:1 in millimetres, a template to print or a profile to cut, `hidden: true` adding hidden lines dashed on their own layer; or as PDF: a shop drawing sheet with the `views` (front, top, right, iso by default) laid out third angle on `sheet` (A4 by default; A3, A2, Letter, Tabloid) at the largest standard scale that fits, overall dimensions, and on a sheet of one part hidden lines dashed and diameter callouts for holes seen end-on; on an assembly a balloon per item and a parts list (`parts: false` to leave them off), where a sub-assembly instance is one item with its own sheet on its own tab; `hidden` forces hidden lines on or off; `note` goes on the title block. `body` (a body's name or index from the report) writes that one body alone to STL or STEP, for a part to print.",
            "inputSchema": { "type": "object", "properties": { "doc": doc_prop, "tab": tab_prop, "format": { "type": "string", "enum": ["stl", "step", "dxf", "pdf"] }, "path": { "type": "string" }, "view": { "type": "string" }, "hidden": { "type": "boolean" }, "body": { "type": "string" }, "views": { "type": "array", "items": { "type": "string" } }, "sheet": { "type": "string" }, "parts": { "type": "boolean" }, "note": { "type": "string" } }, "required": ["format", "path"] }
        },
        {
            "name": "document_url",
            "description": "The browser URL of the document on the server, where a person sees every applied op live.",
            "inputSchema": { "type": "object", "properties": { "doc": doc_prop } }
        }
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local_server(dir: &std::path::Path) -> Server {
        let path = dir.join("part.okpart");
        let (backend, current) =
            Backend::from_args(&["--file".into(), path.display().to_string()]).unwrap();
        Server::new((backend, current))
    }

    fn call(server: &mut Server, id: u64, method: &str, params: Value) -> Value {
        let line =
            json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string();
        serde_json::from_str(&server.handle_line(&line).unwrap()).unwrap()
    }

    fn tool_text(server: &mut Server, name: &str, args: Value) -> (bool, String) {
        let r = call(
            server,
            1,
            "tools/call",
            json!({ "name": name, "arguments": args }),
        );
        let err = r["result"]["isError"].as_bool().unwrap_or(false);
        (
            err,
            r["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .to_string(),
        )
    }

    #[test]
    fn a_model_builds_a_part_through_the_tools() {
        let dir = std::env::temp_dir().join(format!("ok-mcp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut server = local_server(&dir);
        let init = call(
            &mut server,
            1,
            "initialize",
            json!({ "protocolVersion": PROTOCOL, "capabilities": {}, "clientInfo": { "name": "test", "version": "0" } }),
        );
        assert_eq!(init["result"]["protocolVersion"], PROTOCOL);
        assert!(server
            .handle_line(
                &json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }).to_string()
            )
            .is_none());
        let tools = call(&mut server, 2, "tools/list", json!({}));
        let names: Vec<&str> = tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"apply") && names.contains(&"report") && names.contains(&"export"));
        let (err, reference) = tool_text(&mut server, "offkilter_reference", json!({}));
        assert!(!err && reference.contains("add_extrude"));
        let (err, created) =
            tool_text(&mut server, "create_document", json!({ "name": "Bracket" }));
        assert!(!err, "{created}");
        assert!(created.contains("Bracket.okpart"));
        // Bare ops are wrapped for the first tab; results name what they made.
        let (err, applied) = tool_text(
            &mut server,
            "apply",
            json!({ "ops": [
            { "type": "add_sketch", "plane": { "type": "standard", "base": "top", "offset": 0 }, "name": "Base" },
            { "type": "sketch", "id": 1, "op": { "type": "add_rectangle", "a": { "x": 0, "y": 0 }, "b": { "x": 40, "y": 20 } } },
            { "type": "add_extrude", "sketch": 1, "depth": 5, "name": "Plate" }
        ] }),
        );
        assert!(!err, "{applied}");
        assert!(applied.contains("op 0: ok (feature 1)"), "{applied}");
        assert!(applied.contains("op 2: ok (feature 2)"), "{applied}");
        assert!(applied.contains("volume 4000.000"), "{applied}");
        assert!(
            applied.contains("face ref {feature: 2, local: 1, part: 0}"),
            "{applied}"
        );
        // A hole on the top face, through all: the report shows the cylinder.
        let (err, applied) = tool_text(
            &mut server,
            "apply",
            json!({ "ops": [
            { "type": "add_sketch", "plane": { "type": "face", "face": { "feature": 2, "local": 1, "part": 0 }, "offset": 0 }, "name": "Centres" },
            { "type": "sketch", "id": 3, "op": { "type": "add_point", "pos": { "x": 20, "y": 10 } } },
            { "type": "add_hole", "sketch": 3, "diameter": 6, "through_all": true, "name": "Hole" }
        ] }),
        );
        assert!(!err, "{applied}");
        assert!(applied.contains("cylinder hole r 3.000"), "{applied}");
        // A bad op stops the batch and says which one; the earlier ones stay.
        let (err, applied) = tool_text(
            &mut server,
            "apply",
            json!({ "ops": [
            { "type": "add_variable", "name": "w", "expression": "40" },
            { "type": "delete_feature", "id": 99 }
        ] }),
        );
        assert!(!err, "{applied}");
        assert!(applied.contains("ERROR: op 1 failed"), "{applied}");
        assert!(applied.contains("#w = 40"), "{applied}");
        let (err, report) = tool_text(&mut server, "report", json!({ "detail": "full" }));
        assert!(!err);
        let full: Value = serde_json::from_str(&report).unwrap();
        assert_eq!(full["features"].as_array().unwrap().len(), 5);
        let stl = dir.join("out.stl");
        let (err, exported) = tool_text(
            &mut server,
            "export",
            json!({ "format": "stl", "path": stl.display().to_string() }),
        );
        assert!(!err && exported.starts_with("wrote "), "{exported}");
        assert!(std::fs::metadata(&stl).unwrap().len() > 84);
        let shot = dir.join("shot.png");
        let r = call(
            &mut server,
            7,
            "tools/call",
            json!({ "name": "screenshot", "arguments": { "view": "iso", "section": "z:2", "width": 96, "height": 64, "path": shot.display().to_string() } }),
        );
        let content = r["result"]["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "image");
        assert_eq!(content[0]["mimeType"], "image/png");
        assert!(
            content[1]["text"].as_str().unwrap().contains("iso view"),
            "{}",
            r
        );
        let png = std::fs::read(&shot).unwrap();
        let image = ok_render::from_png(&png).unwrap();
        assert_eq!((image.width, image.height), (96, 64));
        assert_eq!(base64(&png[..4]), "iVBORw==");
        assert_eq!(base64(&png), content[0]["data"].as_str().unwrap());
        let (err, text) = tool_text(&mut server, "screenshot", json!({ "view": "sideways" }));
        assert!(err && text.contains("unknown view"), "{text}");
        let step = dir.join("out.step");
        let (err, _) = tool_text(
            &mut server,
            "export",
            json!({ "format": "step", "path": step.display().to_string() }),
        );
        assert!(!err);
        assert!(std::fs::read_to_string(&step)
            .unwrap()
            .contains("MANIFOLD_SOLID_BREP"));
        // The plate from above as a DXF: four edges and the hole's circle.
        let dxf = dir.join("plan.dxf");
        let (err, text) = tool_text(
            &mut server,
            "export",
            json!({ "format": "dxf", "view": "top", "path": dxf.display().to_string() }),
        );
        assert!(!err && text.contains("view top"), "{text}");
        let plan = std::fs::read_to_string(&dxf).unwrap();
        assert_eq!(plan.matches("\nLINE\n").count(), 4, "{plan}");
        assert_eq!(plan.matches("\nCIRCLE\n").count(), 1, "{plan}");
        let (err, text) = tool_text(
            &mut server,
            "export",
            json!({ "format": "dxf", "view": "behind", "path": dxf.display().to_string() }),
        );
        assert!(err && text.contains("unknown view"), "{text}");
        // A shop drawing sheet: the plate's sizes and its hole called out.
        let pdf = dir.join("sheet.pdf");
        let (err, text) = tool_text(
            &mut server,
            "export",
            json!({ "format": "pdf", "views": ["front", "top"], "sheet": "Letter", "note": "checked", "path": pdf.display().to_string() }),
        );
        assert!(
            !err && text.contains("Letter sheet, views front, top"),
            "{text}"
        );
        let sheet = std::fs::read(&pdf).unwrap();
        assert!(sheet.starts_with(b"%PDF-1.4"));
        let sheet = String::from_utf8_lossy(&sheet);
        assert!(
            sheet.contains("\\330") && sheet.contains("(checked \\267 offkilter) Tj"),
            "{sheet}"
        );
        let (err, text) = tool_text(
            &mut server,
            "export",
            json!({ "format": "pdf", "sheet": "b5", "path": pdf.display().to_string() }),
        );
        assert!(err && text.contains("unknown sheet"), "{text}");
        // The sketch on the plate's top face was written with a bare
        // reference; the document stores it with the piece's neighbours.
        let stored = ok_model::Document::from_json(
            &std::fs::read_to_string(dir.join("Bracket.okpart")).unwrap(),
        )
        .unwrap();
        let on_face = stored
            .studio(ok_model::TabId(1))
            .unwrap()
            .features()
            .iter()
            .find_map(|f| match &f.kind {
                ok_model::FeatureKind::Sketch(sf) => match &sf.plane {
                    ok_model::PlaneRef::Face { face, .. } => Some(*face),
                    _ => None,
                },
                _ => None,
            })
            .expect("a sketch on a face");
        assert!(on_face.has_near(), "neighbours attached: {on_face:?}");
        // The exported STEP imports back as a mesh body of its own.
        let (err, imported) = tool_text(
            &mut server,
            "import",
            json!({ "path": step.display().to_string() }),
        );
        assert!(
            !err && imported.starts_with("imported 1 body"),
            "{imported}"
        );
        assert!(imported.contains("Part 1 ("), "{imported}");
        let (err, text) = tool_text(&mut server, "import", json!({ "path": "/nonexistent.stl" }));
        assert!(err && text.contains("could not read"), "{text}");
        // The file on disk carries everything.
        let text = std::fs::read_to_string(dir.join("Bracket.okpart")).unwrap();
        assert_eq!(
            ok_model::Document::from_json(&text)
                .unwrap()
                .studio(ok_model::TabId(1))
                .unwrap()
                .features()
                .len(),
            6,
            "five features and the imported mesh"
        );
        let unknown = call(&mut server, 9, "no/such", json!({}));
        assert_eq!(unknown["error"]["code"], -32601);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_measuring_sheet_is_printed_and_a_photograph_of_it_read() {
        let dir = std::env::temp_dir().join(format!("ok-mcp-photo-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut server = local_server(&dir);
        let (err, created) = tool_text(&mut server, "create_document", json!({ "name": "Scan" }));
        assert!(!err, "{created}");
        let pdf = dir.join("sheet.pdf");
        let (err, text) = tool_text(
            &mut server,
            "measuring_sheet",
            json!({ "path": pdf.display().to_string(), "sheet": "Letter" }),
        );
        assert!(!err && text.contains("Letter measuring sheet"), "{text}");
        assert!(text.contains("229 x 166 mm apart"), "{text}");
        let sheet = String::from_utf8_lossy(&std::fs::read(&pdf).unwrap()).to_string();
        assert!(sheet.starts_with("%PDF-1.4") && sheet.contains("the bar below is 100 mm"));
        let (err, text) = tool_text(
            &mut server,
            "measuring_sheet",
            json!({ "path": "/nonexistent/x.pdf", "sheet": "b5" }),
        );
        assert!(err && text.contains("unknown sheet"), "{text}");
        // A scan of the sheet with a 50 x 30 plate (8 mm hole) and a 20 mm disc on it.
        let scan = ok_photo::sheet_image(
            ok_photo::SheetSize::Letter,
            5.0,
            &ok_photo::Scene {
                rects: vec![[40.0, 40.0, 90.0, 70.0]],
                discs: vec![[150.0, 100.0, 10.0]],
                holes: vec![[65.0, 55.0, 4.0]],
                print: 0.98,
                ..ok_photo::Scene::default()
            }
            .bars(120.0, 130.0, 10.0, 4),
        );
        let photo = dir.join("scan.png");
        std::fs::write(&photo, ok_render::to_png(&scan)).unwrap();
        let out = dir.join("measured.png");
        let r = call(
            &mut server,
            3,
            "tools/call",
            json!({ "name": "measure_photo", "arguments": {
                "path": photo.display().to_string(), "reference": "bars 10",
                "out": out.display().to_string(), "sketch": true } }),
        );
        assert_eq!(r["result"]["isError"], Value::Null, "{r}");
        let content = r["result"]["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "image");
        let caption = content[1]["text"].as_str().unwrap();
        assert!(
            caption.contains("Letter sheet (read from the dots by the origin mark)"),
            "{caption}"
        );
        assert!(
            caption.contains("Reference: a scale with 10 mm bars at (155, 132)")
                && caption.contains("printed at 98."),
            "{caption}"
        );
        assert!(
            caption.contains("Part 1: 50.") && caption.contains("from (40.0, "),
            "{caption}"
        );
        assert!(
            caption.contains("hole 8.0 mm across at (65.") && caption.contains(", round"),
            "{caption}"
        );
        assert!(
            caption.contains("Part 2: 20.0 x ") && caption.contains("centroid (150."),
            "{caption}"
        );
        assert!(caption.contains("; round, 20.0 mm across"), "{caption}");
        assert!(caption.contains("Sketch feature 1 on tab 1"), "{caption}");
        assert!(
            caption.contains("Rectified picture written to"),
            "{caption}"
        );
        let picture = ok_render::from_png(&std::fs::read(&out).unwrap()).unwrap();
        // The picture spans the marks in true millimetres: 98 % of nominal.
        let (lx, ly) = ok_photo::span(ok_photo::SheetSize::Letter);
        let near = |got: usize, want: f64| (got as f64 - want).abs() <= 3.0;
        assert!(
            near(picture.width, (lx * 0.98 + 30.0) * 4.0)
                && near(picture.height, (ly * 0.98 + 30.0) * 4.0),
            "{} x {}",
            picture.width,
            picture.height
        );
        // The outlines became a sketch: the plate's four lines, its hole, and the disc.
        let (err, report) = tool_text(&mut server, "report", json!({ "detail": "full" }));
        assert!(!err, "{report}");
        let full: Value = serde_json::from_str(&report).unwrap();
        let sketch = &full["sketches"][0];
        let kinds: Vec<&str> = sketch["entities"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|e| e["type"].as_str())
            .collect();
        assert_eq!(
            kinds.iter().filter(|k| **k == "line").count(),
            4,
            "{sketch}"
        );
        assert_eq!(
            kinds.iter().filter(|k| **k == "circle").count(),
            2,
            "{sketch}"
        );
        assert_eq!(sketch["regions"], 3, "{sketch}");
        let blank = dir.join("blank.png");
        std::fs::write(
            &blank,
            ok_render::to_png(&ok_render::Image::new(
                300,
                200,
                ok_render::Rgb(255, 255, 255),
            )),
        )
        .unwrap();
        let (err, text) = tool_text(
            &mut server,
            "measure_photo",
            json!({ "path": blank.display().to_string() }),
        );
        assert!(err && text.contains("origin mark"), "{text}");
        let (err, text) = tool_text(
            &mut server,
            "measure_photo",
            json!({ "path": photo.display().to_string(), "reference": "tape" }),
        );
        assert!(err && text.contains("reference"), "{text}");
        // A flatbed scan with no sheet: a rule alone gives the scale.
        let scan = ok_photo::scan_image(
            120.0,
            80.0,
            300.0 / 25.4,
            &ok_photo::Scene {
                rects: vec![[10.0, 10.0, 40.0, 30.0]],
                rule: Some((10.0, 45.0, 100.0, 0.0)),
                ..ok_photo::Scene::default()
            },
        );
        let flat = dir.join("flat.png");
        std::fs::write(&flat, ok_render::to_png(&scan)).unwrap();
        let r = call(
            &mut server,
            4,
            "tools/call",
            json!({ "name": "measure_photo", "arguments": {
                "path": flat.display().to_string(), "reference": "rule" } }),
        );
        let caption = r["result"]["content"][1]["text"].as_str().unwrap();
        assert!(caption.contains("No sheet marks: a flat scan"), "{caption}");
        assert!(
            caption.contains("Rule: 101 ticks over 100 mm read at 11.81"),
            "{caption}"
        );
        assert!(
            caption.contains("Part 1: 30.0 x 20.") && caption.contains("from (10.0, "),
            "{caption}"
        );
        assert!(
            !caption.contains("Part 2"),
            "the rule is not a part: {caption}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
