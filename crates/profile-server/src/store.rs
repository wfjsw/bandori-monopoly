//! Durable account and game-server tables.
//!
//! Both are small JSON documents under the data directory, rewritten
//! atomically (write `*.tmp`, then rename) under one mutex. Accounts are
//! looked up by cleaned display name; the id -- not the name -- is the stable
//! identity a ticket carries.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sha2::Digest;

use crate::error::{ApiError, ApiResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub persona: String,
    #[serde(default)]
    pub cn_id: String,
    #[serde(default)]
    pub profile: Option<String>,
    pub created: u64,
    pub last_seen: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameServer {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub note: String,
}

/// One uploaded game-data blob (a rule build, a ruleset archive, tables).
/// Bytes live at `uploads/{id}/{name}`; the record is the index.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Upload {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: String,
    pub size: u64,
    pub sha256: String,
    #[serde(default)]
    pub owner: String,
    pub created: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct Doc<T> {
    #[serde(default = "Vec::new")]
    items: Vec<T>,
}

impl<T> Default for Doc<T> {
    fn default() -> Self {
        Self { items: Vec::new() }
    }
}

pub struct Store {
    dir: PathBuf,
    accounts: Mutex<Doc<Account>>,
    servers: Mutex<Doc<GameServer>>,
    uploads: Mutex<Doc<Upload>>,
}

fn read_doc<T: for<'de> Deserialize<'de>>(path: &Path) -> Doc<T> {
    match std::fs::read_to_string(path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
            eprintln!("{}: bad JSON ({e}); starting empty", path.display());
            Doc::default()
        }),
        Err(_) => Doc::default(),
    }
}

