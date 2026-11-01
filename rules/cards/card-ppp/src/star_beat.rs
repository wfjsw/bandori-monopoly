//!
//! 规则书（docs/rulebook/cards.json, id `PPP:STAR BEAT!`）:
//! > STAR BEAT!：
//! > [手]：[使用者]获得1层状态“下次结算后可选择在绝对距离5格以内自己拥有的格子上进行一次盖房，随后减少1层”，然后进入移动阶段并选择以下操作之一：
//! > 1. 获得2个星星贴纸，本回合的主要移动改为[传送]到(45×“资金数包含5的玩家数量+1”) mod 60格并结算；
//! > 2. 本回合的主要移动改为移动(2×“资金数包含5的玩家数量+1”)d10格并结算。
//!
//! layer, then either take 2 star stickers and teleport, or roll a pile of d10s.

use card_sdk::abi::MoveKind;
use card_sdk::{ctx, key, Action, CardDef, Msg, On};
use card_sdk::{para, sec, steps, CardText};

pub const STAR_BEAT: CardDef = CardDef::new(
    "PPP:STAR BEAT!",
    &[
        // 规则书: 「下次结算后可选择在绝对距离5格以内自己拥有的格子上进行一次盖房」
        // -- the offer exists only with a layer banked and a tile passing
        // `can_build_on` within 5 (affordability is re-checked inside
        // `card_offer_build`), dispatched only while the card rides the field as
        // its carrier (see `play`). The layer is CEL (`slot`); the near-buildable
        // scan is residual (owned ∩ dist ≤ 5 ∩ can_build_on). The slot is per
        // player, so only the first placed copy dispatches it (see
        // `settle_offer_applies`).
        On::Action(Action::SettleAfter("actor == owner && slot('star_beat.layers') > 0", Some(settle_offer_applies), after_settle)),
        On::Play("", Some(cant_play), play),
    ],
)
    .props(&[(card_sdk::abi::prop::SONG, 1)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"STAR BEAT!"#)
        .raw_name(r#"STAR BEAT!"#)
        .tags(&[r#"手"#])
        .text(CardText::of(&[
            sec("", &[
                para(r#"[手]：[使用者]获得1层状态“下次结算后可选择在绝对距离5格以内自己拥有的格子上进行一次盖房，随后减少1层”，然后进入移动阶段并选择以下操作之一："#),
                steps(&[
                    r#"获得2个星星贴纸，本回合的主要移动改为[传送]到(45×“资金数包含5的玩家数量+1”) mod 60格并结算；"#,
                    r#"本回合的主要移动改为移动(2×“资金数包含5的玩家数量+1”)d10格并结算。"#,
                ]),
            ]),
        ]))
        // sheet cell B8
        .fields(&[("group", r#"Poppin' Party"#), ("owner", "")]));

/// 规则书: 「本回合的主要移动改为」 -- both branches replace the main move, so the card is only
/// playable while it is still available.
fn cant_play(player_id: i32) -> Option<Msg> {
    rules_status::move_blocked(player_id)
}

fn money_has_digit5(player_id: i32) -> bool {
    let mut x = ctx::money_of(player_id).unsigned_abs();
    if x == 0 {
        return false;
    }
    // Kani-only: a u32 has at most 10 decimal digits, so cap the scan to give
    // CBMC a loop bound. Production keeps the plain `while x > 0`.
    #[cfg(kani)]
    let mut i = 0u32;
    #[cfg(kani)]
    while i < 10 && x > 0 {
        if x % 10 == 5 {
            return true;
        }
        x /= 10;
        i += 1;
    }
    #[cfg(not(kani))]
    while x > 0 {
        if x % 10 == 5 {
            return true;
        }
        x /= 10;
    }
    false
}

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「[使用者]获得1层状态…随后减少1层」
    ctx::inc_slot(player_id, "star_beat.layers", 1);
    // 规则书: 「下次结算后可选择在绝对距离5格以内自己拥有的格子上进行一次盖房」 --
    // the card must ride the field as the offer's carrier.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_self_to_field(
        player_id,
        "PPP:STAR BEAT!",
        &Msg::new(key!("star_beat_title")),
    );
    // 规则书: 「(45×“资金数包含5的玩家数量+1”) mod 60格」 / 「(2×“资金数包含5的玩家数量+1”)d10」
    let n = (0..ctx::player_count())
        .filter(|&p| !ctx::player_out(p) && money_has_digit5(p))
        .count() as i32;
    let tiles = ctx::tile_count();
    if tiles <= 0 {
        return Ok(());
    }
    let to = (45 * (n + 1)).rem_euclid(tiles);
    let dice = 2 * (n + 1);
    let pick = ctx::ask_choice(
        player_id,
        &Msg::new(key!("star_beat_title")),
        &Msg::new(key!("star_beat_ask")).i("n", n as i64),
        &[
            Msg::new(key!("star_beat_opt_teleport")).tile("tile", to),
            Msg::new(key!("star_beat_opt_move")).i("n", dice as i64),
        ],
    )?;
    if pick == 0 {
        ctx::add_held_by_name(player_id, "星星贴纸", 2, i32::MAX)?;
        ctx::log(
            player_id,
            &Msg::new(key!("star_beat_stickers"))
                .player_id("who", player_id)
                .i("n", 2),
        );
        // The plan names the destination and settles, and `run_planned_move` runs it as the
        // main move.
        if to >= 0 {
            ctx::plan::set_kind(MoveKind::Teleport);
            ctx::plan::set_teleport_to(to);
            ctx::plan::set_settles(true);
            ctx::log(
                player_id,
                &Msg::new(key!("star_beat_moved"))
                    .player_id("who", player_id)
                    .tile("tile", to),
            );
            ctx::run_planned_move(player_id);
        }
    } else {
        // `run_planned_move` rolls the dice and walks; no separate `ctx::roll`.
        ctx::plan::set_base_dice(dice, 10, "（STAR BEAT!）");
        ctx::plan::set_settles(true);
        ctx::run_planned_move(player_id);
    }
    Ok(())
}

/// Owned tiles within 5 -- 「绝对距离5格以内自己拥有的格子」 (the target set).
fn near_owned(player_id: i32) -> alloc::vec::Vec<i32> {
    let pos = ctx::player_pos(player_id);
    ctx::owned_tiles(player_id)
        .into_iter()
        .filter(|&t| ctx::dist(pos, t) <= 5)
        .collect()
}

/// [`near_owned`] narrowed to what can actually be built now: `can_build_on`
/// does not test affordability, so the offer checks the cost too. A tile left
/// out this way must not burn the layer.
fn near_buildable(player_id: i32) -> alloc::vec::Vec<i32> {
    let money = ctx::money_of(player_id);
    near_owned(player_id)
        .into_iter()
        .filter(|&t| ctx::can_build_on(player_id, t) && ctx::build_cost(t) <= money)
        .collect()
}

/// Residual guard for [`after_settle`]: at least one such tile exists, and this
/// copy carries the offer. The layer slot is per player, so a single placed
/// copy dispatches it -- the first face-up one in placement order; two copies
/// would otherwise offer twice per settle and burn two layers.
fn settle_offer_applies(player_id: i32) -> bool {
    let me = ctx::self_uid();
    let carrier = ctx::field_instances(player_id)
        .into_iter()
        // A face-down card runs no hook, so it is no carrier either.
        .find(|(uid, id)| *id == "PPP:STAR BEAT!" && !ctx::is_face_down_at(*uid))
        .is_some_and(|(uid, _)| uid == me);
    carrier && !near_buildable(player_id).is_empty()
}

/// 规则书: 「下次结算后可选择在绝对距离5格以内自己拥有的格子上进行一次盖房，随后减少1层」.
/// Applicability is the entry's condition + `settle_offer_applies`.
fn after_settle(player_id: i32) -> card_sdk::Asked {
    let pool = near_buildable(player_id);
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("star_beat_title")),
        &Msg::new(key!("star_beat_build")),
    )? {
        return Ok(());
    }
    // 规则书: 「可选择在绝对距离5格以内自己拥有的格子上进行一次盖房，随后减少1层」 --
    // the offer runs its own tile pick over the pool.
    ctx::card_offer_build(player_id, &pool);
    ctx::inc_slot(player_id, "star_beat.layers", -1);
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn after_settle_kani(player_id: i32) -> card_sdk::Asked {
    after_settle(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}
