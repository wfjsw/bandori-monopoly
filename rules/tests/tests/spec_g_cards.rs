//! G-CARDS scenario tests -- card zones, the hand limit, draw / discard /
//! reshuffle, play gates, the opening deal, and the event pile.
//!
//! Unit 2 owns this file and `rules/spec/src/global/cards.rs`. Clauses that
//! are per-act live in `CLAUSES` (armed on every soak step); clauses that only
//! apply inside `opening()` (or to deck construction) are pinned here by
//! scenario.
//!
//! Book source: `docs/rulebook/rulebook-doc.md` (see the area doc
//! `docs/rulebook/spec/global/cards.md` for each clause's quote).

use std::collections::BTreeMap;

use game_core::data::TileKind;
use game_core::engine::{Match, World};
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::MatchMode;
use rules_spec::global::cards::{
    g_cards_01, g_cards_06, g_cards_11, g_cards_12, pile_multiset, CLAUSES,
};
use rules_spec::table::settle_match;
use rules_spec::{data, observe, rules, forall, Clause, StepAct, Table};

// ---------------------------------------------------------------------------
// local helpers (the Table harness auto-answers the mulligan; these do not)
// ---------------------------------------------------------------------------

/// Multiset of the three private piles of `w` player `i`.
fn piles(w: &World, i: usize) -> BTreeMap<String, usize> {
    pile_multiset(w, i)
}

fn piles_str(m: &BTreeMap<String, usize>) -> String {
    rules_spec::global::cards::multiset_str(m)
}

/// A match at the opening mulligan prompt, with `top_first` seeded on P0's
/// draw pile below the opening deal. Player `k` is member `k + 1`.
///
/// The Table constructor answers the mulligan with its keep-default before a
/// test can see it; this helper stops at the prompt so the test can choose
/// keep or redo. Not a `Table` (its fields are private) -- drive `Match` +
/// [`observe`] directly.
fn open_at_mulligan(chars: &[&str], top_first: &[&str]) -> Match {
    let n = chars.len();
    let members: Vec<RoomMember> = (1..=n as i32)
        .map(|id| RoomMember {
            id,
            player: format!("P{}", id - 1),
            bot: false,
            ..Default::default()
        })
        .collect();
    let mut m = Match::new(
        data(),
        rules(),
        &members,
        20261006,
        MatchMode::Solo,
        ScoreWeights::default(),
    );
    {
        let w = m.world_mut();
        w.st.players.sort_by_key(|p| p.member);
    }
    for (k, c) in chars.iter().enumerate() {
        if !c.is_empty() {
            m.set_persona(k as i32 + 1, c);
        }
    }
    {
        let w = m.world_mut();
        for h in w.hidden.iter_mut() {
            h.hand.clear();
            h.draw.clear();
            h.discard.clear();
        }
        // `submit_deck` (run by `quick_start` when `!deck_ready`) replaces the
        // draw pile wholesale. Mark the decks ready so the seed below survives
        // into `opening()`'s deal.
        for p in w.st.players.iter_mut() {
            p.deck_ready = true;
        }
        // A legal-size pile: the named top_first on top, then filler names
        // so the deal of 2 and the mulligan redraw of 2 have cards to take.
        let filler = [
            "通用:GREAT!",
            "通用:PERFECT!",
            "通用:FEVER!",
            "通用:THANKS PARTY!",
            "通用:安可",
            "通用:登上武道馆",
            "通用:雨啊，快点来吧",
            "通用:尽力后的收获",
            "通用:CiRCLE THANKS PARTY!",
            "通用:网络链接异常",
        ];
        let mut pile: Vec<String> = top_first.iter().map(|s| s.to_string()).collect();
        for f in filler {
            if pile.len() >= 10 {
                break;
            }
            if !pile.iter().any(|c| c == f) {
                pile.push(f.to_string());
            }
        }
        // `hidden.draw` top is the **end** of the vec.
        w.hidden[0].draw = pile.into_iter().rev().collect();
    }
    m.quick_start();
    m
}

/// Answer the mulligan prompt as `who` (`keep` = wash nothing, `redo` = wash
/// and redraw). Returns the observed step's clause result.
fn answer_mulligan(m: &mut Match, who: usize, redo: bool) -> Result<(), String> {
    let p = m.state().prompt;
    assert_eq!(p.kind, "mulligan", "expected the mulligan prompt: {:?}", p.title);
    let act = StepAct::Answer {
        who: who as i32 + 1,
        prompt: p.id,
        value: if redo { 1 } else { 0 },
        cards: vec![],
    };
    let obs = observe(m, act, settle_match, CLAUSES);
    obs.act_result
}

