//! G-BANKRUPT scenario tests: [破产] cash-out, [存活], 游戏流程 9, [拍卖],
//! mortgage-to-raise, and the mode end conditions.
//!
//! Each assertion block names the clause it checks. A discrepancy is a plain
//! failing test -- no `#[ignore]`, no allowlist.
//!
//! Sources (verbatim):
//!
//! * `rulebook-doc.md:62` 「[破产]：当玩家无法支付某笔支出时（包括抵押），将所有可折现的资产折现并将所有资金[消耗]或[支付]给导致破产的效果对象（例：[支付]其他玩家的格子[结算]）。」
//! * `rulebook-doc.md:63` 「[存活]：非[破产]状态且未离开游戏的玩家。」
//! * `rulebook-doc.md:129` 「运营阶段及需[支付]或[消耗]资金且资金不足时可以选择**抵押拥有的地契，抵押后[获得]地契购买价格50%的资金**。」
//! * `rulebook-doc.md:133-135` 游戏流程 9 (bankruptcy / 弃权 cleanup + auction)
//! * `rulebook-doc.md:66` 「[拍卖]：[拍卖]地契时所有[存活]且可[消耗]资金的玩家参与喊价，最先喊价最高金额的玩家[消耗]同等资金并获得地契和对应格子拥有权。每名参与玩家不限喊价次数。最低喊价金额为100，最低单次太高喊价金额为100。」
//! * `rulebook-doc.md:114` 「可在所有玩家同意时结束游戏。…」
//! * `rulebook-doc.md:121` 「仅剩一名[存活]玩家时游戏结束，那名玩家获胜。」

use game_core::net::NetMessage;
use rules_spec::global::bankruptcy::CLAUSES;
use rules_spec::{data, forall, frame, StepAct, Table};

/// Drain every open prompt. Auctions must **pass** (`value < 0`); a plain
/// decline would send the prompt fallback and be read as a bid, looping
/// forever on `err.bid_min`. Counteract / mortgage / tile prompts take their
/// fallback as usual.
fn skip_all(t: &mut Table) {
    for _ in 0..64 {
        t.settle();
        let Some(p) = t.prompt() else { return };
        let asked = t.asked();
        if asked.is_empty() {
            return;
        }
        for who in asked {
            if p.kind == "auction" {
                // Pass: the engine reads `value < 0` as 「不出价」.
                let _ = t.decline_one(who, p.id, -1);
            } else {
                let v = if p.kind == "tile" {
                    p.items.len() as i32
                } else {
                    p.fallback
                };
                let _ = t.observed(StepAct::Decline {
                    who: who as i32 + 1,
                    prompt: p.id,
                    value: v,
                });
            }
        }
    }
    panic!("skip_all: prompts never stopped: {}", t.dump_prompt());
}

/// Seat `who` leaves the game (弃权). Observed as a plain `leave` act.
fn leave(t: &mut Table, who: usize) -> Result<(), String> {
    t.observed(StepAct::Act {
        who: who as i32 + 1,
        msg: NetMessage::act("leave"),
    })
}

/// Seat `who` votes `yes`/`no` on the end-match vote.
fn vote(t: &mut Table, who: usize, yes: bool) -> Result<(), String> {
    t.observed(StepAct::Act {
        who: who as i32 + 1,
        msg: NetMessage {
            value: if yes { 1 } else { 0 },
            ..NetMessage::act("vote")
        },
    })
}

/// Place a bid of `amount` in the open auction prompt as `who`.
fn bid(t: &mut Table, who: usize, amount: i32) -> Result<(), String> {
    let p = t.expect_prompt();
    assert_eq!(p.kind, "auction", "bid outside an auction: {}", t.dump_prompt());
    t.observed(StepAct::Answer {
        who: who as i32 + 1,
        prompt: p.id,
        value: amount,
        cards: vec![],
    })
}

/// Pass in the open auction prompt as `who`.
fn pass(t: &mut Table, who: usize) -> Result<(), String> {
    bid(t, who, -1)
}

