# ABI design set — B-class backlog (draft for review)

These 15 documents are the **design-only** answer to the 65 B-class items of
the 2026-10-27 card sheet triage (`docs/rulebook/BACKLOG.md`, section B). No
source is implemented here: each document states a problem, a generic
capability, an exact ABI surface, engine wiring, card usage, save/record
impact, versioning, tests/Kani obligations, alternatives and open questions.

**The group is the unit of review.** Approve, reject or reshape a whole group —
not individual cards. An item is fixed only after its group is approved and
landed. Demoted items (marked *demote to A* in a document) need no new ABI and
are fixed on the A path; they stay listed in the document so the demotion is
recorded.

## Review order and collisions

Every document was written against the same base (ABI v104, `SAVE_VERSION` 12,
`RECORD_VERSION` 4, engine kinds ending at 110, Kani `Op` ending at 340). Each
draft originally reached for the same next numbers, so the band below is
allocated **once, in landing order**, and the documents carry their row of it.
Take these numbers; renumber only if the order actually changes, and then shift
the whole table.

| group | `ABI_VERSION` | `SAVE_VERSION` | engine kinds | Kani `Op` |
|---|---|---|---|---|
| G15 | — | 12 | — | — |
| G01 | 104→105 | 12 | 111–113 | 341–348 |
| G08 | 105→106 | 12→13 | 114–121 | 349–368 |
| G02 | 106→107 | 13 | 122 | 369–377 |
| G04 | 107→108 | 13 | — | 378–397 |
| G03 | 108→109 | 13→14 | 123–125 | — |
| G06 | 109→110 | 14→15 | — | 398–417 |
| G05 + G13 | 110→111 | 15→16 | — | 418–427 |
| G07 | 111→112 | 16 | — | 428–437 |
| G09 | 112→113 | 16 | 126–130 | — |
| G10 | 113→114 | 16→17 | 131 | — |
| G11 | 114→115 | 17→18 | 132–133 | — |
| G12 | 115→116 | 18 | 107 | — |
| G14 | 116→117 | 18 | — | — |

A group that lands changes the numbers for all later ones. Re-run the full
suites after each landing.

**2026-10-31 amendment.** The first table filled Op numbers only from G02
onward and gave G01/G08 no Op row at all, although G01's revised draft imports
eight functions and G08's imports twenty. Recounted from base 340 in landing
order: G01 takes 341–348, G08 349–368, and every group from G02 on shifts ten.
`SAVE_VERSION` stays at the value G08 landed (13) — the base column above
records the *pre-landing* value each document was written against, which is
why G15/G01 say 12.

Recommended landing order (foundation → dependent):

1. **G15** Kani host — model-only, no version change.
2. **G01** instance identity — the uid/trigger payload other groups build on.
3. **G08** zone lifecycle — the single zone writer, instance state, leave hooks.
4. **G02** pay pipeline — payload/stage seams consumed by G09/G10/G12.
5. **G04** move plan — plan frames and the blocked predicate.
6. **G03** roll pipeline — stage ladder; reserves the 111+ kind band.
7. **G06** counteraction/join — depends on G01 payload and G03 kinds.
8. **G05 + G13** press/invoke — must land together (one `invoke_entry` shape).
9. **G07** targeting — depends on G01 identity and G06 window.
10. **G09** tile/build/pass — depends on G02 header and G08 zone writer.
11. **G10** effect outcome — settle facts and defer depend on G02/G09.
12. **G11** information/deck — drawAfter, inspection frames.
13. **G12** status/immunity — depends on G04 blocked-ness and G06 pending.
14. **G14** AI/hand limit — smallest consumer surface, last.

Couplings a reviewer must keep in one head:

- **G05 ↔ G13** share `invoke_entry` and the press-entry resolution; G05 adopts
  G13's import verbatim.
- **G01 ↔ G08** share instance uid and the per-instance store.
- **G02 ↔ G10** share the settle frame and its outcome facts: G02 owns the
  settle stack, the terminal outcome and the `settle_open` fix, G10 adds the
  `paid`/`body_replaced` facts on top (G02 §12).
- **G03 ↔ G11** share the drawAfter point; their kind bands are adjacent.
- **G06 ↔ G12** share pending-window reads.
- **G04 ↔ G12** share the blocked predicate (`cant_move_code_for_activation`).
- **G07 ↔ G12** share the target window and the immunity gate ordering.

## Groups

