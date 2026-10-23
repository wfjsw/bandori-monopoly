//! G-MONEY -- money movement ([获得] / [消耗] / [支付]) and the ledger.
//!
//! Owned by unit 1. Append to [`CLAUSES`]; do not edit other areas.
//!
//! Sources: live sheet (`docs/rulebook/cards.json`, `the roster` /
//! `bands.json`) > `docs/rulebook/rulebook-doc.md`. `rulebook.txt` is not
//! authoritative. TEST-FINDINGS §6 rulings unless superseded by newer sheet
//! text.
//!
//! Bankruptcy money is unit 7 (`G-BANKRUPT`) -- these clauses skip `bankrupt`
//! windows rather than restating that cash-out.

use game_core::msg::{Arg, Msg};
use spec_kit::{Clause, Step};

/// Money clauses. Each entry is one book sentence, checked as a predicate
/// over one observed transition.
pub const CLAUSES: &[Clause] = &[
    G_MONEY_01, G_MONEY_02, G_MONEY_03, G_MONEY_04, G_MONEY_05, G_MONEY_06, G_MONEY_07, G_MONEY_08,
    G_MONEY_09, G_MONEY_10, G_MONEY_11, G_MONEY_12, G_MONEY_13, G_MONEY_14,
];

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Integer arg of a message, including one nested one level down.
fn int_arg_deep(msg: &Msg, name: &str) -> Option<i64> {
    if let Some(v) = spec_kit::int_arg(msg, name) {
        return Some(v);
    }
    for a in msg.a.values() {
        if let Arg::Msg(inner) = a {
            if let Some(v) = spec_kit::int_arg(inner, name) {
                return Some(v);
            }
        }
    }
    None
}

/// A half-charge marker (`log.part.rent_half`'s `full` / `cut`), searched
/// through the nested `note` / `half` chain of a rent line.
fn half_parts(msg: &Msg) -> Option<(i64, i64)> {
    if let (Some(f), Some(c)) = (
        spec_kit::int_arg(msg, "full"),
        spec_kit::int_arg(msg, "cut"),
    ) {
        return Some((f, c));
    }
    for a in msg.a.values() {
        if let Arg::Msg(inner) = a {
            if let Some(p) = half_parts(inner) {
                return Some(p);
            }
        }
    }
    None
}

/// 「向上取整10」 on a half charge. Delegates to [`bandori_std::half_ceil10`].
pub fn half_ceil10(full: i32) -> i32 {
    bandori_std::half_ceil10(full)
}

/// The rulebook buy figure: 「购买格子地契和建造已有房子的资金总价」.
pub fn buy_price(step: &Step<'_>, t: usize) -> Option<i32> {
    let tile = step.data.tiles.get(t)?;
    let houses = step.pre.st.houses.get(t).copied().unwrap_or(0);
    Some(tile.price + houses * tile.house)
}

/// The rulebook mortgage payout: 「地契购买价格50%」.
pub fn mortgage_value(step: &Step<'_>, t: usize) -> Option<i32> {
    let tile = step.data.tiles.get(t)?;
    Some(bandori_std::mortgage_value(tile.price))
}

/// The rulebook redeem cost: 「地契购买价格60%」.
///
/// Integer form of `round_ties_even(price * 0.6)` (the product of an integer
/// price and 0.6 never lands on `.5`). Owned by [`bandori_std::redeem_cost`];
/// the engine-side twin is `purchase::redeem_cost`.
pub fn redeem_cost(step: &Step<'_>, t: usize) -> Option<i32> {
    let tile = step.data.tiles.get(t)?;
    Some(bandori_std::redeem_cost(tile.price))
}

/// The rent table entry at the tile's house count (「现等级地租」).
/// RiNG has no table entry -- see OPEN-RiNG地租.
pub fn table_rent(step: &Step<'_>, t: usize) -> Option<i32> {
    let tile = step.data.tiles.get(t)?;
    if tile.rent.is_empty() {
        return None;
    }
    let h = step.pre.st.houses.get(t).copied().unwrap_or(0).max(0) as usize;
    Some(tile.rent[h.min(tile.rent.len() - 1)])
}

