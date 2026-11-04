//! G14 -- two separate fixes from the same review batch:
//!
//! * **`hand-limit-lift`** (rules 441, `Mor:（NNM）稍微努力了一下` (1)):
//!   > 抽x张卡（可超过上限），回合结束后将手牌弃置到五张
//!
//!   「可超过上限」 is an exemption on that one draw: the cards are drawn, the
//!   hand may stay over the limit, and the turn can still end. The card's own
//!   「回合结束后」 dispose brings the hand back down; the turn-end sweep is the
//!   backstop (a lowered cap, a cancelled schedule).
//!
//! * **`aiplay-hook`** (rules 1253, `Mor:夏日合宿`): a static `aiNever` prop so
//!   the standard / advanced bot policies do not offer a card that is a bad
//!   bot play, while human legality (`playable`) is untouched.
//!
//! Design: `docs/rulebook/spec/abi/G14-ai-and-hand-limit.md`.

#[path = "testworld.rs"]
mod testworld;

use game_core::engine::{CardRules, Match};
use game_core::state::key;
use game_rules::{Call, CardWorld, HostRequest, Outcome};
use rules_spec::global::cards::{g_cards_01, CLAUSES};
use rules_spec::{data, rules, Clause, Table};

const NNM: &str = "Mor:（NNM）稍微努力了一下";
const SUMMER: &str = "Mor:夏日合宿";
const MARK: &str = "角色标记:skill:户山香澄:鼓动";

/// Five harmless hand fillers (distinct from NNM).
const FILLER: [&str; 5] = [
    "通用:GREAT!",
    "通用:PERFECT!",
    "通用:FEVER!",
    "通用:安可",
    "通用:登上武道馆",
];

fn decline_all(t: &mut Table) {
    for _ in 0..12 {
        if t.prompt().is_none() {
            return;
        }
        t.decline();
    }
}

/// A quiet table with `marks` 角色标记 on P0 and no board residue. The named
/// character is NNM's owner; skills are stripped so only the card under test
/// answers.
fn nnm_table(marks: i32) -> Table {
    let mut t = Table::new(&["广町七深", "户山香澄", "花园多惠"]);
    t.clean();
    t.begin_turn(0);
    if marks > 0 {
        t.m.world_mut().st.players[0]
            .tokens
            .push(game_core::state::Counter {
                name: MARK.into(),
                value: marks,
                instance: None,
                max: 0,
            });
    }
    t
}

/// Five cards in hand (NNM + four fillers) and `x` cards on top of the draw
/// pile. Five, not six: CRYCHIC's group skill closes the play gate at six
/// (`hand(actor) >= 6`).
fn arm_hand(t: &mut Table, x: usize) {
    let mut hand: Vec<&str> = vec![NNM];
    hand.extend(FILLER.iter().take(4).copied());
    t.set_hand(0, &hand);
    assert_eq!(t.hand(0).len(), hand.len(), "armed hand");
    let draw = vec!["R:[衍生] 压"; x];
    t.set_draw(0, &draw);
}

/// Play NNM for `x` marks, take branch (1) (draw x), and drain the play's
/// remaining prompts. Returns the hand size right after the draw settled.
fn play_nnm_branch_one(t: &mut Table, marks: i32, x: i32) -> usize {
    assert_eq!(
        t.p(0).token(MARK),
        marks,
        "the table must carry the marks the play spends"
    );
    arm_hand(t, x.max(0) as usize);
    t.play(0, NNM).expect("NNM hand play");
    if x > 1 {
        // `ask_number` answers by option index: 0 = 1, so x - 1 asks for x.
        t.answer(0, x - 1).expect("x");
    }
    t.answer_one(0).expect("branch (1)");
    decline_all(t);
    t.hand(0).len()
}

/// Roll the main move and drain the landing prompts (the roll is what the end
/// branch still owes; the landing is a plain property, no draw).
fn move_quiet(t: &mut Table) {
    t.set_pos(0, 0);
    t.dice(&[1]);
    t.roll(0).expect("main move");
    decline_all(t);
}

