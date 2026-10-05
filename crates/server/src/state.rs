//! Server-wide state: sessions and rooms.
//!
//! Locking: `sessions` and each room have their own mutex, and code never holds a
//! room lock while taking the sessions lock (or the reverse). Critical sections are
//! short and never await.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use game_core::data::GameData;
use game_core::engine::CardRules;

use crate::room::Room;

/// Seconds a seated player may be without an open stream before the AI takes over
/// (`NetProtocol.Timeout`).
pub const PRESENCE_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone)]
pub struct Session {
    pub token: String,
    pub player: String,
    pub character: String,
    pub cn_id: String,
    /// Room id and member id while in a room.
    pub room: Option<(String, i32)>,
}

pub struct Server {
    pub data: Arc<GameData>,
    pub rules: Arc<dyn CardRules>,
    pub sessions: Mutex<HashMap<String, Session>>,
    pub rooms: Mutex<HashMap<String, Arc<Mutex<Room>>>>,
    /// Overridable for tests.
    pub presence_timeout: Duration,
    /// Game clock speed (1.0 in production; tests run faster).
    pub time_scale: f32,
}

impl Server {
    pub fn new(data: Arc<GameData>, rules: Arc<dyn CardRules>) -> Arc<Self> {
        Arc::new(Self {
            data,
            rules,
            sessions: Mutex::new(HashMap::new()),
            rooms: Mutex::new(HashMap::new()),
            presence_timeout: PRESENCE_TIMEOUT,
            time_scale: 1.0,
        })
    }

    pub fn session(&self, token: &str) -> Option<Session> {
        self.sessions.lock().unwrap().get(token).cloned()
    }

    pub fn set_room(&self, token: &str, room: Option<(String, i32)>) {
        if let Some(s) = self.sessions.lock().unwrap().get_mut(token) {
            s.room = room;
        }
    }

    pub fn room(&self, id: &str) -> Option<Arc<Mutex<Room>>> {
        self.rooms.lock().unwrap().get(id).cloned()
    }
}

/// `n` random bytes as lower-case hex.
pub fn random_hex(n: usize) -> String {
    let mut buf = vec![0u8; n];
    getrandom::fill(&mut buf).expect("OS randomness");
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn random_u64() -> u64 {
    let mut buf = [0u8; 8];
    getrandom::fill(&mut buf).expect("OS randomness");
    u64::from_le_bytes(buf)
}

/// Short room code, unambiguous characters only.
pub fn room_code() -> String {
    const ALPHABET: &[u8] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ";
    let mut buf = [0u8; 6];
    getrandom::fill(&mut buf).expect("OS randomness");
    buf.iter().map(|b| ALPHABET[*b as usize % ALPHABET.len()] as char).collect()
}
