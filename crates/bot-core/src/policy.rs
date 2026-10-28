//! The one bot policy -- turn commands and prompt answers.
//!
//! This is the decision code `docs/BOT-DRIVE.md` moves out of the engine: a
//! bot seat is a client, so a host only needs a [`Cx`] over a throwaway world
//! (a determinized fork, or a clone for a live match) and one of the entries
//! below. [`turn_action`] is the turn surface, [`compute_answer`] a prompt
//! answer, and [`act_for`] / [`act_for_view`] the seat-view entries every host
//! calls.
//!
//! One copy is still in `game-core` (`engine/ai.rs`): it answers the seats
//! that are **not** bots -- a human under 托管, timed out or gone -- and their
//! setup, which `docs/BOT-DRIVE.md` step 3 deletes. `rules/tests/tests/`
//! `policy_parity.rs` pins the two together; keep them identical until then.
//!
//! Every seat carries a [`BotMentality`] (only `bot` seats take one): **standard**
//! is the standard policy (`docs/BOT.md`) and **chaos** is legal but maximally
//! disruptive. The thresholds below are the standard policy; chaos replaces the
//! money reserves with [`CHAOS_RESERVE`] and drops the "sometimes" rolls. The web
//! client's 托管 (auto-play) toggle ports both policies to TypeScript over the
//! public match view (`webui/src/game/autopilot.ts`); keep the two in sync.
//!
//! A human who times out or disconnects is answered with the **standard**
//! policy even though `ai` flips on -- see [`bot_mentality`].
//!
//! **Standard reads its thresholds from the seat's resolved
//! [`StrategyParams`]** (`docs/BOT.md` §3.8): the constants below are their
//! defaults, so an empty strategy book is exactly the old policy. Chaos is
//! not parameterised -- it keeps [`CHAOS_RESERVE`] and [`CHAOS_COUNTER_CHANCE`]
//! as literals.

use std::sync::Arc;

use game_core::data::GameData;
use game_core::engine::{purchase, Ask, CardRules, Cx, Match, RawAiAnswer, AI_UNSET};
use game_core::msg::Msg;
use game_core::net::NetMessage;
use game_core::state::prop::EST_COST;
use game_core::state::{stage, BotMentality, MatchPrompt};
use game_core::strategy::{StrategyParams, NEUTRAL_WEIGHT};

use crate::determinize::{determinize, DeterminizerRng};
use crate::view::SeatView;

/// Keep at least this much money after buying.
pub const BUY_RESERVE: i32 = 2_000;
/// Keep at least this much money after building.
pub const BUILD_RESERVE: i32 = 3_500;
/// Keep at least this much money after redeeming.
pub const REDEEM_RESERVE: i32 = 4_000;
/// Buy out a mortgaged deed only while it leaves this much.
pub const FORCE_BUY_RESERVE: i32 = 4_000;
/// Odds a bot plays a hand card rather than rolling in 运营.
pub const PLAY_CARD_CHANCE: f64 = 0.7;
/// Hand cards a bot plays in one turn before it rolls.
pub const MAX_PLAYS_PER_TURN: usize = 2;
/// [pass-reward] card odds ramp (user ruling 2026-10-28). At or below [`PASS_REWARD_CARD_MIN_MONEY`] the
/// seat takes the 2 000 money print; the card odds then rise superlinearly to
/// 1 at [`PASS_REWARD_CARD_MONEY`], above which the seat always draws. The
/// print's value falls as the stack grows, so the ramp is `t²`, not linear.
pub const PASS_REWARD_CARD_MIN_MONEY: i32 = 8_000;
pub const PASS_REWARD_CARD_MONEY: i32 = 80_000;
/// Chaos's coin reserve: voluntary spending only while it leaves this much.
/// The one knob the chaos policy spends to -- without it a chaos bot burns out
/// in a few turns and stops being disruptive.
pub const CHAOS_RESERVE: i32 = 1_000;
/// Chaos declares a counteraction on this share of the counteract offers it gets
/// (a random offered card); the rest of the offers it passes. Rolled per offer
/// from the match RNG, so a repeated offer cycle naturally stops after a skip.
pub const CHAOS_COUNTER_CHANCE: f64 = 0.3;

/// Would buying `price` leave [`BUY_RESERVE`]?
pub fn wants_buy(money: i32, price: i32) -> bool {
    money - price >= BUY_RESERVE
}

/// Would building for `cost` leave [`BUILD_RESERVE`]?
pub fn wants_build(money: i32, cost: i32) -> bool {
    money - cost >= BUILD_RESERVE
}