// =====================================================================
// G-BANKRUPT-02 -- cash-out and payout to the causing effect's target
// =====================================================================

/// [破产] liquidates every fund and pays the causing effect's target.
///
/// 规则书 专有名词 6: 「将所有可折现的资产折现并将所有资金[消耗]或[支付]给导致破产的效果对象（例：[支付]其他玩家的格子[结算]）。」
/// 规则书（登上武道馆）: 「被[指定]的玩家[支付][使用者]X资金。」
#[test]
fn bankrupt_pays_out_to_the_creditor() {
    // 2 players => X = ceil10(2000/1) = 2000. P1 has 1000 cash and one deed
    // whose 抵押 pays 500 (小豆岛, price 1000) -- 1500 against 2000. Not even
    // 抵押 covers it, so the raise-funds path goes straight under.
    let mut t = Table::vanilla(2);
    let land = rules_spec::tile("小豆岛");
    t.own(1, &[land]);
    t.set_money(1, 1_000);
    t.set_money(0, 10_000);
    t.observe(CLAUSES);
    t.give_play(0, "通用:登上武道馆").unwrap();
    skip_all(&mut t);

    let p = t.p(1);
    assert!(p.bankrupt, "P1 is out: {:?}", t.recent_keys(12));
    assert_eq!(t.money(1), 0, "「将所有资金[消耗]或[支付]」 -- the seat is emptied");
    // 折现 of the deed is its 抵押 value (price/2 = 500), on top of the 1000 cash.
    let cashed = data().tiles[land].price / 2;
    assert_eq!(
        t.money(0),
        10_000 + 1_000 + cashed,
        "the creditor (登上武道馆's [使用者]) received the liquidation"
    );
}

/// With no creditor the funds are [消耗]'d, not paid to anyone.
///
/// 规则书 专有名词 6: 「将所有资金[消耗]或[支付]给导致破产的效果对象」 -- the
/// 「或[消耗]」 branch covers a cause with no live payee (self-charge).
#[test]
fn bankrupt_without_creditor_consumes_the_funds() {
    // TEST:selfCharge is the harness probe for a 5000 self-「[消耗]」 (used by
    // `rb_money`); P0 cannot cover it even after 抵押 (小豆岛 = 500).
    let mut t = Table::vanilla(2);
    let land = rules_spec::tile("小豆岛");
    t.own(0, &[land]);
    t.set_money(0, 1_000);
    t.observe(CLAUSES);
    t.give_play(0, "TEST:selfCharge").unwrap();
    skip_all(&mut t);

    assert!(t.p(0).bankrupt, "P0 is out: {:?}", t.recent_keys(12));
    assert_eq!(t.money(0), 0, "all funds left the bankrupt seat");
    assert_eq!(t.money(1), 10_000, "no third party was paid");
}

// =====================================================================
// G-BANKRUPT-08 -- mortgage-to-raise at 50% before bankruptcy
// =====================================================================

/// A shortfall the deeds can cover is raised by 抵押 at 购买价格50%, not by
/// going under.
///
/// 规则书 游戏流程 5: 「运营阶段及需[支付]或[消耗]资金且资金不足时可以选择
/// **抵押拥有的地契，抵押后[获得]地契购买价格50%的资金**。」
#[test]
fn shortfall_may_be_funded_by_mortgage_at_half_price() {
    // 购物中心 is 3000 -> 抵押 1500. 1000 + 1500 = 2500 >= budokan's 2000.
    let mut t = Table::vanilla(2);
    let land = rules_spec::tile("购物中心");
    t.own(1, &[land]);
    t.set_money(1, 1_000);
    t.observe(CLAUSES);
    t.give_play(0, "通用:登上武道馆").unwrap();
    let p = t.expect_prompt();
    assert_eq!(p.kind, "mortgage", "the mortgage offer: {}", t.dump_prompt());
    t.answer_items(1, &[&land.to_string()]).unwrap();
    skip_all(&mut t);

    assert!(!t.p(1).bankrupt, "P1 stays in: {:?}", t.recent_keys(12));
    assert!(t.mortgaged(land), "the deed is mortgaged");
    // 1000 + 1500 (抵押) - 2000 (paid) = 500.
    assert_eq!(t.money(1), 500, "「[获得]地契购买价格50%的资金」 funded the payment");
    assert_eq!(t.money(0), 12_000, "the creditor received 2000");
}

