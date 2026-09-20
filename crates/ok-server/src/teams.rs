//! Teams: named groups of accounts a document can be shared with at once.
//! Stored in `teams.json` next to the users. The creator owns the team and
//! is the only one who can add or remove members or delete it.

use crate::auth::{User, UserInfo};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Team {
    pub id: String,
    pub name: String,
    pub owner: UserInfo,
    /// Members besides the owner.
    #[serde(default)]
    pub members: Vec<UserInfo>,
    pub created: u64,
}

impl Team {
    pub fn has_member(&self, user_id: &str) -> bool {
        self.owner.id == user_id || self.members.iter().any(|m| m.id == user_id)
    }
}

/// A team a document is shared with, and how.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TeamShare {
    pub id: String,
    pub name: String,
    pub role: crate::store::ShareRole,
}

#[derive(Default, Serialize, Deserialize)]
struct Teams {
    teams: Vec<Team>,
}

#[derive(Debug, thiserror::Error)]
pub enum TeamError {
    #[error("team names are 2–48 characters")]
    BadName,
    #[error("no such team")]
    NotFound,
    #[error("only the team's owner can do that")]
    NotOwner,
    #[error("storage error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Clone)]
pub struct TeamStore {
    root: PathBuf,
    teams: Arc<Mutex<Vec<Team>>>,
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl TeamStore {
    pub fn open(root: &Path) -> std::io::Result<TeamStore> {
        std::fs::create_dir_all(root)?;
        let teams: Teams = std::fs::read_to_string(root.join("teams.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        Ok(TeamStore {
            root: root.to_path_buf(),
            teams: Arc::new(Mutex::new(teams.teams)),
        })
    }

    fn save(&self, teams: &[Team]) -> std::io::Result<()> {
        let text = serde_json::to_string_pretty(&Teams {
            teams: teams.to_vec(),
        })?;
        std::fs::write(self.root.join("teams.json"), text)
    }

    pub fn create(&self, name: &str, owner: &User) -> Result<Team, TeamError> {
        let name = name.trim();
        let n = name.chars().count();
        if !(2..=48).contains(&n) {
            return Err(TeamError::BadName);
        }
        let team = Team {
            id: crate::auth::token()[..16].to_string(),
            name: name.to_string(),
            owner: UserInfo::from(owner),
            members: Vec::new(),
            created: now(),
        };
        let mut teams = self.teams.lock().unwrap();
        teams.push(team.clone());
        self.save(&teams)?;
        Ok(team)
    }

    pub fn team(&self, id: &str) -> Option<Team> {
        self.teams
            .lock()
            .unwrap()
            .iter()
            .find(|t| t.id == id)
            .cloned()
    }

    /// Teams the user owns or belongs to.
    pub fn for_user(&self, user_id: &str) -> Vec<Team> {
        self.teams
            .lock()
            .unwrap()
            .iter()
            .filter(|t| t.has_member(user_id))
            .cloned()
            .collect()
    }

    /// Ids of the teams the user owns or belongs to.
    pub fn ids_for(&self, user_id: &str) -> Vec<String> {
        self.for_user(user_id).into_iter().map(|t| t.id).collect()
    }

    fn edit(&self, id: &str, by: &User, f: impl FnOnce(&mut Team)) -> Result<Team, TeamError> {
        let mut teams = self.teams.lock().unwrap();
        let team = teams
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or(TeamError::NotFound)?;
        if team.owner.id != by.id {
            return Err(TeamError::NotOwner);
        }
        f(team);
        let out = team.clone();
        self.save(&teams)?;
        Ok(out)
    }

    pub fn add_member(&self, id: &str, by: &User, member: UserInfo) -> Result<Team, TeamError> {
        self.edit(id, by, |t| {
            if t.owner.id != member.id && !t.members.iter().any(|m| m.id == member.id) {
                t.members.push(member);
            }
        })
    }

    pub fn remove_member(&self, id: &str, by: &User, member_id: &str) -> Result<Team, TeamError> {
        self.edit(id, by, |t| t.members.retain(|m| m.id != member_id))
    }

    pub fn delete(&self, id: &str, by: &User) -> Result<(), TeamError> {
        let mut teams = self.teams.lock().unwrap();
        let pos = teams
            .iter()
            .position(|t| t.id == id)
            .ok_or(TeamError::NotFound)?;
        if teams[pos].owner.id != by.id {
            return Err(TeamError::NotOwner);
        }
        teams.remove(pos);
        self.save(&teams)?;
        Ok(())
    }
}