/// Answer the mulligan for **every** waiting human (the offer is one prompt
/// to all of them), then settle.
fn answer_mulligan_all(m: &mut Match, redo_who: Option<usize>) {
    let p = m.state().prompt;
    assert_eq!(p.kind, "mulligan", "expected the mulligan prompt");
    let waiting: Vec<usize> = p
        .players
        .iter()
        .zip(&p.answers)
        .filter(|(_, &a)| a < 0)
        .map(|(&s, _)| s as usize)
        .collect();
    assert!(!waiting.is_empty(), "nobody waiting on the mulligan");
    for who in waiting {
        let redo = redo_who == Some(who);
        answer_mulligan(m, who, redo).expect("mulligan answer");
    }
    settle_match(m);
}

/// Card id carrying `tag` (and not `not_tag`), first in data order.
fn card_with_tag(tag: &str, not_tag: Option<&str>) -> String {
    data()
        .cards
        .iter()
        .find(|c| {
            c.tags.iter().any(|t| t == tag)
                && not_tag.map(|n| !c.tags.iter().any(|t| t == n)).unwrap_or(true)
        })
        .unwrap_or_else(|| panic!("no card tagged {tag:?}"))
        .id
        .clone()
}

/// A [手] card whose body only moves money / designates -- no draw, no
/// [移除], no self-placement. Safe for the pile-conservation scenario.
fn simple_hand_card() -> String {
    // 「登上武道馆」: [指定] others, they pay X. Pure transfer; the card is a
    // textbook non-[持续] use -> discard.
    "通用:登上武道馆".to_string()
}

/// Is tile `name` a [可购买格子]? `TileData::is_buyable` is Property | Ring.
fn buyable(name: &str) -> usize {
    data()
        .tiles
        .iter()
        .position(|t| t.name == name && t.is_buyable())
        .unwrap_or_else(|| panic!("no buyable tile named {name:?}"))
}


/// A skill-free table at P0's 运营: named characters, skills stripped, board
/// cleaned. Avoids the first-N-character presets (PPP:Returns and friends
/// rewrite the opening and the CiRCLE settle).
fn quiet_table() -> Table {
    let mut t = Table::new(&["美竹兰", "青叶摩卡"]);
    t.strip_skills();
    t.clean();
    t.begin_turn(0);
    t
}

/// One-step main move onto `tile`, draining any prompt. Returns the event
/// keys of the transition.
fn settle_on(t: &mut Table, tile: &str) -> Vec<String> {
    let target = rules_spec::tile(tile);
    t.set_pos(0, (target + 59) % 60);
    t.dice(&[1]);
    let mark = t.mark();
    t.roll(0).expect("roll");
    for _ in 0..8 {
        t.settle();
        if t.prompt().is_none() {
            break;
        }
        t.decline();
    }
    t.keys_since(mark)
}

fn not_buyable_tile() -> usize {
    data()
        .tiles
        .iter()
        .position(|t| t.kind == TileKind("circle".into()))
        .expect("CiRCLE")
}

// ---------------------------------------------------------------------------
// G-CARDS-08 / G-CARDS-07 -- starting deck: 10 distinct, no [衍生]
// ---------------------------------------------------------------------------

/// 规则书 112: 「挑选10张（不能重复）组成本局的初始卡组；卡名带有[衍生]的卡
/// 不可加入初始卡组」.
///
/// The deck-construction gate is `game_core::deck` (the `act("deck")` path
/// calls `is_complete` -> `clean`). Pin both halves: derived is rejected, and
/// a legal deck is exactly 10 distinct ids.
#[test]
fn g_cards_08_starting_deck_is_ten_distinct() {
    let d = data();
    let c = d.persona("美竹兰").expect("美竹兰");
    let pool: Vec<String> = game_core::deck::pool(d.as_ref(), c)
        .into_iter()
        .map(|k| k.id.clone())
        .collect();
    assert!(pool.len() >= 10, "pool has room for 10: {}", pool.len());
    let ten: Vec<String> = pool.iter().take(10).cloned().collect();
    assert_eq!(ten.len(), 10);
    let mut uniq = ten.clone();
    uniq.sort();
    uniq.dedup();
    assert_eq!(uniq.len(), 10, "10 distinct, got {ten:?}");
    assert!(
        game_core::deck::is_complete(d.as_ref(), c, &ten),
        "a 10-distinct pool slice is a complete deck"
    );
}

/// 规则书 75 + 112: 「[衍生]：不能加入初始卡组」.
#[test]
fn g_cards_07_derived_cards_never_in_starting_deck() {
    let d = data();
    let c = d.persona("美竹兰").expect("美竹兰");
    let derived: Vec<&game_core::data::CardData> = d.cards.iter().filter(|k| k.derived).collect();
    assert!(!derived.is_empty(), "the pool has [衍生] cards to reject");
    for k in &derived {
        let why = game_core::deck::cant_play(d.as_ref(), c, k);
        assert!(
            why.is_some(),
            "[衍生] {} must not enter a starting deck (book: 「不能加入初始卡组」)",
            k.id
        );
        // And `clean` must drop it even if a submitter lists it.
        let mut ids: Vec<String> = game_core::deck::pool(d.as_ref(), c)
            .into_iter()
            .map(|x| x.id.clone())
            .take(9)
            .collect();
        ids.push(k.id.clone());
        let cleaned = game_core::deck::clean(d.as_ref(), c, &ids);
        assert!(
            !cleaned.iter().any(|x| x == &k.id),
            "clean kept derived card {}: {cleaned:?}",
            k.id
        );
    }
}