/// Standard's [pass-reward] card odds at `money` -- 0 at or below
/// [`PASS_REWARD_CARD_MIN_MONEY`], 1 at or above [`PASS_REWARD_CARD_MONEY`],
/// superlinear (`t²`) in between.
pub fn pass_reward_card_chance(money: i32) -> f64 {
    if money <= PASS_REWARD_CARD_MIN_MONEY {
        return 0.0;
    }
    if money >= PASS_REWARD_CARD_MONEY {
        return 1.0;
    }
    let t = f64::from(money - PASS_REWARD_CARD_MIN_MONEY)
        / f64::from(PASS_REWARD_CARD_MONEY - PASS_REWARD_CARD_MIN_MONEY);
    t * t
}

/// Would redeeming for `cost` leave [`REDEEM_RESERVE`]?
pub fn wants_redeem(money: i32, cost: i32) -> bool {
    money - cost >= REDEEM_RESERVE
}

/// Mentality-aware buy test: standard keeps [`BUY_RESERVE`], chaos only
/// [`CHAOS_RESERVE`]. Advanced is standard here -- it is the same policy
/// whenever the engine is the one driving the seat (`docs/BOT.md` B5).
pub fn bot_wants_buy(m: BotMentality, money: i32, price: i32) -> bool {
    match m {
        BotMentality::Standard | BotMentality::Advanced => wants_buy(money, price),
        BotMentality::Chaos => money - price >= CHAOS_RESERVE,
    }
}

/// Mentality-aware card-play reserve check against the card's bot-only
/// **estimated execution cost** (`prop::EST_COST`, user ruling 2026-10-07).
///
/// This is a *bot policy*, never legality: a human (and `cant_play`) may still
/// play the card and take the Q1 shortfall path. Standard keeps
/// [`BUY_RESERVE`]; chaos keeps its own [`CHAOS_RESERVE`]. A cost of `0`
/// (unknown / assume free) always passes.
pub fn bot_wants_play_card(m: BotMentality, money: i32, est_cost: i32) -> bool {
    if est_cost <= 0 {
        return true;
    }
    match m {
        BotMentality::Standard | BotMentality::Advanced => money - est_cost >= BUY_RESERVE,
        BotMentality::Chaos => money - est_cost >= CHAOS_RESERVE,
    }
}

/// Mentality-aware build test.
pub fn bot_wants_build(m: BotMentality, money: i32, cost: i32) -> bool {
    match m {
        BotMentality::Standard | BotMentality::Advanced => wants_build(money, cost),
        BotMentality::Chaos => money - cost >= CHAOS_RESERVE,
    }
}

/// Mentality-aware redeem test.
pub fn bot_wants_redeem(m: BotMentality, money: i32, cost: i32) -> bool {
    match m {
        BotMentality::Standard | BotMentality::Advanced => wants_redeem(money, cost),
        BotMentality::Chaos => money - cost >= CHAOS_RESERVE,
    }
}

/// Mentality-aware 「可选择pay…两倍…强行购买」 test.
pub fn bot_wants_force_buy(m: BotMentality, money: i32, price: i32) -> bool {
    match m {
        BotMentality::Standard | BotMentality::Advanced => money - price >= FORCE_BUY_RESERVE,
        BotMentality::Chaos => money - price >= CHAOS_RESERVE,
    }
}

/// The policy this seat actually plays under. A human seat (including one
/// taken over after a time-out / disconnect) is always [`BotMentality::Standard`]
/// -- a mentality is a property of a *bot*, not of whoever is driving it.
///
/// Inlined and branch-light: this sits on every bot decision, and the
/// standard path must not pay for the chaos one.
#[inline]
pub fn bot_mentality(cx: &Cx<'_>, i: usize) -> BotMentality {
    let p = &cx.state().players[i];
    if p.bot {
        p.mentality
    } else {
        BotMentality::Standard
    }
}

#[inline]
pub fn is_chaos(cx: &Cx<'_>, i: usize) -> bool {
    let p = &cx.state().players[i];
    p.bot && p.mentality == BotMentality::Chaos
}

/// The standard policy's parameters for seat `i` (`docs/BOT.md` §3.8).
/// Resolved from public state only (own persona + the other seats'
/// personas); chaos never reads them.
#[inline]
pub fn strategy_of(cx: &Cx<'_>, i: usize) -> StrategyParams {
    game_core::strategy::for_seat_sha(
        cx.data(),
        cx.state(),
        i,
        Some(cx.rules().ruleset_sha256().unwrap_or("stub")),
    )
}

/// Would buying `t` complete seat `i`'s colour group? Only computed when
/// [`StrategyParams::set_complete_bonus_milli`] is on (today it is 0).
fn buy_completes_set(cx: &Cx<'_>, i: usize, t: usize) -> bool {
    let group = cx.tile(t).group;
    (0..cx.data().tiles.len()).all(|u| {
        u == t
            || cx.tile(u).group != group
            || !cx.tile(u).is_buyable()
            || cx.state().owners[u] == i as i32
    })
}

