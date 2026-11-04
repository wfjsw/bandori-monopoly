//! G07 integration tests: the one designation pipeline, the between-turns
//! count and the nearest queries.
//!
//! Plan: `docs/rulebook/spec/abi/G07-targeting-and-distance.md` §10.1 (one
//! test per table row) and §10.2 (the query matrix). Expectations come from the
//! quoted card text and the design's §4, §7 and §8, never from the engine.
//!
//! Seat lists are asserted as bit masks (seat `s` = bit `s`) so the whole
//! ordered set is pinned with one number; tie order is checked by the lowest
//! set bit.

mod common;

use bandori_std::nearest::EXEMPTION;
use common::*;
use game_core::msg::Arg;
use game_rules::CardWorld;

const AIMER: &str = "TEST:aimer";
const TARGET_SELF: &str = "TEST:target_self";
const TARGET_TILE: &str = "TEST:target_tile";
const TARGET_ALL: &str = "TEST:target_all";
const NEAREST: &str = "TEST:nearest";
const TARGET_COUNTER: &str = "TEST:target_counter";
const TARGETED_PROBE: &str = "TEST:targeted_probe";
const EXEMPTION_PROBE: &str = "TEST:exemption_probe";
const CENTRIFUGAL: &str = "Mor:离心力，不为所动";
const DECLARE_WAR: &str = "AG:宣战布告";
const CAMP: &str = "Mor:夏日合宿";
const WELCOME: &str = "Mujica:欢迎来到ave mujica的世界";
const RAIN: &str = "通用:雨啊，快点来吧";
const EXIST: &str = "RAS:EXIST";
const NET_ERROR: &str = "通用:网络链接异常";
const LISA_LINK: &str = "R:必然的联系（莉莎）";
const SKILL_BAND: &str = "skill:Ave Mujica:假面之下的真实";

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

/// How many other players' designations named `seat` since its own turn start.
fn count(t: &Table, seat: usize) -> i32 {
    t.m.world().targeted.get(seat).copied().unwrap_or(0)
}

fn event_int(t: &Table, mark: i32, key: &str, name: &str) -> Option<i64> {
    t.events_since(mark)
        .iter()
        .find(|e| log_key(e).ends_with(key) && int_arg(&e.msg, name).is_some())
        .and_then(|e| int_arg(&e.msg, name))
}

/// Answer the open player pick with the option naming `seat`.
fn pick(t: &mut Table, who: usize, seat: usize) {
    let p = t.expect_prompt();
    let k = p
        .options
        .iter()
        .position(|o| format!("{o:?}").contains(&format!("PlayerId({seat})")))
        .unwrap_or_else(|| panic!("seat {seat} not offered: {}", t.dump_prompt()));
    t.answer(who, k as i32).unwrap();
}

/// Drain everything, recording whether `card` was offered at any point.
fn drain_offers(t: &mut Table, card: &str) -> bool {
    let mut offered = false;
    for _ in 0..80 {
        if t.prompt().is_none() {
            break;
        }
        if t.counteract_offered(card) {
            offered = true;
        }
        t.decline();
    }
    offered
}

/// Answer the open window with `card` as `who`, declining anything else until
/// it comes up.
fn declare_when_offered(t: &mut Table, who: usize, card: &str) {
    for _ in 0..40 {
        if t.counteract_offered(card) && t.asked().contains(&who) {
            t.counteract(who, card).unwrap();
            return;
        }
        assert!(t.prompt().is_some(), "{card} never offered to {who}");
        t.decline();
    }
    panic!("{card} never offered: {}", t.dump_prompt());
}

/// The `to` of the open counter prompt (the link's presented target): the
/// effect-stage offer carries it, the card-stage one does not.
fn offer_target(t: &Table) -> Option<i32> {
    let p = t.prompt()?;
    let Arg::Msg(d) = p.text.args.get("detail")? else {
        return None;
    };
    match d.args.get("to")? {
        Arg::PlayerId(v) | Arg::Tile(v) => Some(*v),
        Arg::Integer(v) => Some(*v as i32),
        _ => None,
    }
}

/// The clean, skill-less, three-seat table with `seat` on turn.
fn at_rest(chars: &[&str], seat: usize) -> Table {
    let mut t = Table::new(chars);
    t.strip_skills();
    t.clean();
    t.begin_turn(seat);
    drain(&mut t);
    t
}

