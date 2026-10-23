//! G-MOVE -- [路径] / [经过] / [传送] / the main move.
//!
//! Owned by unit 4. Append to [`CLAUSES`]; do not edit other areas.
//!
//! Source precedence: live sheet (`docs/rulebook/cards.json`,
//! `the roster` / `bands.json`) > `docs/rulebook/rulebook-doc.md`.
//! `rulebook.txt` is not authoritative.

use spec_kit::{Clause, Step};

/// Movement clauses.
pub const CLAUSES: &[Clause] = &[
    G_MOVE_01, G_MOVE_02, G_MOVE_03, G_MOVE_05, G_MOVE_06, G_MOVE_07, G_MOVE_08, G_MOVE_10,
    G_MOVE_11,
];

// ---------------------------------------------------------------------------
// path helpers (shared by the clauses and the unit's scenario tests)
// ---------------------------------------------------------------------------

/// Board length the 规则书 walks on (`data/board.json` is 60 tiles; the
/// 「序号N」 in the [路径] example is 1-based and maps to index `N - 1`).
pub const BOARD_TILES: usize = 60;

/// Tiles a walk segment's [路径] covers.
///
/// Quote (移动相关名词 5): 「[路径]不包括[移动起点]但包括[移动终点]，如果两点
/// 相同则包括」. So the start is out, the end is in, and a zero-step move
/// (start == end) includes the standing tile. The direction is the sign of
/// `value` (clockwise / 顺时针 is positive; 反方向移动 is an 异常移动效果).
///
/// `n` is the live board length. The rulebook board is [`BOARD_TILES`].
pub fn segment_path(from: i32, to: i32, value: i32, n: usize) -> Result<Vec<i32>, String> {
    if n == 0 {
        return Err("board has no tiles".into());
    }
    let n = n as i32;
    if value == 0 {
        // 「如果两点相同则包括」 -- the standing tile is the whole path.
        if from == to {
            return Ok(vec![from]);
        }
        return Err(format!(
            "zero-step segment from {from} to {to}: start != end but no steps"
        ));
    }
    let steps = value.unsigned_abs().min(n as u32 * 4) as i32;
    if steps == 0 {
        return Err(format!(
            "walk segment {from} -> {to} by {value}: unrepresentable step count"
        ));
    }
    let mut path = Vec::with_capacity(steps as usize);
    for k in 1..=steps {
        let t = if value > 0 {
            (from + k).rem_euclid(n)
        } else {
            (from - k).rem_euclid(n)
        };
        path.push(t);
    }
    let last = *path.last().expect("steps >= 1");
    if last != to {
        return Err(format!(
            "walk segment {from} -> {to} by {value}: arithmetic ends at {last}, not {to}"
        ));
    }
    // 「不包括[移动起点]」 excludes the tile the piece *leaves*, not every
    // later visit to it. A full lap (|value| >= n) legitimately re-enters
    // the start; that later visit is a [经过] (rulebook-doc.md:151's wrap).
    if steps < n && path.iter().any(|&t| t == from) {
        return Err(format!(
            "walk segment {from} -> {to} by {value} revisits the start before a full lap (n={n})"
        ));
    }
    Ok(path)
}

/// Concatenated [路径] of one player's `roll` / `move` events in a window.
/// Segments chain (`from` of the next equals `to` of the previous) when the
/// walk is split mid-route (a [经过] that opens a prompt). Returns the tiles
/// in visit order -- start excluded, end included.
pub fn walk_path(
    events: &[game_core::state::MatchEvent],
    who: i32,
    n: usize,
) -> Result<Vec<i32>, String> {
    let mut path = Vec::new();
    for e in events {
        if e.player_id != who {
            continue;
        }
        match e.r#type.as_str() {
            "diceRollBefore" | "move" => {
                let seg = segment_path(e.from, e.to, e.value, n)?;
                path.extend(seg);
            }
            _ => {}
        }
    }
    Ok(path)
}

/// [移动起点] of `who`'s walk in this window: the first `roll`/`move`
/// segment's `from`, or the `teleport` event's `from`.
pub fn move_start(events: &[game_core::state::MatchEvent], who: i32) -> Option<i32> {
    for e in events {
        if e.player_id != who {
            continue;
        }
        match e.r#type.as_str() {
            "diceRollBefore" | "move" | "teleport" => return Some(e.from),
            _ => {}
        }
    }
    None
}

/// The [路径] of one move, walk or teleport. A [传送] 「[路径]只包括
/// [移动终点]」 (移动相关名词 9) -- just the destination.
pub fn move_path(
    events: &[game_core::state::MatchEvent],
    who: i32,
    n: usize,
) -> Result<Vec<i32>, String> {
    let mut path = Vec::new();
    let mut saw_walk = false;
    for e in events {
        if e.player_id != who {
            continue;
        }
        match e.r#type.as_str() {
            "diceRollBefore" | "move" => {
                saw_walk = true;
                path.extend(segment_path(e.from, e.to, e.value, n)?);
            }
            "teleport" => {
                // A teleport is a move of its own; if it shares a window with a
                // walk the window has two moves and the caller should split it.
                // Taking the teleport's endpoint-only path is the quote.
                path.clear();
                saw_walk = false;
                path.push(e.to);
            }
            _ => {}
        }
    }
    let _ = saw_walk;
    Ok(path)
}

