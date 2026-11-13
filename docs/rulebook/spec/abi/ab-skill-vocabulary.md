# ab-skill-vocabulary — SKILL-06 deck / hand-building vocabulary

Status: landed in the `wf_2376cbc1-b6e-4` worktree (uncommitted at note time).
Backlog item: `docs/rulebook/BACKLOG.md` §SKILL-06 (class B, medium).

This is an A/B-batch note, not one of the G01–G15 group drafts: it records
eight sub-sites the sheet states but the ABI could not express, the generic
capabilities added for them, and the exact surface each one consumes. The
group-draft review procedure does not apply; the unit of review is the whole
batch, because several sub-sites ride the same new verbs.

## 1. Problem

The item named eight sites. Each is a card clause with no vocabulary:

| # | site | clause | class |
|---|---|---|---|
| 1 | `rules/skills/skill-groups/src/circle_staff.rs` | 「只能在卡组中加入"通用"卡」 and 「（除将牌加入卡组的效果以外）」 | deck rule + doubling carve-out |
| 2 | `rules/skills/skill-groups/src/sumimi.rs` | 「你可携带两个角色的专属卡牌」 | deck rule |
| 3 | `rules/skills/skill-groups/src/poppin.rs` | 「所有Poppin' Party角色视为同时拥有"星之鼓动山丘"」 + per-member mortgage + buy lock | engine ownership shape |
| 4 | `rules/skills/skill-personas/src/lisa.rs` | 「你提供的临时火罐将会在两回合后失去，使用…需要支付你600资金（被冲榜类效果被动消耗无需支付）」 | spend provenance + expiry |
| 5 | `rules/skills/skill-personas/src/mutsumi_crychic.rs`, `sakiko_crychic.rs` | 「但只有洗牌时可以为你的乐队技能卡上添加奇迹水晶」 | tagged crystal add |
| 6 | `rules/skills/skill-personas/src/rui.rs` | 「如果你的（2）技能或角色卡被复制，当复制者使x重置为0时，对方失去该技能（2）（抵押或赎回以外）」 | copy identity / trigger provenance |
| 7 | `rules/skills/skill-personas/src/tae.rs` | 「使用角色/团技能的[主]效果或使用手牌的[手]效果」 | trigger card-tag query |
| 8 | `rules/skills/skill-personas/src/soyo.rs` | 「若以此法单次免除了至少1500资金的[支付]」 | attribution of a saving |

Two sub-sites turned out to be already fixed on the data path and needed only
the stale TODO removed plus a regression test: CiRCLE's deck restriction
(item 1's first half — the roster now carries a band trait the deck builder
reads) and sumimi's carry rule (item 2 — `sharesExclusives` already existed).
Their second halves did need work; see §4.1, §4.2.

## 2. Generic capabilities

### C1 — co-held deeds (items 3, and the settle/mortgage path generally)

A tile has one primary owner (`st.owners[t]`, deed display and the ordinary
rent path) plus a **holder row** (`st.co_owners[t]`). Every holder, primary
included when the row is populated, **owns** the deed for eligibility:

- settle (build offer, unowned/rent branches),
- build,
- mortgage / redeem — **per holder** (`st.co_mortgaged[t]` is the set of
  stakes currently mortgaged),
- acquisition-order and bankruptcy scoring,
- rent-funding and "raise funds" scans.

The tile-wide `st.mortgaged[t]` flag stays the **aggregate** of the stakes
(any stake) so display and the force-buy path keep their single-flag shape.
`set_tile_owner` clears both rows; `release_deed` drops one seat's stake and
promotes a living co-holder when the primary goes out; the deed clears only
when the last holder leaves. `release_deed` also re-stamps both seats'
`FIRST_TILE`.

### C2 — deck-composition traits (items 1, 2)

Deck legality is engine data, not a guest verb. `GroupData` carries two band
flags from the roster rows:

- `general_only` (new, `generalOnly`): the band's starting deck may hold only
  `通用`-group cards (`CardData::is_universal`). CiRCLE.
- `shares_exclusives` (pre-existing, `sharesExclusives`): an exclusive card's
  legal owner widens to every persona of the same group. sumimi.

