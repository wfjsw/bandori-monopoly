//! G06 integration tests: counteraction answer contracts, join windows,
//! counteraction history and copy carry-forward.
//!
//! Plan: `docs/rulebook/spec/abi/G06-counteraction-and-join.md` §9.1 (one test
//! per table row). Expectations come from the quoted card text and the design's
//! §6, never from the engine.

mod common;

use common::*;
use game_core::msg::Arg;

const FILL: &str = "R:[衍生] 觉悟";
const MANA: &str = "Sumimi:（真奈）歌唱大赛5连冠";
const NET_ERROR: &str = "通用:网络链接异常";
const JOIN_OPEN: &str = "TEST:joinOpen";
const JOIN_SHIELD: &str = "TEST:joinShield";
const AIMER: &str = "TEST:aimer";
const MORTIS: &str = "Mujica:（睦/mortis）表演的本能";
const TSUGUMI: &str = "AG:（鸫）微小的『能做到』的事";
const YCM: &str = "通用:@Tsugu ycm";
const YOLO: &str = "AG:Y.O.L.O";
const VOCAL: &str = "CRYCHIC:主唱太拼命了";
const REPAINT: &str = "RAS:Repaint";
const CANT_LOOK: &str = "Mujica:无法将视线移开";
const SUNSET: &str = "AG:无论是何种颜色的夕阳";
const EDOGAWA_PARK: usize = 7;

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

fn set_draw_n(t: &mut Table, who: usize, n: usize) {
    let cards: Vec<&str> = vec![FILL; n];
    t.set_draw(who, &cards);
}

/// A clean table without skills, on player 0's turn, at rest.
fn fresh(chars: &[&str]) -> Table {
    let mut t = Table::new(chars);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t
}

/// Drain, recording which seats were asked an offer of `card`.
fn record_offers(t: &mut Table, card: &str, asked: &mut Vec<usize>) -> bool {
    let mut offered = false;
    for _ in 0..60 {
        if t.prompt().is_none() {
            return offered;
        }
        if t.counteract_offered(card) {
            offered = true;
            asked.extend(t.asked());
        } else if std::env::var("DBG_OFFER").is_ok() {
            eprintln!("not offered: {}", t.dump_prompt());
        }
        t.decline();
    }
    offered
}

/// P0 plays `TEST:aimer` (its single effect names the first other seat) at P1,
/// who answers with `opener` (a `SELF_REDIRECT` counter). Returns with the join
/// window open.
fn aimer_join(t: &mut Table, opener: &str) {
    t.give(0, &[AIMER]);
    t.give(1, &[opener]);
    t.play(0, AIMER).unwrap();
    let mut declared = false;
    for _ in 0..30 {
        if t.counteract_offered(opener) {
            t.counteract(1, opener).unwrap();
            declared = true;
            break;
        }
        t.decline();
    }
    assert!(declared, "opener not offered: {}", t.dump_prompt());
}

/// The `to` of the open counter prompt (the link's presented target).
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

/// Decline `who`'s ordinary offer of `card` and declare it in the join window
/// instead (the window presents `who` as the target, §6.4).
fn declare_in_window(t: &mut Table, who: usize, card: &str) {
    for _ in 0..40 {
        if t.prompt().is_none() {
            break;
        }
        if t.counteract_offered(card)
            && t.asked().contains(&who)
            && offer_target(t) == Some(who as i32)
        {
            t.counteract(who, card).unwrap();
            return;
        }
        t.decline();
    }
    panic!("{card} never offered to {who} in the window");
}

/// Declare `card` as `who` once it is offered (draining anything else).
fn declare_when_offered(t: &mut Table, who: usize, card: &str) {
    for _ in 0..30 {
        if t.counteract_offered(card) {
            t.counteract(who, card).unwrap();
            return;
        }
        t.decline();
    }
    panic!("{card} never offered: {}", t.dump_prompt());
}

fn event_int(t: &Table, mark: i32, key: &str, name: &str) -> Option<i64> {
    t.events_since(mark)
        .iter()
        .find(|e| log_key(e).ends_with(key) && int_arg(&e.msg, name).is_some())
        .and_then(|e| int_arg(&e.msg, name))
}

// =====================================================================
// Item 1 -- the draw is the opener's clause, for each shielding joiner
// =====================================================================

