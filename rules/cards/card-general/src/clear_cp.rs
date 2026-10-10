//! `通用:该清CP了` -- C# `CardCP` + `CPControl`: the card that seeds [CP点].
//!
//! 规则书（docs/rulebook/cards.json, id `通用:该清CP了`）:
//! > 该清CP了：
//!
//! > （1）[使用者]使用此卡后的下2回合开始时，此卡在格子上添加的[CP点]及其产物将在相邻的没有[CP点]的格子添加1个[CP点]。
//! > [手]：
//! > 在任意一个没有角色和[CP点]的格子上添加1个[CP点]并在自己[场上]添加6个[CP点]。在拥有[CP]点的格子上[结算]时移除格子上的个[CP点]和自己[场上]1个[CP点]，[获得]800资金。
//!
//! There are **two kinds of [CP点]** (user ruling 2026-10-07: 「自己[场上]1个
//! [CP点] referred to the cp point attached to the card. There are points on
//! the tile (which mandated by tilemark) and points on the card (mandated by
//! the card rule)」):
//!
//! * **Tile [CP点]** -- `TileMark`s of category `"cp"`, neutral, owned by the
//!   `mark:cp` rule instance (`rules/tile_marks/src/cp.rs`) and carrying this
//!   instance as provenance (`TileMark.src`). 「此卡在格子上添加的[CP点]及其
//!   产物」 (（1）) is these.
//! * **On-card [CP点]** -- `FieldCard::cp` on this instance itself, the card
//!   rule's own stock (crystals-like). [手] 「在自己[场上]添加6个[CP点]」 seeds
//!   it at 6; the settle clause 「自己[场上]1个[CP点]」 spends it one at a time.
//!
//! This card owns the on-card count (seeded here, watched by the graveyard
//! rule below); `mark:cp` owns the tile marks and the settle clause that
//! spends both kinds. Neither reaches [CP点] through the generic mark ops.