`game_core::deck::cant_play` is the single gate; `rules/bandori-std/src/roster.rs`
emits the flag from `GroupDef::general_only(true)`.

### C3 — reshuffle-tagged counter add (item 5)

`add_group_pool_shuffled` is `add_group_pool` with a provenance tag: the count
write is identical, and the real host's outcome line names the reshuffle as
its source (`log.outcome.counter_reshuffled`, nested
`log.reason.reshuffled` + the shuffling player). The two CRYCHIC faces move
the Ave Mujica band skill's crystal add onto `Reshuffled`; the band skill's
own third-turn add stands down for those faces.

### C4 — trigger card-tag query (item 7)

`trig_card_tags` returns the sheet tags ([主] / [手] / [特] / …) of every card
named on the current trigger, deduped, as an interned name list. It reads
`CardWorld::card_tags` (roster `tags`), so a listener says 「[手]效果」 without
string-matching ids.

### C5 — doubling carve-out (item 1's second half)

`ctx::n_deck(k, value)` is a normal number answer for the 「将…一个数字变为两倍」
offer (naming it consumes the one-shot slot) but returns the figure unchanged:
「（除将牌加入卡组的效果以外）」. Applied at `通用:GREAT`'s deck insert.

### C6 — temporary-pot provenance (item 4)

The fire-temp kernel (`bandori_std::fire::temp`) now carries, per holder:

- `fire.tempProvider` — who provided the live pots (`provider + 1`, 0 = none),
- `fire.tempExpiry` — the provider's `turn_key` at which they are lost.

`grant_from(provider, holder, n)` stamps both (`max` on the expiry so a later
grant never shortens an earlier batch); `pay_provider` moves
`COST * pots` (600 each) from an **active** spender to the provider;
`expire_owned_by(provider)` runs from the provider's `TurnStartBefore` and
drops the stamp and the counter (no `markerSpent` — the pots are lost, not
spent). `fire::spend_passive` is the carve-out class
(「被冲榜类效果被动消耗无需支付」); the 冲榜 event is its standing caller.

Item 6 needs no new verb: `ctx::invoke` runs the body for the copier while
`persona_skill` names the source, so `holder()` can bind X to the source and a
copier's reset writes the loss latch there. 「抵押或赎回以外」 is structural —
mortgage/redemption money never enters the pay pipeline and raises no `Pay`.

Item 8 needs no new verb either: `SettleBefore` captures the pre-payment money,
`Settle` latches what the ordinary landing would have charged, and
`SettleAfter` diffs them. Two CEL slot names had to be added to the condition
whitelist (`skill.soyoClear.coloured`, `skill.soyoClear.waived`) so the
`slot()` reads resolve; names outside `SLOT_NAMES` read 0.

## 3. Exact ABI surface

Six guest imports, appended to ABI v123's tail. Names are `ruleset_*`; the
native link names are the same.

```text
ruleset_add_co_owner(tile: i32, player_id: i32) -> i32
    Add a holder to tile's deed (idempotent). 1 = added, 0 = no-op.
ruleset_is_co_owner(tile: i32, player_id: i32) -> i32
    Is player_id in the holder row (primary included if listed)?
ruleset_owns_tile(tile: i32, player_id: i32) -> i32
    Primary owner or co-holder. The **single owner read** for eligibility.
ruleset_mortgaged_for(tile: i32, player_id: i32) -> i32
    Is this player's own stake mortgaged? Ordinary deeds: the tile flag.
ruleset_add_group_pool_shuffled(player_id: i32, np: i32, nl: i32, n: i32, max: i32) -> i32
    add_group_pool's count write + a reshuffle-sourced outcome line.
ruleset_trig_card_tags(buf: i32, cap: i32) -> i32
    Out-buffer: postcard Vec<String> of the trigger cards' tags, deduped.
    Two-call protocol (need, then write-if-fits), like trig_cards.
```

Typed card-sdk wrappers:

- `ctx::add_co_owner(tile, player) -> i32`
- `ctx::is_co_owner(tile, player) -> bool`
- `ctx::owns_tile(tile, player) -> bool`
- `ctx::mortgaged_for(tile, player) -> bool`
- `ctx::trigger::card_tags() -> Vec<NameId>`
- `ctx::n_deck(k, value) -> i32`

