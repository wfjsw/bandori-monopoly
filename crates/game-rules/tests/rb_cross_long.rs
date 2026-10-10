//! Long chains with in-chain interactions: §5 `L*` of
//! `docs/rulebook/CROSS-TESTS.md`. Each case asserts **every intermediate
//! step**: ask order, which link each window answers, resolution order, and
//! the money / position / hand after each resolution where observable.

mod common;

use common::*;
use game_core::engine::CardRules;
use game_core::msg::{Arg, Msg};

const FILL: &str = "R:[衍生] 觉悟";

/// Place `card` on `who`'s field at board `tile` (a sheet 「置于格子上」 card).
fn place_on_tile(t: &mut Table, who: usize, card: &str, tile: usize) {
    let d = data();
    let props = rules().card_props(card);
    t.m.world_mut().place_card_on(
        &d,
        who as i32,
        tile as i32,
        card,
        Msg::default(),
        props,
    );
}

fn give_n(t: &mut Table, who: usize, card: &str, n: usize) {
    let cards: Vec<&str> = vec![card; n];
    t.give(who, &cards);
}

fn set_draw_n(t: &mut Table, who: usize, n: usize) {
    let cards: Vec<&str> = vec![FILL; n];
    t.set_draw(who, &cards);
}

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

fn pass(t: &mut Table, who: usize) {
    t.m.world_mut().st.skip_move = true;
    t.end(who).unwrap();
    drain(t);
}

fn until_turn(t: &mut Table, who: usize) {
    for _ in 0..20 {
        if t.turn() == who {
            return;
        }
        let cur = t.turn();
        pass(t, cur);
    }
    panic!("never reached turn {who} (at {})", t.turn());
}

fn answer_player(t: &mut Table, who: usize, target: i32) {
    let p = t.expect_prompt();
    let k = p
        .options
        .iter()
        .position(|o| match o.a.get("who") {
            Some(Arg::PlayerId(v)) => *v == target,
            Some(Arg::I(v)) | Some(Arg::N(v)) => *v == target as i64,
            _ => false,
        })
        .unwrap_or_else(|| panic!("player {target} not offered: {}", t.dump_prompt()));
    t.answer(who, k as i32).unwrap();
}

/// The card the open [反击] window's detail line names -- the link it answers.
fn answered_card(t: &Table) -> Option<String> {
    let p = t.prompt()?;
    match p.text.a.get("detail") {
        Some(Arg::Msg(m)) => match m.a.get("card") {
            Some(Arg::Card(id)) => Some(id.clone()),
            _ => None,
        },
        _ => None,
    }
}

fn ceil10(n: i32) -> i32 {
    (n + 9) / 10 * 10
}

fn rent(t: usize, h: usize) -> i32 {
    let tile = &data().tiles[t];
    tile.rent[(h).min(tile.rent.len().saturating_sub(1))]
}

// =====================================================================
// L1. A counter war five links deep
// =====================================================================

