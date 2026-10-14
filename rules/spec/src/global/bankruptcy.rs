//! G-BANKRUPT -- [破产], [存活], cash-out, auction, leaving.
//!
//! Owned by unit 7. Append to [`CLAUSES`]; do not edit other areas.
//!
//! Source precedence: live sheet (`docs/rulebook/cards.json`,
//! `the roster` / `bands.json`) > `docs/rulebook/rulebook-doc.md`.
//! `rulebook.txt` is not authoritative.

use game_core::msg::Arg;
use spec_kit::{event_key, Clause, Step};

/// Bankruptcy clauses. Each entry is one book sentence, checked as a predicate
/// over one observed transition.
pub const CLAUSES: &[Clause] = &[
    G_BANKRUPT_01,
    G_BANKRUPT_02,
    G_BANKRUPT_03,
    G_BANKRUPT_04,
    G_BANKRUPT_05,
    G_BANKRUPT_06,
    G_BANKRUPT_07,
    G_BANKRUPT_08,
];

/// Seats this step's window took out of the match, as `(seat, creditor)`.
/// `creditor` is the bankrupt event's `other` (`-1` = consumed; leave uses
/// `-2` because the book gives a leaver no creditor leg).
fn outs_of(step: &Step<'_>) -> Vec<(usize, i32)> {
    let mut out = Vec::new();
    for e in step.events {
        let who = e.player_id;
        if who < 0 {
            continue;
        }
        match e.r#type.as_str() {
            "bankrupt" => out.push((who as usize, e.other)),
            "left" => out.push((who as usize, -2)),
            _ => {}
        }
    }
    out
}

/// Buyable tiles `p` owned in `pre` (the deed set 游戏流程 9.2 draws from).
fn buyable_owned(step: &Step<'_>, p: usize) -> Vec<usize> {
    step.pre
        .st
        .owners
        .iter()
        .enumerate()
        .filter(|&(t, &o)| {
            o == p as i32
                && step
                    .data
                    .tiles
                    .get(t)
                    .map(|d| d.is_buyable())
                    .unwrap_or(true)
        })
        .map(|(t, _)| t)
        .collect()
}

/// Tile indices named on the `log.auction_leftovers` sample of this window.
fn auction_sample(step: &Step<'_>) -> Vec<usize> {
    let mut out = Vec::new();
    for e in step.events {
        if event_key(e) != "log.auction_leftovers" {
            continue;
        }
        if let Some(Arg::List(items)) = e.msg.a.get("tiles") {
            for it in items {
                match it {
                    Arg::Tile(t) => out.push(*t as usize),
                    Arg::I(v) | Arg::N(v) => out.push(*v as usize),
                    _ => {}
                }
            }
        }
    }
    out
}

/// A bankrupt / leaving seat's buyable deeds become unowned.
///
/// * **ID:** G-BANKRUPT-01
/// * **Source:** `rulebook-doc.md:135`
/// * **Quote:** 「**其拥有的[可购买格子]变成无主状态**并抽取3张其中格子的地契进行[拍卖]（少于3则全部）。」
/// * **Trigger:** an observed input whose event window contains a `bankrupt`
///   or `left` event for player `P`.
/// * **Condition:** `P` owned at least one buyable tile in `pre`.
/// * **Effect:** in `post`, no buyable tile is owned by `P`. (The auction of
///   min(3, n) deeds is [`G_BANKRUPT_04`]; this one only checks the un-own step.)
/// * **Frame:** n/a (read of `owners`).
/// * **Rulings & OPEN:** 「[可购买格子]」 excludes CiRCLE / 咖啡厅 / 江户川 /
///   流星堂 / [地产商] (`rulebook-doc.md:65`); a non-buyable tile's owner is
///   not covered here. Bankruptcy money has **no ledger legs** (the engine
///   cashes mortgages silently and zeroes the payer) -- that is a documented
///   observability gap and is why G-MONEY-01 skips `bankrupt` windows.
/// * **Executable:** [`g_bankrupt_01`].
const G_BANKRUPT_01: Clause = Clause::new(
    "G-BANKRUPT-01",
    "rulebook-doc.md:135",
    "其拥有的[可购买格子]变成无主状态",
    g_bankrupt_01,
);