Plumbing (full discipline, per new verb):

| layer | file |
|---|---|
| wasm extern + `link_name` | `crates/card-sdk/src/ctx/sys.rs` |
| Kani sys | `crates/card-sdk/src/ctx/sys_kani.rs` |
| Kani `Op` (appended 499–504) | `crates/card-sdk/src/kani_host.rs` |
| typed wrapper | `crates/card-sdk/src/ctx.rs` |
| `HostCtx` ops | `crates/game-rules/src/hostfns.rs` |
| wasmtime `func_wrap` | `crates/game-rules/src/host.rs` |
| `CardWorld` trait + real impl | `crates/game-rules/src/world.rs`, `wasm_rules.rs` |
| native shims | `crates/game-rules/src/native_shims.rs` |
| Kani host / world / build | `rules/kani/src/host.rs`, `world.rs`, `build.rs` |

New `CardWorld` trait methods: `add_co_owner`, `is_co_owner`, `owns_tile`,
`mortgaged_for`, `add_group_pool_shuffled`, `card_tags`. Every one has a
safe default (false / empty), so non-engine hosts (Kani worlds, test stubs)
compile without implementing them. `owns_tile`'s default is
`tile_owner(tile) == player`; `mortgaged_for`'s is
`owns_tile && mortgaged_of != 0`; `add_group_pool_shuffled`'s falls back to
`add_group_pool`. `add_co_owner`'s default is `false` (no co-ownership
elsewhere).

No new engine `ActionKind` / `FilterKind` ids: the work rides existing actions
(`DeckAtGameStart`, `SettleBefore`, `SettleAfter`, `HandChanged`,
`Reshuffled`, `MoveResolved`, `TurnStartBefore`, `Pass`, `DiceRollAfter`,
`Play`) and existing `ChainKind`s.

## 4. Card usage

### 4.1 CiRCLE (items 1)

- `BAND` row sets `.general_only(true)`; `deck::cant_play` refuses any
  non-`通用` card with `err.deck_general_only`, the character's own exclusives
  included.
- `通用:GREAT`'s 「将一张"PERFECT"加入抽卡区」 count is `ctx::n_deck(2, 1)`:
  nameable by the doubling offer, never doubled.
- No `ExclusiveUse` entry: the deck gate is composition, not the in-match play
  gate.

### 4.2 sumimi (item 2)

The carry half was already enforced by `sharesExclusives` on the band row; the
stale TODO said otherwise. Comment corrected; regression test added
(`sumimi_carries_a_partners_exclusive`). The play half stays the engine's
own/base exclusive rule.

### 4.3 Poppin' Party (item 3)

- `grant_ownership` on `DeckAtGameStart`: hill primary = this seat, then
  `add_co_owner(hill, p)` for every `Poppin' Party` persona. Not `silent`
  (a deed write is visible).
- `before_settle` splits the hill rent to the un-mortgaged, still-standing
  band members. The split base is `ctx::rent_of(hill)` — `settleBefore` carries
  no value of its own.
- One split per raise: every band seat carries the card (and `PPP:Returns`
  borrows a second copy via `ctx::place_card`), so the engine dispatches the
  hook once per instance. The `poppin.splitRaise` tile prop latches
  `ctx::trigger::uid()`; later instances of the same raise return early.
- `rules/tiles/src/property.rs`: a co-held deed's single-primary charge stands
  down (the split pays it); `owns_tile` / `mortgaged_for` drive the build,
  mortgage and redeem eligibility.
- Buy lock untouched: the hill cannot change hands while any band member is in
  the game, which already covers the double-pay route.

### 4.4 Lisa (item 4)

