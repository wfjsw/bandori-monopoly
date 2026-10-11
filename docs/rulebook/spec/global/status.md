# G-STATUS -- [停留] / [晕眩] / [除外] / [不可阻挡] / [无法移动] 及其层数

Unit 3 owns this file and `rules/spec/src/global/status.rs`. Append clauses;
do not edit other areas.

Sources: live sheet (`docs/rulebook/cards.json`, `data/characters.json`,
`data/bands.json`) > `docs/rulebook/rulebook-doc.md`. `rulebook.txt` is not
authoritative. TEST-FINDINGS §6 rulings unless superseded by newer sheet text.

The book spells **眩晕** and **晕眩** interchangeably; both mean the same
status. The engine also has a timed `stunStart` variant (expires at turn
start) that is not named in the book -- see G-STATUS-01.

---

## G-STATUS-01

* **ID:** `G-STATUS-01`
* **Source:** `rulebook-doc.md:96-98`
* **Quote:** 「[停留]：…重复受到该效果时可叠加，玩家的每回合结束时移除一层。」
  「[晕眩]：…重复受到该效果时可叠加，玩家的每回合结束时移除一层。」
  「[除外]：…该效果生效时无法重复获得该效果，玩家的每回合开始前移除一层。」
* **Trigger:** every observed input, on the settled `post` world.
* **Condition:** the engine's `stay` / `stun` / `stunStart` / `exile`
  counters.
* **Effect:** every such counter is `>= 0`.
* **Frame:** n/a (pure read).
* **Rulings & OPEN:** "可叠加" + "移除一层" makes the counters layer counts; a
  layer count that went negative would mean a decay ran one too many times.
  `stunStart` is the engine's [晕眩] variant that expires at turn start
  (`Tick::TurnStart`); the book only names one [晕眩], so the mapping is
  engine-side. What each status *forbids* is G-STATUS-02+.
* **Executable:** `rules_spec::global::status::g_status_01`.

---

## G-STATUS-02

* **ID:** `G-STATUS-02`
* **Source:** `rulebook-doc.md:95`
* **Quote:** 「[无法移动]：处于该状态时[主要移动]的[移动终点]视为当前所在格子且不[结算]。无法使用非[传送]的移动效果（如果因效果随机到非[传送]的移动效果则抵消该效果）。」
* **Trigger:** a `roll` (the [主要移动]) by a player who is [无法移动], or a
  non-[传送] movement effect targeting them.
* **Condition:** in `pre`, the player has [停留] (unless [不可阻挡]), [晕眩],
  or [除外] -- any of which grants [无法移动] (items 11-13).
* **Effect:** the [主要移动] does not leave the current tile and does not
  [结算]; a non-[传送] movement effect is cancelled.
* **Frame:** n/a (the move is a no-op).
* **Rulings & OPEN:** the 行动阶段 rows phrase the same fact as a skip
  (row 7 「停留效果发动：跳过主要移动阶段」). A refused `roll` / a skipped move
  phase is the observable form of 「[移动终点]视为当前所在格子且不[结算]」.
  [传送] is *not* blocked by this item (the second sentence only bars
  non-[传送] effects). A [停留] gained *mid-turn* does not raise the skip
  latch (OPEN-停留-中途); the clause only fires when `skip_move` is already
  set.
* **Executable:** `rules_spec::global::status::g_status_02`.

---

## G-STATUS-03

* **ID:** `G-STATUS-03`
* **Source:** `rulebook-doc.md:96`
* **Quote:** 「[停留]：处于该状态时[无法移动]。重复受到该效果时可叠加，玩家的每回合结束时移除一层。」
* **Trigger:** any act; on turn end of the layer's owner.
* **Condition:** `stay` layers on that player.
* **Effect:** while `stay > 0` the player is [无法移动]; repeated grants add
  layers; at end of their own turn one layer is removed.
* **Frame:** only `state:P*:stay` moves on a pure decay.
* **Rulings & OPEN:** 行动阶段 row 7 (「停留效果发动：跳过主要移动阶段」) is
  the skip form of the [无法移动] consequence (see G-STATUS-02). A grant
  logged in the same window as the decay may offset the -1. A [清除] that
  zeros layers is a different clause (card text).
* **Executable:** `rules_spec::global::status::g_status_03`.

---

## G-STATUS-04

