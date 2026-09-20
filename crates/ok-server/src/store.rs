//! On-disk document storage: one `.okpart` JSON file per document plus a
//! small metadata file.

use crate::auth::{User, UserInfo};
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
    /// The account that created the document; `None` means open to all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<UserInfo>,
    /// Accounts the owner shared the document with, who may edit it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub collaborators: Vec<UserInfo>,
    /// Accounts the owner shared the document with read-only.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub viewers: Vec<UserInfo>,
    /// Open invitation links: any signed-in account that presents a token
    /// joins in that role. Only the owner ever sees these.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub invites: Vec<Invite>,
    /// Teams the owner shared the document with, each in a role.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub teams: Vec<crate::teams::TeamShare>,
    /// Where a branched document came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<BranchOrigin>,
}

/// The document (and version, when one was chosen) a branch was made from.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BranchOrigin {
    pub doc: String,
    pub doc_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_name: Option<String>,
}

/// An invitation link to a document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Invite {
    pub token: String,
    pub role: ShareRole,
    pub created: u64,
}

/// What a shared account may do with a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShareRole {
    Editor,
    Viewer,
}

impl DocMeta {
    /// Whether `user` may open and edit the document.
    pub fn can_access(&self, user: Option<&User>) -> bool {
        match &self.owner {
            None => true,
            Some(o) => user.is_some_and(|u| {
                u.id == o.id
                    || self.collaborators.iter().any(|c| c.id == u.id)
                    || self.viewers.iter().any(|c| c.id == u.id)
                    || self.teams.iter().any(|t| u.teams.contains(&t.id))
            }),
        }
    }

    /// Whether `user` may change the document (owner and editors; anyone
    /// for a document without an owner).
    pub fn can_edit(&self, user: Option<&User>) -> bool {
        match &self.owner {
            None => true,
            Some(o) => user.is_some_and(|u| {
                u.id == o.id
                    || self.collaborators.iter().any(|c| c.id == u.id)
                    || self
                        .teams
                        .iter()
                        .any(|t| t.role == ShareRole::Editor && u.teams.contains(&t.id))
            }),
        }
    }

    /// Whether `user` may delete or share the document.
    pub fn can_manage(&self, user: Option<&User>) -> bool {
        match &self.owner {
            None => true,
            Some(o) => user.is_some_and(|u| u.id == o.id),
        }
    }

    /// The metadata as `user` may see it: invitation tokens are the
    /// owner's business only (with one, a viewer could make itself an
    /// editor).
    pub fn visible_to(mut self, user: Option<&User>) -> DocMeta {
        if !self.can_manage(user) {
            self.invites.clear();
        }
        self
    }
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

    /// The state a branch was made from, kept for three-way merges.
    fn base_path(&self, id: &str) -> PathBuf {
        self.root.join("docs").join(format!("{id}.base.okpart"))
    }

    /// Records a branch's origin and the state it started from.
    pub fn set_branch_base(&self, id: &str, json: &str) -> std::io::Result<()> {
        self.meta_or_not_found(id)?;
        std::fs::write(self.base_path(id), json)
    }

    /// The state a branch started from, if it is a branch.
    pub fn read_base(&self, id: &str) -> Option<String> {
        if !Self::valid_id(id) {
            return None;
        }
        std::fs::read_to_string(self.base_path(id)).ok()
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

    pub fn create(
        &self,
        name: &str,
        json: &str,
        owner: Option<UserInfo>,
    ) -> std::io::Result<DocMeta> {
        let id = new_id();
        let meta = DocMeta {
            id: id.clone(),
            name: name.to_string(),
            created: now(),
            updated: now(),
            next_prefix: 1,
            owner,
            collaborators: Vec::new(),
            viewers: Vec::new(),
            invites: Vec::new(),
            teams: Vec::new(),
            parent: None,
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

    fn write_meta(&self, meta: &DocMeta) -> std::io::Result<()> {
        std::fs::write(
            self.meta_path(&meta.id),
            serde_json::to_string_pretty(meta)?,
        )
    }

    /// Adds a collaborator (idempotent).
    /// Shares with `user` in `role`, replacing any earlier role.
    pub fn share(&self, id: &str, user: UserInfo, role: ShareRole) -> std::io::Result<DocMeta> {
        let Some(mut meta) = self.meta(id) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no such document",
            ));
        };
        if meta.owner.as_ref().is_none_or(|o| o.id != user.id) {
            meta.collaborators.retain(|c| c.id != user.id);
            meta.viewers.retain(|c| c.id != user.id);
            match role {
                ShareRole::Editor => meta.collaborators.push(user),
                ShareRole::Viewer => meta.viewers.push(user),
            }
            self.write_meta(&meta)?;
        }
        Ok(meta)
    }

