# Rulebook black-box test findings

Black-box integration tests check every card, character skill and band skill
against the live rule sheet. The sheet is the Google Sheet with cards /
characters / bands tabs, and it is the definitive source. The tests live in
`crates/game-rules/tests/rb_*.rs`, on the shared harness `tests/common/mod.rs`.
They were written without reading `rules/cards/**` or `rules/skills/**`.

* `cargo test -p game-rules --test rb_<group>`: everything green.
* `cargo test -p game-rules --test rb_<group> -- --ignored`: runs the
  **open discrepancies**. Each ignored test keeps the rulebook's assertion, and
  its `#[ignore = "…"]` reason says what the engine does instead.
  * `DISCREPANCY:` marks an engine or card gap.
  * `RULING:` marks an ambiguity waiting on a ruling (§6).
  * `CROSS-AGENT:` marks a test that needs a ruling before either side moves.
  * `TODO(ABI):` marks a hole the engine surface cannot express yet (§3).
* When a fix makes one pass, remove its `#[ignore]`. Never weaken the
  assertion.

## Status (2026-10-06)

Counts are a grep of the `#[test]` / `#[ignore = "…"]` **attributes** per file
in `crates/game-rules/tests/` (comment mentions excluded), so
`pass = tests - ignored` and `open = ignored`. The last full gate
(2026-10-06) was **34 targets, 761 passed / 0 failed / 93 ignored**.

The 93 `#[ignore]`s by kind:

| kind | count |
|---|---|
| `DISCREPANCY` | 75 |
| `RULING` | 13 |
| `CROSS-AGENT` | 1 |
| `TODO(ABI)` | 2 |
| other (q4 harness soaks) | 2 |

Per group (`pass` / `open`):

| group | pass | open |
|---|---|---|
| general (通用 + CiRCLE) | 59 | 2 |
| ppp | 43 | 0 |
| ag | 36 | 0 |
| pp | 53 | 4 |
| roselia | 34 | 5 |
| hhw | 27 | 0 |
| morfonica | 60 | 4 |
| ras | 62 | 3 |
| mygo | 78 | 6 |
| mujica | 51 | 0 |
| sumimi | 20 | 1 |
| crychic | 35 | 0 |
| rulebook | 38 | 0 |

The rest of the tree (same grep): `rb_cross_*` 111 tests / 42 open,
`rb_gap_*` 32 / 15, `rb_fuzz_found` 13 / 8, `rb_chain` 7 / 0, `rb_guards`
10 / 0, `rb_harness` 6 / 0, `rb_money` 4 / 1, `ruleset` 12 / 0,
`fuzz_interactions` 9 / 0, `live_match` 16 / 0, `q4_unintended` 3 / 2
(soak + report generator, not discrepancies),
`rb_unintended_found` 6 / 0. **Whole suite: 851 tests, 93 `#[ignore]`.**

A 0 in "open" means none were found, not that the group is verified correct.
See coverage gaps below.

### Resolved (summary)

Landed themes only; the per-item history is gone from this file.

* **Money pipeline** -- every money movement (print / delete / pay-player)
  opens a [反击] window, pre-split then per payer×payee pair.
* **Clause-89 chain order** -- the ring opens at the next seat; a round
  collects counters to the same timing (§10).
* **Tile rules phase 3** -- CiRCLE reward start-tile check, teleport / walk /
  0-step [经过] then [重叠] at the target, `prop::RENT_HOUSES` virtual
  house-count override.
* **Match-start and per-draw hooks** -- `game_start_hooks` reach every effect
  source; `ctx::draw` raises `drew` per single card.
* **Band crystals on the band skill's field instance** (ABI v29) and the
  **CardDef property map** (ABI v30) replacing the prose-keyed fields.
* **Scoped play flags** -- `TurnCtx.play_from_hand` and `TurnCtx.extreme` are
  armed per play and restored.
* **[不可阻挡] exempts self-chosen moves**; the `abnormal` outcome hook fires
  for self-applied abnormals.
* **Roll windows (ABI 33)** -- card and fire-pot rolls raise the
  「掷骰结算前」 [反击] link.
* **Q4 unintended leaks** -- `play_from_hand` / `turn.abnormal` / `targeted`
  scoping; all six `rb_unintended_found` repros green.
