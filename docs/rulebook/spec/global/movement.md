# G-MOVE -- [路径] / [经过] / [传送] / [主要移动]

Unit 4 owns this file and `rules/spec/src/global/movement.rs`. Append clauses;
do not edit other areas.

Sources: live sheet (`docs/rulebook/cards.json`, `data/characters.json`,
`data/bands.json`) > `docs/rulebook/rulebook-doc.md`. `rulebook.txt` is not
authoritative. TEST-FINDINGS §6 rulings unless superseded by newer sheet text.

Formal-spec note: every clause below is the source text the machine-checked
verifier must implement. The `Trigger / Condition / Effect / Frame` fields
are the contract; `Rulings & OPEN` marks what the text does **not** decide
and must not be invented. `Executable` names the current Rust predicate
(interim until the formal method lands).

Board: `data/board.json` is **60** tiles. The book's 「序号N格」 is 1-based and
maps to engine index `N - 1` (`rules_spec::tile`). Clockwise is increasing
index, wrapping at 60.

---

## G-MOVE-01

* **ID:** `G-MOVE-01`
* **Source:** `rulebook-doc.md:90`
* **Quote:** 「[路径]：[移动起点]与[移动终点]之间的路径所包含的格子，[路径]不包括[移动起点]但包括[移动终点]，如果两点相同则包括」
* **Trigger:** every observed input, on the settled `post` world.
* **Condition:** the board has `n` tiles (indices `0..n`).
* **Effect:** every player's `pos` is in `0..n`.
* **Frame:** n/a (pure read).
* **Rulings & OPEN:** the quote fixes what a [路径] *contains*; the full
  path check is `G-MOVE-02`. 「如果两点相同则包括」 (start == end includes
  the tile) is the zero-step case; whether [经过] also fires there is OPEN
  (see OPEN-经过-0步) -- 行动阶段 13 names only [重叠].
* **Executable:** `rules_spec::global::movement::g_move_01`.
* **Scenario:** `tests/spec_g_move.rs::g_move_01_pos_stays_on_the_board`.

---

## G-MOVE-02

* **ID:** `G-MOVE-02`
* **Source:** `rulebook-doc.md:90`
* **Quote:** 「[路径]：[移动起点]与[移动终点]之间的路径所包含的格子，[路径]不包括[移动起点]但包括[移动终点]，如果两点相同则包括（例：玩家当前在序号5的格子，[移动终点]在序号10的格子，此次移动为顺时针移动则[路径]为序号6到10的格子）」
* **Trigger:** any `roll` / `move` walk segment (or a 0-step `roll`) of one
  player in the window.
* **Condition:** the live board has `n` tiles (60); the segment carries
  `from` / `to` / signed `value`.
* **Effect:** the tiles the segment covers are exactly the `|value|` tiles
  after `from` in `value`'s direction, ending at `to`; `from` is not among
  them (unless `value == 0` and `from == to`, when the quote says 「如果两点
  相同则包括」). Clockwise is positive; the example 5→10 is 0-based 4→9
  with path `5..=9`.
* **Frame:** n/a (pure read of the event window).
* **Rulings & OPEN:** 反方向移动 (negative `value`) is an 异常移动效果
  (移动相关名词 15); the same 「between start and end」 reading applies in
  the direction of travel. Card-modified distances are still arithmetic --
  the quote does not cap the length at 20.
* **Executable:** `rules_spec::global::movement::g_move_02` (plus
  `segment_path`).
* **Scenario:** `tests/spec_g_move.rs::g_move_02_path_excludes_start_includes_end`,
  `g_move_02_path_wraps_the_board`.

---

## G-MOVE-03

* **ID:** `G-MOVE-03`
* **Source:** `rulebook-doc.md:91` + `rulebook-doc.md:212`
* **Quote:** 「[经过]：“[经过]X时Y”的效果为当X位于移动[路径]时执行Y。」
  / 行动阶段 12 「移动起点不触发[经过],移动终点触发[经过]」
* **Trigger:** a walk whose [路径] is known (every `roll` / `move` segment).
* **Condition:** the segment's [路径] per `G-MOVE-02`; no card rewrote the
  window.
* **Effect:** every tile of the [路径] is a [经过] site (「当X位于移动[路径]
  时执行Y」), and the end tile is among them (行动阶段 12). The start tile
  is not.
* **Frame:** n/a (pure read).
* **Rulings & OPEN:** the end tile *also* [结算]s (行动阶段 15); this clause
  only says it is a [经过] site. Whether a 0-step move (start == end, path =
  that tile) also fires [经过] is OPEN -- see OPEN-经过-0步. Recompute when a
  [经过] changes the destination is `G-MOVE-04` (scenario).