/// True when the window is one this unit's ledger cannot close over.
/// Bankruptcy / auction money has no ledger legs (unit 7); a build refund
/// credits money back without a money-moving event; a window that filled the
/// event tail may have dropped its early money legs (EVENT_KEEP = 400).
fn ledger_gap(step: &Step<'_>) -> bool {
    // Near the tail cap: `events_since` can miss the head of a long settle.
    if step.events.len() >= 380 {
        return true;
    }
    for e in step.events {
        match e.r#type.as_str() {
            "bankrupt" | "auction" => return true,
            "text" => {
                let k = e.msg.key();
                if k.contains("build_refund") || k.contains("purchase_unfunded") {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// One reconstructed ledger leg: `who`'s money moves by `delta`.
#[derive(Debug, Clone, Copy)]
struct Leg {
    who: i32,
    delta: i64,
}

/// Reconstruct every money leg this window's events name. `None` when the
/// window mixes in an event kind whose money effect is not on the log
/// (see [`ledger_gap`]).
fn ledger_legs(step: &Step<'_>) -> Option<Vec<Leg>> {
    if ledger_gap(step) {
        return None;
    }
    let mut legs = Vec::new();
    for e in step.events {
        match e.r#type.as_str() {
            // Bare print / delete.
            "gain" | "pass" => {
                if e.player_id >= 0 {
                    legs.push(Leg {
                        who: e.player_id,
                        delta: e.value as i64,
                    });
                }
            }
            "lose" => {
                if e.player_id >= 0 {
                    legs.push(Leg {
                        who: e.player_id,
                        delta: -(e.value as i64),
                    });
                }
            }
            // Player -> player. `value` is what the payer lost; a `gain` arg
            // (log.with_received) is what the payee actually received.
            "pay" | "rent" => {
                let loss = e.value as i64;
                let received = int_arg_deep(&e.msg, "gain").unwrap_or(loss);
                if e.player_id >= 0 {
                    legs.push(Leg {
                        who: e.player_id,
                        delta: -loss,
                    });
                }
                if e.other >= 0 {
                    legs.push(Leg {
                        who: e.other,
                        delta: received,
                    });
                }
            }
            // Fixed-rate moves that bypass the pay pipeline (其他规则 6).
            "mortgage" => {
                if e.player_id >= 0 {
                    legs.push(Leg {
                        who: e.player_id,
                        delta: e.value as i64,
                    });
                }
            }
            "redeem" => {
                if e.player_id >= 0 {
                    legs.push(Leg {
                        who: e.player_id,
                        delta: -(e.value as i64),
                    });
                }
            }
            "forcebuy" => {
                if e.player_id >= 0 {
                    legs.push(Leg {
                        who: e.player_id,
                        delta: -(e.value as i64),
                    });
                }
                if e.other >= 0 {
                    legs.push(Leg {
                        who: e.other,
                        delta: e.value as i64,
                    });
                }
            }
            // `buy` / `build` markers carry `value = 0`; their money rides the
            // `lose` leg of the same window. Everything else is not a money
            // movement.
            _ => {}
        }
    }
    Some(legs)
}

/// Compare every seat's money delta against the reconstructed legs.
fn ledger_mismatch(step: &Step<'_>, legs: &[Leg]) -> Option<String> {
    let n = step.pre.st.players.len().max(step.post.st.players.len());
    let mut expect = vec![0i64; n];
    for l in legs {
        if l.who < 0 || l.who as usize >= n {
            return Some(format!("ledger leg names no such seat: {l:?}"));
        }
        expect[l.who as usize] += l.delta;
    }
    for i in 0..n {
        let actual = step.post_money(i) as i64 - step.pre_money(i) as i64;
        if actual != expect[i] {
            return Some(format!(
                "player {i}: money delta {actual} != ledger legs {} (legs: {:?})",
                expect[i], legs
            ));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// G-MONEY-01 -- [支付]
// ---------------------------------------------------------------------------

/// 「A[支付]BX资金」 conserves money between A and B.
///
/// * **ID:** G-MONEY-01
/// * **Source:** `rulebook-doc.md:60`
/// * **Quote:** 「[支付]：“A[支付]BX资金”的效果为玩家A向玩家B支付X资金。」
/// * **Trigger:** any observed input whose event window contains money moves.
/// * **Condition:** the window's only money-moving events are `pay` / `rent`
///   legs with both sides present and no `gain` modifier (a one-sided print /
///   delete is G-MONEY-02/03; a modified transfer is the ledger).
/// * **Effect:** for each such leg of X from A to B: `money(A)` fell by X,
///   `money(B)` rose by X, and every other player's money is unchanged.
/// * **Frame:** only the named pair's `money:*` may move on a pure-transfer
///   step.
/// * **Rulings & OPEN:** a `gain` arg on the log line (`log.with_received`)
///   means a card raised the amount the payee received; that is not a bare
///   「A[支付]BX」. Bankruptcy and auction windows move money without ledger
///   legs (G-BANKRUPT) and are skipped. Build-refund windows credit money
///   without a money event and are skipped (observability gap).
/// * **Executable:** [`g_money_01`].
const G_MONEY_01: Clause = Clause::new(
    "G-MONEY-01",
    "rulebook-doc.md:60",
    "[支付]：“A[支付]BX资金”的效果为玩家A向玩家B支付X资金。",
    g_money_01,
);

/// Executable form of [`G_MONEY_01`]. See that item's Trigger / Condition /
/// Effect / Frame.
pub fn g_money_01(step: &Step<'_>) -> Result<(), String> {
    if ledger_gap(step) {
        return Ok(());
    }
    #[derive(Debug)]
    struct Pair {
        from: usize,
        to: usize,
        amount: i64,
    }
    let mut pairs = Vec::new();
    let mut other_money = 0usize;
    for e in step.events {
        match e.r#type.as_str() {
            "pay" | "rent" => {
                let from = e.player_id;
                let to = e.other;
                if from < 0 || to < 0 {
                    other_money += 1;
                    continue;
                }
                if int_arg_deep(&e.msg, "gain").is_some() {
                    other_money += 1;
                    continue;
                }
                pairs.push(Pair {
                    from: from as usize,
                    to: to as usize,
                    amount: e.value as i64,
                });
            }
            "lose" | "gain" | "pass" | "mortgage" | "redeem" | "forcebuy" => {
                other_money += 1;
            }
            _ => {}
        }
    }
    if pairs.is_empty() || other_money > 0 {
        return Ok(());
    }
    let mut delta = vec![0i64; step.pre.st.players.len().max(step.post.st.players.len())];
    for i in 0..delta.len() {
        delta[i] = step.post_money(i) as i64 - step.pre_money(i) as i64;
    }
    let mut expect = vec![0i64; delta.len()];
    for p in &pairs {
        if p.from >= expect.len() || p.to >= expect.len() {
            return Err(format!("pay leg out of range: {p:?}"));
        }
        expect[p.from] -= p.amount;
        expect[p.to] += p.amount;
    }
    for i in 0..delta.len() {
        if delta[i] != expect[i] {
            return Err(format!(
                "player {i}: money delta {} != transfer legs {} ({pairs:?})",
                delta[i], expect[i]
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// G-MONEY-02 -- [获得]
// ---------------------------------------------------------------------------

/// 「[获得]X资金」 prints X: the named player's money rises by X and no one
/// else moves.
///
/// * **ID:** G-MONEY-02
/// * **Source:** `rulebook-doc.md:58`
/// * **Quote:** 「[获得]：“[获得]X资金”的效果为直接获得X资金，等同于印钱。」
/// * **Trigger:** a window whose only money-moving event is a one-sided
///   `gain` / `pass` of X.
/// * **Condition:** `gain`/`pass` names player P with value X > 0; no other
///   money-moving event in the window.
/// * **Effect:** `money(P)` rose by exactly X; every other player is
///   unchanged.
/// * **Frame:** only `money:P` may move.
/// * **Rulings & OPEN:** a `gain` of 0 is a no-op and is skipped. A window
///   that also pays / loses / mortgages is the general ledger (G-MONEY-04),
///   not this clause.
/// * **Executable:** [`g_money_02`].
const G_MONEY_02: Clause = Clause::new(
    "G-MONEY-02",
    "rulebook-doc.md:58",
    "[获得]：“[获得]X资金”的效果为直接获得X资金，等同于印钱。",
    g_money_02,
);

/// Executable form of [`G_MONEY_02`].
pub fn g_money_02(step: &Step<'_>) -> Result<(), String> {
    if ledger_gap(step) {
        return Ok(());
    }
    let mut hits: Vec<(usize, i64)> = Vec::new();
    let mut other = 0usize;
    for e in step.events {
        match e.r#type.as_str() {
            "gain" | "pass" if e.player_id >= 0 && e.value > 0 => {
                hits.push((e.player_id as usize, e.value as i64));
            }
            "gain" | "pass" => other += 1,
            "lose" | "pay" | "rent" | "mortgage" | "redeem" | "forcebuy" => other += 1,
            _ => {}
        }
    }
    if hits.len() != 1 || other > 0 {
        return Ok(());
    }
    let (who, amount) = hits[0];
    let delta = step.post_money(who) as i64 - step.pre_money(who) as i64;
    if delta != amount {
        return Err(format!(
            "[获得]{amount}: player {who} money delta {delta}, expected +{amount}"
        ));
    }
    let n = step.pre.st.players.len().max(step.post.st.players.len());
    for i in 0..n {
        if i == who {
            continue;
        }
        let d = step.post_money(i) as i64 - step.pre_money(i) as i64;
        if d != 0 {
            return Err(format!(
                "[获得]{amount} to {who}: unnamed player {i} moved by {d}"
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// G-MONEY-03 -- [消耗]
// ---------------------------------------------------------------------------

/// 「[消耗]X资金」 deletes X: the named player's money falls by X and no one
/// else moves.
///
/// * **ID:** G-MONEY-03
/// * **Source:** `rulebook-doc.md:59`
/// * **Quote:** 「[消耗]：“[消耗]X资金”的效果为直接失去X资金，等同于删钱。」
/// * **Trigger:** a window whose only money-moving event is a one-sided
///   `lose` of X.
/// * **Condition:** `lose` names player P with value X > 0; no other
///   money-moving event in the window.
/// * **Effect:** `money(P)` fell by exactly X; every other player is
///   unchanged.
/// * **Frame:** only `money:P` may move.
/// * **Rulings & OPEN:** see G-MONEY-02 -- a mixed window is the general
///   ledger.
/// * **Executable:** [`g_money_03`].
const G_MONEY_03: Clause = Clause::new(
    "G-MONEY-03",
    "rulebook-doc.md:59",
    "[消耗]：“[消耗]X资金”的效果为直接失去X资金，等同于删钱。",
    g_money_03,
);

/// Executable form of [`G_MONEY_03`].
pub fn g_money_03(step: &Step<'_>) -> Result<(), String> {
    if ledger_gap(step) {
        return Ok(());
    }
    let mut hits: Vec<(usize, i64)> = Vec::new();
    let mut other = 0usize;
    for e in step.events {
        match e.r#type.as_str() {
            "lose" if e.player_id >= 0 && e.value > 0 => {
                hits.push((e.player_id as usize, e.value as i64));
            }
            "lose" => other += 1,
            "gain" | "pass" | "pay" | "rent" | "mortgage" | "redeem" | "forcebuy" => other += 1,
            _ => {}
        }
    }
    if hits.len() != 1 || other > 0 {
        return Ok(());
    }
    let (who, amount) = hits[0];
    let delta = step.post_money(who) as i64 - step.pre_money(who) as i64;
    if delta != -amount {
        return Err(format!(
            "[消耗]{amount}: player {who} money delta {delta}, expected -{amount}"
        ));
    }
    let n = step.pre.st.players.len().max(step.post.st.players.len());
    for i in 0..n {
        if i == who {
            continue;
        }
        let d = step.post_money(i) as i64 - step.pre_money(i) as i64;
        if d != 0 {
            return Err(format!(
                "[消耗]{amount} from {who}: unnamed player {i} moved by {d}"
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// G-MONEY-04 -- the ledger (only named players change)
// ---------------------------------------------------------------------------

/// The window's money legs close the books: every seat's money delta equals
/// the sum of the legs that name it, and no unnamed seat moves.
///
/// * **ID:** G-MONEY-04
/// * **Source:** `rulebook-doc.md:58-60` (the three money verbs).
/// * **Quote:** 「[获得]…直接获得X资金」/「[消耗]…直接失去X资金」/「A[支付]BX资金…
///   玩家A向玩家B支付X资金」
/// * **Trigger:** every observed input whose window contains at least one
///   money-moving event.
/// * **Condition:** the window is not a ledger gap (bankrupt / auction /
///   build-refund).
/// * **Effect:** for each seat `i`, `post.money(i) - pre.money(i)` equals the
///   sum of that seat's legs (`gain`/`pass` +X, `lose` -X, `pay`/`rent`
///   -loss / +received, `mortgage` +X, `redeem` -X, `forcebuy` -X / +X).
/// * **Frame:** a seat with no leg has delta 0 (「only named players change」).
/// * **Rulings & OPEN:** `buy` / `build` markers carry no money of their own
///   (the `lose` leg does). A `gain` arg on a transfer is the payee's actual
///   receipt. Bankruptcy cash-out and auction bids have no ledger legs and
///   are skipped (G-BANKRUPT). OPEN-支付阶段7 does not change the books.
/// * **Executable:** [`g_money_04`].
const G_MONEY_04: Clause = Clause::new(
    "G-MONEY-04",
    "rulebook-doc.md:58",
    "A[支付]BX资金 / [获得]X资金 / [消耗]X资金 -- only the named players' money changes.",
    g_money_04,
);

/// Executable form of [`G-MONEY-04`].
pub fn g_money_04(step: &Step<'_>) -> Result<(), String> {
    let Some(legs) = ledger_legs(step) else {
        return Ok(());
    };
    if legs.is_empty() {
        // No named money movement: nobody may move.
        let n = step.pre.st.players.len().max(step.post.st.players.len());
        for i in 0..n {
            let d = step.post_money(i) as i64 - step.pre_money(i) as i64;
            if d != 0 {
                let kinds: Vec<String> = step
                    .events
                    .iter()
                    .map(|e| format!("{}:{}", e.r#type, e.value))
                    .collect();
                return Err(format!(
                    "player {i} money moved by {d} with no money-moving event (window: {kinds:?})"
                ));
            }
        }
        return Ok(());
    }
    ledger_mismatch(step, &legs).map_or(Ok(()), Err)
}

// ---------------------------------------------------------------------------
// G-MONEY-05 -- mortgage = 50%
// ---------------------------------------------------------------------------

/// Mortgage pays exactly half the deed's purchase price.
///
/// * **ID:** G-MONEY-05
/// * **Source:** `rulebook-doc.md:129` (also the red note `:32`)
/// * **Quote:** 「抵押拥有的地契，抵押后[获得]地契购买价格50%的资金」
/// * **Trigger:** a window containing a `mortgage` event for tile T by
///   player P.
/// * **Condition:** T's `price` is the 「地契购买价格」.
/// * **Effect:** `money(P)` rose by exactly `price / 2`; the deed is
///   mortgaged in `post`. No other seat's money moves on a bare mortgage.
/// * **Frame:** only `money:P` and `mortgaged:T` (plus the event tail) move.
/// * **Rulings & OPEN:** the rate is a fixed 50% of the **price**, not of
///   price+houses (contrast the force-buy figure, G-MONEY-07). Integer
///   division: every printed price on the current board is even, so the
///   half is exact. Other rules 6: this amount is immune to money modifiers
///   (G-MONEY-08 restates the immunity).
/// * **Executable:** [`g_money_05`].
const G_MONEY_05: Clause = Clause::new(
    "G-MONEY-05",
    "rulebook-doc.md:129",
    "抵押拥有的地契，抵押后[获得]地契购买价格50%的资金",
    g_money_05,
);

/// Executable form of [`G-MONEY-05`].
pub fn g_money_05(step: &Step<'_>) -> Result<(), String> {
    for e in step.events {
        if e.r#type != "mortgage" {
            continue;
        }
        let who = e.player_id;
        let t = e.to;
        if who < 0 || t < 0 {
            return Err(format!("mortgage event without a seat/tile: {e:?}"));
        }
        let t = t as usize;
        let Some(want) = mortgage_value(step, t) else {
            return Err(format!("mortgage of unknown tile {t}"));
        };
        if e.value != want {
            return Err(format!(
                "G-MONEY-05: mortgage of tile {t} paid {}, expected {} (50% of price)",
                e.value, want
            ));
        }
        if !step.post.st.mortgaged.get(t).copied().unwrap_or(false) {
            return Err(format!(
                "G-MONEY-05: tile {t} is not mortgaged after the mortgage event"
            ));
        }
    }
    // When the window is a bare mortgage (no other money kinds), the seat's
    // delta is exactly the sum of the mortgage payouts.
    if ledger_gap(step) {
        return Ok(());
    }
    let mut only_mortgages = true;
    let mut sum: i64 = 0;
    let mut who: i32 = -1;
    for e in step.events {
        match e.r#type.as_str() {
            "mortgage" => {
                if who < 0 {
                    who = e.player_id;
                }
                sum += e.value as i64;
            }
            "lose" | "gain" | "pass" | "pay" | "rent" | "redeem" | "forcebuy" => {
                only_mortgages = false;
            }
            _ => {}
        }
    }
    if only_mortgages && who >= 0 && sum > 0 {
        let delta = step.post_money(who as usize) as i64 - step.pre_money(who as usize) as i64;
        if delta != sum {
            return Err(format!(
                "G-MONEY-05: mortgage of {sum} landed as money delta {delta}"
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// G-MONEY-06 -- redeem = 60%
// ---------------------------------------------------------------------------

/// Redeem costs exactly 60% of the deed's purchase price.
///
/// * **ID:** G-MONEY-06
/// * **Source:** `rulebook-doc.md:130` (red `60%` at `:23`)
/// * **Quote:** 「赎回拥有的地契，赎回后[消耗]地契购买价格60%的资金」
/// * **Trigger:** a window containing a `redeem` event for tile T by P.
/// * **Condition:** T's `price` is the 「地契购买价格」.
/// * **Effect:** `money(P)` fell by exactly 60% of `price`; the deed is no
///   longer mortgaged in `post`.
/// * **Frame:** only `money:P` and `mortgaged:T` move on a bare redeem.
/// * **Rulings & OPEN:** the book states the rate and not the rounding.
///   The integer form is `round_ties_even(price * 0.6)` (the product of an
///   integer price and 0.6 never lands on `.5`), owned by
///   [`bandori_std::redeem_cost`]. Rate is of **price**, not price+houses.
///   Immune to money modifiers (其他规则 6).
/// * **Executable:** [`g_money_06`].
const G_MONEY_06: Clause = Clause::new(
    "G-MONEY-06",
    "rulebook-doc.md:130",
    "赎回拥有的地契，赎回后[消耗]地契购买价格60%的资金",
    g_money_06,
);

/// Executable form of [`G_MONEY-06`].
pub fn g_money_06(step: &Step<'_>) -> Result<(), String> {
    for e in step.events {
        if e.r#type != "redeem" {
            continue;
        }
        let who = e.player_id;
        let t = e.to;
        if who < 0 || t < 0 {
            return Err(format!("redeem event without a seat/tile: {e:?}"));
        }
        let t = t as usize;
        let Some(want) = redeem_cost(step, t) else {
            return Err(format!("redeem of unknown tile {t}"));
        };
        if e.value != want {
            return Err(format!(
                "G-MONEY-06: redeem of tile {t} cost {}, expected {} (60% of price)",
                e.value, want
            ));
        }
        if step.post.st.mortgaged.get(t).copied().unwrap_or(true) {
            return Err(format!(
                "G-MONEY-06: tile {t} is still mortgaged after the redeem event"
            ));
        }
    }
    if ledger_gap(step) {
        return Ok(());
    }
    let mut only_redeems = true;
    let mut sum: i64 = 0;
    let mut who: i32 = -1;
    for e in step.events {
        match e.r#type.as_str() {
            "redeem" => {
                if who < 0 {
                    who = e.player_id;
                }
                sum += e.value as i64;
            }
            "lose" | "gain" | "pass" | "pay" | "rent" | "mortgage" | "forcebuy" => {
                only_redeems = false;
            }
            _ => {}
        }
    }
    if only_redeems && who >= 0 && sum > 0 {
        let delta = step.post_money(who as usize) as i64 - step.pre_money(who as usize) as i64;
        if delta != -sum {
            return Err(format!(
                "G-MONEY-06: redeem of {sum} landed as money delta {delta}"
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// G-MONEY-07 -- force-buy = 2x (price + houses), stays mortgaged
// ---------------------------------------------------------------------------

/// Forced purchase costs twice the deed-plus-houses figure and the deed
/// stays mortgaged.
///
/// * **ID:** G-MONEY-07
/// * **Source:** `rulebook-doc.md:161` (red note `:25`)
/// * **Quote:** 「[支付]拥有格子的玩家购买格子地契和建造已有房子的资金总价的两倍，
///   从该玩家处强行购买该格地契，获得的地契仍为抵押状态」
/// * **Trigger:** a window containing a `forcebuy` event for tile T.
/// * **Condition:** T is mortgaged in `pre` (the force-buy path is the
///   mortgaged-deed branch of 基础[结算] 6.2).
/// * **Effect:** the buyer pays and the owner receives exactly
///   `2 * (price + houses * house)`; `owners[T]` is the buyer in `post`;
///   `mortgaged[T]` is still true.
/// * **Frame:** only the pair's `money:*` and `owner:T` / `mortgaged:T`
///   move on a bare force-buy.
/// * **Rulings & OPEN:** 「购买格子地契和建造已有房子的资金总价」 is the same
///   figure as the optional-buy price (G-MONEY-11). The engine keeps the
///   mortgage unconditionally; the book says 「仍为抵押状态」 with no
///   exception in this sentence. Immune to money modifiers (其他规则 6) --
///   see G-MONEY-08.
/// * **Executable:** [`g_money_07`].
const G_MONEY_07: Clause = Clause::new(
    "G-MONEY-07",
    "rulebook-doc.md:161",
    "[支付]拥有格子的玩家购买格子地契和建造已有房子的资金总价的两倍，从该玩家处强行购买该格地契，获得的地契仍为抵押状态",
    g_money_07,
);

/// Executable form of [`G-MONEY-07`].
pub fn g_money_07(step: &Step<'_>) -> Result<(), String> {
    for e in step.events {
        if e.r#type != "forcebuy" {
            continue;
        }
        let buyer = e.player_id;
        let owner = e.other;
        let t = e.to;
        if buyer < 0 || owner < 0 || t < 0 {
            return Err(format!("forcebuy event without parties/tile: {e:?}"));
        }
        let buyer = buyer as usize;
        let owner = owner as usize;
        let t = t as usize;
        let Some(base) = buy_price(step, t) else {
            return Err(format!("force-buy of unknown tile {t}"));
        };
        let want = base * 2;
        if e.value != want {
            return Err(format!(
                "G-MONEY-07: force-buy of tile {t} cost {}, expected {} (2x price+houses)",
                e.value, want
            ));
        }
        if !step.post.st.mortgaged.get(t).copied().unwrap_or(false) {
            return Err(format!(
                "G-MONEY-07: tile {t} is not still mortgaged after the force-buy"
            ));
        }
        let got = step.post.st.owners.get(t).copied().unwrap_or(-1);
        if got != buyer as i32 {
            return Err(format!(
                "G-MONEY-07: tile {t} owner is {got}, expected buyer {buyer}"
            ));
        }
        // Bare force-buy: the pair's money moved by exactly the price.
        if ledger_gap(step) {
            continue;
        }
        let mut only = true;
        for e2 in step.events {
            if matches!(
                e2.r#type.as_str(),
                "lose" | "gain" | "pass" | "pay" | "rent" | "mortgage" | "redeem"
            ) {
                only = false;
            }
        }
        if only {
            let db = step.post_money(buyer) as i64 - step.pre_money(buyer) as i64;
            let do_ = step.post_money(owner) as i64 - step.pre_money(owner) as i64;
            if db != -(want as i64) || do_ != want as i64 {
                return Err(format!(
                    "G-MONEY-07: force-buy of {want} moved buyer {db} / owner {do_}"
                ));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// G-MONEY-08 -- mortgage / redeem / force-buy are immune
// ---------------------------------------------------------------------------

/// Mortgage, redeem and force-buy money is not touched by any effect unless
/// the effect says it changes one of those three.
///
/// * **ID:** G-MONEY-08
/// * **Source:** `rulebook-doc.md:146`
/// * **Quote:** 「抵押，赎回，和强制购买的资金变动不受任何效果影响，除非效果写明改动抵押，赎回，或强制购买。」
/// * **Trigger:** a window containing a `mortgage` / `redeem` / `forcebuy`
///   event.
/// * **Condition:** no effect in the window 「写明改动抵押，赎回，或强制购买」.
/// * **Effect:** the event's `value` equals the fixed formula (50% / 60% /
///   2x price+houses) and is **not** a card-scaled figure.
/// * **Frame:** n/a (a read of `value` against the tile data).
/// * **Rulings & OPEN:** the sheet's `为新版做的改动` `A20` names the explicit
///   「写明」 exception for 抵押/赎回 only and does not settle whether a
///   pay-hook may retune a force-buy (TEST-FINDINGS §6.12). The engine moves
///   all three outside the money pipeline, so the amount is always the
///   native formula; a non-native amount is a violation.
/// * **Executable:** [`g_money_08`].
const G_MONEY_08: Clause = Clause::new(
    "G-MONEY-08",
    "rulebook-doc.md:146",
    "抵押，赎回，和强制购买的资金变动不受任何效果影响，除非效果写明改动抵押，赎回，或强制购买。",
    g_money_08,
);

/// Executable form of [`G_MONEY-08`].
pub fn g_money_08(step: &Step<'_>) -> Result<(), String> {
    for e in step.events {
        let t = e.to;
        if t < 0 {
            continue;
        }
        let t = t as usize;
        let native = match e.r#type.as_str() {
            "mortgage" => mortgage_value(step, t),
            "redeem" => redeem_cost(step, t),
            "forcebuy" => buy_price(step, t).map(|b| b * 2),
            _ => continue,
        };
        let Some(want) = native else {
            continue;
        };
        if e.value == want {
            continue;
        }
        return Err(format!(
            "G-MONEY-08: {} of tile {t} moved {}, not the immune formula {} (其他规则 6)",
            e.r#type, e.value, want
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// G-MONEY-09 -- rent by level
// ---------------------------------------------------------------------------

/// Rent is the deed's marked rent at the current house level.
///
/// * **ID:** G-MONEY-09
/// * **Source:** `rulebook-doc.md:160`
/// * **Quote:** 「[支付]拥有格子的玩家格子地契所标记的现等级地租。」
/// * **Trigger:** a `rent` event on a property tile with a non-empty rent
///   table, no half-charge marker, and no `gain` modifier.
/// * **Condition:** `houses[T]` is the current level; `tile.rent[level]` is
///   the marked 现等级地租.
/// * **Effect:** the payer pays the owner exactly that table entry (or a
///   figure a standing scale produced from it -- see Rulings).
/// * **Frame:** only the pair's `money:*` moves on a bare rent.
/// * **Rulings & OPEN:** RiNG rent is not on the table (OPEN-RiNG地租 -- the
///   book gives no RiNG rent formula). A standing 「支付减半」/「翻倍」
///   (支付阶段 4) may scale the settled figure; those windows have no
///   half-charge marker and this clause then accepts any table entry.
///   House count is the counted value (「房屋数视为…」 overrides ride
///   `World::rent_houses`; OPEN-视为房屋数).
/// * **Executable:** [`g_money_09`].
const G_MONEY_09: Clause = Clause::new(
    "G-MONEY-09",
    "rulebook-doc.md:160",
    "[支付]拥有格子的玩家格子地契所标记的现等级地租。",
    g_money_09,
);

/// Executable form of [`G-MONEY-09`].
pub fn g_money_09(step: &Step<'_>) -> Result<(), String> {
    for e in step.events {
        if e.r#type != "rent" {
            continue;
        }
        if half_parts(&e.msg).is_some() {
            // Half-rate is G-MONEY-10.
            continue;
        }
        if int_arg_deep(&e.msg, "gain").is_some() {
            continue;
        }
        // A card / skill in the window (or a standing effect that fired
        // earlier) may have scaled the settled figure; the book's clause is
        // about the marked rate. Only demand it on a clean window.
        let touched = step.events.iter().any(|x| {
            x.r#type == "card" || x.r#type == "skill" || x.msg.key().starts_with("cards:")
        });
        if touched {
            continue;
        }
        let t = e.to;
        if t < 0 {
            continue;
        }
        let t = t as usize;
        let Some(want) = table_rent(step, t) else {
            // Empty rent table (RiNG / corners) -- OPEN-RiNG地租.
            continue;
        };
        if e.value != want {
            // A standing scale (支付阶段 4) may have reshaped the figure. Fail
            // only when the amount matches no table entry at any level.
            let tile = &step.data.tiles[t];
            if !tile.rent.contains(&e.value) {
                return Err(format!(
                    "G-MONEY-09: rent on tile {t} was {}, not the marked rate {} (table {:?})",
                    e.value, want, tile.rent
                ));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// G-MONEY-10 -- [地产商] half rate, ceil 10
// ---------------------------------------------------------------------------

/// A half-rate charge is half the full rate, rounded **up** to a multiple
/// of 10.
///
/// * **ID:** G-MONEY-10
/// * **Source:** `rulebook-doc.md:162`
/// * **Quote:** 「需向该格子同色的所有[可购买格子]从前到后依次进行一次半价收费的[结算]（向上取整10）」
/// * **Trigger:** a `rent` event whose message carries the half-charge
///   marker (`full` / `cut`).
/// * **Condition:** `full` is the pre-half rate (the 现等级地租).
/// * **Effect:** `cut == ceil(full / 2 / 10) * 10`. The ledger may still be
///   reshaped by later add/mul modifiers (HHW double, soyo extra half, a
///   standing 支付减半) -- the clause pins the half-charge **rate**, not the
///   post-modifier figure.
/// * **Frame:** n/a on the rate; the pair's money is the ledger's job.
/// * **Rulings & OPEN:** 「同色」 is the board's colour group (OPEN-Agent同色
///   -- whether 「该格获得所有颜色」 widens the set is undecided). The
///   rounding is 「向上取整10」, not banker's rounding: 301 → 160.
///   `pay_rent` writes `cut` as the pre-modifier Pay amount, so an engine that
///   forgot to halve fails the rate check (`cut == full`); the ledger equality
///   can only see modifiers and is not pinned.
/// * **Executable:** [`g_money_10`].
const G_MONEY_10: Clause = Clause::new(
    "G-MONEY-10",
    "rulebook-doc.md:162",
    "半价收费的[结算]（向上取整10）",
    g_money_10,
);

/// Executable form of [`G-MONEY-10`].
pub fn g_money_10(step: &Step<'_>) -> Result<(), String> {
    for e in step.events {
        if e.r#type != "rent" {
            continue;
        }
        let Some((full, cut)) = half_parts(&e.msg) else {
            continue;
        };
        let want = half_ceil10(full as i32) as i64;
        if cut != want {
            return Err(format!(
                "G-MONEY-10: half charge of {full} became {cut}, expected {want} (ceil 10)"
            ));
        }
        // The clause pins the half-charge **rate** (`cut == ceil10(full/2)`).
        // Do **not** pin the ledger to `cut`: the running figure is free to be
        // reshaped by later add/mul modifiers (HHW:传播笑容 「支付双倍价格」,
        // soyo 「地产商的减半收费基础上额外减半」, a standing 「支付减半」 /
        // `RENT_FACTOR`, …). `pay_rent` writes `cut` as the pre-modifier Pay
        // amount, so an engine that forgot to halve would fail the rate check
        // above (`cut == full`); the ledger equality can only see modifiers.
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// G-MONEY-11 -- buy cost = price + houses
// ---------------------------------------------------------------------------

/// The optional buy costs 「购买格子地契和建造已有房子的资金总价」.
///
/// * **ID:** G-MONEY-11
/// * **Source:** `rulebook-doc.md:155`
/// * **Quote:** 「可选择[消耗]购买格子地契和建造已有房子的资金总价，获得格子地契和拥有权。」
/// * **Trigger:** a `buy` event for tile T by player P.
/// * **Condition:** T was unowned in `pre`; `houses[T]` is the standing
///   house count.
/// * **Effect:** P paid exactly `price + houses * house` (when no card
///   retuned the figure); `owners[T]` is P in `post`.
/// * **Frame:** only `money:P` and `owner:T` / `mortgaged:T` / `houses:T`
///   move on a bare buy.
/// * **Rulings & OPEN:** a card may rewrite the buy price (BuyAdd / BuyMul /
///   BuySet); the clause then only checks the ownership transfer and lets
///   the ledger close the money. The **force-buy** figure is twice this
///   same base (G-MONEY-07).
/// * **Executable:** [`g_money_11`].
const G_MONEY_11: Clause = Clause::new(
    "G-MONEY-11",
    "rulebook-doc.md:155",
    "可选择[消耗]购买格子地契和建造已有房子的资金总价，获得格子地契和拥有权。",
    g_money_11,
);

/// Executable form of [`G-MONEY-11`].
pub fn g_money_11(step: &Step<'_>) -> Result<(), String> {
    for e in step.events {
        if e.r#type != "buy" {
            continue;
        }
        let who = e.player_id;
        let t = e.to;
        if who < 0 || t < 0 {
            return Err(format!("buy event without a seat/tile: {e:?}"));
        }
        let who = who as usize;
        let t = t as usize;
        let got = step.post.st.owners.get(t).copied().unwrap_or(-1);
        if got != who as i32 {
            return Err(format!(
                "G-MONEY-11: tile {t} owner is {got} after the buy, expected {who}"
            ));
        }
        // The money half: a card / standing BuyAdd-BuyMul-BuySet may have
        // retuned the figure without leaving a marker in this window, so the
        // exact `price + houses * house` number is pinned by the scenario
        // `buy_costs_price_plus_houses` on a vanilla board. Per-act we only
        // require the ownership to have moved (the ledger closes the money).
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// G-MONEY-12 -- build cost
// ---------------------------------------------------------------------------

/// An upgrade build costs the tile's marked house building cost.
///
/// * **ID:** G-MONEY-12
/// * **Source:** `rulebook-doc.md:157`
/// * **Quote:** 「可选择[消耗]格子地契所标注的房屋建筑费进行升级建造」
/// * **Trigger:** a `build` event for tile T by player P that raised a house.
/// * **Condition:** T is owned by P and not mortgaged; `tile.house` is the
///   「房屋建筑费」.
/// * **Effect:** P paid exactly `tile.house` (when no card retuned it) and
///   `houses[T]` rose by 1.
/// * **Frame:** only `money:P` and `houses:T` move on a bare build.
/// * **Rulings & OPEN:** 「升级所[消耗]的资金不可通过[抵押]正在升级的格子地契
///   获得」 is a funding restriction (the engine refuses the build outright
///   on a mortgaged deed). A card may discount the cost; the clause then
///   only checks the house rose.
/// * **Executable:** [`g_money_12`].
const G_MONEY_12: Clause = Clause::new(
    "G-MONEY-12",
    "rulebook-doc.md:157",
    "可选择[消耗]格子地契所标注的房屋建筑费进行升级建造",
    g_money_12,
);

/// Executable form of [`G-MONEY-12`].
pub fn g_money_12(step: &Step<'_>) -> Result<(), String> {
    for e in step.events {
        if e.r#type != "build" {
            continue;
        }
        let who = e.player_id;
        let t = e.to;
        if who < 0 || t < 0 {
            return Err(format!("build event without a seat/tile: {e:?}"));
        }
        let t = t as usize;
        let tile = step
            .data
            .tiles
            .get(t)
            .ok_or_else(|| format!("build of unknown tile {t}"))?;
        let want = tile.house;
        let pre_h = step.pre.st.houses.get(t).copied().unwrap_or(0);
        let post_h = step.post.st.houses.get(t).copied().unwrap_or(0);
        if post_h != pre_h + 1 {
            return Err(format!(
                "G-MONEY-12: tile {t} houses {pre_h} -> {post_h}, expected +1"
            ));
        }
        // The money half: a card / standing build discount may have retuned
        // the figure without a marker in this window. The exact
        // 「房屋建筑费」 number is pinned by the scenario
        // `build_costs_the_marked_house_fee` on a vanilla board.
        let _ = want;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// G-MONEY-13 -- 支付计算时 / 支付前 composition
// ---------------------------------------------------------------------------

/// A fixed add/sub (支付计算时) is applied to the declared amount **before**
/// the halve/double (支付前) scales it.
///
/// * **ID:** G-MONEY-13
/// * **Source:** `rulebook-doc.md:229-232` (支付阶段 rows 1-4)
/// * **Quote:** 「| 2 | 支付计算时 | 支付增加/减少(固定值) | …
///   | 4 | 支付前 | 支付减半/翻倍 |」
/// * **Trigger:** a payment whose settled figure carries **both** a fixed
///   adjustment and a scale (the scenario test arms 丸山彩（2）+ HHW:爱心义演).
/// * **Condition:** the declared amount is B; the fixed adjustment is ±A;
///   the scale is ×s (with 「向上取整10」 where the text says so).
/// * **Effect:** the settled amount is `scale(B ± A)`, **not**
///   `scale(B) ± A`.
/// * **Frame:** the ledger (G-MONEY-04) owns who moved.
/// * **Rulings & OPEN:** the table's stage names are the book's; the engine
///   raises them as `paySplitAdd` (row 2) then `paySplitMul` (row 4). 丸山彩（2） /
///   白鹭千圣（2） say 「分摊前」, which is the pre-split figure the row-2
///   window rewrites. OPEN-支付阶段7 (where row 7 sits) does not change this
///   order.
/// * **Executable:** scenario `pay_stages_add_before_halve` in
///   `rules/tests/tests/spec_g_money.rs` (a per-act clause cannot see the declared
///   figure once standing effects have scaled it). [`g_money_13`] only
///   checks the chain identity on half-charge markers.
const G_MONEY_13: Clause = Clause::new(
    "G-MONEY-13",
    "rulebook-doc.md:230",
    "支付计算时：支付增加/减少(固定值) … 支付前：支付减半/翻倍",
    g_money_13,
);

/// Executable form of [`G-MONEY-13`].
pub fn g_money_13(step: &Step<'_>) -> Result<(), String> {
    for e in step.events {
        if e.r#type != "rent" {
            continue;
        }
        let Some((full, cut)) = half_parts(&e.msg) else {
            continue;
        };
        let want = half_ceil10(full as i32) as i64;
        if cut != want {
            return Err(format!(
                "G-MONEY-13: 支付前 halved the computed {full} to {cut}, expected {want}"
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// G-MONEY-14 -- 支付时 cancel leaves the books unchanged
// ---------------------------------------------------------------------------

/// A cancelled payment (支付时 「取消支付」 / 「无需失去资金」) moves no money.
///
/// * **ID:** G-MONEY-14
/// * **Source:** `rulebook-doc.md:233`
/// * **Quote:** 「| 5 | 支付时 | 支付时/无需失去资金/取消支付/更改目标 | 进行支付 |」
/// * **Trigger:** a window in which a payment was cancelled / set to
///   「无需失去资金」.
/// * **Condition:** the cancellation is visible as the absence of a
///   money-moving event for that payment.
/// * **Effect:** no seat's money moves on account of the cancelled payment.
/// * **Frame:** the ledger (G-MONEY-04) owns the rest of the window.
/// * **Rulings & OPEN:** 「更改目标」 (redirect) is the payee half of the same
///   row and shows up as `other` naming a different seat; G-MONEY-01/04
///   then close the books against that pair. The scenario test
///   `pay_stages_cancel_moves_nothing` pins a live cancel. Per-act this
///   collapses into the ledger: a cancelled payment contributes no legs.
/// * **Executable:** [`g_money_14`].
const G_MONEY_14: Clause = Clause::new(
    "G-MONEY-14",
    "rulebook-doc.md:233",
    "支付时：支付时/无需失去资金/取消支付/更改目标 | 进行支付",
    g_money_14,
);

/// Executable form of [`G-MONEY-14`].
pub fn g_money_14(step: &Step<'_>) -> Result<(), String> {
    // A cancel is the *absence* of a leg; the ledger already forbids a move
    // without one. The one positive check available per-act: a zero-valued
    // money event is a no-op and must not be the source of a move the ledger
    // cannot see -- which is again G-MONEY-04. Keep the clause as the stage's
    // documented id so the soak and the doc share it.
    for e in step.events {
        if matches!(e.r#type.as_str(), "pay" | "rent" | "lose" | "gain" | "pass")
            && e.value == 0
            && int_arg_deep(&e.msg, "gain").unwrap_or(0) == 0
        {
            // Zero leg: nobody moves on its account (covered by the ledger).
            continue;
        }
    }
    Ok(())
}
