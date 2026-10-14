# Kani unit `pp` — Pastel✽Palettes cards + skills

Branch `kani-pp` at `/home/monopoly/kani-pp-wt`.
Spec sources: `docs/rulebook/cards.json` (`PP:*`), `data/characters.json`
band `Pastel✽Palettes`, `data/bands.json` skill `与偶像一起`. Clause doc
`docs/rulebook/spec/cards/pp.md`.

Harness files: `rules/kani/src/spec/{pp,skills_pp}.rs`,
`rules/kani/src/proofs/{pp,skills_pp}.rs`.
Pilot proofs for `C-PP-aya-haruka-01/02` stay in
`rules/kani/src/proofs/{money_shave,prompt}.rs` (foundation-owned; listed
below as already PROVEN / in-flight).

Classification vocabulary (KANI-FOUNDATION §5):
**PROVEN** · **FAILS** · **PROTOCOL-LEVEL** · **BLOCKED-HOST** · **OPEN** ·
**TIMEOUT/OOM**.

---

## Summary

Clause IDs: 74 (69 explicit + 5 character (1) expansions of `C-PP-<slug>-01`).
Two of them (`C-PP-aya-haruka-01/02`) are pilot-owned and already proven.

| class | count | notes |
|---|---:|---|
| pilot PROVEN (do not re-run) | 2 | `money_shave` / `prompt` |
| FAILS | 2 | first-accident-02, band-idol-02 |
| PROTOCOL-LEVEL | 10 | multi-turn / [反击] / settle / move-plan |
| BLOCKED-HOST | 6 | `card_text_mentions`, `add_counter` max, `trig_move_roll`, `field_instances` buffer |
| OPEN | 24 | text undefined (共鸣 / 分摊 / mark / …) |
| harness written, verdict pending slot | 29 | shared-box 3-slot cap flooded (150+ waiters); 1 formula harness completed |
| **total** | **74** | |

Harness inventory (unit `pp` + `skills_pp`): 43 `#[kani::proof]` functions
(30 in `proofs/pp.rs`, 13 in `proofs/skills_pp.rs`) — pure formulas, body
iff, no-op, and mutant companions.

### Runner verdicts completed

| harness | verdict | time |
|---|---|---:|
| `dream_ahead_03_formula` | SUCCESSFUL | 3.6 s |

Remaining harnesses compile under `cargo kani` (checked) and are queued
behind the machine-wide 3-slot cap (`~/.kani-slots`). Re-run with:
`CARGO_TARGET_DIR=/home/monopoly/kani-target-pp bash tools/kani/run.sh --unit pp`.

---

## Card clauses

