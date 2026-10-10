# Rulebook / ABI backlog

**2026-10-10 triage.** Branch: `rulebook-triage` (`/home/monopoly/rulebook-triage-wt`).
This is the standing backlog for continuous iteration on rulebook divergences,
rule-implementation bugs and ABI problems. Fix agents take independent batches
from here; do not merge into the primary branch from this file.

Sources:
- 144 `TODO(规则书)` tags in `rules/` + `crates/`
- live `#[ignore = "…"]` set in `crates/game-rules/tests/*.rs` (85 after the un-ignore batch; 69 DISCREPANCY, 13 RULING, 2 meta)
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

# HIGH

## MONEY-01 — rent / settle multipliers do not have a composition rule
- **kind**: rulebook divergence
- **class**: C
- **severity**: high
- **suggested batch**: rent multipliers
- **locations**
  - `crates/game-rules/tests/rb_gap_money.rs:117` `g01_fire_bird_times_ave_mujica_on_one_rent` `#[ignore = "RULING: order of two multipliers on one rent (Fire bird 1.5x x Ave Mujica 1.5x)"]`
  - `crates/game-rules/tests/rb_cross_tiles.rs:457` `t11_fire_bird_plus_hhw_double` — **D note**: ignore already removed (test still records the question; do not re-open as A)
  - `crates/game-rules/tests/rb_fuzz_found.rs:173` `multiply_order_matters` `#[ignore = "DISCREPANCY: two multiplicative money modifiers do not commute"]`
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
  - `crates/game-rules/tests/rb_gap_money.rs:222` `g04_half_share_then_cancel_one_payer` `#[ignore = "RULING: halving vs cancel-one order on a 分摊 leg (and whether the share recomputes)"]`
  - related: `rules/events/src/front_or_back.rs:49,56` (「分摊」 remainder) — separate small C, see EVENT-03
- **rulebook text**: 爱心义演 / 分摊 legs; 网络链接异常 cancel-one designation (sheet: 「取消一个指定」).
- **current**: share is the pre-drop figure (TEST-FINDINGS §3); halving-vs-cancel order undecided.
- **expected**: decide whether the share recomputes after a designation drop, and whether halving applies before or after the split.
- **question**: C-Q2.

## MONEY-03 — Repaint (「对方此次结算的支付减半」) does not halve the shaped total
- **kind**: implementation bug
- **class**: A
- **severity**: high
- **suggested batch**: payment pipeline
- **locations**
  - `crates/game-rules/tests/rb_ras.rs:937` `repaint_halves_a_shaped_settlement_payment` `#[ignore = "DISCREPANCY: ruling 2026-10-06: 「对方此次结算的支付减半」 -- a Tomorrow's Door surcharge is added at full price instead of halving the shaped settlement payment"]`
  - related tests `rb_cross_tiles.rs:163` `t03_anon_link_plus_repaint` (same shaped-payment path)
- **rulebook**: ruling 2026-10-06 + sheet 「对方此次结算的支付减半」 — the *settlement payment* (post-shape) is halved.
- **current**: Tomorrow's Door surcharge joins at full price after the half.
- **expected**: half applies to the whole shaped settlement payment (surcharge included).
- **class A** because a later ruling already cites the wording.

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
  - tests (green, pin current): `rb_card_paths::tomoe_savior_buy_listener_*`
- **rulebook**: 「立刻支付常规收购价一半的价格从该玩家处收购该地契」 — 「收购」 not 「购买」; base (land / land+houses / 2× force) unnamed.
- **current**: C# reading `price / 2` (land alone); hand-over runs `raise_bought` so every 「购买…时」 listener hears it; mortgage is cleared before the hook.
- **expected**: decide the price base, whether 收购 is a 「购买」, and whether the mortgage stays.
- **question**: C-Q4.