/// Mortgage legs credit exactly half the deed price, randomized over deed
/// prices and starting cash.
///
/// 规则书 游戏流程 5: 「抵押后[获得]地契购买价格50%的资金」.
#[test]
fn mortgage_credits_half_the_deed_price() {
    forall(8, 0xB7A5, |rng, _i| {
        let mut t = Table::vanilla(2);
        let deeds = ["小豆岛", "富士见坂", "购物中心", "弦卷豪宅", "天文馆"];
        let land_name = deeds[rng.pick_idx(deeds.len())];
        let land = rules_spec::tile(land_name);
        let money = rng.range(0, 40) * 100;
        t.own(0, &[land]);
        t.set_money(0, money);
        t.observe(CLAUSES);

        // Voluntary 运营-phase mortgage is the same 50% path.
        t.mortgage(0, land).map_err(|e| format!("mortgage rejected: {e}"))?;

        let half = data().tiles[land].price / 2;
        let delta = t.money(0) as i64 - money as i64;
        if delta != half as i64 {
            return Err(format!(
                "mortgage of {land_name} (price {}) credited {delta}, expected {half}",
                data().tiles[land].price
            ));
        }
        if !t.mortgaged(land) {
            return Err(format!("{land_name} is not mortgaged"));
        }
        Ok(())
    })
    .expect("mortgage 50% contract");
}

// =====================================================================
// 游戏流程 9 -- the seat retires
// =====================================================================

/// 游戏流程 9.1 on 破产: hand and field leave the game; effects stop.
///
/// 规则书 游戏流程 9.1: 「将其控制的所有棋子，角色卡，乐队卡，和手卡移出游戏。所有其正在生效的卡，技能效果停止生效。」
#[test]
fn bankrupt_clears_hand_and_field() {
    let mut t = Table::vanilla(3);
    let land = rules_spec::tile("小豆岛");
    t.own(1, &[land]);
    t.set_money(1, 0);
    t.give(1, &["通用:GREAT"]);
    t.place_raw(1, "通用:[衍生]FEVER!");
    assert!(!t.hand(1).is_empty(), "setup: P1 holds a card");
    assert!(!t.field(1).is_empty(), "setup: P1 has a field card");
    t.observe(CLAUSES);
    t.give_play(0, "通用:登上武道馆").unwrap();
    skip_all(&mut t);

    assert!(t.p(1).bankrupt, "P1 is out: {:?}", t.recent_keys(12));
    assert!(t.hand(1).is_empty(), "「手卡移出游戏」: {:?}", t.hand(1));
    assert!(
        t.field(1).is_empty(),
        "「角色卡，乐队卡…移出游戏」 / 「效果停止生效」: {:?}",
        t.field_ids(1)
    );
}

/// 游戏流程 9 on 弃权: the same cleanup, without a creditor leg.
///
/// 规则书 游戏流程 9: 「当玩家进入[破产]状态或选择离开游戏（弃权）时进行以下操作…」
#[test]
fn leave_runs_the_same_cleanup() {
    let mut t = Table::vanilla(3);
    let land = rules_spec::tile("小豆岛");
    t.own(1, &[land]);
    t.give(1, &["通用:GREAT"]);
    t.place_raw(1, "通用:[衍生]FEVER!");
    t.observe(CLAUSES);
    leave(&mut t, 1).unwrap();
    skip_all(&mut t);

    let p = t.p(1);
    assert!(p.left, "P1 left: {:?}", t.recent_keys(12));
    assert!(!p.bankrupt, "leaving is not [破产]");
    assert!(t.hand(1).is_empty(), "「手卡移出游戏」");
    assert!(t.field(1).is_empty(), "「效果停止生效」");
    assert_eq!(t.owner(land), None, "「[可购买格子]变成无主状态」");
}

