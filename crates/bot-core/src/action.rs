//! Action abstraction (`docs/BOT.md` §3.4): what the search branches on.
//!
//! Searched decisions only -- card plays, buy / build / force-buy, auctions,
//! counteract offers, agent picks. Everything else (roll, end, discard,
//! trivial prompts) is delegated to the heuristic: an empty action list means
//! "not a tree decision" and the caller answers it with the engine's own
//! `aiAnswer` / step machine.

use game_core::data::GameData;
use game_core::engine::purchase;
use game_core::state::{MatchPrompt, MatchState, stage};

// ------------------------------------------------------------ A/B knobs
//
// Measurement-only switches for the `ismcts_vs_bots` strength cells
// (`docs/BOT.md` §5 C1). Read once, from the environment; the shipped
// behaviour is whatever happens when they are absent. Never set in
// production -- they exist so one piece of C1 can be isolated at a time.

/// `BOT_STRENGTH_AB_*` knob: is this measurement switch on?
pub fn ab_on(key: &str) -> bool {
    use std::sync::OnceLock;
    static ON: OnceLock<std::collections::HashSet<&'static str>> = OnceLock::new();
    ON.get_or_init(|| {
        [
            "BOT_STRENGTH_AB_NO_DECLINE",
            "BOT_STRENGTH_AB_NO_FASTPATH",
            "BOT_STRENGTH_AB_NO_LOWSTAKES",
            "BOT_STRENGTH_AB_NO_GATES",
        ]
        .into_iter()
        .filter(|k| std::env::var(k).is_ok())
        .collect()
    })
    .contains(key)
}

/// One abstracted action at a decision.
///
/// `Ord` is a stable total order used as the final tie-break when the search
/// merges root statistics (root-parallel ISMCTS must be deterministic given
/// the seed and thread count).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Action {
    /// Play a hand card.
    Play { card: String },
    /// Buy the landed tile.
    Buy { tile: usize },
    /// Build one more house on the landed tile.
    Build { tile: usize },
    /// End the turn without buying / building (`act: "end"` at 结束). Always
    /// offered where the engine accepts `end`, so the search can pass on a
    /// buy instead of being forced into one.
    Decline,
    /// A buy / build / force-buy offer prompt: option index.
    Offer { index: i32 },
    /// An auction bid (`amount < 0` = pass).
    Bid { amount: i32 },
    /// A [反击] offer: declare a card, or skip (`None`).
    Counteract { card: Option<String> },
    /// An agent / tile pick: option index into the prompt's `items`
    /// (`items.len()` = "none").
    Pick { index: i32 },
    /// A mortgage selection, in the heuristic's order (most valuable first).
    Mortgage { tiles: Vec<usize> },
}

/// The decision surface: a prompt answer, or a turn-surface command.
#[derive(Debug, Clone, PartialEq)]
pub enum Surface {
    /// The open prompt, waiting on the searching seat.
    Prompt(MatchPrompt),
    /// The turn surface: 运营 (card plays) or 结束 (buy / build).
    Turn,
}

/// Is this seat at a decision right now (searchable or not)?
pub fn surface(st: &MatchState, seat: usize) -> Option<Surface> {
    if st.phase != "play" {
        return None;
    }
    if st.prompt.id > 0 {
        if !st.prompt.waiting(seat as i32) {
            return None;
        }
        return Some(Surface::Prompt(st.prompt.clone()));
    }
    if st.busy {
        return None;
    }
    match st.step {
        s if s == stage::OPS && st.turn == seat as i32 => Some(Surface::Turn),
        s if s == stage::END && st.turn == seat as i32 => Some(Surface::Turn),
        _ => None,
    }
}

/// The abstracted action list at `st` for `seat`.
///
/// An empty list means "nothing worth searching here" -- the caller applies
/// the heuristic and moves on. Searched set: card plays, buy / build /
/// force-buy offers, auction bids, counteract offers, agent picks.
pub fn legal_actions(
    data: &GameData,
    st: &MatchState,
    hand: &[String],
    playable: &[bool],
    seat: usize,
) -> Vec<Action> {
    legal_actions_with_cost(data, st, hand, playable, &[], seat)
}

