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
    /// Next id-range prefix to hand to a connecting client (monotonic).
    #[serde(default = "first_prefix")]
    pub next_prefix: u32,
}

fn first_prefix() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionMeta {
    pub id: String,
    pub name: String,
    pub created: u64,
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
            next_prefix: 1,
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

    /// Reserves and returns the next client id-range prefix for a document.
    pub fn take_prefix(&self, id: &str) -> std::io::Result<u32> {
        let Some(mut meta) = self.meta(id) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no such document",
            ));
        };
        let prefix = meta.next_prefix;
        meta.next_prefix = (prefix.wrapping_add(1) & 0xfff).max(1);
        std::fs::write(self.meta_path(id), serde_json::to_string_pretty(&meta)?)?;
        Ok(prefix)
    }

    fn versions_dir(&self, id: &str) -> PathBuf {
        self.root.join("docs").join(format!("{id}.versions"))
    }

    /// Saves the current document as a named version.
    pub fn save_version(&self, id: &str, name: &str) -> std::io::Result<VersionMeta> {
        let Some(json) = self.read(id) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no such document",
            ));
        };
        let dir = self.versions_dir(id);
        std::fs::create_dir_all(&dir)?;
        let vid = new_id();
        let meta = VersionMeta {
            id: vid.clone(),
            name: name.to_string(),
            created: now(),
        };
        std::fs::write(dir.join(format!("{vid}.okpart")), json)?;
        std::fs::write(
            dir.join(format!("{vid}.meta.json")),
            serde_json::to_string_pretty(&meta)?,
        )?;
        Ok(meta)
    }

    pub fn list_versions(&self, id: &str) -> std::io::Result<Vec<VersionMeta>> {
        if !Self::valid_id(id) {
            return Ok(Vec::new());
        }
        let dir = self.versions_dir(id);
        let mut out = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".meta.json") {
                    if let Ok(text) = std::fs::read_to_string(entry.path()) {
                        if let Ok(meta) = serde_json::from_str::<VersionMeta>(&text) {
                            out.push(meta);
                        }
                    }
                }
            }
        }
        out.sort_by(|a, b| b.created.cmp(&a.created));
        Ok(out)
    }

    pub fn read_version(&self, id: &str, vid: &str) -> Option<String> {
        if !Self::valid_id(id) || !Self::valid_id(vid) {
            return None;
        }
        std::fs::read_to_string(self.versions_dir(id).join(format!("{vid}.okpart"))).ok()
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
        let _ = std::fs::remove_dir_all(self.versions_dir(id));
        Ok(())
    }
}