    pub fn unshare(&self, id: &str, user_id: &str) -> std::io::Result<DocMeta> {
        let Some(mut meta) = self.meta(id) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no such document",
            ));
        };
        meta.collaborators.retain(|c| c.id != user_id);
        meta.viewers.retain(|c| c.id != user_id);
        self.write_meta(&meta)?;
        Ok(meta)
    }

    fn meta_or_not_found(&self, id: &str) -> std::io::Result<DocMeta> {
        self.meta(id)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no such document"))
    }

    /// Records where a branched document came from.
    pub fn set_parent(&self, id: &str, parent: BranchOrigin) -> std::io::Result<DocMeta> {
        let mut meta = self.meta_or_not_found(id)?;
        meta.parent = Some(parent);
        self.write_meta(&meta)?;
        Ok(meta)
    }

    /// Shares with a team in `role`, replacing any earlier role.
    pub fn share_team(&self, id: &str, team: crate::teams::TeamShare) -> std::io::Result<DocMeta> {
        let mut meta = self.meta_or_not_found(id)?;
        meta.teams.retain(|t| t.id != team.id);
        meta.teams.push(team);
        self.write_meta(&meta)?;
        Ok(meta)
    }

    pub fn unshare_team(&self, id: &str, team_id: &str) -> std::io::Result<DocMeta> {
        let mut meta = self.meta_or_not_found(id)?;
        meta.teams.retain(|t| t.id != team_id);
        self.write_meta(&meta)?;
        Ok(meta)
    }

    /// Creates an invitation link for a document.
    pub fn create_invite(&self, id: &str, role: ShareRole) -> std::io::Result<Invite> {
        let mut meta = self.meta_or_not_found(id)?;
        let invite = Invite {
            token: crate::auth::token(),
            role,
            created: now(),
        };
        meta.invites.push(invite.clone());
        self.write_meta(&meta)?;
        Ok(invite)
    }

    /// Withdraws an invitation link; accounts that already joined stay.
    pub fn revoke_invite(&self, id: &str, token: &str) -> std::io::Result<DocMeta> {
        let mut meta = self.meta_or_not_found(id)?;
        meta.invites.retain(|i| i.token != token);
        self.write_meta(&meta)?;
        Ok(meta)
    }

    /// Joins `user` to the document in the invited role, or `NotFound`
    /// when the token is not an open invitation of that document.
    pub fn accept_invite(&self, id: &str, token: &str, user: UserInfo) -> std::io::Result<DocMeta> {
        let meta = self.meta_or_not_found(id)?;
        let Some(invite) = meta.invites.iter().find(|i| i.token == token) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no such invitation",
            ));
        };
        self.share(id, user, invite.role)
    }

    /// Reserves and returns the next client id-range prefix for a document.
    /// A branch takes prefixes from the document it was branched from (or
    /// that one's origin, and so on), so ids stay distinct across a branch
    /// family and merges never see two features with one id.
    pub fn take_prefix(&self, id: &str) -> std::io::Result<u32> {
        let mut root = id.to_string();
        for _ in 0..16 {
            match self.meta(&root).and_then(|m| m.parent) {
                Some(p) if self.meta(&p.doc).is_some() => root = p.doc,
                _ => break,
            }
        }
        let Some(mut meta) = self.meta(&root) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no such document",
            ));
        };
        let prefix = meta.next_prefix;
        meta.next_prefix = (prefix.wrapping_add(1) & 0xfff).max(1);
        std::fs::write(self.meta_path(&root), serde_json::to_string_pretty(&meta)?)?;
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
        let _ = std::fs::remove_file(self.base_path(id));
        let _ = std::fs::remove_dir_all(self.versions_dir(id));
        Ok(())
    }
}
