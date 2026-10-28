# Rulebook / ABI backlog

**2026-10-27 audit.** Standing backlog for continuous iteration on rulebook
divergences, rule-implementation bugs and ABI problems. Fix agents take
independent batches from here; do not merge into the primary branch from this
file. (Originally opened 2026-10-10 as `rulebook-triage`; full recount
2026-10-17; card-rule TODO review + reconstruction 2026-10-20; 2026-10-27 card
sheet review + triage.)

On 2026-10-27 the 47 completed item sections and the closed-audit tail were
deleted. On 2026-10-28 every review row was triaged and merged by root cause
into A/B/C/D; the A set is fixed on branch `worktree-card-rulebook-review`, the
B set is grouped by ABI capability with one design per group, and the C set
lives as verbatim questions at the end. D-class rows were dropped.

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
- **D** — stale (already fixed / already landed)

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
**776 A items, fixed on this branch**; **65 B items** under 15 ABI capability
groups with a review design each in `docs/rulebook/spec/abi/`; **50 C
questions** at the end of this file (C-Q1–C-Q88, 12 reusing an existing
question); **1 D item** dropped. Items carry their sheet rows; a row merged
into two items is listed under both.

An A item with no new ABI is applied in place; the target ABI is the current
v103. B items are grouped by the capability that closes them — the groups are
the unit of approval, not the individual card. C items change no code until
ruled; their sites carry a `TODO(规则书)` pointing at the C-Q id.

## A — fixed on this branch (776 items, 1386 rows)

Applied and covered by the ruleset test suite. Every item keeps its sheet row in
`docs/rulebook/card-review-2026-10-27.json`; grouped by file, one slug per entry:

