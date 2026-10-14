# Inventory: card-sdk generic vs game/ruleset-specific

Source brief: `target/scratch/brief-card-std-split.md`.
Dividing line: general card-runtime mechanism vs BanG Dream Monopoly / this ruleset's vocabulary.
Destination: `card-sdk` (generic) | `rules/ruleset-core` (new game crate) | `rules/tiles` (tile rule bodies) | `rules/cond` (CEL) | stay + document.

| # | Item | Kind | Destination | Notes |
|---|------|------|-------------|-------|
| 1 | `CardDef` / `On` entry kinds / manifest / `bandori_on` export | generic | `card-sdk` | wire + authoring mechanism |
| 2 | condition-pre plumbing (`On::pre`, guard fn plumbing) | generic | `card-sdk` | not the game's sugar |
| 3 | `Msg` + i18n key mechanism | generic | `card-sdk` | keys only; game strings live in card locales |
| 4 | `Prompt` / `ask_*` | generic | `card-sdk` | |
| 5 | `trigger::*` access | generic | `card-sdk` | |
| 6 | effect-chain access (`declare_effect`, chain kinds wire) | generic | `card-sdk` | |
| 7 | named counters + bound units + `MarkFilter` | generic | `card-sdk` | mechanism is name-string based |
| 8 | `On::Message` / `send` / `message::*` | generic | `card-sdk` | |
| 9 | generic state map (`state::{get,set,add,min,max,expires}`) | generic | `card-sdk` | |
| 10 | board/tile/seat/money primitives (`tile_count`, `tile_owner`, `money_of`, `gain`/`pay`/`transfer`, piles) | generic | `card-sdk` | monopoly-shaped but runtime mechanism |
| 11 | `ctx`/`sys` raw imports + guest ABI codes (`IMPORT_MODULE`, `bandori_*` linker names) | generic | `card-sdk` | wire must stay host-decodable |
| 12 | `is_placed` / `is_face_down_at` / `is_immune_at` | generic | `card-sdk` | card-instance state |
| 13 | `tile_is` / `tile_attr` / `tile_in` (NEW) | generic | `card-sdk` | Host ABI part: kind / field / class |
| 14 | `state_key::FIRE` name | game | `ruleset-core` | wire string `"fire"` value unchanged |
| 15 | `counter::CRYSTALS` / `counter::CP` names | game | `ruleset-core` | wire strings `"crystals"` / `"cp"` unchanged |
| 16 | `crystals` / `set_crystals` / `add_crystals` / `crystals_at` / `add_crystals_at` / `card_crystals` sugar | game | `ruleset-core` | sugar over named counters |
| 17 | `gain_fire` / `spend_fire` / `fire` / `fire_max` / `add_fire_max` / `decay` | game | `ruleset-core` | 火罐 vocabulary |
| 18 | `band_skills` / `band_skill` / `character_skill` / `add_band_skill` / `invoke_skill` / `band_crystals` / `add_band_crystals` | game | `ruleset-core` | band / character helpers |
| 19 | `in_band` / `character_is` / `skill_blocked` | game | `ruleset-core` | |
| 20 | `is_ring` / `is_circle` / `is_agent` sugar | game | `ruleset-core` | becomes `tile_is(t, "ring"\|"circle"\|"agent")` |
| 21 | `is_shop` / `is_live_house` / `is_buyable` sugar | game | `ruleset-core` | becomes `tile_in(_, t, "shop"\|"livehouse"\|"buyable")` |
| 22 | `is_live_house_for` / `is_color` sugar | game | `ruleset-core` | becomes `tile_in(p, t, …)` / class override |
| 23 | `ring_multiplier` / `add_ring_bonus` | game | `ruleset-core` | RiNG rent convention |
| 24 | `pay_rent` / `offer_buy` / `offer_force_buy` / `offer_build` / buy-kind conventions | game | `ruleset-core` | this ruleset's purchase verbs |
| 25 | `pre::MINE` (`actor == owner`) | generic | `card-sdk` | card-ownership pre, not game vocab |
| 26 | shared guards `crystals_changed_guard` (x11), `mine` (x7), `always` (x4), `any` (x3) | game | `ruleset-core` | dedupe into shared fns |
| 27 | `legacy_*` audit copies (~60) | game | deleted 2026-10-14 (ABI 61) | was NEGATION-AUDIT.md / GUARDS.md §5.1; do not port |
| 28 | `HookKind`/`TriggerKind`/`ChainKind`/`GateKind` wire enums | generic (stay) | `card-sdk` | host needs the codes; document game-named variants' ownership |
| 29 | `HookKind::CircleAffected` / `CpChanged` / `CrystalsChanged` / `FireSpent` etc. | game-named wire | stay in `card-sdk` + doc | keep code; ownership note |
| 30 | `prop::ANY_COLOR` / `prop::COLOR_FOR_PREFIX` / `prop::ALL_COLORS` | game data keys | stay as wire strings; class membership via generic `tile_in` | brief: overrides become generic class marks/props |
| 31 | CEL `is_circle` / `is_ring` / `is_live_house` / `is_buyable` | game vocab | `rules/cond` sugar → `tile_is`/`tile_in` | keep spellings as sugar so existing conditions keep evaluating |
| 32 | CEL `character_is` / `band_is` | game vocab | `rules/cond` + document | game vocab extension or boundary note |
| 33 | CEL `owner.fire` / `owner.crystals` | game vocab | `rules/cond` + document | sugar over generic state/counter reads |
| 34 | CEL `tile_is` / `tile_attr` / `tile_in` (NEW) | generic | `rules/cond` | three host views |
| 35 | `rules/tiles` `settle_reward` / CiRCLE / RiNG / agent / named-tile helpers | game | `rules/tiles` (already out of card-sdk) | `circle settle_reward` shared via rules-tiles — leave, it is already a tile rule body |
| 36 | `rules/tiles` `CIRCLE_MONEY` etc. | game | `rules/tiles` | already correct crate |
| 37 | three `CondView` host views (`SnapshotView`, `SnapView`, `LiveSnap`) | generic trait + game impls | `rules/cond` trait; impls in view + game-rules | add `tile_is`/`tile_attr`/`tile_in` |
| 38 | kani `Op` / `sys_kani` / `build.rs` ARRAY_OPS | generic ABI stubs | `card-sdk` + `rules/kani` | mechanical ABI stub + tables only |