// =====================================================================
// Item 1a -- the between-turns count
// =====================================================================

#[test]
fn g07_centrifugal_own_turn_targeting_is_not_counted() {
    // 规则书: 「在两个你的回合之间（回合1结束，回合2开始）第二次成为其他角色的
    // 目标时」 -- P1 answers P0's own two plays with 宣战布告, which names P0 back.
    // Both names happen during P0's own turn, so P0's count stays 0 and the card
    // is not offered; a targeting *after* the turn ended opens the window on the
    // second one.
    let mut t = at_rest(&["花园多惠", "户山香澄", "山吹沙绫"], 0);
    t.give(0, &[CENTRIFUGAL, AIMER, AIMER]);
    t.give(1, &[DECLARE_WAR, DECLARE_WAR]);
    for _ in 0..2 {
        t.play(0, AIMER).unwrap();
        declare_when_offered(&mut t, 1, DECLARE_WAR);
        drain(&mut t);
    }
    assert_eq!(count(&t, 0), 0, "an own-turn name does not count");
    assert_eq!(count(&t, 1), 2, "the counters' own names do count");
    assert!(
        !drain_offers(&mut t, CENTRIFUGAL),
        "no offer inside the owner's own turn"
    );

    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    t.end(0).unwrap();
    drain(&mut t);
    t.give(1, &[AIMER, AIMER]);
    t.play(1, AIMER).unwrap();
    assert_eq!(count(&t, 0), 1, "the first between-turns name counts once");
    assert!(
        !drain_offers(&mut t, CENTRIFUGAL),
        "one between-turns name is not yet the second"
    );
    t.play(1, AIMER).unwrap();
    assert_eq!(count(&t, 0), 2, "the second between-turns name counts");
    assert!(
        t.counteract_offered(CENTRIFUGAL),
        "the second name offers the card: {}",
        t.dump_prompt()
    );
}

#[test]
fn g07_centrifugal_second_between_turns_targeting_offers() {
    // 规则书: 「第二次成为其他角色技能或卡牌的目标时，你可以打出此卡」
    let mut t = at_rest(&["花园多惠", "户山香澄", "山吹沙绫"], 1);
    t.give(0, &[CENTRIFUGAL]);
    t.give(1, &[WELCOME, WELCOME]);
    t.play(1, WELCOME).unwrap();
    pick(&mut t, 1, 0);
    drain(&mut t);
    assert_eq!(count(&t, 0), 1);
    assert!(
        !drain_offers(&mut t, CENTRIFUGAL),
        "the first targeting does not offer"
    );
    t.play(1, WELCOME).unwrap();
    pick(&mut t, 1, 0);
    assert!(
        t.counteract_offered(CENTRIFUGAL),
        "the second targeting offers: {}",
        t.dump_prompt()
    );
}

#[test]
fn g07_centrifugal_window_closes_at_own_turn_start() {
    // A between-turns name, then the owner's own turn start zeroes the count; a
    // fresh name during that turn does not replace it.
    let mut t = at_rest(&["花园多惠", "户山香澄", "山吹沙绫"], 1);
    t.give(0, &[CENTRIFUGAL, AIMER]);
    t.give(1, &[AIMER]);
    t.play(1, AIMER).unwrap();
    drain(&mut t);
    assert_eq!(count(&t, 0), 1);
    t.begin_turn(0);
    assert_eq!(count(&t, 0), 0, "own turn start zeroes the count");
    t.play(0, AIMER).unwrap();
    drain(&mut t);
    assert_eq!(count(&t, 0), 0, "an own-turn name does not put one back");
    assert!(
        !drain_offers(&mut t, CENTRIFUGAL),
        "no offer after the window closed"
    );
}

#[test]
fn g07_centrifugal_count_is_per_seat() {
    let mut t = at_rest(&["花园多惠", "户山香澄", "山吹沙绫"], 1);
    t.give(1, &[WELCOME, AIMER]);
    t.play(1, WELCOME).unwrap();
    pick(&mut t, 1, 2);
    drain(&mut t);
    assert_eq!(count(&t, 2), 1, "P2 was named once");
    assert_eq!(count(&t, 0), 0, "P0 was not named");
    t.play(1, AIMER).unwrap();
    drain(&mut t);
    assert_eq!(count(&t, 0), 1, "P0 was named once");
    assert_eq!(count(&t, 2), 1, "P2's count is separate");
}

