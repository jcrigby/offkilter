//! Accounts: users with argon2id password hashes, and cookie sessions.
//!
//! Users live in `users.json` under the data directory; sessions in
//! `sessions.json` (a token per signed-in browser). Signing in is optional:
//! documents without an owner stay open to everyone on the server, and a
//! signed-in user owns the documents they create and can share them.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::{FromRef, FromRequestParts};
use axum::http::request::Parts;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const COOKIE: &str = "ok_session";
/// Sessions expire after 30 days without use.
const SESSION_SECS: u64 = 30 * 24 * 3600;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    /// Login name, unique, case-insensitive.
    pub name: String,
    /// argon2id PHC string; never sent to clients (they get `UserInfo`).
    pub password_hash: String,
    pub created: u64,
    /// Ids of the teams the user belongs to; filled in per request by the
    /// `CurrentUser` extractor, never stored.
    #[serde(skip)]
    pub teams: Vec<String>,
}

/// What clients see of a user.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserInfo {
    pub id: String,
    pub name: String,
}

impl From<&User> for UserInfo {
    fn from(u: &User) -> UserInfo {
        UserInfo {
            id: u.id.clone(),
            name: u.name.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Session {
    user: String,
    last_used: u64,
}

#[derive(Default, Serialize, Deserialize)]
struct Users {
    users: Vec<User>,
}

#[derive(Clone)]
pub struct UserStore {
    root: PathBuf,
    users: Arc<Mutex<Vec<User>>>,
    sessions: Arc<Mutex<HashMap<String, Session>>>,
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 32 random bytes as hex: session and invitation tokens.
pub(crate) fn token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("names are 2–32 letters, digits, '_' or '-'")]
    BadName,
    #[error("passwords are at least 8 characters")]
    BadPassword,
    #[error("that name is taken")]
    Taken,
    #[error("wrong name or password")]
    Denied,
    #[error("storage error: {0}")]
    Io(#[from] std::io::Error),
}

impl UserStore {
    pub fn open(root: &Path) -> std::io::Result<UserStore> {
        std::fs::create_dir_all(root)?;
        let users: Users = std::fs::read_to_string(root.join("users.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        let sessions: HashMap<String, Session> =
            std::fs::read_to_string(root.join("sessions.json"))
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
                .unwrap_or_default();
        Ok(UserStore {
            root: root.to_path_buf(),
            users: Arc::new(Mutex::new(users.users)),
            sessions: Arc::new(Mutex::new(sessions)),
        })
    }

    fn save_users(&self, users: &[User]) -> std::io::Result<()> {
        let text = serde_json::to_string_pretty(&Users {
            users: users.to_vec(),
        })?;
        std::fs::write(self.root.join("users.json"), text)
    }

    fn save_sessions(&self, sessions: &HashMap<String, Session>) -> std::io::Result<()> {
        std::fs::write(
            self.root.join("sessions.json"),
            serde_json::to_string_pretty(sessions)?,
        )
    }

    fn valid_name(name: &str) -> bool {
        let n = name.chars().count();
        (2..=32).contains(&n)
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    }

    /// Creates a user and returns them with a fresh session token.
    pub fn register(&self, name: &str, password: &str) -> Result<(User, String), AuthError> {
        if !Self::valid_name(name) {
            return Err(AuthError::BadName);
        }
        if password.chars().count() < 8 {
            return Err(AuthError::BadPassword);
        }
        let salt = SaltString::generate(&mut rand::thread_rng());
        let hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|_| AuthError::BadPassword)?
            .to_string();
        let mut users = self.users.lock().unwrap();
        if users.iter().any(|u| u.name.eq_ignore_ascii_case(name)) {
            return Err(AuthError::Taken);
        }
        let user = User {
            id: token()[..16].to_string(),
            name: name.to_string(),
            password_hash: hash,
            created: now(),
            teams: Vec::new(),
        };
        users.push(user.clone());
        self.save_users(&users)?;
        drop(users);
        let tok = self.start_session(&user.id)?;
        Ok((user, tok))
    }

    pub fn login(&self, name: &str, password: &str) -> Result<(User, String), AuthError> {
        let user = {
            let users = self.users.lock().unwrap();
            users
                .iter()
                .find(|u| u.name.eq_ignore_ascii_case(name))
                .cloned()
                .ok_or(AuthError::Denied)?
        };
        let parsed = PasswordHash::new(&user.password_hash).map_err(|_| AuthError::Denied)?;
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .map_err(|_| AuthError::Denied)?;
        let tok = self.start_session(&user.id)?;
        Ok((user, tok))
    }

    fn start_session(&self, user: &str) -> std::io::Result<String> {
        let tok = token();
        let mut sessions = self.sessions.lock().unwrap();
        let t = now();
        sessions.retain(|_, s| t.saturating_sub(s.last_used) < SESSION_SECS);
        sessions.insert(
            tok.clone(),
            Session {
                user: user.to_string(),
                last_used: t,
            },
        );
        self.save_sessions(&sessions)?;
        Ok(tok)
    }

    pub fn logout(&self, tok: &str) {
        let mut sessions = self.sessions.lock().unwrap();
        if sessions.remove(tok).is_some() {
            let _ = self.save_sessions(&sessions);
        }
    }

    /// The user a session token belongs to, if it is valid.
    pub fn session_user(&self, tok: &str) -> Option<User> {
        let user_id = {
            let mut sessions = self.sessions.lock().unwrap();
            let t = now();
            let (user, touch) = {
                let s = sessions.get_mut(tok)?;
                if t.saturating_sub(s.last_used) >= SESSION_SECS {
                    sessions.remove(tok);
                    return None;
                }
                // Touch at most hourly to avoid a disk write per request.
                let touch = t - s.last_used > 3600;
                if touch {
                    s.last_used = t;
                }
                (s.user.clone(), touch)
            };
            if touch {
                let _ = self.save_sessions(&sessions);
            }
            user
        };
        self.user(&user_id)
    }

    pub fn user(&self, id: &str) -> Option<User> {
        self.users
            .lock()
            .unwrap()
            .iter()
            .find(|u| u.id == id)
            .cloned()
    }

    pub fn user_by_name(&self, name: &str) -> Option<User> {
        self.users
            .lock()
            .unwrap()
            .iter()
            .find(|u| u.name.eq_ignore_ascii_case(name))
            .cloned()
    }
}

static SECURE_COOKIES: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Marks session cookies `Secure` (sent over HTTPS only). Set this when the
/// server is reached through TLS, directly or behind a terminating proxy.
pub fn set_secure_cookies(secure: bool) {
    SECURE_COOKIES.store(secure, std::sync::atomic::Ordering::Relaxed);
}

fn cookie_flags() -> &'static str {
    if SECURE_COOKIES.load(std::sync::atomic::Ordering::Relaxed) {
        "; Path=/; HttpOnly; SameSite=Lax; Secure"
    } else {
        "; Path=/; HttpOnly; SameSite=Lax"
    }
}

/// Cookie value for a session token.
pub fn session_cookie(tok: &str) -> String {
    format!("{COOKIE}={tok}{}; Max-Age={SESSION_SECS}", cookie_flags())
}

pub fn clear_cookie() -> String {
    format!("{COOKIE}={}; Max-Age=0", cookie_flags())
}

/// The session token in a request's cookies, if any.
pub fn token_from_parts(parts: &Parts) -> Option<String> {
    let header = parts
        .headers
        .get(axum::http::header::COOKIE)?
        .to_str()
        .ok()?;
    header.split(';').find_map(|kv| {
        let (k, v) = kv.trim().split_once('=')?;
        (k == COOKIE).then(|| v.to_string())
    })
}

/// Extractor: the signed-in user, or `None` for anonymous requests.
pub struct CurrentUser(pub Option<User>);

impl<S> FromRequestParts<S> for CurrentUser
where
    UserStore: axum::extract::FromRef<S>,
    crate::teams::TeamStore: axum::extract::FromRef<S>,
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let users = UserStore::from_ref(state);
        let teams = crate::teams::TeamStore::from_ref(state);
        let user = token_from_parts(parts)
            .and_then(|t| users.session_user(&t))
            .map(|mut u| {
                u.teams = teams.ids_for(&u.id);
                u
            });
        Ok(CurrentUser(user))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> UserStore {
        let dir = std::env::temp_dir().join(format!(
            "ok-auth-test-{}-{}",
            std::process::id(),
            now() ^ rand::random::<u64>()
        ));
        UserStore::open(&dir).unwrap()
    }

    #[test]
    fn register_login_session_logout() {
        let s = temp();
        let (u, tok) = s.register("alice", "correct horse").unwrap();
        assert_eq!(s.session_user(&tok).unwrap().id, u.id);
        assert!(matches!(
            s.register("Alice", "another one"),
            Err(AuthError::Taken)
        ));
        assert!(matches!(
            s.register("a", "longenough"),
            Err(AuthError::BadName)
        ));
        assert!(matches!(
            s.register("bob", "short"),
            Err(AuthError::BadPassword)
        ));
        assert!(matches!(s.login("alice", "wrong"), Err(AuthError::Denied)));
        let (u2, tok2) = s.login("ALICE", "correct horse").unwrap();
        assert_eq!(u2.id, u.id);
        assert_ne!(tok, tok2);
        s.logout(&tok);
        assert!(s.session_user(&tok).is_none());
        assert!(s.session_user(&tok2).is_some());
        // Reopening the store keeps users and sessions.
        let again = UserStore::open(&s.root).unwrap();
        assert!(again.session_user(&tok2).is_some());
        assert_eq!(again.user_by_name("alice").unwrap().id, u.id);
    }
}
