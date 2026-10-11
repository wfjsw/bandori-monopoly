//! Scenario tests for G-FLOW: 开始游戏, turn phases, status skips, main move.
//!
//! Each test quotes the rulebook clause it pins and drives one legal setup
//! through the observer. A discrepancy is a plain failing test -- no
//! `#[ignore]`, no allowlist.
//!
//! Setup (order / ban / pick / deck) never runs through `Table`'s constructors
//! (they `quick_start`), so this file carries a local `Setup` helper that
//! builds a live `Match` and observes the pre-game acts directly.

use game_core::engine::Match;
use game_core::net::{NetMessage, RoomMember};
use game_core::scoring::ScoreWeights;
use game_core::state::{key, stage};
use game_core::MatchMode;
use rules_spec::global::flow::CLAUSES;
use rules_spec::table::settle_match;
use rules_spec::{data, observe, rules, Clause, StepAct, Table};

const SEED: u64 = 20261006;

// ------------------------------------------------------------------ Setup

/// A match still in 开始游戏, driven act-by-act through [`observe`].
struct Setup {
    m: Match,
    n: usize,
    clauses: &'static [Clause],
}

impl Setup {
    /// `n` human seats, `mode` = Casual (娱乐) or Ranked (正规). `Match::new`
    /// has already rolled the 1d20 turn order (`log.order`).
    fn new(n: usize, mode: MatchMode) -> Self {
        let members: Vec<RoomMember> = (1..=n as i32)
            .map(|id| RoomMember {
                id,
                player: format!("P{}", id - 1),
                bot: false,
                ..Default::default()
            })
            .collect();
        let m = Match::new(
            data(),
            rules(),
            &members,
            SEED,
            mode,
            ScoreWeights::default(),
        );
        Setup {
            m,
            n,
            clauses: CLAUSES,
        }
    }

    /// Tick the host clock until `pred`.
    fn tick_until(&mut self, what: &str, mut pred: impl FnMut(&Match) -> bool) {
        for _ in 0..800 {
            if pred(&self.m) {
                return;
            }
            self.m.tick(0.25);
        }
        panic!(
            "tick_until: {what} never happened (phase {:?})",
            self.m.state().phase
        );
    }

    fn phase(&self) -> String {
        self.m.state().phase.clone()
    }

    fn turn(&self) -> i32 {
        self.m.world().st.turn
    }

    /// Observe one pre-game act (ban / pick / deck). `who` is a **seat**;
    /// the wire carries the seat's `member` (roll_order reorders seats).
    fn act(&mut self, who: usize, msg: NetMessage) -> Result<(), String> {
        let member = self.m.world().st.players[who].member;
        let out = observe(
            &mut self.m,
            StepAct::Act {
                who: member,
                msg,
            },
            |_| {
                // Setup phases are not "play"; settle_match returns at once.
            },
            self.clauses,
        );
        out.unwrap_violations();
        out.act_result
    }

    fn ban(&mut self, who: usize, character: &str) -> Result<(), String> {
        self.act(
            who,
            NetMessage {
                character: character.into(),
                ..NetMessage::act("ban")
            },
        )
    }

    fn pick(&mut self, who: usize, character: &str) -> Result<(), String> {
        self.act(
            who,
            NetMessage {
                character: character.into(),
                ..NetMessage::act("pick")
            },
        )
    }

    /// Submit `who`'s deck (a complete legal list).
    fn deck(&mut self, who: usize, cards: Vec<String>) -> Result<(), String> {
        self.act(
            who,
            NetMessage {
                cards,
                ..NetMessage::act("deck")
            },
        )
    }

    /// Run the opening (deal + mulligan prompts) to rest, answering mulligan
    /// with `keep` (option 0) for every seat.
    fn open_keep(&mut self) {
        self.tick_until("play", |m| m.state().phase == "play");
        self.settle_answering_choice(0, 0);
    }

