//! Durable archive of finished online matches (`docs/REPLAY.md` §1, §4).
//!
//! The store keeps the **last** record per room for the immediate download and
//! may drop it (Redis expires it after 24 h; the in-memory store is an LRU).
//! That copy is a cache. Every sealed record is also written here, once, under
//! the deployment's record directory -- one `.bdrec` plus a `.json` sidecar per
//! match -- so a TTL, a restart, or the next match in the same room never takes
//! it away. A deployment with several processes must point all of them at the
//! same directory (shared mount / same host); the archive is the durable half,
//! the store is the coordination half.
//!
//! Writes are atomic: a record is written to a hidden temp file and renamed
//! into place, then its sidecar the same way. A crash can leave an orphaned
//! `.bdrec` without a sidecar (never listed); it can never expose a half
//! written record under a listed name.
//!
//! Layout:
//!
//! ```text
//! <root>/<room>/<match>.bdrec    the sealed zstd-framed record, as served
//! <root>/<room>/<match>.json     ArchiveMeta: created, seats, members, ...
//! ```
//!
//! Ids are server-generated (`state::random_hex`), so they are safe path
//! components; [`safe`] still rejects anything that is not.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One seat of an archived match, copied from the final view at seal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchiveSeat {
    pub member: i32,
    pub player: String,
    pub bot: bool,
    pub persona: String,
    pub rank: i32,
    pub score: i32,
}

/// What the sidecar holds. Nothing in here reveals a hand; the record beside
/// it does, so the endpoints gate both the same way (`members`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchiveMeta {
    /// The match id (also the file stem).
    pub id: String,
    pub room: String,
    /// `yyyy-MM-dd HH:mm` UTC, for sorting and display.
    pub created: String,
    pub ended: bool,
    /// The match's end reason (`MatchState::end_reason`).
    pub reason: String,
    pub rounds: i32,
    #[serde(with = "game_core::record::u64_str")]
    pub total_ticks: u64,
    pub seats: Vec<ArchiveSeat>,
    /// Member ids, the download authorization set.
    pub members: Vec<i32>,
}

/// An archived match with its bytes.
#[derive(Debug, Clone, PartialEq)]
pub struct ArchivedRecord {
    pub meta: ArchiveMeta,
    pub record: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Archive {
    root: PathBuf,
}

/// A path component the server itself generates. Room and match ids are hex;
/// the rule is spelled out so a caller cannot escape the root.
fn safe(part: &str) -> bool {
    !part.is_empty()
        && part.len() <= 64
        && part
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn stamp(created: &str) -> String {
    created
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>()
}

fn io(e: std::io::Error, at: &Path) -> String {
    format!("{}: {e}", at.display())
}

impl Archive {
    /// `root` is created on first write; a read of an absent root is simply
    /// empty.
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn dir(&self, room: &str) -> PathBuf {
        self.root.join(room)
    }

    fn rec_path(&self, room: &str, id: &str) -> PathBuf {
        self.dir(room).join(format!("{id}.bdrec"))
    }

    fn meta_path(&self, room: &str, id: &str) -> PathBuf {
        self.dir(room).join(format!("{id}.json"))
    }

    /// Write one record and its sidecar. `record` is the exact sealed bytes;
    /// they are not re-encoded here.
    pub fn save(&self, meta: &ArchiveMeta, record: &[u8]) -> Result<PathBuf, String> {
        if !safe(&meta.room) || !safe(&meta.id) {
            return Err(format!("unsafe archive id {:?} / {:?}", meta.room, meta.id));
        }
        let dir = self.dir(&meta.room);
        fs::create_dir_all(&dir).map_err(|e| io(e, &dir))?;
        let json = serde_json::to_vec(meta).map_err(|e| e.to_string())?;
        // Record first: a listed sidecar always has its bytes beside it.
        write_atomic(&self.rec_path(&meta.room, &meta.id), record)?;
        write_atomic(&self.meta_path(&meta.room, &meta.id), &json)?;
        Ok(self.rec_path(&meta.room, &meta.id))
    }

    /// Newest first. Metadata only -- a caller that wants bytes follows up
    /// with [`Self::get`] for the entries it will serve.
    pub fn list(&self, room: &str) -> Result<Vec<ArchiveMeta>, String> {
        if !safe(room) {
            return Ok(Vec::new());
        }
        let dir = self.dir(room);
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(io(e, &dir)),
        };
        let mut out = Vec::new();
        for entry in entries {
            let path = entry.map_err(|e| io(e, &dir))?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            // One unreadable or hand-edited sidecar must not take the whole
            // room's list down; it is simply not listed.
            let Ok(Some(meta)) = read_meta(&path) else {
                continue;
            };
            if meta.room != room {
                continue;
            }
            // The record must still be there; an orphaned sidecar is not a
            // download.
            if !self.rec_path(room, &meta.id).is_file() {
                continue;
            }
            out.push(meta);
        }
        // `created` sorts lexicographically in its fixed format; the id breaks
        // ties between two matches in the same minute.
        out.sort_by(|a, b| b.created.cmp(&a.created).then_with(|| b.id.cmp(&a.id)));
        Ok(out)
    }

    /// The newest match in `room`.
    pub fn last(&self, room: &str) -> Result<Option<ArchivedRecord>, String> {
        match self.list(room)?.into_iter().next() {
            Some(meta) => self.get(room, &meta.id),
            None => Ok(None),
        }
    }

    /// One match by id.
    pub fn get(&self, room: &str, id: &str) -> Result<Option<ArchivedRecord>, String> {
        if !safe(room) || !safe(id) {
            return Ok(None);
        }
        let path = self.meta_path(room, id);
        let Some(meta) = read_meta(&path)? else {
            return Ok(None);
        };
        if meta.room != room || meta.id != id {
            return Ok(None);
        }
        let rec = self.rec_path(room, id);
        let record = match fs::read(&rec) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(io(e, &rec)),
        };
        Ok(Some(ArchivedRecord { meta, record }))
    }
}