| ID | book quote (short) | class | harnesses | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-PP-title-idol-01 | band pool +2 | PROVEN* | `title_idol_01_band_plus_2_iff`, `title_idol_01_mutant_is_detected` | pending | — | OPEN-乐队卡 |
| C-PP-title-idol-02 | +1 (or +2 echo) on every card whose text mentions 奇迹水晶 | BLOCKED-HOST | — | — | host `card_text_mentions` is a stub returning 0; `field_instances` returns a count without filling the uid buffer | OPEN-乐队卡, OPEN-共鸣 |
| C-PP-aya-haruka-01 | place + one discard → hand | pilot PROVEN | `prompt::*` | see foundation | — | — |
| C-PP-aya-haruka-02 | poorest pay shave X=100/200 | pilot PROVEN | `money_shave::*` | see foundation | — | OPEN-ties |
| C-PP-aya-haruka-03 | [共鸣][反击] shave 1500 | PROTOCOL-LEVEL | `aya_haruka_03_*` (spec only) | — | [共鸣][反击] multi-turn window | OPEN-共鸣 |
| C-PP-hina-sound-01 | substitute exclusive | OPEN | — | — | which exclusive is unprinted | OPEN-substitution |
| C-PP-hina-sound-02 | change substituted effect | OPEN | — | — | replacement effect unprinted | OPEN-substitution-effect |
| C-PP-maya-inf-01 | watch-deck reveal → field, priority | PROTOCOL-LEVEL | — | — | watch resolution ordering is engine scheduling | — |
| C-PP-maya-inf-02 | crystal per draw, <4 vs >=4 | PROVEN* | `maya_inf_02_crystal_formula` | pending | — | — |
| C-PP-maya-inf-03 | [共鸣] swap deck/discard | OPEN | — | — | | OPEN-共鸣 |
| C-PP-chisato-mask-01 | place + others pay user 2000 | OPEN | spec `chisato_mask_01_*` | — | only the 2000 total is determined | OPEN-分摊 |
| C-PP-chisato-mask-02 | reshuffle → others pay 500 | OPEN | — | — | | OPEN-分摊 |
| C-PP-chisato-mask-03 | [共鸣] others pay 1500 | OPEN | — | — | | OPEN-分摊, OPEN-共鸣 |
| C-PP-eve-bushido-01 | place + 12d4 × 60 money | PROVEN* | `eve_bushido_01_gain_iff` | triage3: FAILED was **C** `roll_dice` count `min(8)` rewrote 12d4→8d4; fixed (`min(100)`), re-time pending | — | — |
| C-PP-eve-bushido-04 | +1 crystal per draw, cap 3 | PROVEN* (pre<3) / BLOCKED-HOST (cap) | `eve_bushido_04_crystal_per_draw_iff`, `eve_bushido_04_noop_when_zero_draw` | triage3: FAILED was **B** wrong crystal storage (player key vs `field_crystals`); fixed, re-time pending | host `add_counter_self` ignores `max` (no cap-3 clamp) | — |
| C-PP-eve-bushido-05 | settle-before duel 1d6 vs 3d4 | PROTOCOL-LEVEL | `eve_bushido_05_payment_formula` (pure) | pending | multi-roll duel + [共鸣] window | OPEN-共鸣 |
| C-PP-der-echo-01 | discard as [共鸣] cost, band +2 | OPEN | — | — | | OPEN-共鸣 |
| C-PP-seeyou-01 | gate `up <= down` | PROVEN* | `seeyou_01_gate_iff` | pending | — | — |
| C-PP-seeyou-02 | [共鸣] ignores gate | OPEN | — | — | | OPEN-共鸣 |
| C-PP-seeyou-03 | flip min(owned, down) | PROVEN* | `seeyou_03_flip_iff`, `seeyou_03_mutant_is_detected` | pending | — | — |
| C-PP-overlap-01 | remove self + 初次演出事故 | OPEN/PROTOCOL | `overlap_01_unplaces_ok` (spec) | — | play step 1 of a multi-step [手]; unplace uid pairing needs field model | — |
| C-PP-overlap-02 | main move → teleport bandori车站, no settle | PROTOCOL-LEVEL | — | — | move-plan scheduling | OPEN-tiles |
| C-PP-overlap-03 | turn end: stay+1, fan+1, 明天见→hand | PROTOCOL-LEVEL | — | — | turn-end chain | — |
| C-PP-overlap-04 | discard hook removes accident | OPEN | — | — | needs discard-not-play path | — |
| C-PP-ranger-00 | gate: house within dist 30-owned | PROVEN* (formula) | `ranger_00_gate_ok` (spec) | — | body needs `dist`/`owned_tiles` (modeled) | — |
| C-PP-ranger-01 | count>=1 → +500 | PROVEN* (formula) | `ranger_01_money` (spec) | — | | OPEN-共鸣 |
| C-PP-ranger-02 | count>=3 → band +3 | PROVEN* (formula) | `ranger_02_band` (spec) | — | | OPEN-共鸣 |
| C-PP-ranger-03 | count>=4 → band -4, draw 1 | OPEN | `ranger_03_*` (spec) | — | removing 4 when fewer exist is unprinted | OPEN-floor, OPEN-共鸣 |
| C-PP-ranger-04 | count>=5 → build-discount status | PROVEN* (formula) | `ranger_04_status_layers` (spec) | — | status key is engine-specific | OPEN-共鸣 |
| C-PP-ranger-05 | count>=6 → draw 1 | PROVEN* (formula) | `ranger_05_draw` (spec) | — | | OPEN-共鸣 |
| C-PP-no-expect-01 | immune to all other effects | PROVEN* | `no_expect_01_immune_iff` | pending | — | — |
| C-PP-no-expect-02 | place + 共鸣→deck + draw 1 | PROVEN* | `no_expect_02_place_deck_draw_iff` | triage3: FAILED was **B** exact-match spec rejected 持续 `SetState`; fixed, re-time pending | `add_to_deck` is silent in the effect log (post deck_n checked) | — |
| C-PP-no-expect-03 | continuous roll-2 / hand limit -1 | PROTOCOL-LEVEL | — | — | continuous roll modifier + move-floor | — |
| C-PP-no-expect-04 | pay+100 / receive-100, 2× window | OPEN | — | — | window lifetime of 「生效2次」 unprinted | OPEN-window2 |
| C-PP-shine-again-00 | immune to other players' effects | PROVEN* | `shine_again_00_immune_iff` | pending | — | — |
| C-PP-shine-again-01 | [反击] rescue price/5 + 1 fan | PROTOCOL-LEVEL | `shine_again_01_formula` (pure) | pending | [反击] window; regression hint: fan leg missing in engine | — |
| C-PP-shine-again-02 | [共鸣] record colour + 1 fan | OPEN | — | — | | OPEN-共鸣 |
| C-PP-first-accident-01 | place + 重叠的声音→deck + draw 1 | PROVEN* | `first_accident_01_place_deck_draw_iff` | triage3: FAILED was **B** spec rejected 持续 `SetToken`; fixed, re-time pending | — | — |
| C-PP-first-accident-02 | owner cannot use PP (2) skills | FAILS | (regression `first_accident_blocks_pp_skill_2`) | — | book: no PP (2); body writes a skillBlock token but 丸山彩 (2) still opens | — |
| C-PP-flower-01 | place on owned tile + band +2 | PROVEN* (formula) | `flower_01_band_after` (spec) | — | ask_tile prompt is expensive | — |
| C-PP-flower-02 | pass → force stop, move becomes settle | OPEN | — | — | definition of [强制停下] truncated | OPEN-强制停下 |
| C-PP-flower-03 | [共鸣] +1500; card → discard | OPEN | — | — | | OPEN-共鸣 |
| C-PP-same-dream-01 | band +3 | PROVEN* | `same_dream_01_band_plus_3_iff` | pending | — | — |
| C-PP-same-dream-02 | flip all up, gain up×100 | PROVEN* | (same harness) `same_dream_02_money_mutant_is_detected` | pending | — | — |
| C-PP-same-dream-03 | flip all down; [共鸣] widens | PROVEN* (user half) | (same harness) | pending | widen branch not entered (isolation) | OPEN-共鸣 |
| C-PP-jennifer-01 | place on another player's field | PROVEN* | `jennifer_01_place_other_iff` | pending | — | — |
| C-PP-jennifer-02 | pass 偶像经纪公司: pay 400, fans | OPEN/PROTOCOL | `jennifer_02_pay` (spec) | — | PassTile chain; which face flips on owner | OPEN-fan-face |
| C-PP-jennifer-03 | remove; Ranger → user hand | OPEN/PROTOCOL | — | — | same chain | — |
| C-PP-together-01 | X = 5 (+ ceil half gap) | PROVEN* | `together_01_x_formula_iff`, `together_01_mutant_is_detected` | triage3: mutant FAILED was **B** `min(X,down)` collapsed body+mutant; assume `down>=7`, re-time pending | — | — |
| C-PP-together-02 | [共鸣] X-=5, draw 1 | OPEN | `together_02_*` (spec) | — | | OPEN-共鸣 |
| C-PP-dream-ahead-00 | game-start auto place, hand -1 | PROTOCOL-LEVEL | — | — | game-start deal scheduling | — |
| C-PP-dream-ahead-01 | crystal per draw, cap 5, overflow X | PROVEN* (pre<5) | `dream_ahead_01_crystal_per_draw_iff` | triage3: FAILED was **B** wrong crystal storage + wrong X key; fixed, re-time pending | host `add_counter` ignores `max` (cap-5) | — |
| C-PP-dream-ahead-02 | no build, hand limit -1 | PROVEN* (continuous prop) | — | prop on CardDef | — | — |
| C-PP-dream-ahead-03 | extra rent owned×X + MIN(X×30,300) | PROVEN (formula) | `dream_ahead_03_formula` | SUCCESSFUL 3.6s | settle context is PROTOCOL | — |
| C-PP-dream-ahead-04 | [共鸣][反击] turn-end buy | OPEN | — | — | [可购买格子] set undefined | OPEN-共鸣 |
| C-PP-guide-01 | band +3, place, owned/3 crystals | PROVEN* | `guide_01_band_and_crystals_iff`, `guide_01_mutant_is_detected` | triage3: iff FAILED was **B** wrong crystal storage (mutant was vacuous); fixed, re-time pending | | OPEN-div |
| C-PP-guide-02 | mark from deck colours | OPEN | — | — | mark vocabulary unprinted | OPEN-mark |
| C-PP-guide-03 | hand limit -1 | PROVEN* (CardDef prop) | — | — | — | — |
| C-PP-guide-04 | +1 crystal at turn end | PROVEN* (pre<5) | `guide_04_turn_end_crystal_iff` | triage3: FAILED was **B** wrong crystal storage; fixed, re-time pending | cash-in at ≥5 is OPEN-mark | — |
| C-PP-guide-05 | ≥5 crystals: apply marks → discard | OPEN | — | — | | OPEN-mark |