/// Would buying leave the reserve? Uses the quoted price (`docs/PURCHASE.md`), so the AI
/// decides on the figure a hook-aware ruleset would actually charge; for
/// `StubRules` the quote is the plain rulebook formula.
#[inline]
pub fn ai_wants_buy(cx: &Cx<'_>, i: usize, t: usize) -> bool {
    let quote = cx.buy_quote_for(i, t, purchase::BuyKind::Land);
    if !quote.eligible {
        return false;
    }
    let price = quote.price.max(0);
    if is_chaos(cx, i) {
        cx.state().players[i].money - price >= CHAOS_RESERVE
    } else {
        let p = strategy_of(cx, i);
        let completes = p.set_complete_bonus_milli != 0 && buy_completes_set(cx, i, t);
        p.wants_buy_tile(
            cx.state().players[i].money,
            price,
            cx.tile(t).group,
            cx.state().round,
            completes,
        )
    }
}

/// Would building leave the reserve?
#[inline]
pub fn ai_wants_build(cx: &Cx<'_>, i: usize, t: usize) -> bool {
    if is_chaos(cx, i) {
        cx.state().players[i].money - cx.build_cost(t) >= CHAOS_RESERVE
    } else {
        let p = strategy_of(cx, i);
        p.wants_build_tile(
            cx.state().players[i].money,
            cx.build_cost(t),
            cx.tile(t).group,
            cx.state().houses.get(t).copied().unwrap_or(0),
        )
    }
}

/// Would a force-buy at `price` leave the reserve? Mirrors
/// `offer_force_buy`'s `Ask::with_ai` fill: standard reads
/// [`StrategyParams::wants_force_buy`], chaos keeps [`CHAOS_RESERVE`].
/// `price` is the quoted force-buy figure (the prompt's `view.price`),
/// which is not the plain land quote.
#[inline]
pub fn ai_wants_force_buy(cx: &Cx<'_>, i: usize, price: i32) -> bool {
    let money = cx.state().players[i].money;
    if is_chaos(cx, i) {
        bot_wants_force_buy(bot_mentality(cx, i), money, price)
    } else {
        strategy_of(cx, i).wants_force_buy(money, price)
    }
}

/// Standard: first affordable purchase, else first build,
/// else none. Chaos: a random option it can pay for, never "none" while one
/// exists.
pub fn ai_agent_choice(cx: &mut Cx<'_>, p: usize, options: &[usize]) -> i32 {
    if is_chaos(cx, p) {
        let ok: Vec<usize> = options
            .iter()
            .copied()
            .enumerate()
            .filter(|&(_, t)| {
                (cx.state().owners[t] < 0 && ai_wants_buy(cx, p, t))
                    || (cx.state().owners[t] == p as i32 && ai_wants_build(cx, p, t))
            })
            .map(|(k, _)| k)
            .collect();
        return match ok.len() {
            0 => options.len() as i32,
            n => ok[cx.world_mut().rng.below(n)] as i32,
        };
    }
    let params = strategy_of(cx, p);
    let round = cx.state().round;
    if let Some(k) = options.iter().position(|&t| {
        if cx.state().owners[t] >= 0 {
            return false;
        }
        let quote = cx.buy_quote_for(p, t, purchase::BuyKind::Agent);
        quote.eligible
            && params.wants_buy_tile(
                cx.state().players[p].money,
                quote.price.max(0),
                cx.tile(t).group,
                round,
                false,
            )
    }) {
        return k as i32;
    }
    if let Some(k) = options.iter().position(|&t| {
        cx.state().owners[t] == p as i32
            && params.wants_build_tile(
                cx.state().players[p].money,
                cx.build_cost(t),
                cx.tile(t).group,
                cx.state().houses.get(t).copied().unwrap_or(0),
            )
    }) {
        return k as i32;
    }
    options.len() as i32
}