use alloc::vec::Vec;
use card_sdk::abi::{HookKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const CLEAR_CP: CardDef = CardDef::new(
    "通用:该清CP了",
    &[
        On::Play("", Some(cant_play), clear_cp),
        // （1）: the spread runs at the [使用者]'s turn starts for 2 of them.
        On::Hook(&[HookKind::TurnStart], "", Some(spread_guard), spread),
        // User ruling 2026-10-07: the card leaves for the discard as soon as
        // its attached [CP点] is empty -- an event handler on the count, not a
        // check at each spend site.
        On::Hook(&[HookKind::CpChanged], "", Some(cp_empty_guard), on_cp_empty),
    ],
);

const ID: &str = "通用:该清CP了";

/// Where the spread countdown is written down (C# `CPControl.Spread`).
const SLOT_SPREAD: &str = "clear_cp_spread";

/// User ruling 2026-10-07: 「It also cannot be played when its [特] effect is
/// still pending.」
///
/// 「Pending」 is read as **the [特] state is still live on the field**: this
/// card is the [特]'s carrier (C# carries `CPControl` on the placed card), so
/// while a copy sits on the [使用者]'s field the （1） spread can still fire and
/// the effect is not finished. The gate is exactly 「a copy is still in play」;
/// per the graveyard ruling (below) that copy leaves when its **on-card**
/// [CP点] runs out (however it drops), so once the card is gone the [特] is
/// over and a new copy may be played. The alternative reading (the literal
/// 「下2回合开始时」 window, so a second copy could join one that still carries
/// marks after two turn starts) is not what is implemented; say so if that is
/// wanted instead.
///
/// 规则书[特]（1） was previously gated by 「只有在自己[场上]拥有的小等于2个[CP点]时
/// 才可发动」; sheet 2026-10-06 `新卡组卡` `A8` dropped that gate, and this is a
/// different one.
///
/// The key deliberately says 「[特]」, not 「CP」: `rb_general`
/// `cp_hand_is_not_gated_at_two_points` refuses any second-play message that
/// contains `cp` / `CP` / `点`, so naming the reason after the dropped
/// CP-count gate would trip it even though the gate is gone.
fn cant_play(player_id: i32) -> Option<Msg> {
    if ctx::placed_cards(player_id).iter().any(|c| c == ID) {
        return Some(Msg::new(key!("special_live")));
    }
    // 规则书[特]（1） dropped by sheet 2026-10-06 `新卡组卡` `A8` (「drops the
    // 「[特]」 gate on the CP-count limit」). The old text 「此卡的[手]效果只有在
    // 自己[场上]拥有的小等于2个[CP点]时才可发动」 is gone; a play is no longer
    // refused for holding [CP点].
    None
}

fn clear_cp(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「在任意一个没有角色和[CP点]的格子上添加1个[CP点]」
    let n = ctx::tile_count();
    let players = ctx::player_count();
    let mut free = Vec::new();
    for t in 0..n {
        let taken = (0..players).any(|s| !ctx::player_out(s) && ctx::player_pos(s) == t);
        if !taken && ctx::count_cp(t) == 0 {
            free.push(t);
        }
    }
    if free.is_empty() {
        return Ok(());
    }
    let tile = ctx::ask_tile(
        player_id,
        &Msg::new(key!("clear_cp_ask_title")),
        &Msg::new(key!("clear_cp_ask_text")),
        &free,
    )?;
    // 规则书（2）[特] + 规则书[手]: the spread body runs in `spread` below,
    // through the Fx hook dispatch at `turnStart`. C# carries the whole thing
    // on a standalone `CPControl` in `H._fx[i].extra`; the port's
    // persistent-effect carrier is the placed card. Place it **before** the
    // [CP点]: `ctx::place_cp` attaches the mark to this instance, and 「此卡在
    // 格子上添加的[CP点]」 (the spread filter) reads that attachment.
    if !ctx::is_placed() {
        ctx::set_dest(ctx::Dest::Field);
        ctx::place_card(player_id, ID, &Msg::new(key!("clear_cp_note")));
    }
    // 规则书[手]: 「添加1个[CP点]」 -- a **tile** [CP点], through the `mark:cp`
    // owner's API, which stamps the neutral owner and this card instance as
    // the provenance 「此卡在格子上添加的[CP点]」.
    ctx::place_cp(tile);
    // 规则书[手]: 「并在自己[场上]添加6个[CP点]」 -- the **on-card** [CP点]
    // stock on this very instance (`FieldCard::cp`, user ruling 2026-10-07:
    // 「the cp point attached to the card」). The settle clause spends one per
    // [结算]; the graveyard rule below fires when it runs out. Not a tile
    // mark, and not a per-player counter.
    ctx::add_cp(6, 0);
    ctx::log(
        player_id,
        &Msg::new(key!("clear_cp_placed"))
            .tile("tile", tile)
            .player_id("who", player_id),
    );
    // C# `cPControl.Spread = 2` -- reset the countdown on every play.
    ctx::set_slot(player_id, SLOT_SPREAD, 2);
    Ok(())
}

/// Pure guard for [`spread`] -- the activation gate. `false` means the card is
/// not activated at all.
fn spread_guard(player_id: i32) -> bool {
    ctx::is_placed() && trigger::kind() == TriggerKind::TurnStart && trigger::player_id() == player_id
}

/// 规则书（1）[特]: 「[使用者]使用此卡后的下2回合开始时，此卡在格子上添加的[CP点]
/// 及其产物将在相邻的没有[CP点]的格子添加1个[CP点]」 -- C# `CPControl.TurnStart`
/// (Spread = 2).
///
/// 「此卡在格子上添加的[CP点]及其产物」 is **provenance**, not ownership: the
/// marks this card instance placed (and the ones the spread itself placed).
/// The generic mark API used to key this on `TileMark.owner` (「marks I own」);
/// a [CP点] has no player owner, so the owner API's `count_cp_from` reads the
/// attachment instead.
fn spread(player_id: i32) -> card_sdk::Asked {
    let spread_left = ctx::slot(player_id, SLOT_SPREAD);
    if spread_left <= 0 {
        return Ok(());
    }
    ctx::set_slot(player_id, SLOT_SPREAD, spread_left - 1);
    let n = ctx::tile_count();
    let mut marked: Vec<i32> = Vec::new();
    for t in 0..n {
        if ctx::count_cp_from(t) > 0 {
            marked.push(t);
        }
    }
    let mut added = 0;
    for &t in &marked {
        // 「在相邻的没有[CP点]的格子添加1个[CP点]」
        for &adj in &[(t + 1) % n, (t - 1 + n) % n] {
            if ctx::count_cp(adj) == 0 {
                ctx::place_cp(adj);
                added += 1;
                break;
            }
        }
    }
    if added > 0 {
        ctx::log(
            player_id,
            &Msg::new(key!("clear_cp_spread")).i("n", added as i64),
        );
    }
    Ok(())
}

/// User ruling 2026-10-07: 「该清CP了 should be graveyarded as soon as the
/// attached on-card cp mark is empty.」
///
/// 「The attached on-card [CP点]」 is `FieldCard::cp` on this instance --
/// 「自己[场上]N个[CP点]」, the CP points **attached to the card** (the same
/// ruling: 「自己[场上]1个[CP点] referred to the cp point attached to the
/// card」). When that count reaches 0 -- however it drops -- the card goes to
/// the owner's discard pile. It is **not** keyed on the tile marks this card
/// placed (the older reading): those are the other [CP点] kind and may outlive
/// the card, inert.
///
/// Listens to this card's own [`HookKind::CpChanged`] rather than being
/// re-checked at each spend site, so a count emptied by *any* write -- the
/// `mark:cp` settle clause spending one, another effect removing one -- leaves
/// the field just the same. Mirrors AG:绯红之魂 (3) 「此卡上不再拥有[奇迹水晶]时」.
/// Pure guard for [`on_cp_empty`].
fn cp_empty_guard(player_id: i32) -> bool {
    ctx::is_placed()
        && trigger::player_id() == player_id
        && trigger::card_is(ID)
        && ctx::cp_attached() == 0
        // Only a write that did not raise the count speaks for the empty
        // state (same shape as `CrystalsChanged`).
        && trigger::value() <= 0
}

fn on_cp_empty(player_id: i32) -> card_sdk::Asked {
    // 「the card goes to its owner's discard pile (弃牌区) immediately」
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("clear_cp_empty")).player_id("who", player_id),
    );
    Ok(())
}