// ---------------------------------------------------------------------------
// hand-limit-lift
// ---------------------------------------------------------------------------

/// The clause's core: an exempt draw is **not** trimmed. Five in hand, NNM and
/// two marks; playing it spends the card and x=2 draws two more -- six, over
/// the limit of five, and nothing went to the discard.
#[test]
fn g14_nnm_draw_over_cap_is_not_trimmed() {
    let mut t = nnm_table(2);
    let hand = play_nnm_branch_one(&mut t, 2, 2);
    assert_eq!(hand, 6, "hand {:?}: four stayed, two drawn", t.hand(0));
    assert_eq!(t.p(0).hand_limit(), Some(5));
    assert!(t.p(0).over_hand(6), "six is over the limit of five");
    assert!(
        !t.p(0).over_hand_blocking(6),
        "「可超过上限」: the excess must not block"
    );
    assert!(t.p(0).over_hand_allowed, "the exempt draw sets the allowance");
}

/// With the allowance set the end branch is open: `can_end_here` is true and
/// the engine accepts `act:"end"`.
#[test]
fn g14_over_hand_allowed_lets_the_turn_end() {
    let mut t = nnm_table(2);
    play_nnm_branch_one(&mut t, 2, 2);
    move_quiet(&mut t);
    assert!(t.m.state().can_end_here, "the end must be offered");
    t.end(0)
        .unwrap_or_else(|e| panic!("end over the limit was refused: {e}"));
}

/// 「回合结束后将手牌弃置到五张」 -- the card's own scheduled dispose runs at
/// turn end; the allowance is spent with it.
#[test]
fn g14_nnm_end_of_turn_discards_to_five() {
    let mut t = nnm_table(2);
    play_nnm_branch_one(&mut t, 2, 2);
    move_quiet(&mut t);
    t.end(0).expect("end");
    t.settle();
    decline_all(&mut t);
    assert_eq!(t.hand(0).len(), 5, "scheduled dispose: {:?}", t.hand(0));
    assert!(
        !t.p(0).over_hand_allowed,
        "the allowance is spent at turn end"
    );
}

/// The turn-end sweep is the backstop: with the cap lowered to four the
/// scheduled body (to five) is not enough, and the sweep takes the rest.
#[test]
fn g14_turn_end_sweeps_a_lowered_cap() {
    let mut t = nnm_table(2);
    t.set_state(0, key::HAND_LIMIT, 4);
    play_nnm_branch_one(&mut t, 2, 2);
    assert_eq!(t.hand(0).len(), 6);
    move_quiet(&mut t);
    t.end(0).expect("end");
    t.settle();
    decline_all(&mut t);
    assert_eq!(t.hand(0).len(), 4, "swept to the lowered cap: {:?}", t.hand(0));
    assert!(!t.p(0).over_hand_allowed);
}

/// A manual discard back down to the limit clears the allowance; a later draw
/// that overflows blocks the end again.
#[test]
fn g14_discard_clears_the_allowance() {
    let mut t = nnm_table(2);
    play_nnm_branch_one(&mut t, 2, 2);
    let hand = t.hand(0);
    // The `discard` command is only offered over the limit, so one is enough
    // (six -> five).
    t.discard_card(0, &hand[0]).expect("manual discard");
    assert_eq!(t.hand(0).len(), 5, "back at the limit");
    assert!(
        !t.p(0).over_hand_allowed,
        "fitting again clears the allowance"
    );

    // A plain draw now overflows an unallowed hand: nothing trims it for a
    // *pending* draw here (the settle does), so block the end explicitly with
    // a hand-card add.
    t.give(0, &["通用:THANKS PARTY!"]);
    assert_eq!(t.hand(0).len(), 6);
    assert!(t.p(0).over_hand_blocking(6), "an unallowed overflow blocks");
    t.discard_card(0, "通用:THANKS PARTY!").expect("discard again");
    move_quiet(&mut t);
    assert!(!t.p(0).over_hand_blocking(5));
}