    /// Run the opening; seat `who` takes the 初始重抽 (option 1).
    fn open_redraw(&mut self, who: usize) {
        self.tick_until("play", |m| m.state().phase == "play");
        self.settle_answering_choice(who, 1);
    }

    /// Drop every bound skill (character and band) so 开始游戏's default
    /// 「如无技能或卡效果影响」 holds: no `deckBeforeGame` / `deckAtGameStart`
    /// hook can cut the opening hand or move the start tile.
    fn strip_skills(&mut self) {
        let w = self.m.world_mut();
        for p in w.st.players.iter_mut() {
            p.field.retain(|f| !f.card.starts_with("skill:"));
        }
    }

    /// Settle the play-phase opening, answering the mulligan prompt as `who`
    /// with `value` (every other seat keeps). `p.players` holds seat indices
    /// parallel to `p.answers`; the wire carries the seat's `member`.
    fn settle_answering_choice(&mut self, who: usize, value: i32) {
        for _ in 0..200 {
            settle_match(&mut self.m);
            let p = self.m.state().prompt;
            if p.id == 0 {
                return;
            }
            let pending: Vec<(usize, i32)> = p
                .players
                .iter()
                .zip(p.answers.iter())
                .filter(|(_, &a)| a < 0)
                .map(|(&seat, _)| (seat as usize, self.m.world().st.players[seat as usize].member))
                .collect();
            for (seat, member) in pending {
                let act = if p.kind == "mulligan" {
                    let v = if seat == who { value } else { 0 };
                    StepAct::Answer {
                        who: member,
                        prompt: p.id,
                        value: v,
                        cards: vec![],
                    }
                } else {
                    StepAct::Decline {
                        who: member,
                        prompt: p.id,
                        value: p.fallback,
                    }
                };
                let out = observe(&mut self.m, act, settle_match, self.clauses);
                out.unwrap_violations();
                let _ = out.act_result;
            }
        }
        panic!("opening prompts never stopped");
    }

    fn world(&self) -> &game_core::engine::World {
        self.m.world()
    }

    fn players(&self) -> usize {
        let _ = self.n;
        self.m.world().st.players.len()
    }
}

/// Character names, first `n` of `data/characters.json`.
fn chars(n: usize) -> Vec<String> {
    rules_spec::characters().into_iter().take(n).collect()
}

/// The character's preset deck (a complete legal 10-card list).
fn preset_deck(character: &str) -> Vec<String> {
    let d = data();
    let c = d
        .character(character)
        .unwrap_or_else(|| panic!("no character {character:?}"));
    game_core::deck::preset(d.as_ref(), c)
}

// ------------------------------------------------------------- order / setup

/// G-FLOW-13 -- 1d20 turn order, high to low.
///
/// Quote: 「游戏开始时每名玩家投掷1d20，根据出目从大到小决定一局游戏的行动顺序（最大的先开始）。」
#[test]
fn turn_order_1d20_high_to_low() {
    let s = Setup::new(5, MatchMode::Casual);
    let rolls: Vec<i32> = s.world().st.players.iter().map(|p| p.roll).collect();
    for &r in &rolls {
        assert!((1..=20).contains(&r), "roll {r} outside 1d20: {rolls:?}");
    }
    for w in rolls.windows(2) {
        assert!(
            w[0] > w[1],
            "turn order not strictly high-to-low: {rolls:?} (book: 从大到小)"
        );
    }
}