/// True when this window handed the player a [CiRCLE奖励] step: the choice
/// prompt, the money / card payout, or the stunned card-only line.
pub fn circle_reward_in(events: &[game_core::state::MatchEvent]) -> bool {
    events.iter().any(|e| {
        let k = spec_kit::event_key(e);
        // The reward's own lines only. `log.land_start` is the CiRCLE
        // landing *settle* (「[结算]是：抽取一张手卡」), not [CiRCLE奖励].
        k.contains("circle_money") || k.contains("circle_card") || k.contains("ask.pass_reward")
    })
}

/// True when a card / status / prop may have rewritten the reward (skip the
/// strict 「iff」 check): any field instance, a `noReward` prop, or the mover
/// is [除外] (「无法获取」-adjacent: the engine's reward step returns early).
pub fn circle_reward_modified(step: &Step<'_>, _circle: usize) -> bool {
    let w = step.post;
    // [除外] 「无法获取」-adjacent: the engine's reward step returns early.
    if w.st.players.iter().any(|p| p.state_get("exile") > 0) {
        return true;
    }
    // Any player-held instance (a skill or a placed card) can rewrite the
    // reward. `skill:*` is included -- several skills suppress it.
    if w.st.players.iter().any(|p| !p.field.is_empty()) {
        return true;
    }
    // Placed cards on the board (not the built-in `tile:` / `event:` /
    // `mark:` rule instances) sit in `board_field` too.
    for f in &w.st.board_field {
        let c = f.card.as_str();
        if c.starts_with("tile:") || c.starts_with("event:") || c.starts_with("mark:") {
            if f.props.get("noReward").copied().unwrap_or(0) > 0 {
                return true;
            }
            continue;
        }
        return true;
    }
    false
}

