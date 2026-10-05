//! Localizable messages.
//!
//! The engine and the server never produce display text. Every log line, prompt
//! and error is a [`Msg`]: an i18next key plus typed arguments. Each client renders
//! it in its own player's language (`webui/src/locales/*`), resolving players to
//! player names, tiles / cards / characters to their (localized) data names.
//!
//! Keys without a namespace live in the client's `game` namespace; card modules use
//! `cards:<crate>.<key>`.
//!
//! Wire form: `{"k": "log.roll", "a": {"who": {"player_id": 2}, "n": {"n": 6}}}`.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// Args are shared across [`Msg`] clones: a match keeps hundreds of messages in
/// its event tail and clones them into every routine snapshot, so a deep copy
/// per clone was a real cost. Building a message still copy-on-writes, which is
/// free in practice -- a message is written once and then only read.
fn args_empty(a: &Arc<BTreeMap<String, Arg>>) -> bool {
    a.is_empty()
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Msg {
    /// i18next key, e.g. `log.roll`, `err.not_your_turn`.
    #[serde(default)]
    pub k: Arc<str>,
    #[serde(default, skip_serializing_if = "args_empty")]
    pub a: Arc<BTreeMap<String, Arg>>,
}

/// A message argument. The client turns each into text before interpolation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Arg {
    /// A player id -> that player's name.
    PlayerId(i32),
    /// A board tile index -> the tile's name.
    Tile(i32),
    /// A card id -> the card's title.
    Card(String),
    /// A character name (data key) -> its display name.
    Char(String),
    /// An event card id -> its name.
    Event(String),
    /// A band name (data key) -> its display name.
    Band(String),
    /// A number shown with thousands separators (money, prices).
    N(i64),
    /// A plain integer (counts, dice).
    I(i64),
    /// Verbatim text (player-entered names and the like). Never used for UI copy.
    Text(String),
    /// A nested message (reasons, optional clauses).
    Msg(Box<Msg>),
    /// Several values, joined with the locale's list separator.
    List(Vec<Arg>),
}

impl Msg {
    pub fn new(key: impl AsRef<str>) -> Self {
        Self {
            k: Arc::from(key.as_ref()),
            a: Arc::default(),
        }
    }

    /// Insert one argument in place (for bridges that build a message field by
    /// field). Prefer the builder methods, which chain.
    pub fn set_arg(&mut self, name: impl Into<String>, v: Arg) {
        Arc::make_mut(&mut self.a).insert(name.into(), v);
    }

    pub fn key(&self) -> &str {
        &self.k
    }

    pub fn is_empty(&self) -> bool {
        self.k.is_empty()
    }

    pub fn arg(mut self, name: &str, v: Arg) -> Self {
        Arc::make_mut(&mut self.a).insert(name.into(), v);
        self
    }

    pub fn player_id(self, name: &str, player_id: impl TryInto<i32>) -> Self {
        let s = player_id.try_into().unwrap_or(-1);
        self.arg(name, Arg::PlayerId(s))
    }

    pub fn tile(self, name: &str, tile: impl TryInto<i32>) -> Self {
        let t = tile.try_into().unwrap_or(-1);
        self.arg(name, Arg::Tile(t))
    }

    pub fn card(self, name: &str, id: impl Into<String>) -> Self {
        self.arg(name, Arg::Card(id.into()))
    }

    pub fn chara(self, name: &str, character: impl Into<String>) -> Self {
        self.arg(name, Arg::Char(character.into()))
    }

    pub fn event(self, name: &str, id: impl Into<String>) -> Self {
        self.arg(name, Arg::Event(id.into()))
    }

    /// Money / price (thousands separators).
    pub fn n(self, name: &str, v: impl Into<i64>) -> Self {
        self.arg(name, Arg::N(v.into()))
    }

    /// Count / dice value.
    pub fn i(self, name: &str, v: impl Into<i64>) -> Self {
        self.arg(name, Arg::I(v.into()))
    }

    pub fn text(self, name: &str, v: impl Into<String>) -> Self {
        self.arg(name, Arg::Text(v.into()))
    }

    pub fn msg(self, name: &str, m: Msg) -> Self {
        self.arg(name, Arg::Msg(Box::new(m)))
    }

    /// A nested message only when present (optional clauses render as "").
    pub fn opt(self, name: &str, m: Option<Msg>) -> Self {
        match m {
            Some(m) => self.msg(name, m),
            None => self.msg(name, Msg::new("blank")),
        }
    }

    pub fn list(self, name: &str, items: Vec<Arg>) -> Self {
        self.arg(name, Arg::List(items))
    }
}

impl From<&str> for Msg {
    fn from(key: &str) -> Self {
        Msg::new(key)
    }
}

/// Debug rendering for logs and test failures: `key{a=..}`. Not shown to players.
impl fmt::Display for Msg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.k)?;
        if !self.a.is_empty() {
            f.write_str("{")?;
            for (i, (k, v)) in self.a.iter().enumerate() {
                if i > 0 {
                    f.write_str(",")?;
                }
                write!(f, "{k}={}", ArgDebug(v))?;
            }
            f.write_str("}")?;
        }
        Ok(())
    }
}

struct ArgDebug<'a>(&'a Arg);

impl fmt::Display for ArgDebug<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Arg::PlayerId(s) => write!(f, "playerId:{s}"),
            Arg::Tile(t) => write!(f, "tile:{t}"),
            Arg::Card(c) => write!(f, "card:{c}"),
            Arg::Char(c) => write!(f, "char:{c}"),
            Arg::Event(e) => write!(f, "event:{e}"),
            Arg::Band(b) => write!(f, "band:{b}"),
            Arg::N(n) | Arg::I(n) => write!(f, "{n}"),
            Arg::Text(t) => write!(f, "{t:?}"),
            Arg::Msg(m) => write!(f, "({m})"),
            Arg::List(l) => {
                f.write_str("[")?;
                for (i, a) in l.iter().enumerate() {
                    if i > 0 {
                        f.write_str(",")?;
                    }
                    write!(f, "{}", ArgDebug(a))?;
                }
                f.write_str("]")
            }
        }
    }
}

impl std::error::Error for Msg {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_form_is_compact_and_round_trips() {
        let m = Msg::new("log.roll")
            .player_id("who", 2)
            .i("n", 6)
            .opt("why", None);
        let j = serde_json::to_string(&m).unwrap();
        assert_eq!(
            j,
            r#"{"k":"log.roll","a":{"n":{"i":6},"who":{"playerId":2},"why":{"msg":{"k":"blank"}}}}"#
        );
        assert_eq!(serde_json::from_str::<Msg>(&j).unwrap(), m);
        assert_eq!(
            serde_json::to_string(&Msg::new("err.x")).unwrap(),
            r#"{"k":"err.x"}"#
        );
        assert_eq!(m.to_string(), "log.roll{n=6,who=playerId:2,why=(blank)}");
    }
}