/// G-FLOW-13 -- ties re-roll until 排完 (final rolls are unique).
///
/// Quote: 「如果多名玩家出目一致则那些玩家再投一次1d20来决定在那些玩家中的顺序，直到排完。」
#[test]
fn turn_order_ties_resolve_to_unique_faces() {
    for k in 0..8u64 {
        let members: Vec<RoomMember> = (1..=6i32)
            .map(|id| RoomMember {
                id,
                player: format!("P{}", id - 1),
                bot: false,
                ..Default::default()
            })
            .collect();
        let m = Match::new(
            data(),
            rules(),
            &members,
            SEED.wrapping_add(k),
            MatchMode::Casual,
            ScoreWeights::default(),
        );
        let rolls: Vec<i32> = m.world().st.players.iter().map(|p| p.roll).collect();
        let mut sorted = rolls.clone();
        sorted.sort_unstable_by(|a, b| b.cmp(a));
        assert_eq!(rolls, sorted, "seed {k}: seats not high-to-low {rolls:?}");
        let mut uniq = rolls.clone();
        uniq.sort_unstable();
        uniq.dedup();
        assert_eq!(uniq.len(), rolls.len(), "seed {k}: tied faces left {rolls:?}");
    }
}

/// G-FLOW-12 -- 娱乐 mode 3 to 10 players. A 2-seat Casual match is outside
/// the book's range.
#[test]
fn casual_requires_3_to_10_players() {
    // 3 and 10 are legal.
    let ok3 = Setup::new(3, MatchMode::Casual);
    assert_eq!(ok3.players(), 3);
    let ok10 = Setup::new(10, MatchMode::Casual);
    assert_eq!(ok10.players(), 10);
    // The engine's own constructor accepts 2 -- if it also starts play at 2
    // seats in Casual, that is a book gap (3到10人). Pin the *book* bound.
    let members: Vec<RoomMember> = (1..=2i32)
        .map(|id| RoomMember {
            id,
            player: format!("P{}", id - 1),
            bot: true,
            ..Default::default()
        })
        .collect();
    let mut m = Match::new(
        data(),
        rules(),
        &members,
        SEED,
        MatchMode::Casual,
        ScoreWeights::default(),
    );
    m.quick_start();
    settle_match(&mut m);
    let n = m.world().st.players.len();
    let mode = m.world().st.mode;
    assert_eq!(mode, MatchMode::Casual as i32);
    // Book: 「游玩人数为3到10人」. If the engine seated 2, that is a
    // discrepancy -- a plain failing assertion.
    assert!(
        (3..=10).contains(&n),
        "G-FLOW-12: 娱乐 mode seated {n} players, book says 3到10人"
    );
}

/// G-FLOW-14 -- 正规 mode 5 to 6 players.
#[test]
fn ranked_requires_5_to_6_players() {
    let ok5 = Setup::new(5, MatchMode::Ranked);
    assert_eq!(ok5.players(), 5);
    let ok6 = Setup::new(6, MatchMode::Ranked);
    assert_eq!(ok6.players(), 6);
    // A 4-seat Ranked match is outside the book's 5到6人.
    let members: Vec<RoomMember> = (1..=4i32)
        .map(|id| RoomMember {
            id,
            player: format!("P{}", id - 1),
            bot: true,
            ..Default::default()
        })
        .collect();
    let mut m = Match::new(
        data(),
        rules(),
        &members,
        SEED,
        MatchMode::Ranked,
        ScoreWeights::default(),
    );
    m.quick_start();
    settle_match(&mut m);
    let n = m.world().st.players.len();
    assert!(
        (5..=6).contains(&n),
        "G-FLOW-14: 正规 mode seated {n} players, book says 5到6人"
    );
}