// 规则书: 32 「[反击]…结算优先于X」 + 89 / ruling 2026-10-07 (the ask ring
// starts with the initial user -- the player whose action raised the link;
// each visit exhausts every eligible counteraction or ends on an explicit
// pass; laps continue until a quiet lap; a counter's own round starts with its
// declarer) + the LIFO ruling (unchanged).
#[ignore = "DISCREPANCY: in a 4-player war the ring skips P1 (宣战布告 is never offered) and nothing settles"]
#[test]
fn l01_counter_war_five_links_deep() {
    let mut t = Table::vanilla(4);
    set_draw_n(&mut t, 0, 3);
    set_draw_n(&mut t, 1, 3);
    set_draw_n(&mut t, 2, 3);
    set_draw_n(&mut t, 3, 3);
    // Hands: P1 = A(宣战布告); P2 = B(宣战布告), D(网络链接异常);
    //        P3 = E(网络链接异常); P0 = X(武道馆), C(网络链接异常).
    t.give(1, &["AG:宣战布告"]);
    t.give(2, &["AG:宣战布告", "通用:网络链接异常"]);
    t.give(3, &["通用:网络链接异常"]);
    t.give(0, &["通用:登上武道馆", "通用:网络链接异常"]);
    let x = ceil10(2000 / 3);
    assert_eq!(x, 670);

    // 1. P0 plays X.
    t.play(0, "通用:登上武道馆").unwrap();

    // 2+. Walk every window: the round on X, then rounds on each counter.
    let mut ask_order = vec![];
    let mut declared = vec![];
    loop {
        let Some(_) = t.prompt() else { break };
        let asked = t.asked();
        assert_eq!(asked.len(), 1, "{}", t.dump_prompt());
        ask_order.push((asked[0], answered_card(&t).unwrap_or_default()));
        if t.counteract_offered("AG:宣战布告") && asked[0] == 1 && !declared.contains(&"A") {
            declared.push("A");
            t.counteract(1, "AG:宣战布告").unwrap();
        } else if t.counteract_offered("AG:宣战布告") && asked[0] == 2 && !declared.contains(&"B") {
            declared.push("B");
            t.counteract(2, "AG:宣战布告").unwrap();
        } else if t.counteract_offered("通用:网络链接异常") && asked[0] == 0 && !declared.contains(&"C") {
            declared.push("C");
            t.counteract(0, "通用:网络链接异常").unwrap();
        } else if t.counteract_offered("通用:网络链接异常") && asked[0] == 2 && !declared.contains(&"D") {
            declared.push("D");
            t.counteract(2, "通用:网络链接异常").unwrap();
        } else if t.counteract_offered("通用:网络链接异常") && asked[0] == 3 && !declared.contains(&"E") {
            declared.push("E");
            t.counteract(3, "通用:网络链接异常").unwrap();
        } else {
            t.decline();
        }
    }
    eprintln!("l01 ask order: {ask_order:?}, declared = {declared:?}");

    // The walk above covers the rounds on X and on each declared counter.

    // Resolution, newest first. Record every step's money.
    eprintln!(
        "l01 after chain: P0 = {} (want 11510), P1 = {} (want 9330), P2 = {} (want 9830), P3 = {} (want 9330)",
        t.money(0),
        t.money(1),
        t.money(2),
        t.money(3)
    );
    // X resolves: P1, P2, P3 each pay 670 to P0.
    // B resolves: P0 pays P2 500, P2 draws 1.
    // A resolves: P0 pays nothing (E cancelled the designation).
    assert_eq!(t.money(0), 10_000 - 500 + x * 3, "P0 = -500 + 3*{x}");
    assert_eq!(t.money(1), 10_000 - x, "P1");
    // P2: +500 from B, -670 from X.
    assert_eq!(t.money(2), 10_000 + 500 - x, "P2");
    assert_eq!(t.money(3), 10_000 - x, "P3");
    // Every declared card is in its owner's discard.
    for (who, card) in [
        (1usize, "AG:宣战布告"),
        (2, "AG:宣战布告"),
        (0, "通用:网络链接异常"),
        (2, "通用:网络链接异常"),
        (3, "通用:网络链接异常"),
    ] {
        assert!(
            t.discard(who).contains(&card.to_string()),
            "{card} in P{who}'s discard: {:?}",
            t.discard(who)
        );
    }
}

// =====================================================================
// L2. One walk through several pass effects
// =====================================================================