/// Standard: most valuable mortgaged deed that leaves
/// the reserve (the original sort-then-`find`, which short-circuits).
/// Chaos: any affordable one, at random.
fn ai_redeem_choice(cx: &mut Cx<'_>, i: usize) -> Option<usize> {
    if is_chaos(cx, i) {
        let deeds: Vec<usize> = (0..cx.data().tiles.len())
            .filter(|&t| cx.state().owners[t] == i as i32 && cx.state().mortgaged[t])
            .filter(|&t| cx.state().players[i].money - cx.redeem_cost(t) >= CHAOS_RESERVE)
            .collect();
        if deeds.is_empty() {
            return None;
        }
        let k = cx.world_mut().rng.below(deeds.len());
        return Some(deeds[k]);
    }
    let mut deeds: Vec<usize> = (0..cx.data().tiles.len())
        .filter(|&t| cx.state().owners[t] == i as i32 && cx.state().mortgaged[t])
        .collect();
    let p = strategy_of(cx, i);
    deeds.sort_by_key(|&t| p.redeem_key(cx.tile(t).price));
    deeds
        .into_iter()
        .find(|&t| p.wants_redeem(cx.state().players[i].money, cx.redeem_cost(t)))
}

/// A random playable card the rules say a bot would play.
/// Chaos ignores the rule's `ai_play` heuristic (it will even raise a counteraction
/// cards from hand) and only asks `cant_play`.
///
/// Standard additionally gates on the card's [`game_core::strategy::CardPlayParams`]
/// (round window, `hold_for_counteract`, weight `> 0`, per-card cash floor
/// over `estCost`). All defaults are neutral, so the candidate set and the
/// uniform pick are exactly the old ones.
fn ai_card_choice(cx: &mut Cx<'_>, i: usize) -> Option<String> {
    let chaos = is_chaos(cx, i);
    let mentality = bot_mentality(cx, i);
    let money = cx.state().players[i].money;
    let mut hand = cx.world().hidden[i].hand.clone();
    hand.dedup();
    // Bot-only reserve check against the card's estimated execution cost
    // (`prop::EST_COST`, user ruling 2026-10-07). Never legality -- `cant_play`
    // is the only gate for that.
    let affordable = |id: &str| {
        let est = cx.rules().card_prop(id, EST_COST);
        bot_wants_play_card(mentality, money, est)
    };
    if chaos {
        let ok: Vec<String> = hand
            .into_iter()
            .filter(|id| cx.cant_play(i, id, false).is_none() && affordable(id))
            .collect();
        return if ok.is_empty() {
            None
        } else {
            let k = cx.world_mut().rng.below(ok.len());
            Some(ok[k].clone())
        };
    }
    let p = strategy_of(cx, i);
    let round = cx.state().round;
    let mut ok: Vec<String> = Vec::new();
    let mut weights: Vec<i32> = Vec::new();
    for id in hand {
        if cx.cant_play(i, &id, false).is_some() || !cx.rules().ai_play(cx, i, &id) {
            continue;
        }
        let params = p.card(&id);
        if params.play_weight_milli <= 0
            || params.hold_for_counteract
            || round < params.min_round
            || round > params.max_round
        {
            continue;
        }
        let est = cx.rules().card_prop(&id, EST_COST);
        if !p.wants_play_card(money, est, &id) {
            continue;
        }
        ok.push(id);
        weights.push(params.play_weight_milli);
    }
    if ok.is_empty() {
        return None;
    }
    // Uniform among equal weights (the default): the original
    // `ok[rng.below(len)]`, same draw, same mapping. A non-uniform weight
    // set switches to one cumulative draw.
    let uniform = weights.windows(2).all(|w| w[0] == w[1]);
    let k = if uniform {
        cx.world_mut().rng.below(ok.len())
    } else {
        let total: i64 = weights.iter().map(|&w| w.max(0) as i64).sum();
        if total <= 0 {
            cx.world_mut().rng.below(ok.len())
        } else {
            let mut r = (cx.world_mut().rng.below(total as usize)) as i64;
            let mut k = ok.len() - 1;
            for (j, &w) in weights.iter().enumerate() {
                r -= w.max(0) as i64;
                if r < 0 {
                    k = j;
                    break;
                }
            }
            k
        }
    };
    Some(ok[k].clone())
}