/// The live opening piles: 10 cards per seat, distinct names, no [衍生].
/// (Observational twin of G-CARDS-07/08 -- skills are stripped first so a
/// game-start hook cannot add copies.)
#[test]
fn g_cards_08_opening_piles_are_ten_distinct_non_derived() {
    let mut t = Table::new(&["美竹兰", "青叶摩卡"]);
    t.strip_skills();
    for who in 0..2 {
        let mut all = t.hand(who);
        all.extend(t.draw_pile(who));
        all.extend(t.discard(who));
        assert_eq!(all.len(), 10, "P{who} opens with a 10-card deck: {all:?}");
        let mut uniq = all.clone();
        uniq.sort();
        uniq.dedup();
        assert_eq!(uniq.len(), 10, "P{who} deck names are distinct: {all:?}");
        for id in &all {
            let derived = data().card(id).map(|c| c.derived).unwrap_or(true);
            assert!(!derived, "P{who} deck holds [衍生] card {id}: {all:?}");
        }
    }
}

// ---------------------------------------------------------------------------
// G-CARDS-09 / G-CARDS-10 -- opening draw of 2, exactly one optional mulligan
// ---------------------------------------------------------------------------

/// 规则书 125: 「每名玩家的初始格子为1号CiRCLE，拥有10000初始资金并抽取2张手卡」.
#[test]
fn g_cards_09_opening_draw_is_two() {
    let t = Table::new(&["美竹兰", "青叶摩卡"]);
    for who in 0..2 {
        assert_eq!(
            t.hand(who).len(),
            2,
            "P{who} opens with 2 cards: {:?}",
            t.hand(who)
        );
    }
}

/// 规则书 125: 「仅一次，可将抽取的手卡全部洗回抽卡区并重抽2张」.
/// Keep path: the hand is unchanged and no second offer comes.
#[test]
fn g_cards_10_mulligan_keep_is_quiet_and_once() {
    let mut m = open_at_mulligan(&["美竹兰", "青叶摩卡"], &[]);
    {
        let p = m.state().prompt;
        assert_eq!(p.kind, "mulligan", "the opening offers a mulligan");
    }
    let before: Vec<String> = m.world().hidden[0].hand.clone();
    assert_eq!(before.len(), 2);
    answer_mulligan_all(&mut m, None);
    assert_eq!(
        m.world().hidden[0].hand, before,
        "keep leaves the opening hand alone"
    );
    assert_eq!(
        m.state().prompt.id, 0,
        "no second mulligan after the keep"
    );
    // A later turn must not re-offer.
    m.world_mut().next_turn_pending = true;
    settle_match(&mut m);
    assert_eq!(m.state().prompt.id, 0, "still no mulligan at turn 1");
}

/// 规则书 125: the redo path washes the opening hand back and redraws 2.
/// Exactly one optional mulligan -- after redo the offer is gone.
#[test]
fn g_cards_10_mulligan_redraws_two_exactly_once() {
    // Seed P0's draw pile under the deal so the redo's shuffle is observable
    // as a multiset (order is random; names are not).
    let top = ["通用:GREAT!", "通用:PERFECT!", "通用:FEVER!", "通用:THANKS PARTY!"];
    let mut m = open_at_mulligan(&["美竹兰", "青叶摩卡"], &top);
    let hand_before: Vec<String> = m.world().hidden[0].hand.clone();
    let pile_before: Vec<String> = m.world().hidden[0].draw.clone();
    let before = piles(m.world(), 0);
    assert_eq!(
        before.values().sum::<usize>(),
        2 + pile_before.len(),
        "deal of 2 plus the seeded pile"
    );
    answer_mulligan_all(&mut m, Some(0));
    let after = piles(m.world(), 0);
    assert_eq!(
        before, after,
        "mulligan is a wash-and-redraw: multiset conserved \
         (pre [{}] vs post [{}])",
        piles_str(&before),
        piles_str(&after)
    );
    assert_eq!(
        m.world().hidden[0].hand.len(),
        2,
        "redraw is 2 cards: {:?}",
        m.world().hidden[0].hand
    );
    // The old hand went back -- every name that was in hand is still in the
    // piles (identity-free: only the multiset is held).
    for id in &hand_before {
        assert!(
            after.contains_key(id),
            "mulligan dropped {id} from the piles"
        );
    }
    assert_eq!(m.state().prompt.id, 0, "the mulligan is once-only");
}

// ---------------------------------------------------------------------------
// G-CARDS-01 -- hand limit (the finding is the human over-draw)
// ---------------------------------------------------------------------------

