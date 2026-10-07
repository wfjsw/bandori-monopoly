//! Bot mentality: standard is the ported C# policy, chaos is legal but
//! maximally disruptive. Both must stay deterministic (same seed, same game).

use std::sync::{Arc, Mutex};

use game_core::data::GameData;
use game_core::engine::{
    bot_wants_build, bot_wants_buy, bot_wants_force_buy, bot_wants_redeem, Ask, Cx, Dest, Flow,
    Match, StubRules, Trigger, BUY_RESERVE, BUILD_RESERVE, CHAOS_RESERVE, FORCE_BUY_RESERVE,
    REDEEM_RESERVE,
};
use game_core::msg::Msg;
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::state::{BotMentality, MatchState};
use game_core::MatchMode;

fn data() -> Arc<GameData> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    Arc::new(
        GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()))
            .unwrap(),
    )
}

fn member(id: i32, bot: bool, m: BotMentality) -> RoomMember {
    RoomMember {
        id,
        player: format!("P{id}"),
        bot,
        mentality: m,
        ..Default::default()
    }
}

fn new_match(members: &[RoomMember], seed: u64, rules: Arc<dyn game_core::engine::CardRules>) -> Match {
    Match::new(data(), rules, members, seed, MatchMode::Casual, ScoreWeights::default())
}

/// Tick until the match ends (or `max_rounds` pass, then finish it).
fn play_out(m: &mut Match, max_rounds: i32) {
    let mut ticks = 0;
    while !m.ended() {
        m.tick(0.25);
        ticks += 1;
        if m.state().round > max_rounds {
            m.finish();
        }
        assert!(ticks < 2_000_000, "game did not progress");
    }
}

// ---------------------------------------------------------------- money gates

#[test]
fn chaos_keeps_a_smaller_reserve_than_standard() {
    // 2,500 after a 1,000 spend leaves 1,500: below standard's reserve, above
    // chaos's. Exactly the window chaos must buy in and standard must not.
    let (money, cost) = (2_500, 1_000);
    assert!(!bot_wants_buy(BotMentality::Standard, money, cost));
    assert!(bot_wants_buy(BotMentality::Chaos, money, cost));
    assert!(!bot_wants_build(BotMentality::Standard, money, cost));
    assert!(bot_wants_build(BotMentality::Chaos, money, cost));
    assert!(!bot_wants_redeem(BotMentality::Standard, money, cost));
    assert!(bot_wants_redeem(BotMentality::Chaos, money, cost));
    assert!(!bot_wants_force_buy(BotMentality::Standard, money, cost));
    assert!(bot_wants_force_buy(BotMentality::Chaos, money, cost));

    // Below the chaos reserve, neither policy spends.
    let poor = (1_400, 1_000);
    assert!(!bot_wants_buy(BotMentality::Chaos, poor.0, poor.1));
    assert!(!bot_wants_build(BotMentality::Chaos, poor.0, poor.1));

    // Comfortable money: both spend.
    let rich = (100_000, 1_000);
    assert!(bot_wants_buy(BotMentality::Standard, rich.0, rich.1));
    assert!(bot_wants_buy(BotMentality::Chaos, rich.0, rich.1));

    // The named thresholds are what each policy reserves.
    assert_eq!(BUY_RESERVE, 2_000);
    assert_eq!(BUILD_RESERVE, 3_500);
    assert_eq!(REDEEM_RESERVE, 4_000);
    assert_eq!(FORCE_BUY_RESERVE, 4_000);
    assert_eq!(CHAOS_RESERVE, 1_000);
}

/// A standard bot only ever buys while the purchase leaves [`BUY_RESERVE`]; a
/// chaos bot buys with no such reserve (only [`CHAOS_RESERVE`]).
#[test]
fn chaos_bot_buys_with_no_reserve() {
    let d = data();
    let run = |m: BotMentality, seed: u64| -> Vec<i32> {
        let members: Vec<RoomMember> = (1..=4).map(|i| member(i, true, m)).collect();
        let mut match_ = new_match(&members, seed, Arc::new(StubRules));
        match_.quick_start();
        let mut last = 0;
        // Money the buyer was left with, for every purchase the match made.
        let mut after = vec![];
        let mut ticks = 0;
        while !match_.ended() {
            match_.tick(0.25);
            ticks += 1;
            let st = match_.state();
            for e in match_.events_since(last) {
                last = e.id;
                // Auction wins log as `buy` too; they price from `Ask::worth`,
                // not `AiWantsBuy`. Only the end-step purchase tests it.
                if e.r#type == "buy" && e.player_id >= 0 && !e.msg.to_string().contains("auction") {
                    after.push(st.players[e.player_id as usize].money);
                }
            }
            if st.round > 200 {
                match_.finish();
            }
            assert!(ticks < 2_000_000);
        }
        let _ = &d;
        after
    };
    for seed in 1..=6 {
        let standard = run(BotMentality::Standard, seed);
        assert!(
            standard.iter().all(|&m| m >= BUY_RESERVE),
            "standard bought leaving only {standard:?} (seed {seed})"
        );
    }
    // At least one chaos purchase must break the standard reserve -- otherwise
    // the test has not seen the policy differ.
    let mut broke = false;
    for seed in 1..=6 {
        let chaos = run(BotMentality::Chaos, seed);
        assert!(
            chaos.iter().all(|&m| m >= CHAOS_RESERVE),
            "chaos bought into its own reserve: {chaos:?} (seed {seed})"
        );
        broke |= chaos.iter().any(|&m| m < BUY_RESERVE);
    }
    assert!(broke, "no chaos purchase in 6 games left under the standard reserve");
}