* **ID:** `G-STATUS-04`
* **Source:** `rulebook-doc.md:97`
* **Quote:** 「[晕眩]：处于该状态时[无法移动]，无法使用[主动]技能和手卡的[手]效果，无法收付款。」
* **Trigger:** a skill press or hand-card play by a stunned player, or a money
  leg to / from one.
* **Condition:** `stun + stunStart > 0` on that player in `pre`.
* **Effect:** the skill / [手] play does not run; no money moves to or from
  them (the pay is blocked).
* **Frame:** n/a (the act is a no-op).
* **Rulings & OPEN:** cards that print 「可在眩晕时打出」 (e.g. MyGO:壱雫空) and
  skills that print 「在[晕眩]状态下使用」 are card-text exceptions and are out
  of scope of this global clause. **OPEN-晕眩-自得** (TEST-FINDINGS §6.9): does
  「无法收付款」 also block a card's own 「获得」 to the player that same card
  body just stunned? Engine now lets the gain land.
* **Executable:** `rules_spec::global::status::g_status_04`.

---

## G-STATUS-05

* **ID:** `G-STATUS-05`
* **Source:** `rulebook-doc.md:97` + `rulebook-doc.md:204`
* **Quote:** 「重复受到该效果时可叠加，玩家的每回合结束时移除一层。」
  「眩晕效果发动：跳过经营和主要移动阶段」
* **Trigger:** end of the stunned player's turn; their turn start while
  stunned.
* **Condition:** `stun` layers.
* **Effect:** one layer removed at end of their own turn; their turn skips
  运营/出牌 and 主要移动 (the whole active window).
* **Frame:** only `state:P*:stun` / `stunStart` move on a pure decay.
* **Rulings & OPEN:** the book's 眩晕 = 晕眩. `stunStart` is the engine's timed
  variant (expires at turn start); same non-negativity and same skip. The
  auto-skip is logged as `log.stunned_skip`.
* **Executable:** `rules_spec::global::status::g_status_05`.

---

## G-STATUS-06

* **ID:** `G-STATUS-06`
* **Source:** `rulebook-doc.md:98`
* **Quote:** 「[除外]：处于该状态时[无法移动]，不能被[指定]，无法使用[主动]技能和手卡的[手]效果，无法收付款；玩家棋子不失为存在于任何格子上。」
* **Trigger:** a skill press / hand play by an exiled player, a [指定] targeting
  them, or a money leg to / from them.
* **Condition:** `exile > 0` on that player in `pre`.
* **Effect:** the skill / [手] play does not run; they are not a legal [指定]
  target; no money moves to or from them; the pawn counts as on no tile.
* **Frame:** n/a (the act is a no-op).
* **Rulings & OPEN:** 「不失为」 is the book's spelling of 「不视为」. The
  The per-act half is the skill / [手] / money gate. 「不能被[指定]」
  (target legality) and 「玩家棋子不失为存在于任何格子上」 (pawn on no tile)
  are **not** executable here -- no generic [指定] target event is observable
  on `Step`, and "not on a tile" is a presence predicate the engine keeps in
  `players_on`. See Open questions. The engine routes tile income of an
  exiled player to a redirect (无路矢); the general 「无法收付款」 still says
  they do not receive it.
* **Executable:** `rules_spec::global::status::g_status_06`.

---

## G-STATUS-07

* **ID:** `G-STATUS-07`
* **Source:** `rulebook-doc.md:98` + `rulebook-doc.md:201`
* **Quote:** 「该效果生效时无法重复获得该效果，玩家的每回合开始前移除一层。」
  「负面效果：移除一层[除外]」
* **Trigger:** a grant of [除外] while already exiled; the player's own turn
  start.
* **Condition:** `exile > 0` in `pre`.
* **Effect:** a second grant does not raise the counter; at their turn start
  (before the turn proper) one layer is removed.
* **Frame:** only `state:P*:exile` / `exileTo` move on a pure decay.
* **Rulings & OPEN:** one effect may still grant several layers at once
  (MyGO:无路矢 「获得2层[除外]」) -- 「无法重复获得」 bars a *second*
  acquisition while the first is active, not a multi-layer first grant. A
  grant after the decay leaves 0 is a fresh acquisition.
* **Executable:** `rules_spec::global::status::g_status_07`.

---

## G-STATUS-08