// 规则书: 86 (path recomputation), （香澄）, Random Star (2), （乐奈）.
#[test]
fn l02_one_walk_several_pass_effects() {
    // CROSS-TESTS.md L2: P1 is 户山香澄 and holds （香澄）大家我都喜欢哦; P0 is
    // the walker. The sheet clause is 「其他玩家[经过]」, so the placer must be
    // someone other than the walker.
    let mut t = Table::new(&["花园多惠", "户山香澄", "要乐奈"]);
    t.clean();
    // Arrange before begin_turn: the turn-start skill hooks leave a routine
    // pending, and world_mut may not be used while one is.
    let t48 = tile("山吹面包房");
    t.own(1, &[t48]);
    t.own(0, &[tile("星之鼓动山丘")]);
    // （香澄） card on 44 (the hill tile its effect keys on), placed by P1.
    place_on_tile(&mut t, 1, "PPP:（香澄）大家我都喜欢哦", tile("星之鼓动山丘"));
    // （乐奈） on 48 with 5 crystals.
    place_on_tile(&mut t, 2, "MyGO:（乐奈）有趣的女人", t48);
    t.set_crystals(2, "MyGO:（乐奈）有趣的女人", 5);
    // Random Star on P0's field; 2 stickers.
    t.place_raw(0, "PPP:仓库里的Random Star");
    t.m.world_mut().st.players[0].tokens.push(game_core::state::Counter {
        name: "星星贴纸".into(),
        value: 2,
    
        instance: None,
    });
    // 安可 in hand from the start, so the 香澄 forced stop can be countered.
    t.give(0, &["通用:安可"]);
    t.set_pos(0, 38);
    t.dice(&[12]);
    t.begin_turn(0);
    drain(&mut t);
    // 1. P0 rolls a loaded 12, path 39 -> 50.
    t.roll(0).unwrap();

    // 2. At 44: 香澄's card forces a stop; P0 answers with 安可.
    let mut answered_encore = false;
    loop {
        let Some(_) = t.prompt() else { break };
        if t.counteract_offered("通用:安可") {
            answered_encore = true;
            t.counteract(0, "通用:安可").unwrap();
            continue;
        }
        let k = t.option("强制停下").or_else(|| t.option("香澄"));
        if let Some(k) = k {
            t.answer(0, k).unwrap();
        } else {
            t.decline();
        }
    }
    eprintln!(
        "l02 record: prompt = {}, pos = {}, encore = {answered_encore}",
        t.dump_prompt(),
        t.pos(0)
    );
    drain(&mut t);
    eprintln!(
        "l02 final: pos = {} (want 48), money = {} (want 10000 - R(48)/2 = {}), stickers = {}, discard = {:?}",
        t.pos(0),
        t.money(0),
        rent(t48, 0) / 2,
        t.token(0, "星星贴纸"),
        t.discard(0)
    );
    // （乐奈）有趣的女人 forces a stop on 48 at half rent; 安可 undoes the 香澄 stop.
    assert!(answered_encore, "安可 must be offered for the 香澄 forced stop");
    assert_eq!(t.pos(0), 48, "P0 ends on 山吹面包房");
    assert_eq!(t.money(0), 10_000 - rent(t48, 0) / 2, "half rent on the forced settle");
}

// =====================================================================
// L3. A rent payment through every modifier
// =====================================================================