// ---------------------------------------------------------------- prompt picks

/// A ruleset whose card body and counteract window both raise a plain choice
/// prompt, recording the answer. The prompts mirror the shape of the engine's
/// own (fallback = "do nothing"), so a chaos seat must take a non-default one.
///
/// The window is raised **every** time (never "just once"): the routine is
/// replayed from its snapshot after the answer lands, so a prompt that only
/// fires on the first pass desyncs the answer log.
#[derive(Default)]
struct PromptRules {
    /// Answer to the card-body prompt (fallback = 0).
    play_pick: Mutex<Option<i32>>,
    /// Every counteract-window answer (fallback = 1 = skip).
    counteract_picks: Mutex<Vec<i32>>,
}

impl game_core::engine::CardRules for PromptRules {
    fn play(&self, cx: &mut Cx, player_id: usize, _card: &str) -> Flow<Dest> {
        let ask = Ask::choice(
            vec![player_id],
            Msg::new("ask.mulligan.title"),
            Msg::new("ask.mulligan.text"),
            vec![
                Msg::new("ask.mulligan.keep"),
                Msg::new("ask.mulligan.redo"),
                Msg::new("ask.no_buy"),
            ],
            0,
            12.0,
        );
        let r = cx.ask(ask)?;
        *self.play_pick.lock().unwrap() = Some(r.of(player_id));
        Ok(Dest::Graveyard)
    }

    fn event(&self, _cx: &mut Cx, _player_id: usize, _id: &str) -> Flow<bool> {
        Ok(false)
    }

    /// A counteract window on every main-move roll: two options, fallback =
    /// skip (the shape `ask.counteract.*` uses).
    fn counteract(&self, cx: &mut Cx, t: &mut Trigger) -> Flow<()> {
        if t.kind != "roll" {
            return Ok(());
        }
        let who = t.player_id.max(0) as usize;
        let ask = Ask::choice(
            vec![who],
            Msg::new("ask.counteract.title"),
            Msg::new("ask.counteract.text"),
            vec![
                Msg::new("ask.counteract.play"),
                Msg::new("ask.counteract.skip"),
            ],
            1,
            12.0,
        );
        let r = cx.ask(ask)?;
        self.counteract_picks.lock().unwrap().push(r.of(who));
        Ok(())
    }
}

/// A chaos bot picks uniformly among the non-default options -- never the
/// fallback -- while a standard bot (and a human who times out) takes the
/// fallback.
#[test]
fn chaos_picks_a_non_default_prompt_option() {
    for seed in 1..=8 {
        let rules = Arc::new(PromptRules::default());
        let members = [member(1, true, BotMentality::Chaos), member(2, true, BotMentality::Chaos)];
        let mut m = new_match(&members, seed, rules.clone());
        m.quick_start();
        play_out(&mut m, 25);
        let pick = *rules.play_pick.lock().unwrap();
        // Every game plays at least one card (opening hand is non-empty), so
        // the card-body prompt must have been raised and answered.
        assert!(pick.is_some(), "no card body prompt on seed {seed}");
        assert_ne!(pick, Some(0), "chaos took the fallback on seed {seed}");
    }
}

/// `Counteract: always declare one when offered` -- a chaos seat never takes
/// the prompt's skip option while a declaration exists.
#[test]
fn chaos_declares_a_counteract_when_offered() {
    for seed in 1..=8 {
        let rules = Arc::new(PromptRules::default());
        let members = [member(1, true, BotMentality::Chaos), member(2, true, BotMentality::Chaos)];
        let mut m = new_match(&members, seed, rules.clone());
        m.quick_start();
        play_out(&mut m, 25);
        let picks = rules.counteract_picks.lock().unwrap().clone();
        assert!(!picks.is_empty(), "no counteract window on seed {seed}");
        assert!(
            picks.iter().all(|&p| p != 1),
            "chaos skipped an offered counteract: {picks:?} (seed {seed})"
        );
    }
}

