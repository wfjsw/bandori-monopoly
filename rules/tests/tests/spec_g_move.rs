//! G-MOVE scenario tests -- [路径] / [经过] / [传送] / [主要移动] / [重叠].
//!
//! Unit 4. Clauses live in `rules/spec/src/global/movement.rs` and are also
//! checked on every soak act via `tests/global_soak.rs`. This file pins the
//! book sentences the per-act window cannot see on its own: the exact path
//! of a loaded-dice walk, the CiRCLE reward iff, the one-main-move rule, the
//! settle tail, [重叠] corner cases, and 行动阶段 rows 7–14 order.
//!
//! Spec only from the 规则书. A discrepancy is a plain failing test.

use rules_spec::global::movement::{move_path, segment_path, walk_path, BOARD_TILES};
use rules_spec::{forall, tile, Table};

// -------------------------------------------------------------------------
// helpers
// -------------------------------------------------------------------------

/// Drain every open prompt (declining optional windows).
fn drain(t: &mut Table) {
    for _ in 0..40 {
        if t.prompt().is_none() {
            return;
        }
        t.decline();
    }
    panic!("prompts never stopped: {}", t.dump_prompt());
}

/// Drain, reporting whether a G-SETTLE-06/07 deed buy/build prompt was
/// offered. That prompt *is* the settle of an unowned/own [可购买格子]; a
/// declined offer logs no `land`/`rent` key, so the prompt is the marker.
fn drain_settle(t: &mut Table) -> bool {
    let mut saw = false;
    for _ in 0..40 {
        let Some(p) = t.prompt() else {
            return saw;
        };
        let k = p.title.key();
        if k.contains("buy") || k.contains("build") {
            saw = true;
        }
        t.decline();
    }
    panic!("prompts never stopped: {}", t.dump_prompt());
}

/// Did the window settle? A `land`/`rent` key, or a G-SETTLE buy/build prompt
/// (declined or accepted -- the offer is the settle).
fn settled(keys: &[String], saw_settle_prompt: bool) -> bool {
    saw_settle_prompt
        || keys
            .iter()
            .any(|k| k.contains("land") || k.contains("rent"))
}

/// Drain, answering the CiRCLE reward with `pick` (0 = money, 1 = card).
fn drain_circle(t: &mut Table, pick: i32) {
    for _ in 0..40 {
        if t.prompt().is_none() {
            return;
        }
        if t.prompt().unwrap().title.key().contains("pass_reward") {
            t.answer_one(pick).unwrap();
        } else {
            t.decline();
        }
    }
    panic!("prompts never stopped: {}", t.dump_prompt());
}

/// Vanilla table with seat 0 on their turn at `pos`, the other seat parked.
fn at(pos: usize) -> Table {
    let mut t = Table::vanilla(2);
    t.set_pos(0, pos);
    t.set_pos(1, (pos + 30) % BOARD_TILES);
    t
}

/// The walk tiles of the window's `roll` / `move` events for `who`.
fn path_of(t: &Table, mark: i32, who: usize) -> Vec<i32> {
    let evs = t.events_since(mark);
    walk_path(&evs, who as i32, BOARD_TILES).expect("path arithmetic")
}

/// Synthetic `teleport` event for a pure path-shape check.
fn teleport_event(who: i32, from: i32, to: i32) -> game_core::state::MatchEvent {
    game_core::state::MatchEvent {
        id: 1,
        r#type: "teleport".into(),
        player_id: who,
        from,
        to,
        ..Default::default()
    }
}

// -------------------------------------------------------------------------
// G-MOVE-01 / 02 -- [路径]
// -------------------------------------------------------------------------