* **Executable:** `rules_spec::global::movement::g_move_03`.
* **Scenario:** `tests/spec_g_move.rs::g_move_03_end_settles_start_does_not`.

---

## G-MOVE-04

* **ID:** `G-MOVE-04`
* **Source:** `rulebook-doc.md:140`
* **Quote:** 「移动时对[路径]中的所有格子依次触发[经过]格子的效果。如果某个格子的[经过]效果会改变[移动终点]则重新计算[路径]，可能导致后续格子不算作[经过]。」
* **Trigger:** a [经过] effect on a path tile rewrites the [移动终点]
  **during the move** (「移动时」).
* **Condition:** the original [路径] was computed to the old end; a [经过]
  on some path tile changes the end before the walk reaches the later tiles.
* **Effect:** the [路径] is recomputed from the same [移动起点] to the new
  [移动终点]; tiles that were in the old path but not the new one are no
  longer [经过] sites.
* **Frame:** n/a.
* **Rulings & OPEN:** the quote says 「可能导致」 -- it does not force every
  later tile to drop, only that they may. The sheet's destination-rewriters
  are timed **after** the walk: `RAS:（LOCK）追逐梦想的步伐`（2）「在触发结算
  前将行动终点改为"旭汤澡堂"」 and 丰川祥子（CRYCHIC）(2) 「主要移动结束时，
  [触发结算]前…使自己的移动终点沿最短路径向…靠近1格」. Those change the
  settle target (行动阶段 14), not the walk's [路径], so the recompute rule
  does not bite. No current sheet card rewrites the end mid-walk; if one
  appears, this clause applies and later tiles drop.
* **Executable:** scenario-only (a per-act clause cannot see 「a [经过]
  changed the destination」 without naming the rewriting card). The
  determined part of the LOCK text (the settle runs at 旭汤澡堂) is pinned
  by the scenario; the mid-walk recompute has no sheet-card instance.
* **Scenario:** `tests/spec_g_move.rs::g_move_04_pass_rewrites_the_end_before_settle`.

---

## G-MOVE-05

* **ID:** `G-MOVE-05`
* **Source:** `rulebook-doc.md:94` + `rulebook-doc.md:212`
* **Quote:** 「[传送]：[路径]只包括[移动终点]的移动动作，直接将棋子放置于[移动终点]。」
  / 行动阶段 12 「[传送]后在终点触发[经过]」
* **Trigger:** a `teleport` event in the window.
* **Condition:** the event carries `from` / `to`.
* **Effect:** the move's [路径] is exactly `[to]` (never the tiles in
  between); the piece is placed on `to`. [经过] fires at `to`, including
  when `from == to`.
* **Frame:** n/a (pure read of the path shape).
* **Rulings & OPEN:** whether a same-tile teleport's [经过] is unconditional
  is settled by 回合階段&註釋 `B41` 「空降至目标地格并在该格依次触发[经过],
  [重叠],和[结算]」 (TEST-FINDINGS records the same-tile carve-out as wrong).
  The [传送] as *main* move settle shape is an open ruling (TEST-FINDINGS §6
  item 1) and is not checked here.
* **Executable:** `rules_spec::global::movement::g_move_05` (plus
  `move_path`).
* **Scenario:** `tests/spec_g_move.rs::g_move_05_teleport_path_is_only_the_end`.

---

## G-MOVE-06

* **ID:** `G-MOVE-06`
* **Source:** `rulebook-doc.md:67` + `rulebook-doc.md:151`
* **Quote:** 「[CiRCLE奖励]：[获得]2000资金或抽1张卡。」 / 「[经过]CiRCLE且
  [移动起点]不为CiRCLE时获得[CiRCLE奖励]。」
* **Trigger:** a walk / [传送] whose [路径] contains the CiRCLE tile.
* **Condition:** 移动起点 is not CiRCLE; no `noReward` prop, no field card,
  mover not [除外].
* **Effect:** a [CiRCLE奖励] step is reached (the money-or-card choice, or a
  forced card under [晕眩]). When it pays money, the player gains exactly
  2000; when it draws, the player draws exactly 1.
* **Frame:** n/a (the reward itself is a money / hand write).
* **Rulings & OPEN:** [晕眩] 「无法收付款」 forces the card half (回合階段&
  註釋 `C44`); that path is scenario-tested in the rb_* suite. A
  `circleAffected` rewrite replaces the payout and is skipped here. The
  「iff」 -- no reward when 移动起点 == CiRCLE -- is checked when the window
  is clean and the step starts a move (not a prompt-answer continuation).