/// [`legal_actions`] plus the per-card bot-only **estimated execution cost**
/// (`prop::EST_COST`, user ruling 2026-10-07). A card whose estimate would dip
/// below [`crate::ai_reserve`] is dropped from the candidate set -- this is a
/// search policy, never legality (`cant_play` is the only legality gate).
pub fn legal_actions_with_cost(
    data: &GameData,
    st: &MatchState,
    hand: &[String],
    playable: &[bool],
    est_cost: &[i32],
    seat: usize,
) -> Vec<Action> {
    match surface(st, seat) {
        None => Vec::new(),
        Some(Surface::Turn) => turn_actions(data, st, hand, playable, est_cost, seat),
        Some(Surface::Prompt(p)) => prompt_actions(data, st, &p, seat),
    }
}

/// Is this prompt's kind one the search branches on?
pub fn searchable_prompt(p: &MatchPrompt) -> bool {
    match p.kind.as_str() {
        "auction" | "mortgage" | "tile" | "pick" => true,
        "choice" => {
            let n = p.options.len();
            if n <= 1 {
                return false;
            }
            let title = p.title.k.as_ref();
            // Force-buy / buy / build offers, [反击] chains, and non-trivial
            // choices (pay / target / card-rule prompts) are all searched.
            // Mulligan and other 2-option defaults fall out via the caller's
            // heuristic when the list comes back empty... they don't: keep
            // mulligan out explicitly.
            if title == "ask.mulligan.title" {
                return false;
            }
            true
        }
        _ => false,
    }
}

fn turn_actions(
    data: &GameData,
    st: &MatchState,
    hand: &[String],
    playable: &[bool],
    est_cost: &[i32],
    seat: usize,
) -> Vec<Action> {
    let mut out = Vec::new();
    if st.step == stage::OPS {
        // Card plays only; the roll / end is the heuristic's one sensible move.
        let money = st.players.get(seat).map(|p| p.money).unwrap_or(0);
        // The seat's `StrategyParams` (`docs/BOT.md` §3.8): the card-play
        // reserve is the default `BUY_RESERVE`, or the card's own floor.
        let params = game_core::strategy::for_seat(data, st, seat);
        for (i, c) in hand.iter().enumerate() {
            if !playable.get(i).copied().unwrap_or(true) {
                continue;
            }
            let est = est_cost.get(i).copied().unwrap_or(0);
            if est > 0 && money - est < params.play_card_reserve_for(c) {
                continue;
            }
            out.push(Action::Play { card: c.clone() });
        }
        return out;
    }
    if st.step == stage::END {
        let t = st.landed;
        if t >= 0 {
            let t = t as usize;
            if buyable(data, st, seat, t) {
                out.push(Action::Buy { tile: t });
            }
            if can_build(data, st, seat, t) {
                out.push(Action::Build { tile: t });
            }
        }
        // Decline -- "end the turn without buying" -- only when the turn would
        // otherwise have no action at all. The 2026-10-08 C1 bisect
        // (`docs/BOT.md` §5 C1) proved that putting Decline on the menu next
        // to a *legal* Buy/Build costs ~5 wins on StubRules: `net_worth` counts
        // a deed at its price, so a buy is worth-neutral at the instant and
        // the tie goes to the progressive bias, which leans Decline below
        // `BUY_RESERVE` -- exactly where the old bot was forced to buy. So a
        // paid option that the engine accepts stays forced (the search cannot
        // pass on it); Decline covers the empty menu -- the unaffordable /
        // gate-refused buy of the stall, where the alternative was a refused
        // `Buy` replayed every 200 ms. Gated on `can_end_here`
        // (`why_not_act`'s end branch: `err.over_hand` / `err.roll_first`).
        //
        // A/B: `BOT_STRENGTH_AB_NO_DECLINE` drops the option entirely;
        // `BOT_STRENGTH_AB_NO_GATES` drops the `can_end_here` half of the gate
        // (it also drops the buy/build/roll/end gates -- see [`ab_on`]).
        if out.is_empty()
            && !ab_on("BOT_STRENGTH_AB_NO_DECLINE")
            && (st.can_end_here || ab_on("BOT_STRENGTH_AB_NO_GATES"))
        {
            out.push(Action::Decline);
        }
    }
    out
}

