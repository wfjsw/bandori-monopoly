//! Per-tile deed-text composition (`docs/TILES.md` 「Tile text」).
//!
//! A tile rule (and any other rule instance that speaks for a board tile)
//! declares the deed-popup description its entries speak for
//! (`CardRules::card_texts` / `CardDef::texts`). The view composes, per board
//! tile, the speaking instances in **body-precedence order** -- the same order
//! the settle body runs replacements in, so the one that wins the body also
//! wins the text.
//!
//! Precedence (the generic override rule; no event-specific branching here):
//!
//! 1. **Replacements** -- any speaking instance other than the tile's own
//!    `tile:*` base: player-field instances on that tile, and board-owned
//!    instances that declare a text naming that tile (an `event:*` with a
//!    named tile, an extra attached `tile:*`, …). Ordered by
//!    `effect_order_key` (`docs/ENGINE.md` 「Effect ordering」): lookup group,
//!    then source order (`players[p].field` index / `board_field` index), then
//!    declaration order. Among replacements, **first claim wins** -- same as
//!    the settle body's `cancelled` bail.
//! 2. **Base** -- the `tile:*` instance bound to that tile
//!    (`FieldCard::tile == tile` and `card` is that tile's `tile_rule_id`).
//!    Its texts use `tile: ""` (the instance's own tile). Only wins when no
//!    replacement speaks for that slot.
//!
//! A speaker whose key is `""` suppresses that slot (the effective text is
//! nothing). A replacement that does not declare a given slot has no opinion
//! (the base can still win it). The consumer takes the first entry of each
//! slot; empty key = render nothing.
//!
//! TODO(规则书): the book's 「该改变[触发结算]的效果优先于其他任何改变[触发结算]
//! 的效果」 is **not** implemented as a priority rule -- the engine runs hook
//! instances in placement order / first claim (`event:协助CiRCLE重建` carries
//! the same TODO on its settle body). Text precedence is built on that same
//! ordering, so it is placement order / first claim here too.

use crate::data::{tile_rule_id, GameData};
use crate::engine::rules::{CardRules, TileTextDecl};
use crate::engine::World;
use crate::state::{FieldCard, TileTextEntry};

/// The lookup-group half of `game_rules`'s `effect_order_key`
/// (`docs/ENGINE.md` 「Effect ordering」), kept in step by hand -- `game-core`
/// does not depend on `game-rules`. Status → band → character → field →
/// hand → discard → deck → removed → Board (board-owned `tile:*` / `event:*` /
/// `mark:*` sort after the player groups).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
#[allow(dead_code)] // the full scan order, not just the groups we walk today
enum LookupGroup {
    Status = 0,
    Band = 1,
    Character = 2,
    Field = 3,
    Hand = 4,
    Discard = 5,
    Deck = 6,
    Removed = 7,
    Board = 8,
}

/// One speaking declaration, pre-resolution.
struct Speak {
    /// `(lookup group, source order, declaration order)` -- `effect_order_key`.
    key: (u8, u32, u32),
    tile: i32,
    from: String,
    slot: String,
    key_text: String,
    /// The tile's own `tile:*` base speaking for its own tile.
    base: bool,
}