#[test]
fn join_draw_belongs_to_the_opener() {
    let mut t = fresh(&["花园多惠", "纯田真奈", "户山香澄"]);
    set_draw_n(&mut t, 2, 3);
    t.give(2, &[NET_ERROR]);
    aimer_join(&mut t, MANA);
    declare_in_window(&mut t, 2, NET_ERROR);
    // 真奈's body asks its owner, naming the joiner.
    let p = t.expect_prompt();
    assert_eq!(
        t.asked(),
        vec![1],
        "the ask is the opener's: {}",
        t.dump_prompt()
    );
    assert!(
        p.title.key().ends_with("net_error_draw_title"),
        "the optional-draw ask: {}",
        t.dump_prompt()
    );
    assert!(
        t.prompt_refers("mana_champion_draw"),
        "the ask names the joiner: {}",
        t.dump_prompt()
    );
    t.answer_one(0).unwrap(); // 可 -- yes
    drain(&mut t);
    assert_eq!(t.hand(2).len(), 1, "the joiner draws 1: {:?}", t.hand(2));
    assert_eq!(t.hand(1).len(), 0, "the opener draws 0: {:?}", t.hand(1));
}

#[test]
fn join_draw_is_optional_per_joiner() {
    let mut t = fresh(&["花园多惠", "纯田真奈", "户山香澄"]);
    set_draw_n(&mut t, 2, 3);
    t.give(2, &[NET_ERROR]);
    aimer_join(&mut t, MANA);
    declare_in_window(&mut t, 2, NET_ERROR);
    assert!(t.prompt_refers("mana_champion_draw"), "{}", t.dump_prompt());
    t.answer_one(1).unwrap(); // 可 -- no
    drain(&mut t);
    assert_eq!(t.hand(2).len(), 0, "the joiner declines: {:?}", t.hand(2));
    assert_eq!(
        t.hand(1).len(),
        0,
        "the opener declines too: {:?}",
        t.hand(1)
    );
    assert_eq!(t.money(0), 10_000, "no money moves");
    assert_eq!(t.money(1), 10_000, "no money moves");
    assert_eq!(t.money(2), 10_000, "no money moves");
}

// =====================================================================
// Item 2 -- the shield mark is readable on the opener's link
// =====================================================================

#[test]
fn join_shield_reads_back_on_the_opener_link() {
    // `TEST:joinOpen` reports `joiner_shielded(i)` for every declarer;
    // `TEST:joinShield` marks through the legacy `set_join_used` alias.
    let mut t = fresh(&["花园多惠", "纯田真奈", "户山香澄"]);
    t.give(2, &[JOIN_SHIELD]);
    aimer_join(&mut t, JOIN_OPEN);
    let mark = t.mark();
    declare_when_offered(&mut t, 2, JOIN_SHIELD);
    drain(&mut t);
    assert_eq!(
        event_int(&t, mark, "join_report", "who"),
        Some(2),
        "the declarer is reported: {:?}",
        t.keys_since(mark)
    );
    assert_eq!(
        event_int(&t, mark, "join_report", "shielded"),
        Some(1),
        "set_join_used marks the running joiner: {:?}",
        t.keys_since(mark)
    );
}

// =====================================================================
// Item 3 -- tsugumi's numeric-range offer
// =====================================================================

#[test]
fn tsugumi_not_offered_on_sunset() {
    // No RANGE_CARD: 「无法对无论是何种颜色的夕阳生效」.
    let mut t = fresh(&["花园多惠", "仓田真白"]);
    t.dice(&[1]);
    t.give(1, &[TSUGUMI]);
    t.give(0, &[SUNSET]);
    t.play(0, SUNSET).unwrap();
    let mut asked = Vec::new();
    assert!(
        !record_offers(&mut t, TSUGUMI, &mut asked),
        "tsugumi is not offered on the sunset: {}",
        t.dump_prompt()
    );
}

#[test]
fn tsugumi_offered_on_yolo_and_forces_the_extreme() {
    for (want_max, want) in [(true, 4i64), (false, 1i64)] {
        let mut t = fresh(&["花园多惠", "仓田真白"]);
        t.give(1, &[YOLO]);
        t.give(0, &[TSUGUMI]);
        t.begin_turn(1);
        drain(&mut t);
        t.dice(&[3]);
        t.roll(1).unwrap();
        answer_with_tsugumi(&mut t, YOLO, 1, 0);
        let mark = t.mark();
        let pick = if want_max {
            t.option("tsugumi_can_max").unwrap()
        } else {
            t.option("tsugumi_can_min").unwrap()
        };
        t.answer(0, pick).unwrap();
        drain(&mut t);
        let n = event_int(&t, mark, "yolo_boost", "n");
        assert_eq!(
            n,
            Some(want),
            "the answered 1d4 settles at {want}: {:?}",
            t.keys_since(mark)
        );
    }
}

