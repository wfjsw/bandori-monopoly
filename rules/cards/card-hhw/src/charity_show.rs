//!
//! 规则书（docs/rulebook/cards.json, id `HHW:爱心义演`）:
//! > 爱心义演：
//! > 打出此卡的回合内，你若进行掷骰移动，每初次经过一个属于你的格子，使你的总移动数+2，本回合中向其他玩家支付时你的付款减半（向上取整10）（若弦卷心已将专属卡置于CiRCLE上，则CiRCLE也算作属于弦卷心的格子）
//!
//! +2 steps the first time you pass each of your tiles, and half pay to others.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ActionKind, FilterKind};
use card_sdk::CardText;
use card_sdk::{key, Action, CardDef, Filter, Msg, On};

const ID: &str = "HHW:爱心义演";
const KOKORO_ID: &str = "HHW:（kkr）前往笑容集结的地方！";

/// Stored on the card instance (`FieldCard::props`), not in the player's
/// keyed-state map, so the write stays inside this rule's scope.
const PROP_TURN: &str = "charity.turn";
// The per-tile first-pass marker is a dynamic instance prop, `charity.seen.<tile>`,
// built inline; it has no const of its own.

pub const CHARITY_SHOW: CardDef = CardDef::new(
    "HHW:爱心义演",
    &[
        On::Play("", None, play),
        // 规则书: 「本回合中向其他玩家支付时你的付款减半（向上取整10）」 -- only payments to
        // other players, only on the armed turn (`charity.turn`). The turn-key
        // latch is an instance prop (residual).
        On::Filter(Filter::PaySplitMul("card.placed && actor == owner && target >= 0 && target != owner && value > 0", Some(pay_applies), pay_mul)),
        // 规则书: 「你若进行掷骰移动，每初次经过一个属于你的格子，使你的总移动数+2」
        // -- a dice (main) walk step onto a tile that counts as yours, first
        // time this turn. `mine` / the first-pass marker are residual.
        // 规则书: 「你若进行掷骰移动」 -- `move.rolled` is a dice move, forced or not.
        On::Action(Action::PassTile("card.placed && actor == owner && move.kind != Teleport && move.rolled && tile.id >= 0", Some(pass_applies), pass_tile)),
        // 规则书: 「打出此卡的回合内」 -- both effects end with the turn.
        On::Action(Action::TurnEndAfter("card.placed && actor == owner", None, turn_end_after)),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"爱心义演"#)
        .raw_name(r#"爱心义演"#)
        .tags(&[])
        .text(CardText::para(r#"打出此卡的回合内，你若进行掷骰移动，每初次经过一个属于你的格子，使你的总移动数+2，本回合中向其他玩家支付时你的付款减半（向上取整10）（若弦卷心已将专属卡置于CiRCLE上，则CiRCLE也算作属于弦卷心的格子）"#))
        // sheet cell F12
        .fields(&[("group", r#"Hello, Happy World!"#), ("owner", "")]));

fn play(player_id: i32) -> card_sdk::Asked {
    // The hook dispatch only runs on placed cards, so this placement stands in for
    // the player attachment (same pattern as `card-sumimi`'s 儿时玩伴的鼓励); the
    // `TurnEndAfter` hook below files it away at the owner's turn end.
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(player_id, ID, &Msg::new(key!("charity_show_note")));
    // Instance props (not player state): scoped to this placed card.
    ctx::set_prop(PROP_TURN, ctx::turn_key());
    ctx::log(
        player_id,
        &Msg::new(key!("charity_show_played")).player_id("who", player_id),
    );
    Ok(())
}

/// Your own land always is; CiRCLE counts when your `CardKokoroCircle` sits on it
/// (rule: 「若弦卷心已将专属卡置于CiRCLE上，则 CiRCLE也算作属于弦卷心的格子」).
fn mine(player_id: i32, t: i32) -> bool {
    if ctx::tile_owner(t) == player_id {
        return true;
    }
    // `find_card` finds that player's Kokoro card instance and `tile_at` reads the
    // tile it sits on; an unbound placement (tile `-1`) counts as on CiRCLE -- the
    // only circle tile the rule names.
    if ctx::is_start(t) {
        return match ctx::find_card(player_id, KOKORO_ID).and_then(ctx::tile_at) {
            Some(pt) => pt == t || pt < 0,
            None => false,
        };
    }
    false
}

/// Residual guard for [`pay_mul`]: the turn-key latch (`charity.turn` is the
/// turn the effects were armed for).
fn pay_applies(player_id: i32) -> bool {
    ctx::prop(PROP_TURN) == ctx::turn_key()
}

/// `target >= 0 && target != owner && value > 0` is the condition; [`pay_applies`] is
/// the residual.
fn pay_mul(player_id: i32, value: i32) -> i32 {
    // `CeilTo(amount / 2.0, 10)` -- half, rounded up to a multiple of 10.
    let half = (value as i64 + 1) / 2;
    let half = ((half + 9) / 10) * 10;
    let half = half.clamp(0, i32::MAX as i64) as i32;
    ctx::log(
        player_id,
        &Msg::new(key!("charity_show_pay"))
            .player_id("who", player_id)
            .n("from", value as i64)
            .n("to", half as i64),
    );
    half
}

/// Residual guard for [`pass_tile`]: the turn-key latch, the tile counts as
/// yours, and it has not been seen this turn. `mine` is a derived lookup
/// (tile owner + the Kokoro CiRCLE claim); the first-pass marker is a
/// dynamic key (docs/GUARDS.md §4.2).
fn pass_applies(player_id: i32) -> bool {
    if ctx::prop(PROP_TURN) != ctx::turn_key() {
        return false;
    }
    let t = trigger::tile();
    if t < 0 || !mine(player_id, t) {
        return false;
    }
    let key = alloc::format!("charity.seen.{t}");
    // TODO(规则书)（每初次经过一个属于你的格子）: the spec reads 「初次」 once per
    //   move, but the marker stores the turn key -- a tile passed earlier in
    //   the same turn never counts again.
    ctx::prop(&key) != ctx::turn_key()
}

/// Applicability is the `condition` + [`pass_applies`].
fn pass_tile(player_id: i32) -> card_sdk::Asked {
    let t = trigger::tile();
    let key = alloc::format!("charity.seen.{t}");
    ctx::set_prop(&key, ctx::turn_key());
    ctx::log(
        player_id,
        &Msg::new(key!("charity_show_pass"))
            .player_id("who", player_id)
            .tile("tile", t),
    );
    // 规则书: 「使你的总移动数+2」 -- the walk re-reads this relative field every
    // step, so the +2 lands on the walk it was earned on.
    ctx::plan::add_extra_steps(2);
    Ok(())
}

fn turn_end_after(player_id: i32) -> card_sdk::Asked {
    ctx::set_prop(PROP_TURN, 0);
    ctx::set_dest(ctx::Dest::Graveyard, card_sdk::abi::ZoneChangeCause::Effect);
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn pay_mul_kani(player_id: i32, value: i32) -> i32 {
    pay_mul(player_id, value)
}

#[cfg(kani)]
#[inline(always)]
pub fn pass_tile_kani(player_id: i32) -> card_sdk::Asked {
    pass_tile(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn turn_end_after_kani(player_id: i32) -> card_sdk::Asked {
    turn_end_after(player_id)
}