* **Executable:** `rules_spec::global::movement::g_move_06`.
* **Scenario:** `tests/spec_g_move.rs::g_move_06_reward_iff_pass_and_start_not_circle`.

---

## G-MOVE-07

* **ID:** `G-MOVE-07`
* **Source:** `rulebook-doc.md:132`
* **Quote:** 「移动阶段中需进行1次[主要移动]。基础[主要移动]为**投掷1d20**，
  在地图上顺时针移动投掷结果数量的格子，到达[移动终点]后需进行[结算]。」
* **Trigger:** a `roll` walk segment whose `dice` face is the 1d20 and whose
  `value` equals that face (no extra / reversed steps).
* **Condition:** the face `d` is in `1..=20`; one unmodified clockwise
  segment.
* **Effect:** `to == (from + d) % n` -- clockwise, `d` tiles.
* **Frame:** n/a.
* **Rulings & OPEN:** cards that add / fix / reverse the distance (冰川纱夜
  (2), 白金燐子 (2), 美竹兰 (2), …) rewrite the dice table or the step count.
  The per-act clause only pins the clockwise arithmetic when `value == dice`
  (distance equals face); a face outside 1..=20 is a card rewrite and is not
  a base-1d20 violation. A `roll` that became a [传送] logs `teleport`, not
  `roll`. The 「base is 1d20」 shape is scenario-tested with loaded dice.
* **Executable:** `rules_spec::global::movement::g_move_07`.
* **Scenario:** `tests/spec_g_move.rs::g_move_07_main_move_is_1d20_clockwise`.

---

## G-MOVE-08

* **ID:** `G-MOVE-08`
* **Source:** `rulebook-doc.md:207`
* **Quote:** 行动阶段 7 「一回合只能触发一次［主要移动］效果,多次触发［主要
  移动］时无效」
* **Trigger:** a `roll` command (`StepAct::Act` with `act == "diceRollBefore"`).
* **Condition:** the turn's [主要移动] has already been spent, or the
  engine refuses the roll for another reason.
* **Effect:** a refused / extra [主要移动] is a pure no-op -- no walk /
  teleport / overlap events, and every player's `pos` is unchanged. At most
  one `roll` walk event per window.
* **Frame:** all `pos:*` unchanged when the roll logs no walk.
* **Rulings & OPEN:** the book does not say which of two simultaneous
  [主要移动] effects wins; only that the second is 无效. A card that 「视为
  主要移动」 after one already ran is that second effect.
* **Executable:** `rules_spec::global::movement::g_move_08`.
* **Scenario:** `tests/spec_g_move.rs::g_move_08_second_main_move_does_nothing`.

---

## G-MOVE-09

* **ID:** `G-MOVE-09`
* **Source:** `rulebook-doc.md:132` + `rulebook-doc.md:141`
* **Quote:** 「到达[移动终点]后需进行[结算]。[主要移动]和所需[结算]完成后
  进入结束阶段。」 / 「技能或卡所导致的[主要移动]会表明到达终点后是否[结算]。」
* **Trigger:** a completed [主要移动] (or a card-driven move that names a
  settle flag).
* **Condition:** the move settles (the default for the base move).
* **Effect:** the [移动终点] [结算]s (行动阶段 15 「触发地块/落点效果」).
  When the effect says 不[结算], no settle runs.
* **Frame:** n/a.
* **Rulings & OPEN:** what counts as 「表明…是否[结算]」 on a card is the
  card's own wording; the scenario uses `通用:@Tsugu ycm`'s 「并不[结算]」.
  A no-settle move still runs the movement tail (行动阶段 13 [重叠]) --
  that is `G-MOVE-10`, not this clause.
* **Executable:** scenario-only. The per-act window cannot tell a settle
  that a card suppressed from one that never ran without naming the card;
  the determined parts (settle follows the default move; a 「不[结算]」
  move settles to nothing) are pinned by the scenario.
* **Scenario:** `tests/spec_g_move.rs::g_move_09_settle_follows_the_main_move`.

---

## G-MOVE-10

* **ID:** `G-MOVE-10`
* **Source:** `rulebook-doc.md:213`
* **Quote:** 行动阶段 13 「移动终点触发[重叠] 原地[传送]/移动(移动0格)时触发
  [重叠]」