* **ID:** `G-STATUS-08`
* **Source:** `rulebook-doc.md:98`
* **Quote:** 「处于该状态时玩家技能和[场地]上的[持续]效果停止生效。获得该状态的效果会写明状态结束后在玩家回合开始时要[传送]到的格子。」
* **Trigger:** any window in which an exiled player would fire a skill /
  [持续] hook, and the turn start where their [除外] ticks out.
* **Condition:** `exile > 0` in `pre` (or ticking to 0 at this turn start).
* **Effect:** no player-skill or tile-[持续] body runs on their behalf; when
  the last layer ticks out, they [传送] to the tile the granting effect wrote
  (`exileTo`).
* **Frame:** n/a.
* **Rulings & OPEN:** the book states a convention on the *granting* effect's
  text (it must name the return tile), not a constraint the engine can fail;
  the checkable half is the return [传送] on expiry (the 「技能和[持续]效果
  停止生效」 half is not executable here -- skill/`[持续]` activations are not
  distinguished on `Step` as "on behalf of an exiled player"). A grant with
  no `exileTo` entry has no named tile -- **OPEN-除外-归位** for whether that
  is legal.
* **Executable:** `rules_spec::global::status::g_status_08`.

---

## G-STATUS-09

* **ID:** `G-STATUS-09`
* **Source:** `rulebook-doc.md:99`
* **Quote:** 「[不可阻挡]：处于该状态时无视已存在的[停留]，[晕眩]效果也不会获得新的层数；无法获得新的[除外]层数。[停留]，[晕眩]，[除外]在适当时机依旧掉层。处于该状态时可选择受到的[传送]，[强制移动]，[强制停下]效果是否生效。」
* **Trigger:** any act while `unstoppable > 0`.
* **Condition:** `unstoppable > 0` in `pre`.
* **Effect:** existing [停留] does not skip the move; no new [晕眩] or [除外]
  layers are gained; existing layers still tick at their book timing;
  [传送]/[强制移动]/[强制停下] are optional to the receiver.
* **Frame:** n/a.
* **Rulings & OPEN:** the choice half (「可选择…是否生效」) is a player
  decision -- not a fixed post-condition, so this clause checks the
  *prohibition* half (no new layers) and the *non-skip* half. Layer decay is
  G-STATUS-03 / 05 / 07 (「依旧掉层」).
* **Executable:** `rules_spec::global::status::g_status_09`.

---

## G-STATUS-10

* **ID:** `G-STATUS-10`
* **Source:** `rulebook-doc.md:100`
* **Quote:** 「[异常移动效果]：包括[传送]，[停留]，[晕眩]，[除外]，[强制移动]，[强制停下]，反方向移动。」
* **Trigger:** any window that names an [异常移动效果] (e.g. a card that says
  「不受异常移动效果影响」 or 「受到[除外]以外的异常移动效果」).
* **Condition:** n/a -- this is a set-membership definition.
* **Effect:** the set is exactly the seven named members. Nothing outside it is
  an [异常移动效果]; every named member is one.
* **Frame:** n/a.
* **Rulings & OPEN:** the book uses 「包括」 (includes), which could be
  open-ended. Read with the closed list that follows and with R:选择自己的
  舞台's 「[除外]以外的异常移动效果」 (which only makes sense as a closed set),
  the set is exactly these seven. **OPEN-异常移动效果-封闭** if a later sheet
  adds members.
* **Executable:** `rules_spec::global::status::g_status_10`.
* **Constant:** `rules_spec::global::status::ABNORMAL_MOVE_EFFECTS`.

---

## Scenario tests

`rules/tests/tests/spec_g_status.rs` drives clean arranged scenarios (no clearing
cards in the window) for the exact decay timings, the stun skip, the exile
return [传送], and the [不可阻挡] prohibitions.

---

## Iff coverage

Addendum standard: exact effect + exact frame + trigger boundary + duration +
mutation tests.