/// G-MOVE-01 -- every `pos` stays a legal board index (the board is 60).
#[test]
fn g_move_01_pos_stays_on_the_board() {
    forall(12, 0x6D6F_7665, |rng, _i| {
        let n = BOARD_TILES;
        let start = rng.below(n as u64) as usize;
        let steps = rng.range(1, 20);
        let mut t = at(start);
        t.dice(&[steps]);
        t.roll(0).map_err(|e| format!("roll rejected: {e}"))?;
        drain_circle(&mut t, 0);
        drain(&mut t);
        let pos = t.pos(0);
        if pos >= n {
            return Err(format!("pos {pos} outside 0..{n}"));
        }
        Ok(())
    })
    .expect("G-MOVE-01");
}

/// G-MOVE-02 -- [路径] excludes the start and includes the end; clockwise;
/// the board has 60 tiles; start == end includes the tile.
///
/// Quote (移动 5): 「[路径]不包括[移动起点]但包括[移动终点]，如果两点相同
/// 则包括（例：玩家当前在序号5的格子，[移动终点]在序号10的格子，此次移动为
/// 顺时针移动则[路径]为序号6到10的格子）」.
#[test]
fn g_move_02_path_excludes_start_includes_end() {
    // The quote's own example: 序号5 -> 序号10 is 0-based 4 -> 9, path 5..=9.
    assert_eq!(
        segment_path(4, 9, 5, BOARD_TILES).unwrap(),
        vec![5, 6, 7, 8, 9],
        "the quote's 序号5->序号10 example"
    );

    // 「如果两点相同则包括」 -- a zero-step move's path is the standing tile.
    assert_eq!(
        segment_path(7, 7, 0, BOARD_TILES).unwrap(),
        vec![7],
        "start == end includes the tile"
    );

    forall(16, 0x5041_5448, |rng, _i| {
        let n = BOARD_TILES;
        let start = rng.below(n as u64) as usize;
        let steps = rng.range(1, 20);
        let mut t = at(start);
        t.dice(&[steps]);
        let mark = t.mark();
        t.roll(0).map_err(|e| format!("roll rejected: {e}"))?;
        drain_circle(&mut t, 0);
        drain(&mut t);
        let path = path_of(&t, mark, 0);
        let expect: Vec<i32> = (1..=steps).map(|k| (start as i32 + k) % n as i32).collect();
        if path != expect {
            return Err(format!(
                "path {path:?} != clockwise {steps} from {start} {expect:?}"
            ));
        }
        if path.contains(&(start as i32)) {
            return Err(format!("path {path:?} includes the start {start}"));
        }
        let end = (start as i32 + steps) % n as i32;
        if path.last() != Some(&end) {
            return Err(format!("path {path:?} misses the end {end}"));
        }
        if t.pos(0) as i32 != end {
            return Err(format!("pos {} != end {end}", t.pos(0)));
        }
        Ok(())
    })
    .expect("G-MOVE-02");
}

/// G-MOVE-02 wrap: the path continues across the board's end (clockwise on 60).
#[test]
fn g_move_02_path_wraps_the_board() {
    // From tile 58 (0-based), 4 steps: 59, 0, 1, 2.
    assert_eq!(
        segment_path(58, 2, 4, BOARD_TILES).unwrap(),
        vec![59, 0, 1, 2],
        "clockwise wrap past the board's end"
    );
}

// -------------------------------------------------------------------------
// G-MOVE-03 -- [经过] per path tile; start does not, end does
// -------------------------------------------------------------------------

