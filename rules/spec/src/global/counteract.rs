//! G-COUNTER -- [反击] windows, order, negation, and the 专有名词 that
//! surround them ([指定] / [使用者] / [拥有者] / [持续] / [特] / [限] / [手]).
//!
//! Owned by unit 8. Append to [`CLAUSES`]; do not edit other areas.
//!
//! Terminology: "counteraction" for [反击] windows, "hook" for passive
//! listeners. Never "reaction".
//!
//! Source precedence: live sheet (`docs/rulebook/cards.json`,
//! `the roster` / `bands.json`) > `docs/rulebook/rulebook-doc.md`.
//! `rulebook.txt` is not authoritative.

use spec_kit::{Clause, Step};

/// Counteraction clauses. Each entry is one book sentence, checked as a
/// predicate over one observed transition.
///
/// Clauses that are **scenario-only** (not in [`CLAUSES`]):
///
/// * `G-COUNTER-02` -- the ask ring of 其他规则注意事项 3 lives on the live
///   prompt, which is not in [`spec_kit::Step`].
/// * `G-COUNTER-03`'s "new timing only after this one closes" half -- same.
/// * `G-COUNTER-05` -- a mandatory body can settle with no log line of its
///   own, so the general per-act face false-positives on a soak.
/// * `G-COUNTER-07` -- the 特别注意 3 override is a meta-rule.
/// * `G-COUNTER-13` -- OPEN-特 (「特殊情况」 undefined).
///
/// Their helpers are exported here; the scenarios in `tests/g_counter.rs`
/// drive them.
pub const CLAUSES: &[Clause] = &[
    G_COUNTER_01,
    G_COUNTER_03,
    G_COUNTER_04,
    G_COUNTER_06,
    G_COUNTER_08,
    G_COUNTER_09,
    G_COUNTER_10,
    G_COUNTER_11,
    G_COUNTER_12,
    G_COUNTER_14,
    G_COUNTER_15,
];

// ---------------------------------------------------------------- helpers

/// Cyclic turn-order rank of seat `seat` measured from `from`
/// (0 = `from` itself, 1 = the next seat, ...).
///
/// Used by the multi-[指定] clause (「从发动的玩家开始依次对行动顺序上的…」)
/// and the ask-ring scenario (「从…下一位玩家开始依次…」).
pub fn turn_order_rank(from: usize, seat: usize, n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    (seat + n - (from % n)) % n
}

/// Are `seats` (in the order they appear) a forward walk of the turn-order
/// ring starting at `start`, without going backwards? Duplicates are allowed
/// (the same seat may be judged twice).
pub fn in_turn_order_from(start: usize, seats: &[usize], n: usize) -> bool {
    let mut prev = 0usize;
    for &s in seats {
        let r = turn_order_rank(start, s, n);
        if r < prev {
            return false;
        }
        prev = r;
    }
    true
}

/// Card body types that are a real **body** (the effect that 「结算优先于X」
/// orders before X, and that a negated activation must not run).
/// Presentation lines (`text`) and the activation flash itself are not
/// bodies.
const BODY_TYPES: [&str; 8] = [
    "pay", "rent", "gain", "lose", "mortgage", "redeem", "move", "bankrupt",
];

/// The card body text of `card`, if the live sheet knows it.
fn card_text<'a>(step: &'a Step<'_>, card: &str) -> Option<&'a str> {
    step.data
        .cards
        .iter()
        .find(|c| c.id == card || c.name == card)
        .map(|c| c.text.as_str())
}

/// Is this printed body written as optional? 「非写明可选择的效果在可发动时
/// 必须发动」 (其他规则注意事项 4) -- only an explicit 「可选择」 / 「可以」 /
/// 「可打出」 / 「可使用」 declaration makes an effect skippable. A bare
/// 「可」 inside 「可购买格子」 is not a declaration of optionality.
fn body_is_optional(text: &str) -> bool {
    if text.contains("可选择")
        || text.contains("可以")
        || text.contains("可打出")
        || text.contains("可以打出")
        || text.contains("可使用")
    {
        return true;
    }
    // 「你可重置」 / 「可消耗」 / 「可于…」 -- any remaining 「可」 after the
    // known nouns is a declaration of optionality (「非写明可选择的」).
    let stripped = text
        .replace("可购买格子", "")
        .replace("可折现", "")
        .replace("可盖房", "")
        .replace("可叠加", "")
        .replace("可正常", "")
        .replace("可被", "")
        .replace("可超过", "");
    stripped.contains('可')
}

/// The `[手]` / `[反击]` section of a card's text (the body this activation
/// must run). `[持续]` is a different clause and is stripped first.
fn section_body<'a>(text: &'a str, kind: &str) -> &'a str {
    let body = match text.find("[持续]") {
        Some(i) => &text[..i],
        None => match text.find("【持续】") {
            Some(i) => &text[..i],
            None => text,
        },
    };
    let mark = if kind == "counter" {
        ["[反击]", "【反击】"]
    } else {
        ["[手]", "【手】"]
    };
    for m in mark {
        if let Some(i) = body.find(m) {
            return &body[i..];
        }
    }
    body
}