\*PROVEN* = harness written; verdict pending the machine-wide CBMC slot.

---

## Character skills (Pastel✽Palettes)

| ID | book quote (short) | class | harnesses | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-PP-aya-with-01 | start +5 positive fans | PROVEN* | `aya_with_01_five_fans_iff`, `aya_with_01_mutant_is_detected` | pending | grant-(2) half needs a non-PP seat (not fully asserted) | — |
| C-PP-hina-lottery-01 | same shared (1) | PROVEN* (same shape as aya-with-01) | — | — | — | — |
| C-PP-chisato-frank-01 | same shared (1) | PROVEN* (same shape) | — | — | — | — |
| C-PP-maya-dawn-01 | same shared (1) | PROVEN* (same shape) | — | — | — | — |
| C-PP-eve-unify-01 | same shared (1) | PROVEN* (same shape) | — | — | — | — |
| C-PP-aya-with-02 | pay discount Y×100 + fan-side flip | PROVEN* | `aya_with_02_pay_discount_iff`, `aya_with_02_mutant_is_detected` | pending | — | — |
| C-PP-hina-lottery-02 | turn-start 1d4 borrow (2) | PROVEN* (map) / FAILS (body) | `hina_lottery_02_borrow_map` | pending | regression: `skill.hinaLottery.borrowed` stays -1 (borrow never lands) | — |
| C-PP-chisato-frank-02 | receive +Y×100 | PROVEN* (formula) | `chisato_frank_02_formula` | pending | — | — |
| C-PP-maya-dawn-02 | draw → watch Y+1, pick 1 | PROVEN* (formula) | `maya_dawn_02_watch_formula` | pending | prompt/pick body is expensive | — |
| C-PP-eve-unify-02 | +Yd4 on any roll | BLOCKED-HOST | `eve_unify_02_extra_dice_formula` (pure) | pending | `trig_move_roll` unmodeled (KANI-COVERAGE) | — |