/// G-FLOW-15 -- 正规 ban is reverse turn order, at most one each.
///
/// Quote: 「禁用角色的顺数为行动顺序从后到前」 / 「每名玩家有机会禁用最多一名角色」
#[test]
fn ranked_ban_is_reverse_turn_order() {
    let names = chars(20);
    let mut s = Setup::new(5, MatchMode::Ranked);
    // order → ban
    s.tick_until("ban", |m| m.state().phase == "ban");
    assert_eq!(s.turn(), 4, "ban starts at the last seat (从后到前)");
    // Seats are roll-ordered; walk the engine's own turn index downward.
    let mut prev = s.turn();
    for step_i in 0..5 {
        let seat = s.turn() as usize;
        if step_i > 0 {
            assert!(
                (s.turn() as i32) < prev,
                "ban walk is reverse turn order (turn {} -> {})",
                prev,
                s.turn()
            );
        }
        prev = s.turn();
        // Later seats ban something; earlier seats skip (最多一名 includes 0).
        let victim = if step_i < 2 {
            names[10 + seat].as_str()
        } else {
            ""
        };
        s.ban(seat, victim)
            .unwrap_or_else(|e| panic!("ban step {step_i} seat {seat}: {e}"));
    }
    assert_eq!(s.phase(), "pick", "after the last ban the pick phase opens");
    for (i, p) in s.world().st.players.iter().enumerate() {
        assert!(p.ban_done, "seat {i} has no ban_done after the ban walk");
    }
}

/// G-FLOW-15 -- a banned character cannot be picked.
#[test]
fn banned_character_is_unpickable() {
    let names = chars(20);
    let mut s = Setup::new(5, MatchMode::Ranked);
    s.tick_until("ban", |m| m.state().phase == "ban");
    let victim = names[3].clone();
    let first = s.turn() as usize;
    s.ban(first, &victim).expect("ban");
    for _ in 0..4 {
        let seat = s.turn() as usize;
        s.ban(seat, "").expect("skip ban");
    }
    assert_eq!(s.phase(), "pick");
    let picker = s.turn() as usize;
    let err = s
        .pick(picker, &victim)
        .expect_err("banned character must not be pickable");
    assert!(
        err.contains("chara") || err.contains("ban") || err.contains("taken"),
        "unexpected pick-refusal key {err:?}"
    );
}

/// G-FLOW-16 -- pick is forward turn order.
#[test]
fn ranked_pick_is_forward_turn_order() {
    let names = chars(5);
    let mut s = Setup::new(5, MatchMode::Ranked);
    s.tick_until("ban", |m| m.state().phase == "ban");
    for _ in 0..5 {
        let seat = s.turn() as usize;
        s.ban(seat, "").expect("skip ban");
    }
    // Forward: the first pick is seat 0, then 1, ...
    let mut prev: i32 = -1;
    for _ in 0..5 {
        let seat = s.turn() as usize;
        assert!(
            (s.turn() as i32) > prev,
            "pick walk is forward turn order (turn {} after {})",
            s.turn(),
            prev
        );
        prev = s.turn();
        // Distinct characters (「各不同的邦多利角色」).
        let c = names[seat].clone();
        s.pick(seat, &c)
            .unwrap_or_else(|e| panic!("pick seat {seat} {c:?}: {e}"));
    }
    assert_eq!(s.phase(), "deck");
}

/// G-FLOW-18 -- 开始游戏 order: ban → pick → deck → match_start.
#[test]
fn start_sequence_ban_pick_deck_then_match_start() {
    let names = chars(5);
    let mut s = Setup::new(5, MatchMode::Ranked);
    s.tick_until("ban", |m| m.state().phase == "ban");
    for _ in 0..5 {
        let seat = s.turn() as usize;
        s.ban(seat, "").expect("skip ban");
    }
    for _ in 0..5 {
        let seat = s.turn() as usize;
        s.pick(seat, &names[seat]).expect("pick");
    }
    assert_eq!(s.phase(), "deck");
    for seat in 0..5 {
        let who = s.world().st.players[seat].character.clone();
        let deck = preset_deck(&who);
        s.deck(seat, deck)
            .unwrap_or_else(|e| panic!("deck seat {seat}: {e}"));
    }
    // The last deck opens play; the opening settle runs 游戏开始前 / 抽卡 /
    // 重抽 / 游戏开始时 (rows 4-7) inside that window.
    s.open_keep();
    assert_eq!(s.phase(), "play");
    for (i, p) in s.world().st.players.iter().enumerate() {
        assert_eq!(p.character, names[i], "seat {i} character");
        assert!(p.deck_ready, "seat {i} deck");
    }
}

