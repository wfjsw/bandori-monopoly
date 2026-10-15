# Money rounding — one round after add/mul (user ruling 2026-10-15)

**RESOLVES** the OPEN-取整 family in `docs/rulebook/spec/OPEN-QUESTIONS.md`
(OPEN-取整 / OPEN-半价取整 / OPEN-减半取整 / OPEN-ROUND10 / OPEN-half-rent /
OPEN-减半取整 vs rainbow CeilTo-10 / OPEN-取整顺序 for running-amount modifiers).

## Ruling

Money rounding (**up** to a multiple of 10) is performed **once**, **after** all
add/mul modifiers of an amount have been processed. Modifier cards (减半 /
1.5倍 / +x / ×y …) must transform the running amount **exactly**; they must
**not** CeilTo-10 inside their own body.

Where the book prints 「向上取整10」 on a formula, that printed rounding **is**
the end-of-calc rounding (it is not an extra early round). Examples that keep
their CeilTo: `[地产商]` half-rent, the `[分摊]` share formula
`ceil10(ceil(total/n))`, `HHW:爱心义演` 「减半（向上取整10）」.

An amount that **no** modifier touched is **not** rounded: it stays exactly as
the base formula produced it (rent table, printed X, mortgage/redeem rate, …).

## Engine

| Site | Behaviour |
|---|---|
| `crates/game-core/src/engine/play.rs` `money_inner` | Tracks `modified` across `effect` / `payTotal*` / `payAdd` / `payMul`+`scale_settle_payment` / `payChoose` / `payAt` / `pay`. After the chain, a **positive** modified figure is `ceil10`'d once. Negative figures are not rounded (the Q3 reversal must see the true sign). |
| `pay_total` (pre-split / 「[分摊]」) | If a `payTotal*` hook rewrote the total, `ceil10` it once before the share formula. Untouched totals divide as-is; each share is still `ceil10(ceil(total/n))` (printed rule). |
| `ceil10_money` | `((x + 9) / 10) * 10` for `x > 0`; `x` unchanged otherwise. |

### Modifier stages (rulebook 支付阶段)

| Stage | Hooks | Rounding |
|---|---|---|
| 2 计算时 ± | `payAdd` / `payTotalAdd` | transform exactly |
| 4 支付前 × | `payMul` / `payTotalMul` / `set_pay_factor` / `scale_settle_payment` | transform exactly (factor path keeps its f64 milli scale) |
| 5 支付时 | `payChoose` / `payAt` / `pay` (incl. amount rewrites) | transform exactly; still counts toward the one end round |
| settle | — | **the one `ceil10`**, if any modifier rewrote the figure |

## Card inventory (running-amount modifiers)

| Card / skill | File | Transform | CeilTo in body? | Change |
|---|---|---|---|---|
| `Mor:秘密与青春的虹彩` (rainbow) | `rules/cards/card-morfonica/src/secret_rainbow.rs` | (1) ×0.5 (2) ×1.5 | **was yes** | **DONE** — writes fold `(x+1)/2` / `(x*3+1)/2`, no CeilTo; engine rounds once |
| `AG:（摩卡）0.5倍速` (half-speed / moca_half) | `rules/cards/card-ag/src/moca_half.rs` | ×0.5 | **was yes** | **DONE** — same |
| `R:Fire bird` | `rules/cards/card-roselia/src/fire_bird.rs` | ×1.5 | **was yes** | **DONE** — same |
| `HHW:爱心义演` (charity_show) | `rules/cards/card-hhw/src/charity_show.rs` | ×0.5 | yes (printed 「向上取整10」) | **KEEP** — printed end-of-calc; idempotent with engine round when sole modifier. Stack caveat: another modifier after it still sees the early round. |
| tukushi_try / tomori_poem / mana_donut / soyo_colors | `rules/skills/…`, `card-mygo/soyo_colors.rs` | ×0.5 (`(x+1)/2`) | no | fold already; engine now also rounds at settle (behaviour change on non-multiples of 10) |
| ave_mujica bend | `rules/skills/skill-groups/src/ave_mujica.rs` | ×1.5 (`(x*3+1)/2`) | no | fold already; engine now rounds at settle |
| chisato_frank / aya_with / fever / nfo / … | various | +x / −x / ×2 | no | exact integer; engine rounds at settle if the figure is not already a multiple of 10 |
| plan/tile factors (`set_pay_factor(500)`, `RENT_FACTOR`) | saki_move, sakiko_lead, repaint, rana_funny, taki_serious, council_check, kasumi_love_all, studio_storm, ringing_bloom | ×0.5 / ×0.25 / … | truncate in `scale_settle_payment` | engine rounds at settle when the scale rewrote the figure |

