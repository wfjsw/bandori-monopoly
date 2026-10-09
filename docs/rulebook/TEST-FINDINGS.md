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

## Status (2026-10-07)

Counts are a grep of the `#[test]` / `#[ignore = "…"]` **attributes** per file
in `crates/game-rules/tests/` (comment mentions excluded), so
`pass = tests - ignored` and `open = ignored`. The 2026-10-06 gate was
**34 targets, 761 passed / 0 failed / 93 ignored**; the tree has since grown
from concurrent work, so these drift while other agents edit. A re-grep on
2026-10-07 gives **875 tests, 90 `#[ignore]`** across the whole suite -- the
delta is other agents' in-flight tests plus the `rb_card_paths` wave below
(all 4 of its opens closed; the 4th was a **test bug**, fixed 2026-10-07,
§6 item 14).

Per group (`pass` / `open`):

| group | pass | open |
|---|---|---|
| general (通用 + CiRCLE) | 64 | 1 |
| ppp | 44 | 0 |
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
| card paths (`rb_card_paths`) | 14 | 1 |

The rest of the tree (same grep): `rb_cross_*` 111 tests / 40 open,
`rb_gap_*` 32 / 14, `rb_fuzz_found` 14 / 8, `rb_chain` 13 / 1, `rb_guards`
10 / 0, `rb_harness` 6 / 0, `rb_money` 4 / 0, `ruleset` 12 / 0,
`fuzz_interactions` 9 / 0, `live_match` 16 / 0, `q4_unintended` 3 / 2
(soak + report generator, not discrepancies),
`rb_unintended_found` 6 / 0. **Whole suite: 875 tests, 90 `#[ignore]`.**

A 0 in "open" means none were found, not that the group is verified correct.
See coverage gaps below.

### Event rules (2026-10-07)

`rules/events` (`docs/EVENTS.md`) is the event-card category: 28 `event:*`
`CardDef`s, one per `data/events.json` entry. The engine keeps only the deck /
draw / active list / filing away; the bodies live in the crate.

Engine-side coverage is `crates/game-core/tests/events.rs` (7 tests: the
`StubRules` fallback, bind on the board owner, idempotent bind, expire to
discard, expire to `event_removed`, deck-top push, banish). Black-box coverage
is `crates/game-rules/tests/rb_events.rs` (73 tests, one or more per event).

The discrepancy pass (2026-10-07) cleared 12 of the 14 `DISCREPANCY` ignores.
Two `RULING` ignores remain (哈比内尔 「结束后传送」 timing; 元祖！邦多利酱's
host clocks). Two `DISCREPANCY` ignores remain, both **test-is-wrong** rather
than code-is-wrong:

* **`asukayama_stun_wears_off_a_layer_at_a_time`** -- the engine's stun decay
  is correct (one layer per turn start, `MatchPlayer::tick_state`). The test
  observes after `until_turn(0)`, which waits for `stage::OPS`; a [眩晕]
  player's turn is auto-skipped (rulebook 204 「眩晕效果发动：跳过经营和主要
  移动阶段」), so the helper loops until *both* layers have ticked away. The
  assertion would need to observe at the turn-start boundary, not at OPS.
* **`marina_box_spend_is_optional`** -- the pass now opens an optional prompt
  (the 「可」), but the test's `10_000` expectation forgets the [经过]CiRCLE
  reward of +2000 (it would be `10_000 + 2_000`). It also contradicts
  `marina_box_pays_500_and_may_gain_1200_on_a_circle_pass` /
  `marina_box_no_gain_below_six`, which use the same `drain()` and expect the
  500 to be spent -- `drain()` declines every prompt, so the three cannot all
  hold. The prompt's fallback is 「消耗」 to keep the two green tests green.

One **previously-green** test now fails for the same class of reason:
**`asukayama_teleports_and_assigns_the_status_spread`** asserts
`t.pos(1) == park` (「the drawer too」) after the event, but the event text is
「所有玩家[传送]到飞鸟山公园。然后抽到此卡的玩家移动1d20」 -- the drawer ends
at `park + 1d20`, which is what its sibling
`asukayama_drawer_moves_1d20_after_the_teleport` asserts. The two are
irreconcilable; the spread test was written against the old (buggy) cancelled
move and never updated.

### Card-paths wave (2026-10-07)