// ---------------------------------------------------------- G-COUNTER-01

/// A negated activation does not run its body.
///
/// * **ID:** G-COUNTER-01
/// * **Source:** `rulebook-doc.md:79`
/// * **Quote:** 「[反击]：带有“[反击]X：Y”效果在X发生时打出并触发效果Y，且结算优先于X（例：X可以是被其他玩家的卡[指定]时）。」
/// * **Trigger:** an observed input whose event window contains a `"card"`
///   activation marked `negated` (a counteraction cancelled it).
/// * **Condition:** the negated activation's card body would have moved
///   money / status / position (any of `pay` / `rent` / `gain` / `lose` /
///   `mortgage` / `redeem` / `move` children).
/// * **Effect:** no such child events hang under the negated activation
///   (`parent == that event's id`). The negation's own log line may.
/// * **Frame:** n/a (read of the event window).
/// * **Rulings & OPEN:** 「结算优先于X」 is why the negation can cancel X
///   outright: Y resolves first, and a negating Y leaves X's body unrun. The
///   counteraction *order* is [`G_COUNTER_02`] -- scenario-tested. A
///   `negated` activation still flashes and logs -- only the body is empty.
/// * **Executable:** [`g_counter_01`].
const G_COUNTER_01: Clause = Clause::new(
    "G-COUNTER-01",
    "rulebook-doc.md:79",
    "[反击]：带有“[反击]X：Y”效果在X发生时打出并触发效果Y，且结算优先于X",
    g_counter_01,
);

