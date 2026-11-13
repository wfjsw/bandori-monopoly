//! SKILL-06: the A/B-class vocabulary behind deck construction, hand
//! building, shared deed ownership and the budgeted activation tags.
//!
//! One block per sub-site. Each carries the sheet clause it pins; the
//! rulebook text is the source, never the implementation. Deck gates are read
//! through `game_core::deck` (the `act("deck")` path's own gate), everything
//! else through real presses and settles on the fixture roster.

mod common;
use common::*;

use game_core::msg::Arg;
use game_core::state::{stage, MatchEvent};

const BAND_MUJICA: &str = "skill:Ave Mujica:假面之下的真实";
const MUTSUMI: &str = "skill:若叶睦（CRYCHIC）:精致的人偶";
const SAKIKO: &str = "skill:丰川祥子（CRYCHIC）:你愿意和我组建乐队吗？";
const UMIRI: &str = "skill:八幡海铃:熟练的支援贝斯手";
const RUI: &str = "skill:八潮瑠唯:正论暴击";
const RUI_CARD: &str = "Mor:（Rui）正论恶魔";
const TAE: &str = "skill:花园多惠:花园警察，出警！";
const SOYO: &str = "skill:长崎素世:通透的颜色";
const HILL_NAME: &str = "星之鼓动山丘";
const TEMP: &str = "tempFire";

// ---------------------------------------------------------- local helpers

/// Answer every open prompt with its decline.
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
    for _ in 0..60 {
        if t.turn() == who && t.step() == stage::OPS {
            return;
        }
        let cur = t.turn();
        if t.turn() == who {
            t.end(who).unwrap();
            drain(t);
            continue;
        }
        pass(t, cur);
    }
    panic!("never reached turn {who} (at {})", t.turn());
}

/// Put `event` on top of the event deck and land `who` on the CiRCLE 咖啡厅
/// corner (`faces` are the dice the body rolls, after the 1-step landing).
fn draw_event_card(t: &mut Table, who: usize, event: &str, faces: &[i32]) {
    let mut deck: Vec<String> = t.event_deck().into_iter().filter(|e| e != event).collect();
    deck.insert(0, event.to_string());
    let deck: Vec<&str> = deck.iter().map(String::as_str).collect();
    t.set_event_deck(&deck);
    until_turn(t, who);
    let corner = tile("CiRCLE 咖啡厅");
    t.set_pos(who, (corner + 59) % 60);
    let mut all = vec![1];
    all.extend_from_slice(faces);
    t.dice(&all);
    t.roll(who).unwrap();
}

fn co_owners(t: &Table, t44: usize) -> Vec<i32> {
    t.m.world().st.co_owners[t44].clone()
}

/// The cause key a payment event carries (the nested `log.part.reason` in a
/// `log.pay`'s `src`), if any. `log_key` only unwraps `log.card_effect`, so
/// the temp-pot provenance has to be read out of the args by hand.
fn pay_cause(e: &MatchEvent) -> Option<String> {
    let Arg::Msg(src) = e.msg.args.get("src")? else {
        return None;
    };
    let Arg::Msg(reason) = src.as_ref().args.get("reason")? else {
        return None;
    };
    Some(reason.key().to_string())
}

// =====================================================================
// 1. CiRCLE band （1）: 「只能在卡组中加入"通用"卡」
// =====================================================================