#[test]
fn g07_named_then_negated_still_counts() {
    // 网络链接异常 cancels the designation of one target. `宣战布告`'s count
    // reads the naming, not the landing -- the second name still offers.
    let mut t = at_rest(&["花园多惠", "户山香澄", "山吹沙绫"], 1);
    t.give(0, &[CENTRIFUGAL, NET_ERROR]);
    t.give(1, &[AIMER, AIMER]);
    let mark = t.mark();
    t.play(1, AIMER).unwrap();
    // 网络链接异常 answers at the card stage too (cancelling before the name is
    // made); the *effect* window is the one that cancels after the count.
    let mut cancelled = false;
    for _ in 0..30 {
        if t.counteract_offered(NET_ERROR) && offer_target(&t) == Some(0) {
            t.counteract(0, NET_ERROR).unwrap();
            cancelled = true;
            break;
        }
        assert!(t.prompt().is_some(), "the effect window never opened");
        t.decline();
    }
    assert!(cancelled, "the effect-stage offer came");
    drain(&mut t);
    assert_eq!(
        event_int(&t, mark, "aimer_done", "got"),
        Some(-1),
        "the designation was cancelled"
    );
    assert_eq!(count(&t, 0), 1, "the cancelled name still counted");
    assert!(
        !drain_offers(&mut t, CENTRIFUGAL),
        "one name is not the second"
    );
    t.play(1, AIMER).unwrap();
    assert!(
        t.counteract_offered(CENTRIFUGAL),
        "the second name offers: {}",
        t.dump_prompt()
    );
}

#[test]
fn g07_refused_designation_does_not_count() {
    // 夏日合宿's `Untargetable` refuses another player's naming outright.
    let mut t = at_rest(&["花园多惠", "户山香澄"], 1);
    t.place_raw(0, CAMP);
    t.give(1, &[AIMER]);
    let mark = t.mark();
    t.play(1, AIMER).unwrap();
    drain(&mut t);
    assert_eq!(event_int(&t, mark, "aimer_done", "got"), Some(-1));
    assert_eq!(count(&t, 0), 0, "a refused name never reaches the count");
}

// =====================================================================
// Item 1b -- tile designations count too
// =====================================================================

/// P1's turn; P0 owns `mine`; the probe sees every raise.
fn tile_table(chars: usize) -> (Table, usize) {
    let mut t = at_rest(&["花园多惠", "户山香澄", "山吹沙绫"][..chars], 1);
    let mine = tile("月之森女子学院");
    t.set_owner(mine, Some(0));
    t.place_raw(2, TARGETED_PROBE);
    t.give(0, &[CENTRIFUGAL]);
    t.give(1, &[TARGET_TILE, TARGET_TILE]);
    (t, mine)
}

fn hit_tile(t: &mut Table, who: usize, tile: usize) {
    t.play(who, TARGET_TILE).unwrap();
    t.answer_tile(who, tile).unwrap();
}

#[test]
fn g07_centrifugal_tile_targeting_counts() {
    let (mut t, mine) = tile_table(3);
    let mark = t.mark();
    hit_tile(&mut t, 1, mine);
    drain(&mut t);
    assert_eq!(
        count(&t, 0),
        1,
        "a tile designation names its owner's count"
    );
    let who = event_int(&t, mark, "targeted_probe_done", "who");
    let at = event_int(&t, mark, "targeted_probe_done", "tile");
    let by = event_int(&t, mark, "targeted_probe_done", "by");
    assert_eq!(who, Some(0), "the owner is named: {:?}", t.keys_since(mark));
    assert_eq!(at, Some(mine as i64));
    assert_eq!(by, Some(1));
}

#[test]
fn g07_centrifugal_tile_targeting_offers() {
    let (mut t, mine) = tile_table(3);
    hit_tile(&mut t, 1, mine);
    drain(&mut t);
    assert!(!drain_offers(&mut t, CENTRIFUGAL), "one hit is not two");
    hit_tile(&mut t, 1, mine);
    assert!(
        t.counteract_offered(CENTRIFUGAL),
        "the second hit offers: {}",
        t.dump_prompt()
    );
    // The window carries the tile: the offer is for the tile hit, not a player.
    let p = t.expect_prompt();
    assert!(
        format!("{:?}", p.text).contains("tile"),
        "the window carries `tile >= 0`: {}",
        t.dump_prompt()
    );
}