/// G-MOVE-03 -- the start tile is not a [经过] site; the end tile is.
///
/// Quote (行动阶段 12): 「移动起点不触发[经过],移动终点触发[经过]」 and
/// 移动 6: 「[经过]X时Y”的效果为当X位于移动[路径]时执行Y」.
#[test]
fn g_move_03_end_settles_start_does_not() {
    forall(12, 0x5041_5353, |rng, _i| {
        let n = BOARD_TILES;
        // Pick a walk that never [经过]s CiRCLE so the reward prompt does
        // not split the window.
        let (start, steps) = loop {
            let s = rng.below(n as u64) as usize;
            let k = rng.range(1, 20);
            let hits_circle = (1..=k).any(|j| (s + j as usize) % n == tile("CiRCLE"));
            if s != tile("CiRCLE") && !hits_circle {
                break (s, k);
            }
        };
        let mut t = at(start);
        t.set_pos(1, (start + 40) % BOARD_TILES);
        t.dice(&[steps]);
        let mark = t.mark();
        t.roll(0).map_err(|e| format!("roll rejected: {e}"))?;
        let saw_settle_prompt = drain_settle(&mut t);
        let end = (start + steps as usize) % BOARD_TILES;
        if t.pos(0) != end {
            return Err(format!("ended on {} want {end}", t.pos(0)));
        }
        let keys = t.keys_since(mark);
        // The end is the [移动终点] and settles (行动阶段 15). Agents log
        // `agent_skip` even when they have nothing to do and are not used as
        // the settle marker here -- skip agent/circle ends for this check.
        let end_kind = rules_spec::data().tiles[end].kind.clone();
        if matches!(end_kind.as_str(), "agent" | "circle") {
            return Ok(());
        }
        if !settled(&keys, saw_settle_prompt) {
            return Err(format!(
                "end tile {end} did not settle (行动阶段 15): {keys:?} settle_prompt={saw_settle_prompt}"
            ));
        }
        // 「移动起点不触发[经过]」: only the end settles. A second land/rent
        // line means the start (or a mid-path tile) also settled. A G-SETTLE
        // buy/build prompt counts as one settle site.
        let lands = keys
            .iter()
            .filter(|k| k.contains("land") || k.contains("rent"))
            .count()
            + usize::from(saw_settle_prompt);
        if lands > 1 {
            return Err(format!(
                "start {start} looks like a settle site too ({lands} settle lines): {keys:?}"
            ));
        }
        Ok(())
    })
    .expect("G-MOVE-03");
}

// -------------------------------------------------------------------------
// G-MOVE-04 -- path recompute when a [经过] changes the destination
// -------------------------------------------------------------------------

/// G-MOVE-04 -- 「如果某个格子的[经过]效果会改变[移动终点]则重新计算[路径]，
/// 可能导致后续格子不算作[经过]」 (`rulebook-doc.md:140`).
///
/// The quote is about a [经过] effect that rewrites the [移动终点] **during
/// the move** (「移动时」), so later path tiles drop out of the walk.
///
/// The sheet's destination-rewriters are timed later:
/// `RAS:（LOCK）追逐梦想的步伐`（2）「如果此卡拥有者的主要移动[经过]了
/// "Bandori车站"则**在触发结算前**将行动终点改为"旭汤澡堂"」 -- the trigger
/// is the Bandori pass, but the rewrite is 行动阶段 14 (结算前), after the
/// walk is over. What that card text *does* determine is pinned here: the
/// piece ends on 旭汤澡堂, not on the raw 34+20. The mid-walk recompute
/// itself has no sheet-card instance that fires it (see the area doc).
#[test]
fn g_move_04_pass_rewrites_the_end_before_settle() {
    let bandori = tile("Bandori车站");
    let asayu = tile("旭汤澡堂");
    assert_eq!(bandori, 35, "board shape the scenario assumes");
    assert_eq!(asayu, 52);

    let mut t = at(bandori - 1); // 34
    t.set_pos(1, 10);
    t.place_raw(0, "RAS:（LOCK）追逐梦想的步伐");
    t.dice(&[20]);
    let mark = t.mark();
    t.roll(0).unwrap_or_else(|e| panic!("roll rejected: {e}"));
    drain(&mut t);
    let keys = t.keys_since(mark);
    // 「在触发结算前将行动终点改为"旭汤澡堂"」 -- the settle runs at 52.
    if t.pos(0) != asayu {
        panic!(
            "G-MOVE-04: pos {} want 旭汤澡堂 {asayu} (LOCK's 结算前 rewrite): {keys:?}",
            t.pos(0)
        );
    }
    // The rewrite is at 结算前, so the walk itself is the raw 34+20 path;
    // only the settle target moved. (A mid-walk 「[经过]…改变[移动终点]」
    // would recompute the path and drop the tiles past the new end -- see
    // the area doc. No current sheet card does that.)
    let path = path_of(&t, mark, 0);
    let raw_end = (bandori - 1 + 20) % BOARD_TILES;
    if path.last() != Some(&(raw_end as i32)) {
        panic!(
            "G-MOVE-04: walk path {:?} should still end at the raw {raw_end} (the rewrite is 结算前)",
            path
        );
    }
}