- **crates/card-sdk/src/ctx.rs** — anon-tokyo-pair-cap-owner (pair cap only refused a mark of the same owner, allowing a second crystal on the same pair); rimi-stay-latch-refused (A [停留] latch made the card unplayable although it grants [不可阻挡]); rimi-kani-tile-walk-bound (The Kani `ask_tile_walk` model rejected the card's 8 options)
- **crates/game-core/src/engine/ops.rs** — anon-tokyo-pair-cap-owner (pair cap only refused a mark of the same owner, allowing a second crystal on the same pair)
- **crates/game-rules/src/cond_pre.rs** — saaya-pass-latch-shared-stale (One per-player flag shared by copies and outliving the card); saaya-wrong-helper-and-slot-doc (Comments named `set_card_tile` and described the slot as a tile); dc-slotnames-register (dice_cast counter pre reads noCounteractTurn, which cond_pre never registers)
- **crates/game-rules/src/hostfns.rs** — anon-tokyo-pair-cap-owner (pair cap only refused a mark of the same owner, allowing a second crystal on the same pair)
- **docs/rulebook/spec/kani/r.md** — louder-ring-mult (louder comment and docs claimed the ring multiplier has no world slot)
- **rules/cards/card-ag/locales/en.json** — tomoe-locale-mortgage-state (Savior of Shopping Street locale names a stale mortgage state and a single arm); detour-counter-locale (Counter log hard-coded a reverse and a same-move 1d6)
- **rules/cards/card-ag/locales/zh-CN.json** — tomoe-locale-mortgage-state (Savior of Shopping Street locale names a stale mortgage state and a single arm); detour-counter-locale (Counter log hard-coded a reverse and a same-move 1d6)
- **rules/cards/card-ag/src/any_color_sunset.rs** — sunset-fire-choice-unconditional (Roll-4 fire/2000 choice was gated on a non-full pot); sunset-stray-hand-tag (Any Color Sunset declares a 手 tag the cell does not print); sunset-pick-order-todo (Roll-6 pick order loss is unrecorded)
- **rules/cards/card-ag/src/crimson_soul.rs** — crimson-place-before-ask (Crimson Soul asked and paid before placing, against 「然后选择」); crimson-separate-500-pays (Crimson Soul collapsed k [消耗]s of 500 into one pay); crimson-self-payment-500 (Self-payment lost the payee's 500); crimson-module-summary (Module blurb described only a payment cut); crimson-zero-delta-comment (CounterChanged comment claimed a zero delta means an empty placement); crimson-missing-antecedent-todo (Cell's truncated 「因导致的」 reading was unrecorded); crimson-drop-value-gt-zero (PaySplitChooseBefore refused a zero amount the book does not exclude); crimson-commit-after-comment (Comment justified gain-before-spend on a same-body discard)
- **rules/cards/card-ag/src/declare_war.rs** — declare-war-tile-citation (Tile half lacked a rulebook citation); declare-war-module-summary (Module summary dropped the tile half); declare-war-player-out-citation (Out-seat exclusion was uncited)
- **rules/cards/card-ag/src/detour.rs** — detour-extra-arm-no-reward (Counter path disarmed the CiRCLE reward for every plan); detour-unused-tile-seat (DetourPick tile/seat fields are always -1 and never read); detour-invoke-skill-todo (Inlined 视为使用 skips the skill body's costs/effects and was untagged); detour-counteract-citation (Clause (1) entry lacked a 规则书 citation); detour-no-reward-disarm (The CiRCLE veto was never disarmed); detour-event-order-comment (Comment claimed DiceRollAfter is the next event after DiceRollPlan); detour-apply-plan-citation (apply_plan implements the whole (2) without a citation)
- **rules/cards/card-ag/src/himari_plus_one.rs** — himari-any-roll-guard (Guard only admitted the move roll, not card-driven rolls); himari-g4-kinds-comment (G4 comment named the wrong kinds); himari-hook-twice-comment (Open question comment on the two hook kinds); himari-condition-citation (Trigger line was a bare quote, not a 规则书 citation); himari-any-roll-todo (Bare hedge TODO about roll kinds)
- **rules/cards/card-general/locales/en.json** — net-error-join-draw-optional (true-奈 join draw is mandatory although the clause says 「可抽一张卡」)
- **rules/cards/card-general/locales/zh-CN.json** — net-error-join-draw-optional (true-奈 join draw is mandatory although the clause says 「可抽一张卡」)
- **rules/cards/card-general/src/net_error.rs** — net-error-join-draw-optional (true-奈 join draw is mandatory although the clause says 「可抽一张卡」); net-error-recancel-cancelled-seat (A second net_error in the same window re-picks an already-cancelled designation); net-error-immune-tag-comment (Card-arm comment said the immune tags are unported and the cancel runs at the target window); net-error-own-seat-todo (Card guard refuses the play's own seat although the sheet does not); net-error-clause1-citation (Clause-1 guard cited the clause by number without the verbatim quote); net-error-effect-source-gate (Effect guard answers non-hand sources though clauses 1/2 name only 手卡's [手] effect); net-error-cancelled-trigger (Guard never checked Trigger.Cancelled and the TODO claimed it was impossible); net-error-missing-effect-arm (counteract has no Effect arm, so an effect-window answer is spent for nothing); net-error-dead-target-arm (The Target arm can never run and its comments described it as live)
- **rules/cards/card-general/src/parking_space.rs** — parking-space-owner-gate (House transfer is gated on the user owning Space, which the cell does not require); parking-space-settle-tile-todo (Settle replacement keys on the landing tile, not a set_settle_tile override); parking-space-leaves-play-todo (Deferred [停留] is lost if the card leaves play before the turn end); parking-space-cant-play-citation (cant_play implements the Space prerequisite without a clause citation); parking-space-overflow-houses (Houses left over when destinations run out stay on Space, unrecorded); parking-space-facedown-mark (skillBlock keeps blocking after the card is flipped face-down); parking-space-can-build-gate (Destinations are gated on can_build_on though the cell only says owned tiles); parking-space-registration-citation (Clause-(1) hook registrations carry no clause citation); parking-space-clause2-citation (Clause-(2) citation lacks the 规则书: prefix and sits away from the call)
- **rules/cards/card-general/src/perfect.rs** — perfect-add-vs-move-todo (Perfect's add-vs-move divergence was unrecorded); perfect-untagged-gain (PERFECT's 3000 is a bare literal, so CiRCLE's doubling is inert)
- **rules/cards/card-general/src/rain.rs** — rain-refused-target-reprompt (A refused target ends the designation loop short of X); rain-candidate-comment (Rain's comment claims the prompt pool filters exile); rain-user-through-target (The mandatory user is pushed without ctx::target, so cancellation never sees them)
- **rules/cards/card-general/src/thanks_party.rs** — thanks-party-paygate-todo (Pay-gated seats never see the 「可」 offer); thanks-party-others-comment (Comment blamed the filter on out-of-game seats, which ctx::others already drops); thanks-party-threshold-quote (Threshold citation and module summary still say 大于35 / > 35); thanks-party-untagged-payouts (Payouts are bare literals, so CiRCLE's doubling is inert); thanks-party-threshold-citation (The r < 35 threshold has no citation at the compare); thanks-party-out-player-todo (A payer who leaves mid-resolution silently loses the 1500)
- **rules/cards/card-mujica/locales/en.json** — loc-j11-targeted-prompt (j11 targeted prompt narrows 其他卡 to another player's card); loc-sparkler-log (sparkler turn-end log claims an extra turn even on the last removal)
- **rules/cards/card-mujica/locales/zh-CN.json** — loc-sparkler-log (sparkler turn-end log claims an extra turn even on the last removal)
- **rules/cards/card-mujica/src/black_birthday.rs** — bb-clause-citations (Black Birthday pay branches carry no rulebook citations); bb-wrong-quote (Black Birthday bracket quote says 在1000以下 instead of 不多于1000); bb-replay-stable-payers (Black Birthday re-reads ctx::others per replay, so a bankrupt payer skips the next payer); bb-out-payer-second-leg-todo (Black Birthday skips the second 200 once the payer is out without a TODO); bb-cant-play-todo (Black Birthday refuses a play with no other player but records neither citation nor TODO)
- **rules/cards/card-mujica/src/cant_look_away.rs** — cla-stale-todo (cant_look_away TODO understates the gap (claims only the counter path)); cla-selfmove-todo (cant_look_away self-move comment reads as implemented); cla-counteract-citation (cant_look_away counteraction clause lacked a conforming 规则书 citation); cla-forced-comment (Comments claim Forced = true maps onto ctx::plan::*); cla-cardused-spelling (cant_look_away comment names the retired skillUsed link kind)
- **rules/cards/card-mujica/src/crystal_swap.rs** — cs-destination-todo (crystal_swap destination set is unfiltered with no TODO at the omission site); cs-add-cap-todo (crystal_swap Slot::add prose admits the missing cap with no TODO tag); cs-facedown-todo (crystal_swap drops face-down cards with no citation and no TODO); cs-field-citation (crystal_swap slots() had no 规则书 citation for the field scope); cs-slots-doc-cap (crystal_swap slots() doc codifies the cap-blind reading)
- **rules/cards/card-mujica/src/dice_cast.rs** — dc-counter-turn-gate (dice_cast counter pre wrongly requires turn_player == owner); dc-placed-comment (dice_cast PLACED doc claims the follow-up reaches cast); dc-judgement-todo (dice_cast judgement TODO is garbled and misplaced in turn_end); dc-turnend-actor-gate (dice_cast discards only at the owner's turn end); dc-citations (dice_cast implementing lines lack 规则书 citations); dc-counter-mode-todo (dice_cast is a [反击] but the cell carries no marker)
- **rules/cards/card-mujica/src/doll_garden.rs** — dg-move-gate (doll_garden can_move reads stay/stun only, missing unstoppable, stunStart and exile); dg-sum-zero-main-move (doll_garden returns before any card_move when the sum is 0, leaving the main move and stale plan); dg-pay-gate-todo (doll_garden forced pay for a stunned/exiled victim is refused with no TODO)
- **rules/cards/card-mujica/src/heart_rain.rs** — heart-rain-refused-stay-counted-as-gained (心の雨 fallback counts a refused stay grant as a gain); heart-rain-stun-then-own-gain-todo (No site record for the stun-then-own-gain ruling (C-Q29))
- **rules/cards/card-mujica/src/j11.rs** — j11-stale-no-rulebook-entry-todo (#J11 TODO still claims the card has no rulebook entry); j11-missing-clause-citations (#J11 implementing lines carry C# names but no 规则书 citation); j11-targeted-by-card-guard (Targeted arm ignores the by_card >= 0 half and silently keeps the C# owner skip); j11-unprinted-payment-cancel (Payment is zeroed/cancelled though the cell only discards and stuns); j11-turn-end-owner-only (Crystal decay runs only at the owner's turn end though the cell says 每回合结束时); j11-cancel-doc-wrong-arm (pay_choose_cancel doc points at a nonexistent Effect arm of counteract)
- **rules/cards/card-mujica/src/mortis_instinct.rs** — mortis-citation-inserts-any (Citation inserts 任意 not present in the cell); mortis-todo-form-and-header (TODO lacks the （clause）: book -- us form; header overclaims)
- **rules/cards/card-mujica/src/nyamu_card.rs** — nyamu-citations-and-misleading-comments (Flip clauses have no citations and the header/comment claim they are observed)
- **rules/cards/card-mujica/src/saki_move.rs** — saki-forced-comment (Comment lists Forced = true as mapped onto ctx::plan but it is never set)
- **rules/cards/card-mygo/locales/en.json** — mygo-ordinary-log-wording (ordinary_log repeats the unqualified 'next main move' timing)
- **rules/cards/card-mygo/locales/zh-CN.json** — mygo-ordinary-log-wording (ordinary_log repeats the unqualified 'next main move' timing)
- **rules/cards/card-mygo/src/anon_tokyo.rs** — anon-tokyo-pair-cap-owner (pair cap only refused a mark of the same owner, allowing a second crystal on the same pair); anon-tokyo-adjacency-limit ([限] refusal for an empty adjacent-buyable list is undocumented); anon-tokyo-move-gate (clause-2 step is gated on move_blocked without a TODO); anon-tokyo-pay-before-refusal (half-deed 消耗 runs before place_mark_pair can refuse the placement); anon-tokyo-dead-link-guard (link_guard is unreferenced dead code with a stale doc)
- **rules/cards/card-mygo/src/endless_journey.rs** — endless-journey-passed-tiles (X uses the planned move length instead of the tiles actually entered); endless-journey-settle-order (body-replace suppression is order-dependent but stated unconditionally)
- **rules/cards/card-mygo/src/even_lost.rs** — even-lost (clause (2) is pressable with the main move spent and lacks citations)
- **rules/cards/card-mygo/src/haneoka.rs** — haneoka-quotes (roll comments misquote the inclusive 20 branch and the window's role); haneoka-build-gate (build branch hand-rolls the gate and logs a house that never rose)
- **rules/cards/card-mygo/src/hitoshizuku.rs** — hitoshizuku (locked-stay exception undocumented and the user's own payment leg skipped)
- **rules/cards/card-mygo/src/meet_again.rs** — meet-again (pre lacks the reachability TODO and the cap measures from the live face)
- **rules/cards/card-mygo/src/miracle.rs** — miracle-pass-record (record and reward were separate entries admitted against the pre-step world); miracle-band-crystal-drain (only the first band-skill instance's crystals were drained)
- **rules/cards/card-mygo/src/tomori_no_longer.rs** — hitoshizuku (locked-stay exception undocumented and the user's own payment leg skipped)
- **rules/cards/card-pp/src/infinite_possibility.rs** — inf-possibility-swap-copies (可能性为∞ (2) appends copies instead of exchanging the two piles); inf-possibility-drew-comment (Drew comment claimed once per draw batch); inf-possibility-clause2-comments (Clause (2) sites lacked a citation and described a copy as an exchange); inf-possibility-header (Header implied the card has no On::Play entry)
- **rules/cards/card-pp/src/jennifer.rs** — jennifer-player-out-todo (No inline TODO for the player_out escape); jennifer-user-storage ([使用者] stored in a per-player slot collides between copies); jennifer-empty-targets-gate (Unprinted whole-play refusal when no other player is present)
- **rules/cards/card-pp/src/no_expectation.rs** — noexpect-zero-face (An already-0 roll face never clears resolve); noexpect-roll-order-todo (No inline TODO for the reshape composition order); noexpect-citations (Payment-bend sites had no clause citation); noexpect-charges (「生效2次」 charges were never consumed and shared across copies); noexpect-kani-unproved (Kani exports exist but no proof drives them)
- **rules/cards/card-pp/src/overlapping_voices.rs** — overlap-move-gate-text (Gate TODO and comment scope); overlap-discard-scope ([特] fired on any owner discard, not only this card's own entry)
- **rules/cards/card-pp/src/ranger.rs** — ranger-discount-text (Comment claimed the engine refunds the 可溢出 half); ranger-field-count (「场上的卡」 count includes bound skill: instances); ranger-fans-doc (Fan doc explained only through C# helper names); ranger-range-display (Rejection message clamped the range the gate did not); ranger-crystal-floor (Removing 4 crystals silently floors at 0)
- **rules/cards/card-ppp/src/returns.rs** — returns-swap-unplaces-own-band (Returns [持续]（2） swap removed the owner's own PPP band card by name); returns-dead-nobuild4-token (Dead `returns.noBuild4` token with a comment claiming poppin reads it); returns-stale-header-todo-sentence (Header claimed [持续] machinery missing with a dangling '(TODO below)'); returns-sustained-citation-markers ([持续] citations lacked the `规则书:` marker); returns-sticker-tax-outlives-card ([持续]（3） sticker tax survived Returns leaving or flipping face down)
- **rules/cards/card-ppp/src/rimi_choco.rs** — rimi-move-does-not-settle (The walk could inherit `resolve: false` from a stale `turn.plan`); rimi-unstoppable-not-read-by-abnormal-gate (Comment claimed `abnormal_gate` honours [不可阻挡]; it never reads it); rimi-unstoppable-window-todo (「期间」 window outliving the move was unrecorded); rimi-main-move-gate-citation (The main-move replacement gate had only a C# explanation); rimi-stay-latch-refused (A [停留] latch made the card unplayable although it grants [不可阻挡]); rimi-expiry-clobber (`set_expires` shortened an existing longer [不可阻挡] grant); rimi-enter-move-phase-citation (「立刻进入移动阶段」 uncited at the `card_move` call); rimi-kani-tile-walk-bound (The Kani `ask_tile_walk` model rejected the card's 8 options)
- **rules/cards/card-ppp/src/saaya_sky.rs** — saaya-payout-needs-card-present (A pass was dropped when the card left before turn end); saaya-pass-latch-shared-stale (One per-player flag shared by copies and outliving the card); saaya-divergence-todos (No TODO(规则书) recorded the latch divergences); saaya-wrong-helper-and-slot-doc (Comments named `set_card_tile` and described the slot as a tile)
- **rules/cards/card-ppp/src/signpost.rs** — signpost-clause-citations (The 1000-loss and main-move clauses uncited at their implementing lines); signpost-out-gate-todo (Unprinted `owner.out == 0` gate on the unconditional turn-end loss)
- **rules/cards/card-ppp/src/star_beat.rs** — star-beat-dead-settle-hook (SettleAfter never dispatched: the played card went to the graveyard); star-beat-missing-citation-markers (Grant, gate and build sites cited without `规则书:` or at all); star-beat-divergence-todo (No TODO(规则书) for the carrier and hidden-layer divergences); star-beat-double-tile-prompt (Two tile prompts for one 盖房); star-beat-entry-overclaim (Entry comment overstated affordability and reachability)
- **rules/cards/card-ppp/src/tae_sound.rs** — tae-teleport-citation (Teleport half uncited at the lookup/teleport); tae-fire-citation (Fire half uncited at the gain); tae-header-paraphrase (Header paraphrased the ruling as 'Instruments (no settle)'); tae-absent-tile-todo (Silent skip when the shop tile is absent was unrecorded)
- **rules/kani/src/host.rs** — mor-host-model-notes (Mor courage notes still describe the old 1-based / land-only host model)
- **rules/kani/src/invoke.rs** — aya-filter-not-invoked (money_shave ran the On::Action hook, never the PaySplitAdd filter body)
- **rules/kani/src/ir_fill.rs** — aya-filter-not-invoked (money_shave ran the On::Action hook, never the PaySplitAdd filter body)
- **rules/kani/src/proofs/ag.rs** — crimson-place-before-ask (Crimson Soul asked and paid before placing, against 「然后选择」); crimson-separate-500-pays (Crimson Soul collapsed k [消耗]s of 500 into one pay)
- **rules/kani/src/proofs/general.rs** — harvest-doubling-bound (harvest step bound ignored doubling and blamed the wrong stub)
- **rules/kani/src/proofs/money_shave.rs** — aya-filter-not-invoked (money_shave ran the On::Action hook, never the PaySplitAdd filter body)
- **rules/kani/src/proofs/mor.rs** — mor-host-model-notes (Mor courage notes still describe the old 1-based / land-only host model); courage-roll-not-pinned (courage-02 never tied the payout to the rolled tile face)
- **rules/kani/src/proofs/mygo.rs** — mygo-stay-and-narration (mygo specs under-asserted stay/stun, omitted the self-pay, and misdescribed haneoka logging)
- **rules/kani/src/proofs/pp.rs** — ranger-self-count (ranger proof omitted the Ranger itself and left the clause helpers dead)
- **rules/kani/src/proofs/ppp.rs** — ppp-comment-fixes (ppp comments claimed a missing cap clamp and cited a quote absent from the cell)
- **rules/kani/src/proofs/sumimi.rs** — sumimi-model (sumimi proof asserted set semantics over a bare gain and never modelled a third persona)
- **rules/kani/src/spec/ag.rs** — crimson-place-before-ask (Crimson Soul asked and paid before placing, against 「然后选择」)
- **rules/kani/src/spec/general.rs** — harvest-doubling-bound (harvest step bound ignored doubling and blamed the wrong stub)
- **rules/kani/src/spec/mor.rs** — mor-host-model-notes (Mor courage notes still describe the old 1-based / land-only host model); courage-roll-not-pinned (courage-02 never tied the payout to the rolled tile face); rui-trailing-setstate (rui_demon spec baked the unconditional trailing SetState into the expected shape)
- **rules/kani/src/spec/mygo.rs** — mygo-stay-and-narration (mygo specs under-asserted stay/stun, omitted the self-pay, and misdescribed haneoka logging)
- **rules/kani/src/spec/pp.rs** — ranger-self-count (ranger proof omitted the Ranger itself and left the clause helpers dead)
- **rules/kani/src/spec/r.rs** — louder-ring-mult (louder comment and docs claimed the ring multiplier has no world slot)
- **rules/kani/src/spec/sumimi.rs** — sumimi-model (sumimi proof asserted set semantics over a bare gain and never modelled a third persona)
- **rules/skills/skill-groups/src/lib.rs** — returns-sticker-tax-outlives-card ([持续]（3） sticker tax survived Returns leaving or flipping face down)
- **rules/tests/tests/ag.rs** — ag-same-sky-quote (Same-sky test citation drops the opening-hand clause)
- **rules/tests/tests/cross_move.rs** — cross-move-want-human-fixture (want_human fixture: stale quote, wrong X assumption, no X declared)
- **rules/tests/tests/general.rs** — general-35-threshold (Stale 「大于35」 threshold in declaration-war citations); general-cp-renumber (Stale clause number （2） after the sheet renumbered to （1）); general-cpchanged-rename (Comment names the removed ActionKind::CpChanged)
- **rules/tests/tests/guards.rs** — guards-self-payment (Guard test never proves a self payment is excluded)
- **rules/tests/tests/hhw.rs** — hhw-dice-sum-tile (3d20 is a sum, so the free-house tile was wrong)
- **rules/tests/tests/mujica.rs** — mujica-quote-verbatim (Quote inserts 任意 and drops the placement exclusion)
- **rules/tests/tests/pp.rs** — pp-overlap-docstring (Overlap test docstring drops the move-stage entry)
- **rules/tests/tests/ras.rs** — ras-hey-kids-split-todo (Hey Kids fixture embeds the unimplemented split clause without a TODO); ras-hey-kids-fee-empty-target (No fee test puts houses on the moved-away tile)
- **rules/tests/tests/roselia.rs** — roselia-fire-bird-comment (Fire bird comment implies a per-tile tick); roselia-lisa-discard (Lisa link test never asserts the card is discarded)
- **rules/tests/tests/spec_card_ag.rs** — ag-half-speed-other-turn (Owner-only TurnEnd trigger never sees another seat's turn end)
- **rules/tests/tests/spec_card_general.rs** — general-perfect-fever (Perfect contract hand never holds a FEVER! to preserve)
- **rules/tests/tests/spec_card_hhw.rs** — hhw-wacha-jump-position (Wacha jump position assertion is vacuous on the card's own tile)
- **rules/tests/tests/spec_card_mor.rs** — mor-rui-x-no-skill (Rui devil no-skill branch never checks X is untouched)
- **rules/tests/tests/spec_card_pp.rs** — pp-title-idol-seat (Title-idol fixture sits a Poppin' Party seat, so no [共鸣]); pp-first-accident-skills (First-accident spec still describes the old all-PP lock and offers no skills)
- **rules/tests/tests/spec_card_r.rs** — r-resolve-removed-pays (Resolve removal test never proves the card was actually [移除]d); r-own-stage-inert-answer (Own-stage tests use a non-teleporting fixture and never probe the answer)
- **rules/tests/tests/sumimi.rs** — sumimi-clause-number (Sumimi comment cites a nonexistent （3）)
- **crates/game-core/src/engine/cx.rs** — extreme-not-read-by-card-rolls (an armed forced extreme was ignored by every card effect roll); extreme-unreached-rolls (Forced extreme never reached card rolls (CAN effect a no-op; move rolls ignored it))
- **crates/game-core/src/engine/ops.rs** — karaoke-fixed-roll-stale (karaoke: stale fixed_roll hijacks a later card's dice); louder-rent-unit-wiring (louder's +5 to 价格基础乘数 was read by no rent path)
- **crates/game-core/src/engine/play.rs** — debut-locked-spend (debut_success: a locked payment aborted buy/build); ordinary-lastwalk-written-by-every-walk (`lastWalk` was written by every non-teleport walk, not just main moves); extreme-unreached-rolls (Forced extreme never reached card rolls (CAN effect a no-op; move rolls ignored it)); lead-cancelled-move-followers (Followers still walk when the shared main move is cancelled by a moveBefore counteraction); before-live-teleport-resolve (set_resolve from a passTile counteraction is discarded on a teleport); ordinary-lastwalk-actual-distance (next_steps consumed by non-main rolls and lastWalk recording the planned length); louder-rent-unit-wiring (louder's +5 to 价格基础乘数 was read by no rent path); childhood-plan-start-one-shot (plan.start outlived 「本次移动」); here-the-world-counter-plays (A counteraction never counts as a played card)
- **crates/game-core/src/engine/play/purchase.rs** — louder-rent-unit-wiring (louder's +5 to 价格基础乘数 was read by no rent path)
- **crates/game-rules/src/cond_pre.rs** — mujica-dice-cast-stale-slot-name (SLOT_NAMES still registered the dead `diceCastActive` name for the 骰子已经掷下 lock)
- **crates/game-rules/src/wasm_rules.rs** — maze-buyassign-writeback (BuyAssign raze never lands: counteract write-back drops trigger.buy); sumimi-mana-champion-held-arm (真奈 join redirect armed for a hand holder that never plays); mor-summer-camp-tile-untargetable (target_tile skipped `Untargetable` for the tile's owner (夏日合宿)); exist-self-target-redirect (Another player's self-targeted card never reached EXIST's redirect)
- **docs/rulebook/BACKLOG.md** — backlog-loyalty-stale-sections (BACKLOG loyalty entries describe code that later commits already fixed); ch05-mortis-quote (CH-05 adds an 任意 the Mortis cell does not print)
- **docs/rulebook/COVERAGE.md** — coverage-overstated-ok (COVERAGE marks clauses ok although the cited tests do not pin them); coverage-wrong-refs (COVERAGE rows cite wrong formulas and test names that do not exist)
- **docs/rulebook/SETTLE-STAGES.md** — settle-stages-proud-light (SETTLE-STAGES names ChainKind::PassPlayer for proud_light)
- **docs/rulebook/TEST-FINDINGS.md** — test-findings-encore (TEST-FINDINGS still lists the fixed encore gap)
- **docs/rulebook/spec/FINDINGS.md** — look-away-stale-move-claim (Look-away docs claim the engine moves every other player)
- **docs/rulebook/spec/cards/crychic.md** — spec-quotes-and-conditions (Spec text contradicts the cell or the landed code)
- **docs/rulebook/spec/cards/mor.md** — spec-quotes-and-conditions (Spec text contradicts the cell or the landed code)
- **docs/rulebook/spec/cards/mujica.md** — look-away-stale-move-claim (Look-away docs claim the engine moves every other player)
- **docs/rulebook/spec/cards/r.md** — spec-quotes-and-conditions (Spec text contradicts the cell or the landed code)
- **docs/rulebook/spec/kani/mor.md** — kani-courage-blocked-host (kani/mor courage-01 mislabels a harness gap as BLOCKED-HOST)
- **docs/rulebook/spec/kani/mujica.md** — look-away-stale-move-claim (Look-away docs claim the engine moves every other player)
- **rules/bandori-std/src/fire.rs** — fire-substitution-window-before-temp (Crystal substitution window must cover the whole fire cost, before any temp pot burns); l12-temp-spend-credits (Temp-pot spends raised no markerSpent)
- **rules/bandori-std/src/lib.rs** — rent-bonus-doc-and-return (add_rent_unit_bonus documented as returning the multiplier but returned the bonus); louder-rent-unit-wiring (louder's +5 to 价格基础乘数 was read by no rent path)
- **rules/cards/card-ag/locales/en.json** — tomoe-when-locale (counter_reason claimed another player mortgaged even on the self arm)
- **rules/cards/card-ag/locales/zh-CN.json** — tomoe-when-locale (counter_reason claimed another player mortgaged even on the self arm)
- **rules/cards/card-ag/src/crimson_soul.rs** — crimson-cardused-entry-filter (CardUsed admits any owner field press, not only a skill press)
- **rules/cards/card-ag/src/detour.rs** — detour-counter-linger (A counter's card is never placed, so its dice-roll hooks stay dead)
- **rules/cards/card-ag/src/moca_half.rs** — moca-turn-end-owner-gate (MOCA: the crystal tick must fire only at the owner's turn end); moca-g02-stale-comment (MOCA: roll_half doc quotes a g02 comment that has been rewritten); moca-final-roll-stage (MOCA halves before additive roll modifiers; DiceRollAfter is the final stage)
- **rules/cards/card-ag/src/one_of_us.rs** — one-of-us-quote-typo (ONE OF US: citations say 次地契 instead of 此地契); one-of-us-comment-traceability (ONE OF US: implementing sites lacked clause citations / accurate comments); one-of-us-sold-deed-split (ONE OF US: a designated deed sold to a third party stopped splitting); one-of-us-duplicate-copy-state (ONE OF US: pair/rent/refs overwritten by a second copy (player state → instance props)); one-of-us-handover-todo (ONE OF US: no site record of the hand-over divergences)
- **rules/cards/card-ag/src/proud_light.rs** — proud-light-pass-step-comment (PROUD LIGHT: comment calls the pass step 'mid-route'); proud-light-multi-occupant-todo (PROUD LIGHT: multi-occupant pick unrecorded); proud-light-pass-per-tile (PROUD LIGHT: passTile is one raise per tile, so extra occupants are ignored)
- **rules/cards/card-ag/src/ran_as_usual.rs** — ran-self-skill-todo-stale (RAN AS USUAL: stale TODO claiming self-applied abnormals get no window); ran-return-through-gate (RAN AS USUAL: the return teleport ran the abnormal gate and could be refused); ran-counteract-citation (RAN AS USUAL: [反击] registration uncited); ran-missing-tags (RAN AS USUAL: [反击] (and 手) tags missing); ran-at-end-doc (RAN AS USUAL: at_end doc says the restore body is held); ran-restore-incomplete (RAN AS USUAL: undo restores only position + three counters)
- **rules/cards/card-ag/src/same_sky.rs** — same-sky-double-discard (SAME SKY: take_card + to_discard ate a second held copy); same-sky-quote-drift (SAME SKY: quote says 大于等于3); same-sky-discard-citation (SAME SKY: the played-to-discard move was uncited); same-sky-stale-sketch (SAME SKY: doc sketch does not match the code (ActionKind slice + Some))
- **rules/cards/card-ag/src/shop_friends.rs** — shop-friends-gate-comment (SHOP FRIENDS: gate comment over-broad on the skip half); shop-friends-ownership-citation (SHOP FRIENDS: ownership half of the clause uncited at the filter); shop-friends-settle-todo (SHOP FRIENDS: settle-on-arrival stated as fact)
- **rules/cards/card-ag/src/sunset.rs** — sunset-designates-prop (SUNSET's play designates tiles and players but set no DESIGNATES prop); sunset-guard-citations (play_guard's two clause checks carried no 规则书 citation); sunset-fewer-than-four (fewer-than-four reading unrecorded; take-all comment described an unreachable case); sunset-spliced-quote (Spliced citation 「[指定][使用者]以外的所有玩家」 is not contiguous); sunset-half-rounding (pre-round-100 halving rounds up to 10 with no record); sunset-tile-targeting (tile picks never ran the targeting gate, so NO_TARGET-marked tiles were still designated/demolished); sunset-zero-others (zero-other guard (and body early return) skip step 1 although the cell makes no such condition)
- **rules/cards/card-ag/src/tomoe_savior.rs** — tomoe-acquire-todo (acquire site had no TODO for the 收购-vs-购买 pipeline differences); tomoe-counteract-citations (trigger / money / self-arm lines carried no citation); tomoe-self-precondition (extra still-mortgaged precondition on the self arm was unrecorded)
- **rules/cards/card-ag/src/tsugumi_can.rs** — extreme-not-read-by-card-rolls (an armed forced extreme was ignored by every card effect roll); ycm-not-an-exclusion (the ycm early-return made the one named exception the one card that could not be countered); broken-abi-todo (spliced TODO sentence and a false 'the filter is what keeps them apart'); tsugumi-cant-play-citation (cant_play attributed the gate to effect (1) and quoted non-sheet text); play-fires-both-effects (playing the card fires (2) and also auto-runs (1); the sheet says choose one)
- **rules/cards/card-ag/src/yolo.rs** — extreme-not-read-by-card-rolls (an armed forced extreme was ignored by every card effect roll); yolo-citations (no 规则书 citation anywhere in the file); yolo-kinds-comment (G4 comment named the filter kind DiceRollValue instead of the trigger kinds); yolo-fixed-face-todo (added +1d4 over a face fixed by another effect was unrecorded at the site)
- **rules/cards/card-crychic/locales/en.json** — tomori-optional-reset (Optional X reset fired on timeout/empty answer); locale-wording (locales: wrong X bound and per-turn abnormal wording)
- **rules/cards/card-crychic/locales/zh-CN.json** — tomori-optional-reset (Optional X reset fired on timeout/empty answer); locale-wording (locales: wrong X bound and per-turn abnormal wording)
- **rules/cards/card-crychic/src/debut_success.rs** — debut-citations (debut_success: truncated clause quote and C#-only at_end doc)
- **rules/cards/card-crychic/src/elegant_shout.rs** — elegant-shout-citations (elegant_shout: registrations carry no clause citations)
- **rules/cards/card-crychic/src/fate_together.rs** — fate-hit-bookkeeping (fate_together (2): player-shared hit bit and lost hit on the replay pass); fate-collection-todo (fate_together: rent-only narrowing unmarked at the guard); fate-no-tile-ahead-todo (fate_together: no-tile-ahead behaviour unmarked); fate-citations (fate_together: missing citations at registrations, hit write and full (2) quote); crychic-band-3-dead-drawn-hook (CRYCHIC band (3) is registered on the drawn-card-only `Drawn` and never fires for the placed skill)
- **rules/cards/card-crychic/src/forever.rs** — forever-comments (forever: draw-only expiry comment and missing (1) citation)
- **rules/cards/card-crychic/src/haruhikage.rs** — haruhikage-settle-arms (haruhikage SettleBefore arm: owner-only pin, no pot cost, no other-mover variant); haruhikage-skill-pick (haruhikage: offer only skills with an On::Play); haruhikage-special-route (haruhikage [特]: (2) not used; interrupt watched the wrong signal); haruhikage-soyo-latch (haruhikage soyo arm: teleport latch never armed and stale TODO); haruhikage-citation (haruhikage: no citation at the play body)
- **rules/cards/card-crychic/src/karaoke.rs** — karaoke-movement-roll (karaoke: faces rolled as Effect 1d20 instead of the movement roll); karaoke-fallback-comment (karaoke: pick comment claimed the fallback is the highest face)
- **rules/cards/card-crychic/src/mutsumi_never.rs** — mutsumi-offer-unconditional (mutsumi_never (1): (2) was hidden without a band skill); mutsumi-empty-hand-discard (mutsumi_never (2): discard skipped silently on an empty hand)
- **rules/cards/card-crychic/src/my_own_problem.rs** — myown-comments (my_own_problem: nearest scan uncited; 不生效 doc misframed)
- **rules/cards/card-crychic/src/sakiko_lead.rs** — sakiko-overlap-citation (Overlap gate lacked its own 规则书 citation)
- **rules/cards/card-crychic/src/soyo_back.rs** — soyo-build-leg (Build leg bypassed the build path and offered unbuildable tiles); soyo-take-card-citation (take_card line uncited)
- **rules/cards/card-crychic/src/taki_even_if.rs** — taki-reroll-until-new (Reroll was silently capped at ten even with a duplicate face); taki-applicability-citation (Counteraction window/precondition uncited); taki-main-move-window (Card offered on card-driven rolls outside the move phase); taki-turn-rolls-comment (Comment claimed every face this turn was in the list); taki-extreme-reroll (Reroll now honours an armed extreme)
- **rules/cards/card-crychic/src/tomori_inner_shout.rs** — tomori-optional-reset (Optional X reset fired on timeout/empty answer); tomori-fire-pool-gate (Pre required pool fire, blocking crystal/temp substitution); tomori-landing-condition (Mover's landing near you was never checked); tomori-2-citation-markers ((2) pointers lacked the 规则书 marker); tomori-band-skill-gate ((3) gated on the persona group, which never becomes MyGO!!!!!); tomori-latch-per-instance (Latch/half keyed on the mover, shared by two copies); tomori-stale-x (Stale X paid out with no 想要成为人类 in play); tomori-half-todo-reframe (Half-pay TODO framed a non-gap as unsettled)
- **rules/cards/card-crychic/src/vocal_too_hard.rs** — vocal-self-pay (Condition accepted a self-payment); vocal-trigger-citation (Trigger condition uncited)
- **rules/cards/card-crychic/src/want_human.rs** — want-human-armed-zero (An armed +0 (X = 20) read as unarmed)
- **rules/cards/card-crychic/src/want_to_grab.rs** — want_to_grab-field-placement (Play body never places the card, so every hook is dead); want_to_grab-window-expiry (Nothing closes the clause (2)/(3) window at the owner's turn start); want_to_grab-first-passer-latch (The latch re-armed after each grab, so every later passer fired again); want_to_grab-pass-owner-gate (PassPlayer pre dropped the owner-position gate); want_to_grab-citations (Clause (2) hook registrations and walk lines lacked 规则书 citations); want_to_grab-stale-todo (Clause (3) TODO claimed set_teleport_to was missing); want_to_grab-module-doc (Module doc pointed at TODOs that did not record the held pieces)
- **rules/cards/card-general/locales/en.json** — thanks-party-35-boundary (THANKS PARTY logs still say 超过 35 / > 35 for an inclusive-35 result); thanks-party-extra-log (Extra-turn log claims nobody joined); marina-log-tile-text (Log text dropped #1格子 and 影响)
- **rules/cards/card-general/locales/zh-CN.json** — thanks-party-35-boundary (THANKS PARTY logs still say 超过 35 / > 35 for an inclusive-35 result); thanks-party-extra-log (Extra-turn log claims nobody joined); marina-log-tile-text (Log text dropped #1格子 and 影响)
- **rules/cards/card-general/src/budokan.rs** — budokan-y-unused (budokan never names the printed Y)
- **rules/cards/card-general/src/clear_cp.rs** — clear-cp-no-free-tile (clear_cp discards itself without the six on-card CP when no tile is free); clear-cp-play-gate-todo (Copy-in-play gate is a ruling the sheet does not carry and sat untagged); clear-cp-graveyard-todo (Graveyard-on-empty is a ruling the cell never prints); clear-cp-special-tag (The 特 tag asserts a section the sheet dropped); clear-cp-spread-duration-citation (The printed 2-turn duration had only a C# note); clear-cp-neighbour-tiebreak (Unrecorded arbitrary neighbour choice when both neighbours are empty)
- **rules/cards/card-general/src/effort.rs** — effort-doubling-slot (The printed 6 is tagged with an unreachable doubling slot 0)
- **rules/cards/card-general/src/encore.rs** — encore-guard-citation (The counteract trigger clause lost its citation and pay-pipeline note); encore-log-narrowing-todo (Log narrowing note was prose and misnamed the blocker); encore-activation-negation (Encore used the weaker effect negation for a full invalidation)
- **rules/cards/card-general/src/fever.rs** — fever-doubled-comment (Comment claimed doubling is unported and the read equivalent to C#'s Mem); fever-doubling-slot (The 600 is tagged with an unreachable doubling slot 0)
- **rules/cards/card-general/src/gacha10.rs** — gacha10-unconditional-draw (10次招募 skips the printed draw when the payment settles 0); gacha10-once-limit (「1回限定」 is unenforced: no gate and no record)
- **rules/cards/card-general/src/marina_work.rs** — marina-tile-id-comment (Comment falsely says the trigger carries no tile id); marina-citations (Registration and card typing carried no citation); marina-log-quote (Line 37 quoted a paraphrase as if it were the log)
- **rules/cards/card-general/src/perfect.rs** — perfect-add-only-discard (Create-only graveyard push now exists; perfect already uses it)
- **rules/cards/card-general/src/tsugu_ycm.rs** — tsugu-divergence-todos (Tsugu carries no TODO(规则书) at its two acknowledged divergences); tsugu-untagged-gain (Both (4) 1000 gains are bare literals, so the CiRCLE doubling never reaches them); tsugu-buy-candidates-quote (Buy candidates use the pre-stage native price and ignore BuyGate eligibility); tsugu-expensive-tile-default (Comment claims a most-expensive-tile default that the code did not provide)
- **rules/cards/card-hhw/locales/en.json** — backstage-reveal-pair (backstage_tour: first keep/flip decision made before the second card is named); balloon-record-all-rolls (balloon_show: the four results are neither logged nor all offered); kokoro-log-amount (settle log hard-coded 6,000 regardless of what moved)
- **rules/cards/card-hhw/locales/zh-CN.json** — backstage-reveal-pair (backstage_tour: first keep/flip decision made before the second card is named); balloon-record-all-rolls (balloon_show: the four results are neither logged nor all offered); kokoro-log-amount (settle log hard-coded 6,000 regardless of what moved)
- **rules/cards/card-hhw/src/backstage_tour.rs** — backstage-reshuffle-hook (backstage_tour: the short-pile reshuffle raises no `reshuffled`); backstage-reshuffle-order (backstage_tour: discard re-inserted in the inverse of the quoted C# order); backstage-short-pile-note (backstage_tour: unprinted short-pile reshuffle carried no rule book note); backstage-reveal-pair (backstage_tour: first keep/flip decision made before the second card is named)
- **rules/cards/card-hhw/src/balloon_show.rs** — balloon-mod60-citation (balloon_show: citation drops 「mod60」 and the 序号 conversion is uncited); balloon-record-all-rolls (balloon_show: the four results are neither logged nor all offered)
- **rules/cards/card-hhw/src/believe_you.rs** — believe-hand-count (believe_you: a second copy of the card counts as no other hand card); believe-paid-gate-todo (believe_you: unprinted full-payment gate had no TODO); believe-rival-citation (believe_you: rival guard carries no rule book citation); believe-reshuffle (believe_you: 重洗你的抽牌堆 refilled from the discard and raised no hook); believe-chooser-self-todo (believe_you: the self-eligibility question is unrecorded)
- **rules/cards/card-hhw/src/black_suits.rs** — black-curly-quotes (black_suits: citations use ASCII quotes where the cell uses “”); black-mark-kind (black_suits: the placed crystal is labelled with the card name); black-borrow-todo (black_suits: borrowed-effect divergence carried no TODO at all); black-pass-hook-comment (black_suits: comment names a `PassTile` hook that does not exist)
- **rules/cards/card-hhw/src/charity_show.rs** — charity-pay-mul-quote (charity_show: pay citation drops 「（向上取整10）」); charity-mine-comment (charity_show: CiRCLE comment names an API the code does not use); charity-extra-steps-todo (charity_show: the TODO block is garbled and mistagged); charity-seen-comment (charity_show: a `///` describing a prop attaches to the CardDef)
- **rules/cards/card-hhw/src/dream_return.rs** — dream-target-narrowing (dream_return: the mortgage note is labelled rule book and the filters are unrecorded); dream-emulation-comment (dream_return: comment credits the band skill with a double it never runs); dream-success-gate-citation (dream_return: the success gate explains itself only through C#); dream-amount-todo (dream_return: the payment amount basis is unrecorded); dream-crystal-binding-todo (dream_return: the two crystals are one unbound pool); dream-relief-todo (dream_return: relief base pre- or post-double is unrecorded)
- **rules/cards/card-hhw/src/hagumi_marks.rs** — hagumi-remove-one-mark (A stop spends one 育美标记, not every mark on the tile); hagumi-gain-before-remove (Gain 2000 before spending the mark); hagumi-standin-instance-order (Place the stand-in before the mark so it binds to a real instance); hagumi-teleport-pass-tile (The stop hook also dispatches at a teleport destination); hagumi-literal-60 (The >60 branch uses the sheet's literal 60); hagumi-summary-over60 (Module summary omits the >60 branch); hagumi-abnormal-gate-todo (Abnormal gate skips the 2000 and removal with no TODO); hagumi-roll-count-todo (「投掷次数+1」 reading is unruled); hagumi-option-index (The hagumi ask is per tile, so two rolls on one tile lose an outcome)
- **rules/cards/card-hhw/src/happy_lucky.rs** — happy-drawn-board-geometry (Farthest-tile scan must use the drawn 12x10 fold, not the 16x16 ring); happy-board-size-guard (The 60-tile guard silently no-ops on other boards)
- **rules/cards/card-hhw/src/kanon_march.rs** — kanon-jump-runs-as-main-move (The jump is the main move and must run immediately); kanon-jump-move-gate (The jump is refused while the main move is shut); kanon-play-origin-branch (Branch on the play's origin, not is_placed); kanon-clause-citations (Missing 规则书 citations on stage, mark and teleport clauses); kanon-verbatim-quotes (Doc quotes drifted from the sheet text); kanon-moveafter-once-per-walk (One mark per completed backward walk, not per tile); kanon-dest-index-fallback (Fall back to the printed #4/#30 when tile names miss); kanon-placement-comment (Gate doc overstated that placement is always available)
- **rules/cards/card-hhw/src/kaoru_thief.rs** — kaoru-dice-plan-scope (kaoru DiceRollPlan rewrite had no 主要移动 scope and keyed on the turn player); kaoru-applicability-guards (kaoru dice rewrite ran for movers who cannot move and for exiled pieces); kaoru-counter-memo (kaoru mark memo lived in per-player slots, not the card instance); kaoru-one-player (unprinted 'one player' restriction on the mark); kaoru-charge-name (充能 charge counter name is an assumption); kaoru-quote (bracketed [强制停下] inside a 规则书 quote)
- **rules/cards/card-hhw/src/kokoro_circle.rs** — kokoro-facedown-gate (play gate counted a face-down copy that produces no effect); kokoro-shortpay-todo (stale TODO quoted 「视为此卡未生效」, absent from this cell); kokoro-log-amount (settle log hard-coded 6,000 regardless of what moved); kokoro-one-copy-gate (unprinted one-copy and seat-wide play gate)
- **rules/cards/card-hhw/src/misaki_card.rs** — misaki-window-cite (timing half of the window clause uncited); misaki-span-cite (`规则书:` labelled a C# host routine); misaki-between-cite (`between(owner) > 0` conjunct had no citation); misaki-log-tile (move log named the chosen tile although the piece never goes there); misaki-all-fire (「消耗所有火罐」 spent only the capped pool, never temp pots)
- **rules/cards/card-hhw/src/smile_parade.rs** — parade-own-redirect-latch (笑容大游行 (3): only this card's own redirect may be exempt, not every card settle); parade-2-any-settler-any-move (笑容大游行 (2): offered only to the owner and never after a teleport); parade-1-citation-passTile (笑容大游行 (1): no clause citation at the counteract and a walk-only chain kind); parade-3-settle-actor (笑容大游行 (3): redirected settle ran as the card owner, not the player who landed); parade-stale-engine-claims (笑容大游行: false engine claims (first-hook-only, settle_instead))
- **rules/cards/card-hhw/src/smile_patrol.rs** — patrol-header-free-tile (微笑巡逻队 header binds the free house to 'that tile'); patrol-citation-and-play-gate (微笑巡逻队: unquoted 规则书 line and an unrecorded play gate); patrol-payment-amount (微笑巡逻队: adopted payment amount unrecorded); patrol-3d20-reading (微笑巡逻队: sum-vs-three-faces 3d20 reading unrecorded)
- **rules/cards/card-hhw/src/sports_talent.rs** — talent-reward-per-reroll (运动的天赋: a re-roll pays nothing); talent-fixed-face-guard (运动的天赋: roll_after_applies never refused and the comments claimed it did); talent-stale-quote (运动的天赋: citations quote superseded 「结果为10以上为止」); talent-rollplan-target (运动的天赋: clamp gate keyed on the turn player, not the mover); talent-turn-end-scope (运动的天赋: decay only at the owner's turn end); talent-counteract-name (运动的天赋: comments link a `counteract` fn that does not exist); talent-reward-citation (运动的天赋: implementing-site citation missing on reward()); talent-slot-not-reset (运动的天赋: the reward ladder survives the card instance); talent-reroll-cap (运动的天赋: arbitrary 10-attempt cap on 「直至」)
- **rules/cards/card-morfonica/locales/en.json** — rainbow-reason (The counteraction prompt only shows clause (1)'s direction); nanami-press-gate-and-comment (稍微努力了一下 press can burn a crystal with no mark in hand); nanami-effect1-label (稍微努力了一下 option (1) label says the draw happens at turn end); nanami-note-locale-key (nanami_effort_note is undefined and serves two meanings)
- **rules/cards/card-morfonica/locales/zh-CN.json** — rainbow-reason (The counteraction prompt only shows clause (1)'s direction); nanami-press-gate-and-comment (稍微努力了一下 press can burn a crystal with no mark in hand); nanami-effect1-label (稍微努力了一下 option (1) label says the draw happens at turn end); nanami-note-locale-key (nanami_effort_note is undefined and serves two meanings); tsukushi-guard-log (迷茫的庭园 guard log drops 你的 from the window)
- **rules/cards/card-morfonica/src/centrifugal.rs** — centrifugal-gate-citation-and-readings (离心力 gate lacks the window citation and the second-time reading note); centrifugal-summary-hostile (离心力 header calls every targeting hostile); centrifugal-immunity-citations (离心力 immunity body and TurnStart registration lack clause citations); centrifugal-immuneall-timing-doc (离心力 doc says ImmuneAll is checked before targeting); centrifugal-immuneall-own-effects (离心力 shield only covers other players' effects)
- **rules/cards/card-morfonica/src/hold_hands_again.rs** — hold-hands-payafter-discard (再次牵起手来 discards on every payAfter, even when the filter never cancelled); hold-hands-place-order (再次牵起手来 spends before placing and skips placement on bankruptcy); hold-hands-counter-citations (再次牵起手来 counteraction pre and persist entry lack citations); hold-hands-persist-threshold (再次牵起手来 [持续] carries a 大于0 guard the clause lacks)
- **rules/cards/card-morfonica/src/mashiro_pay.rs** — mashiro-play-condition-citation (小白 play condition has no clause citation); mashiro-log-who-to (小白 log renders payer and payee backwards); mashiro-paid-in-settle (小白's bank redirect still counts as a payment to another player)
- **rules/cards/card-morfonica/src/nanami_effort.rs** — nanami-press-waiver (稍微努力了一下 press waives neither count nor CD); nanami-crystal-quote (稍微努力了一下 (3) quote inserts an invented 为其); nanami-effect-draw-dead-code (effect_draw marked unreachable and dead code); nanami-effect1-quote (稍微努力了一下 (1) quote is a rewritten composite); nanami-circle-gain-pass (稍微努力了一下 (2) pays the CiRCLE reward as an ordinary gain); nanami-press-gate-and-comment (稍微努力了一下 press can burn a crystal with no mark in hand); nanami-counterchanged-comparison (稍微努力了一下 (4) is compared to the wrong AG card); nanami-card-text-newline (稍微努力了一下 (3) card text drops the sheet's line break); nanami-citation-markers (稍微努力了一下 (2)/(3) citations lack the 规则书 marker); nanami-note-locale-key (nanami_effort_note is undefined and serves two meanings)
- **rules/cards/card-morfonica/src/noble_blue.rs** — noble-todo-clause (高贵的微蓝 TODO quotes a clause the cell does not contain); noble-settle-open-todo (高贵的微蓝 forced settle carries no TODO for OPEN-触发结算); noble-designate-open-todo (高贵的微蓝 mark gate assumes 指定 == target_tile)
- **rules/cards/card-morfonica/src/pure_wings.rs** — pure-wings-teleport-comment (纯真振翅 comment calls teleport_to a raw position write); pure-wings-steps-wording (纯真振翅 comment calls -1 the steps default)
- **rules/cards/card-morfonica/src/rui_devil.rs** — rui-citations (rui_devil cites no clause at its implementing lines); rui-skill-id (Comment names the nonexistent skill id 正论恶魔)
- **rules/cards/card-morfonica/src/secret_rainbow.rs** — rainbow-variants (Variant personas never resolve in GRADES, so neither clause works for them); rainbow-fold (Integer fold drifts when the card is not the only fractional modifier); rainbow-stage (The scale answers the effect window, not the 支付前 stage); rainbow-selfpay (actor != target suppresses the card on self-payments); rainbow-reason (The counteraction prompt only shows clause (1)'s direction); rainbow-cnid (Table's second column is not the roster cn_id); rainbow-header (Header summary omits 同级生)
- **rules/cards/card-morfonica/src/starry_night.rs** — starry-guard-scope (The main-move/fixed-roll guard also narrows clause (2)'s even-roll attack); starry-x-cap (X is capped twice by unprinted rules (the 5 and cash on hand)); starry-cash-gate (Even-roll removal is suppressed when the owner has under 1,000 cash); starry-centre (场地中央 is unrecorded at the placement site); starry-citations (Hook registrations carry no clause citations); starry-owner-arg (attack_done message never binds {{owner}}); starry-reroll-default (Comment claims a 'default yes under 8' the engine does not have)
- **rules/cards/card-morfonica/src/summer_camp.rs** — summer-blocked-counter (SLOT_BLOCKED is never incremented, so the draw is unconditional); summer-by-comment (`by >= 0` gloss describes a distinction the guard does not make); summer-citations (The shield gate cites no clause at its implementing lines)
- **rules/cards/card-morfonica/src/toko_swap.rs** — toko-redeem (The free redemption only clears the flag, losing the redeem record); toko-order (The difference is paid after the redemption, not before); toko-color (同色 compares the raw tile group, ignoring anyColor / colorFor); toko-notarget (A noTarget deed is offered, then aborts the whole card after the pick); toko-target-comment ('// 规则书:' line quotes no clause)
- **rules/cards/card-morfonica/src/tritone.rs** — tritone-owed-amount-and-store (Tritone books the declared amount in a shared player slot instead of what the gain actually moved); tritone-missing-citations (The crystal write and the owed store carried no clause citation); turn-end-cadence-scope (The 「每回合结束时」 tick is owner-only in tritone but every-turn in fire_bird)
- **rules/cards/card-morfonica/src/tsukushi_garden.rs** — garden-false-default-comment (Comment claims a conditional default-Yes that ctx::ask_yes cannot express); garden-self-step-direction (Self branch announces a tile ahead while a reverse walk lands behind); garden-settle-before-move (The settle choice is asked before the move starts); unprinted-whole-card-move-gate (Whole-card play is refused under 無法移動 / a spent main move although the cells print no [限])
- **rules/cards/card-morfonica/src/your_light.rs** — light-start-log-without-tile (The start log fires even when the school tile is absent and nothing is set); unprinted-whole-card-move-gate (Whole-card play is refused under 無法移動 / a spent main move although the cells print no [限])
- **rules/cards/card-mujica/src/sakiko_cut.rs** — sakiko-mandatory-mortgage-and-answer-cursor (Sakiko Cut mortgages for free off the draw pile and mis-consumes its logged mortgage answer); sakiko-from-deck-todo (Sakiko Cut from-deck TODO was mangled and denied the existing play-origin query); sakiko-next-draw-discard-todo (Empty-hand branch only logs, and its TODO cited a stale attachment blocker); sakiko-deferred-discard-todo (Discard runs after the teleport, by index, from a hand the settle may have changed); sakiko-mortgageable-prop (Mortgageable filter had no clause citation and hard-coded the RiNG kind instead of NO_MORTGAGE); sakiko-replay-note (Replay note claimed an unconditional empty list and an inert replay)
- **rules/cards/card-mujica/src/sparkler.rs** — sparkler-terminal-turn-todo (Terminal crystal removal withholds the extra turn without a TODO marker); sparkler-turn-doc (Handler doc described a turn argument the function never had); sparkler-guard-doc (Guard doc claimed counter_is was not in the condition vocabulary); sparkler-trigger-citations (Both trigger registrations lacked a clause citation); sparkler-header-summary (Header summary implied every turn end grants an extra turn)
- **rules/cards/card-mujica/src/uika_fearless.rs** — uika-memory-latch-not-cleared (uika_fearless reads a write-once memory.dirty latch that is never cleared); uika-fixed-roll-extra-condition (Fix-roll option was hidden when a fixed roll already existed); uika-fix-roll-quote (Two citations dropped 「移动掷骰」 from the quoted clause)
- **rules/cards/card-mujica/src/umiri_card.rs** — umiri-extra-copy-skip (Extra band-skill copies on the holder are skipped without a book note); umiri-no-others-gate (The no-other-live-player play gate and its missing dest were undocumented); umiri-clause-citations (On::Action registrations and the take line cited by number/C# only); umiri-header-space (The cell's trailing space after 「。」 was dropped from the header quote and meta text); umiri-turn-start-take (The clause (2) take was skipped on the turn start that returns the card to the user); umiri-turn-end-discard-order (The end-hook draw runs before the instance reaches the discard); umiri-turn-start-pre-owner-condition (TurnStart pre suppressed the hop whenever the card already sat on the user's field); umiri-next-of-doc (next_of doc claimed ctx::neighbor skips exiled players)
- **rules/cards/card-mujica/src/welcome_mujica.rs** — welcome-clause1-quote (Clause (1) citation spliced the intro paragraph with effect (1)); welcome-stamp-citation (switchedRound stamp had no citation for the clause it resolves); welcome-duplicate-switch-log (Every counteract target was logged twice and no-op targets were announced as switched)
- **rules/cards/card-mygo/src/no_road.rs** — no-road-standin-never-retired (无路矢's field stand-in, slot and designation outlive the exile and re-arm on a later exile); no-road-exiled-pick (Exiled players are offered as 场上 targets); no-road-citation-markers (Three clause quotes lacked the `规则书:` marker); exile-return-no-settle (The [除外] return is the main move but lands with resolve=false, so it never settles)
- **rules/cards/card-mygo/src/ordinary.rs** — ordinary-lastwalk-written-by-every-walk (`lastWalk` was written by every non-teleport walk, not just main moves); ordinary-lastwalk-gate (C#-only `lastWalk > 0` prerequisite gate unrecorded); ordinary-trigger-citation (Trigger half of the clause had no citation)
- **rules/cards/card-mygo/src/rana_funny.rs** — rana-trap-not-at-endpoint (有趣的女人's trap never fires on the move endpoint or a teleport); rana-crystal-skipped-on-trap (A trapping pass grows no crystal); rana-parfait-choice (The parfait-or-stop choice is skipped without a 抹茶芭菲, and cited with straight quotes); rana-citations (Missing citation on the discard line)
- **rules/cards/card-mygo/src/rinne_rain.rs** — rina-settle-phase (轮符雨's extra settle runs at 结束阶段前 instead of 结束阶段); rinne-settle-name (Comments name a non-existent `ctx::settle`)
- **rules/cards/card-mygo/src/soyo_colors.rs** — soyo-halves-every-rent (混合的颜色 halves a plain landing, not only an agent settle); soyo-citations (Missing `规则书:` markers on the halving sites)
- **rules/cards/card-mygo/src/taki_serious.rs** — taki-settles-exiled-stayers (想认真去做 settles exiled [停留] holders who are on no tile); taki-no-stayer-refusal (想认真去做 refuses the play when nobody holds [停留]); taki-citations (Missing `规则书:` markers on the stayer filter and the settle site)
- **rules/cards/card-mygo/src/that_day_rain.rs** — rain-decay-owner-only (那天的雨 burns a crystal only at the owner's turn end)
- **rules/cards/card-mygo/src/tomori_no_longer.rs** — tomori-refill-guard-scope (refill hook fires for any same-owner card's crystal loss, not this card's); tomori-locked-stay-decay (lockedStay never decays, so it protects later ordinary stay layers from 壱雫空); tomori-stale-substitution-comments (comment claims a post-commit fireSpent refund and a turn-end sweep); fire-substitution-window-before-temp (Crystal substitution window must cover the whole fire cost, before any temp pot burns)
- **rules/cards/card-pp/locales/en.json** — aya-when-label-truncated (Counteraction label omits the [共鸣] cost gate); chisato-resonance-unreachable ((2) [共鸣] ran only inside the one-shot [手], so it became dead after placement); resonance-prompt-wording (Shared [共鸣] prompt said the effect would be upgraded, false for 明天见's bypass); shine-again-no-colour-message (Refusal said every colour is recorded when only owned colours are exhausted)
- **rules/cards/card-pp/locales/zh-CN.json** — aya-when-label-truncated (Counteraction label omits the [共鸣] cost gate); chisato-resonance-unreachable ((2) [共鸣] ran only inside the one-shot [手], so it became dead after placement); resonance-prompt-wording (Shared [共鸣] prompt said the effect would be upgraded, false for 明天见's bypass); shine-again-no-colour-message (Refusal said every colour is recorded when only owned colours are exhausted)
- **rules/cards/card-pp/src/aya_longing.rs** — aya-module-doc-wrong-stage-and-gate (aya_longing module doc names a dead PaySplitChoose hook and says all shaving is poorest-gated); aya-missing-citations-on-x-and-poorest (X formula and the 最少 condition have no 规则书 citation at their implementing lines); aya-registration-quote-drops-floor ((2) registration citation drops 「（最低0）」); aya-open-readings-todos (Chosen readings (tie = least, [共鸣] semantics, ≤0 skip) are unrecorded at their sites)
- **rules/cards/card-pp/src/chisato_mask.rs** — chisato-resonance-unreachable ((2) [共鸣] ran only inside the one-shot [手], so it became dead after placement); chisato-play-body-not-idempotent (A field press re-ran the whole [手]: second copy placed and 2000 charged again); chisato-splits-bypass-targeting (Split payers were raw ctx::others -- Untargetable and the target window bypassed); chisato-reshuffle-citation (The 500 implementing line had no citation)
- **rules/cards/card-pp/src/dream_ahead.rs** — dream-ahead-overflow-only-on-draw (The over-cap→X conversion and the cap only ran on Drew, so foreign crystal adds bypassed both); dream-ahead-header-wrong-hook (Header named SettleAfter as the live hook although the tax is wired on SettleBody); dream-ahead-free-tile-pre ((4)'s available-free-tile fact was missing from the activation pre); dream-ahead-clause-4-citations ((4) implementing lines carried only an unlabelled elided quote); dream-ahead-opening-deal (Opening deal raised no Drew, so the placed card gained nothing for the dealt hand)
- **rules/cards/card-pp/src/eve_bushido.rs** — eve-role-key-todo ((1) hook keys on [拥有者] while the clause says [使用者], unrecorded at the site); eve-2-precondition-citations (The (2) precondition (crystal and other player's deed) had no citation on its guards); eve-1-crystal-citation (The crystal-add line had no citation); eve-tie-skips-resonance (A tie returned before the 共鸣 window, so the fuel's own effect was unobtainable); eve-payer-direction-citation (Payer/payee direction implemented without the clause half that states it)
- **rules/cards/card-pp/src/first_live_accident.rs** — first-live-unguarded-pp-skills (Three P✽P (2) skills never consult skill_blocked, and the header claimed a shared gate); first-live-token-not-bound (The skillBlock token was written with instance -1, so it outlived the banished card); first-live-citation-prefix ([持续] citation lacked the 规则书 prefix and the clause's 。); hina-lottery-accident-lock (冰川日菜抽中的大奖 (2) keeps rolling while 初次演出事故 seals Pastel✽Palettes (2) skills)
- **rules/cards/card-pp/src/hina_sound.rs** — hina-mami-missing-from-subs (SUBS omitted 大和麻弥, so the card could never stand in for it); hina-use-condition-filter (「符合使用条件时」 is not filtered; the ask offers every substitute unconditionally)
- **rules/cards/card-pp/src/jennifer.rs** — jennifer-player-out-guards ([持续] steps are skipped when [使用者] is out)
- **rules/cards/card-pp/src/overlapping_voices.rs** — unprinted-whole-card-move-gate (Whole-card play is refused under 無法移動 / a spent main move although the cells print no [限])
- **rules/cards/card-pp/src/ranger.rs** — ranger-discount-status (Clause 4's status is turn-scoped and 可溢出 is not refunded)
- **rules/cards/card-pp/src/resonance.rs** — res-cost-single-primitive (共鸣 fuel cost removes two hand copies and discards only one); res-branch-citations (Try-resonance offer and branch carry no 规则书 marker); res-band-card-target (2 crystals gated on the persona's band instead of written to the owner's PP band card); res-effect-then-crystals-order (The 2 crystals are added before the caller's [共鸣] branch instead of after it)
- **rules/cards/card-pp/src/same_dream.rs** — same-dream-fan-comment (Module doc says every fan move raises FansFlip)
- **rules/cards/card-pp/src/see_you_tomorrow.rs** — seeyou-stale-abort-todo (TODO and comment describe a pre-commit abort that now exists); seeyou-resonance-reading-todo ((2) mandatory-cost reading and gate short-circuit have no site TODO)
- **rules/cards/card-pp/src/shine_again.rs** — shine-gain-pipeline (Rescue money bypasses the Money pipeline through gain_raw); shine-per-instance-record (「此卡未记录」 bitset is a player slot shared by every copy); shine-wrong-hook-name (Comment says (2) is offered from the PaySplitChoose hook); shine-citations ((1) helper and (2) record body carry no 规则书 citation); shine-deed-choice-todo (Which deed when several match 「÷5」 is unrecorded at the site); shine-pool-before-cost (The 共鸣 is paid before the empty-pool re-check)
- **rules/cards/card-pp/src/strong_flower.rs** — flower-stale-header (Header points at a TODO that no longer exists); flower-missing-stop-gate ([强制停下] written without the abnormal Stop gate); flower-target-pipeline ([指定] tile chosen without the targeting pipeline); flower-entry-citation (PassTile entry line has no 规则书 citation)
- **rules/cards/card-pp/src/title_idol.rs** — idol-face-down-skip (Clause 2 skips face-down cards although the cell states no state condition); idol-band-total-cap (Clause 1 passes i32::MAX past the band card's total-10 cap)
- **rules/cards/card-pp/src/trainee_guide.rs** — guide-buildoff-dead-hooks (Banked build-discount layers never apply because (3) always discards the card); guide-clause-citations (Fresh clause quotes without the 规则书 marker, and a drifted quote); guide-turn-end-seat ((2) granted a crystal at every seat's turn end); guide-div3-todo (÷3 rounding unprinted and unrecorded); guide-mark-table-todo (Colour-count table and mono/dual vocabulary unprinted and unrecorded); guide-cashin-order ((3) runs item 2 before item 1)
- **rules/cards/card-ppp/locales/en.json** — auction-locale-hand-entry (Locale note/log say 抽到 where the sheet says 加入手牌)
- **rules/cards/card-ppp/locales/zh-CN.json** — auction-locale-hand-entry (Locale note/log say 抽到 where the sheet says 加入手牌)
- **rules/cards/card-ppp/src/arisa_wait.rs** — arisa-guard-doc-clause-one (event_after_guard is the (1) eventAfter gate, not IEventDefer.Resolved; cite (1))
- **rules/cards/card-ppp/src/auction_pulled.rs** — auction-turn-start-owner-gate (Bare 「每回合开始时」 is narrowed to the owner's turn start unrecorded; header disagreed with body); auction-place-citation (Placement clause had no citation at its implementation site); auction-text-sections ([持续] label/paragraph glued into the [特] section); auction-immune-prop (Immunity only set by the Drawn body; other placement paths never stamp it)
- **rules/cards/card-ppp/src/bang_dream.rs** — bang-dream-guard-citation-todo (No-land play refusal justified only by C#, with no clause citation or TODO)
- **rules/cards/card-ppp/src/caught.rs** — caught-header-player-choice (Header says 'the next player ahead' but any player ahead is offered); caught-cant-play-citations (cant_play gate cites only C# routines, no clauses); caught-main-move-resolve (A leaked plan resolve=false skips the main move's landing settle); caught-duplicate-build (Explicit build offer duplicates the landing settle's own offer)
- **rules/cards/card-ppp/src/kasumi_love_all.rs** — kasumi-header-others-and-effects (Header says 'anyone' and omits discard/crystal effects)
- **rules/cards/card-ppp/src/maze_warehouse.rs** — maze-ryuseido-charge (6000/[强制停下] keyed on the plan-time looped flag, not on reaching 流星堂); maze-citations (Free-buy/raze clauses quoted without the 规则书: token); maze-no-ryuseido (Gate passes when the board has no 流星堂 and the play becomes a no-op); maze-free-buy-count (Free-buy log prints the die face, not the tiles actually crossed); maze-plan-reset (Walk inherits stop_at/extra_steps/reverse from the shared turn plan); maze-walk-log (Walk log names the planned landing before the move runs)
- **rules/cards/card-ppp/src/pipopa.rs** — pipopa-citations (Pipopa play body has no clause citations); pipopa-header-stays (Header's 'stays in play' is about the created card, not this banished one)
- **rules/cards/card-ppp/src/popipa.rs** — popipa-citations (Popipa play body has no clause citations)
- **rules/cards/card-ppp/src/popipapapipopa.rs** — popipapapipopa-gloss (Gloss renders 「消耗或支付時」 as only a payment the owner owes)
- **rules/cards/card-ppp/src/random_star.rs** — random-star-abnormal-gate ([强制停下] applies without the [异常移动效果] gate)
- **rules/cards/card-ppp/src/to_you_far_away.rs** — far-away-build-set-before-max (献给远方的你: the build offer took the maximum over every owned tile, not over buildable ones); far-away-teleport-ate-main-move (献给远方的你: card_move consumed the turn's main move for a teleport the sheet never makes a main move); far-away-cant-play-citation (献给远方的你: the no-other-player gate carried no rulebook citation)
- **rules/cards/card-ppp/src/tomorrows_door.rs** — door-route-cursor-per-instance (Tomorrow's Door: the route cursor was a per-player slot, so two copies shared one counter); door-clause3-citations (Tomorrow's Door: the (3) owner-tile path carried no rulebook citation at its implementing lines); door-settle-after-naming (Tomorrow's Door: comments pointed the tax at a `settle_after` hook the card never registers)
- **rules/cards/card-ras/src/be_strongest.rs** — be-strongest-missing-citations (Move-guard and destination table lacked 规则书 citations); be-strongest-roll-skipped-on-fallback (Fallback path never casts the 1d10 the text opens with)
- **rules/cards/card-ras/src/change_world.rs** — change-world-spots-per-player-livehouse (Placement only scanned four hardcoded tile names, missing per-player livehouse recolours); change-world-tile-keyed-charge (The next-charge hooks were payee-keyed and dropped when the tile had no houses); change-world-turn-latch-per-instance (The 期间 turn latch was one per-player slot, so a second copy re-armed the first); change-world-counted-house-n (n read the standing houses, not the counted value the rent is computed from); change-world-cancelled-charge-discard (A cancelled charge buried the card without collecting anything); change-world-missing-citations (Missing 规则书 citations at the crystal gain and the surcharge registration)
- **rules/cards/card-ras/src/chuchu_music.rs** — chuchu-bonus-cap-on-extra (The 500 cap was applied to the whole payout instead of the extra gain); chuchu-refill-without-hop (The crystal refill was skipped when there was no seat to hop to); chuchu-crystals-empty-state-trigger (「奇迹水晶为0时」 was checked only in the holder's turn-end tick); chuchu-per-instance-props (User and hits lived in per-player slots, shared by two copies); chuchu-out-user-skips-everything (An out user aborted the hop and the refill as well as the gain); chuchu-missing-citations (Clause-5 growth and the [使用者] writes had no citations); chuchu-divergence-markers (No TODO markers for any of the file's divergences); chuchu-discard-before-draw (The draw ran while the card was still on the field)
- **rules/cards/card-ras/src/crush_drum.rs** — crush-drum-comment-actor-scope (Comment narrowed the immunity to "this player" against 任何[使用者]); crush-drum-stale-todo-payload (TODO described the card-play payload without `by_card`)
- **rules/cards/card-ras/src/exist.rs** — exist-self-target-redirect (Another player's self-targeted card never reached EXIST's redirect); exist-flip-before-draw (The draw ran before the card was filed to the discard); exist-influence-latch-todo-wording (TODO said the latch means "the effect resolved" but it is written at declaration); exist-missing-citations (Latch and turn-start registrations had no 规则书 citations); exist-used-prop-comment (Comments said the redirect hook sets `used`)
- **rules/cards/card-ras/src/guerrilla.rs** — guerrilla-offturn-discard-arm (turn-end revive never arms on an off-turn discard); guerrilla-owned-livehouse-scan (「可盖房的live house格子」 scan uses a name list and tile kind); guerrilla-settle-comment (comment claims the settle only logs and opens no buy); guerrilla-affordable-pool (destination pool narrowed to affordable tiles); guerrilla-card-charge-window (window opens on a card's 收费 outside any settle); guerrilla-revive-needs-deed (revive suppressed when the player owns no deed); guerrilla-buy-result (card_buy result ignored and teleport note lost on a refused buy); guerrilla-free-tile-gate (unprinted free-tile precondition blocks the whole counteraction); guerrilla-paid-in-settle-rent-only (paid_in_settle only accumulates rent payments)
- **rules/cards/card-ras/src/hey_kids.rs** — hey-kids-window-gates (unprinted mortgaged/houses gates on the counteraction pre); hey-kids-straight-quote (fee citation uses ASCII quotes, not the sheet's curly ones)
- **rules/cards/card-ras/src/layer_keep.rs** — layer-keep-counter-placement (counteraction body never places the card, so it is buried); layer-keep-missing-todos (the counter body and slot read carried no TODO for the missing writer); layer-keep-delete-by-index (spending a die deletes every kept die with the same face); layer-keep-field-counter (placed copy cannot answer later fire rolls (「可保留多个骰点」 unreachable)); layer-keep-doc-comments (docs claim a writer, a CHARGE restriction and placement via play); layer-keep-quote (slot-read citation drops 「，可打出此卡并」); layer-keep-fire-cite (missing citation at the one-fire requirement); layer-keep-tag (card metadata lacks the [反击] tag); layer-keep-free-play (hand placement branch is open outside any fire window); layer-keep-fixed-roll-cite (missing citation at the move-roll replacement); layer-keep-fixed-roll-lifetime (fixed_roll is never one-shot cleared)
- **rules/cards/card-ras/src/lock_dream.rs** — lock-dream-latch-lifetime (lock_dream: the settle latch must die with the move that armed it); lock-dream-deck-doc (lock_dream: DeckBeforeGame doc sits on the deck-presence guard); lock-dream-place-citation (lock_dream: placement call has no rulebook citation); lock-dream-start-teleport (lock_dream: game-start [传送] is a bare position write)
- **rules/cards/card-ras/src/pareo_far.rs** — pareo-far-citation (pareo_far: mark grant has no rulebook citation)
- **rules/cards/card-ras/src/please_choose.rs** — please-choose-self-settle (please_choose: holder is wrongly excluded from their own settle); please-choose-dead-const (please_choose: unused LIVEHOUSE_BUYABLE table); please-choose-settler-class (please_choose: livehouse test uses the counteractor's colour view); please-choose-opt1-citation (please_choose: option (1) branch has no citation); please-choose-invalid-pos (please_choose: an invalid settler position resolves the pick invisibly); please-choose-class-comment (please_choose: comment calls the class helper "the whole check"); please-choose-band-fanout (please_choose: landing never counts as a band-skill trigger)
- **rules/cards/card-ras/src/repaint.rs** — repaint-path-direction (repaint: X counts a forward path even on reversed moves); repaint-pre-comment (repaint: header comment claims MainMove-only and always-forward); repaint-x-gate (repaint: play pre requires X > 0 though the book does not); repaint-missing-todos (repaint: real divergences carried no TODO(规则书)); repaint-trigger-citation (repaint: trigger clause has no rulebook citation); repaint-factor-comment (repaint: comment describes a multiplicative fold the code does not do); repaint-target-citation (repaint: target window has no citation)
- **rules/cards/card-ras/src/riot.rs** — riot-trigger-citation (riot: [反击] trigger has no per-site citation); riot-doc-summary (riot: summary omits the counteractor's extra draw); riot-draw-order (riot: the extra card is drawn inside the seat loop, not after it); riot-all-players-citation (riot: out-player skip is uncited); riot-payer-hit (riot: the pre misses a forced payment's payer)
- **rules/cards/card-ras/src/studio_storm.rs** — storm-livehouse-gate (storm: play gate uses a 4-name list instead of the livehouse class); storm-crystal-citation (storm: crystal clause cited only inside `play`); storm-tile-doc (storm: on_own_lh documented as a tile getter); storm-gate-citation (storm: play gate cites nothing from the sheet); storm-pass-doc (storm: pass_tile doc claims a bool guard)
- **rules/cards/card-ras/src/unstoppable.rs** — unstoppable-mod6 (unstoppable: 1d6mod6 implemented as clamp(1, 6)); unstoppable-gate-citation (unstoppable: main-move gate uncited); unstoppable-gate-doc (unstoppable: gate doc explains only the C# routine); unstoppable-spots-citation (unstoppable: destination table cites only the C# member)
- **rules/cards/card-roselia/locales/en.json** — lisa-bond-note-pending (The placed-card note claims a skill exemption nothing enforces)
- **rules/cards/card-roselia/locales/zh-CN.json** — lisa-bond-note-pending (The placed-card note claims a skill exemption nothing enforces)
- **rules/cards/card-roselia/src/ako_dark.rs** — ako-dark-offhand-origin (Ako dark draws from a hand declaration: origin read from the enclosing activation); ako-dark-window-citation (Window guard lacks a rulebook citation); ako-dark-stun-log (Stun log fires even when the grant is refused)
- **rules/cards/card-roselia/src/before_live.rs** — before-live-pass-tile (Offer suppressed on the landing step and teleports); before-live-citation (Trigger clause uncited at the entry); before-live-log-after-gate (Stop log printed before the abnormal gate)
- **rules/cards/card-roselia/src/blue_rose.rs** — blue-rose-zero-livehouse (0 deeds falls into the teleport branch although 0>=0 satisfies clause (1)); blue-rose-citations (Clause sites lack citations)
- **rules/cards/card-roselia/src/cookie_time.rs** — cookie-time-empty-gate-todo (Whole-card refusal on an empty discard is unrecorded); cookie-time-reshuffle-comment (Reshuffled note labelled 规则书 but rests on C#)
- **rules/cards/card-roselia/src/council_check.rs** — council-check-discard-refund (Refund only reachable through a completed settle); council-check-refund-threshold-todo (Undefined 「本卡原应拿到的资金」 has no TODO); council-check-citations (Stop and rent-tracking clauses uncited); council-check-header-name (Header summary names the retired card 压); council-check-slot-comment (Slot described as a per-card Mem stand-in); council-check-discard-quote (Non-verbatim 「此卡置入弃牌堆」 quote); council-check-two-copy-tile-guard (Earlier copy consumes the shared stop of a later copy's tile); council-check-near-inclusive-todo (Inclusive 「前后三格内」 reading unrecorded)
- **rules/cards/card-roselia/src/fire_bird.rs** — fire-bird-pay-mul-anchor (1.5x anchored on the payee, dropped by a payee redirect); fire-bird-x-range-todo (Unruled X bound 1..=10); fire-bird-short-pay-todo (Short-pay TODO quotes a clause the sheet lacks); fire-bird-stacking-todo (Stack-vs-replace of the 1.5x unrecorded); fire-bird-state-citation (「自己场地上存在此卡时」 uncited at the hook sites); fire-bird-pure-guard-docs (Guard doc copied onto non-guard bodies); turn-end-cadence-scope (The 「每回合结束时」 tick is owner-only in tritone but every-turn in fire_bird)
- **rules/cards/card-roselia/src/lisa_bond.rs** — lisa-bond-discard-dest (Played 必然的联系 must go to the discard, not stay in play); lisa-bond-header-pending (lisa_bond header claims a working 距离最近 skip); lisa-bond-trigger-citation (Missing 规则书 citation on the lisa_bond trigger)
- **rules/cards/card-roselia/src/louder.rs** — louder-open-todo (louder lacks a TODO for the undefined 价格基础乘数 mapping); louder-citations (louder's trigger and effect lines carry no 规则书 citation); louder-header-wording (louder header states a rent multiplier as fact)
- **rules/cards/card-roselia/src/nfo.rs** — nfo-face5-hook-lifetime (NFO face 5 leaves no live source for the gain hook); nfo-character-card-crystals (NFO crystals belong on the character card, not a private token); nfo-gain-guard-kind (NFO's gain offer fires on every Effect raise); nfo-replay-self-filter (NFO replay filter wrongly excludes its own id); nfo-pay-shave-citations (NFO's shave has no 规则书 citation); nfo-face3-comment-pointer (Face-3 comment points at a nonexistent counteract); nfo-face5-comment-accuracy (Face-5 comment names a raw token and hides the dead hook)
- **rules/cards/card-roselia/src/resolve.rs** — resolve-citations (觉悟's [特] payout and [手] removal carry no citation); resolve-header-sentence (觉悟 header sentence is split around the quote block); resolve-clause-sections (觉悟's two labelled clauses are crammed into one para)
- **rules/cards/card-roselia/src/ringing_bloom.rs** — ringing-bloom-factor-reconcile (Ringing Bloom never disarms RENT_FACTOR on a tile that caught up with the max); ringing-bloom-acquisition-hook (A tile acquired while the card is placed never got its half-charge armed); ringing-bloom-guard-citation (The RiNG guard cited only the C# predicate); ringing-bloom-payout-doc (pay_after doc claimed a payout per standing house); ringing-bloom-play-citation (The clause (1) registration had no citation); ringing-bloom-todo-narrow (The removal TODO overstated the gap: Action::Discarded exists)
- **rules/cards/card-roselia/src/sayo_play.rs** — sayo-cost-todo (No TODO recorded that the skill's press body and its cost are skipped); sayo-counteract-citation (The [反击] play clause was uncited at its registration); sayo-module-doc (Module doc presented the free C# extension as the realisation); sayo-persona-guard (The activation never required a bound character skill)
- **rules/cards/card-roselia/src/sprechchor.rs** — sprechchor-turn-window (Sprechchor stayed playable long past the turn-start window); sprechchor-citations (The roll/gain clause and the 可打出此卡 permission were uncited, and a foreign quote read as the card's text)
- **rules/cards/card-roselia/src/to_the_peak.rs** — tothepeak-ring-gate (Local is_deed never matched a RiNG, so the card charged 1.5x and raised nothing); tothepeak-build-gate (The build gate bypassed NO_BUILD / NO_BUILD_ABOVE); tothepeak-walk-gate (The walk branch never checked that the main move is still available); tothepeak-main-move-citation (The 「视为你的主要移动」 half of the clause was uncited at card_move); tothepeak-lap-note (The 0 -> full-lap remap was undocumented)
- **rules/cards/card-sumimi/src/childhood_cheer.rs** — childhood-plan-start-one-shot (plan.start outlived 「本次移动」); childhood-placed-carrier-unrecorded (Placed field instance is a hook stand-in the book never has); childhood-turn-end-discard-record (Turn-end discard of the stand-in unrecorded)
- **rules/cards/card-sumimi/src/here_the_world.rs** — here-the-world-two-copies (Hold and user were player-keyed while crystals were per instance); here-the-world-drew-per-card (Drew documented as a batch); here-the-world-citations (Registration lines cited nothing checkable); here-the-world-counter-plays (A counteraction never counts as a played card)
- **rules/cards/card-sumimi/src/idol_and_band.rs** — idol-and-band-set-not-gain (「重设为3000」 credited through the gain pipeline)
- **rules/cards/card-sumimi/src/l11.rs** — l11-cover-in-gates-and-spends (L11's cover was ignored by every gate and by four band skills); l11-discard-state-trigger (Discard at 0 was a turn-end re-check)
- **rules/cards/card-sumimi/src/l12.rs** — l12-temp-spend-credits (Temp-pot spends raised no markerSpent); l12-marker-name-filter (markerSpent handler matched every counter); l12-search-order (Body places before it selects); l12-pool-self-exclusion (id != ID dropped every sibling L12); l12-six-crystal-state-trigger (The 6-crystal return was an inline re-check); l12-reshuffle (Deck reshuffle called not expressible); l12-citations (Clause (2) cited only in a distant doc)
- **rules/cards/card-sumimi/src/mana_champion.rs** — mana-trigger-guard (mana_champion trigger guard misses the tile and payer halves); mana-join-prose (mana_champion join comments described the wrong mechanism); mana-draw-gate-prose (mana_champion shield check had no citation and the header misstated the draw gate)
- **rules/cards/card-sumimi/src/no_breakup.rs** — nobreakup-header (no_breakup header claimed a rollback the body lacks); nobreakup-digit-term (拥有相同数字 target term has no TODO); nobreakup-helper-citations (digit predicate and predict had no clause citations); nobreakup-gate-order (gate order contradicts the sheet)
- **rules/cards/card-sumimi/src/now_sumimi.rs** — nowsumimi-nonbuyable (card unofferable on non-buyable tiles); nowsumimi-price-tag-comments (now_sumimi comments wrong about the window and the abnormal exclusion)
- **rules/cards/card-sumimi/src/sweet_escape.rs** — sweet-refused-move (sweet_escape buys even when the move was refused); sweet-double-offer (settle's optional buy duplicates the forced buy); sweet-header (sweet_escape header read 'free tile' as unowned); sweet-price-tag (sweet_escape 收费标价 reading unrecorded); sweet-colour-primitive (colour filter ignores ANY_COLOR tiles)
- **rules/cards/card-sumimi/src/two_donuts.rs** — donuts-latch-terminals (latch never fires without a settle); donuts-endpoint (window endpoint used the passer's live position); donuts-exclude-orig (original tile not excluded when the passer ends on it); donuts-overflow (overflow conversion counted from the wrong basis); donuts-clause-number (fire-pot tail numbered (3) though the sheet has only (1) and (2))
- **rules/cards/card-sumimi/src/two_in_one.rs** — twoinone-play-gate (play had no Sumimi persona gate); twoinone-find-card (find_card lookup miss leaves the old skill on the field); twoinone-clause2-citation (counteraction trigger uncited); twoinone-persona-todo (counteraction adds an unstated persona restriction)
- **rules/kani/src/host.rs** — toko-redeem (The free redemption only clears the flag, losing the redeem record); toko-color (同色 compares the raw tile group, ignoring anyColor / colorFor)
- **rules/kani/src/proofs/crychic.rs** — want_to_grab-field-placement (Play body never places the card, so every hook is dead)
- **rules/kani/src/proofs/mor.rs** — tritone-owed-amount-and-store (Tritone books the declared amount in a shared player slot instead of what the gain actually moved); toko-redeem (The free redemption only clears the flag, losing the redeem record)
- **rules/kani/src/proofs/r.rs** — ako-dark-offhand-origin (Ako dark draws from a hand declaration: origin read from the enclosing activation); nfo-face5-hook-lifetime (NFO face 5 leaves no live source for the gain hook)
- **rules/kani/src/proofs/ras.rs** — riot-draw-order (riot: the extra card is drawn inside the seat loop, not after it)
- **rules/kani/src/proofs/skills_pp.rs** — l11-crystal-substitution-cover (Honor 「Sumimi:#L11」 crystal substitution at every band-skill cost (gates + removals) and quote it verbatim)
- **rules/kani/src/proofs/skills_ppp.rs** — l11-crystal-substitution-cover (Honor 「Sumimi:#L11」 crystal substitution at every band-skill cost (gates + removals) and quote it verbatim)
- **rules/kani/src/proofs/skills_sumimi.rs** — l11-crystal-substitution-cover (Honor 「Sumimi:#L11」 crystal substitution at every band-skill cost (gates + removals) and quote it verbatim)
- **rules/kani/src/spec/crychic.rs** — want_to_grab-field-placement (Play body never places the card, so every hook is dead)
- **rules/kani/src/spec/mor.rs** — tritone-owed-amount-and-store (Tritone books the declared amount in a shared player slot instead of what the gain actually moved); starry-x-cap (X is capped twice by unprinted rules (the 5 and cash on hand))
- **rules/kani/src/spec/r.rs** — ako-dark-offhand-origin (Ako dark draws from a hand declaration: origin read from the enclosing activation); blue-rose-zero-livehouse (0 deeds falls into the teleport branch although 0>=0 satisfies clause (1))
- **rules/kani/src/spec/ras.rs** — riot-draw-order (riot: the extra card is drawn inside the seat loop, not after it)
- **rules/markers/src/cp.rs** — cp-settle-reads-self-field-cp-as-src-card-stock (mark:cp settle spends the mark's src card and goes inert without one, while the cell says the settler's own field and unconditional effects)
- **rules/skills/skill-groups/src/ave_mujica.rs** — l11-crystal-substitution-cover (Honor 「Sumimi:#L11」 crystal substitution at every band-skill cost (gates + removals) and quote it verbatim); l11-cover-in-gates-and-spends (L11's cover was ignored by every gate and by four band skills)
- **rules/skills/skill-groups/src/crychic.rs** — l11-crystal-substitution-cover (Honor 「Sumimi:#L11」 crystal substitution at every band-skill cost (gates + removals) and quote it verbatim); crychic-band-3-dead-drawn-hook (CRYCHIC band (3) is registered on the drawn-card-only `Drawn` and never fires for the placed skill)
- **rules/skills/skill-groups/src/hhw.rs** — dream-skill-payee-guard (传播笑容 (2) matches the payer instead of the collecting player); l11-crystal-substitution-cover (Honor 「Sumimi:#L11」 crystal substitution at every band-skill cost (gates + removals) and quote it verbatim); l11-cover-in-gates-and-spends (L11's cover was ignored by every gate and by four band skills)
- **rules/skills/skill-groups/src/lib.rs** — l11-crystal-substitution-cover (Honor 「Sumimi:#L11」 crystal substitution at every band-skill cost (gates + removals) and quote it verbatim); l11-cover-in-gates-and-spends (L11's cover was ignored by every gate and by four band skills)
- **rules/skills/skill-groups/src/mygo.rs** — l11-crystal-substitution-cover (Honor 「Sumimi:#L11」 crystal substitution at every band-skill cost (gates + removals) and quote it verbatim); l11-cover-in-gates-and-spends (L11's cover was ignored by every gate and by four band skills)
- **rules/skills/skill-groups/src/pastel.rs** — l11-crystal-substitution-cover (Honor 「Sumimi:#L11」 crystal substitution at every band-skill cost (gates + removals) and quote it verbatim); l11-cover-in-gates-and-spends (L11's cover was ignored by every gate and by four band skills)
- **rules/skills/skill-groups/src/poppin.rs** — l11-crystal-substitution-cover (Honor 「Sumimi:#L11」 crystal substitution at every band-skill cost (gates + removals) and quote it verbatim); l11-cover-in-gates-and-spends (L11's cover was ignored by every gate and by four band skills)
- **rules/skills/skill-groups/src/sumimi.rs** — l11-crystal-substitution-cover (Honor 「Sumimi:#L11」 crystal substitution at every band-skill cost (gates + removals) and quote it verbatim); l11-cover-in-gates-and-spends (L11's cover was ignored by every gate and by four band skills)
- **rules/skills/skill-personas/src/hina_lottery.rs** — hina-lottery-accident-lock (冰川日菜抽中的大奖 (2) keeps rolling while 初次演出事故 seals Pastel✽Palettes (2) skills)
- **rules/skills/skill-personas/src/sakiko_life.rs** — sakiko-life-deck-top-dest (状态2 auto-play never removes the card from the draw pile and drops its Dest)
- **rules/skills/skill-personas/src/uika_imprisoned.rs** — uika-memory-latch-not-cleared (uika_fearless reads a write-once memory.dirty latch that is never cleared)
- **rules/status/src/exile.rs** — exile-return-no-settle (The [除外] return is the main move but lands with resolve=false, so it never settles)
- **rules/tests/tests/general.rs** — tsugu-buy-candidates-quote (Buy candidates use the pre-stage native price and ignore BuyGate eligibility)
- **rules/tests/tests/tile_designation.rs** — one-of-us-duplicate-copy-state (ONE OF US: pair/rent/refs overwritten by a second copy (player state → instance props))
- **webui/public/assets/i18n/cards-en.json** — backstage-reveal-pair (backstage_tour: first keep/flip decision made before the second card is named); balloon-record-all-rolls (balloon_show: the four results are neither logged nor all offered); kokoro-log-amount (settle log hard-coded 6,000 regardless of what moved)
- **webui/public/assets/i18n/cards-zh-CN.json** — backstage-reveal-pair (backstage_tour: first keep/flip decision made before the second card is named); balloon-record-all-rolls (balloon_show: the four results are neither logged nor all offered); kokoro-log-amount (settle log hard-coded 6,000 regardless of what moved)

## B — needs new ABI (65 items, 242 rows, 15 capability groups)

One design per group, for review before implementation. The designs are the
unit of approval; a card is fixed only after its group lands. Group files live
in `docs/rulebook/spec/abi/`.

### G01 — Instance identity and per-instance store

Design: `docs/rulebook/spec/abi/G01-instance-identity-and-state.md`

- **skill-fired-signal** (high, rows 152,184,232,445,1091,1243,1244,1481) — No per-resolution skill-fire signal, no guest emit, and X/skill-instance scope; `rules/cards/card-morfonica/src/rui_devil.rs`
- **trigger-instance-identity** (medium, rows 252,283,285,1029,1039) — Trigger/marker identity and per-instance state are not addressable (uid, marker tile, card.prop, dest-by-uid); `rules/cards/card-morfonica/src/starry_night.rs`
- **counter-cap-uid** (medium, rows 301,342,1056) — No per-uid counter cap read or clamp beyond one call; `rules/cards/card-mujica/src/sparkler.rs`

### G02 — Pay pipeline payload, stages and raw-money windows

Design: `docs/rulebook/spec/abi/G02-pay-pipeline.md`

- **pay-payload-kind-tile** (high, rows 226,230,305,306,309,323,953,959,1005,1031,1241,1353,1427) — Card-driven payments cannot carry kind, must-buy, tile, settle tile or a fixed no-modifier tranche; `rules/cards/card-morfonica/src/rui_devil.rs`
- **pay-stage-presplit** (medium, rows 21,1397,1539) — No interactive pre-stage before PaySplitAdd and no split-leg/total flag on the trigger; `rules/cards/card-crychic/src/vocal_too_hard.rs`
- **raw-money-window** (medium, rows 572,1333) — Raw money paths (lose_raw, redeem, auction) open no pay window; `rules/cards/card-morfonica/src/tritone.rs`
- **charge-paying-source** (high, rows 458,459,1360) — ChargePaying cannot tell a skill spend and only sees the pool share; `rules/cards/card-mygo/src/tomori_no_longer.rs`
- **fire-bird-paid-signal** (medium, rows 1441) — A discounted payment misreads as a shortfall; `rules/cards/card-roselia/src/fire_bird.rs`
- **rent-figure** (medium, rows 513,1388) — The link surcharge uses rent_of and ignores mortgage/dice/rent-factor partners; `rules/cards/card-mygo/src/anon_tokyo.rs`
- **money-counters** (medium, rows 1009,1472) — gains_this_turn misses raw credits; paid_in_settle is world-wide; `rules/cards/card-hhw/src/hagumi_marks.rs`

### G03 — Roll pipeline stages and prompt option actions

Design: `docs/rulebook/spec/abi/G03-roll-and-prompts.md`

- **roll-stage-effect-rolls** (high, rows 62,179,596,598,1053,1224,1367,1479) — Effect rolls and non-turn rollers skip the value/after stages; no per-roller move dice; `rules/cards/card-hhw/src/sports_talent.rs`
- **roll-history-per-player** (low, rows 97,1159,1160) — turn_rolls is turn-wide, not per roller; `rules/cards/card-crychic/src/taki_even_if.rs`
- **reroll-post-pick-signal** (high, rows 150,242,249,892,893,1002,1169,1187,1262,1379) — No reroll/post-pick signal; rerolled faces skip DiceRollValue and armed boosts/crystal checks; `rules/cards/card-morfonica/src/starry_night.rs`
- **please-choose-option1** (medium, rows 750) — please_choose option (1) never plays a card; `rules/cards/card-ras/src/please_choose.rs`
- **own-stage-answer** (high, rows 671,672,673,674,1455) — own_stage's resolveOverride/stageEnd answers are never consumed; `rules/cards/card-roselia/src/own_stage.rs`

### G04 — Move plan, walk sync and forced moves

Design: `docs/rulebook/spec/abi/G04-move-plan.md`

- **want-to-grab-board-axis** (medium, rows 107,1003) — Clause (3) nearest-orthogonal teleport needs a board-coordinate axis query; `rules/cards/card-crychic/src/want_to_grab.rs`
- **plan-shape-scope** (high, rows 131,132,288,290,338,339,340,560,835,963,964,1038,1064,1095,1361,1483) — Turn plan is one sticky struct: no reset, no pay_factor getter/multiplier, shaped moves leak; `rules/cards/card-morfonica/src/pure_wings.rs`
- **charity-rolled-flag** (medium, rows 209,1215,1216) — charity_show: move.main proxies 「掷骰移动」; `rules/cards/card-hhw/src/charity_show.rs`
- **plan-live-sync** (high, rows 203,973,1025,1182) — Plan writes after a window and next_steps ordering never reach the running walk; `rules/cards/card-hhw/src/charity_show.rs`
- **effect-move-forced-path** (high, rows 129,200,291,293,295,431,1183,1184,1286) — Effect moves take the main-move path: no forced flag, no walk void, followers dropped; `rules/cards/card-crychic/src/sakiko_lead.rs`
- **trigger-walk-payload** (high, rows 292,294,298,954,1006,1007,1101) — diceRollResult carries no roll_source, planned start, parity or effective walk data; `rules/cards/card-ag/src/detour.rs`
- **nested-teleport-tail** (medium, rows 693) — A skill teleport nested in a settle raises no MoveAfter; `rules/cards/card-roselia/src/lisa_bond.rs`

### G05 — Press surface for placed cards

Design: `docs/rulebook/spec/abi/G05-placed-card-press.md`

- **press-surface-placed-card** (high, rows 380,487,488,490,955,967,974,1521,1555) — Placed non-skill cards and field actions are on no press surface; `rules/cards/card-hhw/src/kanon_march.rs`

### G06 — Counteraction, join and negation windows

Design: `docs/rulebook/spec/abi/G06-counteraction-and-join.md`

- **join-window-report** (medium, rows 844,845,942,1090,1094,1312,1510) — No nested per-seat join window or joiner report; chosen designations unreadable; `rules/cards/card-general/src/net_error.rs`
- **extreme-range-event-counter** (medium, rows 158,980,982,990) — Extreme counter admits every card play and has no event-chain scoping; `rules/cards/card-ag/src/tsugumi_can.rs`
- **counter-link-move-face** (medium, rows 838) — A counter link loses the move face, so repaint cannot answer a chain; `rules/cards/card-ras/src/repaint.rs`
- **counteraction-history** (high, rows 426,1060) — No per-turn counteraction history (who countered whom); `rules/cards/card-mujica/src/cant_look_away.rs`
- **window-timing-negation** (medium, rows 395,936,940,1527,1643) — The effect window opens before the abnormal lands or is refused; guards see no pending negation; `rules/cards/card-mygo/src/ordinary.rs`
- **mortis-copy-links** (medium, rows 312,313,315,1045) — mortis copies nothing; an empty counteraction link list is never offered; `rules/cards/card-mujica/src/mortis_instinct.rs`

### G07 — Targeting pipeline and distance

Design: `docs/rulebook/spec/abi/G07-targeting-and-distance.md`

- **target-count-pipeline** (medium, rows 274,1037,1061,1294) — targeted_count reset timing, uncounted tile targetings, self-target short-circuit; `rules/cards/card-morfonica/src/centrifugal.rs`
- **lisa-distance-exemption** (medium, rows 691,1451) — No mechanism grants the 距离最近 exemption; `rules/cards/card-roselia/src/lisa_bond.rs`

### G08 — Zone and attachment lifecycle

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

### G09 — Tile, build, pass and house-removal hooks

Design: `docs/rulebook/spec/abi/G09-tile-build-pass.md`

- **marina-suppression-window** (medium, rows 880,885,1493) — No pass-time "affected" window and no reader for the CiRCLE suppression tag; `rules/cards/card-general/src/marina_work.rs`
- **parade-tile-swap** (medium, rows 1089) — smile_parade (3): the tile swap covers the settle only, not [经过]; `rules/cards/card-hhw/src/smile_parade.rs`
- **build-pipeline-queries** (medium, rows 118,123,124,521,748,1206) — Card-driven builds bypass buildability, effective price and build hooks; `rules/cards/card-hhw/src/smile_patrol.rs`
- **tile-write-effect-declare** (medium, rows 36,38,989) — Tile-only writes declare no Effect, so 「或你拥有的格子」 is unreachable; `rules/cards/card-ag/src/declare_war.rs`
- **guerrilla-pass-path** (medium, rows 866) — guerrilla: the window never checks a passed opponent Live House; `rules/cards/card-ras/src/guerrilla.rs`
- **house-removed-kind** (medium, rows 819,826) — No HouseRemoved kind: a razed max tile leaves the view and the half stale; `rules/cards/card-roselia/src/ringing_bloom.rs`

### G10 — Effect outcome, cause and settle facts

Design: `docs/rulebook/spec/abi/G10-effect-outcome.md`

- **event-defer** (medium, rows 567,568) — No IEventDefer and eventAfter has no cause; `rules/cards/card-ppp/src/arisa_wait.rs`
- **settle-outcome-flags** (medium, rows 792,1428,1449) — A settle cannot report body-replaced/paid/cancelled outcome; `rules/cards/card-ppp/src/tomorrows_door.rs`
- **host01-journal** (medium, rows 724,728) — Nested-settle writes are invisible to the master re-run (HOST-01); `rules/cards/card-ras/src/be_strongest.rs`
- **redirect-latch** (medium, rows 781,1465) — 「造成影响」 latches on any payee-equals-owner effect, not a landed redirect; `rules/cards/card-ras/src/exist.rs`
- **world-snapshot** (medium, rows 782,1077) — No play-time world snapshot/restore for no_breakup; `rules/cards/card-sumimi/src/no_breakup.rs`

### G11 — Information and deck inspection

Design: `docs/rulebook/spec/abi/G11-information-and-deck.md`

- **backstage-peek-refill** (medium, rows 1018) — backstage_tour: a peek empties the deck and the first flip triggers the refill; `rules/cards/card-hhw/src/backstage_tour.rs`
- **reveal-deck-inspection** (medium, rows 165,170,171,537,612,615,1092,1400) — No public reveal output or deck-inspection hook; piles only log counts; `rules/cards/card-hhw/src/believe_you.rs`
- **drew-batch** (medium, rows 1401,1480) — Drew fires per card; the sheet keys on a draw action; `rules/cards/card-pp/src/infinite_possibility.rs`

### G12 — Status, blocked and immunity gates

Design: `docs/rulebook/spec/abi/G12-status-and-immunity.md`

- **move-gate-replay-read** (high, rows 225,988,1498) — Tsugu refuses the whole play on a spent main move; no replay-stable moved read; `rules/cards/card-general/src/tsugu_ycm.rs`
- **unstoppable-blocked-read** (medium, rows 403,404,1035,1454) — Engine gate never reads [不可阻挡] and own_stage's turn-end branch keys on abnormal kind, not blocked-ness; `rules/cards/card-morfonica/src/tsukushi_garden.rs`
- **major-abnormal-scope** (medium, rows 1098,1171) — Abnormal check for want_human is a turn-wide diff, not move-scoped; `rules/cards/card-crychic/src/want_human.rs`
- **immunity-causer** (medium, rows 608,1412) — Card immunity is causer-blind; an owner-exempt flavour is unavailable; `rules/cards/card-pp/src/shine_again.rs`
- **dice-immunity** (medium, rows 710,1462) — [特] immunity for dice-adding cards is unimplemented; `rules/cards/card-ras/src/crush_drum.rs`

### G13 — Skill invoke and copy context

Design: `docs/rulebook/spec/abi/G13-invoke-and-copy.md`

- **invoke-skill-entry** (medium, rows 437,996) — invoke_skill runs On::Play only and loses the skill instance; `rules/cards/card-morfonica/src/nanami_effort.rs`
- **tomori-borrowed-second-half** (medium, rows 187,995) — Borrowed (2): the roll/停留/delayed settle half never runs; `rules/cards/card-crychic/src/tomori_inner_shout.rs`

### G14 — CardDef AI hooks and hand limit

Design: `docs/rulebook/spec/abi/G14-ai-and-hand-limit.md`

- **hand-limit-lift** (medium, rows 441) — A guest state increase does not reach the host hand-limit read; draw trims; `rules/cards/card-morfonica/src/nanami_effort.rs`
- **aiplay-hook** (low, rows 1253) — No CardDef AiPlay hook; bots offer a never-play card; `rules/cards/card-morfonica/src/summer_camp.rs`

### G15 — Kani verification host support

Design: `docs/rulebook/spec/abi/G15-kani-host.md`

- **kani-owned-livehouse** (low, rows 623) — blue_rose proofs are vacuous: the Kani host cannot own a buyable tile; `rules/kani/src/spec/r.rs`


## D — dropped (1 items, 1 rows)

Stale: already fixed, already landed, or duplicated by a live item.

- **dream-ahead-counteraction-recorded** — Turn-end [反击] rider is documented by an accurate TODO, wiring unchanged; `rules/cards/card-pp/src/dream_ahead.rs`


---

# Convention markers (not gaps — do not count as open)

`rules/{events,tiles,markers}/src/lib.rs`, `rules/skills/{skill-personas,
skill-groups}/src/lib.rs`, `crates/game-core/src/data.rs` (`event_rule_id` doc),
`crates/card-sdk/src/ctx.rs:576` (cross-ref to the ruling-6 note). These name the
`TODO(规则书)` policy: a gap in the text is tagged, never silently dropped.


# card-std-split leftovers (2026-10-12)

Deferred from the `card-std-split` / ABI v56 pass (bandori-std + `tile_is`/
`tile_attr`/`tile_in`). Not merge blockers once drift is green.

- **CD-01 CEL `is_live_house` group-10 quirk.** **Closed** (2026-10-17 audit):
  the CEL `is_live_house` sugar was removed (abi.rs v70). The only live answer
  is `tile_in(_, t, "livehouse")` = `VENUE_GROUP = 6`
  (`rules/bandori-std/src/kind.rs:81,90`); `is_venue` / `is_live_house_for`
  route through it (`bandori-std/src/lib.rs:96-105`). No second SnapSrc
  group-10 answer remains to reconcile.
- **CD-02 `crystals_changed_guard` residuals.** `bandori_std::crystals_changed_of`
  is the shared base (`bandori-std/src/lib.rs:370-375`); per-card copies still
  carry local residuals (value vs 0, `is_placed`, …) — e.g.
  `rules/cards/card-mujica/src/j11.rs:165-174`, `card-roselia/src/fire_bird.rs:25`.
  Fold the identical prefix onto the helper.
- **CD-03 `legacy_*` audit copies.** Done 2026-10-14: the G3 audit scaffolding
  and every `legacy_*` copy were deleted (GUARDS.md §5.1/§10; ABI 61).
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

## MOVE-04 — 若能再次交汇 re-roll loop hits `world_mut` while a routine is pending
- **kind**: test-harness sequencing (not a card/host defect)
- **class**: D (fixed 2026-10-10; see D-05)
- **severity**: high (re-roll cards are common)
- **suggested batch**: fuzz/determinism / movement
- **locations**
  - `rules/tests/tests/cross_move.rs` `m06_reunion_20_tile_cap` / `m06b_reunion_stops_on_passing` (un-ignored, green twice)
- **rulebook**: 若能再次交汇 — re-roll the move dice (with a 20-tile cap / stop-on-passing shape).
- **was**: `t.dice` after `t.roll` called `Match::world_mut` while the `diceRollResult` [反击] window was a pending routine.
- **fixed**: queue all faces before `t.roll` (snapshot replay also means mid-window loads never reach the re-roll). Card (`meet_again.rs`) and host `do_move_roll` / `set_move_roll` write-back are correct and unchanged.

## SETTLE-01 — 笑容大游行 tile-swap overflows the stack
- **status**: **closed** (2026-10-18 engine-verbs batch). Stack overflow half
  **fixed** by STACK-01; swap-axes half **fixed**; and the reverse direction
  (「交换位置」 is a two-way exchange) **landed** with the host-visible
  `Move::settle_redirect` flag — `trigger::is_settle_redirect()` marks the
  nested settle a card caused, so the ping-pong that used to hit the
  settle-depth cap is gone. `SLOT_REPLAY` is deleted (guest `set_slot` writes
  were the wrong home). `g30b_smile_parade_reverse_swap_runs_as_card_tile`
  asserts a settle on 弦卷集团 runs as the card's tile and does **not** discard
  the parade.
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
  crystals; (3) one-way card-tile → agent body + discard after **any** player's
  settle on the card's tile (LOY-SMILE-01, 2026-10-18: 「任何玩家」 includes the
  owner; (1)'s redirect and (2)'s arrival are keyed off `SLOT_REPLAY`, not off
  "the actor is the owner"). The crystal tick is unconditional; only the
  zero-crystal discard names 「仍位于"弦卷集团"」.

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

## SKILL-07 — soyo 「持续至你的下回合开始」 recolour has no expiry
- **kind**: implementation bug
- **class**: **A — closed 2026-10-21** (`0ce220b1`, verified this batch)
- **severity**: medium
- **suggested batch**: skills: CRYCHIC / tiles
- **locations**
  - `rules/skills/skill-personas/src/soyo_clear.rs` — `COLOURED` remembers the
    landing as `tile + 1`; `declare_cap` (on `TurnStartBefore` /
    `DeckAtGameStart`, gated `trigger::player_id() == player_id`) expires the
    `colorFor:<p>` prop and clears the slot. Both entries are on the card's
    `silent` list: the expiry is bookkeeping, not a printed effect.
  - pinned by `rules/tests/tests/mygo.rs` `soyo_recolour_expires_at_next_turn_start`.
- **rulebook**: 「持续至你的下回合开始」.
- **was**: the `colorFor:<p>` write was permanent.
- **residual (already filed, not this item)**: 「若以此法单次免除了至少1500资金的
  [支付]…」 attribution is **SKILL-06**'s provenance-tagged-spends capability
  (`soyo_clear.rs` `TODO(规则书)[judgement]`). The penalty latch itself is wired
  in `on_pass`.

## SKILL-08 — Rana parking 「你与其他玩家重合时」 never fires (suspected bug)
- **kind**: implementation bug
- **class**: D (fixed 2026-10-10; see D-07)
- **severity**: medium
- **suggested batch**: movement / skills: MyGO
- **locations**
  - `rules/skills/skill-personas/src/rana_parking.rs` `on_overlap` (un-ignored via new test `rana_cat_overlap_takes_800_and_gives_a_parfait`, green twice)
- **rulebook**: 「你与其他玩家重合时」.
- **was**: pre is `MINE` (owner is the mover) but the body read `player_id` as "the other" and bailed when it equalled the owner — always.
- **fixed**: body reads `trigger::target()` (the other player already on the landing tile). Amount stays 800 (sheet / `characters.json`; older `rulebook.txt` prose says 1000).

## SKILL-09 — kaoru_prince 「每次[经过]或被[经过]时」 fires for every passer
- **kind**: stale
- **class**: D (already fixed; backlog entry was against an older revision)
- **severity**: medium
- **suggested batch**: movement
- **locations**
  - `rules/skills/skill-personas/src/kaoru_prince.rs` — Pass half is already `pre::MINE`; 被[经过] is a filtered `PassTile`
  - live pins: `hhw.rs` `kaoru_skill_1_own_walk_gains_one_pot_per_step` / `kaoru_skill_1_another_players_walk_gives_nothing` / `kaoru_skill_1_being_passed_gains_one_pot_not_one_per_step`
- **rulebook**: 「每次[经过]或被[经过]时」 — `Pass` half is "I pass" (`actor == owner`).
- **fixed already**: `On::Action(&[ActionKind::Pass], pre::MINE, …)` + `PassTile` for the 被[经过] half (was `HookKind::Pass` pre-v85). Nothing to un-ignore.

## TILE-03 — RiNG rent table / mortgageability / `ringMultiplier`
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: rent multipliers / tiles
- **locations**
  - `rules/tiles/src/ring.rs:13` (dice-rent table + 「至少1个」 floor have no book clause)
  - `crates/game-core/src/state.rs:455` / `crates/card-sdk/src/abi.rs:572` / `crates/card-sdk/src/ctx.rs:453` (`ring_multiplier` TODO)
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

## CRYSTAL-04 — NNM (2) 「获得x次经过CiRCLE时的资金奖励」 does not scale with the live reward
- **kind**: implementation bug
- **class**: **A — closed** (earlier batch, `fix: CRYSTAL-04 …`; the 2026-10-20
  entry still described the bug and is corrected 2026-10-21)
- **severity**: medium
- **suggested batch**: crystal/marker accounting / rent multipliers
- **locations**
  - `rules/cards/card-morfonica/src/nanami_effort.rs:162-170` —
    `gain_circle_money` raises `PassRewardAffected` / `REWARD_MONEY` and pays the
    live figure.
  - `rules/tests/tests/morfonica.rs:468`
    `nnm_option2_gains_x_times_circle_money_reward` (expects Morfonica's 1000).
- **rulebook**: Sheet 2026-10-06 新卡组卡 G7 (2) 「获得x次经过CiRCLE时的资金奖励」 (was 「获得x*2000资金」); Morfonica rewrites the CiRCLE reward to 1000/1500/2000.
- **was**: hard-coded 2000.
- **now**: each of the x rewards runs the money half of the CiRCLE reward step
  (`circleAffected` between pick and payout), so Morfonica's 1000/1500/2000 cycle
  (and any other listener) scales it.

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

## LOW-02 — property / upgrade book clauses that live elsewhere
- **class**: A (docs only) — **closed 2026-10-21**. The module doc in
  `rules/tiles/src/property.rs` now cites both live sites instead of restating
  them (and the old `TODO(规则书)` is gone):
  - 「每块地有标注的等级上限」 -- the cap is the rent-table length
    (`rent.len() - 1`; `prop::BUILD_MAX` in `crates/game-core/src/state.rs`);
    refusals live in `crates/game-core/src/engine/ops.rs` `why_not_build_on`
    (no upgrade row / stamped `BUILD_MAX = 0` / `houses >= rent.len() - 1`).
    `rules/tiles/src/ring.rs` is the `buildMax = 0` RiNG case.
  - 「升级所[消耗]的资金不可通过[抵押]正在升级的格子地契获得」 -- enforced by
    `Cx::offer_mortgage_funds`'s `exclude`
    (`crates/game-core/src/engine/play.rs`), **not** by `mortgage` (the 2026-10-20
    entry had that site wrong). Restated beside the deed-economics clauses in
    `rules/tiles/src/deed.rs`.
- no behaviour change.

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

## STA-01 — [不可阻挡] does not refuse new [晕眩]/[除外] at grant verbs
- **kind**: implementation bug
- **class**: A
- **severity**: medium
- **suggested batch**: crystal/marker accounting
- **locations**: `rules/status/src/unstoppable.rs:14-15`
- **rulebook**: glossary 14 「无法获得新的[除外]层数」/「也不会获得新的层数」.
- **current (2026-10-18)**: **done** -- both halves refuse in the host grant
  verbs before the engine service (no abnormal window for a refused grant):
  [晕眩] in `status::stun::add` / `add_start` (`status-hosts-rest`), [除外] in
  `rules/status/src/exile.rs::grant` (`twodonuts-exile`).

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

## STA-04 — [除外] re-gain stacks despite 「该效果生效时无法重复获得该效果」
- **kind**: implementation bug
- **class**: A
- **severity**: medium
- **suggested batch**: crystal/marker accounting
- **locations**: `rules/status/src/exile.rs::grant`
- **rulebook**: 「该效果生效时无法重复获得该效果」 (`data/rules.txt`).
- **current**: **done** (2026-10-18) -- the guard lives in the host grant verbs
  (`grant` / `send` / `hold`); `give_exile` is a dumb layer write.

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

## AYA-LONG-01 — payment shaves land at `Effect`, not `PaySplitAdd`
- **kind**: implementation bug
- **class**: **closed 2026-10-21** (v96 `PaySplitMulBefore`)
- **severity**: medium
- **suggested batch**: payment pipeline
- **locations**: `rules/cards/card-pp/src/aya_longing.rs` -- (2) 「将金额降低1500
  （最低0）」 rides `Action::PaySplitMulBefore`; (1) X stays a pure
  `PaySplitAdd` filter above it.
- **was**: the interactive half landed at `Effect` (the declaration), so sibling
  add modifiers saw the 1500 already gone. The 2026-10-20 entry called this
  class A; the code TODO had already reclassified it B -- correct, one shared
  capability with AVE-CLAW-01.
- **residual (`TODO(规则书)` in the body)**: local `.max(0)` per clause vs a
  final floor after the whole add/sub stage is still unnamed by the book.
  Pinned by `money.rs`
  `aya_longing_1500_shave_lands_after_sibling_pay_split_add`.

## AVE-CLAW-01 — bend clawback latches the declared amount, not the `PaySplitMul` input
- **kind**: implementation bug
- **class**: **closed 2026-10-21** (v96 `PaySplitMulBefore`)
- **severity**: medium
- **suggested batch**: payment pipeline
- **locations**: `rules/skills/skill-groups/src/ave_mujica.rs` -- `latch_orig`
  rides `Action::PaySplitMulBefore`, so `ORIG` is the figure the 1.5x bend is
  about to see (after any `PaySplitAdd` rewrite).
- **was**: stored the *declared* amount, so whenever a `PaySplitAdd` also
  rewrote the figure the clawback delta was against the wrong baseline.
- **pinned by** `money.rs` `ave_mujica_clawback_uses_post_add_pay_split_mul_input`.

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
else in the 153-tag sweep already had a home (see the index above).

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

# Batch 2026-10-21 (`gap-batch8-meda`) — proposed-batch-8 sweep + wave 3 check

Take from "Proposed batch order" item 8 (medium-A contained bugs). Most of that
item turned out to be **already landed or reclassified**; the batch became an
accuracy pass plus the one shared class-B capability that the two remaining
payment bugs needed.

## Closed this batch

- **SKILL-07** — already fixed in `0ce220b1` (2026-10-20 tree, unnoticed by
  that day's audit). Expiry verified: `soyo_clear.rs` `COLOURED` / `declare_cap`,
  pinned by `mygo.rs` `soyo_recolour_expires_at_next_turn_start`. Residual
  「以此法免除」 attribution stays under **SKILL-06**.
- **CRYSTAL-08** (rise half) — already fixed in `0ce220b1`. `apply_view` +
  `Action::HouseAdded` track a build past the play snapshot; pinned by
  `cross_tiles.rs` `t10b_ringing_bloom_tracks_a_later_build`. Fall half and the
  non-discard disarm are **class B** (no `HouseRemoved` kind); （3） X stays
  **C-Q36**. Entry rewritten, not deleted.
- **LOW-02** — docs only, this batch. `rules/tiles/src/property.rs` module doc
  now cites both live sites (rent-table length / `why_not_build_on` for the
  level cap; `Cx::offer_mortgage_funds`'s `exclude` for the upgrade-funding
  bar). The 2026-10-20 entry had the second site wrong (`mortgage`).

## Closed by the engine work on this branch

- **AYA-LONG-01** and **AVE-CLAW-01** — one shared root cause: the money
  pipeline had no writable interactive stage between `paySplitAdd` and
  `paySplitMul`. Both card bodies were latching / shaving at `Effect` (the
  declaration) instead. **v96 `PaySplitMulBefore`** (engine id 110) is that
  stage; `aya_longing.rs` (2) and `ave_mujica.rs` `latch_orig` moved onto it.
  The 2026-10-20 batch table listed both as class A; the code TODOs had already
  reclassified them as B — correct, one ABI bump closes both. Pinned by
  `money.rs` `aya_longing_1500_shave_lands_after_sibling_pay_split_add` and
  `ave_mujica_clawback_uses_post_add_pay_split_mul_input`.
- **W3-KAORU-01** (the wave-3 high) — `kaoru_thief` 「经过该玩家」 re-homed off
  `passPlayer` onto `passTile`; pinned by
  `settle_stages.rs` `m4_kaoru_thief_force_stop_stops_on_the_marked_player`.

## Verification (2026-10-21)

Run from `/home/monopoly/gap-batch8-meda-wt` after
`node tools/build-ruleset.mjs` (ruleset-tests loads the card **wasm**; a Rust
edit to `rules/cards` is invisible to it until that is re-run):

- `cargo test -p game-kinds -p card-sdk --no-fail-fast` — green.
- `cargo test -p game-rules --no-fail-fast` — green except
  `--test settle_depth`, which **stack-overflows on master too** (pre-existing,
  not this batch).
- `cargo test -p ruleset-tests --no-fail-fast` — every target green except two
  tests that **fail identically with master's code** in this same harness
  (checked by `git checkout 41345b5c --` of the four changed rule files plus
  `play.rs`, rebuild the wasm, re-run -- both still fail; restored after):
  - `cross_status::s05_crychic_band_six_cards` — "P1 neither pays nor
    receives: 9000 != 10000". The CRYCHIC band (1) 「无法获得或失去资金」 lock
    (a `PaySplitChoose` cancel) does not void P1's leg of 登上武道馆. This is
    **SETTLE-02 regressing** — the 2026-10-17 audit recorded it as fixed and
    the test un-ignored/green. Re-open below.
  - `gap_money::g02_half_speed_plus_expectations_floor_on_a_rent` — fails in
    the **arrangement** assert ("landed on the rent tile: 32 != 29"), before
    any money assertion. Movement/setup, not the payment pipeline.
- `cargo check --workspace --all-targets` — clean (warnings only).

**Pre-existing, not this batch** (verified above; filed so they are not lost):
- **REOPEN SETTLE-02** — `s05_crychic_band_six_cards` red again.
- **TEST-ARRANGE g02** — `g02_half_speed_plus_expectations_floor_on_a_rent`
  stops at "landed on the rent tile: 32 != 29".
- **settle_depth stack overflow** — `game-rules --test settle_depth` aborts
  with a stack overflow on master too; the name suggests it is the STACK-01
  guard, which the 2026-10-10 audit recorded as fixed. Worth a look.

## Wave 3 clause check (checking half)

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

### W3-KAORU-01 `HHW:（薰）怪盗hello happy` — 「经过该玩家时」 is wired to end-tile 重叠, not mid-route [经过]
- **class**: **A — closed 2026-10-21** (this batch)
- **severity**: high
- **rule text**: 「你本回合的移动阶段可以选择在经过该玩家时使自己强制停下并触发结算。」
- **code**: `rules/cards/card-hhw/src/kaoru_thief.rs` — was
  `On::Action(Action::PassPlayer(…))`; the engine's `passPlayer` is overlap at
  move **end** only (`crates/game-core/src/engine/play.rs:1812-1834`).
- **was**: The body then `plan::set_stop_at` + `set_resolve(true)` — a mid-walk
  force-stop — but `move.remaining > 0` is already false when `PassPlayer`
  fires, so the printed optional stop-and-settle never engaged during a walk
  past the mark. Same latent bug as the 凑友希那 case in SETTLE-STAGES §4 M4,
  which the original sweep missed.
- **now**: re-homed onto `Action::PassTile` (行动阶段 12 [经过], per path step)
  with the mark-match as a board read (`player_pos(marked) == trigger::tile()` —
  a `passTile` payload has no `target`). Residual comment un-inverted
  (`trigger::target()` is the overlapped player, not the mover). Pinned by
  `settle_stages.rs` `m4_kaoru_thief_force_stop_stops_on_the_marked_player`.
  `SETTLE-STAGES.md` M4 lists the card.

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

### Wave 3 — already filed / already fixed (not duplicated)
- **CRYSTAL-06 / C-Q35** (薰 charge/decay) — open, C, low.
- **CH-13** (Here the world two-cards) — **fixed** 2026-10-13.
- **C-Q33** (Here capture vs auto-play) — open question.
- **C-Q44** (Door co-owned hill) — open question, `t22_hill_coownership` ignored.
- NNM (2) 「hard-codes 2000」 — **stale claim, already fixed**. `gain_circle_money`
  raises `PassRewardAffected` / `REWARD_MONEY` and pays the live figure (test
  `nnm_option2_gains_x_times_circle_money_reward` expects Morfonica's 1000).
  The CRYSTAL-04 bullet below is out of date on this point.
- **MOVE-03 / C-Q6** multi-activation — not live on this card (NNM already has
  one `On::Play` branching on `is_placed()`).
- Entity resolution: every quoted tile / character / group / token / status on
  these five cards resolves (`data/board.json`, roster, `is_shop` = group-10
  deeds). No dangling names.

---

10. **:159–160 §10 Resolved-summary bullet "ring opens at the next seat".** Superseded by the 2026-10-07 ruling at :787-798 (ring starts at the **initial user**; exhaust before advance). The ileuxali "next seat" clause is marked Superseded at :757-763.
11. **:1379–1381 "Pre-existing … `inter_yolo_pushes_haneoka_over_20` fails with `err.play_timing` on master source too".** Stale — that test is green (un-ignored 2026-10-10).

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

## Closed in this batch (fixed on `worktree-card-loyalty-review`)

- **LOY-DISP-01 `R:向着顶点` `.text()` moved 「视为你的主要移动」.** The phrase
  belongs to the walk sentence; `.text()` had it after 「加盖一层房屋」. Behavior
  already matched the sheet (`to_the_peak.rs` walk = `card_move`, build = not;
  `spec_card_r.rs:975` asserts the sheet order). **Fixed.**
- **LOY-DISP-02 `RAS:（和奏瑞依）寄于指尖的执念` `.text()` dropped `[反击]` and
  「可打出此卡并」.** Read as an automatic keep; the code is an `On::Counteract`.
  **Fixed.**
- **LOY-DISP-03 `RAS:练习室里的风暴`（2）`.text()` kept the pre-2026-10-06
  wording.** Sheet 新卡组卡 H8 rewrote 「那个玩家进行一次…[结算]」 into
  「对那个玩家进行一次相当于…的收费」. **Fixed** (behavior still open — see
  LOY-STORM-01).
- **LOY-DISP-04 `通用:该清CP了` `.text()` showed `[特]：` and 「（2）」.** The
  sheet has no `[特]：` line and numbers the clause 「（1）」 (a stale number left
  after sheet 2026-10-06 A8 dropped a gate). **Fixed.**
- **LOY-DISP-05 `1d6mod6` vs `1d6` on four cards** (`AG:商店街的青梅竹马`,
  `AG:无论是何种颜色的夕阳`, `Mor:（筑紫）迷茫的庭园`, `R:NFO`). Live sheet says
  `1d6` (`spec/cards/r.md:103-104` already rules it authoritative). **Fixed.**
  `RAS:UNSTOPPABLE` is the reverse and stays under `OPEN-unstoppable-mod`.
- **LOY-DISP-06 `HHW:（花音）Wacha Mocha 啪嗒进行曲` dropped 「此卡」.** **Fixed.**
- **LOY-SOUL-01 `AG:绯红之魂` [手] choice capped by cash.** 「选择[消耗]1到5次
  500资金」 is a printed 1..=5; `max = min(5, money/500)` pre-empted the
  shortfall path (NEGATION-AUDIT V2 removed the money play gate). **Fixed** to
  `ask_number(…, 1, 5)`. Tests already expected 5 options.
- **LOY-RET-01 `PPP:Returns` [持续]（3） tax fired on the owner's own PPP
  presses.** 「使用**因此卡获得的**团卡的主动效果时需要支付1星星贴纸」 — the
  code keyed on the player token `returns.copied` (armed on every borrow), so
  from game start every band press paid. Now keys on the borrowed **instance**
  (`returns.borrow_uid` = the uid `place_card` returned;
  `spend_copy_sticker` compares `self_uid()`). **Fixed.**
- **LOY-RET-02 `PPP:Returns` meta `.tags` listed 「手」.** Printed card is
  [特]/[持续] only. **Fixed.**

## Open — status after the 2026-10-18 fix batch

**Closed by the fix batch** (worktree `worktree-card-loyalty-review`, see the
commit log): **LOY-TRAIN-01** (the [共鸣] branch now asks for the two marks),
**LOY-TRAIN-02** (cash-in moved to a `CounterChanged` state clause so any write
landing ≥5 fires it), **LOY-OVERLAP-01** (the missing `Discarded` hook is in;
both branches `[移除]`/banish the accident), **LOY-RET-03** (`copy_sticker_gate`
refuses the borrowed copy's press at 0 stickers), **LOY-SMILE-01** (「任何玩家」
includes the owner; (1)'s redirect and (2)'s arrival are keyed off `SLOT_REPLAY`
instead; the crystal tick is unconditional), **LOY-ANON-01** (「（上限1）」 is
pair-atomic), **LOY-STORM-01** (a one-shot 「收费」 to the tile owner, not a
`card_settle_at`).

**LOY-DREAM-01** landed as a **Class-B fallback**: `On::Action(&[TurnEndBefore],
…)` with a `TODO(规则书) LOY-DREAM-01` naming the missing turn-end
counteraction window (`is_hook_only` lists the turn-end family, so
`On::Counteract` there is a dead entry). The buy still settles before the turn
end's own effects; the [反击] declaration shape is the open half.

**Still open**: the **LOY-C pile**.

## Open

### LOY-TSUGU-01 — unprinted whole-card move gate (replay quirk worked around)
- **class**: A. **severity**: medium.
- **rule text**: `通用:@Tsugu ycm` is `[手]：投掷3d10…` with **no [限]**. Steps
  (1) / (3) and the 「[获得]1000资金」 arm of (4) have nothing to do with the
  main move.
- **code**: `card-general/src/tsugu_ycm.rs` — the whole-card `cant_play =
  rules_status::move_blocked` is gone. Only the (2) teleport arm reads the gate
  (the (4) arm already did).
- **quirk**: a `cant_move` read inside the body makes the (2) buy `ask_yes`
  disappear. Cause: after a `HostRequest` (the roll / the move) the guest body
  re-runs from the top with the answers replayed; `cant_move` now sees
  "already moved" and returns early, suppressing the later prompt. Same shape
  on `AG:（鸫）微小的『能做到』的事` (its own follow-up [反击] re-runs the body).
- **workaround**: write the arm's destination into a per-player slot *before*
  the move and treat `player_pos == destination || slot == destination` as
  "already moved here", so the gate is only consulted on the first pass. The
  buy offer stays ungated.
- **status**: **fixed** (2026-10-28 batch). Pinned by
  `tsugu_effects_land_when_the_move_is_spent` plus the existing 4 (2) tests.
  The replay replaying host answers by position is still the underlying model;
  an arg-identity check in `request_or_pause` would let cards read the gate
  plainly.

### tsukushi_garden / your_light / overlapping_voices whole-card move gates
- Same unprinted-gate class. `morfonica:tsukushi_garden` and
  `pp:overlapping_voices` drop the gate and guard only their move arm;
  `morfonica:your_light` keeps the refusal for its distribution branch and
  guards the start arm. `your_light`'s start arm keeps a
  `TODO(规则书) OPEN-移动起点` (whether a teleport counts as 「移动起点」).
  Pinned by `tsukushi_gains_and_guards_even_under_cant_move`,
  `light_play_survives_cant_move_but_the_start_waits`,
  `overlap_steps_land_when_the_move_is_spent`.
- **status**: **fixed** (2026-10-28 batch).

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

### LOY-TRAIN-01 `PP:练习生解密指南` [共鸣] pays 500 and never adds the two marks
- **class**: A. **severity**: high.
- **rule text**: 「如果[共鸣]则[消耗]500资金并添加任意2个标记。」
- **code**: `card-pp/src/trainee_guide.rs:121-141` — **fixed**: the [共鸣]
  branch pays 500 and then asks the two mono/dual picks.
- **status**: **fixed** (2026-10-18 fix batch).

### LOY-TRAIN-02 `PP:练习生解密指南`（3）≥5 crystals only sampled in `turn_end`
- **class**: A. **severity**: high.
- **rule text**: 「（3）此卡拥有至少5个[奇迹水晶]时…」
- **code**: `card-pp/src/trainee_guide.rs:44` — **fixed**: the cash-in rides an
  `On::Action(Action::CounterChanged("…card.counter('crystals') >= 5"))`, so any
  write landing ≥5 (the [手] placement included) fires it. Pinned by
  `guide_cash_in_fires_at_placement_when_crystals_ge_5` and
  `guide_cash_in_fires_when_turn_end_crosses_5`.
- **status**: **fixed** (2026-10-18 fix batch). Residual build-off layers are
  the `PP:练习生解密指南` bullet in the LOY-C pile.

### LOY-OVERLAP-01 `PP:[衍生]重叠的声音` [特] discard hook is missing
- **class**: A. **severity**: high.
- **rule text**: 「[特]：此卡进入拥有此卡的玩家的弃卡区时[移除]拥有此卡的玩家的
  [场地]上的“初次演出事故”（如果有）。」 — only [特] banishes; the [手] step 1
  wording 「[移除]此卡和[使用者][场地]上的“初次演出事故”（如果有）」 is a
  separate clause.
- **code**: `card-pp/src/overlapping_voices.rs:34-37` — **fixed**: an
  `On::Action(Action::Discarded("actor == owner && trigger_card == card.id"))`
  banishes the accident. Pinned by `overlap_discard_hook_banishes_accident`.
- **status**: **fixed** (commit 23da8116, 2026-10-18). The accident is taken off
  the field with `unplace_card_named`, which has `[移除]` semantics (the
  instance and its bound tokens die).

### LOY-RET-03 `PPP:Returns` 「需要支付1星星贴纸」 is not a cost gate
- **class**: A. **severity**: high.
- **rule text**: 「使用因此卡获得的团卡的主动效果时**需要支付**1星星贴纸。」
- **code**: `skill-groups/src/lib.rs` — `add_tok(…, -1)` floors at 0, so a
  0-sticker owner still runs the press and the "payment" moves 0.
- **note**: `Err(Prompt)` is the **pause** sentinel, not a refusal
  (`card-sdk/src/lib.rs:81`), so a play body cannot refuse its own activation.
  The gate belongs on the borrowed instance's `On::Play` (`cant_play`), keyed
  the same way as LOY-RET-01's `returns.borrow_uid`.

### LOY-SMILE-01 `HHW:笑容大游行`（3）discard skips the owner
- **class**: A. **severity**: medium.
- **rule text**: 「任何玩家在此卡放置的格子上[触发结算]后此卡放入弃牌堆。」
- **code**: `card-hhw/src/smile_parade.rs:261-274` — **fixed**: `resolve_guard`
  now keys off the redirect tag (`trigger::move_tag(TAG_REDIRECT) != 0`), not a
  blanket owner skip, so 「任何玩家」 includes the owner.
- **related**: (1)'s crystal countdown (`turn_end`, `:134-137`) is now the
  printed unconditional tick; only the zero-crystal discard names 弦卷集团.
- **status**: **fixed** (2026-10-18 fix batch).

### LOY-ANON-01 `MyGO:[千早爱音]Anon Tokyo` 「（上限1）」 produces half-links
- **class**: A. **severity**: medium.
- **rule text**: 「并在上述的两个格子间放置1个[奇迹水晶]（上限1）」
- **code**: `card-mygo/src/anon_tokyo.rs:136-146` — the pair placement is
  **fixed**: `place_mark_pair` is place-or-refuse atomic, so a half-link can no
  longer happen (commit 6f9c5564).
- **still open**: the 消耗 runs before the refusal, so a re-link at the cap pays
  the half and places nothing -- the 消耗 and the 放置 are one body
  (in-file TODO).
- **Class C** on whether a tile may sit in two links at once (the singular
  「1个…（上限1）」 leans no).

### LOY-STORM-01 `RAS:练习室里的风暴`（2）is still a forced re-settle
- **class**: A on "charge, not settle"; **C** on the recipient when mortgaged.
  **severity**: medium.
- **rule text**: sheet 2026-10-06 新卡组卡 H8: 「…对那个玩家进行一次相当于此卡
  所在格子普通[结算]的(4-X)/4倍价格的**收费**」 (was 「那个玩家进行一次此卡所在
  格子的[结算]」).
- **code**: `card-ras/src/studio_storm.rs` — **fixed 2026-10-18**: now a one-shot
  `ctx::transfer(settler → tile owner, rent_of(tile) * (4-X) / 4)`. A full
  `card_settle_at` would also fire settle hooks / counteractions and offer
  build/buy, none of which the 「收费」 wording asks for. Amount matches the old
  `set_rent_factor(250 * (4-X))` figure (`3340 * 2 / 4 = 1670` in
  `ras.rs` `storm_part2_rent_factor`).
- **still open — Class C**: 「普通[结算]」 of a **mortgaged** tile is a forced-buy
  offer, not rent, so there is no obvious 收费 recipient. Charge nothing on a
  mortgaged tile for now (`TODO(规则书)[ruling]` on the call site); needs a
  ruling. `ras.rs` `storm_part2_discards_and_charges` renamed from
  `…_forces_a_settle` to match the sheet.

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

## Quoted-name sweep (all 180) — no unresolved entity names

Every “...” that names a thing resolves: derived cards (`“FEVER!”` →
`通用:[衍生]FEVER!`, `“觉悟”` → `R:[衍生] 觉悟` **with a space in the id**,
`“拍卖撤下来了”`, `“表演的本能”`, `“共鸣”`, …), tiles (`“bandori车站”` vs board
`Bandori车站` — sheet case drift; impls use the canonical spelling via
`const STATION`), `“MyGO!!!!!”` → `in_group`, `“抹茶芭菲”` → a player token.
Quoted formulas are expressions. `“afterglow”` is a named movement style and
already carries `TODO(规则书)[judgement]` in `detour.rs`.

Sheet typos deliberately **not** propagated into `.text()`: `“FEVER!“` /
`“PERFECT“` (closing quote), 「1000的的的资金」.

# Newly filed 2026-10-18 (card loyalty review, wave 2)

15 more quote-dense cards reviewed clause-by-clause (3 subagents × 5). Reports:
job tmp `wave2-{a,b,c}.md`. **32 new findings** (5 high, 8 medium, rest low).
Display text / quoted names were clean on all 15 — the problems are behavioral.

## High

### LOY-BLACK-01 `HHW:黑衣人的补给` 「移除那格的一个奇迹水晶」 is dead code
- **class**: A. **severity**: high.
- **rule text**: 「你经过“弦卷集团”格子后，移除那格的一个奇迹水晶。」
- **code**: `card-hhw/src/black_suits.rs:84-91` — **fixed**: `counteract` now
  places the card as the C# `BlackSuitFx` stand-in (the 育美 pattern), so the
  `Action::Pass` removal (`:141-160`) dispatches. Pinned by
  `black_clothes_supply_removes_a_crystal_after_passing_the_group_tile`.
- **status**: **fixed** (2026-10-18 fix batch). Related medium, still open and
  carried as the in-file TODO on `sync_circle_instance`:
  「该格上拥有奇迹水晶时」 is only re-evaluated by this card's own place/pass,
  not by any external crystal add/clear.

### LOY-SEEYOU-01 `PP:[衍生]明天见` a declined play is still announced as played
- **class**: A. **severity**: high.
- **rule text**: 「（1）只有[使用者]的正面[P✽P粉丝]数量少等于反面[P✽P粉丝]数量才可
  使用。」+「（2）[共鸣]无视此卡的[特]效果（1）。」
- **code**: `card-pp/src/see_you_tomorrow.rs:45-83` — `cant_play` allows whenever
  a 共鸣 is in hand even if `up > down`; the body `return Ok(())` on a declined
  `try_resonance`.
- **gap (card fate — fixed)**: the body now sets `ctx::Dest::Hand` before
  returning, so declining keeps 明天见 and the 共鸣 in hand with no effect
  (`seeyou_declined_echo_keeps_card_in_hand`).
- **gap (announcement — open)**: the play is already announced when the body
  runs, so a refused 共鸣 still counts as a played card -- the log, the `card`
  window and the played/two-cards counters all ran and it can be counteracted
  (in-file `TODO(规则书)`). Fix: move the 共鸣 into the new activation
  collector (`docs/ACTIVATION.md` §2) and `Activation::Decline` on refusal.
- **note**: the bypass structure itself is right (gate holds → free play; gate
  fails → only via 共鸣).

### LOY-TOMORI-01 `CRYCHIC:（灯）内心的呐喊`（3）window and body are both off
- **class**: A. **severity**: high. ×2.
- **rule text**: 「…**你**位于此卡前后5格内时…」 and the (3) body 「使用高松灯
  （MyGO!!!!!）的（2）效果」.
- **code**: `card-crychic/src/tomori_inner_shout.rs:26` — **fixed**: the pre now
  requires 「你」 near the card (`dist(pos(owner), card.tile) <= 5`, plus the
  borrowed (2)'s own mover-near-you fact), and `pull` teleports the mover to the
  controller's tile (`ctx::player_pos`, `:215`). The redirect settles once at
  the destination and the mover's payments on it are halved (`Filter::PaySplitMul`),
  so the old 「no settle」 comment is gone.
- **note**: the remaining half is (2)'s 1d20 + [停留] + delayed settle, filed as
  TOMORI-BIND-01.
- **status**: **fixed** (window / destination / half-pay).

### LOY-TOMORI-02 `CRYCHIC:（灯）内心的呐喊`（2）field copies are swept by CRYCHIC (2)
- **class**: A. **severity**: high.
- **rule text**: 「此卡不会被你乐队技能的（2）效果移除」.
- **code**: `skill-groups/src/crychic.rs:149-180` — **fixed**: the band (2)
  sweep names this card (`SHOUT`), unplaces its field copy and shuffles it into
  the draw pile (`add_to_dest(..., DeckRandom)`), sparing it the plain
  `CRYCHIC:` unplace. Pinned by
  `crychic_band_2_spares_the_field_copy_but_sweeps_other_crychic_cards`.
- **status**: **fixed**.

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

- **LOY-JEN-01** `PP:找回珍妮弗` [med/A] — **fixed 2026-10-20** (600241be): the
  steps run in printed order (pay → fans → [移除] → Ranger,
  `jennifer.rs:111-134`; `jennifer_runs_its_steps_in_printed_order`).
- **LOY-FEVER-01** `通用:[衍生]FEVER!` [med/A] ×2 — **fixed** (dcca74d8):
  （2） now `set_transfer_to_dest([使用者], Graveyard)` (`fever.rs:125`) and X
  counts every field instance with no face-down filter (`fever.rs:86`);
  `fever_leaves_to_the_users_discard_pile` / `fever_x_counts_face_down_field_cards`.
- **LOY-THANKS-01** `通用:CiRCLE THANKS PARTY!` [med/A] — the `money_of < 500`
  cash gate is **fixed** (dcca74d8). Residue: `can_pay` (晕眩/除外 PayGate)
  still narrows 「所有其他玩家」 (`thanks_party.rs:47`, in-file TODO).
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
- **LOY-JEN-02** `PP:找回珍妮弗` — skipped steps when the user is
  `player_out`; text has no escape. **Fixed**: the `player_out` conjuncts are
  gone, so a user bankrupted by step 1 still gets step 2's fan and step 3's
  Ranger (`jennifer_steps_continue_after_the_user_is_bankrupted`).
- **LOY-BLACK-02** `HHW:黑衣人的补给` [low/C] — the borrowed pass-reward's
  「移动起点不为CiRCLE」 exemption does not re-bind (弦卷集团 is kind `agent`, so
  `is_start` never applies). Two readings of 「获得"CiRCLE"格子的全部效果」.
- **LOY-RANDOM-02** `PPP:仓库里的Random Star` [low/C] — (2) offer suppressed on
  landing step and teleports (`move.remaining > 0 && move.kind != Teleport`).
- **LOY-WAIT-01** `PPP:（有咲）等等等一下` — （2） is already filed as ABI-09 (a)
  `IEventDefer`. (1)'s 「非因为此卡导致」 filter is vacuous until Defer lands
  (OPEN-wait-causality).

# Engine verbs (2026-10-18)

The `/simplify` pass on the loyalty batch deferred four fixes that needed
engine capabilities rather than rule-side bandaids. Three landed here.

## Landed

### FIRE-TOKENS — 火罐 re-homed onto the persona-skill token counter (2026-10-24)
Branch `worktree-fire-tokens`. `MatchPlayer.state["charge"]`/`["tempFire"]` →
`tokens` rows `name="charge"`/`"tempFire"`, `instance=<persona-skill uid>`,
live cap on `Counter.max`. Wire names unchanged. `gain_marker` /
`spend_marker` / `add_marker_max` fold the held-token store and stay dumb
(uncapped add). The uncapped `gain_charge`/`spend_charge`/`add_charge_max`
sugar is deleted. Skill press-gates and cond-ir `charge(p)` read the token.
Kani `arm_gain_marker` was aligned to the production uncapped add in the
same batch (it had modelled a `charge_max` clamp the engine does not have);
`conf_hostops2` / `conf_model` unit tests are green. The CBMC proof suite
itself was **not** run this batch.

Open follow-ups from the code review of this batch (2026-10-24):

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

### ENG-CARD-MESSAGE — `Target::Board` reaches tile rules; CiRCLE reward is messaged
- `Target::Board(id)` / `Target::Uid(uid)` now resolve over **every** standing
  board-owned rule on `BOARD_OWNER` (tile rules, marks, events, status hosts),
  not just `mark_rule_instances()` (`crates/game-rules/src/hostfns.rs`
  `resolve_receivers`). The id is what disambiguates; `Target::Board`'s shape
  is unchanged so this is a strict superset. Docs: `docs/CARDS.md`
  「Cross-card messages」.
- **Consumer:** `event:协助CiRCLE重建` re-homes 「CiRCLE原本的所有效果」 onto
  「CiRCLE咖啡厅」 by messaging `tile:circle` (`On::Message("settleReward")`)
  instead of calling `rules_tiles::circle::settle_reward` across modules. The
  reward body runs **as `tile:circle`** (its prompt, its `passRewardAffected`
  window, its `prop::NO_REWARD` read) and is now private to that module.
  `rules/events` no longer depends on `rules-tiles` -- the only production
  card→card body call in `rules/*` is gone. Test:
  `circle_rebuild_pays_the_reward_on_a_cafe_pass`.
- Also fixed on the way: the rebuild event's `prop::NO_REWARD` arm lives on
  the `tile:circle` instance (the source owns arming, the reader is the tile),
  so expiring the event did not drop it and the **next** pass of CiRCLE was
  silently unpaid. The event now declines to re-arm on the expiring pass
  (order-independent: the reward step either already consumed the armed veto
  or will). Test: `circle_rebuild_restores_the_reward_after_it_expires`.
- The remaining half of the `TODO(ABI)` in `circle_rebuild.rs` -- a real
  temporary tile-rule swap -- is **ABI-13**.
### SUITE-TRIO — three long-standing red tests, 2026-10-21
Three tests that had been red on master, each a different hole. All three
failed identically on `d70086ed` / `43f2efc9`, so none was an in-flight
regression.

- **`cross_status::s05_crychic_band_six_cards`** (SETTLE-02, reopened) —
  `Cx::money_inner`'s settlement-stage loop folded only `t.value` / `t.target`
  and **dropped `t.is_cancelled()`**, so a Flag-shaped `paySplitChoose` answer
  of `false` (contract: "stops the payment", abi.rs; 规则书 支付阶段 5
  「取消支付 / 无需失去资金」 G-MONEY-14) was ignored. Every other cancel
  point in that function (`effect`, `pay`, `payCombinedCancel`) already
  honoured it. Fixed in `crates/game-core/src/engine/play.rs` by mirroring
  `payCombinedCancel`. Side effect: `Mujica:#J11`'s `pay_choose_cancel`
  (stun cancel) was documented as dead and is now live.
- **`gap_money::g02_half_speed_plus_expectations_floor_on_a_rent`** —
  `RollValue`'s payload constructor read `trigger::value()` (the raise's
  *original* face) rather than `trigger::move_roll()` (the running face after
  `set_move_roll` rewrites). With two DiceRollValue modifiers the second
  folded the original face and the last writer won: 7 → (−2) → 5, the ½ lost.
  Fixed in `crates/card-sdk/src/payload.rs` to use the same
  `move_roll().unwrap_or(value())` idiom the roll skills already use.
  **Open ruling on the composition order** -- see `OPEN-ROLLRESHAPE-01` below;
  the fix makes the two reshapes *fold* (previously one clobbered the other),
  but which order they fold in is a separate question the card text does not
  settle.
- **`spec_card_general::fever_gain_boost`** — `通用:[衍生]FEVER!` stored its
  C# `Card.User` Mem in the **per-player keyed-state map** (`fever_user`), so
  playing it materialised a default-valued state key the
  `C-GEN-fever-frame` allowlist correctly rejects. The Mem belongs on the
  card instance (`FieldCard::props`, as `RAS:EXIST`'s `exist.used` does) and
  is 1-based so seat 0 is representable. Fixed in
  `rules/cards/card-general/src/fever.rs`; the frame is untouched and the
  「放入[使用者]弃卡区」 routing still works. New test
  `fever_user_mem_lands_on_the_placed_instance` pins the placement (the
  owner==user case cannot tell a right write from a lost one).

### ENG-REDIRECT — host-visible "this settle is a redirect" flag
- `MoveCtx.settle_redirect: bool` (`crates/game-core/src/engine/move_ctx.rs`),
  set **only** in `card_settle_move` (`play.rs`) when the engine runs a
  card-initiated `card_settle_at`. Plan-seed sites force it false so it cannot
  stick.
- `card_sdk::ctx::trigger::is_settle_redirect()`, wired through
  `sys`/`sys_kani`/`Op`/`hostfns`/`native_shims`/`host`/`wasm_rules`/kani.
- **Consumer:** `HHW:笑容大游行` drops `SLOT_REPLAY` entirely. The nested
  settle's flag stops the ping-pong, so **SETTLE-01's reverse direction is
  implemented** — a settle on 弦卷集团 while the card sits elsewhere now runs
  as the card's tile (`g30b_smile_parade_reverse_swap_runs_as_card_tile`).
  `TAG_AS_AGENT` stays: it marks (1)'s outer placement-move endpoint, which the
  nested-move flag does not.

### ENG-CHARGE-RENT — a rent-shaped 「收费」 that is not a settle
- `ctx::charge_rent(payer, tile, milli_scale) -> paid` (1000 = ×1.0). Computes
  the rent **as a settle would** (`rent_figure`, shared with `pay_rent`), scales
  it at the money pipeline's `paySplitMul` stage alongside the rent factors
  (`Pay::milli_scale`, `spec/ROUNDING.md`), and runs it through `money()` as
  `PayKind::Rent` — so `pay_is_rent` / `RENT_FACTOR` / `PaySplitAdd` riders see
  it and `paid_in_settle` is tallied, but settle hooks and buy/build offers do
  not.
- Refuses (0) on unowned / mortgaged / payer==owner. Mortgaged is Class C
  (`TODO(规则书)[ruling]`) — 「普通[结算]」 there is a forced-buy, not a rent.
- **Consumer:** `RAS:练习室里的风暴`（2）on the verb (was `transfer`+`rent_of`).
  Unblocks the same shape in `anon_tokyo`'s `link_rent` and `dream_return`'s
  `rent_of * 2` (not migrated here).

### ENG-PAIRED-MARK — one mark bound to two tiles
- `TileMark.partner: Option<i32>` (`crates/game-core/src/state.rs`),
  Serde-defaulted like `src`/`instance` so pre-partner saves load.
- `ctx::place_mark_pair(a, b, …) -> bool` — place-or-refuse **atomic**: returns
  false and places neither row when either end already carries a matching mark.
  `ctx::mark_partner(tile, filter) -> i32` reads the other end.
- Rows carry `src`/`instance` like any mark, so `destroy_instance_units` drops
  them with the creating instance (no second lifetime mechanism).
- **Consumer:** `MyGO:[千早爱音]Anon Tokyo` drops its `LINK{tile}` slot graph.
  The field stand-in is now placed *before* the crystal so both rows bind to its
  `instance` (they were legacy `instance: None` rows that would never die).
  Class C kept: whether a tile may sit in two links; the singular
  「1个…（上限1）」 leans no and the either-end refusal is that reading.

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
    possible at all -- `SUITE-TRIO`)
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

# Summary counts

**2026-10-21 addendum — `circle_rebuild` TODO(ABI) sweep.** The two
`TODO(ABI)` tags in `rules/events/src/circle_rebuild.rs` were filed and the
tractable half landed. Card→card messaging already existed (`On::Message` /
`ctx::send`, ABI v50) but only ever reached `mark:cp`; `Target::Board` now
resolves any standing board-owned rule, and the rebuild event messages
`tile:circle` for the re-homed [经过] reward instead of calling into
`rules-tiles` (that crate dep is gone from `rules/events`). The stale
`prop::NO_REWARD` arm that ate the first CiRCLE reward after the event expired
is fixed. The tile-swap half is **ABI-13** (new, MEDIUM) -- a real capability
gap, not a bug. Four tests added under `rules/tests/tests/events.rs`.

**Recounted 2026-10-20** against master `b8e26436` after the card-rule TODO
review. The 2026-10-10 figures (78 open from 144 TODOs + ~85 ignores) and the
2026-10-17 figures (157 TODOs, ABI v85) are historical.

**2026-10-18 addendum** — card loyalty review wave 1 (15 quote-dense cards +
a `.text()`/quoted-name sweep of all 180): **8 display/behavior items closed**
(LOY-DISP-01..06, LOY-SOUL-01, LOY-RET-01/02) and **11 opened**.
The same day's **fix batch** closed 7 more (LOY-TRAIN-01/02, LOY-OVERLAP-01,
LOY-RET-03, LOY-SMILE-01, LOY-ANON-01, LOY-STORM-01) and landed LOY-DREAM-01 as
a Class-B fallback. **Wave 2** (15 more cards) opened **32** items (5 high).
See "Newly filed 2026-10-18 (wave 1)" and "(wave 2)". **140 cards remain
unreviewed at clause level.**

**2026-10-20 addendum — card-rule TODO review + reconstruction.** Walked all
153 `TODO(规则书)` tags in `rules/` + `crates/` against the backlog and against
the live code:
- **7 stale tags settled** (D-02 NEGATION-AUDIT V2 leftovers) — rewritten as
  `NEGATION-AUDIT V2 (settled)` notes in the six card bodies + `money.rs`.
  160 → 153 tags. Of the 153, 7 are convention markers and **146 are live gaps**.
- **2 orphans filed**: DONUT-VOID-01 (`two_donuts.rs` barred-acquisition void),
  ABI-MARKER-SPENT (`fireSpent` → `MarkerSpent`, previously an unfiled
  "separate ticket").
- **Every other tag already had a home** — the tags map onto the items in this
  file (the old standalone cross-ref index was folded away in the 2026-10-27
  cleanup). Notable clusters: CH-07 「视为此卡未生效」 is 7 card-body tags on one
  observer; the settle-priority rule is EVENT-03 / C-Q40h across
  `circle_rebuild.rs` + `tile_text.rs`.
- **Verified live** (not stale): ruling 6 / 7 (force-buy and auction still skip
  `bought`/`buyAfter`; auction money is still a direct delete), ruling 8
  (MONEY-05 巴 base), ruling 10 (LINGER-10), GUARD-PERVISIT /
  GUARD-SETTLE-DEPTH (book states no bounds). STA-01 and STA-04 are closed
  (2026-10-18) and stay closed.
- **Header drift fixed**: ABI v85 → **v89** (markers / Phase C batch 2 /
  status-vocab-zero landed after the last audit).

What the 2026-10-17 audit changed (kept for the record):
- **4 items closed** (the tree had landed them and the doc had not noticed):
  SETTLE-02 (both halves — the `cantPlayHand` B-split is `FilterKind::PlayGate`),
  SKILL-02 (hina lottery borrow), SKILL-03 (`prop::PLAYABLE_STUNNED`), CD-01
  (CEL `is_live_house` sugar deleted).
- **3 items partially closed**: ABI-09 (host clocks landed; 5 sub-caps open),
  HOST-02 (tables + `MoveBefore` landed; 行动阶段 3/6 + 支付阶段 3 open),
  EVENT-04 (song-table half gone; `end_it_all` NON_DERIVED remains).
- **22 new items filed** under "Newly filed 2026-10-17" (STA-01..05, AYA-GRANT-01,
  AYA-POOL-01, EVE-01, HAG-01, NYAMU-01, DETOUR-01, TOMORI-BIND-01, NFO-CANCEL-01,
  AYA-LONG-01, AVE-CLAW-01, LISA-BOND-01, HARU-SOYO-FX, DRAW-OUT-01, FORCE-WRITE-01,
  LINGER-10, NOBUY-CARD-01, GUARD-PERVISIT, GUARD-SETTLE-DEPTH) — each had a live
  `TODO(规则书)` and no backlog id.
- **2 live RULING tests given a home**: `t22_hill_coownership` → C-Q44,
  `g29_maya_look_at_top_inside_the_draw_curve` → C-Q45.
- **All path cites** moved to `skill-personas/` / `skill-groups/`; several kind
  names updated for the v85 `ActionKind`/`FilterKind` system.

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

## Per class (open items only; D excluded)

| class | meaning | open items (2026-10-20) |
|---|---|---|
| **A** | fixable now from clear text + current ABI | ~40 |
| **B** | needs a new/changed ABI capability | ~30 |
| **C** | needs a user ruling | ~32 |
| **D** | stale (already fixed / landed) | 6 note groups + the closures above |

*(A+B+C ≈ 100 open root-cause items after dedup of 146 live `TODO(规则书)` gap
tags + 11 RULING ignore rows + the 2026-10-10 ABI audit + the loyalty waves.
Treat these as approximate — the per-item entries are the source of truth.
Some A items are "contained engine bug" one-liners; some B items unblock
several A/C.)*

## Per kind

| kind | count (approx) |
|---|---|
| implementation bug | ~38 |
| ABI defect | ~30 |
| rulebook divergence | ~22 |
| rulebook gap / implementation debt / tooling | ~6 |
| stale | 6 note groups |

## Per suggested batch

| batch | items | severity mix |
|---|---|---|
| payment pipeline | MONEY-01..08, SETTLE-06, SETTLE-07, ABI-11, ABI-12, NFO-CANCEL-01, AYA-LONG-01, AVE-CLAW-01, FORCE-WRITE-01, NOBUY-CARD-01 | 4 high C |
| movement | MOVE-06..15, ABI-02, CRYSTAL-07, EVE-01, LISA-BOND-01 | (MOVE-01..05 fixed) |
| counteraction windows | CH-08..13, WINDOW-03, ABI-03, GUARD-PERVISIT | 1 high C (WINDOW-03) |
| rent multipliers | MONEY-01, TILE-01..03, CRYSTAL-08 | 1 high C |
| host ABI: out-buffers / queries | ABI-01, ABI-02, STA-02, STA-05, HOST-01 | 1 high B (HOST-01) |
| HostRequest handlers / reply sentinels | ABI-04, ABI-09, HOST-05 | (HOST-06/07/08/13 fixed) |
| crystal/marker accounting | CRYSTAL-02/05/06/07/08, STA-01 **closed**, STA-04 **closed**, HAG-01, HARU-SOYO-FX, MONEY-07, **DONUT-VOID-01**, **ABI-MARKER-SPENT** | 1 high via CH-07 |
| draw/discard pipeline | ABI-07, ABI-08, DRAW-OUT-01, CH-13 fixed | |
| skills: PP | AYA-GRANT-01 **closed (not a bug)**, AYA-POOL-01 | 0 |
| skills: Roselia / CRYCHIC | SKILL-05, SKILL-07, TOMORI-BIND-01 | |
| skills: Mujica | SKILL-04, CH-08, NYAMU-01 | |
| skills: CiRCLE / sumimi / HHW | SKILL-06, SKILL-10, STA-03, CRYSTAL-05/06, SETTLE-01 reverse | |
| settle stages | SETTLE-05, MOVE-08, HOST-02 remainder, LINGER-10, GUARD-SETTLE-DEPTH | |
| events | EVENT-01..04, DETOUR-01, ABI-09 remainder | |
| engine: settle recursion / stack safety | STACK-01 **fixed**; SETTLE-01 reverse-swap TODO | 0 (stack) |
| fuzz/determinism | HOST-03 (landed) | 0 |
| tiles | TILE-06, LOW-02, HOST-04 (landed) | |

---

# Proposed batch order

Do these in order; each batch should be a PR that does not share files with the next.
**2026-10-20 rewrite** — batches 1/2/3/4/5 below are already done, kept for the record.
STA-01 / STA-04 dropped out of batch 6 (closed 2026-10-18).

1. ~~**high-A engine: settle recursion / stack safety** (STACK-01)~~ **fixed 2026-10-10**
2. ~~**high-A engine bugs — movement** (MOVE-01..05, SKILL-08, SKILL-09, TILE-04, TILE-05)~~ **fixed**
3. ~~**high-A counteraction windows** (CH-02..04, CH-06, WINDOW-01, WINDOW-02)~~ **fixed 2026-10-11/13**
4. ~~**high-A money** (MONEY-03, CRYSTAL-03, SETTLE-02, SETTLE-03)~~ **fixed** (SETTLE-02 closed in the 2026-10-17 audit)
5. ~~**high-A host ABI: reply sentinels + quote==charge** (HOST-06, HOST-07, HOST-08)~~ **fixed**
6. **high-B unblockers** — the live ones, in this order
   - ~~**AYA-GRANT-01** skill-attachment grant~~ **closed 2026-10-20 -- not a bug; the clause names the character**
   - **CH-05 play-history + nested PlayCtx** (unblocks Mortis, Mana Champion, dice_cast counters)
   - **HOST-01 HostRequest nested-settle write journal** (unblocks inline equivalence)
   - ~~**STA-01 / STA-04** status grant-verb enforcement~~ **closed 2026-10-18**
   - **CH-07 general 「视为此卡未生效」 observer** (C-Q5 first, then `CardWasted`)
7. **C questions to the user** (all of §C below, now C-Q1..C-Q45, in one sitting).
   While waiting, continue on:
8. ~~**medium-A contained bugs** (SKILL-07 soyo expiry, CRYSTAL-08 ringing_bloom
   continuous max, AYA-LONG-01, AVE-CLAW-01, NFO-CANCEL-01, LOW-02 docs)~~
   **done 2026-10-21** — SKILL-07 / CRYSTAL-08 (rise) were already landed in
   `0ce220b1` and are now verified + recorded; LOW-02 closed; AYA-LONG-01 +
   AVE-CLAW-01 closed via v96 `PaySplitMulBefore` (they were class B in
   disguise). **NFO-CANCEL-01 is still open and is class B** — `PayAfter` has
   no "PaySplitAt ran" bit — so it moved to the medium-B pile (item 9).
9. **medium-B ABI pile** (ABI-01..12, HOST-05 ask_money, STA-02/05, HAG-01,
   NFO-CANCEL-01 (moved here 2026-10-21),
   EVE-01, TOMORI-BIND-01, HARU-SOYO-FX, DRAW-OUT-01, FORCE-WRITE-01,
   **ABI-MARKER-SPENT**) — one ABI bump, many cards.
10. **medium-C leftovers + events** (MONEY-01..08, MOVE-06..15, SETTLE-05..07,
    CH-08..10, TILE-03/06, EVENT-01..04, DETOUR-01, NYAMU-01, STA-03,
    LINGER-10, NOBUY-CARD-01, **DONUT-VOID-01**) after the user answers §C.
11. **D cleanup** — ~~rewrite NEGATION-AUDIT V2 TODOs as settled notes~~ **done
    2026-10-20**; strike the stale TEST-FINDINGS / COVERAGE / CROSS-TESTS /
    NEGATION-AUDIT / SETTLE-STAGES sections listed above (all still stale as of
    2026-10-20).

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

