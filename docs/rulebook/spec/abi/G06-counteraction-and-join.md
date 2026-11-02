# G06 — Counteraction, join and negation windows

Status: **draft for review, design only** (2026-11-01 revision of the
2026-10-28 draft). No source is edited by this document.
Worktree `/home/monopoly/web/.claude/worktrees/g06-design-review`, branch
`worktree-g06-design-review`. Fits **on top of ABI v111** (G01, G08, G02, G04
and G03 have landed); it does not redo them.
Group: **G06**. Family: the [反击] chain — its answer contracts, its payload
carry-forward, the 真奈 join window, the counteraction history, and the two
guards that depend on a *declared* answer.

Names are spelled out (G03 R25's rule): **counteraction** is the [反击]
mechanism, never "reaction"; **hook** is a passive listener ([持续] / [特] /
`On::Action` / `On::Filter`); the engine-side carrier of one raise is a
**trigger**, its module-side mirror a **link**; a **join** is the nested
window a `SELF_REDIRECT` counter opens on the link it answers.

Ground truth is `docs/rulebook/cards.json`; every quoted fragment below was
re-read there and is identified by its card id beside it. The **row ids are
the triage sheet rows** — the same ids `BACKLOG.md` section B uses. They are
neither `cards.json` line numbers nor line numbers of the review CSV or the
review Markdown; the CSV is cited by `file:line` and the review Markdown by
its card section.

Reading order: `docs/GUARDS.md` §4.2 (the window vocabulary), §4.5 (the
valid-option-first rule), `docs/NEGATION-AUDIT.md` (the V1 ordering), G03
§5.2 (the roll transaction a counter on a roll answers), G04 §4.3 (plan
frames a counter may write), G12 §5.1 (the refusal the abnormal window must
sit behind) and G13 (why a copied Counteract entry is not invocable).

## 0. Review record

This revision came out of a full review of the 2026-10-28 draft. Every cited
site was re-read against master, every claim about the engine was re-derived
from the code, and the band arithmetic was re-run from the groups that landed
before this one. What the draft got wrong:

| # | Draft defect | Correction |
|---|---|---|
| R1 | The header named worktree `card-rulebook-review` and base ABI 109 / SAVE 14; master is ABI 111 / SAVE 14 with G01/G08/G02/G04/G03 landed, and G06 lands at 112 / 15. | Header and §8 carry the real base and landing numbers; the README row is corrected (§8). |
| R2 | Nearly every `wasm_rules.rs` line cited in §1 and §4 was stale by 300–900 lines (e.g. `6140-6155` for the counter-link build, `7270-7330` for `bridge_trigger`, `7633` for `can_counteract_now`). | Every site is re-cited against master; the counter-link build is `6846-6847`, `bridge_trigger` is `8023`, `can_counteract_now` is `8425` (§2). |
| R3 | §1.12 described `rain-designates-prop` as 「可指定其一个卡上的[持续]」 and no card in `cards.json` says that. | The row (`BACKLOG` join slug, row 1510) is the rain `DESIGNATES` defect: the card designates players and does not declare the property, so every designation reader sees nothing (`CARD-REVIEW` §`通用:雨啊，快点来吧`). §2.12 says so. |
| R4 | Item 12 appeared in the §4 table but in **no** §1 body and in **no** §6 card section; `Join` had no designation field although §1.12 promised one. | §2.12 and §6.9 exist; the join records the designation per joiner. |
| R5 | §1.7 claimed a counter link has no `mv`, so repaint's condition fails. The draft's premise is right for master but its consequence analysis was not: the *write* path matters more. `set_move_roll` writes the link the counter answers (`resolve_jobs:7255`), and a counter answering a **counter** writes to that counter's link, which no later reader revisits — the face never reaches the roll that settles. | §6.3 carries the payload **and** propagates a face write to the nearest ancestor roll link (R11 below fixes the ordering). |
| R6 | §5.3's "copy the answered link" was written as a flat clone. That also copies `answered`, `seq`, `answers_declared`, `negation`, `spared`, `target` and the join block, and it leaves the root link without a subject, so `NumberInterval` would answer `false` on every play. | The clone is followed by an explicit override block; `subject_card` is stamped for roots at bridge time (§5.3, §6.3). |
| R7 | §4.3 added `join`, `subject_card` and `answers_declared` to the guest `Trigger` but §5.3 and §7 claimed `bridge_trigger` "keeps" them — `bridge_trigger` (`8023-8110`) builds a new literal and drops the first two, and `CounteractResume` clones the bridge trigger across a suspend. | `bridge_trigger` carries both; the engine `CoreTrigger` mirrors them (§5.3, §6.3, §7). |
| R8 | §5.4's `join::open()` was specified as an ordinary paused import. Under the replay model an `Ask` in the join **rebuilds the whole counteraction window** (the chain is a local; `Halt::Ask` discards the copy), so the answer log would replay the opener past `open()` while the join state is gone. | `open` and `shield` are **always-applied** requests (`request_always`, §6.4) and the join is re-derived from the replayed answers; a nested *settle* keeps the opener's body from re-running through the existing `ResumeFnWork` suspension shape. |
| R9 | §5.4 gave `join::open` `Result<(), Prompt>`, but `request_or_pause` answers `i32` and a `Prompt`-valued op needs its own sentinel path. | `join::open` returns `bool`/`Result<(), Prompt>` on the guest side only by mapping the `i32` at the host-function boundary; the request itself stays `i32` (§6.4). |
| R10 | §5.4 said the join round runs with `Priority::over(&seats)` and `link.target = cursor` but never said the target override leaks: after the round the committed link's `target` is the last joiner, not the effect's recipient. | The answered link's `target` is saved before the round and restored after (§6.4). |
| R11 | §1.7 said repaint must be able to answer an "inner counter"; with LIFO the inner settles first and an *outer* absolute write would overwrite its cut. No ordering design existed. | The face is composed: a counter's write is applied to the link it answers **and** propagated to the ancestor roll link when it settles, so the later-settling outer body reads the composed face (YOLO is additive, `yolo.rs:48`). §6.3. |
| R12 | §5.1's `contract_admits` read `t.subject_card` off the trigger "where `t.card` is set (`play.rs:4814` area)" — a guess. Only `target_window` and friends stamp `card`; `abnormal_gate`'s `effect` raise does not, so numeric effects that matter here would fail the filter. | `subject_card` is stamped from the innermost resolving frame when the raise carries no card (§6.3). |
| R13 | §5.2 typed `answers_declared` as `Vec<u32>` but computed `pending_negation` on "the link" without saying how the chain is reached from a guard, and `extreme_armed`'s `turn.extreme` half was written without noting that the extreme is per play and already restored by `play_from_hand` (`play.rs:5685-5692,5797-5799`). | §5.2 reads through a chain cursor installed around the offer and the body; the extreme lifetime is item 4's own fix (§6.6). |
| R14 | §5.5 pushed the history with `cx.world_copy().turn.countered_this_turn.push(...)` — one whole world copy per declaration, and `TurnCtx` is rebuilt at the turn boundary, not cleared at it, so nothing needs copying. | The counter is a `World` field with `reset_countered()` beside `reset_gains()` at `turn_start` (§5.6), like `World::gains`. |
| R15 | §4.8 placed `countered_this_turn` on `TurnCtx` and §7 then said its clear is "the default scope". `TurnCtx` is rebuilt per turn, so a save taken mid-turn loses it and the sentence in §7 about a `#[serde(default)]` load contradicts its own container choice. | `World.countered_this_turn` (SAVE'd), cleared at `turn_start` (§5.6, §7). |
| R16 | §5.1 asserted `counteracts_to_play` "ORs then ANDs". `contract_admits` must not be an AND on the designates fallback: an `Effect`-only entry offered through `trigger_designates` must pass the same contract, and `counteract_entry` returned the first entry by kind, which may be the wrong one when a card has two Counteract entries. | `counteract_entry` scans the card's entries and returns the first that satisfies the contract; the designates fallback is inside the same scan (§6.1). |
| R17 | The draft demanded `ManifestOn.all_kinds` *and* filled the kind mask with every kind. Filling the mask makes `ruleset.declares()` true for every kind set-wide whenever Mortis is present, which defeats the early-out at `counteract_orig:5462` — unmentioned. | Keep `all_kinds` (explicit beats inferring from an empty list) but say that the `declares` early-out now costs one bridge trigger per raise while Mortis is in the set (§6.1). |
| R18 | §8 said "the seventh group in landing order, entry inserted immediately below the constant" without the landed G03 entry text and with the wrong numbers; §5.7's Op sentence read "G01/G08/G02/G04/G03 take 341-410 ahead of this group". | Base 111, landing 112; the entry is inserted below `v111`; ops are 411–425 (§8). |
| R19 | §9.4's obligations named `rules/kani/src/proofs/abi_g06.rs` and `spec/abi_g06.rs`; the `mod` lists name units by crate band or by pilot (`cond_ops`, `hostops_smoke`), never by group, and an unregistered group-name file would dangle. | `spec/g06_windows.rs` + `proofs/g06_windows.rs` are added to `rules/kani/src/spec/mod.rs` and `proofs/mod.rs` beside the ungated `cond_ops` pair; the files are new, not assumed (§9.4). |
| R20 | §9.1 named tests at paths that do not exist (`gap_window.rs` g19/g20 *do* exist, but the new ones were placed in unnamed files) and omitted the existing suites that pin the behaviour being changed. | One new `rules/tests/tests/g06_windows.rs`, plus the changed suites listed in §9.1. |
| R21 | Names were abbreviated and two of them invented units: `CounterRecord` beside `CounteractEntry`; `joiner(i)` without saying what it returns; `answered_kind(i)` without saying `i` counts from the root; the "chain cursor" was never named. | The header's naming rules; `CounteractRecord`; `joiner`/`joiner_shielded` documented as index-into-declaration-order; `answered_kind` documented; the cursor is `CounteractChainCursor`. |
| R22 | §1.6 and the item table's footnote said the empty-links half is "A", but the same paragraph made the matcher rule depend on the copy machinery. | The matcher half is A (no wire field, `all_kinds` only rides the manifest); `can_counteract` and the body stay B (§6.1, §6.9). |
| R23 | §11's open questions 1 and 6 asked things the design had already decided elsewhere (§5.4 nesting depth, G02's `total_stage`), and question 8 was answered by master: `Counteracted` is an `OnKind` alias (`abi.rs:2105-2106` documents it as unraised, yet `cant_look_away` declares it). | §11 keeps only live questions; nesting and the alias are stated in the design. |
| R24 | §10 had A1–A6 with no A2 (the draft's own text says "Rejected for…" after A1 and starts again at A3). | §11 is a complete A1–A9. |
| R25 | The draft's §1.4 "demote to A" and §1.6 half-A both demote per *fix size*, not per surface. Item 4's event kind is A but the filter that keeps tsugumi off non-intervals is item 3's, so landing 4 alone over-offers. | Item 4 is B with an A sub-branch that may land early (§2.4); item 6 is A+B as in R22. |
| R26 | §1.9 said the current join "only works when the answered effect happens to carry the joiner as its own `target` (a per-seat pay leg)". Re-reading `can_counteract` (`net_error.rs:58-65`) the Effect guard requires `target == player_id`, which is *always* false for a bystander — so **no** single-target effect ever offers a joiner, per-seat or not. | §2.9 states the stronger fact. |

## 1. Items

| # | slug | sev | rows | disposition |
|---|------|-----|------|-------------|
| 1 | `net-error-join-draw-ownership` | medium | 942 | B: `join::joiner_count` / `joiner` / `joiner_shielded`; the draw moves to the opener |
| 2 | `net-error-join-used-getter` | low | 1312 | B: `join::shield` (the reader); alias keeps `set_join_used` |
| 3 | `extreme-range-filter-missing` | medium | 158, 982, 990 | B: `prop::RANGE_CARD`, `NumericShape`, `trigger::extreme_armed` |
| 4 | `event-effects-not-counterable` | medium | 980 | B: `ChainKind::Event` (A sub-branch) + item 3's filter + the event extreme scope |
| 5 | `vocal-split-leg` | medium | 21 | B: `AnswerPayShape::Whole` (reads G02's `total_stage`) |
| 6 | `mortis-empty-counteract-links` | medium | 313, 1045 | **A** for the matcher (`all_kinds`); B for the copy (`can_counteract`) |
| 7 | `repaint-counter-link-roll` | medium | 838 | B: payload carry-forward + face propagation to the roll link |
| 8 | `cla-counterer-set` | high | 426, 1060 | B: `World.countered_this_turn` + `ctx::countered_by_this_turn` |
| 9 | `mana-nested-window` | medium | 845, 1090 | B: the join window (`join::open`, `Priority::over`, the `JoinOpen` request) |
| 10 | `mana-joiner-count` | medium | 844, 1094 | B: `join::joiner_count` (report, not a substituted flag) |
| 11 | `nowsumimi-cancelled-guard` | medium | 936, 940 | B: `trigger::pending_negation` + `AnswerEffect::NEGATES` |
| 12 | `rain-designates-prop` | medium | 1510 | **A** for the missing property; B for the join's designation record (rain's two body bugs are already fixed) |
| 13 | `ordinary-effect-window-not-post-landing` | medium | 395 | B: the `abnormal` pipeline splits into refused-before / landed (rides G12) |
| 14 | `own-stage-immuneall-ordering` | medium | 1527, 1643 | B: the same split; the card needs no new import |
| 15 | `mortis-copy-effect-unimplemented` | medium | 312, 315 | B: a counter link carries `copied`, the matcher resolves the source entry |

Group coverage against `BACKLOG.md` section B: `join-window-report` (1, 2, 9,
10, 12), `extreme-range-event-counter` (3, 4), `counter-link-move-face` (7),
`counteraction-history` (8), `window-timing-negation` (11, 13, 14),
`mortis-copy-links` (6, 15). Items 1, 9 and 10 share the join; 3, 4, 11 and
14 share the pending/declared readers; 6 and 15 share one card and one
surface.

## 2. Problem, per item

Each entry names the verbatim fragment, the current site, what the book
requires and the root cause. Root causes are **R1-R5** defined in §3.

### 2.1 `net-error-join-draw-ownership` — Sumimi:（真奈）歌唱大赛5连冠, row 942

> 「若以此种方式使你免于受到该影响，打出那张[反击]卡的玩家可抽一张卡」

The draw is the **opener's** clause, chosen by the opener's card. It lives in
the *joiner's* body (`net_error.rs:106-114`) inside a `join_redirect_target()`
branch, and it is unconditional although the book's 「可」 asks
(`CARD-REVIEW` §`（真奈）…`, review line 2603 and the net_error line-114
entry). Two consequences: any other joining counter cannot draw at all, and
`mana_champion` cannot see **who** joined or whether the joiner actually
shielded, so it can neither run the clause nor the fallback of 2.10 itself.

Expected: the opener reads a per-seat report after the join round and asks
each shielding joiner. **Root fix:** the join is a window with a report (R2,
R3).

### 2.2 `net-error-join-used-getter` — same card, row 1312

`ctx::set_join_used()` (`card-sdk/src/ctx.rs:3377`) writes `Trigger.join_used` through
`ruleset_set_join_used` (`hostfns.rs:3855-3858`); there is **no reader** —
neither a `ctx::join_used()` nor a host import
(`CARD-REVIEW` §`（真奈）…`, the `net_error.rs:108` missing-todo entry). The
write lands on the link the joiner answers, which is where the opener could
read it, but nothing exports it. The only surrogate reader is
`ctx::designation_cancelled` (`mana_champion.rs:89`), a different fact.
`ctx::self_target` (`card-sdk/src/ctx.rs:3383`) has zero callers.

Expected: a named mark the opener reads per joiner. **Root fix:** R3.

### 2.3 `extreme-range-filter-missing` — AG:（鸫）微小的『能做到』的事, rows 158, 982, 990

> 「使任意结果只有数字区间的效果以理论最大值或最小值结算，除@Tsugu ycm
> 以外无法直接改变骰子点数（ex：可以对YOLO生效但无法对无论是何种颜色的夕阳
> 生效）」

`can_counteract` (`tsugumi_can.rs:39-46`) returns `true` for every `card`
trigger; the ycm exception exists only as a comment (`:41-44`), and nothing
in the guard could act on it even if it were coded. Three defects:

1. **No numeric-shape predicate.** Nothing on the link or on the entry says
   whether the answered card's result is a pure number interval, so the card
   is offered against every card play, including those with no roll and the
   sheet's own negative example (`CARD-REVIEW` §`微小的…`, the
   `tsugumi_can.rs:39` and `:41` entries).
2. **No stacking guard.** `turn.extreme` is only written at *resolution*
   (`set_extreme`, `tsugumi_can.rs:108`; the ask is `:99`), and every offer for a window is
   collected before any body runs, so a second copy sees `0` and re-picks;
   under LIFO the earlier declaration's pick wins
   (`CARD-REVIEW`, the `:73` entry).
3. **The ycm exception is unreachable**, and `yolo` / `tsugu_ycm` declare no
   property the filter could read.

Expected: the offer is gated on the answered card being a number-range card,
and on no extreme being armed. **Root fix:** R1 (no contract) and R4
(guards see a pre-resolution world).

### 2.4 `event-effects-not-counterable` — same card, row 980

「任意…效果」 has no source restriction. The entry lists
`ChainKind::Card` only (`tsugumi_can.rs:21`), and `draw_event`
(`play.rs:5833-5900`) never scopes `turn.extreme` — the save/arm/restore pair
exists only in `play_from_hand` (`play.rs:5685-5692`, `:5797-5799`). An
event-armed extreme therefore leaks into the next play, and an event is never
offered (`CARD-REVIEW`, the `:19` entry, probed with `event:飞鸟山之战`).

**A sub-branch:** `ChainKind::Event` is an existing constant and
`turn.extreme` is existing saved state, so the kind and the lifetime can land
without new surface. The filter that keeps the offer correct is item 3's.

### 2.5 `vocal-split-leg` — CRYCHIC:主唱太拼命了, row 21

> 「一次性向其他玩家支付至少5000资金时，免除此次支付。」

「一次性」 is one transfer, excluding a 「[分摊]」 share (`spec/cards/crychic.md:77-82`).
Every split leg sets `Pay::runs_total_stage = false` (`hostfns.rs:793`, the
split entry; the field's meaning is `play.rs:399-403`), so the leg skips the
command-wide stages and runs its own pipeline (`money_inner`,
`play.rs:3997`), and the pay trigger carries the same flag
(`TriggerPay.total_stage`, `world.rs:161`). The card already spells the rule
in its **condition** (`pay_total_stage` and `value >= 5000`,
`vocal_too_hard.rs:28`), which is the right predicate but the wrong place: it
is re-evaluated from a world copy for every candidate card although the fact
is a fixed property of the link, and it is duplicated by every future
whole-payment counter. Worse, the window only raises on the legs, so a
counter that needs the whole figure is dead for exactly the payments
「一次性」 is meant to keep (`CARD-REVIEW` §`主唱太拼命了`; the card's own TODO
at `:22-27`). G02 §2.20's `payTotalBefore` window is the prerequisite; this
group supplies the contract field so the condition can shrink to the amount
threshold alone.

Expected: the offer is gated by the entry on a whole transfer (`total_stage`
true). **Root fix:** R1.

### 2.6 `mortis-empty-counteract-links` — Mujica:（睦/mortis）表演的本能, rows 313, 1045

> 「（此卡复制[反击]卡时可在符合条件时打出）」

`mortis_instinct.rs:21` declares `On::Counteract(&[], …)`. `On::triggers()`
maps the empty slice to an empty vector (`lib.rs:1585`), the host builds
`counteract_mask` from those spellings (`host.rs:1046-1135`), and
`CardInfo::entry_iter` requires a spelling match (`host.rs:143-151`), so the
entry is never offered (`CARD-REVIEW` §`表演的本能`, the `:14` entries). The
README convention "empty = never" is right for `Message` and wrong here: a
copy's kinds are the copied card's and are dynamic.

The dispatch half is A (one manifest flag plus the matcher). The
`can_counteract` half (`mortis_instinct.rs:36-40` returns `false`) needs the
copy (2.15, B).

### 2.7 `repaint-counter-link-roll` — RAS:Repaint, row 838

> 「当任意其他玩家进行移动掷骰并进入移动阶段后，打出此卡…使目标玩家的此次移动数-X」

The condition needs `move.roll != null` (`repaint.rs:20`). A counter link is
synthesized bare (`CoreTrigger::new("card"|"cardUsed", cursor)`,
`wasm_rules.rs:6846-6847`), so it carries no `mv`, the body early-returns
(`repaint.rs:55-57`) and the condition fails; and the link's *kind* is
`card`, never `DiceRollResult`, so a card declaring only that kind is not
offered at all (`CARD-REVIEW` §`Repaint`, the `:17` entries).

Two further facts decide the write half. `set_move_roll` writes the link the
counter answers (`resolve_jobs:7255`), and the roll link is `chain[0]` — the
**root** — so a repaint answering the root works once the payload rides
forward. A repaint answering a counter writes to that counter's link, which
only that counter reads next; the outer counter settles *later* under LIFO
and writes the face it computed, so the cut is lost unless the write is
propagated to the roll link (R11). `yolo.rs:48` is additive
(`before + n`), which is what makes composition possible.

Expected: the payload rides forward and the cut reaches the roll that
settles. **Root fix:** R1, R4.

### 2.8 `cla-counterer-set` — Mujica:无法将视线移开, rows 426, 1060

> 「使当前回合内对你打出过[反击]的所有玩家向你选择的**方向**强制移动
> 1~4以内的任意步数并[触发结算]」

No per-turn counteraction history exists. `run` (`cant_look_away.rs:53-124`)
falls back to `trigger::player_id()`: on the counter path that is the
immediate counterer (one seat), and on the hand-play path it is the actor, so
`counter_user == player_id` and the body returns at `:73-75` and moves
**nobody** — the On::Play entry is dead (`CARD-REVIEW` §`无法将视线移开`, the
high-severity `:65` entry and `:105`). `ctx::turn_key()` names the turn but
cannot rebuild the set (`BACKLOG ABI-03`).

The `On::Play` and `On::Counteract` entries share `run`
(`cant_look_away.rs:17`, `:26-30`) with no way to say "the target set is the
history, not the actor", so the fix invents one. The self-move clause
(「若使用者在自身回合内选择了使用者自己…」) is unreachable until the set
exists.

### 2.9 `mana-nested-window` — Sumimi:（真奈）歌唱大赛5连冠, rows 845, 1090

> 「此时场上其他玩家可如同自身的对应目标被指定一般打出[反击]卡，且其
> 反击卡中针对打出玩家自身的效果改为你」

The engine has only a declaration-scoped stamp: `SELF_REDIRECT`
(`abi.rs:1465`), the pre-scan (`wasm_rules.rs:6716-6725`), the declaration
set/re-arm (`:6865-6874`) and the bridge reset (`:8106-8107`). There is **no
window**: no participating-seat set, no nested round, no presentation of the
joiner as the target, no per-joiner state. A joiner's guard that checks
"am I the target" therefore never passes — `can_counteract`'s Effect branch
requires `target == player_id` (`net_error.rs:60-65`), and `trigger::target()`
is the real recipient. A *multi-target* effect (武道馆) lists every seat, so
its joiner passes by accident; a **single-target** effect never does —
`cross_chain::c23` and `gap_window::g19` only pin the multi-target and
loose-condition cases, and a card targeting one seat (`net_error` itself,
`:60-65`) is offered to nobody (`CARD-REVIEW`, the `:24`/`:28`/`:50`
entries). The pre-scan also arms the redirect for a seat that merely *holds*
the card without playing it (`CARD-REVIEW`, the `wasm_rules.rs:5973` CSV
entry, which is `:6718-6729` of master, probed).

Expected: a nested round over the other seats, each offered as if the target,
each able to redirect its self-targeted clause to the opener. **Root fix:**
R3.

### 2.10 `mana-joiner-count` — same card, rows 844, 1094

> 「若没有人在此卡的效果期间打出[反击]卡，你抽一张卡。」

The fallback needs "did **anyone** declare a counter during this card's
effect", not "was a designation cancelled": a joiner who shielded nothing
still bars the draw, and a designation cancelled by a non-joiner does not
(`CARD-REVIEW`, the `:55` and `:66` entries). Today
`mana_champion.rs:89` substitutes `designation_cancelled` for both the
fallback and the joiner's own draw (`:98`).

### 2.11 `nowsumimi-cancelled-guard` — Sumimi:现在她是Sumimi的小初啦, rows 936, 940

> 「当…失去资金的总额即将超过你所在格子的[收费标价]时打出此卡」

A payment an earlier counter already negated (`set_cancelled`) still reads
its declared `value`, so `now_sumimi` is offered — and gains — for a payment
that will never settle. Guards run at offer time against a link clone, and
declared answers are only folded into the link at resolution, so there is no
pending-negation signal (`now_sumimi.rs:23-29`, `CARD-REVIEW` §`现在她是…`).
**Root fix:** R4.

### 2.12 `rain-designates-prop` — 通用:雨啊，快点来吧, row 1510

> 「投掷2d2并记录结果为X。[指定]X名玩家（其中必须包括[使用者]）」

`designations(seat)` is the **static** designation list: it answers
`world.others` only when the play's card declares `prop::DESIGNATES`
(`wasm_rules.rs:2213-2222`). rain's def (`rain.rs:15`) declares no property,
so every reader — above all net_error's clause 1, which branches on
`ctx::designations(...).is_empty()` (`net_error.rs:50-53`, `:85-86`) — sees
an empty list and takes clause 2 (cancel the whole play) instead of clause 1
(cancel one designation) (`CARD-REVIEW` §`雨啊，快点来吧`, the `:15` entry).
Two body bugs the draft also listed are already fixed on master: the
mandatory user goes through `ctx::target` (`rain.rs:35`), so its designation
is in the cancel pipeline, and a refused pick `continue`s (`:56-58`).

Under a join the designation also has to become readable: net_error's
`counteract`'s `Effect` arm (`net_error.rs:142-150`) is unreachable under a
join, because `can_counteract` refuses before it, so the card would be
consumed for nothing (`CARD-REVIEW` §`网络链接异常`, the `:148` entry). The opener can
neither see which designation a joiner named nor report it.

Surface: the property (A) plus a per-joiner designation record (B). **Root
fix:** R3.

### 2.13 `ordinary-effect-window-not-post-landing` — MyGO:普通与理所当然, row 395

> 「[反击] 受到异常移动效果影响后可打出」

The `effect` window opens while the abnormal is still being *declared*:
`abnormal_gate` raises it (`wasm_rules.rs:6273-6285`) **before** the
`ImmuneAll` check (`:6289-6291`), so a fully protected player still spends
`ordinary` on a move that never happens and every `effect` guard reads a move
that may be refused (`ordinary.rs:17-19`; `CARD-REVIEW` §`普通与理所当然`).

Expected: the refusal (immune, gate) first, then the window for what landed.
Surface: the abnormal pipeline gains an explicit refused/landed split; a
refused abnormal raises a hook-only `abnormalRefused` with no window. G12
§5.1 owns the gate the raise must move behind.

### 2.14 `own-stage-immuneall-ordering` — R:选择自己的舞台, rows 1527, 1643

The card half of 2.13: `own_stage.rs:16-19` states the same ordering gap and
`can_counteract` (`:44-48`) reads the effect list of a declaration that may
still be refused. Same surface; the card needs no new import. Its own
settle answer is G03 §4.7's.

### 2.15 `mortis-copy-effect-unimplemented` — Mujica:（睦/mortis）表演的本能, rows 312, 315

> 「此卡打出时效果为场上其他玩家打出的上一张卡（不计入效果带有放置于场上
> 的卡）」

`run` is a bare log (`mortis_instinct.rs:45-56`): no play-history scan, no
placed-card filter, no execution of the copied card's effect (`CARD-REVIEW`
§`表演的本能`, the `:50` entry). The B half of 2.6: the entry declares
`copies_previous`, the matcher's `counteract_entry` walks the play history
back to the most recent other player's non-placed card, resolves **its**
Counteract entry for the link (with its own contract) and sets
`link.copied`; the copied body runs, `trigger::link_copied()` naming the
source, and mortis's own `run` is deleted. Which card was copied and its
memory is the CH-05 question and is read, not decided, here; the `all_kinds`
fallback of 2.6 stands for a link whose copy is unresolvable.

## 3. Shared root causes

1. **A counteraction entry declares only kinds.** Its contract with the link
   — which numeric shape, which payment shape, what it does to the link — is
   not part of the ABI, so the offer index can ask only "does the kind
   match". Every over-offer here (2.3, 2.5) is that hole; every under-offer
   (2.6, 2.7) is the same hole from the other side.
2. **A counter link is a synthetic stub.** It is built with kind, seat and
   card only; the answered link's payload (`mv`, `pay`, roll, effects,
   subject, extension tag, join) is dropped (`wasm_rules.rs:6846-6847`,
   `bridge_trigger:8023-8110`). An entry answering a counter cannot see what
   it answers, and a later counter cannot see what the counter answered.