/// A skill button the seat would press right now: a placed persona /
/// group skill rule the player may activate.
///
/// Chaos presses every usable one (at most one press per skill per turn --
/// a skill whose body is a no-op must not park the bot in a press loop).
/// Standard only presses when the seat's [`game_core::strategy::SkillParams`]
/// say so; the default entry is "never", which is the old standard policy
/// (skills were left to the player). The gate is the rule's own
/// `cant_play` (`why_not_act` asks the same one before honouring a `skill`
/// command).
fn ai_skill_choice(cx: &mut Cx<'_>, i: usize) -> Option<String> {
    let placed = cx.world().placed_cards(i as i32);
    let chaos = is_chaos(cx, i);
    let params = if chaos {
        None
    } else {
        Some(strategy_of(cx, i))
    };
    let candidates: Vec<String> = cx
        .data()
        .skill_rules_of(&cx.state().players[i].persona)
        .into_iter()
        .filter(|id| {
            placed.contains(id)
                && !cx.world().turn.played.contains(id)
                && cx.rules_cant_play(i, id).is_none()
        })
        .collect();
    let round = cx.state().round;
    let mut ok: Vec<String> = Vec::new();
    for id in candidates {
        if let Some(p) = &params {
            let sk = p.skill(&id);
            // `min_charge` / `min_named_counter` / `keep_markers` are tuner
            // metadata in S1 -- the S1 heuristic enforces the weight and
            // the round window only (`docs/BOT.md` §3.8).
            if sk.play_weight_milli <= 0 || round < sk.min_round || round > sk.max_round {
                continue;
            }
            // Odds per offer; the "always" group draws nothing extra, and a
            // zero weight (the default) short-circuits above so the RNG
            // stream of the old policy is untouched.
            if sk.play_weight_milli < NEUTRAL_WEIGHT {
                let roll = cx.world_mut().rng.f64() * 1000.0;
                if roll >= sk.play_weight_milli as f64 {
                    continue;
                }
            }
        }
        ok.push(id);
    }
    if ok.is_empty() {
        None
    } else {
        let k = cx.world_mut().rng.below(ok.len());
        Some(ok[k].clone())
    }
}

/// The auction ceiling for seat `s` on a tile of price `base`. Standard
/// randomises around the price; chaos will spend down to [`CHAOS_RESERVE`].
pub fn ai_auction_worth(cx: &mut Cx<'_>, s: usize, base: i32) -> i32 {
    let money = cx.state().players[s].money;
    if is_chaos(cx, s) {
        return (money - CHAOS_RESERVE).max(0);
    }
    let p = strategy_of(cx, s);
    let roll = cx.world_mut().rng.f64();
    p.auction_worth(roll, base, money)
}

/// The standard / chaos policy's next **turn-surface** command for seat
/// `i`, without applying it. This is the single decision implementation:
/// [`act_for`] serialises it for a seat view. `None` when the surface is not
/// this seat's to act on.
///
/// The standard path is the standard bot policy -- the chaos branch is
/// taken *before* it and returns, so a standard seat never pays for it.
pub fn turn_action(cx: &mut Cx<'_>, i: usize) -> Option<NetMessage> {
    if cx.state().phase != "play" || cx.state().busy || cx.state().turn != i as i32 {
        return None;
    }
    if cx.state().step == stage::OPS {
        if is_chaos(cx, i) {
            // Cards first (every legal opportunity, no cap), then every
            // usable skill, then redeem down to the reserve. Only then
            // does the turn move on.
            if let Some(card) = ai_card_choice(cx, i) {
                return Some(NetMessage {
                    act: "play".into(),
                    card,
                    ..Default::default()
                });
            }
            if let Some(sk) = ai_skill_choice(cx, i) {
                return Some(NetMessage {
                    act: "skill".into(),
                    card: sk,
                    ..Default::default()
                });
            }
            if let Some(t) = ai_redeem_choice(cx, i) {
                return Some(NetMessage {
                    act: "redeem".into(),
                    value: t as i32,
                    ..Default::default()
                });
            }
        } else {
            let p = strategy_of(cx, i);
            if let Some(t) = ai_redeem_choice(cx, i) {
                return Some(NetMessage {
                    act: "redeem".into(),
                    value: t as i32,
                    ..Default::default()
                });
            }
            if (cx.state().played_count as usize) < p.max_plays_per_turn.max(0) as usize
                && cx.world_mut().rng.chance(p.play_card_chance())
            {
                if let Some(card) = ai_card_choice(cx, i) {
                    return Some(NetMessage {
                        act: "play".into(),
                        card,
                        ..Default::default()
                    });
                }
            }
            // A skill press only when the seat's `SkillParams` ask for one
            // (the default entry is "never", the old policy). No RNG is
            // drawn otherwise, so the stream above is untouched.
            if let Some(sk) = ai_skill_choice(cx, i) {
                return Some(NetMessage {
                    act: "skill".into(),
                    card: sk,
                    ..Default::default()
                });
            }
        }
        if cx.state().skip_move {
            return Some(NetMessage::act("end"));
        }
        return Some(NetMessage::act("roll"));
    }
    if cx.state().step != stage::END {
        return None;
    }
    // Buy / build are settle-time offers only: the tile rule's
    // `ctx::offer_buy` / `ctx::offer_build` prompt already carried the
    // bot's `ai_wants_*` answer. END is discard / end-turn.
    if cx.over_hand(i) {
        let hand_len = cx.world().hidden[i].hand.len();
        if hand_len == 0 {
            return None;
        }
        let k = cx.world_mut().rng.below(hand_len);
        return Some(NetMessage {
            act: "discard".into(),
            card: cx.world().hidden[i].hand[k].clone(),
            ..Default::default()
        });
    }
    Some(NetMessage::act("end"))
}