/// Compose `MatchState::tile_texts`: one speaker list per board tile, in
/// body-precedence order (replacements first, base last). Pure -- no clock, no
/// `HashMap` iteration; two runs of the same world produce the same lists.
pub fn compose(world: &World, data: &GameData, rules: &dyn CardRules) -> Vec<Vec<TileTextEntry>> {
    let n_tiles = data.tiles.len();
    let mut out: Vec<Vec<TileTextEntry>> = vec![Vec::new(); n_tiles];
    if n_tiles == 0 {
        return out;
    }

    // The base instance of each tile: the first `board_field` row whose
    // `card` is that tile's `tile_rule_id` and whose `tile` is that square.
    // An extra attached `tile:*` of the same id is a *replacement*.
    let mut base_uid = vec![None; n_tiles];
    for f in &world.st.board_field {
        let Ok(t) = usize::try_from(f.tile) else {
            continue;
        };
        let Some(td) = data.tiles.get(t) else {
            continue;
        };
        let want = tile_rule_id(&td.kind);
        if want.is_empty() || f.card != want {
            continue;
        }
        if base_uid[t].is_none() {
            base_uid[t] = Some(f.uid);
        }
    }

    let mut speakers: Vec<Speak> = Vec::new();
    // Player fields first (lookup group < Board), then the board owner --
    // the same walk `effect_order_key` orders.
    for (p, player) in world.st.players.iter().enumerate() {
        let Ok(p) = u32::try_from(p) else {
            continue;
        };
        for (n, f) in player.field.iter().enumerate() {
            let Ok(n) = u32::try_from(n) else {
                continue;
            };
            let group = if f.band_skill {
                LookupGroup::Band
            } else if f.card.starts_with("skill:") {
                LookupGroup::Character
            } else {
                LookupGroup::Field
            };
            push_speakers(
                world,
                data,
                rules,
                f,
                (group as u8, (p << 16) | n),
                &base_uid,
                false,
                &mut speakers,
            );
        }
    }
    for (n, f) in world.st.board_field.iter().enumerate() {
        let Ok(n) = u32::try_from(n) else {
            continue;
        };
        let is_base = usize::try_from(f.tile)
            .ok()
            .and_then(|t| base_uid.get(t).copied().flatten())
            .is_some_and(|u| u == f.uid);
        push_speakers(
            world,
            data,
            rules,
            f,
            (LookupGroup::Board as u8, n),
            &base_uid,
            is_base,
            &mut speakers,
        );
    }

    // Replacements first (by `effect_order_key`), base last: the default text
    // only wins an unclaimed slot.
    speakers.sort_by(|a, b| {
        a.base
            .cmp(&b.base)
            .then(a.key.0.cmp(&b.key.0))
            .then(a.key.1.cmp(&b.key.1))
            .then(a.key.2.cmp(&b.key.2))
    });
    for s in speakers {
        let Ok(t) = usize::try_from(s.tile) else {
            continue;
        };
        if t >= out.len() {
            continue;
        }
        out[t].push(TileTextEntry {
            from: s.from,
            slot: s.slot,
            key: s.key_text,
        });
    }
    out
}

/// Resolve one instance's declared texts into `speakers`.
fn push_speakers(
    world: &World,
    data: &GameData,
    rules: &dyn CardRules,
    f: &FieldCard,
    source: (u8, u32),
    _base_uid: &[Option<i32>],
    is_base: bool,
    speakers: &mut Vec<Speak>,
) {
    let decls = rules.card_texts(&f.card);
    if decls.is_empty() {
        return;
    }
    for (di, d) in decls.iter().enumerate() {
        let Ok(decl) = u32::try_from(di) else {
            continue;
        };
        let tile = resolve_tile(world, data, f, d);
        if tile < 0 {
            continue;
        }
        let Ok(t) = usize::try_from(tile) else {
            continue;
        };
        // `tile: ""` on the designated base instance is the base text; any
        // other speaker (a named-tile text, an extra `tile:*`, a player-field
        // card on the square) is a replacement.
        let base = is_base && d.tile.is_empty();
        speakers.push(Speak {
            key: (source.0, source.1, decl),
            tile: t as i32,
            from: f.card.clone(),
            slot: d.slot.clone(),
            key_text: d.key.clone(),
            base,
        });
    }
}

/// The board tile a declaration speaks for: `""` = the instance's own
/// `FieldCard::tile`, otherwise the board tile named in `data`.
fn resolve_tile(
    world: &World,
    data: &GameData,
    f: &FieldCard,
    d: &TileTextDecl,
) -> i32 {
    if d.tile.is_empty() {
        return f.tile;
    }
    world.tile_named(data, &d.tile)
}