| group | title | items | doc |
|---|---|---|---|
| G01 | Instance identity and per-instance store | 3 | [G01](G01-instance-identity-and-state.md) |
| G02 | Pay pipeline payload, stages and raw-money windows | 7 | [G02](G02-pay-pipeline.md) |
| G03 | Roll pipeline stages and prompt option actions | 5 | [G03](G03-roll-and-prompts.md) |
| G04 | Move plan, walk sync and forced moves | 7 | [G04](G04-move-plan.md) |
| G05 | Press surface for placed cards | 1 | [G05](G05-placed-card-press.md) |
| G06 | Counteraction, join and negation windows | 6 | [G06](G06-counteraction-and-join.md) |
| G07 | Targeting pipeline and distance | 2 | [G07](G07-targeting-and-distance.md) |
| G08 | Zone and attachment lifecycle | 10 | [G08](G08-zone-lifecycle.md) |
| G09 | Tile, build, pass and house-removal hooks | 6 | [G09](G09-tile-build-pass.md) |
| G10 | Effect outcome, cause and settle facts | 5 | [G10](G10-effect-outcome.md) |
| G11 | Information and deck inspection | 3 | [G11](G11-information-and-deck.md) |
| G12 | Status, blocked and immunity gates | 5 | [G12](G12-status-and-immunity.md) |
| G13 | Skill invoke and copy context | 2 | [G13](G13-invoke-and-copy.md) |
| G14 | CardDef AI hooks and hand limit | 2 | [G14](G14-ai-and-hand-limit.md) |
| G15 | Kani verification host support | 1 | [G15](G15-kani-host.md) |

The item slugs, severities and sheet rows are listed under each group in
`docs/rulebook/BACKLOG.md` section B. Each document's header repeats its own
slug/row table.

## Conventions every design follows

- Generic mechanism only; card and band vocabulary stays in `bandori-std` and
  the card crates.
- No display text in the ABI — `Msg` keys only.
- No player decision inside an import — ask, pause, replay with the answer.
- All three cfg paths (wasm32 / native / Kani) for every import, with a bounded
  Kani model and an invariant.
- Sugar translates before the ABI.
- New save fields are `#[serde(default)]`; a save/record bump is stated and
  justified per group. Replays ride archived bundles.
- `ABI_VERSION` bumps on wire change only, with the exact `abi.rs` line text.

## Known data corrections

G13 found that four rows in its group (437/996/187/995) were mis-filed by the
triage merge; the document carries the true rows (734/735/737, 407/408/421/426)
as well and explains the mismatch. G07 carries the merged sub-slugs of its
targeting item (`centrifugal-between-turns-counter`,
`centrifugal-tile-targeting`, `welcome-self-target-pipeline`). §11 of each
document lists the open questions the sheet does not pin — those follow the C
path, not the B one.



## Consolidated-slug map

The BACKLOG lists one consolidated slug per finding; the documents split it into
reviewable items and are keyed to those slugs. To trace a BACKLOG row, use this
map (consolidated → slugs the document names):