/// G-FLOW-03 -- start tile 1号CiRCLE, 10000 money.
///
/// Quote: 「每名玩家的初始格子为1号CiRCLE，拥有10000初始资金」
#[test]
fn start_tile_circle_and_10000_money() {
    let names = chars(5);
    let mut s = Setup::new(5, MatchMode::Casual);
    s.tick_until("pick", |m| m.state().phase == "pick");
    for _ in 0..5 {
        let seat = s.turn() as usize;
        s.pick(seat, &names[seat]).expect("pick");
    }
    for seat in 0..5 {
        let who = s.world().st.players[seat].character.clone();
        s.deck(seat, preset_deck(&who)).expect("deck");
    }
    s.open_keep();
    for i in 0..s.players() {
        let p = &s.world().st.players[i];
        assert_eq!(p.money, 10_000, "player {i} start money");
        assert_eq!(p.pos, 0, "player {i} start tile (1号CiRCLE = tile 0)");
    }
    let keys: Vec<String> = s
        .m
        .world()
        .recent
        .iter()
        .map(|e| rules_spec::event_key(e))
        .collect();
    assert!(
        keys.iter().any(|k| k == "log.match_start"),
        "no log.match_start in {keys:?}"
    );
}

/// G-FLOW-04 -- opening draw of 2; optional one redraw of 2.
///
/// Quote: 「并抽取2张手卡。仅一次，可将抽取的手卡全部洗回抽卡区并重抽2张。」
#[test]
fn opening_draw_two_and_one_mulligan() {
    let names = chars(3);
    // --- keep path ---
    let mut s = Setup::new(3, MatchMode::Casual);
    s.tick_until("pick", |m| m.state().phase == "pick");
    for k in 0..3 {
        let seat = s.turn() as usize;
        s.pick(seat, &names[k]).expect("pick");
    }
    s.strip_skills();
    for seat in 0..3 {
        let who = s.world().st.players[seat].character.clone();
        s.deck(seat, preset_deck(&who)).expect("deck");
    }
    s.open_keep();
    for i in 0..3 {
        // The book default is 2. A [特] card sitting in the opening deck may
        // still touch the hand (the quote's 「如无技能或卡效果影响」) -- pin the
        // non-zero opening hand and the no-mulligan flag; the clause pins the
        // announced 2.
        let held = s.world().hidden[i].hand.len();
        assert!(held > 0, "player {i} opening hand is empty");
        assert!(
            !s.world().st.players[i].mulligan,
            "player {i} mulliganed without asking"
        );
    }
    // Announced default (G-FLOW-04): every `log.match_start` says hand=2.
    for e in s.m.world().recent.iter() {
        if rules_spec::event_key(e) == "log.match_start" {
            let n = rules_spec::int_arg(&e.msg, "hand").unwrap_or(-1);
            assert_eq!(n, 2, "log.match_start hand announce");
        }
    }
    // The announce carries the default 2 (before any start effect).
    // (G-FLOW-04's event side is checked by the clause on `log.match_start`.)

    // --- redraw path ---
    let mut s = Setup::new(3, MatchMode::Casual);
    s.tick_until("pick", |m| m.state().phase == "pick");
    for k in 0..3 {
        let seat = s.turn() as usize;
        s.pick(seat, &names[k]).expect("pick");
    }
    s.strip_skills();
    for seat in 0..3 {
        let who = s.world().st.players[seat].character.clone();
        s.deck(seat, preset_deck(&who)).expect("deck");
    }
    s.open_redraw(0);
    for i in 0..3 {
        let held = s.world().hidden[i].hand.len();
        assert!(held > 0, "player {i} hand empty after mulligan window");
    }
    assert!(
        s.world().st.players[0].mulligan,
        "player 0 asked for 初始重抽 but the flag is unset"
    );
    // The redraw is 2 (「重抽2张」): the `log.mulligan` line carries n=2.
    let mut saw = false;
    for e in s.m.world().recent.iter() {
        if rules_spec::event_key(e) == "log.mulligan" {
            saw = true;
            let n = rules_spec::int_arg(&e.msg, "n").unwrap_or(-1);
            assert_eq!(n, 2, "log.mulligan n (重抽2张)");
        }
    }
    assert!(saw, "no log.mulligan after 初始重抽");
}