/// The card id a [反击] offer option declares (`Arg::Card` on the option's
/// `ask.counteract.play` label -- the same read `cx::option_card` makes).
/// `MatchPrompt::card` is the *answered* link's card, not the offered one, so
/// the per-option label is the only place an offered card id lives.
pub fn option_card_id(m: &game_core::msg::Msg) -> Option<String> {
    match m.a.get("card") {
        Some(game_core::msg::Arg::Card(id)) => Some(id.clone()),
        _ => None,
    }
}

/// The abstracted action for a [反击] offer's option index (`None` = the
/// fallback's skip). One action per offered card, so the search can branch on
/// *which* card to declare.
pub fn counteract_action(p: &MatchPrompt, index: i32) -> Action {
    // Sayo's distance choice carries source metadata for the heuristic, but
    // +1 and +2 must remain separate choices before a source is declared.
    if p.text.key() == "ask.counteract.move_extension" {
        return Action::Offer {
            index: if index < 0 { p.fallback } else { index },
        };
    }
    if index < 0 || index == p.fallback {
        return Action::Counteract { card: None };
    }
    match p.options.get(index as usize).and_then(option_card_id) {
        Some(id) => Action::Counteract { card: Some(id) },
        // An option with no card id cannot name a declaration; treat it as the
        // skip rather than inventing a card.
        None => Action::Counteract { card: None },
    }
}

/// The `act` option index for a [反击] declaration: the offer whose label names
/// `card`, else the first non-fallback option, else the fallback.
fn counteract_option_index(p: &MatchPrompt, card: &str) -> i32 {
    if !card.is_empty() {
        if let Some(i) = p
            .options
            .iter()
            .position(|o| option_card_id(o).as_deref() == Some(card))
        {
            return i as i32;
        }
    }
    (0..p.options.len() as i32)
        .find(|&i| i != p.fallback)
        .unwrap_or(p.fallback)
}

fn prompt_actions(data: &GameData, st: &MatchState, p: &MatchPrompt, seat: usize) -> Vec<Action> {
    if !searchable_prompt(p) {
        return Vec::new();
    }
    let mut out = Vec::new();
    match p.kind.as_str() {
        "auction" => {
            out.push(Action::Bid { amount: -1 }); // pass
            // The engine refuses any positive bid from the current top
            // bidder (`err.already_top_bid`, `mod.rs` place_bid): when the
            // auction's `bidder` is us the only legal answer is pass.
            if p.bidder == seat as i32 {
                return out;
            }
            let min = if p.bid <= 0 { 100 } else { p.bid + 100 };
            let money = st.players.get(seat).map(|x| x.money).unwrap_or(0);
            // A few bid levels as fractions of the quoted worth / price
            // (`docs/BOT.md` §3.4). Own worth is hidden in `aiAnswer.worth`;
            // the quoted price is the public scale. The engine floors bids to
            // 100 (`place_bid`), so generate round levels only. The 3/4 level
            // and the cash margin are the seat's `StrategyParams`
            // (`docs/BOT.md` §3.8, defaults `750` / `1000` = the old literals).
            let params = game_core::strategy::for_seat(data, st, seat);
            let frac = params.bid_frac_milli as i64;
            let base = if p.tile >= 0 {
                purchase::quote_native(data, st, p.tile as usize).max(0)
            } else {
                st.buy_price.max(0)
            };
            for a in [
                min,
                min + 100,
                ((base as i64 * frac / 1000) as i32 / 100 * 100).max(min / 100 * 100),
                (base / 100 * 100).max(min / 100 * 100),
                money.saturating_sub(params.auction_cash_margin.max(0)),
            ] {
                let a = (a / 100 * 100).max(min);
                if a >= min && a <= money {
                    out.push(Action::Bid { amount: a });
                }
            }
            out.sort_by_key(|a| match a {
                Action::Bid { amount } => *amount,
                _ => 0,
            });
            out.dedup();
        }
        "mortgage" => {
            // Subsets via the heuristic's order (most valuable deeds first).
            let mut deeds: Vec<(usize, i32)> = p
                .items
                .iter()
                .filter_map(|s| s.parse::<usize>().ok())
                .filter(|&t| st.owners.get(t).copied().unwrap_or(-1) == seat as i32)
                .map(|t| {
                    let price = data.tiles.get(t).map(|x| x.price).unwrap_or(0);
                    (t, price)
                })
                .collect();
            deeds.sort_by_key(|&(_, price)| std::cmp::Reverse(price));
            let need = p.bid;
            let mut acc = Vec::new();
            let mut sum = 0;
            for (t, price) in deeds {
                acc.push(t);
                sum += price / 2;
                if sum >= need {
                    out.push(Action::Mortgage { tiles: acc.clone() });
                    if out.len() >= 3 {
                        break;
                    }
                }
            }
            // No subset reaches the need: offer nothing rather than a set the
            // engine would refuse (`err.mortgage_short`) -- the heuristic /
            // engine's auto-mortgage path handles a raise the deeds cannot
            // cover.
        }
        "tile" => {
            // Agent picks (and plain tile picks): one action per item + none.
            for i in 0..p.items.len() {
                out.push(Action::Pick { index: i as i32 });
            }
            out.push(Action::Pick {
                index: p.items.len() as i32,
            });
        }
        "pick" => {
            for i in 0..p.items.len() {
                out.push(Action::Pick { index: i as i32 });
            }
        }
        "choice" => {
            let title = p.title.k.as_ref();
            if title == "ask.counteract.title" {
                // [反击] offer: one declaration per offered card (the option
                // label names it -- `MatchPrompt::card` is the answered link,
                // not the offer), plus the fallback's skip.
                for i in 0..p.options.len() {
                    out.push(counteract_action(p, i as i32));
                }
                if out.is_empty() {
                    out.push(Action::Counteract { card: None });
                }
            } else {
                // buy / build / force-buy offers and other non-trivial choices.
                for i in 0..p.options.len() {
                    out.push(Action::Offer { index: i as i32 });
                }
            }
        }
        _ => {}
    }
    out
}