/// [存活] is 「非[破产]状态且未离开游戏」.
///
/// 规则书 专有名词 7: 「[存活]：非[破产]状态且未离开游戏的玩家。」
#[test]
fn alive_is_neither_bankrupt_nor_left() {
    let mut t = Table::vanilla(3);
    let land = rules_spec::tile("小豆岛");
    t.own(1, &[land]);
    t.set_money(1, 0);
    t.observe(CLAUSES);
    t.give_play(0, "通用:登上武道馆").unwrap();
    skip_all(&mut t);
    leave(&mut t, 2).unwrap();
    skip_all(&mut t);

    assert!(t.p(1).bankrupt, "P1 破产");
    assert!(t.p(2).left, "P2 弃权");
    let alive: Vec<usize> = (0..3)
        .filter(|&i| {
            let p = t.p(i);
            !p.bankrupt && !p.left
        })
        .collect();
    assert_eq!(alive, vec![0], "only P0 is [存活]");
}

// =====================================================================
// 游戏流程 9.2 + [拍卖]
// =====================================================================

/// A leaver's deeds are freed and min(3, n) are put up for auction.
///
/// 规则书 游戏流程 9.2: 「其拥有的[可购买格子]变成无主状态**并抽取3张其中格子的地契进行[拍卖]（少于3则全部）。」
#[test]
fn leave_auctions_up_to_three_deeds() {
    let mut t = Table::vanilla(4);
    let names = ["小豆岛", "富士见坂", "成为人类桥", "Bandori车站", "星之鼓动山丘"];
    let deeds: Vec<usize> = names.iter().map(|n| rules_spec::tile(n)).collect();
    t.own(1, &deeds);
    t.observe(CLAUSES);
    leave(&mut t, 1).unwrap();
    let keys = t.recent_keys(30);
    assert!(
        keys.iter().any(|k| k.contains("auction_leftovers")),
        "the leftover auction is announced: {keys:?}"
    );
    for &d in &deeds {
        assert_eq!(t.owner(d), None, "deed {d} is unowned (「变成无主状态」)");
    }
    skip_all(&mut t);
    assert!(t.p(1).left, "P1 left");
    assert_eq!(
        t.field(1).len() + t.hand(1).len(),
        0,
        "cleanup ran with the auction"
    );
}

/// Fewer than 3 deeds: all of them are auctioned.
///
/// 规则书 游戏流程 9.2: 「抽取3张其中格子的地契进行[拍卖]（少于3则全部）。」
#[test]
fn leave_auctions_all_when_fewer_than_three() {
    let mut t = Table::vanilla(4);
    let deeds = [rules_spec::tile("小豆岛"), rules_spec::tile("富士见坂")];
    t.own(1, &deeds);
    t.observe(CLAUSES);
    leave(&mut t, 1).unwrap();
    skip_all(&mut t);
    assert!(t.p(1).left);
    for &d in &deeds {
        assert_eq!(t.owner(d), None, "deed {d} freed");
    }
}

/// [拍卖] wait-list is the [存活] set (「所有[存活]…玩家参与喊价」).
///
/// 规则书 专有名词 10: 「[拍卖]地契时所有[存活]且可[消耗]资金的玩家参与喊价…」
#[test]
fn auction_prompt_waits_on_alive_seats() {
    let mut t = Table::vanilla(4);
    let land = rules_spec::tile("小豆岛");
    t.own(1, &[land]);
    t.observe(CLAUSES);
    leave(&mut t, 1).unwrap();

    let p = t.expect_prompt();
    assert_eq!(p.kind, "auction", "the leftover auction opens: {}", t.dump_prompt());
    for &s in &p.players {
        let seat = s as usize;
        let pl = t.p(seat);
        assert!(
            !pl.bankrupt && !pl.left,
            "auction waits on seat {s} who is not [存活]: {}",
            t.dump_prompt()
        );
    }
    assert!(
        !p.players.contains(&1),
        "the leaver is not a bidder: {}",
        t.dump_prompt()
    );
    skip_all(&mut t);
}