#[test]
fn circle_deck_admits_only_general_cards() {
    let d = data();
    let c = d.persona("都筑诗船").expect("CiRCLE persona");
    // A group card outside 通用 is rejected; the CiRCLE characters' own cards
    // are all 通用 rows in this roster, so any group card proves the point.
    let group_card = d
        .cards
        .iter()
        .find(|k| !k.is_universal() && !k.exclusive() && !k.derived)
        .expect("a group card");
    assert_eq!(
        game_core::deck::cant_play(d.as_ref(), c, group_card)
            .expect("group card rejected")
            .key(),
        "err.deck_general_only"
    );
    let exclusive_elsewhere = d
        .cards
        .iter()
        .find(|k| k.exclusive() && !k.is_universal() && !k.derived)
        .expect("an exclusive card");
    assert_eq!(
        game_core::deck::cant_play(d.as_ref(), c, exclusive_elsewhere)
            .expect("exclusive rejected")
            .key(),
        "err.deck_general_only"
    );
    // 通用 cards are admitted; a ten-card pool slice made of them is a legal
    // deck (`clean` accepts every one).
    let pool: Vec<String> = game_core::deck::pool(d.as_ref(), c)
        .into_iter()
        .map(|k| k.id.clone())
        .collect();
    assert!(
        pool.iter().all(|id| {
            d.card(id).is_some_and(|k| k.is_universal())
        }),
        "the CiRCLE pool is 通用-only: {pool:?}"
    );
    assert!(pool.len() >= 10, "the 通用 pool has room for 10");
    let ten: Vec<String> = pool.iter().take(10).cloned().collect();
    for id in &ten {
        let k = d.card(id).expect("pool card");
        assert!(
            game_core::deck::cant_play(d.as_ref(), c, k).is_none(),
            "通用 {id} is admitted"
        );
    }
    let cleaned = game_core::deck::clean(d.as_ref(), c, &ten);
    assert_eq!(cleaned.len(), 10, "all ten survive clean");
}

// =====================================================================
// 2. Sumimi band （3）: 「你可携带两个角色的专属卡牌」
// =====================================================================

#[test]
fn sumimi_carries_a_partners_exclusive() {
    let d = data();
    let mana = d.persona("纯田真奈").expect("Sumimi persona");
    let card = d
        .card("Sumimi:(初华（Sumimi）)儿时玩伴的鼓励")
        .expect("the partner's exclusive");
    assert!(card.exclusive(), "the card is exclusive to the other seat");
    assert_ne!(card.owner, mana.name);
    assert!(
        game_core::deck::cant_play(d.as_ref(), mana, card).is_none(),
        "a Sumimi member carries the partner's exclusive"
    );
    assert!(
        game_core::deck::pool(d.as_ref(), mana)
            .iter()
            .any(|k| k.id == card.id),
        "the card is in the deck pool"
    );
    // The rest of the roster is unaffected: a CRYCHIC member still cannot.
    let tomori = d.persona("高松灯（CRYCHIC）").expect("CRYCHIC persona");
    assert_eq!(
        game_core::deck::cant_play(d.as_ref(), tomori, card)
            .expect("outside the group")
            .key(),
        "err.deck_exclusive"
    );
}

// =====================================================================
// 3. Poppin' Party band （3）: 「游戏开始时所有Poppin' Party角色视为同时拥有
//    "星之鼓动山丘"，可正常抵押…非Poppin' Party角色…改为分摊支付给所有未抵押
//    "星之鼓动山丘"的Poppin' Party角色」
// =====================================================================

#[test]
fn poppin_hill_is_co_owned_at_game_start() {
    let t = Table::new(&["户山香澄", "花园多惠", "青叶摩卡"]);
    let hill = tile(HILL_NAME);
    assert_eq!(t.owner(hill), Some(0), "a primary holder exists");
    let mut c = co_owners(&t, hill);
    c.sort();
    assert_eq!(c, vec![0, 1], "every band character is a holder");
    assert!(t.m.world().owns(hill as i32, 0));
    assert!(t.m.world().owns(hill as i32, 1));
    assert!(!t.m.world().owns(hill as i32, 2), "no non-band holder");
}

#[test]
fn poppin_member_lands_on_the_hill_as_an_owner() {
    let mut t = Table::new(&["户山香澄", "青叶摩卡"]);
    let hill = tile(HILL_NAME);
    let m = t.money(0);
    t.set_pos(0, hill - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), hill);
    assert_eq!(t.money(0), m, "a holder pays no rent on the hill");
}