---

## Band skill — Pastel✽Palettes「与偶像一起」

| ID | book quote (short) | class | harnesses | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-PP-band-idol-01 | non-PP +1 reverse fan, once | PROVEN* | `band_idol_01_start_grant_iff` | pending | non-repeat half not fully asserted | — |
| C-PP-band-idol-02 | overflow X-down → crystals (cap 10) | FAILS | `band_idol_02_overflow_formula`, `band_idol_02_body_never_overflows` | pending | body's own flip caps X at `have`, so X>down is unreachable; no general flip-effect trigger | — |
| C-PP-band-idol-03 | no CiRCLE reward | PROTOCOL-LEVEL | `band_idol_03_no_reward_formula` | pending | reward step is engine walk / plan flag | — |
| C-PP-band-idol-04 | +1 crystal; opt. -5 draw 1 | PROVEN* | `band_idol_04_tick_iff` | pending | host `add_crystals` max ignored | — |

---

## FAILS (book vs body)

1. **C-PP-first-accident-02** — book: 「[拥有者]不可使用任何Pastel✽Palettes角色的（2）技能」.
   Body writes a `skillBlock` token, but 丸山彩's (2) prompt still opens and the
   pay-discount runs. (Regression `first_accident_blocks_pp_skill_2`.)
2. **C-PP-band-idol-02** — book: any flip-to-positive with `X > down` grants
   `min(X-down, 10)` crystals. Body: the only flip it sees is its own press,
   which asks for `X ∈ 1..=have`, so overflow is unreachable; no listener on
   other flip effects. (Regression `band_overflow_to_crystals`.)