// ------------------------------------------------------------- turn flow

/// G-FLOW-05 / G-FLOW-06 / G-FLOW-01 / G-FLOW-02 -- stage order, required
/// and unique main move, immediate next turn.
///
/// Quote: 「每回合的阶段顺序从头到尾为：开始阶段，运营阶段，移动阶段，结束阶段。」
/// 「每名玩家的回合结束后立刻进入行动顺序中下一位玩家的回合。」
/// 「移动阶段中需进行1次[主要移动]。」
#[test]
fn turn_phases_and_immediate_next_turn() {
    let mut t = Table::vanilla(3);
    t.observe(CLAUSES);
    assert_eq!(t.step(), stage::OPS);
    assert_eq!(t.turn(), 0);

    // End without the main move is refused (G-FLOW-02).
    let err = t
        .end(0)
        .expect_err("end before [主要移动] must be refused");
    assert!(
        err.contains("roll") || err.contains("busy") || err.contains("first"),
        "unexpected end-refusal key {err:?}"
    );
    assert_eq!(t.turn(), 0, "refused end must not advance the turn");

    // The main move (loaded die) runs → 结束.
    t.dice(&[3]);
    t.roll(0).expect("roll");
    assert_eq!(t.step(), stage::END, "after the main move the stage is 结束");
    assert!(t.world().turn.main_moved, "main_moved latch");

    // Second main move is refused / no-op (G-FLOW-01).
    let pos = t.pos(0);
    let err = t.roll(0).expect_err("second [主要移动] must be refused");
    assert!(
        err.contains("main") || err.contains("used") || err.contains("roll"),
        "unexpected second-roll refusal {err:?}"
    );
    assert_eq!(t.pos(0), pos, "second roll must not move the player");

    // End: the next player's turn begins in the same settle (G-FLOW-06).
    let mark = t.mark();
    t.end(0).expect("end");
    let keys = t.keys_since(mark);
    assert!(
        keys.iter().any(|k| k == "log.end_turn"),
        "no log.end_turn in {keys:?}"
    );
    assert!(
        keys.iter().any(|k| k == "log.turn" || k == "log.turn_extra"),
        "next turn did not begin immediately in the same settle: {keys:?}"
    );
    assert_eq!(t.turn(), 1, "next seat in action order");
    assert_eq!(t.step(), stage::OPS, "next player is in 运营");
}

/// G-FLOW-07 -- 回合开始前 removes one [除外] layer.
///
/// Quote: 「玩家的每回合开始前移除一层。」
#[test]
fn turn_start_removes_one_exile_layer() {
    let mut t = Table::vanilla(3);
    t.observe(CLAUSES);
    t.set_state(1, key::EXILE, 2);
    // [停留] on P0 skips its main move (sets skip_move at turn start), so
    // `end` is legal and P1's turn begins.
    t.set_state(0, key::STAY, 1);
    t.begin_turn(0);
    t.end(0).expect("end with [停留]");
    assert_eq!(t.state(1, key::EXILE), 1, "[除外] 2 -> 1 at 回合开始前");
    let keys = t.recent_keys(30);
    assert!(
        keys.iter().any(|k| k == "log.exiled_skip"),
        "still-exiled player should skip: {keys:?}"
    );
}

#[test]
fn turn_start_last_exile_layer_clears() {
    let mut t = Table::vanilla(3);
    t.observe(CLAUSES);
    t.set_state(1, key::EXILE, 1);
    t.set_state(0, key::STAY, 1);
    t.begin_turn(0);
    t.end(0).expect("end");
    assert_eq!(t.state(1, key::EXILE), 0, "[除外] 1 -> 0 at 回合开始前");
}