#[test]
fn tsugumi_not_offered_twice() {
    // A declared copy arms the extreme before the next seat is offered.
    let mut t = fresh(&["花园多惠", "仓田真白", "户山香澄"]);
    t.give(1, &[TSUGUMI]);
    t.give(2, &[TSUGUMI]);
    t.give(0, &[YCM]);
    t.dice(&[1, 1, 1]);
    t.play(0, YCM).unwrap();
    let mut second_asks = 0;
    let mut first_declared = false;
    for _ in 0..40 {
        if t.prompt().is_none() {
            break;
        }
        if t.counteract_offered(TSUGUMI) {
            let asked = t.asked();
            if asked.contains(&1) && !first_declared {
                first_declared = true;
                t.counteract(1, TSUGUMI).unwrap();
            } else {
                if asked.contains(&2) {
                    second_asks += 1;
                }
                t.decline();
            }
        } else if t.option("choice_counter").is_some() {
            let k = t.option("choice_counter").unwrap();
            t.answer_one(k).unwrap();
        } else if t.option("tsugumi_can_max").is_some() {
            let k = t.option("tsugumi_can_max").unwrap();
            t.answer_one(k).unwrap();
        } else {
            t.decline();
        }
    }
    assert!(first_declared, "the first copy is offered");
    assert_eq!(second_asks, 0, "the second copy is not offered");
}

#[test]
fn tsugumi_offered_on_ycm() {
    // ycm declares RANGE_CARD -- the exception is a target.
    let mut t = fresh(&["花园多惠", "仓田真白"]);
    t.give(1, &[YCM]);
    t.give(0, &[TSUGUMI]);
    t.dice(&[9, 9, 8]);
    t.begin_turn(1);
    drain(&mut t);
    let mark = t.mark();
    t.play(1, YCM).unwrap();
    // ycm's own play is the range-card link; tsugumi answers it directly.
    declare_when_offered(&mut t, 0, TSUGUMI);
    let k = t.option("choice_counter").expect("the printed choice");
    t.answer(0, k).unwrap();
    let k = t.option("tsugumi_can_max").unwrap();
    t.answer(0, k).unwrap();
    drain(&mut t);
    assert!(
        t.keys_since(mark)
            .iter()
            .any(|k| k.ends_with("tsugumi_can_forced")),
        "the forced extreme is logged: {:?}",
        t.keys_since(mark)
    );
    assert!(!t
        .keys_since(mark)
        .iter()
        .any(|k| k.ends_with("tsugumi_can_ycm")));
}

/// Declare `answered` as `answered_who`, then tsugumi as `tsugumi_who`, and
/// pick the printed effect (1) on the tsugumi choice.
fn answer_with_tsugumi(t: &mut Table, answered: &str, answered_who: usize, tsugumi_who: usize) {
    declare_when_offered(t, answered_who, answered);
    declare_when_offered(t, tsugumi_who, TSUGUMI);
    let k = t.option("choice_counter").expect("the printed choice");
    t.answer(tsugumi_who, k).unwrap();
}

#[test]
fn tsugumi_answers_a_numeric_event() {
    // 「可以对…生效」 reaches an event's pure interval too, and the extreme is
    // scoped to the event play.
    let corner = tile("CiRCLE 咖啡厅");
    let mut t = fresh(&["花园多惠", "仓田真白"]);
    t.give(1, &[TSUGUMI]);
    t.set_event_deck(&["一切不会结束"]);
    t.set_pos(0, (corner + 59) % 60);
    // The move, then the event's own 1d4 face. The armed extreme settles the
    // event without spending that face, so it is also the next play's roll.
    t.dice(&[1, 3]);
    t.roll(0).unwrap();
    // The drawn event plays: its `Event` link is answered by tsugumi directly.
    declare_when_offered(&mut t, 1, TSUGUMI);
    let k = t.option("choice_counter").expect("the printed choice");
    t.answer(1, k).unwrap();
    let k = t.option("tsugumi_can_max").unwrap();
    t.answer(1, k).unwrap();
    drain(&mut t);
    // 1d4 forced to 4: everyone teleports to RiNG 4.
    assert_eq!(t.pos(0), tile("RiNG 4"), "the event rolled its maximum");
    // The roll was this turn's main move, so the next play needs a new turn.
    t.begin_turn(0);
    drain(&mut t);
    // The next play is plain: a loaded 3 picks branch 3 (a draw), not a
    // 6-branch multi-select.
    set_draw_n(&mut t, 0, 2);
    t.give(0, &[SUNSET]);
    let mark = t.mark();
    t.play(0, SUNSET).unwrap();
    assert!(
        !t.prompt()
            .is_some_and(|p| p.title.key().contains("any_color_sunset_pick")),
        "the next play rolls plain: {}",
        t.dump_prompt()
    );
    drain(&mut t);
    assert!(
        t.keys_since(mark)
            .iter()
            .any(|k| k.ends_with("any_color_sunset_opt3")),
        "branch 3 landed: {:?}",
        t.keys_since(mark)
    );
}