* **Fuzz findings** -- `fire_over_cap`, `negative_status_leak`,
  `money_ledger_gap`, `settle_loop_stuck`, `money_depth_opens_windows`
  (loud safety cap, `Cx::reentrant_hooks`).
* **Sheet sync of 2026-10-06** -- 21 drifted cells into `cards-sheet.csv`;
  all landed except NNM's new (2) (card bug, §4).
* **User rulings of 2026-10-06** -- implemented and green (see the Decided
  list in §6).
* **Card-side wave** -- per-uid crystal moves, match-start 「初始N」 pots and
  fan init, exclusives place/recycle, roll-window wrappers (热气球演出,
  寄于指尖), Ringing Bloom / 椎名立希 (1) / 丰川祥子 (1) / CRYCHIC (2) /
  乐奈 tile lookup fixes.
* **Rename:** [反击] is `counteract` throughout (ABI v28).

## Rulebook doc check (2026-10-06)

The live Google Doc 规则书 tab is the ground truth for the **core rules**.
Snapshot: [rulebook-doc.md](rulebook-doc.md). Clause-by-clause engine verdicts
are in [COVERAGE.md](COVERAGE.md) §Rulebook doc (2026-10-06). Black-box tests:
`crates/game-rules/tests/rb_rulebook.rs` (38 tests, 0 `#[ignore]`).

Still open from that check:

* **The 时点流程 tables are missing from `data/rules.txt`.** It stubs 开始游戏
  / 回合流程 / 行动阶段 / 支付阶段 with 「（详细时点表见规则书原文）」. The
  doc's tables carry 8 game-start steps, 11 turn-flow steps, 19 action windows
  and 7 payment windows. Sync `data/rules.txt` to add those tables (and the
  bold / red markers) without rewording anything -- the numbered clause lists
  keep their numbering, so no citation moves. Do the sync after the tile-rules
  work settles.
* **Missing timing points** (structure, not assertable black-box): the
  engine has no trigger kind for 行动阶段 3 / 6 / 11 (回合开始后 / 经营阶段后 /
  移动前) or for 支付阶段 3 (「支付计算后」). The effects those windows gate are
  present under other names; the gap matters only for cards that hook a window
  the engine never raises.
* **支付阶段 1 / 5 and 7 shaping.** One `effect` raise carries window 5's
  cancel/retarget power at window 1's position; `payChoose` / `payAt` are
  unnamed extra stages; `paid` fires only on a payer-side loss and is distinct
  from `payAfter`. Card-facing order is pinned by `rb_gap_money::g01`/`g02`.

Ambiguities from this check that still need a ruling live in §6 (RiNG rent,
agent 「同色」, RiNG mortgageability, 强制购买 pay-hook scope).

## Open items

### 3. Missing card/ABI capabilities (TODO(ABI) placed)

Two agents are on the remaining ABI holes right now (**in progress (ABI
batch)**); entries below marked that way are theirs.