/// Executable form of [`G_BANKRUPT_01`].
pub fn g_bankrupt_01(step: &Step<'_>) -> Result<(), String> {
    let who = outs_of(step);
    if who.is_empty() {
        return Ok(());
    }
    let n = step.post.st.owners.len();
    for (p, _creditor) in &who {
        for t in 0..n {
            if step.post.st.owners[t] == *p as i32 {
                let buyable = step
                    .data
                    .tiles
                    .get(t)
                    .map(|d| d.is_buyable())
                    .unwrap_or(true);
                if buyable {
                    return Err(format!("out player {p} still owns buyable tile {t}"));
                }
            }
        }
    }
    Ok(())
}

/// [破产] cash-out: every fund leaves the bankrupt seat.
///
/// * **ID:** G-BANKRUPT-02
/// * **Source:** `rulebook-doc.md:62`
/// * **Quote:** 「[破产]：当玩家无法支付某笔支出时（包括抵押），将所有可折现的资产折现并将所有资金[消耗]或[支付]给导致破产的效果对象（例：[支付]其他玩家的格子[结算]）。」
/// * **Trigger:** an observed input whose event window contains a `bankrupt`
///   event for player `P`.
/// * **Condition:** `P` could not cover the expense even after 抵押.
/// * **Effect:** in `post`, `P.money == 0` (「将所有资金[消耗]或[支付]…」). The
///   amount handed to the causing effect's target is the `bankrupt` event's
///   `value`; exact legs are checked in the scenario tests (the event tail has
///   no ledger legs for the cash-in itself).
/// * **Frame:** n/a (read of `money`).
/// * **Rulings & OPEN:** 「可折现的资产」 is not defined as a set (houses?
///   only deeds?) -- OPEN-可折现资产. The determined part is the 50% mortgage
///   liquidation of deeds (`rulebook-doc.md:129`) and that all funds leave
///   `P`. A `left` (弃权) seat is not covered: the book gives no cash-out for
///   leaving.
/// * **Executable:** [`g_bankrupt_02`].
const G_BANKRUPT_02: Clause = Clause::new(
    "G-BANKRUPT-02",
    "rulebook-doc.md:62",
    "将所有可折现的资产折现并将所有资金[消耗]或[支付]给导致破产的效果对象",
    g_bankrupt_02,
);

/// Executable form of [`G_BANKRUPT_02`].
pub fn g_bankrupt_02(step: &Step<'_>) -> Result<(), String> {
    for e in step.events {
        if e.r#type != "bankrupt" || e.player_id < 0 {
            continue;
        }
        let p = e.player_id as usize;
        let money = step.post_money(p);
        if money != 0 {
            return Err(format!(
                "bankrupt player {p} still holds {money} money (「将所有资金[消耗]或[支付]」)"
            ));
        }
    }
    Ok(())
}