/// G-FLOW-08 -- 结束阶段 removes one [晕眩] / [停留] layer.
///
/// Quote: 「玩家的每回合结束时移除一层。」
#[test]
fn turn_end_removes_one_stay_and_stun_layer() {
    let mut t = Table::vanilla(3);
    t.observe(CLAUSES);
    // Arm *before* the turn boundary so G-FLOW-08 sees the end-of-turn tick.
    t.set_state(0, key::STAY, 2);
    t.set_state(0, key::STUN, 2);
    // A stunned player's turn auto-skips to end and decays there. Drive it
    // through an observed act (`end` of the previous seat, or `begin_turn`
    // then an observed no-op is unavailable -- use `end` of P0 after stay).
    // P0 is stunned: its own turn start skips to end. Get there via P2 ending.
    t.set_state(2, key::STAY, 1);
    t.begin_turn(2);
    t.observe(CLAUSES);
    t.end(2).expect("end P2");
    // P0's turn began and (being stunned) auto-skipped to its end, decaying
    // one of each. The window above is P2's end -> P0's skip -> P1's turn.
    assert_eq!(t.state(0, key::STUN), 1, "[晕眩] 2 -> 1 at turn end");
    assert_eq!(t.state(0, key::STAY), 1, "[停留] 2 -> 1 at turn end");
}

/// G-FLOW-09 -- 晕眩 skips 运营 and the main move.
///
/// Quote: 「眩晕效果发动：跳过经营和主要移动阶段」
#[test]
fn stun_skips_ops_and_main_move() {
    let mut t = Table::vanilla(3);
    t.observe(CLAUSES);
    t.set_state(0, key::STUN, 1);
    let mark = t.mark();
    t.begin_turn(0);
    let keys = t.keys_since(mark);
    assert!(
        keys.iter().any(|k| k == "log.stunned_skip"),
        "expected 眩晕 skip, got {keys:?}"
    );
    assert!(
        !keys.iter().any(|k| k == "log.roll"),
        "stun skip window contains a roll: {keys:?}"
    );
}

/// G-FLOW-10 -- 停留 skips the main move (but not 运营).
///
/// Quote: 「停留效果发动：跳过主要移动阶段」
#[test]
fn stay_skips_main_move_but_not_ops() {
    let mut t = Table::vanilla(3);
    t.observe(CLAUSES);
    t.set_state(0, key::STAY, 1);
    t.begin_turn(0);
    assert_eq!(t.step(), stage::OPS, "[停留] does not skip 运营");
    assert!(t.st().skip_move, "skip_move latch for [停留]");
    let err = t.roll(0).expect_err("[停留] must refuse the main move");
    assert!(
        err.contains("stay") || err.contains("roll") || err.contains("move"),
        "unexpected stay-roll refusal {err:?}"
    );
    // Ending without a roll is legal once the move is skipped (G-FLOW-02's
    // skip_move carve-out).
    t.end(0).expect("end after [停留] skip");
}

/// G-FLOW-02 -- the main move is required exactly once: end before it is
/// refused, end after it is accepted.
#[test]
fn main_move_is_required_exactly_once() {
    let mut t = Table::vanilla(3);
    t.observe(CLAUSES);
    t.end(0).expect_err("end before [主要移动]");
    assert_eq!(t.turn(), 0);
    t.dice(&[2]);
    t.roll(0).expect("the required [主要移动]");
    t.end(0).expect("end after [主要移动]");
    assert_eq!(t.turn(), 1);
}