- `declare_cap` (the provider's `TurnStartBefore`) calls
  `fire::temp::expire_owned_by`; the entry is no longer `silent` because the
  expiry narrates `log.fire_temp_expired`.
- `use_skill` grants through `grant_from(player_id, who, 1)`.
- `fire::spend` / `spend_skill` (active) call `temp::pay_provider` for the temp
  pots taken; `spend_passive` does not. The 冲榜 event now uses `fire::total`
  (temp pots are 火罐 too) and `spend_passive`.

### 4.5 CRYCHIC faces (item 5)

- 若叶睦（CRYCHIC）: `HandChanged(actor == owner)` drives 状态1/状态2 (band id is
  outside CEL, so the guard is a residual); `Reshuffled` adds one crystal via
  `add_group_crystals_shuffled`.
- 丰川祥子（CRYCHIC）: `MoveResolved` + `TurnStartBefore` set the state from the
  five-tile geometry; `Reshuffled` adds the crystal the same way. Its
  `TurnStartBefore` is no longer `silent` (the state edge narrates).
- The Ave Mujica band skill's third-turn add stands down for those two faces
  (`shuffle_only` matches the persona skill ids).

### 4.6 Rui (item 6)

`holder()` finds the seat owning `skill:八潮瑠唯:正论暴击`; an invoked copy
reads/writes X there. A reset by a copier sets `skill.yuriCrit.lost` on the
source and logs `rui_lost`; `mine()` refuses the source's own body once latched.
The copier's seat keeps its copy.

### 4.7 Tae (item 7)

`can_negate` narrows the categories: a hand play (`TriggerKind::Card`) only
with the sheet's `手` tag; a field-card press (`CardUsed`) only when the
pressed card is a skill card (a skill press is its [主] effect). Other
triggers are not counterable by this clause.

### 4.8 Soyo (item 8)

`SettleBefore` (`arm_waiver`, silent) captures money; `Settle` applies the
re-colour and latches the ordinary landing's charge; `SettleAfter` (guarded by
the `coloured == tile.id + 1` pin and `waived > 0`) computes the saving and
latches `skill.soyoClear.penalty` at 1500 or more. The nested dealer settle has
its own `settleAfter`, so the pin is what keeps the two apart. `SLOT_NAMES`
gains `skill.soyoClear.coloured` and `skill.soyoClear.waived`.

## 5. Save / record impact

- `MatchState.co_owners` / `co_mortgaged`: **new fields, `#[serde(default)]`**.
  An old save (and an old client) loads them empty = every deed ordinary, so
  `SAVE_VERSION` does not move.
- Same two rows mirrored with `#[serde(default)]` into the replay/rollback
  snapshot types: `engine::world::TurnSnapshot`, `engine::journal::WorldSnapshot`,
  `card_sdk::ctx::TurnSnapshot`.
- `BoardDelta` gains whole-row `co_owners` / `co_mortgaged` entries
  (`#[serde(default)]`), applied before the aggregate flag is recomputed and
  restored in reverse. `PassBase` carries the same rows.
- The `poppin.splitRaise` latch is a tile prop — a world write, not a state
  shape change.
- No engine/action kind ids change, so the replay model and archived engine
  bundles are unaffected.

## 6. Versioning

This worktree deliberately does **not** touch `ABI_VERSION` (123) or its doc
block; the integration/landing pass owns the bump. The six imports append to
the existing tail, and the Kani `Op` ids are appended at 499–504
(`add_group_pool_shuffled`, `trig_card_tags`, `add_co_owner`, `is_co_owner`,
`owns_tile`, `mortgaged_for`), after 498 (`persona`). `SAVE_VERSION` is
unchanged.

## 7. Tests

New file `rules/tests/tests/ab_skills.rs` (16 tests):

- deck rules: `circle_deck_admits_only_general_cards`,
  `sumimi_carries_a_partners_exclusive`
- co-held hill: `poppin_hill_is_co_owned_at_game_start`,
  `poppin_member_lands_on_the_hill_as_an_owner`,
  `poppin_hill_rent_splits_across_unmortgaged_members`,
  `poppin_member_mortgage_is_per_stake`
- fire-temp provenance: `lisa_temp_pot_charges_its_provider_on_use`,
  `lisa_temp_pot_expires_after_two_turns`,
  `lisa_temp_pot_is_free_on_a_passive_spend`