// =====================================================================
// Item 5 -- vocal accepts a whole transfer only
// =====================================================================

#[test]
fn vocal_not_offered_on_a_split_leg() {
    // A [分摊] leg is not 「一次性」 even when the leg is >= 5000.
    let mut t = fresh(&["花园多惠", "仓田真白", "户山香澄"]);
    t.give(1, &[VOCAL]);
    t.give(2, &[VOCAL]);
    t.give(0, &["TEST:split12k"]);
    t.play(0, "TEST:split12k").unwrap();
    let mut asked = Vec::new();
    assert!(
        !record_offers(&mut t, VOCAL, &mut asked),
        "no offer on a split leg: {}",
        t.dump_prompt()
    );
    drain(&mut t);
    assert_eq!(t.money(1), 10_000 - 6000, "P1 pays its 6000 leg");
    assert_eq!(t.money(2), 10_000 - 6000, "P2 pays its 6000 leg");
}

#[test]
fn vocal_offered_on_a_whole_transfer() {
    let mut t = fresh(&["花园多惠", "仓田真白"]);
    t.give(0, &[VOCAL]);
    let t29 = tile("弦卷豪宅");
    let rent = data().tiles[t29].rent[3];
    assert!(rent >= 5000, "R = {rent}");
    t.set_owner(t29, Some(1));
    t.set_houses(t29, 3);
    t.set_pos(0, t29 - 1);
    t.dice(&[1]);
    let mark = t.mark();
    t.roll(0).unwrap();
    declare_when_offered(&mut t, 0, VOCAL);
    drain(&mut t);
    assert_eq!(t.money(0), 10_000, "the whole 6920 is waived");
    assert_eq!(t.money(1), 10_000, "the payee receives nothing");
    assert!(
        t.keys_since(mark)
            .iter()
            .any(|k| k.ends_with("vocal_too_hard_note")),
        "the waiver is logged: {:?}",
        t.keys_since(mark)
    );
}

// =====================================================================
// Item 6/15 -- mortis dispatches the copied entry at every kind
// =====================================================================

/// P0 plays `source` twice: the first play enters the copy record, the second
/// is answered by P2's mortis, which copies the first. Returns the mark taken
/// at the second play. (`source` must be a playable card with a permissive
/// entry at the kind the second play raises.)
fn mortis_copies(t: &mut Table, source: &str) -> i32 {
    t.give(0, &[source, source]);
    t.give(2, &[MORTIS]);
    t.play(0, source).unwrap();
    drain(t);
    let mark = t.mark();
    t.play(0, source).unwrap();
    declare_when_offered(t, 2, MORTIS);
    drain(t);
    mark
}

#[test]
fn mortis_empty_links_match_every_kind() {
    // `card`: the copied source is a `Card` counter, answered on a later card
    // play. The second play runs the entry once as its own follow-up and once
    // as the copy.
    {
        let mut t = fresh(&["花园多惠", "仓田真白", "户山香澄"]);
        let mark = mortis_copies(&mut t, "TEST:cardCopy");
        assert_eq!(
            t.keys_since(mark)
                .iter()
                .filter(|k| k.ends_with("card_copy_body"))
                .count(),
            2,
            "own follow-up then the copied entry: {:?}",
            t.keys_since(mark)
        );
    }
    // `settle`: the source is a card with a `Settle` counter entry, played
    // earlier; P1 walks onto P0's tile and P2's mortis copies the source.
    {
        let mut t = fresh(&["花园多惠", "仓田真白", "户山香澄"]);
        let t5 = 5usize;
        t.set_owner(t5, Some(0));
        t.give(0, &["TEST:settleCopy"]);
        t.give(2, &[MORTIS]);
        t.play(0, "TEST:settleCopy").unwrap();
        drain(&mut t);
        t.begin_turn(1);
        drain(&mut t);
        t.set_pos(1, (t5 + 59) % 60);
        t.dice(&[1]);
        let mark = t.mark();
        t.roll(1).unwrap();
        declare_when_offered(&mut t, 2, MORTIS);
        drain(&mut t);
        assert!(
            t.keys_since(mark)
                .iter()
                .any(|k| k.ends_with("settle_copy_body")),
            "the copied Settle entry ran: {:?}",
            t.keys_since(mark)
        );
    }
    // `pay`: the source is a card with a `Pay` counter entry, played earlier;
    // the payment settles on P1's landing and P2's mortis copies the source.
    {
        let mut t = fresh(&["花园多惠", "仓田真白", "户山香澄"]);
        let t5 = 5usize;
        t.set_owner(t5, Some(0));
        t.give(0, &["TEST:payCopy"]);
        t.give(2, &[MORTIS]);
        t.play(0, "TEST:payCopy").unwrap();
        drain(&mut t);
        t.begin_turn(1);
        drain(&mut t);
        t.set_pos(1, (t5 + 59) % 60);
        t.dice(&[1]);
        let mark = t.mark();
        t.roll(1).unwrap();
        declare_when_offered(&mut t, 2, MORTIS);
        drain(&mut t);
        assert!(
            t.keys_since(mark)
                .iter()
                .any(|k| k.ends_with("pay_copy_body")),
            "the copied Pay entry ran: {:?}",
            t.keys_since(mark)
        );
    }
}