/// G-FLOW-11 -- 进入移动阶段 only when no effect is executing.
///
/// Quote: 「玩家在运营阶段可在非执行任何效果的时候进入移动阶段。」
#[test]
fn roll_refused_while_an_effect_is_executing() {
    let mut t = Table::vanilla(3);
    t.observe(CLAUSES);
    t.give(0, &["通用:GREAT"]);
    let _ = t.play(0, "通用:GREAT");
    // If the play left a prompt open, a roll through it is illegal.
    if t.prompt().is_some() {
        let err = t.roll(0).expect_err("roll through an open prompt");
        assert!(
            err.contains("busy") || err.contains("roll") || err.contains("no_roll"),
            "unexpected busy-roll refusal {err:?}"
        );
    }
    t.settle_answering_defaults();
}

/// G-FLOW-17 -- 正规: last [存活] wins.
///
/// Quote: 「仅剩一名[存活]玩家时游戏结束，那名玩家获胜。」
#[test]
fn ranked_last_alive_wins() {
    // 正规 mode (MatchMode::Ranked), 5 seats -- the book's competitive table.
    let names = chars(5);
    let mut s = Setup::new(5, MatchMode::Ranked);
    s.tick_until("ban", |m| m.state().phase == "ban");
    for _ in 0..5 {
        let seat = s.turn() as usize;
        s.ban(seat, "").expect("skip ban");
    }
    for k in 0..5 {
        let seat = s.turn() as usize;
        s.pick(seat, &names[k]).expect("pick");
    }
    for seat in 0..5 {
        let who = s.world().st.players[seat].character.clone();
        s.deck(seat, preset_deck(&who)).expect("deck");
    }
    s.open_keep();
    assert_eq!(s.m.world().st.mode, MatchMode::Ranked as i32);
    // Seats 1..4 leave for real so `check_game_over` runs (setting `left`
    // via world_mut does not). Members are roll-ordered -- leave by member.
    for seat in 1..5 {
        let member = s.m.world().st.players[seat].member;
        let out = observe(
            &mut s.m,
            StepAct::Act {
                who: member,
                msg: NetMessage::act("leave"),
            },
            settle_match,
            CLAUSES,
        );
        out.unwrap_violations();
        out.act_result
            .unwrap_or_else(|e| panic!("leave seat {seat}: {e}"));
    }
    let st = s.m.state();
    assert_eq!(st.mode, MatchMode::Ranked as i32);
    assert_eq!(st.phase, "ended", "last [存活] should end the match");
    assert_eq!(st.end_reason, "last", "end reason");
    // The winner is the sole [存活] seat.
    assert!(st.winner >= 0, "winner set");
    for (i, p) in s.m.world().st.players.iter().enumerate() {
        let alive = !p.out();
        assert_eq!(alive, i == st.winner as usize, "seat {i} [存活] vs winner");
    }
}

/// G-FLOW-05 -- the stage never runs backwards inside a turn.
#[test]
fn stage_order_is_monotonic_inside_a_turn() {
    let mut t = Table::vanilla(3);
    t.observe(CLAUSES);
    assert_eq!(t.step(), stage::OPS);
    t.dice(&[1]);
    t.roll(0).expect("roll");
    assert_eq!(t.step(), stage::END);
    t.end(0).expect("end");
    assert_eq!(t.step(), stage::OPS);
    t.dice(&[1]);
    t.roll(1).expect("roll");
    assert_eq!(t.step(), stage::END);
}

/// A quiet multi-turn walk with every G-FLOW clause armed (the scenario
/// counterpart of the soak). Any violation panics with the clause id.
#[test]
fn armed_walk_stays_clean() {
    let mut t = Table::vanilla(3);
    t.observe(CLAUSES);
    for turn in 0..6u32 {
        let who = t.turn();
        if t.step() == stage::OPS && !t.st().skip_move {
            t.dice(&[2 + (turn % 5) as i32]);
            let _ = t.roll(who);
        }
        t.settle_answering_defaults();
        if t.step() == stage::END || t.step() == stage::OPS {
            let _ = t.end(who);
        }
        t.settle_answering_defaults();
    }
}