/// The card id a prompt option declares (`Arg::Card` on the option's label).
/// Used by the counteraction offer, whose options are `ask.counteract.play` messages
/// naming the card each would declare.
fn option_card(m: &Msg) -> Option<String> {
    m.a.get("card").and_then(|a| match a {
        game_core::msg::Arg::Card(id) => Some(id.clone()),
        _ => None,
    })
}

/// The answer `seat`'s policy would give for `ask` -- the per-seat body of the
/// prompt fill. Same RNG draws as the prompt-open fill (so a live match that
/// re-runs it replays), which is also why a fork's answer can differ from the
/// live one: the fork owns a fresh stream (`docs/BOT.md` §1, the hidden-state
/// audit).
pub fn answer_for_entry(cx: &mut Cx<'_>, ask: &Ask, seat: usize) -> i32 {
    let fallback = ask.view.fallback;
    // A tile prompt carries its targets in `items` (answer == len is
    // "none"); every other prompt's answers index `options`.
    let n = if ask.view.items.is_empty() {
        ask.view.options.len() as i32
    } else {
        ask.view.items.len() as i32
    };
    let counteract = ask.view.title.key() == "ask.counteract.title";
    if ask.view.kind == "multi" {
        // Bitmask of exactly `count` distinct options. Standard takes the
        // fallback (first `count`); chaos rolls a random valid mask.
        return if is_chaos(cx, seat) {
            chaos_multi_pick(cx, ask)
        } else {
            fallback
        };
    }
    if is_chaos(cx, seat) {
        if counteract && n > 1 && cx.world_mut().rng.f64() >= CHAOS_COUNTER_CHANCE {
            // This offer it passes (the fallback is the skip option).
            fallback
        } else {
            chaos_pick(cx, fallback, n)
        }
    } else if counteract {
        // Standard: declare on the seat's per-card propensity
        // (`docs/BOT.md` §3.8 "counteraction"; default 600‰, user
        // ruling 2026-10-08). `CounterParams` 0 holds a card back.
        counteract_pick(cx, seat, ask, fallback, n)
    } else if ask.view.title.key() == "ask.pass_reward.title" {
        // Standard: the card odds ramp with the seat's money (user
        // ruling 2026-10-28). Both the server bot and 托管 come through
        // here, so there is no second copy to keep in step.
        pass_reward_pick(cx, seat, ask, fallback)
    } else {
        fallback
    }
}

/// Standard's [pass-reward] answer: the card on
/// [`pass_reward_card_chance`], else `fallback` (the money
/// print). Both option wordings carry the card ("card" on a pass,
/// "card_landed" on a stop on the start square). Only the intermediate
/// band draws the world RNG, so the stream moves exactly where the
/// outcome is undecided.
fn pass_reward_pick(cx: &mut Cx<'_>, seat: usize, ask: &Ask, fallback: i32) -> i32 {
    let money = cx.state().players.get(seat).map_or(0, |p| p.money);
    let chance = pass_reward_card_chance(money);
    if chance <= 0.0 {
        return fallback;
    }
    if chance < 1.0 && cx.world_mut().rng.f64() >= chance {
        return fallback;
    }
    ask.view
        .options
        .iter()
        .position(|o| {
            matches!(
                o.key(),
                "ask.pass_reward.card" | "ask.pass_reward.card_landed"
            )
        })
        .map_or(fallback, |i| i as i32)
}

/// Standard's counteraction answer: the first offered card whose
/// [`game_core::strategy::CounterParams`] propensity hits, else `fallback`
/// (the skip). Each candidate draws from the world RNG (so it replays) --
/// one draw per offered card until one hits, so with `k` offered cards
/// the per-offer declare rate is `1 - (1 - p)^k` on a uniform per-card
/// propensity `p` (600‰ default: 60 % for one card, 84 % for two). A
/// zero propensity draws nothing and never hits, which is how a book
/// holds a card back.
fn counteract_pick(cx: &mut Cx<'_>, seat: usize, ask: &Ask, fallback: i32, n: i32) -> i32 {
    let p = strategy_of(cx, seat);
    for i in 0..n {
        if i == fallback {
            continue;
        }
        let Some(id) = ask.view.options.get(i as usize).and_then(option_card) else {
            continue;
        };
        let propensity = p.counteract_propensity(&id, None);
        if propensity <= 0 {
            continue;
        }
        let roll = cx.world_mut().rng.f64() * 1000.0;
        if roll < propensity as f64 {
            return i;
        }
    }
    fallback
}