3. **`[反击]`-window state is ad-hoc trigger fields.** `join_redirect` and
   `join_used` are two scalars on one link, written by one card, read by
   almost nobody, reset by the bridge, unsaved, with no per-seat grain. An
   embedded scalar cannot express "which seats, which of them declared,
   which shielded, what each designated".
4. **Guards and bodies see different worlds.** Guards run at offer time on a
   link that carries no resolution state; bodies run at resolution. Anything
   pending — a declared negation, a declared extreme — is invisible exactly
   where the book needs it, and the window opens before a refusal exists.
5. **The turn is the enemy of "all players this turn".** The engine counts
   gains (`World.gains`) but not counteractions, so a per-turn set has to be
   built from nothing.

## 4. Capability as an invariant

> **A counteraction entry declares the numeric shape and the payment shape it
> answers; every declared counter is a full link carrying the answered
> payload forward; a join is a first-class window with a participating-seat
> set, a per-seat declaration record and its own nested round; and every fact
> a guard needs about a *declared* answer is readable before resolution.**

Derived obligations:

* **Contract exactness.** An entry is offered **iff** its declared kinds, its
  numeric shape and its payment shape all admit the answered link. No card
  body is asked before the contract passes.
* **Payload carry-forward.** A counter link exposes everything the link it
  answers exposed (move, roll, pay shape, effect list, subject card, join),
  with the counter's own facts added (`card` = the counter, `by_card` =
  declarer, `answered` = parent), and `chain_depth` says how deep it is.