/// 游戏流程 9.1: the seat is cleared -- hand, character/band/field cards, and
/// its ongoing effects stop.
///
/// * **ID:** G-BANKRUPT-03
/// * **Source:** `rulebook-doc.md:134`
/// * **Quote:** 「将其控制的所有棋子，角色卡，乐队卡，和手卡移出游戏。所有其正在生效的卡，技能效果停止生效。」
/// * **Trigger:** an observed input whose event window contains a `bankrupt`
///   or `left` event for player `P`.
/// * **Condition:** (none beyond the trigger.)
/// * **Effect:** in `post`, `P`'s hand is empty and `P`'s field is empty (the
///   field holds the character card, the band card and every 「[持续]」
///   instance -- they leave the game with the seat, so their effects stop).
///   `P` is marked out (`bankrupt` or `left`).
/// * **Frame:** n/a (reads of `hand` / `field` / flags).
/// * **Rulings & OPEN:** 「棋子」 has no world slot besides `pos`, which the
///   book does not say is cleared -- OPEN-棋子移出. 「乐队卡」 is undefined as a
///   card class (OPEN-乐队卡); the field clear covers whatever the seat held.
/// * **Executable:** [`g_bankrupt_03`].
const G_BANKRUPT_03: Clause = Clause::new(
    "G-BANKRUPT-03",
    "rulebook-doc.md:134",
    "将其控制的所有棋子，角色卡，乐队卡，和手卡移出游戏。所有其正在生效的卡，技能效果停止生效。",
    g_bankrupt_03,
);

/// Executable form of [`G_BANKRUPT_03`].
pub fn g_bankrupt_03(step: &Step<'_>) -> Result<(), String> {
    for (p, _c) in outs_of(step) {
        let hand = step.post.hidden.get(p).map(|h| h.hand.len()).unwrap_or(0);
        if hand != 0 {
            return Err(format!(
                "out player {p} still holds {hand} hand cards (「手卡移出游戏」)"
            ));
        }
        let field = step
            .post
            .st
            .players
            .get(p)
            .map(|x| x.field.len())
            .unwrap_or(0);
        if field != 0 {
            return Err(format!(
                "out player {p} still holds {field} field cards (「角色卡，乐队卡…移出游戏」 / 「效果停止生效」)"
            ));
        }
        let ok = step
            .post
            .st
            .players
            .get(p)
            .map(|x| x.bankrupt || x.left)
            .unwrap_or(false);
        if !ok {
            return Err(format!(
                "player {p} has a bankrupt/left event but is not marked out in post"
            ));
        }
    }
    Ok(())
}

/// 游戏流程 9.2: auction min(3, n) of the freed deeds.
///
/// * **ID:** G-BANKRUPT-04
/// * **Source:** `rulebook-doc.md:135`
/// * **Quote:** 「**其拥有的[可购买格子]变成无主状态**并抽取3张其中格子的地契进行[拍卖]（少于3则全部）。」
/// * **Trigger:** a window that takes `P` out with `n > 0` buyable deeds in
///   `pre`, and the match is still playing afterwards (a last-survivor end
///   cancels the auction).
/// * **Condition:** `P` owned `n` buyable tiles in `pre`.
/// * **Effect:** the window logs an `auction_leftovers` sample of exactly
///   `min(3, n)` tiles, every one of which was `P`'s deed in `pre`.
/// * **Frame:** n/a (read of the event list).
/// * **Rulings & OPEN:** which 3 of the n are drawn is the engine's shuffle --
///   the book only fixes the count. Mortgage status of the drawn deeds is
///   OPEN-拍卖抵押状态. When the game ends on the same step (one [存活] left)
///   the book does not say the auction still runs -- the clause skips. A hook
///   that steals a deed before the draw shrinks the pool; the clause then
///   requires the sample to match the still-unowned remainder.
/// * **Executable:** [`g_bankrupt_04`].
const G_BANKRUPT_04: Clause = Clause::new(
    "G-BANKRUPT-04",
    "rulebook-doc.md:135",
    "抽取3张其中格子的地契进行[拍卖]（少于3则全部）",
    g_bankrupt_04,
);