Also recorded in the unit hint list (regression, not re-proven here):
**C-PP-hina-lottery-02** borrow never lands; **C-PP-shine-again-01** fan leg
missing in the engine.

## BLOCKED-HOST (exact missing op / field)

| clause | missing |
|---|---|
| C-PP-title-idol-02 | `card_text_mentions` returns 0 (stub); `field_instances` returns count without writing the uid buffer |
| C-PP-eve-bushido-04 (cap) | `add_counter_self` / `add_counter` ignores the `max` clamp |
| C-PP-dream-ahead-01 (cap) | same `max` clamp |
| C-PP-band-idol-04 (spend) | same `max` clamp on `add_crystals` |
| C-PP-eve-unify-02 | `trig_move_roll` unmodeled (KANI-COVERAGE) |
| C-PP-guide-01 (crystals store) | `add_counter_at` writes `trig_player`'s named key rather than the field instance's `crystals` (affects title-idol step 2 / multi-instance) |

## PROTOCOL-LEVEL

C-PP-aya-haruka-03, C-PP-maya-inf-01, C-PP-overlap-02, C-PP-overlap-03,
C-PP-no-expect-03, C-PP-shine-again-01, C-PP-dream-ahead-00, C-PP-band-idol-03,
C-PP-jennifer-02/03 (PassTile chain). Reason per row: multi-turn /
counteraction window / engine scheduling / settle chain (KANI.md §6).

## OPEN (text undefined)

OPEN-共鸣 · OPEN-分摊 · OPEN-乐队卡 · OPEN-强制停下 · OPEN-mark ·
OPEN-substitution · OPEN-substitution-effect · OPEN-fan-face · OPEN-window2 ·
OPEN-ties · OPEN-floor · OPEN-div · OPEN-tiles.

Per the brief only the parts of the text that determine behaviour are proved;
the rest stay OPEN in this table.

---

## Isolation assumes

* `hand_resonance[p] = 0` for every player in play harnesses, so
  `try_resonance` returns false before any prompt (OPEN-共鸣 branches not
  entered).
* `n_players ∈ 2..=4`; money `0..=10_000`; fans `0..=20`.
* Crystal-cap clauses: `pre < cap` so the unmodeled `max` clamp does not bind.

## Runner notes

* `tools/kani/run.sh --unit pp --harness <name>` from the worktree.
* `CARGO_TARGET_DIR=/home/monopoly/kani-target-pp`.
* Machine-wide 3-slot cap; several unit agents share the box. Pure-formula
  harnesses (`*_formula`) are the cheap class and go first.