#[test]
fn poppin_hill_rent_splits_across_unmortgaged_members() {
    let mut t = Table::new(&["户山香澄", "花园多惠", "青叶摩卡"]);
    let hill = tile(HILL_NAME);
    let rent = data().tiles[hill].rent[0];
    let (m0, m1, m2) = (t.money(0), t.money(1), t.money(2));
    t.begin_turn(2);
    t.set_pos(2, hill - 1);
    t.dice(&[1]);
    t.roll(2).unwrap();
    drain(&mut t);
    assert_eq!(t.money(2), m2 - rent, "the non-member pays the rent");
    assert_eq!(t.money(0), m0 + rent / 2, "half to the first member");
    assert_eq!(
        t.money(1),
        m1 + rent / 2,
        "half to the second member, not the primary alone"
    );

    // 「未抵押"星之鼓动山丘"的」 -- a mortgaged member is off the split. A
    // mortgage is an 运营阶段 act of the holder's own turn.
    until_turn(&mut t, 1);
    t.mortgage(1, hill).unwrap();
    let (m0, m1, m2) = (t.money(0), t.money(1), t.money(2));
    t.begin_turn(2);
    t.set_pos(2, hill - 1);
    t.dice(&[1]);
    t.roll(2).unwrap();
    drain(&mut t);
    assert_eq!(t.money(2), m2 - rent, "the payer still pays the full rent");
    assert_eq!(t.money(0), m0 + rent, "the unmortgaged member takes it all");
    assert_eq!(t.money(1), m1, "the mortgaged member gets nothing");
}

#[test]
fn poppin_member_mortgage_is_per_stake() {
    let mut t = Table::new(&["户山香澄", "花园多惠", "青叶摩卡"]);
    let hill = tile(HILL_NAME);
    let (m0, m1) = (t.money(0), t.money(1));
    // Each holder mortgages their own stake: the deed stays un-mortgaged for
    // the other, and both payouts land. A mortgage is an 运营阶段 act of the
    // holder's own turn, so hand the turn between the two.
    t.mortgage(0, hill).unwrap();
    until_turn(&mut t, 1);
    t.mortgage(1, hill).unwrap();
    let half = data().tiles[hill].price / 2;
    assert_eq!(t.money(0), m0 + half, "P0's own stake");
    assert_eq!(t.money(1), m1 + half, "P1's own stake");
    assert!(t.mortgaged(hill), "the aggregate reads mortgaged");
    // Redeeming one stake clears only that one.
    until_turn(&mut t, 0);
    t.redeem(0, hill).unwrap();
    assert!(!t.m.world().co_mortgaged_by(hill as i32, 0));
    assert!(t.m.world().co_mortgaged_by(hill as i32, 1));
}

// =====================================================================
// 4. 今井莉莎 （3）: 「你提供的临时火罐将会在两回合后失去，使用你提供的临时
//    火罐需要支付你600资金（被冲榜类效果被动消耗无需支付）」
// =====================================================================

/// Press lisa's （2） to hand P1 a temp pot: both fire and the cancel answer.
fn lisa_grants_temp(t: &mut Table) {
    t.set_fire(0, 2, 2);
    t.begin_turn(0);
    t.set_pos(0, 10);
    t.set_pos(1, 20);
    t.skill(0, &t.skill_id(0, "慈爱女神")).unwrap();
    // 「你可额外消耗一个火罐…若你这样做」 -- answer yes.
    t.answer(0, 0).unwrap();
    drain(t);
}

#[test]
fn lisa_temp_pot_charges_its_provider_on_use() {
    let mut t = Table::new(&["今井莉莎", "青叶摩卡"]);
    t.clean();
    lisa_grants_temp(&mut t);
    assert_eq!(t.token(1, TEMP), 1, "the temp pot landed");
    assert_eq!(t.state(1, "fire.tempProvider"), 1, "P0 is the provider");
    // P1 spends it actively (a card effect): the use pays the provider 600.
    t.begin_turn(1);
    let mark = t.mark();
    t.give_play(1, "TEST:spendFire").unwrap();
    drain(&mut t);
    assert_eq!(t.token(1, TEMP), 0, "the pot was the one spent");
    assert_eq!(t.money(1), 10_000 - 600, "the user pays");
    assert_eq!(t.money(0), 10_000 + 600, "the provider is paid");
    assert!(
        t.events_since(mark)
            .iter()
            .any(|e| pay_cause(e).is_some_and(|k| k.ends_with("fire_temp_pay"))),
        "{:?}",
        t.keys_since(mark)
    );
}