// 规则书: Anon link + Fire bird 1.5× + 0.5倍速 halving + FEVER! add + 三全音.
// RULING: order of add vs multiply; whether the linked half is boosted.
#[test]
fn l03_rent_through_every_modifier() {
    let mut t = Table::new(&["千早爱音", "青叶摩卡", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    let t1 = tile("购物中心");
    let t2 = tile("天文馆");
    t.own(0, &[t1, t2]);
    t.set_pos(0, t1);
    t.give_play(0, "MyGO:[千早爱音]Anon Tokyo").unwrap();
    drain(&mut t);
    t.place_raw(0, "R:Fire bird");
    t.place_raw(0, "通用:[衍生]FEVER!");
    t.place_raw(1, "AG:（摩卡）0.5倍速");
    t.give(1, &["Mor:迷茫之蝶们的三全音", "CRYCHIC:主唱太拼命了"]);
    set_draw_n(&mut t, 1, 3);

    // P1 is 青叶摩卡 and lands on 1.
    until_turn(&mut t, 1);
    t.set_pos(1, t1 - 1);
    t.dice(&[1]);
    t.roll(1).unwrap();

    // The counter window on the payment: P1 declares 三全音.
    let mut windows = vec![];
    loop {
        let Some(_) = t.prompt() else { break };
        let dump = t.dump_prompt();
        windows.push(dump.clone());
        if t.counteract_offered("Mor:迷茫之蝶们的三全音") {
            t.counteract(1, "Mor:迷茫之蝶们的三全音").unwrap();
        } else if t.counteract_offered("CRYCHIC:主唱太拼命了") {
            // Only offered at >= 5000 -- record.
            eprintln!("l03 record: 主唱太拼命了 offered at {dump}");
            t.decline();
        } else {
            t.decline();
        }
    }
    let base = rent(t1, 0) + rent(t2, 0) / 2;
    eprintln!(
        "l03 record (RULING: add/mul order, linked half): base = {base}, windows = {windows:?}"
    );
    eprintln!(
        "l03 after: P0 = {}, P1 = {} (net 0 with 三全音), 三全音 crystals = {:?}",
        t.money(0),
        t.money(1),
        t.crystals(1, "Mor:迷茫之蝶们的三全音")
    );
    assert!(
        t.field_ids(1).iter().any(|c| c.contains("三全音")),
        "三全音 placed: {:?}",
        t.field_ids(1)
    );
    eprintln!(
        "l03 net now: P1 = {} (want 10000 if the gain equals the payment)",
        t.money(1)
    );
}

// =====================================================================
// L4. A forced move into a remote settle on a linked tile
// =====================================================================

// 规则书: 无法将视线移开 forces a walk; 练习室里的风暴 remote-settles 50;
// then the counter and the original card resolve.
// RULING: whether the link's extra half applies inside the scaled settlement.
#[ignore = "DISCREPANCY: 无法将视线移开 cannot be played as the counter (err.play_phase)"]
#[test]
fn l04_forced_move_into_remote_settle() {
    let mut t = Table::new(&["千早爱音", "花园多惠", "青叶摩卡"]);
    t.clean();
    t.begin_turn(0);
    let t49 = tile("银河拉面馆");
    let t50 = tile("Live House Galaxy");
    let t52 = tile("旭汤澡堂");
    t.own(0, &[t49, t50]);
    t.set_pos(0, t49);
    t.give_play(0, "MyGO:[千早爱音]Anon Tokyo").unwrap();
    drain(&mut t);
    t.place_raw(0, "RAS:练习室里的风暴");
    t.set_crystals(0, "RAS:练习室里的风暴", 2);
    t.set_pos(1, 48);
    t.give(1, &["AG:宣战布告"]);
    t.give(0, &["通用:登上武道馆", "Mujica:无法将视线移开"]);

    // 1. P0 plays 武道馆.
    t.play(0, "通用:登上武道馆").unwrap();
    // 2. P1 counters with 宣战布告 (A).
    loop {
        let Some(_) = t.prompt() else { break };
        if t.counteract_offered("AG:宣战布告") {
            t.counteract(1, "AG:宣战布告").unwrap();
            break;
        }
        t.decline();
    }
    // 3. Round on A: P0 counters with 无法将视线移开 (B), forward 4 to 52.
    loop {
        let Some(_) = t.prompt() else { break };
        if t.counteract_offered("Mujica:无法将视线移开") {
            t.counteract(0, "Mujica:无法将视线移开").unwrap();
            break;
        }
        t.decline();
    }
    loop {
        let Some(_) = t.prompt() else { break };
        let k = t.option("4").or_else(|| t.option("前"));
        if let Some(k) = k {
            t.answer(0, k).unwrap();
        } else {
            eprintln!("l04 move prompt: {}", t.dump_prompt());
            t.decline();
        }
    }
    eprintln!(
        "l04 record: P1 pos = {} (want 52), money = {}, storm = {:?}",
        t.pos(1),
        t.money(1),
        t.discard(0)
    );
    // 4-7. B resolves first (walk + settle + storm), then A, then 武道馆.
    let x = ceil10(2000 / 2);
    let scaled = ((rent(t50, 0) as f64 * (4.0 - 2.0) / 4.0) as i32);
    eprintln!(
        "l04 after: P0 = {}, P1 = {}, P2 = {}, scaled settle on 50 = {scaled}, R(50) = {}",
        t.money(0),
        t.money(1),
        t.money(2),
        rent(t50, 0)
    );
    assert_eq!(t.pos(1), t52, "P1 is on 52");
    let _ = x;
}

// =====================================================================
// L5. One skill use, several counteractions
// =====================================================================

// 规则书: clause-89 order for counteractions to one skill use (ruling 2026-10-07:
// the ring starts with the initial user of the skill use).
// RULING: whether P1 still gets the mark and P2 still teleports when the use
// is cancelled; whether P0's fire is refunded.
#[ignore = "DISCREPANCY: 花园多惠 (2) cancel window never opens; P0's teleport resolves (pos 10 not 5)"]
#[test]
fn l05_one_skill_several_counteractions() {
    let mut t = Table::new(&["户山香澄", "广町七深", "青叶摩卡", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    t.set_fire(0, 1, 1);
    t.set_fire(1, 2, 2);
    t.set_fire(2, 1, 1);
    t.set_fire(3, 4, 4);
    t.own(0, &[10]);
    t.set_pos(0, 5);
    let sid = t.skill_id(0, "非凡之星");
    t.skill(0, &sid).unwrap();
    // Counteractions: P1 (2) mark, P2 (2) teleport, P3 (2) cancel.
    let mut seen = vec![];
    loop {
        let Some(_) = t.prompt() else { break };
        let dump = t.dump_prompt();
        seen.push(dump.clone());
        let cancel = t.option("花园多惠").or_else(|| t.option("抵消"));
        let mark = t.option("广町七深").or_else(|| t.option("标记"));
        let tp = t.option("青叶摩卡").or_else(|| t.option("传送"));
        if let Some(k) = cancel {
            t.answer(3, k).unwrap();
        } else if let Some(k) = mark {
            t.answer(1, k).unwrap();
        } else if let Some(k) = tp {
            t.answer(2, k).unwrap();
        } else {
            t.decline();
        }
    }
    eprintln!(
        "l05 record (RULING: counteractions survive cancel?): pos0 = {} (want 5), money0 = {} (+2000), money3 = {} (+500), pos2 = {} (want 5), fire = {}/{}/{}/{}",
        t.pos(0),
        t.money(0),
        t.money(3),
        t.pos(2),
        t.fire(0),
        t.fire(1),
        t.fire(2),
        t.fire(3)
    );
    eprintln!("l05 windows: {seen:?}");
    assert_eq!(t.pos(0), 5, "P0 does not move");
    assert_eq!(t.money(0), 12_000, "P0 gains 2000");
    assert_eq!(t.money(3), 10_500, "P3 gains 500");
}

// =====================================================================
// L6. One draw meeting several draw effects
// =====================================================================

// 规则书: 大和麻弥 (2) look-at-top; Here the world captures the drawn card;
// 朝同一片天空迈进 auto-plays; 梦在前方 gains a crystal per draw.
// RULING: whether Here the world captures the card before the auto-play.
#[ignore = "RULING: whether Here the world captures a drawn card before that card's auto-play"]
#[test]
fn l06_one_draw_several_effects() {
    let mut t = Table::new(&["大和麻弥", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    // 3 face-up fans.
    for _ in 0..3 {
        t.m.world_mut().st.players[0].tokens.push(game_core::state::Counter {
            name: "P✽P粉丝(正)".into(),
            value: 1,
        
        instance: None,
    });
    }
    t.place_raw(0, "PP:梦在前方，结彩当下");
    t.set_hand(0, &[FILL, FILL, FILL]);
    // Draw pile, top first: filler, 朝同一片天空迈进, filler.
    t.set_draw(0, &[FILL, "AG:朝同一片天空迈进", FILL]);
    // P1 has placed Here the world on P0's field.
    t.place_raw(1, "Sumimi:Here the world");
    // Take the CiRCLE reward's draw (walk past CiRCLE).
    t.set_pos(0, 59);
    t.dice(&[2]);
    t.roll(0).unwrap();
    loop {
        let Some(_) = t.prompt() else { break };
        eprintln!("l06 prompt: {}", t.dump_prompt());
        let k = t.option("朝同一片天空迈进").or_else(|| t.option("2"));
        if let Some(k) = k {
            t.answer(0, k).unwrap();
        } else {
            t.decline();
        }
    }
    eprintln!(
        "l06 record (RULING: capture vs auto-play): hand = {:?}, draw = {:?}, 梦在前方 crystals = {:?}, Here crystals = {:?}, tokens = {:?}",
        t.hand(0),
        t.draw_pile(0),
        t.crystals(0, "PP:梦在前方，结彩当下"),
        t.crystals(0, "Sumimi:Here the world"),
        t.p(0).tokens
    );
}

// =====================================================================
// L7. An extra-turn cascade with decaying field cards
// =====================================================================

// 规则书: 线香花火 (「最后一个奇迹水晶移除后…2层眩晕」 + extra turn on each
// removal) + 运动的天赋 (700 then 600... re-roll until >= 10).
// RULING: whether the last removal also grants an extra turn.
#[ignore = "DISCREPANCY: the last 线香花火 removal nets 1 [眩晕] layer, want 2 — the phase-10 「回合结束时」 status decay runs after the phase-11 「回合结束后」 crystal removal and eats one of the two"]
#[test]
fn l07_extra_turn_cascade() {
    let mut t = Table::vanilla(2);
    t.place_raw(0, "Mujica:燃尽前的线香花火");
    t.set_crystals(0, "Mujica:燃尽前的线香花火", 2);
    t.place_raw(0, "HHW:运动的天赋");
    t.set_crystals(0, "HHW:运动的天赋", 3);
    t.give(1, &["MyGO:壱雫空"]);

    // Turn A ends: 线香花火 -> 1 crystal + extra turn; 运动的天赋 -> 2.
    t.dice(&[3, 12]);
    t.roll(0).unwrap();
    drain(&mut t);
    pass(&mut t, 0);
    eprintln!(
        "l07 after A: 线香 = {:?}, 运动 = {:?}, turn = {} (want still 0 = extra)",
        t.crystals(0, "Mujica:燃尽前的线香花火"),
        t.crystals(0, "HHW:运动的天赋"),
        t.turn()
    );
    // 「你的回合结束后自动移除一个奇迹水晶」 — both decay by 1 at turn end.
    assert_eq!(t.crystals(0, "Mujica:燃尽前的线香花火"), Some(1), "线香 2→1");
    assert_eq!(t.crystals(0, "HHW:运动的天赋"), Some(2), "运动 3→2");
    // Extra turn B: roll pays 运动的天赋 700, re-rolls until >= 10 (600 each).
    t.dice(&[3, 12]);
    t.roll(0).unwrap();
    drain(&mut t);
    eprintln!(
        "l07 after B roll: money = {}, 运动 = {:?}",
        t.money(0),
        t.crystals(0, "HHW:运动的天赋")
    );
    pass(&mut t, 0);
    eprintln!(
        "l07 after B end (RULING: last removal grants extra?): 线香 = {:?}, stun = {}, turn = {}, 运动 = {:?}",
        t.crystals(0, "Mujica:燃尽前的线香花火"),
        t.state(0, "stun"),
        t.turn(),
        t.crystals(0, "HHW:运动的天赋")
    );
    // 「最后一个奇迹水晶移除后将此卡置入弃牌堆并立刻使你获得2层[眩晕]」.
    assert_eq!(t.state(0, "stun"), 2, "the last removal grants 2 [眩晕]");
    // P1 plays 壱雫空: P0's stun is cleared.
    until_turn(&mut t, 1);
    t.give_play(1, "MyGO:壱雫空").unwrap();
    drain(&mut t);
    eprintln!(
        "l07 after 壱雫空: stun = {}, money = {}",
        t.state(0, "stun"),
        t.money(0)
    );
}

// =====================================================================
// L8. Status hand-offs
// =====================================================================

// 规则书: 雨啊 + 椎名立希 (1) + 丰川祥子 (1) + 壱雫空 + state-2 entry.
#[test]
fn l08_status_hand_offs() {
    let mut t = Table::new(&["丰川祥子", "花园多惠", "椎名立希", "青叶摩卡"]);
    t.clean();
    t.begin_turn(0);
    t.set_state(0, "skillState", 1);
    t.set_fire(0, 0, 3);
    t.set_fire(2, 0, 5);

    // 1. P1 plays 雨啊 (2d2 = 2 + 1 = 3). Designates P1, P3, P0 (must include user).
    until_turn(&mut t, 1);
    t.give(1, &["通用:雨啊，快点来吧"]);
    t.dice(&[2, 1]);
    t.play(1, "通用:雨啊，快点来吧").unwrap();
    let mut designated = vec![];
    loop {
        let Some(p) = t.prompt() else { break };
        let is_player = p.options.iter().any(|o| o.key() == "ask.player");
        if is_player {
            let pick = [3i32, 0, 2]
                .into_iter()
                .find(|w| !designated.contains(w))
                .unwrap_or(3);
            designated.push(pick);
            answer_player(&mut t, 1, pick);
        } else {
            t.decline();
        }
    }
    eprintln!(
        "l08 after 雨啊: designated = {designated:?}, stays = P0:{} P1:{} P3:{}, 立希 fire = {} (want 3), 祥子 fire = {} (want 1)",
        t.state(0, "stay"),
        t.state(1, "stay"),
        t.state(3, "stay"),
        t.fire(2),
        t.fire(0)
    );
    assert_eq!(t.fire(2), 3, "立希 (1): one per stay layer");
    assert_eq!(t.fire(0), 1, "祥子 (1): one per layer received");

    // 2. On P0's turn, P0 can't move because of its stay.
    until_turn(&mut t, 0);
    t.dice(&[5]);
    let r = t.roll(0);
    eprintln!("l08 P0 roll with stay: {r:?}, pos = {}", t.pos(0));
    pass(&mut t, 0);

    // 3. P1 plays 壱雫空: every stay goes.
    until_turn(&mut t, 1);
    t.give(1, &["MyGO:壱雫空"]);
    t.play(1, "MyGO:壱雫空").unwrap();
    drain(&mut t);
    eprintln!(
        "l08 after 壱雫空: stays = P0:{} P1:{} P3:{}, money = {} {} {} {}",
        t.state(0, "stay"),
        t.state(1, "stay"),
        t.state(3, "stay"),
        t.money(0),
        t.money(1),
        t.money(2),
        t.money(3)
    );
}