- reshuffle carve-out: `band_third_turn_add_stands_down_on_a_crychic_face`,
  `mutsumi_face_adds_on_a_reshuffle`, `sakiko_face_adds_on_a_reshuffle`
- copy identity: `rui_copy_reset_costs_the_source_the_skill`
- trigger tags: `tae_counteracts_a_hand_card_with_the_tag`,
  `tae_does_not_counteract_a_hand_card_without_the_tag`
- attribution: `soyo_waiver_denies_the_next_circle_pot`

The payment-cause assertions walk the log args (`log.pay`'s nested `src` →
`log.part.reason` → `reason`), because the cause is not the event key.

Commands run from the worktree:

```sh
CARGO_BUILD_JOBS=4 cargo check --workspace          # 0 errors
node tools/build-ruleset.mjs                        # then the test run
CARGO_BUILD_JOBS=4 cargo test -p ruleset-tests --test ab_skills
CARGO_BUILD_JOBS=4 cargo test -p ruleset-tests      # full suite
```

Kani proofs and the fuzz targets were not run (opt-in).

## 8. Alternatives considered

- **A separate `co_owner` list instead of reusing `owners`.** Rejected: the
  hill's primary needs a deed row for display and the ordinary rent path, and
  a parallel list would leave two "who owns this" reads to keep in step. The
  row keeps the primary and every holder in one list.
- **A flag on the tile ("co-owned") with the holder list elsewhere.** Rejected:
  the set is the fact; `mortgaged_for` needs a per-holder row anyway.
- **Tagging crystal adds with a generic "source" argument on
  `add_group_pool`.** Rejected as an arity break of an existing import with
  many callers; a sibling verb costs one import and leaves the old one's
  contract alone.
- **A generic trigger "tag" read on the card chain.** `trig_card_tags` is the
  narrow version; the generic provenance read (whose skill is running) is
  larger and not needed here.
- **A deck-building API.** Rejected: deck legality is data (`generalOnly`,
  `sharesExclusives`) read by the engine's deck gate; no guest code runs at
  deck construction.
- **A provenance counter per temp pot.** Rejected: only Lisa provides temp
  pots; one stamp per holder with a `max` expiry keeps a batch's life intact.

## 9. Open questions / risks

1. **The split base.** `settleBefore` carries no value, so the Poppin' split
   divides `ctx::rent_of(hill)` (the tile's current rent, houses included)
   rather than the settle's own figure. If another effect ever reshapes the
   settled amount before the split, the split would not see it. The command
   pre-split stage (`PIPELINE-AUDIT` Q2) still shapes the divided figure.
2. **A co-held hill with no band card left.** The tile's single-primary charge
   stands down whenever a holder row exists, and only the split pays. If the
   band card is gone while the row survives, the hill charges nobody. A held
   row implies the card was in play; `release_deed` shrinks the row as seats
   go out, but the row itself is not tied to the card instance.
3. **`poppin.splitRaise`** is a persisted tile prop. It is one i32 overwritten
   per raise; a stale value can only collide with a recycled raise id.
4. **Single fire-temp stamp.** A second provider (none today) would overwrite
   the first. The `max` expiry keeps the later grant from shortening an
   earlier one, but the provider id is replaced.
5. **Soyo attribution ordering** depends on the rollback/re-run model: the
   `Settle` body's writes are rolled back while the nested dealer settle
   commits, so `WAS` must be captured in `SettleBefore`. That is documented in
   the skill, but it is a real ordering coupling.
6. **Silent-flag removals** (Lisa turn start, Sakiko state sync, Poppin grant)
   make those entries visible bodies, so they narrate each turn. The narration
   is the outcome line, not a printed effect; an empty turn still logs the
   entry's own line.
7. **`card_tags` is roster data.** A test host or a roster row without tags
   yields an empty list, so `tae`'s guard reads "no [手] tag" rather than
   erroring. That is deliberate (safe default), but a future tag-less card
   would silently not be counterable.
8. **Kani stubs.** The Kani `CardWorld`/host keep the trait defaults
   (`add_co_owner` false etc.). The proofs that touch the new predicates model
   them as "not a co-holder", which is the ordinary save shape; co-ownership
   itself is covered by the integration tests, not the proofs.
