# Marker counters as owner-declared units

Status: design concluded 2026-10-18 (usage census + card-face analysis), first
implementation in this branch. Companion to `docs/STATUS-HOSTS.md`: that doc
moves the **conditions** ([停留]/[晕眩]/[除外]/[不可阻挡]) into `rules/status`;
this doc covers the **quantities** (标志物/指示物) so neither family leaves
vocabulary in `crates/`.

Motivation (user request 2026-10-18): 火罐 was designed to be bound to the
player's skill card -- a single source of write, other cards communicating with
the skill to modify it -- but the usage census found it a free resource with
10+ unowned writers (Ave Mujica's band skill raw-sets the pool, `Sumimi:一人
两个甜甜圈` grants across seats, `lisa_goddess` invents a parallel `tempFire`
key, `Sumimi:Sumimi是二人一体的` hardcodes 初始/上限 pairs). P✽P粉丝 is a
second free-keyed quantity. Neither may pollute the engine.

---

## 1. Rulebook ground fact

`data/rules.txt` 「标志物解释」:

> · CP点：放置于路面上的指示物
> · 火罐：技能指示物，在任意时刻，您持有的火罐数量不能超过持有上限
> · 雨伞：代表人物[停留]状态
> · 弹簧：代表人物[晕眩]状态
> · 奇迹水晶：卡牌指示物
> · P✽P粉丝：Pastel✽Palettes乐队标
> · 抹茶巴菲：Crepe饼

雨伞/弹簧 are physical *representations* of statuses (the `rules/status`
family). The rest are quantities.

The **face grammar** -- the ownership phrasings actually printed on cards:

| Phrasing | Host named | Example |
|---|---|---|
| 「为此卡添加Y」 | this card instance | 奇迹水晶 (`skill:Ave Mujica:假面之下的真实`, Paspale 「为此卡添加等量溢出的[奇迹水晶]」) |
| 「（初始N，上限M）」 / 「（火罐上限N）」 / 「其初始火罐」 | the face printing the numbers | 34 character skills; `skill:若叶睦:天生的演员` 「获得其初始火罐」 |
| 「你的技能拥有火罐」 | the character skill; the speaker is a **guest** | `skill:Ave Mujica:假面之下的真实` |
| 「你获得Z」 / 「将X个Z变正」 across seats and cards | a shared kind -- **host-owned** | P✽P粉丝 「非Pastel✽Palettes角色获得1个反面[P✽P粉丝]」 |

Lifetime clauses that fix the classes:

* 「替换为sumimi的另一名角色**及其初始火罐数**」 (`Sumimi:Sumimi是二人一体的`) --
  swap **re-declares** the pool at the new face's 初始; never carry-over.
* 「如有场上有多张Pastel✽Palettes乐队卡时**此效果不重复发动**」 -- creation is
  once-per-match, not once-per-instance.
* 「X大于拥有的反面的[P✽P粉丝]时可为此卡添加**等量溢出**的[奇迹水晶]（最多10个）」
  -- the writer must learn the **actual** amount (the overflow is `X - owned`),
  and the 10 is the card's **total** crystal cap (user ruling 2026-10-17).
* 「在任意时刻，您持有的火罐数量不能超过持有上限」 -- the cap is an invariant,
  not a per-write courtesy.

## 2. Taxonomy

| Family | Members | Shape | Home |
|---|---|---|---|
| conditions (状态) | [停留] / [晕眩] / [除外] / [不可阻挡] | layered state, gates, wear-off ticks | `rules/status` hosts (STATUS-HOSTS.md) |
| markers (指示物) | 火罐, P✽P粉丝, CP点, 奇迹水晶, 抹茶巴菲, designate marks | named counters, units in locations | this doc |
| companions | 无手牌 (`noHand`) | a slot; not a glossary marker | `rules/status` for symmetry |
| plain resources | 资金 | engine money | engine |

## 3. The model

A marker is `⟨kind, owner, units@locations, policy⟩`.

* **owner** is a rule, in one of two classes:
  * **Instance-owned** -- the creating card/skill instance. On-card units die
    with the instance.
  * **Kind-owned** -- one standing host pseudo-card per kind (the `mark:cp`
    precedent, `rules/markers/src/cp.rs`). Bound units live until an effect
    removes them; the host owns the protocol. The kind-owned host family lives
    in `rules/markers` (one crate, one `CardDef` per `mark:*` category).
* **units** live in locations: on-card pool / tile / holder
  (`ctx::{place_mark, add_held, move_units}` -- existing ABI).
* **policy** (cap clamp, partial gain, overflow conversions, 效果适用 logs) is
  declared once by the owner and applied by a shared kernel.

**Decision procedure.** For a quantity Q named on face F:

1. 「为此卡添加Q」 -> instance-owned by F.
2. 「（初始N，上限M）」 / 「Q上限N」 / 「其初始Q」 -> instance-owned by the face
   printing the numbers.
3. 「你的技能拥有Q」 -> instance-owned by the *skill* named; F's clauses are
   guests.
4. Q held by arbitrary seats, created/flipped by many unrelated faces, claimed
   by no face -> kind-owned host.

The procedure is total and single-valued over the usage census (2026-10-18);
any new phrasing extends the table, never the model.

## 4. Protocols

1. **Owner-routed writes.** `grant` / `spend` / `adjust_cap` run the owner's
   policy and return **actuals**, so caller clauses keep their own text
   (`Sumimi:一人两个甜甜圈` converts the pots that did not fit into 500资金).
   Ordinary grant/spend is not a message round-trip.
2. **`send` / `On::Message`** for owner-decides cases (spend substitution
   「以代替此次技能的火罐消耗」, refusal, redirect).
3. **Regime guests.** A clause like Ave Mujica's 「进入状态2时将火罐补充至上限
   且每回合结束时失去1火罐…退出状态2时，失去所有剩余火罐」 is a named regime
   protocol (`fill_to_cap` / `drain` / `burn_all` + a read-only exit test) over
   the *owner's* pool -- never a raw write.
4. **Flip** = `move_units` between two named counters of one owner (e.g.
   `fansDown` -> `fansUp`) with a **pre-raise**, so listeners see the pre-flip
   count (Paspale clause （2）'s 「X大于拥有的反面」).
5. **Once-flags** for 「不重复发动」 (creation idempotent).
6. **Re-declare on swap.** Unplace/place the skill; the new face's declaration
   *is* the new 初始/上限.

## 5. Storage and wire

Storage stays the wire-visible named data (`MatchPlayer.state["charge"]`,
`["ppFansUp"]/["ppFansDown"]`, `FieldCard` counters): the webui keys
(`webui/src/core/names.ts`, `Players.tsx`, `autopilot.ts`) are frozen and are
the projection. Ownership, lifetime and protocol are the semantics layered on
top; physical re-homing of a key into an instance counter/units table is a
non-semantic refactor and is not required by this design.

What matters: the engine treats these names as **opaque data**. Only the
rules-layer kernel writes them; only the owner's policy decides what a write
means.

## 6. Engine surface (what `crates/` may know)

Kept -- name-as-data mechanisms only:

* instance counters + bound units (`place_mark` / `add_held` / `move_units`);
* marker windows (`markerGain` / `markerSpend`) with the name as payload;
* raises (`CounterChanged`, and the spent/gained raises) with the name as
  payload;
* the owner stamp (`World::marker_owner` -- "a marker is owned by the rule that
  creates it, wherever its copies sit", ruling 2026-10-07).

Retired -- every game name (purge inventory; status as of 2026-10-19):

| Site today | Target | Status |
|---|---|---|
| `game-core/state.rs` `key::CHARGE`, `MatchPlayer::charge()/charge_max()` | generic named-state reads from the rules layer | **done** -- `key::CHARGE` kept as the wire key constant (webui contract); `charge()`/`charge_max()` are thin `state_get`/`state_max` wrappers, "wire key only" |
| `game-core/ops.rs` `charge`/`charge_max`/`add_charge_max`/`gain_charge` (+ `log.gain_charge` built engine-side) | dumb value ops; policy and the 效果适用 log move to the kernel | **done** -- `gain_marker`/`add_marker_max` take the name as data; no ops-side `log.gain_charge` (wasm_rules `post_log_value` keeps the one 效果适用 path) |
| `game-rules/hostfns.rs` charge verbs; `HostRequest::Marker { name: "charge" }` | generic marker ops; the name is a parameter | **done** -- `marker_get`/`gain_marker`/`spend_marker`/`add_marker_max` take `name`; charge verbs remain thin re-exports (guest ABI unchanged) |
| `game-rules/wasm_rules.rs` `spend_charge`, `charge_spent_log`, the `fireSpent` raise | generic `MarkerSpent { name }` | **partial** -- `charge_spent_log` → `marker_spent_log` with name; wire kind `fireSpent` (73) kept (card bodies subscribe `FireSpent`) with `TODO(ABI)` to rename to `MarkerSpent` |
| `card-sdk/abi.rs` `state_key::CHARGE` ("Fire pots held"), `ChargePaying` | rules-layer names; the substitution window is the generic spend window | **done** -- `state_key::CHARGE` doc = wire key constant only; `ChargePaying` (78) documented as the generic marker spend-substitution window (name is payload). `ABI_VERSION` not bumped |
| `rules/skills/skill-groups/src/fans.rs` free keys written by anyone | host protocol (§7) | **done** -- `mark:pp_fans` host in `rules/markers/src/pp_fans.rs`; `fans.rs` is the re-export facade |

## 7. Bindings

### 火罐 -- instance-owned by the character skill

Owner: the seat's active character-skill instance (the face printing
「（初始N，上限M）」; textually the character card's 初始火罐). The numbers
live on that face's own `CardDef` (`.fire_pots(N, M)` → props `fireInitial` /
`fireCap`, keys in `bandori_std::fire`); `fire::declare_own` reads them back
through `ctx::card_prop`. Cross-face reads (`Sumimi:Sumimi是二人一体的`,
`skill:若叶睦:天生的演员`) take the named skill id and read that skill's
`CardDef` the same way.

Guests, all through the kernel: hand cards (`Sumimi:一人两个甜甜圈`,
`Sumimi:儿时玩伴的鼓励`, `PPP:（沙绫）`, `PPP:（多惠）`, `AG:无论是何种颜色的
夕阳`), events (`event:发送熊饼表情`, `event:冲榜`), the Ave Mujica regime
(`skill:Ave Mujica:假面之下的真实`), `PPP:[衍生]拍卖撤下来了` (`adjust_cap`).

lisa's 「可超过上限的临时火罐」 (`skill:今井莉莎:慈爱女神`) is a **second
declared counter** of the same instance (`fire.temp`), not a free key.

### P✽P粉丝 -- kind-owned host

Owner: the standing `mark:pp_fans` pseudo-card, home
`rules/markers/src/pp_fans.rs` (the `mark:*` kind-owned host family; the
rename from `rules/tile_marks` is because hosts are not tile-only --
`mark:cp` is tile-bound, `mark:pp_fans` is holder-bound). It owns
`fansUp`/`fansDown` per holder, the flip protocol (pre-raise), and the
once-per-match initial grant (「多张…乐队卡时此效果不重复发动」). Storage
stays the wire keys; `rules/skills/skill-groups/src/fans.rs` re-exports the
verbs so call sites keep the `fans::*` signatures.

The Pastel band skill keeps only what its face binds to it: clause （1） as a
creator call to the host, clause （2） as its listener (per its **own owner
seat** -- 「你…受到」), and the overflow crystals on itself (「为此卡」).
Persona skills and `card-pp/**` cards keep calling `fans::{grant,flip_*}` -- the
facade signatures do not change.

**Two affiliations are not the same thing** (the faces distinguish them):
「非Pastel✽Palettes**角色**」 is a test on the character's group;
「多张Pastel✽Palettes**乐队卡**」 is about band-skill instances. A seat may be
either, both, or neither -- the initial grant tests the first, the dedupe tests
the second, clause （2） is per band instance.

### Others

* 奇迹水晶 -- instance-owned (「为此卡添加」); multi-writer by card text via
  the kernel.
* CP点 -- kind-owned (`mark:cp`, already the model).
* 抹茶巴菲 -- instance-owned by `skill:要乐奈:投币式停车场的猫` (unique skill;
  instance == kind here). Cross-writes (`card-mygo/src/rana_funny.rs`) route
  through the kernel.

## 8. Invariants

* **I1** every unit belongs to exactly one ⟨kind, owner, location⟩.
* **I2** value <= declared max **at all times** (the book's 「在任意时刻」).
* **I3** on-card units die with the instance; bound units live until removed.
* **I4** every change passes the owner's policy exactly once (one 效果适用 log,
  one window point).

## 9. Enforcement

* Card bodies lose raw cross-instance writes (ABI surface + `pure::` refusal);
  a CI grep gate keeps them out.
* Kani iff proofs on the kernel bodies (exact effect sets -- the
  `twodonuts_01_exile_iff` shape); I2 as a Kani assert.
* `rules/spec` clauses pin I2 and the face-grammar bindings.
* The multiplicity matrix (§7 two-affiliations) as tests: N Palettes personas,
  M band cards, PP-band + non-Palettes character (they *receive* the initial
  fan and hold clause （2）), Palettes character + other band.

## 10. Open pins (policy orderings, not structure)

1. 「进入状态2时」 clause order -- Ave Mujica 「将火罐补充至上限」 vs
   `skill:八幡海铃:熟练的支援贝斯手` 「进入状态2时你可消耗X个火罐」.
   **Implemented as recommended: band refill first**, then face entry clauses
   (makes 八幡海铃's entry-spend real, not free). The band skill's
   `TurnStartBefore` body runs the refill before character-skill entry hooks
   in the same pass; documented on `fire::regime` and `ave_mujica::on_state`.
2. `fire.temp` vs pool spend order when both are live. **Implemented as
   recommended: temp first** (it is the over-cap allowance) --
   `fire::spend` draws `fire::temp` before the declared pool.
3. Fan rows of out-players. **Recommend: persist** (the text never ties fan
   lifetime to being alive). Not yet encoded as a test; `clean()` resets fans
   only for the test harness.

## 11. What landed (2026-10-19)

1. This doc; exceptions filed as BACKLOG tickets (nyamu's undefined
   「失去2个火罐」 benefit stays `TODO(规则书)`; lisa's temp-pot 600-cost /
   two-turn expiry stays `TODO(规则书)[judgement]`).
2. **Fire**: kernel in `rules/bandori-std/src/fire.rs` (`declare`/`redeclare`/
   `overlay`/`end_overlay`, `gain`/`spend`/`adjust_cap` with actuals, `temp`
   as a second declared counter, `regime::{fill_to_cap, drain, burn_all}`).
   The face's 「（初始N，上限M）」 lives on its own `CardDef` (`.fire_pots(N, M)`
   → props `fireInitial`/`fireCap`); `fire_pot(player)` →
   `declare_own` reads them through `ctx::card_prop` (no body literals, no
   central table). Guests routed: two_in_one (`CardDef` re-declare), mutsumi_actor
   (fire overlay via the last-play latch; character
   cover still `TODO(规则书)[judgement]`), ave_mujica (regime verbs),
   kaoru_prince / uika_imprisoned (declare), yuri_crit (stops touching
   CHARGE), lisa_goddess (`fire::temp::grant`).
3. **Fans**: kind-owned host protocol in `rules/skills/skill-groups/src/fans.rs`
   (`mark:pp_fans` -- the host family lives in `rules/markers`; this holder-
   bound counter's protocol is the facade module, storage stays the wire
   keys). Once-per-match initial grant (`fans::initial_grant_once`) replaces
   pastel.rs's "lowest seat" dedupe; flip pre-raise preserved; `pastel.rs`
   clause （1） is a caller, clause （2） stays the per-instance listener.
4. **Purge** §6's inventory from `crates/` and `card-sdk` (see the §6 table).
5. Enforcement: the `fire_decl_is_on_the_skill_def` pin (source grep for
   stray `fire_pot(player, N, M)` / raw CHARGE / `tempFire` writes in
   skill bodies, and that each parenthetical face carries `.fire_pots`).
   Kani iff proofs not yet added (BACKLOG).

Relation to the parallel branches: `twodonuts-exile` (exile into
`rules/status`) and `status-hosts-rest` (stay/stun/noHand) are the conditions
family; this branch owns the quantities family end to end
(`bandori-std` fire kernel + charge, fans). Merge order:
conditions first, then this.