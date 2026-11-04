//!
//! 规则书（docs/rulebook/cards.json, id `HHW:黑衣人的补给`）:
//! > 黑衣人的补给：
//! > [反击] 经过“CiRCLE”格子（#1）时可打出此卡，在“弦卷集团”（#29格）格子上放置一个奇迹水晶，该格上拥有奇迹水晶时，该格获得“CiRCLE”格子的全部效果。你经过“弦卷集团”格子后，移除那格的一个奇迹水晶。
//!
//! CiRCLE, drop a crystal on 弦卷集团 so that tile borrows CiRCLE's effect.
//!
//! `SETTLE-STAGES.md` §4 M3 / `docs/TILES.md`: 「该格获得"CiRCLE"格子的全部
//! 效果」 is the tile **gaining CiRCLE's effect-list entry** -- a card attaching
//! a `tile:circle` rule instance to 弦卷集团, additive alongside its own
//! `tile:agent` body. It is not an after-hook: a body replace must not leave a
//! borrowed draw running. The instance lives exactly while the crystal does.

use bandori_std::ctx::{self, trigger};
use bandori_std::zones::ZoneChangeCause;
use card_sdk::abi::{ActionKind, ChainKind, MarkFilter};
use card_sdk::CardText;
use card_sdk::{key, Action, CardDef, Msg, On};

/// It is a locale key so the placement line reads as a 奇迹水晶, not as the card's
/// own name.
const MARK: &str = key!("black_suits_crystal");

/// One-shot claim for the pass body: the raise uid that already spent this
/// copy's crystal. A fixed-key seat latch holding the last raise uid: one
/// key, overwritten in place, shared by every copy so two stand-ins answer
/// one pass once; `trigger::uid()` is nonzero inside a raise.
const CLAIM: &str = "black.claimed";