/// Uniform among `0..n` except `fallback`, falling back to `fallback` when
/// it is the only option (or `n` is empty). Allocation-free: one draw from
/// the `n-1` non-default slots, shifted past `fallback`.
fn chaos_pick(cx: &mut Cx<'_>, fallback: i32, n: i32) -> i32 {
    if n <= 1 {
        return fallback;
    }
    let has_fallback = fallback >= 0 && fallback < n;
    let non_default = n - i32::from(has_fallback);
    if non_default <= 0 {
        return fallback;
    }
    let mut k = cx.world_mut().rng.below(non_default as usize) as i32;
    if has_fallback && k >= fallback {
        k += 1;
    }
    k
}

/// A random valid multi-select mask: exactly `count` distinct options out
/// of `0..options.len()`. Falls back to the prompt's fallback when the
/// shape is degenerate (0 options, or `count` past what a 31-bit mask can
/// name).
fn chaos_multi_pick(cx: &mut Cx<'_>, ask: &Ask) -> i32 {
    let n = ask.view.options.len().min(31) as i32;
    let want = ask.view.count.clamp(1, n.max(1));
    if n <= 0 || want > n {
        return ask.view.fallback;
    }
    // Draw `want` distinct indices without replacement.
    let mut pool: Vec<i32> = (0..n).collect();
    let mut mask = 0i32;
    for _ in 0..want {
        let i = cx.world_mut().rng.below(pool.len());
        let x = pool.swap_remove(i);
        mask |= 1 << x;
    }
    mask
}

/// The engine's own answer for `member`'s live prompt, **computed** from the
/// throwaway [`Cx`] -- the seat-view twin of the prompt-open fill that reads
/// the precomputed [`Ask`].
///
/// A determinized fork has no precomputed fill (`stub_pending` can only mirror
/// the searching seat's public `aiAnswer`, which the online frame no longer
/// carries). This entry runs the same choice functions the prompt-open fill
/// ran:
///
/// * [`ai_auction_worth`] for an auction ceiling,
/// * [`Cx::auto_mortgage`] for a mortgage selection,
/// * the `Ask::with_ai` policy tests the settle offers stamp -- "no"
///   only when the fill said no ([`ai_wants_buy`] / [`ai_wants_build`] /
///   [`ai_wants_force_buy`], 0 = accept),
/// * [`ai_agent_choice`] for the agent landing's tile pick
///   (`rules/tiles`' `ask.agent.title`),
/// * [`answer_for_entry`] for every other prompt (the fallback, or
///   a counteraction propensity roll).
///
/// The `with_ai` row matters: those fills are **not** the fallback. A
/// recompute that only ran `answer_for_entry` answered a fundable buy
/// with the prompt's "decline" slot, and online 托管 (which trusts this
/// number over its own policy table) never bought a tile. Keep the
/// title-keyed rows in step with the `Ask::with_ai` sites
/// (`offer_buy` / `offer_build` / `offer_force_buy`) and
/// `rules/tiles` (`agent.rs`).
///
/// `None` when the prompt does not wait on `seat` or it has already answered.
/// The result is **not** bit-identical to the live match's precomputed fill
/// whenever that fill drew from the match RNG (auction ceilings, counteraction
/// propensity) -- the fork owns a fresh stream. That is expected and
/// listed in the hidden-state audit (`docs/BOT.md` §1). The rows above are
/// deterministic and must match exactly.
pub fn compute_answer(
    cx: &mut Cx<'_>,
    prompt: &MatchPrompt,
    seat: usize,
    fallback: i32,
) -> Option<RawAiAnswer> {
    let i = seat;
    let view = prompt;
    if view.id == 0 {
        return None;
    }
    let k = view.player_index(i as i32)?;
    if view.answers.get(k).copied().unwrap_or(-1) >= 0 {
        return None;
    }
    match prompt.kind.as_str() {
        "auction" => {
            let base = if prompt.tile >= 0 {
                purchase::quote_native(cx.data(), &cx.world().st, prompt.tile as usize).max(0)
            } else {
                0
            };
            let worth = ai_auction_worth(cx, i, base);
            Some(RawAiAnswer {
                answer: 0,
                picked: Vec::new(),
                worth,
            })
        }
        "mortgage" => {
            let need = prompt.bid;
            let deeds: Vec<usize> = prompt.items.iter().filter_map(|s| s.parse().ok()).collect();
            let picked = if deeds.is_empty() {
                cx.auto_mortgage(i, need)
            } else {
                cx.auto_mortgage_from(i, need, deeds)
            };
            Some(RawAiAnswer {
                answer: 0,
                picked,
                worth: 0,
            })
        }
        _ => {
            // `Ask::with_ai` rows first: the settle offers stamp a policy
            // test, not the fallback, and `answer_for_entry` would
            // return the prompt's "decline" / "none" slot for them. Title
            // key is the marker (the client's `autopilot.ts` reads the
            // same keys). 0 = accept the offer.
            let t = prompt.tile;
            let title = prompt.title.key();
            let policy: Option<i32> = match title {
                "ask.buy.title" if t >= 0 => Some(if ai_wants_buy(cx, i, t as usize) {
                    0
                } else {
                    1
                }),
                "ask.build.title" if t >= 0 => Some(if ai_wants_build(cx, i, t as usize) {
                    0
                } else {
                    1
                }),
                "ask.force_buy.title" if t >= 0 => {
                    let price = if prompt.price >= 0 {
                        prompt.price
                    } else {
                        purchase::quote_native(cx.data(), &cx.world().st, t as usize).max(0)
                    };
                    Some(if ai_wants_force_buy(cx, i, price) {
                        0
                    } else {
                        1
                    })
                }
                "ask.agent.title" => {
                    let tiles: Vec<usize> =
                        prompt.items.iter().filter_map(|s| s.parse().ok()).collect();
                    Some(ai_agent_choice(cx, i, &tiles))
                }
                _ => None,
            };
            if let Some(answer) = policy {
                return Some(RawAiAnswer {
                    answer,
                    picked: Vec::new(),
                    worth: 0,
                });
            }
            // The prompt-open fill's per-seat body, on a throwaway `Ask` view of
            // the same prompt (the fallback / counteraction propensity path). The
            // prompt's own `Ask` is already filled on a stubbed fork, so
            // read the entry through the shared helper rather than re-filling.
            let ask = Ask {
                view: prompt.clone(),
                ai: vec![AI_UNSET],
                ai_picked: vec![Vec::new()],
                worth: vec![0],
                use_pool: true,
            };
            let answer = answer_for_entry(cx, &ask, i);
            Some(RawAiAnswer {
                answer: if answer == AI_UNSET { fallback } else { answer },
                picked: Vec::new(),
                worth: 0,
            })
        }
    }
}

