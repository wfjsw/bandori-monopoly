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
| G01 | 104→105 | 12 | 111–112 | 341–346 |
| G08 | 105→106 | 12→13 | 113–120 | 351–370 |
| G02 | 106→107 | 13 | 122 | 370–378 |
| G04 | 110→111 | 13 | — | 378–391 |
| G03 | 108→109 | 13→14 | 123–125 | 398–410 |
| G06 | 111→112 | 15→16 | 121 | 411–426 |
| G05 + G13 | 110→111 | 14→15 | — | 347–350, 431–440 |
| G07 | 112→113 | 16 | — | 441–450 |
| G09 | 115→116 | 17 | 135–139 | 452–463 |
| G10 | 116→117 | 17→18 | 140 | — |
| G11 | 117→118 | 18 | 141–142 | — |
| G12 | 113→114 | 16 | 134 | 443–450 |
| G14 | 114→115 | 16→17 | — | — |

A group that lands changes the numbers for all later ones. Re-run the full
suites after each landing.

**2026-10-31 amendment.** The original table filled Op numbers only from G02
onward and gave G01/G08 no Op row at all, although G01's revised draft imports
functions and G08's imports twenty. G01's review then dropped its `Actions`
offer kind in favour of G05's offer surface, and a second review split two
concerns out of G01 — `effective_card` (then spelt `real_id`) to G05 and
`created_by` to G08 — because they are offer/skin and zone lifecycle, not
identity. G01 keeps six ops (341–346)
and its two kinds (111–112); the vacated ids sit with their new groups
(G05 347–350, G08 351) so no later band moves for them. Recounted from base
340 in landing order, G08's twenty imports take 351–370 and every group from
G02 on shifts eleven; kind 121 stays free, which costs nothing. `SAVE_VERSION`
stays at the value G08 landed (13) — the base column above
records the *pre-landing* value each document was written against, which is
why G15/G01 say 12.

**2026-10-31 second amendment (G03).** The G03 review found the draft asked
for eight roll imports plus `trig_roll` while its Op row was still `—`. G03
takes 398–406 (nine). At landing the two 「是否触发结算」 answer pairs
(§4.7's four ops) joined the same band: G03 takes **398–410** (thirteen),
and every group after it shifts thirteen: G06 411 onward (it settles at
411–426, see the 2026-11-02 amendment), G05+G13 431–440, G07 441–450. G04's
landed 378–397 is untouched.

**2026-11-01 amendment (G05).** The G05 review found the group is not a press
surface at all: every clause the sheet prints on a placed card is an
engine-raised offer (a prompt at the event the clause names) or a passive
change, so no card gets a button. The card-side bit, the press `Call` and the
wire field are gone; the group instead needs one guest `Action` constructor over
the existing plan kind (kind 67, no new id), a per-move offer claim, and the
effective-card marker. The offer itself is the entry's `declare`
(`On::declare_arg`), not an ask in the body: a declined offer must leave no
card flash and no log line, which only a decline at the admission layer gives. The claim takes `Op` 349 and its writer 350, so **G08 shifts to 351**
and its twenty imports take 351–370; the kind column stays `—` because no engine id
is consumed. `SAVE_VERSION` still moves for the marker's field.

**2026-11-02 amendment (G06).** G04 and G05 have landed; `ABI_VERSION` is 111
and `SAVE_VERSION` 15 (G05 landed one SAVE step ahead of its own row, which is
why the row reads 15→16 and the constant 15). The G06 row still carried the
pre-G03 numbers (109→110 / 14→15), so the row is rewritten to **111→112 /
15→16**. Every pending row after G06 shifts one ABI step: G07 112→113, G09
113→114, G10 114→115, G11 115→116, G12 116→117, G14 117→118; SAVE rows do not
move. G06's sixteen imports take the free **411–426**, leaving 427–430 free so
G05+G13's landed 431–440 and every later op band keep their numbers. G06 also
takes the one engine kind that stayed free, 121 (`abnormalRefused`, hook-only).

**2026-11-04 amendment (G09).** G02's landed rent/settle arithmetic took kinds
126–131, which had swallowed this group's allocated 126–130 *and* G10's 131;
G08's row band 113–120 went unused. Kind ids are wire-stable, so the allocation
moves, not the landed ids: G09 takes **132–136** (five: two hand-window kinds
and three effect-only spellings), G10 **137**, G11 **138–139**. G12 keeps the
still-free 107 and G14 the empty row; 113–120 stay spare. G09's twelve imports
(the position/tile conversion, the build reads and verb, the move-path reads)
take **452–463**, right after G07's landed 441–450.

**2026-11-04 amendment (G12 review and landing).** The G12 row's kind column
read 107, which is the **ruleset** namespace's `fansFlip`; free **engine** ids
resumed at 121 (G06) and G11's row holds 132–133, so G12 takes **134**. Its
eight imports take the unused tail of G07's Op band, **443–450** (G07 landed
441–442 only). G12 then landed **ahead of G09–G11**, so its row is the real
113→114 with SAVE unchanged at 16, and every later ABI row shifts one step.