| Clause | iff | notes |
|---|---|---|
| G-STATUS-01 | full | invariant; `layers_nonneg_verifier_rejects_mutants` |
| G-STATUS-02 | partial | roll half: driver + mutants + no-op; non-[传送] half is a gap |
| G-STATUS-03 | full | exact decay + `state:P0:stay` frame + no-op + mutants |
| G-STATUS-04 | partial | money gate: mutants + no-op + rent drivers; skill-press half is a gap |
| G-STATUS-05 | full | exact decay + row 4 skip + mutants |
| G-STATUS-06 | partial | money + [手] drivers; [指定] / pawn-on-no-tile are gaps |
| G-STATUS-07 | full | exact decay + no re-acquire + mutants |
| G-STATUS-08 | partial | return [传送] exact + frame + mutants; skills/`[持续]` stop is a gap |
| G-STATUS-09 | partial | prohibition half + mutants + no-op; choice half is a gap |
| G-STATUS-10 | full | closed-set definition + `abnormal_move_effect_set` |

**Full iff: 5 / 10. Coverage gaps: 7** (table below).

## Coverage gaps

Uncovered halves (addendum item 6). Each names the missing seam.

| Clause | Missing | Seam needed |
|---|---|---|
| G-STATUS-02 | second sentence: 「无法使用非[传送]的移动效果」 | an observable non-teleport movement-effect event (or a card that randomizes into one) so the cancel can be asserted |
| G-STATUS-04 | [主动] skill press while stunned (only [手] play is driven) | a `skill` act on a stunned seat via `Table::skill` (seam exists; not yet driven) |
| G-STATUS-04 | card-text exceptions (「可在眩晕时打出」) boundary | a card with the waiver *and* one without, same act |
| G-STATUS-06 | 「不能被[指定]」 | a [指定] target-pick event on `Step` |
| G-STATUS-06 | 「玩家棋子不失为存在于任何格子上」 | a presence predicate (`players_on`) exposed on the observed window |
| G-STATUS-08 | 「玩家技能和[场地]上的[持续]效果停止生效」 | skill / tile-[持续] activations attributed to an exiled seat |
| G-STATUS-09 | choice half 「可选择受到的[传送]，[强制移动]，[强制停下]效果是否生效」 | a forced-move prompt whose every option is drivable |

## Findings

Classification (addendum item 7): **LESS** = engine does less than the book;
**MORE** = does more / leaks / fires when it should not; **WRONG** = different
amount / target / timing.

* **G-STATUS-02** — **WRONG** (also MORE).
  * Book (`rulebook-doc.md:95`): 「[主要移动]的[移动终点]视为当前所在格子且不[结算]」;
    行动阶段 row 4: 「眩晕效果发动：跳过经营和主要移动阶段」.
  * Engine: a [晕眩]/[除外] in force at the `roll` still walks to another tile
    and settles (`log.roll`, `log.land_unowned`). `skip_move` is latched only
    for [停留] at turn start; mid-运营 [晕眩] does not stop the move.
  * Destination is not the current tile (WRONG) and the move+settle run at all
    (MORE). Failing tests: `stun_in_force_at_roll_does_not_walk`, soak.

* **G-STATUS-06** — **MORE**.
  * Book (`rulebook-doc.md:98`): 「[除外]：…无法收付款」.
  * Engine: the [除外] payee is not gated (only [晕眩] is). Rent of 760 lands
    on an `exile=1` owner (`log.rent`). MyGO:无路矢's 「[除外]期间本应获得的
    格子收入由此前指定的那名玩家获得」 is a *redirect*, not a licence to pay
    the exiled seat.
  * Failing tests: `exile_owner_cannot_receive_rent`, soak.

Contrast (green): `stun_owner_cannot_receive_rent` -- the [晕眩] payee *is*
gated.

## Open questions

* **OPEN-晕眩-自得** (TEST-FINDINGS §6.9): does 「无法收付款」 block a card's
  own 「获得」 to the player that same card body just stunned? Example:
  Mujica:心の雨's fallback does 「眩晕」 then 「获得1000资金」. Engine now: the
  gain lands. Verifiers skip a `gain` in a window that also granted the status.
* **OPEN-除外-归位**: is a [除外] grant with no named return tile legal? The
  book says the granting effect 「会写明」 the tile (a text convention). The
  engine's `exileTo` can be absent / -1.
* **OPEN-异常移动效果-封闭**: 「包括」 could be open-ended. Clauses treat the
  seven-item list as closed (see G-STATUS-10).
* **OPEN-停留-中途**: a [停留] gained mid-move does not stop the current move
  (TEST-FINDINGS §6 / glossary 51). G-STATUS-02 only fires when the skip latch
  is held or the status is [晕眩]/[除外].