/// The policy's next command for `member` -- the seat-view entry every host
/// calls. `None` when the seat has nothing to decide.
pub fn act_for(m: &Match, member: i32) -> Option<NetMessage> {
    let st = m.state();
    let i = st.players.iter().position(|p| p.member == member)?;
    // A prompt is an answer surface in any phase (初始重抽 opens in setup); the
    // turn surfaces below only exist in play.
    if st.phase != "play" && st.prompt.id == 0 {
        return None;
    }
    // Votes are answered by the scheduler (`tick_vote`), not here; the
    // client keeps the vote branch above this action in `plan`.
    if st.vote.id > 0 {
        return None;
    }
    if st.prompt.id > 0 {
        // `compute_answer` owns the waiting check (another seat's
        // prompt, an answered slot) -- and the state carries the live prompt.
        let prompt = st.prompt;
        let a = {
            let mut cx = m.bot_cx();
            compute_answer(&mut cx, &prompt, i, prompt.fallback)?
        };
        let mut msg = NetMessage {
            act: "answer".into(),
            prompt: prompt.id,
            ..Default::default()
        };
        match prompt.kind.as_str() {
            "auction" => msg.value = a.worth,
            "mortgage" => msg.cards = a.picked,
            "pick" if !prompt.items.is_empty() => {
                // Card picks answer with the card list; `compute_answer`
                // carries the selection in `picked` where the prompt has
                // one, else the fallback index names the item.
                msg.cards = a.picked;
                if msg.cards.is_empty() {
                    if let Some(c) = prompt.items.get(a.answer.max(0) as usize) {
                        msg.cards.push(c.clone());
                    }
                }
            }
            _ => msg.value = a.answer,
        }
        return Some(msg);
    }
    let mut cx = m.bot_cx();
    turn_action(&mut cx, i)
}

/// The policy's command for the seat owning `view` -- determinize, then decide
/// on the fork. `None` when there is nothing to decide or the sample failed
/// (caller falls back). Never searches: standard / chaos seats answer here,
/// only 进阶 goes through `decide`.
///
/// The fork is a plain restore and stays **policy-free** (no answer provider):
/// the decision runs once on it. `rng` is the determinizer's sampling stream
/// and is consumed; do not reuse it across decisions.
pub fn act_for_view(
    data: &Arc<GameData>,
    rules: &Arc<dyn CardRules>,
    view: &SeatView,
    rng: &mut DeterminizerRng,
) -> Option<NetMessage> {
    let (fork, _report) = determinize(view, data, rules, rng).ok()?;
    act_for(&fork, view.you)
}