// -------------------------------------------------------------------------
// G-MOVE-05 -- [传送] path is only the end; [经过] fires at the end
// -------------------------------------------------------------------------

/// G-MOVE-05 -- 「[传送]：[路径]只包括[移动终点]的移动动作，直接将棋子放置于
/// [移动终点]。」 and 行动阶段 12 「[传送]后在终点触发[经过]」.
#[test]
fn g_move_05_teleport_path_is_only_the_end() {
    assert_eq!(
        move_path(&[teleport_event(0, 20, tile("CiRCLE") as i32)], 0, BOARD_TILES).unwrap(),
        vec![tile("CiRCLE") as i32],
        "[传送] path is the endpoint only"
    );

    // Exile-return: 「获得该状态的效果会写明状态结束后在玩家回合开始时要
    // [传送]到的格子」 -- write CiRCLE as the return tile. The endpoint is a
    // [经过] of CiRCLE with 移动起点 != CiRCLE, so the reward fires.
    let mut t = at(20);
    t.set_pos(1, 40);
    t.set_draw(0, &["通用:GREAT", "通用:PERFECT!"]);
    let money = t.money(0);
    t.set_state(0, "exile", 1);
    t.set_state(0, "exileTo", tile("CiRCLE") as i32);
    t.set_pos(0, 20);
    t.begin_turn(0);
    drain_circle(&mut t, 0);
    drain(&mut t);
    if t.pos(0) != tile("CiRCLE") {
        panic!(
            "G-MOVE-05: exile-return [传送] should land on CiRCLE, got {}",
            t.pos(0)
        );
    }
    if t.money(0) != money + 2000 {
        panic!(
            "G-MOVE-05: the endpoint is a [经过] of CiRCLE (start was 20): money {} want {money}+2000",
            t.money(0)
        );
    }
}

// -------------------------------------------------------------------------
// G-MOVE-06 -- [CiRCLE奖励]
// -------------------------------------------------------------------------

/// G-MOVE-06 -- 「[经过]CiRCLE且[移动起点]不为CiRCLE时获得[CiRCLE奖励]。」
/// 「[CiRCLE奖励]：[获得]2000资金或抽1张卡。」
#[test]
fn g_move_06_reward_iff_pass_and_start_not_circle() {
    let circle = tile("CiRCLE");
    // (a) pass from away: reward fires, money half is +2000.
    forall(8, 0xC10C_1E01, |rng, _i| {
        let steps = rng.range(1, 20);
        let start = (circle + BOARD_TILES - steps as usize) % BOARD_TILES;
        if start == circle {
            return Ok(());
        }
        let mut t = at(start);
        t.set_pos(1, 30);
        t.set_draw(0, &["通用:GREAT", "通用:PERFECT!", "通用:FEVER!"]);
        t.set_hand(0, &[]);
        let money = t.money(0);
        t.dice(&[steps]);
        t.roll(0).map_err(|e| format!("roll rejected: {e}"))?;
        drain_circle(&mut t, 0);
        drain(&mut t);
        if t.pos(0) != circle {
            return Err(format!("ended on {} want CiRCLE", t.pos(0)));
        }
        // +2000 reward, +1 settle draw on CiRCLE (「[结算]是：抽取一张手卡」).
        if t.money(0) != money + 2000 {
            return Err(format!(
                "[CiRCLE奖励] money half: {} want {money}+2000",
                t.money(0)
            ));
        }
        if t.hand(0).len() != 1 {
            return Err(format!(
                "landing [结算] draws 1 (on top of the reward): {:?}",
                t.hand(0)
            ));
        }
        Ok(())
    })
    .expect("G-MOVE-06 pass-from-away");

    // (b) card half: +1 card from the reward (plus the landing draw).
    let mut t = at((circle + BOARD_TILES - 3) % BOARD_TILES);
    t.set_pos(1, 30);
    t.set_draw(0, &["通用:GREAT", "通用:PERFECT!", "通用:FEVER!"]);
    t.set_hand(0, &[]);
    let money = t.money(0);
    t.dice(&[3]);
    t.roll(0).unwrap();
    drain_circle(&mut t, 1);
    drain(&mut t);
    assert_eq!(t.pos(0), circle);
    assert_eq!(t.money(0), money, "card half gains no money");
    assert_eq!(t.hand(0).len(), 2, "reward draw + landing draw: {:?}", t.hand(0));

    // (c) 移动起点 == CiRCLE: walking off pays nothing (「且[移动起点]不为CiRCLE」).
    let mut t = at(circle);
    t.set_pos(1, 30);
    let money = t.money(0);
    t.dice(&[3]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 3, "walked 1, 2, 3 off CiRCLE");
    assert_eq!(
        t.money(0),
        money,
        "no reward: the move started on CiRCLE and never re-passed it"
    );
}

