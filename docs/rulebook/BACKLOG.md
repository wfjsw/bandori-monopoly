# Rulebook / ABI backlog

**2026-10-28 cleanup.** Standing backlog for continuous iteration on rulebook
divergences, rule-implementation bugs and ABI problems. Fix agents take
independent batches from here; do not merge into the primary branch from this
file. (Originally opened 2026-10-10 as `rulebook-triage`; full recount
2026-10-17; card-rule TODO review + reconstruction 2026-10-20; 2026-10-27 card
sheet review + triage; 2026-10-28 triage applied, closed items removed.)

On 2026-10-27 the 47 completed item sections and the closed-audit tail were
deleted. On 2026-10-28 every review row was triaged and merged by root cause
into A/B/C/D, and the 776 A items were fixed on branch
`worktree-card-rulebook-review`. This file now lists **open work only**: the B
items grouped by ABI capability, the still-open legacy items, and the C
questions as verbatim text at the end. Every fixed, landed or dropped item —
the A list, the D row, and all earlier closed-batch notes — was removed from
this file on 2026-10-28; the record lives in the branch's commit history.

Sources (re-counted 2026-10-27 against master `325845ba`):
- **162** `TODO(规则书)` tags in `rules/` + `crates/`. Breakdown: `rules/cards`
  72, `rules/skills` 27, `rules/events` 21, `crates/` 23, other 19. Some are
  convention markers (a module doc naming the policy, not a gap); the
  convention-marker list is below.
- **13** live `#[ignore = "RULING: …"]` rulebook tests in
  `rules/tests/tests/*.rs`, zero live `DISCREPANCY:` (comment mentions of the
  convention do not count). Plus 2 meta `q4_unintended` tooling ignores (soak /
  report generator). `crates/game-rules/tests/` is clean.
- `docs/rulebook/TEST-FINDINGS.md` (§3 ABI / §4 per-card / §6 rulings), `COVERAGE.md`, `CROSS-TESTS.md`, `NEGATION-AUDIT.md`, `PIPELINE-AUDIT.md`, `SETTLE-STAGES.md`, `docs/GUARDS.md`, `docs/MARKERS.md`, `docs/STATUS-HOSTS.md`, and the ambiguity backlog `docs/rulebook/spec/OPEN-QUESTIONS.md` (OPEN-* ids ↔ the C-Q list below)
- 2026-10-10 ABI audit of `crates/game-rules/src/{host,hostfns,native_shims,wasm_rules}.rs`, `crates/card-sdk/src/{ctx,sys,abi}.rs`, `crates/game-core` purchase/play