* **Trigger:** a completed move (walk of any length, or [传送]) that ends on
  a tile another surviving player occupies.
* **Condition:** no card suppresses [重叠] (青叶摩卡's 「不引发其他被动效果
  （如玩家重叠）」 and friends -- skipped when a field card is in play).
* **Effect:** an `overlap` event names the mover and the sharing player(s).
  A 0-step move and a same-tile [传送] fire it too.
* **Frame:** n/a.
* **Rulings & OPEN:** the number of `overlap` legs is one per other player on
  the tile. The book's example is 要乐奈 (3) 「你与其他玩家重合时」.
* **Executable:** `rules_spec::global::movement::g_move_10`.
* **Scenario:** `tests/spec_g_move.rs::g_move_10_overlap_at_the_end_tile`.

---

## G-MOVE-11

* **ID:** `G-MOVE-11`
* **Source:** `rulebook-doc.md:207-214`
* **Quote:** 行动阶段 7–14: 主要移动阶段前 → 掷骰前 → 掷骰 → 掷骰后 →
  移动前 → 移动时/[经过] → 移动后/[重叠] → 结算前/[移动终点] → 结算
  (rows 12–15: 「移动起点不触发[经过],移动终点触发[经过]」 / 「移动终点触发
  [重叠]」 / 「触发地块/落点效果」).
* **Trigger:** a complete main-move window (a walk that reaches its end, or
  one split by a mid-route prompt).
* **Condition:** vanilla (no field cards).
* **Effect:** the observable tail orders: the walk (`roll`/`move`) precedes
  every `overlap`, and every `overlap` precedes the settle lines
  (`log.land*` / `log.rent`). A [经过] that is not a walk (the CiRCLE
  reward) sits after the walk that reached it and before the landing's
  settle.
* **Frame:** n/a.
* **Rulings & OPEN:** the pre-move windows (rows 7–11) raise hooks, not log
  lines; only the observable tail is ordered here. [强制停下] is truncated
  -- OPEN-强制停下 -- and is not part of this ordering.
* **Executable:** `rules_spec::global::movement::g_move_11`.
* **Scenario:** `tests/spec_g_move.rs::g_move_11_walk_then_overlap_then_settle`,
  `g_move_11_circle_reward_sits_between_walk_and_settle`.

---

## Open questions

* **OPEN-强制停下** -- 「[强制停下]」 definition is truncated
  (`rulebook-doc.md:93`: 「8. [强制停下]：部分效果所导致的」). Clauses that
  mention it specify only what remains. (TEST-FINDINGS also leaves the
  path-hook order vs a `SettleBefore` end-rewrite as an open ruling; the
  engine currently lets the forced stop win.)
* **OPEN-经过-0步** -- when [移动起点] == [移动终点] the quote puts the tile
  in the [路径] (「如果两点相同则包括」), but 行动阶段 13 names only [重叠]
  for 「原地[传送]/移动(移动0格)」 and says nothing about [经过]. Whether a
  0-step move also fires [经过] on the standing tile is undetermined. The
  engine currently fires [重叠] only (its own `TODO(规则书)`).

## Coverage gaps

| clause | missing seam | what a seam would need |
|---|---|---|
| G-MOVE-04 (mid-walk recompute) | no sheet card rewrites the [移动终点] **during** the walk (「移动时」). The sheet's rewriters (`RAS:（LOCK）追逐梦想的步伐`（2）, 丰川祥子（CRYCHIC）(2)) are timed 「在触发结算前」 / 「主要移动结束时」 -- after the walk. | A card whose [经过] body writes the plan's destination immediately (mid-walk), so the walk's later tiles can be observed dropping out of the [路径]. |
| G-MOVE-09 (settle flag on card-driven 主要移动) | the per-act window cannot tell a settle a card suppressed from one that never ran without naming the card. The default (settle follows) and the 「不[结算]」 branch are scenario-pinned (`通用:@Tsugu ycm`). | An event that names the move's `resolve` flag on the move tail. |

Partial coverage (which part): G-MOVE-03 asserts the end settles and that
there is only one settle line (start does not settle); it does not observe
a per-tile [经过] effect on a mid-path tile (no generic per-tile log). The
per-tile [经过] is exercised via CiRCLE (G-MOVE-06) and the LOCK scenario.

## Findings

(Empty -- no genuine engine discrepancy was left red. G-MOVE-04's earlier
red was a **test misreading** of LOCK's timing (「在触发结算前」), not an
engine gap; the scenario now pins the determined part. Scenario failures
that are genuine engine gaps land here with the clause ID and one line.)
