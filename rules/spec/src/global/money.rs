//! G-MONEY -- money movement ([获得] / [消耗] / [支付]) and the ledger.
//!
//! Owned by unit 1. Append to [`CLAUSES`]; do not edit other areas.
//!
//! Source precedence: live sheet (`docs/rulebook/cards.json`,
//! `data/characters.json` / `bands.json`) > `docs/rulebook/rulebook-doc.md`.
//! `rulebook.txt` is not authoritative.

use spec_kit::{Clause, Step};

/// Money clauses. Each entry is one book sentence, checked as a predicate
/// over one observed transition.
pub const CLAUSES: &[Clause] = &[G_MONEY_01];

/// 「A[支付]BX资金」 conserves money between A and B.
///
/// * **ID:** G-MONEY-01
/// * **Source:** `rulebook-doc.md:60`
/// * **Quote:** 「[支付]：“A[支付]BX资金”的效果为玩家A向玩家B支付X资金。」
/// * **Trigger:** any observed input whose event window contains money moves.
/// * **Condition:** the window's only money-moving events are `pay` / `rent`
///   legs with no `gain` modifier (a one-sided print / delete is a different
///   clause; a modified transfer is a different clause).
/// * **Effect:** for each such leg of X from A to B: `money(A)` fell by X,
///   `money(B)` rose by X, and every other player's money is unchanged.
/// * **Frame:** only `money:*` may move on a pure-transfer step.
/// * **Rulings & OPEN:** a `gain` arg on the log line (`log.with_received`)
///   means a card raised the amount the payee received; that is not a bare
///   「A[支付]BX」 and is out of scope here. Bankruptcy and auction windows
///   move money without ledger legs and are skipped.
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
    // Skip windows the book's ledger cannot close over (documented gaps).
    for e in step.events {
        if e.r#type == "bankrupt" || e.r#type == "auction" {
            return Ok(());
        }
    }

    // Collect the bare transfers: pay/rent with both sides present and no
    // `gain` modifier (the amount is what A lost and B gained).
    #[derive(Debug)]
    struct Leg {
        from: usize,
        to: usize,
        amount: i64,
    }
    let mut legs = Vec::new();
    let mut other_money_events = 0usize;
    for e in step.events {
        match e.r#type.as_str() {
            "pay" | "rent" => {
                let from = e.player_id;
                let to = e.other;
                if from < 0 || to < 0 {
                    // One-sided (should be lose/gain); not a bare transfer.
                    other_money_events += 1;
                    continue;
                }
                if spec_kit::int_arg(&e.msg, "gain").is_some() {
                    // A modifier raised the payee's take -- not a bare transfer.
                    other_money_events += 1;
                    continue;
                }
                legs.push(Leg {
                    from: from as usize,
                    to: to as usize,
                    amount: e.value as i64,
                });
            }
            "lose" | "gain" | "mortgage" | "redeem" | "pass" | "buy" | "build" | "forcebuy" => {
                other_money_events += 1;
            }
            _ => {}
        }
    }
    if legs.is_empty() || other_money_events > 0 {
        // Nothing to say, or the window is a mix this narrow clause does not
        // cover (unit 1 writes the general ledger).
        return Ok(());
    }

    let mut delta = vec![0i64; step.pre.st.players.len().max(step.post.st.players.len())];
    for i in 0..delta.len() {
        delta[i] = step.post_money(i) as i64 - step.pre_money(i) as i64;
    }

    let mut expect = vec![0i64; delta.len()];
    for leg in &legs {
        if leg.from >= expect.len() || leg.to >= expect.len() {
            return Err(format!("pay leg out of range: {leg:?}"));
        }
        expect[leg.from] -= leg.amount;
        expect[leg.to] += leg.amount;
    }
    for i in 0..delta.len() {
        if delta[i] != expect[i] {
            return Err(format!(
                "player {i}: money delta {} != transfer legs {} (legs: {:?})",
                delta[i],
                expect[i],
                legs.iter()
                    .map(|l| format!("{}->{}:{}", l.from, l.to, l.amount))
                    .collect::<Vec<_>>()
            ));
        }
    }
    Ok(())
}