/// 规则书 126: 「基础手卡上限为5张…立刻将手卡弃至上限张」.
/// A bot seat auto-discards down inside the same transition.
#[test]
fn g_cards_01_bot_discards_down_to_the_limit() {
    use game_core::state::BotMentality;
    let mut t = quiet_table();
    t.make_bot(0, BotMentality::Standard);
    t.set_hand(
        0,
        &[
            "通用:GREAT!",
            "通用:PERFECT!",
            "通用:FEVER!",
            "通用:安可",
            "通用:登上武道馆",
        ],
    );
    assert_eq!(t.hand(0).len(), 5);
    t.set_draw(0, &["通用:THANKS PARTY!"]);
    t.observe(CLAUSES);
    // 江户川乐器店 settle = draw 1 -> 6, then the bot discards down to 5.
    let keys = settle_on(&mut t, "江户川乐器店");
    assert!(
        keys.iter().any(|k| k == "log.draw"),
        "the overflow draw must happen: {keys:?}"
    );
    // At rest the bot must sit at the limit (book: 「立刻…弃至上限张」).
    assert_eq!(
        t.hand(0).len(),
        5,
        "bot discarded down to the limit: {:?}",
        t.hand(0)
    );
}

/// 规则书 126, the known finding: a **human** seat is left over the limit
/// after a draw (`draw_cards` only auto-discards for `p.ai`). A discrepancy
/// is a **plain failing test** -- no catch, no allowlist. The clause is armed
/// so the panic names G-CARDS-01; the book assertion is the deliverable.
#[test]
fn g_cards_01_human_over_draw_must_discard_down() {
    let mut t = quiet_table();
    // Human seat (quiet_table leaves ai off).
    assert!(!t.p(0).ai, "P0 is a human seat");
    t.set_hand(
        0,
        &[
            "通用:GREAT!",
            "通用:PERFECT!",
            "通用:FEVER!",
            "通用:安可",
            "通用:登上武道馆",
        ],
    );
    t.set_draw(0, &["通用:THANKS PARTY!"]);
    assert_eq!(t.hand(0).len(), 5, "at the limit before the draw");
    t.observe(&[Clause::new(
        "G-CARDS-01",
        "rulebook-doc.md:126",
        "基础手卡上限为5张，任何时刻若持有手卡超过上限则需要立刻将手卡弃至上限张。",
        g_cards_01,
    )]);
    // 江户川乐器店 settle = draw 1 -> 6, over the limit of 5. `observe` panics
    // on the clause violation -- that panic is the plain failing test. If the
    // engine somehow does not panic, the book assertion below still fails.
    let keys = settle_on(&mut t, "江户川乐器店");
    assert!(
        keys.iter().any(|k| k == "log.draw"),
        "the overflow draw must happen: {keys:?} hand={:?}",
        t.hand(0)
    );
    // Book: 「任何时刻若持有手卡超过上限则需要立刻将手卡弃至上限张」.
    let limit = t.p(0).hand_limit().unwrap_or(5) as usize;
    assert!(
        t.hand(0).len() <= limit,
        "G-CARDS-01: human seat holds {} cards over handLimit {limit} after a draw; \
         book says 「立刻将手卡弃至上限张」 (discard down immediately). hand={:?}",
        t.hand(0).len(),
        t.hand(0)
    );
}

// ---------------------------------------------------------------------------
// G-CARDS-02 / G-CARDS-03 / G-CARDS-04 -- pile moves
// ---------------------------------------------------------------------------

/// 规则书 126: 「使用后的非[持续]手卡进入弃卡区」 + pile conservation.
/// A simple [手] play: the card leaves hand for discard, the multiset holds.
#[test]
fn g_cards_03_non_continuous_play_goes_to_discard() {
    forall(6, 0xC4A12D5, |rng, _i| {
        let mut t = Table::vanilla(2);
        let card = simple_hand_card();
        let extra = 1 + rng.range(0, 3) as usize;
        let filler: Vec<String> = data()
            .cards
            .iter()
            .filter(|c| c.id != card && !c.derived)
            .take(extra)
            .map(|c| c.id.clone())
            .collect();
        let mut hand: Vec<String> = vec![card.clone()];
        hand.extend(filler.iter().cloned());
        let hand_refs: Vec<&str> = hand.iter().map(String::as_str).collect();
        t.set_hand(0, &hand_refs);
        t.set_draw(0, &["通用:GREAT!", "通用:PERFECT!"]);
        t.set_discard(0, &["通用:FEVER!"]);
        let pre = piles(t.world(), 0);
        t.observe(CLAUSES);
        t.play(0, &card).map_err(|e| format!("play rejected: {e}"))?;
        let post = piles(t.world(), 0);
        // Non-[持续], no [移除] / return-to-hand: the triple is conserved and
        // the card sits in the discard.
        if pre != post {
            return Err(format!(
                "pile multiset moved on a simple [手] play: pre [{}] vs post [{}]",
                piles_str(&pre),
                piles_str(&post)
            ));
        }
        if !t.discard(0).iter().any(|c| c == &card) {
            return Err(format!(
                "non-[持续] {card} not in discard after use: discard={:?}",
                t.discard(0)
            ));
        }
        if t.hand(0).iter().any(|c| c == &card) {
            return Err(format!("{card} still in hand: {:?}", t.hand(0)));
        }
        Ok(())
    })
    .expect("non-[持续] play -> discard");
}