/// `BuyableHere` (autopilot.ts / `why_not_act`'s buy gate).
///
/// The shape half is re-checked here; the engine's own eligibility half --
/// the plan's no-buy flag, the quote's `BuyGate`, and the funds -- is
/// [`MatchState::can_buy_here`], computed inside `Match::state` from the same
/// predicates `why_not_act` uses. Never offer a Buy the engine would refuse
/// (`err.cannot_buy` / `err.buy_poor`).
pub fn buyable(data: &GameData, st: &MatchState, seat: usize, i: usize) -> bool {
    let Some(t) = data.tiles.get(i) else {
        return false;
    };
    matches!(t.kind.as_str(), "property" | "ring")
        && st.phase == "play"
        && st.step == stage::END
        && st.turn == seat as i32
        && !st.busy
        && !st.bought
        && st.landed == i as i32
        && st.players.get(seat).map(|p| p.pos) == Some(i as i32)
        && st.owners.get(i).copied().unwrap_or(-1) < 0
        && (st.can_buy_here || ab_on("BOT_STRENGTH_AB_NO_GATES"))
}

/// `CanBuildHere` (autopilot.ts). Gated on [`MatchState::can_build_here`] --
/// `why_not_build` (the plan's `can_build` flag, the NO_BUILD vetoes) plus the
/// funds half of `why_not_act`'s build branch.
pub fn can_build(data: &GameData, st: &MatchState, seat: usize, i: usize) -> bool {
    let Some(t) = data.tiles.get(i) else {
        return false;
    };
    t.kind == "property"
        && st.phase == "play"
        && st.step == stage::END
        && st.turn == seat as i32
        && !st.busy
        && !st.bought
        && !st.built
        && st.landed == i as i32
        && st.players.get(seat).map(|p| p.pos) == Some(i as i32)
        && st.owners.get(i).copied().unwrap_or(-1) == seat as i32
        && t.rent.len() > 1
        && !st.mortgaged.get(i).copied().unwrap_or(false)
        && st.houses.get(i).copied().unwrap_or(0) < t.rent.len() as i32 - 1
        && (st.can_build_here || ab_on("BOT_STRENGTH_AB_NO_GATES"))
}

/// Hand cards the engine would let `seat` play right now (from the view's
/// `playable` flags, parallel to `hand`).
pub fn playable_cards(hand: &[String], playable: &[bool]) -> Vec<String> {
    hand.iter()
        .enumerate()
        .filter(|(i, _)| playable.get(*i).copied().unwrap_or(true))
        .map(|(_, c)| c.clone())
        .collect()
}