/// Executable form of [`G_BANKRUPT_04`].
pub fn g_bankrupt_04(step: &Step<'_>) -> Result<(), String> {
    let sample = auction_sample(step);
    for (p, _c) in outs_of(step) {
        let deeds = buyable_owned(step, p);
        if deeds.is_empty() {
            continue;
        }
        // A last-survivor (or other) end on the same step cancels the auction.
        if step.post.st.phase == "ended" {
            continue;
        }
        // Deeds a hook already took off `P` (or off the bank) are out of the pool.
        let pool: Vec<usize> = deeds
            .iter()
            .copied()
            .filter(|&t| step.post.st.owners.get(t).copied().unwrap_or(-1) < 0)
            .collect();
        // Also count deeds that left `P` but are now owned by someone else as
        // removed from the auction pool (the engine filters `owners[t] < 0`).
        let expect = pool.len().min(3);
        if sample.is_empty() {
            if expect == 0 {
                continue;
            }
            return Err(format!(
                "out player {p} left {expect} unowned buyable deeds in the pool {:?}, but the window logged no auction_leftovers",
                pool
            ));
        }
        if sample.len() != expect {
            return Err(format!(
                "out player {p}: auction sample {:?} ({} tiles), expected {expect} of pool {:?}",
                sample,
                sample.len(),
                pool
            ));
        }
        for &t in &sample {
            if !deeds.contains(&t) {
                return Err(format!(
                    "auction sample tile {t} was not a buyable deed of out player {p} in pre (deeds {deeds:?})"
                ));
            }
        }
    }
    Ok(())
}

/// [拍卖] seats: the winner is [存活].
///
/// * **ID:** G-BANKRUPT-05
/// * **Source:** `rulebook-doc.md:66`
/// * **Quote:** 「[拍卖]地契时所有[存活]且可[消耗]资金的玩家参与喊价…」
/// * **Trigger:** a step whose window completed an auction (`log.auction_won`).
/// * **Condition:** the winning bid names player `W`.
/// * **Effect:** `W` is [存活] in `post` (`!bankrupt && !left`). The full
///   participant list (「所有[存活]且可[消耗]资金的玩家参与喊价」) lives on the
///   host prompt, not in `World`, so the everyone-participates half is a
///   scenario assertion in `tests/g_bankrupt.rs`.
/// * **Frame:** n/a (read of flags).
/// * **Rulings & OPEN:** 「可[消耗]资金」 is OPEN-可消耗资金 (cash? cash +
///   mortgageable? the engine's `can_pay`?). This clause checks only the
///   [存活] half of the conjunction.
/// * **Executable:** [`g_bankrupt_05`].
const G_BANKRUPT_05: Clause = Clause::new(
    "G-BANKRUPT-05",
    "rulebook-doc.md:66",
    "[拍卖]地契时所有[存活]且可[消耗]资金的玩家参与喊价",
    g_bankrupt_05,
);

/// Executable form of [`G_BANKRUPT_05`].
pub fn g_bankrupt_05(step: &Step<'_>) -> Result<(), String> {
    for e in step.events {
        if event_key(e) != "log.auction_won" {
            continue;
        }
        let w = e.player_id;
        if w < 0 {
            continue;
        }
        let alive = step
            .post
            .st
            .players
            .get(w as usize)
            .map(|x| !x.bankrupt && !x.left)
            .unwrap_or(false);
        if !alive {
            return Err(format!(
                "auction winner {w} is not [存活] (「所有[存活]…玩家参与喊价」)"
            ));
        }
    }
    Ok(())
}

/// [拍卖] settlement: the highest bidder pays the bid and takes the deed.
///
/// * **ID:** G-BANKRUPT-06
/// * **Source:** `rulebook-doc.md:66`
/// * **Quote:** 「最先喊价最高金额的玩家[消耗]同等资金并获得地契和对应格子拥有权。」
/// * **Trigger:** a window containing a completed auction (`log.auction_won`).
/// * **Condition:** the winning bid is `X` on tile `T` by player `W`.
/// * **Effect:** `post`'s `owners[T] == W` (「获得地契和对应格子拥有权」). On a
///   window with no other money-moving events, `W`'s net money change is
///   `mortgage credits - X` (「[消耗]同等资金」).
/// * **Frame:** n/a (reads of `money` / `owners`).
/// * **Rulings & OPEN:** 「最先喊价最高金额」 -- with a 100 minimum raise the
///   highest amount is unique; the first (and only) player to bid it is the
///   winner. OPEN-拍卖最高额 covers other readings. Houses on the tile ride
///   along (the engine's `incl_houses` note); the book is silent -- OPEN-拍卖房屋.
/// * **Executable:** [`g_bankrupt_06`].
const G_BANKRUPT_06: Clause = Clause::new(
    "G-BANKRUPT-06",
    "rulebook-doc.md:66",
    "最先喊价最高金额的玩家[消耗]同等资金并获得地契和对应格子拥有权",
    g_bankrupt_06,
);

