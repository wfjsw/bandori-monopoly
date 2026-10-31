//!
//! 规则书（docs/rulebook/cards.json, id `HHW:（薰）怪盗hello happy`）:
//! > （薰）怪盗hello happy：
//! >  将此卡放置于场上（充能3，衰减1）并向一名玩家场上放置一个怪盗标记，你本回合的移动阶段可以选择在经过该玩家时使自己强制停下并触发结算。此卡在场上时所有在薰所在格子的人如果可以移动，则主要移动改为投掷1d2（前后）和1d10（距离）进行结算。
//!
//! Place this card in play and put a thief mark on one other player.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ActionKind, Dice, FilterKind};
use card_sdk::CardText;
use card_sdk::{key, Action, CardDef, Filter, Msg, On, PlanValue};

const ID: &str = "HHW:（薰）怪盗hello happy";

pub const KAORU_THIEF: CardDef = CardDef::new(
    "HHW:（薰）怪盗hello happy",
    &[
        On::Play("", Some(cant_play), play),
        On::Action(Action::TurnEnd("actor == owner && card.placed", None, turn_end)),
        // 「你本回合的移动阶段可以选择在经过该玩家时使自己强制停下」 -- the offer is about the *marked* player.
        // SETTLE-STAGES §4 M4: 「经过该玩家」 is the mover's step onto a tile that player
        // stands on (行动阶段 12 [经过]) -- a mid-route `passTile`, NOT `passPlayer` (13
        // 「[重叠]」, end-tile only). Riding `passPlayer` made `move.remaining > 0` false
        // whenever the raise fired, so the mid-walk force-stop never engaged.
        On::Action(Action::PassTile("actor == owner && card.placed && move.is_main_move && move.remaining > 0 && card.counter('marked') != 0 && card.counter('turn') == turn_key", Some(marked_on_tile), pass_marked)),
        // TODO(规则书)（所有在薰所在格子的人）: The move-gate conjuncts are the status hosts'
        // `MoveGate` clauses (`exile` / `stay` / stun -- the same reads the engine's
        // walk clamp makes). `move.is_main_move` pins `actor` to the turn player, so the actor
        // is the mover. W3-KAORU-03 / C-Q48, OPEN.
        On::Filter(Filter::DiceRollPlan("card.placed && move.is_main_move && actor != owner && pos(actor) == owner.pos && state(actor,'exile') == 0 && state(actor,'stun') + state(actor,'stunStart') == 0 && (state(actor,'stay') == 0 || state(actor,'unstoppable') > 0) && state(owner,'exile') == 0 && plan.fixed_roll == null", None, roll_plan)),
        On::Action(Action::CounterChanged("actor == owner && card.placed && counter_is('crystals') && trigger_card == card.id && card.counter('crystals') == 0 && value <= 0", Some(crystals_changed_guard), on_crystals_changed)),
    ],
)
// 规则书「充能3，衰减1」: the `⌛3` marker the client shows.
.props(&[(bandori_std::marker::TURNS_LEFT, 3)])
.meta(&card_sdk::meta::CardMeta::new()
        .name(r#"怪盗hello happy"#)
        .raw_name(r#"（薰）怪盗hello happy"#)
        .tags(&[])
        .text(CardText::para(r#"将此卡放置于场上（充能3，衰减1）并向一名玩家场上放置一个怪盗标记，你本回合的移动阶段可以选择在经过该玩家时使自己强制停下并触发结算。此卡在场上时所有在薰所在格子的人如果可以移动，则主要移动改为投掷1d2（前后）和1d10（距离）进行结算。"#))
        // sheet cell F5
        .fields(&[("group", r#"Hello, Happy World!"#), ("owner", r#"濑田薰"#)]));

fn cant_play(player_id: i32) -> Option<Msg> {
    // TODO(规则书)（向一名玩家场上放置一个怪盗标记）: the book says 「一名玩家」 and does not exclude the
    // user's own field (spec: OPEN) -- we offer `others` only and refuse the play
    // outright when no other player is alive.
    if ctx::others(player_id).is_empty() {
        return Some(Msg::new(key!("x_no_rival")));
    }
    None
}

fn play(player_id: i32) -> card_sdk::Asked {
    let others = ctx::others(player_id);
    // `others` only (the own field is not offered) -- see the TODO in `cant_play`.
    let who = ctx::ask_player(
        player_id,
        &Msg::new(key!("kaoru_thief_ask_title")),
        &Msg::new(key!("kaoru_thief_ask_text")),
        &others,
    )?;
    // t.index : -1`: out / exile / `ImmuneAll` / `Untargetable` / the `target` [反击]
    // window all fail the designation, and a `redirect` hook may move the hit.
    let hit = ctx::target(who);
    ctx::set_fate(ctx::Fate::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("kaoru_thief_note")));
    // TODO(规则书)（充能3，衰减1）: the book never names the counter -- we use the 奇迹水晶
    // `crystals` counter (spec OPEN-HHW-CHARGE); if 充能 is not 奇迹 水晶, other cards'
    // crystal reads see the wrong counter.
    ctx::set_crystals(3);
    // The mark is a held counter on that player (「…玩家场上」), named for the book's
    // 「怪盗标记」; `skill:濑田薰:梦幻的王子殿下` reads and removes the same counter.
    if let Some(who) = hit {
        ctx::add_held_by_name(who, "怪盗标记", 1, i32::MAX)?;
        // Per-instance counters, not the owner's slots, so two copies in play do not
        // overwrite each other's memo; `marked` stores the seat + 1 (0 = none).
        ctx::set_counter("marked", who + 1);
        ctx::set_counter("turn", ctx::turn_key());
        ctx::log(
            player_id,
            &Msg::new(key!("kaoru_thief_marked")).player_id("who", who),
        );
    }
    Ok(())
}

/// Residual guard for [`pass_marked`]: the marked player stands on the tile being
/// passed. A `passTile` payload has **no** `target` (the other player is read off
/// the board) -- `trigger::target()` would be meaningless here. The memo is `seat
/// + 1` on this card's own counter (`counter()` is not CEL), and an [除外] player
/// is 「不存在于任何格子上」, so a stale `pos` must not match.
fn marked_on_tile(_player_id: i32) -> bool {
    let marked = ctx::counter("marked") - 1;
    marked >= 0
        && ctx::state::get(marked, bandori_std::state_key::EXILE) == 0
        && ctx::player_pos(marked) == trigger::tile()
}

fn pass_marked(player_id: i32) -> card_sdk::Asked {
    let marked = ctx::counter("marked") - 1;
    let who = marked;
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("kaoru_thief_stop_title")),
        &Msg::new(key!("kaoru_thief_stop_ask")).player_id("who", who),
    )? {
        return Ok(());
    }
    if !ctx::gate(trigger::player_id(), bandori_std::kind::abnormal::Stop) {
        return Ok(());
    }
    let tile = trigger::tile();
    if tile >= 0 {
        ctx::plan::set_stop_at(tile);
        ctx::plan::set_settles(true);
    }
    Ok(())
}

/// `actor == owner && card.placed` is the condition.
fn turn_end(_player_id: i32) -> card_sdk::Asked {
    // 「衰减1」 -- the tick.
    ctx::dec_turns_left();
    ctx::remove_crystal()?;
    Ok(())
}

///
/// TODO(规则书): 「（充能3，衰减1）」 names the charge and the decay but never says what
/// happens at 0, while every other crystal card spells out 「为0时 置入弃牌堆」. leaving a
/// spent card on the field. Residual guard -- the name is also
/// `counter_is('crystals')` in the condition (kept here as well). Card identity is
/// already in the CEL condition (`trigger_card == card.id`).
fn crystals_changed_guard(_player_id: i32) -> bool {
    trigger::name() == bandori_std::counter::CRYSTALS
}

fn on_crystals_changed(player_id: i32) -> card_sdk::Asked {
    ctx::set_fate(ctx::Fate::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("kaoru_thief_decayed")).player_id("who", player_id),
    );
    Ok(())
}

fn roll_plan(_player_id: i32, v: PlanValue) -> PlanValue {
    // TODO(规则书) 规则书: 「投掷1d2（前后）」 -- the host pre-rolls the direction face into
    // `recorded_roll` (`wasm_rules.rs` peeks `d(6)` for every DiceRollPlan /
    // DiceRollValue drive) so this Filter stays pure. W3-KAORU-02: the printed die
    // is a **1d2**, but the recorded face is a d6 peek (faces 1-6, logged as the
    // direction) and the polarity (face 1/2 vs odd/even) is unruled -- see C-Q47.
    // Closing the die shape needs a true `Dice { sides: 2 }` recorded face or a
    // card-side `roll(1,2)` feeding `reverse`.
    let reverse = v.recorded_roll.map(|dir| dir % 2 == 1);
    if let Some(dir) = v.recorded_roll {
        // This hook's `player_id` is the card's owner; `who` is the mover.
        let mover = trigger::player_id();
        ctx::log(
            mover,
            &Msg::new(key!("kaoru_thief_rollplan"))
                .player_id("who", mover)
                .i("dir", dir as i64),
        );
    }
    // 规则书: 「和1d10（距离）」 -- the face is *exactly* 1d10 (clear extra dice).
    PlanValue {
        reverse,
        clear_dice: true,
        base: Some(Dice {
            count: 1,
            sides: 10,
        }),
        ..v
    }
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
pub fn turn_end_kani(player_id: i32) -> card_sdk::Asked {
    turn_end(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn pass_marked_kani(player_id: i32) -> card_sdk::Asked {
    pass_marked(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn roll_plan_kani(player_id: i32, v: card_sdk::PlanValue) -> card_sdk::PlanValue {
    roll_plan(player_id, v)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_crystals_changed_kani(player_id: i32) -> card_sdk::Asked {
    on_crystals_changed(player_id)
}