### Base formulas (NOT running-amount modifiers — unchanged)

| Site | Formula | Why kept |
|---|---|---|
| `[地产商]` half-rent | `ceil(full/2/10)*10` | printed 「半价收费的[结算]（向上取整10）」 |
| `[分摊]` share | `ceil10(ceil(total/n))` | printed share rule |
| G-MONEY agent half-charge | `half_ceil10` | printed |
| sunset-01 X | `ceil10(sum,n)` then optional `ceil10(x,2)` | printed X formula (OPEN-取整顺序 stays for the pre-100 half of X itself) |
| budokan X | `ceil10(2000/alive_others)` | printed |
| `R:登上武道馆之巅` (to_the_peak) 1.5× build | `ceil_to(build_cost*3/2, 10)` then `ctx::pay` | computed charge from scratch (no running amount); CeilTo is its end-of-calc |
| `R:湛蓝蔷薇` (blue_rose) 20% gain | `CeilTo(sum*0.2, 10)` | gain formula, not a pay modifier |
| mortgage 50% / redeem 60% | floor / ties-even | valuation (OPEN-抵押半价取整 / OPEN-60%舍入 stay for odd prices) |
| glory half, mashiro half, one_of_us half, … | floor half as a **derived transfer amount** | each is a fresh `Pay` base, not a mid-chain rewrite |

## Integer fold vs exact rational

Modifier bodies write an integer fold. For 减半 / 1.5× the fold rounds the
`.5` **up** (`(x+1)/2`, `(x*3+1)/2`) so a **sole** modifier still settles at
`round10(exact)`:

| base | exact fold | integer fold | final `round10` |
|---|---|---|---|
| 25 half | 12.5 | 13 | 20 |
| 25 ×1.5 | 37.5 | 38 | 40 |
| 18 half | 9 | 9 | 10 |
| 18 ×1.5 | 27 | 27 | 30 |
| 38 half | 19 | 19 | 20 |
| 38 ×1.5 | 57 | 57 | 60 |
| 58 half | 29 | 29 | 30 |
| 58 ×1.5 | 87 | 87 | 90 |

**OPEN (precision):** when **several** fractional modifiers stack, an integer
intermediate can diverge from the rational fold (e.g. 25 half → 13, then ×7 →
91 → 100, vs exact 12.5×7 = 87.5 → 90). The ruling wants the rational fold;
the minimal implementation uses integer folds + one deferred round. Treat
multi-fractional stacks as OPEN until a scaled/rational running amount lands.

## Kani pins

`final = round10(fold of add/mul)` — see
`rules/kani/src/spec/{mor,ag}.rs` (`rainbow_01_amount` / `rainbow_02_amount` /
`half_speed_04_amount_ceil10`) and the matching `*_iff` proofs. The body-write
pin is the fold (`rainbow_01_fold` / `half_speed_04_fold`); the settled pin is
`round10(fold)`.

## ABI

ABI 71 (`rules/card-sdk/src/abi.rs`): host semantics of `set_pay_amount`
composition (one end-of-calc `ceil10`). No new import. SAVE_VERSION unchanged.
Replays stay compatible via archived engine bundles (repo rule); this change
does not deploy.