/// Minimum bid 100; minimum raise 100; unlimited bids; the highest bidder
/// pays and takes the deed.
///
/// 规则书 专有名词 10: 「最先喊价最高金额的玩家[消耗]同等资金并获得地契和对应格子拥有权。每名参与玩家不限喊价次数。最低喊价金额为100，最低单次太高喊价金额为100。」
#[test]
fn auction_bid_floor_raise_and_winner() {
    let mut t = Table::vanilla(4);
    let land = rules_spec::tile("小豆岛");
    t.own(1, &[land]);
    t.set_money(0, 5_000);
    t.set_money(2, 5_000);
    t.set_money(3, 5_000);
    t.observe(CLAUSES);
    leave(&mut t, 1).unwrap();

    let p = t.expect_prompt();
    assert_eq!(p.kind, "auction", "{}", t.dump_prompt());
    // Minimum bid is 100: anything below is refused.
    assert!(
        bid(&mut t, 0, 50).is_err(),
        "「最低喊价金额为100」 -- 50 is refused"
    );
    assert!(
        bid(&mut t, 0, 99).is_err(),
        "「最低喊价金额为100」 -- 99 is refused"
    );
    bid(&mut t, 0, 100).expect("first bid of 100");
    // Minimum raise is 100: 150 is only +50 over 100.
    assert!(
        bid(&mut t, 2, 150).is_err(),
        "「最低单次太高喊价金额为100」 -- +50 is refused"
    );
    bid(&mut t, 2, 200).expect("raise to 200");
    // 「每名参与玩家不限喊价次数」 -- P0 bids again (not the top bidder).
    bid(&mut t, 0, 300).expect("P0's second bid");
    bid(&mut t, 2, 400).expect("P2's second bid");
    pass(&mut t, 3).expect("P3 passes");
    pass(&mut t, 0).expect("P0 passes");
    skip_all(&mut t);

    assert_eq!(
        t.owner(land),
        Some(2),
        "「最先喊价最高金额的玩家…获得地契和对应格子拥有权」"
    );
    assert_eq!(t.money(2), 5_000 - 400, "「[消耗]同等资金」 -- the bid of 400");
    assert_eq!(t.money(0), 5_000, "P0 only bid, did not win");
    assert_eq!(t.money(3), 5_000, "P3 passed");
}

/// The winner pays exactly the bid, randomized over starting cash.
///
/// 规则书 专有名词 10: 「最先喊价最高金额的玩家[消耗]同等资金并获得地契和对应格子拥有权。」
#[test]
fn auction_winner_pays_exactly_the_bid() {
    forall(6, 0xA11C, |rng, _i| {
        let mut t = Table::vanilla(4);
        let land = rules_spec::tile("小豆岛");
        t.own(1, &[land]);
        let cash = 1_000 + rng.range(0, 30) * 100;
        t.set_money(2, cash);
        t.set_money(0, 0);
        t.set_money(3, 0);
        t.observe(CLAUSES);
        leave(&mut t, 1).unwrap();

        let p = t.expect_prompt();
        if p.kind != "auction" {
            return Err(format!("expected an auction, got {}", t.dump_prompt()));
        }
        bid(&mut t, 2, 100)?;
        for who in t.asked() {
            if who != 2 {
                pass(&mut t, who)?;
            }
        }
        skip_all(&mut t);

        if t.owner(land) != Some(2) {
            return Err(format!(
                "P2 should own the deed at 100, owner {:?}",
                t.owner(land)
            ));
        }
        let delta = t.money(2) as i64 - cash as i64;
        if delta != -100 {
            return Err(format!(
                "P2 money delta {delta}, expected -100 (「[消耗]同等资金」)"
            ));
        }
        Ok(())
    })
    .expect("auction winner pays the bid");
}

// =====================================================================
// mode end conditions
// =====================================================================

