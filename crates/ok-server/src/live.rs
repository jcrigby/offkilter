//! Live documents: the server's copy of an open document, its op sequence,
//! and a broadcast channel to connected clients.

use crate::store::DocStore;
use ok_model::Document;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

/// Messages from a client.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Hello {
        name: Option<String>,
    },
    Op {
        op: serde_json::Value,
        id: u64,
        #[serde(default)]
        base: Option<u32>,
    },
    Snapshot,
}

/// Messages to clients.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// `prefix` is this client's 12-bit id-range prefix: allocate ids as
    /// `(prefix << 20) | counter`.
    Welcome {
        client: u64,
        prefix: u32,
        seq: u64,
        doc: String,
        clients: usize,
        /// The document is shared with this client read-only.
        #[serde(default)]
        read_only: bool,
    },
    /// `hash` is the document's structural hash after this op, so replicas
    /// can detect divergence.
    Op {
        op: serde_json::Value,
        seq: u64,
        from: u64,
        id: u64,
        base: Option<u32>,
        hash: String,
    },
    Error {
        message: String,
        id: u64,
    },
    Snapshot {
        doc: String,
        seq: u64,
    },
    Presence {
        clients: usize,
        names: Vec<String>,
    },
}

/// What `LiveDoc::apply_many` did: one result per applied op, and the
/// failure that stopped it, if any.
#[derive(Debug, Serialize)]
pub struct ApplyOutcome {
    pub results: Vec<ok_model::DocOpResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub struct LiveDoc {
    pub id: String,
    state: Mutex<LiveState>,
    pub tx: broadcast::Sender<ServerMessage>,
    store: DocStore,
}

struct LiveState {
    doc: Document,
    seq: u64,
    next_client: u64,
    clients: HashMap<u64, String>,
}

impl LiveDoc {
    fn load(id: &str, store: DocStore) -> Option<Arc<LiveDoc>> {
        let json = store.read(id)?;
        let doc = Document::from_json(&json).ok()?;
        let (tx, _) = broadcast::channel(256);
        Some(Arc::new(LiveDoc {
            id: id.to_string(),
            state: Mutex::new(LiveState {
                doc,
                seq: 0,
                next_client: 1,
                clients: HashMap::new(),
            }),
            tx,
            store,
        }))
    }

    /// Registers a client and returns its id plus a welcome message.
    pub fn join(&self, name: Option<String>, read_only: bool) -> (u64, ServerMessage) {
        let prefix = self.store.take_prefix(&self.id).unwrap_or(1);
        let mut st = self.state.lock().unwrap();
        let client = st.next_client;
        st.next_client += 1;
        st.clients
            .insert(client, name.unwrap_or_else(|| format!("client {client}")));
        let msg = ServerMessage::Welcome {
            client,
            prefix,
            seq: st.seq,
            doc: st.doc.to_json(),
            clients: st.clients.len(),
            read_only,
        };
        let presence = self.presence_locked(&st);
        drop(st);
        let _ = self.tx.send(presence);
        (client, msg)
    }

    pub fn leave(&self, client: u64) {
        let mut st = self.state.lock().unwrap();
        st.clients.remove(&client);
        let presence = self.presence_locked(&st);
        drop(st);
        let _ = self.tx.send(presence);
    }

    fn presence_locked(&self, st: &LiveState) -> ServerMessage {
        let mut names: Vec<String> = st.clients.values().cloned().collect();
        names.sort();
        ServerMessage::Presence {
            clients: st.clients.len(),
            names,
        }
    }

    /// Applies an op from a client; on success broadcasts it, on failure
    /// returns the error for that client only.
    pub fn apply(
        &self,
        from: u64,
        id: u64,
        op: serde_json::Value,
        base: Option<u32>,
    ) -> Result<u64, String> {
        let mut st = self.state.lock().unwrap();
        let text = op.to_string();
        st.doc
            .apply_json_with_base(&text, base)
            .map_err(|e| e.to_string())?;
        st.seq += 1;
        let seq = st.seq;
        let json = st.doc.to_json();
        let name = st.doc.name.clone();
        let hash = format!("{:016x}", st.doc.structural_hash());
        drop(st);
        let _ = self.store.write(&self.id, &json, Some(&name));
        let _ = self.tx.send(ServerMessage::Op {
            op,
            seq,
            from,
            id,
            base,
            hash,
        });
        Ok(seq)
    }

