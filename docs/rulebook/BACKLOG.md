# Rulebook / ABI backlog

**2026-10-10 triage.** Branch: `rulebook-triage` (`/home/monopoly/rulebook-triage-wt`).
This is the standing backlog for continuous iteration on rulebook divergences,
rule-implementation bugs and ABI problems. Fix agents take independent batches
from here; do not merge into the primary branch from this file.

Sources:
- 144 `TODO(规则书)` tags in `rules/` + `crates/`
- live `#[ignore = "…"]` set in `rules/tests/tests/*.rs` and `crates/game-rules/tests/*.rs` (85 after the un-ignore batch; 69 DISCREPANCY, 13 RULING, 2 meta)
- `docs/rulebook/TEST-FINDINGS.md` (§3 ABI / §4 per-card / §6 rulings / 2026-10-09 triage), `COVERAGE.md`, `CROSS-TESTS.md`, `NEGATION-AUDIT.md`, `PIPELINE-AUDIT.md`, `SETTLE-STAGES.md`, `docs/GUARDS.md`
- 2026-10-10 ABI audit of `crates/game-rules/src/{host,hostfns,native_shims,wasm_rules}.rs`, `rules/card-sdk/src/{ctx,sys,abi}.rs`, `crates/game-core` purchase/play

Ignored-test run: `node tools/build-ruleset.mjs` (no `CARGO_TARGET_DIR`) then
`cargo test -p game-rules --no-fail-fast -- --ignored` (run twice). Nine
stale ignores that passed reliably were un-ignored in commit
`triage: un-ignore 9 stale rulebook tests + refresh TEST-FINDINGS` — see D-01.

**Class**
- **A** — fixable now from unambiguous rulebook text + current ABI
- **B** — needs a new/changed ABI capability (named in our vocabulary)
- **C** — rulebook text ambiguous; user ruling needed
- **D** — stale (already fixed / already landed)

**Severity**: high (common play) | medium | low (rare edge)

Sort: severity, then class. One item = one root cause; every file:line / test is listed.

---

# card-std-split leftovers (2026-10-12)

Deferred from the `card-std-split` / ABI v56 pass (ruleset-core + `tile_is`/
`tile_attr`/`tile_in`). Not merge blockers once drift is green.

- **CD-01 CEL `is_live_house` group-10 quirk.** `SnapSrc` / `LiveSnap` still
  answer CEL `is_live_house(t)` as buyable && group **10** (shop), while the
  host / `tile_in(_, t, "livehouse")` is buyable && group **6**. Preserved so
  existing condition strings keep the same results. Align CEL sugar with the
  host in a dedicated pass and re-check card_events.
- **CD-02 `crystals_changed_guard` residuals.** `ruleset_core::crystals_changed_of`
  is the shared base; the x11 per-card copies still carry local residuals
  (value vs 0, `is_placed`, …). Fold the identical prefix onto the helper.
- **CD-03 `legacy_*` audit copies.** Delete only after NEGATION-AUDIT is clean
  (GUARDS.md G3). Not part of the split.
- **CD-04 HookKind game-named wire variants** (`CircleAffected`, `CpChanged`,
  `CrystalsChanged`, …) stay in `card-sdk` as wire codes; ownership is
  documented. A rename would be an ABI break for no gain.
- **CD-05 native arena trap leak.** `native::enter`'s frame stack has a depth-64
  safety valve if `Drop` is skipped on a non-unwinding trap. Fine for
  `C-unwind`; revisit if a non-unwinding abort path appears.

# HIGH

## STACK-01 — nested `card_settle_at` recursion overflows the stack (engine defect)
- **status**: **fixed** (2026-10-10, `stack-01-settle-queue`) — nested settles run on an
  explicit work stack (`game-core/src/engine/work.rs`); `card_settle_at` enqueues and
  `Cx::drain_work` pumps. A drive that needs a nested settle while the pump is running
  suspends (`Halt::Suspended`) and resumes after — same order as the old synchronous
  nest, one Rust frame deep. `flush_deferred → start → execute` is also iterative
  (guard flag). Rulebook-level `MAX_SETTLE_DEPTH` = 64 logs and stops a rules infinite
  loop (two cards settling each other); it is **not** the stack-safety mechanism.
  Regression: `crates/game-rules/tests/settle_depth.rs` (120-deep chain on a 512 KiB
  thread). `ckpt_equiv -- out 8 4 120 standard|chaos` now completes.
  Equivalence: `ckpt_equiv` `4 4 60` and `8 4 40` standard+chaos and `card_events`
  are **byte-identical** to master af2e1a1. `8 4 120` standard+chaos complete.
  Resume keeps completed hook bodies exactly once (`settle_tail_once`).
- **kind**: implementation bug
- **class**: A
- **severity**: high (any long bot game / real match that nests a settle hook; already aborts `ckpt_equiv`)
- **suggested batch**: engine: settle recursion / stack safety
- **locations**
  - repro (2026-10-10, this worktree, release, 8 MiB main stack):
    `cargo run -p game-rules --release --example ckpt_equiv -- out 8 4 120 standard`
    → `thread 'main' has overflowed its stack` / `fatal runtime error: stack overflow, aborting`
  - recursion cycle (LD_PRELOAD `backtrace()` on SIGSEGV + `addr2line`, 5-frame repeat):
    ```
    game_core::engine::play::Cx::card_settle_at          (play.rs:790)
      → Cx::settle_at                                    (play.rs:1677)
        → CardRules::counteract                          (wasm_rules.rs:5610)
          → RulesBridge::drive_inner_body                (wasm_rules.rs:1878)
            → RulesBridge::apply_host_request            (wasm_rules.rs:2231 HostRequest::SettleAt)
              → Cx::card_settle_at                       (play.rs:790)  … repeat
    ```
  - no depth guard on nested settles (`MAX_SETTLE_REDIRECTS` = 8 only caps *redirects inside one* settlement, `play.rs:41/1641`; `MAX_COUNTERACT_DEPTH` = 16 only caps one counteract *tree*, `wasm_rules.rs:53/3149`; `MAX_MONEY_DEPTH` = 32 only caps the money pipeline, `play.rs:27/3048`)
  - guest callers of `ctx::card_settle_at` from hook bodies (any of these can re-enter `settle_at`):
    `rules/cards/card-hhw/src/smile_parade.rs:122` (`On::Hook(SettleBody, settle_instead)` — direct self-nest: the body cancels the tile settle and `card_settle_at`s at 「弦卷集团」),
    `card-ras/src/{studio_storm,guerrilla}.rs`, `card-mygo/src/{taki_serious,rinne_rain}.rs`,
    `card-morfonica/src/noble_blue.rs`, `skills/skill-characters/src/{kaede_support,tomori_crychic,tukushi_try,tomori_poem}.rs`
  - related: `crates/bot-core/src/ismcts.rs:823-830` already documents "a rollout's trigger chain can nest deeply" and spawns 32 MiB stacks for that reason
