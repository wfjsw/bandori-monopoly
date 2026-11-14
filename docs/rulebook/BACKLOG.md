# Rulebook / ABI backlog

Standing backlog for continuous iteration on rulebook divergences,
rule-implementation bugs and ABI problems. Fix agents take independent batches
from here on their own worktree task branch. This file lists **open work
only**; closed ids that source comments still cite keep a one-line stub in
[Resolved ids cited from source](#resolved-ids-cited-from-source), and older
history lives in the commit log.

History in one paragraph: opened 2026-10-10 as `rulebook-triage`; full recount
2026-10-17; card-rule TODO review 2026-10-20; the 2026-10-27 card sheet review
(1733 verified findings over 180 of 183 cards, triaged 2026-10-28 into 776 A
items fixed on `worktree-card-rulebook-review`, 65 B items under 15 ABI groups,
50 C items and the numbered questions below) was followed by the 2026-11-03,
2026-11-06, 2026-11-08 and 2026-11-13 close-out sweeps; the last one closed
every class-A and class-B item (22 ids) and removed them from this list; this
file was flattened and re-verified against master `77d56311` on 2026-11-13,
and the A/B sweep landed as ABI v124.

Sources (re-counted 2026-11-13):
- **268** `TODO(规则书)` tags in `rules/` + `crates/`. Some are convention
  markers (a module doc naming the policy, not a gap); the convention-marker
  list is at the end.
- **8** live `#[ignore = "RULING: …"]` rulebook tests in
  `rules/tests/tests/*.rs` (C-Q13, C-Q38/EVENT-01, C-Q44, C-Q2/MONEY-02,
  C-Q23/MOVE-13, C-Q24/MOVE-14, C-Q18/MONEY-07, C-Q45), the one G04 plan-stage
  ignore (`g03_rolls.rs`), and 2 meta `q4_unintended` tooling ignores. Zero live
  `DISCREPANCY:` (doc-comment mentions of the convention do not count). No
  `row N` TODO(ABI) ignore is left. `crates/game-rules/tests/` is clean.
- `docs/rulebook/TEST-FINDINGS.md` (§3 ABI / §4 per-card / §6 rulings), `COVERAGE.md`, `CROSS-TESTS.md`, `NEGATION-AUDIT.md`, `PIPELINE-AUDIT.md`, `SETTLE-STAGES.md`, `docs/GUARDS.md`, `docs/MARKERS.md`, `docs/STATUS-HOSTS.md`, `docs/rulebook/CARD-REVIEW-2026-10-27.md`, and the ambiguity backlog `docs/rulebook/spec/OPEN-QUESTIONS.md` (OPEN-* ids ↔ the C-Q list below)
- the 2026-10-10 ABI audit of `crates/game-rules/src/{host,hostfns,native_shims,wasm_rules}.rs`, `crates/card-sdk/src/{ctx,sys,abi}.rs`, `crates/game-core` purchase/play

**Current ABI: v124** (`crates/card-sdk/src/abi.rs` `ABI_VERSION`). The body
still quotes older era names in places; where a kind has since been renamed
(`HookKind`/`GateKind` → `ActionKind`/`FilterKind`; `MoveRoll` → `DiceRollResult`;
the `Pay*` / `Roll*` families restructured; `ruleset-core` → `bandori-std`;
`CpChanged`/`CrystalsChanged` → `CounterChanged`), treat the live name as
canonical. The ABI has landed every group design in `docs/rulebook/spec/abi/`
through G15 — no group design is left; `ctx::roll` is the windowed default and
`ctx::roll_direct` the windowless variant, both returning `Result<i32, Prompt>`.

**Crate renames** — every path below is quoted in the new names:
- `rules/skills/skill-characters/` → `rules/skills/skill-personas/`
- `rules/skills/skill-bands/` → `rules/skills/skill-groups/`
- `rules/ruleset-core/` → `rules/bandori-std/`

Crates: `rules/bandori-std`, `rules/status`, `rules/personas`, `rules/markers`,
`crates/cond-ir`, `rules/spec`, `rules/kani`, plus the per-band card crates
under `rules/cards/` and the skill crates under `rules/skills/`.

Ignored-test run: `node tools/build-ruleset.mjs` (no `CARGO_TARGET_DIR`) then
`cargo test -p ruleset-tests -- --ignored` and
`cargo test -p game-rules --no-fail-fast -- --ignored` (run twice).

**Class**
- **A** — fixable now from unambiguous rulebook text + current ABI
- **B** — needs a new/changed ABI capability (named in our vocabulary)
- **C** — rulebook text ambiguous; user ruling needed

**Severity**: high (common play) | medium | low (rare edge)

Every A and B item is closed (2026-11-13 sweep, ABI v124); only C rulings are
left, so the list below is flat and sorted by severity. One item = one root
cause; every file:line / test is listed. C items change no code until ruled;
their sites carry a `TODO(规则书)` pointing at the C-Q id, and the verbatim
questions are in the appendix.

---

## MONEY-02 — 「支付减半」 vs cancel-one / 分摊 recompute order
- **kind**: rulebook divergence
- **class**: C
- **severity**: high
- **suggested batch**: payment pipeline
- **locations**
  - `rules/tests/tests/gap_money.rs:253-255` `g04_half_share_then_cancel_one_payer` `#[ignore = "RULING: halving vs cancel-one order on a 分摊 leg (and whether the share recomputes)"]`
  - related: `rules/events/src/front_or_back.rs:45-47,57-65` (「分摊」 remainder) — separate small C, see MONEY-08
- **rulebook text**: 爱心义演 / 分摊 legs; 网络链接异常 cancel-one designation (sheet: 「取消一个指定」).
- **current**: the split divides the pre-drop shaped total (`pay_total` /
  `pay_total_stages`, `crates/game-core/src/engine/play.rs:5140-5160,5206-5230`);
  halving-vs-cancel order undecided.
- **expected**: decide whether the share recomputes after a designation drop, and whether halving applies before or after the split.
- **question**: C-Q2.

## MONEY-04 — 强制购买 / auction money sits outside the money pipeline
- **kind**: rulebook divergence
- **class**: C
- **severity**: high (force-buy is common)
- **suggested batch**: payment pipeline
- **locations**
  - `crates/game-core/src/engine/play.rs:3477-3480` (ruling 6 TODO on `offer_force_buy`: no `bought`/`buyAfter`), `:3580-3598` (force-buy commit is a direct transfer), `:5868,5880` (auction: money stays direct until ruling 7)
  - `docs/PURCHASE.md:208-213,232-236` rulings 6/7 undecided; TEST-FINDINGS §6 item 12
  - `rules/tiles/src/property.rs:17,70` (force-buy 「不受任何资金变动效果影响」)
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
  - `crates/game-core/src/engine/play.rs:1815` (ruling 8 TODO)
  - `rules/cards/card-ag/src/tomoe_savior.rs:70-73` (extra mortgage income raw vs `gain`, `price / 2` base)
  - `docs/PURCHASE.md:250` ruling 8; TEST-FINDINGS §6 item 13
  - tests (green, pin current): `card_paths::tomoe_savior_buy_listener_*`
- **rulebook**: 「立刻支付常规收购价一半的价格从该玩家处收购该地契」 — 「收购」 not 「购买」; base (land / land+houses / 2× force) unnamed.
- **current**: implemented as `price / 2` (land alone); hand-over runs `acquire` so every 「购买…时」 listener hears it; mortgage is cleared before the hook.
- **expected**: decide the price base, whether 收购 is a 「购买」, and whether the mortgage stays.
- **question**: C-Q4.

## W3-KAORU-03 — 「所有在薰所在格子的人」 excludes 薰 himself
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **rule text**: 「此卡在场上时所有在薰所在格子的人如果可以移动，则主要移动改为投掷1d2（前后）和1d10（距离）进行结算。」
- **code**: `rules/cards/card-hhw/src/kaoru_thief.rs:26-30` — the TODO ends
  "W3-KAORU-03 / C-Q48, OPEN"; the filter is
  `actor != owner && pos(actor) == owner.pos && …`.
- **gap**: 「所有」 reads as every piece sharing 薰's tile, including 薰.
- **question**: see C-Q48.

## SKILL-04 — 状态2 exit conditions missing (umiri; nyamu carved)
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: skills: Mujica
- **locations**
  - `rules/skills/skill-personas/src/umiri.rs:340-350` (「直到状态2结束为止」 with no clause that ends it; the band skill's fire-pot drain is the engine's stand-in exit)
  - `rules/skills/skill-personas/src/nyamu.rs:68-72,166-182` (no sheet exit; a `TurnEnd` hook now supplies the flip-back moment)
  - contrast: 三角初华 fire-pot cap, 若叶睦 hand-size exit (those are stated)
- **question**: C-Q11.

## TILE-06 — 江户川乐器店 / CiRCLE shared body
- **kind**: rulebook divergence
- **class**: C
- **severity**: low→medium
- **suggested batch**: tiles
- **locations**
  - `rules/tiles/src/edogawa.rs:9-11` (book names both in one sentence with different extra text)
  - `rules/tiles/src/circle.rs:30-32,75-76,191-192` (stunned card-only reward is a ruling `C44`/`D13`, not book; exile bar is engine bookkeeping)
- **question**: C-Q16 (only if the two shops ever diverge).

## MONEY-06 — 「支付」 reach of a non-settlement loss
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: payment pipeline
- **locations**
  - `crates/game-core/src/engine/play.rs:3432-3449` (TODO; `scale_settle_payment` only scales settle payments, `PayKind::Gain` returns unscaled at `:3445`)
- **question**: C-Q17.

## MONEY-07 — Popipapapipopa crystal spend vs owner rent
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: crystal/marker accounting
- **locations**
  - `rules/tests/tests/gap_res.rs:126-128` `g26_popipa_crystals_and_kasumi_fire_on_one_settle` (`#[ignore = "RULING: …"]`)
- **rulebook**: 「消耗或支付」 — owner's own outlay vs a rent the owner receives.
- **question**: C-Q18.

## MOVE-06 — E14 0-step case: [经过] vs [重叠]
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: movement
- **locations**
  - `crates/game-core/src/engine/play.rs:2219-2223` (implemented as E14 writes it: 重叠 only)
  - `rules/tests/tests/rulebook.rs:941,962-967` (green; pins 重叠 only)
- **rulebook**: `E14` names only [重叠] for the 0-step case; `B41`+`E13` read as both-fire for a teleport.
- **question**: C-Q20.

## MOVE-07 — teleport as the main move (settle shape)
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: movement
- **locations**
  - `rules/cards/card-mygo/src/no_road.rs:99,171`, `rules/status/src/exile.rs:171`, `rules/cards/card-ag/src/shop_friends.rs:71`
  - `rules/tests/tests/general.rs:91` `tsugu_lt26_choose_move_settles` (pins the immediate offer)
  - TEST-FINDINGS §6 item 1
- **question**: C-Q21.

## MOVE-08 — want_to_grab (2) 「经过你」 = passTile vs passPlayer (R5)
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: settle stages
- **locations**
  - `rules/cards/card-crychic/src/want_to_grab.rs:37-42` (R5; left as `PassPlayer`)
  - `docs/rulebook/SETTLE-STAGES.md` §9 R5 (`:477-478`, Pending)
- **question**: C-Q22.

## LINGER-10 — 「本回合」 linger in an extra turn / another player's turn
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: settle stages
- **locations**: `crates/game-core/src/engine/play.rs:843-844`
- **current**: linger effects last exactly one turn (ruling 10).

## MOVE-13 — 星月夜's re-roll scope (whole dice set vs base die)
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: movement
- **locations**
  - `rules/tests/tests/gap_move.rs:115-117` `g08_starlit_reroll_of_a_whole_3d20_set` (`#[ignore = "RULING: …"]`; assert full set)
- **question**: C-Q23.

## SETTLE-05 — no_breakup R3 timing (settleAfter vs tileResolved)
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: settle stages
- **locations**
  - `rules/cards/card-sumimi/src/no_breakup.rs:33,38` (the R3 TODO marker is gone; the implemented point is `Action::TileResolved`)
  - `docs/rulebook/SETTLE-STAGES.md:467-471` §9 R3 (still "Pending"; its "source carries an R3 TODO" sentence is stale)
- **question**: C-Q10.

## SETTLE-06 — two halvings on one settle (香澄 + 祥，移动)
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: payment pipeline
- **locations**
  - TEST-FINDINGS §6 item 4 (engine pins ¼)
  - `rules/tests/tests/mujica.rs:443,490-492` `saki_move_halves_a_forced_stop_and_pay` (green)
- **question**: C-Q28.

## SETTLE-07 — [晕眩] vs a card's own follow-on gain
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: payment pipeline
- **locations**
  - `rules/cards/card-mujica/src/heart_rain.rs:59` (`TODO(规则书)` on the stun-then-gain order)
  - `rules/tests/tests/mujica.rs:176,189` `heart_rain_fallback_stun_and_money`, `heart_rain_nobody_in_range_fallback` (green)
  - TEST-FINDINGS §6 item 9
- **question**: C-Q29.

## CH-08 — welcome_mujica (1) 「若指定了不存在状态2的玩家则无效果」
- **status**: open as a **ruling** only. The implementation reading is in place:
  the switch is a **no-op unless the target's `skillState` is 1 or 2**
  (`rules/cards/card-mujica/src/welcome_mujica.rs:71-73`) — a player who cannot
  reach 状态2 is not toggled.
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: skills: Mujica
- **locations**
  - `rules/cards/card-mujica/src/welcome_mujica.rs:68-73` (`TODO(规则书)`), the gate at `:71-73`
- **question**: C-Q30.

## CH-09 — dice_cast 「（此卡可以被反击）」
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: counteraction windows
- **locations**
  - `rules/cards/card-mujica/src/dice_cast.rs:29-33` (`TODO(规则书)[judgement]`)
- **related**: the turn-key `PLACED` latch also backs `cant_play` (`x_in_play`,
  `:59-64`), so a second copy in hand is refused in the same turn although the
  cell prints no once-per-turn cap; the `noCounteractTurn` latch is still the
  other seats' hand-counter block (`:98`). Probe: a second copy fails
  `Err("cards:card-mujica.x_in_play")`. The reverse-order counter play landed
  with G06; `cross_chain.rs:752` `c15_dice_cast_itself_countered` is green.
- **question**: C-Q31 (must other [反击]s still answer this card's own play window?).

## SKILL-10 — misaki 「额外视为CiRCLE」
- **kind**: rulebook divergence + ABI defect
- **class**: C then B
- **severity**: medium
- **suggested batch**: tiles / skills: HHW
- **locations**
  - `rules/skills/skill-personas/src/misaki.rs:111-120` (per-player tile-kind override; what 「额外视为CiRCLE」 pays is not stated; `tile_is` is static, only per-player colour exists)
- **question**: C-Q37.

## EVENT-01 — 前往哈比内尔王国旅游 teleport timing
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: events
- **locations**
  - `rules/tests/tests/events.rs:401-402` `habinel_teleports_to_smile_ship` `#[ignore = "RULING: …"]`
- **question**: C-Q38.

## LOY-STARRY-01 — 蝴蝶飞舞的星月夜 「你每次移动掷骰时可以放弃第一次的结果重骰一次」
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **locations**: `rules/cards/card-morfonica/src/starry_night.rs:27,49,167` — narrowed
  to the main move's free roll; forced-move rolls never see the rider. Text says 「每次」.

## MARKERS-LISA-01 — temp-pot 600-cost and two-turn expiry under-specified
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: skills: Roselia / fire kernel
- **locations**: `rules/bandori-std/src/fire.rs:460-464` (`temp` module),
  `rules/skills/skill-personas/src/lisa.rs:125-130`
- **rulebook**: （3）「你提供的临时火罐将会在两回合后失去，使用你提供的临时火罐
  需要支付你600资金」 — the spend path has to know a pot's *provenance* to charge
  the provider, and 「被冲榜类效果被动消耗无需支付」 carves out a class of
  spends the engine does not tag. The counter itself is real
  (`fire::temp`, spendable temp-first per MARKERS.md §10(2)); the 600-cost and
  the two-turn expiry stay open. In-source `TODO(规则书)[judgement]`.

## LOY-STORM-01 — 「普通[结算]」 of a mortgaged tile has no 收费 recipient
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **rule text**: sheet 2026-10-06 新卡组卡 H8 「…对那个玩家进行一次相当于此卡所在
  格子普通[结算]的(4-X)/4倍价格的**收费**」.
- **locations**: `rules/cards/card-ras/src/studio_storm.rs:134-136` charges a one-shot
  「收费」 (`ctx::charge_rent`); a mortgaged tile's 「普通[结算]」 is a forced-buy
  offer and `crates/game-core/src/engine/play.rs:3210-3230` returns 0, so there is
  no recipient and it charges nothing (`TODO(规则书)[ruling]`). Needs a ruling.

## DRAW-OUT-01 — 「受到抽卡效果」 includes self-caused out-of-turn draws?
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: draw/discard pipeline
- **locations**: `crates/game-rules/src/wasm_rules.rs:7171`
- **current**: today yes. `drawOut` / 「受到抽卡效果」 may mean only draws
  another effect forced.
- **question**: C-Q70.

## DETOUR-01 — 「"afterglow"式的移动」 under-specified; counter vs text
- **kind**: rulebook divergence
- **class**: C
- **severity**: medium
- **suggested batch**: events / AG cards
- **locations**: `rules/cards/card-ag/src/detour.rs:20-23,247-251`
- **rulebook**: 「初始Afterglow角色」 is never named; the counter logs 「羽泽鸫式」
  while implementing 美竹兰 reverse.

## MONEY-08 — 分摊 remainder (1000 % n)
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- **suggested batch**: payment pipeline
- **locations**
  - `rules/events/src/front_or_back.rs:57-66` (two TODOs; the split drops the remainder)
- **question**: C-Q19 (who gets the remainder).

## MOVE-02 — does a 「固定为X*6」 face still take later additions?
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- **locations**: `rules/cards/card-ag/src/yolo.rs:45-51` (`TODO(规则书)`, points
  here); `rules/tests/tests/cross_move.rs:539` `m11_rinko_plus_yolo` (green;
  pins the add-anyway reading). Contrast `starry_night` / `eve`, which refuse
  over a fixed face.

## MOVE-14 — 普通与理所当然 copy signed or unsigned 移动格数
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- **suggested batch**: movement
- **locations**
  - `rules/tests/tests/gap_move.rs:343-345` `g13_ordinary_copies_a_reversed_move_distance` (`#[ignore = "RULING: …"]`; assert unsigned)
- **question**: C-Q24.

## MOVE-15 — heat_head 「减少1d6」
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- **suggested batch**: movement / events
- **locations**
  - `rules/events/src/heat_head.rs:70-76` (subtract from face vs `add_extra_dice(-1, 6)`; sheet says 减少1d6 not 少投1d6)
- **question**: C-Q25.

## CH-10 — ran_as_usual (1) once-per-turn gate
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- **suggested batch**: counteraction windows
- **locations**
  - `rules/cards/card-ag/src/ran_as_usual.rs:20-28` (an unprinted once-per-turn gate; `slot('asUsualTurn') != turn_key`)
- **question**: C-Q32.

## CRYSTAL-05 — 育美标记 circle-tile case (not in the rulebook text)
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- **suggested batch**: crystal/marker accounting / HHW cards
- **locations**
  - `rules/cards/card-hhw/src/hagumi_marks.rs:210` (circle-tile behaviour not in the rulebook text; held)
- **question**: C-Q34.

## CRYSTAL-06 — 薰 charge/decay 「（充能3，衰减1）」 has no 0-clause
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- **suggested batch**: HHW cards
- **locations**
  - `rules/cards/card-hhw/src/kaoru_thief.rs:130-146` (keeps decay-out behaviour)
- **question**: C-Q35.

## EVENT-02 — asukayama settle-on-land vs settle-free walk
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- **suggested batch**: events
- **locations**
  - `rules/events/src/asukayama.rs:14-15` (derived 迷子的追逐), `:28-29` (settle-on-land default)
- **question**: C-Q39.

## EVENT-03 — event text gaps (one-shot / count / 卡组 / 下2回合 / 同色)
- **kind**: rulebook divergence
- **class**: C (several small)
- **severity**: low
- **suggested batch**: events
- **locations**
  - `rules/events/src/embers.rs:126,200` (spread one-shot vs repeating; copies removed but event stays)
  - `rules/events/src/fix_note.rs:45` (「累计…加盖后将此卡移除」 — 3 builds vs 1)
  - `rules/events/src/kizuna_music.rs:23` (「卡组」 = draw pile vs whole card pool)
  - `rules/events/src/lulu.rs:34,43` (under-500 teleport: payment prompt / force debt / fizzle; real-world shout)
  - `rules/events/src/surprise_duel.rs:42` (「下2回合」 window)
  - `rules/events/src/tears.rs:22` (「同色地契」)
  - `rules/events/src/circle_rebuild.rs:124-129,154-160` (priority vs placement order — B if we add priority; the tile-swap half closed as ABI-13)
- **questions**: C-Q40 (batch; the 弦卷地产 tie-break, old 40g, is closed — bids must
  clear the standing bid by ≥100, `crates/game-core/src/engine/mod.rs:2720`).

## LOW-03 — `Mujica:#J11` has no rulebook entry
- **kind**: rulebook gap
- **class**: C
- **severity**: low
- **locations**: `rules/cards/card-mujica/src/j11.rs:120-121`
- **question**: C-Q41.

## LOW-04 — umiri_card 「置入弃牌堆」 for a taken band-skill card
- **kind**: ABI/model gap
- **class**: C then B
- **severity**: low
- **locations**: `rules/cards/card-mujica/src/umiri_card.rs:151-162` — since G08
  the card is really discarded (`unplace_at(..., Dest::Graveyard)`), but a skill
  rule id is dropped by `file_row` (`crates/game-core/src/engine/ops.rs:2524-2558`),
  so it never reaches a pile and a `skill:` id is not drawable.
- **question**: C-Q42 (should a `skill:` id be drawable / filed?).

## LOW-05 — sheet text-drift / typo leftovers
- TEST-FINDINGS §8 (`:615-623`): 丸山彩 / 上原绯玛丽 / 羽泽鸫 which column is
  current; Pastel Palettes alternate band design is a draft; typos “PERFECT“
  (cards.json:174, great.rs:25) / “FEVER!“ (cards.json:177, perfect.rs:28) /
  星月夜 「支付X次1000的的资金」 (cards.json:54, starry_night.rs:41) /
  「流星堂的的[结算]」 (rules/tiles/src/event.rs:4) / 游击演出 stray `\[特]`
  (cards.json:152, guerrilla.rs:3).
- **kind**: rulebook divergence
- **class**: C (sheet ownership)
- **severity**: low

## STA-03 — `noHand` 2→1→0 dance is engine behaviour, not a quoted clause
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- **suggested batch**: skills: CiRCLE / sumimi
- **locations**: `rules/status/src/no_hand.rs:6-9`
- **rulebook**: 「手卡上限数量减1」 is card text; the 2→1→0 ladder is ours.
- **current**: the dance **moved** from the inline `play.rs` steps into
  `status:no_hand`'s `TurnStart` / `TurnEnd` hooks. The `TODO(规则书)` stays (the
  ladder is still our reading of card text, not a glossary clause); the host now
  owns it.

## AYA-POOL-01 — 「将Y个…变正」 pool-of-Y vs per-seat remaining-Y
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- **suggested batch**: skills: PP
- **locations**: `rules/skills/skill-personas/src/aya.rs:125-128`
- **current**: the slice. Per-seat remaining-Y would let overflow crystals fire.

## NYAMU-01 — flip-to-back spend of 2 fire pots has no stated benefit
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- **suggested batch**: skills: Mujica
- **locations**: `rules/cards/card-mujica/src/nyamu_card.rs:75` (no in-source id tag survives)
- **rulebook**: 「每当此卡将从正面翻至背面时可选择失去2个火罐」 — the clause
  does not say what losing the 2 fire pots buys (the text is silent).

## AYA-LONG-01 — the 最低0 floor's scope
- **kind**: rulebook gap
- **class**: C
- **severity**: low
- **locations**: `rules/cards/card-pp/src/aya_longing.rs:44-45`, `:130-140` (`TODO(规则书)` in the body)
- **rulebook**: 「（最低0）」 — a local `.max(0)` per clause vs a final floor after
  the whole add/sub stage is still unnamed by the book.
- **pinned by**: `money.rs` `aya_longing_1500_shave_lands_after_sibling_pay_split_add`.

## LISA-BOND-01 — 「使用技能进行传送后」 names no stage number
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- **suggested batch**: movement / skills: Roselia
- **locations**: `rules/cards/card-roselia/src/lisa_bond.rs:21-24`
- **rulebook**: closest is stage 13 「传送后在终点触发重叠」.
- **question**: C-Q85.

## NOBUY-CARD-01 — `no_buy` gates only `BuyKind::Land`
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- **suggested batch**: payment pipeline
- **locations**: `crates/game-core/src/engine/play.rs:3827`
- **current**: whether it also bars a card-driven (`BuyKind::Card`) buy is undecided (ruling 6).

## GUARD-PERVISIT — book states no per-visit counteract bound
- **kind**: rulebook gap
- **class**: C
- **severity**: low
- **suggested batch**: counteraction windows
- **locations**: `crates/game-rules/src/wasm_rules.rs:357` (`MAX_COUNTERACT_PER_VISIT = 16`)

## GUARD-SETTLE-DEPTH — book states no settle-depth bound
- **kind**: rulebook gap
- **class**: C
- **severity**: low
- **suggested batch**: engine: settle recursion / stack safety
- **locations**: `crates/game-core/src/engine/play.rs:47-48` (`MAX_SETTLE_DEPTH = 64`)
- **cross-link**: STACK-01 (the queue, not this rulebook bound, is the stack-safety mechanism).

## DONUT-VOID-01 — 一人两个甜甜圈 has no clause for a barred acquisition
- **kind**: rulebook gap
- **class**: C
- **severity**: low
- **suggested batch**: crystal/marker accounting
- **locations**: `rules/cards/card-sumimi/src/two_donuts.rs:60-67`
- **rulebook**: 「获得[除外]直至你原本所在格子被其他玩家经过」 +
  glossary 「该效果生效时无法重复获得该效果」.
- **current**: when the presser is already [除外], the no-stack guard bars the
  (1) acquisition and the whole effect is treated as void — no carrier, no
  `SLOT_ORIG` / `SLOT_BY`, no log. The card is still spent (the play itself
  resolved).
- **question**: C-Q87.

## LOY-C-PILE — four small C readings that need a ruling; do not guess
- **kind**: rulebook divergence (batched)
- **class**: C
- **severity**: low
- `AG:绯红之魂`（1）— sheet text 「因导致的[消耗]或[支付]」 is missing its
  antecedent (`cards.json:12`, `cards-sheet.csv:12`). The implementation takes
  the maximal reading (any owner consume/pay; `crimson_soul.rs:48-54`).
- `MyGO:[千早爱音]Anon Tokyo` — the linked-rent hook requires the payee be the
  card's owner (`anon_tokyo.rs:26,179`); the printed clause keys on the tile
  being linked. The closed LOY-ANON-01 kept `place_mark_pair` place-or-refuse
  on both link ends; whether a tile may sit in two links at all is C-Q89.
- `通用:该清CP了` — the "a copy is still in play" play gate is a 2026-10-07 user
  ruling attached to a `[特]` the sheet no longer carries (`clear_cp.rs:91-96`).
- `PP:练习生解密指南` build-off layers — `set_build_discount(1000*n, 1)` +
  peel-1 gives −3000 total at n=2; neither pure reading of the quoted status
  「下次盖房的价格减少1000（可溢出），盖房后减少1层」 (`trainee_guide.rs:205-220`).

## LOY-SEEYOU-02 — `PP:[衍生]明天见`: is `[共鸣]` a mandatory cost of the bypass or merely an ignore?
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- Pool reading is what the code intends (`see_you_tomorrow.rs:69-73`).

## LOY-MUSTBUY-01 — 「必须购买」 shortfall on `RAS:游击演出` + `Sumimi:Sweet Escape`
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- Soft-fail (log broke, skip; `guerrilla.rs:140-152`, `sweet_escape.rs:144-148`)
  vs glossary `[破产]`. Decide once.
- **question**: C-Q74.

## LOY-MUTSUMI-01 — `CRYCHIC:（睦）从没有觉得...`
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- (2)'s `set_dest(Banished)` is an unprinted `[移除]` (`mutsumi_never.rs:95-99`);
  only (3) prints 「此卡[移除]」.
- **question**: C-Q69.

## LOY-BLACK-02 — `HHW:黑衣人的补给`: the borrowed pass reward
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- The 「移动起点不为CiRCLE」 exemption does not re-bind (弦卷集团 is kind `agent`,
  so `is_start` never applies; `rules/tiles/src/circle.rs:88-94`). Two readings
  of 「获得"CiRCLE"格子的全部效果」.

## LOY-RANDOM-02 — `PPP:仓库里的Random Star`
- **kind**: rulebook divergence
- **class**: C
- **severity**: low
- (2) offer suppressed on landing step and teleports (`random_star.rs:18-24`:
  `move.remaining > 0 && move.kind != Teleport`).

---

# C-class questions (verbatim, for the user)

Answer or amend these; an entry leaves the list (and moves to the resolved-ids
stub section) once ruled. Questions whose sites changed are marked *updated*.

**C-Q2 (MONEY-02, 分摊 × halving × cancel-one).**
On a 分摊 leg: (1) if one designation is cancelled (网络链接异常 「取消一个指定」), does the per-payer share recompute from the remaining designations, or is it the pre-drop figure? (2) Does a 「支付减半」 apply before the split, to each share, or after? Options for (1): recompute / pre-drop. Options for (2): pre-split / per-share / post-split.

**C-Q3 (MONEY-04, 强制购买 & auction vs the money pipeline).**
Three related questions on 基础[结算] 6.2 / 其他 6 「不受任何资金变动效果影响」:
(3a) Does that clause also bar a card that 「写明改动强制购买」?
(3b) Is 强制购买 a 「购买」 for 「购买…时」 listeners (`bought` / `buyAfter`)?
(3c) Is an auction win a 「购买格子」 for discounts, and does the auction [消耗] run the pay pipeline?

**C-Q4 (MONEY-05, 巴 「常规收购价一半」).**
AG:（巴）商店街的救世主 「立刻支付常规收购价一半的价格从该玩家处收购该地契」:
(4a) What is 「常规收购价」 — land price only (the reading implemented), land + houses, or 2× force-buy total?
(4b) Is 「收购」 a 「购买」 for 「购买…时」 listeners?
(4c) Does the mortgage stay on the hand-over (the reading implemented keeps it; this port clears it so Afterglow's 「自动免费在上面加盖一栋房子」 can fire)?

**C-Q6 (MOVE-03, multi-activation).** *(updated)*
A card / band skill with several press effects (MyGO band (2)/(3), Mor:（NNM）稍微努力了一下): should the player pick which activation fires, or should one `On::Play` entry branch? The engine now uses **one entry with an in-body branch** (placed vs hand) for MyGO and NNM. Is that the model for all of them?

**C-Q7 (MOVE-05, mid-turn [停留]).**
Glossary 51 says [停留]=「无法移动」. If a player gains [停留] *during* a move (after the roll, before the walk ends), does the current move stop immediately, or does the prohibition only apply to the next move? (Engine now: the latter; the immobile clamp runs only between roll and walk.)

**C-Q10 (SETTLE-05, no_breakup R3).**
Sumimi:no_breakup 「若传送并触发结算后未能使资金变为拥有相同数字，回到原处并取消所有受到的效果」: is 「触发结算后」 `settleAfter` or the `tileResolved` terminal? And does 「取消所有受到的效果」 undo money as well (「不进行任何结算」 scopes to the return)?

**C-Q11 (SKILL-04, 状态2 exits).**
For skills that say 「直到状态2结束为止」 with no clause that ends it (umiri, nyamu, mutsumi): what ends 状态2? Options: (a) never (permanent once named), (b) same exits as the Ave Mujica band skill (fire-pot 0 at turn end), (c) a per-skill exit you specify, (d) other.

**C-Q12 (SKILL-05, CRYCHIC band (3) / (2) scope) — implemented; confirm only.**
Both halves are implemented and pinned by the un-ignored `card_paths::mutsumi_never_2_*` tests: (3) cashes out on the (2)-driven removal, and (2) is narrowed to `BandCrychic`. Confirm if the wider 「乐队技能」 reading was wanted.

**C-Q13 (TILE-01, Anon link + mortgaged partner).**
Does the Anon Tokyo link add half of a *mortgaged* partner tile's 收费?

**C-Q14 (TILE-02 residue, agent 「同色」) — main question ruled 2026-10-13.**
Two residues only: soyo's 「其他颜色」 (does the extra half apply on a same-colour agent?) and the 「视为live house」 half of Ruling 5. For reference, the ruling is that a per-player colour override **does** widen the colour group (`ctx::is_color` / `agent_colour_set`), an ANY_COLOR-only member is buyable / rentable but never buildable, and another seat's deed settles as the agent's 「半价收费的[结算]（向上取整10）」 (`t16_soyo_mixed_colors`, green).

**C-Q15 (TILE-03, RiNG).**
(15a) The dice-rent table 「地主拥有的 RiNG 数量 × ringMultiplier × 1d20」 and the 「至少1个」 floor are `data/match_rules.json` notes, not rulebook text. Keep them?
(15b) Is RiNG mortgageable? (游戏流程 5 says 「抵押拥有的地契」 with no exclusion; the engine refuses via `NO_MORTGAGE`, `rules/tiles/src/ring.rs:41`.)

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
CRYCHIC:想要抓住... (2) 「当第一位其他玩家经过你」: is that 经过 (mid-route pass of my tile, 行动阶段 12) or 重叠 (`passPlayer`, end-tile overlap)? Text says 「经过」; the engine currently uses `PassPlayer`.

**C-Q23 (MOVE-13, 星月夜 re-roll).**
Does 星月夜's re-roll cover the whole dice set or only the base die? (Ignored test asserts full set.)

**C-Q24 (MOVE-14, 普通与理所当然).**
Does 普通与理所当然 copy a signed or unsigned 移动格数? (Ignored test asserts unsigned.)

**C-Q25 (MOVE-15, heat_head).**
「减少1d6」: subtract 1d6 from the face, or drop a die from the pool (「少投1d6」)? Sheet says 减少1d6.

**C-Q26b (SETTLE-04 residual, 线香花火 / extra turn).**
Does 「until your next turn start」 end at the first turn start after, including an extra turn's? (R-7.) Q26a (the last-crystal removal does *not* grant an extra turn) was ruled 2026-10-13.

**C-Q28 (SETTLE-06, two halvings).**
（香澄）大家我都喜欢哦 halves 「地租」 and 祥，移动 halves 「支付价格」. Do they stack to ¼, or does the 支付阶段 「支付减半/翻倍」 window apply once? Engine now: ¼ (different nouns).

**C-Q29 (SETTLE-07, 晕眩 vs own gain).**
Rulebook 49 「无法收付款」: does it block a card's own 「获得」 to a player the same card body just stunned? (Mujica:心の雨 fallback does 「眩晕」 then 「获得1000资金」. Engine now: the gain lands.)

**C-Q30 (CH-08, welcome_mujica).**
「若指定了不存在状态2的玩家则无效果」: does this gate the switch on the target already being in 状态2 (only 2→1), or rule out targets for whom 状态2 is unreachable? Engine now: the switch is a no-op unless the target's `skillState` is 1 or 2 — so a player who cannot reach 状态2 is not toggled.

**C-Q31 (CH-09, dice_cast).**
「（此卡可以被反击）」: must other [反击]s still answer this card's own play window before the lock is set? (The reverse-order counter play landed with G06; the ruling question remains.)

**C-Q32 (CH-10, ran_as_usual once-per-turn).**
An unprinted once-per-turn gate applies to （兰）像往常一样 「此卡可以当反击使用」 that the passage does not name. Keep the limit (narrows), or drop it (text has no per-turn limit)? Engine now: kept.

**C-Q34 (CRYSTAL-05, 育美标记).**
A circle-tile case (remove every 育美标记 and gain 1,000 per mark 「经过起点」) is implemented although the rulebook text does not carry it. Keep it?

**C-Q35 (CRYSTAL-06, 薰 charge/decay).**
「（充能3，衰减1）」 names the charge and the decay but never says what happens at 0 (every other crystal card says 「为0时置入弃牌堆」). Keep decay-out, or leave the spent card on the field?

**C-Q36 (CRYSTAL-08, Ringing Bloom (3) X).**
「X为你收费格上的房屋数」: the count after (2) 「视为」 raises it (engine now), or the real house count?

**C-Q37 (SKILL-10, misaki).**
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
(40g) 弦卷地产: ~~「出价最大」 tie-break~~ — closed: a bid must clear the standing bid by ≥100 (`crates/game-core/src/engine/mod.rs:2720`).
(40h) 圆 rebuild: 「该改变[触发结算]的效果优先于其他任何改变[触发结算]的效果」 — real priority, or placement order (engine now)?

**C-Q41 (LOW-03, Mujica #J11).**
`Mujica:#J11` has no rulebook entry at all (the body follows the implementation alone). Please supply the clause, or confirm the implemented behaviour is the intended rule.

**C-Q42 (LOW-04, umiri 「置入弃牌堆」) — updated.**
「置入弃牌堆」 for a *taken band-skill card*: since G08 the card is really discarded (`Dest::Graveyard`), but a `skill:` rule id is dropped by `file_row` and never reaches a pile. Should a `skill:` id sit in the discard and be drawable, or be dropped as now?

**C-Q43 (TILE-05 follow-up, Parking Space × 要乐奈 (2) body-replace order).**
When 「通用:[都筑诗船]Parking Space」 is on Space and 「skill:要乐奈:投币式停车场的猫」（2） also fires on a settle at Space (both are 「将该次结算改为…」 settle-body replaces, `SETTLE-STAGES.md` §9 R4):
(a) if Rana owns Space, which replace wins — Parking Space's 停留 or Rana's 抹茶芭菲?
(b) if a third party owns Space, does Parking Space's body-replace also swallow Rana's 「免除付款并抽1张卡」 draw, or does the draw still happen as a skill effect outside the replaced body?
Engine now: first-to-claim (`trigger::cancelled()`); field hooks run in seat order, so Parking Space (P0) would win over Rana (P1). The CROSS-TESTS T24 path is unowned Space and does not need this answer.

**C-Q44 (TILE-01 follow-up, co-owned hill as 「[拥有者]拥有的格子」).**
When a deed is co-owned (星之鼓动山丘 is 「所有Poppin' Party角色视为同时拥有」), does it count as 「[拥有者]拥有的格子」 for Tomorrow's Door (3)'s surcharge for **each** co-owner? Test `t22_hill_coownership` (`rules/tests/tests/cross_tiles.rs:1034`) is `#[ignore = "RULING: …"]`. Cross-ref `OPEN-door-hill` in `docs/rulebook/spec/OPEN-QUESTIONS.md`.

**C-Q45 (春日影 draw-to-6 replaceable draws).**
春日影's 「抽卡直到手牌为6张」 iterates. Is **each intermediate draw** a replaceable draw (so a draw-replacement effect can fire per card), or only the final one? Test `g29_maya_look_at_top_inside_the_draw_curve` (`rules/tests/tests/gap_res.rs:248`) is `#[ignore = "RULING: …"]` and asserts yes. Cross-ref `OPEN-春日影抽6`. Note the sheet-sync 2026-10-17 added 「且手牌不多于5张」 to the [特] moment, which changes when the curve can start.

**C-Q47 (1d2 polarity).**
`HHW:（薰）怪盗hello happy`「投掷1d2（前后）」: for the direction die, is 前/后 **face 1 / face 2**, or **odd / even**? The port reads odd = reverse. The die-vs-d6-parity half closed as W3-KAORU-02 (ABI v124, `PlanValue.direction` + `direction_roll` roll a real 1d2), so only the polarity mapping is left.

**C-Q48 (W3-KAORU-03, 「所有在薰所在格子的人」).**
Does 「所有在薰所在格子的人」 include 薰 himself? If 薰's own main move is also meant to become 1d2+1d10, the `turn_player != owner` conjunct is an unprinted extra gate. A plausible design reading is that the effect weirdifies *other* pieces standing with 薰.

**C-Q51 (clear-cp-spread-window).** *(filed 2026-10-28; rows 1507; `rules/cards/card-general/src/clear_cp.rs`)*
[使用者]使用此卡后的下2回合开始时

Current: spread_guard requires trigger::player_id() == owner

Decision needed: After the card is used, whose two turn starts must the spread fire on — the user's own next two, or the next two chronological (any-player) starts?

**C-Q52 (fever-floor).** *(filed 2026-10-28; rows 932; `rules/cards/card-general/src/fever.rs`)*
X降低200（可小于0）

Current: (v.value + x).max(0) with no marker

Decision needed: When X is negative, may the boosted amount go negative (making the owner pay the difference), or is it floored at 0 as the code does — the cell only says X may be below 0 and prints no floor on the result?

**C-Q53 (marina-window-guard) — updated.** *(filed 2026-10-28; rows 881, 882; `rules/cards/card-general/src/marina_work.rs`)*
[经过]#1格子且#1格子受到其他效果影响时

Current: `ChainKind::TileEffectBefore` with `tile_is(tile.id, "circle")` (`:22-25`), so non-circle passes (incl. the re-homed cafe) no longer offer it.

Decision needed: Which passed tile counts as #1 (CiRCLE or its re-homed cafe), and what counts as 「受到其他效果影响」?

**C-Q54 (tsugu-step-3-before-2).** *(filed 2026-10-28; rows 853, 854; `rules/cards/card-general/src/tsugu_ycm.rs`)*
投掷3d10并根据结果依次进行以下操作：

Current: The linger lands at r>=28 before the (2) block (`:79-81,85`), so the step-2 buy itself is already discounted (tsugu_ge28_buy_discount_1500 pins it).

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

**C-Q58 (forced-settle-main-flag).** *(filed 2026-10-28; rows 1269, 565; `rules/cards/card-morfonica/src/noble_blue.rs`, `rules/cards/card-mygo/src/taki_serious.rs`)*
立刻进行一次触发结算 / 进行一次[触发结算]

Current: Both call `ctx::settle_tile_at(..., true)`: noble_blue.rs:47 and taki_serious.rs:73. main=true writes `st.landed` and `move.is_main_move`, so main-only settle hooks (e.g. 「不能通过主要移动的结算盖房」) fire for a settle where the player never moved.

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

Current: turn_snapshot pos (the turn-start square) with no record.

Decision needed: What is 「起始地点」 — CiRCLE, the seat's opening tile, or the pre-move position?

**C-Q69 (mutsumi-removal-dest) — updated.** *(filed 2026-10-28; rows 101, 102, 1154; `rules/cards/card-crychic/src/mutsumi_never.rs`)*
消耗乐队技能卡上的3个奇迹水晶（不足3个则改为全部消耗），立即执行乐队技能的（2）效果，然后弃一张卡。

Current: `Dest::Banished` at `:99`, logged as an unprinted 「被 [移除]」; only (3) prints 「此卡[移除]」.

Decision needed: Does (2) banish the played card (an unprinted [移除]) or use the default graveyard dest that only (3) prints?

**C-Q70 (out-of-turn-self-caused-draw).** *(filed 2026-10-28; rows 1000, 55; `rules/cards/card-crychic/src/elegant_shout.rs`, `crates/game-rules/src/wasm_rules.rs`)*
当你在回合外受到抽卡效果时打出

Current: `if n > 0 && player_id >= 0 && player_id != cx.state().turn` (`crates/game-rules/src/wasm_rules.rs:7171`) opens for every effect-caused out-of-turn draw, a self-caused one included, and the card answers it.

Decision needed: Does 「受到抽卡效果」 cover a draw the player causes themself during another player's turn, or only an externally caused draw?

**C-Q71 (myown-offer-gate).** *(filed 2026-10-28; rows 1179; `rules/cards/card-crychic/src/my_own_problem.rs`)*
若受到[异常移动效果]影响，此卡不生效

Current: The guard is `can_counteract` (the offer filter), so the card is never offered once an abnormal move already hit; the body also re-checks and logs 不生效.

Decision needed: Should 「此卡不生效」 leave the card unoffered (a spent, played no-op) or let it be played and resolve as a no-op?

**C-Q72 (aya-empty-discard-condition).** *(filed 2026-10-28; rows 1541; `rules/cards/card-pp/src/aya_longing.rs`)*
[手]：将此卡放置在[使用者]的[场地]并将弃卡区中的一张卡加入手卡。

Current: On::Play("", None, aya_longing); empty graveyard returns after placing (`:102-104`).

Decision needed: Is a non-empty discard a legality condition that blocks the play, or does the play legally resolve with only its placement half?

**C-Q73 (chisato-stale-designation-cancel).** *(filed 2026-10-28; rows 1402; `rules/cards/card-pp/src/chisato_mask.rs`)*
（1）[拥有者]的弃卡区洗入抽卡区时其他玩家[分摊][支付][拥有者]500资金。

Current: split_pay consults designation_cancelled unconditionally; the flag clears only at the next hand play.

Decision needed: Does a designation cancel from the current play also skip a later reshuffle's 500 分摊, or only the split inside that play?

**C-Q74 (guerrilla-mustbuy-shortfall).** *(filed 2026-10-28; rows 871, 1677; `rules/cards/card-ras/src/guerrilla.rs`)*
且必须购买

Current: On a shortfall the body logs guerrilla_broke and skips card_buy, so no raise-funds/破产 path runs.

Decision needed: On a 「必须购买」 purchase the player cannot cover, must the money pipeline run raise-funds/[破产], or does the buy soft-fail?

**C-Q75 (hey-kids-fee-scope).** *(filed 2026-10-28; rows 755, 1476, 760; `rules/cards/card-ras/src/hey_kids.rs`, `docs/rulebook/COVERAGE.md`)*
随后，你失去“转移后各格房屋造价总和－获得房屋数量×500”的资金。

Current: `cost_in` sums only the houses just moved (`rules/cards/card-ras/src/hey_kids.rs:131`), while 「各格」 can also read all owned tiles or the touched tiles including houses already standing there and the source remainder; `COVERAGE.md:728` bakes the narrow reading.

Decision needed: Does 「转移后各格房屋造价总和」 sum every owned tile, the tiles touched by the transfer (including houses already there and the source remainder), or only the houses just moved?

**C-Q76 (hey-kids-split-grammar).** *(filed 2026-10-28; rows 756, 1475; `rules/cards/card-ras/src/hey_kids.rs`)*
转移的房屋与目标格子中，至少一方数量为1

Current: `max = houses.min(targets.len()).max(1)` (`:75`), so 2 houses onto 2 targets offers k = 2.

Decision needed: Does 「至少一方数量为1」 constrain the player's k so one of houses/targets must be exactly 1, or only describe the minimum (allowing k>=2 when both exceed 1)?

**C-Q77 (be-strongest-livehouse-table-open) — updated.** *(filed 2026-10-28; rows 725, 1673; `rules/cards/card-ras/src/be_strongest.rs`)*
roll 1d10，传送到livehouse对应的格子（按格子编号排序，若为10或以上传送到“Live House”）

Current: A hardcoded 9-name table whose content equals every group-6 tile board-wide, plus a new `owns_livehouse` branch and sheet fallback 「若你没有Livehouse格子」 (`:33-57`).

Decision needed: Is the sorted livehouse table every group-6 tile on the board, or only the livehouses the user owns?

**C-Q78 (star-beat-layer-burn-ruling).** *(filed 2026-10-28; rows 652, 1426; `rules/cards/card-ppp/src/star_beat.rs`)*
可选择在绝对距离5格以内自己拥有的格子上进行一次盖房，随后减少1层

Current: A declined offer returns before the decrement (layer kept, `:178-186`); the decrement runs regardless of `card_offer_build`'s false (layer burned, `:187-188`).

Decision needed: When exactly does 「随后减少1层」 resolve — after a declined offer too, and after a build that fails for lack of funds?

**C-Q79 (repaint-exist-redirect).** *(filed 2026-10-28; rows 1669; `rules/cards/card-ras/src/repaint.rs`)*
使目标玩家的此次移动数-X

Current: On an EXIST redirect (`hit != them`) the body returns before `set_move_roll` and `set_pay_factor`, so neither effect happens.

Decision needed: When an EXIST redirect changes the target, do 「目标玩家」/「对方」 still mean the mover, so both effects apply to him?

**C-Q80 (repaint-min-roll-floor).** *(filed 2026-10-28; rows 1670; `rules/cards/card-ras/src/repaint.rs`)*
使目标玩家的此次移动数-X

Current: The engine re-applies `min_roll` after the diceRollResult window (`m.roll.max(mr)`, `crates/game-core/src/engine/play.rs:1373-1376`), so the cut is truncated.

Decision needed: Does the mover's 结果为10以上 floor re-apply after Repaint cuts the roll (truncating X), or does the cut stand?

**C-Q81 (storm-clause-readings).** *(filed 2026-10-28; rows 716, 1684, 1685; `rules/cards/card-ras/src/studio_storm.rs`)*
在距此卡所在格子X个格子处[结算]时 / 对那个玩家进行一次相当于此卡所在格子普通[结算]的(4-X)/4倍价格的收费

Current: `ctx::dist` is the undirected ring minimum (`crates/game-rules/src/wasm_rules.rs:1961-1969`), `charge_rent` charges the house-level/dice figure, and a mortgaged tile returns 0 so the card discards and charges nothing.

Decision needed: For 「在距此卡所在格子X个格子处」 and 「相当于普通[结算]的(4-X)/4倍价格的收费」: clockwise or undirected distance, base or house-level rent, and who (if anyone) is charged when the tile is mortgaged?

**C-Q82 (ringing-bloom-zero-value-pay).** *(filed 2026-10-28; rows 821; `rules/cards/card-roselia/src/ringing_bloom.rs`)*
你的任意非RiNG格子收费后

Current: No trigger::value() check, so the card discards and pays 500×X although the tile collected nothing; in a reflected rent only the 0-value parent pay carries target == owner.

Decision needed: Does 「收费后」 require the tile to have collected something, or does a fully counteracted / blocked / reflected pay still fire the 500×X?

**C-Q83 (sayo-cost-open).** *(filed 2026-10-28; rows 681, 1376; `rules/cards/card-roselia/src/sayo_play.rs`)*
打出时视为使用一次此卡使用者的技能

Current: The condition has no charge(owner) >= 6 and the body never spends 6 火罐, so the card grants +1/+2 at 0 pots; tests pin the free behaviour.

Decision needed: Does 「视为使用一次技能」 pay the skill's cost (its ≥6 火罐 gate and 6-pot spend), or is the use free?

**C-Q84 (debut-lock-waives-payee).** *(filed 2026-10-28; rows 51, 1001; `crates/game-core/src/engine/play.rs`)*
打出此卡后，本回合内你的资金不会下降（除拍卖与写明不受资金变动效果影响的情况外）

Current: money_inner returns Paid { paid: true, ..default } at the money_locked branch before the split windows, log_money, the payee credit at the transfer stage and the pay/paid raises -- the payer is spared and the payee is skipped.

Decision needed: Does the money lock spare only the user's debit while the payee still receives (and the pay events fire), or does it void the whole transfer?

**C-Q85 (lisa-bond-stage-ambiguity).** *(filed 2026-10-28; rows 694; `rules/cards/card-roselia/src/lisa_bond.rs`)*
当你使用技能进行传送后

Current: the code commits to MoveAfter (stage 13) and the stage TODO stays.

Decision needed: Which stage is 「使用技能进行传送后」 — MoveAfter (action stage 13), or a post-teleport point that the teleport's own landing precedes?

**C-Q86 (nobreakup-predict-model).** *(filed 2026-10-28; rows 783, 1076; `rules/cards/card-sumimi/src/no_breakup.rs`)*
进行一次可使你的资金变动为拥有相同数字的传送

Current: `predict` (`:52-60`) subtracts only table rent; purchase, build, force-buy, agent half-charge, event settle and dice rent are unmodelled, so legal destinations are omitted once any rent-repeat exists.

Decision needed: Which settle money changes must the prediction model cover (purchase, build, force-buy, agent half-charge, event settle, dice rent) for 「进行一次可使你的资金变动为…的传送」?

**C-Q87 (donuts-grant-void).** *(filed 2026-10-28; rows 828, 830; `rules/cards/card-sumimi/src/two_donuts.rs`)*
（1）获得[除外]直至你原本所在格子被其他玩家经过。

Current: A second copy returns at the top (`:58`) before recording SLOT_ORIG/placing, while a grant refused by [不可阻挡] or a countered abnormal window is discarded and the body still records, places and later runs (2) and the pots.

Decision needed: When (1)'s [除外] grant is barred — a second copy already exiled, [不可阻挡], or a countered abnormal window — is the whole card void, or do the record, (2) and the pot payout still run?

**C-Q88 (l12-per-pot-reading).** *(filed 2026-10-28; rows 1319; `rules/cards/card-sumimi/src/l12.rs`)*
每当你消耗火罐时，为此卡添加一个[奇迹水晶]

Current: The comment grounded one crystal per pot in `t.value` without saying the clause itself does not state it.

Decision needed: Is a crystal added per 火罐 spent or per 消耗 event (a mixed temp+pool spend now raising two events)?

**C-Q89 (anon-tokyo-tile-in-two-links).** *(filed 2026-11-13; `rules/cards/card-mygo/src/anon_tokyo.rs`)*
并在上述的两个格子间放置1个[奇迹水晶]（上限1）

Current: `place_mark_pair` is place-or-refuse atomic on **both ends**: a tile already carrying a link crystal of this kind refuses a second pair outright, so no tile ever sits at the end of two links. The singular 「放置1个…（上限1）」 leans that way, but nothing rules on it.

Decision needed: Does 「上限1」 cap crystals per tile (a tile is an end of at most one link), or only the pair (a re-link of the same pair), so a tile may be shared by two different links?

---

# Resolved ids cited from source

One line each, so a source comment that cites the id still resolves. Outcome
noted where it is known; all of these are closed in the tree.

- **HOST-03** — closed: the 行动阶段 3 「回合开始后」 and 6 「经营、出牌阶段后」 hooks fire in stage order, once, and not on a 眩晕-skipped turn (`host03_windows.rs`, `ab_small.rs`). Cited: `rules/tests/tests/host03_windows.rs`, `rules/tests/tests/ab_small.rs`, `rules/tests/tests/g09_tile_build_pass.rs`.
- **LOY-ANON-01** — closed: the capped re-link pays nothing (place-or-refuse stays atomic). Cited: `rules/tests/tests/ab_small.rs`, `crates/card-sdk/src/abi.rs`.
- **LOY-THANKS-01** — closed: the offer reaches a pay-gated seat. Cited: `rules/tests/tests/ab_small.rs`, `crates/card-sdk/src/abi.rs`.
- **CD-02** — closed: the folded `crystals_changed_of` guard still fires the j11 drain. Cited: `rules/tests/tests/ab_small.rs`, `crates/card-sdk/src/abi.rs`.
- **EVENT-04** — closed (ABI v124): `ctx::card_derived` / event-card enumeration read `CardInfo.derived`; the `NON_DERIVED` hand table is gone. Cited: `rules/tests/tests/ab_misc.rs`, `rules/events/src/end_it_all.rs`, `crates/card-sdk/src/kani_host.rs`.
- **MARKERS-KANI-01** — closed: the marker model has Kani coverage. Cited: `rules/kani/tests/conf_fire_windows.rs`.
- **COND-01** — closed: `run_hook` admits each entry against the live world, so a `slot()` condition sees an earlier entry's write in the same window (`ab_cond.rs`). Cited: `rules/tests/tests/ab_cond.rs`, `rules/fixtures/test-cards/src/lib.rs`, `rules/skills/skill-personas/src/kanon.rs`.
- **FIRE-TOKENS-1** — closed: SAVE_VERSION 23 folds a v22 `state["charge"]` / `["tempFire"]` into held-token rows. Cited: `rules/tests/tests/ab_small.rs`, `crates/game-core/src/engine/mod.rs`.
- **FIRE-TOKENS-2** — closed: the temp-pot / charge rows are held tokens with provenance. Cited: `crates/game-core/src/engine/debug.rs`.
- **LOY-NET-01** — closed: the static `DESIGNATES` props are stamped at targeting, so the counter window's pre-body read sees them. Cited: `rules/tests/tests/g07_targeting.rs`, `rules/tests/tests/ab_misc.rs`, `crates/card-sdk/src/abi.rs`.
- **ABI-11** — closed (ABI v124): per-player lost-since-turn accumulator, `ctx::lost_this_turn`. Cited: `rules/kani/src/world.rs`, `rules/tests/tests/ab_payment.rs`, `rules/fixtures/test-cards/src/lib.rs`.
- **ABI-12** — closed (ABI v124): `tile_prop_state` is tri-state (-1 / 0 / 1). Cited: `rules/tests/tests/ab_payment.rs`, `crates/card-sdk/src/ctx/sys.rs`, `crates/card-sdk/src/kani_host.rs`.
- **ABI-13** — closed (ABI v124): a temporary tile-rule swap, `ctx::tile_swap_rules` / `ctx::tile_set_rule` over board-field instances, identity reads static, expiry reverses; `circle_rebuild` / `smile_parade` migrated. Cited: `rules/tests/tests/ab_tile_swap.rs`, `rules/fixtures/test-cards/src/lib.rs`, `rules/cards/card-hhw/src/smile_parade.rs`.
- **SKILL-06** — closed (ABI v124): roster `general_only` in `deck::cant_play`, co-ownership, temp-pot provenance, and the carve-outs. Cited: `rules/tests/tests/ab_skills.rs`, `crates/card-sdk/src/abi.rs`.
- **HAG-01** — closed (ABI v124): the post-commit `markerChanged` action. Cited: `rules/tests/tests/ab_tokens_trap.rs`, `crates/card-sdk/src/abi.rs`, `crates/card-sdk/src/kinds.rs`.
- **FORCE-WRITE-01** — closed (ABI v124): force-buy add / mul / set filter kinds write 「强制购买」 through the Force skip. Cited: `rules/tests/tests/ab_payment.rs`, `rules/fixtures/test-cards/src/lib.rs`, `crates/card-sdk/src/abi.rs`.
- **NFO-CANCEL-01** — closed (ABI v124): the `pay_split_ran` bit splits a pre-`PaySplitAt` cancel from a shave. Cited: `rules/tests/tests/ab_payment.rs`, `rules/cards/card-roselia/src/nfo.rs`, `crates/card-sdk/src/kani_host.rs`.
- **STA-02** — closed (ABI v124): `AbnormalChoice` action kind raised at the abnormal gate; `status:unstoppable` answers `ask_yes`, journaled, AI defaults to refuse. Cited: `rules/tests/tests/g12_status.rs`, `rules/tests/tests/ab_unstoppable_choice.rs`, `rules/cards/card-ppp/src/rimi_choco.rs`.
- **W3-KAORU-02** — closed (ABI v124): `PlanValue.direction` + `direction_roll` give the real 1d2 with an explicit direction. Cited: `rules/tests/tests/ab_misc.rs`, `rules/cards/card-hhw/src/kaoru_thief.rs`, `crates/card-sdk/src/kani_host.rs`.
- **G15** — closed: the Kani verification host (model-only, no wire change). Cited: `rules/kani/src/spec/r.rs`, `rules/kani/src/proofs/hostops_smoke.rs`, `rules/kani/src/proofs/r.rs`.
- **DECLARE-TRAP-LOG** — closed: `entry_admitted` takes `on_trap`, landing with both trap arms. Cited: `rules/tests/tests/ab_tokens_trap.rs`, `crates/rules-native/src/lib.rs`, `crates/card-sdk/src/abi.rs`.
- **ENG-COST** — closed: `PlayEntry.cost` + `On::Play(..).cost`, the press gate refuses / charges before the body. Cited: `rules/tests/tests/ab_misc.rs`, `rules/skills/skill-groups/src/crychic.rs`, `rules/skills/skill-groups/src/ave_mujica.rs`.
- **MONEY-01** — ruled 2026-10-31 (C-Q1): within one stage every live multiplier composes as a product; no ordering question is left (`rules/spec/src/global/money.rs:1188`, `rules/tests/tests/gap_money.rs:116`, `fuzz_found.rs:169`).
- **ABI-02** — closed: the settle-redirect read moved onto the trigger (`is_settle_redirect`, `rules/cards/card-hhw/src/kokoro_circle.rs:95`).
- **ABI-04** — closed: a designation still in the pool is not re-asked (`ctx::ask_player_target` seeds the tag +1; `rain.rs:45`, `no_road.rs:65`, `please_choose.rs:99`, `cx.rs`).
- **ABI-08** — closed: draw-pile auto-play may skip the hand discard, and may be declined; the empty-hand follow-on is implemented (`spec_card_mujica.rs:907-955`, `test-cards/src/lib.rs:4366`).
- **ABI-09 / HOST-05** — v122/v123: engine auction request, free-amount money ask, persona-skin list (`ctx::auction`, `ctx::ask_money`, `ctx::persona_skins`).
- **CH-05** — join: the `SELF_REDIRECT` property arms the join; the redirect seat is `-1` (`mana_champion.rs:8`, `kani/src/world.rs:1125`).
- **CH-07** — closed by ruling C-Q5: 「视为此卡未生效」 still spends the card; the `wasted` bit is the fact listeners read (`no_road.rs:60,75`).
- **HOST-01** — closed (G10): an inline provider's nested settle is journal-equivalent to the host routine, and body reads see host writes (`inline_provider.rs:270,309`).
- **HOST-02** — closed (G10): a body's board write is what the following routine sees (`g10_effect_outcome.rs:772-775`).
- **HOST-06** — closed: `card_buy` / `acquire` answer whether the deed actually changed hands (refused buys do not log; `tsugu_ycm.rs:140`, `tomoe_savior.rs:74`).
- **HOST-07** — closed (ABI v53): quote == charge; a `BuyGate`-refused tile is dropped by `eligible` (`tsugu_ycm.rs:117`, `general.rs:142,169`).
- **HOST-08** — closed: `plan::set_build_anywhere` folds into `MoveCtx::build_anywhere` (`abi.rs:287`, `state.rs:30`, `move_ctx.rs:207`).
- **HOST-09 / HOST-10** — closed: the unused import and five unused host linker names are gone (`abi.rs:290`).
- **HOST-13** — closed: `agent` is kept on the auction audit line (`wasm_rules.rs:6567`).
- **MOVE-11** — recorded in TEST-FINDINGS (the `wasted` latch lifetime; `my_own_problem.rs:55`).
- **SETTLE-01** — re-entry guard, now under STACK-01 (`smile_parade.rs:17`).
- **SETTLE-02** — `lock_pay` is scoped (`cross_status.rs:163`; TEST-FINDINGS `:361`).
- **STA-01** — negative status stacks always run; the wear-off tick is declared (`bandori-std/src/status.rs:183,278`).
- **STACK-01** — the settle-chain queue on a 512 KiB thread is the stack-safety mechanism (`third_party/cel-rust/src/context.rs:146`).
- **TOOL-01** — deterministic iteration cap in `bot-core/tests/root_parallel.rs:361`.
- **LOY-TSUGU-01** — a re-run that asks a different request kind at a slot is not latched (`tsugu_ycm.rs:107`, `overlapping_voices.rs:88`, `g10_effect_outcome.rs:737`).
- **LOY-DREAM-01** — the turn-end buy window opens on the placed instance (`dream_ahead.rs:35`, `pp.rs:687`).
- **LOY-SEEYOU-01** — a refusal declines without committing the play (`see_you_tomorrow.rs:63`, `play.rs:6253`).
- **LOY-SOUL-01** — uncapped 「1到5次」 reading (`thanks_party.rs:37`).
- **LOY-TRAIN-01 / LOY-TRAIN-02** — crystal placement writes and the turn-end add (`pp.rs:1123,1164-1189`).
- **LOY-OVERLAP-01** — the discard hook banishes the drawn card (`pp.rs:303`).
- **LOY-TOMORI-03** — 「最早拥有的格子」 unpinned note (`crychic.rs:2024`).
- **LOY-MUSTBUY-02** — closed 2026-11-07: 「必须购买」 replaces the optional offer via the one-shot `no_buy` settle latch; pinned by `ras::guerrilla_teleports_and_must_buy` and `spec_card_sumimi::sweetescape_teleports_and_must_buy` (`guerrilla.rs:140`, `sweet_escape.rs:122`, `play.rs:1662,2773`, `work.rs:54`, `cx.rs:565,2573`, `ops.rs:66`).
- **AYA-GRANT-01** — settled 2026-10-20: the clause names the character (`aya.rs:16`, `pp.rs:1869`).
- **AVE-CLAW-01** — the one-sided bend clawback latches the `PaySplitMul` input (`money.rs:863`).
- **CRYSTAL-04** — the default 2000 comes from card metadata (`kani/src/proofs/mor.rs:1598`).
- **W3-KAORU-01** — latent settle-stage bug carried into SETTLE-STAGES (`settle_stages.rs:235`).
- **W3-DISP-01** — closed by the 2026-11-08 sweep (wave-3 prose).
- **OPEN-ROLLRESHAPE-01** — the 「最终结算」 reading is recorded in the test comment (`gap_money.rs:185`): the reshape lands before `tile`.
- **C-Q1 (MONEY-01)** — ruled 2026-10-31; see MONEY-01 above.
- **C-Q5 (CH-07)** — ruled: the card is still spent and discarded; the fact is the `wasted` bit on `discarded` (`ctx::set_ineffective_at`), not a "use" ledger (`no_road.rs:60,75`).
- **C-Q8 (WINDOW-03, EXIST)** — ruled (b): the latch is spent on the `target` trigger; a later-negated designation still draws (`g07_exist_redirect_then_negated_designation`).
- **C-Q9 (ABI-05, PP band (2) fan-flip)** — answered by implementation: `FansFlip` is its own timing (ruleset wire 107), listened to by the PP band (`bandori-std/src/kind.rs:41-50`, `pastel.rs:35`).
- **C-Q26 (花火-02/03)** — split into Q26a and Q26b (`no_road.rs:86` carries the adjacent ruling).
- **C-Q26a (花火-02/03)** — ruled 2026-10-13: the last-crystal removal does **not** grant an extra turn (`sparkler.rs:60-69`, `cross_long.rs:576`); the Q26b residual is open above.
- **C-Q46 (W3-NANAMI-01)** — answered: the waived (2) press is the bare body (invoke-entry path skips the gate; `nanami_effort.rs:179`). Confirm only.
- **C-Q49 (W3-HERE-01)** — closed 2026-11-06: (2) completion files the card to the graveyard (`Dest::Graveyard`, G08); the `here_held` prop replaced the old slot guard (`here_the_world.rs:26-89,152`).
- **C-Q50 (W3-ONEOFUS-01)** — closed 2026-11-08, ruled: (a) real ownership move (`ctx::rehome_card`), (b) unconditional even for a deed sold on, (c) the deeds are card attachments and die with the card (`one_of_us_hands_over_a_deed_sold_to_a_third_party`).

---

# Convention markers (not gaps — do not count as open)

`rules/{events,tiles,markers}/src/lib.rs`, `rules/skills/{skill-personas,
skill-groups}/src/lib.rs`, `crates/game-core/src/data.rs:328` (`event_rule_id`
doc), `crates/card-sdk/src/ctx.rs:789` (cross-ref to the ruling-6 note). These
name the `TODO(规则书)` policy: a gap in the text is tagged, never silently
dropped.

# Stale doc references (drift notes)

These name other docs whose text no longer matches the tree; fixing them is
doc maintenance, not rule work.

- **`docs/rulebook/TEST-FINDINGS.md:159-160`** §10 Resolved-summary bullet "ring opens at the next seat". Superseded by the 2026-10-07 ruling at `:764-795` (ring starts at the **initial user**; exhaust before advance). The ileuxali "next seat" clause is retired at `:767-770`.
- **`docs/rulebook/COVERAGE.md:17-20`** header "**774 tests, 102 ignored**" dated 2026-10-06 is historical. Every listed `**ign**` row below is still stale vs the live set:

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
| `:807` Ringing Bloom "(2)/(3) do not apply" | tests green; open item is C-Q36 (the X reading) |

- **`docs/rulebook/CROSS-TESTS.md:33-39`** ring rules quote the superseded clause-89 start / one-activation-per-visit (superseded by the 2026-10-07 initial-user ruling). The "70 pass / 41 ignore" tally is not in this file — it is `COVERAGE.md:1242-1244` describing CROSS-TESTS §1–§5 (2026-10-06); both are historical.
- **`docs/rulebook/NEGATION-AUDIT.md:315-322`** N1 (hook vs counteraction order) and N2 (marker spends) are listed as open in the §9 table, but V1 was fixed 2026-10-07 (`:61`, `:88-91`) and §5 was answered the same day (`:236-255`). N3–N6 (`:323-324`) remain open (folded into MONEY-04 / SETTLE-05 / C-Q3 / C-Q10).
- **`docs/rulebook/SETTLE-STAGES.md:176`** "latent bug" row on 凑友希那 force-stop is stale — it says `HookKind::PassPlayer … never fires`, but live `rules/skills/skill-personas/src/yukina.rs:43-47` uses `ActionKind::PassTile` with `move.remaining > 0` and notes the §4 M4 latent bug is fixed. The `TODO(规则书) PIPELINE-AUDIT Q7` marker at L5 is already gone.