## CH-01 — self-applied abnormal / self-[传送] never opens a [反击] window
- **kind**: implementation bug + missing ABI
- **class**: A (wire the abnormal/Effect chain on `card_move` teleport) then B if the move-tag cannot express 「因任何原因」
- **severity**: high (安可 is a staple [反击])
- **suggested batch**: counteraction windows
- **locations**
  - `rules/cards/card-general/src/encore.rs:35` (「因任何原因」)
  - `rules/cards/card-ag/src/ran_as_usual.rs:23` (「包括你的技能」), `:27` (C# once-per-turn gate — separate C, see CH-07), `:92` (`Plan.Reverse` skill move is abnormal but `_abnormalTurn` never counted it)
  - `crates/game-rules/tests/rb_pp.rs:963` `ix_encore_blocks_overlap_teleport` `#[ignore = "DISCREPANCY: book says 安可 counters any abnormal movement on its user; no window opens for 重叠的声音's self-teleport"]`
  - `crates/game-rules/tests/rb_cross_move.rs:1166` `m28b_my_own_problem_after_forced_move` (是我自己的问题 still fires after an abnormal move)
  - `crates/game-rules/tests/rb_cross_move.rs:404` `m09_ras_band_first_abnormal_only` (RAS band (1) cannot decline a second abnormal — no window)
  - `crates/game-rules/src/wasm_rules.rs:2801` (self-applied abnormals note)
  - **D**: `choose_your_stage_answers_an_abnormal_move` (`rb_roselia.rs:369`) — ignore already removed; keep as a D note only
- **rulebook**: 通用:安可 「[反击][使用者]即将因任何原因受到[异常移动效果]影响时：无效此次…」; （兰）像往常一样 (2) 「受到异常移动效果（包括你的技能）」.
- **current**: `abnormal` chain only opens when `by != target`; `card_move` / `plan::set_teleport_to` never runs the abnormal gate at all.
- **expected**: every abnormal move on the user — self-inflicted included — opens the [反击] window before it lands.
- **capability if B needed**: move-tag hook for 「因任何原因/包括你的技能」 self-abnormal; teleport-as-main-move raises `Effect` + `AbKind::Teleport`.

## CH-02 — 花园多惠 (2) cancel window never opens
- **kind**: implementation bug
- **class**: A
- **severity**: high
- **suggested batch**: counteraction windows
- **locations**
  - `crates/game-rules/tests/rb_cross_chain.rs:1092` `c24_hanae_cancels_a_hand_effect`
  - `crates/game-rules/tests/rb_cross_chain.rs:1126` `c25_hanae_vs_skill_main`
  - `crates/game-rules/tests/rb_cross_long.rs:413` `l05_one_skill_several_counteractions`
  - `crates/game-core/src/engine/play.rs:759` (comment: cancelled trigger / press negated)
- **rulebook**: 花园多惠 (2) — cancel a hand effect / skill use (「取消」 window).
- **current**: card resolves normally; no cancel window.
- **expected**: a cancel [反击] window opens for the targeted hand effect / skill use, and the effect is cancelled on success.

## CH-03 — 骰子已经掷下 is placed twice and does not shut counter windows
- **kind**: implementation bug
- **class**: A
- **severity**: high
- **suggested batch**: counteraction windows
- **locations**
  - `crates/game-rules/tests/rb_cross_chain.rs:678` `c14_dice_cast_shuts_windows`
  - `crates/game-rules/tests/rb_gap_window.rs:111` `g18_shut_window_blocks_the_negate_one` (also names 网络链接异常 drop-one residue)
  - `rules/cards/card-mujica/src/dice_cast.rs:55,85` (「（此卡可以被反击）」 / lock timing)
- **rulebook**: 骰子已经掷下 — place the lock and shut further [反击] windows for that timing.
- **current**: placed twice; windows stay open (网络链接异常 can still drop a 武道馆 target).
- **expected**: one placement; remaining counter windows for that timing are closed.

## CH-04 — 无法将视线移开 cannot be played as a counter and does not force the move
- **kind**: implementation bug
- **class**: A
- **severity**: high
- **suggested batch**: counteraction windows
- **locations**
  - `crates/game-rules/tests/rb_cross_chain.rs:743` `c16_cannot_look_away_answers_a_counter` (`does not force the counter-user to move`)
  - `crates/game-rules/tests/rb_cross_long.rs:336` `l04_forced_move_into_remote_settle` (`err.play_phase`)
  - `rules/cards/card-mujica/src/cant_look_away.rs:48` (counteraction history ABI hole — see ABI-04; the play-as-counter half is separate)
- **rulebook**: 无法将视线移开 — usable as a [反击]; forces the counter-user to move.
- **current**: `err.play_phase` when played as the counter; even when it runs, the forced move does not happen.
- **expected**: legal as a [反击] response; the target is forced to move as stated.

## CH-05 — nested [反击] copy machinery (play-history / nested PlayCtx)
- **kind**: ABI defect
- **class**: B
- **severity**: high (Mortis and Mana Champion are the two "copy a counter" cards)
- **suggested batch**: host ABI: out-buffers / counteraction windows
- **locations**
  - `rules/cards/card-mujica/src/mortis_instinct.rs:22` (CanCounteract delegates to copy), `:37` (copy walks `H._playHistory`)
  - `rules/cards/card-sumimi/src/mana_champion.rs:57` (nested `H.Counteract(copy, p)`, shield + counteractor's draw)
  - `rules/cards/card-mujica/src/dice_cast.rs:55` (「（此卡可以被反击）」 needs the reverse-order counter play)
- **rulebook**: Mortis 「此卡打出时效果为场上任意其他玩家打出的上一张卡…」 + 「（此卡复制[反击]卡时可在符合条件时打出）」; Mana Champion 「此时场上其他玩家可如同自身的对应目标被指定一般打出[反击]卡，且其反击卡中针对打出玩家自身的效果改为你…」
- **current**: no play-history query, no cross-card `PlayCtx` nesting (`ctx::play_card` runs `play` without the copied card's identity / counteraction mode). Mortis copy is a stub; Mana Champion shield/draw unexpressed.
- **expected**: copy the latest eligible play and nest a full Play/Counteract run with forwarded Tags/Extreme/Doubled.
- **capability**: `play-history query` + `nested PlayCtx` (cross-card identity + counteraction mode).

## CH-06 — 网络链接异常 still drops the whole multi-target card in join fixtures
- **kind**: implementation bug
- **class**: A
- **severity**: high
- **suggested batch**: counteraction windows
- **locations**
  - `crates/game-rules/tests/rb_gap_window.rs:145` `g19_manas_join_window_lets_a_third_party_drop_one`
  - `crates/game-rules/tests/rb_cross_chain.rs:264` (test body of `c05_…` still describes the old whole-card negate; **D**: the ignore on `c05_cancels_one_designation_of_four` was already removed)
  - ABI already landed: `ctx::designations` / `ctx::cancel_designation` / `designation_cancelled` (TEST-FINDINGS §3)
- **rulebook**: sheet cancels **one** designation (「取消一个指定」 / 「改为你」), the rest of the play's designations land.
- **current**: when 真奈's join window is open, the whole 武道馆 is negated (everyone keeps 10000) instead of dropping only P0's designation.
- **expected**: per-pair cancel only.

## CH-07 — 「视为此卡未生效」 has no observer
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
- **kind**: implementation bug
- **class**: A
- **severity**: high (凑友希那 (3) / 香澄 stop-pots are common)
- **suggested batch**: movement
- **locations**
  - `crates/game-rules/tests/rb_cross_move.rs:527` `m12a_yukina_forced_stop` (stops P1 at 37, want 36)
  - `crates/game-rules/tests/rb_cross_move.rs:553` `m12b_yukina_stop_countered` (stop does not land on 36)
  - `crates/game-rules/tests/rb_cross_move.rs:696` `m15_first_forced_stop_wins` (（香澄） card does not force a stop at 44 via `place_raw`)
  - `crates/game-core/src/engine/play.rs:1400-1416` (`plan::set_stop_at` / `m.stop_at`)
  - `crates/game-core/src/engine/ops.rs:38` `set_stop_at`; `crates/game-core/src/engine/move_ctx.rs:142-143`
  - `rules/skills/skill-characters` 凑友希那 practice force-stop
  - **D**: `g11_forced_stop_beats_lock_end_rewrite` ignore already removed
- **rulebook**: 「令该玩家强制停下并[触发结算]」 — the passer stops on the named tile and settles there.
- **current**: `set_stop_at` during a `PassTile` hook does not land the piece on the hook's tile (off-by-one / never fires).
- **expected**: piece stops on the hook tile and settles.

## MOVE-02 — 白金燐子 (2) spends fire / prompts X but never moves
- **kind**: implementation bug
- **class**: A
- **severity**: high (Roselia staple)
- **suggested batch**: movement
- **locations**
  - `crates/game-rules/tests/rb_cross_move.rs:458` `m10_rinko_while_stunned` (`prompts for X but never moves (0 instead of 12) while stunned`)
  - `crates/game-rules/tests/rb_cross_move.rs:491` `m11_rinko_plus_yolo` (`never moves (pos 0 instead of 12 or 14)`)
  - `rules/skills/skill-characters/src/rinko_1cm.rs:58` (「此技能可以正常使用或在[晕眩]状态下使用」 stun-waiver flag is B — see SKILL-03)
- **rulebook**: 白金燐子 (2) — spend X fire before the move roll to move X (works while stunned).
- **current**: X prompt appears; the piece never moves (stunned and not).
- **expected**: after paying X the piece moves as stated (stun waived by 「可以正常使用」).

## MOVE-03 — MyGO band (2) move-1 replacement never fires / multi-activation `On::Play`
- **kind**: implementation bug + ABI defect
- **class**: B (multi-activation selection) unblocks A
- **severity**: high
- **suggested batch**: movement / skills: MyGO
- **locations**
  - `crates/game-rules/tests/rb_cross_move.rs:927` `m22_mygo_band_move1` (`move-1 replacement does not fire`)
  - `crates/game-rules/tests/rb_mygo.rs` `band_draw_for_two_crystals` (2-crystal draw half unreachable)
  - `rules/skills/skill-bands/src/` MyGO (2)/(3) — two `On::Play` entries; `use_skill` runs only the first
  - also Morfonica NNM: `rules/cards/card-morfonica/src/nanami_effort.rs`, `crates/game-rules/tests/rb_morfonica.rs:468` `nnm_option2_gains_x_times_circle_money_reward`
  - TEST-FINDINGS §6 item 8 (multi-activation selection)
- **rulebook**: MyGO band (2) has several activations (move-1 replacement and a 2-crystal draw); NNM 「发动以下效果中的一个」.
- **current**: `use_skill` runs only the first `On::Play`; later activations unreachable; a hand play can be refused (`nanami_effort_not_placed`).
- **expected**: either one entry branching on `is_placed()`, or a real selection prompt (see C-Q6).
- **capability**: multi-activation selection on `CardDef` / `use_skill`.

## MOVE-04 — 若能再次交汇 re-roll loop hits `world_mut` while a routine is pending
- **kind**: implementation bug
- **class**: A
- **severity**: high (re-roll cards are common)
- **suggested batch**: fuzz/determinism / movement
- **locations**
  - `crates/game-rules/tests/rb_cross_move.rs:258` `m06_reunion_20_tile_cap`
  - `crates/game-rules/tests/rb_cross_move.rs:284` `m06b_reunion_stops_on_passing`
- **rulebook**: 若能再次交汇 — re-roll the move dice (with a 20-tile cap / stop-on-passing shape).
- **current**: re-roll loop calls `world_mut` while a guest routine is pending → trap / abort; the cap/stop cannot be observed.
- **expected**: re-roll is safe under a pending routine (use the plan/overlay seam, not `world_mut`).

## MOVE-05 — mid-turn [停留] does not stop the main move
- **kind**: rulebook divergence
- **class**: C (glossary vs ileuxali design) — or A if glossary 51 is treated as rulebook
- **severity**: high
- **suggested batch**: movement
- **locations**
  - `crates/game-rules/tests/rb_mygo.rs:285` `rain_stay_blocks_this_turn_move` `#[ignore = "DISCREPANCY: glossary says [停留]=无法移动; engine lets a mid-turn [停留] holder complete the main move anyway"]`
  - `crates/game-core` move-branch selection (prohibition checked once)
  - TEST-FINDINGS §9 (ileuxali: check once at move-branch selection)
- **rulebook**: glossary 51 「[停留]=无法移动」.
- **current**: a [停留] gained mid-move does not stop the current move.
- **expected**: either stop immediately (glossary) or only the next move (design reference).
- **question**: C-Q7.

## SETTLE-01 — 笑容大游行 tile-swap overflows the stack
- **kind**: implementation bug
- **class**: A
- **severity**: high (HHW signature continuous)
- **suggested batch**: HHW cards / crystal/marker accounting
- **locations**
  - `crates/game-rules/tests/rb_cross_tiles.rs:547` `t13_smile_parade` `#[ignore = "DISCREPANCY: 笑容大游行 overflows the stack when it triggers"]`
  - `crates/game-rules/tests/rb_gap_res.rs:281` `g30_smile_parade_swap_with_a_field_card_in_place`
  - `rules/cards/card-hhw/src/smile_parade.rs:164` (（3） 「交换位置」 is a board *position* exchange, not a colour override / SettleBody)
  - `rules/card-sdk` / `crates/game-rules/src/wasm_rules.rs:4420` (trait-method recursion overflow note)
- **rulebook**: 笑容大游行 (3) 「[持续] 当此卡位于格子上时，那格视为与"弦卷集团"格子交换位置，任何玩家在此卡放置的格子上[触发结算]后此卡放入弃牌堆」.
- **current**: trigger re-enters and overflows the stack; the swap is modeled on the wrong axes (colour override / SettleBody).
- **expected**: resolve the settle as if the mover landed on 弦卷集团 (rents/ownership exchanged for that settle); discard after; no recursion.

## SETTLE-02 — CRYCHIC band (1) at 6+ hand cards still lets money move
- **kind**: implementation bug
- **class**: A
- **severity**: high
- **suggested batch**: skills: Roselia / payment pipeline
- **locations**
  - `crates/game-rules/tests/rb_cross_status.rs:159` `s05_crychic_band_six_cards`
  - `rules/skills/skill-bands/src/crychic.rs:155` (「无法从手中打出任何牌」 is a global play gate — also A/B, see SKILL-05)
- **rulebook**: CRYCHIC band (1) at 6+ hand cards blocks gain/loss (sheet).
- **current**: money still moves (gain/loss not blocked).
- **expected**: no money gain/loss while the gate is up.

## SETTLE-03 — 丰川祥子 (1) does not absorb stays when passing another player
- **kind**: implementation bug
- **class**: A
- **severity**: high
- **suggested batch**: movement / skills: Mujica
- **locations**
  - `crates/game-rules/tests/rb_cross_status.rs:268` `s08_sakiko_absorbs_statuses`
  - `crates/game-rules/tests/rb_gap_window.rs:221` `g21_summer_camp_immune_to_the_rains_zone` (same absorb + 夏日合宿 un-designated)
- **rulebook**: 丰川祥子 (1) absorbs stays when passing another player.
- **current**: absorb does nothing on a pass.
- **expected**: stays on the passed player are absorbed / cancelled as the clause says.

## WINDOW-01 — Welcome to Ave Mujica's World (1) cannot be played (`err.play_phase`)
- **kind**: implementation bug
- **class**: A
- **severity**: high
- **suggested batch**: HostRequest handlers / counteraction windows
- **locations**
  - `crates/game-rules/tests/rb_cross_chain.rs:1207` `c27_welcome_state_switch`
  - `rules/cards/card-mujica/src/welcome_mujica.rs:31` (（1） 「若指定了不存在状态2的玩家则无效果」 reading — C, see CH-08)
- **rulebook**: 「转换任意一名玩家的状态」 (状态1 ↔ 状态2).
- **current**: play is refused with `err.play_phase`.
- **expected**: playable in its window.

## WINDOW-02 — R.I.O.T. + 宣战布告 same timing loses the 500 payment
- **kind**: implementation bug
- **class**: A
- **severity**: high (war declaration is a whole-table event)
- **suggested batch**: counteraction windows / payment pipeline
- **locations**
  - `crates/game-rules/tests/rb_cross_chain.rs:638` `c13_riot_and_war_declaration` (P0 ends 12000, want 11500)
  - `crates/game-rules/tests/rb_cross_long.rs:105` `l01_counter_war_five_links_deep` (ring skips P1; 宣战布告 never offered; nothing settles)
- **rulebook**: R.I.O.T. / 宣战布告 — 500 payment and the war ring offers.
- **current**: 500 missing; in a 4-player war the ring skips P1 and nothing settles.
- **expected**: payment lands; every eligible seat is offered.

## WINDOW-03 — EXIST redirect / 「造成影响」 latch is at declaration, not settlement
- **kind**: rulebook divergence
- **class**: C
- **severity**: high
- **suggested batch**: counteraction windows / draw/discard pipeline
- **locations**
  - `rules/cards/card-ras/src/exist.rs:55` (「造成影响」 = effect resolved vs declaration)
  - `crates/game-rules/tests/rb_gap_money.rs:334` `g07_exist_redirect_then_negated_designation` `#[ignore = "RULING: does a negated EXIST redirect count as 「造成影响」? assert draws 1"]`
  - `crates/game-rules/tests/rb_cross_chain.rs:599` `c12_exist_vs_summer_camp` (找回珍妮弗 does not land on P2 with 夏日合宿 + EXIST)
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
  - `crates/game-rules/tests/rb_fuzz_found.rs:85` `unknown_card_id` (unknown card id in hand/draw/discard/field)
  - `crates/game-rules/tests/rb_fuzz_found.rs:128` `determinism_break` (same seed + answers diverge)
  - `crates/game-rules/tests/rb_fuzz_found.rs:142` `save_restore_break` (save/restore diverges)
  - `crates/game-rules/tests/rb_fuzz_found.rs:160` `negation_not_total` (negating a no-target [手] did not reduce to never playing it)
  - `crates/game-rules/tests/rb_fuzz_found.rs:186` `monotonicity_break` (a +N roll modifier shortened a move)
  - **D**: `card_trap_reachable`, `immunity_gap` ignores already removed
  - `crates/game-rules/tests/inline_provider.rs:247` B2 HostRequest learn-pass gap (see HOST-01)
- **rulebook**: general engine invariants (same answers → same world; save/restore round-trip; negation is total; +N never shortens).
- **current**: each shape is a latent fuzz repro still ignored.
- **expected**: green under the existing `rb_fuzz_found` assertions.
- **note**: may share one pipeline root cause (guest/world overlay + HostRequest ordering); investigate as one batch.

## HOST-01 — HostRequest B2 equivalence gap (learn pass vs live+host_effects)
- **kind**: ABI defect / implementation bug
- **class**: B
- **severity**: high (breaks inline guest equivalence under real rules)
- **suggested batch**: host ABI: out-buffers / HostRequest handlers
- **locations**
  - `crates/game-rules/tests/inline_provider.rs:247` (B2 equivalence gap TODO), `:343` (cross-ref)
  - `docs/BOT.md` §3.2
- **rulebook**: n/a — ABI contract: inline learn pass must see the same world as the replay model's last pass (`live + host_effects`).
- **current**: learn pass runs on a copy taken *before* the HostRequests it discovers; a body that queries state a host routine changed (money after a `pay`) sees the pre-effect value.
- **expected**: rebase the guest's writes onto the updated live world between passes.
- **capability**: two-pass drive rebase / overlay_guest_state extended to HostRequest sequencing.

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
- **locations**
  - `crates/game-rules/tests/rb_pp.rs:877` `band_skill_overflow_to_crystals`
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
  - `rules/events/src/bangdream_chan.rs:35` / `crates/game-rules/tests/rb_events.rs:1047` `bangdream_chan_shortens_the_clocks` (免费时间/恢复时间 are host clocks)
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
- **locations**
  - `crates/game-rules/tests/rb_pp.rs:1078` `accident_blocks_pp_skill_2`
- **rulebook**: 初次演出事故 blocks the PP skill; 丸山彩 (2) PayChoose hook must not prompt.
- **current**: `skillBlock` token is set (`tok=1`) but the guard/body `skill_blocked` check does not see it.
- **expected**: hook-guard dispatch consults the token.

## SKILL-02 — 冰川日菜 (2) lottery borrow never runs
- **kind**: implementation bug
- **class**: A
- **severity**: medium
- **suggested batch**: skills: PP
- **locations**
  - `crates/game-rules/tests/rb_pp.rs:1128` `hina_skill_2_borrows_skill` (`skill.hinaLottery.borrowed` stays -1)
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
  - tests: `rb_card_paths::mutsumi_never_2_*`
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
- **class**: A
- **severity**: medium
- **suggested batch**: movement / skills: MyGO
- **locations**
  - `rules/skills/skill-characters/src/rana_parking.rs:46` (pre is `MINE` but body reads `player_id` as "the other"; bails when it equals the owner — always)
- **rulebook**: 「你与其他玩家重合时」.
- **current**: clause never fires.
- **expected**: fires on overlap; read `target` as the other player.

## SKILL-09 — kaoru_prince 「每次[经过]或被[经过]时」 fires for every passer
- **kind**: implementation bug
- **class**: A
- **severity**: medium
- **suggested batch**: movement
- **locations**
  - `rules/skills/skill-characters/src/kaoru_prince.rs:33` (`""` pre currently fires for every passer)
- **rulebook**: 「每次[经过]或被[经过]时」 — `Pass` half is "I pass" (`actor == owner`).
- **current**: unfiltered (suspected bug).
- **expected**: filter `actor == owner` on the Pass half.

## TILE-01 — Anon Tokyo link arrangements (Fire bird / Repaint / ONE OF US / linked rent)
- **kind**: implementation bug
- **class**: A (arrangements) + C (mortgaged partner half)
- **severity**: medium
- **suggested batch**: rent multipliers / tiles
- **locations**
  - `crates/game-rules/tests/rb_cross_tiles.rs:128` `t02_anon_link_plus_fire_bird`
  - `crates/game-rules/tests/rb_cross_tiles.rs:163` `t03_anon_link_plus_repaint`
  - `crates/game-rules/tests/rb_cross_tiles.rs:199` `t04_anon_link_mortgaged_partner` (RULING)
  - `crates/game-rules/tests/rb_cross_tiles.rs:228` `t05_one_of_us_shares`
  - `crates/game-rules/tests/rb_cross_tiles.rs:838` `t21_riki_plus_anon_link` (quarter-rent on a linked tile)
  - `rules/cards/card-ag/src/one_of_us.rs:138,200` (raw money add / 平分 shape)
- **question**: C-Q13 (mortgaged partner tile's half 收费).

## TILE-02 — agent half-rent 「同色」 vs colour overrides
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: rent multipliers
- **locations**
  - `rules/tiles/src/agent.rs:19` (「同色」 = board `group`; book never says whether `anyColor` / `colorFor:<p>` widens it)
  - `crates/game-rules/tests/rb_cross_tiles.rs:638` `t16_soyo_mixed_colors` (（soyo）混合的颜色 agent half-rent)
  - TEST-FINDINGS §6 item 10
- **question**: C-Q14.

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
- **kind**: implementation bug
- **class**: A
- **severity**: medium
- **suggested batch**: movement
- **locations**
  - `crates/game-rules/tests/rb_cross_tiles.rs:495` `t12_student_council_check`
- **rulebook**: 学生会的检查 — placement + forced stop + 「本回合无法加盖房屋」 (linger `NO_BUILD` already landed).
- **current**: placement / forced stop arrangement wrong.

## TILE-05 — Parking Space + 要乐奈 (2) teleport arrangement
- **kind**: implementation bug
- **class**: A
- **severity**: medium
- **suggested batch**: movement
- **locations**
  - `crates/game-rules/tests/rb_cross_tiles.rs:957` `t24_parking_space`
- **rulebook**: Parking Space body-replace + Rana (2) Space teleport as the main move.

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
- **locations**
  - `crates/game-rules/tests/rb_gap_res.rs:209` `g28_maya_look_at_top_still_counts_as_a_draw`
- **rulebook**: look-at-top + keep is a draw for 「梦在前方」's crystal.
- **current**: `drew` not raised on the kept card.
- **expected**: raises `drew` like any draw.

## DRAW-02 — 星月夜 even-roll crystal-removal offer never prompts
- **kind**: implementation bug
- **class**: A
- **severity**: medium
- **suggested batch**: draw/discard pipeline / skills: Morfonica
- **locations**
  - `crates/game-rules/tests/rb_morfonica.rs:304` `starry_even_roll_of_another_player_offers_crystal_removal`
  - `rules/cards/card-morfonica/src/starry_night.rs`
- **rulebook**: 「当其他玩家的移动掷骰为偶数时可[支付]1000资金并移除此卡一个[奇迹水晶]」.
- **current**: engine never prompts.

## DRAW-03 — 游击演出 [特] window at turn end never opens
- **kind**: implementation bug
- **class**: A
- **severity**: medium
- **suggested batch**: skills: RAS
- **locations**
  - `crates/game-rules/tests/rb_ras.rs:1411` `guerrilla_special_offers_to_mark_a_deed_as_livehouse`
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
  - `crates/game-rules/tests/rb_gap_res.rs:133` `g26_popipa_crystals_and_kasumi_fire_on_one_settle`
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
  - `crates/game-rules/tests/rb_rulebook.rs:866`
- **rulebook**: `E14` names only [重叠] for the 0-step case; `B41`+`E13` read as both-fire for a teleport.
- **question**: C-Q20.

## MOVE-07 — teleport as the main move (settle shape)
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: movement
- **locations**
  - TEST-FINDINGS §6 item 1
  - `crates/game-rules/tests/rb_general.rs` `tsugu_lt26_choose_move_settles` (pins the immediate offer)
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
- **kind**: implementation bug
- **class**: A
- **severity**: medium
- **suggested batch**: movement / draw/discard pipeline
- **locations**
  - `crates/game-rules/tests/rb_cross_move.rs:140` `m03_eve_then_yolo` (`pos 5 instead of 8`)
- **note**: may share a root with MOVE-03 / Y.O.L.O additive timing (**D**: `inter_yolo_pushes_haneoka_over_20` ignore already removed).

## MOVE-11 — 是我自己的问题 still fires after an abnormal move effect
- **kind**: implementation bug
- **class**: A
- **severity**: medium
- **suggested batch**: counteraction windows / movement
- **locations**
  - `crates/game-rules/tests/rb_cross_move.rs:1166` `m28b_my_own_problem_after_forced_move` (and `m28a` body)
- **rulebook**: sheet says it does **not** fire after an abnormal move effect.
- **current**: still fires.

## MOVE-12 — RAS band (1) cannot decline the second abnormal-move effect
- **kind**: implementation bug
- **class**: A
- **severity**: medium
- **suggested batch**: counteraction windows
- **locations**
  - `crates/game-rules/tests/rb_cross_move.rs:404` `m09_ras_band_first_abnormal_only` (`err.play_phase` / no window)
- **rulebook**: RAS band (1) — first abnormal only; a second is optional.

## MOVE-13 — 星月夜's re-roll scope (whole dice set vs base die)
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: movement
- **locations**
  - `crates/game-rules/tests/rb_gap_move.rs:122` `g08_starlit_reroll_of_a_whole_3d20_set` (assert full set)
- **question**: C-Q23.

## MOVE-14 — 普通与理所当然 copy signed or unsigned 移动格数
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- **suggested batch**: movement
- **locations**
  - `crates/game-rules/tests/rb_gap_move.rs:352` `g13_ordinary_copies_a_reversed_move_distance` (assert unsigned)
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
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: settle stages
- **locations**
  - `crates/game-rules/tests/rb_cross_long.rs:522` `l07_extra_turn_cascade` (want 2 stun layers, get 1)
  - TEST-FINDINGS §6 items 2 and R-7
- **question**: C-Q26 (does removing the last crystal grant the extra turn? does 「until your next turn start」 end at an extra turn's start?).

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
  - `crates/game-rules/tests/rb_mujica.rs` `saki_move_halves_a_forced_stop_and_pay` (green)
- **question**: C-Q28.

## SETTLE-07 — [晕眩] vs a card's own follow-on gain
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: payment pipeline
- **locations**
  - TEST-FINDINGS §6 item 9
  - `crates/game-rules/tests/rb_mujica.rs` `heart_rain_fallback_stun_and_money`, `heart_rain_nobody_in_range_fallback`
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
- **kind**: implementation bug
- **class**: A
- **severity**: medium
- **suggested batch**: crystal/marker accounting
- **locations**
  - `crates/game-rules/tests/rb_cross_chain.rs:1175` `c26_immunity_vs_crystal_mover`
- **rulebook**: 「此卡不受任何其他效果影响」 — should not be a crystal endpoint.
- **current**: offered anyway.

## CH-12 — [不可阻挡] still grants new [除外] from 无路矢
- **kind**: implementation bug
- **class**: A
- **severity**: medium
- **suggested batch**: crystal/marker accounting
- **locations**
  - `crates/game-rules/tests/rb_gap_window.rs:302` `g23_unstoppable_refuses_the_exile_cost`
- **rulebook**: glossary 51 「无法获得新的[除外]层数」.
- **current**: 无路矢 still grants 2 [除外] to the holder.

## CH-13 — Here the world does not land on the second card played in one turn
- **kind**: implementation bug
- **class**: A
- **severity**: medium
- **suggested batch**: draw/discard pipeline
- **locations**
  - `crates/game-rules/tests/rb_cross_chain.rs:957` `c21_here_the_world`
  - **D note**: `l06_one_draw_several_effects` ignore already removed (capture-vs-autoplay is C-Q33)
- **rulebook**: Here the world captures a drawn / second card.

## CRYSTAL-01 — crystal can pay a skill's fire cost (FirePaying)
- **kind**: ABI defect
- **class**: B
- **severity**: medium
- **suggested batch**: crystal/marker accounting
- **locations**
  - `crates/game-rules/tests/rb_mygo.rs:126` `light_crystal_pays_skill_fire_cost`
  - `crates/game-rules/tests/rb_mygo.rs:215` `light_removed_when_crystals_run_out`
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
- **locations**
  - `rules/cards/card-ag/src/crimson_soul.rs:126` (spending the last crystal on a [支付] loses the payee their 500)
- **rulebook**: rule (1) — 500 is owed the moment the crystal is spent; not conditioned on the card still being in play.
- **current**: paid after rule (3) may have discarded the card.
- **expected**: pay 500 as part of the spend, before the discard check.

## CRYSTAL-04 — NNM (2) 「获得x次经过CiRCLE时的资金奖励」 does not scale with the live reward
- **kind**: implementation bug
- **class**: A
- **severity**: medium
- **suggested batch**: crystal/marker accounting / rent multipliers
- **locations**
  - `rules/cards/card-morfonica/src/nanami_effort.rs:114` (hard-codes 2000)
  - `crates/game-rules/tests/rb_morfonica.rs:468` `nnm_option2_gains_x_times_circle_money_reward`
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
  - `crates/game-rules/tests/rb_events.rs:338` `habinel_teleports_to_smile_ship` `#[ignore = "RULING: …"]`
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
- **kind**: ABI defect
- **class**: A
- **severity**: high (common play with buy/build hooks)
- **suggested batch**: HostRequest handlers / out-sentinel answers
- **locations**
  - `crates/game-rules/src/wasm_rules.rs:2239-2343` — `Buy` / `Acquire` / `AgentOffer` / `Build` / `OfferBuild` / `Mortgage` / `DrawEvent` / `PayRent` / `OfferBuy` / `OfferForceBuy` / `OfferBuildOne` / `RaiseBought` all `return Ok(1);` unconditionally
  - `crates/game-core/src/engine/play.rs` `card_buy` / `card_acquire` / `card_build` / `buy` / `build` — `Flow<()>`; failure is a silent `Ok(())`
  - consumers that trust the reply: `rules/cards/card-pp/src/dream_ahead.rs:193` (`if !ctx::card_buy(...) { return; }`), `rules/skills/skill-bands/src/afterglow.rs:34` (`if ctx::card_build(...) { log free house }`)
- **ABI contract**: `rules/card-sdk/src/ctx.rs:2437` `pub fn card_buy(...) -> bool`, `:2479` `pub fn buy(...) -> bool`
- **current**: every purchase HostRequest resumes as `true` (`!= 0` on answer `1`)
- **expected**: `0` when the engine no-op'd (BuyGate cancel, already owned, `buyBefore` cancel, payment unpaid, `why_not_build_on`, `buildBefore` cancel, max houses)
- **impact**: `dream_ahead` still spends 3 crystals and logs "bought" after a refused buy; `afterglow` always logs a free house even when `card_build` refused
- **fix sketch**: make `card_buy`/`card_build`/`card_acquire` return success (or inspect post-state: owner changed / houses+1) and encode that in `apply_host_request`

## HOST-07 — `ctx::buy_quotes` is not the hook-aware quote; ignores `player_id`/`kind`
- **kind**: ABI defect
- **class**: A
- **severity**: high (common play with buy-hook cards)
- **suggested batch**: host ABI: queries / quote==charge
- **locations**
  - `crates/game-rules/src/hostfns.rs:1613-1638` — `let _ = (player_id, kind);` quotes from `is_buyable` + `buy_price` (`quote_native` = land + standing houses); never `base_quote`, never `BuyGate`/`BuyAdd`/`BuyMul`/`BuySet`
  - correct path exists: `crates/game-rules/src/wasm_rules.rs:5424+` `CardRules::buy_quote` (hooks) and `crates/game-core/src/engine/play/purchase.rs:211-237` `base_quote` / `quote_for`
  - guest caller: `rules/tiles/src/agent.rs:87` `ctx::buy_quotes(player_id, BuyKind::Agent, &same)`
  - hook cards that change the charge: `card-general/src/tsugu_ycm.rs:48-50` (`BuyAdd` −1500), `card-ppp/src/maze_warehouse.rs:20-21` (`BuySet` 0), Roselia band `BuyMul` ½
- **ABI contract** (`docs/PURCHASE.md:125-129`, `purchase.rs:83-87`): post-hook price and `BUYABLE + BuyGate`; "The commit re-quotes, so quote == charge."
- **current**: guest quote = native land+houses; eligibility = `is_buyable` only. Force kind would also be wrong (`base_quote` doubles)
- **expected**: same `CardRules::buy_quote` / `quote_for` the engine uses (or the cheap path only when `declares_buy()` is false)
- **impact**: agent offer can show full price when Tsugu's −1500 applies, non-zero when maze_warehouse makes it free, or allow a buy BuyGate would refuse. Engine's built-in `agent_landing` (`play.rs:2068`) *does* use hook-aware `buy_quote_for` → dual-path drift
- **also stale**: `rules/card-sdk/src/ctx.rs:2445` and `docs/PURCHASE.md:110` still say "One `HostRequest::Quote`" — that variant does not exist (`host.rs:236`)

## HOST-08 — `plan::set_no_build` / `plan::set_build_anywhere` trap on native, unregistered on wasm
- **kind**: ABI defect
- **class**: A (`set_no_build`) / B (`set_build_anywhere`)
- **severity**: medium (latent; no current caller in `card_all.wasm`)
- **suggested batch**: host ABI: plan flags
- **locations**
  - guest wrappers `rules/card-sdk/src/ctx.rs:1670-1676`; imports `rules/card-sdk/src/ctx/sys.rs:494-497`
  - **not** registered in `crates/game-rules/src/host.rs` linker (gap vs `set_no_buy` / `set_can_build`)
  - native `crates/game-rules/src/native_shims.rs:1261-1268`, `:3223-3231` — `Err(HostErr::trap("set_no_build: no host body yet"))`
  - intent already documented `crates/game-rules/src/world.rs:971`: "`no_build` -- folded into `can_build = false`; one flag is enough"
  - live flag `crates/game-core/src/engine/move_ctx.rs:160-164` `can_build`; enforced `play.rs:2656-2657`
- **current**: calling either API traps on native; on wasm a module that keeps the import fails to instantiate
- **expected**: `set_no_build(true)` ≡ `set_can_build(false)`; `set_build_anywhere` needs a new plan capability (builds are landing-only today)

## HOST-09 — `set_bonus` dead guest import + native trap
- **kind**: ABI defect (dead surface)
- **class**: D (dead) / A if a card starts using it
- **severity**: low
- **suggested batch**: guest imports vs host registration
- **locations**
  - `rules/card-sdk/src/ctx/sys.rs:524-525` `pub fn set_bonus(n: i32, ptr: i32, len: i32);`
  - no `ctx` wrapper, never called; not in host linker; `native_shims.rs:1320-1322` / `:3339-3341` trap `"set_bonus: no host body yet"`
  - absent from built `card_all.wasm` imports (GC)
- **expected**: implement (move-plan / dice bonus buffer) or delete the import + native stub

## HOST-10 — host linker names with no guest import (dead registrations)
- **kind**: ABI defect (dead surface)
- **class**: D
- **severity**: low
- **suggested batch**: guest imports vs host registration
- **locations** (registered in `crates/game-rules/src/host.rs` but **not** in `rules/card-sdk/src/ctx/sys.rs`)
  | host name | guest actually uses |
  |---|---|
  | `extreme` (`host.rs:3492`) | only `set_extreme` (no reader) |
  | `move_kind` (`:4437`) | `trig_move_kind` |
  | `move_tag` (`:4353`) | `trig_move_tag` |
  | `set_trigger_target` (`:4283`) | `trig_set_pay_target` (same body) |
  | `set_trigger_cancelled` (`:4290`) | `trig_set_cancelled` |
- **current**: not a runtime break (wasm imports are the guest's names) but confuses the next ABI table
- **expected**: drop the unused linker names or alias them explicitly

## HOST-11 — CEL `roll_source` sentinel doc disagrees with guest/host
- **kind**: ABI defect (sentinel mismatch)
- **class**: C (which sentinel is the contract?) / A to align
- **severity**: low
- **suggested batch**: CEL vocab vs guest readers
- **locations**
  - `rules/cond/src/vocab.rs:196-204` — `doc: "where a roll came from, -1 = none"`, `ty: Ty::Int` (not OptInt)
  - `rules/card-sdk/src/abi.rs:627-629` — `roll_source::NONE = 0`
  - host fills `0` for unattributed: `hostfns.rs:1893-1894`, `cond_pre.rs:1252`, ambient `:1341`
  - existing guard uses the guest code: `rules/cards/card-ras/src/layer_keep.rs:29` `"… && roll_source == 1"`
- **current**: guards written as `roll_source == -1` or `== null` never match
- **expected**: align docs (and any OptInt) to `0 = none`, or change the fill to `-1` and migrate `NONE`

## HOST-12 — stale purchase-surface comments
- **kind**: stale
- **class**: D
- **severity**: low
- **suggested batch**: docs / comments
- **locations**
  - `wasm_rules.rs:2247-2249` "v40 purchase surface. P0: stubs" — arms are implemented
  - `ctx.rs:2445` + `docs/PURCHASE.md:110` "One `HostRequest::Quote`" — no such variant
  - `hostfns.rs:1614` documents native quote as acceptable because "commit re-quotes" — that *violates* the stated `quote == charge` contract (see HOST-07)

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
| `c05_cancels_one_designation_of_four` | `crates/game-rules/tests/rb_cross_chain.rs:265` | whole-card negate (ABI per-pair cancel landed) |
| `l06_one_draw_several_effects` | `crates/game-rules/tests/rb_cross_long.rs:471` | capture-vs-autoplay (record-only) |
| `t11_fire_bird_plus_hhw_double` | `crates/game-rules/tests/rb_cross_tiles.rs:458` | multiplier order (still a C question, MONEY-01) |
| `immunity_gap` | `crates/game-rules/tests/rb_fuzz_found.rs:198` | effect on immune player |
| `card_trap_reachable` | `crates/game-rules/tests/rb_fuzz_found.rs:108` | card module trapped |
| `g11_forced_stop_beats_lock_end_rewrite` | `crates/game-rules/tests/rb_gap_move.rs:270` | forced stop vs SettleBefore end-rewrite |
| `inter_yolo_pushes_haneoka_over_20` | `crates/game-rules/tests/rb_mygo.rs:1379` | Y.O.L.O timing CROSS-AGENT |
| `choose_your_stage_answers_an_abnormal_move` | `crates/game-rules/tests/rb_roselia.rs:369` | self-[传送] window (see CH-01; the encore twin `ix_encore_…` is still open) |
| `sweet_escape_gate_requires_high_rent` | `crates/game-rules/tests/rb_sumimi.rs:185` | 「收费标价」 (still a C question if reopened) |

## D-02 — NEGATION-AUDIT V2 "no activation cost" TODOs (already landed)
- One root cause, already applied: the six text-less money play gates are gone; in-body 「[消耗]/[支付]」 is effect content and takes the Q1 shortfall path.
- Locations (documentation leftovers only):
  - `rules/cards/card-roselia/src/fire_bird.rs:18`
  - `rules/cards/card-general/src/gacha10.rs:13`
  - `rules/cards/card-ag/src/crimson_soul.rs:28`
  - `rules/cards/card-morfonica/src/starry_night.rs:23`
  - `rules/cards/card-hhw/src/believe_you.rs:22`
  - `rules/cards/card-hhw/src/kokoro_circle.rs:30`
  - `crates/game-rules/tests/rb_money.rs:365`
- Tests green: `gacha10_playable_while_short_then_bankrupts`, `fire_bird_playable_while_short_then_mortgages`, `crimson_soul_playable_while_short_then_bankrupts`, `believe_you_playable_while_short_then_bankrupts`, `kokoro_circle_playable_while_short_then_bankrupts`, `starry_night_playable_while_short_then_bankrupts`.
- Action: rewrite the TODO as a settled note (or delete) so it stops counting as open.

## D-03 — 2026-10-09 batch already fixed (do not re-open)
- 凑友希那 (3) fire cap; 要乐奈 (2) Space teleport; 白金燐子 (2) pre-roll window (test bug); 麻里奈小姐的礼物箱 「可」; 飞鸟山之战 stun layers (test bug); CiRCLE band (2) doubling; 游击演出 ends on the chosen tile.
- See TEST-FINDINGS "Resolved (2026-10-09)".

## D-04 — other already-landed surfaces referenced by TODOs
- per-pair cancel (`ctx::cancel_designation`); `payAfter` / `gains_this_turn`; `overlay_guest_state`; `EXILE_MAIN`; ABI v35 skill-attachment / `invoke_skill` / `raise_bought` / `add_follower`; purchase surface P0–P5; settle/move stage model ABI 43; counteract ring ruling 2026-10-07.

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
7. **L947–952 "Known failures at time of writing" (`rb_card_paths` 4 cases).** **Stale** — closed the same day.
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

*(A+B+C = 77 open root-cause items after dedup of 144 TODOs + ~85 ignore rows + the 2026-10-10 ABI audit. Some A items are "contained engine bug" one-liners; some B items unblock several A/C.)*

## Per kind

| kind | count (approx) |
|---|---|
| implementation bug | 38 |
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
| events | EVENT-01..04, ABI-09 | |
| fuzz/determinism | FUZZ-01, HOST-03 | 1 high A |
| tiles | TILE-06, HOST-04, LOW-02 | |

---

# Proposed batch order

Do these in order; each batch should be a PR that does not share files with the next.

1. **high-A engine bugs — movement** (MOVE-01, MOVE-02, MOVE-04, SKILL-08, SKILL-09, TILE-04, TILE-05)
   - Unblocks the most ignored `rb_cross_move` tests. Files: `game-core/engine/{play,ops,move_ctx}.rs`, `skill-characters/{rana_parking,kaoru_prince}.rs`.
2. **high-A counteraction windows** (CH-02, CH-03, CH-04 play-as-counter, CH-06, WINDOW-01, WINDOW-02)
   - Files: `game-rules/src/wasm_rules.rs` chain, `card-mujica/*`, `card-roselia` Hanae.
3. **high-A money** (MONEY-03 Repaint shaped half, CRYSTAL-03 crimson_soul 500 timing, SETTLE-02 CRYCHIC 6-hand block, SETTLE-03 Sakiko absorb)
4. **high-A host ABI: reply sentinels + quote==charge** (HOST-06, HOST-07, HOST-08 `set_no_build`)
   - Files: `crates/game-rules/src/{wasm_rules,hostfns,native_shims}.rs`, `rules/card-sdk/src/ctx.rs`. Do not share a PR with card bodies.
5. **high-B unblockers** (in this order)
   - **MOVE-03 multi-activation selection** (unblocks MyGO band (2) + NNM)
   - **CH-05 play-history + nested PlayCtx** (unblocks Mortis, Mana Champion, dice_cast counters)
   - **CH-01 self-abnormal Effect chain** (unblocks 安可 / 像往常一样 / 我自己的问题 / RAS band (1))
   - **CRYSTAL-01 FirePaying** (unblocks 灯 不再迷茫)
   - **HOST-01 HostRequest rebase** (unblocks inline equivalence + some fuzz)
6. **high-A fuzz/determinism** (FUZZ-01) once HOST-01 is in.
7. **C questions to the user** (collect all of §C below in one sitting). While waiting, continue on:
8. **medium-A contained bugs** (SKILL-02 hina lottery, SKILL-07 soyo expiry, DRAW-01/02/03, CH-11/12/13, CRYSTAL-04 NNM live reward, CRYSTAL-08 ringing_bloom continuous max, MOVE-10/11/12, LOW-02 docs)
9. **medium-B ABI pile** (ABI-01..12, HOST-04/05/09/10/11) — one ABI bump, many cards.
10. **medium-C leftovers + events** (EVENT-01..04) after the user answers §C.
11. **D cleanup** — rewrite NEGATION-AUDIT V2 TODOs as settled notes; strike the stale TEST-FINDINGS / COVERAGE / CROSS-TESTS sections listed above.

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

**C-Q14 (TILE-02, agent 「同色」).**
「每种颜色拥有一个地产商格子」 / half-charge 「同色」: does a per-player colour override (`anyColor` / 「该格获得所有颜色」 / `colorFor:<p>`) widen the colour group? (Engine now: board `group` only.)

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
(26a) Does removing the last crystal of 燃尽前的线香花火 also grant 「获得一个额外回合」?
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

---

*End of backlog. Standing file: `docs/rulebook/BACKLOG.md` on branch `rulebook-triage`.*
