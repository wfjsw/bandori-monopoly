//! `mark:cp` -- the [CP点] tile-mark **owner**.
//!
//! 规则书（`data/rules.txt` 125, 「标志物解释」）:
//! > · CP点：放置于路面上的指示物
//!
//! and the one card that speaks of them (`docs/rulebook/cards.json`,
//! id `通用:该清CP了`; not a `data/rules.txt` passage, so quoted inline on the
//! clauses that use it rather than as a `//! >` block):
//! [手] 「在任意一个没有角色和[CP点]的格子上添加1个[CP点]并在自己[场上]添加6个
//! [CP点]。在拥有[CP]点的格子上[结算]时移除格子上的个[CP点]和自己[场上]1个[CP点]，
//! [获得]800资金。」
//!
//! There are **two kinds of [CP点]** (user ruling 2026-10-07: 「自己[场上]1个
//! [CP点] referred to the cp point attached to the card. There are points on
//! the tile (which mandated by tilemark) and points on the card (mandated by
//! the card rule)」):
//!
//! * **Tile [CP点]** -- the `TileMark`s of category [`card_sdk::abi::mark::
//!   CP_CATEGORY`], owned by this rule instance. That is what `data/rules.txt`
//!   125 defines (「放置于路面上的指示物」) and what the board sees.
//! * **On-card [CP点]** -- `FieldCard::cp` on the 该清CP了 card instance, the
//!   card rule's own stock (`rules/cards/card-general/src/clear_cp.rs`). This
//!   rule only *spends* it (the settle clause) and never owns it.
//!
//! This board-owned rule instance owns the **tile-mark** lifecycle, so no card
//! has to:
//!
//! * **Placement** -- 「在任意一个没有角色和[CP点]的格子上添加1个[CP点]」: a
//!   [CP点] goes on a tile with no character on it and no [CP点] yet. The
//!   *where* is the placer's gate (it knows its own targets); the *what* is
//!   `ctx::place_cp`, which stamps the category and the attachment.
//! * **Stacking** -- 「添加1个[CP点]」 per placement, and only onto a 「没有
//!   [CP点]的格子」, so a tile carries one [CP点] mark whose `count` is how many
//!   [CP点] sit there (in practice 1).
//! * **Landing** -- 「在拥有[CP]点的格子上[结算]时移除格子上的个[CP点]和自己
//!   [场上]1个[CP点]，[获得]800资金」, the `On::Hook` below. It spends **both**
//!   kinds: the tile's mark and one on-card [CP点] of the card the mark is
//!   attached to (`TileMark.src`).
//! * **Removal** -- 「移除格子上的个[CP点]」, `ctx::clear_cp` (one per [结算];
//!   C# `tileMark.count--`).
//!
//! A [CP点] mark is **not owned by any player**: `TileMark.owner` is always
//! [`BOARD_OWNER`] (`-1`). Provenance is `TileMark.src` (the placing card
//! instance) plus `TileMark.card` (its id, for the view's 「来自」) -- never an
//! owner. 通用:该清CP了 (1) 「此卡在格子上添加的[CP点]及其产物」 keys on that
//! provenance.
//!
//! ## The settle clause's reading (recorded)
//!
//! 「在拥有[CP]点的格子上[结算]时移除格子上的个[CP点]和自己[场上]1个[CP点]，
//! [获得]800资金」
//!
//! * **Trigger.** Any player's [结算] on a tile that actually carries a
//!   [CP点]. The bare 「[结算]时」 is board-generic -- it does not say 「自己
//!   [结算]时」 -- so this instance's `SettleBody` entry hears every settle and
//!   checks the tile. 行动阶段 15 (`SETTLE-STAGES.md` §4 M2): 「[结算]时」 is
//!   an entry in the settle's effect list, not the 「[触发结算]后」 window.
//! * **「格子上的个[CP点]」** is the tile mark (the sheet dropped the numeral;
//!   read as 「1个」, one per [结算], matching C# `tileMark.count--`).
//! * **「自己[场上]1个[CP点]」** is the **on-card** [CP点] of the 该清CP了 card
//!   the tile mark is attached to -- the mark's `src` -- per user ruling
//!   2026-10-07. That card must have ≥1; if it is gone or empty the clause
//!   does not fire. (C# instead kept a per-player `Tok(Seat, "CP点")` and
//!   gated the whole clause on `m.Seat == Seat`; the ruling replaces the
//!   counter with the card's own count and this reading drops the gate.)
//! * **「[获得]800资金」** goes to **the settling player** -- the subject of
//!   「在…[结算]时」 carries over to 「移除…，[获得]…」, and the rulebook tip
//!   （rulebook.txt 2067: 「短时间内吃多个CP点达到2000以上收益」） has the eater
//!   (whoever lands on the CP tile) profit 800 a bite. The house style names
//!   「你」/「[使用者]」 when the card's user is the subject; the bare 「[获得]」
//!   here takes the settler. (C# paid `Seat`, but its `m.Seat == Seat` gate
//!   made settler and card controller the same player.)

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{CardDef, Msg, On};