#[test]
fn lisa_temp_pot_expires_after_two_turns() {
    let mut t = Table::new(&["今井莉莎", "青叶摩卡"]);
    t.clean();
    lisa_grants_temp(&mut t);
    assert_eq!(t.token(1, TEMP), 1);
    // The grant was on P0's turn; its expiry is two turn keys on. P1's turn
    // does not drop it, P0's next turn start does.
    t.begin_turn(1);
    assert_eq!(t.token(1, TEMP), 1, "still alive one turn later");
    t.begin_turn(0);
    assert_eq!(t.token(1, TEMP), 0, "「两回合后失去」");
    assert_eq!(t.state(1, "fire.tempProvider"), 0, "the stamp is gone too");
    assert!(
        t.recent_keys(12)
            .iter()
            .any(|k| k.ends_with("fire_temp_expired")),
        "{:?}",
        t.recent_keys(12)
    );
}

#[test]
fn lisa_temp_pot_is_free_on_a_passive_spend() {
    let mut t = Table::new(&["今井莉莎", "青叶摩卡", "花园多惠"]);
    t.clean();
    lisa_grants_temp(&mut t);
    assert_eq!(t.token(1, TEMP), 1);
    // The grant's teleport consumed P0's main move; give it a fresh turn.
    t.begin_turn(0);
    // 「被冲榜类效果被动消耗无需支付」: 冲榜 takes every pot, no 600.
    // Seat order rolls: P0 5, P1 10, P2 20 -- P0 is rank 3 (1000), P2 rank 1.
    let m0 = t.money(0);
    let mark = t.mark();
    draw_event_card(&mut t, 0, "冲榜", &[5, 10, 20]);
    while let Some(p) = t.prompt() {
        if p.kind == "choice" && !t.asked().is_empty() {
            t.answer(t.asked()[0], 0).unwrap();
        } else {
            t.decline();
        }
    }
    assert_eq!(t.token(1, TEMP), 0, "the temp pot was lost to the event");
    // P0 is rank 3 (1000) -- and no 600 from the temp pot.
    assert_eq!(t.money(0), m0 + 1000, "no provider charge on a passive spend");
    assert!(
        !t.events_since(mark)
            .iter()
            .any(|e| pay_cause(e).is_some_and(|k| k.ends_with("fire_temp_pay"))),
        "{:?}",
        t.keys_since(mark)
    );
}

// =====================================================================
// 5. 若叶睦（CRYCHIC）（3） / 丰川祥子（CRYCHIC）（4）:
//    「只有洗牌时可以为你的乐队技能卡上添加奇迹水晶」
// =====================================================================

/// P0 in Ave Mujica 状态2 with `face` as the character skill, ready for a
/// turn start. Hand size 3 keeps the two-state axis in 2.
fn mujica_in_two(face: &str) -> Table {
    let mut t = Table::new(&["八幡海铃", "青叶摩卡"]);
    t.clean();
    t.strip_skills();
    t.set_character_raw(0, "若叶睦（CRYCHIC）");
    t.place_raw(0, BAND_MUJICA);
    t.place_raw(0, face);
    t.set_fire(0, 1, 1);
    t.set_hand(0, &["通用:GREAT", "通用:GREAT", "通用:GREAT"]);
    t.begin_turn(0);
    drain(&mut t);
    t.set_state(0, "skillState", 2);
    t
}