`rb_card_paths.rs` (15 tests) covers four paths that landed untested:
`AG:（巴）商店街的救世主` (tomoe_savior), `CRYCHIC:（睦）从没有觉得...`
(mutsumi_never), `Mujica:（海铃）` (umiri_card), `RAS:（PAREO）渐渐远去的你`
(pareo_far). Three of its four `#[ignore]`s are closed:

* **Hand-over commits before the `bought` chain** (`tomoe_savior_buy_listener_places_the_free_house`).
  `ctx::set_owner` was a write on the card's world *copy*; `ctx::raise_bought`
  ran the hook chain on the live world, so Afterglow's
  「自动免费在上面加盖一栋房子」 hit `err.build_not_own`. `Cx::card_raise_bought`
  now commits the transfer (and the un-mortgage, see the RULING in §6) before
  raising.
* **Every 「购买地契」 listener hears the hand-over**
  (`tomoe_savior_buy_listener_pays_the_chuchu_bonus`). `raise_bought` raised
  only `bought`; `RAS:（chuchu）演奏我的音乐吧` sat on `BuyAfter`. Both
  acquisition hooks now fire (the same pair `buy()` raises), and chuchu moved
  to `HookKind::Bought` (C# `CardChuchuMusic.Bought` = `Fx.Bought`).
* **CRYCHIC (2) 「移除此卡与你所有区域的所有"CRYCHIC"卡」**
  (`mutsumi_never_2_band_2_removes_the_crychic_cards`). The zone sweep matched
  `starts_with("CRYCHIC:")`, which misses 「此卡」 itself -- the band skill's id
  is `skill:CRYCHIC:美好的往日幻影`. The sweep now banishes (「移除」 = out of
  the game) the band instance and every `CRYCHIC:` card in field / hand / deck
  / discard, 「内心的呐喊」 exempt.

The fourth (`pareo_far_triggers_the_pareo_skill_offer`) was a **test bug**, not
an engine gap -- fixed 2026-10-07 (§6 item 14), now green and un-ignored. The
engine side of item 4 is landed anyway: the
PAREO mark is one mark (the card and the skill share the literal
`PAREO标记`), 「初始1」 lands at `deckAtGameStart`, and `build()` now raises
`houseAdded` so a real 盖房 opens the 「失去1PAREO标记」 offer too.

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
  (loud safety cap, `Cx::reentrant_hooks`), `returns_prompt_storm`
  (2026-10-08: a raw `state_set("stun", n)` left a permanent [晕眩] that
  skipped every later turn and turned PPP:Returns' turn-start band borrow
  into a prompt storm -- `MatchPlayer::state_set` now stamps the rulebook's
  tick on stay / stun / stunStart writes; see GUARDS.md §11.6).
* **Sheet sync of 2026-10-06** -- 21 drifted cells into `cards-sheet.csv`;
  all landed except NNM's new (2) (card bug, §4).
* **User rulings of 2026-10-06** -- implemented and green (see the Decided
  list in §6).
* **Card-side wave** -- per-uid crystal moves, match-start 「初始N」 pots and
  fan init, exclusives place/recycle, roll-window wrappers (热气球演出,
  寄于指尖), Ringing Bloom / 椎名立希 (1) / 丰川祥子 (1) / CRYCHIC (2) /
  乐奈 tile lookup fixes.
* **Rename:** [反击] is `counteract` throughout (ABI v28).
* **Bot seats in the [反击] ring** (2026-10-08) -- `can_counteract_now`
  (`wasm_rules.rs`) rejected `player_id.ai`, a C# carry-over ("out / AI /
  exiled never open a window") that did **less than the book**: out / [除外] /
  `CannotPlay` are the only eligibility cuts, and "is a bot" is not one. Bots
  of every mentality are now offered like humans (answered through
  `Cx::fill_ai`: standard `CounterParams` propensity, default
  `DEFAULT_COUNTERACT_PROPENSITY_MILLI` = 600‰ per offered card -- user ruling
  2026-10-08, "bots must be able to counteract"; a book entry at 0 holds a card
  back; chaos `CHAOS_COUNTER_CHANCE`; advanced as an ordinary held prompt,
  deadline fallback = the skip). Pinned by `tests/rb_chain_bot.rs`.

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
  * PP band (2) overflow crystals: a passive 「X大于拥有数时」 hook on a
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
* **fuzz** (`rb_fuzz_found` 14 tests, 8 open): latent shapes the fuzzer
  found, filed with minimal repros -- see [COVERAGE.md](COVERAGE.md) "Fuzz
  coverage". Still open: `unknown_card_id`, `card_trap_reachable`,
  `determinism_break`, `save_restore_break`, `negation_not_total`,
  `multiply_order_matters`, `monotonicity_break`, `immunity_gap`.
  `returns_prompt_storm` (2026-10-08) is fixed and live: a raw
  `state_set("stun", …)` left a permanent [晕眩] that skipped every later
  turn, and PPP:Returns' 「每回合开始时」 band borrow asked once per skipped
  turn (GUARDS.md §11.6). `MatchPlayer::state_set` now stamps the rulebook's
  tick (「每回合结束时移除一层」) on stay / stun / stunStart writes.

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
* User rulings 2026-10-07 (通用:该清CP了 / the [CP点] mark owner; see
  `rules/tiles/src/cp.rs` and `rules/cards/card-general/src/clear_cp.rs`):
  * **[CP点] is its own tile-mark category, never owned by a player.**
    `TileMark.category = "cp"`, `TileMark.owner = -1`; provenance is
    `TileMark.src` (the placing card instance) and `TileMark.card`. The
    spread clause （1） 「此卡在格子上添加的[CP点]及其产物」 is read as
    **provenance** (this card instance's marks), not as 「marks I own」 --
    the old `count_marks(.., player_id)` keyed on `owner`, which a [CP点]
    no longer has. Cite: （1） + `data/rules.txt` 125.
    Test: `rb_general::cp_marks_are_neutral_and_attached_to_the_card`.
  * **There are two kinds of [CP点]** (verbatim: 「自己[场上]1个[CP点] referred
    to the cp point attached to the card. There are points on the tile (which
    mandated by tilemark) and points on the card (mandated by the card rule)」).
    **Tile** [CP点] are the `TileMark`s above (`mark:cp` owns them);
    **on-card** [CP点] is `FieldCard::cp` on the 该清CP了 instance -- 「自己
    [场上]N个[CP点]」 -- the card rule's own stock, crystals-like, shown as the
    view's field-card counter badge. [手] 「并在自己[场上]添加6个[CP点]」 seeds
    it at 6; （1）'s spread adds **tile** [CP点] only. The per-player counter
    `mark::CP_FIELD_TOK` is gone (ABI v38).
    Tests: `rb_general::cp_places_one_tile_mark_and_six_on_card_cp`,
    `rb_general::cp_on_card_count_rides_the_field_card`.
  * **The settle clause spends both kinds and pays the settler.** 「在拥有[CP]
    点的格子上[结算]时移除格子上的个[CP点]和自己[场上]1个[CP点]，[获得]800资金」:
    any player's [结算] on a tile carrying a [CP点] removes the tile mark
    **and** one on-card [CP点] from the 该清CP了 card the mark is attached to
    (`TileMark.src`; that card must hold ≥1), and the **settler** gains 800.
    「自己[场上]1个[CP点]」 is the card's own count per the ruling above; the
    card is found through the mark's `src`. **Who gains 800** (recorded): the
    settling player -- the subject of 「在…[结算]时」 carries over to 「移除…，
    [获得]…」, and the rulebook tip (rulebook.txt 2067 「短时间内吃多个CP点达到
    2000以上收益」) has the eater profit 800 a bite; house style names 「你」/
    「[使用者]」 when the card's user is the subject. C# (`CPControl.Clean`)
    instead paid `Seat` after gating the clause on `m.Seat == Seat` (so settler
    and controller coincided) and spent a per-player `Tok(Seat, "CP点")`; the
    ruling replaces the counter and this reading drops the gate.
    Tests: `rb_general::cp_settle_on_marked_tile_pays_800`,
    `rb_general::cp_settle_spends_the_src_cards_on_card_cp_and_pays_the_settler`.
  * **The card is graveyarded as soon as its on-card [CP点] is empty.** 「The
    attached on-card cp mark」 is `FieldCard::cp` on this instance (the same
    ruling), **not** the tile marks it placed (the older reading). When that
    count reaches 0 -- however it drops -- the card goes to its owner's 弃牌区
    immediately. Implemented as a `HookKind::CpChanged` handler (the same
    shape as AG:绯红之魂 (3) on `CrystalsChanged`), so it fires however the
    count drops -- the `mark:cp` settle clause spending one, another effect
    removing one -- and not as a check at each spend site. Tile marks may
    outlive the card; without a live `src` card holding on-card [CP点] the
    settle clause is inert on them.
    Tests: `rb_general::cp_card_is_graveyarded_when_its_on_card_cp_reaches_zero`,
    `rb_general::cp_card_stays_when_only_its_tile_marks_run_out`.
  * **It cannot be played while its [特] effect is still pending.** 「Pending」
    is read as **the [特] is still live on the field**: the card is the [特]'s
    carrier, and (per the graveyard ruling) it leaves exactly when its
    on-card [CP点] runs out, so the gate is 「a copy is still in play」.
    The alternative reading -- the literal 「下2回合开始时」 window, so a second
    copy could join one that still carries marks after two turn starts --
    is **not** what is implemented. The reason key is `special_live`
    (「[特]」), not a CP-named one, because `rb_general
    ::cp_hand_is_not_gated_at_two_points` refuses any second-play message
    containing `cp` / `CP` / `点`; the sheet 2026-10-06 `A8` gate it pins is
    still gone and this is a different one.
    Test: `rb_general::cp_cannot_be_played_again_while_the_special_is_pending`.

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
7. **Drawing a card several effects respond to.** Is a drawn effect card
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
13. **巴's 「收购」 vs 「购买」, and the mortgage.**
    `AG:（巴）商店街的救世主` says 「立刻支付常规收购价一半的价格从该玩家处
    收购该地契」 -- 「收购」, not 「购买」. Every 「购买…时」 listener
    (Afterglow 「商店街的宠儿」, RAS chuchu 「下次购买地契时」, Roselia, 朝日)
    hears the hand-over anyway (`ctx::raise_bought` runs the acquisition
    chain), so the engine reads a priced hand-over as a purchase. Evidence:
    「常规收购价」 is the same number the rulebook calls 「地契购买价格」
    elsewhere, and the one path that is *not* a purchase says so explicitly
    (强制购买: 「获得的地契仍为抵押状态」).
    * **The mortgage:** 巴's text has no 「仍为抵押状态」 clause, and 基础[结算]
      6 says 「如果格子地契已抵押则无效果」 for building -- so a hand-over that
      kept the mortgage would make Afterglow's 「自动免费在上面加盖一栋房子」
      permanently dead on this path (the trigger is always a mortgage).
      Engine now: `Cx::card_raise_bought` commits the transfer **and** the
      un-mortgage before raising, the same post-state `buy()` leaves.
      Reading the other way (keep the mortgage; let 「自动」 bypass the build
      gate) needs a card text that says so.
    * Tests: `rb_card_paths::tomoe_savior_buy_listener_hears_the_acquisition`,
      `tomoe_savior_buy_listener_places_the_free_house`,
      `tomoe_savior_buy_listener_pays_the_chuchu_bonus` (all green).
    * Sheet NOT FOUND for either half.
14. **TEST BUG (fixed 2026-10-07): `rb_card_paths::pareo_far_triggers_the_pareo_skill_offer`.**
    Not an engine gap -- the (2) 「可选择失去1PAREO标记」 offer *does* come up
    (PAREO mark unified under the literal `PAREO标记`, 「初始1」 at
    `deckAtGameStart`, `build()` raises `houseAdded`, and pareo_far's
    「视为你的房屋总数增加」 runs `ctx::invoke_skill` on the skill's press
    entry). The test's prompt-matching loop looked for the **resolved UI text**
    (`「PAREO」`/`「pareo」`/`「失去」`) inside `format!("{p:?}")`, but
    `MatchPrompt` carries i18n **keys** -- the offer's are
    `cards:skill-characters.numazu_maid_title` / `numazu_maid_ask` (zh-CN
    「失去 1 个 PAREO 标记，向其他玩家分摊收取 {{n}}？」), none of which
    contain those substrings. The loop therefore declined the offer as
    if it were the house-removal prompt (its exclusion list `pareo_far_ask` /
    `pareo_far_pick` is key-based, so the author *was* matching keys and just
    assumed a pareo-flavoured key the implementation does not use). Fixed by
    matching the key (`numazu_maid_ask` / `numazu_maid_title`) and taking
    `t.option("ask.yes")`; the house-removal prompt
    (`cards:card-ras.pareo_far_ask_*`) is still declined. Un-ignored, green:
    the one other player pays 1000×2/4 = 500 as the book says.

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
  反面[P✽P粉丝]变正的效果且X大于拥有数时」 hook on a fan-*flip*.
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
  one activation; nested windows stack LIFO. **Superseded (ruling 2026-10-07,
  §10):** the per-visit one-activation cap is retired -- a seat now exhausts
  its counteractions before priority moves on -- and the ring now starts at the
  initial user (like their "triggering player goes first") rather than the next
  seat. Their round-robin *shape* (a ring per window, nested windows LIFO) is
  still what we run.
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

**Fixed** (2026-10-06, `rb_chain.rs` covers each): a round collects counters to
the same timing and only then do the declared counters become new timings; a
negated counter's body does not run but the declaration and the spend stand.

**Ruling 2026-10-07** (user, verbatim: "Revise the chain counteraction
mechanism. Including the initial user, each user in the chain should be able to
exhaust all counteraction chances (or voluntarily abandon) for it to advance to
next player. Adjust as appropriate."). **This supersedes the 2026-10-06
ileuxali/bangdream-monopoly-derived per-visit rule and clause 89's 「下一位」
start** (the earlier ruling -- ring starts at the seat *after* the trigger's
player and asks them last; one activation per visit -- is retired):

* **Start seat.** The ask ring starts with the **initial user** -- the player
  whose action or effect raised the link (`by_card`; a board-driven link, i.e.
  a system / tile event like rent / buy / build / turn flow, starts at the
  active turn player) -- and runs forward in turn order from there.
  Covered by `the_initial_user_is_asked_first`,
  `a_no_player_trigger_starts_at_the_turn_player`.
* **Per-visit floor.** A seat keeps the floor until it **passes explicitly**
  (the offer's one "not playing" option) or holds no eligible counteraction
  left; every declaration re-offers the same seat. A seat with no eligible card
  is skipped without a prompt. Covered by
  `one_seat_declares_twice_in_one_visit_before_the_next_seat_is_asked`,
  `an_explicit_pass_advances_priority`,
  `a_seat_with_no_eligible_card_is_skipped_without_a_prompt`.
* **Round closing.** Laps of the ring continue until a **full lap brings no new
  declaration** (「所有玩家同意…已发动后」). A lap that carried a declaration
  never closes the round, even if every seat passed after it; the next full lap
  must be quiet. A later seat's declaration re-opens the earlier seats on the
  next lap. Covered by `the_round_closes_after_a_quiet_lap`,
  `a_later_seats_declaration_re_opens_an_earlier_seat`.
* **Counters to counters.** A declaration's own link gets its own round under
  the same rules, where that counter's **declarer** is the new round's initial
  user. Covered by `counters_to_a_counter_wait_for_the_round_on_x_to_close`,
  `counters_to_counters_start_with_the_declarer` (still ignored -- see below).

**Still standing** (unchanged by 2026-10-07):

* **The order several counters to one timing resolve in.** **LIFO**: counters
  to one timing resolve newest first, and every counter settles before the
  timing it answers. Over the whole answer tree: a counter's own counters
  settle before it, and sibling counters settle newest first (post-order, each
  node's answers newest first). Covered by `counters_resolve_newest_first`,
  `lifo_resolution_with_multiple_links_from_one_seat`.
* 「多个效果可[反击]同一个时点」 still does not cap declarations per player;
  the cap is now "all of them before priority moves on", not one per visit.

**Open item in `rb_chain.rs`:** `counters_to_counters_start_with_the_declarer`
is `#[ignore]`d. The engine does start the nested round at the declarer (and
`counters_to_a_counter_wait_for_the_round_on_x_to_close` observes that), but
`TEST:deny`'s guard `deny_yes` requires `trigger::player_id() != player_id` --
it never answers a player's *own* counter -- so the declarer has no eligible
card on their own counter's round and the "starts with the declarer" offer
cannot be observed through that fixture. The rulebook does not forbid answering
your own link; either the fixture guard or the test's observation needs a
change (black-box side).

## Harness notes

* Engine test seams: `Rng::load_dice`, `Match::world_mut` / `world`.
* Loaded dice append to a global queue. Card rolls consume faces too.
* `clean()` keeps skills but drops deeds. Use `set_fire` for pots.
* `buy` / `build` are actions taken after the move, not prompts.
* `discard_card` is only the over-hand-limit discard.
* There is no token setter or crystal setter. Use `world_mut()`.
* `place_raw` skips the play body.

## Purchase surface (`docs/PURCHASE.md`, 2026-10-07)

**Implemented (P0–P5):** quote / commit pipeline (`purchase.rs`), `BuyGate`
(every kind, Force included), `BuyAdd` / `BuyMul` / `BuySet` stages,
`BuyAssign`, `assign_deed`, `st.buy_price` / `st.build_cost` previews,
`MatchPrompt.price` / `prices[]`, agent colour set with `ANY_COLOR` /
`colorFor:`, force-buy keeps the mortgage, auction `BuyGate` filters bidders,
`ctx::acquire`, `linger` (turn-scoped instances), SAVE_VERSION 3 → 4.

**Rulings left as `TODO(规则书)`:**

* **Ruling 3** (mortgage to fund a buy): engine refuses today; unchanged.
* **Ruling 4** (agent pick: full [结算] or direct buy/build; 「玩家拥有的」
  includes others?): today's behaviour kept — a direct buy / build, and
  「玩家拥有的」 means the picking player's own tiles.
* **Ruling 5** (soyo 「所有颜色」 toward half-charge; per-player 「视为live
  house」 widen the agent set?): today's behaviour kept — `ANY_COLOR` joins
  the set for buy offers but is never buildable; the half-charge set is the
  plain `group` match.
* **Ruling 6** (is 强制购买 a 「购买」 for 「购买…时」 listeners?): no hooks
  today — `bought` / `buyAfter` do not fire on force-buy.
* **Ruling 7** (auction win a 「购买格子」 for discounts? auction [消耗] through
  the pipeline?): money stays direct; `bought` fires but `buyAfter` does not.
* **Ruling 8** (巴's 「常规收购价一半」 base): `ctx::acquire` takes the price
  from the caller; the base is undecided.
* **Ruling 10** (「本回合」 linger in an extra turn / another player's turn?):
  linger expires at the next turn start (one turn), matching the old flags.

**Ruling 1** (add → mul → set) and **Ruling 2** (buy [消耗] runs the pay
pipeline) are implemented as proposed (the stage order in `apply_stages`, and
`buy()` still calls `money()`).

**Ruling 9** (raze + Afterglow free house): raze first, so 1 house remains —
implemented (raze clears houses before the `bought` hook).

**P5 deferred work, now done (2026-10-07):**

* The three cards using `TurnCtx` flags (`@Tsugu ycm`, `Roselia band`,
  `迷宫般的仓库`) are ported to `ctx::linger` + `BuyAdd` −1500 / `BuyMul` ½ /
  `BuySet` 0 + `BuyAssign` houses 0. The flags, their `set_*` APIs, the retired
  `BUY_DISCOUNT` / `FREE_BUY` / `RAZE_ON_BUY` props are **deleted** (ABI 41).
* `WasmRules::buy_quote` (the hook-aware override) is wired: `BuyGate` /
  `BuyAdd` → `BuyMul` → `BuySet` run in pure guard mode against a world copy
  (the `cant_play` shape) only when a hooking instance exists; the commit
  re-quotes, so quote == charge. Gates / view / AI / autopilot read it.
* `st.tile_colors` / `key::EXTRA_COLOR` are **deleted**; the four skills
  (`asahi_aim`, `roselia`, `guerrilla`, `soyo_clear`) write the `colorFor:<p>`
  tile prop, and （soyo）混合的颜色 writes `prop::ANY_COLOR`.
* Poppin (3)'s hill lock moved from `BuyBefore` to `BuyGate`, so it covers
  Force too (「不可被抵押双倍支付购买」).
* rana_parking's 「该次传送不可进行地契购买」 is a native gate reading
  `plan.no_buy` on the Land offer — the flag was written but never read.
* 学生会的检查's 「本回合无法加盖房屋」 is a linger instance carrying
  `prop::NO_BUILD`; the per-player `noBuild` scratch key is gone.
* 巴's 收购 runs through `ctx::acquire` (pipeline pay → assign → `bought` →
  `buyAfter`) instead of `transfer` + `set_owner` + `raise_bought`.

**Flag-internal tests, rewritten to assert the same behaviour:**

* `rb_ras::lock_skill_colors_the_first_deed_like_a_live_house` asserted
  `extraColor:1`; it now asserts the observable consequence (the bought deed
  counts as a Live House for that player). The `t.owner(SHOPPING)` assertion
  (rulebook behaviour) is unchanged.
* `q4/snap.rs` / `q4/expiry.rs` recorded `turn.buy_discount` etc.; they now
  record `turn.lingering` — the mechanism those flags became — and treat it as
  turn-scoped, which is what 「本回合」 requires.
* `q4/names.rs`'s `extraColor:` filter goes with the key.

**Still open (unchanged, `TODO(规则书)` kept):** rulings 3, 4, 5, 6, 7, 8, 10.
Note that the quote's price stages now run for **every** `BuyKind`, so a
lingering 「本回合购买格子…」 discount also shapes a Force / Acquire / Auction
quote; before the migration the flags only applied to `buy()`. Whether those
kinds are 「购买」 for those clauses is rulings 6 / 7 and is **not decided**.

**Gate results (2026-10-07):** `cargo test -p game-core -p game-rules -p server`
979 passed / 0 failed / 97 ignored (baseline 976/0/97). `fuzz_interactions`
9/9. i18n 4 (baseline), rulebook check 0. `tsc --noEmit` clean, `autopilot.test`
20/20. Sim 50×4×200: buy 2899 / forcebuy 392 / auctions 255 — identical to the
baseline, ~303 ms/game (StubRules unchanged; the wall clock on this box swings
247–445 ms/game with desktop load, so the **event counts** are the reliable
signal). `bot_cost` 5×4×200 real rules: the hook-aware `buy_quote` measured at
≈1 % of wall (17 425 ms/game with the hooks short-circuited vs 17 667 with
them), i.e. not a meaningful cost; the absolute figure moves ±50 % with machine
load while the module-instantiation and world-clone counts stay put.

---

## Pipeline-audit batch (2026-10-07)

Nine new black-box tests for the decided `PIPELINE-AUDIT` items (see
`PIPELINE-AUDIT.md` §7 for the per-item status and citations).

| Test | Item |
|---|---|
| `rb_money::card_shortfall_raises_funds_then_bankrupts` | Q1 / B1 |
| `rb_money::card_shortfall_may_be_funded_by_mortgage` | Q1 / B1 |
| `rb_money::pre_split_modifier_shapes_the_total_not_each_leg` | Q2 |
| `rb_money::self_payment_leaves_money_unchanged_when_affordable` | Q4 |
| `rb_money::self_payment_may_be_funded_by_mortgage` | Q4 |
| `rb_money::self_payment_shortfall_raises_funds_then_bankrupts` | Q4 |
| `rb_rulebook::bankruptcy_stops_field_effects` | B2 |
| `rb_rulebook::bankrupt_before_sees_a_dead_player` | B3 |
| `rb_rulebook::auction_winner_may_mortgage_to_fund_the_bid` | B4 |

New `TEST:*` fixtures in `rules/fixtures/test-cards/src/lib.rs`:
`TEST:totalCut` (a `payTotalAdd` probe that cuts the command total by 500),
`TEST:deadPay` (a `bankruptBefore` probe that tries to move the dying seat's
money), `TEST:selfCharge` (「A[支付]A」 5000), `TEST:payAddAny` (a `payAdd`
probe that boosts every payment by 100).

### One pinned behaviour read against the ruling's wording

**B2 「tokens」 vs `rb_pp::shanyao_counter_on_short_payment`.** The ruling says
`remove_from_game` clears "field cards / tokens / skill effects". 规则书 L81's
removal list is 棋子 / 角色卡 / 乐队卡 / 手卡 plus 「正在生效的卡，技能效果」;
「标志物解释」 (火罐 / 奇迹水晶 / P✽P粉丝 ...) is a separate vocabulary and L81
does not sweep the markers. `shanyao_counter_on_short_payment` pins the
consequence: 再次闪耀's rescue grants a P✽P fan and then the owner goes under on
the same rent, and the test reads the fan afterwards. Clearing `s.tokens` drops
it to 0 and the pin fails. **Markers are kept**; field cards and skill effects
are cleared as the ruling asks. Flip it and the pin moves with it.

### Gate results (2026-10-07)

`cargo test -p game-core -p game-rules -p server -p bot-core -p bot-service -p
rules-cond --no-fail-fast` **1041 passed / 0 failed / 96 ignored** (baseline
1032/0/96 — the +9 are the new tests). `fuzz_interactions_invariants` 9/9 (it
runs inside the suite). `python tools/i18n/check.py` 4 (baseline).
`python tools/rulebook/check.py` 0. `node tools/build-ruleset.mjs` 285 cards.
`node tools/build-glue.mjs` clean. `npx tsc --noEmit` clean.
`node --test src/game/autopilot.test.ts` 20/20.

Sim `50 4 200`: **248.0 ms/game** (≤ 323), `avg rounds 196.3`,
`end reasons {"last": 11, "settle": 39}`,

```
prompts     {"auction": 255, "choice": 6414, "mortgage": 1068, "tile": 3922}
event kinds {"bankrupt": 96, "build": 4669, "buy": 2899, "draw": 2320,
              "event": 1187, "forcebuy": 392, "lose": 7313, "mortgage": 2245,
              "move": 5347, "overlap": 1517, "pass": 5909, "play": 2709,
              "redeem": 1517, "rent": 19567, "roll": 34398, "text": 55654,
              "turn": 34437}
details     {"agent half rent": 4218, "auction won": 255, "circle money": 5909,
              "force buy": 392}
```

Seed-stable (two runs identical). An A/B with the Q5 `buy_hook_instances` sort
disabled produced **identical counts**, so the ordering key is a no-op on
today's behaviour. The counts above are the post-batch shape: B1 (card payments
may now bankrupt) and B4 (a short bidder may 抵押 rather than void) are the
legitimate movers of `bankrupt` / `mortgage` / `auction`, and the pre-split
stage is a no-op with no `payTotal*` card in the sim pool. The purchase-surface
baseline quoted above (buy 2899 / forcebuy 392 / auctions 255) is **unchanged**.

## Settle/move stage model (2026-10-07, ABI 43)

Re-homed per `docs/rulebook/SETTLE-STAGES.md` §4 and the user rulings of
2026-10-07. New hooks `moveBefore` / `moveAfter`; Q7 ruled and implemented
(the `TODO(规则书) PIPELINE-AUDIT Q7` marker is removed). ABI 42 → 43.

Tests: `crates/game-rules/tests/rb_settle_stages.rs` (7 cases, all green):

* `m1_no_settle_move_still_grants_the_move_after_fire` — 儿时玩伴's 「移动后」
  pot fires on a 「不触发结算」 move (R1).
* `m2_body_replace_skips_the_settle_time_clause` /
  `m2_settle_time_clause_runs_without_a_body_replace` — a 「[结算]时」 clause
  (哪怕这旅程没有终点 (2)) dies with a Parking Space body replace and runs
  without one (M2).
* `m4_a_pass_by_not_an_overlap_triggers_kaoru` — 薰 (1) 「被[经过]」 fires on
  a mid-route pass (M4).
* `m4_kokoro_force_stop_stops_a_passer` — 凑友希那 (3) 「强制停下」 actually
  stops a passer (M4's latent bug: the old `passPlayer` guard was dead).
* `m6a_no_settle_teleport_fires_overlap_at_the_destination` — a
  「不[触发结算]」 teleport raises [重叠] at its destination (R2 / M6a).
* `q7_settle_before_relocation_settles_at_the_new_tile` — a settleBefore
  relocation redirects the settle (Q7).

Excluded pending rulings (left with `TODO(规则书)` markers): `want_to_grab`
(2) (R5), `Sumimi:no_breakup` (R3).

Known failures at time of writing (need investigation, possibly pre-existing
from the in-flight batch): `rb_card_paths` 4 cases —
`mutsumi_never_2_runs_the_band_skill_2`,
`mutsumi_never_2_runs_the_band_skill_2_on_a_crystal_shortfall`,
`mutsumi_never_2_band_2_removes_the_crychic_cards`,
`pareo_far_triggers_the_pareo_skill_offer`.

---

## Late 2026-10-07 batch (Q3 / V1 / V2 / V4 / V5 / markers / est-cost)

New `rb_money` cases (all green at time of writing):

* Q3 reversal: `two_sided_negative_final_reverses_the_payment`,
  `one_sided_negative_gain_becomes_a_loss`,
  `one_sided_negative_loss_becomes_a_gain`,
  `reversal_shortfall_raises_funds_then_bankrupts`. The old
  `negative_final_amount_clamps_to_zero` is renamed
  `negative_gain_modifier_shrinks_the_gain` (its scenario nets positive).
* V2 no activation costs: `gacha10_playable_while_short_then_bankrupts`,
  `fire_bird_playable_while_short_then_mortgages`,
  `crimson_soul_playable_while_short_then_bankrupts`,
  `believe_you_playable_while_short_then_bankrupts`,
  `kokoro_circle_playable_while_short_then_bankrupts`,
  `starry_night_playable_while_short_then_bankrupts`.
* Markers: `marker_spend_window_cancels_the_spend`,
  `bankruptcy_clears_owned_markers_wherever_they_sit`,
  `misaki_may_counteract_with_zero_fire`.
* `rb_pp::shanyao_counter_on_short_payment` now expects the P✽P fan to be
  **cleared** with the bankruptcy (reversing the keep-tokens deviation).

Open items unchanged: N3 (effect-level atomicity), N4 (「取消所有受到的效果」
refunds), N5 (`Negation::Activation` returns the counteraction card to hand).