/// 规则书 80 (contrapositive of 126): a [持续] card's [手] places it on the
/// 场地, not into the discard.
#[test]
fn g_cards_03_continuous_play_stays_on_the_field() {
    let mut t = Table::vanilla(2);
    let card = card_with_tag("持续", Some("衍生"));
    // Must also be a [手] card that places itself.
    let text = data().card(&card).unwrap().text.clone();
    assert!(text.contains("将此卡放置"), "{card} places itself: {text}");
    t.give(0, &[&card]);
    let pre = piles(t.world(), 0);
    t.observe(CLAUSES);
    t.play(0, &card).expect("play continuous");
    assert!(
        t.on_field(0, &card),
        "[持续] {card} belongs on the 场地 after its [手], field={:?}",
        t.field_ids(0)
    );
    assert!(
        !t.discard(0).iter().any(|c| c == &card),
        "[持续] {card} must not enter the discard on use: {:?}",
        t.discard(0)
    );
    // The triple loses exactly the played card (it moved to the field).
    let post = piles(t.world(), 0);
    let mut expect = pre.clone();
    let e = expect.get_mut(&card).expect("played card was in the piles");
    *e -= 1;
    if *e == 0 {
        expect.remove(&card);
    }
    assert_eq!(
        expect, post,
        "hand+draw+discard loses exactly the [持续] card: \
         pre [{}] expect [{}] post [{}]",
        piles_str(&pre),
        piles_str(&expect),
        piles_str(&post)
    );
}

/// 规则书 126: 「当抽卡区抽光时将弃卡区洗卡并放回抽卡区」 -- and only then.
/// With cards still on the draw pile a discard must **not** reshuffle.
#[test]
fn g_cards_04_no_reshuffle_while_draw_pile_holds_cards() {
    // A non-[持续] play files the card to the discard and runs the
    // empty-pile maintenance. With cards still on the draw pile that must
    // **not** reshuffle. (`act("discard")` is only legal when over the hand
    // limit, so the play is the mechanical discard.)
    let mut t = Table::vanilla(2);
    t.set_hand(0, &["通用:登上武道馆"]);
    t.set_draw(0, &["通用:PERFECT!", "通用:FEVER!"]);
    t.set_discard(0, &["通用:安可"]);
    let mark = t.mark();
    t.observe(CLAUSES);
    t.play(0, "通用:登上武道馆").expect("play");
    let keys = t.keys_since(mark);
    assert!(
        !keys.iter().any(|k| k == "log.reshuffle"),
        "filed a card with a live draw pile and reshuffled: {keys:?}"
    );
    assert!(
        t.discard(0).iter().any(|c| c == "通用:登上武道馆"),
        "the discard grew: {:?}",
        t.discard(0)
    );
    assert_eq!(t.draw_pile(0).len(), 2, "the draw pile is untouched");
}

/// 规则书 126: an empty draw pile does reshuffle the discard (on the next
/// draw / the empty-pile maintenance).
#[test]
fn g_cards_04_empty_draw_reshuffles_the_discard() {
    let mut t = quiet_table();
    t.set_hand(0, &[]);
    t.set_draw(0, &[]);
    t.set_discard(0, &["通用:GREAT!", "通用:PERFECT!", "通用:FEVER!"]);
    t.observe(CLAUSES);
    let keys = settle_on(&mut t, "江户川乐器店");
    assert!(
        keys.iter().any(|k| k == "log.draw"),
        "the settle must draw: {keys:?} pos={}",
        t.pos(0)
    );
    assert!(
        keys.iter().any(|k| k == "log.reshuffle"),
        "empty draw pile reshuffled the discard: {keys:?}"
    );
    assert_eq!(
        t.hand(0).len(),
        1,
        "drew the one card the settle owes: {:?}",
        t.hand(0)
    );
    assert!(
        t.discard(0).is_empty(),
        "the discard was consumed: {:?}",
        t.discard(0)
    );
    assert_eq!(t.draw_pile(0).len(), 2, "the rest sits on the new pile");
}

/// Assert the event tail's ids are unique and strictly increasing (one
/// `next_event` counter allocated every id).
fn assert_event_ids_unique_increasing(t: &Table) {
    let ids: Vec<i32> = t.m.world().recent.iter().map(|e| e.id).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    let mut dedup = sorted.clone();
    dedup.dedup();
    assert_eq!(
        sorted.len(),
        dedup.len(),
        "event ids must be unique: {ids:?}"
    );
    assert!(
        ids.windows(2).all(|w| w[0] < w[1]),
        "event ids must be strictly increasing: {ids:?}"
    );
}