* **Face composition.** A counter's face write lands on the link it answers
  **and** on every enclosing roll link, in settle order, so a later-settling
  answer reads the composed face and the root settlement reads the final one.
* **Join is a window, not a flag.** The opener's body opens a nested round
  restricted to the other live seats; each seat is offered as if it were the
  target; each seat carries its own declared / shielded / designated record;
  the round settles to completion before the opener's body continues; the
  opener reads the report.
* **History is turn state.** Every declaration appends `(declarer, countered,
  card)` once, before any body; the list is readable for the rest of the turn
  and empty on the next.
* **Pending is visible to guards.** A guard can read how many declared,
  unresolved answers on this link declare that they negate, and whether an
  extreme is already armed.
* **Landing precedes the window.** For `abnormal`, the refusal is decided
  before any `effect` window opens; a refused abnormal raises its own
  hook-only point.

## 5. Surface

Everything is generic mechanism: no display text, no player decision inside
an import.

### 5.1 `card_sdk::abi` — the entry contract

`CounteractEntry` (`lib.rs:569-601`) gains three fields and one defaulted
builder each. The positional `On::Counteract(&[…], condition, guard, body)`
signature is unchanged; the new fields default.

```rust
/// What numeric shape the answered effect must have.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq,
         serde::Serialize, serde::Deserialize)]
pub enum NumericShape {
    #[default]
    Any = 0,
    /// The answered link's *subject* card declares `prop::RANGE_CARD`.
    NumberInterval = 1,
}

/// What payment shape the answered link must have (`pay` / `effect`-on-Pay).
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq,
         serde::Serialize, serde::Deserialize)]
pub enum AnswerPayShape {
    #[default]
    Any = 0,
    /// `TriggerPay.total_stage` — one whole payment, never a 「[分摊]」 share.
    Whole = 1,
}

/// What the entry's body does to the link, for the offer-time readers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq,
         serde::Serialize, serde::Deserialize)]
pub struct AnswerEffect(pub u8);

impl AnswerEffect {
    pub const NONE: Self = Self(0);
    /// The body may call `trigger::set_cancelled` / `negate_effect` / `spare`.
    pub const NEGATES: Self = Self(1);
    /// The body may call `ctx::set_extreme`.
    pub const ARMS_EXTREME: Self = Self(2);

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}
```

