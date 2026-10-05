//! Wire types shared by server and client (`NetMessage.cs`, `RoomInfo.cs`,
//! `RoomMember.cs`, `NetProtocol.cs`).
//!
//! The transport changed (TCP/Steam -> HTTP + SSE), but the payload vocabulary did
//! not: `NetMessage` is still what `MatchEngine::act` consumes, and the golden-master
//! oracle emits it, so the shape is kept flat and field-for-field.

use serde::{Deserialize, Serialize};

use crate::scoring::ScoreWeights;
use crate::state::MatchState;
use crate::MatchMode;

/// `NetProtocol.Game`
pub const GAME: &str = "bandori-monopoly";
/// Protocol version. The original is `"8"` (TCP/Steam); this port speaks HTTP + SSE,
/// so it starts a new number to keep old clients from half-working.
pub const VERSION: &str = "9";
/// `NetProtocol.MaxMessage` -- request body limit, bytes.
pub const MAX_MESSAGE: usize = 1_048_576;
/// `NetProtocol.RejoinWindow` -- seconds a dropped player keeps their seat.
pub const REJOIN_WINDOW: f32 = 120.0;
/// `NetProtocol.Timeout` -- seconds of silence before a client counts as dropped.
pub const TIMEOUT: f32 = 20.0;

/// `NetProtocol.Reject*` reason codes.
pub mod reject {
    pub const PASSWORD: &str = "password";
    pub const FULL: &str = "full";
    pub const PLAYING: &str = "playing";
    pub const VERSION: &str = "version";
    pub const REJOIN: &str = "rejoin";
}

/// `NetProtocol.Describe(reason)` -- the message for a reject reason
/// (`err.join.<reason>`; the client has one per reason code).
pub fn describe(reason: &str) -> crate::msg::Msg {
    crate::msg::Msg::new(if reason.is_empty() { "err.join.failed".to_string() } else { format!("err.join.{reason}") })
}

/// `NetMessage.cs` -- one flat bag; `t` says which fields matter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NetMessage {
    pub t: String,
    pub version: String,
    pub player: String,
    pub character: String,
    pub cn_id: String,
    pub password: String,
    pub reason: String,
    pub you: i32,
    pub on: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room: Option<RoomInfo>,
    pub act: String,
    pub card: String,
    pub cards: Vec<String>,
    pub notes: Vec<String>,
    pub value: i32,
    pub prompt: i32,
    pub op: String,
    pub target: i32,
    pub arg: String,
    #[serde(rename = "match", skip_serializing_if = "Option::is_none")]
    pub r#match: Option<MatchState>,
    pub token: String,
    pub rejoin: i32,
    pub seq: i32,
}

impl Default for NetMessage {
    fn default() -> Self {
        Self {
            t: String::new(),
            version: String::new(),
            player: String::new(),
            character: String::new(),
            cn_id: String::new(),
            password: String::new(),
            reason: String::new(),
            you: 0,
            on: false,
            room: None,
            act: String::new(),
            card: String::new(),
            cards: vec![],
            notes: vec![],
            value: 0,
            prompt: 0,
            op: String::new(),
            target: -1,
            arg: String::new(),
            r#match: None,
            token: String::new(),
            rejoin: 0,
            seq: 0,
        }
    }
}

impl NetMessage {
    /// An `act` message, the common case for match input.
    pub fn act(act: impl Into<String>) -> Self {
        Self { t: "act".into(), act: act.into(), ..Self::default() }
    }
}

/// `RoomInfo.cs`. The C# `[NonSerialized]` transport fields (Steam lobby, address,
/// port, LAN source) have no equivalent over HTTP and are dropped.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RoomInfo {
    pub id: String,
    pub name: String,
    pub ranked: bool,
    pub max_players: i32,
    pub locked: bool,
    pub playing: bool,
    pub theme: String,
    pub weights: ScoreWeights,
    pub members: Vec<RoomMember>,
}


impl RoomInfo {
    pub fn mode(&self) -> MatchMode {
        if self.ranked {
            MatchMode::Ranked
        } else {
            MatchMode::Casual
        }
    }

    /// Player counts allowed to start: Ranked 5-6, Casual 3-10 (from the game's own
    /// multiplayer guide; the host may force-start below the minimum).
    pub fn player_range(&self) -> (usize, usize) {
        if self.ranked {
            (5, 6)
        } else {
            (3, 10)
        }
    }

    pub fn host(&self) -> Option<&RoomMember> {
        self.members.iter().find(|m| m.host)
    }

    pub fn is_full(&self) -> bool {
        self.members.len() as i32 >= self.max_players
    }
}

/// `RoomMember.cs` (the client-local `local` flag is not serialized).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RoomMember {
    pub id: i32,
    pub player: String,
    pub character: String,
    pub cn_id: String,
    pub ready: bool,
    pub host: bool,
    pub bot: bool,
    pub away: bool,
}

/// `RoomService.Bot` -- the next bot name not already in `taken`: the data's bot
/// name list in order, then again with a 2, 3, ... suffix.
pub fn bot_name<'a, S: AsRef<str>>(names: &[S], taken: impl Iterator<Item = &'a str> + Clone) -> String {
    for k in 0..names.len() * 3 {
        let base = names[k % names.len()].as_ref();
        let name = if k >= names.len() { format!("{base}{}", k / names.len() + 1) } else { base.to_string() };
        if !taken.clone().any(|t| t == name) {
            return name;
        }
    }
    String::new()
}

/// `RoomHost.CleanName` (RoomHost.cs:391) -- trim, cap at 16 **UTF-16 code units**
/// (C# `string.Length`). Unlike C# `Substring`, a surrogate pair cut in half at the
/// limit is dropped instead of leaving an invalid half character. May return ""
/// (callers reject empty names).
pub fn clean_name(name: &str) -> String {
    let mut out = String::new();
    let mut units = 0;
    for ch in name.trim().chars() {
        units += ch.len_utf16();
        if units > 16 {
            break;
        }
        out.push(ch);
    }
    out
}