/// 正规: the last [存活] player wins.
///
/// 规则书 正规 4: 「仅剩一名[存活]玩家时游戏结束，那名玩家获胜。」
#[test]
fn last_alive_player_wins() {
    // 正规 wants 5-6 players; use 5 so the setup is book-legal.
    let mut t = Table::vanilla(5);
    t.observe(CLAUSES);
    for who in 1..4 {
        leave(&mut t, who).unwrap();
        skip_all(&mut t);
    }
    assert_eq!(
        t.st().phase,
        "play",
        "two [存活] left (P0 and P4): {:?}",
        t.recent_keys(8)
    );
    leave(&mut t, 4).unwrap();
    skip_all(&mut t);
    let st = t.st();
    assert_eq!(st.phase, "ended", "「游戏结束」");
    assert_eq!(st.winner, 0, "「那名玩家获胜」");
    assert_eq!(st.end_reason, "last");
}

/// 娱乐: the match may end when **all** [存活] players agree.
///
/// 规则书 娱乐 5: 「可在所有玩家同意时结束游戏。」
#[test]
fn entertainment_ends_by_unanimous_consent() {
    let mut t = Table::vanilla(3);
    t.observe(CLAUSES);
    vote(&mut t, 0, true).expect("propose the end");
    vote(&mut t, 1, true).expect("P1 agrees");
    assert_eq!(
        t.st().phase,
        "play",
        "one holdout keeps the match going: {:?}",
        t.recent_keys(6)
    );
    vote(&mut t, 2, true).expect("P2 agrees");
    skip_all(&mut t);
    let st = t.st();
    assert_eq!(st.phase, "ended", "「所有玩家同意时结束游戏」");
    assert_eq!(st.end_reason, "vote");
}

/// A single dissent blocks the 娱乐 end (consent means *all*).
///
/// 规则书 娱乐 5: 「可在所有玩家同意时结束游戏。」
#[test]
fn one_dissent_blocks_the_consent_end() {
    let mut t = Table::vanilla(3);
    t.observe(CLAUSES);
    vote(&mut t, 0, true).expect("propose");
    vote(&mut t, 1, false).expect("P1 refuses");
    // The dissent closes the vote (the engine puts a cooldown on a new one).
    // 「所有玩家同意」 did not happen, so the match continues.
    assert_eq!(t.st().phase, "play", "「所有玩家同意」 did not happen");
}

// =====================================================================
// frame: the retirement step only touches the out seat and the board
// =====================================================================

/// The leave step's footprint is the leaver's zone plus the freed deeds.
///
/// 规则书 游戏流程 9. Frame is the clause's 「and nothing else moved」.
#[test]
fn leave_frame_only_touches_the_out_seat_and_deeds() {
    forall(6, 0xF4A3, |rng, _i| {
        let mut t = Table::vanilla(4);
        let names = ["小豆岛", "富士见坂", "购物中心", "天文馆"];
        let k = 1 + rng.pick_idx(3);
        let deeds: Vec<usize> = (0..k).map(|i| rules_spec::tile(names[i])).collect();
        t.own(1, &deeds);
        t.give(1, &["通用:GREAT"]);
        t.place_raw(1, "通用:[衍生]FEVER!");
        let pre = t.snapshot_world();
        t.observe(CLAUSES);
        leave(&mut t, 1).map_err(|e| format!("leave rejected: {e}"))?;

        let mut allowed: Vec<String> = vec![
            "money:P1".into(),
            "hand:P1".into(),
            "hand_n:P1".into(),
            "field:P1".into(),
            "state:P1".into(),
            "token:P1".into(),
            "out:P1".into(),
            "left:P1".into(),
            "bankrupt:P1".into(),
            "assets:P1".into(),
            "persona:P1".into(),
            "crystals:P1".into(),
            "out_count".into(),
            "mark:".into(),
        ];
        for &d in &deeds {
            allowed.push(format!("owner:T{d}"));
            allowed.push(format!("mortgaged:T{d}"));
            allowed.push(format!("houses:T{d}"));
        }
        let allowed: Vec<&str> = allowed.iter().map(String::as_str).collect();
        frame(&pre, t.world(), &allowed)
    })
    .expect("leave frame");
}