## Host ABI replacement map (v56)

| Old host import | Kind | New |
|---|---|---|
| `is_ring(tile)` | kind | `tile_is(tile, "ring")` |
| `is_circle(tile)` | kind | `tile_is(tile, "circle")` |
| `is_agent(tile)` | kind | `tile_is(tile, "agent")` |
| `is_buyable(tile)` | class | `tile_in(-1, tile, "buyable")` |
| `is_shop(tile)` | class | `tile_in(-1, tile, "shop")` |
| `is_live_house(tile)` | class | `tile_in(-1, tile, "livehouse")` |
| `is_live_house_for(p, tile)` | class + override | `tile_in(p, tile, "livehouse")` |
| `is_color(p, tile, g)` | class + override | `tile_in(p, tile, "color:{g}")` |
| `tile_group` / `tile_price` (keep as sugar) | field | `tile_attr(tile, "group"\|"price")` |

`tile_attr` fields: `group`, `area`, `tier`, `price`, `house`, `rent` (table length), `index`.
String fields (`area`) parse as i32 when numeric; `kind`/`color` answered by `tile_is` / later.

## CEL three host views

1. `rules/cond/src/view_impl.rs` — `SnapshotView`
2. `crates/game-rules/src/cond_pre.rs` — `SnapView` / `SnapSrc`
3. `crates/game-rules/src/wasm_rules.rs` — `LiveSnap`

## Out of scope / already done

- flash declarable (ABI v55) — landed on master 896d246
- kani strfacade name interning — landed
- `rules/tiles` already holds tile rule bodies (not card-sdk) — no move needed