/// Turn the abstracted action into the public `act` command.
pub fn to_net_message(action: &Action, st: &MatchState, seat: usize) -> game_core::net::NetMessage {
    use game_core::net::NetMessage;
    match action {
        Action::Play { card } => NetMessage {
            act: "play".into(),
            card: card.clone(),
            ..Default::default()
        },
        Action::Buy { tile } => NetMessage {
            act: "buy".into(),
            value: *tile as i32,
            ..Default::default()
        },
        Action::Build { tile } => NetMessage {
            act: "build".into(),
            value: *tile as i32,
            ..Default::default()
        },
        Action::Decline => NetMessage::act("end"),
        Action::Offer { index } => NetMessage {
            act: "answer".into(),
            prompt: st.prompt.id,
            value: *index,
            ..Default::default()
        },
        Action::Bid { amount } => NetMessage {
            act: "answer".into(),
            prompt: st.prompt.id,
            value: *amount,
            ..Default::default()
        },
        Action::Counteract { card } => {
            let p = &st.prompt;
            let value = match card {
                None => p.fallback,
                // The offer whose label names this card -- never "whichever
                // declaration happens to be first".
                Some(id) => counteract_option_index(p, id),
            };
            NetMessage {
                act: "answer".into(),
                prompt: p.id,
                value,
                cards: card.clone().into_iter().collect(),
                ..Default::default()
            }
        }
        Action::Pick { index } => {
            let p = &st.prompt;
            if p.kind == "pick" || p.kind == "mortgage" {
                // A single-item pick for the `pick` surface: the card list.
                let mut cards = Vec::new();
                if let Some(c) = p.items.get(*index as usize) {
                    cards.push(c.clone());
                }
                NetMessage {
                    act: "answer".into(),
                    prompt: p.id,
                    value: 0,
                    cards,
                    ..Default::default()
                }
            } else {
                NetMessage {
                    act: "answer".into(),
                    prompt: p.id,
                    value: *index,
                    ..Default::default()
                }
            }
        }
        Action::Mortgage { tiles } => NetMessage {
            act: "answer".into(),
            prompt: st.prompt.id,
            value: 0,
            cards: tiles.iter().map(|t| t.to_string()).collect(),
            ..Default::default()
        },
    }
    .with_seat(seat)
}

/// `NetMessage` has no seat field; the engine takes the seat from `act`'s
/// caller. This trait method keeps the call sites tidy.
trait WithSeat {
    fn with_seat(self, seat: usize) -> Self;
}

impl WithSeat for game_core::net::NetMessage {
    fn with_seat(self, _seat: usize) -> Self {
        self
    }
}