```rust
pub struct CounteractEntry {
    // …existing fields…
    pub numeric: crate::abi::NumericShape,   // NEW, offer filter
    pub pay_shape: AnswerPayShape,           // NEW, offer filter
    pub effect: crate::abi::AnswerEffect,    // NEW, pending readers only
}
```

Builders (sugar that writes the field into the card table at build time,
nothing at runtime):

```rust
impl On {
    pub const fn answers_numbers(self, n: abi::NumericShape) -> Self;
    pub const fn answers_whole_payments(self) -> Self;
    pub const fn declares_negation(self) -> Self;   // |= NEGATES
    pub const fn arms_extreme(self) -> Self;        // |= ARMS_EXTREME
    pub const fn copies_previous(self) -> Self;     // the entry is the copied card's
}
```

`prop::RANGE_CARD = "rangeCard"` is a **new card property** beside
`SELF_REDIRECT` (`abi.rs:1465`): `1` = the card's result is a pure number
interval. `AG:无论是何种颜色的夕阳` does not declare
it (the sheet's own negative example); `AG:Y.O.L.O` and `通用:@Tsugu ycm` do.

### 5.2 `ManifestOn` — wire fields

Five trailing fields, all `#[serde(default)]` (postcard is not
self-describing, so they must stay last):

```rust
#[serde(default)] pub numeric: NumericShape,
#[serde(default)] pub pay_shape: AnswerPayShape,
#[serde(default)] pub answer_effect: AnswerEffect,
#[serde(default)] pub all_kinds: bool,         // a Counteract entry with no links
#[serde(default)] pub copies_previous: bool,   // dispatch the copied card's entry
```

`ManifestOn.all_kinds` is set by the manifest conversion
(`lib.rs:1583-1585`) when `On::Counteract` carries an empty `links` slice.
It is explicit rather than inferred from `triggers.is_empty()`, because an
empty list means "never" for `Message` and every other entry kind; the flag
also lets `entry_iter` (`host.rs:143-151`) answer a kind query without a
second rule.

The two enums and the bit set decode through the v107 wire-code-table rule
(`abi.rs:828-829`): an unknown `numeric` / `pay_shape` value falls back to
`Any`, an unknown `effect` bit is ignored.

### 5.3 Trigger payload and link state

```rust
pub struct Trigger {                       // crates/game-rules/src/world.rs:35
    // …
    /// The card whose effect this link *is*: the raised play's card on a root
    /// link, the counter card's own id on a counter link (so an entry answering
    /// it sees the counter as its subject). Stamped for roots at the bridge
    /// (the raise's `card`, else the innermost resolving frame).
    pub subject_card: Option<String>,      // NEW, carried forward
    /// The join window opened on this link, when one is open.
    pub join: Option<TriggerJoin>,         // NEW
    /// Declared, unresolved answers on this link (indices into the chain, in
    /// declaration order). Empty on a new counter link. Feeds §6.2's readers.
    pub answers_declared: Vec<u32>,        // NEW
    /// The card a copy resolves (mortis, §2.15). Set by `counteract_entry`
    /// when it resolves a copy, read by `trigger::link_copied`. `None` else.
    pub copied: Option<String>,            // NEW
}

pub struct TriggerJoin {
    /// The `SELF_REDIRECT` declarer.
    pub opener: i32,
    /// Participating seats, seat order; only these are offered.
    pub seats: Vec<i32>,
    /// Per seat, parallel to `seats`: declared a counter in this window.
    pub declared: Vec<bool>,
    /// Per seat, parallel to `seats`: the counter landed and called `shield`.
    pub used: Vec<bool>,
    /// Per seat, parallel to `seats`: the designation the joiner named
    /// (`ctx::cancel_designation` while this body runs), `-1` when none.
    pub designated: Vec<i32>,
    /// Has `open()` completed its round at least once this pass?
    pub opened: bool,
}
```

`countered_this_turn` state (5.6) and the join both ride `World`/`Trigger`,
which save (7). `join_redirect: Option<i32>` and `join_used: bool` stay as the
compatibility view: `join_redirect` mirrors `join.opener`, `join_used` is
`join.used.iter().any(|&x| x)`. `ctx::join_redirect_target()`,
`ctx::set_join_used()` and `ctx::self_target()` keep working and become
aliases.

The engine `CoreTrigger` (`rules.rs:367`) mirrors `subject_card`, `join`
(`JoinState` with the same shape), `answers_declared` and `copied`, so
`bridge_trigger` and the write-back carry them without a second encoding.

### 5.4 Imports (`card_sdk::ctx`, re-exported through `bandori_std::ctx`)

```rust
pub mod join {
    /// The opener when this counter answers inside a join window.
    pub fn opener() -> Option<i32>;
    /// The participating seats of the window this link answers.
    pub fn seats() -> Vec<i32>;
    /// This joiner's counter shields the opener: marks `used` for the running
    /// seat and returns `true`. `false` (and no mark) outside a join.
    /// Always applied, idempotent (see §6.4).
    pub fn shield() -> bool;
    /// Opener side: build and settle the nested join round on the answered
    /// link. `Err(Prompt)` if the round pauses. `true` when it completed.
    pub fn open() -> Result<bool, Prompt>;
    /// Opener side, after `open()`: the round was built.
    pub fn opened() -> bool;
    /// Opener side: how many seats **declared** (bars the fallback).
    pub fn joiner_count() -> i32;
    /// Opener side: the i-th declarer, in declaration order (`-1` if none).
    pub fn joiner(i: i32) -> i32;
    /// Opener side: did declaration `i` land and shield?
    pub fn joiner_shielded(i: i32) -> bool;
    /// Opener side: the designation declaration `i` named (`-1` if none).
    pub fn designation(i: i32) -> i32;
}
```

```rust
pub mod trigger {
    /// Declared, unresolved answers on this link whose entry declares
    /// `AnswerEffect::NEGATES`.
    pub fn pending_negation() -> i32;
    /// `turn.extreme != 0` on the current play, or a declared answer on this
    /// link declaring `ARMS_EXTREME`.
    pub fn extreme_armed() -> bool;
    /// See `Trigger::subject_card`.
    pub fn subject_card() -> Option<String>;
    /// The card a `copied` counter link plays (§2.15); `None` otherwise.
    pub fn link_copied() -> Option<String>;
    /// 0 = the root link, n = n counters deep.
    pub fn chain_depth() -> i32;
    /// The kind of the link at depth `i`, counting up from the root;
    /// `None` when `i` is out of range or the link is not on the chain.
    pub fn answered_kind(i: i32) -> Option<TriggerKind>;
}

/// Distinct players who declared a [反击] against `seat` this turn, in
/// declaration order. Empty on the next turn.
pub fn countered_by_this_turn(seat: i32) -> Vec<i32>;
```

Operation count: nine `join`, six `trigger`, one `ctx` — fifteen (9).

### 5.5 Condition vocabulary (`crates/cond`, window variables)

`subject_range_card` (bool), `extreme_armed` (bool), `pending_negation`
(int), `chain_depth` (int). A split leg is G02's `pay_total_stage` (`0`), so
this group adds none for it. All four are documented in `GUARDS.md` §4.2 next
to the other effect-window variables. Purely additive; no wire id. The
verified scalars are the cheap index; the conditions are the escape hatch
(§11 A4).

### 5.6 State

```rust
// World, game-core/src/engine/world.rs (beside `gains`)
/// [反击] declarations made this turn, in declaration order.
#[serde(default)]
pub countered_this_turn: Vec<CounteractRecord>,
```

```rust
pub struct CounteractRecord {
    pub declarer: i32,
    pub countered: i32,
    pub card: String,
}
```

`World::reset_countered(&mut self) { self.countered_this_turn.clear(); }` is
called from `turn_start` (`play.rs:733`, beside `reset_gains`).

### 5.7 Classification

| item | NEW | EXTENDED | WIRE-BREAK | NO-WIRE |
|---|---|---|---|---|
| 1 | `join::{open,opened,joiner_count,joiner,joiner_shielded}` | `Trigger.join` | `ManifestOn` | |
| 2 | `join::shield` replaces the read gap | `join_used` alias | | yes |
| 3 | `NumericShape`, `prop::RANGE_CARD`, `extreme_armed`, `pending_negation`, `ARMS_EXTREME` | `CounteractEntry` | `ManifestOn.numeric` | |
| 4 | — | card kind list, event extreme scope | | yes |
| 5 | `AnswerPayShape` (reads G02's `total_stage`) | `TriggerPay` (read only) | `ManifestOn.pay_shape` | |
| 6 | `ManifestOn.all_kinds` | matcher, `Ruleset::build` | `ManifestOn.all_kinds` | |
| 7 | `subject_card`, `chain_depth`, `answered_kind` | counter-link build, `bridge_trigger`, `resolve_jobs` | | |
| 8 | `countered_by_this_turn`, `CounteractRecord` | `World` | | (save only) |
| 9 | `join::*` window, `HostRequest::JoinOpen`, `CounteractChainCursor` | `Priority`, `Trigger.join` | `ManifestOn` (via 1) | |
| 10 | `join::joiner_count` | | | yes |
| 11 | `pending_negation` | `NEGATES` | `ManifestOn.answer_effect` | |
| 12 | `join::designation`, `TriggerJoin.designated` | `DESIGNATES` usage, join record | | |
| 13/14 | `abnormalRefused` (engine kind 121, hook-only) | `abnormal_gate` order | engine kind 121 | |
| 15 | `trigger::link_copied`, link `copied`, `copies_previous` | `counteract_entry` | `ManifestOn.copies_previous` | |

**The wire break is the five trailing `ManifestOn` fields** (postcard, not
self-describing). The imports alone would not need a bump; the manifest shape
does.

## 6. Engine wiring

### 6.1 Offer contract (items 3, 5, 6)

`Ruleset::build` (`host.rs:1037-1135`): an `OnKind::Counteract` entry with an
empty `triggers` sets that entry's `all_kinds` and inserts **every
non-hook-only kind** into the card's `KindMask`:

```rust
if ok == Some(OnKind::Counteract) && o.triggers.is_empty() {
    for kind in every_non_hook_only_kind() {
        declared.insert(kind).map_err(RuleError::Load)?;
        cm.insert(kind).map_err(RuleError::Load)?;
    }
}
```

`every_non_hook_only_kind()` walks `game_kinds::ENGINE_KINDS`
(`game-kinds/src/lib.rs:42`) skipping `hook_only`, and the loaded ruleset kind
table (`kind_flags`) skipping `hook_only` ruleset kinds. Side effect to
accept: while Mortis is in the set `ruleset.declares(kind)` is true for every
kind, so `counteract_orig` (`wasm_rules.rs:5462`) builds one bridge trigger
per raise instead of early-outing. That is one clone per raise — cheap — and
is the price of a dynamic entry.

`counteracts_to_play` (`host.rs:2501`) ORs `counteracts_to(card, kind)` with
the designates rule (`trigger_designates`, `host.rs:2485`), then admits only
when the entry contract passes. `counteract_entry` (`host.rs:2507`) must not
return "the first entry by kind": a card may carry several Counteract
entries, one per shape. It scans the card's Counteract entries in order and
returns the first whose contract admits:

```rust
fn counteract_entry(&self, card: i32, t: &crate::Trigger) -> Option<i32> {
    let info = self.cards().get(card as usize)?;
    let kinds = [Some(crate::TriggerKind::from_str(&t.kind)),
                 self.trigger_designates(t).then_some(crate::TriggerKind::Effect)];
    for kind in kinds.into_iter().flatten() {
        for (i, o) in info.on.iter().enumerate().filter(|(_, o)| {
            OnKind::from_i32(o.kind) == Some(OnKind::Counteract) && o.triggers.iter().any(|s| *s == kind.spelling())
        }) {
            if contract_admits(info, o, t) { return Some(i as i32); }
        }
    }
    None
}

fn contract_admits(info: &CardInfo, o: &ManifestOn, t: &crate::Trigger) -> bool {
    match o.numeric {
        NumericShape::Any => {}
        NumericShape::NumberInterval => {
            let Some(subject) = t.subject_card.as_deref() else { return false };
            if info.props.get(prop::RANGE_CARD).copied() != Some(1) { return false; }
        }
    }
    match o.pay_shape {
        AnswerPayShape::Any => {}
        AnswerPayShape::Whole => {
            let Some(p) = &t.pay else { return false };
            if !p.total_stage { return false; }   // a 分摊 leg
        }
    }
    true
}
```

An entry with `copies_previous` (mortis) is dispatched in place of the card's
own: `counteract_entry` walks the play history back to the most recent other
player's card that is not placed on the field, resolves **that** card's
Counteract entry for the same kind with the same contract, sets
`link.copied` and returns the source id, so the body that runs is the copied
card's. The history scan and its memory are CH-05's; the `all_kinds` mask is
what gets a copying card offered at every kind.

`counteracts_to_play`, `any_counter_candidate` (`:7434`) and `declare_one`
(`:7487`) inherit both unchanged. `RANGE_CARD` is read from the **subject**
card, never from the counter: on `card`/`cardUsed` counter links the subject
is the answered counter (`:6868`), so tsugumi answering YOLO's counter reads
YOLO's property and arms YOLO's `1d4` — the book's own 「可以对YOLO生效」.
The anti-stacking lock of `dice_cast` (`slot 'noCounteractTurn'`,
`dice_cast.rs:34`) still gates a hand counter through
`can_counteract_now` (`:8425`), so `all_kinds` does not reopen it.

### 6.2 Pending readers (items 3, 11)

Both are computed from the chain at offer time, never from saved state. The
guests reach the chain through a **counteraction chain cursor** the settle
walk installs on the PlayCtx around every counter body and every offer:

```rust
// PlayCtx (game-core/src/engine/cx.rs), type-erased like `counteract_resume`
pub(crate) counteract_chain: Option<Box<dyn std::any::Any>>,

// game-rules
pub struct CounteractChainCursor {
    /// The live chain. Valid for the duration of one drive / one offer.
    pub chain: *mut Vec<ChainLink>,
    /// The node being offered / driven (its `answered` is the link).
    pub index: usize,
}
```

```rust
fn pending_negation(&self, cursor: &CounteractChainCursor) -> i32 {
    let chain = unsafe { &*cursor.chain };
    let link = &chain[chain[cursor.index].answered].link;
    link.answers_declared.iter()
        .filter(|&&d| self.entry_effect(&chain[d as usize]).contains(AnswerEffect::NEGATES))
        .count() as i32
}
```

`answers_declared` is appended at declaration (`:6859-6863`), before any
body runs, and `build_round`'s re-offer of the same seat sees it — which is
what makes 「一次副本就够」 work without waiting for resolution.
`extreme_armed` ORs `turn.extreme != 0` (the existing `extreme` host
function, `hostfns.rs:2012`) with the same `ARMS_EXTREME` scan.

This is **declared intent**, not outcome: the entry promises "my body may
negate". The bit is the entry's own contract and is checked per
`set_cancelled` card by the static sweep in §9.3.

### 6.3 Counter links carry the payload forward (item 7)

In `build_round`, replace the bare `CoreTrigger::new` (`:6846-6847`) with a
copy of the answered link followed by an override block:

```rust
let mut link = chain[timing].link.clone();   // mv / pay / effects / subject / join / tags
link.kind        = if uid >= 0 { "cardUsed" } else { "card" };
link.player_id   = cursor;
link.by_card     = Some(cursor as i32);
link.card        = Some(id.clone());
link.subject_card = Some(id.clone());        // a counter's own subject is itself
link.seq         = (at + 1) as u32;
link.answers     = timing as u32;
link.target      = chain[timing].seat as i32;
link.answers_declared.clear();               // its own pending set
link.negation    = Negation::None;
link.spared.clear();
link.roll_root   = nearest_roll_link(chain, timing);
```

`nearest_roll_link` walks `answered` upward and answers the first index whose
link kind is a roll-face kind (`DiceRollResult` / `DiceRollBefore` / …), else
`ROLL_ROOT_NONE`.

The ROOT link also needs a subject. `bridge_trigger` stamps it, because no
raise site knows about counteraction contracts:

```rust
let subject = t.card.clone().filter(|s| !s.is_empty())
    .or_else(|| resolving_snapshot().last().map(|(card, _, _)| card.clone()));
// … then, in the literal:
subject_card: subject,
join: t.join.as_ref().map(Into::into),
```

The innermost resolving frame is the card body whose effect this raise is
(`push_resolving_frame`, `:10332`/`:10385`); a raise from turn flow has
neither and stays `None`. `answered_kind(i)` walks `answers` from the node
upward; `chain_depth` is `i + 1` for a node at depth `i` in that walk.

`COUNTERACT_MOVE_EXTENSION` now rides forward in `mv.tags`, so an inner
counter sees the outer extension; the `retain` at `:7227-7229` / `:7251-7253`
already strips it per body.

**Face propagation.** After a counter body settles, before the write-back,
compare the body's face (`on_link.mv.roll`, else `on_link.value`) with the
pre-body clone's: when it moved and `link.roll_root != ROLL_ROOT_NONE`, copy
both `mv.roll` and `value` into `chain[roll_root].link`. Then the write-back
to the answered link proceeds as today. Because the inner answer settles
first, the outer body's `on_link` clone already carries the inner cut, and an
additive body (`yolo.rs:48` `before + n`) composes on it; the root's final
value is what `counteract_orig`'s write-back (`:5869-5876`) hands to the
engine. A counter's own declaration clone freezes the answered face, so
repaint's X (`on_path`, `repaint.rs:36-46`) still counts the path as declared
(「原本预计路径」), and the carry-forward keeps its `move_dir` the actor's.

### 6.4 Join window (items 1, 2, 9, 10, 12)

The window is captured at `open()`, not at declaration, so a seat that
leaves mid-window is dropped by `can_counteract_now`.

**Declaring.** When a `SELF_REDIRECT` counter is declared, `build_round`
creates the block instead of only stamping a seat (`:6865-6874`); the
pre-scan (`:6716-6725`) and its cleanup (`:6737-6750`) go away, which also
fixes the holder-without-play arming:

```rust
if declares_self_redirect(idx) {
    let opener = cursor as i32;
    let seats: Vec<i32> = (0..n as i32).filter(|&s| s != opener && !cx.world().out(s as usize)).collect();
    let k = seats.len();
    chain[timing].link.join = Some(TriggerJoin { opener, seats, declared: vec![false; k], used: vec![false; k], designated: vec![-1; k], opened: false });
    chain[timing].link.join_redirect = Some(opener);
}
```

**Opening (the control flow).** `join::open()` is a host request, not a
prompt. It is **always applied**: the live path consumes its logged answer
for alignment but runs the routine regardless, because an `Ask` anywhere in
the round rebuilds the whole counteraction walk (the chain is a local and
`Halt::Ask` discards the copy), and the opener's body is then re-run past the
same call. `inline::request_always` is the shared helper:

```rust
/// Like `request_or_pause`, but the routine runs on every live pass: the
/// logged answer is consumed for alignment and ignored.
pub fn request_always<C: HostCtx>(c: &mut C, req: HostRequest, pause: Pause) -> Result<i32, HostError>
```

In a simulation the inline host returns `None` for `JoinOpen`, so the same
live path runs.

The drive's `NeedHost` arm handles `HostRequest::JoinOpen { at }` like its
existing `SettleAt` branch (`:3999-4047`): append the answer `1` to the
replay log, push a `ResumeFnWork` and return `Halt::suspended()` — that is
what keeps the opener's body from re-running over a **settle** inside a
joiner's body, so already-committed join effects are not applied twice. The
closure carries the chain cursor, the driver context and the answer list, and
on each run:

1. if a `JoinResume` is parked, continue `settle_join` from it;
2. otherwise `build_round` on the answered link with `Priority::over(&seats)`,
   `depth + 1` and a fresh budget;
3. when the round is closed, collect a post-order `order` over exactly the
   joiner nodes and `settle_join` it;
4. when it completes, clear `opened`'s answer path and re-enter the opener's
   body with `drive_inner_body_from(…, answers, true)` and `set_drive_dest`.

A suspension inside `settle_join` re-pushes the closure with the advanced
`JoinResume` and returns `Suspended`; the parent `resolve_jobs` parks its
`HandResume` as usual. `JoinResume { at, order, next }` lives in the
`CounteractResume` box beside the hook list.

`settle_join` reuses the per-node settle body extracted from `resolve_jobs`
(`settle_node(cx, chain, trigger, idx) -> Flow<i32>`, the loop body at
`:7148-7290`), so logging, negation check, dest and the discarded raise are
identical; the walk simply emits no second `"settles"` banner.

**Presentation.** `Priority` (`:8335-8376`) gains an optional seat list
(`Priority::over(&[i32])`): `seat()` indexes the list and `answered()` moves
through it. For each visit `build_round` sets `link.target = cursor` (the
joiner) before the offer, so a guard written as "I am the designated target"
passes (`net_error.rs:60-65`). `by_card` stays the play's causer, so
「来自你以外」 keeps reading the causer. The answered link's `target` is
saved before the round and restored after it, so the committed effect still
aims at the real recipient.

**Shielding and designation.** `join::shield()` is always applied and
idempotent: it marks `used[k]` for the running seat and answers `true`. A
joiner's `ctx::cancel_designation(seat)` records `designated[k] = seat` when
the current body answers a join link (the host function already reaches the
current link; see G03's `cancel_designation` note). Nothing else is
automatic: the body still decides what to cancel, with `join::opener()` as
the redirect destination (「改为你」). `net_error` becomes the short body
of §6.9 (`opener().unwrap_or(player_id)` then `cancel_designation` then
`shield`).

On an accepted offer the engine sets `declared[k]` for the visited seat
before the body runs (`declare_one`, `:7487`), so the flag exists even when
the body is later negated.

**Report.** `joiner_count` / `joiner(i)` / `joiner_shielded(i)` /
`designation(i)` read the block after the round. The first two index the
`seats`-parallel `declared` list; `declared` counts even when the counter was
negated (「打出过」).

**Nesting.** A joiner's counter is a normal link and may be answered further;
a joiner that is itself a `SELF_REDIRECT` card opens a nested window on its
own link. `MAX_COUNTERACT_DEPTH` bounds both.

**Fallback.** None engine-side: the opener's body decides from
`joiner_count()`. `mana_champion`'s body becomes

```rust
ctx::join::open()?;
for i in 0..ctx::join::joiner_count() {
    let j = ctx::join::joiner(i);
    if ctx::join::joiner_shielded(i) && ctx::ask_yes(j, …)? {
        ctx::draw(j, 1)?;
    }
}
if ctx::join::joiner_count() == 0 { ctx::draw(player_id, 1)?; }
```

**Example trace.** P1 plays a two-target effect on P0, who answers 真奈; the
window's other seats are offered `net_error` as if each were the target; P3
declares, cancels P0's designation through the redirect and shields; the
round closes; P0's body asks P3 whether to draw. Re-run `cross_chain::c22` /
`c23` and `gap_window::g19` / `g20`: they pin the declaration-scoped half and
its window refusal, and must land on the same verdicts through the new window
(g19's multi-target effect still offers P3; c22's no-joiner case still bars
the draw).

### 6.5 History (item 8)

At declaration, before the hand removal and before any body:

```rust
let mut w = cx.world_copy();
w.countered_this_turn.push(CounteractRecord {
    declarer: cursor as i32,
    countered: chain[timing].seat as i32,
    card: id.clone(),
});
cx.swap_world(w);
```

`countered_by_this_turn(seat)` filters the vector, dedupes by declarer and
keeps declaration order. `turn_start` clears it, so 「当前回合内」 is the
default scope and needs no key comparison.

`cant_look_away` needs no entry field: `run` takes
`let targets = ctx::countered_by_this_turn(player_id);` and returns only when
that is empty — true for the play entry on a fresh turn and false for the
counter entry (the immediate counterer is already in the list). The
`target == owner && actor != owner` condition stays for the counter entry.

### 6.6 Extreme lifetime (item 4)

`draw_event` (`play.rs:5833`) gets the same scope pair as `play_from_hand`
(`play.rs:5685-5692`, `:5797-5799` for the model): save `prev_extreme`, set
`Extreme::Plain` before the `event` raise (`:5853`), restore when the body
returns, and again before the `eventAfter` raise (`:5896`), so the After hooks
read a plain world and nothing leaks into the next play. `tsugumi_can` declares
`ChainKind::Event` and `answers_numbers(NumberInterval)`; the event card that
rolls an interval carries `RANGE_CARD` where one exists.

### 6.7 Refusal before the window (items 13, 14)

In `abnormal_gate` (`wasm_rules.rs:6204-6320`) the order becomes:

1. out / status-host gate refusal (G12 §5.1) — on any refusal raise the
   hook-only `abnormalRefused` (new kind, `value` = the `AbKind`) and answer
   `false`;
2. `untargetable` (unchanged, `:6248-6264`);
3. `immuneAll` (today `:6289-6291`, via `immune` at `:6312`) — on refusal
   the same `abnormalRefused`, answer `false`;
4. the `effect` window (the current raise at `:6273`);
5. `target` / the landing and the `abnormal` settlement hook (unchanged).

`abnormalRefused` is a hook-only `EngineKind` (id 121) and a hook-only ruleset
kind, so no window opens on it and `ruleset.declares` may be false; it gets
the same `KindMask`/`hook_only` rows as `abnormalGuard`. The `abnormalGuard` raise stays where it is
(before all of it, `:6257-6262`); G12 §5.1's gate refusal moves ahead of it
only where G12 lands it.

### 6.8 Kani model

`sys_kani` / `kani_host::Op` gains one op per import (§9's band 411-425);
`native_shims` mirrors them with the `ruleset_*` names and `host.rs` registers
them next to `join_redirect_target` (`host.rs:5899-5905`). The stubs are
shape-consistent only: `join::open` = a no-op answering `1` with `opened`
set, `joiner_count` = 0, `pending_*` = 0, `countered_by_this_turn` = empty.
The pure contract/report functions are modelled in `spec/g06_windows.rs` and
proved in `proofs/g06_windows.rs` (§9.4).

### 6.9 Card usage

The mechanism is generic; the cards still have to say what they mean. Every
card below is named by its `cards.json` id. A card that only *reads* counts
as usage too, because the guards are the point of the surface.

**`AG:（鸫）微小的『能做到』的事`** (`tsugumi_can.rs`) — the entry becomes

```rust
On::Counteract(&[ChainKind::Card, ChainKind::Event], …)
    .answers_numbers(NumericShape::NumberInterval)
    .arms_extreme()
```

`can_counteract` (`:39-46`) drops the blanket `true` and the comment-only
ycm exception; it keeps only the extreme-armed lock, now
`trigger::extreme_armed()`. The body's
`set_extreme` stays and picks from the answered face. The event branch of the
body needs the answered link's roll, which the carry-forward provides.

**`AG:Y.O.L.O`** and **`通用:@Tsugu ycm`** — add `.props(&[(prop::RANGE_CARD,
1)])`. ycm's result is `ctx::roll(player_id, 3, 10, RollPurpose::Effect,
roll_source::CARD)`; YOLO's is additive (`let before = trigger::move_roll()…;
trigger::set_move_roll(before + n)`). The negative example,
`AG:无论是何种颜色的夕阳`, is the one `RANGE_CARD` card whose roll is a
table, and it gains nothing.

**`CRYCHIC:主唱太拼命了`** (`vocal_too_hard.rs`) — the entry takes
`.answers_whole_payments()` and its condition drops `pay_total_stage` (`:28`);
once G02 §2.20 raises the window on the command total, the card sees it there.
No body change.

**`Mujica:（睦/mortis）表演的本能`** (`mortis_instinct.rs`) — the empty list
stays (now meaning every kind) and the entry gains `.copies_previous()`;
`can_counteract` (`:35-38`) goes from `false` to delegating to the copied
card's entry (the matcher resolves it), and `run` (`:45-56`) goes away in
favour of the copied body, `trigger::link_copied()` naming the source (§2.15).

**`RAS:Repaint`** (`repaint.rs`) — condition unchanged (`:15-21`); the body
loses nothing, but `on_path` (`:36-46`) must count the *declaration*
face, which the carry-forward's frozen clone gives it. The
factor uses G04's `ctx::plan::multiply_pay_factor(500, PayFactorScope::ThisMove)`
(`:84`), which composes, so two repaints stack through the plan frame.

**`Mujica:无法将视线移开`** (`cant_look_away.rs`) — `run` (`:53`) takes
`ctx::countered_by_this_turn(player_id)` as its target set and returns when
it is empty (`:65`, `:73-75`); the `target == owner && actor != owner`
condition stays for the counter entry, and the self-move clause reads
`ctx::player_id()` from the same list. The `On::Play` entry stops being dead.

**`Sumimi:（真奈）歌唱大赛5连冠`** (`mana_champion.rs`) — the body becomes
the `open` / report / fallback block of §6.4; `designation_cancelled`
(`:89-93`) and the unconditional draw (`:98`) are replaced by the report and
the fallback.

**`Sumimi:现在她是Sumimi的小初啦`** (`now_sumimi.rs`) — the guard reads
`trigger::pending_negation() == 0` (`:23-29`) instead of the bare `value`
comparison alone.

**`通用:雨啊，快点来吧`** (`rain.rs`) — declares `.props(&[(prop::DESIGNATES,
1)])` (`:15`). The mandatory user already goes through `ctx::target` (`:35`)
and a refused pick already `continue`s (`:56-58`), so the property is the
whole card change.

**`Sumimi:网络链接异常`** (`net_error.rs`) — the join branches collapse to

```rust
let opener = ctx::join::opener().unwrap_or(player_id);
ctx::cancel_designation(opener);
ctx::join::shield();
```

and the counteraction match's `Effect` arm (`:142-150`), which is the link a
join answers, becomes reachable through the redirect. The draw moves out (`:115-121`) to the opener (§6.4). The
`can_counteract` (`:44-54`) reaches the join through the answered link's
join block instead of the `join_redirect_target()` probe (`:106-114`).

**`MyGO:普通与理所当然`** (`ordinary.rs`) / **`R:选择自己的舞台`**
(`own_stage.rs`) — no card change beyond the ordering (§6.7); `own_stage`'s
`can_counteract` (`:44-48`) already reads the landed effect list, which is
now guaranteed to be one.

**`Mujica:（睦/mortis）`'s copy target** — every Counteract entry that can be
copied benefits from the contract fields without change (the matcher asks the
source card's entries with the source's own contract).

## 7. Save, record and replay

* **SAVE_VERSION 14 → 15** (the next free value). `World.countered_this_turn`
  is durable turn state. The field is `#[serde(default)]`, so a v14 save
  loads with an empty history; replays ride archived bundles anyway.
* **RECORD_VERSION stays 4.** No input-stream shape changes; the record's
  `EngineStamp.save_version` moves with `SAVE_VERSION`.
* **Join state is not saved.** It lives on the in-flight chain
  (`CounteractResume` is `Box<dyn Any>` on the PlayCtx, like the rest of the
  chain, STACK-01). A save taken mid-chain already loses the chain. A suspend
  writes `on_link` back before parking (`:7230`), so the in-memory resume
  keeps `join`; `bridge_trigger` copies it.
* `join_redirect` / `join_used` stay on `Trigger`, so a module built before
  the bump keeps working against its own engine.

## 8. Versioning and band arithmetic

`ABI_VERSION` 111 → **112**, entry inserted immediately below the v111
entry (`abi.rs:803-815`):

```
/// v112: **counteraction answer contracts, join windows, counteraction
///    history, payload carry-forward**. `CounteractEntry` gains `numeric` /
///    `pay_shape` / `effect` with `On::answers_numbers` /
///    `.answers_whole_payments` / `.declares_negation` / `.arms_extreme`
///    sugar, and `ManifestOn` the trailing `numeric` / `pay_shape` /
///    `answer_effect` / `all_kinds` / `copies_previous` fields -- postcard is not
///    self-describing, so **that** is the wire break. An empty `links` list
///    on a Counteract entry now means every chain kind (`all_kinds`) instead
///    of never, and `copies_previous` dispatches the copied card's entry
///    (mortis). New `prop::RANGE_CARD`; new imports
///    `ctx::join::{opener,seats,shield,open,opened,joiner_count,joiner,
///    joiner_shielded,designation}`,
///    `trigger::{pending_negation,extreme_armed,subject_card,link_copied,
///    chain_depth,answered_kind}` and `ctx::countered_by_this_turn`; new
///    window variables `subject_range_card` / `extreme_armed` /
///    `pending_negation` / `chain_depth` (`pay_total_stage` is G02's).
///    Counter links carry the answered link's payload forward and a face
///    write propagates to the enclosing roll link. `set_join_used` stays as
///    an alias of `ctx::join::shield`. New hook-only engine kind 121
///    (`abnormalRefused`, raised on an abnormal the gate or `immuneAll`
///    refused; no window opens on it). SAVE_VERSION 14 -> 15
///    (`World.countered_this_turn`); RECORD_VERSION unchanged.
```

Band arithmetic from the README: G06 lands `ABI_VERSION` 111→112, engine
kind 121, Kani `Op` 411-425 and `SAVE_VERSION` 14→15; the README's G06 row said 109→110/14→15
(stale by two ABI steps) and every later row's ABI number shifts two:
G05+G13 112→113, G07 113→114, G09 114→115, G10 115→116, G11 116→117,
G12 117→118, G14 118→119. The op bands need no shift (G03's 398-410 landed).

## 9. Tests and Kani obligations

### 9.1 Per item (`rules/tests/tests/g06_windows.rs`)

| item | test | assertion |
|---|---|---|
| 1 | `join_draw_belongs_to_the_opener` | joiner shields under 真奈; the **opener** is asked and draws 1, the joiner draws 0 |
| 1 | `join_draw_is_optional_per_joiner` | opener declines the ask → no draw, money unchanged |
| 2 | `join_shield_reads_back_on_the_opener_link` | `joiner_shielded(0)` is true after a landing joiner; `set_join_used` alias holds |
| 3 | `tsugumi_not_offered_on_sunset` | 夕阳 play → no offer |
| 3 | `tsugumi_offered_on_yolo_and_forces_the_extreme` | YOLO play → offered, `1d4` settles at 4 / 1 |
| 3 | `tsugumi_not_offered_twice` | two copies, one play → the second is not offered |
| 3 | `tsugumi_offered_on_ycm` | ycm play → offered (the exception is a target) |
| 4 | `tsugumi_answers_a_numeric_event` | numeric event armed at max, and the next play rolls plain |
| 5 | `vocal_not_offered_on_a_split_leg` | 3-way split of 6000 → no offer on any leg |
| 5 | `vocal_offered_on_a_whole_transfer` | a single 5000 transfer → offered, 0 paid |
| 6 | `mortis_empty_links_match_every_kind` | the matcher dispatches the entry at `card`, `settle`, `pay` … |
| 7 | `repaint_cuts_a_roll` | repaint on the root roll → face reduced by the owner's path tiles, settlement halved |
| 7 | `repaint_answers_yolo_and_the_cut_composes` | repaint answers YOLO's counter → final face is cut **then** boosted, X from the original path |
| 8 | `cant_look_away_moves_every_counterer_this_turn` | two counterers this turn → both walk |
| 8 | `counter_history_is_empty_next_turn` | next turn the play entry moves nobody |
| 9 | `join_window_offers_a_single_target_effect` | a one-target effect: bystanders are offered |
| 9 | `join_offers_no_seat_twice` | seats are a set; the opener is not offered |
| 9 | `join_restores_the_effect_target` | after the round the effect still settles on the original recipient |
| 10 | `fallback_draw_bars_on_a_missed_joiner` | a joiner who shields nothing still bars the draw |
| 11 | `now_sumimi_not_offered_after_a_declared_cancel` | net_error cancels the link first → no offer |
| 12 | `rain_designations_reach_net_error` | rain's designations are visible: net_error offers clause 1 and cancels one |
| 12 | `join_designation_is_readable` | `join::designation(0)` names the cancelled seat |
| 13 | `ordinary_not_offered_to_an_immune_player` | ImmuneAll holder + abnormal → no offer, `abnormalRefused` logged |
| 14 | `own_stage_answers_only_landed_abnormals` | a refused abnormal spends nothing |
| 15 | `mortis_plays_the_copied_entry` | a copied counter's entry runs, `link_copied` names the source |
| 8 | `counter_history_survives_save` | save/load round-trip keeps the list |

Suites that must stay green: `cross_chain::c22` / `c23`, `gap_window::g19` /
`g20`, `cross_chain::c20`, `spec_card_ras::repaint_contract`, `ag`'s
`tiny_can_*`, `general`'s net_error tests, every `*_kani` `ppcost` export.

### 9.2 Offer-matrix test

`chain_eligibility.rs` gains a table-driven `contract_admits_matrix`: (kind ×
subject property × pay shape × extreme × designates) against a synthetic
entry, asserting offer/refusal exactly — the direct unit of the invariant,
including the designates fallback under both shapes.

### 9.3 Static sweep

A `ruleset-tests` test (not a compiler check; the body is erased) walks every
`On::Counteract` entry whose body text calls `trigger::set_cancelled` /
`negate_effect` / `spare` and asserts `.declares_negation()`; and every entry
with a numeric body (`set_extreme`) asserts `.arms_extreme()`. The source is
read from `rules/cards/*/src/*.rs` text, keyed by card id.

### 9.4 Kani obligations

`rules/kani/src/spec/g06_windows.rs` models the pure functions;
`rules/kani/src/proofs/g06_windows.rs` proves:

* `contract_admits_iff` — admits is true iff the kind matches (direct or
  designates), numeric shape holds and pay shape holds. One mutant per
  predicate must fail.
* `join_report_iff` — `joiner_count` = declared count, `joiner(i)` = i-th
  declarer, `joiner_shielded(i)` = used[i], `designation(i)` = designated[i];
  a mutant that folds `declared` into `used` fails.
* `history_append_iff` — `countered_by_this_turn(s)` is exactly the declarers
  of records whose `countered == s`, deduped, order kept; a mutant that keeps
  other seats' records fails.
* `pending_negation_iff` — the count is exactly the declared answers marked
  `NEGATES`; a mutant counting all answers fails.
* `empty_links_total` — an empty-links entry admits every non-hook-only kind
  (the `all_kinds` model).
* `face_compose` — for two answers on one roll link, the propagated result is
  `outer(inner(face))` in settle order.

Card bodies stay PROTOCOL-LEVEL (window/chain), as in `spec/kani/*.md`; the
obligations sit on the SDK-side pure functions.

## 10. Alternatives

**A1 — keep the link stamp, add only the getter (no window).** `join_used`
becomes readable, the joiner keeps drawing itself, the opener reads the flag
for the fallback. Smallest change; rejected because "who joined" and "who
shielded" collapse into one boolean, a single-target effect still offers no
bystander, and every joining card must re-implement the draw. Kept as the
compatibility alias inside the chosen design.

**A2 — let the draw stay in the joiner's body and give the opener only a
count (the draft's implicit shape).** One new reader, no report.
Rejected for the same reason plus: the opener's card is what owns the
clause, so a joiner's card would have to know about 真奈.

**A3 — a dynamic counteract mask (`fn() -> &'static [ChainKind]`) for
Mortis.** Closures cannot see the copied card at ruleset-build time; a
runtime call per offer costs more than a bitmask shift. Rejected in favour of
empty = all, one field in the manifest and one loop in the build.

**A4 — express the range and pay contracts only as condition variables.**
Simplest surface, but every offer then pays a world copy, a CEL scope and an
evaluation to discover the card is ineligible. Rejected; the entry fields
stay the cheap index and the variables the body escape hatch.

**A5 — counteraction history as a global log with turn keys, no per-turn
state.** Saves one vector clear per turn but grows without bound within a
game and needs a turn filter on every read. Rejected; `World.gains` is
already the turn's scratch for the same shape.

**A6 — make `counteracted` the kind of a counter link and deprecate
`card`.** Cleaner naming, but `cant_look_away` / `dice_cast` already declare
`Card` for counter links; those entries would silently close. Deferred (§11).

**A7 — open the join immediately at declaration (from `build_round`)
instead of from the opener's body.** The opener could then not read the
report or decide per joiner, and the round would settle before the opener's
own body, reversing LIFO. Rejected.

**A8 — run the join round in a fresh nested `counteract` on the answered
link.** A second window would re-offer the opener and the whole outer ring,
not the joiners. Rejected.

**A9 — infer "empty links = every kind" from `triggers.is_empty()` and drop
`all_kinds`.** Works until a second entry kind wants the same convention; the
explicit flag also keeps `entry_iter` generic. Rejected for one wire byte.

## 11. Open questions

1. **「一次性」 scope (5).** Is `total_stage == false` the whole test, or must
   the leg also belong to a `ctx::split_pay` (a bare transfer leg outside a
   split would then still qualify)? Design assumes "any leg".
2. **Negated joiner.** Does a joiner whose counter was negated still bar the
   fallback draw (「打出过」 says yes) and still draw (it shielded nothing)?
   Design says bars-but-does-not-draw.
3. **Shield with nothing to shield.** When every designation on the play is
   already cancelled, does the opener still draw for a joiner who calls
   `shield`? Design says yes (the mark is the joiner's own claim).
4. **`@Tsugu ycm` exception.** Does 「除@Tsugu ycm以外无法直接改变骰子点数」
   make ycm a legal target (design assumption, via `RANGE_CARD`), or an
   excluded one? OPEN-@Tsugu / C-AG-tiny-can-02.
5. **Is "pure number interval" a card property or a per-effect flag?** A card
   can have one interval effect and one table effect (ycm has only the
   interval; 夕阳 only the table). A per-entry flag would be exact and is not
   designed here.
6. **`counteracted` as the real counter-link kind (A6)** — a compatibility
   sweep now or later?
7. **`Counteracted` as a third alias.** `abi.rs:2105-2106` documents the kind as
   "not raised by the current engine", yet `cant_look_away` declares it.
   Raise it beside `card` / `cardUsed`, or leave it reserved?
8. **Join depth cap.** A nested join is allowed and bounded by
   `MAX_COUNTERACT_DEPTH`; whether the book intends a joiner to open a join
   of its own is unruled.

## 12. Necessity

This section answers one question: why does each piece of §5 exist — and why
could it not be done smaller? It is written item by item so a reviewer can
strike an item without striking the group, and surface by surface so a
reviewer can strike a field without losing sight of what it holds up.

### 12.1 The group

Counteraction is the mechanism the whole card pool threads through: 15 of the
65 B-class items, 21 sheet rows, the 真奈 join, the extreme forcer, the
repaint cut, the counterer history, the negation guards and the abnormal
window. Every one of them is a ruled clause that the current engine either
answers wrongly or cannot express. The question is not whether the chain
needs work — the sheet already says it does — but whether the work needs
*this* shape. Four facts force it:

1. The offer index can only ask "does the kind match" (§3 R1). A contract
   that is not in the entry cannot be enforced, and the two over-offer items
   (3, 5) are exactly that missing predicate. An A-path fix would put the
   predicate into each card's `can_counteract`, where it costs a world copy,
   a condition evaluation and a duplicated rule per card — and it still could
   not see the subject or the payment shape through a counter link, because a
   counter link is a stub (R2).
2. A counter link drops the answered payload (R2). Repaint cannot read the
   roll it cuts, and any future entry answering a counter is blind. Fixing
   that is the carry-forward; once the payload is there, the settle-order
   loss (R5/R11) has to be answered too, which is the face propagation.
3. The join is two scalars with no per-seat grain (R3). The book's
   「如同自身的对应目标被指定一般」 is a *presentation* promise: the seat must
   be offered as the target. A scalar cannot present anything, cannot report
   who shielded, and is armed for holders who never played the card. A
   getter alone (alternative A1) makes the flag readable but leaves the
   single-target case unoffered.
4. Guards run before resolution (R4). The negation guard and the extreme lock
   need facts that only exist after a declaration, and the abnormal window
   opens before the refusal exists. No reordering inside a card can supply a
   fact the engine has not recorded.

### 12.2 Per item

Each line states what breaks for a player today and what the smallest
alternative would be.

* **1 (draw ownership).** Today only `网络链接异常` can draw for a joiner, and
  真奈 cannot run its own clause. A1 keeps the draw in the joiner's body, so
  every future joining card must re-implement 真奈's clause. Necessity: the
  clause belongs to the opener's card; without a report the opener cannot
  ask it.
* **2 (shield reader).** `set_join_used` writes to nothing. A1 gives a bare
  boolean, which cannot distinguish "who joined" from "who shielded" and
  cannot be indexed per seat. Necessity: one reader per seat, which is
  `join::shield` plus `joiner_shielded`.
* **3 (extreme filter).** The card is offered on every play, arms twice, and
  its own documented exception (ycm) is unreachable (`tsugumi_can.rs:39-46`).
  Necessity: a shape predicate on the entry, and a pending-extreme reader so
  a second copy does not re-pick.
* **4 (event effects).** An event-armed extreme leaks into the next play and
  events are never offered. `ChainKind::Event` and the saved extreme already
  exist, so this lands as soon as item 3's filter exists; without the filter
  the event branch over-offers exactly as the card branch does.
* **5 (split leg).** The card's own TODO (`vocal_too_hard.rs:22-27`) records
  that it is dead for the payments 「一次性」 is meant to keep. Necessity: the
  contract field, plus G02 §2.20's total window as the prerequisite.
* **6 (empty links).** The entry is never offered (`mortis_instinct.rs:21`).
  No runtime closure can decide kinds at build time (A3). Necessity: the
  manifest flag and the build-side expansion.
* **7 (repaint).** The card answers only the root and its cut is lost between
  counters (§2.7). Necessity: the carry-forward is what makes the condition
  true at all; the propagation is what makes the cut survive LIFO.
* **8 (history).** The `On::Play` entry moves nobody and the counter entry
  moves one seat (`cant_look_away.rs:55-70`). Necessity: the history is the
  only source of "all players this turn"; dedupe and clear-at-turn-end are
  part of it, not extras.
* **9 (join window).** Single-target effects offer no bystander
  (`net_error.rs:60-65`). Necessity: the presentation, the seat set and the
  round; the existing declaration stamp cannot be stretched to either.
* **10 (joiner count).** The fallback is tied to "a designation was
  cancelled", which a shielding-nothing joiner does not produce
  (`mana_champion.rs:89-98`). Necessity: `joiner_count` over `declared`.
* **11 (pending negation).** `现在她是Sumimi的小初啦` gains on a payment a
  counter will negate. Necessity: a declared-intent reader before resolution;
  the entry bit is its only honest source.
* **12 (designation property).** Without `DESIGNATES` every designation
  reader sees nothing, and `网络链接异常` takes the whole-play branch
  (`net_error.rs:50-53`). Necessity: the property (A) — the two body bugs the
  draft also listed are already fixed on master, so the A part is now one
  line. The per-joiner record is the B part, needed because the cancel
  happens inside the joiner's body.
* **13, 14 (abnormal ordering).** A protected player spends the card on a
  move that never happens. Necessity: the refused/landed split and the
  hook-only `abnormalRefused`; G12 §5.1 owns the gate itself.
* **15 (mortis).** The card logs and does nothing (`mortis_instinct.rs:45-56`).
  Necessity: the copied-entry dispatch and the `link.copied` fact; without
  them item 6 only makes an inert card *offered*.

### 12.3 Surface, field by field

| surface | forced by | if removed |
|---|---|---|
| `NumericShape` / `prop::RANGE_CARD` | 3, 4, 15 | tsugumi over-offers; the copied entry cannot be matched |
| `AnswerPayShape` | 5 | split legs are waived one by one |
| `AnswerEffect` bits | 3, 11 | the second extreme copy re-picks; the negation guard has no source |
| `ManifestOn.all_kinds` / `copies_previous` | 6, 15 | inert entries stay unoffered; the copy cannot dispatch |
| `Trigger.subject_card` | 3, 4, 15 | the filter has no subject through a counter link |
| `Trigger.join` / `TriggerJoin` | 1, 2, 9, 10, 12 | the join is a scalar again (§12.1 point 3) |
| `Trigger.answers_declared`, link `copied` | 3, 11, 15 | no pending fact, no copy |
| `World.countered_this_turn` | 8 | no per-turn set; save would lose it |
| engine kind 121 | 13, 14 | a refused abnormal is unloggable as such |
| the fifteen imports | 1, 2, 9, 10, 12, 15, 3, 11, 8, 7 | each is a datum no other import exposes |

Nothing in §5 is speculative: every field is read by at least one item, and
every item's test in §9.1 reads the field it exists for.

### 12.4 The breaks are the price, and it is small

* **ABI 112.** Five trailing `ManifestOn` fields must exist because postcard
  is not self-describing. The imports and conditions alone would not need a
  bump; the manifest does. One bump now, defaulted decode, no migration.
* **SAVE 15.** The history is the only durable fact. It is
  `#[serde(default)]`, so old saves load empty, and it clears at
  `turn_start`.
* **No new display text, no decision in an import.** Every joiner decision
  is still `ask_yes` in the opener's body.
* **Landing independence.** Items 6, 12 (A part) and 4's kind can land ahead
  of the rest (the A sub-branches in §1); item 5's split-total behaviour
  waits on G02 §2.20; item 9's nesting waits on G13's copy rule. Nothing in
  the group needs a later group to compile.

### 12.5 What would argue against the group

The honest counter-arguments, and their answers:

* *"A1 is enough."* It cannot present a seat as the target, so single-target
  effects stay unjoinable; it cannot distinguish shield from declaration, so
  the fallback and the draw both stay wrong.
* *"Put the filters in `can_counteract`."* They are still blind to the
  subject and the payment shape through a stub link, and they re-evaluate a
  static property on every offer.
* *"Never mind the join."* Then items 1, 2, 9, 10 and half of 12 close
  ungranted, and `cross_chain::c23` / `gap_window::g19` keep passing only
  through multi-target luck.
* *"Defer the copy (15)."* Legitimate: 15 is the only item whose memory rule
  is undecided (CH-05). Striking it leaves 6 as the A matcher and costs the
  `copies_previous` field and the `link_copied` reader, nothing else.
