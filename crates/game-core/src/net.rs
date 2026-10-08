//! Wire types shared by server and client (`NetMessage.cs`, `RoomInfo.cs`,
//! `RoomMember.cs`, `NetProtocol.cs`).
//!
//! The transport changed (TCP/Steam -> HTTP + SSE): what the C# sent as one flat
//! `NetMessage` bag over the socket is now a **match command** (`POST
//! /api/rooms/{id}/act`), room listings (`RoomInfo`) and SSE state frames. So the
//! command is the fields the client sends (`webui/src/core/types.ts`
//! `Command`) -- the 17 join/room/match fields of the C# union are gone rather
//! than carried as dead weight.

use serde::{Deserialize, Serialize};

use crate::scoring::ScoreWeights;
use crate::state::BotMentality;
use crate::MatchMode;

/// `NetProtocol.Game`
pub const GAME: &str = "bandori-monopoly";
/// Protocol version. The original is `"8"` (TCP/Steam); this port speaks HTTP + SSE,
/// so it starts a new number to keep old clients from half-working.
pub const VERSION: &str = "9";
/// `NetProtocol.MaxMessage` -- request body limit, bytes.
pub const MAX_MESSAGE: usize = 1_048_576;
/// `NetProtocol.RejoinWindow` -- seconds a dropped player keeps their player.
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
    crate::msg::Msg::new(if reason.is_empty() {
        "err.join.failed".to_string()
    } else {
        format!("err.join.{reason}")
    })
}

/// `NetMessage.cs` as it survives the move to HTTP: one **match command**.
/// The client sends exactly these fields; `serde(default)` keeps partial
/// commands (`{"act":"roll"}`) working and unknown fields from older clients
/// are ignored rather than rejected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NetMessage {
    /// "roll" | "buy" | "build" | "mortgage" | "redeem" | "play" | "discard" |
    /// "end" | "ban" | "pick" | "deck" | "answer" | "vote" | "leave" | "debug".
    pub act: String,
    /// Solo-only console operation: money / tp / give / draw / state.
    pub debug: String,
    /// `ban` / `pick`: the character id.
    pub character: String,
    /// `play` / `discard`: the card id.
    pub card: String,
    /// `deck` (the deck) / `answer` (the picked options).
    pub cards: Vec<String>,
    /// `buy`/`build`/`mortgage`/`redeem`: the tile; `roll`: the die total the
    /// client is confirming; `answer`/`vote`: the number chosen.
    pub value: i32,
    /// `answer`: the prompt id being answered.
    pub prompt: i32,
    /// `debug`: player index, -1 = the sender. `answer`: the original target
    /// player field (not currently used by the prompt handler).
    pub target: i32,
}

impl Default for NetMessage {
    fn default() -> Self {
        Self {
            act: String::new(),
            debug: String::new(),
            character: String::new(),
            card: String::new(),
            cards: Vec::new(),
            value: 0,
            prompt: 0,
            target: -1,
        }
    }
}

impl NetMessage {
    /// An `act` message, the common case for match input.
    pub fn act(act: impl Into<String>) -> Self {
        Self {
            act: act.into(),
            ..Self::default()
        }
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
    /// Bot decision policy (bots only). Carried on the room record so the match
    /// can apply it when it starts; serde-defaults to standard.
    pub mentality: BotMentality,
}

/// `RoomService.Bot` -- the next bot name not already in `taken`: the data's bot
/// name list in order, then again with a 2, 3, ... suffix.
pub fn bot_name<'a, S: AsRef<str>>(
    names: &[S],
    taken: impl Iterator<Item = &'a str> + Clone,
) -> String {
    if names.is_empty() {
        return String::new();
    }
    for round in 0.. {
        for base in names {
            let base = base.as_ref();
            let name = if round == 0 {
                base.to_string()
            } else {
                format!("{base}{}", round + 1)
            };
            if !taken.clone().any(|t| t == name) {
                return name;
            }
        }
        if round > 64 {
            break; // every name in the list is taken 65 times over
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