/// Executable form of [`G_BANKRUPT_06`].
pub fn g_bankrupt_06(step: &Step<'_>) -> Result<(), String> {
    for e in step.events {
        if event_key(e) != "log.auction_won" {
            continue;
        }
        let w = e.player_id;
        let tile = e.to;
        let bid = e.value;
        if w < 0 || tile < 0 {
            return Err(format!(
                "auction_won with winner={w} tile={tile} (need both sides)"
            ));
        }
        let w = w as usize;
        let tile = tile as usize;
        let owner = step.post.st.owners.get(tile).copied().unwrap_or(-2);
        if owner != w as i32 {
            return Err(format!(
                "auction winner {w} does not own tile {tile} in post (owner {owner}); 「获得地契和对应格子拥有权」"
            ));
        }
        // 「[消耗]同等资金」 -- the bid left the winner. A raise-funds mortgage
        // may have printed part of it first. Only check a pure auction window.
        let mixed = step.events.iter().any(|e2| {
            matches!(
                e2.r#type.as_str(),
                "pay" | "rent" | "lose" | "gain" | "build" | "forcebuy" | "bankrupt"
            )
        });
        if mixed {
            continue;
        }
        let mut mortgage_credit = 0i64;
        for e2 in step.events {
            if e2.r#type == "mortgage" && e2.player_id == w as i32 {
                mortgage_credit += e2.value as i64;
            }
        }
        let delta = step.post_money(w) as i64 - step.pre_money(w) as i64;
        let expect = mortgage_credit - bid as i64;
        if delta != expect {
            return Err(format!(
                "auction winner {w} money delta {delta}, expected {expect} (mortgage credit {mortgage_credit} minus bid {bid})"
            ));
        }
    }
    Ok(())
}

/// 正规 last survivor wins.
///
/// * **ID:** G-BANKRUPT-07
/// * **Source:** `rulebook-doc.md:121`
/// * **Quote:** 「仅剩一名[存活]玩家时游戏结束，那名玩家获胜。」
/// * **Trigger:** a step that leaves exactly one [存活] player and ends the
///   match (`phase` went to `ended`).
/// * **Condition:** `pre` had more than one [存活]; `post` has one.
/// * **Effect:** the match is over and `post.st.winner` is that [存活] seat.
/// * **Frame:** n/a (reads of flags / `phase` / `winner`).
/// * **Rulings & OPEN:** the quote is the 正规-mode rule; 娱乐 mode ends by
///   consent (`rulebook-doc.md:114`). The engine also auto-ends on the last
///   survivor in every mode -- that is compatible with 娱乐's 「参考时点」 and
///   is not asserted as a 娱乐 requirement. MatchMode ↔ 正规/娱乐 is
///   OPEN-模式映射.
/// * **Executable:** [`g_bankrupt_07`].
const G_BANKRUPT_07: Clause = Clause::new(
    "G-BANKRUPT-07",
    "rulebook-doc.md:121",
    "仅剩一名[存活]玩家时游戏结束，那名玩家获胜。",
    g_bankrupt_07,
);