/// Control: a plain `ctx::draw` overflow on the same table still trims in the
/// same transition and leaves the allowance false.
#[test]
fn g14_plain_draw_still_trims() {
    let mut t = nnm_table(0);
    t.set_hand(0, &FILLER);
    assert_eq!(t.hand(0).len(), 5);
    t.set_draw(0, &["通用:THANKS PARTY!"]);
    // 江户川乐器店 settle = draw 1 -> 6, immediately trimmed back to 5.
    t.set_pos(0, 29);
    t.dice(&[1]);
    t.roll(0).expect("main move");
    decline_all(&mut t);
    assert_eq!(t.hand(0).len(), 5, "plain draw trimmed: {:?}", t.hand(0));
    assert!(!t.p(0).over_hand_allowed);
}

/// The draw request carries the flag. `ctx::draw_over_hand_limit` pauses on
/// `HostRequest::Draw { over_hand_limit: true }`; the plain `ctx::draw` card
/// pauses with `false`.
#[test]
fn g14_draw_over_hand_limit_reaches_the_host() {
    let r = testworld::ruleset();
    let mut w = testworld::TestWorld::new(1);
    w.set_held(0, MARK, 2, 0);
    let call = |card: &str| Call::Play {
        card: r.card(card).expect("card"),
        player_id: 0,
        arg: Vec::new(),
    };

    // Drive the answer log like `rules/tests/tests/ruleset.rs`: the marker
    // window stands (1), the x ask is answered with option index 1 (= x 2),
    // the branch with 0 (draw x) -- appended at the slot the run names.
    let mut answers: Vec<i32> = Vec::new();
    let out = loop {
        match r.run(&w, call(NNM), &answers, None).unwrap() {
            Outcome::NeedHost(HostRequest::Raise { .. }, _) => answers.push(1),
            Outcome::NeedHost(HostRequest::Marker { .. }, _) => answers.push(1),
            Outcome::NeedHost(HostRequest::Draw { over_hand_limit, .. }, _) => {
                assert!(over_hand_limit, "「可超过上限」 must reach the host");
                break;
            }
            Outcome::NeedInput(p, _) => {
                while answers.len() < p.answer_slot as usize {
                    answers.push(0);
                }
                answers.push(i32::from(p.title.key().contains("how_many")));
            }
            other => panic!("expected the exempt draw request, got {other:?}"),
        }
    };
    let _ = out;

    // Plain contrast: 10次招募 first pays (reply 0), then draws unexempt.
    let mut answers: Vec<i32> = Vec::new();
    loop {
        match r
            .run(&w, call("通用:10次招募（1回限定）"), &answers, None)
            .unwrap()
        {
            Outcome::NeedHost(HostRequest::Raise { .. }, _) => answers.push(1),
            Outcome::NeedHost(HostRequest::Pay { .. }, _) => answers.push(0),
            Outcome::NeedHost(HostRequest::PayCommand { .. }, _) => answers.push(0),
            Outcome::NeedHost(HostRequest::Draw { over_hand_limit, .. }, _) => {
                assert!(!over_hand_limit, "a plain ctx::draw carries false");
                break;
            }
            other => panic!("expected the plain draw request, got {other:?}"),
        }
    }
}

/// 「回合结束后」 -- the draw schedules the dispose callback rather than doing
/// it inline.
#[test]
fn g14_effect_draw_schedules_the_discard() {
    let mut t = nnm_table(2);
    play_nnm_branch_one(&mut t, 2, 2);
    let scheduled: Vec<_> = t
        .m
        .world()
        .scheduled
        .iter()
        .filter(|s| s.entry == "discard_down_to_five")
        .collect();
    assert_eq!(
        scheduled.len(),
        1,
        "one turn-end dispose: {:?}",
        t.m.world().scheduled
    );
    assert_eq!(scheduled[0].target, 0);
    assert_eq!(scheduled[0].card, NNM);
}