/// `None` when the file vanished between the directory read and this call.
fn read_meta(path: &Path) -> Result<Option<ArchiveMeta>, String> {
    let text = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io(e, path)),
    };
    serde_json::from_slice(&text)
        .map(Some)
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// Write through a unique temp file, sync, then rename. A concurrent reader
/// sees either the old file or the complete new one.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let dir = path.parent().ok_or_else(|| "no parent".to_string())?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("record");
    let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    {
        let mut f = fs::File::create(&tmp).map_err(|e| io(e, &tmp))?;
        f.write_all(bytes).map_err(|e| io(e, &tmp))?;
        f.sync_all().map_err(|e| io(e, &tmp))?;
    }
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        io(e, path)
    })
}

/// The download file name for an archived match.
pub fn download_name(meta: &ArchiveMeta) -> String {
    format!("bdrec-{}-{}-{}.bdrec", meta.room, stamp(&meta.created), meta.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "bm-archive-{tag}-{}-{}",
            std::process::id(),
            crate::state::random_hex(4)
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn meta(room: &str, id: &str, created: &str) -> ArchiveMeta {
        ArchiveMeta {
            id: id.into(),
            room: room.into(),
            created: created.into(),
            ended: true,
            reason: "score".into(),
            rounds: 3,
            total_ticks: 100,
            seats: vec![],
            members: vec![1],
        }
    }

    #[test]
    fn save_list_get_roundtrip_newest_first() {
        let dir = tmp("list");
        let a = Archive::new(dir.clone());
        a.save(&meta("R", "m1", "2026-01-01 10:00"), b"one").unwrap();
        a.save(&meta("R", "m2", "2026-01-02 10:00"), b"two").unwrap();
        a.save(&meta("R", "m3", "2026-01-02 10:00"), b"three")
            .unwrap();

        let list = a.list("R").unwrap();
        assert_eq!(
            list.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            ["m3", "m2", "m1"]
        );
        assert_eq!(a.last("R").unwrap().unwrap().record, b"three");
        assert_eq!(a.get("R", "m1").unwrap().unwrap().record, b"one");
        assert!(a.get("R", "nope").unwrap().is_none());
        assert!(a.list("other").unwrap().is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_sidecar_without_bytes_is_not_listed() {
        let dir = tmp("orphan");
        let a = Archive::new(dir.clone());
        a.save(&meta("R", "m1", "2026-01-01 10:00"), b"one").unwrap();
        fs::remove_file(a.rec_path("R", "m1")).unwrap();
        assert!(a.list("R").unwrap().is_empty());
        assert!(a.get("R", "m1").unwrap().is_none());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn unsafe_components_stay_inside_the_root() {
        let dir = tmp("safe");
        let a = Archive::new(dir.clone());
        a.save(&meta("R", "m1", "2026-01-01 10:00"), b"one").unwrap();
        assert!(a.get("../R", "m1").unwrap().is_none());
        assert!(!safe("../R"));
        assert!(safe("aB3-_"));
        assert!(download_name(&meta("R", "m1", "2026-01-01 10:00")).ends_with("-m1.bdrec"));
        let _ = fs::remove_dir_all(dir);
    }
}