#[test]
fn g07_centrifugal_own_tile_does_not_count() {
    // The owner names their own tile: `owner == by`, so the count stands still
    // but the hook still fires.
    let (mut t, mine) = tile_table(3);
    t.set_owner(mine, Some(1));
    let mark = t.mark();
    hit_tile(&mut t, 1, mine);
    drain(&mut t);
    assert_eq!(count(&t, 1), 0);
    assert_eq!(event_int(&t, mark, "targeted_probe_done", "who"), Some(1));
    assert_eq!(
        event_int(&t, mark, "targeted_probe_done", "tile"),
        Some(mine as i64)
    );
}

#[test]
fn g07_tile_during_own_turn_does_not_count() {
    // A designation during the named seat's own turn is before the window.
    let (mut t, mine) = tile_table(3);
    t.set_owner(mine, Some(1));
    t.begin_turn(1);
    hit_tile(&mut t, 1, mine);
    drain(&mut t);
    assert_eq!(count(&t, 1), 0, "own-turn tile designation counts 0");
}

// =====================================================================
// Item 1c -- the self tail runs the pipeline
// =====================================================================

/// A welcome table whose P0 can really switch state; the probe watches P2's
/// field.
fn welcome_table() -> Table {
    let mut t = at_rest(&["丰川祥子", "三角初华", "户山香澄"], 0);
    t.place_raw(2, TARGETED_PROBE);
    t.place_raw(0, SKILL_BAND);
    t.set_state(0, "skillState", 1);
    t.give(0, &[WELCOME]);
    t
}

fn welcome_self(t: &mut Table) {
    t.play(0, WELCOME).unwrap();
    pick(t, 0, 0);
}

#[test]
fn g07_welcome_self_target_runs_the_target_window() {
    let mut t = welcome_table();
    let mark = t.mark();
    welcome_self(&mut t);
    drain(&mut t);
    assert_eq!(
        event_int(&t, mark, "targeted_probe_done", "who"),
        Some(0),
        "the self name raises `targeted`: {:?}",
        t.keys_since(mark)
    );
    assert_eq!(event_int(&t, mark, "targeted_probe_done", "tile"), Some(-1));
    assert_eq!(
        event_int(&t, mark, "targeted_probe_done", "by"),
        Some(0),
        "the source is the picker"
    );
    assert_eq!(t.state(0, "skillState"), 2, "the pick still lands");
}

#[test]
fn g07_welcome_self_target_skips_immunity() {
    // 离心力's `ImmuneAll` is 「你受到的所有效果」 from *others*; one's own pick
    // lands through one's own shield.
    let mut t = welcome_table();
    t.place_raw(0, CENTRIFUGAL);
    welcome_self(&mut t);
    drain(&mut t);
    assert_eq!(
        t.state(0, "skillState"),
        2,
        "an own-source pick is not immune: {:?}",
        t.recent_keys(15)
    );
}

#[test]
fn g07_welcome_self_target_can_be_cancelled() {
    // 网络链接异常-ish: a window counter may cancel a self designation.
    let mut t = welcome_table();
    t.give(2, &[TARGET_COUNTER]);
    let mark = t.mark();
    welcome_self(&mut t);
    // The counter is offered inside the self window and names the user.
    let mut cancelled = false;
    for _ in 0..30 {
        if t.counteract_offered(TARGET_COUNTER) {
            t.counteract(2, TARGET_COUNTER).unwrap();
            cancelled = true;
            break;
        }
        assert!(t.prompt().is_some(), "the self window never offered");
        t.decline();
    }
    drain(&mut t);
    assert!(cancelled, "the counter was offered");
    assert_eq!(
        event_int(&t, mark, "target_counter_done", "who"),
        Some(0),
        "the counter saw the self name"
    );
    assert_eq!(
        t.state(0, "skillState"),
        1,
        "the cancelled pick does nothing"
    );
}