// -------------------------------------------------------------------------
// G-MOVE-07 -- base [主要移动] is 1d20 clockwise
// -------------------------------------------------------------------------

/// G-MOVE-07 -- 「基础[主要移动]为**投掷1d20**，在地图上顺时针移动投掷结果
/// 数量的格子」.
#[test]
fn g_move_07_main_move_is_1d20_clockwise() {
    forall(16, 0x3164_3230, |rng, _i| {
        let face = rng.range(1, 20);
        let start = rng.below(BOARD_TILES as u64) as usize;
        let mut t = at(start);
        t.dice(&[face]);
        let mark = t.mark();
        t.roll(0).map_err(|e| format!("roll rejected: {e}"))?;
        drain_circle(&mut t, 0);
        drain(&mut t);
        let evs = t.events_since(mark);
        let rolls: Vec<_> = evs.iter().filter(|e| e.r#type == "diceRollBefore").collect();
        if rolls.is_empty() {
            return Err(format!("no roll event: {:?}", t.keys_since(mark)));
        }
        let head = rolls[0];
        if head.dice != face {
            return Err(format!("roll event dice {} != loaded face {face}", head.dice));
        }
        let end = (start as i32 + face) % BOARD_TILES as i32;
        if t.pos(0) as i32 != end {
            return Err(format!("pos {} != {start}+{face} = {end}", t.pos(0)));
        }
        Ok(())
    })
    .expect("G-MOVE-07");
}

// -------------------------------------------------------------------------
// G-MOVE-08 -- one [主要移动] per turn
// -------------------------------------------------------------------------

/// G-MOVE-08 -- 行动阶段 7 「一回合只能触发一次［主要移动］效果,多次触发
/// ［主要移动］时无效」.
#[test]
fn g_move_08_second_main_move_does_nothing() {
    let mut t = at(10);
    t.set_pos(1, 40);
    t.dice(&[3]);
    t.roll(0).unwrap();
    drain_circle(&mut t, 0);
    drain(&mut t);
    assert_eq!(t.pos(0), 13, "the one [主要移动] ran");
    let pos = t.pos(0);
    let money = t.money(0);
    // A second `roll` is refused and must not move anyone.
    t.dice(&[5]);
    let mark = t.mark();
    let err = t.roll(0).unwrap_err();
    assert!(
        !err.is_empty(),
        "second roll refused: {err}"
    );
    assert_eq!(t.pos(0), pos, "the refused roll did not move");
    assert_eq!(t.money(0), money, "the refused roll moved no money");
    let keys = t.keys_since(mark);
    assert!(
        !keys
            .iter()
            .any(|k| k == "log.roll" || k == "log.move_forward" || k == "teleport"),
        "the refused roll logged a walk: {keys:?}"
    );
}

// -------------------------------------------------------------------------
// G-MOVE-09 -- settle follows unless the effect says not to
// -------------------------------------------------------------------------

/// G-MOVE-09 -- 「到达[移动终点]后需进行[结算]」 and 「技能或卡所导致的
/// [主要移动]会表明到达终点后是否[结算]」 (`rulebook-doc.md:132,141`).
#[test]
fn g_move_09_settle_follows_the_main_move() {
    // Landing on an unowned [可购买格子] offers the buy (the settle body,
    // G-SETTLE-06).
    let mut t = at(0);
    t.set_pos(1, 40);
    t.dice(&[3]); // -> 水族馆 (index 3)
    let mark = t.mark();
    t.roll(0).unwrap();
    let mut saw_settle_prompt = false;
    for _ in 0..40 {
        let Some(p) = t.prompt() else { break };
        let k = p.title.key();
        if k.contains("pass_reward") {
            t.answer_one(0).unwrap();
        } else {
            if k.contains("buy") || k.contains("build") {
                saw_settle_prompt = true;
            }
            t.decline();
        }
    }
    assert_eq!(t.pos(0), 3);
    let keys = t.keys_since(mark);
    assert!(
        settled(&keys, saw_settle_prompt),
        "the end tile settled (「到达[移动终点]后需进行[结算]」): {keys:?} settle_prompt={saw_settle_prompt}"
    );

    // A 「不[结算]」 move skips the settle. `通用:@Tsugu ycm`'s high half is
    // 「结果至少为26则进进入移动阶段并将本回合的[主要移动]改为[传送]到
    // "bandori车站"并不[结算]」. The teleport is the main move; the landing
    // must not rent/settle (only the card's own buy-anywhere offer).
    let mut t = at(10);
    t.set_pos(1, 40);
    t.dice(&[9, 9, 8]); // 3d10 sum 26
    let mark = t.mark();
    t.give_play(0, "通用:@Tsugu ycm").unwrap();
    assert_eq!(
        t.pos(0),
        tile("Bandori车站"),
        "the [主要移动] became a [传送]: {:?}",
        t.keys_since(mark)
    );
    let keys = t.keys_since(mark);
    // 不[结算]: the unowned Bandori车站 does not announce a buy/land settle.
    // (The card's own 「可选择购买任意无主的[可购买格子]」 gate is not a settle.)
    assert!(
        !keys.iter().any(|k| k.contains("land_unowned") || k.contains("rent")),
        "「不[结算]」: no settle on the landing: {keys:?}"
    );
    // The card's own 「可选择购买任意无主的[可购买格子]」 gate is not the settle.
    drain(&mut t);
    assert_eq!(t.pos(0), tile("Bandori车站"));
    assert_eq!(
        t.owner(tile("Bandori车站")),
        None,
        "no settle, no forced buy"
    );
}

// -------------------------------------------------------------------------
// G-MOVE-10 -- [重叠] at the end tile
// -------------------------------------------------------------------------

/// G-MOVE-10 -- 行动阶段 13 「移动终点触发[重叠] 原地[传送]/移动(移动0格)时
/// 触发[重叠]」.
#[test]
fn g_move_10_overlap_at_the_end_tile() {
    // (a) walk onto a shared tile.
    let mut t = at(10);
    t.set_pos(1, 13);
    t.dice(&[3]);
    let mark = t.mark();
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 13);
    let keys = t.keys_since(mark);
    assert!(
        keys.iter().any(|k| k.contains("overlap")),
        "walk onto P1's tile raises [重叠]: {keys:?}"
    );

    // (b) 0-step move on a shared tile (行动阶段 13 「移动(移动0格)时触发[重叠]」).
    let mut t = at(10);
    t.set_pos(1, 10);
    t.m.world_mut().turn.fixed_roll = Some(0);
    let mark = t.mark();
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 10, "0-step move stayed put");
    let keys = t.keys_since(mark);
    assert!(
        keys.iter().any(|k| k.contains("overlap")),
        "0-step move on a shared tile raises [重叠]: {keys:?}"
    );

    // (c) 原地[传送] onto one's own tile.
    let mut t = at(20);
    t.set_pos(1, 20);
    t.set_state(0, "exile", 1);
    t.set_state(0, "exileTo", 20);
    let mark = t.mark();
    t.begin_turn(0);
    drain(&mut t);
    assert_eq!(t.pos(0), 20, "原地[传送]");
    let keys = t.keys_since(mark);
    assert!(
        keys.iter().any(|k| k.contains("overlap")),
        "原地[传送] onto one's own tile raises [重叠]: {keys:?}"
    );
}