#[test]
fn band_third_turn_add_stands_down_on_a_crychic_face() {
    // The band's own 「处于状态2的第三回合开始时为此卡添加一个奇迹水晶」. The
    // setup already ran the first (state 0 -> 1) start; the third start in
    // state 2 is the third more.
    let mut t = mujica_in_two(MUTSUMI);
    for _ in 0..3 {
        t.begin_turn(0);
        drain(&mut t);
    }
    assert_eq!(t.crystals(0, BAND_MUJICA), Some(0), "no add on the third start");
    // Control: a face without the carve-out still takes the band's add.
    let mut u = mujica_in_two(UMIRI);
    for _ in 0..3 {
        u.begin_turn(0);
        drain(&mut u);
    }
    assert_eq!(
        u.crystals(0, BAND_MUJICA),
        Some(1),
        "the band add still fires for an ordinary face"
    );
}

/// One reshuffle with the face bound: the add rides `Reshuffled`.
fn reshuffle_adds(face: &str) {
    let mut t = mujica_in_two(face);
    t.set_draw(0, &[]);
    t.set_discard(0, &["通用:GREAT", "通用:GREAT"]);
    t.give(0, &["TEST:drawer"]);
    let mark = t.mark();
    t.play(0, "TEST:drawer").unwrap();
    drain(&mut t);
    assert_eq!(
        t.crystals(0, BAND_MUJICA),
        Some(1),
        "「只有洗牌时可以…添加」: {:?}",
        t.keys_since(mark)
    );
    assert!(
        t.keys_since(mark)
            .iter()
            .any(|k| k.ends_with("counter_reshuffled")),
        "the add is tagged as a reshuffle: {:?}",
        t.keys_since(mark)
    );
}

#[test]
fn mutsumi_face_adds_on_a_reshuffle() {
    reshuffle_adds(MUTSUMI);
}

#[test]
fn sakiko_face_adds_on_a_reshuffle() {
    reshuffle_adds(SAKIKO);
}

// =====================================================================
// 6. 八潮瑠唯 （1）: 「如果你的（2）技能或角色卡被复制，当复制者使x重置为0时，
//    对方失去该技能（2）（抵押或赎回以外）」
// =====================================================================

#[test]
fn rui_copy_reset_costs_the_source_the_skill() {
    let mut t = Table::new(&["八潮瑠唯", "八潮瑠唯"]);
    t.clean();
    t.strip_skills();
    // P0 owns the skill; P1 carries a copy subject to a different skill
    // (`persona_skill` names the first one, so P1's is not the RUI id).
    t.place_raw(0, RUI);
    t.place_raw(1, "skill:花园多惠:花园警察，出警！");
    t.place_raw(1, RUI);
    // P1's turn plays the card; set X after its turn start (the source's
    // `declare` reset it).
    t.begin_turn(1);
    t.set_state(0, "skill.yuriCrit.x", 5);
    t.give(1, &[RUI_CARD]);
    // First payment: P0's own body misses (20 > 5, X -> 6), the copier's
    // hits (1 <= 6) and resets the source's X to 0.
    t.dice(&[20, 1, 1, 1, 1, 1, 1, 1, 1, 1]);
    let mark = t.mark();
    t.play(1, RUI_CARD).unwrap();
    drain(&mut t);
    let keys = t.keys_since(mark);
    let lost = keys
        .iter()
        .position(|k| k.ends_with("rui_lost"))
        .expect("the copier's reset logs the source's loss");
    assert_eq!(t.state(0, "skill.yuriCrit.lost"), 1, "the loss latch");
    // The reset was the source's X (the copier read 6 there), not its own 0.
    let copier_hit_x = t
        .events_since(mark)
        .iter()
        .find(|e| {
            e.player_id == 1
                && log_key(e).ends_with("rui_hit")
                && int_arg(&e.msg, "x").is_some()
        })
        .and_then(|e| int_arg(&e.msg, "x"));
    assert_eq!(copier_hit_x, Some(6), "the copier reset the source's X");
    // From the loss on, the source's own body is off: no further P0-side
    // hit / miss on the later legs (`events_since` is index-aligned with
    // `keys_since`).
    let p0_fires_after = t
        .events_since(mark)
        .iter()
        .enumerate()
        .skip(lost + 1)
        .filter(|(_, e)| e.player_id == 0)
        .filter(|(_, e)| {
            let k = log_key(e);
            k.ends_with("rui_hit") || k.ends_with("rui_miss")
        })
        .count();
    assert_eq!(p0_fires_after, 0, "the source's （2） is gone: {keys:?}");
}