#[test]
fn g07_exist_redirects_a_self_designation() {
    // EXIST's parenthetical 「包括其他玩家指向自身的卡」 -- the redirect runs in
    // the same tail, so a self pick is re-named to the EXIST holder.
    let mut t = welcome_table();
    t.place_raw(1, EXIST);
    t.place_raw(1, SKILL_BAND);
    t.set_state(1, "skillState", 1);
    let mark = t.mark();
    welcome_self(&mut t);
    drain(&mut t);
    assert_eq!(
        event_int(&t, mark, "targeted_probe_done", "who"),
        Some(1),
        "the self name was redirected: {:?}",
        t.keys_since(mark)
    );
    assert_eq!(t.state(0, "skillState"), 1, "the user is no longer named");
    assert_eq!(t.state(1, "skillState"), 2, "the EXIST holder is");
}

#[test]
fn g07_rain_self_designation_uses_the_pipeline() {
    // 雨 names the user as part of its list; that name raises the hook and
    // stays out of the count.
    let mut t = at_rest(&["花园多惠", "户山香澄", "山吹沙绫"], 0);
    t.place_raw(2, TARGETED_PROBE);
    t.give(0, &[RAIN]);
    t.dice(&[1, 1]); // 2d2 -> X = 2: the user, then one other
    let mark = t.mark();
    t.play(0, RAIN).unwrap();
    // The play's counter window comes first, then the remaining pick.
    for _ in 0..20 {
        let Some(p) = t.prompt() else { break };
        if p.kind == "player" {
            pick(&mut t, 0, 1);
            break;
        }
        t.decline();
    }
    drain(&mut t);
    assert_eq!(
        event_int(&t, mark, "targeted_probe_done", "who"),
        Some(0),
        "the user's own name raises the hook: {:?}",
        t.keys_since(mark)
    );
    assert_eq!(event_int(&t, mark, "targeted_probe_done", "by"), Some(0));
    assert_eq!(count(&t, 0), 0, "a self name stays out of the count");
}

#[test]
fn g07_target_all_includes_the_actor_through_the_pipeline() {
    let mut t = at_rest(&["花园多惠", "户山香澄", "山吹沙绫"], 0);
    t.place_raw(2, TARGETED_PROBE);
    t.give(0, &[TARGET_ALL]);
    let mark = t.mark();
    t.play(0, TARGET_ALL).unwrap();
    drain(&mut t);
    // Actor + both others = 0b111.
    assert_eq!(
        event_int(&t, mark, "target_all_done", "got"),
        Some(0b111),
        "every seat in the list lands: {:?}",
        t.keys_since(mark)
    );
    assert_eq!(
        event_int(&t, mark, "targeted_probe_done", "who"),
        Some(0),
        "the actor's own entry still windows"
    );
    assert_eq!(count(&t, 0), 0, "the self entry does not count");
    assert_eq!(count(&t, 1), 1, "the others do");
    assert_eq!(count(&t, 2), 1);
}

#[test]
fn g07_tile_self_designation_can_be_cancelled() {
    // The own-tile window is real: a counter cancels it and `target_tile`
    // answers false.
    let (mut t, mine) = tile_table(3);
    t.set_owner(mine, Some(1));
    t.give(2, &[TARGET_COUNTER]);
    let mark = t.mark();
    hit_tile(&mut t, 1, mine);
    let mut cancelled = false;
    for _ in 0..30 {
        if t.counteract_offered(TARGET_COUNTER) {
            t.counteract(2, TARGET_COUNTER).unwrap();
            cancelled = true;
            break;
        }
        assert!(t.prompt().is_some(), "the own-tile window never offered");
        t.decline();
    }
    drain(&mut t);
    assert!(cancelled, "the counter was offered");
    assert_eq!(
        event_int(&t, mark, "target_tile_done", "got"),
        Some(0),
        "the own-tile name was cancelled: {:?}",
        t.keys_since(mark)
    );
    assert_eq!(event_int(&t, mark, "target_counter_done", "who"), Some(1));
}

#[test]
fn g07_own_tile_raises_the_targeted_hook() {
    let (mut t, mine) = tile_table(3);
    t.set_owner(mine, Some(1));
    let mark = t.mark();
    hit_tile(&mut t, 1, mine);
    drain(&mut t);
    assert_eq!(event_int(&t, mark, "targeted_probe_done", "who"), Some(1));
    assert_eq!(
        event_int(&t, mark, "targeted_probe_done", "tile"),
        Some(mine as i64),
        "the own tile is carried"
    );
}