**Current ABI: v104** (`crates/card-sdk/src/abi.rs` `ABI_VERSION`). The body
still quotes older era names in places; where a kind has since been renamed
(`HookKind`/`GateKind` → `ActionKind`/`FilterKind`; `MoveRoll` → `DiceRollResult`;
the `Pay*` / `Roll*` families restructured; `ruleset-core` → `bandori-std`),
treat the live name as canonical. v104 widened the turn-snapshot wire to
`(pos, state, money, owners, houses, mortgaged)` (像往常一样's undo).
v103 reworked the roll API:
`ctx::roll` is the windowed default and `ctx::roll_direct` the windowless
variant, both returning `Result<i32, Prompt>`; findings written against the
pre-v103 `ctx::roll` were rechecked in triage.

**Crate renames** — every path below is quoted in the new names:
- `rules/skills/skill-characters/` → `rules/skills/skill-personas/`
- `rules/skills/skill-bands/` → `rules/skills/skill-groups/`
- `rules/ruleset-core/` → `rules/bandori-std/`

Crates: `rules/bandori-std`, `rules/status`, `rules/personas`, `rules/markers`,
`rules/cond-ir`, `rules/spec`, `rules/kani`, plus the per-band card crates
under `rules/cards/` and the skill crates under `rules/skills/`.

Ignored-test run: `node tools/build-ruleset.mjs` (no `CARGO_TARGET_DIR`) then
`cargo test -p ruleset-tests -- --ignored` and
`cargo test -p game-rules --no-fail-fast -- --ignored` (run twice).

**Class**
- **A** — fixable now from unambiguous rulebook text + current ABI
- **B** — needs a new/changed ABI capability (named in our vocabulary)
- **C** — rulebook text ambiguous; user ruling needed

**Severity**: high (common play) | medium | low (rare edge)

Sort: severity, then class. One item = one root cause; every file:line / test is listed.

---

# 2026-10-27 card sheet review intake

Full review of all 183 cards on `新卡组卡` (gid 648135790) against their
implementations and comments; the live sheet was verified identical to
`docs/rulebook/cards-sheet.csv` the same day. Report:
`docs/rulebook/CARD-REVIEW-2026-10-27.md`; machine-readable rows:
`card-review-2026-10-27.{json,csv}`.

- 1733 verified findings: 125 high / 587 medium / 1021 low.
- 180 of 183 cards affected; clean: `PP:有你与我在这里共度`, `R:轨迹`, `通用:GREAT`.
- All high are unrecorded. Top cards by volume: `HHW:（花音）Wacha Mocha
  啪嗒进行曲` (26), `Mujica:#J11` (23), `RAS:（和奏瑞依）寄于指尖的执念` (22),
  `Sumimi:（真奈）歌唱大赛5连冠` (21).
- Dominant roots: dead hooks because a play body never places the card on the
  field; missing owner gates on `TurnEnd`/`PassTile` hooks; `ctx::roll` ignoring
  `turn.extreme` (fixed, v103 path); card-effect spends wrongly offering
  skill-cost substitution; `PaySplitMul`/`DESIGNATES` props absent.
- Comment work: 241 missing call-site citations, 259 missing `TODO(规则书)`,
  362 misleading comments, 52 wrong quotes, 42 stale TODOs.

**Triage result (2026-10-28).** Every one of the 1733 rows was read against its
cell and classified, then merged by root cause; no row is unclassified. Final:
**776 A items, fixed on the triage branch** (their item list was removed from
this file on 2026-10-28); **65 B items** under 15 ABI capability groups with a
review design each in `docs/rulebook/spec/abi/`; **50 C items**, mapped to the
**88 numbered questions** at the end of this file (C-Q1–C-Q88; 12 items reuse an
existing question); **1 D item** dropped. Items carry their sheet rows; a row
merged into two items is listed under both.

B items are grouped by the capability that closes them — the groups are the
unit of approval, not the individual card. C items change no code until ruled;
their sites carry a `TODO(规则书)` pointing at the C-Q id.

# B — needs new ABI (65 items, 242 rows, 15 capability groups)

One design per group, for review before implementation. The designs are the
unit of approval; a card is fixed only after its group lands. Group files live
in `docs/rulebook/spec/abi/`.

## G01 — Instance identity and per-instance store

Design: `docs/rulebook/spec/abi/G01-instance-identity-and-state.md`

- **skill-fired-signal** (high, rows 152,184,232,445,1091,1243,1244,1481) — No per-resolution skill-fire signal, no guest emit, and X/skill-instance scope; `rules/cards/card-morfonica/src/rui_devil.rs`
- **trigger-instance-identity** (medium, rows 252,283,285,1029,1039) — Trigger/marker identity and per-instance state are not addressable (uid, marker tile, card.prop, dest-by-uid); `rules/cards/card-morfonica/src/starry_night.rs`
- **counter-cap-uid** (medium, rows 301,342,1056) — No per-uid counter cap read or clamp beyond one call; `rules/cards/card-mujica/src/sparkler.rs`

## G02 — Pay pipeline payload, stages and raw-money windows

Design: `docs/rulebook/spec/abi/G02-pay-pipeline.md`

- **pay-payload-kind-tile** (high, rows 226,230,305,306,309,323,953,959,1005,1031,1241,1353,1427) — Card-driven payments cannot carry kind, must-buy, tile, settle tile or a fixed no-modifier tranche; `rules/cards/card-morfonica/src/rui_devil.rs`
- **pay-stage-presplit** (medium, rows 21,1397,1539) — No interactive pre-stage before PaySplitAdd and no split-leg/total flag on the trigger; `rules/cards/card-crychic/src/vocal_too_hard.rs`
- **raw-money-window** (medium, rows 572,1333) — Raw money paths (lose_raw, redeem, auction) open no pay window; `rules/cards/card-morfonica/src/tritone.rs`
- **charge-paying-source** (high, rows 458,459,1360) — ChargePaying cannot tell a skill spend and only sees the pool share; `rules/cards/card-mygo/src/tomori_no_longer.rs`
- **fire-bird-paid-signal** (medium, rows 1441) — A discounted payment misreads as a shortfall; `rules/cards/card-roselia/src/fire_bird.rs`
- **rent-figure** (medium, rows 513,1388) — The link surcharge uses rent_of and ignores mortgage/dice/rent-factor partners; `rules/cards/card-mygo/src/anon_tokyo.rs`
- **money-counters** (medium, rows 1009,1472) — gains_this_turn misses raw credits; paid_in_settle is world-wide; `rules/cards/card-hhw/src/hagumi_marks.rs`

## G03 — Roll pipeline stages and prompt option actions

Design: `docs/rulebook/spec/abi/G03-roll-and-prompts.md`

- **roll-stage-effect-rolls** (high, rows 62,179,596,598,1053,1224,1367,1479) — Effect rolls and non-turn rollers skip the value/after stages; no per-roller move dice; `rules/cards/card-hhw/src/sports_talent.rs`
- **roll-history-per-player** (low, rows 97,1159,1160) — turn_rolls is turn-wide, not per roller; `rules/cards/card-crychic/src/taki_even_if.rs`
- **reroll-post-pick-signal** (high, rows 150,242,249,892,893,1002,1169,1187,1262,1379) — No reroll/post-pick signal; rerolled faces skip DiceRollValue and armed boosts/crystal checks; `rules/cards/card-morfonica/src/starry_night.rs`
- **please-choose-option1** (medium, rows 750) — please_choose option (1) never plays a card; `rules/cards/card-ras/src/please_choose.rs`
- **own-stage-answer** (high, rows 671,672,673,674,1455) — own_stage's resolveOverride/stageEnd answers are never consumed; `rules/cards/card-roselia/src/own_stage.rs`

## G04 — Move plan, walk sync and forced moves

Design: `docs/rulebook/spec/abi/G04-move-plan.md`

- **want-to-grab-board-axis** (medium, rows 107,1003) — Clause (3) nearest-orthogonal teleport needs a board-coordinate axis query; `rules/cards/card-crychic/src/want_to_grab.rs`
- **plan-shape-scope** (high, rows 131,132,288,290,338,339,340,560,835,963,964,1038,1064,1095,1361,1483) — Turn plan is one sticky struct: no reset, no pay_factor getter/multiplier, shaped moves leak; `rules/cards/card-morfonica/src/pure_wings.rs`
- **charity-rolled-flag** (medium, rows 209,1215,1216) — charity_show: move.main proxies 「掷骰移动」; `rules/cards/card-hhw/src/charity_show.rs`
- **plan-live-sync** (high, rows 203,973,1025,1182) — Plan writes after a window and next_steps ordering never reach the running walk; `rules/cards/card-hhw/src/charity_show.rs`
- **effect-move-forced-path** (high, rows 129,200,291,293,295,431,1183,1184,1286) — Effect moves take the main-move path: no forced flag, no walk void, followers dropped; `rules/cards/card-crychic/src/sakiko_lead.rs`
- **trigger-walk-payload** (high, rows 292,294,298,954,1006,1007,1101) — diceRollResult carries no roll_source, planned start, parity or effective walk data; `rules/cards/card-ag/src/detour.rs`
- **nested-teleport-tail** (medium, rows 693) — A skill teleport nested in a settle raises no MoveAfter; `rules/cards/card-roselia/src/lisa_bond.rs`

## G05 — Press surface for placed cards

Design: `docs/rulebook/spec/abi/G05-placed-card-press.md`

- **press-surface-placed-card** (high, rows 380,487,488,490,955,967,974,1521,1555) — Placed non-skill cards and field actions are on no press surface; `rules/cards/card-hhw/src/kanon_march.rs`

## G06 — Counteraction, join and negation windows

Design: `docs/rulebook/spec/abi/G06-counteraction-and-join.md`

- **join-window-report** (medium, rows 844,845,942,1090,1094,1312,1510) — No nested per-seat join window or joiner report; chosen designations unreadable; `rules/cards/card-general/src/net_error.rs`
- **extreme-range-event-counter** (medium, rows 158,980,982,990) — Extreme counter admits every card play and has no event-chain scoping; `rules/cards/card-ag/src/tsugumi_can.rs`
- **counter-link-move-face** (medium, rows 838) — A counter link loses the move face, so repaint cannot answer a chain; `rules/cards/card-ras/src/repaint.rs`
- **counteraction-history** (high, rows 426,1060) — No per-turn counteraction history (who countered whom); `rules/cards/card-mujica/src/cant_look_away.rs`
- **window-timing-negation** (medium, rows 395,936,940,1527,1643) — The effect window opens before the abnormal lands or is refused; guards see no pending negation; `rules/cards/card-mygo/src/ordinary.rs`
- **mortis-copy-links** (medium, rows 312,313,315,1045) — mortis copies nothing; an empty counteraction link list is never offered; `rules/cards/card-mujica/src/mortis_instinct.rs`

## G07 — Targeting pipeline and distance

Design: `docs/rulebook/spec/abi/G07-targeting-and-distance.md`

- **target-count-pipeline** (medium, rows 274,1037,1061,1294) — targeted_count reset timing, uncounted tile targetings, self-target short-circuit; `rules/cards/card-morfonica/src/centrifugal.rs`
- **lisa-distance-exemption** (medium, rows 691,1451) — No mechanism grants the 距离最近 exemption; `rules/cards/card-roselia/src/lisa_bond.rs`

## G08 — Zone and attachment lifecycle

Design: `docs/rulebook/spec/abi/G08-zone-lifecycle.md`

- **nyamu-face-flip-trigger** (high, rows 347,348,1043,1272) — No face-flip trigger; both nyamu flip clauses are dead; `rules/cards/card-mujica/src/nyamu_card.rs`
- **one-of-us-rehome** (medium, rows 1107) — ONE OF US: the card instance never reaches the survivor's field; `rules/cards/card-ag/src/one_of_us.rs`
- **one-of-us-cause** (low, rows 1124) — ONE OF US: forfeit also runs the bankruptcy hand-over; `rules/cards/card-ag/src/one_of_us.rs`
- **same-sky-mulligan-order** (low, rows 32) — SAME SKY: the copy is swept back only after the mulligan window; `rules/cards/card-ag/src/same_sky.rs`
- **cardplayed-payload** (medium, rows 34,677,680) — cardPlayed lacks a cancelled flag and payload; hand counters raise no cardPlayed; `rules/cards/card-ag/src/same_sky.rs`
- **hand-changed-event** (high, rows 57,58,197,642,1688) — No add-to-hand/hand-changed event for tutors, bounces and hand counters; `rules/cards/card-crychic/src/fate_together.rs`
- **wasted-flag** (medium, rows 510,522,523,1396,1701) — No 未生效/wasted flag on Discarded; `rules/cards/card-mygo/src/miracle.rs`
- **card-on-card** (high, rows 1075) — Back-face-up placement on a card is not modelled; `rules/cards/card-sumimi/src/here_the_world.rs`
- **here-the-world-leave-hook** (high, rows 639,645,867,875,1074,1252,1474,1488,1514,1515,1689,1695) — No hook runs when a placed card leaves the field (generic); `rules/cards/card-morfonica/src/summer_camp.rs`
- **tomori-earliest-owned** (medium, rows 188,994) — Placement uses the lowest board index, not the first-acquired deed; `rules/cards/card-crychic/src/tomori_inner_shout.rs`

## G09 — Tile, build, pass and house-removal hooks

Design: `docs/rulebook/spec/abi/G09-tile-build-pass.md`

- **marina-suppression-window** (medium, rows 880,885,1493) — No pass-time "affected" window and no reader for the CiRCLE suppression tag; `rules/cards/card-general/src/marina_work.rs`
- **parade-tile-swap** (medium, rows 1089) — smile_parade (3): the tile swap covers the settle only, not [经过]; `rules/cards/card-hhw/src/smile_parade.rs`
- **build-pipeline-queries** (medium, rows 118,123,124,521,748,1206) — Card-driven builds bypass buildability, effective price and build hooks; `rules/cards/card-hhw/src/smile_patrol.rs`
- **tile-write-effect-declare** (medium, rows 36,38,989) — Tile-only writes declare no Effect, so 「或你拥有的格子」 is unreachable; `rules/cards/card-ag/src/declare_war.rs`
- **guerrilla-pass-path** (medium, rows 866) — guerrilla: the window never checks a passed opponent Live House; `rules/cards/card-ras/src/guerrilla.rs`
- **house-removed-kind** (medium, rows 819,826) — No HouseRemoved kind: a razed max tile leaves the view and the half stale; `rules/cards/card-roselia/src/ringing_bloom.rs`

## G10 — Effect outcome, cause and settle facts

Design: `docs/rulebook/spec/abi/G10-effect-outcome.md`

- **event-defer** (medium, rows 567,568) — No IEventDefer and eventAfter has no cause; `rules/cards/card-ppp/src/arisa_wait.rs`
- **settle-outcome-flags** (medium, rows 792,1428,1449) — A settle cannot report body-replaced/paid/cancelled outcome; `rules/cards/card-ppp/src/tomorrows_door.rs`
- **host01-journal** (medium, rows 724,728) — Nested-settle writes are invisible to the master re-run (HOST-01); `rules/cards/card-ras/src/be_strongest.rs`
- **redirect-latch** (medium, rows 781,1465) — 「造成影响」 latches on any payee-equals-owner effect, not a landed redirect; `rules/cards/card-ras/src/exist.rs`
- **world-snapshot** (medium, rows 782,1077) — No play-time world snapshot/restore for no_breakup; `rules/cards/card-sumimi/src/no_breakup.rs`

## G11 — Information and deck inspection

Design: `docs/rulebook/spec/abi/G11-information-and-deck.md`

- **backstage-peek-refill** (medium, rows 1018) — backstage_tour: a peek empties the deck and the first flip triggers the refill; `rules/cards/card-hhw/src/backstage_tour.rs`
- **reveal-deck-inspection** (medium, rows 165,170,171,537,612,615,1092,1400) — No public reveal output or deck-inspection hook; piles only log counts; `rules/cards/card-hhw/src/believe_you.rs`
- **drew-batch** (medium, rows 1401,1480) — Drew fires per card; the sheet keys on a draw action; `rules/cards/card-pp/src/infinite_possibility.rs`

## G12 — Status, blocked and immunity gates

Design: `docs/rulebook/spec/abi/G12-status-and-immunity.md`

- **move-gate-replay-read** (high, rows 225,988,1498) — Tsugu refuses the whole play on a spent main move; no replay-stable moved read; `rules/cards/card-general/src/tsugu_ycm.rs`
- **unstoppable-blocked-read** (medium, rows 403,404,1035,1454) — Engine gate never reads [不可阻挡] and own_stage's turn-end branch keys on abnormal kind, not blocked-ness; `rules/cards/card-morfonica/src/tsukushi_garden.rs`
- **major-abnormal-scope** (medium, rows 1098,1171) — Abnormal check for want_human is a turn-wide diff, not move-scoped; `rules/cards/card-crychic/src/want_human.rs`
- **immunity-causer** (medium, rows 608,1412) — Card immunity is causer-blind; an owner-exempt flavour is unavailable; `rules/cards/card-pp/src/shine_again.rs`
- **dice-immunity** (medium, rows 710,1462) — [特] immunity for dice-adding cards is unimplemented; `rules/cards/card-ras/src/crush_drum.rs`

## G13 — Skill invoke and copy context

Design: `docs/rulebook/spec/abi/G13-invoke-and-copy.md`

- **invoke-skill-entry** (medium, rows 437,996) — invoke_skill runs On::Play only and loses the skill instance; `rules/cards/card-morfonica/src/nanami_effort.rs`
- **tomori-borrowed-second-half** (medium, rows 187,995) — Borrowed (2): the roll/停留/delayed settle half never runs; `rules/cards/card-crychic/src/tomori_inner_shout.rs`

## G14 — CardDef AI hooks and hand limit

Design: `docs/rulebook/spec/abi/G14-ai-and-hand-limit.md`

- **hand-limit-lift** (medium, rows 441) — A guest state increase does not reach the host hand-limit read; draw trims; `rules/cards/card-morfonica/src/nanami_effort.rs`
- **aiplay-hook** (low, rows 1253) — No CardDef AiPlay hook; bots offer a never-play card; `rules/cards/card-morfonica/src/summer_camp.rs`

## G15 — Kani verification host support

Design: `docs/rulebook/spec/abi/G15-kani-host.md`

- **kani-owned-livehouse** (low, rows 623) — blue_rose proofs are vacuous: the Kani host cannot own a buyable tile; `rules/kani/src/spec/r.rs`

---

# Convention markers (not gaps — do not count as open)

`rules/{events,tiles,markers}/src/lib.rs`, `rules/skills/{skill-personas,
skill-groups}/src/lib.rs`, `crates/game-core/src/data.rs` (`event_rule_id` doc),
`crates/card-sdk/src/ctx.rs:576` (cross-ref to the ruling-6 note). These name the
`TODO(规则书)` policy: a gap in the text is tagged, never silently dropped.

# card-std-split leftovers (2026-10-12)

Deferred from the `card-std-split` / ABI v56 pass (bandori-std + `tile_is`/
`tile_attr`/`tile_in`). Not merge blockers once drift is green.

- **CD-02 `crystals_changed_guard` residuals.** `bandori_std::crystals_changed_of`
  is the shared base (`bandori-std/src/lib.rs:370-375`); per-card copies still
  carry local residuals (value vs 0, `is_placed`, …) — e.g.
  `rules/cards/card-mujica/src/j11.rs:165-174`, `card-roselia/src/fire_bird.rs:25`.
  Fold the identical prefix onto the helper.
- **CD-04 game-named wire variants.** **Restated** (2026-10-17 audit): the
  `HookKind` / `GateKind` types were retired at ABI v85 in favour of
  `ActionKind` / `FilterKind`, and the game-named kinds left `card-sdk` for
  `rules_bandori-std::kind` (v70) as opaque ruleset-declared ids. Wire
  renames since: `CircleAffected` → `PassRewardAffected`, `firePaying` →
  `chargePaying` / `ChargePaying` (78), `FlipEffect`/`FlipAfter` retired (v83)
  → `FansFlip` (107). `CpChanged` / `CrystalsChanged` are no longer SDK wire
  names. No further rename planned; document ownership in `bandori-std`.
- **CD-05 native arena trap leak.** `native::enter`'s frame stack has a depth-64
  safety valve if `Drop` is skipped on a non-unwinding trap. Fine for
  `C-unwind`; revisit if a non-unwinding abort path appears.

# HIGH

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
  - `rules/cards/card-mujica/src/mortis_instinct.rs:37` (CanCounteract delegates to copy), `:51` (copy walks `H._playHistory`)
  - `rules/cards/card-sumimi/src/mana_champion.rs:66-69` (nested `H.Counteract(copy, p)`, shield + counteractor's draw)
  - `rules/cards/card-mujica/src/dice_cast.rs:29-36` (「（此卡可以被反击）」 needs the reverse-order counter play)
- **rulebook**: Mortis 「此卡打出时效果为场上其他玩家打出的上一张卡…」 + 「（此卡复制[反击]卡时可在符合条件时打出）」; Mana Champion 「此时场上其他玩家可如同自身的对应目标被指定一般打出[反击]卡，且其反击卡中针对打出玩家自身的效果改为你…」
- **current**: join retarget works (g19). No play-history query; full nested PlayCtx window still TODO.
- **expected**: copy the latest eligible play and nest a full Play/Counteract run with forwarded Tags/Extreme/Doubled.
- **capability**: `play-history query` + `nested PlayCtx` (cross-card identity + counteraction mode).

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
  - `rules/cards/card-roselia/src/fire_bird.rs:50`
  - `rules/cards/card-hhw/src/kokoro_circle.rs:49`
  - `rules/cards/card-morfonica/src/starry_night.rs:53`
  - `rules/cards/card-morfonica/src/noble_blue.rs:32`
  - related: `rules/cards/card-hhw/src/believe_you.rs` wasted flag
- **rulebook**: 「视为此卡未生效」 (haneoka / fire_bird short-pay / starry_night / noble_blue / kokoro_circle / no_road / miracle (3)).
- **current**: clause names a state with no observer. `PlayCtx.Effective = false` is the C# side channel and is not being ported (routine should *return* whether it took effect).
- **expected**: rule (a) does the card get spent / discarded (haneoka says yes; noble_blue/starry_night "spent anyway" implies no), (b) what counts as a "use" this suppresses (later 「抵消一次任意付款」 etc.).
- **question**: C-Q5.
- **capability (after the ruling)**: routine return value for took-effect + `CardWasted` (C# `H.CardWasted`).

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

## HOST-01 — HostRequest B2 equivalence gap (learn pass vs live+host_effects)
- **kind**: ABI defect / implementation bug
- **class**: B
- **severity**: high (breaks inline guest equivalence under real rules)
- **suggested batch**: host ABI: out-buffers / HostRequest handlers
- **locations**
  - `rules/tests/tests/inline_provider.rs:247` (B2 equivalence gap TODO), `:343` (cross-ref)
  - `docs/BOT.md` §3.2
- **rulebook**: n/a — ABI contract: inline learn pass must see the same world as the replay model's last pass (`live + host_effects`).
- **current**: PARTIAL (top-level done; nested-settle remainder). Each pass shares `live` after `NeedHost`, so the **landing** pass reads `live + host_effects`; write-through replace-on-rerun means the event stream carries that pass's lines (`docs/ENGINE.md` "Drive replay semantics"). Pinned for the top-level `pay` path by `effect_applied_log::reread_after_host_effect_sees_the_updated_value`. **Remaining gap:** a write inside a host routine's *nested settle* (the agent-tile offer buying a deed during `HostRequest::Move`) may not be on live before the next pass shares -- `RAS:成为最强`'s ownership check still sees the pre-buy deed (`be_strongest_always_rolls_and_ignores_the_face_on_the_fallback`).
- **expected**: rebase onto the updated live world between passes, including nested settle writes (the write journal this item names).
- **capability**: write journal on `Run` for nested settle writes. Top-level HostRequest sequencing is in.

## HOST-02 — guest world writes are not visible to the host routine that follows
- **kind**: ABI defect / implementation bug
- **class**: B
- **severity**: medium (silently no-ops a routine whose guard reads board state)
- **suggested batch**: host ABI: `apply_host_request` world sync
- **locations**
  - `crates/game-rules/src/wasm_rules.rs:3597` (`adopt_turn_policy` / `overlay_guest_state` at the `NeedHost` boundary)
  - `crates/game-core/src/engine/play.rs:3219` (`apply_redeem` owner guard)
  - `rules/cards/card-morfonica/src/toko_swap.rs` (worked around 2026-10-28)
  - `rules/cards/card-pp/src/first_live_accident.rs` (seal token vs the body's own draw hook, 2026-10-28)
- **rulebook**: n/a — ABI contract. A body's writes before a host request must be what the routine — and any hook the routine raises — sees.
- **current**: Only the turn **policy** knobs, the plan knobs (`resolve` / `pay_factor` / …) and `players[].state` **decreases** cross at `NeedHost`; `owners` / `mortgaged` / `houses` / `field` / token writes stay on the guest copy until the re-run. `Mor:（toko）`'s 「互换后将你得到的地契免费赎回」 via `ctx::apply_redeem` therefore returned 0 every time (the guard read the *old* owner) while the money pipeline moved; the branch now writes `set_mortgaged` + the `redeemed` record by hand. The same staleness hits hook bodies: 「初次演出事故」 sets its `skillBlock` token and then draws, but the `DrewBefore` skill sees the pre-body world, so 大和麻弥's sealed (2) still offers on that one draw.
- **expected**: rebase the live world on the guest copy before `apply_host_request` (or overlay board rows for the routine's duration), so a routine's guards see the body's preceding writes. Then the toko workaround can go back to `apply_redeem` (the `log.redeem` / `redeemed` record then come for free).
- **capability**: board-row overlay at the pause boundary; needs no new import.

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
  - `rules/cards/card-mujica/src/cant_look_away.rs` (`run`, `TODO(规则书)`)
- **card**: `Mujica:无法将视线移开`
- **rulebook clause**: 「使当前回合内对你打出过[反击]的所有玩家向你选择的方向强制移动1~4以内的任意步数并[触发结算]」
- **missing**: a per-turn counteraction history (who countered whom) the
  engine lacks. `ctx::turn_key()` names the turn but cannot rebuild the list.
  The body today moves only the immediate counterer
  (`trigger::player_id()` of the answered link) — less than the book.
- **fix needs**: SDK query for the per-turn counteraction history (declarer ×
  countered seat) + cond vocab to filter it + append at the counter-declaration
  loop (`crates/game-rules/src/wasm_rules.rs` ~5480, where `link.target` = the
  countered seat and `actor`/`player_id` = the declarer) + ABI bump.
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
- **status**: **partially fixed** (2026-10-17 audit) — of six sub-capabilities,
  only **(b) host clock write has landed**: `bangdream_chan.rs:42-45` writes
  `prop::BONUS_TIME_SET` (免费时间) and `prop::BASE_TIME_REFILL_SET` (恢复时间),
  documented at `crates/card-sdk/src/abi.rs:1086-1108`. Still open: (a) `IEventDefer`,
  (c) `CantPlayHand` gate, (d) auction primitive, (e) free-amount `ask_money`,
  (f) cosmetic ask.
- **kind**: ABI defect
- **class**: B
- **severity**: medium
- **suggested batch**: HostRequest handlers
- **locations**
  - (a) `rules/cards/card-ppp/src/arisa_wait.rs:46` (defer event to owner's turn start + extra 流星堂 draw) — open
  - (b) `rules/events/src/bangdream_chan.rs:42` / `rules/tests/tests/events.rs` `bangdream_chan_shortens_the_clocks` — **fixed**
  - (c) `rules/events/src/a_a_o.rs:29` (`CannotPlay` / `Fx.CantPlayHand` no engine field) — open
  - (d) `rules/events/src/tsurumaki_estate.rs:37` (「竞价」 auction primitive) — open
  - (e) `rules/events/src/tsurumaki_estate.rs:53` (free-amount `ask_money`) — open (also HOST-05)
  - (f) `rules/events/src/ex_quest.rs:36` (skin-change ask) — open
- **capability**: `IEventDefer` (Defer/Resolved), ~~host clock write~~, `CantPlayHand` gate, auction primitive, `ask_money`, cosmetic ask.

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
  - `crates/card-sdk/src/abi.rs` property map (v30)
- **capability**: tri-state tile prop (absent / clear / set) for mortgage-stays.

## ABI-13 — temporary tile-rule swap (「效果迁移至…」 / 「视为与…交换位置」)
- **kind**: ABI defect (missing capability)
- **class**: B
- **severity**: medium
- **suggested batch**: host ABI: tile rules
- **locations**
  - `rules/events/src/circle_rebuild.rs` (`TODO(ABI) ABI-13` on `on_settle_body`
    and the cafe branch of `on_pass`) — the live consumer
  - `rules/cards/card-hhw/src/smile_parade.rs` (「那格视为与"弦卷集团"格子交换位置」)
    -- the same shape, currently hand-rolled as `card_settle_at` +
    `set_settle_as_agent` + `is_settle_redirect`
  - `rules/tiles/src/lib.rs` module doc already promises the model: *"a card
    that bends a tile attaches, **swaps** or retunes rule instances"*. Only
    attach (`place_card_on`) and retune (`set_tile_prop`) exist.
- **rulebook**: 「CiRCLE原本的所有效果迁移至CiRCLE咖啡厅并覆盖其原本效果」
  (协助CiRCLE重建); 「那格视为与"弦卷集团"格子交换位置」 (笑容大游行 (3)).
- **current**: a card re-homes another tile's effects one clause at a time.
  `event:协助CiRCLE重建` hand-suppresses the [经过] reward (`prop::NO_REWARD`),
  re-implements the reward on the cafe by messaging `tile:circle`
  (`"settleReward"`, landed), and replaces the two settle bodies by hook. Any
  effect the migrating tile gains later, and any instance state bound to it,
  does **not** ride along.
- **capability**: a guest verb that rebinds which tile **rule instance**
  governs a board square, temporarily, e.g. `ctx::tile_swap_rules(a, b)` /
  `ctx::tile_set_rule(tile, rule_id)`, applied immediately (it must predate
  every hook that changes settlement) and reversed when the swapping instance
  expires.
- **must keep (the charge-replenish constraint)**: the square's **identity**
  is unchanged. `tile_is(tile.id, "circle")` / `ctx::is_start` /
  `guards::OWN_START` keep reading the static board, so 「每次[经过]CiRCLE时
  获得N个[火罐]」 (`skill-personas`: 千早爱音 / 白金燐子 / 冰川纱夜 / 慈爱女神 …)
  still fires on the real CiRCLE and does **not** ride the migration to
  「CiRCLE咖啡厅」. Those are their own cards' effects keyed *on* CiRCLE, not
  CiRCLE's own effects, and the rulebook migrates only the latter.
  Test: `circle_rebuild_leaves_charge_replenish_on_the_real_circle`.
- **also keep**: tile text composition (`docs/TILES.md` 「Tile text」) resolves
  a square's base text by `tile_rule_id(kind)` + `FieldCard::tile`; a swap has
  to leave the composed text right (a replacement instance's `texts` should
  win the slots it claims, as today). `prop::START` is stamped on `tile:circle`
  and is a property of the **square**, not of the migrating effects -- it must
  not travel with a swapped rule.
- **save/replay**: a world write, not a state-shape change, so
  `Match::save()` and the archived engine bundles stay valid
  (`docs/rulebook/BACKLOG.md` replay-compat rule).
- **unblocks**: a generic 「效果迁移至…」 / 「视为与…交换位置」 clause; 笑容大游行
  would drop its hand-rolled settle-redirect pair.

## SKILL-04 — 状态2 exit conditions missing (several character skills)
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: skills: Mujica
- **locations**
  - `rules/skills/skill-personas/src/kaede_support.rs:284` (「直到状态2结束为止」 with no clause that ends it)
  - `rules/skills/skill-personas/src/mumei_streamer.rs:131` (no exit; 「退出状态2后，被你翻面的卡自动翻回」 has no moment)
  - `rules/skills/skill-personas/src/mutsumi_actor.rs:89` (状态2 character swap: 「上一位打出过手牌的玩家」 latch + 「角色卡堆」)
  - contrast: 三角初华 fire-pot cap, 若叶睦 hand-size exit (those are stated)
- **question**: C-Q11.

## SKILL-05 — CRYCHIC band (3) cash-out skipped on nested `transform_now`
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: skills: Roselia / CRYCHIC
- **locations**
  - `rules/skills/skill-groups/src/crychic.rs:79` (「当此卡移除时」 should fire on the (2)-driven 「移除此卡」; nested run has no running instance so `crystals()` reads 0)
  - `rules/cards/card-crychic/src/mutsumi_never.rs:32` (C# narrows (2) to `BandCrychic` specifically — wider 「乐队技能」 reading taken)
  - `rules/skills/skill-groups/src/crychic.rs:155` (「无法从手中打出任何牌」 global play gate)
  - tests: `card_paths::mutsumi_never_2_*`
- **question**: C-Q12 (does (3) cash out on the (2)-driven removal? is (2) limited to BandCrychic?).

## SKILL-06 — deck-construction / hand-building rules with no vocabulary
- **kind**: ABI defect / rulebook divergence
- **class**: B
- **severity**: medium
- **suggested batch**: skills: CiRCLE / sumimi
- **locations**
  - `rules/skills/skill-groups/src/circle_staff.rs:77,81` (「只能在卡组中加入"通用"卡」; 「（除将牌加入卡组的效果以外）」 carve-out)
  - `rules/skills/skill-groups/src/sumimi.rs:81` (「你可携带两个角色的专属卡牌」)
  - `rules/skills/skill-groups/src/poppin.rs:315` (「所有Poppin' Party角色视为同时拥有"星之鼓动山丘"」 co-ownership + buy lock)
  - `rules/skills/skill-personas/src/lisa_goddess.rs:124` (temporary fire-pot provenance / 「被冲榜类效果被动消耗无需支付」)
  - `rules/skills/skill-personas/src/mutsumi_crychic.rs:125`, `saki_crychic.rs:131` (「只有洗牌时可以…添加奇迹水晶」 untagged carve-out)
  - `rules/skills/skill-personas/src/yuri_crit.rs:95` (「复制者」 / 「（抵押或赎回以外）」)
  - `rules/skills/skill-personas/src/tae_police.rs:100` (「[主]效果」「[手]效果」 tag query)
  - `rules/skills/skill-personas/src/soyo_clear.rs:114` (「以此法免除」 attribution)
- **capability**: deck-list / character-exclusive tag / co-owned tile / provenance-tagged spends / tag query on triggers.

## TILE-06 — 江户川乐器店 / CiRCLE shared body
- **kind**: rulebook divergence
- **class**: C
- **severity**: low→medium
- **suggested batch**: tiles
- **locations**
  - `rules/tiles/src/edogawa.rs:9` (book names both in one sentence with different extra text)
  - `rules/tiles/src/circle.rs:29,59,129` (stunned card-only reward is a ruling `C44`/`D13`, not book; exile bar is engine bookkeeping)
- **question**: C-Q16 (only if the two shops ever diverge).

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

## MOVE-02 residual — does a 「固定为X*6」 face still take later additions?
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- **locations**: `rules/cards/card-ag/src/yolo.rs:49` (`TODO(规则书)`, points here);
  `rules/tests/tests/cross_move.rs:551` `m11_rinko_plus_yolo` (pins the add-anyway reading)
- **rulebook**: 白金燐子 (2) 「使这回合移动掷骰的结果固定为X*6」 + Y.O.L.O 「使结果增加1d4
  结果的数字」 — the text does not say whether a fixed face refuses the addition.
  Contrast `starry_night` / `eve_unify`, which refuse over a fixed face.

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
- **status**: open as a **ruling** only. The implementation reading is in place
  (2026-10-17 verified): the switch is a **no-op unless the target's `skillState`
  is 1 or 2** (`welcome_mujica.rs:58-66`, also `:105`) — a player who cannot
  reach 状态2 is not toggled. The older "Engine now: free toggle" note was stale.
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: skills: Mujica
- **locations**
  - `rules/cards/card-mujica/src/welcome_mujica.rs:58-66` (the gate)
- **question**: C-Q30.

## CH-09 — dice_cast 「（此卡可以被反击）」
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: counteraction windows
- **locations**
  - `rules/cards/card-mujica/src/dice_cast.rs:85`
- **related** (carried from the closed CH-03): the turn-key `PLACED` latch also backs `cant_play` (`x_in_play`, `:59-64`), so a second copy in hand is refused in the same turn although the cell prints no once-per-turn cap; the `noCounteractTurn` latch is still the other seats' hand-counter block (`:98`). Probe: a second copy fails `Err("cards:card-mujica.x_in_play")`.
- **question**: C-Q31 (must other [反击]s still answer this card's own play window?).

## CH-10 — ran_as_usual (1) C# once-per-turn gate
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- **suggested batch**: counteraction windows
- **locations**
  - `rules/cards/card-ag/src/ran_as_usual.rs:27` (C# adds a once-per-turn gate the passage does not name)
- **question**: C-Q32.

## CRYSTAL-02 — miracle (3) / crystal-on-discard 「视为此卡未生效」
- **kind**: ABI defect
- **class**: B (after CH-07 ruling)
- **severity**: medium
- **suggested batch**: crystal/marker accounting
- **locations**
  - `rules/cards/card-mygo/src/miracle.rs:138` (needs `PlayCtx.Effective` / `H.CardWasted`)
- **capability**: `CardWasted` when a card is discarded with crystals left.

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
- **kind**: implementation bug (rise half closed) + ABI defect (fall half)
- **class**: A (rise, **closed 2026-10-21** in `0ce220b1`) / **B** (fall + disarm)
- **severity**: medium
- **suggested batch**: rent multipliers / HHW cards
- **locations**
  - `rules/cards/card-roselia/src/ringing_bloom.rs` — `apply_view` recomputes the
    counted max and the half-charge `RENT_FACTOR` set; `on_house_added` rides
    `Action::HouseAdded` (engine kind 75) so the 「视为」 tracks a build past the
    play snapshot. Pinned by `rules/tests/tests/cross_tiles.rs`
    `t10b_ringing_bloom_tracks_a_later_build`.
  - **still open (class B)**: the **fall** path. There is no `HouseRemoved`
    kind, so a raze of the max tile (or a sale) leaves the view at the old max
    until the next build. A removal path other than `pay_after`'s discard also
    leaves the tile `RENT_FACTOR`s armed -- there is no general "instance left
    the field" hook to disarm on.
  - **still open (C-Q36)**: `pay_after`'s （3） `X` reads the **counted**
    (`rent_houses_of`) value, not the standing houses. The sheet says
    「房屋数」 without 「视为」; the reading taken is that (2) has already
    redefined what 「房屋数」 means for this player's tiles. See C-Q36.
- **rulebook**: (2) 「视为」 is continuous (tracks the max as houses move).
- **expected**: track rises (done) and falls (needs a `HouseRemoved` kind or an
  equivalent) for the life of the card.

## SKILL-10 — misaki_other 「额外视为CiRCLE」
- **kind**: rulebook divergence + ABI defect
- **class**: C then B
- **severity**: medium
- **suggested batch**: tiles / skills: HHW
- **locations**
  - `rules/skills/skill-personas/src/misaki_other.rs:107` (per-player tile-kind override; what 「额外视为CiRCLE」 pays is not stated)
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
- **status**: **half fixed** (2026-10-17 audit) — the song-table half is gone:
  `data/song_cards.json` is no longer in live `data/`, and `kizuna_music.rs`
  counts song cards via `prop::SONG` with no duplicated id table. Still open:
  `end_it_all.rs`'s `NON_DERIVED` list is a hand copy of `data/events.json`.
- **kind**: implementation debt
- **class**: A (keep in step) / B (ctx read)
- **severity**: low
- **suggested batch**: events
- **locations**
  - `rules/events/src/end_it_all.rs:14` (`data/events.json` not reachable from `ctx`)
- **expected**: `ctx` can read the data file, or the table is generated.

## HOST-02 — 时点流程 tables missing from `data/rules.txt` + missing timing kinds
- **status**: **partial** (2026-10-13, `closeout-5-engine`) — the four 时点流程
  tables (开始游戏 / 回合流程 / 行动阶段 / 支付阶段) are now in
  `data/rules.txt` verbatim from `docs/rulebook/rulebook.txt` (`data/rules.txt:109-177`).
  **行动阶段 11 (移动前) also landed** as `ActionKind::MoveBefore` (ABI v43).
  Still open: missing trigger kinds 行动阶段 3 (回合开始后) / 6 (经营、出牌阶段后)
  and 支付阶段 3 (「支付计算后」), plus 支付阶段 1 / 5 / 7 shaping (one `effect`
  raise carries window 5's power at window 1's position).
- **kind**: rulebook divergence / engine surface
- **class**: A (sync the tables) + B (timing kinds)
- **severity**: medium
- **suggested batch**: fuzz/determinism / settle stages
- **locations**
  - `data/rules.txt:109-177` 时点流程 (tables synced: 开始游戏 :111, 回合流程 :121, 行动阶段 :136, 支付阶段 :162)
  - missing trigger kinds: 行动阶段 3 / 6, 支付阶段 3 (「支付计算后」)
  - 支付阶段 1 / 5 / 7 shaping (one `effect` raise carries window 5's power at window 1's position)
- **expected**: sync the tables without rewording; raise the missing windows when a card hooks them.

## HOST-05 — `ask_tile(allowNone: true)` / free-amount `ask_money`
- **status**: **partial** (2026-10-13, `closeout-5-engine`) —
  `pareo_far` now uses `ask_tiles` (TileId, 「不选」 → `-1`) so the optional
  house-removal is one prompt. Free-amount `ask_money` is still missing;
  `tsurumaki_estate`'s 「竞价」 still steps 1000-at-a-time through `ask_pick`.
- **kind**: ABI defect
- **class**: B
- **severity**: low
- **suggested batch**: HostRequest handlers
- **locations**
  - `rules/cards/card-ras/src/pareo_far.rs` (ask_tiles 「不选」)
  - `rules/events/src/tsurumaki_estate.rs:53` (ask_money still open)

## LOW-01 — event shout / skin-change / auction leftovers
- Covered by ABI-09 / EVENT-03. Keep as one low batch: `lulu.rs:37` shout logged only; `ex_quest.rs:36` skin; `tsurumaki_estate.rs:37` sealed bid vs open auction.

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

# Newly filed 2026-10-17 (TODO sweep)

Found by sweeping the live **157** `TODO(规则书)` tags against this file. Each
had a code TODO but no backlog id. Not yet severity-sorted; take them in the
suggested batches. **Status-layer items (STA-*) are one root-cause family** —
the status crate's grant verbs do not yet enforce every glossary clause.

## STA-02 — no prompt surface for 「可选择受到的[传送]，[强制移动]，[强制停下]效果是否生效」
- **kind**: ABI defect
- **class**: B
- **severity**: medium
- **suggested batch**: host ABI: out-buffers
- **locations**: `rules/status/src/unstoppable.rs:18-19`
- **capability**: a per-effect opt-out prompt on the [不可阻挡] holder.

## STA-03 — `noHand` 2→1→0 dance is engine behaviour, not a quoted clause
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- **suggested batch**: skills: CiRCLE / sumimi
- **locations**: `rules/status/src/no_hand.rs:6-7`
- **rulebook**: 「手卡上限数量减1」 is card text; the 2→1→0 ladder is ours.
- **current (2026-10-18, `status-hosts-rest`)**: the dance **moved** from the
  inline `play.rs` steps into `status:no_hand`'s `TurnStart` / `TurnEnd`
  hooks. The `TODO(规则书)` stays (the ladder is still our reading of card
  text, not a glossary clause); the host now owns it.

## STA-05 — [除外] 「不能被[指定]」 / on-board clause still engine-side
- **kind**: ABI defect
- **class**: B
- **severity**: medium
- **suggested batch**: host ABI: out-buffers
- **locations**: `rules/status/src/exile.rs:47-49`
- **capability**: Untargetable / on-board gate kinds (today `MatchPlayer::status_gates`).

## AYA-POOL-01 — 「将Y个…变正」 pool-of-Y vs per-seat remaining-Y
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- **suggested batch**: skills: PP
- **locations**: `rules/skills/skill-personas/src/aya_with.rs:122-124`
- **current**: the slice. Per-seat remaining-Y would let overflow crystals fire.

## EVE-01 — `card_move` never raises `diceRollBefore`
- **kind**: implementation bug
- **class**: B
- **severity**: medium
- **suggested batch**: movement
- **locations**: `rules/skills/skill-personas/src/eve_unify.rs:38-43`
- **rulebook**: 「因任意原因投掷前」 also covers card-driven rolls.
- **current**: the `card_move` site (`play.rs` ~743) never raises `diceRollBefore`.

## HAG-01 — held-token writes have no post-commit `*Changed` event
- **kind**: ABI defect
- **class**: B
- **severity**: medium
- **suggested batch**: crystal/marker accounting
- **locations**: `rules/skills/skill-personas/src/hagumi_homerun.rs:220-223`, `:259-263`
- **current**: held tokens raise only pre-move `markerGain`; the overflow sweep
  is re-run only at this skill's own write sites.
- **capability**: post-commit `*Changed` for held tokens.

## NYAMU-01 — flip-to-back spend of 2 fire pots has no stated benefit
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- **suggested batch**: skills: Mujica
- **locations**: `rules/cards/card-mujica/src/nyamu_card.rs:57-62`
- **rulebook**: 「每当此卡将从正面翻至背面时可选择失去2个火罐」 — the clause
  does not say what losing the 2 fire pots buys (C# also silent).

## MARKERS-LISA-01 — temp-pot 600-cost and two-turn expiry under-specified
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: skills: Roselia / fire kernel
- **locations**: `rules/status/src/fire.rs` (`temp` module),
  `rules/skills/skill-personas/src/lisa_goddess.rs`
- **rulebook**: （3）「你提供的临时火罐将会在两回合后失去，使用你提供的临时火罐
  需要支付你600资金」 — the spend path has to know a pot's *provenance* to charge
  the provider, and 「被冲榜类效果被动消耗无需支付」 carves out a class of
  spends the engine does not tag. The counter itself is real
  (`fire::temp`, spendable temp-first per MARKERS.md §10(2)); the 600-cost and
  the two-turn expiry stay open. In-source `TODO(规则书)[judgement]`.

## MARKERS-MUTSU-01 — 状态2 character cover pile has no vocabulary
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: skills: Ave Mujica
- **locations**: `rules/skills/skill-personas/src/mutsumi_actor.rs`
- **rulebook**: 状态2's 「从角色卡堆拿取一张上一位打出过手牌的玩家的角色卡将其
  覆盖于自己角色卡上」 — 「角色卡堆」 is a pile the vocabulary has no read for.
  The fire half (「并获得其初始火罐」) landed as a registry overlay
  (`fire::overlay` / `end_overlay`) keyed on the last-play latch; the character
  cover stays open. In-source `TODO(规则书)[judgement]`.

## MARKERS-KANI-01 — fire kernel iff proofs not yet added
- **kind**: coverage gap
- **class**: A
- **severity**: low
- **suggested batch**: kani / fire
- **locations**: `rules/kani/src/proofs/` (missing `fire_kernel` harness)
- **rulebook**: MARKERS.md §9 wants Kani iff proofs on the kernel bodies
  (exact effect sets -- one grant/spend effect + one 效果适用 log + window; no
  money/pos side effects) and I2 as a Kani assert. Not yet written; the
  `registry_has_no_body_literals` / `i2_*` / `fire_regime_*` native pins in
  `rules/tests/tests/markers.rs` cover the same ground at the integration
  level.

## DETOUR-01 — 「"afterglow"式的移动」 under-specified; counter C# vs text
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: events / AG cards
- **locations**: `rules/cards/card-ag/src/detour.rs:55-58`, `:119-122`
- **rulebook**: 「初始Afterglow角色」 is never named; the counter logs 「羽泽鸫式」
  while implementing 美竹兰 reverse.

## TOMORI-BIND-01 — pull second half needs TomoriBind + delayed settle queue
- **kind**: ABI defect
- **class**: B
- **severity**: medium
- **suggested batch**: skills: CRYCHIC
- **locations**: `rules/cards/card-crychic/src/tomori_inner_shout.rs:140-143`
- **rulebook**: （3） second half — a 1d20 (≤6: `H.TomoriBind` + `TomoriLaterFx`
  delayed settles at half pay).
- **capability**: bind counter + delayed-settle-at-half-pay queue.

## NFO-CANCEL-01 — NFO (3) cannot tell a pre-`PaySplitAt` cancel from a shave-to-0
- **kind**: implementation bug
- **class**: B
- **severity**: medium
- **suggested batch**: payment pipeline
- **locations**: `rules/cards/card-roselia/src/nfo.rs:30-36`
- **current**: `PayAfter` has no "PaySplitAt ran" bit, so the card is unplaced
  even when the payment was cancelled before the shave.

## AYA-LONG-01 residual — the 最低0 floor's scope
- **kind**: rulebook gap
- **class**: C
- **severity**: low
- **locations**: `rules/cards/card-pp/src/aya_longing.rs` (`TODO(规则书)` in the body)
- **rulebook**: 「（最低0）」 — a local `.max(0)` per clause vs a final floor after
  the whole add/sub stage is still unnamed by the book.
- **pinned by**: `money.rs` `aya_longing_1500_shave_lands_after_sibling_pay_split_add`.

## LISA-BOND-01 — 「使用技能进行传送后」 names no stage number
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- **suggested batch**: movement / skills: Roselia
- **locations**: `rules/cards/card-roselia/src/lisa_bond.rs:21-23`
- **rulebook**: closest is stage 13 「传送后在终点触发重叠」.

## HARU-SOYO-FX — 春日影 rent-cancel half needs `H.ExtraOf<SoyoTeleportFx>`
- **kind**: ABI defect
- **class**: B
- **severity**: medium
- **suggested batch**: crystal/marker accounting
- **locations**: `rules/cards/card-crychic/src/haruhikage.rs:247-250`
- **capability**: `Fx.MoveBefore` hook + a remembered tile (see ABI-08 `ExtraOf`).

## DRAW-OUT-01 — 「受到抽卡效果」 includes self-caused out-of-turn draws?
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: draw/discard pipeline
- **locations**: `crates/game-rules/src/wasm_rules.rs:3944-3951`
- **current**: today yes. `drawOut` / 「受到抽卡效果」 may mean only draws
  another effect forced.

## FORCE-WRITE-01 — a hook that *does* write 「强制购买」 has no wire spelling
- **kind**: ABI defect
- **class**: B
- **severity**: medium
- **suggested batch**: payment pipeline
- **locations**: `crates/game-rules/src/wasm_rules.rs:8621-8630`
- **capability**: a wire target kind for 「强制购买」; generic `Buy*` cannot retune Force.

## LINGER-10 — 「本回合」 linger in an extra turn / another player's turn
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: settle stages
- **locations**: `crates/game-core/src/engine/play.rs:508-512`
- **current**: linger effects last exactly one turn (ruling 10).

## NOBUY-CARD-01 — `no_buy` gates only `BuyKind::Land`
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- **suggested batch**: payment pipeline
- **locations**: `crates/game-core/src/engine/play.rs:2599-2603`
- **current**: whether it also bars a card-driven (`BuyKind::Card`) buy is undecided (ruling 6).

## GUARD-PERVISIT — book states no per-visit counteract bound
- **kind**: rulebook gap
- **class**: C
- **severity**: low
- **suggested batch**: counteraction windows
- **locations**: `crates/game-rules/src/wasm_rules.rs:116-121` (`MAX_COUNTERACT_PER_VISIT = 16`)

## GUARD-SETTLE-DEPTH — book states no settle-depth bound
- **kind**: rulebook gap
- **class**: C
- **severity**: low
- **suggested batch**: engine: settle recursion / stack safety
- **locations**: `crates/game-core/src/engine/play.rs:41-46` (`MAX_SETTLE_DEPTH = 64`)
- **cross-link**: STACK-01 (the queue, not this rulebook bound, is the stack-safety mechanism).

## Missing backlog homes for live RULING tests
Two of the 11 remaining `#[ignore = "RULING: …"]` tests had no id here. Filed as C-Q44 / C-Q45 below.
- `rules/tests/tests/cross_tiles.rs:984` `t22_hill_coownership` → **C-Q44**
- `rules/tests/tests/gap_res.rs:251` `g29_maya_look_at_top_inside_the_draw_curve` → **C-Q45**

---

# Newly filed 2026-10-20 (card-rule TODO review)

Two items the 2026-10-17/18 waves referenced but never gave an id. Everything
else in the 153-tag sweep already had a home (the item sections of this file).

## DONUT-VOID-01 — 一人两个甜甜圈 has no clause for a barred acquisition
- **kind**: rulebook gap
- **class**: C
- **severity**: low
- **suggested batch**: crystal/marker accounting
- **locations**: `rules/cards/card-sumimi/src/two_donuts.rs:50-55`
- **rulebook**: 「获得[除外]直至你原本所在格子被其他玩家经过」 +
  glossary 「该效果生效时无法重复获得该效果」.
- **current**: when the presser is already [除外], the no-stack guard bars the
  (1) acquisition and the whole effect is treated as void — no carrier, no
  `SLOT_ORIG` / `SLOT_BY`, no log. The card is still spent (the play itself
  resolved).
- **question**: does a barred acquisition still spend the card and leave
  nothing, or does the card stay in hand / go to the graveyard without the
  exile? The book is silent. We chose "void + still spent" because the press
  itself has no [限] and the no-stack clause only names the *effect*.

## ABI-MARKER-SPENT — `fireSpent` wire kind should become `MarkerSpent`
- **kind**: ABI defect (rename)
- **class**: B
- **severity**: low
- **suggested batch**: crystal/marker accounting
- **locations**: `crates/game-rules/src/wasm_rules.rs` (`spend_charge` /
  `charge_spent_log` / the `fireSpent` raise), `docs/MARKERS.md` §6 row,
  `docs/STATUS-HOSTS.md:560`, card bodies subscribing `FireSpent`
- **current**: `charge_spent_log` is already name-generic
  (`marker_spent_log`), but the wire kind is still `fireSpent` (73) because
  card bodies subscribe `FireSpent`. Called a "separate ticket" by the Phase C
  batch 2 and status-vocab-zero notes and never filed.
- **capability**: rename the wire kind to `MarkerSpent { name }` (keep 73 as a
  source-compat alias for one ABI bump, then drop it), or declare `fireSpent`
  the permanent name of the fire counter's spend raise and close this as D.
- **cross-link**: MARKERS-LISA-01 (temp-pot spend provenance) — a
  `MarkerSpent` carrying `name` is the same wire shape that tag needs.

---

# Wave 3 clause check (2026-10-21)

Five large, previously unreviewed bodies reviewed clause-by-clause against
`docs/rulebook/cards.json` (the same method as the 2026-10-18 waves 1–2):
`card-morfonica/nanami_effort`, `card-hhw/kaoru_thief`, `card-ppp/tomorrows_door`,
`card-sumimi/here_the_world`, `card-ag/one_of_us`. Full report:
job tmp `wave3-review.md`. **7 new findings** (1 high A, 1 high C, 1 med A+C,
3 med B/C, 1 low A). Tomorrow's Door is clean (C-Q44 already filed).
**~150 cards remain unreviewed at clause level** — keep taking slices.

### W3-NANAMI-01 `Mor:（NNM）稍微努力了一下` — (3) press runs the skill button / clause （3）, not 「你的（2）技能」
- **class**: C (what a waived （2） press does) with a clear wrong-clause defect
- **severity**: high
- **rule text**: 「（3）将此卡放置在场上并放置x个奇迹水晶，你可以移除一个奇迹水晶视为发动你的（2）技能，此次技能不受数量或轮数限制」
- **code**: `rules/cards/card-morfonica/src/nanami_effort.rs:237-259` — `use_skill`
  spends 1 crystal then `ctx::play_card` on the first `skill:*` field card, which
  runs `skill:广町七深:这是很普通的事吧？`'s **`On::Play`** = that skill's clause
  **（3）** 「使用手中的角色标记，视为使用该角色的技能」
  (`rules/skills/skill-personas/src/nanami_ordinary.rs:48,162-187`).
- **gap**: The printed target is clause **（2）** 「当有玩家在使用角色技能时，你可以支付一个火罐得到一个该角色的标记（当场上存活玩家数量大于等于3时：每种角色标记上限1，该技能对被动技能存在3回合CD）」. 「不受数量或轮数限制」 names exactly the two limits that exist only on （2） (mark cap + 3-turn CD), and neither is waived. The comment at `nanami_effort.rs:252-253` states the waiver is deliberately unimplemented. Tests only assert the crystal spend / (4) discard and `decline_all` the prompts.
- **question**: see C-Q46.
- **note**: Contrast `CRYCHIC:（灯）内心的呐喊`（3） 「使用一次高松灯（MyGO!!!!!）的（2）技能」, which implements *that* body directly (`tomori_inner_shout.rs:155-197`) rather than `play_card` on the skill.

### W3-KAORU-02 `HHW:（薰）怪盗hello happy` — 「投掷1d2（前后）」 is a d6-peek parity, not a 1d2
- **class**: B (die shape) + C (polarity)
- **severity**: medium
- **rule text**: 「主要移动改为投掷1d2（前后）和1d10（距离）进行结算。」
- **code**: `rules/cards/card-hhw/src/kaoru_thief.rs:162-188` —
  `reverse = v.recorded_roll.map(|dir| dir % 2 == 1)` over the host's generic
  d6 peek.
- **gap**: The sheet asks for two dice; distance (1d10) is correct, but direction
  is a hidden d6 parity (faces 1–6, logged as the direction), not a rolled 1d2.
  The TODO at `:168-171` claiming 「the host does not fill `recorded_roll` yet」
  is **stale** — `wasm_rules.rs:2805-2818` pre-rolls `peek.d(6)` into
  `recorded_roll` for every `DiceRollPlan` / `DiceRollValue` drive.
- **capability**: a true `Dice { sides: 2 }` recorded face, or a card-side
  `roll(1,2)` whose result becomes `reverse`.
- **question**: polarity (face 1/2 vs odd/even) is C — see C-Q47.

### W3-KAORU-03 `HHW:（薰）怪盗hello happy` — 「所有在薰所在格子的人」 excludes 薰 himself
- **class**: C
- **severity**: medium
- **rule text**: 「此卡在场上时所有在薰所在格子的人如果可以移动，则主要移动改为投掷1d2（前后）和1d10（距离）进行结算。」
- **code**: `rules/cards/card-hhw/src/kaoru_thief.rs:31` — filter pre
  `turn_player != owner && pos(turn_player) == owner.pos && …`.
- **gap**: 「所有」 reads as every piece sharing 薰's tile, including 薰. The
  `!= owner` conjunct is an unprinted extra gate justified only by C#
  `m.Seat != Seat`.
- **question**: see C-Q48.

### W3-HERE-01 `Sumimi:Here the world` — (2) completion discards this card; the sheet never says so
- **class**: C
- **severity**: medium
- **rule text**: 「当奇迹水晶数为0时，那名玩家将那张卡加入手牌，并使此卡使用者抽一张卡。」
- **code**: `rules/cards/card-sumimi/src/here_the_world.rs:161-185` —
  `ctx::set_dest(ctx::Dest::Graveyard)` after returning the held card
  (comment: C# `H.Unplace(this, "discard", "结束了")`).
- **gap**: Nothing prints a discard / unplace of Here the world. The discard is
  load-bearing for a one-shot reading (the `slot('here_held_len') <= 0` guard on
  `Drew` would otherwise re-arm (2) on the next draw) — which is exactly why it
  needs a ruling rather than a silent C# carry.
- **question**: see C-Q49.
- **note**: neighbours already filed, not duplicated: **CH-13** (fixed),
  **C-Q33** (capture vs auto-play).

### W3-ONEOFUS-01 `AG:ONE OF US` — bankruptcy hand-over skips 「放置在该卡上」 and does real ownership, not 「视为拥有」
- **class**: A (the printed two-step can be written today) + C (「视为拥有」 vs real owner change)
- **severity**: medium
- **rule text**: 「当其中一方破产时，将两张被指定地契放置在该卡上并转移到存活方的游戏区，该方视为拥有此地契」
- **code**: `rules/cards/card-ag/src/one_of_us.rs:262-291` — for each designated
  tile still owned by the leaver, `ctx::set_owner(t, survivor)`. The comment at
  `:258-261` admits the C# does not park the deeds on the card.
- **gap**: Three printed details missing or altered: (1) 「将两张被指定地契放置在该卡上」
  never happens; (2) 「视为拥有」 is usually a continuous view, the code rewrites
  the real owner; (3) 「两张被指定地契」 is unconditional, the loop only moves
  deeds the leaver still owns (a designated deed sold to a third party stays put).
- **question**: see C-Q50.
- **note**: income splitting (`lose_raw`/`gain_raw`, 「不受其他任何效果影响」)
  already matches the sheet (TILE-01 t05 fixed).

### W3-DISP-01 `.text()` line-break joins (display only)
- **class**: A
- **severity**: low
- **locations**
  - `rules/cards/card-morfonica/src/nanami_effort.rs:47-51` — sheet (3) breaks
    before 「（2）技能」
  - `rules/cards/card-ppp/src/tomorrows_door.rs:38-40` — sheet (2) breaks before
    「（1）效果的序列」; sheet also uses blank lines between (1)(2)(3)
  - `rules/cards/card-sumimi/src/here_the_world.rs:101` — sheet keeps （1） and
    （2） on separate lines
- **gap**: Non-whitespace wording is identical to `cards.json` on all five cards.
  Only printed line structure differs, so the UI shows a denser paragraph.
- **fix**: literal newline restore in the three `.text()` strings. No behaviour impact.

---

# Stale doc references (drift notes)

These name other docs whose text no longer matches the tree; fixing them is
doc maintenance, not rule work.

## TEST-FINDINGS.md
- **`:159–160` §10 Resolved-summary bullet "ring opens at the next seat".**
  Superseded by the 2026-10-07 ruling at `:787-798` (ring starts at the
  **initial user**; exhaust before advance). The ileuxali "next seat" clause is
  marked Superseded at `:757-763`.
- **`:1379–1381` "Pre-existing … `inter_yolo_pushes_haneoka_over_20` fails with
  `err.play_timing` on master source too".** Stale — that test is green
  (un-ignored 2026-10-10).

## COVERAGE.md
Header (:17-20) "**774 tests, 102 ignored**" dated 2026-10-06 is historical. Every
listed `**ign**` row below is still stale vs the live set:

| row | live state |
|---|---|
| `:284` CiRCLE band unported | fixed |
| `:378-382` NNM dual On::Play (Mor #1–#5) | fixed (`1e80323`) |
| `:560` MyGO band draw unreachable | fixed (one `On::Play`) |
| `:500-501` 壱雫空 #2/#3 | green |
| `:552` 要乐奈 Space teleport | fixed |
| `:741` 游击演出 #3 | fixed |
| `:822` 凑友希那 #3 fire-cap | fixed |
| `:199-201` （祥子）带领着大家 #1–#3 | green |
| `:186` `if_only_it_lasted_discards_at_hand_7` | green |
| `:1138` / `:1145` Top-25 #3 祥，移动 / #10 壱雫空 | halves now implemented |
| `:807` Ringing Bloom "(2)/(3) do not apply" | tests green; open item is C-Q36 / CRYSTAL-08 |

## CROSS-TESTS.md
`:33-39` ring rules quote the superseded clause-89 start / one-activation-per-visit
(superseded by the 2026-10-07 initial-user ruling). The "70 pass / 41 ignore"
tally is not in this file — it is `COVERAGE.md:1242-1244` describing CROSS-TESTS
§1–§5 (2026-10-06); both are historical.

## NEGATION-AUDIT.md
`:315-319` N1 (hook vs counteraction order) and `:320` N2 (marker spends) are
listed as open in the §9 table, but V1 was fixed 2026-10-07 (`:61`, `:88-91`)
and §5 was answered the same day (`:238-254`). N3–N6 (`:321-324`) remain open
(folded into MONEY-04 / SETTLE-05 / C-Q3 / C-Q10).

## SETTLE-STAGES.md
`:169` "latent bug" row on 凑友希那 force-stop is stale — it says
`HookKind::PassPlayer … never fires`, but live `rules/skills/skill-personas/src/kokoro_practice.rs:39-47`
uses `ActionKind::PassTile` with `move.remaining > 0` and notes the §4 M4
latent bug is fixed. The `TODO(规则书) PIPELINE-AUDIT Q7` marker at L5 is already gone.

---

# Newly filed 2026-10-18 (card loyalty review, wave 1)

Subagent clause-by-clause review of the 15 most quote-dense cards against
`docs/rulebook/cards.json` (ground fact), plus a mechanical `.text()` /
quoted-name sweep of all 180. Reports: job tmp `wave1-{mixed,ppp,pp}.md` +
`findings-main.md`. **Quoted wordings** were the focus: a `.text()` that drops
or moves a 「…」 clause, and a “...” proper name that fails to resolve.

## Open

### Replay host-answer matching by position (LOY-TSUGU-01 residual)
- **class**: B. **severity**: low.
- A guest body that re-runs after a `HostRequest` sees the answers replayed by
  position, so a `cant_move` read inside the body flips on the re-run and a later
  prompt disappears. Worked around in `tsugu_ycm.rs` by latching the destination
  in a slot (2026-10-28 batch; pinned by
  `tsugu_effects_land_when_the_move_is_spent`), not fixed at the root.
- **fix**: an arg-identity check in `request_or_pause` would let cards read the
  gate plainly.

### LOY-DREAM-01 `PP:梦在前方，结彩当下`（4）is a turn-end [反击], not a hand press
- **class**: A. **severity**: high.
- **rule text**: 「（4）[共鸣][反击][拥有者]回合结束时且在[拥有者]所在格子前后3格
  以内有无主的[可购买格子]：使用此卡上3个[奇迹水晶]，[拥有者]购买任意…」
- **code**: `card-pp/src/dream_ahead.rs:50` — `On::Action(Action::TurnEndBefore(
  "…", Some(has_free_near), buy_one))`. Field card; the residual guard is the
  clause's third availability fact (a free unowned buyable tile within 3), not
  the removed `can_buy` press shape.
- **status**: **partially fixed** — the buy fires at the owner's turn end
  (`dream_ahead_buys_a_tile_at_turn_end`). Still open: the printed shape is a
  [反击] *declaration*; turn-end is hook-only, so the entry is an `On::Action`
  and the [反击] window is the missing half (in-file `TODO(规则书) LOY-DREAM-01`).

### LOY-ANON-01 residual — the 消耗 runs before the pair refusal
- **class**: A on the payment order; **C** on whether a tile may sit in two
  links at once (the singular 「1个…（上限1）」 leans no). **severity**: medium.
- **rule text**: 「并在上述的两个格子间放置1个[奇迹水晶]（上限1）」
- **code**: `card-mygo/src/anon_tokyo.rs:136-146` — `place_mark_pair` is
  place-or-refuse atomic (commit 6f9c5564), but the 消耗 still runs first, so a
  re-link at the cap pays the half and places nothing: the 消耗 and the 放置
  are one body (in-file TODO).

### LOY-STORM-01 residual — 「普通[结算]」 of a mortgaged tile has no 收费 recipient
- **class**: C. **severity**: medium.
- **rule text**: sheet 2026-10-06 新卡组卡 H8 「…对那个玩家进行一次相当于此卡所在
  格子普通[结算]的(4-X)/4倍价格的**收费**」.
- **code**: `card-ras/src/studio_storm.rs` charges a one-shot 「收费」
  (`ctx::charge_rent`). A mortgaged tile's 「普通[结算]」 is a forced-buy offer,
  not rent, so there is no obvious recipient; it charges nothing there for now
  (`TODO(规则书)[ruling]` on the call site) — needs a ruling.

### LOY-C pile (Class C — needs a ruling; do not guess)
- `PPP:STAR BEAT!` — 「下次结算后…随后减少1层」 is neither one-shot nor a clean
  bank: the layer survives a decline but burns on a failed build
  (`star_beat.rs:155-184`).
- `通用:@Tsugu ycm` — 「依次进行以下操作」 is 1→2→3→4, but the code runs 3
  (the −1500 BuyAdd) before 2's optional buy so the buy is discounted. 「本回合」
  vs 「依次」 fight. Related to `OPEN-GEN-tsugu-3-order`; the concrete
  discount-on-buy question is not yet named there.
- `AG:绯红之魂`（1）— sheet text 「因导致的[消耗]或[支付]」 is missing its
  antecedent (both `cards.json` and `cards-sheet.csv`). Implementation takes the
  maximal reading (any owner consume/pay).
- `MyGO:[千早爱音]Anon Tokyo` — linked-rent hook requires the payee be the
  card's owner; the printed clause keys on the tile being linked.
- `通用:该清CP了` — the "a copy is still in play" play gate is a 2026-10-07 user
  ruling attached to a `[特]` the sheet no longer carries.
- `PP:再次闪耀`（2）— 「[共鸣]记录一个此卡未记录的颜色」 has no 同色地契
  restriction (contrast (1)); `record()` shares (1)'s filter and over-restricts.
  Also which deed when several match for 「÷5」 (code takes max).
- `PP:练习生解密指南` build-off layers — `set_build_discount(1000*n, 1)` +
  peel-1 gives −3000 total at n=2; neither pure reading of the quoted status
  「下次盖房的价格减少1000（可溢出），盖房后减少1层」.
- `PP:[若宫伊芙]属于我的武士道！` — 「（1）[使用者]抽卡后」 hooks on owner; they
  coincide on every normal path (the card is placed on [使用者]的[场地]).

# Newly filed 2026-10-18 (card loyalty review, wave 2)

15 more quote-dense cards reviewed clause-by-clause (3 subagents × 5). Reports:
job tmp `wave2-{a,b,c}.md`. **32 new findings** (5 high, 8 medium, rest low).
Display text / quoted names were clean on all 15 — the problems are behavioral.

## High

### LOY-BLACK-01 residual — 「该格上拥有奇迹水晶时」 is only re-evaluated by this card
- **class**: B. **severity**: medium.
- **rule text**: 「该格上拥有奇迹水晶时，该格获得“CiRCLE”格子的全部效果」
- **code**: `card-hhw/src/black_suits.rs:104` (`TODO(规则书)`): the state is
  re-checked only on this card's own place/pass, not on any external crystal
  add/clear (`sync_circle_instance`). Same root as HAG-01 (no post-commit token
  `*Changed` event).

### LOY-SEEYOU-01 `PP:[衍生]明天见` a declined play is still announced as played
- **class**: B (the announcement half). **severity**: high.
- **rule text**: 「（1）只有[使用者]的正面[P✽P粉丝]数量少等于反面[P✽P粉丝]数量才可
  使用。」+「（2）[共鸣]无视此卡的[特]效果（1）。」
- **code**: `card-pp/src/see_you_tomorrow.rs:45-83` — `cant_play` allows whenever
  a 共鸣 is in hand even if `up > down`; the body `return Ok(())` on a declined
  `try_resonance`.
- **gap (announcement — open)**: the play is already announced when the body
  runs, so a refused 共鸣 still counts as a played card -- the log, the `card`
  window and the played/two-cards counters all ran and it can be counteracted
  (in-file `TODO(规则书)`). Fix: move the 共鸣 into the new activation
  collector (`docs/ACTIVATION.md` §2) and `Activation::Decline` on refusal.
- **note**: the bypass structure itself is right (gate holds → free play; gate
  fails → only via 共鸣).

### LOY-NET-01 `通用:网络链接异常` clause 1/2 branch sees only `DESIGNATES` cards
- **class**: B. **severity**: high.
- **rule text**: clause 1 「手卡的[手]效果且有[指定]目标则取消其对目标之一的[指定]」
  vs clause 2 「…没有[指定]目标则抵消其所有的效果」.
- **code**: `card-general/src/net_error.rs` uses `ctx::designations()`, which is
  `DESIGNATES`-prop-only. Cards that `[指定]` a subset (e.g. `通用:雨啊，快点来吧`)
  report empty and take clause 2 (full cancel) instead of clause 1.
- **note**: at the Card window effects are not yet declared (`effect::count()==0`),
  so this needs static play/card targeting metadata (C# `CardDef.Targeting`
  stand-in) or always route clause 1 through the `Target` window. Do not guess.

### LOY-GUERRILLA-01 `RAS:游击演出` 「经过属于其他玩家的livehouse」 not implemented
- **class**: B. **severity**: high.
- **rule text**: 「当你经过属于其他玩家的livehouse格子后…」.
- **code**: `card-ras/src/guerrilla.rs:83-88` (TODO). The counteraction opens on
  any rent-paid-to-someone-else. Needs a move-path / `H.Settling` snapshot on the
  `Paid` trigger.

## Medium

- **LOY-THANKS-01 residual** `通用:CiRCLE THANKS PARTY!` [med/A] — `can_pay`
  (晕眩/除外 PayGate) still narrows 「所有其他玩家」 (`thanks_party.rs:47`,
  in-file TODO).
- **LOY-TOMORI-03** `CRYCHIC:（灯）内心的呐喊` [med/A] ×2 — （3）'s 「最早拥有的格子」
  (`firstTile` slot is never written; falls back to `min()` tile index); （2）'s
  「将其洗入抽牌堆」 cannot move a field copy.
- **LOY-GUERRILLA-02** `RAS:游击演出` [med/C] — `paid_in_settle` only accumulates
  `PayKind::Rent`; text says any settle-caused payment to another player.
- **LOY-STARRY-01** `Mor:蝴蝶飞舞的星月夜` [med/C] — 「你每次移动掷骰时可以放弃第一次
  的结果重骰一次」 is narrowed to the main move's free roll; forced-move rolls never
  see the rider. Text says 「每次」.
- **LOY-SEEYOU-02** `PP:[衍生]明天见` [low/C] — is `[共鸣]` a mandatory cost of the
  bypass or merely an ignore? (Pool reading is what the code intends.)

## Low / Class C

- **LOY-RANDOM-01** `PPP:仓库里的Random Star` [low/A] — (2)'s 「强制停下」 skips the
  `[异常移动效果]` gate (`ctx::gate(…, AbKind::Stop)`); a `[不可阻挡]` mover cannot
  refuse, and the 2 stickers are spent even if the stop is refused. Compare
  `hagumi_marks.rs:144-146`.
- **LOY-MUSTBUY-01** 「必须购买」 shortfall on `RAS:游击演出` + `Sumimi:Sweet Escape`
  [low/C] — soft-fail (log broke, skip) vs glossary `[破产]`. Decide once.
- **LOY-MUSTBUY-02** [low/A] — the settle's optional buy still opens before the
  「必须购买」 offer on both cards. 「必须购买」 should replace the optional.
- **LOY-MUTSUMI-01** `CRYCHIC:（睦）从没有觉得...` [low/C] — (2)'s
  `set_dest(Banished)` is an unprinted `[移除]`; only (3) prints 「此卡[移除]」.
- **LOY-BLACK-02** `HHW:黑衣人的补给` [low/C] — the borrowed pass-reward's
  「移动起点不为CiRCLE」 exemption does not re-bind (弦卷集团 is kind `agent`, so
  `is_start` never applies). Two readings of 「获得"CiRCLE"格子的全部效果」.
- **LOY-RANDOM-02** `PPP:仓库里的Random Star` [low/C] — (2) offer suppressed on
  landing step and teleports (`move.remaining > 0 && move.kind != Teleport`).
- **LOY-WAIT-01** `PPP:（有咲）等等等一下` — （2） is already filed as ABI-09 (a)
  `IEventDefer`. (1)'s 「非因为此卡导致」 filter is vacuous until Defer lands
  (OPEN-wait-causality).

# Engine follow-ups

## FIRE-TOKENS — open follow-ups (2026-10-24)

The 火罐 re-home onto the persona-skill token counter landed on
`worktree-fire-tokens`. Two follow-ups remain:

- **FIRE-TOKENS-1 — legacy `state["charge"]` is never folded into the token
  row.** The webui falls back to `state["charge"]` for display
  (`webui/src/core/names.ts`), but nothing migrates the value into `tokens`,
  so a match restored from a pre-re-home blob shows N pots while
  `fire::get`/`max` read 0 -- skills gate wrong and gains declare against a
  cap of 0. **Replays are unaffected** (records carry their engine bundle and
  are replayed by the archived one). The live path is the server's room
  persist/restore (`crates/server/src/room.rs` `persist` / `Room::restore_game`
  / `MatchHandle`) across a crash or deploy. Fix is a one-shot fold on decode:
  if `state["charge"]`/`["tempFire"]` exist and no matching `tokens` row does,
  copy value + `StateVar.max` into a row bound to the persona-skill uid.
  Needs a look at where the match blob is actually decoded first.
- **FIRE-TOKENS-2 — debug `state` cheat cannot reach fire.**
  `crates/game-core/src/engine/debug.rs` validates on
  `state.contains_key("charge")` and writes `MatchPlayer.state`, both dead
  after the re-home. Give it a token-store equivalent (or drop the charge
  case).

## Not landed: ENG-COST — a generic press activation cost

`PPP:Returns` 「需要支付1星星贴纸」 is a cost of activation. The rule side keeps
`copy_sticker_gate` / `spend_copy_sticker` / `copy_sticker_cost`
(`rules/skills/skill-groups/src/lib.rs`), which works but means every
sticker-spending body budgets `2 + tax` and calls `spend_copy_sticker` first.

The engine form would be a cost declaration on `On::Play` (the same family as
the [共鸣] fuel), charged by the press pipeline before the body runs. That is a
real `card-sdk` mechanism, but it is an ABI shape change to the hook table, and
three other verbs were landing in the same files. Deferred — the ruleset helper
is the current home, and a second activation-cost card is the trigger to
generalize.

# Newly filed 2026-10-21 (suite-trio follow-up)

## OPEN-ROLLRESHAPE-01 — order of two DiceRollValue reshapes (「最终结算」)
- **kind**: rulebook ambiguity (needs a ruling)
- **class**: C (open question) / B if the ruling changes the fold
- **severity**: low (one step of movement, rare stack)
- **suggested batch**: skills: AG / PP
- **locations**
  - `rules/cards/card-ag/src/moca_half.rs` `roll_half` (`TODO(规则书)` on the
    composition)
  - `rules/cards/card-pp/src/no_expectation.rs` `roll_shave`
  - `crates/card-sdk/src/payload.rs` `RollValue` (the fold that made stacking
    possible at all)
  - `rules/tests/tests/gap_money.rs:159` `g02_half_speed_plus_expectations_floor_on_a_rent`
- **rulebook**
  - `AG:（摩卡）0.5倍速` (2): 「此卡在场时，你的移动掷骰的**最终结算**/2（向上取整）」
  - `PP:不要背负期待` (1): 「非回合开始时进行投掷的**投掷结果减少2**」
  - `docs/rulebook/spec/cards/ag.md` `C-AG-half-speed-03` ruling: 「最终结算」
    is **after additive modifiers**.
- **the two readings**, for roll 7 starting at tile 27:
  | reading | arithmetic | move | lands |
  |---|---|---|---|
  | half last (「最终结算」 after the additive −2) | 7 − 2 = 5, ceil(5/2) | **3** | 30 |
  | half first (placement order, as implemented) | ceil(7/2) = 4, 4 − 2 | **2** | 29 |
- **current**: implemented as placement order (half first) and the test pins
  29. The test's own comment ("roll 7 -> 5 -> 2 either way round") is loose:
  `5 -> 2` is a *floor* half while the card prints 向上取整, and "either way
  round" is false -- the two orders give 2 and 3 respectively.
- **why this is open, not a bug**: 「最终结算」 is not defined for a roll that
  another effect has already reshaped. The spec ruling says "after additive
  modifiers", which points at half-last (3); the engine's documented contract
  (`moca_half.rs`, ABI v74) is "placement order among roll reshapes is
  preserved", which points at half-first (2). Neither the card text nor the
  rulebook body settles it. The C# port calls the half `DiceRollAfter`, but
  the C# port is not a spec.
- **what a ruling must say**: whether 「投掷结果减少2」 is an *additive
  modifier* that 「最终结算」 follows (→ 3), or a same-stage reshape that runs
  in placement order (→ 2). Also whether 「最终结果最小为0」 in the same
  不要背负期待 clause applies before or after the half.
- **until ruled**: keep placement order + the pinned 29; the composition now
  *folds* rather than clobbering, which is strictly better than before.

## STATE-INIT-01 — default-valued per-player slots materialised at placement
- **kind**: implementation bug / observation-frame smell
- **class**: A (mechanical), low severity
- **suggested batch**: card-rule cleanup
- **locations** (all write a per-player `set_slot` at play/placement whose
  written value is `StateVar::default()`, or a 0-based player id that makes
  seat 0 indistinguishable from unset)
  - `rules/cards/card-pp/src/jennifer.rs:86` `set_slot(hit, SLOT_USER, player_id)`
    -- same 0-based `Card.User` bug FEVER! just left behind; writes 0 onto the
    *target's* state when the jennifer player is seat 0
  - `rules/cards/card-ras/src/chuchu_music.rs:77-78` `SLOT_USER` (0-based) +
    `SLOT_HITS = 0` (pure default init)
  - `rules/cards/card-roselia/src/council_check.rs:108-109` `SLOT_GOT = 0`,
    `SLOT_STOP = 0`
  - `rules/cards/card-ras/src/layer_keep.rs:115` `SLOT_UNUSED = 0`
  - `rules/cards/card-crychic/src/fate_together.rs:49` `SLOT_HIT = 0`
- **was**: `通用:[衍生]FEVER!` had this and it was caught by
  `C-GEN-fever-frame` (`spec_card_general::fever_gain_boost`). The frame is a
  good detector; the other cards have no such frame so the smell is silent.
- **expected**: a per-player key is written when it takes on meaning, not
  pre-zeroed at placement. Per-card-instance Mem belongs on
  `FieldCard::props` (`ctx::set_prop` after `place_card` -- `RAS:EXIST`'s
  `exist.used`, and now FEVER!'s `fever.user`), not in the player's
  keyed-state map. Player-id Mem slots are **1-based** (`player_id + 1`) so
  seat 0 survives the default-0 read.
- **done right already**: `Mujica:（海铃）` `umiri_card.rs:67`,
  `Sumimi:Here the world` `here_the_world.rs:119`, `AG:One of us`
  `one_of_us.rs:145` (1-based), `RAS:EXIST` `exist.rs:44` (instance prop).
- **note**: several of these are observation-frame issues rather than
  behaviour bugs -- the write is a no-op at 0. Still worth cleaning so a
  future frame check does not trip, and so the 0-based `Card.User` cases
  (jennifer, chuchu_music) get their seat-0 bug fixed.

# Open engine defects (carried)

## COND-01 — `slot()` conditions on FilterValue miss mid-roll state writes (possible engine defect)
- **Symptom (2026-10-17, seen while splitting eve_unify):** a `slot('…') >= n`
  *condition* on an `On::FilterValue` at `DiceRollPlan` did not observe a
  per-player state write made earlier in the same roll, while `state::get`
  inside the body saw it. The condition and the body therefore disagree about
  the same world.
- **Suspect:** the condition (CEL) evaluation uses a snapshot taken before
  mid-roll writes (or a cached `slot` view), while body calls read the live
  world. Affects any entry whose gate counts up during the roll.
- **Likely affected:** `kanon_lost`'s `On::FilterValue(DiceRollPlan)` gate
  `slot('skill.kanonLost.silent') >= 3` (TODO(bug) at the entry in
  `rules/skills/skill-personas/src/kanon_lost.rs`); any future gate written the
  same way. eve_unify worked around it (`actor == owner` + `state::get` in the
  body).
- **Check:** a two-roll scenario where the slot increments mid-roll and the
  gate must flip on the same roll; compare `slot()` in the condition vs
  `state::get` in the body.
- **Fix needs:** make the condition see the same world the body sees (evaluate
  conditions against the live world, or document that conditions are
  pre-roll-snapshot and move such gates into the body); a test pinning both
  readings agree.

---

# Newly filed 2026-10-23 (ABI v98 `declare` + `can_fire`)

## Not landed: DECLARE-TRAP-LOG — every "entry did not fire" trap should log

`docs/ACTIVATION.md` §2 "Follow-up (tracked)". Severity is already correct and
pinned (`trapping_declare_never_costs_the_card`, `declare_arg`): a trapping
`OP_DECLARE` declines and does not cost the card. This is observability only.

The split is backwards today -- loudest where the player needs it least:

| site | logs `log.card_trap`? | the player… |
|---|---|---|
| residual guard (`entry_admitted`) | no ("historical dispatch leniency") | — |
| `OP_DECLARE` on a hook (`declare_after`) | **no** | clicked nothing; a 「可选择」 silently did not apply |
| `OP_DECLARE` on a play / [反击] | yes | just clicked something and would notice |
| `OP_RUN` (body) | yes | — |

The hook path is the automatic 「可选择」 case -- the whole reason `declare`
exists for hooks -- and the reference is `AG:绯红之魂`'s `pay_offer`. If that
collector traps the player is **never asked**, the −1000 silently does not
apply, and nothing is logged: they are looking at a payment that did not get
cut, not at a dialog that failed to appear. The residual guard has the same
hole and it is older.

- **kind**: ABI observability gap
- **class**: B
- **severity**: low (diagnostics; correctness is already held)
- **suggested batch**: card-sdk / game-rules host
- **locations**: `crates/game-rules/src/host.rs` `declare_after` (hook
  `OP_DECLARE` trap arm) and `entry_admitted` (residual-guard trap arm);
  `docs/ACTIVATION.md` §2 "Follow-up (tracked)"
- **shape** (peer-reviewed): `on_trap: Option<&mut dyn FnMut(&mut W, i32)>` on
  `entry_admitted`, mirroring `run_hook`'s existing `on_body`. **Do not** put
  `trapped` on `Admitted::No` / `HookRun` -- a trapping declare yields
  `Admitted::No`, which makes `run_hook` return `Ok(None)`, so the drive has no
  channel left to learn that one of them trapped.
- **land both arms together** (hook `OP_DECLARE` *and* the residual guard) or
  the table gains a fifth row and it is a half-fix.

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
「若指定了不存在状态2的玩家则无效果」: does this gate the switch on the target
already being in 状态2 (only 2→1), or rule out targets for whom 状态2 is
unreachable? Engine now (2026-10-17 verified at `welcome_mujica.rs:58-66`):
the switch is a **no-op unless the target's `skillState` is 1 or 2** — so a
player who cannot reach 状态2 is not toggled. The older "free toggle" note was
stale.

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

**C-Q44 (TILE-01 follow-up, co-owned hill as 「[拥有者]拥有的格子」).** *(filed 2026-10-17 — was missing a backlog home)*
When a deed is co-owned (星之鼓动山丘 is 「所有Poppin' Party角色视为同时拥有」), does it
count as 「[拥有者]拥有的格子」 for Tomorrow's Door (3)'s surcharge for **each**
co-owner? Test `t22_hill_coownership` (`rules/tests/tests/cross_tiles.rs:984`) is
`#[ignore = "RULING: …"]`. Cross-ref `OPEN-door-hill` in `docs/rulebook/spec/OPEN-QUESTIONS.md`.

**C-Q45 (春日影 draw-to-6 replaceable draws).** *(filed 2026-10-17 — was missing a backlog home)*
春日影's 「抽卡直到手牌为6张」 iterates. Is **each intermediate draw** a
replaceable draw (so a draw-replacement effect can fire per card), or only the
final one? Test `g29_maya_look_at_top_inside_the_draw_curve`
(`rules/tests/tests/gap_res.rs:251`) is `#[ignore = "RULING: …"]` and asserts yes.
Cross-ref `OPEN-春日影抽6`. Note the sheet-sync 2026-10-17 added 「且手牌不多于5张」
to the [特] moment, which changes when the curve can start.

**C-Q46 (W3-NANAMI-01, NNM (3) 「视为发动你的（2）技能」).** *(filed 2026-10-21)*
`Mor:（NNM）稍微努力了一下`（3）: 「你可以移除一个奇迹水晶视为发动你的（2）技能，
此次技能不受数量或轮数限制」. 广町七深's printed （2） is 「当有玩家在使用角色技能时，
你可以支付一个火罐得到一个该角色的标记」 (cap 1 per mark type at ≥3 players, 3-turn
CD on passive skills). (a) What does a **waived** （2） press do as a crystal
activation — pay 1 [火罐] and gain a mark of a chosen character, waiving cap+CD?
(b) Or is 「（2）技能」 loose wording for the character's skill button (clause （3）,
spend a mark to borrow a skill), in which case 「不受数量或轮数限制」 has nothing
to waive and the printed numbers are wrong? (c) Other. The code currently invokes
（3） via `play_card` and waives nothing.

**C-Q47 (W3-KAORU-02, 1d2 polarity).** *(fileed 2026-10-21)*
`HHW:（薰）怪盗hello happy`「投掷1d2（前后）」: for the direction die, is 前/后
**face 1 / face 2**, or **odd / even**? The port reads odd = reverse (the C# the
module cites used `num == 2` = reverse). Separately the die is currently a d6
parity, not a 1d2 — that half is class B (W3-KAORU-02).

**C-Q48 (W3-KAORU-03, 「所有在薰所在格子的人」).** *(filed 2026-10-21)*
Does 「所有在薰所在格子的人」 include 薰 himself? The port drops the owner
(C# `m.Seat != Seat`). If 薰's own main move is also meant to become 1d2+1d10,
the `turn_player != owner` conjunct is an unprinted extra gate. A plausible
design reading is that the effect weirdifies *other* pieces standing with 薰.

**C-Q49 (W3-HERE-01, Here the world end-of-hold).** *(filed 2026-10-21)*
`Sumimi:Here the world`（2）: 「当奇迹水晶数为0时，那名玩家将那张卡加入手牌，并
使此卡使用者抽一张卡。」 Nothing prints that *this* card leaves play. The port
discards it (C# `H.Unplace(this, "discard", "结束了")`). If it stayed, the
`slot('here_held_len') <= 0` guard on `Drew` would re-arm (2) on the next draw.
Does the card leave play on (2) completion, or stay and re-arm?

**C-Q50 (W3-ONEOFUS-01, ONE OF US bankruptcy hand-over).** *(filed 2026-10-21)*
`AG:ONE OF US`: 「当其中一方破产时，将两张被指定地契放置在该卡上并转移到存活方的
游戏区，该方视为拥有此地契」. (a) Is 「视为拥有」 a **view** on top of the old
owner, or a real ownership move? (b) Must **both** designated deeds move even if
one was sold to a third party before the bankruptcy (the text says 「两张被指定地契」
unconditionally)? (c) Does 「放置在该卡上」 have any mechanical consequence beyond
the intermediate park / log shape?

---

**C-Q51 (clear-cp-spread-window).** *(filed 2026-10-28; rows 1507; `rules/cards/card-general/src/clear_cp.rs`)*
[使用者]使用此卡后的下2回合开始时

Current: spread_guard requires trigger::player_id() == owner

Decision needed: After the card is used, whose two turn starts must the spread fire on — the user's own next two, or the next two chronological (any-player) starts?

**C-Q52 (fever-floor).** *(filed 2026-10-28; rows 932; `rules/cards/card-general/src/fever.rs`)*
X降低200（可小于0）

Current: (v.value + x).max(0) with no marker

Decision needed: When X is negative, may the boosted amount go negative (making the owner pay the difference), or is it floored at 0 as the code does — the cell only says X may be below 0 and prints no floor on the result?

**C-Q53 (marina-window-guard).** *(filed 2026-10-28; rows 881, 882; `rules/cards/card-general/src/marina_work.rs`)*
[经过]#1格子且#1格子受到其他效果影响时

Current: pre::MINE only; the card is offered on every passRewardAffected, including the re-homed cafe

Decision needed: Which passed tile counts as #1 (CiRCLE or its re-homed cafe), and what counts as 「受到其他效果影响」, so the window opens only for a #1 passage carrying another effect rather than on every passRewardAffected?

**C-Q54 (tsugu-step-3-before-2).** *(filed 2026-10-28; rows 853, 854; `rules/cards/card-general/src/tsugu_ycm.rs`)*
投掷3d10并根据结果依次进行以下操作：

Current: The linger lands at r>=28 before the (2) block, so the step-2 buy itself is already discounted (tsugu_ge28_buy_discount_1500 pins it).

Decision needed: Does 「依次」 install (3)'s −1500 only after (2)'s optional buy, so the buy from (2) is charged at full price, or does the turn-long modifier cover it?

**C-Q55 (tsugu-gain-fallback).** *(filed 2026-10-28; rows 1716; `rules/cards/card-general/src/tsugu_ycm.rs`)*
结果小于26则选择[获得]1000资金或进入移动阶段并将本回合的[主要移动]改为[传送]到“bandori车站”并[结算]。

Current: Picking the move when `move_blocked` is set (or no station tile) pays 1000 instead; latent today because the whole-card gate already requires the main move.

Decision needed: When (4)'s player picks the teleport but the main move is already gone, does the 1000 gain silently substitute, or is the choice taken against availability?

**C-Q56 (tritone-payee-unnamed).** *(filed 2026-10-28; rows 388, 1266, 1349; `rules/cards/card-morfonica/src/tritone.rs`)*
弃置此卡并支付由此卡获得的资金

Current: `ctx::pay(player_id, owed, ...)` pays the bank (`to: -1`); the cell names no payee.

Decision needed: Who receives 「支付由此卡获得的资金」 — the bank, or the players the money was gained from?

**C-Q57 (garden-unstoppable-extra-halves).** *(filed 2026-10-28; rows 405; `rules/cards/card-morfonica/src/tsukushi_garden.rs`)*
直到下个你的回合开始时，你无法被异常移动

Current: The grant adds [不可阻挡], which also bypasses the [停留] move gate and refuses new [晕眩]/[除外] layers.

Decision needed: Does the granted [不可阻挡] status's extra halves (ignoring [停留], refusing new [晕眩]/[除外]) fall under 「无法被异常移动」, or must the grant be narrowed to the abnormal-move half?

**C-Q58 (forced-settle-main-flag).** *(filed 2026-10-28; rows 1269, 565; `rules/cards/card-morfonica/src/noble_blue.rs, rules/cards/card-mygo/src/taki_serious.rs`)*
立刻进行一次触发结算 / 进行一次[触发结算]

Current: Both call `ctx::card_settle_at(..., true)`: noble_blue.rs:54 and taki_serious.rs:76. main=true writes `st.landed` and `move.main`, so main-only settle hooks (e.g. 「不能通过主要移动的结算盖房」) fire for a settle where the player never moved.

Decision needed: Is a card-forced 「触发结算」 a main move settle (main=true, overwriting 落地 and firing main-only settle hooks), or a plain settle?

**C-Q59 (believe-chooser-self).** *(filed 2026-10-28; rows 1201; `rules/cards/card-hhw/src/believe_you.rs`)*
选择一名玩家

Current: Only other players are offered and the card is refused outright with no rival.

Decision needed: Does 「选择一名玩家」 include the user as a legal chooser, so the card is still playable with every rival out?

**C-Q60 (charity-first-pass-scope).** *(filed 2026-10-28; rows 204; `rules/cards/card-hhw/src/charity_show.rs`)*
每初次经过一个属于你的格子

Current: The seen marker holds the turn key, so a tile passed earlier never counts on a later dice move of the same turn.

Decision needed: Is 「初次经过」 once per tile per move or once per tile per turn?

**C-Q61 (dream-rent-shape).** *(filed 2026-10-28; rows 125, 126, 1097; `rules/cards/card-hhw/src/dream_return.rs`)*
向场上你以外的任意一名玩家的一个格子进行一次支付

Current: `ctx::transfer` runs as PayKind::Card, so RENT_FACTOR, dice-rent and pay_is_rent are lost; crystals and the record are written after the payment, so the skill's (2) can never see it.

Decision needed: Is the payment a rent-shaped charge (RENT_FACTOR, dice rent, pay_is_rent) that the holder's band skill (2) can see, or a plain Card-kind transfer?

**C-Q62 (tomori-two-x-diff).** *(filed 2026-10-28; rows 993; `rules/cards/card-crychic/src/tomori_inner_shout.rs`)*
获得两个X差值*120的资金

Current: `diff * 120`; C-CRYCHIC-tomoricry-01 reads (|ΔX| * 2) * 120 and tracks OPEN-两个X差值.

Decision needed: Does 「两个X差值*120」 pay |ΔX|*120, or (|ΔX|*2)*120 as the recorded reading C-CRYCHIC-tomoricry-01 has it?

**C-Q63 (vocal-pre-modifier-amount).** *(filed 2026-10-28; rows 22; `rules/cards/card-crychic/src/vocal_too_hard.rs`)*
支付至少5000资金

Current: The effect window's `value` is the amount before paySplit/rounding, so a halved 9000 payment still opens the card.

Decision needed: Is 「支付至少5000资金」 tested on the declared pre-modifier amount or on the payment actually made after halving/rounding?

**C-Q64 (declare-war-skill-vs-card).** *(filed 2026-10-28; rows 37; `rules/cards/card-ag/src/declare_war.rs`)*
被其他玩家的卡效果影响

Current: Any rival card body stamped as `by_card` opens the card; persona/group skill bodies ride the same Pay path and also open it.

Decision needed: Does a persona/group skill body that charges a player count as a 「卡效果」 for 「被其他玩家的卡效果影响」?

**C-Q65 (detour-missing-plans).** *(filed 2026-10-28; rows 221; `rules/cards/card-ag/src/detour.rs`)*
视为使用一次初始Afterglow角色的

Current: PLANS has four entries but only three distinct skills; Moca's teleport and Tsugumi's reverse/deferred-move (2) are unreachable.

Decision needed: Which 「(2) effect of an initial Afterglow character」 does the card use — a fixed four-entry plan list, or any/one of the five (which would add Moca's teleport and Tsugumi's reverse)?

**C-Q66 (misaki-endpoint).** *(filed 2026-10-28; rows 300; `rules/cards/card-hhw/src/misaki_card.rs`)*
位于你的移动起点与预定移动终点之间

Current: `(1..=roll)` includes the planned end tile while the start (d == 0) is excluded -- asymmetrical.

Decision needed: Does 「位于你的移动起点与预定移动终点之间」 include the planned destination tile while excluding the start, or exclude both endpoints?

**C-Q67 (misaki-neighbour).** *(filed 2026-10-28; rows 1096; `rules/cards/card-hhw/src/misaki_card.rs`)*
那格及相邻2格上的所有其他玩家[支付]你500资金

Current: `for d in -2..=2` charges two tiles on each side plus the target tile.

Decision needed: Does 「那格及相邻2格」 mean the target tile plus its two neighbours (3 tiles) or two tiles on each side of it (5 tiles)?

**C-Q68 (ran-start-tile-undefined).** *(filed 2026-10-28; rows 45, 1111; `rules/cards/card-ag/src/ran_as_usual.rs`)*
回到起始地点

Current: turn_snap pos (the turn-start square) with no record.

Decision needed: What is 「起始地点」 — CiRCLE, the seat's opening tile, or the pre-move position?

**C-Q69 (mutsumi-removal-fate).** *(filed 2026-10-28; rows 101, 102, 1154; `rules/cards/card-crychic/src/mutsumi_never.rs`)*
消耗乐队技能卡上的3个奇迹水晶（不足3个则改为全部消耗），立即执行乐队技能的（2）效果，然后弃一张卡。

Current: Comment claimed a nonexistent Removed variant and that Banished posts nothing.

Decision needed: Does (2) banish the played card (an unprinted [移除]) or use the default graveyard fate that only (3) prints?

**C-Q70 (out-of-turn-self-caused-draw).** *(filed 2026-10-28; rows 1000, 55; `rules/cards/card-crychic/src/elegant_shout.rs, crates/game-rules/src/wasm_rules.rs`)*
当你在回合外受到抽卡效果时打出

Current: `if n > 0 && player_id >= 0 && player_id != cx.state().turn` (crates/game-rules/src/wasm_rules.rs:4315) opens for every effect-caused out-of-turn draw, a self-caused one included, and the card answers it.

Decision needed: Does 「受到抽卡效果」 cover a draw the player causes themself during another player's turn, or only an externally caused draw?

**C-Q71 (myown-offer-gate).** *(filed 2026-10-28; rows 1179; `rules/cards/card-crychic/src/my_own_problem.rs`)*
若受到[异常移动效果]影响，此卡不生效

Current: The guard is `can_counteract` (the offer filter), so the card is never offered once an abnormal move already hit; the body also re-checks and logs 不生效.

Decision needed: Should 「此卡不生效」 leave the card unoffered (a spent, played no-op) or let it be played and resolve as a no-op?

**C-Q72 (aya-empty-discard-condition).** *(filed 2026-10-28; rows 1541; `rules/cards/card-pp/src/aya_longing.rs`)*
[手]：将此卡放置在[使用者]的[场地]并将弃卡区中的一张卡加入手卡。

Current: On::Play("", None, aya_longing); empty graveyard returns after placing

Decision needed: Is a non-empty discard a legality condition that blocks the play, or does the play legally resolve with only its placement half?

**C-Q73 (chisato-stale-designation-cancel).** *(filed 2026-10-28; rows 1402; `rules/cards/card-pp/src/chisato_mask.rs`)*
（1）[拥有者]的弃卡区洗入抽卡区时其他玩家[分摊][支付][拥有者]500资金。

Current: split_pay consults designation_cancelled unconditionally; the flag clears only at the next hand play

Decision needed: Does a designation cancel from the current play also skip a later reshuffle's 500 分摊, or only the split inside that play?

**C-Q74 (guerrilla-mustbuy-shortfall).** *(filed 2026-10-28; rows 871, 1677; `rules/cards/card-ras/src/guerrilla.rs`)*
且必须购买

Current: On a shortfall the body logs guerrilla_broke and skips card_buy, so no raise-funds/破产 path runs.

Decision needed: On a 「必须购买」 purchase the player cannot cover, must the money pipeline run raise-funds/[破产], or does the buy soft-fail?

**C-Q75 (hey-kids-fee-scope).** *(filed 2026-10-28; rows 755, 1476, 760; `rules/cards/card-ras/src/hey_kids.rs, docs/rulebook/COVERAGE.md`)*
随后，你失去“转移后各格房屋造价总和－获得房屋数量×500”的资金。

Current: `cost_in` sums only the houses just moved (rules/cards/card-ras/src/hey_kids.rs:128), while 「各格」 can also read all owned tiles or the touched tiles including houses already standing there and the source remainder; COVERAGE.md:728 bakes the narrow reading.

Decision needed: Does 「转移后各格房屋造价总和」 sum every owned tile, the tiles touched by the transfer (including houses already there and the source remainder), or only the houses just moved?

**C-Q76 (hey-kids-split-grammar).** *(filed 2026-10-28; rows 756, 1475; `rules/cards/card-ras/src/hey_kids.rs`)*
转移的房屋与目标格子中，至少一方数量为1

Current: `max = houses.min(targets.len())`, so 2 houses onto 2 targets offers k = 2.

Decision needed: Does 「至少一方数量为1」 constrain the player's k so one of houses/targets must be exactly 1, or only describe the minimum (allowing k>=2 when both exceed 1)?

**C-Q77 (be-strongest-livehouse-table-open).** *(filed 2026-10-28; rows 725, 1673; `rules/cards/card-ras/src/be_strongest.rs`)*
roll 1d10，传送到livehouse对应的格子（按格子编号排序，若为10或以上传送到“Live House”）

Current: LIVEHOUSES is every group-6 tile board-wide; the reasoning lived only in spec prose.

Decision needed: Is the sorted livehouse table every group-6 tile on the board, or only the livehouses the user owns?

**C-Q78 (star-beat-layer-burn-ruling).** *(filed 2026-10-28; rows 652, 1426; `rules/cards/card-ppp/src/star_beat.rs`)*
可选择在绝对距离5格以内自己拥有的格子上进行一次盖房，随后减少1层

Current: A declined offer returns before the decrement (layer kept); the decrement runs regardless of `card_offer_build`'s false (layer burned).

Decision needed: When exactly does 「随后减少1层」 resolve — after a declined offer too, and after a build that fails for lack of funds?

**C-Q79 (repaint-exist-redirect).** *(filed 2026-10-28; rows 1669; `rules/cards/card-ras/src/repaint.rs`)*
使目标玩家的此次移动数-X

Current: On an EXIST redirect (`hit != them`) the body returns before `set_move_roll` and `set_pay_factor`, so neither effect happens.

Decision needed: When an EXIST redirect changes the target, do 「目标玩家」/「对方」 still mean the mover, so both effects apply to him?

**C-Q80 (repaint-min-roll-floor).** *(filed 2026-10-28; rows 1670; `rules/cards/card-ras/src/repaint.rs`)*
使目标玩家的此次移动数-X

Current: The engine re-applies `min_roll` after the diceRollResult window (`m.roll.max(mr)`, crates/game-core/src/engine/play.rs:1428-1434), so the cut is truncated.

Decision needed: Does the mover's 结果为10以上 floor re-apply after Repaint cuts the roll (truncating X), or does the cut stand?

**C-Q81 (storm-clause-readings).** *(filed 2026-10-28; rows 716, 1684, 1685; `rules/cards/card-ras/src/studio_storm.rs`)*
在距此卡所在格子X个格子处[结算]时 / 对那个玩家进行一次相当于此卡所在格子普通[结算]的(4-X)/4倍价格的收费

Current: `ctx::dist` is the undirected ring minimum (crates/game-rules/src/wasm_rules.rs:1961-1969), `charge_rent` charges the house-level/dice figure, and a mortgaged tile returns 0 so the card discards and charges nothing.

Decision needed: For 「在距此卡所在格子X个格子处」 and 「相当于普通[结算]的(4-X)/4倍价格的收费」: clockwise or undirected distance, base or house-level rent, and who (if anyone) is charged when the tile is mortgaged?

**C-Q82 (ringing-bloom-zero-value-pay).** *(filed 2026-10-28; rows 821; `rules/cards/card-roselia/src/ringing_bloom.rs`)*
你的任意非RiNG格子收费后

Current: No trigger::value() check, so the card discards and pays 500×X although the tile collected nothing; in a reflected rent only the 0-value parent pay carries target == owner.

Decision needed: Does 「收费后」 require the tile to have collected something, or does a fully counteracted / blocked / reflected pay still fire the 500×X?

**C-Q83 (sayo-cost-open).** *(filed 2026-10-28; rows 681, 1376; `rules/cards/card-roselia/src/sayo_play.rs`)*
打出时视为使用一次此卡使用者的技能

Current: The pre has no charge(owner) >= 6 and the body never spends 6 火罐, so the card grants +1/+2 at 0 pots; tests pin the free behaviour.

Decision needed: Does 「视为使用一次技能」 pay the skill's cost (its ≥6 火罐 gate and 6-pot spend), or is the use free?

**C-Q84 (debut-lock-waives-payee).** *(filed 2026-10-28; rows 51, 1001; `crates/game-core/src/engine/play.rs`)*
打出此卡后，本回合内你的资金不会下降（除拍卖与写明不受资金变动效果影响的情况外）

Current: money_inner returns Paid { paid: true, ..default } at the money_locked branch before the split windows, log_money, the payee credit at the transfer stage and the pay/paid raises -- the payer is spared and the payee is skipped.

Decision needed: Does the money lock spare only the user's debit while the payee still receives (and the pay events fire), or does it void the whole transfer?

**C-Q85 (lisa-bond-stage-ambiguity).** *(filed 2026-10-28; rows 694; `rules/cards/card-roselia/src/lisa_bond.rs`)*
当你使用技能进行传送后

Current: the code commits to MoveAfter (stage 13) and the stage TODO stays

Decision needed: Which stage is 「使用技能进行传送后」 — MoveAfter (action stage 13), or a post-teleport point that the teleport's own landing precedes?

**C-Q86 (nobreakup-predict-model).** *(filed 2026-10-28; rows 783, 1076; `rules/cards/card-sumimi/src/no_breakup.rs`)*
进行一次可使你的资金变动为拥有相同数字的传送

Current: `predict` (rules/cards/card-sumimi/src/no_breakup.rs:52-60) subtracts only table rent; purchase, build, force-buy, agent half-charge, event settle and dice rent are unmodelled, so legal destinations are omitted once any rent-repeat exists.

Decision needed: Which settle money changes must the prediction model cover (purchase, build, force-buy, agent half-charge, event settle, dice rent) for 「进行一次可使你的资金变动为…的传送」?

**C-Q87 (donuts-grant-void).** *(filed 2026-10-28; rows 828, 830; `rules/cards/card-sumimi/src/two_donuts.rs`)*
（1）获得[除外]直至你原本所在格子被其他玩家经过。

Current: A second copy returns at the top (rules/cards/card-sumimi/src/two_donuts.rs:58) before recording SLOT_ORIG/placing, while a grant refused by [不可阻挡] or a countered abnormal window is discarded and the body still records, places and later runs (2) and the pots.

Decision needed: When (1)'s [除外] grant is barred — a second copy already exiled, [不可阻挡], or a countered abnormal window — is the whole card void, or do the record, (2) and the pot payout still run?

**C-Q88 (l12-per-pot-reading).** *(filed 2026-10-28; rows 1319; `rules/cards/card-sumimi/src/l12.rs`)*
每当你消耗火罐时，为此卡添加一个[奇迹水晶]

Current: The comment grounded one crystal per pot in C# t.value without saying the clause itself does not state it.

Decision needed: Is a crystal added per 火罐 spent or per 消耗 event (a mixed temp+pool spend now raising two events)?