/// Engine-level regression: a body that triggers a draw-with-reshuffle after a
/// lazy header has been flushed must leave **exactly one** `log.reshuffle` and
/// **exactly one** `log.draw` in the tail, the landing `text` line
/// (`log.land_draw`) present, and the pile state the host effects produced.
/// The header's posting path (outside the replace-log) must not change pile
/// state or drop the reshuffle's log line. Event ids must be unique and
/// strictly increasing -- a duplicate id means two `World` copies allocated
/// from the same `next_event`.
#[test]
fn draw_with_reshuffle_after_a_lazy_header_logs_once_each() {
    let mut t = quiet_table();
    t.set_hand(0, &[]);
    t.set_draw(0, &[]);
    t.set_discard(0, &["通用:GREAT!", "通用:PERFECT!", "通用:FEVER!"]);
    t.observe(CLAUSES);
    let keys = settle_on(&mut t, "江户川乐器店");
    assert_event_ids_unique_increasing(&t);
    let lands = keys.iter().filter(|k| *k == "log.land_draw").count();
    let reshuffles = keys.iter().filter(|k| *k == "log.reshuffle").count();
    let draws = keys.iter().filter(|k| *k == "log.draw").count();
    assert_eq!(lands, 1, "exactly one landing line: {keys:?}");
    assert_eq!(reshuffles, 1, "exactly one reshuffle: {keys:?}");
    assert_eq!(draws, 1, "exactly one draw: {keys:?}");
    assert_eq!(t.draw_pile(0).len(), 2, "reshuffle moved the 3, draw took 1: {:?}", t.draw_pile(0));
    assert_eq!(t.discard(0).len(), 0, "discard emptied into the draw pile: {:?}", t.discard(0));
    assert_eq!(t.hand(0).len(), 1, "one card drawn: {:?}", t.hand(0));
}

/// The same with a prompt between the header flush and the draw (re-run path).
#[test]
fn draw_with_reshuffle_after_a_prompt_logs_once_each() {
    let mut t = quiet_table();
    t.set_hand(0, &[]);
    t.set_draw(0, &[]);
    t.set_discard(0, &["通用:GREAT!", "通用:PERFECT!", "通用:FEVER!"]);
    t.observe(CLAUSES);
    let keys = settle_on(&mut t, "江户川乐器店");
    while t.prompt().is_some() {
        t.decline();
    }
    let keys2 = t.keys_since(0);
    assert_event_ids_unique_increasing(&t);
    let lands = keys2.iter().filter(|k| *k == "log.land_draw").count();
    let reshuffles = keys2.iter().filter(|k| *k == "log.reshuffle").count();
    let draws = keys2.iter().filter(|k| *k == "log.draw").count();
    assert_eq!(lands, 1, "exactly one landing line: {keys:?} / {keys2:?}");
    assert_eq!(reshuffles, 1, "exactly one reshuffle: {keys:?} / {keys2:?}");
    assert_eq!(draws, 1, "exactly one draw: {keys:?} / {keys2:?}");
    assert_eq!(t.hand(0).len(), 1, "one card drawn: {:?}", t.hand(0));
}

/// G-CARDS-02 on a pure draw: the pile multiset is conserved.
#[test]
fn g_cards_02_pure_draw_conserves_the_pile_multiset() {
    forall(6, 0xD7A0, |rng, _i| {
        let mut t = quiet_table();
        let n_draw = 1 + rng.range(0, 3) as usize;
        let mut draw: Vec<String> = vec![];
        for k in 0..n_draw {
            let c = data()
                .cards
                .iter()
                .filter(|c| !c.derived && c.id != "通用:登上武道馆")
                .nth(k + rng.range(0, 5) as usize)
                .map(|c| c.id.clone())
                .unwrap_or_else(|| "通用:GREAT!".into());
            draw.push(c);
        }
        let draw_refs: Vec<&str> = draw.iter().map(String::as_str).collect();
        t.set_hand(0, &[]);
        t.set_draw(0, &draw_refs);
        t.set_discard(0, &[]);
        let pre = piles(t.world(), 0);
        t.observe(CLAUSES);
        // 江户川乐器店 settle draws 1: a pure draw.
        let keys = settle_on(&mut t, "江户川乐器店");
        if !keys.iter().any(|k| k == "log.draw") {
            return Err(format!(
                "settle drew nothing (vacuous): keys={keys:?} pos={} hand={:?}",
                t.pos(0),
                t.hand(0)
            ));
        }
        let post = piles(t.world(), 0);
        if pre != post {
            return Err(format!(
                "pure draw moved the multiset: pre [{}] post [{}]",
                piles_str(&pre),
                piles_str(&post)
            ));
        }
        Ok(())
    })
    .expect("pure-draw conservation");
}

// ---------------------------------------------------------------------------
// G-CARDS-05 -- [手] only in 运营
// ---------------------------------------------------------------------------