    /// Applies ops in order through the live document, as a script or an
    /// agent would, stopping at the first that fails (the ones before it
    /// stay applied). Every applied op reaches the connected clients like
    /// any other edit. Returns each op's result and the error, if any.
    pub fn apply_many(&self, ops: Vec<serde_json::Value>) -> ApplyOutcome {
        let mut st = self.state.lock().unwrap();
        let mut results = Vec::with_capacity(ops.len());
        let mut messages = Vec::with_capacity(ops.len());
        let mut error = None;
        for (i, op) in ops.into_iter().enumerate() {
            match st.doc.apply_json_with_base(&op.to_string(), None) {
                Ok(r) => {
                    st.seq += 1;
                    let hash = format!("{:016x}", st.doc.structural_hash());
                    messages.push(ServerMessage::Op {
                        op,
                        seq: st.seq,
                        from: 0,
                        id: 0,
                        base: None,
                        hash,
                    });
                    results.push(r);
                }
                Err(e) => {
                    error = Some(format!("op {i} failed: {e}"));
                    break;
                }
            }
        }
        let json = st.doc.to_json();
        let name = st.doc.name.clone();
        drop(st);
        if !messages.is_empty() {
            let _ = self.store.write(&self.id, &json, Some(&name));
        }
        for m in messages {
            let _ = self.tx.send(m);
        }
        ApplyOutcome { results, error }
    }

    /// Replaces the live document (e.g. restoring a version) and tells
    /// every client to reload it.
    pub fn replace(&self, json: &str) -> Result<(), String> {
        let doc = Document::from_json(json).map_err(|e| e.to_string())?;
        let mut st = self.state.lock().unwrap();
        st.doc = doc;
        st.seq += 1;
        let seq = st.seq;
        let name = st.doc.name.clone();
        drop(st);
        let _ = self.store.write(&self.id, json, Some(&name));
        let hash = format!(
            "{:016x}",
            Document::from_json(json)
                .map(|s| s.structural_hash())
                .unwrap_or(0)
        );
        let op = serde_json::json!({ "type": "replace_document", "json": json });
        let _ = self.tx.send(ServerMessage::Op {
            op,
            seq,
            from: 0,
            id: 0,
            base: None,
            hash,
        });
        Ok(())
    }

    /// Merges `theirs`' changes since `base` into the live document (see
    /// `Document::merge_from`), or only plans them when `apply` is false.
    /// Applied ops go to every client like any other edit; ops the document
    /// refuses are reported as conflicts.
    pub fn merge_from(&self, base: &Document, theirs: &Document, apply: bool) -> ok_model::Merge {
        let mut st = self.state.lock().unwrap();
        let mut merge = st.doc.merge_from(base, theirs);
        if !apply {
            return merge;
        }
        let mut sent = Vec::new();
        for op in std::mem::take(&mut merge.ops) {
            let value = serde_json::to_value(&op).unwrap_or_default();
            match st.doc.apply_with_base(op, None) {
                Ok(_) => {
                    st.seq += 1;
                    let hash = format!("{:016x}", st.doc.structural_hash());
                    sent.push(ServerMessage::Op {
                        op: value,
                        seq: st.seq,
                        from: 0,
                        id: 0,
                        base: None,
                        hash,
                    });
                }
                Err(e) => merge.conflicts.push(format!("refused: {e}")),
            }
        }
        let json = st.doc.to_json();
        let name = st.doc.name.clone();
        drop(st);
        if !sent.is_empty() {
            let _ = self.store.write(&self.id, &json, Some(&name));
        }
        for msg in sent {
            let _ = self.tx.send(msg);
        }
        merge
    }

    pub fn snapshot(&self) -> ServerMessage {
        let st = self.state.lock().unwrap();
        ServerMessage::Snapshot {
            doc: st.doc.to_json(),
            seq: st.seq,
        }
    }

    pub fn client_count(&self) -> usize {
        self.state.lock().unwrap().clients.len()
    }
}

/// All open documents.
#[derive(Clone)]
pub struct DocHub {
    store: DocStore,
    open: Arc<Mutex<HashMap<String, Arc<LiveDoc>>>>,
}

impl DocHub {
    pub fn new(store: DocStore) -> DocHub {
        DocHub {
            store,
            open: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn store(&self) -> &DocStore {
        &self.store
    }

    pub fn get(&self, id: &str) -> Option<Arc<LiveDoc>> {
        let mut open = self.open.lock().unwrap();
        if let Some(d) = open.get(id) {
            return Some(d.clone());
        }
        let d = LiveDoc::load(id, self.store.clone())?;
        open.insert(id.to_string(), d.clone());
        Some(d)
    }

    /// Drops the live copy when nobody is connected (the disk copy is current).
    pub fn release(&self, id: &str) {
        let mut open = self.open.lock().unwrap();
        if open.get(id).is_some_and(|d| d.client_count() == 0) {
            open.remove(id);
        }
    }

    /// Forgets the live copy after an external write (PUT / DELETE).
    pub fn evict(&self, id: &str) {
        self.open.lock().unwrap().remove(id);
    }
}
