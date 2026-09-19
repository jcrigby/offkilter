//! On-disk document storage: one `.okpart` JSON file per document plus a
//! small metadata file.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocMeta {
    pub id: String,
    pub name: String,
    pub created: u64,
    pub updated: u64,
}

#[derive(Clone)]
pub struct DocStore {
    root: PathBuf,
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn new_id() -> String {
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}{:02x}", t as u64 ^ (c << 48), c & 0xff)
}

impl DocStore {
    pub fn open(root: &Path) -> std::io::Result<DocStore> {
        std::fs::create_dir_all(root.join("docs"))?;
        Ok(DocStore {
            root: root.to_path_buf(),
        })
    }

    fn doc_path(&self, id: &str) -> PathBuf {
        self.root.join("docs").join(format!("{id}.okpart"))
    }

    fn meta_path(&self, id: &str) -> PathBuf {
        self.root.join("docs").join(format!("{id}.meta.json"))
    }

    fn valid_id(id: &str) -> bool {
        !id.is_empty() && id.chars().all(|c| c.is_ascii_hexdigit())
    }

    pub fn list(&self) -> std::io::Result<Vec<DocMeta>> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(self.root.join("docs"))? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(id) = name.strip_suffix(".meta.json") {
                if let Some(meta) = self.meta(id) {
                    out.push(meta);
                }
            }
        }
        out.sort_by(|a, b| b.updated.cmp(&a.updated));
        Ok(out)
    }

    pub fn meta(&self, id: &str) -> Option<DocMeta> {
        if !Self::valid_id(id) {
            return None;
        }
        let text = std::fs::read_to_string(self.meta_path(id)).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn create(&self, name: &str, json: &str) -> std::io::Result<DocMeta> {
        let id = new_id();
        let meta = DocMeta {
            id: id.clone(),
            name: name.to_string(),
            created: now(),
            updated: now(),
        };
        std::fs::write(self.doc_path(&id), json)?;
        std::fs::write(self.meta_path(&id), serde_json::to_string_pretty(&meta)?)?;
        Ok(meta)
    }

    pub fn read(&self, id: &str) -> Option<String> {
        if !Self::valid_id(id) {
            return None;
        }
        std::fs::read_to_string(self.doc_path(id)).ok()
    }

    pub fn write(&self, id: &str, json: &str, name: Option<&str>) -> std::io::Result<()> {
        let Some(mut meta) = self.meta(id) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no such document",
            ));
        };
        if let Some(n) = name {
            meta.name = n.to_string();
        }
        meta.updated = now();
        std::fs::write(self.doc_path(id), json)?;
        std::fs::write(self.meta_path(id), serde_json::to_string_pretty(&meta)?)?;
        Ok(())
    }

    pub fn delete(&self, id: &str) -> std::io::Result<()> {
        if !Self::valid_id(id) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no such document",
            ));
        }
        std::fs::remove_file(self.doc_path(id))?;
        std::fs::remove_file(self.meta_path(id))?;
        Ok(())
    }
}