/// 规则书 128: 「非[反击]的[手]效果只能在运营阶段使用」.
/// Outside the actor's 运营 a [手] play must be refused.
#[test]
fn g_cards_05_hand_play_outside_ops_is_refused() {
    let mut t = Table::vanilla(2);
    let card = simple_hand_card();
    t.give(0, &[&card]);
    assert_eq!(t.turn(), 0, "vanilla starts on P0");
    assert_eq!(
        t.step(),
        game_core::state::stage::OPS,
        "P0 is in 运营"
    );
    // Hand the turn to P1: P0's play is no longer P0's 运营.
    t.begin_turn(1);
    assert_eq!(t.turn(), 1);
    let r = t.play(0, &card);
    assert!(
        r.is_err(),
        "a [手] play outside the actor's 运营 must be refused, got {r:?}"
    );
    assert!(
        t.hand(0).iter().any(|c| c == &card),
        "the refused play left the card in hand"
    );
}

/// Same gate across the actor's own move stage (not 运营).
#[test]
fn g_cards_05_hand_play_in_move_stage_is_refused() {
    let mut t = Table::vanilla(2);
    let card = simple_hand_card();
    t.give(0, &[&card]);
    t.m.world_mut().st.step = game_core::state::stage::MOVE;
    // Drive the raw act: `Table::play` would settle, and a mid-move world is
    // not at rest. The refusal itself is the clause.
    let r = t.m.act(1, &game_core::net::NetMessage {
        card: card.clone(),
        ..game_core::net::NetMessage::act("play")
    });
    assert!(
        r.is_err(),
        "a [手] play in the move stage must be refused, got {r:?}"
    );
}

// ---------------------------------------------------------------------------
// G-CARDS-06 -- [限] X as a play gate (Anon Tokyo)
// ---------------------------------------------------------------------------

/// 规则书 82: 「带有“[限]X”的卡只有在X成立时才可从手中打出」.
/// `MyGO:[千早爱音]Anon Tokyo` prints X = 「自己所在的格子是[可购买格子]」.
#[test]
fn g_cards_06_limit_gate_anon_tokyo() {
    let card = "MyGO:[千早爱音]Anon Tokyo";
    // The card is exclusive to 千早爱音 -- bind that character (no skills
    // needed; `set_character_raw` writes the name the exclusive gate reads).
    // Not on a [可购买格子]: the play must be refused.
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "千早爱音");
    t.give(0, &[card]);
    t.set_pos(0, not_buyable_tile());
    assert!(
        !data().tiles[t.pos(0)].is_buyable(),
        "CiRCLE is not a [可购买格子]"
    );
    let r = t.play(0, card);
    assert!(
        r.is_err(),
        "[限] X fails (not on a [可购买格子]) so the play must be refused, got {r:?}"
    );
    assert!(
        t.hand(0).iter().any(|c| c == card),
        "still in hand after the refusal"
    );

    // On a [可购买格子]: X holds, the play is accepted (or at least activates).
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "千早爱音");
    t.give(0, &[card]);
    t.set_pos(0, buyable("购物中心"));
    assert!(data().tiles[t.pos(0)].is_buyable());
    t.observe(&[Clause::new(
        "G-CARDS-06",
        "rulebook-doc.md:82",
        "[限]：带有“[限]X”的卡只有在X成立时才可从手中打出。",
        g_cards_06,
    )]);
    let r = t.play(0, card);
    // The body designates a neighbouring tile and may prompt; accept the
    // play itself once the activation is logged.
    let keys = t.recent_keys(20);
    let played = keys.iter().any(|k| k.contains("play") || k.contains("Anon"));
    if r.is_err() && !played {
        panic!("[限] X holds on 购物中心 but the play was refused: {r:?} keys={keys:?}");
    }
}

// ---------------------------------------------------------------------------
// G-CARDS-11..13 -- the event pile
// ---------------------------------------------------------------------------

/// 规则书 153-154: a drawn event is public, never in hand, and is filed to
/// the event discard after resolution (unless it stays on the board).
/// Cafe settle = draw 1 hand card + draw 1 event.
#[test]
fn g_cards_11_event_draw_is_public_and_never_in_hand() {
    let mut t = quiet_table();
    t.set_hand(0, &[]);
    t.set_draw(0, &["通用:GREAT!"]);
    t.set_event_deck(&["对邦"]);
    t.m.world_mut().event_discard.clear();
    t.observe(CLAUSES);
    let keys = settle_on(&mut t, "CiRCLE 咖啡厅");
    // Drain whatever the event opened.
    for _ in 0..16 {
        t.settle();
        if t.prompt().is_none() {
            break;
        }
        t.decline();
    }
    // Public draw logged (the tile announce and/or the event activation);
    // never in a hand. The engine speaks the draw as `log.land_event` /
    // `log.event` / `card:event` depending on the path.
    assert!(
        keys.iter().any(|k| k == "log.event" || k == "log.land_event" || k == "card:event"),
        "the event draw is logged (public): {keys:?}"
    );
    for who in 0..2 {
        assert!(
            !t.hand(who).iter().any(|c| c == "对邦"),
            "event card entered P{who}'s hand: {:?}",
            t.hand(who)
        );
    }
    // 对邦 is a one-shot: it was filed to the event discard (G-CARDS-12).
    // The pile was a single card, so the same window then took that discard
    // as the new pile (G-CARDS-13) -- the event is in one of the two, never
    // in a hand and never in limbo.
    let filed = t.event_discard().iter().any(|e| e == "对邦")
        || t.event_deck().iter().any(|e| e == "对邦");
    assert!(
        filed,
        "one-shot event filed to the event discard (or the reshuffled pile):          discard={:?} deck={:?} removed={:?}",
        t.event_discard(),
        t.event_deck(),
        t.event_removed()
    );
}