/// The board-owned [CP点] rule. One instance on [`BOARD_OWNER`], governing no
/// single tile (`tile = -1`): [CP点] is a board-wide category, not a tile kind.
/// Registered next to the `tile:*` rules in [`crate::CARDS`]; `bind_tiles`
/// places it (see `docs/TILES.md`).
pub const MARK_CP: CardDef = CardDef::new("mark:cp", &[On::Hook(&[HookKind::SettleBody], "", Some(lands_on_cp), on_land)]);

/// 规则书: 「在拥有[CP]点的格子上[结算]时」 -- the tile must actually carry a
/// [CP点], and 「自己[场上]1个[CP点]」 -- the on-card [CP点] of the card the
/// mark is attached to (`TileMark.src`) -- must be there to remove (a [结算]
/// with no on-card [CP点] to spend does not pay out). Pure guard for
/// [`on_land`].
///
/// The hook's `player_id` is this instance's owner, which for a board-owned
/// rule is [`BOARD_OWNER`] (`-1`) -- the settler is `trigger::player_id()`, the
/// same shape as `tile:circle`'s Pass entry.
fn lands_on_cp(_owner: i32) -> bool {
    // A field card that replaced the settle body (「将该次结算改为…」) skips
    // this entry like every other (`SETTLE-STAGES.md` §4 M2).
    if trigger::cancelled() {
        return false;
    }
    let seat = trigger::player_id();
    let tile = trigger::tile();
    if seat < 0 || tile < 0 || ctx::count_cp(tile) <= 0 {
        return false;
    }
    // 「自己[场上]1个[CP点]」 -- the card this tile's mark is attached to.
    let src = ctx::cp_src_at(tile);
    src >= 0 && ctx::cp_at(src) > 0
}

/// 规则书: 「移除格子上的个[CP点]和自己[场上]1个[CP点]，[获得]800资金」 --
/// C# `CPControl.SettleAfter` -> `Clean`. The tile-mark write goes through the
/// owner API; the on-card write goes to the mark's `src` card and raises
/// `cpChanged` against it (the 该清CP了 graveyard rule hears about it like any
/// other write). The 800 goes to the settler -- see the reading above.
fn on_land(_owner: i32) -> card_sdk::Asked {
    let seat = trigger::player_id();
    let tile = trigger::tile();
    if seat < 0 || tile < 0 {
        return Ok(());
    }
    // Read the attachment **before** the write: dropping the last mark takes
    // its provenance with it.
    let src = ctx::cp_src_at(tile);
    // 「移除格子上的个[CP点]」 -- C# `tileMark.count--`, one per [结算]. A
    // tile-mark write: it does not touch the on-card count.
    ctx::clear_cp(tile);
    // 「自己[场上]1个[CP点]」 -- one on-card [CP点] of the attached 该清CP了
    // card (user ruling 2026-10-07).
    if src >= 0 {
        ctx::add_cp_at(src, -1, 0);
    }
    // 「[获得]800资金」 -- the settling player (see the reading above). C# was
    // `H.GainR(Seat, Reward, …)`, `Reward = c.N(0, 800)`.
    ctx::gain(
        seat,
        800,
        &Msg::new("log.cp_clean").player_id("who", seat),
    )?;
    Ok(())
}