// =====================================================================
// Item 2 -- the nearest queries
// =====================================================================

/// Four seats: P0 and P3 co-located, P1 and P2 tied ahead.
fn matrix() -> Table {
    let mut t = at_rest(&["花园多惠", "户山香澄", "山吹沙绫", "牛込里美"], 0);
    t.set_pos(0, 0);
    t.set_pos(1, 10);
    t.set_pos(2, 10);
    t.set_pos(3, 0);
    t.give(0, &[NEAREST]);
    t
}

fn mask_for(seats: &[usize]) -> i64 {
    seats.iter().fold(0i64, |m, &p| m | (1i64 << p))
}

#[test]
fn g07_nearest_forward_matches_the_legacy_loop() {
    let mut t = matrix();
    let mark = t.mark();
    t.play(0, NEAREST).unwrap();
    drain(&mut t);
    let fwd = event_int(&t, mark, "nearest_done", "fwd").unwrap();
    assert_eq!(fwd, mask_for(&[1, 2]), "both tied seats, seat order");
    // The legacy first-found pick is the lowest seat of the tied set.
    assert_eq!(fwd.trailing_zeros(), 1, "P1 is first");
}

#[test]
fn g07_nearest_shortest_returns_all_ties() {
    let mut t = matrix();
    let mark = t.mark();
    t.play(0, NEAREST).unwrap();
    drain(&mut t);
    let run = event_int(&t, mark, "nearest_done", "short").unwrap();
    assert_eq!(run, 0b1000, "the co-located seat is the only zero");
}

#[test]
fn g07_nearest_forward_excludes_co_located_shortest_includes() {
    let mut t = matrix();
    let mark = t.mark();
    t.play(0, NEAREST).unwrap();
    drain(&mut t);
    let fwd = event_int(&t, mark, "nearest_done", "fwd").unwrap();
    let short = event_int(&t, mark, "nearest_done", "short").unwrap();
    assert_eq!(fwd & 0b1000, 0, "forward skips a co-located seat");
    assert_ne!(short & 0b1000, 0, "shortest includes it");
}

#[test]
fn g07_nearest_with_prop_adds_only_matching_seats() {
    // The value is the grantor (`by`); presence is the fact. A matching seat
    // off the raw set is appended; a non-matching one is not.
    let mut t = matrix();
    let uid = t.place_raw(3, "TEST:probe");
    t.m.world_mut().set_prop_at(uid, EXEMPTION, 0);
    let other = t.place_raw(1, "TEST:probe");
    t.m.world_mut().set_prop_at(other, EXEMPTION, 5);
    let mark = t.mark();
    t.play(0, NEAREST).unwrap();
    drain(&mut t);
    let raw = event_int(&t, mark, "nearest_done", "fwd").unwrap();
    let exempt = event_int(&t, mark, "nearest_done", "exempt").unwrap();
    assert_eq!(raw, mask_for(&[1, 2]), "the raw set is unchanged");
    assert_eq!(
        exempt,
        mask_for(&[1, 2, 3]),
        "only the matching seat is added"
    );
}

// --- the waiver on the bond instance -------------------------------------

/// P0 (莉莎) skill-teleports onto P1's tile and answers 必然的联系, placing the
/// copy on P1. Returns the placed uid.
fn bond_on_p1(t: &mut Table) -> i32 {
    t.set_pos(0, 0);
    t.set_pos(1, 10);
    t.set_pos(2, 20);
    t.set_fire(0, 2, 2);
    t.give(0, &[LISA_LINK]);
    t.set_draw(0, &["通用:安可"]);
    let skill = t.skill_id(0, "慈爱女神");
    t.skill(0, &skill).unwrap();
    while t.prompt().is_some() {
        t.decline(); // the settle-cancel ask
    }
    t.dice(&[3]);
    t.roll(0).unwrap();
    let mut placed = false;
    for _ in 0..40 {
        let Some(p) = t.prompt() else { break };
        if t.counteract_offered(LISA_LINK) && !placed {
            t.counteract(0, LISA_LINK).unwrap();
            continue;
        }
        if p.kind == "player" {
            pick(t, 0, 1);
            placed = true;
            continue;
        }
        t.decline();
    }
    drain(t);
    t.field(1)
        .into_iter()
        .find(|f| f.card == LISA_LINK)
        .map(|f| f.uid)
        .unwrap_or_else(|| panic!("bond not placed: {:?}", t.field_ids(1)))
}