- **ABI / engine contract**: `HostRequest::SettleAt` is a synchronous nested routine (`wasm_rules.rs:2231-2238` → `cx.card_settle_at` → `settle_at` → `raise!` → `counteract` → `drive` …). A hook body that settles therefore stacks a full `settle_at` + `drive_inner_body` (a large `Run` frame) on top of the outer one. Guest **fuel** (`be_wasmi.rs:51` / `be_wasmtime.rs:72`) bounds guest recursion only; these are **host** frames.
- **current**: unbounded mutual recursion `card_settle_at ↔ settle_at ↔ counteract ↔ drive_inner_body ↔ apply_host_request`. Long seeded bot games hit it on the 8 MiB main stack (`ckpt_equiv`).
- **expected**: nested settles are either (a) queued and pumped by an iterative loop at the outermost `execute` / `tick` (preferred — same shape as `flush_deferred`'s intent, but without `start()` re-entering `execute()`), or (b) refused past a `MAX_SETTLE_DEPTH` (stopgap; log + no-op the inner settle). A bigger stack is a workaround only.
- **can a real match hit this?**
  - **Server — yes, more easily than `ckpt_equiv`.** Room work runs on `tokio::task::spawn_blocking` (`crates/server/src/lib.rs:134`, `api.rs:503`, `botsvc.rs:440`, …) with the Rust default ~2 MiB thread stack (no `stack_size` in `crates/server/Cargo.toml`). A settle-nesting chain that already dies at 8 MiB will kill the blocking worker at ~1/4 the depth → aborted thread / failed `act` / dropped room tick.
  - **Browser / wasm — yes for the host side.** The guest module is fuel-bounded, but `card_settle_at` runs as a **host import** on the JS/embedder native stack (and on wasmi/wasmtime's host call stack). Each nested `drive_inner_body` allocates a full `Run` on that stack. Worker stacks are typically 1–2 MiB. wasmtime's "trap tears the stack" note (`be_wasmtime.rs:43`) covers *guest* traps, not host frames.
  - **bot-core ISMCTS** already works around this with 32 MiB stacks; that workaround does not cover server workers or the browser.
- **fix sketch**
  1. **Preferred:** give `Cx` a settle-job queue. `card_settle_at` pushes `(player, tile, main)` and returns; the outermost `execute` / `tick_play` drains the queue in a `while` loop (no `start()` inside `flush_deferred` — that path has the same re-entry shape via `Deferred::Leave` → `start(Routine::Act)` → `execute` → `flush_deferred`).
  2. **Stopgap:** `MAX_SETTLE_DEPTH` (e.g. 8) in `card_settle_at` / `settle_at`; past it, log and skip. Cheap, unblocks fuzz / `ckpt_equiv`, does not fix the shape.
  3. **Do not** "fix" this by raising server/browser stack sizes alone (bot-core already tried that for one caller).
- **cross-links**
  - **SETTLE-01** (笑容大游行 stack overflow, `t13_smile_parade` / `g30_*`) is one *instance* of this cycle — `smile_parade::settle_instead` is a `SettleBody` hook that calls `card_settle_at`. Fix STACK-01 first; then SETTLE-01 may reduce to the swap-axes bug alone.
  - **HOST-01** (HostRequest B2 learn-pass rebase) is a different defect in the same `apply_host_request` surface.
  - `flush_deferred` → `start` → `execute` re-entry (`engine/mod.rs:1172-1286`) is a second unbounded host-stack growth path (Leave → Act → Leave → …); fold it into the same batch.
- **notes / reproduction**
  - Fails reliably with `games=8 players=4 max_rounds=120 standard` on an 8 MiB stack. Completes for `games=2 max_rounds=40` on 2 MiB — the nest is seed/length dependent, not a constant-depth crash.
  - Backtrace captured with a SIGSEGV + `sigaltstack` + `backtrace()` LD_PRELOAD (`target/scratch/btso.c`, scratch only — not committed) because `gdb` is not installed on the host. Frame offsets in the release `ckpt_equiv` binary: `+0x55d2d3` `+0x55ab6f` `+0x546f24` `+0xb091c9` `+0xaef4fd` (5-cycle).
  - Tag any fix `TODO(规则书)` only if a *rule* text forces a nested settle; the engine defect itself is not a rulebook divergence.


## MONEY-01 — rent / settle multipliers do not have a composition rule
- **kind**: rulebook divergence
- **class**: C
- **severity**: high
- **suggested batch**: rent multipliers
- **locations**
  - `rules/tests/tests/gap_money.rs:117` `g01_fire_bird_times_ave_mujica_on_one_rent` `#[ignore = "RULING: order of two multipliers on one rent (Fire bird 1.5x x Ave Mujica 1.5x)"]`
  - `rules/tests/tests/cross_tiles.rs:457` `t11_fire_bird_plus_hhw_double` — **D note**: ignore already removed (test still records the question; do not re-open as A)
  - `rules/tests/tests/fuzz_found.rs:173` `multiply_order_matters` `#[ignore = "DISCREPANCY: two multiplicative money modifiers do not commute"]`
  - TEST-FINDINGS §6 Timing/order R-1
- **rulebook text**: Fire bird 「支付…的1.5倍」, Ave Mujica 状态2 「从[收取]与[支付]的资金改为1.5倍（只对自己结算）」, HHW band double-pay, 摩卡 half. All live in window `回合階段&註釋` `B30`/`C30` with no product/stack rule.
- **current**: multipliers compose in declaration order; two muls do not commute (fuzz).
- **expected**: one agreed composition (product? last-writer? one window once?).
- **question (verbatim in §C list)**: C-Q1.

## MONEY-02 — 「支付减半」 vs cancel-one / 分摊 recompute order
- **kind**: rulebook divergence
- **class**: C
- **severity**: high
- **suggested batch**: payment pipeline
- **locations**
  - `rules/tests/tests/gap_money.rs:222` `g04_half_share_then_cancel_one_payer` `#[ignore = "RULING: halving vs cancel-one order on a 分摊 leg (and whether the share recomputes)"]`
  - related: `rules/events/src/front_or_back.rs:49,56` (「分摊」 remainder) — separate small C, see EVENT-03
- **rulebook text**: 爱心义演 / 分摊 legs; 网络链接异常 cancel-one designation (sheet: 「取消一个指定」).
- **current**: share is the pre-drop figure (TEST-FINDINGS §3); halving-vs-cancel order undecided.
- **expected**: decide whether the share recomputes after a designation drop, and whether halving applies before or after the split.
- **question**: C-Q2.

## MONEY-03 — Repaint (「对方此次结算的支付减半」) does not halve the shaped total
- **kind**: test-setup bug (implementation already correct)
- **class**: A
- **severity**: high
- **suggested batch**: payment pipeline
- **status**: **fixed** (this batch, `fix: MONEY-03 …`) — the pipeline
  already halves the shaped total (`payAdd` runs Tomorrow's Door's surcharge,
  `payMul` applies `plan::set_pay_factor(500)`). The test's dice math put
  the hill off the planned path (`hill - 1 - 5 - 2` → X = 0, Repaint never
  offered), so the discrepancy was never actually observed. Dice is now
  `(hill − 5) + X` from a close start. Un-ignored, green twice.
- **locations**
  - `rules/tests/tests/ras.rs` `repaint_halves_a_shaped_settlement_payment` (un-ignored)
  - `rules/cards/card-ppp/src/tomorrows_door.rs` (`pay_add` at `payAdd`)
  - `crates/game-core/src/engine/play.rs` (`scale_settle_payment` at `payMul`)
  - related tests `cross_tiles.rs:163` `t03_anon_link_plus_repaint` (Anon link arrangement, still ignored)
- **rulebook**: ruling 2026-10-06 + sheet 「对方此次结算的支付减半」 — the *settlement payment* (post-shape) is halved.
- **was**: test dice put the hill off-path; X = 0 so Repaint was never offered.
- **expected**: half applies to the whole shaped settlement payment (surcharge included). **Confirmed.**

## MONEY-04 — 强制购买 / auction money sits outside the money pipeline
- **kind**: rulebook divergence
- **class**: C
- **severity**: high (force-buy is common)
- **suggested batch**: payment pipeline
- **locations**
  - `crates/game-core/src/engine/play.rs:2266` (「支付」 reach of non-settlement loss), `:2302` (ruling 6: no `bought`/`buyAfter` on force-buy), `:3955`, `:3967` (ruling 7: auction money stays direct)
  - `docs/PURCHASE.md` rulings 3/6/7; TEST-FINDINGS §6 item 12
  - `rules/tiles/src/property.rs:10,68` (force-buy 「不受任何资金变动效果影响」)
- **rulebook**: 基础[结算] 6.2 / 其他 6 — mortgage / redeem / forced purchase 「不受任何资金变动效果影响」; auction win / 支付 through pipeline undecided.
- **current**: force-buy sum moved outside the pipeline entirely (no pay hook, even one that 「写明改动强制购买」); auction money direct; `bought` fires on auction but `buyAfter` does not; force-buy fires neither.
- **expected**: decide (a) whether 「不受…影响」 also bars 「写明」 exceptions, (b) whether 强制购买 is a 「购买」 for 「购买…时」 listeners, (c) whether auction is a 「购买格子」 and whether auction [消耗] runs the pipeline.
- **question**: C-Q3 (three sub-parts).

## MONEY-05 — 巴 「常规收购价一半」 base + 收购-as-购买
- **kind**: rulebook divergence
- **class**: C
- **severity**: high (AG shopping-street is a signature card)
- **suggested batch**: payment pipeline
- **locations**
  - `crates/game-core/src/engine/play.rs:899` (ruling 8)
  - `rules/cards/card-ag/src/tomoe_savior.rs:71` (extra mortgage income raw vs `gain`), `:85` (ruling 8 base)
  - `docs/PURCHASE.md` ruling 8; TEST-FINDINGS §6 item 13
  - tests (green, pin current): `card_paths::tomoe_savior_buy_listener_*`
- **rulebook**: 「立刻支付常规收购价一半的价格从该玩家处收购该地契」 — 「收购」 not 「购买」; base (land / land+houses / 2× force) unnamed.
- **current**: C# reading `price / 2` (land alone); hand-over runs `raise_bought` so every 「购买…时」 listener hears it; mortgage is cleared before the hook.
- **expected**: decide the price base, whether 收购 is a 「购买」, and whether the mortgage stays.
- **question**: C-Q4.

## CH-01 — self-applied abnormal / self-[传送] never opens a [反击] window
- **status**: **fixed** (2026-10-13, `closeout-2-movement`) — the abnormal
  `Effect` chain already opened for self-applied abnormals (the green twin
  `choose_your_stage_answers_an_abnormal_move` proved it); `ix_encore_…` was
  a **D** test drive that never declared 安可. Un-ignored and green after the
  drive plays the counteraction. `m28b` (MOVE-11) and `m09` (MOVE-12) are
  filed under their own MOVE ids and fixed alongside.
- **kind**: implementation bug + missing ABI
- **class**: A (wire the abnormal/Effect chain on `card_move` teleport) then B if the move-tag cannot express 「因任何原因」
- **severity**: high (安可 is a staple [反击])
- **suggested batch**: counteraction windows
- **locations**
  - `rules/cards/card-general/src/encore.rs:35` (「因任何原因」)
  - `rules/cards/card-ag/src/ran_as_usual.rs:23` (「包括你的技能」), `:27` (C# once-per-turn gate — separate C, see CH-07), `:92` (`Plan.Reverse` skill move is abnormal but `_abnormalTurn` never counted it)
  - `rules/tests/tests/pp.rs:963` `ix_encore_blocks_overlap_teleport` (un-ignored, green)
  - `rules/tests/tests/cross_move.rs:1166` `m28b_my_own_problem_after_forced_move` (see MOVE-11)
  - `rules/tests/tests/cross_move.rs:404` `m09_ras_band_first_abnormal_only` (see MOVE-12)
  - `crates/game-rules/src/wasm_rules.rs:2801` (self-applied abnormals note)
  - **D**: `choose_your_stage_answers_an_abnormal_move` (`roselia.rs:369`) — ignore already removed; keep as a D note only
- **rulebook**: 通用:安可 「[反击][使用者]即将因任何原因受到[异常移动效果]影响时：无效此次…」; （兰）像往常一样 (2) 「受到异常移动效果（包括你的技能）」.
- **was**: `abnormal` chain only opens when `by != target`; `card_move` / `plan::set_teleport_to` never runs the abnormal gate at all.
- **now**: every abnormal move on the user — self-inflicted included — opens the [反击] window before it lands (`HostRequest::Move` / `Teleport` run `abnormal_gate`).

## CH-02 — 花园多惠 (2) cancel window never opens
- **status**: **fixed** (2026-10-11, `backlog-windows-a`) — the bound skill declared
  `On::Counteract` but lacked `COUNTERACT_FROM_FIELD`, so `declare_one` never
  offered it. Added that prop plus `COUNTERACT_GROUP` (one cancel per activation;
  the skill stays in play and the seat keeps the floor). The redundant `ask_yes`
  confirm is gone — the [反击] window's play-vs-skip is the 「你可以使用」 choice.
  Tests: `counteract_offered` matches `ask.counteract.skill`; `prompt_refers`
  reads an `ask_yes` question from the title/text.
- **kind**: implementation bug
- **class**: A
- **severity**: high
- **suggested batch**: counteraction windows
- **locations**
  - `rules/tests/tests/cross_chain.rs` `c24_hanae_cancels_a_hand_effect`, `c25_hanae_vs_skill_main` (un-ignored, green twice)
  - `rules/tests/tests/cross_long.rs` `l05_one_skill_several_counteractions` (un-ignored, green twice)
  - `rules/skills/skill-characters/src/tae_police.rs`
- **rulebook**: 花园多惠 (2) — cancel a hand effect / skill use (「取消」 window).
- **was**: card resolves normally; no cancel window.

## CH-03 — 骰子已经掷下 is placed twice and does not shut counter windows
- **status**: **fixed** (2026-10-11, `backlog-windows-a`) — `can_counteract_now`
  honours the well-known state key `noCounteractTurn` (the latch owner is exempt
  「其他」; field counteractions stay a separate path, the clause names
  「从手牌中」). A turn-key placement latch replaces the uid-based `is_placed`
  check (a hand play has `current_uid = -1`, so play + the own-counteract
  follow-up double-placed).
- **kind**: implementation bug
- **class**: A
- **severity**: high
- **suggested batch**: counteraction windows
- **locations**
  - `rules/tests/tests/cross_chain.rs` `c14_dice_cast_shuts_windows` (un-ignored, green twice; asserts single placement)
  - `rules/tests/tests/gap_window.rs` `g18_shut_window_blocks_the_negate_one` (un-ignored, green twice)
  - `rules/cards/card-mujica/src/dice_cast.rs`, `crates/game-rules/src/wasm_rules.rs` `can_counteract_now`
- **rulebook**: 骰子已经掷下 — place the lock and shut further [反击] windows for that timing.
- **was**: placed twice; windows stay open.

## CH-04 — 无法将视线移开 cannot be played as a counter and does not force the move
- **status**: **fixed** (2026-10-11, `backlog-windows-a`) — the card only declared
  `ChainKind::Counteracted`, which the engine never raises; it now also declares
  `Card`/`SkillUsed` with `target == owner && actor != owner` (the counter link's
  `target` is the countered seat, `actor` the declarer). Force-move lands on
  `trigger::player_id` of the answered link (the immediate 「有玩家对你使用[反击]
  后」); the full-turn history (ABI-04) stays a separate hole.
- **kind**: implementation bug
- **class**: A
- **severity**: high
- **suggested batch**: counteraction windows
- **locations**
  - `rules/tests/tests/cross_chain.rs` `c16_cannot_look_away_answers_a_counter` (un-ignored, green twice)
  - `rules/tests/tests/cross_long.rs` `l04_forced_move_into_remote_settle` (un-ignored, green twice)
  - `rules/cards/card-mujica/src/cant_look_away.rs`
- **rulebook**: 无法将视线移开 — usable as a [反击]; forces the counter-user to move.
- **was**: `err.play_phase` when played as the counter; forced move did not happen.

## CH-05 — nested [反击] copy machinery (play-history / nested PlayCtx)
- **status**: **partially fixed** (2026-10-13, `closeout-3-windows`) — join retarget
  (`改为你`) landed for 真奈 (CH-06 / g19 green). `Trigger.join_redirect` /
  `join_used` + `prop::SELF_REDIRECT` + `ctx::join_redirect_target()` /
  `set_join_used()` / `self_target()`. The pre-scan in `hand_counteractions`
  sets `join_redirect` when a `SELF_REDIRECT` counter is in a hand, so the
  redirect is live before any body runs. The full nested PlayCtx window (per-seat
  「如同自身的对应目标被指定一般」 with its own chain) and play-history query
  (Mortis copy) are still open.
- **kind**: ABI defect
- **class**: B
- **severity**: high (Mortis and Mana Champion are the two "copy a counter" cards)
- **suggested batch**: host ABI: out-buffers / counteraction windows
- **locations**
  - `rules/cards/card-mujica/src/mortis_instinct.rs:22` (CanCounteract delegates to copy), `:37` (copy walks `H._playHistory`)
  - `rules/cards/card-sumimi/src/mana_champion.rs:57` (nested `H.Counteract(copy, p)`, shield + counteractor's draw)
  - `rules/cards/card-mujica/src/dice_cast.rs:55` (「（此卡可以被反击）」 needs the reverse-order counter play)
- **rulebook**: Mortis 「此卡打出时效果为场上任意其他玩家打出的上一张卡…」 + 「（此卡复制[反击]卡时可在符合条件时打出）」; Mana Champion 「此时场上其他玩家可如同自身的对应目标被指定一般打出[反击]卡，且其反击卡中针对打出玩家自身的效果改为你…」
- **current**: join retarget works (g19). No play-history query; full nested PlayCtx window still TODO.
- **expected**: copy the latest eligible play and nest a full Play/Counteract run with forwarded Tags/Extreme/Doubled.
- **capability**: `play-history query` + `nested PlayCtx` (cross-card identity + counteraction mode).

## CH-06 — 网络链接异常 still drops the wrong designation in join fixtures
- **status**: **fixed** (2026-10-13, `closeout-3-windows`) — 真奈's join retarget
  (「改为你」) landed. `SELF_REDIRECT` on 真奈 marks the counter as opening a
  join; `Trigger.join_redirect` is set at declaration; 网络链接异常's
  self-targeted cancel now cancels 真奈's user's designation (not the joiner's).
  Joiner draws 1 (真奈's shield clause). `g19_manas_join_window_lets_a_third_party_drop_one`
  un-ignored, green. Remaining TODOs (nested window shape, fallback-draw
  joiner-count) are tagged `TODO(规则书)` under CH-05.
- **kind**: implementation bug
- **class**: B (capability: nested PlayCtx / join retarget — see CH-05)
- **severity**: high
- **suggested batch**: host ABI: counteraction windows
- **locations**
  - `rules/tests/tests/gap_window.rs` `g19_manas_join_window_lets_a_third_party_drop_one` (un-ignored, green)
  - `rules/cards/card-sumimi/src/mana_champion.rs` (join retarget + fallback draw)
  - `rules/cards/card-general/src/net_error.rs` (join-aware cancel)
- **rulebook**: sheet cancels **one** designation (「取消一个指定」 / 「改为你」).
- **was**: P3's cancel dropped P3's own designation (P3 exempt, P0 pays).

## CH-07 — 「视为此卡未生效」 has no observer
- **status**: **minimal path landed** (2026-10-13, `closeout-3-windows`) —
  无路矢's refused-exile early-out is the first observer: the card is spent and
  does nothing (no place / no designate / no later teleport) when glossary 14
  refuses the [除外] cost. The general took-effect return value + `CardWasted`
  (C-Q5) is still open.
- **kind**: rulebook divergence + ABI defect
- **class**: C (semantics) then B (`PlayCtx.Effective` / return-value)
- **severity**: high (appears on many short-pay cards)
- **suggested batch**: skills: MyGO / HHW cards / host ABI
- **locations** (one root cause, many cards)
  - `rules/cards/card-mygo/src/haneoka.rs:52` (「放入弃牌堆且视为此卡未生效」 — card *is* discarded)
  - `rules/cards/card-mygo/src/no_road.rs:48`
  - `rules/cards/card-mygo/src/miracle.rs:138` (（3） crystals left on discard → 未生效; wants `CardWasted`)
  - `rules/cards/card-roselia/src/fire_bird.rs:35`
  - `rules/cards/card-hhw/src/kokoro_circle.rs:49`
  - `rules/cards/card-morfonica/src/starry_night.rs:53`
  - `rules/cards/card-morfonica/src/noble_blue.rs:32`
  - related: `rules/cards/card-hhw/src/believe_you.rs` wasted flag
- **rulebook**: 「视为此卡未生效」 (haneoka / fire_bird short-pay / starry_night / noble_blue / kokoro_circle / no_road / miracle (3)).
- **current**: clause names a state with no observer. `PlayCtx.Effective = false` is the C# side channel and is not being ported (routine should *return* whether it took effect).
- **expected**: rule (a) does the card get spent / discarded (haneoka says yes; noble_blue/starry_night "spent anyway" implies no), (b) what counts as a "use" this suppresses (later 「抵消一次任意付款」 etc.).
- **question**: C-Q5.
- **capability (after the ruling)**: routine return value for took-effect + `CardWasted` (C# `H.CardWasted`).

## MOVE-01 — forced stop (`plan::set_stop_at`) does not land the piece on the hook tile
- **kind**: test-harness drive / arrangement (not an engine defect)
- **class**: D (fixed 2026-10-10; see D-08)
- **severity**: high (凑友希那 (3) / 香澄 stop-pots are common)
- **suggested batch**: movement
- **locations**
  - `rules/tests/tests/cross_move.rs` `m12a_yukina_forced_stop` / `m12b_yukina_stop_countered` / `m15_first_forced_stop_wins` (un-ignored, green twice)
  - engine landing already correct: `settle_stages.rs` `m4_kokoro_force_stop_stops_a_passer`, `gap_move.rs` `g11`/`g12`, `roselia.rs` yukina stop-pot
- **rulebook**: 「令该玩家强制停下并[触发结算]」 — the passer stops on the named tile and settles there.
- **was**: m12a/m12b declined the `ask_yes` offer (option matcher missed `ask.yes`/`ask.no`); m15 used `place_raw` (tile:-1) so the `card.placed && self_tile()` pre never matched. m12b also pinned 41 instead of the rolled dest 37.
- **fixed**: match the `kokoro_practice_force` dump and `answer_one(0)`; `place_on_tile` both stop-pot cards; assert 37 for the countered stop. Un-ignored, green twice.

## MOVE-02 — 白金燐子 (2) spends fire / prompts X but never moves
- **kind**: test-harness drive shape (not an engine defect)
- **class**: D (fixed 2026-10-10; see D-06)
- **severity**: high (Roselia staple)
- **suggested batch**: movement
- **locations**
  - `rules/tests/tests/cross_move.rs` `m10_rinko_while_stunned` / `m11_rinko_plus_yolo` (un-ignored, green twice)
  - green twins already pin the drive: `roselia.rs` `rinko_skill_2_fixes_the_move_roll_to_x_times_six`, `gap_move.rs` `g09_fixed_roll_plus_starlit_reroll`
- **rulebook**: 白金燐子 (2) — 「你可以在移动掷骰前消耗X个火罐，使这回合移动掷骰的结果固定为X*6」. The press is a **pre-roll shape**; the player still takes the main roll.
- **was**: m10/m11 pressed the skill and drained prompts, then asserted pos without `t.roll(0)`.
- **fixed**: take the main roll after the X prompt (same drive as the green roselia/gap twins). Un-ignored, green twice. m11 records 14 (Y.O.L.O adds +2 over the fixed 12; the 「固定」 vs additives question stays open as C-Q24-adjacent RULING in the test).

## MOVE-03 — MyGO band (2) move-1 replacement never fires / multi-activation `On::Play`
- **status**: **fixed** (2026-10-13, `closeout-2-movement`) — the multi-activation
  half was already merged into one `On::Play` with a which-effect prompt (the
  green twins `band_move_one_for_a_crystal` / `band_draw_for_two_crystals` /
  `band_press_offers_both_effects_when_both_are_available`). `m22_mygo_band_move1`
  was a **D** test drive: it rolled first and looked for a window inside
  `t.roll`, but 「于移动掷骰前选择移动1格以替代移动掷骰」 is a skill press
  *before* the roll (`t.skill`). Un-ignored and green with the green twin's
  drive. NNM 「发动以下效果中的一个」 is a separate in-entry choice (C-Q6).
- **kind**: implementation bug + ABI defect
- **class**: B (multi-activation selection) unblocks A
- **severity**: high
- **suggested batch**: movement / skills: MyGO
- **locations**
  - `rules/tests/tests/cross_move.rs:927` `m22_mygo_band_move1` (un-ignored, green)
  - `rules/tests/tests/mygo.rs` `band_draw_for_two_crystals` / `band_move_one_for_a_crystal`
  - `rules/skills/skill-bands/src/mygo.rs` — one `On::Play` with a selection prompt
  - also Morfonica NNM: `rules/cards/card-morfonica/src/nanami_effort.rs`
  - TEST-FINDINGS §6 item 8 (multi-activation selection)
- **rulebook**: MyGO band (2) has several activations (move-1 replacement and a 2-crystal draw); NNM 「发动以下效果中的一个」.

## MOVE-04 — 若能再次交汇 re-roll loop hits `world_mut` while a routine is pending
- **kind**: test-harness sequencing (not a card/host defect)
- **class**: D (fixed 2026-10-10; see D-05)
- **severity**: high (re-roll cards are common)
- **suggested batch**: fuzz/determinism / movement
- **locations**
  - `rules/tests/tests/cross_move.rs` `m06_reunion_20_tile_cap` / `m06b_reunion_stops_on_passing` (un-ignored, green twice)
- **rulebook**: 若能再次交汇 — re-roll the move dice (with a 20-tile cap / stop-on-passing shape).
- **was**: `t.dice` after `t.roll` called `Match::world_mut` while the `moveRoll` [反击] window was a pending routine.
- **fixed**: queue all faces before `t.roll` (snapshot replay also means mid-window loads never reach the re-roll). Card (`meet_again.rs`) and host `do_move_roll` / `set_move_roll` write-back are correct and unchanged.

## MOVE-05 — mid-turn [停留] does not stop the main move
- **status**: **fixed** (2026-10-13, `closeout-2-movement`) — glossary 51
  「[停留]=无法移动」 is rulebook, so a stay gained in 运营 locks the turn's
  main move immediately. `give_stay` / `give_exile` now maintain the
  `skip_move` latch as a consequence of the state (set on gain, clear when the
  last layer drops and no [除外] holds the move); [不可阻挡] 「无视已存在的
  [停留]」 keeps the walk legal. `rain_stay_blocks_this_turn_move` un-ignored
  and green. C-Q7's *mid-walk* half (stay gained after the roll) is unchanged:
  the walk still completes — only the pre-roll lock is in scope here.
  Green twins that moved after a stay grant (`rain_extra_settle_at_turn_end`,
  `rain_day_drains_one_crystal_per_turn_end`, `g15_…`) now use [不可阻挡] or
  skip the roll, since the glossary forbids the move.
- **kind**: rulebook divergence
- **class**: A (glossary 51 is rulebook)
- **severity**: high
- **suggested batch**: movement
- **locations**
  - `rules/tests/tests/mygo.rs:285` `rain_stay_blocks_this_turn_move` (un-ignored, green)
  - `crates/game-core/src/engine/ops.rs` `give_stay` / `give_exile` (skip latch)
  - TEST-FINDINGS §9 (ileuxali: check once at move-branch selection)
- **rulebook**: glossary 51 「[停留]=无法移动」; 51 「[不可阻挡]」 「无视已存在的[停留]」.
- **was**: a [停留] gained mid-turn did not stop the current move (latch set only at turn start).
- **question**: C-Q7 mid-walk half still open (stay gained *during* a walk).

## SETTLE-01 — 笑容大游行 tile-swap overflows the stack
- **status**: stack overflow half **fixed** by STACK-01; swap-axes half **fixed**
  (this batch, `fix: SETTLE-01 …`) — (1)/(3) share one `SettleBody` (the host
  runs only the first hook of a kind), (3) redirects a settle on the card's
  tile to the 弦卷集团 agent body and then discards the card. `t13_smile_parade`
  un-ignored, green. **Open:** the reverse direction (a settle on 弦卷集团
  while the card sits elsewhere should run as the card's tile) is
  `TODO(规则书)` — a two-way swap re-enters and hits the settle-depth cap;
  a host-visible re-entrancy flag is needed (`SLOT_REPLAY` guest writes do not
  cross `card_settle_at`).
- **kind**: implementation bug
- **class**: A
- **severity**: high (HHW signature continuous)
- **suggested batch**: HHW cards / crystal/marker accounting
- **locations**
  - `rules/tests/tests/cross_tiles.rs` `t13_smile_parade` (un-ignored)
  - `rules/tests/tests/gap_res.rs` `g30_smile_parade_swap_with_a_field_card_in_place` (un-ignored)
  - `rules/cards/card-hhw/src/smile_parade.rs` (`settle_body` / `on_resolved`)
- **rulebook**: 笑容大游行 (3) 「[持续] 当此卡位于格子上时，那格视为与"弦卷集团"格子交换位置，任何玩家在此卡放置的格子上[触发结算]后此卡放入弃牌堆」.
- **fixed**: (1) end-as-agent via a one-shot move tag; (2) move-to-end + clear
  crystals; (3) one-way card-tile → agent body + discard after any other
  player's settle on the card's tile.

## SETTLE-02 — CRYCHIC band (1) at 6+ hand cards still lets money move
- **kind**: implementation bug
- **class**: A (money lock) / B (play gate, split below)
- **severity**: high
- **suggested batch**: skills: Roselia / payment pipeline
- **status**: **money half fixed** (this batch, `fix: SETTLE-02 …`) —
  `lock_pay` / `force_card` now scope to the skill owner (`你`): a payment
  between two other players is untouched. `s05` un-ignored, green twice.
- **locations**
  - `rules/tests/tests/cross_status.rs:159` `s05_crychic_band_six_cards` (un-ignored)
  - `rules/skills/skill-bands/src/crychic.rs` (`lock_pay` / `force_card`)
  - `rules/skills/skill-bands/src/crychic.rs` (「无法从手中打出任何牌」 — **B**, see below)
- **rulebook**: CRYCHIC band (1) at 6+ hand cards blocks gain/loss (sheet).
- **was**: `lock_pay` zeroed **every** payment in the game (the empty pre +
  owner-hand guard fired on any money movement).
- **expected**: no money gain/loss **for the skill owner** while the gate is up.
- **B split — 「无法从手中打出任何牌」**: a *global* play gate (any hand card,
  not one card's `cant_play`). The engine's play path consults the played
  card's own gate only. Capability: **`cantPlayHand`** (a `GateKind` /
  standing veto consulted on every hand play, the shape
  `On::Gate(&[GateKind::CantPlayHand], …)`). Not implemented here. The s05
  `t.play(1, FILL)` line is a record, not an assertion.

## SETTLE-03 — 丰川祥子 (1) does not absorb stays when passing another player
- **kind**: implementation bug
- **class**: A
- **severity**: high
- **suggested batch**: movement / skills: Mujica
- **status**: **fixed** (this batch, `fix: SETTLE-03 …`) — two defects: the
  test declined the YesNo (`option("转移")` never matches `ask.yes`/`ask.no`
  labels; match `sakiko_life_take` and answer `ask.yes`), and the body
  *copied* the layers instead of transferring (now `state::add` on the source
  before `give_stay`/`give_stun`). `s08` green twice. `g21` stays ignored for
  its separate 夏日合宿 half (P1 still receives a stay).
- **locations**
  - `rules/tests/tests/cross_status.rs:268` `s08_sakiko_absorbs_statuses` (un-ignored)
  - `rules/skills/skill-characters/src/sakiko_life.rs` (`on_pass_player` transfer)
  - `rules/tests/tests/gap_window.rs:221` `g21_summer_camp_immune_to_the_rains_zone` (still ignored: 夏日合宿)
- **rulebook**: 丰川祥子 (1) absorbs stays when passing another player.
- **was**: absorb did nothing on a pass (test declined the ask; body copied).
- **expected**: stays on the passed player are absorbed / cancelled as the clause says.

## WINDOW-01 — Welcome to Ave Mujica's World (1) cannot be played (`err.play_phase`)
- **status**: **fixed** (2026-10-11, `backlog-windows-a`) — **test bug**. `err.play_phase`
  was the harness leaving a turn-start skill hook prompt open (若叶睦 / 丰川祥子
  `ask_yes` at `TurnStartBefore`), so the match was `busy` and `cant_play`
  refused the play. Drain after `begin_turn`. The card itself was always playable.
- **kind**: test-harness drive
- **class**: A
- **severity**: high
- **suggested batch**: counteraction windows
- **locations**
  - `rules/tests/tests/cross_chain.rs` `c27_welcome_state_switch` (un-ignored, green twice)
  - `rules/cards/card-mujica/src/welcome_mujica.rs:31` (（1） 「若指定了不存在状态2的玩家则无效果」 reading — C, see CH-08)
- **rulebook**: 「转换任意一名玩家的状态」 (状态1 ↔ 状态2).
- **was**: play refused with `err.play_phase`.

## WINDOW-02 — R.I.O.T. + 宣战布告 same timing loses the 500 payment
- **status**: **fixed** (2026-10-13, `closeout-3-windows`) — a
  `DESIGNATES` play is one [反击] timing (CROSS-TESTS C13): its `Effect`
  counters also answer the play's own `card` raise. `effect::hits` falls back
  to the static `DESIGNATES` list when the effects list is empty. The offer
  scan, probe and **body dispatch** all use the same Effect-on-DESIGNATES-play
  entry lookup, so 宣战布告 is declared on the play and its 500 lands.
  `c13_riot_and_war_declaration` un-ignored, green (2026-10-11).
  `l01_counter_war_five_links_deep` — the five-link war now nests counters to
  counters (C answers B, D answers C, E answers A) instead of putting every
  counter on X's round. Un-ignored, green.
- **kind**: implementation bug
- **class**: A
- **severity**: high (war declaration is a whole-table event)
- **suggested batch**: counteraction windows / payment pipeline
- **locations**
  - `rules/tests/tests/cross_chain.rs` `c13_riot_and_war_declaration` (un-ignored, green)
  - `rules/tests/tests/cross_long.rs` `l01_counter_war_five_links_deep` (un-ignored, green)
  - `crates/game-rules/src/host.rs` `counteracts_to_play` / `counteract_entry`, `rules/card-sdk/src/ctx.rs` `effect::hits`
- **rulebook**: R.I.O.T. / 宣战布告 — 500 payment and the war ring offers.
- **was**: 500 missing; war never declared on the play's timing.

## WINDOW-03 — EXIST redirect / 「造成影响」 latch is at declaration, not settlement
- **status**: **partially fixed** (2026-10-13, `closeout-3-windows`) — the
  C12 overlap (EXIST × 夏日合宿) is fixed: `target_player` now runs Redirect
  **before** Untargetable, so EXIST 「目标将改为你」 re-names first and the gate
  checks the final name. `c12_exist_vs_summer_camp` un-ignored, green.
  The 「造成影响」 latch itself (declaration vs settlement, C-Q8) is still open.
- **kind**: rulebook divergence
- **class**: C
- **severity**: high
- **suggested batch**: counteraction windows / draw/discard pipeline
- **locations**
  - `rules/cards/card-ras/src/exist.rs:55` (「造成影响」 = effect resolved vs declaration)
  - `rules/tests/tests/gap_money.rs:334` `g07_exist_redirect_then_negated_designation` `#[ignore = "RULING: does a negated EXIST redirect count as 「造成影响」? assert draws 1"]`
  - `rules/tests/tests/cross_chain.rs:599` `c12_exist_vs_summer_camp` (找回珍妮弗 does not land on P2 with 夏日合宿 + EXIST)
- **rulebook**: EXIST — 「造成影响」 then draw.
- **current**: flag written at declaration (redirect re-names the recipient before the chain opens); a later negate still leaves the flag set and the draw unearned.
- **expected**: decide declaration vs settlement (「nothing landed on me」).
- **question**: C-Q8.

## FUZZ-01 — determinism / save-restore / unknown card id / negation totality
- **kind**: implementation bug
- **class**: A
- **severity**: high (breaks every sim / replay)
- **suggested batch**: fuzz/determinism
- **locations**
  - `rules/tests/tests/fuzz_found.rs:85` `unknown_card_id` (unknown card id in hand/draw/discard/field)
  - `rules/tests/tests/fuzz_found.rs:128` `determinism_break` (same seed + answers diverge)
  - `rules/tests/tests/fuzz_found.rs:142` `save_restore_break` (save/restore diverges)
  - `rules/tests/tests/fuzz_found.rs:160` `negation_not_total` (negating a no-target [手] did not reduce to never playing it)
  - `rules/tests/tests/fuzz_found.rs:186` `monotonicity_break` (a +N roll modifier shortened a move)
  - **D**: `card_trap_reachable`, `immunity_gap` ignores already removed
  - `rules/tests/tests/inline_provider.rs:247` B2 HostRequest learn-pass gap (see HOST-01 -- top-level pay path now pinned green; nested-settle remainder)
- **rulebook**: general engine invariants (same answers → same world; save/restore round-trip; negation is total; +N never shortens).
- **current**: each shape is a latent fuzz repro still ignored.
- **expected**: green under the existing `fuzz_found` assertions.
- **note**: may share one pipeline root cause (guest/world overlay + HostRequest ordering); investigate as one batch. HOST-01's top-level rebase is in (see its entry); the nested-settle remainder is the open part.

## HOST-01 — HostRequest B2 equivalence gap (learn pass vs live+host_effects)
- **kind**: ABI defect / implementation bug
- **class**: B
- **severity**: high (breaks inline guest equivalence under real rules)
- **suggested batch**: host ABI: out-buffers / HostRequest handlers
- **locations**
  - `rules/tests/tests/inline_provider.rs:247` (B2 equivalence gap TODO), `:343` (cross-ref)
  - `docs/BOT.md` §3.2
- **rulebook**: n/a — ABI contract: inline learn pass must see the same world as the replay model's last pass (`live + host_effects`).
- **current**: PARTIAL (top-level done; nested-settle remainder). Each pass shares `live` after `NeedHost`, so the **landing** pass reads `live + host_effects`; write-through replace-on-rerun means the event stream carries that pass's lines (`docs/ENGINE.md` "Drive replay semantics"). Pinned for the top-level `pay` path by `effect_applied_log::reread_after_host_effect_sees_the_updated_value`. **Remaining gap:** a write inside a host routine's *nested settle* (the agent-tile offer buying a deed during `HostRequest::Move`) may not be on live before the next pass shares -- `RAS:成为最强`'s ownership check still sees the pre-buy deed (`be_strongest_rolls_only_with_a_livehouse`).
- **expected**: rebase onto the updated live world between passes, including nested settle writes (the write journal this item names).
- **capability**: write journal on `Run` for nested settle writes. Top-level HostRequest sequencing is in.

---

# MEDIUM

## ABI-01 — turn-level dice-immunity flag + "this play adds dice" tag
- **kind**: ABI defect
- **class**: B
- **severity**: medium
- **suggested batch**: host ABI: out-buffers
- **locations**
  - `rules/cards/card-ras/src/crush_drum.rs:42` `TODO(规则书)[judgement](ABI)`
  - trigger payload is only `{Kind, Player, Card, Play}` — no `Play.Def.AddsDice`
- **rulebook**: Crush Drum 「（此卡可以被反击）」 / dice-adding immunity clause.
- **current**: no `H._turnCtx.DiceCardsImmune`, no `AddsDice` on the trigger.
- **capability**: `turn-level dice-immunity flag` + `Play.Def.AddsDice` on the card-play trigger.

## ABI-02 — move-tag hook `circleNormal` / 「正常进行而不受到其上的额外效果」
- **kind**: ABI defect
- **class**: B
- **severity**: medium
- **suggested batch**: host ABI: out-buffers / movement
- **locations**
  - `rules/cards/card-general/src/marina_work.rs:40`
  - `rules/cards/card-hhw/src/kokoro_circle.rs:101` (skips when `H.CircleNormal` — else 6,000 charged on every CiRCLE settle)
  - `rules/cards/card-mujica` / CiRCLE listeners
- **rulebook**: 「正常进行而不受到其上的额外效果」.
- **current**: tile extra effects still apply; trigger carries no tile id / no move tags.
- **capability**: move-tag hook (`circleNormal`) on the settle/trigger payload + tile id for logs.

## ABI-03 — counteraction history (`_counteractedAgainst`)
- **kind**: ABI defect
- **class**: B
- **severity**: medium
- **suggested batch**: counteraction windows
- **locations**
  - `rules/cards/card-mujica/src/cant_look_away.rs:48`
- **rulebook**: filter on players counteracted this turn.
- **current**: `ctx::turn_key()` covers half the filter; the list itself cannot be built.
- **capability**: per-player counteraction-history list on the turn.

## ABI-04 — targeting flags + force-target play tag
- **kind**: ABI defect
- **class**: B
- **severity**: medium
- **suggested batch**: HostRequest handlers
- **locations**
  - `rules/cards/card-ras/src/please_choose.rs:53,71` (forced play of a targeting card)
  - `rules/cards/card-ras/src/please_choose.rs:96` (（2） no-settle + band-skill `PassTile` fan-out)
  - `rules/cards/card-ag/src/tsugumi_can.rs:39` (`RangeCard` / `Extreme` not in trigger vocabulary)
- **rulebook**: 「立即打出一张可将你指定为目标的牌并将你指定为目标（之一）」; 「（不触发结算但视为可触发乐队技能）」.
- **current**: hand listing / `card_replayable` ready; `H.Target` / `Card.Targeting` / force-target tag missing.
- **capability**: `CardDef.targeting()` + `Normal` hand filter + force-target play tag + `BandBase.PassTile` fan-out.

## ABI-05 — flip-effect trigger (PP band overflow)
- **kind**: ABI defect
- **class**: B
- **severity**: medium
- **suggested batch**: host ABI: out-buffers / skills: PP
- **status**: **fixed** (this batch, `fix: ABI-05 …`) — `TriggerKind::FlipEffect` /
  `HookKind::FlipEffect` (94) raised with the clause's intended X *before* the
  counters move. `PP:有你与我在这里共度` raises it; the PP band (2) hooks it and
  adds `X - owned` overflow crystals. Test: `band_skill_overflow_to_crystals`.
- **locations**
  - `rules/tests/tests/pp.rs:877` `band_skill_overflow_to_crystals`
- **rulebook**: PP band (2) 「你因任意原因受到将X个反面[P✽P粉丝]变正的效果且X大于拥有数时」 — passive hook on a fan-*flip*.
- **current**: no flip-effect trigger; overflow never becomes crystals.
- **capability**: flip-effect trigger (or fold into fan-spend / fan-gain points — see C-Q9).

## ABI-06 — world snapshot / restore (「取消所有受到的效果」)
- **kind**: ABI defect
- **class**: B (after C-Q10 on R3 timing)
- **severity**: medium
- **suggested batch**: settle stages
- **locations**
  - `rules/cards/card-sumimi/src/no_breakup.rs:124` (R3 `settleAfter` vs `tileResolved`; revert half not wired)
  - `rules/cards/card-ag/src/ran_as_usual.rs` (undo uses `turn_snap` of 4 fields only — incomplete vs C# full snapshot)
  - `rules/cards/card-ag/src/one_of_us.rs:138,200` (「不受其他任何效果影响」 raw money add vs `transfer`)
- **rulebook**: 「取消所有受到的效果（不进行任何结算）」 / 「若传送并触发结算后未能使资金变为拥有相同数字，回到原处并取消所有受到的效果」.
- **current**: no world-snapshot/restore; teleport settles and consumes the main move even when the revert should fire.
- **capability**: world snapshot / restore (pos, stay, stun, exile, money, owners, houses, mortgage).

## ABI-07 — deck-inspection hook (可能性为∞)
- **kind**: ABI defect
- **class**: B
- **severity**: medium
- **suggested batch**: draw/discard pipeline
- **locations**
  - `rules/cards/card-pp/src/infinite_possibility.rs:30`
- **rulebook**: [特]「观看卡组并观看到此卡时将此卡展示给所有玩家并将其放置在自己[场上]（次效果优先于其他后续效果）」.
- **current**: nothing observes a deck inspection.
- **capability**: deck-inspection interrupt hook (`H.Present` / reveal-while-looking) + `PlaceCard` before the rest of the look.

## ABI-08 — `FromDeck` play-source + one-shot `DiscardNextFx`
- **kind**: ABI defect
- **class**: B
- **severity**: medium
- **suggested batch**: draw/discard pipeline
- **locations**
  - `rules/cards/card-mujica/src/sakiko_cut.rs:42` (FromDeck 3-way prompt)
  - `rules/cards/card-mujica/src/sakiko_cut.rs:102` (「若无手牌则弃掉下一张抽到的牌」 — `Drew` hook kind exists (ABI v23); temporary `H.ExtraOf` attachment cannot outlive the play)
- **capability**: `PlayCtx.FromDeck` + `ExtraOf` one-shot attachment that outlives the play.

## ABI-09 — event defer (`IEventDefer`) + host clocks
- **kind**: ABI defect
- **class**: B
- **severity**: medium
- **suggested batch**: HostRequest handlers
- **locations**
  - `rules/cards/card-ppp/src/arisa_wait.rs:36` (defer event to owner's turn start + extra 流星堂 draw)
  - `rules/events/src/bangdream_chan.rs:35` / `rules/tests/tests/events.rs:1047` `bangdream_chan_shortens_the_clocks` (免费时间/恢复时间 are host clocks)
  - `rules/events/src/a_a_o.rs:28` (`CannotPlay` / `Fx.CantPlayHand` no engine field)
  - `rules/events/src/tsurumaki_estate.rs:37,53` (「竞价」 auction primitive + free-amount `ask_money`)
  - `rules/events/src/ex_quest.rs:36` (skin-change ask)
- **capability**: `IEventDefer` (Defer/Resolved), host clock write, `CantPlayHand` gate, auction primitive, `ask_money`, cosmetic ask.

## ABI-10 — skill-targeting exemption 「距离最近」 + per-card Mem state
- **kind**: ABI defect
- **class**: B
- **severity**: medium
- **suggested batch**: skills: Roselia / host ABI
- **locations**
  - `rules/cards/card-roselia/src/lisa_bond.rs:71` (ignore 「距离最近」 then discard)
  - `rules/cards/card-pp/src/hina_sound.rs:60,64` (RealId on placed substitute; `H.TryResonance` + `Fx.Actions` swap)
  - `rules/cards/card-pp/src/shine_again.rs:35` (「[共鸣]」 timing)
- **capability**: persistent skill-targeting exemption on a placed card; per-card Mem (`RealId`); `TryResonance` + `Fx.Actions`.

## ABI-11 — lost-since-turn accumulator (NOW SUMIMI)
- **kind**: ABI defect
- **class**: B
- **severity**: medium
- **suggested batch**: payment pipeline
- **locations**
  - `rules/cards/card-sumimi/src/now_sumimi.rs:63` (「失去资金的总额即将超过」 — `H._lostSinceTurn`)
- **capability**: per-player money-lost-since-turn accumulator (or `PayAfter` that sees the running total).

## ABI-12 — `tile_prop` cannot distinguish `FORCE_STAYS_MORTGAGED (=0)` from absent
- **kind**: ABI defect
- **class**: B
- **severity**: medium
- **suggested batch**: payment pipeline / crystal/marker accounting
- **locations**
  - `crates/game-core/src/engine/play.rs:2377`
  - `rules/card-sdk/src/abi.rs` property map (v30)
- **capability**: tri-state tile prop (absent / clear / set) for mortgage-stays.

## SKILL-01 — hook-guard dispatch does not see `skillBlock` (丸山彩 (2) under 初次演出事故)
- **kind**: implementation bug
- **class**: B (contained)
- **severity**: medium
- **suggested batch**: skills: PP
- **status**: **fixed** (this batch, `fix: SKILL-01 …`) — the residual guard
  *was* consulting `skill_blocked` correctly; the test's `set_fans` helper
  replaced the whole tokens vector and wiped `skillBlock`. The helper now
  updates the two fan counters in place. Test: `accident_blocks_pp_skill_2`.
- **locations**
  - `rules/tests/tests/pp.rs:1078` `accident_blocks_pp_skill_2`
- **rulebook**: 初次演出事故 blocks the PP skill; 丸山彩 (2) PayChoose hook must not prompt.
- **current**: `skillBlock` token is set (`tok=1`) but the guard/body `skill_blocked` check does not see it.
- **expected**: hook-guard dispatch consults the token.

## SKILL-02 — 冰川日菜 (2) lottery borrow never runs
- **kind**: implementation bug
- **class**: A
- **severity**: medium
- **suggested batch**: skills: PP
- **locations**
  - `rules/tests/tests/pp.rs:1128` `hina_skill_2_borrows_skill` (`skill.hinaLottery.borrowed` stays -1)
- **rulebook**: 「每回合开始时投掷1d4…借用对应（2）」.
- **current**: `place_raw` + `begin_turn` never reaches `borrowed`; the test also pins a 0-based scratch index (fix the test to pin the face in the log).
- **expected**: roll 1d4 at turn start and borrow the matching (2).

## SKILL-03 — stun waiver 「此技能可以正常使用或在[晕眩]状态下使用」
- **kind**: ABI defect
- **class**: B
- **severity**: medium
- **suggested batch**: skills: Roselia
- **locations**
  - `rules/cards/../skills/skill-characters/src/rinko_1cm.rs:58`
- **capability**: per-card waiver of the engine's generic stun gate (`CardDef` flag or play-guard order).

## SKILL-04 — 状态2 exit conditions missing (several character skills)
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: skills: Mujica
- **locations**
  - `rules/skills/skill-characters/src/kaede_support.rs:284` (「直到状态2结束为止」 with no clause that ends it)
  - `rules/skills/skill-characters/src/mumei_streamer.rs:131` (no exit; 「退出状态2后，被你翻面的卡自动翻回」 has no moment)
  - `rules/skills/skill-characters/src/mutsumi_actor.rs:89` (状态2 character swap: 「上一位打出过手牌的玩家」 latch + 「角色卡堆」)
  - contrast: 三角初华 fire-pot cap, 若叶睦 hand-size exit (those are stated)
- **question**: C-Q11.

## SKILL-05 — CRYCHIC band (3) cash-out skipped on nested `transform_now`
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: skills: Roselia / CRYCHIC
- **locations**
  - `rules/skills/skill-bands/src/crychic.rs:79` (「当此卡移除时」 should fire on the (2)-driven 「移除此卡」; nested run has no running instance so `crystals()` reads 0)
  - `rules/cards/card-crychic/src/mutsumi_never.rs:32` (C# narrows (2) to `BandCrychic` specifically — wider 「乐队技能」 reading taken)
  - `rules/skills/skill-bands/src/crychic.rs:155` (「无法从手中打出任何牌」 global play gate)
  - tests: `card_paths::mutsumi_never_2_*`
- **question**: C-Q12 (does (3) cash out on the (2)-driven removal? is (2) limited to BandCrychic?).

## SKILL-06 — deck-construction / hand-building rules with no vocabulary
- **kind**: ABI defect / rulebook divergence
- **class**: B
- **severity**: medium
- **suggested batch**: skills: CiRCLE / sumimi
- **locations**
  - `rules/skills/skill-bands/src/circle_staff.rs:77,81` (「只能在卡组中加入"通用"卡」; 「（除将牌加入卡组的效果以外）」 carve-out)
  - `rules/skills/skill-bands/src/sumimi.rs:81` (「你可携带两个角色的专属卡牌」)
  - `rules/skills/skill-bands/src/poppin.rs:315` (「所有Poppin' Party角色视为同时拥有"星之鼓动山丘"」 co-ownership + buy lock)
  - `rules/skills/skill-characters/src/lisa_goddess.rs:124` (temporary fire-pot provenance / 「被冲榜类效果被动消耗无需支付」)
  - `rules/skills/skill-characters/src/mutsumi_crychic.rs:125`, `saki_crychic.rs:131` (「只有洗牌时可以…添加奇迹水晶」 untagged carve-out)
  - `rules/skills/skill-characters/src/yuri_crit.rs:95` (「复制者」 / 「（抵押或赎回以外）」)
  - `rules/skills/skill-characters/src/tae_police.rs:100` (「[主]效果」「[手]效果」 tag query)
  - `rules/skills/skill-characters/src/soyo_clear.rs:114` (「以此法免除」 attribution)
- **capability**: deck-list / character-exclusive tag / co-owned tile / provenance-tagged spends / tag query on triggers.

## SKILL-07 — soyo 「持续至你的下回合开始」 recolour has no expiry
- **kind**: implementation bug
- **class**: A
- **severity**: medium
- **suggested batch**: skills: CRYCHIC / tiles
- **locations**
  - `rules/skills/skill-characters/src/soyo_clear.rs:85` (`colorFor:<p>` write does not expire)
- **rulebook**: 「持续至你的下回合开始」.
- **current**: write is permanent (matches the old behaviour, not the clause).
- **expected**: prop expires at the player's next turn start.

## SKILL-08 — Rana parking 「你与其他玩家重合时」 never fires (suspected bug)
- **kind**: implementation bug
- **class**: D (fixed 2026-10-10; see D-07)
- **severity**: medium
- **suggested batch**: movement / skills: MyGO
- **locations**
  - `rules/skills/skill-characters/src/rana_parking.rs` `on_overlap` (un-ignored via new test `rana_cat_overlap_takes_800_and_gives_a_parfait`, green twice)
- **rulebook**: 「你与其他玩家重合时」.
- **was**: pre is `MINE` (owner is the mover) but the body read `player_id` as "the other" and bailed when it equalled the owner — always.
- **fixed**: body reads `trigger::target()` (the other player already on the landing tile). Amount stays 800 (sheet / `characters.json`; older `rulebook.txt` prose says 1000).

## SKILL-09 — kaoru_prince 「每次[经过]或被[经过]时」 fires for every passer
- **kind**: stale
- **class**: D (already fixed; backlog entry was against an older revision)
- **severity**: medium
- **suggested batch**: movement
- **locations**
  - `rules/skills/skill-characters/src/kaoru_prince.rs` — Pass half is already `pre::MINE`; 被[经过] is a filtered `PassTile`
  - live pins: `hhw.rs` `kaoru_skill_1_own_walk_gains_one_pot_per_step` / `kaoru_skill_1_another_players_walk_gives_nothing` / `kaoru_skill_1_being_passed_gains_one_pot_not_one_per_step`
- **rulebook**: 「每次[经过]或被[经过]时」 — `Pass` half is "I pass" (`actor == owner`).
- **fixed already**: `On::Hook(&[HookKind::Pass], pre::MINE, …)` + `PassTile` for the 被[经过] half. Nothing to un-ignore.

## TILE-01 — Anon Tokyo link arrangements (Fire bird / Repaint / ONE OF US / linked rent)
- **status**: **fixed** (this batch, 2026-10-13, `closeout-4-tiles`) —
  t02 (Fire bird needs crystals to survive to the settle), t03 (P0 plays
  Repaint on P1's roll; plain /2, no ceil-10), t05 (ONE OF US share is a raw
  `adjust_money` split so FEVER! cannot boost it), t21 (play Anon Tokyo on
  P1's turn; arm [停留] after `until_turn`). Un-ignored, green.
- **kind**: implementation bug + test setup
- **class**: A (arrangements) + C (mortgaged partner half)
- **severity**: medium
- **suggested batch**: rent multipliers / tiles
- **locations**
  - `rules/tests/tests/cross_tiles.rs` `t02_anon_link_plus_fire_bird` (green)
  - `rules/tests/tests/cross_tiles.rs` `t03_anon_link_plus_repaint` (green)
  - `rules/tests/tests/cross_tiles.rs` `t04_anon_link_mortgaged_partner` (**RULING** — left ignored)
  - `rules/tests/tests/cross_tiles.rs` `t05_one_of_us_shares` (green)
  - `rules/tests/tests/cross_tiles.rs` `t21_riki_plus_anon_link` (green)
  - `rules/cards/card-ag/src/one_of_us.rs` (`ctx::lose_raw` / `gain_raw`)
  - `rules/card-sdk/src/ctx.rs` (`adjust_money` — raw money move, no pipeline)
- **question**: C-Q13 (mortgaged partner tile's half 收费) — still open.

## TILE-02 — agent half-rent 「同色」 vs colour overrides
- **status**: **fixed** (this batch, 2026-10-13, `closeout-4-tiles`) —
  「无主或玩家拥有的」 reads as unowned **or any player-owned**: unowned →
  buy, own → build, another seat's non-mortgaged deed → the agent's
  「半价收费的[结算]（向上取整10）」. `ANY_COLOR` still widens the colour set
  for buy/rent but is never buildable. soyo's PayMul halves again on top.
  `t16_soyo_mixed_colors` un-ignored, green (R/2/2).
- **kind**: rulebook divergence (resolved)
- **class**: C
- **severity**: medium
- **suggested batch**: rent multipliers
- **locations**
  - `rules/tiles/src/agent.rs` + `crates/game-core/src/engine/play.rs` (`agent_landing`)
  - `rules/tests/tests/cross_tiles.rs` `t16_soyo_mixed_colors` (green)
  - TEST-FINDINGS §6 item 10 / Ruling 4–5
- **question**: C-Q14 — superseded by the above reading.

## TILE-03 — RiNG rent table / mortgageability / `ringMultiplier`
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: rent multipliers / tiles
- **locations**
  - `rules/tiles/src/ring.rs:13` (dice-rent table + 「至少1个」 floor have no book clause)
  - `crates/game-core/src/state.rs:455` / `rules/card-sdk/src/abi.rs:572` / `rules/card-sdk/src/ctx.rs:453` (`ring_multiplier` TODO)
  - `docs/TILES.md` TODOs; TEST-FINDINGS §6 items 9 & 11
  - no black-box test
- **rulebook**: 「所有RiNG不可升级」 is the only RiNG clause; 游戏流程 5 「抵押拥有的地契」 with no exclusion (engine refuses RiNG mortgage).
- **question**: C-Q15.

## TILE-04 — 学生会的检查 placement / forced stop
- **kind**: test window / arrangement
- **class**: D (fixed 2026-10-10; see D-09)
- **severity**: medium
- **suggested batch**: movement
- **locations**
  - `rules/tests/tests/cross_tiles.rs` `t12_student_council_check` (un-ignored, green twice)
  - body already correct (same shape as green 香澄 / `student_council_check_adds_juewu_not_ya`)
- **rulebook**: 学生会的检查 — placement + forced stop + 「本回合无法加盖房屋」 (linger `NO_BUILD` already landed).
- **was**: the test `t.play`'d a SettleBefore [反击] as a free [手] (silent no-op) and declined the allowNone stand-in; `t1 - 2` also usize-underflowed on 购物中心 = 1.
- **fixed**: drive the SettleBefore window (roll + `counteract`), answer yes then the tile, wrap the start index. Un-ignored, green twice.

## TILE-05 — Parking Space + 要乐奈 (2) teleport arrangement
- **kind**: test setup
- **class**: D (fixed 2026-10-10; see D-09)
- **severity**: medium
- **suggested batch**: movement
- **locations**
  - `rules/tests/tests/cross_tiles.rs` `t24_parking_space` (un-ignored, green twice)
  - engine path already green in `g15_parking_space_settle_replaced_plus_rain_extra_settle`
- **rulebook**: Parking Space body-replace + Rana (2) Space teleport as the main move.
- **was**: P0 played Parking Space on P1's turn (`err.play_phase`); stay was read before `TurnEndAfter`.
- **fixed**: setup on P0's turn; assert stay after `pass`. Un-ignored, green twice.
- **open C (owned-Space body-replace order)**: see C-Q43 — only needed when Space has a deed owner; the CROSS-TESTS T24 path is unowned.

## TILE-06 — 江户川乐器店 / CiRCLE shared body
- **kind**: rulebook divergence
- **class**: C
- **severity**: low→medium
- **suggested batch**: tiles
- **locations**
  - `rules/tiles/src/edogawa.rs:9` (book names both in one sentence with different extra text)
  - `rules/tiles/src/circle.rs:29,59,129` (stunned card-only reward is a ruling `C44`/`D13`, not book; exile bar is engine bookkeeping)
- **question**: C-Q16 (only if the two shops ever diverge).

## DRAW-01 — Maya (2) look-at-top kept card does not raise `drew`
- **kind**: implementation bug
- **class**: A
- **severity**: medium
- **suggested batch**: draw/discard pipeline
- **status**: **fixed** (this batch, `fix: DRAW-01 …`) — a `DrewBefore`
  replacement's `add_to_hand` now pushes to the run's `draw_log`, so the after
  points (`drawn` / `drew`) fire for the kept card (「此次加手视为抽卡动作」).
  Test: `g28_maya_look_at_top_still_counts_as_a_draw`.
- **locations**
  - `rules/tests/tests/gap_res.rs:209` `g28_maya_look_at_top_still_counts_as_a_draw`
- **rulebook**: look-at-top + keep is a draw for 「梦在前方」's crystal.
- **current**: `drew` not raised on the kept card.
- **expected**: raises `drew` like any draw.

## DRAW-02 — 星月夜 even-roll crystal-removal offer never prompts
- **kind**: implementation bug
- **class**: A
- **severity**: medium
- **suggested batch**: draw/discard pipeline / skills: Morfonica
- **status**: **fixed** (this batch, `fix: DRAW-02 …`) — the hook did prompt;
  `attack` billed the *roller* (C#) while the sheet's omitted subject of 「可消耗」
  is the card controller. The owner now pays 1,000 and removes a crystal (net 0
  with （3）'s payout). Test: `starry_even_roll_of_another_player_offers_crystal_removal`.
- **locations**
  - `rules/tests/tests/morfonica.rs:304` `starry_even_roll_of_another_player_offers_crystal_removal`
  - `rules/cards/card-morfonica/src/starry_night.rs`
- **rulebook**: 「当其他玩家的移动掷骰为偶数时可[支付]1000资金并移除此卡一个[奇迹水晶]」.
- **current**: engine never prompts.

## DRAW-03 — 游击演出 [特] window at turn end never opens
- **kind**: implementation bug
- **class**: A
- **severity**: medium
- **suggested batch**: skills: RAS
- **status**: **fixed** (this batch, `fix: DRAW-03 …`) — own-card `Discarded`
  dispatch (the card that entered the discard hears its own discard) plus a
  `Discarded` hook that arms the turn-end check (C# `OnDiscarded`). The test
  now discards through the engine (`discard_card`) rather than a raw
  `set_discard`. Test: `guerrilla_special_offers_to_mark_a_deed_as_livehouse`.
- **locations**
  - `rules/tests/tests/ras.rs:1411` `guerrilla_special_offers_to_mark_a_deed_as_livehouse`
- **rulebook**: [特] (card in discard + ≥1000 paid in settle + no buildable livehouse owned or purchasable) at turn end.
- **current**: card stays in the discard with no prompt.
- **expected**: the [特] offer opens at turn end when the gate holds.

## MONEY-06 — 「支付」 reach of a non-settlement loss
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: payment pipeline
- **locations**
  - `crates/game-core/src/engine/play.rs:2266` (`scale_settle_payment` only scales settle payments; `gain` never scaled)
- **question**: C-Q17.

## MONEY-07 — Popipapapipopa crystal spend vs owner rent
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: crystal/marker accounting
- **locations**
  - `rules/tests/tests/gap_res.rs:133` `g26_popipa_crystals_and_kasumi_fire_on_one_settle`
- **rulebook**: 「消耗或支付」 — owner's own outlay vs a rent the owner receives.
- **question**: C-Q18.

## MONEY-08 — 分摊 remainder (1000 % n)
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- **suggested batch**: payment pipeline
- **locations**
  - `rules/events/src/front_or_back.rs:49,56`
- **question**: C-Q19 (who gets the remainder).

## MOVE-06 — E14 0-step case: [经过] vs [重叠]
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: movement
- **locations**
  - `crates/game-core/src/engine/play.rs:1298` (implemented as E14 writes it: 重叠 only)
  - `rules/tests/tests/rulebook.rs:866`
- **rulebook**: `E14` names only [重叠] for the 0-step case; `B41`+`E13` read as both-fire for a teleport.
- **question**: C-Q20.

## MOVE-07 — teleport as the main move (settle shape)
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: movement
- **locations**
  - TEST-FINDINGS §6 item 1
  - `rules/tests/tests/general.rs` `tsugu_lt26_choose_move_settles` (pins the immediate offer)
- **question**: C-Q21.

## MOVE-08 — want_to_grab (2) 「经过你」 = passTile vs passPlayer (R5)
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: settle stages
- **locations**
  - `rules/cards/card-crychic/src/want_to_grab.rs:29` (R5; left as `PassPlayer` C#-carried)
  - `docs/rulebook/SETTLE-STAGES.md` §9 R5
- **question**: C-Q22.

## MOVE-09 — want_to_grab (3) ray-nearest board grid geometry
- **kind**: ABI defect
- **class**: B
- **severity**: medium
- **suggested batch**: movement
- **locations**
  - `rules/cards/card-crychic/src/want_to_grab.rs:78`
- **rulebook**: 「[传送]至一个与自身所在格正上，正下，正左，正右直线距离最近的格子…并[触发结算]，视为你的主要移动。」
- **capability**: board grid geometry ray-nearest search + `plan::set_teleport_to` teleport-with-settle (`H.MainMoveAs`).

## MOVE-10 — 若宫伊芙 (2) fan flip + Y.O.L.O dice accounting
- **status**: **fixed** (2026-10-13, `closeout-2-movement`) — **D** test drive.
  Eve's (2) is an `ask_number` 0..=up choice (options are `ask.intOption`), so
  Y = 2 is `answer(0, 2)` — the same drive as the green twins
  `g10_eve_plus_yd4_on_a_2d20_move` / `g32`. The old matcher looked for
  「若宫伊芙」/「粉丝」 in the options (never present) and declined, so only
  Y.O.L.O's 1d4 landed (pos 5 = 4+1). Un-ignored and green (pos 8 = 4+2+2).
- **kind**: test-harness drive
- **class**: D
- **severity**: medium
- **suggested batch**: movement / draw/discard pipeline
- **locations**
  - `rules/tests/tests/cross_move.rs:140` `m03_eve_then_yolo` (un-ignored, green)

## MOVE-11 — 是我自己的问题 still fires after an abnormal move effect
- **status**: **fixed** (2026-10-13, `closeout-2-movement`) — 「（2）若受到
  [异常移动效果]影响，此卡不生效」 now gates the [反击] offer itself
  (`can_counteract`). The signal is the longer-lived `abnormalHit` latch (set
  when an abnormal lands, cleared when the player's next [主要移动] completes),
  so a forced move on *another* player's turn still suppresses the owner's
  next main-move window. The test drive also had a **D** bug: the saki_move
  target pick is a `saki_move_title` choice (not `kind == "player"`) and the
  direction is `saki_move_dir` option 0 — without that the force-move never
  landed. Un-ignored and green.
- **kind**: implementation bug + test-harness drive
- **class**: A (+D)
- **severity**: medium
- **suggested batch**: counteraction windows / movement
- **locations**
  - `rules/tests/tests/cross_move.rs:1166` `m28b_my_own_problem_after_forced_move` (un-ignored, green)
  - `rules/cards/card-crychic/src/my_own_problem.rs` `can_counteract`
  - `crates/game-core/src/state.rs` `key::ABNORMAL_HIT`
- **rulebook**: sheet says it does **not** fire after an abnormal move effect.

## MOVE-12 — RAS band (1) cannot decline the second abnormal-move effect
- **status**: **fixed** (2026-10-13, `closeout-2-movement`) — the drop of the
  second+ hit sat on `HookKind::Abnormal` (the *outcome*, too late to cancel)
  with pre `actor == owner` (the causer, not the recipient). Moved to
  `On::Gate(&[GateKind::AbnormalGuard])` with pre `target == owner`, so the
  declaration gate refuses the second [异常移动效果] of the turn. The test
  drive also had a **D** bug (same saki_move matcher as MOVE-11) that left
  `err.play_phase` because the force-move never landed. Un-ignored and green.
- **kind**: implementation bug + test-harness drive
- **class**: A (+D)
- **severity**: medium
- **suggested batch**: counteraction windows
- **locations**
  - `rules/tests/tests/cross_move.rs:404` `m09_ras_band_first_abnormal_only` (un-ignored, green)
  - `rules/skills/skill-bands/src/ras.rs` `on_abnormal` (now `AbnormalGuard`)
- **rulebook**: RAS band (1) 「每回合可选择只获得第一次受到的[异常移动效果]」.

## MOVE-13 — 星月夜's re-roll scope (whole dice set vs base die)
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: movement
- **locations**
  - `rules/tests/tests/gap_move.rs:122` `g08_starlit_reroll_of_a_whole_3d20_set` (assert full set)
- **question**: C-Q23.

## MOVE-14 — 普通与理所当然 copy signed or unsigned 移动格数
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- **suggested batch**: movement
- **locations**
  - `rules/tests/tests/gap_move.rs:352` `g13_ordinary_copies_a_reversed_move_distance` (assert unsigned)
- **question**: C-Q24.

## MOVE-15 — heat_head 「减少1d6」
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- **suggested batch**: movement / events
- **locations**
  - `rules/events/src/heat_head.rs:57` (subtract from face vs `add_extra_dice(-1, 6)`; sheet says 减少1d6 not 少投1d6)
- **question**: C-Q25.

## SETTLE-04 — 线香花火 last crystal / extra turn / stun cascade (R-7)
- **status**: **fixed** (2026-10-13, `closeout-3-windows`) — C-Q26a ruling: the
  last crystal removal does **not** grant an extra turn. The terminal effect is
  discard + 2 [眩晕]. (Granting one would auto-skip it on the stun — `play.rs`
  `stunned` -> `end_turn` — and its 「每回合结束时移除一层」 tick would eat one of
  the two layers, netting 1.) `l07_extra_turn_cascade` un-ignored, green.
  C-Q26b (「until your next turn start」 vs an extra turn's start) is still open.
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: settle stages
- **locations**
  - `rules/tests/tests/cross_long.rs:522` `l07_extra_turn_cascade` (un-ignored, green)
  - `rules/cards/card-mujica/src/sparkler.rs` (last removal skips `give_extra_turn`)
  - TEST-FINDINGS §6 items 2 and R-7
- **question**: C-Q26b (does 「until your next turn start」 end at an extra turn's start?).

## SETTLE-05 — no_breakup R3 timing (settleAfter vs tileResolved)
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: settle stages
- **locations**
  - `rules/cards/card-sumimi/src/no_breakup.rs:124`
  - `docs/rulebook/SETTLE-STAGES.md` §9 R3
- **question**: C-Q27.

## SETTLE-06 — two halvings on one settle (香澄 + 祥，移动)
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: payment pipeline
- **locations**
  - TEST-FINDINGS §6 item 4 (engine pins ¼)
  - `rules/tests/tests/mujica.rs` `saki_move_halves_a_forced_stop_and_pay` (green)
- **question**: C-Q28.

## SETTLE-07 — [晕眩] vs a card's own follow-on gain
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: payment pipeline
- **locations**
  - TEST-FINDINGS §6 item 9
  - `rules/tests/tests/mujica.rs` `heart_rain_fallback_stun_and_money`, `heart_rain_nobody_in_range_fallback`
- **question**: C-Q29.

## CH-08 — welcome_mujica (1) 「若指定了不存在状态2的玩家则无效果」
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: skills: Mujica
- **locations**
  - `rules/cards/card-mujica/src/welcome_mujica.rs:31`
- **question**: C-Q30.

## CH-09 — dice_cast 「（此卡可以被反击）」
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: counteraction windows
- **locations**
  - `rules/cards/card-mujica/src/dice_cast.rs:85`
- **question**: C-Q31 (must other [反击]s still answer this card's own play window?).

## CH-10 — ran_as_usual (1) C# once-per-turn gate
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- **suggested batch**: counteraction windows
- **locations**
  - `rules/cards/card-ag/src/ran_as_usual.rs:27` (C# adds a once-per-turn gate the passage does not name)
- **question**: C-Q32.

## CH-11 — 「会在骗着买水晶的人」 offers 「不要背负期待」 as a crystal endpoint
- **status**: **fixed** (2026-10-13, `closeout-3-windows`) — 「此卡不受任何其他
  效果影响」 is a static [特] property (`prop::IMMUNE` / `FieldCard.immune`),
  stamped at placement from the card's static props, so `place_raw` and engine
  placement see it without running the play body. `c26_immunity_vs_crystal_mover`
  un-ignored, green.
- **kind**: implementation bug
- **class**: A
- **severity**: medium
- **suggested batch**: crystal/marker accounting
- **locations**
  - `rules/tests/tests/cross_chain.rs:1175` `c26_immunity_vs_crystal_mover` (un-ignored, green)
  - `crates/game-core/src/engine/ops.rs` `place_card_on` (stamps `immune` from `prop::IMMUNE`)
- **rulebook**: 「此卡不受任何其他效果影响」 — should not be a crystal endpoint.
- **was**: `set_self_immune` was a play-body side effect; `place_raw` never ran it.

## CH-12 — [不可阻挡] still grants new [除外] from 无路矢
- **status**: **fixed** (2026-10-13, `closeout-3-windows`) — glossary 14
  「无法获得新的[除外]层数」/「也不会获得新的层数」 now refuses new [除外]/[晕眩]
  in `abnormal_gate`. 无路矢's exile is its teleport cost, so a refused gain
  negates the card (CH-07 minimal: spent, no place / no designate / no later
  teleport). `g23_unstoppable_refuses_the_exile_cost` un-ignored, green.
- **kind**: implementation bug
- **class**: A
- **severity**: medium
- **suggested batch**: crystal/marker accounting
- **locations**
  - `rules/tests/tests/gap_window.rs:302` `g23_unstoppable_refuses_the_exile_cost` (un-ignored, green)
  - `crates/game-rules/src/wasm_rules.rs` `abnormal_gate` (unstoppable × Stun/Exile)
  - `rules/cards/card-mygo/src/no_road.rs` (early-out when the exile is refused)
- **rulebook**: glossary 14 「无法获得新的[除外]层数」.
- **was**: 无路矢 still granted 2 [除外] to the holder.

## CH-13 — Here the world does not land on the second card played in one turn
- **status**: **fixed** (2026-10-13, `closeout-3-windows`) — `TriggerKind::TwoCards`
  (「当有人同一回合内打出两张卡时」) is now raised on the play that makes it two
  hand-card plays in a turn (`TurnCtx.cards_played`), opening its own [反击]
  window. `c21_here_the_world` un-ignored, green.
- **kind**: implementation bug
- **class**: A
- **severity**: medium
- **suggested batch**: draw/discard pipeline
- **locations**
  - `rules/tests/tests/cross_chain.rs:957` `c21_here_the_world` (un-ignored, green)
  - `crates/game-core/src/engine/play.rs` `play_from_hand` (raises `"twoCards"`)
  - **D note**: `l06_one_draw_several_effects` ignore already removed (capture-vs-autoplay is C-Q33)
- **rulebook**: Here the world captures a drawn / second card.
- **was**: the TwoCards trigger was never raised.

## CRYSTAL-01 — crystal can pay a skill's fire cost (FirePaying)
- **kind**: ABI defect
- **class**: B
- **severity**: medium
- **suggested batch**: crystal/marker accounting
- **status**: **fixed** (this batch, `fix: CRYSTAL-01 …`) — `TriggerKind::FirePaying` /
  `HookKind::FirePaying` (78) raised inside `ctx::spend_fire` before the pots
  move; a hook that substitutes cancels the link. 不再迷茫 (2) hooks it and
  burns one crystal; (3) sweeps the card at 0 via `CounterChanged`.
  Tests: `light_crystal_pays_skill_fire_cost`, `light_removed_when_crystals_run_out`.
- **locations**
  - `rules/tests/tests/mygo.rs:126` `light_crystal_pays_skill_fire_cost`
  - `rules/tests/tests/mygo.rs:215` `light_removed_when_crystals_run_out`
  - `rules/cards/card-mygo/src/tomori_no_longer.rs:7,125`
- **rulebook**: 灯 不再迷茫 (2) 「你使用角色技能时可移除此卡上的一个[奇迹水晶]以代替此次技能的火罐消耗」; when crystals run out the card is [移除]d.
- **current**: `spend_fire` has no pre-spend substitution hook (only post-commit `fireSpent`); crystal count never empties.
- **capability**: `FirePaying` hook / substitute consult inside `spend_fire`.

## CRYSTAL-02 — miracle (3) / crystal-on-discard 「视为此卡未生效」
- **kind**: ABI defect
- **class**: B (after CH-07 ruling)
- **severity**: medium
- **suggested batch**: crystal/marker accounting
- **locations**
  - `rules/cards/card-mygo/src/miracle.rs:138` (needs `PlayCtx.Effective` / `H.CardWasted`)
- **capability**: `CardWasted` when a card is discarded with crystals left.

## CRYSTAL-03 — 绯红之魂 500 owed at crystal-spend time, paid after possible discard
- **kind**: implementation bug
- **class**: A
- **severity**: medium
- **suggested batch**: crystal/marker accounting / payment pipeline
- **status**: **fixed** (this batch, `fix: CRYSTAL-03 …`) — the 500 is paid in
  `pay_choose` as part of the crystal spend, before `add_crystals` raises
  `CounterChanged` (which rule (3) answers with the discard). The `PayAfter`
  tag is gone. Test: `ag::soul_last_crystal_still_pays_the_payee_500`.
- **locations**
  - `rules/cards/card-ag/src/crimson_soul.rs` (was `:126`; now paid inline in
    `pay_choose`)
- **rulebook**: rule (1) — 500 is owed the moment the crystal is spent; not conditioned on the card still being in play.
- **was**: paid after rule (3) may have discarded the card.
- **expected**: pay 500 as part of the spend, before the discard check.

## CRYSTAL-04 — NNM (2) 「获得x次经过CiRCLE时的资金奖励」 does not scale with the live reward
- **kind**: implementation bug
- **class**: A
- **severity**: medium
- **suggested batch**: crystal/marker accounting / rent multipliers
- **status**: **fixed** (this batch, `fix: CRYSTAL-04 …`) — each of the x
  rewards now runs the money half of the CiRCLE reward step (`circleAffected`
  between pick and payout), so Morfonica's 1000/1500/2000 cycle (and any other
  listener) scales it. Test: `nnm_option2_gains_x_times_circle_money_reward`.
- **locations**
  - `rules/cards/card-morfonica/src/nanami_effort.rs:114` (hard-codes 2000)
  - `rules/tests/tests/morfonica.rs:468` `nnm_option2_gains_x_times_circle_money_reward`
- **rulebook**: Sheet 2026-10-06 新卡组卡 G7 (2) 「获得x次经过CiRCLE时的资金奖励」 (was 「获得x*2000资金」); Morfonica rewrites the CiRCLE reward to 1000/1500/2000.
- **current**: hard-coded 2000.
- **expected**: query the live CiRCLE money reward (`circleAffected` listeners).

## CRYSTAL-05 — 育美标记 circle-tile case (C# only)
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- **suggested batch**: crystal/marker accounting / HHW cards
- **locations**
  - `rules/cards/card-hhw/src/hagumi_marks.rs:159` (C# `HagumiMarkFx.PassTile` circle-tile case not in the rulebook text; held)
- **question**: C-Q34.

## CRYSTAL-06 — 薰 charge/decay 「（充能3，衰减1）」 has no 0-clause
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- **suggested batch**: HHW cards
- **locations**
  - `rules/cards/card-hhw/src/kaoru_thief.rs:142` (keeps C# decay-out behaviour)
- **question**: C-Q35.

## CRYSTAL-07 — charity_show +2 steps not synced mid-walk
- **kind**: implementation bug
- **class**: A
- **severity**: low
- **suggested batch**: movement / HHW cards
- **locations**
  - `rules/cards/card-hhw/src/charity_show.rs:173` (`m.extra_steps` vs `TurnCtx::plan.extra_steps` not synced mid-walk)
- **expected**: +2 extends an in-flight walk (engine mirrors plan writes onto the live move).

## CRYSTAL-08 — ringing_bloom (2) 「视为」 max is snapshotted, not continuous
- **kind**: implementation bug
- **class**: A
- **severity**: medium
- **suggested batch**: rent multipliers / HHW cards
- **locations**
  - `rules/cards/card-roselia/src/ringing_bloom.rs:87` (snapshot at play; no `houseAdded` refresh; no fall path)
  - `:107` (（3） X = counted value — **C-Q36** if the user prefers real houses)
  - `:114` (removal path other than discard leaves tile props armed)
- **rulebook**: (2) 「视为」 is continuous (tracks the max as houses move).
- **expected**: track rises (and, if possible, falls) for the life of the card.

## SKILL-10 — misaki_other 「额外视为CiRCLE」
- **kind**: rulebook divergence + ABI defect
- **class**: C then B
- **severity**: medium
- **suggested batch**: tiles / skills: HHW
- **locations**
  - `rules/skills/skill-characters/src/misaki_other.rs:107` (per-player tile-kind override; what 「额外视为CiRCLE」 pays is not stated)
- **question**: C-Q37.

## EVENT-01 — 前往哈比内尔王国旅游 teleport timing
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: events
- **locations**
  - `rules/tests/tests/events.rs:338` `habinel_teleports_to_smile_ship` `#[ignore = "RULING: …"]`
- **question**: C-Q38.

## EVENT-02 — asukayama settle-on-land vs settle-free walk
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- **suggested batch**: events
- **locations**
  - `rules/events/src/asukayama.rs:12` (derived 迷子的追逐), `:21` (settle-on-land default)
- **question**: C-Q39.

## EVENT-03 — event text gaps (one-shot / count / 卡组 / 下2回合 / 同色)
- **kind**: rulebook divergence
- **class**: C (several small)
- **severity**: low
- **suggested batch**: events
- **locations**
  - `rules/events/src/embers.rs:65` (spread one-shot vs repeating), `:137` (copies removed but event stays)
  - `rules/events/src/fix_note.rs:39` (「累计…加盖后将此卡移除」 — 3 builds vs 1)
  - `rules/events/src/kizuna_music.rs:45` (「卡组」 = draw pile vs whole card pool)
  - `rules/events/src/lulu.rs:28` (under-500 teleport: payment prompt / force debt / fizzle), `:37` (real-world shout)
  - `rules/events/src/surprise_duel.rs:34` (「下2回合」 window)
  - `rules/events/src/tears.rs:16` (「同色地契」)
  - `rules/events/src/circle_rebuild.rs:108` (priority vs placement order — B if we add priority)
  - `rules/events/src/tsurumaki_estate.rs:81` (tie-break)
- **questions**: C-Q40 (batch).

## EVENT-04 — data tables duplicated in event bodies
- **kind**: implementation debt
- **class**: A (keep in step) / B (ctx read)
- **severity**: low
- **suggested batch**: events
- **locations**
  - `rules/events/src/end_it_all.rs:14` (`data/events.json` not reachable from `ctx`)
  - `rules/events/src/kizuna_music.rs:16` (`data/song_cards.json` ditto)
- **expected**: `ctx` can read the data file, or the table is generated.

## HOST-02 — 时点流程 tables missing from `data/rules.txt` + missing timing kinds
- **kind**: rulebook divergence / engine surface
- **class**: A (sync the tables) + B (timing kinds)
- **severity**: medium
- **suggested batch**: fuzz/determinism / settle stages
- **locations**
  - TEST-FINDINGS "Rulebook doc check" — `data/rules.txt` stubs 开始游戏 / 回合流程 / 行动阶段 / 支付阶段
  - missing trigger kinds: 行动阶段 3 / 6 / 11, 支付阶段 3 (「支付计算后」)
  - 支付阶段 1 / 5 / 7 shaping (one `effect` raise carries window 5's power at window 1's position)
- **expected**: sync the tables without rewording; raise the missing windows when a card hooks them.

## HOST-03 — guard-side kind-rejection lint (G3)
- **kind**: tooling gap
- **class**: A
- **severity**: low
- **suggested batch**: fuzz/determinism
- **locations**
  - `rules/cond/src/lib.rs:106` (`precheck` lint helper, GUARDS.md §5.2)

## HOST-04 — `tile_kind` / ring query missing on the ABI
- **kind**: ABI defect
- **class**: B
- **severity**: low
- **suggested batch**: tiles
- **locations**
  - `rules/cards/card-sumimi/src/sweet_escape.rs:18`, `now_sumimi.rs:29` (C# `TileData.kind == "ring"`; ABI has no `tile_kind`)

## HOST-05 — `ask_tile(allowNone: true)` / free-amount `ask_money`
- **kind**: ABI defect
- **class**: B
- **severity**: low
- **suggested batch**: HostRequest handlers
- **locations**
  - `rules/cards/card-ras/src/pareo_far.rs:48` (ask_tile allowNone)
  - `rules/events/src/tsurumaki_estate.rs:53` (ask_money)


## HOST-06 — HostRequest purchase replies always `1` (guest `bool` is a lie)
- **status**: **fixed** (2026-10-11, `backlog-host-abi-a`, `fix: HOST-06 …`) — `buy` /
  `build` / `mortgage` / `card_buy` / `card_build` / `card_acquire` /
  `card_mortgage` / `card_offer_build` / `card_raise_bought` now return
  `Flow<bool>` (`true` = the deed/house/mortgage actually committed), and the
  matching `apply_host_request` arms answer `did as i32` instead of `Ok(1)`.
  Guest callers that logged a buy on the old lie are gated on the real outcome
  (`tsugu_ycm`'s `tsugu_buy`, `tomoe_savior`'s `tomoe_savior_bought`);
  `dream_ahead` (spend 3 crystals only on success) and `afterglow` (log the
  free house only on success) already checked the bool and are now truthful.
  ABI v52 → **v53** (return-value meaning). Tests: `ag.rs`
  `band_free_house_skipped_when_build_refused` (NO_BUILD vetoes the free
  house, no `afterglow_free_house` log), `general.rs`
  `tsugu_buy_gate_refused_does_not_log_a_buy` (Poppin' hill `BuyGate` refuses
  the deed, no `tsugu_buy` log). `DrawEvent` / `PayRent` / `OfferBuy` /
  `OfferForceBuy` / `OfferBuildOne` still answer `1`: their guest wrappers are
  `asked()` (prompt sentinel), not a purchase bool.
- **kind**: ABI defect
- **class**: A
- **severity**: high (common play with buy/build hooks)
- **suggested batch**: HostRequest handlers / out-sentinel answers
- **locations**
  - `crates/game-rules/src/wasm_rules.rs` `apply_host_request` — `Buy` / `Acquire` / `AgentOffer` / `Build` / `OfferBuild` / `Mortgage` / `RaiseBought` now answer the real outcome
  - `crates/game-core/src/engine/play.rs` `card_buy` / `card_acquire` / `card_build` / `buy` / `build` / `mortgage` — `Flow<bool>`
  - consumers that trust the reply: `rules/cards/card-pp/src/dream_ahead.rs:193` (`if !ctx::card_buy(...) { return; }`), `rules/skills/skill-bands/src/afterglow.rs:34` (`if ctx::card_build(...) { log free house }`)
- **ABI contract**: `rules/card-sdk/src/ctx.rs` `card_buy` / `buy` / `card_build` / `acquire` `-> bool` = the real outcome (ABI v53)
- **was**: every purchase HostRequest resumed as `true` (`!= 0` on answer `1`)
- **expected**: `0` when the engine no-op'd (BuyGate cancel, already owned, `buyBefore` cancel, payment unpaid, `why_not_build_on`, `buildBefore` cancel, max houses). **Confirmed.**
- **impact was**: `dream_ahead` spent 3 crystals and logged "bought" after a refused buy; `afterglow` always logged a free house even when `card_build` refused; `tsugu_ycm` / `tomoe_savior` logged a buy on a refused commit

## HOST-07 — `ctx::buy_quotes` is not the hook-aware quote; ignores `player_id`/`kind`
- **status**: **fixed** (2026-10-11, `backlog-host-abi-a`, `fix: HOST-07 …`) —
  `hostfns::buy_quotes` now calls `CardWorld::buy_quotes`, which `Run`
  implements through the live `Cx` → `purchase::quote_for` /
  `CardRules::buy_quote` (the same hook-aware path `buy` charges). Still a
  query: no pause, no world write. `player_id` and `kind` select the buyer and
  the pipeline shape (`Force` / `Acquire` name the owner as seller). The
  default `CardWorld` impl stays the native land+houses figure for stub
  worlds. Stale "One `HostRequest::Quote`" doc comments removed
  (`ctx.rs` / `docs/PURCHASE.md`). ABI v53. Test: `general.rs`
  `agent_quote_carries_the_tsugu_buy_add` (agent offer quotes
  `max(0, native − 1500)` under Tsugu's linger `BuyAdd`).
- **kind**: ABI defect
- **class**: A
- **severity**: high (common play with buy-hook cards)
- **suggested batch**: host ABI: queries / quote==charge
- **locations**
  - `crates/game-rules/src/hostfns.rs` `buy_quotes` — now `c.st().wr().buy_quotes(...)`
  - `crates/game-rules/src/wasm_rules.rs` `Run::buy_quotes` — `purchase::quote_for(cx.rules(), …)`
  - `crates/game-rules/src/world.rs` `CardWorld::buy_quotes` default
  - guest caller: `rules/tiles/src/agent.rs:87` `ctx::buy_quotes(player_id, BuyKind::Agent, &same)`
  - hook cards that change the charge: `card-general/src/tsugu_ycm.rs` (`BuyAdd` −1500), `card-ppp/src/maze_warehouse.rs` (`BuySet` 0), Roselia band `BuyMul` ½
- **ABI contract** (`docs/PURCHASE.md`, `purchase.rs`): post-hook price and `BUYABLE + BuyGate`; quote == charge. **Confirmed.**
- **was**: guest quote = native land+houses; eligibility = `is_buyable` only
- **expected**: same `CardRules::buy_quote` / `quote_for` the engine uses. **Confirmed.**

## HOST-08 — `plan::set_no_build` / `plan::set_build_anywhere` trap on native, unregistered on wasm
- **status**: **fixed** (2026-10-11, `backlog-host-abi-a`, `fix: HOST-08 …`) —
  both have host bodies on native and wasm. `set_no_build(on)` folds into
  `set_can_build(!on)` (`World::set_no_build`, one plan flag). `set_build_anywhere`
  is a new `MovePlan.build_anywhere` flag that lifts `why_not_build`'s
  landing-only check. Registered in the `host.rs` linker beside `set_no_buy` /
  `set_can_build`; `native_shims` ops call the hostfns. ABI v53. Test:
  `game-core` `move_ctx::tests::no_build_folds_into_can_build`.
- **kind**: ABI defect
- **class**: A
- **severity**: medium (latent; no current caller in `card_all.wasm`)
- **suggested batch**: host ABI: plan flags
- **locations**
  - guest wrappers `rules/card-sdk/src/ctx.rs` `plan::set_no_build` / `set_build_anywhere`; imports `rules/card-sdk/src/ctx/sys.rs`
  - registered in `crates/game-rules/src/host.rs` linker (was a gap vs `set_no_buy` / `set_can_build`)
  - native `crates/game-rules/src/native_shims.rs` — host bodies (was a trap)
  - `World::set_no_build` / `set_build_anywhere` (`engine/ops.rs`); `MovePlan.build_anywhere` (`state.rs`); enforced `play.rs` `why_not_build`
- **was**: calling either API trapped on native; on wasm a module that kept the import failed to instantiate
- **expected**: `set_no_build(true)` ≡ `set_can_build(false)`; `set_build_anywhere` opens builds past the landing-only rule. **Confirmed.**

## HOST-09 — `set_bonus` dead guest import + native trap
- **status**: **fixed** (2026-10-11, `backlog-host-abi-a`, `fix: HOST-09 …`) —
  deleted. No `ctx` wrapper, never called, absent from the built wasm (GC).
  Import + `native_shims` stub + `bandori_set_bonus` symbol removed. Nothing
  to implement: no card text names a move-plan / dice bonus buffer.
- **kind**: ABI defect (dead surface)
- **class**: D (dead)
- **severity**: low
- **suggested batch**: guest imports vs host registration
- **locations** (removed)
  - `rules/card-sdk/src/ctx/sys.rs` `set_bonus`
  - `crates/game-rules/src/native_shims.rs` `op_set_bonus` / `bandori_set_bonus`
- **expected**: delete the import + native stub. **Confirmed.**

## HOST-10 — host linker names with no guest import (dead registrations)
- **status**: **fixed** (2026-10-11, `backlog-host-abi-a`, `fix: HOST-10 …`) —
  dropped the five unused linker names. The guest's real names stay
  (`set_extreme`, `trig_move_kind`, `trig_move_tag`, `trig_set_pay_target`,
  `trig_set_cancelled`); the hostfns they share are unchanged.
- **kind**: ABI defect (dead surface)
- **class**: D
- **severity**: low
- **suggested batch**: guest imports vs host registration
- **locations** (removed from `crates/game-rules/src/host.rs`)
  | host name | guest actually uses |
  |---|---|
  | `extreme` | only `set_extreme` (no reader) |
  | `move_kind` | `trig_move_kind` |
  | `move_tag` | `trig_move_tag` |
  | `set_trigger_target` | `trig_set_pay_target` (same body) |
  | `set_trigger_cancelled` | `trig_set_cancelled` |
- **expected**: drop the unused linker names. **Confirmed.**

## HOST-11 — CEL `roll_source` sentinel doc disagrees with guest/host
- **status**: **fixed** (2026-10-11, `backlog-host-abi-a`, `fix: HOST-11 …`) —
  the doc is aligned to the live encoding: `0 = none` (`roll_source::NONE`).
  The host fills `0` and `layer_keep` already codes against that; no migration.
  `ty: Ty::Int` is correct (a sentinel, not an optional).
- **kind**: ABI defect (sentinel mismatch)
- **class**: A (doc align)
- **severity**: low
- **suggested batch**: CEL vocab vs guest readers
- **locations**
  - `rules/cond/src/vocab.rs` — `doc: "where a roll came from, 0 = none (roll_source::NONE)"` (was `-1 = none`)
  - `rules/card-sdk/src/abi.rs` — `roll_source::NONE = 0` (the contract)
  - host fills `0` for unattributed: `hostfns.rs`, `cond_pre.rs`
  - existing guard uses the guest code: `rules/cards/card-ras/src/layer_keep.rs` `"… && roll_source == 1"`
- **was**: guards written as `roll_source == -1` never matched
- **expected**: align docs to `0 = none`. **Confirmed.**

## HOST-12 — stale purchase-surface comments
- **status**: **fixed** (2026-10-11, `backlog-host-abi-a`, `fix: HOST-12 …`) —
  "P0: stubs" dropped (HOST-06 rewrote the arms); "One `HostRequest::Quote`"
  dropped from `ctx.rs` / `docs/PURCHASE.md` (HOST-07); the hostfns comment
  that justified a native quote is gone (HOST-07 routes through the hook-aware
  path).
- **kind**: stale
- **class**: D
- **severity**: low
- **suggested batch**: docs / comments
- **locations** (cleaned)
  - `wasm_rules.rs` — "v40 purchase surface. P0: stubs"
  - `ctx.rs` + `docs/PURCHASE.md` — "One `HostRequest::Quote`"
  - `hostfns.rs` — "commit re-quotes" justification for a native quote

## TOOL-01 — `root_parallel::early_stop_stops_no_later_than_the_full_budget_search` is flaky
- **status**: **fixed** (2026-10-11, `backlog-host-abi-a`, `fix: TOOL-01 …`) —
  the test compared two separate 200 ms wall-clock searches
  (`on.iterations <= off.iterations` + `off.elapsed >= 190ms`) and failed
  ~1 in 3 under load. It now pins a shared **iteration cap** (24) with a
  wide wall budget: the full-budget run must hit the cap, the early-stop
  run must not exceed it, and same seed ⇒ same pick. No wall-clock
  assertion left.
- **kind**: tooling gap
- **class**: A
- **severity**: low (CI / agent runs under load)
- **suggested batch**: tooling
- **locations**
  - `crates/bot-core/tests/root_parallel.rs` `early_stop_stops_no_later_than_the_full_budget_search`
  - sibling `early_stop_is_off_in_deterministic_mode_and_the_iteration_cap_binds` (iteration cap ⇒ `early_stop` cannot fire)
- **expected**: deterministic stopping rule (iteration budget), assert on the decision rule rather than timing. **Confirmed.**

## HOST-13 — `HostRequest::AgentOffer` drops `agent`
- **kind**: ABI defect
- **class**: C (keep for audit log?)
- **severity**: low
- **suggested batch**: HostRequest handlers
- **locations**: `wasm_rules.rs:2264-2267` `agent: _` discarded; guest `rules/tiles/src/agent.rs:160` passes the agent tile
- **current**: harmless for the commit (tile state is re-checked) but the answer log cannot audit which agent offered
- **expected**: keep the field for logging / audit, or document why it is dropped


---

# LOW / rare-edge

## LOW-01 — event shout / skin-change / auction leftovers
- Covered by ABI-09 / EVENT-03. Keep as one low batch: `lulu.rs:37` shout logged only; `ex_quest.rs:36` skin; `tsurumaki_estate.rs:37` sealed bid vs open auction.

## LOW-02 — property / upgrade book clauses that live elsewhere
- `rules/tiles/src/property.rs:19` (「升级所[消耗]的资金不可通过[抵押]…」 enforced in `mortgage`), `:21` (「每块地有标注的等级上限」 = `buildMax`)
- **class**: A (docs only) — cite the right site; no behaviour change.

## LOW-03 — `Mujica:#J11` has no rulebook entry
- **kind**: rulebook gap
- **class**: C
- **severity**: low
- **locations**: `rules/cards/card-mujica/src/j11.rs:115`
- **question**: C-Q41.

## LOW-04 — umiri_card 「置入弃牌堆」 for a taken band-skill card
- **kind**: ABI/model gap
- **class**: C then B
- **severity**: low
- **locations**: `rules/cards/card-mujica/src/umiri_card.rs:162` (detached, not discarded; `skill:` id in the discard would be drawable)
- **question**: C-Q42.

## LOW-05 — sheet text-drift / typo leftovers
- TEST-FINDINGS §8: 丸山彩 / 上原绯玛丽 / 羽泽鸫 which column is current; Pastel Palettes alternate band design is a draft; typos “PERFECT“ / “FEVER!“ / 星月夜 「的的」 / 游击演出 stray `\[特]`.
- **class**: C (sheet ownership) — low.

## LOW-06 — 上原绯玛丽 (2) prompts under a once-per-turn C# gate?
- See CH-10.

---

# D — stale (already fixed / already landed)

Do **not** put these in the open backlog. Keep as notes so they are not re-filed.

## D-01 — nine stale `#[ignore]`s already removed (2026-10-10 triage; run twice, green)
| test | file | former reason |
|---|---|---|
| `c05_cancels_one_designation_of_four` | `rules/tests/tests/cross_chain.rs:265` | whole-card negate (ABI per-pair cancel landed) |
| `l06_one_draw_several_effects` | `rules/tests/tests/cross_long.rs:471` | capture-vs-autoplay (record-only) |
| `t11_fire_bird_plus_hhw_double` | `rules/tests/tests/cross_tiles.rs:458` | multiplier order (still a C question, MONEY-01) |
| `immunity_gap` | `rules/tests/tests/fuzz_found.rs:198` | effect on immune player |
| `card_trap_reachable` | `rules/tests/tests/fuzz_found.rs:108` | card module trapped |
| `g11_forced_stop_beats_lock_end_rewrite` | `rules/tests/tests/gap_move.rs:270` | forced stop vs SettleBefore end-rewrite |
| `inter_yolo_pushes_haneoka_over_20` | `rules/tests/tests/mygo.rs:1379` | Y.O.L.O timing CROSS-AGENT |
| `choose_your_stage_answers_an_abnormal_move` | `rules/tests/tests/roselia.rs:369` | self-[传送] window (see CH-01; the encore twin `ix_encore_…` is still open) |
| `sweet_escape_gate_requires_high_rent` | `rules/tests/tests/sumimi.rs:185` | 「收费标价」 (still a C question if reopened) |

## D-02 — NEGATION-AUDIT V2 "no activation cost" TODOs (already landed)
- One root cause, already applied: the six text-less money play gates are gone; in-body 「[消耗]/[支付]」 is effect content and takes the Q1 shortfall path.
- Locations (documentation leftovers only):
  - `rules/cards/card-roselia/src/fire_bird.rs:18`
  - `rules/cards/card-general/src/gacha10.rs:13`
  - `rules/cards/card-ag/src/crimson_soul.rs:28`
  - `rules/cards/card-morfonica/src/starry_night.rs:23`
  - `rules/cards/card-hhw/src/believe_you.rs:22`
  - `rules/cards/card-hhw/src/kokoro_circle.rs:30`
  - `rules/tests/tests/money.rs:365`
- Tests green: `gacha10_playable_while_short_then_bankrupts`, `fire_bird_playable_while_short_then_mortgages`, `crimson_soul_playable_while_short_then_bankrupts`, `believe_you_playable_while_short_then_bankrupts`, `kokoro_circle_playable_while_short_then_bankrupts`, `starry_night_playable_while_short_then_bankrupts`.
- Action: rewrite the TODO as a settled note (or delete) so it stops counting as open.

## D-03 — 2026-10-09 batch already fixed (do not re-open)
- 凑友希那 (3) fire cap; 要乐奈 (2) Space teleport; 白金燐子 (2) pre-roll window (test bug); 麻里奈小姐的礼物箱 「可」; 飞鸟山之战 stun layers (test bug); CiRCLE band (2) doubling; 游击演出 ends on the chosen tile.
- See TEST-FINDINGS "Resolved (2026-10-09)".

## D-04 — other already-landed surfaces referenced by TODOs
- per-pair cancel (`ctx::cancel_designation`); `payAfter` / `gains_this_turn`; `overlay_guest_state`; `EXILE_MAIN`; ABI v35 skill-attachment / `invoke_skill` / `raise_bought` / `add_follower`; purchase surface P0–P5; settle/move stage model ABI 43; counteract ring ruling 2026-10-07.

## D-05 — MOVE-04 若能再次交汇 re-roll (test-harness sequencing; 2026-10-10)
- Root cause: `Table::dice` → `Match::world_mut` after `t.roll` left the `moveRoll` [反击] window pending (`mod.rs:783` assert). The card body's `do_move_roll` / `set_move_roll` / `log` never touches `world_mut`; real matches never do either.
- Secondary: routines replay from their start-time snapshot, so faces loaded after the halt never reach the re-roll. All faces must be queued before `t.roll` (same pattern as `gap_move` G08 / `hhw` 运动的天赋).
- Fix: merge the re-roll faces into the pre-roll `t.dice` load in `m06` / `m06b`. Un-ignored, green twice. No engine/card change.

## D-06 — MOVE-02 白金燐子 (2) (test drive shape; 2026-10-10)
- Root cause: m10/m11 pressed the skill and asserted pos without taking the main roll. `set_fixed_roll` is a **pre-roll shape** of the upcoming main move; it does not auto-run the walk. The press-is-the-move pattern (`ctx::card_move`) is for 「代替本回合的移动」 skills (要乐奈 (2) etc.), not this clause.
- Green twins already pin the drive: `rinko_skill_2_fixes_the_move_roll_to_x_times_six` (roselia), `g09_fixed_roll_plus_starlit_reroll` (gap_move).
- Fix: `t.roll(0)` after the X prompt. m10 (stunned mid-turn) and m11 (Y.O.L.O) un-ignored, green twice. m11 records 14 (Y.O.L.O +2 over the fixed 12).
- Related (not this item): `fixed_roll` is not one-shot-cleared after consumption (latent, no test); the 「固定」 vs later additives question stays a RULING (starry_night / eve_unify already refuse additives over a fixed face; Y.O.L.O does not).

## D-07 — SKILL-08 要乐奈 (3) overlap (pre/body mismatch; 2026-10-10)
- Root cause: `on_overlap` sat on `PassPlayer` with `pre::MINE` (owner is the mover) but read `trigger::player_id()` as "the other" and bailed when it equalled the owner — always. `target` is the other player already on the landing tile (`abi.rs` PassPlayer payload).
- Fix: read `trigger::target()`. New test `rana_cat_overlap_takes_800_and_gives_a_parfait` (mygo) pins +800/−800 and a 抹茶芭菲 on the other. Green twice.
- Amount: 800 (sheet / `characters.json`). Older `rulebook.txt` prose says 1000 — sheet wins.

## D-08 — MOVE-01 forced-stop landing (test drive / arrangement; 2026-10-10)
- Root cause: the engine `set_stop_at` path is already correct (see green `m4_kokoro_force_stop_stops_a_passer`, `g11`, `g12`, `m16`, roselia yukina). The three cross_move ignores were test bugs:
  - m12a/m12b: `ask_yes` options are `ask.yes`/`ask.no`; the matcher looked for 「凑友希那」/「强制停下」 in options (they live on the prompt dump) and `decline()` answered no. Same bug class as the 2026-10-09 yukina prompt-match fix.
  - m12b: also pinned 41 instead of the rolled dest 32+5 = 37; the 安可 window only opens after the force-stop is accepted (AbnormalGate).
  - m15: `place_raw` leaves `tile: -1`, so `card.placed && self_tile() == tile.id` never matches. Both stop-pot cards need `place_on_tile`.
- Fix: dump-match `kokoro_practice_force` + `answer_one(0)`; assert 37 for the countered stop; `place_on_tile` for 香澄@44 and 乐奈@48. Un-ignored, green twice. m12b records fire spent even when countered (RULING recorded, not decided).

## D-09 — TILE-04 / TILE-05 (test window / arrangement; 2026-10-10)
- **TILE-04 学生会的检查**: the card is a SettleBefore [反击], not a free [手]. `t.play` silently no-ops (no `On::Play` entry) and never places. The body's ask_yes (allowNone stand-in) + ask_tile also had to be accepted. `t1 - 2` usize-underflowed on 购物中心 = 1. Fix: roll + `counteract`, answer yes then the tile, wrap the start. Engine forced-stop + rent-half already matched the green 香澄 shape.
- **TILE-05 Parking Space + 要乐奈 (2)**: P0 played the [手] on P1's turn (`err.play_phase`); stay was read before `TurnEndAfter`. Fix: setup on P0's turn; assert stay after `pass`. Engine path (teleport → body-replace → stay at turn end, skillBlock) already green in `g15`.
- Both un-ignored, green twice.

---

# Stale sections in TEST-FINDINGS.md / companion docs

Checked 2026-10-10 against the tree (including the un-ignore batch). Do **not** re-file these as open items; a later docs pass should strike or rewrite them.

## TEST-FINDINGS.md
1. **L25–56 Status counts (2026-10-07).** "875 tests, 90 `#[ignore]`" and the per-group table are historical. This tree greps **85** `#[ignore = "` attributes (69 DISCREPANCY, 13 RULING, 2 meta).
2. **L90–97** `asukayama_teleports_and_assigns_the_status_spread` "now fails" — the test is green / not ignored.
3. **L309–312 §4 NNM dual `On::Play` "card bug".** **Stale.** Commit `1e80323` (2026-10-09) merged the two Play entries into one branching on `is_placed()`; `nnm_discard_marks_draw_x` is green. Only `nnm_option2` (hard-coded 2000 vs live CiRCLE reward) remains — see CRYSTAL-04.
4. **L326–328 MyGO band (2) draw half "unreachable".** **Stale.** Commit `e373458` merged 迷途之星 to one press entry; `band_draw_for_two_crystals` is green. The sibling cross-move "move-1 replacement does not fire" remains open (MOVE-03).
5. **L333–341 cross list.** Partially stale: m27 （祥子）带领着大家 is green; 网络链接异常 "still drops every designation (c05, g19)" — `c05` un-ignored 2026-10-10 (one designation cancels); only `g19` still sees the whole-card negate on the join path (CH-06).
6. **L481–487 §6 item 8 Multi-activation.** Partially stale (NNM + MyGO merged; dispatch `0fb939b` runs every matching Hook/Gate entry). Remaining question is the general pattern / sheet NOT FOUND for NNM (2) choice-inside-one-entry — C-Q6.
7. **L947–952 "Known failures at time of writing" (`card_paths` 4 cases).** **Stale** — closed the same day.
8. **L877–887 B2 markers-kept / `shanyao` pin.** Contradicted by L975–976 (P✽P fan now expected **cleared** with bankruptcy). The later statement is live.
9. **L1095–1098 wider-B "NNM / MyGO band (2) multi-activation".** Same as #3/#4.
10. **§10 Resolved-summary bullet "ring opens at the next seat".** Superseded by the 2026-10-07 ruling in §10 (ring starts at the **initial user**; exhaust before advance).
11. **2026-10-10 un-ignore notes** were refreshed in the same commit as the un-ignores (`choose_your_stage`, `immunity_gap`, `card_trap_reachable`, `inter_yolo`, `sweet_escape`, `c05`, `l06`, `t11`, `g11`). Any pre-2026-10-10 "still open" claim about those nine is stale.

## COVERAGE.md
Header ignore counts and many `**ign**` rows are stale vs the live set: CiRCLE band unported (fixed), NNM dual On::Play, MyGO band draw unreachable, 壱雫空 #2/#3 (green), 要乐奈 Space teleport (fixed), 游击演出 #3 (fixed), 凑友希那 #3 fire-cap (fixed), （祥子）带领着大家 (green), `if_only_it_lasted_discards_at_hand_7` (green), Top-25 #3 祥，移动 / #10 壱雫空, Ringing Bloom "(2)/(3) do not apply" (tests green; open item is the X RULING C-Q36).

## CROSS-TESTS.md
L36–40 ring rules quote the superseded clause-89 start / one-activation-per-visit. Counts "70 pass / 41 ignore" are historical.

## NEGATION-AUDIT.md
§9 N1 (hook vs counteraction order) and N2 (marker spends) are listed as open but V1 was fixed 2026-10-07 and §5 was answered the same day. N3–N6 remain open (folded into MONEY-04 / SETTLE-05 / C-Q3 / C-Q10).

## SETTLE-STAGES.md
L162 "latent bug" on 凑友希那 force-stop may be stale (M4 done / m4_kokoro pins the stop). L5 `TODO(规则书) PIPELINE-AUDIT Q7` marker is gone (Q7 ruled).


---

# Summary counts

## Per class (open items only; D excluded from "open")

| class | meaning | open items |
|---|---|---|
| **A** | fixable now from clear text + current ABI | 37 |
| **B** | needs a new/changed ABI capability | 17 |
| **C** | needs a user ruling | 23 |
| **D** | stale (already fixed / landed) | 6 note groups (9 removed ignores + V2 + 2026-10-09 batch + landed surfaces + dead host names + stale comments) |

*(A+B+C = 78 open root-cause items after dedup of 144 TODOs + ~85 ignore rows + the 2026-10-10 ABI audit. Some A items are "contained engine bug" one-liners; some B items unblock several A/C.)*

## Per kind

| kind | count (approx) |
|---|---|
| implementation bug | 39 |
| ABI defect | 24 |
| rulebook divergence | 14 |
| stale | 6 note groups |
| implementation debt / tooling | 2 |

## Per suggested batch

| batch | items | severity mix |
|---|---|---|
| payment pipeline | MONEY-01..08, SETTLE-06, SETTLE-07, ABI-11, ABI-12, CRYSTAL-03 | 4 high C/A |
| movement | MOVE-01..15, TILE-04, TILE-05, SKILL-08, SKILL-09, ABI-02, CRYSTAL-07 | 4 high A |
| counteraction windows | CH-01..13, WINDOW-01..03, ABI-03, MOVE-11, MOVE-12 | 7 high |
| rent multipliers | MONEY-01, TILE-01..03, CRYSTAL-04, CRYSTAL-08 | 1 high C |
| host ABI: out-buffers / queries | ABI-01, ABI-02, ABI-05, HOST-01, HOST-04, HOST-07 | 2 high A/B |
| HostRequest handlers / reply sentinels | ABI-04, ABI-09, HOST-05, HOST-06, HOST-13, WINDOW-01 | 1 high A |
| host ABI: plan flags / dead imports | HOST-08, HOST-09, HOST-10, HOST-11, HOST-12 | 1 medium A |
| crystal/marker accounting | CRYSTAL-01..08, CH-11, CH-12, MONEY-07 | 1 high (via CH-07) |
| draw/discard pipeline | ABI-07, ABI-08, DRAW-01..03, CH-13 | |
| skills: MyGO | MOVE-03, CRYSTAL-01, SKILL-08 | |
| skills: Roselia / CRYCHIC | SETTLE-02, SKILL-03, SKILL-05, SKILL-07 | |
| skills: PP | SKILL-01, SKILL-02, ABI-05 | |
| skills: Mujica | SETTLE-03, SKILL-04, CH-08 | |
| HHW cards | SETTLE-01, CRYSTAL-05..07 | |
| settle stages | SETTLE-04, SETTLE-05, MOVE-08, HOST-02 | |
| engine: settle recursion / stack safety | STACK-01 **fixed**; SETTLE-01 swap-axes **fixed** (reverse still TODO) | 0 (stack) |
| events | EVENT-01..04, ABI-09 | |
| fuzz/determinism | FUZZ-01, HOST-03 | 1 high A |
| tiles | TILE-06, HOST-04, LOW-02 | |

---

# Proposed batch order

Do these in order; each batch should be a PR that does not share files with the next.

1. ~~**high-A engine: settle recursion / stack safety** (STACK-01)~~ **fixed** — see the STACK-01 entry
   - Unblocks `ckpt_equiv`, long bot games, server `spawn_blocking` workers (~2 MiB), and the browser host stack. Preferred fix: queue nested `card_settle_at` jobs and pump them iteratively (also unroll `flush_deferred` → `start` → `execute`). Stopgap: `MAX_SETTLE_DEPTH`. Then re-check SETTLE-01 (笑容大游行). Files: `crates/game-core/src/engine/{play,mod}.rs`, `crates/game-rules/src/wasm_rules.rs` (`HostRequest::SettleAt`).
2. **high-A engine bugs — movement** (MOVE-01, MOVE-02, MOVE-04, SKILL-08, SKILL-09, TILE-04, TILE-05)
   - Unblocks the most ignored `cross_move` tests. Files: `game-core/engine/{play,ops,move_ctx}.rs`, `skill-characters/{rana_parking,kaoru_prince}.rs`.
3. ~~**high-A counteraction windows** (CH-02, CH-03, CH-04 play-as-counter, CH-06, WINDOW-01, WINDOW-02)~~
   - **fixed 2026-10-11** (`backlog-windows-a`): CH-02, CH-03, CH-04, WINDOW-01,
     WINDOW-02 c13 (one-timing Effect on a DESIGNATES play).
   - CH-06 reclassified **B** (needs CH-05 nested PlayCtx for 真奈's join retarget).
   - WINDOW-02 l01 remains (five-link war: E should cancel A's designation).
4. **high-A money** (MONEY-03 Repaint shaped half, CRYSTAL-03 crimson_soul 500 timing, SETTLE-02 CRYCHIC 6-hand block, SETTLE-03 Sakiko absorb)
5. **high-A host ABI: reply sentinels + quote==charge** (HOST-06, HOST-07, HOST-08 `set_no_build`)
   - Files: `crates/game-rules/src/{wasm_rules,hostfns,native_shims}.rs`, `rules/card-sdk/src/ctx.rs`. Do not share a PR with card bodies.
6. **high-B unblockers** (in this order)
   - **MOVE-03 multi-activation selection** (unblocks MyGO band (2) + NNM)
   - **CH-05 play-history + nested PlayCtx** (unblocks Mortis, Mana Champion, dice_cast counters)
   - **CH-01 self-abnormal Effect chain** (unblocks 安可 / 像往常一样 / 我自己的问题 / RAS band (1))
   - **CRYSTAL-01 FirePaying** (unblocks 灯 不再迷茫)
   - **HOST-01 HostRequest rebase** (unblocks inline equivalence + some fuzz)
7. **high-A fuzz/determinism** (FUZZ-01) once HOST-01 is in.
8. **C questions to the user** (collect all of §C below in one sitting). While waiting, continue on:
9. **medium-A contained bugs** (SKILL-02 hina lottery, SKILL-07 soyo expiry, DRAW-01/02/03, CH-11/12/13, CRYSTAL-04 NNM live reward, CRYSTAL-08 ringing_bloom continuous max, MOVE-10/11/12, LOW-02 docs)
10. **medium-B ABI pile** (ABI-01..12, HOST-04/05/09/10/11) — one ABI bump, many cards.
11. **medium-C leftovers + events** (EVENT-01..04) after the user answers §C.
12. **D cleanup** — rewrite NEGATION-AUDIT V2 TODOs as settled notes; strike the stale TEST-FINDINGS / COVERAGE / CROSS-TESTS sections listed above.

---

# Verbatim C-class questions (ready to show the user)

**C-Q1 (MONEY-01, rent multipliers).**
When two multiplicative money modifiers apply to one rent / settle payment (e.g. Fire bird 「支付…的1.5倍」 × Ave Mujica 状态2 「从[收取]与[支付]的资金改为1.5倍」 × HHW band double-pay × 摩卡 half), in what order do they compose? Options: (a) product of all live multipliers, (b) apply in declaration order as written, (c) only one multiplier per window (「支付减半/翻倍」 applies once), (d) other.

**C-Q2 (MONEY-02, 分摊 × halving × cancel-one).**
On a 分摊 leg: (1) if one designation is cancelled (网络链接异常 「取消一个指定」), does the per-payer share recompute from the remaining designations, or is it the pre-drop figure? (2) Does a 「支付减半」 apply before the split, to each share, or after? Options for (1): recompute / pre-drop. Options for (2): pre-split / per-share / post-split.

**C-Q3 (MONEY-04, 强制购买 & auction vs the money pipeline).**
Three related questions on 基础[结算] 6.2 / 其他 6 「不受任何资金变动效果影响」:
(3a) Does that clause also bar a card that 「写明改动强制购买」?
(3b) Is 强制购买 a 「购买」 for 「购买…时」 listeners (`bought` / `buyAfter`)?
(3c) Is an auction win a 「购买格子」 for discounts, and does the auction [消耗] run the pay pipeline?

**C-Q4 (MONEY-05, 巴 「常规收购价一半」).**
AG:（巴）商店街的救世主 「立刻支付常规收购价一半的价格从该玩家处收购该地契」:
(4a) What is 「常规收购价」 — land price only (C#), land + houses, or 2× force-buy total?
(4b) Is 「收购」 a 「购买」 for 「购买…时」 listeners?
(4c) Does the mortgage stay on the hand-over (C# keeps it; this port clears it so Afterglow's 「自动免费在上面加盖一栋房子」 can fire)?

**C-Q5 (CH-07, 「视为此卡未生效」).**
The clause 「视为此卡未生效」 appears on 羽丘的不可思议女孩, Fire bird, 星月夜, noble_blue, kokoro_circle, 无路矢, 奇迹 (3). Two decisions:
(5a) Does the card still get spent / discarded when it is "not effective"? (haneoka 「放入弃牌堆且视为此卡未生效」 says yes; noble_blue / starry_night's "the card is spent anyway" implies no.)
(5b) What counts as a "use" that this suppresses? (e.g. haneoka's later 「置入弃牌堆并抵消一次任意付款」, miracle (3)'s crystal-on-discard.)

**C-Q6 (MOVE-03, multi-activation).**
A card / band skill with several `On::Play` entries (MyGO band (2)/(3), Mor:（NNM）稍微努力了一下): should the player pick which activation fires, or should one entry branch on 「placed vs hand」? (NNM's 「发动以下效果中的一个」 is a choice *inside* one entry — is that the model for all of them?)

**C-Q7 (MOVE-05, mid-turn [停留]).**
Glossary 51 says [停留]=「无法移动」. If a player gains [停留] *during* a move (after the roll, before the walk ends), does the current move stop immediately, or does the prohibition only apply to the next move? (Engine now: the latter. Design reference also the latter.)

**C-Q8 (WINDOW-03, EXIST 「造成影响」).**
EXIST: 「造成影响」 then draw. If the redirect is later negated by a [反击], does that count as 「造成影响」? Options: (a) declaration (the redirect re-named the recipient — engine now), (b) settlement (nothing landed on me — no draw).

**C-Q9 (ABI-05, PP band (2) fan-flip).**
PP band (2) is passive on 「将X个反面[P✽P粉丝]变正的效果且X大于拥有数时」. Is a fan-flip its own timing, or is this folded into the fan-spend / fan-gain points? (No flip-effect trigger exists today.)

**C-Q10 (SETTLE-05, no_breakup R3).**
Sumimi:no_breakup 「若传送并触发结算后未能使资金变为拥有相同数字，回到原处并取消所有受到的效果」: is 「触发结算后」 `settleAfter` or the `tileResolved` terminal? And does 「取消所有受到的效果」 undo money as well (「不进行任何结算」 scopes to the return)?

**C-Q11 (SKILL-04, 状态2 exits).**
For skills that say 「直到状态2结束为止」 with no clause that ends it (kaede_support, mumei_streamer, mutsumi_actor): what ends 状态2? Options: (a) never (permanent once named), (b) same exits as the Ave Mujica band skill (fire-pot 0 at turn end), (c) a per-skill exit you specify, (d) other.

**C-Q12 (SKILL-05, CRYCHIC band (3) / (2) scope).**
(12a) When CRYCHIC band (2) 「移除此卡与你所有区域的所有"CRYCHIC"卡」 removes the band card itself, does (3) 「当此卡移除时」 cash out? (12b) Is (2) limited to `BandCrychic` specifically (C#) or any 「乐队技能的（2）效果」 (this port)?

**C-Q13 (TILE-01, Anon link + mortgaged partner).**
Does the Anon Tokyo link add half of a *mortgaged* partner tile's 收费?

**C-Q14 (TILE-02, agent 「同色」) — superseded 2026-10-13.**
「每种颜色拥有一个地产商格子」 / half-charge 「同色」: a per-player colour
override (`anyColor` / 「该格获得所有颜色」 / `colorFor:<p>`) **does** widen
the colour group (`ctx::is_color` / `agent_colour_set`). An ANY_COLOR-only
member is buyable / rentable but never buildable. The choice set 「无主或玩家
拥有的」 is unowned **or any player-owned**; another seat's deed settles as the
agent's 「半价收费的[结算]（向上取整10）」. `t16_soyo_mixed_colors` is live and
green. Open residue: soyo's 「其他颜色」 (does the extra half apply on a
same-colour agent?) and the 「视为live house」 half of Ruling 5.

**C-Q15 (TILE-03, RiNG).**
(15a) The dice-rent table 「地主拥有的 RiNG 数量 × ringMultiplier × 1d20」 and the 「至少1个」 floor are `data/match_rules.json` notes, not rulebook text. Keep them?
(15b) Is RiNG mortgageable? (游戏流程 5 says 「抵押拥有的地契」 with no exclusion; the engine refuses.)

**C-Q16 (TILE-06, 江户川乐器店 vs CiRCLE).**
The book names 江户川乐器店 and CiRCLE in one sentence but gives them different extra text. Should the two shops diverge, which body wins?

**C-Q17 (MONEY-06, 「支付」 reach).**
Does 「支付」 (and the 支付阶段 shaping / counters) also reach a non-settlement loss a card forces outside a settle? (Engine now: settle payments only; `gain` is never scaled.)

**C-Q18 (MONEY-07, Popipapapipopa).**
Does Popipapapipopa's crystal 「消耗或支付」 reduce a *rent the owner receives*? (Reads as the owner's own outlay.) Engine assertion in the ignored test: the rent is reduced; the fire gain is independent.

**C-Q19 (MONEY-08, 分摊 remainder).**
When 1000 is 分摊 n ways and 1000 % n ≠ 0, who gets the remainder? Options: dropped (engine now), to the first seat in turn order, to the payer, other.

**C-Q20 (MOVE-06, E14 0-step).**
For a 0-step move / teleport: does [经过] (`passTile`) fire as well as [重叠] (`overlap`)? `E14` names only [重叠]; `B41`+`E13` read as both-fire for a teleport. Engine now: [重叠] only.

**C-Q21 (MOVE-07, teleport as the main move).**
Should a teleport that *is* the main move settle like a main-move landing (announce + end-step buy offer) or like any teleport (immediate `offer_buy`)? Engine now: immediate offer.

**C-Q22 (MOVE-08, want_to_grab R5).**
CRYCHIC:想要抓住... (2) 「当第一位其他玩家经过你」: is that 经过 (mid-route pass of my tile, 行动阶段 12) or 重叠 (`passPlayer`, end-tile overlap)? Text says 「经过」; C# used `PassPlayer` (engine now).

**C-Q23 (MOVE-13, 星月夜 re-roll).**
Does 星月夜's re-roll cover the whole dice set or only the base die? (Ignored test asserts full set.)

**C-Q24 (MOVE-14, 普通与理所当然).**
Does 普通与理所当然 copy a signed or unsigned 移动格数? (Ignored test asserts unsigned.)

**C-Q25 (MOVE-15, heat_head).**
「减少1d6」: subtract 1d6 from the face, or drop a die from the pool (「少投1d6」)? Sheet says 减少1d6.

**C-Q26 (SETTLE-04, 线香花火 / extra turn).**
(26a) ~~Does removing the last crystal of 燃尽前的线香花火 also grant 「获得一个额外回合」?~~
  **Ruled 2026-10-13**: no. The terminal effect is discard + 2 [眩晕]; granting an
  extra turn would auto-skip it on the stun and eat one layer.
(26b) Does 「until your next turn start」 end at the first turn start after, including an extra turn's? (R-7.)

**C-Q27** — same as C-Q10 (no_breakup R3). *(kept for numbering of SETTLE-05)*

**C-Q28 (SETTLE-06, two halvings).**
（香澄）大家我都喜欢哦 halves 「地租」 and 祥，移动 halves 「支付价格」. Do they stack to ¼, or does the 支付阶段 「支付减半/翻倍」 window apply once? Engine now: ¼ (different nouns).

**C-Q29 (SETTLE-07, 晕眩 vs own gain).**
Rulebook 49 「无法收付款」: does it block a card's own 「获得」 to a player the same card body just stunned? (Mujica:心の雨 fallback does 「眩晕」 then 「获得1000资金」. Engine now: the gain lands.)

**C-Q30 (CH-08, welcome_mujica).**
「若指定了不存在状态2的玩家则无效果」: does this gate the switch on the target already being in 状态2 (only 2→1), or rule out targets for whom 状态2 is unreachable? Engine now: free toggle.

**C-Q31 (CH-09, dice_cast).**
「（此卡可以被反击）」: must other [反击]s still answer this card's own play window before the lock is set? (Engine wants the C# reverse-order counter play; see CH-05.)

**C-Q32 (CH-10, ran_as_usual once-per-turn).**
C# adds a once-per-turn gate to （兰）像往常一样 「此卡可以当反击使用」 that the passage does not name. Keep the C# limit (narrows), or drop it (text has no per-turn limit)? Engine now: kept.

**C-Q33 (CH-13, Here the world).**
When a drawn effect card is captured by Here the world, is it captured before that card's auto-play, or after? (`l06_one_draw_several_effects` ignore already removed — record-only.)

**C-Q34 (CRYSTAL-05, 育美标记).**
C# `HagumiMarkFx.PassTile` has a circle-tile case (remove every 育美标记 and gain 1,000 per mark 「经过起点」) that the rulebook text does not carry. Keep the C# behaviour?

**C-Q35 (CRYSTAL-06, 薰 charge/decay).**
「（充能3，衰减1）」 names the charge and the decay but never says what happens at 0 (every other crystal card says 「为0时置入弃牌堆」). Keep C# decay-out, or leave the spent card on the field?

**C-Q36 (CRYSTAL-08, Ringing Bloom (3) X).**
「X为你收费格上的房屋数」: the count after (2) 「视为」 raises it (engine now), or the real house count?

**C-Q37 (SKILL-10, misaki_other).**
「下次经过"弦卷集团"前可将"弦卷集团"格子额外视为CiRCLE」: what does 「额外视为CiRCLE」 pay on top of the tile's own effect? And is the override per-player tile-kind (not colour)?

**C-Q38 (EVENT-01, habinel).**
「结束后传送到微笑号」: is the teleport immediate, or only after the [除外] wears off?

**C-Q39 (EVENT-02, asukayama).**
「所有玩家[传送]到飞鸟山公园。然后抽到此卡的玩家移动1d20」 — is the walk also settle-free? (Engine now: settle-on-land.)

**C-Q40 (EVENT-03, event text gaps).**
Batch of event ambiguities — please answer each:
(40a) 余烬: does the spread repeat on later turn starts of the drawer, or one-shot? Does the event itself leave play when the copies are removed?
(40b) 修复笔记: 「累计…加盖后将此卡移除」 — is that 3 builds (one per designated tile) or a single build anywhere on them?
(40c) 羁绊音乐: 「卡组」 = the draw pile only, or the player's whole card pool?
(40d) 露露: a player who chose [传送] but holds under 500 — payment prompt / force debt, or does the choice fizzle?
(40e) 惊喜决斗: 「下2回合」 = the turn after next (engine now), or a two-turn window that expires at the first start?
(40f) 泪: 「同色地契」 = one colour in common, or the same single deed colour pair?
(40g) 弦卷地产: 「出价最大」 tie-break (engine now: earliest seat in turn order).
(40h) 圆 rebuild: 「该改变[触发结算]的效果优先于其他任何改变[触发结算]的效果」 — real priority, or placement order (engine now)?

**C-Q41 (LOW-03, Mujica #J11).**
`Mujica:#J11` has no rulebook entry at all (body follows C# alone). Please supply the clause, or confirm the C# behaviour is the intended rule.

**C-Q42 (LOW-04, umiri 「置入弃牌堆」).**
「置入弃牌堆」 for a *taken band-skill card*: a band attachment is not a card in a pile (C# only detaches it). Detach (engine now), or put it in the discard (and is a `skill:` id drawable)?

**C-Q43 (TILE-05 follow-up, Parking Space × 要乐奈 (2) body-replace order).**
When 「通用:[都筑诗船]Parking Space」 is on Space and 「skill:要乐奈:投币式停车场的猫」（2） also fires on a settle at Space (both are 「将该次结算改为…」 settle-body replaces, `SETTLE-STAGES.md` §9 R4):
(a) if Rana owns Space, which replace wins — Parking Space's 停留 or Rana's 抹茶芭菲?
(b) if a third party owns Space, does Parking Space's body-replace also swallow Rana's 「免除付款并抽1张卡」 draw, or does the draw still happen as a skill effect outside the replaced body?
Engine now: first-to-claim (`trigger::cancelled()`); field hooks run in seat order, so Parking Space (P0) would win over Rana (P1). The CROSS-TESTS T24 path is unowned Space and does not need this answer.

---

*End of backlog. Standing file: `docs/rulebook/BACKLOG.md` on branch `rulebook-triage`.*