**2026-11-04 amendment (G14).** The G14 review found that fixing the trim alone
is not enough: `cant_act`'s `end` gate refuses an over-limit hand, so the very
turn end the card's own scheduled discard hangs off would never be reached, and
the formal `G-CARDS-01` clause would fail on the act. G14 therefore adds a
durable per-player allowance (`overHandAllowed`, `serde(default)`) that exempts
an allowed excess from the end gate and from the clause, is cleared when the
hand fits again and swept at turn end, and the ABI row gains one `ruleset_draw`
flag word (an arity break for that import). The group also adds one static
card prop (`aiNever`), which changes no id: it rides the existing `props` map.

**2026-11-04 landing note (G14).** G14 landed after G12, so it takes the next
free numbers, **114→115** / **16→17**, and the not-yet-landed G09–G11 rows
shift one more step (ABI and SAVE both).

**2026-11-04 landing note (G09).** G09 landed after G12 and G14 and takes the
next free numbers: **115→116** / SAVE stays at 17. G12's landed 134 swallowed
the amended kind block's first id and the G12/G14 landings took 443–450, so
G09's five kinds move to **135–139**; its imports keep the **452–463** band.
G10 and G11 shift to 116→117 / kind 140 and 117→118 / kinds 141–142.
`ABI_VERSION` is 116 and `SAVE_VERSION` 17.

Recommended landing order (foundation → dependent):

1. **G15** Kani host — model-only, no version change.
2. **G01** instance identity — the uid/trigger payload other groups build on.
3. **G08** zone lifecycle — the single zone writer, instance state, leave hooks.
4. **G02** pay pipeline — payload/stage seams consumed by G09/G10/G12.
5. **G04** move plan — plan frames and the blocked predicate.
6. **G03** roll pipeline — the cast ladder over G04's frame; kinds 123–125.
7. **G06** counteraction/join — depends on G01 payload and G03 kinds.
8. **G05 + G13** offer/invoke — must land together (one `invoke_entry` shape).
9. **G07** targeting — depends on G01 identity and G06 window.
10. **G09** tile/build/pass — depends on G02 header and G08 zone writer.
11. **G10** effect outcome — settle facts and defer depend on G02/G09.
12. **G11** information/deck — drawAfter, inspection frames.
13. **G12** status/immunity — depends on G04 blocked-ness and G06 pending. *(landed early, at ABI 114)*
14. **G14** AI/hand limit — smallest consumer surface. *(landed early, at ABI 115, after G12)*

Couplings a reviewer must keep in one head:

- **G05 ↔ G13** share `invoke_entry`: G05's crystal offer invokes a named clause
  and adopts G13's import verbatim.
- **G01 ↔ G08** share instance uid and the per-instance store; G08 also
  carries the creator edge (`created_by`) that G01's borrowed-row hooks rely
  on.
- **G02 ↔ G10** share the settle frame and its outcome facts: G02 owns the
  settle stack, the terminal outcome and the `settle_open` fix, G10 adds the
  `paid`/`body_replaced` facts on top (G02 §12).
- **G02 ↔ G09** share the build payment: G09's pipeline is a `PayKind::Build`
  leg over G02's header, priced by `build_price`, and G09's `TileWrite` effect
  reuses G02's `effect` window shape.
- **G03 ↔ G04** share the window raise point: G04 owns the plan snapshot/fold,
  G03 raises every roll window inside it and hands the plan-write item (G03
  item 7) to G04 §4.3.
- **G03 ↔ G11** share the drawAfter point; their kind bands are adjacent.
- **G06 ↔ G12** share pending-window reads.
- **G04 ↔ G12** share the blocked predicate (`cant_move_code_for_activation`).
- **G07 ↔ G12** share the target window and the immunity gate ordering.

## Groups

| group | title | items | doc |
|---|---|---|---|
| G01 | Instance identity and per-instance store | 10 | [G01](G01-instance-identity-and-state.md) |
| G02 | Pay pipeline payload, stages and raw-money windows | 7 | [G02](G02-pay-pipeline.md) |
| G03 | Roll pipeline stages and prompt option actions | 5 | [G03](G03-roll-and-prompts.md) |
| G04 | Move plan, walk sync and forced moves | 7 | [G04](G04-move-plan.md) |
| G05 | Offers from placed cards | 5 (1 shared with G13) | [G05](G05-placed-card-press.md) |
| G06 | Counteraction, join and negation windows | 6 | [G06](G06-counteraction-and-join.md) |
| G07 | Targeting pipeline and distance | 2 | [G07](G07-targeting-and-distance.md) |
| G08 | Zone and attachment lifecycle | 11 | [G08](G08-zone-lifecycle.md) |
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
| `effect-move-forced-path` | `sakiko-followers-card-move`, `misaki-teleport`, `dest-forced-hop`, `plan-set-forced` |
| `trigger-walk-payload` | `detour-parity-unused`, `misaki-planned-start`, `misaki-fire-roll-signal` |
| `nested-teleport-tail` | `lisa-bond-nested-teleport-window` |
| `press-surface-placed-card` | `nanami-press-invokes-clause-3` (shared with G13), `kanon-press-surface-abi`, `even-lost-not-pressable`, `placed-card-press-surface`, `hina-clause-2-swap-missing` — the consolidated row's nine rows are mis-filed; G05 §0 gives the true rows. The clauses are offers (engine-raised prompts), not buttons |
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