fn write_doc<T: Serialize>(path: &Path, doc: &Doc<T>) -> Result<(), String> {
    let parent = path.parent().ok_or("no parent directory")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(doc).map_err(|e| e.to_string())?;
    {
        let mut f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        f.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
    }
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

impl Store {
    pub fn open(dir: impl Into<PathBuf>) -> Result<Self, String> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let accounts = read_doc(&dir.join("accounts.json"));
        let servers = read_doc(&dir.join("servers.json"));
        let uploads = read_doc(&dir.join("uploads.json"));
        Ok(Self {
            dir,
            accounts: Mutex::new(accounts),
            servers: Mutex::new(servers),
            uploads: Mutex::new(uploads),
        })
    }

    fn account_path(&self) -> PathBuf {
        self.dir.join("accounts.json")
    }

    fn server_path(&self) -> PathBuf {
        self.dir.join("servers.json")
    }

    fn upload_path(&self) -> PathBuf {
        self.dir.join("uploads.json")
    }

    /// Directory holding upload `id`'s bytes.
    pub fn upload_dir(&self, id: &str) -> PathBuf {
        self.dir.join("uploads").join(id)
    }

    /// The account with display name `name`, if any.
    pub fn account_by_name(&self, name: &str) -> Option<Account> {
        self.accounts
            .lock()
            .unwrap()
            .items
            .iter()
            .find(|a| a.name == name)
            .cloned()
    }

    pub fn account(&self, id: &str) -> Option<Account> {
        self.accounts
            .lock()
            .unwrap()
            .items
            .iter()
            .find(|a| a.id == id)
            .cloned()
    }

    /// Find the account named `name`, or create it. Returns the account.
    pub fn ensure_account(
        &self,
        name: &str,
        persona: &str,
        cn_id: &str,
        now: u64,
    ) -> ApiResult<Account> {
        if let Some(mut a) = self.account_by_name(name) {
            a.last_seen = now;
            if !persona.is_empty() {
                a.persona = persona.to_string();
            }
            if !cn_id.is_empty() {
                a.cn_id = cn_id.to_string();
            }
            self.update_account(a.clone())?;
            return Ok(a);
        }
        let a = Account {
            id: random_hex(12),
            name: name.to_string(),
            persona: persona.to_string(),
            cn_id: cn_id.to_string(),
            profile: None,
            created: now,
            last_seen: now,
        };
        let mut doc = self.accounts.lock().unwrap();
        doc.items.push(a.clone());
        write_doc(&self.account_path(), &doc).map_err(ApiError::internal)?;
        Ok(a)
    }

    pub fn update_account(&self, a: Account) -> ApiResult<Account> {
        let mut doc = self.accounts.lock().unwrap();
        let Some(slot) = doc.items.iter_mut().find(|x| x.id == a.id) else {
            return Err(ApiError::unauthorized());
        };
        *slot = a.clone();
        write_doc(&self.account_path(), &doc).map_err(ApiError::internal)?;
        Ok(a)
    }

    pub fn set_profile(&self, id: &str, profile: String) -> ApiResult<()> {
        let mut a = self.account(id).ok_or_else(ApiError::unauthorized)?;
        a.profile = Some(profile);
        a.last_seen = crate::now();
        self.update_account(a).map(|_| ())
    }

    pub fn servers(&self) -> Vec<GameServer> {
        self.servers.lock().unwrap().items.clone()
    }

    pub fn add_server(&self, name: &str, url: &str, note: &str) -> ApiResult<GameServer> {
        let url = url.trim().trim_end_matches('/').to_string();
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err(ApiError::bad("err.server.url"));
        }
        let mut doc = self.servers.lock().unwrap();
        if doc.items.iter().any(|s| s.url == url) {
            return Err(ApiError::bad("err.server.duplicate"));
        }
        let s = GameServer {
            id: random_hex(6),
            name: name.trim().chars().take(32).collect(),
            url,
            note: note.trim().chars().take(120).collect(),
        };
        doc.items.push(s.clone());
        write_doc(&self.server_path(), &doc).map_err(ApiError::internal)?;
        Ok(s)
    }

    pub fn remove_server(&self, id: &str) -> ApiResult<bool> {
        let mut doc = self.servers.lock().unwrap();
        let before = doc.items.len();
        doc.items.retain(|s| s.id != id);
        let removed = doc.items.len() != before;
        if removed {
            write_doc(&self.server_path(), &doc).map_err(ApiError::internal)?;
        }
        Ok(removed)
    }

    pub fn uploads(&self) -> Vec<Upload> {
        self.uploads.lock().unwrap().items.clone()
    }

    pub fn upload(&self, id: &str) -> Option<Upload> {
        self.uploads
            .lock()
            .unwrap()
            .items
            .iter()
            .find(|u| u.id == id)
            .cloned()
    }

    /// Store one uploaded blob. The bytes land under `uploads/{id}/` before
    /// the index entry is written; a failed index write removes them again.
    pub fn put_upload(&self, name: &str, kind: &str, owner: &str, bytes: &[u8]) -> ApiResult<Upload> {
        let id = random_hex(12);
        let name = safe_filename(name);
        let dir = self.upload_dir(&id);
        std::fs::create_dir_all(&dir).map_err(|e| ApiError::internal(e.to_string()))?;
        let file = dir.join(&name);
        let tmp = dir.join(".upload.tmp");
        std::fs::write(&tmp, bytes)
            .and_then(|_| std::fs::rename(&tmp, &file))
            .map_err(|e| ApiError::internal(e.to_string()))?;

        let digest = sha2::Sha256::digest(bytes);
        let sha256: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        let u = Upload {
            id,
            name,
            kind: kind.trim().chars().take(32).collect(),
            size: bytes.len() as u64,
            sha256,
            owner: owner.to_string(),
            created: crate::now(),
        };
        let mut doc = self.uploads.lock().unwrap();
        doc.items.push(u.clone());
        if let Err(e) = write_doc(&self.upload_path(), &doc) {
            doc.items.pop();
            let _ = std::fs::remove_dir_all(&dir);
            return Err(ApiError::internal(e));
        }
        Ok(u)
    }

    /// Remove an upload and its bytes. `owner` `""` matches anyone.
    pub fn remove_upload(&self, id: &str, owner: &str) -> ApiResult<bool> {
        let mut doc = self.uploads.lock().unwrap();
        let Some(pos) = doc
            .items
            .iter()
            .position(|u| u.id == id && (owner.is_empty() || u.owner == owner))
        else {
            return Ok(false);
        };
        let u = doc.items.remove(pos);
        write_doc(&self.upload_path(), &doc).map_err(ApiError::internal)?;
        let _ = std::fs::remove_dir_all(self.upload_dir(&u.id));
        Ok(true)
    }
}

/// A display name reduced to something safe as a single path component.
fn safe_filename(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = base
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        .take(96)
        .collect();
    let cleaned = cleaned.trim_start_matches('.').to_string();
    if cleaned.is_empty() { "upload.bin".to_string() } else { cleaned }
}

/// `n` random bytes as hex.
pub fn random_hex(n: usize) -> String {
    let mut buf = vec![0u8; n];
    getrandom::fill(&mut buf).expect("system randomness");
    buf.iter().map(|b| format!("{b:02x}")).collect()
}