#[test]
fn mortis_plays_the_copied_entry() {
    // The copied body runs and `link_copied` names the source card.
    let mut t = fresh(&["花园多惠", "仓田真白", "户山香澄"]);
    let mark = mortis_copies(&mut t, "TEST:cardCopy");
    let copied = t.events_since(mark).iter().find_map(|e| {
        if !log_key(e).ends_with("card_copy_body") {
            return None;
        }
        match e.msg.args.get("copied") {
            Some(Arg::Card(c)) if !c.is_empty() => Some(c.clone()),
            _ => None,
        }
    });
    assert_eq!(
        copied.as_deref(),
        Some("TEST:cardCopy"),
        "link_copied names the source: {:?}",
        t.keys_since(mark)
    );
}

// =====================================================================
// Item 7 -- repaint cuts the declaration face
// =====================================================================

#[test]
fn repaint_cuts_a_roll() {
    let mut t = Table::vanilla(3);
    t.own(0, &[EDOGAWA_PARK]);
    t.set_pos(1, 5);
    t.set_money(0, 20_000);
    t.set_money(1, 20_000);
    t.give(0, &[REPAINT]);
    t.begin_turn(1);
    drain(&mut t);
    // Path 6..=8 contains the owned 7 -> X = 1; 3 - 1 = 2 lands on 7.
    t.dice(&[3]);
    t.roll(1).unwrap();
    assert!(t.counteract_offered(REPAINT), "{}", t.dump_prompt());
    t.counteract(0, REPAINT).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(1), EDOGAWA_PARK, "the cut landed the walk");
    // 0-house rent 140, 「支付减半」 -> 70.
    assert_eq!(t.money(1), 20_000 - 70, "the settlement is halved");
    assert_eq!(t.money(0), 20_070);
}

#[test]
fn repaint_answers_yolo_and_the_cut_composes() {
    let mut t = Table::vanilla(3);
    t.own(0, &[EDOGAWA_PARK]);
    t.set_pos(1, 5);
    t.give(0, &[REPAINT]);
    t.give(1, &[YOLO]);
    t.begin_turn(1);
    drain(&mut t);
    t.dice(&[3, 2]); // main roll 3, YOLO's 1d4 = 2
    t.roll(1).unwrap();
    declare_when_offered(&mut t, 1, YOLO);
    assert!(t.counteract_offered(REPAINT), "{}", t.dump_prompt());
    let mark = t.mark();
    t.counteract(0, REPAINT).unwrap();
    drain(&mut t);
    // X counts the original path (1); the cut is 3 - 1 = 2, then +2 = 4 -> 9.
    assert_eq!(event_int(&t, mark, "repaint_cut", "n"), Some(1));
    assert_eq!(t.pos(1), 9, "cut then boosted");
}

// =====================================================================
// Item 8 -- the turn's counteraction history
// =====================================================================

/// P0 plays `TEST:echo`; P1 and P2 each answer it with `TEST:trigger_seen`
/// (one counterer each; the second answers the first counter's new link).
fn two_counterers(t: &mut Table) {
    t.give(0, &["TEST:echo"]);
    t.give(1, &["TEST:trigger_seen"]);
    t.give(2, &["TEST:trigger_seen"]);
    t.play(0, "TEST:echo").unwrap();
    declare_when_offered(t, 1, "TEST:trigger_seen");
    declare_when_offered(t, 2, "TEST:trigger_seen");
    drain(t);
}