/// Progressive-bias prior per action, in `[0, 1]` (`docs/BOT-RESEARCH` #3).
///
/// Folds the engine's own heuristic signals -- the live prompt's `aiAnswer` /
/// `ai_picked`, the `wants_buy` / `wants_build` thresholds, and the bot-only
/// `estCost` -- into one preference number per abstracted action. The
/// heuristic's own choice gets `1.0`; alternatives get a kind-specific base
/// (buy / build follow the thresholds, card plays are cheaper = better, a
/// [反击] skip is the default). This is *search* guidance only -- never
/// legality, never the answer. `hand` / `est_cost` are parallel view columns.
pub fn action_priors(
    data: &GameData,
    st: &MatchState,
    hand: &[String],
    est_cost: &[i32],
    ai_answer: Option<&crate::view::AiAnswer>,
    seat: usize,
    actions: &[Action],
) -> Vec<f64> {
    // The seat's `StrategyParams` (`docs/BOT.md` §3.8): the prior constants
    // and the `wants_buy` / `wants_build` thresholds all read them. Defaults
    // are the old f64s (`800/1000.0 == 0.8` exactly).
    let params = game_core::strategy::for_seat(data, st, seat);
    let preferred = heuristic_preferred(data, st, ai_answer, seat, &params);
    let est_of = |card: &str| -> Option<i32> {
        hand.iter()
            .position(|c| c == card)
            .and_then(|i| est_cost.get(i).copied())
    };
    // Cheapest known estCost among the playable cards in this action list.
    let cheapest = actions
        .iter()
        .filter_map(|a| match a {
            Action::Play { card } => est_of(card),
            _ => None,
        })
        .filter(|&c| c > 0)
        .min();
    actions
        .iter()
        .map(|a| {
            if Some(a) == preferred.as_ref() {
                return 1.0;
            }
            match a {
                Action::Buy { tile } => {
                    let price = st.buy_price.max(0);
                    let money = st.players.get(seat).map(|p| p.money).unwrap_or(0);
                    let group = data.tiles.get(*tile).map(|x| x.group).unwrap_or(0);
                    if params.wants_buy_tile(money, price, group, st.round, false) {
                        params.prior_buy_yes()
                    } else {
                        params.prior_buy_no()
                    }
                }
                Action::Build { tile } => {
                    let cost = st.build_cost.max(0);
                    let money = st.players.get(seat).map(|p| p.money).unwrap_or(0);
                    let group = data.tiles.get(*tile).map(|x| x.group).unwrap_or(0);
                    let houses = st.houses.get(*tile).copied().unwrap_or(0);
                    if params.wants_build_tile(money, cost, group, houses) {
                        params.prior_build_yes()
                    } else {
                        params.prior_build_no()
                    }
                }
                Action::Decline => {
                    // The heuristic's default at 结束 when it would not buy or
                    // build; below a paid option when it would.
                    if preferred.is_none() {
                        1.0
                    } else {
                        params.prior_alt()
                    }
                }
                Action::Play { card } => {
                    // `ai_picked` / `aiAnswer.picked` marks the heuristic's
                    // preferred cards; cheaper execution cost is better.
                    if ai_answer
                        .map(|a| a.picked.iter().any(|p| p == card))
                        .unwrap_or(false)
                    {
                        return 1.0;
                    }
                    let mut base = params.prior_play_base();
                    match (est_of(card), cheapest) {
                        (Some(c), _) if c <= 0 => base += params.prior_play_bonus(),
                        (Some(c), Some(best)) if best > 0 => {
                            base += params.prior_play_bonus()
                                * (1.0 - (c as f64 / best.max(c) as f64));
                        }
                        _ => {}
                    }
                    base
                }
                Action::Offer { index } | Action::Pick { index } => {
                    if ai_answer.map(|a| a.answer) == Some(*index) {
                        1.0
                    } else {
                        params.prior_alt()
                    }
                }
                Action::Bid { amount } => {
                    // Closest to the heuristic's auction ceiling (`worth`).
                    let worth = ai_answer.map(|a| a.worth).unwrap_or(0);
                    if worth <= 0 {
                        return params.prior_bid_neutral();
                    }
                    if *amount == worth {
                        return 1.0;
                    }
                    if *amount < 0 {
                        return params.prior_alt(); // pass
                    }
                    let d = (*amount - worth).abs() as f64 / (worth.abs() as f64).max(1.0);
                    (1.0 - d).clamp(params.prior_bid_floor(), params.prior_bid_ceil())
                }
                Action::Counteract { card } => match card {
                    // Skip is the heuristic default unless aiAnswer declares.
                    None => {
                        if ai_answer.is_none()
                            || ai_answer.map(|a| a.answer) == Some(st.prompt.fallback)
                        {
                            params.prior_counter_skip()
                        } else {
                            params.prior_alt()
                        }
                    }
                    Some(_) => {
                        if ai_answer
                            .map(|a| a.picked.iter().any(|p| !p.is_empty()))
                            .unwrap_or(false)
                        {
                            1.0
                        } else {
                            params.prior_counter_declare()
                        }
                    }
                },
                Action::Mortgage { tiles } => {
                    let picked: Vec<String> = tiles.iter().map(|t| t.to_string()).collect();
                    if let Some(a) = ai_answer {
                        if a.picked == picked {
                            return 1.0;
                        }
                    }
                    params.prior_alt()
                }
            }
        })
        .collect()
}