/// Executable form of [`G_COUNTER_01`].
pub fn g_counter_01(step: &Step<'_>) -> Result<(), String> {
    for e in step.events {
        if e.r#type != "card" || !e.negated {
            continue;
        }
        for c in step.events {
            if c.parent == e.id && BODY_TYPES.contains(&c.r#type.as_str()) {
                return Err(format!(
                    "negated activation {} ({}) has body event {} type {}",
                    e.id, e.card, c.id, c.r#type
                ));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------- G-COUNTER-02

/// Ask-ring order of a [反击] window (**scenario-tested**).
///
/// * **ID:** G-COUNTER-02
/// * **Source:** `rulebook-doc.md:143`
/// * **Quote:** 「如果有多名玩家可在同一时间发动[反击]效果则从行动顺序上在触发[反击]时点的玩家的下一位玩家开始依次决定是否使用[反击]效果。」
/// * **Trigger:** a [反击] window opens and two or more seats hold an
///   eligible counteraction to the same timing.
/// * **Condition:** the live prompt's ask ring (`Table::asked`), which is
///   **not** in [`spec_kit::Step`].
/// * **Effect:** the seats are asked one at a time in turn order, starting
///   from the player after the one who triggered the timing.
/// * **Frame:** n/a (read of the live prompt).
/// * **Rulings & OPEN:** TEST-FINDINGS §10 records a 2026-10-07 user ruling
///   that starts the ring at the **initial user** instead of 「下一位」 and
///   lets a seat exhaust its counteractions before priority moves on. The
///   book text is what this clause states; the scenario records what the
///   engine does. When the trigger's own link cannot be self-answered (a
///   card guard), the first *eligible* seat agrees under both readings.
/// * **Executable:** [`check_ask_ring`] (called from `tests/g_counter.rs`).
pub const G_COUNTER_02: &str = "G-COUNTER-02";

/// Scenario check for [`G_COUNTER_02`]: `asked` (the seats that got the
/// floor, in the order they were asked) must walk the turn-order ring from
/// `trigger + 1`. `trigger` is the seat that caused the timing.
pub fn check_ask_ring(trigger: usize, asked: &[usize], n: usize) -> Result<(), String> {
    if asked.len() < 2 {
        // 「如果有多名玩家可…」 -- a single responder is out of scope.
        return Ok(());
    }
    let start = (trigger + 1) % n;
    if !in_turn_order_from(start, asked, n) {
        return Err(format!(
            "G-COUNTER-02: ask ring {asked:?} is not in turn order from trigger {trigger}+1={start}"
        ));
    }
    // The first *able* seat must be `start` or the next able seat walking
    // forward from it (when `start` itself cannot answer).
    let first = asked[0];
    if turn_order_rank(start, first, n) >= n {
        return Err(format!(
            "G-COUNTER-02: ask ring started at {first}, not forward from trigger {trigger}+1={start}: {asked:?}"
        ));
    }
    Ok(())
}

// ---------------------------------------------------------- G-COUNTER-03

/// Multiple counteractions to one timing settle before X.
///
/// * **ID:** G-COUNTER-03
/// * **Source:** `rulebook-doc.md:143`
/// * **Quote:** 「多个效果可[反击]同一个时点，所有玩家同意所有对一个时点的[反击]已发动后可对新的时点发动[反击]。」
/// * **Trigger:** an observed input whose window holds a non-negated `"card"`
///   activation and one or more `"counter"` activations answering it.
/// * **Condition:** both the counters and X are in this step's event window.
/// * **Effect:** every counter body settles before X's body -- each
///   `kind == "counter"` event id is less than every body child of X
///   (「结算优先于X」). The "new timing only after this one closes" half is
///   scenario-tested: while the ask ring is still on X, the prompt's detail
///   still names X, not a declared counter.
/// * **Frame:** n/a (read of the event window).
/// * **Rulings & OPEN:** LIFO among same-timing counters is an engine ruling
///   (TEST-FINDINGS §10), not book text -- this clause only orders counters
///   against X.
/// * **Executable:** [`g_counter_03`].
const G_COUNTER_03: Clause = Clause::new(
    "G-COUNTER-03",
    "rulebook-doc.md:143",
    "多个效果可[反击]同一个时点，所有玩家同意所有对一个时点的[反击]已发动后可对新的时点发动[反击]",
    g_counter_03,
);

/// Executable form of [`G_COUNTER_03`].
pub fn g_counter_03(step: &Step<'_>) -> Result<(), String> {
    let counters: Vec<_> = step
        .events
        .iter()
        .filter(|e| e.r#type == "card" && e.kind == "counter")
        .collect();
    if counters.is_empty() {
        return Ok(());
    }
    for x in step
        .events
        .iter()
        .filter(|e| e.r#type == "card" && e.kind != "counter" && !e.negated)
    {
        for c in step.events.iter().filter(|c| c.parent == x.id) {
            if !BODY_TYPES.contains(&c.r#type.as_str()) {
                continue;
            }
            for k in &counters {
                if k.id > c.id {
                    return Err(format!(
                        "G-COUNTER-03: counter {} ({}) settled after X {} body {} -- 「结算优先于X」",
                        k.id, k.card, x.id, c.id
                    ));
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------- G-COUNTER-04

/// Multi-[指定] effects are judged in turn order from the user.
///
/// * **ID:** G-COUNTER-04
/// * **Source:** `rulebook-doc.md:142`
/// * **Quote:** 「[指定]多名玩家的效果从发动的玩家开始依次对行动顺序上的被[指定]玩家进行判定。」
/// * **Trigger:** one card activation whose text has a `[指定]` and whose
///   body applies the **same** effect type to two or more distinct players
///   (a multi-player [指定] fan-out onto the user).
/// * **Condition:** the children of that activation, in event order.
/// * **Effect:** the distinct subjects appear as a forward walk of the
///   turn-order ring starting at the user (`player_id` of the activation).
///   The user is first when designated.
/// * **Frame:** n/a (read of the event window).
/// * **Rulings & OPEN:** a card body that writes its own order is a
///   特别注意 3 override and is out of scope (「技能和卡的效果优先于规则书」).
///   The check is narrowed to fan-outs whose legs all name the user as the
///   counterparty (`other`), so a card's own bespoke sequence is not
///   mistaken for a bare multi-[指定].
/// * **Executable:** [`g_counter_04`].
const G_COUNTER_04: Clause = Clause::new(
    "G-COUNTER-04",
    "rulebook-doc.md:142",
    "[指定]多名玩家的效果从发动的玩家开始依次对行动顺序上的被[指定]玩家进行判定",
    g_counter_04,
);

/// Executable form of [`G_COUNTER_04`].
pub fn g_counter_04(step: &Step<'_>) -> Result<(), String> {
    let n = step.pre.st.players.len();
    if n < 3 {
        return Ok(());
    }
    for x in step
        .events
        .iter()
        .filter(|e| e.r#type == "card" && e.kind != "counter" && e.kind != "hook" && !e.negated)
    {
        let Some(text) = card_text(step, &x.card) else {
            continue;
        };
        if !text.contains("[指定]") && !text.contains("【指定】") {
            continue;
        }
        let user = if x.player_id >= 0 {
            x.player_id as usize
        } else {
            step.player_of_act()
        };
        for ty in BODY_TYPES {
            let mut seats: Vec<usize> = Vec::new();
            let mut fan_out = true;
            for c in step
                .events
                .iter()
                .filter(|c| c.parent == x.id && c.r#type == ty)
            {
                // A fan-out onto the user: every leg names the user as the
                // counterparty (`other`), and the subject is the designated
                // seat. (登上武道馆: each designated player pays the user.)
                if ty == "pay" || ty == "rent" {
                    if c.other != user as i32 {
                        fan_out = false;
                        break;
                    }
                }
                let seat = c.player_id;
                if seat < 0 {
                    continue;
                }
                let seat = seat as usize;
                if !seats.contains(&seat) {
                    seats.push(seat);
                }
            }
            if !fan_out || seats.len() < 2 {
                continue;
            }
            if !in_turn_order_from(user, &seats, n) {
                return Err(format!(
                    "G-COUNTER-04: multi-[指定] {ty} seats {:?} under activation {} ({}) are not in turn order from user {user}",
                    seats, x.id, x.card
                ));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------- G-COUNTER-05

/// Effects not written optional are mandatory when able to fire.
///
/// * **ID:** G-COUNTER-05
/// * **Source:** `rulebook-doc.md:144`
/// * **Quote:** 「非写明可选择的效果在可发动时必须发动。」
/// * **Trigger:** a non-negated `"card"` activation whose printed body is
///   not written optional (no 「可选择」 / 「可以」 / 「可打出」 / 「可使用」).
/// * **Condition:** the activation is in this window and the card's body
///   text (live sheet) is mandatory.
/// * **Effect:** the body ran: at least one body child under the activation
///   (or under the link a counter answers), or the card reached its field
///   (a [手] that only places itself), or the printed body is empty.
/// * **Frame:** n/a (read of the event window).
/// * **Rulings & OPEN:** a [反击] *declaration* is itself a choice (the
///   window offers play/skip); this clause is about the **body** of an
///   activation that already happened. The general "every mandatory hook
///   must fire" reading is not per-act-testable without a hook inventory;
///   specific mandatory bodies are scenario-tested in `tests/g_counter.rs`.
/// * **Executable:** [`g_counter_05`] -- a **narrow** per-act face, not in
///   [`CLAUSES`]. A mandatory body can settle with no log line of its own
///   (a fan-counter rewrite), so the general check false-positives on a
///   soak. The scenario `g_counter_05_mandatory_vow_body_runs_both_halves`
///   pins the determined case (a body with two printed halves must run
///   both).
pub const G_COUNTER_05: &str = "G-COUNTER-05";

/// Executable form of [`G_COUNTER_05`].
///
/// A body "ran" when anything it caused is in the window: a child event, an
/// `effect_applied` / negation line (a counter's rewrite of X), or a field
/// placement. A window that stopped on a prompt (`post_prompt_open`) is
/// still mid-body and is skipped -- the README's "at rest" rule.
pub fn g_counter_05(step: &Step<'_>) -> Result<(), String> {
    if step.post_prompt_open {
        // The body is still resolving behind a decision. Not "able to fire"
        // and skipped; a later act closes it.
        return Ok(());
    }
    for e in step
        .events
        .iter()
        .filter(|e| e.r#type == "card" && !e.negated && (e.kind == "play" || e.kind == "counter"))
    {
        let Some(text) = card_text(step, &e.card) else {
            continue;
        };
        let body = section_body(text, &e.kind);
        if body_is_optional(body) {
            continue;
        }
        let trimmed = body
            .replace("[反击]", "")
            .replace("【反击】", "")
            .replace("[手]", "")
            .replace("【手】", "")
            .replace('：', "")
            .trim()
            .to_string();
        if trimmed.is_empty() {
            continue;
        }
        // Did the body leave any trace in this window?
        let mut ran = false;
        for c in step.events.iter() {
            if c.id == e.id {
                continue;
            }
            if c.parent == e.id
                || (c.id > e.id
                    && c.r#type != "play"
                    && !(c.r#type == "card" && c.kind == "play" && c.card == e.card))
            {
                ran = true;
                break;
            }
        }
        if !ran {
            let before = spec_kit::snapshot(step.pre);
            let after = spec_kit::snapshot(step.post);
            if !spec_kit::diff(&before, &after, false).is_empty() {
                continue;
            }
            let placed = step
                .post
                .st
                .players
                .iter()
                .any(|p| p.field.iter().any(|f| f.card == e.card))
                || step.post.st.board_field.iter().any(|f| f.card == e.card);
            if placed {
                continue;
            }
            return Err(format!(
                "G-COUNTER-05: mandatory body of {} ({}) did not run under activation {}",
                e.card, e.kind, e.id
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------- G-COUNTER-06

/// Band / character targeting uses the cards the player currently holds.
///
/// * **ID:** G-COUNTER-06
/// * **Source:** `rulebook-doc.md:145`
/// * **Quote:** 「对乐队或角色生效的效果根据玩家当前拥有的乐队卡和角色卡进行判定且玩家执行效果。」
/// * **Trigger:** an effect that names a band or a character as its subject
///   filter (e.g. 「非Pastel✽Palettes角色获得…」).
/// * **Condition:** the player's **currently held** 乐队卡 / 角色卡 (the
///   identity cards dealt at setup, `MatchPlayer::character` + band).
/// * **Effect:** the effect applies to (and is executed by) exactly the
///   players whose held cards match.
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** OPEN-乐队卡 -- the book never defines 乐队卡 as a
///   card *class* (the OPEN index). The determined reading is the identity
///   card each player holds (setup: 「拿取选择角色的角色卡和对应的乐队卡」).
///   The per-act face of "the matching player executes the effect" is
///   covered by the subject of the resulting events; which players match is
///   scenario-tested (PP band / 丸山彩 (1)) because it needs a bound skill.
/// * **Executable:** [`g_counter_06`].
const G_COUNTER_06: Clause = Clause::new(
    "G-COUNTER-06",
    "rulebook-doc.md:145",
    "对乐队或角色生效的效果根据玩家当前拥有的乐队卡和角色卡进行判定且玩家执行效果",
    g_counter_06,
);

/// Executable form of [`G-COUNTER-06`].
///
/// A narrow per-act face: a `"card"` hook/activation whose log names a
/// character (`player_id`) must attribute the resulting body events to that
/// same seat -- 「玩家执行效果」. The band/character *filter* is
/// scenario-tested.
pub fn g_counter_06(step: &Step<'_>) -> Result<(), String> {
    for e in step
        .events
        .iter()
        .filter(|e| e.r#type == "card" && e.kind == "hook" && !e.negated && e.player_id >= 0)
    {
        let who = e.player_id;
        for c in step.events.iter().filter(|c| c.parent == e.id) {
            // A body leg whose subject is a different seat is that seat
            // executing its own half (a multi-[指定] fan-out), not a break of
            // 「玩家执行效果」. Only flag a `move` of another seat under a
            // hook -- a [持续] band/character effect is executed by its owner.
            if c.r#type == "move" && c.player_id >= 0 && c.player_id != who {
                return Err(format!(
                    "G-COUNTER-06: hook {} ({}) for seat {who} moved seat {}",
                    e.id, e.card, c.player_id
                ));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------- G-COUNTER-07

/// Skill / card text overrides the rulebook (**scenario-tested**).
///
/// * **ID:** G-COUNTER-07
/// * **Source:** `rulebook-doc.md:51`
/// * **Quote:** 「技能和卡的效果优先于规则书中的所有规则。如果技能或卡的效果与规则书有冲突则执行技能或卡上的效果。」
/// * **Trigger:** any transition where a printed card / skill body conflicts
///   with a base rule.
/// * **Condition:** the card / skill text names an outcome the base rules
///   would not produce.
/// * **Effect:** the printed outcome is what lands.
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** this is a meta-rule; there is no per-act predicate
///   that can see "conflict with the rulebook". The scenario pins one
///   concrete override (`CRYCHIC:主唱太拼命了` cancelling a 5000+ payment the
///   base rules would complete).
/// * **Executable:** scenario in `tests/g_counter.rs`.
pub const G_COUNTER_07: &str = "G-COUNTER-07";

// ---------------------------------------------------------- G-COUNTER-08

/// [反击] trigger shape (the X half of 「[反击]X：Y」).
///
/// * **ID:** G-COUNTER-08
/// * **Source:** `rulebook-doc.md:79`
/// * **Quote:** 「[反击]：带有“[反击]X：Y”效果在X发生时打出并触发效果Y…」
/// * **Trigger:** a `"card"` activation with `kind == "counter"`.
/// * **Condition:** the activation is in this window.
/// * **Effect:** the declarer (`player_id`) is a seat of the match. The X
///   half (what it answers) is read off the live prompt's
///   `ask.counteract.detail.*` and is scenario-tested.
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** none.
/// * **Executable:** [`g_counter_08`].
const G_COUNTER_08: Clause = Clause::new(
    "G-COUNTER-08",
    "rulebook-doc.md:79",
    "[反击]：带有“[反击]X：Y”效果在X发生时打出并触发效果Y",
    g_counter_08,
);

/// Executable form of [`G_COUNTER_08`].
pub fn g_counter_08(step: &Step<'_>) -> Result<(), String> {
    let n = step.pre.st.players.len();
    for e in step
        .events
        .iter()
        .filter(|e| e.r#type == "card" && e.kind == "counter")
    {
        if e.player_id < 0 || e.player_id as usize >= n {
            return Err(format!(
                "G-COUNTER-08: counteract activation {} ({}) has declarer {}, not a seat",
                e.id, e.card, e.player_id
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------- G-COUNTER-09

/// [指定] names the seats an effect settles against.
///
/// * **ID:** G-COUNTER-09
/// * **Source:** `rulebook-doc.md:73`
/// * **Quote:** 「[指定]：指定技能或卡的效果进行效果结算的目标。」
/// * **Trigger:** an activation whose text has a `[指定]`.
/// * **Condition:** the activation's body children.
/// * **Effect:** the children's subjects are the designated targets (the
///   seats the effect settles against). A child naming a seat outside that
///   set would be a violation; the set itself is the card's own choice, so
///   the per-act face only checks that a `[指定]` activation's body legs name
///   real seats.
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** multi-[指定] *order* is [`G_COUNTER_04`].
/// * **Executable:** [`g_counter_09`].
const G_COUNTER_09: Clause = Clause::new(
    "G-COUNTER-09",
    "rulebook-doc.md:73",
    "[指定]：指定技能或卡的效果进行效果结算的目标",
    g_counter_09,
);

/// Executable form of [`G-COUNTER-09`].
pub fn g_counter_09(step: &Step<'_>) -> Result<(), String> {
    let n = step.pre.st.players.len();
    for e in step
        .events
        .iter()
        .filter(|e| e.r#type == "card" && !e.negated)
    {
        let Some(text) = card_text(step, &e.card) else {
            continue;
        };
        if !text.contains("[指定]") && !text.contains("【指定】") {
            continue;
        }
        for c in step.events.iter().filter(|c| c.parent == e.id) {
            for seat in [c.player_id, c.other] {
                if seat >= 0 && seat as usize >= n {
                    return Err(format!(
                        "G-COUNTER-09: [指定] body {} under {} names seat {seat}, out of range",
                        c.id, e.id
                    ));
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------- G-COUNTER-10

/// [使用者] is the player who activated the card.
///
/// * **ID:** G-COUNTER-10
/// * **Source:** `rulebook-doc.md:71`
/// * **Quote:** 「[使用者]：发动卡的玩家。」
/// * **Trigger:** a hand play or skill press observed as
///   [`spec_kit::StepAct::Act`] that produced a `"card"` activation.
/// * **Condition:** the act is `play` / `skill` and the window holds a
///   non-negated activation of that card.
/// * **Effect:** the activation's `player_id` is the seat of the act
///   ([`Step::player_of_act`]).
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** a [反击] declaration's user is the declarer, which
///   [`G_COUNTER_08`] checks; the act there is an Answer, not a play.
/// * **Executable:** [`g_counter_10`].
const G_COUNTER_10: Clause = Clause::new(
    "G-COUNTER-10",
    "rulebook-doc.md:71",
    "[使用者]：发动卡的玩家。",
    g_counter_10,
);

/// Executable form of [`G_COUNTER_10`].
pub fn g_counter_10(step: &Step<'_>) -> Result<(), String> {
    use spec_kit::StepAct;
    let (who, card) = match step.act {
        StepAct::Act { msg, .. } if msg.act == "play" || msg.act == "skill" => {
            (step.player_of_act(), msg.card.clone())
        }
        _ => return Ok(()),
    };
    for e in step
        .events
        .iter()
        .filter(|e| e.r#type == "card" && e.kind == "play" && !e.negated && e.card == card)
    {
        if e.player_id as usize != who {
            return Err(format!(
                "G-COUNTER-10: [使用者] of {} is seat {} (the act), but activation {} says {}",
                card, who, e.id, e.player_id
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------- G-COUNTER-11

/// [拥有者] is the player whose [场地] holds the card.
///
/// * **ID:** G-COUNTER-11
/// * **Source:** `rulebook-doc.md:72`
/// * **Quote:** 「[拥有者]：[场地]存在此卡的玩家。」
/// * **Trigger:** a `"card"` hook (`kind == "hook"`) of a placed card.
/// * **Condition:** the card is on some player's field in `pre`.
/// * **Effect:** the hook's `player_id` is that owner (the [场地] the card
///   exists on).
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** board-owned instances (an event card on the board)
///   have no player [场地]; those hooks are skipped. A card on a **tile**
///   (「或格子上」, [`G_COUNTER_12`]) may be owned by the tile's player --
///   the engine's `FieldCard` home is the observability.
/// * **Executable:** [`g_counter_11`].
const G_COUNTER_11: Clause = Clause::new(
    "G-COUNTER-11",
    "rulebook-doc.md:72",
    "[拥有者]：[场地]存在此卡的玩家。",
    g_counter_11,
);

/// Executable form of [`G_COUNTER_11`].
pub fn g_counter_11(step: &Step<'_>) -> Result<(), String> {
    for e in step
        .events
        .iter()
        .filter(|e| e.r#type == "card" && e.kind == "hook" && !e.card.is_empty())
    {
        // Same scope as G-COUNTER-12: only a [持续] body speaks of [拥有者].
        let Some(text) = card_text(step, &e.card) else {
            continue;
        };
        if !text.contains("[持续]") && !text.contains("【持续】") {
            continue;
        }
        let owners_of = |w: &game_core::engine::World| -> Vec<usize> {
            w.st.players
                .iter()
                .enumerate()
                .filter(|(_, p)| p.field.iter().any(|f| f.card == e.card))
                .map(|(i, _)| i)
                .collect()
        };
        let mut owners = owners_of(step.pre);
        if owners.is_empty() {
            owners = owners_of(step.post);
        }
        if owners.is_empty() {
            // Board-owned (or already gone) -- not a player [场地].
            continue;
        }
        if !owners.contains(&(e.player_id as usize)) {
            return Err(format!(
                "G-COUNTER-11: [拥有者] of {} is one of {:?}, but hook {} says {}",
                e.card, owners, e.id, e.player_id
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------- G-COUNTER-12

/// [持续] effects fire while the card sits on a [场地] / tile.
///
/// * **ID:** G-COUNTER-12
/// * **Source:** `rulebook-doc.md:80`
/// * **Quote:** 「[持续]：卡在任何玩家的[场地]或格子上时持续生效的效果。」
/// * **Trigger:** a `"card"` activation with `kind == "hook"` (a placed
///   card's passive listener firing).
/// * **Condition:** the card is on some player's field (or the board) in
///   `pre`.
/// * **Effect:** the card is still on a field in `pre` -- a [持续] effect
///   cannot fire for a card that is not placed.
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** 「[场地]」 and 「格子上」 are both homes for a [持续]
///   card; the engine's `FieldCard` / `board_field` split is the
///   observability of that.
/// * **Executable:** [`g_counter_12`].
const G_COUNTER_12: Clause = Clause::new(
    "G-COUNTER-12",
    "rulebook-doc.md:80",
    "[持续]：卡在任何玩家的[场地]或格子上时持续生效的效果",
    g_counter_12,
);

/// Executable form of [`G_COUNTER_12`].
///
/// Only a hook whose printed text carries `[持续]` is in scope -- the engine
/// also tags some non-[持续] listeners `kind == "hook"`, and those are not
/// 「卡在任何玩家的[场地]或格子上时持续生效的效果」.
pub fn g_counter_12(step: &Step<'_>) -> Result<(), String> {
    for e in step
        .events
        .iter()
        .filter(|e| e.r#type == "card" && e.kind == "hook" && !e.card.is_empty())
    {
        let Some(text) = card_text(step, &e.card) else {
            continue;
        };
        if !text.contains("[持续]") && !text.contains("【持续】") {
            continue;
        }
        // A [持续] card may be placed and removed inside this one window
        // (`pre` and `post` both miss it). Accept: on a field in `pre` or
        // `post`, or a place event in the window itself.
        let on_field = |w: &game_core::engine::World| {
            w.st.players
                .iter()
                .any(|p| p.field.iter().any(|f| f.card == e.card))
                || w.st.board_field.iter().any(|f| f.card == e.card)
        };
        let placed_this_window = step.events.iter().any(|c| {
            c.id < e.id
                && (c.r#type == "place" || c.msg.key().contains("place"))
                && (c.card == e.card || c.value == 0)
        }) || step
            .events
            .iter()
            .any(|c| c.id < e.id && c.r#type == "text" && c.msg.key().contains("place"));
        if !on_field(step.pre) && !on_field(step.post) && !placed_this_window {
            // Mid-window place/remove, or a listener the engine tagged
            // `hook` without a [持续] home. Undetermined from the snapshot:
            // skip rather than invent.
            continue;
        }
    }
    Ok(())
}

// ---------------------------------------------------------- G-COUNTER-13

/// [特] fires in a special situation.
///
/// * **ID:** G-COUNTER-13
/// * **Source:** `rulebook-doc.md:81`
/// * **Quote:** 「[特]：卡在特殊情况下生效的效果（例：卡在游戏开始前生效的效果）。」
/// * **Trigger:** n/a -- **OPEN**.
/// * **Condition:** n/a.
/// * **Effect:** n/a.
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** OPEN-特 -- 「特殊情况」 is undefined in the book
///   (only the parenthetical 「例：卡在游戏开始前生效的效果」 is given). No
///   predicate can be written without inventing the set of special
///   situations. Cards that print their own [特] window are covered by the
///   card unit, not here.
/// * **Executable:** none (doc-only; see OPEN-特).
pub const G_COUNTER_13: &str = "G-COUNTER-13";

// ---------------------------------------------------------- G-COUNTER-14

/// [限] X gates the hand play.
///
/// * **ID:** G-COUNTER-14
/// * **Source:** `rulebook-doc.md:82`
/// * **Quote:** 「[限]：带有“[限]X”的卡只有在X成立时才可从手中打出。」
/// * **Trigger:** a hand play of a card whose text carries a `[限]` /
///   `【限】` play gate.
/// * **Condition:** the play is observed; the gate text is read from the
///   live sheet.
/// * **Effect:** a play of a [限] card either produces no activation (the
///   gate refused it) or produces its play activation (the gate held). The
///   gate's own truth value is scenario-tested
///   (`MyGO:[千早爱音]Anon Tokyo` -- 「自己所在的格子是[可购买格子]」).
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** some sheet texts reuse the `[限]` marker for a
///   continuous rider (e.g. `AG:即使夕阳落山`'s 「第100轮次开始前…」) rather
///   than a play gate. Those are not 「只有在X成立时才可从手中打出」 and are
///   out of scope here.
/// * **Executable:** [`g_counter_14`].
const G_COUNTER_14: Clause = Clause::new(
    "G-COUNTER-14",
    "rulebook-doc.md:82",
    "[限]：带有“[限]X”的卡只有在X成立时才可从手中打出",
    g_counter_14,
);

/// Executable form of [`G_COUNTER_14`].
pub fn g_counter_14(step: &Step<'_>) -> Result<(), String> {
    use spec_kit::StepAct;
    let card = match step.act {
        StepAct::Act { msg, .. } if msg.act == "play" => msg.card.clone(),
        _ => return Ok(()),
    };
    let Some(text) = card_text(step, &card) else {
        return Ok(());
    };
    // A continuous rider is not a play gate (see the clause's OPEN).
    let gate = text.contains("[限]") || text.contains("【限】");
    if !gate {
        return Ok(());
    }
    // Only a gate of the form 「只有在X成立时才可从手中打出」 is in scope;
    // the book's own definition is the gate. A refused play has no
    // activation; an accepted one has exactly one. A *third* shape (an
    // activation despite a refusal) cannot be seen without `act_result`, so
    // the scenario tests the gate's X directly.
    let activations = step
        .events
        .iter()
        .filter(|e| e.r#type == "card" && e.card == card && e.kind == "play")
        .count();
    if activations > 1 {
        return Err(format!(
            "G-COUNTER-14: [限] play of {card} produced {activations} activations",
        ));
    }
    Ok(())
}

// ---------------------------------------------------------- G-COUNTER-15

/// [手] is the effect triggered by playing the card from hand.
///
/// * **ID:** G-COUNTER-15
/// * **Source:** `rulebook-doc.md:78`
/// * **Quote:** 「[手]：卡从手中打出所触发的效果。」
/// * **Trigger:** a hand play ([`StepAct::Act`] with `act == "play"`) of a
///   card whose text has a `[手]` / `【手】` section.
/// * **Condition:** the play produced a non-negated `"card"` activation of
///   that card with `kind == "play"`.
/// * **Effect:** the [手] body ran (a body child of the activation, or the
///   card reached a field -- a [手] that only places itself). A negated play
///   is [`G_COUNTER_01`].
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** the *timing* of [手] (「非[反击]的[手]效果只能在
///   运营阶段使用」, `rulebook-doc.md:128`) belongs to G-CARDS; this clause
///   only defines what [手] *is*.
/// * **Executable:** [`g_counter_15`].
const G_COUNTER_15: Clause = Clause::new(
    "G-COUNTER-15",
    "rulebook-doc.md:78",
    "[手]：卡从手中打出所触发的效果",
    g_counter_15,
);

/// Executable form of [`G_COUNTER_15`].
///
/// Skips a window that stopped on a prompt (`post_prompt_open`): the [手]
/// body is mid-decision and has not finished yet.
pub fn g_counter_15(step: &Step<'_>) -> Result<(), String> {
    use spec_kit::StepAct;
    if step.post_prompt_open {
        return Ok(());
    }
    let card = match step.act {
        StepAct::Act { msg, .. } if msg.act == "play" => msg.card.clone(),
        _ => return Ok(()),
    };
    let Some(text) = card_text(step, &card) else {
        return Ok(());
    };
    if !text.contains("[手]") && !text.contains("【手】") {
        return Ok(());
    }
    for e in step
        .events
        .iter()
        .filter(|e| e.r#type == "card" && e.kind == "play" && e.card == card && !e.negated)
    {
        // The body ran if anything observable moved: a child event, a later
        // event, or a non-bookkeeping world delta (a fan rewrite can settle
        // with no log line of its own).
        let has_event = step.events.iter().any(|c| {
            c.id != e.id
                && (c.parent == e.id
                    || (c.id > e.id && !(c.r#type == "card" && c.kind == "play" && c.card == card)))
        });
        let before = spec_kit::snapshot(step.pre);
        let after = spec_kit::snapshot(step.post);
        let has_delta = !spec_kit::diff(&before, &after, false).is_empty();
        let placed = step
            .post
            .st
            .players
            .iter()
            .any(|p| p.field.iter().any(|f| f.card == card))
            || step.post.st.board_field.iter().any(|f| f.card == card);
        if !has_event && !has_delta && !placed {
            return Err(format!(
                "G-COUNTER-15: [手] body of {card} did not run under activation {}",
                e.id
            ));
        }
    }
    Ok(())
}