// -------------------------------------------------------------------------
// G-MOVE-11 -- 行动阶段 rows 7–14 ordering
// -------------------------------------------------------------------------

/// G-MOVE-11 -- walk ([经过]) then [重叠] then [结算].
#[test]
fn g_move_11_walk_then_overlap_then_settle() {
    let mut t = at(10);
    t.set_pos(1, 13);
    t.own(1, &[13]);
    t.dice(&[3]);
    let mark = t.mark();
    t.roll(0).unwrap();
    drain(&mut t);
    let keys = t.keys_since(mark);
    let walk = keys
        .iter()
        .position(|k| k.contains("roll") || k.contains("move"));
    let overlap = keys.iter().position(|k| k.contains("overlap"));
    let settle = keys
        .iter()
        .position(|k| k.contains("land") || k.contains("rent") || k.contains("pay"));
    let (Some(w), Some(o), Some(s)) = (walk, overlap, settle) else {
        panic!("missing a stage in the move tail: {keys:?}");
    };
    assert!(w < o, "行动阶段 12 [经过]/walk before 13 [重叠]: {keys:?}");
    assert!(o < s, "行动阶段 13 [重叠] before 15 [结算]: {keys:?}");
}

/// G-MOVE-11 -- the [经过] of a mid-path CiRCLE (the reward step) sits after
/// the walk that reached it and before the landing's settle.
#[test]
fn g_move_11_circle_reward_sits_between_walk_and_settle() {
    // 58 -> 1 passes CiRCLE (0) then lands on 1.
    let mut t = at(58);
    t.set_pos(1, 30);
    t.dice(&[3]);
    let mark = t.mark();
    t.roll(0).unwrap();
    // Reward prompt is open before the landing settles.
    assert_eq!(
        t.prompt().map(|p| p.title.key().to_string()),
        Some("ask.pass_reward.title".into()),
        "the walk halted at the [经过] CiRCLE reward: {}",
        t.dump_prompt()
    );
    let keys_mid = t.keys_since(mark);
    assert!(
        keys_mid.iter().any(|k| k.contains("roll") || k.contains("move")),
        "the approach to CiRCLE is logged before the prompt: {keys_mid:?}"
    );
    assert!(
        !keys_mid.iter().any(|k| k.contains("land")),
        "the landing has not settled yet: {keys_mid:?}"
    );
    t.answer_one(0).unwrap();
    // G-SETTLE-06: the settle of the unowned landing deed is the optional
    // buy prompt -- offered after the reward, before the turn ends.
    let saw_settle_prompt = drain_settle(&mut t);
    let keys = t.keys_since(mark);
    let reward = keys.iter().position(|k| k.contains("pass_reward"));
    // A declined buy/build logs no key; the prompt itself sits after every
    // key so far (including the reward).
    let settle = keys
        .iter()
        .position(|k| k.contains("land") || k.contains("rent"))
        .or(if saw_settle_prompt { Some(keys.len()) } else { None });
    let (Some(r), Some(s)) = (reward, settle) else {
        panic!("missing reward or settle: {keys:?} settle_prompt={saw_settle_prompt}");
    };
    assert!(r < s, "reward before the landing settle: {keys:?}");
    assert_eq!(t.pos(0), 1);
    assert_eq!(t.money(0), 12_000);
}