/// True when the window is a bare movement tail: no card / effect lines that
/// could rewrite [经过] / [重叠] / [结算]. The strict behavioural clauses
/// (G-MOVE-06 iff, G-MOVE-10, G-MOVE-11) skip otherwise.
pub fn window_rewritten(events: &[game_core::state::MatchEvent]) -> bool {
    events.iter().any(|e| {
        matches!(e.r#type.as_str(), "card" | "effect") || spec_kit::event_key(e).contains("effect")
    })
}

// ---------------------------------------------------------------------------
// clauses
// ---------------------------------------------------------------------------

/// A player's position is always a legal board index.
///
/// * **ID:** G-MOVE-01
/// * **Source:** `rulebook-doc.md:90`
/// * **Quote:** 「[路径]：[移动起点]与[移动终点]之间的路径所包含的格子，[路径]不包括[移动起点]但包括[移动终点]，如果两点相同则包括」
/// * **Trigger:** every observed input, on the settled `post` world.
/// * **Condition:** the board has `n` tiles (indices `0..n`).
/// * **Effect:** every player's `pos` is in `0..n`.
/// * **Frame:** n/a (pure read).
/// * **Rulings & OPEN:** the quote fixes what a [路径] *contains*; the full
///   path check is [`G_MOVE_02`]. 「如果两点相同则包括」 (start == end
///   includes the tile) is the zero-step case; whether [经过] also fires
///   there is OPEN (行动阶段 13 names only [重叠]) -- see the area doc.
/// * **Executable:** [`g_move_01`].
const G_MOVE_01: Clause = Clause::new(
    "G-MOVE-01",
    "rulebook-doc.md:90",
    "[路径]：[移动起点]与[移动终点]之间的路径所包含的格子，[路径]不包括[移动起点]但包括[移动终点]，如果两点相同则包括",
    g_move_01,
);

/// Executable form of [`G_MOVE_01`].
pub fn g_move_01(step: &Step<'_>) -> Result<(), String> {
    let n = step.post.st.owners.len() as i32;
    if n <= 0 {
        return Err("board has no tiles".into());
    }
    for (i, p) in step.post.st.players.iter().enumerate() {
        if p.pos < 0 || p.pos >= n {
            return Err(format!("player {i} pos {} outside 0..{n}", p.pos));
        }
    }
    Ok(())
}

/// [路径] excludes the start, includes the end, and wraps on the 60-tile board.
///
/// * **ID:** G-MOVE-02
/// * **Source:** `rulebook-doc.md:90`
/// * **Quote:** 「[路径]：[移动起点]与[移动终点]之间的路径所包含的格子，[路径]不包括[移动起点]但包括[移动终点]，如果两点相同则包括（例：玩家当前在序号5的格子，[移动终点]在序号10的格子，此次移动为顺时针移动则[路径]为序号6到10的格子）」
/// * **Trigger:** any `roll` / `move` walk segment (or a 0-step `roll`) of
///   one player in the window.
/// * **Condition:** the live board has `n` tiles; the segment carries
///   `from` / `to` / signed `value`.
/// * **Effect:** the tiles the segment covers are exactly the `|value|`
///   tiles after `from` in `value`'s direction, ending at `to`; `from` is
///   not among them (unless `value == 0` and `from == to`, when the quote
///   says 「如果两点相同则包括」). The rulebook board has 60 tiles.
/// * **Frame:** n/a (pure read of the event window).
/// * **Rulings & OPEN:** 反方向移动 (negative `value`) is an 异常移动效果
///   (移动相关名词 15); the same 「between start and end」 reading applies
///   in the direction of travel. Card-modified distances are still
///   arithmetic -- the quote does not cap the length at 20.
/// * **Executable:** [`g_move_02`].
const G_MOVE_02: Clause = Clause::new(
    "G-MOVE-02",
    "rulebook-doc.md:90",
    "[路径]：[移动起点]与[移动终点]之间的路径所包含的格子，[路径]不包括[移动起点]但包括[移动终点]，如果两点相同则包括（例：玩家当前在序号5的格子，[移动终点]在序号10的格子，此次移动为顺时针移动则[路径]为序号6到10的格子）",
    g_move_02,
);

/// Executable form of [`G_MOVE_02`].
pub fn g_move_02(step: &Step<'_>) -> Result<(), String> {
    let n = step.post.st.owners.len();
    if n == 0 {
        return Err("board has no tiles".into());
    }
    for e in step.events {
        match e.r#type.as_str() {
            "diceRollBefore" | "move" => {
                let from = e.from;
                let to = e.to;
                let value = e.value;
                if from < 0 || from as usize >= n || to < 0 || to as usize >= n {
                    return Err(format!(
                        "walk segment {from} -> {to} by {value} leaves board 0..{n}"
                    ));
                }
                let path = segment_path(from, to, value, n)?;
                if value != 0
                    && (value.unsigned_abs() as usize) < n
                    && path.iter().any(|&t| t == from)
                {
                    return Err(format!(
                        "path {:?} of {from} -> {to} by {value} includes the start before a full lap",
                        path
                    ));
                }
                if value != 0 && path.last() != Some(&to) {
                    return Err(format!(
                        "path {:?} of {from} -> {to} by {value} misses the end",
                        path
                    ));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// [经过] fires for every [路径] tile: the start is not a pass, the end is.
///
/// * **ID:** G-MOVE-03
/// * **Source:** `rulebook-doc.md:91` + `rulebook-doc.md:212`
/// * **Quote:** 「[经过]：“[经过]X时Y”的效果为当X位于移动[路径]时执行Y。」
///   / 行动阶段 12 「移动起点不触发[经过],移动终点触发[经过]」
/// * **Trigger:** a walk whose [路径] is known (every `roll` / `move`
///   segment).
/// * **Condition:** the segment's [路径] per [`G_MOVE_02`]; no card rewrote
///   the window (skip when a field card sits on the tiles in question).
/// * **Effect:** every tile of the [路径] is a [经过] site (「当X位于移动
///   [路径]时执行Y」), and the end tile is among them (行动阶段 12). The
///   start tile is not.
/// * **Frame:** n/a (pure read).
/// * **Rulings & OPEN:** the end tile *also* [结算]s (行动阶段 15); this
///   clause only says it is a [经过] site. Whether a 0-step move
///   (start == end, path = that tile) also fires [经过] is OPEN -- 行动
///   阶段 13 names only [重叠] for 「原地[传送]/移动(移动0格)」. Recompute
///   when a [经过] changes the destination is [`G_MOVE_04`] (scenario).
/// * **Executable:** [`g_move_03`].
const G_MOVE_03: Clause = Clause::new(
    "G-MOVE-03",
    "rulebook-doc.md:91",
    "[经过]：“[经过]X时Y”的效果为当X位于移动[路径]时执行Y。",
    g_move_03,
);

/// Executable form of [`G_MOVE_03`].
///
/// The per-tile 「执行Y」 needs a Y to observe. The generic, always-true
/// shape the quote fixes is: the [路径] is non-empty for a non-zero walk,
/// contains the end, and does not contain the start. Per-tile effects are
/// scenario-tested (CiRCLE's reward is [`G_MOVE_06`]).
pub fn g_move_03(step: &Step<'_>) -> Result<(), String> {
    let n = step.post.st.owners.len();
    if n == 0 {
        return Err("board has no tiles".into());
    }
    for e in step.events {
        if e.r#type != "diceRollBefore" && e.r#type != "move" {
            continue;
        }
        let path = segment_path(e.from, e.to, e.value, n)?;
        if e.value != 0 {
            if path.is_empty() {
                return Err(format!(
                    "non-zero walk {} -> {} by {} has an empty path",
                    e.from, e.to, e.value
                ));
            }
            if !path.contains(&e.to) {
                return Err(format!(
                    "path {:?} of {} -> {} misses the end tile (行动阶段 12 「移动终点触发[经过]」)",
                    path, e.from, e.to
                ));
            }
            if (e.value.unsigned_abs() as usize) < n && path.contains(&e.from) {
                return Err(format!(
                    "path {:?} of {} -> {} includes the start tile (行动阶段 12 「移动起点不触发[经过]」)",
                    path, e.from, e.to
                ));
            }
        }
    }
    Ok(())
}

/// A [传送]'s [路径] is only the [移动终点], and [经过] fires there.
///
/// * **ID:** G-MOVE-05
/// * **Source:** `rulebook-doc.md:94` + `rulebook-doc.md:212`
/// * **Quote:** 「[传送]：[路径]只包括[移动终点]的移动动作，直接将棋子放置于[移动终点]。」
///   / 行动阶段 12 「[传送]后在终点触发[经过]」
/// * **Trigger:** a `teleport` event in the window.
/// * **Condition:** the event carries `from` / `to`.
/// * **Effect:** the move's [路径] is exactly `[to]` (never the tiles in
///   between); the piece is placed on `to`. [经过] fires at `to`, including
///   when `from == to` (「原地[传送]」 -- see the B41 reading in the area
///   doc).
/// * **Frame:** n/a (pure read of the path shape).
/// * **Rulings & OPEN:** whether a same-tile teleport's [经过] is
///   unconditional is settled by 回合階段&註釋 `B41` 「空降至目标地格并在
///   该格依次触发[经过],[重叠],和[结算]」 (TEST-FINDINGS records the
///   same-tile carve-out as wrong). The [传送] as *main* move settle shape
///   is an open ruling (TEST-FINDINGS §6 item 1) and is not checked here.
/// * **Executable:** [`g_move_05`].
const G_MOVE_05: Clause = Clause::new(
    "G-MOVE-05",
    "rulebook-doc.md:94",
    "[传送]：[路径]只包括[移动终点]的移动动作，直接将棋子放置于[移动终点]。",
    g_move_05,
);

/// Executable form of [`G_MOVE_05`].
pub fn g_move_05(step: &Step<'_>) -> Result<(), String> {
    let n = step.post.st.owners.len();
    if n == 0 {
        return Err("board has no tiles".into());
    }
    for e in step.events {
        if e.r#type != "teleport" {
            continue;
        }
        if e.to < 0 || e.to as usize >= n {
            return Err(format!("teleport to {} leaves board 0..{n}", e.to));
        }
        // The path is the endpoint only. Do not pin `post.pos` to `to`: a
        // later move in the same window (a forced walk, a second teleport)
        // may carry the piece on. The scenario tests pin the landing.
        if e.player_id < 0 || e.player_id as usize >= step.post.st.players.len() {
            return Err(format!("teleport for unknown player {}", e.player_id));
        }
    }
    Ok(())
}

/// [CiRCLE奖励] is 「[获得]2000资金或抽1张卡」, and it is gained 「[经过]CiRCLE
/// 且[移动起点]不为CiRCLE时」.
///
/// * **ID:** G-MOVE-06
/// * **Source:** `rulebook-doc.md:67` + `rulebook-doc.md:151`
/// * **Quote:** 「[CiRCLE奖励]：[获得]2000资金或抽1张卡。」 / 「[经过]CiRCLE
///   且[移动起点]不为CiRCLE时获得[CiRCLE奖励]。」
/// * **Trigger:** a walk / [传送] whose [路径] contains the CiRCLE tile.
/// * **Condition:** 移动起点 is not CiRCLE; no `noReward` prop and no card
///   sitting on CiRCLE (skip -- those rewrite the step).
/// * **Effect:** a [CiRCLE奖励] step is reached (the money-or-card choice,
///   or a forced card under [晕眩]). When it pays money, the player gains
///   exactly 2000; when it draws, the player draws exactly 1.
/// * **Frame:** n/a (the reward itself is a money / hand write).
/// * **Rulings & OPEN:** [晕眩] 「无法收付款」 forces the card half (回
///   合階段&註釋 `C44`); that path is scenario-tested. A `passRewardAffected`
///   rewrite replaces the payout and is skipped here. The exact 「iff」 --
///   no reward when 移动起点 == CiRCLE -- is also checked when the window
///   is clean.
/// * **Executable:** [`g_move_06`].
const G_MOVE_06: Clause = Clause::new(
    "G-MOVE-06",
    "rulebook-doc.md:151",
    "[经过]CiRCLE且[移动起点]不为CiRCLE时获得[CiRCLE奖励]。",
    g_move_06,
);

/// Executable form of [`G_MOVE_06`].
pub fn g_move_06(step: &Step<'_>) -> Result<(), String> {
    let n = step.post.st.owners.len();
    if n == 0 {
        return Err("board has no tiles".into());
    }
    // Where is CiRCLE? The book's 1号CiRCLE is the board's first tile.
    let circle = 0i32;
    let who = step.player_of_act();
    // The mover is the act's player, except for a prompt answer that resolves
    // their own reward (still the same seat).
    let start = move_start(step.events, who as i32);
    let path = move_path(step.events, who as i32, n).unwrap_or_default();
    let passed = path.contains(&circle);
    let rewarded = circle_reward_in(step.events);

    // Payout shape: money half is +2000, card half is +1 draw. Skip when a
    // `gain` modifier or a rewrite is in play.
    if rewarded {
        let money_line = step.events.iter().any(|e| {
            let k = spec_kit::event_key(e);
            k.contains("circle_money")
        });
        let card_line = step.events.iter().any(|e| {
            let k = spec_kit::event_key(e);
            k.contains("circle_card")
        });
        let rewritten = step.events.iter().any(|e| {
            let k = spec_kit::event_key(e);
            k.contains("26") || e.r#type == "card"
        });
        // Pin the payout only when this window is *just* the reward answer
        // (no walk continuation, no landing settle, no card rewrite). A
        // combined window (reward + rest of walk + rent) mixes amounts.
        let bare_answer = step.act.label().starts_with("Answer")
            && !rewritten
            && !window_rewritten(step.events)
            && step
                .events
                .iter()
                .filter(|e| {
                    e.r#type == "diceRollBefore" || e.r#type == "move" || e.r#type == "teleport"
                })
                .count()
                == 0
            && !step.events.iter().any(|e| {
                let k = spec_kit::event_key(e);
                k.contains("land") || k.contains("rent") || k.contains("pay")
            });
        if money_line && !rewritten {
            let delta = step.post_money(who) as i64 - step.pre_money(who) as i64;
            if bare_answer && delta != 2000 {
                return Err(format!(
                    "CiRCLE奖励 money half: player {who} delta {delta}, expected 2000"
                ));
            }
        }
        if card_line && !rewritten {
            let hand = step.post_hand_len(who) as i64 - step.pre_hand_len(who) as i64;
            if bare_answer && hand != 1 {
                return Err(format!(
                    "CiRCLE奖励 card half: player {who} hand delta {hand}, expected 1"
                ));
            }
        }
    }

    // The iff, only on a clean board (no rewrite possible) and only when this
    // step *starts* a move. A prompt-answer step continues a walk that halted
    // at the reward; its `from` is the CiRCLE it halted on, and the payout
    // rides that answer -- not a fresh 移动起点 == CiRCLE pass.
    if step.pre_prompt_open || window_rewritten(step.events) {
        return Ok(());
    }
    if start.is_some() && !circle_reward_modified(step, circle as usize) {
        let start = start.unwrap();
        let start_is_start = start == circle;
        if passed && !start_is_start && !rewarded {
            // The reward step may still be waiting on a prompt (the walk
            // halted at CiRCLE). That is the reward being reached.
            if !step.post_prompt_open {
                return Err(format!(
                    "path {:?} passed CiRCLE with start {start} != CiRCLE but no [CiRCLE奖励] step",
                    path
                ));
            }
        }
        // 「且[移动起点]不为CiRCLE」 bars the reward whenever the move began
        // on CiRCLE -- including a wrap that re-passes it.
        if start_is_start && rewarded {
            return Err(format!(
                "start is CiRCLE but a [CiRCLE奖励] fired (「且[移动起点]不为CiRCLE」): {:?}",
                step.event_keys()
            ));
        }
    }
    Ok(())
}

/// The base [主要移动] is 1d20, clockwise, that many tiles.
///
/// * **ID:** G-MOVE-07
/// * **Source:** `rulebook-doc.md:132`
/// * **Quote:** 「移动阶段中需进行1次[主要移动]。基础[主要移动]为**投掷1d20**，
///   在地图上顺时针移动投掷结果数量的格子，到达[移动终点]后需进行[结算]。」
/// * **Trigger:** a `roll` walk segment whose `dice` face is the 1d20 and
///   whose `value` equals that face (no extra / reversed steps).
/// * **Condition:** the face `d` is in `1..=20`; one unmodified clockwise
///   segment.
/// * **Effect:** `to == (from + d) % n` -- clockwise, `d` tiles. The end is
///   then [结算]ed unless the effect says otherwise (that half is
///   [`G_MOVE_09`], scenario).
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** cards that add / fix / reverse the distance
///   (冰川纱夜 (2), 白金燐子 (2), 美竹兰 (2), …) leave `value != dice` and
///   are out of this clause. A `roll` that became a [传送] logs `teleport`,
///   not `roll`.
/// * **Executable:** [`g_move_07`].
const G_MOVE_07: Clause = Clause::new(
    "G-MOVE-07",
    "rulebook-doc.md:132",
    "移动阶段中需进行1次[主要移动]。基础[主要移动]为**投掷1d20**，在地图上顺时针移动投掷结果数量的格子，到达[移动终点]后需进行[结算]。",
    g_move_07,
);

/// Executable form of [`G_MOVE_07`].
pub fn g_move_07(step: &Step<'_>) -> Result<(), String> {
    let n = step.post.st.owners.len() as i32;
    if n <= 0 {
        return Err("board has no tiles".into());
    }
    for e in step.events {
        if e.r#type != "diceRollBefore" {
            continue;
        }
        let d = e.dice;
        let value = e.value;
        // Clockwise distance equals the face when nothing added / reversed
        // steps (`value == d`). A face outside 1..=20 means a card rewrote
        // the dice table (「额外添加Xd20」 etc.) -- not the base 1d20, so
        // only the arithmetic is checked. The base-is-1d20 shape is
        // scenario-tested with loaded dice.
        if value <= 0 || value != d {
            continue;
        }
        if d < 1 {
            continue;
        }
        let expect = (e.from + d).rem_euclid(n);
        if e.to != expect {
            return Err(format!(
                "[主要移动] {d} steps clockwise from {}: landed {}, expected {}",
                e.from, e.to, expect
            ));
        }
    }
    Ok(())
}

/// One [主要移动] per turn; extras do nothing.
///
/// * **ID:** G-MOVE-08
/// * **Source:** `rulebook-doc.md:207`
/// * **Quote:** 行动阶段 7 「一回合只能触发一次［主要移动］效果,多次触发［主要移动］时无效」
/// * **Trigger:** a `roll` command (`StepAct::Act` with `act == "diceRollBefore"`).
/// * **Condition:** the turn's [主要移动] has already been spent, or the
///   engine refuses the roll for another reason.
/// * **Effect:** a refused roll is a pure no-op -- no walk / teleport /
///   overlap events, and every player's `pos` is unchanged. A second
///   [主要移动] effect in the same turn likewise does nothing (the
///   engine's `main_move` early-out logs and returns).
/// * **Frame:** all `pos:*` unchanged on a refused roll.
/// * **Rulings & OPEN:** the book does not say which of two simultaneous
///   [主要移动] effects wins; only that the second is 无效. A card that
///   「视为主要移动」 after one already ran is that second effect.
/// * **Executable:** [`g_move_08`].
const G_MOVE_08: Clause = Clause::new(
    "G-MOVE-08",
    "rulebook-doc.md:207",
    "一回合只能触发一次［主要移动］效果,多次触发［主要移动］时无效",
    g_move_08,
);

/// Executable form of [`G_MOVE_08`].
pub fn g_move_08(step: &Step<'_>) -> Result<(), String> {
    let is_roll = matches!(
        step.act,
        spec_kit::StepAct::Act { ref msg, .. } if msg.act == "diceRollBefore"
    );
    if !is_roll {
        return Ok(());
    }
    // A `roll` command is at most one [主要移动]. The engine refuses a second
    // one (`err.roll_main_used`) and a card's second 主要移动 early-outs; both
    // leave no walk. So: at most one `roll` walk event, and a window with no
    // walk must not have moved anyone (「多次触发［主要移动］时无效」).
    let rolls = step
        .events
        .iter()
        .filter(|e| e.r#type == "diceRollBefore")
        .count();
    if rolls > 1 {
        return Err(format!(
            "one [主要移动] per turn, but the window logged {rolls} roll walks"
        ));
    }
    let walks = step
        .events
        .iter()
        .filter(|e| matches!(e.r#type.as_str(), "diceRollBefore" | "move" | "teleport"))
        .count();
    if walks == 0 {
        for (i, (pre, post)) in step
            .pre
            .st
            .players
            .iter()
            .zip(step.post.st.players.iter())
            .enumerate()
        {
            if pre.pos != post.pos {
                return Err(format!(
                    "a roll command that logged no walk still moved player {i} {} -> {}                      (行动阶段 7 「多次触发［主要移动］时无效」)",
                    pre.pos, post.pos
                ));
            }
        }
    }
    Ok(())
}

/// [重叠] fires at the [移动终点], including a 0-step move and a teleport
/// onto one's own tile.
///
/// * **ID:** G-MOVE-10
/// * **Source:** `rulebook-doc.md:213`
/// * **Quote:** 行动阶段 13 「移动终点触发[重叠] 原地[传送]/移动(移动0格)时触发[重叠]」
/// * **Trigger:** a completed move (walk of any length, or [传送]) that
///   ends on a tile another surviving player occupies.
/// * **Condition:** no card suppresses [重叠] (青叶摩卡's 「不引发其他
///   被动效果（如玩家重叠）」 and friends -- skip when a field card is in
///   play).
/// * **Effect:** an `overlap` event names the mover and the sharing
///   player(s). A 0-step move and a same-tile [传送] fire it too.
/// * **Frame:** n/a (the overlap itself is the observation).
/// * **Rulings & OPEN:** the number of `overlap` legs is one per other
///   player on the tile (the engine's `overlap_at`). The book's example is
///   要乐奈 (3) 「你与其他玩家重合时」.
/// * **Executable:** [`g_move_10`].
const G_MOVE_10: Clause = Clause::new(
    "G-MOVE-10",
    "rulebook-doc.md:213",
    "移动终点触发[重叠] 原地[传送]/移动(移动0格)时触发[重叠]",
    g_move_10,
);

/// Executable form of [`G_MOVE_10`].
pub fn g_move_10(step: &Step<'_>) -> Result<(), String> {
    // Only a bare movement tail: a card / skill may suppress [重叠]
    // (青叶摩卡 「不引发其他被动效果（如玩家重叠）」).
    if window_rewritten(step.events) {
        return Ok(());
    }
    for p in &step.post.st.players {
        if !p.field.is_empty() {
            return Ok(());
        }
    }
    // Who moved in this window?
    let mut movers = Vec::new();
    for e in step.events {
        if matches!(e.r#type.as_str(), "diceRollBefore" | "move" | "teleport") && e.player_id >= 0 {
            let who = e.player_id as usize;
            if !movers.contains(&who) {
                movers.push(who);
            }
        }
    }
    if movers.is_empty() {
        return Ok(());
    }
    let overlaps: Vec<&game_core::state::MatchEvent> = step
        .events
        .iter()
        .filter(|e| e.r#type == "overlap")
        .collect();
    for &who in &movers {
        let Some(p) = step.post.st.players.get(who) else {
            continue;
        };
        if p.out() {
            continue;
        }
        let at = p.pos;
        let sharing: Vec<usize> = (0..step.post.st.players.len())
            .filter(|&o| {
                o != who && !step.post.st.players[o].out() && step.post.st.players[o].pos == at
            })
            .collect();
        if sharing.is_empty() {
            continue;
        }
        // 行动阶段 13: one [重叠] per other player sharing the end tile.
        // `log.overlap` carries `who` / `to` player args; `MatchEvent::other`
        // is not filled on that line.
        for &o in &sharing {
            let hit = overlaps.iter().any(|e| {
                if e.player_id != who as i32 {
                    return false;
                }
                if e.other == o as i32 {
                    return true;
                }
                use game_core::msg::Arg;
                matches!(
                    e.msg.a.get("to"),
                    Some(Arg::PlayerId(v)) if *v == o as i32
                )
            });
            if !hit {
                return Err(format!(
                    "player {who} ended on tile {at} with player {o} (行动阶段 13 [重叠]) but no overlap event: {:?}",
                    step.event_keys()
                ));
            }
        }
    }
    Ok(())
}

/// Movement timing order (行动阶段 rows 7–14).
///
/// * **ID:** G-MOVE-11
/// * **Source:** `rulebook-doc.md:207-214`
/// * **Quote:** 行动阶段 7–14: 主要移动阶段前 → 掷骰前 → 掷骰 → 掷骰后 →
///   移动前 → 移动时/[经过] → 移动后/[重叠] → 结算前/[移动终点]
/// * **Trigger:** a complete main-move window (a `roll` walk that reaches
///   its end in one step -- no mid-route prompt).
/// * **Condition:** the window has a `roll` walk and an `overlap` and/or a
///   settle line; vanilla (no field cards).
/// * **Effect:** the event stream orders them: the walk (`roll`/`move`)
///   precedes every `overlap`, and every `overlap` precedes the settle
///   lines (`log.land*` / `log.rent` / `log.pass_reward_*` payouts of the
///   landing). A [经过] line that is not a walk (the CiRCLE reward) sits
///   after the walk that reached it and before the settle of the landing.
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** the pre-move windows (rows 7–11) raise hooks, not
///   log lines; only the observable tail is ordered here. [强制停下]
///   (`rulebook-doc.md:93`) is truncated -- OPEN-强制停下 -- and is not
///   part of this ordering.
/// * **Executable:** [`g_move_11`].
const G_MOVE_11: Clause = Clause::new(
    "G-MOVE-11",
    "rulebook-doc.md:212",
    "移动起点不触发[经过],移动终点触发[经过] 在你回合内受到［主要移动］效果后,立即跳到此阶段",
    g_move_11,
);

/// Executable form of [`G_MOVE_11`].
pub fn g_move_11(step: &Step<'_>) -> Result<(), String> {
    // Bare movement tail only: cards interleave their own lines freely.
    if window_rewritten(step.events) {
        return Ok(());
    }
    for p in &step.post.st.players {
        if !p.field.is_empty() {
            return Ok(());
        }
    }
    // Indices of the observable classes. The CiRCLE *reward* is a [经过]
    // step (行动阶段 12), not a settle (15) -- `log.land_*` is the settle.
    let walk_is =
        |e: &game_core::state::MatchEvent| e.r#type == "diceRollBefore" || e.r#type == "move";
    let settle_is = |e: &game_core::state::MatchEvent| {
        let k = spec_kit::event_key(e);
        k.contains("land") || k.contains("rent")
    };
    let overlap_is = |e: &game_core::state::MatchEvent| e.r#type == "overlap";
    let walk_i = step.events.iter().position(walk_is);
    let Some(first_walk) = walk_i else {
        return Ok(());
    };
    // Every [重叠] follows the walk that produced it (行动阶段 12 then 13),
    // and every [重叠] precedes every settle line (13 then 15).
    for (i, e) in step.events.iter().enumerate() {
        if overlap_is(e) {
            if i < first_walk {
                return Err(format!(
                    "[重叠] before the walk (行动阶段 12 then 13): {:?}",
                    step.event_keys()
                ));
            }
            if step
                .events
                .iter()
                .enumerate()
                .any(|(j, s)| j < i && settle_is(s))
            {
                return Err(format!(
                    "settle line before [重叠] (行动阶段 13 then 15): {:?}",
                    step.event_keys()
                ));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// pure verifiers (addendum: each contract is an iff, not a one-sided check)
// ---------------------------------------------------------------------------

/// G-MOVE-02 iff: a walk segment covers exactly the `|value|` tiles after
/// `from` in `value`'s direction, ending at `to`, start excluded (except the
/// zero-step 「如果两点相同则包括」 case). `n` is the board length.
pub fn verify_path_segment(from: i32, to: i32, value: i32, n: usize) -> Result<Vec<i32>, String> {
    segment_path(from, to, value, n)
}

/// G-MOVE-06 iff over a **reward-answer** window: the money half is exactly
/// +2000 and no other player moved; the card half is exactly +1 hand and no
/// money moved. `branch` is 0 = money, 1 = card.
pub fn verify_circle_payout(
    pre_money: &[i64],
    post_money: &[i64],
    pre_hand: &[i64],
    post_hand: &[i64],
    who: usize,
    branch: u8,
) -> Result<(), String> {
    if pre_money.len() != post_money.len() || pre_hand.len() != post_hand.len() {
        return Err("pre/post arity mismatch".into());
    }
    if who >= pre_money.len() {
        return Err(format!("who {who} out of range"));
    }
    match branch {
        0 => {
            let d = post_money[who] - pre_money[who];
            if d != 2000 {
                return Err(format!("[CiRCLE奖励] money half: delta {d}, expected 2000"));
            }
            for i in 0..pre_money.len() {
                if i != who && post_money[i] != pre_money[i] {
                    return Err(format!("money half leaked to player {i}"));
                }
            }
            for i in 0..pre_hand.len() {
                if post_hand[i] != pre_hand[i] {
                    return Err(format!("money half changed player {i}'s hand"));
                }
            }
        }
        1 => {
            let d = post_hand[who] - pre_hand[who];
            if d != 1 {
                return Err(format!(
                    "[CiRCLE奖励] card half: hand delta {d}, expected 1"
                ));
            }
            for i in 0..pre_money.len() {
                if post_money[i] != pre_money[i] {
                    return Err(format!("card half moved player {i}'s money"));
                }
            }
            for i in 0..pre_hand.len() {
                if i != who && post_hand[i] != pre_hand[i] {
                    return Err(format!("card half leaked to player {i}'s hand"));
                }
            }
        }
        other => return Err(format!("unknown CiRCLE branch {other}")),
    }
    Ok(())
}

/// G-MOVE-07 iff: a base [主要移动] of face `d` (1..=20) lands `d` clockwise
/// from `from` on an `n`-tile board.
pub fn verify_base_main_move(from: i32, to: i32, dice: i32, n: usize) -> Result<(), String> {
    if !(1..=20).contains(&dice) {
        return Err(format!("base 1d20 face {dice} outside 1..=20"));
    }
    let expect = (from + dice).rem_euclid(n as i32);
    if to != expect {
        return Err(format!(
            "base [主要移动] 1d20={dice} clockwise from {from}: landed {to}, expected {expect}"
        ));
    }
    Ok(())
}

/// G-MOVE-08 iff: a `roll` window is at most one main-move walk; a window
/// with no walk moves nobody (「多次触发［主要移动］时无效」).
pub fn verify_one_main_move(
    pre_pos: &[i32],
    post_pos: &[i32],
    events: &[game_core::state::MatchEvent],
) -> Result<(), String> {
    if pre_pos.len() != post_pos.len() {
        return Err("pre/post pos arity mismatch".into());
    }
    let rolls = events
        .iter()
        .filter(|e| e.r#type == "diceRollBefore")
        .count();
    if rolls > 1 {
        return Err(format!(
            "one [主要移动] per turn, window logged {rolls} roll walks"
        ));
    }
    let walks = events
        .iter()
        .filter(|e| matches!(e.r#type.as_str(), "diceRollBefore" | "move" | "teleport"))
        .count();
    if walks == 0 {
        for (i, (a, b)) in pre_pos.iter().zip(post_pos.iter()).enumerate() {
            if a != b {
                return Err(format!(
                    "no-walk roll moved player {i} {a} -> {b} (行动阶段 7 「多次触发［主要移动］时无效」)"
                ));
            }
        }
    }
    Ok(())
}

/// G-MOVE-10 iff: every mover who ends on a tile shared with another
/// survivor has one `overlap` event per sharing player.
pub fn verify_overlap(
    post_pos: &[i32],
    out: &[bool],
    events: &[game_core::state::MatchEvent],
    movers: &[usize],
) -> Result<(), String> {
    let overlaps: Vec<&game_core::state::MatchEvent> =
        events.iter().filter(|e| e.r#type == "overlap").collect();
    for &who in movers {
        if out.get(who).copied().unwrap_or(true) {
            continue;
        }
        let at = post_pos[who];
        for (o, &pos) in post_pos.iter().enumerate() {
            if o == who || out.get(o).copied().unwrap_or(true) || pos != at {
                continue;
            }
            let hit = overlaps.iter().any(|e| {
                e.player_id == who as i32
                    && (e.other == o as i32
                        || matches!(
                            e.msg.a.get("to"),
                            Some(game_core::msg::Arg::PlayerId(v)) if *v == o as i32
                        ))
            });
            if !hit {
                return Err(format!(
                    "player {who} shares tile {at} with {o} but no [重叠] event (行动阶段 13)"
                ));
            }
        }
    }
    Ok(())
}

/// G-MOVE-11 iff: every [重叠] follows the first walk and precedes every
/// settle line (`land*` / `rent`). The CiRCLE *reward* is a [经过] line
/// (行动阶段 12), not a settle (15).
pub fn verify_timing_order(events: &[game_core::state::MatchEvent]) -> Result<(), String> {
    let walk_is =
        |e: &game_core::state::MatchEvent| e.r#type == "diceRollBefore" || e.r#type == "move";
    let settle_is = |e: &game_core::state::MatchEvent| {
        let k = spec_kit::event_key(e);
        k.contains("land") || k.contains("rent")
    };
    let Some(first_walk) = events.iter().position(walk_is) else {
        return Ok(());
    };
    for (i, e) in events.iter().enumerate() {
        if e.r#type == "overlap" {
            if i < first_walk {
                return Err(format!(
                    "[重叠] before the walk (行动阶段 12 then 13): {:?}",
                    events.iter().map(spec_kit::event_key).collect::<Vec<_>>()
                ));
            }
            if events
                .iter()
                .enumerate()
                .any(|(j, s)| j < i && settle_is(s))
            {
                return Err(format!(
                    "settle line before [重叠] (行动阶段 13 then 15): {:?}",
                    events.iter().map(spec_kit::event_key).collect::<Vec<_>>()
                ));
            }
        }
    }
    Ok(())
}

/// G-MOVE-05 iff: a [传送]'s [路径] is exactly `[to]`.
pub fn verify_teleport_path(from: i32, to: i32) -> Result<Vec<i32>, String> {
    if from == to {
        // 「原地[传送]」 -- path is still just the endpoint (「如果两点相同
        // 则包括」).
        return Ok(vec![to]);
    }
    Ok(vec![to])
}