/// Answer cant_look_away's prompts, picking `seats` in order with 2 steps
/// forward, and declining anything else.
fn walk_all(t: &mut Table, seats: &[i32]) {
    let mut left = seats.to_vec();
    let mut last_mark = t.mark();
    for _ in 0..80 {
        if std::env::var("DBG_RUN").is_ok() {
            for e in t.events_since(last_mark) {
                if log_key(&e).contains("cant_look") {
                    eprintln!("EV {} {:?}", log_key(&e), e.msg);
                }
            }
            last_mark = t.mark();
        }
        let Some(p) = t.prompt() else { return };
        if std::env::var("DBG_OFFER").is_ok() {
            eprintln!(
                "WALK_PROMPT runs={} hist={:?} pos={} rec={:?} {}",
                t.state(1, "cantLookRuns"),
                t.m.world().countered_this_turn,
                t.pos(1),
                t.recent_keys(30),
                t.dump_prompt()
            );
        }
        if t.prompt_refers("cant_look_away_pick") {
            if std::env::var("DBG_RUN").is_ok() {
                eprintln!("PICK left={:?} {}", left, t.dump_prompt());
            }
            let who = left.remove(0);
            let k = p
                .options
                .iter()
                .position(|o| match o.args.get("who") {
                    Some(Arg::PlayerId(v)) => *v == who,
                    Some(Arg::Integer(v)) | Some(Arg::Number(v)) => *v == who as i64,
                    _ => false,
                })
                .unwrap_or_else(|| panic!("seat {who} not offered: {}", t.dump_prompt()));
            t.answer_one(k as i32).unwrap();
        } else if t.prompt_refers("cant_look_away_steps") {
            t.answer_one(1).unwrap(); // 2 steps
        } else if t.prompt_refers("cant_look_away_dir") {
            t.answer_one(0).unwrap(); // forward
        } else {
            t.decline();
        }
    }
    panic!("walk_all did not finish: {}", t.dump_prompt());
}

#[test]
fn cant_look_away_moves_every_counterer_this_turn() {
    let mut t = fresh(&["花园多惠", "仓田真白", "户山香澄"]);
    two_counterers(&mut t);
    assert_eq!(
        t.m.world().countered_this_turn.len(),
        2,
        "both declarations are on the turn's history"
    );
    // No money: the landings raise no buy prompt, so the body does not re-run.
    for s in 0..3 {
        t.set_money(s, 0);
    }
    t.give(0, &[CANT_LOOK]);
    t.play(0, CANT_LOOK).unwrap();
    // History order, then the caster's own seat (they are on their turn).
    walk_all(&mut t, &[1, 2, 0]);
    assert_eq!(t.pos(1), 2, "P1 walked");
    assert_eq!(t.pos(2), 2, "P2 walked");
}

#[test]
fn counter_history_is_empty_next_turn() {
    let mut t = fresh(&["花园多惠", "仓田真白", "户山香澄"]);
    two_counterers(&mut t);
    t.m.world_mut().st.skip_move = true;
    t.end(0).unwrap();
    drain(&mut t);
    assert!(
        t.m.world().countered_this_turn.is_empty(),
        "the next turn starts empty: {:?}",
        t.m.world().countered_this_turn
    );
    // It is P1's turn now: the play entry's only candidate is the caster. No
    // money keeps the landing free of a buy prompt (no body re-run).
    t.set_money(1, 0);
    t.give(1, &[CANT_LOOK]);
    let mark = t.mark();
    t.play(1, CANT_LOOK).unwrap();
    let p = t.expect_prompt();
    assert!(
        t.prompt_refers("cant_look_away_pick"),
        "{}",
        t.dump_prompt()
    );
    let seats: Vec<i32> = p
        .options
        .iter()
        .filter_map(|o| match o.args.get("who") {
            Some(Arg::PlayerId(v)) => Some(*v),
            Some(Arg::Integer(v)) | Some(Arg::Number(v)) => Some(*v as i32),
            _ => None,
        })
        .collect();
    assert_eq!(
        seats,
        vec![1],
        "only the caster is offered: {}",
        t.dump_prompt()
    );
    walk_all(&mut t, &[1]);
    assert_eq!(t.pos(0), 0, "P0 does not move");
    assert_eq!(t.pos(2), 0, "P2 does not move");
    assert!(t
        .keys_since(mark)
        .iter()
        .any(|k| k.ends_with("cant_look_away_ordered")));
}

#[test]
fn counter_history_survives_save() {
    let mut t = fresh(&["花园多惠", "仓田真白", "户山香澄"]);
    two_counterers(&mut t);
    let saved = t.m.save();
    let t2 = game_core::engine::Match::restore(data(), rules(), &saved).expect("restore");
    let recs = &t2.world().countered_this_turn;
    assert_eq!(recs.len(), 2, "the list survives save/load: {recs:?}");
    assert_eq!(recs[0].declarer, 1);
    assert_eq!(recs[1].declarer, 2);
    // Both answer the answered play's own link.
    assert!(recs.iter().all(|r| r.countered == 0));
    assert!(recs.iter().all(|r| r.card == "TEST:trigger_seen"));
}