/// The engine's own heuristic choice at this decision (the progressive-bias
/// anchor): `aiAnswer` mapped onto an abstracted action for prompts, the
/// `wants_buy` / `wants_build` thresholds for the turn surface.
fn heuristic_preferred(
    data: &GameData,
    st: &MatchState,
    ai_answer: Option<&crate::view::AiAnswer>,
    seat: usize,
    params: &game_core::strategy::StrategyParams,
) -> Option<Action> {
    if st.prompt.id > 0 && st.prompt.waiting(seat as i32) {
        let a = ai_answer?;
        // A [反击] offer maps through the per-option card label; the generic
        // path only sees `MatchPrompt::card` (the answered link).
        if st.prompt.title.k.as_ref() == "ask.counteract.title" {
            return Some(counteract_action(&st.prompt, a.answer));
        }
        return crate::sim::ai_answer_to_action(st, a, seat);
    }
    if st.step == stage::END {
        let t = st.landed;
        if t >= 0 {
            let t = t as usize;
            let money = st.players.get(seat).map(|p| p.money).unwrap_or(0);
            let group = data.tiles.get(t).map(|x| x.group).unwrap_or(0);
            let houses = st.houses.get(t).copied().unwrap_or(0);
            if buyable(data, st, seat, t)
                && params.wants_buy_tile(money, st.buy_price.max(0), group, st.round, false)
            {
                return Some(Action::Buy { tile: t });
            }
            if can_build(data, st, seat, t)
                && params.wants_build_tile(money, st.build_cost.max(0), group, houses)
            {
                return Some(Action::Build { tile: t });
            }
        }
        return Some(Action::Decline);
    }
    None
}

/// What to answer without searching (`docs/BOT.md` §3.4, "trivial decisions").
#[derive(Debug, Clone, PartialEq)]
pub enum Trivial {
    /// Exactly one legal action -- it is forced, so the search cannot improve
    /// on it. Answer it directly.
    Forced(Action),
    /// A low-stakes menu (see [`is_low_stakes`]); the engine's heuristic
    /// answers, same as an empty action list.
    LowStakes,
}

/// Should the search skip this root entirely? Conservative importance test
/// (`docs/BOT.md` §3.4):
///
/// * **one legal action** -- the choice is forced; searching only re-discovers
///   it. This alone is 27–34% of searched bot CPU (`bot_cpu` §6).
/// * **no money / ownership / card-cost consequence** -- every candidate is a
///   decline / auction pass / [反击] skip. The search cannot buy anything the
///   heuristic does not already know. (The "all priors identical" alternative
///   is deliberately *not* used on its own: two equal-prior card plays still
///   differ tactically, so it is only a skip when the menu is already free.)
///
/// Always keeps the full search for buys / builds / force-buy offers /
/// auctions with a real bid / [反击] declarations / card plays / agent picks /
/// tile choices / mortgages -- anything that moves money, ownership or cards.
pub fn trivial_decision(actions: &[Action]) -> Option<Trivial> {
    // A/B: `BOT_STRENGTH_AB_NO_FASTPATH` turns the whole skip off (the pre-C1
    // behaviour: every root with ≥1 action is searched);
    // `BOT_STRENGTH_AB_NO_LOWSTAKES` keeps only the forced single-action skip.
    if ab_on("BOT_STRENGTH_AB_NO_FASTPATH") {
        return match actions.len() {
            0 => Some(Trivial::LowStakes), // the empty list is not a search
            _ => None,
        };
    }
    match actions.len() {
        0 => Some(Trivial::LowStakes), // caller answers with the heuristic
        1 => Some(Trivial::Forced(actions[0].clone())),
        _ if !ab_on("BOT_STRENGTH_AB_NO_LOWSTAKES") && is_low_stakes(actions) => {
            Some(Trivial::LowStakes)
        }
        _ => None,
    }
}

/// Does this abstracted action move money, ownership, or spend a card?
fn has_stakes(a: &Action) -> bool {
    match a {
        Action::Decline => false,
        Action::Bid { amount } => *amount > 0,
        Action::Counteract { card: None } => false,
        Action::Counteract { card: Some(_) } => true,
        // Offers (force-buy / buy / build) commit money on the non-fallback
        // option; treat any offer as staked -- the fallback alone would have
        // been a 1-action root.
        Action::Offer { .. } => true,
        // Agent picks and tile choices are searched by design.
        Action::Pick { .. } => true,
        // A card play moves the board even at estCost 0.
        Action::Play { .. } => true,
        Action::Buy { .. } | Action::Build { .. } | Action::Mortgage { .. } => true,
    }
}

fn is_low_stakes(actions: &[Action]) -> bool {
    !actions.is_empty() && actions.iter().all(|a| !has_stakes(a))
}
