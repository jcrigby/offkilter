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
    pub fn join(&self, name: Option<String>) -> (u64, ServerMessage) {
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