/// The allowance is durable: a save / restore keeps it, the end gate and the
/// scheduled dispose still work.
#[test]
fn g14_allowance_survives_save_restore() {
    let mut t = nnm_table(2);
    play_nnm_branch_one(&mut t, 2, 2);
    move_quiet(&mut t);
    let json = t.m.save();
    t.m = Match::restore(data(), rules(), &json).expect("restore");
    assert!(t.p(0).over_hand_allowed, "the allowance survives");
    assert!(t.m.state().can_end_here);
    t.end(0).expect("end after restore");
    t.settle();
    decline_all(&mut t);
    assert_eq!(t.hand(0).len(), 5);
    assert!(!t.p(0).over_hand_allowed);
}

/// The log part names the exemption (and the plain overflow names the plain
/// part), so a replay reads why the hand stayed over.
#[test]
fn g14_over_limit_log_part() {
    let mut t = nnm_table(2);
    let mark = t.mark();
    play_nnm_branch_one(&mut t, 2, 2);
    let seen: Vec<String> = t.events_since(mark).iter().map(|e| format!("{:?}", e.msg)).collect();
    // The closing quote keeps `over_hand` from matching `over_hand_allowed`.
    assert!(
        seen.iter().any(|m| m.contains("log.part.over_hand_allowed")),
        "the exempt draw must log the allowed part: {seen:?}"
    );
    assert!(
        !seen.iter().any(|m| m.contains("log.part.over_hand\"")),
        "the exempt draw must not log the plain part: {seen:?}"
    );

    // The plain overflow logs the plain part.
    let mut t = nnm_table(0);
    t.set_hand(0, &FILLER);
    t.set_draw(0, &["通用:THANKS PARTY!"]);
    let mark = t.mark();
    t.set_pos(0, 29);
    t.dice(&[1]);
    t.roll(0).expect("main move");
    decline_all(&mut t);
    let seen: Vec<String> = t.events_since(mark).iter().map(|e| format!("{:?}", e.msg)).collect();
    assert!(
        seen.iter().any(|m| m.contains("log.part.over_hand\"")),
        "the plain overflow must log the plain part: {seen:?}"
    );
    assert!(
        !seen.iter().any(|m| m.contains("log.part.over_hand_allowed")),
        "a plain overflow must not log the allowed part: {seen:?}"
    );
}

// ---------------------------------------------------------------------------
// aiplay-hook
// ---------------------------------------------------------------------------

/// The card declares the static prop.
#[test]
fn g14_summer_declares_ai_never() {
    assert_eq!(
        rules().card_prop(SUMMER, game_core::state::prop::AI_NEVER),
        1
    );
}

/// A standard bot holding only 夏日合宿 rolls and ends instead of playing it.
#[test]
fn g14_standard_bot_skips_summer_camp() {
    let mut t = Table::vanilla(2);
    t.make_bot(0, game_core::state::BotMentality::Standard);
    t.set_hand(0, &[SUMMER]);
    t.begin_turn(0);
    // A bot policy runs only on a drive hint (`docs/BOT-DRIVE.md`); the seat's
    // own schedule rolls / answers, the drive makes it act.
    t.tick_until("the turn leaves the seat", |t| t.turn() != 0);
    assert!(
        t.hand(0).iter().any(|c| c == SUMMER),
        "the bot must keep 夏日合宿: {:?}",
        t.hand(0)
    );
}