| consolidated slug | document names |
|---|---|
| `skill-fired-signal` | `skill-fired-signal`, `rui-x-write`, `sayo-skill-used-event`, `tomori-x-instance-scope`, `umiri-user-key-per-instance` |
| `trigger-instance-identity` | `starry-trigger-uid`, `black-marker-hook`, `black-copy-crystals`, `hagumi-multi-standin-abi` |
| `counter-cap-uid` | `sparkler-per-instance-counter-cap`, `cs-crystal-cap-read` |
| `pay-payload-kind-tile` | `rui-fixed-100`, `mashiro-pay-kind-rewrite`, `kokoro-pay-buy`, `kokoro-rent-kind`, `kokoro-pay-tile`, `door-pay-tile-vs-settle-tile`, `kokoro-circlenormal-skip` |
| `pay-stage-presplit` | `vocal-split-leg`, `aya-stage-2-interactive-hook-missing` |
| `raw-money-window` | `popipapapipopa-spend-window`, `tritone-lose-half-unreachable` |
| `charge-paying-source` | `tomori-charge-paying-temp-share`, `tomori-charge-paying-skill-scope` |
| `fire-bird-paid-signal` | `fire-bird-paid-signal` |
| `rent-figure` | `anon-tokyo-link-rent-figure` |
| `money-counters` | `guerrilla-paid-per-seat`, `hagumi-money-in-counter-abi` |
| `roll-stage-effect-rolls` | `talent-effect-rolls-unpaid`, `moca-roll-post-additive-stage`, `noexpect-roll-scope`, `dg-move-dice` |
| `roll-history-per-player` | `taki-per-player-rolls`, `taki-extreme-reroll` |
| `reroll-post-pick-signal` | `starry-value-stage`, `want-human-boost-vs-reroll`, `want-human-crystal-on-reroll`, `hagumi-per-roll-options-abi`, `layer-keep-keep-point` |
| `please-choose-option1` | `please-choose-opt1-inert` |
| `own-stage-answer` | `own-stage-answer` |
| `want-to-grab-board-axis` | `want_to_grab-clause3-teleport` |
| `plan-shape-scope` | `pure-wings-stale-steps`, `pure-wings-stale-direction`, `pure-wings-pay-factor-leak`, `saki-move-leaks-plan-shape`, `dg-plan-reset`, `sakiko-pay-factor-composition`, `repaint-pay-factor`, `taki-pay-factor-leak`, `rana-pay-factor-multiply` |
| `charity-rolled-flag` | `charity-dice-move-gate` |
| `plan-live-sync` | `charity-extra-steps-sync`, `detour-plan-writes-lost`, `misaki-effective-steps` |
| `effect-move-forced-path` | `sakiko-followers-card-move`, `misaki-teleport`, `fate-forced-hop`, `plan-set-forced` |
| `trigger-walk-payload` | `detour-parity-unused`, `misaki-planned-start`, `misaki-fire-roll-signal` |
| `nested-teleport-tail` | `lisa-bond-nested-teleport-window` |
| `press-surface-placed-card` | `kanon-press-surface-abi`, `even-lost-not-pressable`, `placed-card-press-surface`, `hina-clause-2-swap-missing` |
| `join-window-report` | `net-error-join-draw-ownership`, `net-error-join-used-getter`, `mana-nested-window`, `mana-joiner-count`, `rain-designates-prop` |
| `extreme-range-event-counter` | `extreme-range-filter-missing`, `event-effects-not-counterable` |
| `counter-link-move-face` | `repaint-counter-link-roll` |
| `counteraction-history` | `cla-counterer-set` |
| `window-timing-negation` | `nowsumimi-cancelled-guard`, `ordinary-effect-window-not-post-landing`, `own-stage-immuneall-ordering` |
| `mortis-copy-links` | `mortis-empty-counteract-links`, `mortis-copy-effect-unimplemented` |
| `target-count-pipeline` | `target-count-pipeline`, `centrifugal-between-turns-counter`, `centrifugal-tile-targeting`, `welcome-self-target-pipeline` |
| `lisa-distance-exemption` | `lisa-distance-exemption`, `lisa-bond-exemption-gap` |
| `nyamu-face-flip-trigger` | `nyamu-face-flip-trigger` |
| `one-of-us-rehome` | `one-of-us-rehome` |
| `one-of-us-cause` | `one-of-us-cause` |
| `same-sky-mulligan-order` | `same-sky-mulligan-order` |
| `cardplayed-payload` | `cardplayed-payload` |
| `hand-changed-event` | `hand-changed-event` |
| `wasted-flag` | `wasted-flag` |
| `card-on-card` | `card-on-card` |
| `here-the-world-leave-hook` | `here-the-world-leave-hook` |
| `tomori-earliest-owned` | `tomori-earliest-owned` |
| `marina-suppression-window` | `marina-suppression-window` |
| `parade-tile-swap` | `parade-tile-swap` |
| `build-pipeline-queries` | `build-pipeline-queries`, `miracle-free-build-choice` |
| `tile-write-effect-declare` | `tile-write-effect-declare` |
| `guerrilla-pass-path` | `guerrilla-pass-path` |
| `house-removed-kind` | `house-removed-kind` |
| `event-defer` | `event-defer`, `arisa-clause-2-event-defer`, `arisa-clause-1-cause-filter` |
| `settle-outcome-flags` | `settle-outcome-flags`, `door-settle-body-replace-fact`, `council-check-cancelled-settle` |
| `host01-journal` | `host01-journal`, `be-strongest-host01-nested-settle-ownership` |
| `redirect-latch` | `redirect-latch`, `exist-influence-latch-scope` |
| `world-snapshot` | `world-snapshot`, `nobreakup-revert-half` |
| `backstage-peek-refill` | `backstage-peek-refill`, `backstage-flip-refill` |
| `reveal-deck-inspection` | `reveal-deck-inspection`, `believe-reveal`, `guide-reveal-is-only-a-count`, `inf-possibility-deck-inspection` |
| `drew-batch` | `drew-batch`, `inf-possibility-drew-per-action` |
| `move-gate-replay-read` | `detour-false-move-blocked-gate`, `move-gate-replay-read` |
| `unstoppable-blocked-read` | `unstoppable-blocked-read` |
| `major-abnormal-scope` | `major-abnormal-scope` |
| `immunity-causer` | `immunity-causer` |
| `dice-immunity` | `dice-immunity` |
| `invoke-skill-entry` | `nanami-press-invokes-clause-3`, `invoke-skill-entry` |
| `tomori-borrowed-second-half` | `tomori-borrowed-second-half` |
| `hand-limit-lift` | `hand-limit-lift` |
| `aiplay-hook` | `aiplay-hook` |
| `kani-owned-livehouse` | `kani-owned-livehouse`, `blue-rose-dead-spec` |
| (extra, no BACKLOG row) | `detour-counter-hooks-dead` |

open questions the sheet does not pin — those follow the C path, not the B one.