/// Return to P0's play phase: a played hand card needs it, and the bond flow
/// ends at the post-roll step. The bond instance outlives the turn boundary.
fn back_to_p0(t: &mut Table) {
    t.end(0).unwrap();
    drain(t);
    t.begin_turn(0);
    drain(t);
}

/// Run the waiver probe in a card body (the helper needs a host) and answer
/// the record's `mark`.
fn probe_exemption(t: &mut Table) -> i32 {
    t.give(0, &[EXEMPTION_PROBE]);
    let mark = t.mark();
    t.play(0, EXEMPTION_PROBE).unwrap();
    drain(t);
    mark
}

fn exempt_mask(t: &Table, mark: i32, grantor: usize) -> i64 {
    event_int(t, mark, "exemption_probe_done", &format!("g{grantor}"))
        .unwrap_or_else(|| panic!("no exemption probe: {:?}", t.keys_since(mark)))
}

/// Probe once and answer the mask for `grantor`.
fn probe_exempt(t: &mut Table, grantor: usize) -> i64 {
    let mark = probe_exemption(t);
    exempt_mask(t, mark, grantor)
}

#[test]
fn g07_lisa_bond_grants_the_waiver() {
    let mut t = Table::new(&["今井莉莎", "户山香澄", "山吹沙绫"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    let uid = bond_on_p1(&mut t);
    assert_eq!(
        t.m.world().prop_at(uid, EXEMPTION),
        0,
        "the placer (seat 0) is the stored grantor"
    );
    back_to_p0(&mut t);
    let mark = probe_exemption(&mut t);
    assert_ne!(exempt_mask(&t, mark, 0) & (1 << 1), 0, "P0 is exempt on P1");
    assert_ne!(
        event_int(&t, mark, "exemption_probe_done", "candidates").unwrap() & (1 << 1),
        0,
        "the candidate list carries the co-located holder"
    );
}

#[test]
fn g07_lisa_bond_waiver_by_seat_zero() {
    // The value `0` is the seat-0 grantor, not an absent key: the bond is on
    // the field, so the read is a real grant.
    let mut t = Table::new(&["今井莉莎", "户山香澄", "山吹沙绫"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    bond_on_p1(&mut t);
    back_to_p0(&mut t);
    let mark = probe_exemption(&mut t);
    assert_ne!(exempt_mask(&t, mark, 0) & (1 << 1), 0);
    assert_eq!(exempt_mask(&t, mark, 1) & (1 << 1), 0, "not seat 1");
}

#[test]
fn g07_lisa_bond_waiver_is_per_grantor() {
    let mut t = Table::new(&["今井莉莎", "户山香澄", "山吹沙绫"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    bond_on_p1(&mut t);
    back_to_p0(&mut t);
    let mark = probe_exemption(&mut t);
    assert_ne!(exempt_mask(&t, mark, 0) & (1 << 1), 0);
    assert_eq!(
        exempt_mask(&t, mark, 2) & (1 << 1),
        0,
        "another seat is not exempt"
    );
}

#[test]
fn g07_lisa_bond_waiver_dies_with_the_instance() {
    let mut t = Table::new(&["今井莉莎", "户山香澄", "山吹沙绫"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    bond_on_p1(&mut t);
    back_to_p0(&mut t);
    assert_ne!(probe_exempt(&mut t, 0) & (1 << 1), 0);
    t.m.world_mut().st.players[1]
        .field
        .retain(|f| f.card != LISA_LINK);
    assert_eq!(probe_exempt(&mut t, 0), 0, "the waiver went with the card");
}

#[test]
fn g07_lisa_bond_refused_gate_grants_nothing() {
    // 夏日合宿 on the candidate refuses another source; the copy is not
    // placed, so no grant exists.
    let mut t = Table::new(&["今井莉莎", "户山香澄", "山吹沙绫"]);
    t.clean();
    t.place_raw(1, CAMP);
    t.begin_turn(0);
    drain(&mut t);
    // The teleport lands on P1, then the bond's ask names P1.
    t.set_pos(0, 0);
    t.set_pos(1, 10);
    t.set_pos(2, 20);
    t.set_fire(0, 2, 2);
    t.give(0, &[LISA_LINK]);
    t.set_draw(0, &["通用:安可"]);
    let skill = t.skill_id(0, "慈爱女神");
    t.skill(0, &skill).unwrap();
    while t.prompt().is_some() {
        t.decline();
    }
    t.dice(&[3]);
    t.roll(0).unwrap();
    let mut declared = false;
    for _ in 0..40 {
        let Some(p) = t.prompt() else { break };
        if t.counteract_offered(LISA_LINK) && !declared {
            t.counteract(0, LISA_LINK).unwrap();
            declared = true;
            continue;
        }
        if p.kind == "player" {
            pick(&mut t, 0, 1);
            continue;
        }
        t.decline();
    }
    drain(&mut t);
    assert!(declared, "the bond was offered");
    assert!(
        !t.on_field(1, LISA_LINK),
        "a refused name places nothing: {:?}",
        t.field_ids(1)
    );
    back_to_p0(&mut t);
    assert_eq!(probe_exempt(&mut t, 0), 0, "nothing was granted");
}

#[test]
fn g07_lisa_goddess_offers_the_designated_seat() {
    let mut t = Table::new(&["今井莉莎", "户山香澄", "山吹沙绫"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    bond_on_p1(&mut t);
    // Now P0 and P1 are co-located at 10; the waiver puts P1 back in the
    // candidate set beside the nearest-ahead P2, so the pick appears.
    back_to_p0(&mut t);
    t.set_fire(0, 2, 2);
    let skill = t.skill_id(0, "慈爱女神");
    t.skill(0, &skill).unwrap();
    let p = t.expect_prompt();
    assert!(
        p.title.key().ends_with("lisa_pick_title"),
        "the exempt seat makes the pick non-singleton: {}",
        t.dump_prompt()
    );
    assert!(
        p.options
            .iter()
            .any(|o| format!("{o:?}").contains("PlayerId(1)")),
        "the designated seat is offered: {}",
        t.dump_prompt()
    );
    while t.prompt().is_some() {
        t.decline();
    }
}

#[test]
fn g07_lisa_goddess_no_prompt_without_waiver() {
    let mut t = Table::new(&["今井莉莎", "户山香澄", "山吹沙绫"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.set_pos(0, 0);
    t.set_pos(1, 10);
    t.set_pos(2, 20);
    t.set_fire(0, 2, 2);
    let skill = t.skill_id(0, "慈爱女神");
    t.skill(0, &skill).unwrap();
    if let Some(p) = t.prompt() {
        assert!(
            !p.title.key().ends_with("lisa_pick_title"),
            "a single nearest seat needs no pick: {}",
            t.dump_prompt()
        );
    }
    while t.prompt().is_some() {
        t.decline();
    }
    t.dice(&[3]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(
        t.pos(0),
        10,
        "the plain path still teleports to the nearest"
    );
}

// =====================================================================
// Message helpers stay honest
// =====================================================================

#[test]
fn g07_self_target_fixture_answers_the_user() {
    // `target_self` is the readability sugar over `target(user)`; it must name
    // the user, not someone else.
    let mut t = at_rest(&["花园多惠", "户山香澄"], 0);
    t.give(0, &[TARGET_SELF]);
    let mark = t.mark();
    t.play(0, TARGET_SELF).unwrap();
    drain(&mut t);
    let got = t
        .events_since(mark)
        .iter()
        .find(|e| log_key(e).ends_with("target_self_done"))
        .and_then(|e| match e.msg.args.get("got") {
            Some(Arg::Integer(v)) => Some(*v),
            _ => None,
        });
    assert_eq!(got, Some(0), "the user's own seat");
}

#[test]
fn g07_tile_targeting_names_the_owner() {
    // A tile designation is the owner's name: `target_tile` answers the owner
    // seat, and the probe sees `tile >= 0` with that owner.
    let (mut t, mine) = tile_table(3);
    let mark = t.mark();
    hit_tile(&mut t, 1, mine);
    drain(&mut t);
    assert_eq!(
        event_int(&t, mark, "target_tile_done", "got"),
        Some(1),
        "the designation succeeded"
    );
    assert_eq!(
        event_int(&t, mark, "targeted_probe_done", "who"),
        Some(0),
        "the tile's owner is named"
    );
    assert_eq!(event_int(&t, mark, "targeted_probe_done", "by"), Some(1));
}