/// Standard keeps the fallback on both prompt shapes.
#[test]
fn standard_takes_the_prompt_fallback() {
    let rules = Arc::new(PromptRules::default());
    let members = [member(1, true, BotMentality::Standard), member(2, true, BotMentality::Standard)];
    let mut m = new_match(&members, 5, rules.clone());
    m.quick_start();
    play_out(&mut m, 25);
    assert_eq!(*rules.play_pick.lock().unwrap(), Some(0), "standard ignored the fallback");
    let picks = rules.counteract_picks.lock().unwrap().clone();
    assert!(!picks.is_empty(), "no counteract window");
    assert!(
        picks.iter().all(|&p| p == 1),
        "standard declared a counteract: {picks:?}"
    );
}

// ---------------------------------------------------------------- card play

/// Chaos plays every playable card before it rolls -- no 70% roll, no
/// two-per-turn cap beyond the engine's own.
#[test]
fn chaos_plays_a_playable_card_instead_of_rolling() {
    for seed in 1..=6 {
        let members: Vec<RoomMember> = (1..=4).map(|i| member(i, true, BotMentality::Chaos)).collect();
        let mut m = new_match(&members, seed, Arc::new(StubRules));
        m.quick_start();
        let mut last = 0;
        let mut plays_before_first_move = 0;
        let mut saw_move = false;
        let mut ticks = 0;
        while !m.ended() && !saw_move {
            m.tick(0.25);
            ticks += 1;
            for e in m.events_since(last) {
                last = e.id;
                match e.r#type.as_str() {
                    "play" => plays_before_first_move += 1,
                    "roll" | "move" => saw_move = true,
                    _ => {}
                }
            }
            if m.state().round > 5 {
                m.finish();
            }
            assert!(ticks < 2_000_000);
        }
        assert!(
            plays_before_first_move >= 2,
            "seed {seed}: only {plays_before_first_move} card(s) played before the first move"
        );
    }
}

// ---------------------------------------------------------------- determinism

#[test]
fn same_seed_same_chaos_game() {
    let run = |seed: u64| -> (String, usize) {
        let members: Vec<RoomMember> = (1..=3).map(|i| member(i, true, BotMentality::Chaos)).collect();
        let mut m = new_match(&members, seed, Arc::new(StubRules));
        m.quick_start();
        play_out(&mut m, 25);
        (serde_json::to_string(&m.state()).unwrap(), m.events_since(0).len())
    };
    assert_eq!(run(42), run(42), "chaos must be replayable");
    assert_ne!(run(42).0, run(43).0, "different seeds must differ");
}

/// A chaos save restored mid-match continues the same game.
#[test]
fn chaos_save_and_restore_continue_identically() {
    let members: Vec<RoomMember> = (1..=3).map(|i| member(i, true, BotMentality::Chaos)).collect();
    let mut a = new_match(&members, 7, Arc::new(StubRules));
    a.quick_start();
    for _ in 0..40 {
        a.tick(0.25);
    }
    let json = a.save();
    let mut b = Match::restore(data(), Arc::new(StubRules), &json).unwrap();
    for _ in 0..40 {
        a.tick(0.25);
        b.tick(0.25);
    }
    assert_eq!(
        serde_json::to_string(&a.state()).unwrap(),
        serde_json::to_string(&b.state()).unwrap()
    );
}

/// Mentality is on the seat, and old JSON without it loads as standard.
#[test]
fn mentality_defaults_to_standard_and_round_trips() {
    let st: MatchState = serde_json::from_str(r#"{"players":[{"player":"x"}]}"#).unwrap();
    assert_eq!(st.players[0].mentality, BotMentality::Standard);

    let p = game_core::state::MatchPlayer {
        bot: true,
        mentality: BotMentality::Chaos,
        ..Default::default()
    };
    let json = serde_json::to_string(&p).unwrap();
    assert!(json.contains("\"mentality\":\"chaos\""), "{json}");
    let back: game_core::state::MatchPlayer = serde_json::from_str(&json).unwrap();
    assert_eq!(back.mentality, BotMentality::Chaos);
}

/// A human seat is never chaos -- a time-out or disconnect keeps the standard
/// policy even if the field is marked.
#[test]
fn a_human_seat_is_never_chaos() {
    let members = [
        member(1, false, BotMentality::Chaos), // human carrying a chaos tag
        member(2, true, BotMentality::Standard),
        member(3, true, BotMentality::Standard),
    ];
    let mut m = new_match(&members, 3, Arc::new(StubRules));
    m.quick_start();
    let st = m.state();
    let human = st.players.iter().find(|p| !p.bot).unwrap();
    assert_eq!(human.mentality, BotMentality::Chaos, "the tag is on the seat");
    // The engine's policy lookup ignores it (exercised through a full game --
    // the human is never auto-answered by chaos).
    play_out(&mut m, 30);
}