/// The shared bot policy (`Match::bot_action`'s body) honours the allowance:
/// a blocking overflow gets a discard, an allowed excess gets the end.
#[test]
fn g14_standard_bot_keeps_an_allowed_excess() {
    let mut t = nnm_table(0);
    // 夏日合宿 is never offered to the standard policy, so the hand stays put.
    t.set_hand(0, &[SUMMER, SUMMER, SUMMER, SUMMER, SUMMER, SUMMER]);
    t.make_bot(0, game_core::state::BotMentality::Standard);
    move_quiet(&mut t);
    let member = t.p(0).member;
    let cmd = bot_core::policy::act_for(&t.m, member).expect("a command");
    assert_eq!(cmd.act, "discard", "a blocking overflow discards");

    t.m.world_mut().st.players[0].over_hand_allowed = true;
    let cmd = bot_core::policy::act_for(&t.m, member).expect("a command");
    assert_eq!(cmd.act, "end", "an allowed excess keeps the cards");
}

/// Contrast: the same fixture plays a card without the prop. The standard
/// policy only *may* play per turn (a chance roll), so give it turns.
#[test]
fn g14_standard_bot_plays_a_normal_card() {
    const CARD: &str = "通用:10次招募（1回限定）";
    let mut t = Table::vanilla(2);
    t.make_bot(0, game_core::state::BotMentality::Standard);
    t.set_hand(0, &[CARD]);
    let mut played = false;
    for _ in 0..10 {
        t.begin_turn(0);
        t.tick_until("the turn leaves the seat", |t| t.turn() != 0);
        if !t.hand(0).iter().any(|c| c == CARD) {
            played = true;
            break;
        }
    }
    assert!(played, "the control card is never played: {:?}", t.hand(0));
}

/// Chaos keeps no avoidance list: the card is played.
#[test]
fn g14_chaos_still_plays_summer_camp() {
    let mut t = Table::vanilla(2);
    t.make_bot(0, game_core::state::BotMentality::Chaos);
    t.set_hand(0, &[SUMMER]);
    t.begin_turn(0);
    t.tick_until("summer camp leaves the hand", |t| {
        !t.hand(0).iter().any(|c| c == SUMMER)
    });
}

/// Humans are untouched: `playable` stays true and the manual play is
/// accepted.
#[test]
fn g14_human_can_still_play_summer_camp() {
    let mut t = Table::vanilla(2);
    assert!(t.skills(0).is_empty() || !t.p(0).ai);
    t.set_hand(0, &[SUMMER]);
    t.begin_turn(0);
    t.play(0, SUMMER).expect("human play of 夏日合宿");
    assert!(t.on_field(0, SUMMER), "placed to the field");
}

/// G-CARDS-01 must not fire on the permitted excess, while a plain overflow
/// still violates it.
#[test]
fn g_cards_01_allows_a_card_permitted_draw() {
    let mut t = nnm_table(2);
    arm_hand(&mut t, 2);
    t.observe(&[Clause::new(
        "G-CARDS-01",
        "rulebook-doc.md:126",
        "基础手卡上限为5张，任何时刻若持有手卡超过上限则需要立刻将手卡弃至上限张。",
        g_cards_01,
    )]);
    // `observe` panics on a violation at each observed step.
    t.play(0, NNM).expect("NNM hand play");
    t.answer(0, 1).expect("x");
    t.answer_one(0).expect("branch (1)");
    decline_all(&mut t);
    assert_eq!(t.hand(0).len(), 6);

    // The control: a plain overflow on a fresh unallowed table still trips
    // the clause unless it is trimmed in the same transition -- it is, so this
    // half also passes; the assert is the hand going back to five.
    let mut t = nnm_table(0);
    t.set_hand(0, &FILLER);
    t.set_draw(0, &["通用:THANKS PARTY!"]);
    t.set_pos(0, 29);
    t.dice(&[1]);
    t.observe(CLAUSES);
    t.roll(0).expect("main move");
    decline_all(&mut t);
    assert_eq!(t.hand(0).len(), 5);
}

// (`ViewExtra::ai_playable` itself is pinned in
// `crates/game-core/tests/view_extra.rs` -- its JSON twin is `aiPlayable` on
// the frame, merged by both view builders.)