pub const BLACK_SUITS: CardDef = CardDef::new(
    "HHW:黑衣人的补给",
    &[
        // 规则书: 「你经过“弦卷集团”格子后，移除那格的一个奇迹水晶」 -- the tile
        // and the owner's own pass are the condition; the per-raise claim in the
        // body does the dedupe. `ActionKind::Pass` (not `PassTile`) matches the
        // misaki 「经过弦卷集团」 shape (`skill-personas/src/misaki.rs`).
        On::Action(Action::Pass("card.placed && actor == owner && tile.id == tile_named('弦卷集团')", None, pass_tile)),
        // 规则书: 「该格上拥有奇迹水晶时，该格获得“CiRCLE”格子的全部效果」 -- an
        // external clear (another card, a bounce, a bankrupt owner's purge) must
        // resync too. The guard admits only a raise about *this* stand-in.
        // TODO(规则书)（你经过“弦卷集团”格子后，移除那格的一个奇迹水晶）: this
        //   retires the same-site `sync_circle_instance` calls below once the
        //   hook fire is trusted; they stay as the belt-and-braces path.
        On::Action(Action::MarkChanged(
            "tile.id == tile_named('弦卷集团')",
            Some(changed_is_my_crystal),
            resync,
        )),
        // 规则书[反击]: 「经过“CiRCLE”格子（#1）时可打出此卡」 --
        // `t.Kind == "pass" && t.Seat == seat && Tile(t.Tile)?.kind == "circle"`.
        // `PassBefore` is the category; the owner's own pass of CiRCLE is the
        // condition. The `circle < 0` defensive check is dropped (`tile_named`
        // missing = -1 never equals a live `tile.id`).
        On::Counteract(
            &[ChainKind::PassBefore],
            "actor == owner && tile.id == tile_named('CiRCLE')",
            None,
            counteract).counter_reason(key!("black_suits_when")),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"黑衣人的补给"#)
        .raw_name(r#"黑衣人的补给"#)
        .tags(&[r#"反击"#])
        .text(CardText::para(r#"[反击] 经过“CiRCLE”格子（#1）时可打出此卡，在“弦卷集团”（#29格）格子上放置一个奇迹水晶，该格上拥有奇迹水晶时，该格获得“CiRCLE”格子的全部效果。你经过“弦卷集团”格子后，移除那格的一个奇迹水晶。"#))
        // sheet cell F8
        .fields(&[("group", r#"Hello, Happy World!"#), ("owner", "")]));

fn counteract(player_id: i32) -> card_sdk::Asked {
    let group = ctx::tile_named("弦卷集团");
    // `place_mark_new`: one row per crystal.
    if group >= 0 {
        // 「你经过“弦卷集团”格子后，移除那格的一个奇迹水晶」 -- field hooks only
        // dispatch to placed instances, and a [反击] declaration would otherwise
        // be discarded, leaving the `Pass` hook below dead. Place this card
        // as the stand-in for the effect (same pattern as `HHW:育美标记`),
        // *before* the mark, so the row is bound to the stand-in
        // (`place_mark` stamps `instance = ctx::self_uid()`) and the
        // `MarkChanged` guard can name it.
        if !ctx::is_placed() {
            ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Effect);
            ctx::place_card(
                player_id,
                "HHW:黑衣人的补给",
                &Msg::new(key!("black_suits_attach")),
            );
        }
        let _row = ctx::place_mark(
            group,
            MARK,
            "",
            player_id,
            ctx::self_uid(),
            1,
            &Msg::new(key!("black_suits_crystal_note")),
            card_sdk::abi::Stack::Fresh,
        );
        ctx::log(
            player_id,
            &Msg::new(key!("black_suits_placed"))
                .player_id("who", player_id)
                .tile("tile", group),
        );
        // 「该格上拥有奇迹水晶时，该格获得“CiRCLE”格子的全部效果」 --
        // `docs/TILES.md` / `SETTLE-STAGES.md` §4 M3: attach a `tile:circle`
        // rule instance to 弦卷集团, additive next to its `tile:agent` body.
        // The instance's own `On::Settle` (landing draw) and Pass entry (the
        // [经过] reward) are what 「全部效果」 means; no card hook draws.
        sync_circle_instance(group);
    }
    Ok(())
}

/// Does this raise concern this stand-in's own crystal? The subject instance is
/// the stand-in itself; for a destroyed instance `card.placed` is already
/// false, so the guard does not check it.
fn changed_is_my_crystal(_player_id: i32) -> bool {
    trigger::name() == MARK && trigger::instance() == ctx::self_uid()
}

/// Re-evaluate the borrowed `tile:circle` for the mark's tile.
pub fn resync(_player_id: i32) -> card_sdk::Asked {
    sync_circle_instance(trigger::tile());
    Ok(())
}

/// Crystals this player parked on `tile` (mark owner = the card's owner).
fn own_crystals(tile: i32, owner: i32) -> i32 {
    ctx::count_marks(tile, &MarkFilter::any().kind(MARK).owner(owner))
}

/// Attach or drop the borrowed `tile:circle` instance so it exists exactly
/// while 弦卷集团 carries a crystal.
fn sync_circle_instance(group: i32) {
    // The book keys the borrow on the crystal's presence. The `MarkChanged`
    // entry above (and the `counterChanged` path of a purge) is what keeps this
    // true outside this card's own bodies; the direct calls here stay too.
    let has = ctx::count_marks(group, &MarkFilter::any().kind(MARK)) > 0;
    // `place_card_on(BOARD_OWNER, tile, …)` is the "attach a rule to a tile"
    // gesture (`game_core::state::BOARD_OWNER`, `docs/TILES.md`). The board
    // field is `field_instances(-1)`; a borrowed `tile:circle` on *this* tile
    // is one whose `tile_at` is the group (the real CiRCLE's own instance
    // governs the CiRCLE square and is left alone).
    let borrowed = ctx::field_instances(-1)
        .into_iter()
        .find(|(uid, id)| id == "tile:circle" && ctx::tile_at(*uid) == Some(group))
        .map(|(uid, _)| uid);
    if has && borrowed.is_none() {
        // 规则书: 「该格获得"CiRCLE"格子的全部效果」 -- the borrowed instance is
        // this stand-in's child, so it drops with the card.
        let uid = ctx::place_card_on(
            -1,
            group,
            "tile:circle",
            &Msg::new(key!("black_suits_attach")),
        );
        if uid >= 0 {
            ctx::set_created_by_at(uid, ctx::self_uid());
        }
    } else if !has {
        if let Some(uid) = borrowed {
            // The glow is gone before the stand-in leaves (an external clear):
            // drop the child now, from this body (not a creator cascade).
            ctx::unplace_at(uid, ctx::Dest::Graveyard, ZoneChangeCause::Effect);
        }
    }
}

/// 「你经过“弦卷集团”格子后，移除那格的一个奇迹水晶」 -- the pass-tile effect's
/// `mark.count--`, which is `bump_mark`'s single-tick form. Dropping the last
/// crystal takes the borrowed `tile:circle` instance with it.
///
/// One removal per raise: a pass dispatches one job per placed copy, so each
/// job first claims the raise on its own prop. Every copy still files itself
/// (the trailing `own_crystals == 0` check runs outside the claim), so the
/// other copy leaves too.
fn pass_tile(player_id: i32) -> card_sdk::Asked {
    // `actor == owner && tile.id == tile_named('弦卷集团')` is the condition.
    let t = trigger::tile();
    if ctx::state::get(player_id, CLAIM) != trigger::uid() {
        ctx::state::set(player_id, CLAIM, trigger::uid());
        // The owner's own crystal is the one spent (`owner(player_id)`), so two
        // players' stand-ins never eat each other's crystals.
        ctx::bump_mark(t, &MarkFilter::any().kind(MARK).owner(player_id), -1);
    }
    sync_circle_instance(t);
    // Stand-in cleanup: nothing left to remove, so file the card away.
    if own_crystals(t, player_id) == 0 {
        ctx::set_dest(ctx::Dest::Graveyard, card_sdk::abi::ZoneChangeCause::Effect);
    }
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn pass_tile_kani(player_id: i32) -> card_sdk::Asked {
    pass_tile(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn resync_kani(player_id: i32) -> card_sdk::Asked {
    resync(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}