/// Executable form of [`G_BANKRUPT_07`].
pub fn g_bankrupt_07(step: &Step<'_>) -> Result<(), String> {
    if step.pre.st.phase == "ended" || step.post.st.phase != "ended" {
        return Ok(());
    }
    let alive: Vec<usize> = (0..step.post.st.players.len())
        .filter(|&i| {
            let p = &step.post.st.players[i];
            !p.bankrupt && !p.left
        })
        .collect();
    if alive.len() != 1 {
        return Ok(());
    }
    let sole = alive[0] as i32;
    let winner = step.post.st.winner;
    if winner != sole {
        return Err(format!(
            "only survivor is seat {sole} but winner is {winner} (「那名玩家获胜」)"
        ));
    }
    Ok(())
}

/// Mortgage-to-raise at 50% before going under.
///
/// * **ID:** G-BANKRUPT-08
/// * **Source:** `rulebook-doc.md:129`
/// * **Quote:** 「运营阶段及需[支付]或[消耗]资金且资金不足时可以选择**抵押拥有的地契，抵押后[获得]地契购买价格50%的资金**。」
/// * **Trigger:** a window containing `mortgage` events (the raise-funds path
///   that runs before bankruptcy).
/// * **Condition:** the mortgage is of a tile `T` owned by the mortgagor.
/// * **Effect:** each mortgage leg credits exactly `T.price * 50%` to the
///   mortgagor (`event.value == price / 2`). On a pure-mortgage window the
///   mortgagor's net money change equals the sum of those credits.
/// * **Frame:** n/a (read of `money` / `mortgaged`).
/// * **Rulings & OPEN:** integer halving of an odd price is OPEN-抵押半价取整
///   (the shipped board's prices are all even). 「可以选择」 means the offer
///   may be declined -- a declined mortgage is not an error here. Voluntary
///   运营-phase mortgages take the same 50% path; this clause does not care
///   which cause raised the funds.
/// * **Executable:** [`g_bankrupt_08`].
const G_BANKRUPT_08: Clause = Clause::new(
    "G-BANKRUPT-08",
    "rulebook-doc.md:129",
    "抵押拥有的地契，抵押后[获得]地契购买价格50%的资金",
    g_bankrupt_08,
);

/// Executable form of [`G_BANKRUPT_08`].
pub fn g_bankrupt_08(step: &Step<'_>) -> Result<(), String> {
    let n = step.pre.st.players.len().max(step.post.st.players.len());
    let mut credit = vec![0i64; n];
    let mut saw = false;
    for e in step.events {
        if e.r#type != "mortgage" || e.player_id < 0 {
            continue;
        }
        saw = true;
        let p = e.player_id as usize;
        if p >= n {
            continue;
        }
        // 「[获得]地契购买价格50%的资金」 -- each leg is half the deed price.
        if e.to >= 0 {
            let price = step
                .data
                .tiles
                .get(e.to as usize)
                .map(|d| d.price)
                .unwrap_or(0) as i64;
            let half = price / 2;
            if e.value as i64 != half {
                return Err(format!(
                    "mortgage of tile {} credited {}, expected price/2 = {half} (「购买价格50%」)",
                    e.to, e.value
                ));
            }
        }
        credit[p] += e.value as i64;
    }
    if !saw {
        return Ok(());
    }
    // Pure-mortgage windows: the net money change is exactly the credit.
    // Mixed windows (pay / bankrupt / buy ...) are G-MONEY's ledger territory.
    let mixed = step.events.iter().any(|e| {
        matches!(
            e.r#type.as_str(),
            "pay" | "rent" | "lose" | "gain" | "buy" | "build" | "forcebuy" | "bankrupt"
        )
    });
    if mixed {
        return Ok(());
    }
    for p in 0..n {
        if credit[p] == 0 {
            continue;
        }
        let delta = step.post_money(p) as i64 - step.pre_money(p) as i64;
        if delta != credit[p] {
            return Err(format!(
                "player {p} money delta {delta}, expected mortgage credit {} (「[获得]…50%」)",
                credit[p]
            ));
        }
    }
    Ok(())
}