/// 规则书 154: 「如果没有事件可抽取则将事件弃卡区洗切并当作新的事件卡堆」.
#[test]
fn g_cards_13_event_discard_reshuffles_only_when_pile_empty() {
    let cafe = data()
        .tiles
        .iter()
        .position(|v| v.kind == TileKind("cafe".into()))
        .expect("cafe");

    // Pile still holds one after the draw: no reshuffle of the discard.
    let mut t = Table::vanilla(2);
    t.set_hand(0, &[]);
    t.set_draw(0, &["通用:GREAT!", "通用:PERFECT!"]);
    t.set_event_deck(&["对邦", "飞鸟山之战"]);
    t.m.world_mut().event_discard = vec!["冲榜".into()];
    t.set_pos(0, (cafe + 59) % 60);
    t.dice(&[1]);
    let mark = t.mark();
    t.observe(CLAUSES);
    t.roll(0).expect("roll");
    for _ in 0..16 {
        t.settle();
        if t.prompt().is_none() {
            break;
        }
        t.decline();
    }
    let keys = t.keys_since(mark);
    assert!(
        !keys.iter().any(|k| k == "log.events_reshuffled"),
        "reshuffled while the pile still had room: {keys:?}"
    );
    // With a second card still on the pile there is no reshuffle, so the
    // just-resolved event sits in the event discard.
    assert!(
        t.event_discard().iter().any(|e| e == "对邦"),
        "the just-resolved event is in the discard: {:?}",
        t.event_discard()
    );

    // Empty the pile and draw again: the discard becomes the new pile.
    let mut t = Table::vanilla(2);
    t.set_hand(0, &[]);
    t.set_draw(0, &["通用:GREAT!", "通用:PERFECT!"]);
    t.set_event_deck(&["对邦"]);
    t.m.world_mut().event_discard = vec!["冲榜".into()];
    t.set_pos(0, (cafe + 59) % 60);
    t.dice(&[1]);
    let mark = t.mark();
    t.observe(CLAUSES);
    t.roll(0).expect("roll");
    for _ in 0..16 {
        t.settle();
        if t.prompt().is_none() {
            break;
        }
        t.decline();
    }
    let keys = t.keys_since(mark);
    // After 对邦 resolved and the pile emptied, the discard (对邦 + the old
    // 冲榜) becomes the new pile.
    assert!(
        keys.iter().any(|k| k == "log.events_reshuffled")
            || t.event_discard().is_empty(),
        "the emptied pile took the discard: keys={keys:?} deck={:?} discard={:?}",
        t.event_deck(),
        t.event_discard()
    );
}

/// G-CARDS-11/12 per-act clauses over a controlled event draw.
#[test]
fn g_cards_11_clause_over_a_controlled_event_draw() {
    let clauses = [
        Clause::new(
            "G-CARDS-11",
            "rulebook-doc.md:153",
            "抽取的事件卡不进入手卡并向所有玩家公开，效果立刻生效。",
            g_cards_11,
        ),
        Clause::new(
            "G-CARDS-12",
            "rulebook-doc.md:154",
            "事件结算后进入事件弃卡区。",
            g_cards_12,
        ),
    ];
    let mut t = quiet_table();
    t.set_hand(0, &[]);
    t.set_draw(0, &["通用:GREAT!", "通用:PERFECT!"]);
    t.set_event_deck(&["对邦"]);
    t.observe(&clauses);
    let _ = settle_on(&mut t, "CiRCLE 咖啡厅");
}

/// Every clause in the area table has a non-empty id / source / quote and a
/// unique id.
#[test]
fn g_cards_clause_table_is_well_formed() {
    assert!(!CLAUSES.is_empty());
    for c in CLAUSES {
        assert!(c.id.starts_with("G-CARDS-"), "{}", c.id);
        assert!(!c.source.is_empty(), "{} has no source", c.id);
        assert!(!c.quote.is_empty(), "{} has no quote", c.id);
    }
    let mut ids: Vec<&str> = CLAUSES.iter().map(|c| c.id).collect();
    let n = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), n, "duplicate clause ids");
}

// Clause fns named in the doc are referenced by the local `Clause::new` arms
// and by `CLAUSES` via `rules_spec::global::cards`.