// =====================================================================
// Item 9 -- the join is a window over seats, on a single-target effect
// =====================================================================

#[test]
fn join_window_offers_a_single_target_effect() {
    // The effect names one seat (P1); the bystanders P2 and P3 are still
    // offered as if they were the target.
    let mut t = fresh(&["花园多惠", "纯田真奈", "户山香澄", "山吹沙绫"]);
    t.give(2, &[JOIN_SHIELD]);
    t.give(3, &[JOIN_SHIELD]);
    aimer_join(&mut t, JOIN_OPEN);
    let mut asked = Vec::new();
    let offered = record_offers(&mut t, JOIN_SHIELD, &mut asked);
    assert!(offered, "bystanders are offered: {}", t.dump_prompt());
    assert!(asked.contains(&2), "P2 was offered: {asked:?}");
    assert!(asked.contains(&3), "P3 was offered: {asked:?}");
}

#[test]
fn join_offers_no_seat_twice() {
    let mut t = fresh(&["花园多惠", "纯田真奈", "户山香澄", "山吹沙绫"]);
    t.give(2, &[JOIN_SHIELD]);
    t.give(3, &[JOIN_SHIELD]);
    aimer_join(&mut t, JOIN_OPEN);
    // Declare for each seat as it is asked and count the asks.
    let mut counts = [0usize; 4];
    for _ in 0..40 {
        let Some(_) = t.prompt() else { break };
        if t.counteract_offered(JOIN_SHIELD) {
            let asked = t.asked();
            for &s in &asked {
                counts[s] += 1;
            }
            t.counteract(asked[0], JOIN_SHIELD).unwrap();
        } else {
            t.decline();
        }
    }
    assert_eq!(counts[1], 0, "the opener is not in the join: {counts:?}");
    assert_eq!(counts[2], 1, "P2 is offered once: {counts:?}");
    assert_eq!(counts[3], 1, "P3 is offered once: {counts:?}");
}

#[test]
fn join_restores_the_effect_target() {
    // A joiner that cancels nothing (probe): after the round the effect still
    // settles on its original recipient.
    let mut t = fresh(&["花园多惠", "纯田真奈", "户山香澄"]);
    t.give(2, &["TEST:probe"]);
    aimer_join(&mut t, JOIN_OPEN);
    let mark = t.mark();
    declare_when_offered(&mut t, 2, "TEST:probe");
    drain(&mut t);
    assert_eq!(
        event_int(&t, mark, "aimer_done", "got"),
        Some(1),
        "the effect still aims at P1: {:?}",
        t.keys_since(mark)
    );
    assert_eq!(event_int(&t, mark, "aimer_done", "count"), Some(1));
}

#[test]
fn fallback_draw_bars_on_a_missed_joiner() {
    // A declaration that shields nothing still bars the fallback draw and is
    // not asked for the per-shield draw.
    let mut t = fresh(&["花园多惠", "纯田真奈", "户山香澄"]);
    set_draw_n(&mut t, 1, 3);
    t.give(2, &["TEST:probe"]);
    aimer_join(&mut t, MANA);
    declare_when_offered(&mut t, 2, "TEST:probe");
    drain(&mut t);
    assert_eq!(t.hand(1).len(), 0, "no fallback draw: {:?}", t.hand(1));
    assert_eq!(t.hand(2).len(), 0, "the quiet joiner draws nothing");
}

// =====================================================================
// Item 11 -- a declared negation hides now_sumimi
// =====================================================================

#[test]
fn now_sumimi_not_offered_after_a_declared_cancel() {
    // 登上武道馆 designates P0 as a payer (a `Pay` ride on a designating play);
    // net_error negates that link's declaration before P0 is visited.
    let mut t = fresh(&["花园多惠", "仓田真白"]);
    t.give(0, &["Sumimi:现在她是Sumimi的小初啦"]);
    // P0's own tile tag: a 2000 payment over 水族馆's 320 is a 「超过」.
    t.set_pos(0, tile("水族馆"));
    // net_error answers the payment's effect link as its own target (the
    // payer), so it must sit with the payer.
    t.give(0, &[NET_ERROR]);
    t.give(1, &["通用:登上武道馆"]);
    t.begin_turn(1);
    drain(&mut t);
    t.play(1, "通用:登上武道馆").unwrap();
    // P0 (the payer) negates the declaration first.
    declare_when_offered(&mut t, 0, NET_ERROR);
    // P0 (the designated payer) is visited next; the declared negation keeps
    // now_sumimi home.
    let mut asked = Vec::new();
    assert!(
        !record_offers(&mut t, "Sumimi:现在她是Sumimi的小初啦", &mut asked),
        "no offer over a declared negation: {}",
        t.dump_prompt()
    );
    drain(&mut t);
    assert_eq!(t.hand(0).len(), 1, "the card stays: {:?}", t.hand(0));
    assert_eq!(t.money(0), 10_000, "the negated payment does not settle");
}