* ~~网络链接异常: cancel *one* designation.~~ **Done (2026-10-06, Group A).**
  The **static targeting query** (`ctx::designations`, C#
  `H.Db.Card(id).Targeting` + `H.Others`, declared via `prop::DESIGNATES`) and
  **per-pair cancel** (`ctx::cancel_designation` / `designation_cancelled`,
  C# `play.Tags["immune"+seat]`) landed: one designation drops, the rest of the
  play's designations land. `rb_money::per_pair_cancel_drops_one_designation`,
  `rb_gap_money::g03_net_error_drops_one_leg_of_a_split` and
  `rb_general::net_cancels_one_target_not_all` are green and un-ignored.
  The share is the **pre-drop** figure (RULING; `g04`'s halving-vs-cancel
  order stays RULING).
* ~~育美 (2) / `hagumi_marks` (2): count gains during the turn.~~
  **Done (2026-10-06, Group A).** 「资金变动」 (支付阶段 7) merges into
  `payAfter`, which now fires on **any** money change (a `gain` and a
  `gain_fixed` included; `paid` stays the payer-loss [反击] window). A
  per-player **turn gain counter** (`ctx::gains_this_turn`, reset at each turn
  start) is what 「当前回合内你每获得过一次资金」 reads, so a pure-hand (2) sees
  the count with no field stand-in.
* ~~壱雫空: the host routine needs a merged guest-state view.~~
  **Done (2026-10-06, Group A).** `Cx::overlay_guest_state` /
  `restore_guests_to`: the guest's pending player state (status + money) is
  overlaid on the live world for a host routine's duration and dropped again,
  keeping only the routine's own effects -- so a status clear reaches
  `can_pay` without double-counting on the replay. 壱雫空's money now runs
  through `transfer` / `gain` (the full money pipeline, payAdd/payMul/payChoose
  + the `pay` [反击] window). Clearing a [停留] to 0 also un-skips the turn's
  move (`give_stay`).
* ~~无路矢: honour the `exileMain` slot so the expiry teleport runs.~~
  **Done (2026-10-06, Group A).** The exile tick reads and consumes
  `state_key::EXILE_MAIN`: the return teleport runs as the main move and
  `TurnCtx::main_moved` is set, so the player cannot also roll
  (`err.roll_main_used`).
* ~~`tomoe_savior`, `mutsumi_never`, `sakiko_lead` (`m27_sakiko_leads`),
  `umiri_card`, `pareo_far`: remaining card-side hooks.~~
  **Done (ABI v35, 2026-10-06, Group B).** One general skill / band / follow
  surface landed, not five one-offs:
  * `ctx::band_skill` / `ctx::character_skill` / `ctx::band_skills` /
    `ctx::add_band_skill` -- the skill **attachment surface** (C#
    `H._fx[i].bands` / `.skill` / `H.MakeBand`). `extra` is a 「拿取」 copy:
    「相同乐队技能卡的效果不可叠加」 (`add_band_skill` refuses an id already
    attached) and 「不视为那个乐队的角色」 (`in_band` still reads only the
    character).
  * `ctx::invoke_skill(player_id, skill_id)` -- run a skill rule's press entry
    (`On::Play`) for a player, nested like `play_card` (C#
    `BandCrychic.TransformNow()` / `SkillPareo -> Offer()`). No `skillUsed`
    raise -- that is the player's own press.
  * `ctx::raise_bought(player_id, tile)` -- C# `f.Bought(i, t)`: a card that
    handed a deed over (tomoe_savior) announces the acquisition so the `bought`
    hook chain (Afterglow's free house, ...) hears it.
  * `ctx::plan::add_follower(player_id)` -- 「使你的下次主要移动结果对那些玩家
    一起执行，你先触发结算，此后其他玩家按行动顺序依次触发结算」: the engine
    replays the move's result for each follower after the mover settles, in the
    recorded order, carrying the same plan / `pay_factor`.
  * `m27_sakiko_leads` is **green and un-ignored**.
  * Remaining `TODO(规则书)` on the five: mutsumi_never's C# narrows (2) to
    `BandCrychic` specifically (the port takes the wider 「乐队技能」 reading);
    umiri_card's 「置入弃牌堆」 for a *taken band-skill card* has no home in the
    attachment model (C# `Drop` only detaches it) -- detached, not discarded.
* Anon Tokyo uses a field stand-in and slots, because the original
  attachment model / mark notes are not readable.

### 4. Remaining per-card discrepancies

The `#[ignore]` reasons are authoritative. This list is the current
`#[ignore` set (2026-10-06 working tree) -- anything not named here has
either no test or a green one (see §7 and [COVERAGE.md](COVERAGE.md)).

* **general** (2)
  * 网络链接异常 negates a whole multi-target card (see §3, TODO(ABI)).
  * The CiRCLE band (2) doubling is unported.
* **pp** (4)
  * 丸山彩's (2) PayChoose hook still prompts under 初次演出事故 -- the
    `skillBlock` token is set but the guard/body `skill_blocked` check does
    not see it (hook-guard dispatch).
  * 冰川日菜 (2) never rolls 1d4 nor borrows a skill (`skill.hinaLottery`
    stays -1).
  * 安可 doesn't counter 重叠的声音's self-teleport.
  * PP band (2) overflow crystals: a passive 「X大于拥有数时」 reaction to a
    fan-flip effect, and there is no flip-effect trigger to observe (§6).
* **roselia** (5)
  * 凑友希那 (3) stop-pot offers no prompt when a player [经过]s (two tests,
    one of them against 安可).
  * 凑友希那 (1) grants no fire pot on a RiNG landing.
  * 白金燐子 (2) has no pre-roll pot window (two tests).
  * 选择自己的舞台 opens no window on a self-inflicted [传送].
* **morfonica** (4)
  * 星月夜's even-roll crystal-removal offer never prompts.
  * 纯真振翅 never rolls after the teleport.
  * （NNM）稍微努力了一下 is a **card bug**: its CardDef declares two
    `On::Play` entries and the host dispatches only the first, so a hand
    play is refused (`nanami_effort_not_placed`). Two tests. See §6
    multi-activation.
* **ras** (3)
  * 游击演出 lands one tile past the chosen one, and its [特] never opens.
  * Repaint halves a shaped settlement payment only partially (the Tomorrow's
    Door surcharge joins at full price) -- §6.
* **mygo** (6)
  * 灯 不再迷茫: a crystal cannot pay a skill's fire cost, so the card is
    never [移除]d when its crystals run out (two tests).
  * A mid-turn [停留] doesn't stop the move (also an ambiguity -- §6).
  * 要乐奈's Space teleport spends the 3 fire but leaves the piece at the
    origin.
  * MyGO band (2)'s 2-crystal **draw** half is unreachable: the band skill
    declares two `On::Play` activations and `use_skill` runs only the first
    (the move-1 half). See §6 multi-activation.
  * `inter_yolo_pushes_haneoka_over_20` (CROSS-AGENT, §6 Y.O.L.O timing).
* **mujica** (0)
* **sumimi** (1)
  * Sweet Escape's 「收费标价」 gate is a ruling (§6).
* **cross** (`rb_cross_chain` 11, `rb_cross_move` 13, `rb_cross_tiles` 11,
  `rb_cross_status` 2, `rb_cross_long` 5): the `#[ignore]` reasons in those
  files name each case. The big ones: 骰子已经掷下 is placed twice and does
  not shut the windows (c14, g18); 无法将视线移开 cannot be played as a
  counter and does not force the counter-user to move (c16, l01, l04);
  花园多惠 (2)'s cancel window never opens (c24, c25, l05); Anon Tokyo's link
  needs a placed-tile stand-in (t01–t03); 笑容大游行 stack-overflows (t13,
  g30); 网络链接异常 still drops every designation (c05, g19);
  （祥子）带领着大家 (m27, §3 ABI batch); 是我自己的问题 (m28a/m28b).
* **gap** (`rb_gap_*` 15): the `RULING` ones are in §6. The `DISCREPANCY`
  ones are g18 (c14's shut-window), g19 (真奈's join + 网络链接异常),
  g21 (祥子 (1) absorbs nothing), g23 ([不可阻挡] still gains [除外] from
  无路矢), g28 (Maya (2)'s kept card does not raise `drew`), g30 (t13's
  stack overflow).
* **fuzz** (`rb_fuzz_found` 8): latent shapes the fuzzer found, filed with
  minimal repros -- see [COVERAGE.md](COVERAGE.md) "Fuzz coverage".
  Still open: `unknown_card_id`, `card_trap_reachable`, `determinism_break`,
  `save_restore_break`, `negation_not_total`, `multiply_order_matters`,
  `monotonicity_break`, `immunity_gap`.

### 6. Ambiguities needing a ruling

The single open-rulings list. [COVERAGE.md](COVERAGE.md)'s order-dependent
pair list and [CROSS-TESTS.md](CROSS-TESTS.md) §G12 point here; each entry
names the test that is `#[ignore = "RULING: …"]` / `CROSS-AGENT` on it.

**Decided (reference):** the user rulings of 2026-10-06 and the
sheet-superseded readings, all implemented and green. Kept for reference
only.

* User rulings 2026-10-06: THANKS PARTY's X does not count the user;
  壱雫空 「每清除一种效果」 is per effect type per affected player (user gains
  1000 extra per own type); 勇气展翅 pays land price + half the house cost;
  爱心义演 rounds the *payment* up to a multiple of 10 (halved first);
  祥，移动 「支付价格」 covers shaped payments; Hey Kids triggers when another
  player settles rent on my tiles; 秘密与青春的虹彩 grades come from a static
  normalised table; Returns sits beside the PPP card and neutralises only
  band (4).
* Sheet-superseded 2026-10-06: 黑色生日 「不多于1000」 is inclusive (1000
  pays 800); 羽丘 的 thresholds are 「至少为10/15/20」.

**Open rulings:**

The engine currently does whatever the "Engine now" line says. Each item
names the test that pins or records it. Dropped from this list: the items
the sheet answered and the engine now implements (same-tile [经过]/[重叠],
`(R+100)/2`, 晕眩 → card-only CiRCLE reward, 回合流程 rows 1-3) and the user
rulings above.

1. **Teleport as the main move.** Should a teleport that *is* the main move
   settle like a main-move landing, with the announce and the end-step buy
   offer? Or like any teleport, with an immediate `offer_buy`?
   * Engine now: it settles like any teleport, because `Move::new` defaults
     `main = false`.
   * Carrying `main` through breaks `rb_general::tsugu_lt26_choose_move_settles`,
     which pins the immediate offer.
2. **燃尽前的线香花火, last crystal.** Does removing the last crystal also
   grant 「获得一个额外回合」?
   * If it does, that extra turn's own end ticks one [眩晕] off. That would
     explain `rb_cross_long::l07_extra_turn_cascade` (stun 1, want 2).
   * The turn-end order (行动阶段 18 before 19) is already correct.
3. **（燐子）Ringing Bloom (3) X.** Is 「X为你收费格上的房屋数」 the count after
   (2) 「视为」 raises it, or the real count?
   * Engine now: the counted value, `prop::RENT_HOUSES`. `TODO(规则书)` in
     the card. Read with the real count, X would be 0 on exactly the tiles
     (2) raised. Test: `rb_cross_tiles::t10_ringing_bloom` (green; pins the
     counted value).
4. **Two halvings on one settle.** （香澄）大家我都喜欢哦 halves 「地租」 and
   祥，移动 halves 「支付价格」. Do they stack to ¼, or does the 支付阶段
   「支付减半/翻倍」 window apply once?
   * Engine now: ¼ (the two clauses modify different nouns). Test:
     `rb_mujica::saki_move_halves_a_forced_stop_and_pay` (green, pins 70).
5. **Sweet Escape 「收费标价」.** Does it mean purchase price or rent?
   * The tiles ±2 of CiRCLE sum to price 8200 (meets ≥2000) but to base
     rent 820 (fails).
   * Test: `sweet_escape_gate_requires_high_rent` (RULING).
6. **Tomorrow's Door (3) on a co-owned 星之鼓动山丘.** Does the co-owned hill
   count as 「[拥有者]拥有的格子」? Test: `rb_cross_tiles::t22_hill_coownership`
   (RULING, record-only).
7. **Drawing a card several effects react to.** Is a drawn effect card
   captured by one effect, or auto-played? Test:
   `rb_cross_long::l06_one_draw_several_effects` (RULING, record-only).
8. **Multi-activation selection.** A card with several `On::Play` entries
   (MyGO band (2)/(3), Mor:（NNM）稍微努力了一下): `use_skill` runs the first
   entry only, so a later activation is unreachable and a hand play can be
   refused. Ruling: one entry that branches on `is_placed()`, or a real
   selection prompt? Engine now: the first entry only.
   Tests: `rb_mygo::band_draw_for_two_crystals`,
   `rb_morfonica::nnm_discard_marks_draw_x` (both DISCREPANCY).
   NNM (2)'s new 「x次经过CiRCLE时的资金奖励」 waits on this. Sheet NOT FOUND
   (NNM's 「发动以下效果中的一个」 is a choice *inside* one entry).
9. **[晕眩] vs a card's own follow-on gain.** Does rulebook 49
   「无法收付款」 block a card's own 「获得」 to a player that the same card
   body just stunned?
   * Example: Mujica:心の雨's fallback does 「眩晕」 then 「获得1000资金」.
   * Engine now: the gain lands. Tests:
     `rb_mujica::heart_rain_fallback_stun_and_money`,
     `heart_rain_nobody_in_range_fallback`.

9. **RiNG rent.** 「地主拥有的 RiNG 数量 × ringMultiplier × 1d20」 and the
   「至少1个」 floor are `data/match_rules.json` notes, not rulebook text.
   「所有RiNG不可升级」 is the only RiNG clause in the book.
   `docs/TILES.md` `TODO(规则书)`; `rules/tiles/src/ring.rs`.
   (`game-core`'s `ring_rent_is_rings_times_multiplier_times_d20` pins the
   C# shape.) No black-box test.
10. **Agent 「同色」.** 「每种颜色拥有一个地产商格子」 never says whether
    `extraColor` / 「该格获得所有颜色」 widens the colour group.
    `docs/TILES.md` `TODO(规则书)`. Engine now: the board's `group`.
    Test: `rb_cross_tiles::t16_soyo_mixed_colors` (ignored).
11. **RiNG mortgageability.** 游戏流程 5 says 「抵押拥有的地契」 with no
    exclusion, but the engine refuses (`err.mortgage_ring`, `mortgageable`
    filters `kind != "ring"`). Is RiNG mortgageable, and if not where does
    the book say so? No test. (Sheet hint: `为新版做的改动` `A12` makes
    CiRCLE unmortgageable and PPP band says 星之鼓动山丘 「可正常抵押」;
    nothing names RiNG.)
12. **强制购买 pay-hook scope.** 基础[结算] 6.2 / 其他 6 say mortgage /
    redeem / forced purchase 「不受任何资金变动效果影响」. The engine
    implements that by moving the force-buy sum **outside** the money
    pipeline entirely, so no pay hook can target it, even one that
    「写明改动强制购买」. `docs/TILES.md` `TODO(规则书)`. Sheet hint
    (`为新版做的改动` `A20`) covers **抵押/赎回 only** and has an explicit
    「写明」 exception, so it does not settle the force-buy case.

**Still open, from the sheet text:**

* Which of the 技能 / 「改」 / extra-column texts is current for 丸山彩,
  上原绯玛丽 and 羽泽鸫? The 技能 column is what's implemented. NOT FOUND.
* The band tab's alternate Pastel✽Palettes 技能卡/角色卡/團卡 design: is it
  adopted or just a draft? NOT FOUND (clearly a draft; the live band skill
  is `团技能` `D7`).
* Sheet typos left out of `data/cards.json` (confirmed still present):
  “PERFECT“ / “FEVER!“ quote marks, 星月夜 「的的」, 游击演出 stray `\[特]`.

**Timing / order (CROSS-TESTS §G12 R-1..R-7 and the gap `RULING` tests):**

* **Y.O.L.O timing (R-4).** Now that Y.O.L.O is own-roll-only, when is its
  +1d4 added relative to the card's own roll and to other additive dice
  modifiers? `rb_mygo::inter_yolo_pushes_haneoka_over_20` is `CROSS-AGENT`
  on this. Engine now: the +1d4 is added to the card's own roll result.
  Sheet HINT (`新卡组卡` `C3` 「任意掷骰结算前…使结果增加1d4」) says additive
  before the roll is finalised, but not the order against other adders.
* **Fan-flip trigger.** PP band (2) is a passive 「你因任意原因受到将X个
  反面[P✽P粉丝]变正的效果且X大于拥有数时」 reaction to a fan-*flip*.
  There is no flip-effect trigger to observe, so the X>owned overflow never
  becomes crystals. Ruling: is a fan-flip its own timing, or is this folded
  into the fan-spend / fan-gain points?
  (`rb_pp::band_skill_overflow_to_crystals`, DISCREPANCY.) Sheet NOT FOUND.
* **R-1 money multiply × multiply:** Fire bird 1.5× vs Ave Mujica 1.5× vs
  摩卡 half vs HHW band double-pay. (`rb_gap_money::g01_*`,
  `rb_cross_tiles::t11_*`, both RULING.) Engine now: the multipliers
  compose in declaration order. Sheet HINT: all live in the one window
  `回合階段&註釋` `B30`/`C30`; no product/stack rule.
* **R-3 money cancel-one × split-share:** does X recompute after a
  designation is dropped? (`rb_gap_money::g03_*`, TODO(ABI); `g04_*`,
  RULING: halving vs cancel-one order on a 分摊 leg.) Sheet NOT FOUND.
* **R-5 path hook order:** does a per-tile forced stop beat a
  `SettleBefore` end-rewrite? (`rb_gap_move::g11_*`, RULING.) Engine now:
  the forced stop wins. Sheet NOT FOUND.
* **R-6 counteract close vs join:** once a round on X closes, may a
  真奈-style joiner still enter it? Engine now: no. No dedicated test;
  `rb_gap_window::g19_*` is the related DISCREPANCY (join + 网络链接异常).
  Sheet NOT FOUND (the joiner text is in `技能改动` `A51`).
* **R-7 extra-turn × until-turn:** does 「until your next turn start」 end at
  the first turn start after, including an extra turn's?
  (`rb_cross_long::l07_extra_turn_cascade`, DISCREPANCY -- tied to item 2.)
  Sheet NOT FOUND.
* **Other `RULING` ignores** (sheet NOT FOUND for all): G07 (does a negated
  EXIST redirect count as 「造成影响」? `rb_gap_money::g07_*`), G08 (星月夜's
  re-roll over a whole dice set, `rb_gap_move::g08_*`), G13 (does 普通与理所当然
  copy a signed 移动格数? `rb_gap_move::g13_*`), G26 (does Popipapapipopa's
  crystal spend reduce a rent the *owner* receives?
  `rb_gap_res::g26_*`), G29 (is each iteration of 春日影's draw-to-6 a
  replaceable draw? `rb_gap_res::g29_*`), t04 (does the Anon link add half
  of a mortgaged partner tile's 收费? `rb_cross_tiles::t04_*`).
* **三全音 payback recipient** (bank vs original payee). Sheet NOT FOUND
  (`新卡组卡` `G2` 「弃置此卡并支付由此卡获得的资金」 names no payee).
  `rb_morfonica::tritone_countdown_discards_and_pays_back` drives the
  countdown but does not pin the payee.
* **像往常一样 money refund on an undone forced move.** Sheet HINT
  (`新卡组卡` `C6` 「取消所有受到的效果（不进行任何结算）」) is broad enough
  to include undoing the money, but 「不进行任何结算」 scopes to the return.
  No test.

**Sheet search (2026-10-06):** every tab of the skill-rulebook Google Sheet
(`1xZ3avBsNBXbl3bQ74lmPs0YQFgZkPD0Sdzmx7ZGGEDY`) was exported and searched;
dumps in `target/scratch/sheets/tab_*.txt`. Rulings-bearing tabs are
`回合階段&註釋` and `为新版做的改动`. The ANSWERED items are folded into
the Decided list above; the NOT FOUND / HINT items still undecided are the
rulings 9-12 and the Timing / order block. Legend: **ANSWERED** = the cell
text decides it; **HINT** = relevant but not conclusive; **NOT FOUND**.
Character / band skill text drift is in §8.

### 7. Coverage gaps

The clause-level picture lives in [COVERAGE.md](COVERAGE.md); this is the
short list.

* No test file mentions these rule ids at all (COVERAGE "Part C mechanical
  checks"): AG 无论是何种颜色的夕阳, AG （巴）商店街的救世主, HHW （薰）怪盗
  hello happy, HHW （美咲）, skill:濑田薰:梦幻的王子殿下,
  R （纱夜）弹奏弹奏弹奏，继续弹奏, R 必然的联系（莉莎）. The exclusive
  cards PP:[大和麻弥]可能性为∞ and PP:[冰川日菜]会发出怎样的声音呢？ are
  also never played (their *skills* are tested).
* Marked not testable or shallow:
  * **hhw:** 笑容大游行 (also a stack overflow -- t13), 怪盗hello happy,
    kkr, 育美, 美咲, 北泽育美, 濑田薰.
  * **roselia:** 纱夜, 轨迹, 莉莎, 宇田川亚子.
  * **sumimi and crychic:** many tests are "is a counter" / "skill is bound"
    only.
  * **pp:** 冰川日菜 exclusive.
  * **ras:** 鳰原令王那.

### 8. Housekeeping

* `python tools/i18n/check.py` has 4 problems that predate this work:
  `err.skill_not_ported`, `anim.turnBanner`.
* `target/scratch/tainted/` holds the discarded rule-breaking test drafts.
  Delete them when no longer needed.
* `docs/rulebook/cards.json` still holds the old 寄于指尖 text (no [反击]
  wrapper) -- a sync target for whoever owns that file.
* `data/bands.json` matches `团技能` `D`-column for all 12 bands. No drift.
* `data/characters.json` matches `角色技能` `E`-column for 53 of 54
  characters. **One drift: 北泽育美 「全垒打！」** -- `data/characters.json`
  (and the hand-written `data/skill_simple.json`, which is not a second
  source of truth) carry three extra clauses the sheet does not. `data/` is
  another agent's; not edited here.

### 9. Design reference: `ileuxali/bangdream-monopoly`

This is a fan reimplementation in Unity, not the original game. All its cards
are dummies, so it gives **no rulings** on card text (§6). Its Accepted design
docs still answer engine-shape questions. Most of what it suggested is now
implemented (match-start hooks, per-draw hooks, the money pipeline's
AddDiff → MultDiff → Cancel pre-split then per-pair, 「[经过]」 on every
entered tile, restrictions as status-rule data). What is still load-bearing:

* **Counteract priority.** A round-robin ring per window; each visit allows
  one activation; nested windows stack LIFO. Their "triggering player goes
  first" contradicts clause 89 (our ring starts at the *next* seat); the
  per-visit one-activation cap is the model for the §10 ruling.
* **Mid-turn [停留].** Prohibition is checked once, at move-branch selection,
  so a [停留] gained mid-move doesn't stop the current move. This contradicts
  our `rain_stay_blocks_this_turn_move` expectation. Treat that one as an
  ambiguity (§4 / glossary 51).
* **「向上取整10」.** Their money splitting rounds each share up to a multiple
  of 10 globally, which hints that 爱心义演's wording restates a general rule.

### 10. Counteract chain order vs the rulebook

The rulebook (`data/rules.txt`):

* **32:** 「[反击]…在X发生时打出并触发效果Y，且结算优先于X」.
* **89:** 「如果有多名玩家可在同一时间发动[反击]效果则从行动顺序上在触发[反击]时点
  的玩家的**下一位**玩家开始依次决定是否使用[反击]效果。多个效果可[反击]同一个时点，
  所有玩家同意所有对一个时点的[反击]已发动后可对新的时点发动[反击]。」

Our engine: `hand_counteractions` / `build_round` / `resolve_rounds` /
`chain_starter` / `Priority` in `crates/game-rules/src/wasm_rules.rs`.

**Fixed** (2026-10-06, `rb_chain.rs` covers each): the ring opens at the next
seat (clause 89); a round collects counters to the same timing and only then
do the declared counters become new timings; a declaration advances priority
to the next responder and a player may declare again when the ring returns;
a negated counter's body does not run but the declaration and the spend
stand.

**Rulings** (what the book leaves open; recorded in the `hand_counteractions`
doc comment):

* **The order several counters to one timing resolve in.** **LIFO**: counters
  to one timing resolve newest first, and every counter settles before the
  timing it answers. Over the whole answer tree: a counter's own counters
  settle before it, and sibling counters settle newest first (post-order, each
  node's answers newest first). Covered by `counters_resolve_newest_first`.
* **Whether one player may declare more than one counter to a timing.** One
  activation **per responder visit** (ileuxali/bangdream-monopoly's accepted
  round-robin): a declaration advances priority to the next responder and does
  not reset; a player may declare again when the ring comes back to them; the
  round closes after every responder has passed consecutively with no
  activation in between. 「多个效果可[反击]同一个时点」 doesn't cap it per
  player. Covered by `a_player_may_declare_again_when_the_ring_returns`.

`ileuxali/bangdream-monopoly` asks the triggering player *first*, which also
contradicts clause 89, so it isn't a model for the ring; its round-robin
per-player cap is the model for the ruling above.

## Harness notes

* Engine test seams: `Rng::load_dice`, `Match::world_mut` / `world`.
* Loaded dice append to a global queue. Card rolls consume faces too.
* `clean()` keeps skills but drops deeds. Use `set_fire` for pots.
* `buy` / `build` are actions taken after the move, not prompts.
* `discard_card` is only the over-hand-limit discard.
* There is no token setter or crystal setter. Use `world_mut()`.
* `place_raw` skips the play body.