// =====================================================================
// 7. 花园多惠 （2）: 「使用角色/团技能的[主]效果或使用手牌的[手]效果时」
// =====================================================================

/// P0 is 多惠 with four fire; P1 plays `card` from hand.
fn tae_against(card: &str) -> Table {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "花园多惠");
    t.place_raw(0, TAE);
    t.set_fire(0, 4, 4);
    t.begin_turn(1);
    t.give(1, &[card]);
    t.dice(&[1, 1, 1]);
    t.play(1, card).unwrap();
    t
}

#[test]
fn tae_counteracts_a_hand_card_with_the_tag() {
    // 通用:GREAT carries the [手] tag.
    let t = tae_against("通用:GREAT");
    assert!(
        t.counteract_offered("花园警察"),
        "a [手] hand play opens the offer: {}",
        t.dump_prompt()
    );
}

#[test]
fn tae_does_not_counteract_a_hand_card_without_the_tag() {
    // TEST:drawer has no tags at all, so it is not a 「[手]效果」.
    let t = tae_against("TEST:drawer");
    assert!(
        !t.counteract_offered("花园警察"),
        "a tagless hand play is not offered: {}",
        t.dump_prompt()
    );
}

// =====================================================================
// 8. 长崎素世 （2）: 「若以此法单次免除了至少1500资金的[支付]，则你下次经过
//    CiRCLE时不获得火罐」
// =====================================================================

#[test]
fn soyo_waiver_denies_the_next_circle_pot() {
    let mut t = Table::new(&["长崎素世", "青叶摩卡"]);
    t.clean();
    let agent = tile("弦卷集团");
    let target = tile("弦卷豪宅"); // 3600 unowned: the baseline to waive
    let cheap = tile("微笑号"); // P1's, rent 300 -> the agent charges 150
    t.own(1, &[cheap]);
    t.set_fire(0, 1, 1);
    t.begin_turn(0);
    t.set_pos(0, agent);
    t.dice(&[1]);
    t.roll(0).unwrap();
    eprintln!("after roll: {}", t.dump_prompt());
    // 「移动掷骰后可消耗一个火罐…」 -- take it.
    t.answer(0, 0).unwrap();
    eprintln!("after answer: {}", t.dump_prompt());
    // The re-coloured landing is the agent tile's own settle; settle the
    // cheap other-owned square instead of buying (the buy is automatic): the
    // ordinary property would have charged the 3600 buy price.
    t.answer_tile(0, cheap).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), target);
    assert_eq!(t.money(0), 10_000 - 150, "the agent's half rent on 微笑号");
    assert_eq!(
        t.state(0, "skill.soyoClear.penalty"),
        1,
        "a single waiver of 3600 latches the penalty: {:?}",
        t.recent_keys(16)
    );
    assert!(
        t.recent_keys(16).iter().any(|k| k.ends_with("soyo_waived")),
        "{:?}",
        t.recent_keys(16)
    );
    // 「你下次经过CiRCLE时不获得火罐」 -- the next pass gains nothing (and the
    // latch is consumed).
    let fire = t.charge(0);
    t.end(0).unwrap();
    drain(&mut t);
    t.begin_turn(0);
    let mark = t.mark();
    t.set_pos(0, 59);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 0, "landed on CiRCLE");
    assert_eq!(t.charge(0), fire, "no pot from the pass");
    assert!(
        t.keys_since(mark).iter().any(|k| k.ends_with("soyo_denied")),
        "{:?}",
        t.keys_since(mark)
    );
    assert!(
        !t.keys_since(mark).iter().any(|k| k.ends_with("soyo_gain")),
        "the gain is denied: {:?}",
        t.keys_since(mark)
    );
    assert_eq!(t.state(0, "skill.soyoClear.penalty"), 0, "the latch is consumed");
}