// =====================================================================
// Item 12 -- rain's designations and the join designation record
// =====================================================================

#[test]
fn rain_designations_reach_net_error() {
    // Clause 1, not 2: the designations are visible, so only P1's own
    // designation goes and the play keeps running.
    let mut t = fresh(&["花园多惠", "仓田真白"]);
    t.give(1, &[NET_ERROR]);
    t.give(0, &["通用:雨啊，快点来吧"]);
    t.dice(&[1, 1]); // x = 2: P0 + one other
    let mark = t.mark();
    t.play(0, "通用:雨啊，快点来吧").unwrap();
    declare_when_offered(&mut t, 1, NET_ERROR);
    drain(&mut t);
    let keys = t.keys_since(mark);
    assert!(
        keys.iter().any(|k| k.ends_with("net_error_target")),
        "clause 1 cancels one designation: {keys:?}"
    );
    assert!(
        !keys.iter().any(|k| k.ends_with("net_error_card")),
        "the play is not cancelled outright: {keys:?}"
    );
    assert_eq!(t.state(1, "stay"), 0, "P1's designation was dropped");
    assert_eq!(t.state(0, "stay"), 1, "the play still lands on P0");
}

#[test]
fn join_designation_is_readable() {
    // The joiner's cancel is recorded per declaration and readable by the
    // opener, naming the redirected seat (「改为你」 -> the opener).
    let mut t = fresh(&["花园多惠", "纯田真奈", "户山香澄"]);
    t.give(2, &[JOIN_SHIELD]);
    aimer_join(&mut t, JOIN_OPEN);
    let mark = t.mark();
    declare_when_offered(&mut t, 2, JOIN_SHIELD);
    drain(&mut t);
    assert_eq!(
        event_int(&t, mark, "join_report", "designation"),
        Some(1),
        "the report names the cancelled opener: {:?}",
        t.keys_since(mark)
    );
    assert_eq!(event_int(&t, mark, "join_shield_done", "who"), Some(1));
}

// =====================================================================
// Item 13/14 -- the refusal precedes the window
// =====================================================================

#[test]
fn ordinary_not_offered_to_an_immune_player() {
    let mut t = fresh(&["花园多惠", "仓田真白"]);
    // The holder must be the affected seat (`target == owner`).
    t.give(1, &["MyGO:普通与理所当然"]);
    t.place_raw(1, "TEST:shield"); // P1 is ImmuneAll
    t.give(0, &["TEST:stunner"]);
    t.set_state(1, "lastWalk", 5);
    let mark = t.mark();
    t.play(0, "TEST:stunner").unwrap();
    let mut asked = Vec::new();
    assert!(
        !record_offers(&mut t, "MyGO:普通与理所当然", &mut asked),
        "an immune target opens no window: {}",
        t.dump_prompt()
    );
    drain(&mut t);
    assert_eq!(t.state(1, "stun"), 0, "the abnormal was refused");
    assert!(
        t.keys_since(mark)
            .iter()
            .any(|k| k.contains("abnormalRefused")),
        "abnormalRefused is logged: {:?}",
        t.events_since(mark)
    );
}

#[test]
fn own_stage_answers_only_landed_abnormals() {
    let mut t = fresh(&["花园多惠", "仓田真白"]);
    t.give(1, &["R:选择自己的舞台"]);
    t.place_raw(1, "TEST:shield");
    t.give(0, &["TEST:stunner"]);
    let mark = t.mark();
    t.play(0, "TEST:stunner").unwrap();
    let mut asked = Vec::new();
    assert!(
        !record_offers(&mut t, "R:选择自己的舞台", &mut asked),
        "a refused abnormal opens no window: {}",
        t.dump_prompt()
    );
    drain(&mut t);
    assert!(
        t.hand(1).iter().any(|c| c == "R:选择自己的舞台"),
        "the refused card is not spent: {:?}",
        t.hand(1)
    );
    assert!(
        t.keys_since(mark)
            .iter()
            .any(|k| k.contains("abnormalRefused")),
        "abnormalRefused is logged: {:?}",
        t.events_since(mark)
    );
}
