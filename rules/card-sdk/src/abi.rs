//! Host/guest ABI. Shared verbatim by the guest (ruleset.wasm) and the host
//! (`game-rules`). Bump [`ABI_VERSION`] on any incompatible change; the host
//! refuses rulesets built against a different version.

#[cfg(target_arch = "wasm32")]
use alloc::{borrow::Cow, boxed::Box, format, string::String, string::ToString, vec::Vec};
#[cfg(not(target_arch = "wasm32"))]
use std::borrow::Cow;

/// Increment on any change to imports, exports, or their semantics.
/// v2: `play_card` (cross-module card calls); one module per card.
/// v6: full `TriggerKind` set + `trig_target` / `trig_tile` / `trig_value`.
/// v7: board/hand/status query + op wave (is_buyable ... in_group, marker spend,
///     sweep_to_deck, trig_card_is; `Trigger.card`).
/// v8: `ruleset_cant_play` export (`CardDef.cant_play`) + `add_to_deck_at`.
/// v9: `cant_move`, `hand_size`, `is_deed`/`is_start`/`is_venue`.
/// v10: `trig_step` (`Trigger.step`) so a counteraction can tell which turn step
///      (0/1/2/3) it fired in, for kinds that aren't step-specific (e.g.
///      `mortgage` fires during both step 1 and step 3); paired
///      `*Before`/`*After` kinds filling in the missing half of every
///      trigger point, plus new hooks for buy/build/discard/end turn/leave.
/// v11: `trig_by_card` (`Trigger.by_card`) -- the player whose card caused this
///      trigger, or -1 when it was not card-caused. "Hit by another card" is
///      `by_card >= 0 && by_card != player_id`.
/// v12: `trig_set_pay_amount` -- a counteraction to a `pay`/`paid` trigger may rewrite
///      the pending amount (0 = cancel the payment).
///      Also: `set_move_roll` now actually reaches the engine (the write-back from
///      the counteraction's trigger to `Trigger.value` was missing).
/// v13: `trig_pay_is_rent` (`t.Pay.IsRent`) -- is this `pay`/`paid` trigger rent,
///      as opposed to a buy/build/forced loss. Card-driven payments are never rent.
/// v14: `t.Move` context -- `trig_move_flags` (a [`MoveFlags`] bitset), plus
///      `trig_move_main` and `trig_move_dir`. A move-caused trigger (moveRoll /
///      pass / settle*) can now say what kind of move it was, and any number of
///      orthogonal modifiers on it. Replaces the bespoke `trig_move_fire_roll`.
/// v15: `trig_set_cancelled` / `trig_cancelled` -- a
///      counteraction may negate the trigger's effect outright: the engine then
///      skips the effect body (land / event / play)
///      but still runs the point's Before/After hooks.
/// v16: `trig_set_pay_target` -- a counteraction may also redirect the payee of a
///      pending `pay` (`t.Pay.to`; -1 = the bank). The transfer amount
///      follows `set_pay_amount`, so a reduced payment credits the payee less too.
/// v17: field-card (`Fx`) hooks + per-card crystals. New `TriggerKind`s
///      (`TurnEnd`, `Drawn`, `PassTile`, `PayAfter`, `DiceRollAfter`, `CardPlayed`,
///      `Targeted`, `PaySplitChoose`) are **hook points**, not [反击] points: the
///      engine runs every *placed* card's `counteract` against them automatically.
///      `crystals` / `set_crystals` / `add_crystals` on the running field card.
/// v18: `take_from_hand`; `drawn` self-dispatches to the card named on `t.card`
///      (the one just drawn, still in hand).
/// v19: `cards_in(player_id, pile, buf, cap)` -- list a player's [`CardPile`] as a
///      postcard `Vec<String>` written into a guest-owned buffer (the first
///      host->guest string return). `take_card(player_id, pile, id)` replaces
///      `take_from_hand` and works on any pile.
/// v20: turn plan + scheduling -- `schedule_turn_end` (a card asks for a
///      `turnEnd` call at this turn's end or at the end of a player's next turn),
///      `set_no_money_loss`, `set_fixed_roll` / `fixed_roll`, `set_next_steps`,
///      `turn_main_steps`, marker-cap move, `card_replayable`.
///      `play_card` now returns the inner card's `Dest`, and the inner card runs
///      as itself (its own id and `Dest`, no longer the outer card's).
/// v21: `CardDef` standardized like triggers -- `id` + a table of `On` entry
///      points (`Play(gate, effect)`, `Counteract(kinds, guard, effect)`,
///      `Hook(kinds, effect)`, `AtEnd`). The manifest lists each entry with
///      its trigger kinds, so the host dispatches only to cards that declared
///      the kind at hand; one export `ruleset_on(card, entry, op, player_id)`
///      replaces `ruleset_play` / `_can_react` / `_react` / `_cant_play` (the
///      pre-v21 names, kept here as history).
/// v23: hook kinds for the remaining raise points -- TurnEndBefore / TurnEndAfter,
///      PaySplitAdd / PaySplitMul / PaySplitAt (the Money pipeline), Discarded, DeckBeforeGame /
///      DeckAtGameStart, Drew, Reshuffled, Bought, SettleBody, BeforeOut,
///      Teleported. Move payload `trig_move_remaining` / `trig_move_total` and
///      `MoveFlags::TELEPORT_WALK`. `schedule_turn_end` takes a mode (bit 1 =
///      the player's next turn, bit 2 = before the wear-off).
///      DiceRollAfter now fires before the moveRoll [反击] window.
///      `DiceRollPlan` is the point `On::RollPlan` (v22) is dispatched at.
/// v24: movement shaping -- `set_steps` / `set_reverse` / `set_signed` /
///      `set_stop_at` / `set_parity` / `set_resolve` / `set_settle_tile` /
///      `set_pay_factor` / `set_rent_factor` / `set_no_buy` / `set_can_build` /
///      `set_can_build` / `set_teleport_walk` / `set_min_roll` /
///      `set_extra_steps` / `set_more_steps` / `set_fire_roll` /
///      `set_no_circle_reward` / `set_settle_as_agent` and the `move_*` getters,
///      on the move being planned (guest: `ctx::plan::*`).
/// v25: card-driven abnormal grants / `teleport_to`
///      go through the abnormal gate: the `abnormalGuard` hook (new kind),
///      then the `abnormal` [反击] window when another player caused it; a blocked
///      effect does not apply. `abnormal_count(player_id)`,
///      `placed_tile(player_id, id)`, `play_doubled()` and
///      `trig_cards` (the drawn cards on a `drew` trigger).
/// v26: the targeting pipeline -- `target(player_id, tile, single)` pauses the
///      run with a `Target` host request (`target` / `target_tile`): the off-board kind,
///      the `immuneAll` / `untargetable` / `redirect` guard hooks (new kinds),
///      the `targeted_count` counter, then the `targeted` hooks
///      and the `target` [反击] window. `immuneAll` also gates card-driven
///      abnormal effects and payments. `sweep_to_deck`
///      became `shuffle_into_deck(player_id, hand, discard)`.
/// v27: the move model. `MoveFlags` is gone: a move is exactly one `MoveKind`
///      (Walk | Teleport), and what it resolves is a separate category,
///      `Settle` (ROUTE = the tiles it passes resolve 经过; DEST = the landing
///      resolves 结算). A teleport-walk becomes a Teleport whose destination
///      is computed from the roll; no-Resolve clears DEST. CHARGE_ROLL
///      is card-owned state now (`plan::set_tag` / `trigger::move_tag`), not an
///      engine flag. `ctx::card_move` / `ctx::agent_landing` run a move or an
///      agent landing from inside a card; the plan gained `set_kind` / `set_tag`
///      / `set_start` / `set_teleport_to` and the Base/Dice roll tables
///      (`set_base_dice` / `add_base_dice` / `add_extra_dice` -- a flat add is a
///      `0`-sided term, a flat add).
///      NOT yet in v27, though named in earlier drafts of this line: the play
///      context (`CardDef.targeting`, `trigger::play_*`, `set_immune`,
///      `add_mark_flags(.., NO_TARGET)`) and `plan::set_stopped`. Targeting is
///      the `HostRequest::Target` gate; the rest is unported.
/// v28: the [反击] vocabulary is `counteract` throughout. `On::CounterAct` /
///      `OnKind::CounterAct` / `Call::CounterAct` became `Counteract`, the
///      trigger-kind wire string `reacted` became `counteracted`, and the
///      guest entry points are `counteract` / `can_counteract` (formerly
///      `react` / `can_react`). Only the `counteracted` string is on the wire;
///      the rest is naming.
/// v29: two semantic breaks share this bump.
///      (a) **Band crystals are the band-skill field instance's crystals.**
///      `group_crystals` / `add_group_crystals` no longer touch the keyed state
///      `bandCrystals` (deleted, along with the old per-player store): they
///      are sugar over the player's band-skill field card (`skill:<band>:<skill>`,
///      `FieldCard::group_skill`) -- the same instance `crystals` / `add_crystals`
///      touch from inside a band skill's own handler, so 「乐队卡 / 团卡」 crystal
///      text is one pool. `max` = 0 now means uncapped (it used to clamp the
///      count to 0); `add_group_crystals` returns the new count (it used to
///      return the delta). A player with no band skill reads 0 and every write
///      is a no-op; a swapped or removed band card takes its crystals with it.
///      (b) the match-start / per-draw raise points (see the `TriggerKind`
///      notes on `DeckBeforeGame` / `DeckAtGameStart` / `Drawn` / `Drew`, and
///      `prop::NO_REWARD`).
/// v30: card-declared static **properties** ride the manifest -- a generic
///      `props` map of named `key -> i32` values on `ManifestEntry`, declared
///      by `CardDef::props`, replacing the engine's rulebook-prose matching
///      (「手卡上限数量减1」 / 「可在眩晕时打出」). The keys the engine reads are
///      named constants in [`prop`] (mirrored in `game_core::state::prop`):
///      `handLimitDelta` and the hand-play exemption. The host keeps them on `CardInfo`, the
///      engine queries `CardRules::card_prop(card, key)` (default 0) and
///      stamps the whole map onto the `FieldCard` at placement.
/// v31: tile rule instances (`docs/TILES.md`) -- `OnKind::Settle` (a rule's
///      settle body), the tile-data `prop` keys (`price` / `house` / `group` /
///      `buildMax` / `rentLen` / `rent:N` / `ringMult`), `place_card` on
///      `BOARD_OWNER`, and the `ctx::settle_*` primitives.
/// v32: the tile-bending surface replaces the engine's tile flags
///      (`docs/TILES.md` Phase 3). `tile_prop` / `set_tile_prop` read and write
///      a rule instance's props **by tile**; `settle_circle_reward` is the
///      经过 CiRCLE reward (`tile:circle`'s Pass entry). `prop::NO_REWARD`
///      replaces the `noCircleReward` plan flag and per-player state latch,
///      `prop::RENT_FACTOR` / `PAY_FACTOR` replace `rent_factor` / `pay_factor`,
///      `prop::BUY_DISCOUNT` / `FREE_BUY` / `RAZE_ON_BUY` replace the turn-ctx
///      buy knobs, `prop::NO_BUILD` replaces the `noBuild` state key, and
///      `prop::ANY_COLOR` / `prop::COLOR_FOR_PREFIX` replace the old per-player
///      colour override key. Board-owned instances now hear
///      the field hooks they declare (`passTile` &c.).
/// v33: `roll_ask` (a card-/skill-driven roll that raises the `Roll` chain
///      link -- the 「掷骰结算前」 [反击] window -- with the roller, the face and
///      a `t.Roll.Source` code) and `trig_roll_source` /
///      [`roll_source`]`::{NONE,CHARGE,CARD,SKILL}`. `ctx::roll` is unchanged
///      (no window); a roll that [反击]s must answer goes through
///      `ctx::roll_ask` / `ctx::do_move_roll_ask`.
/// v34: `prop::RENT_HOUSES` (virtual rent-house-count, 「房屋数视为…」 -- gone
///      with a placed card) and `ctx::rent_houses_of` (the counted value the
///      rent lookup reads). `ctx::houses_of` remains real.
/// v35: the skill / band-skill / follow surface.
///      (a) `invoke_skill(player_id, id)` -- run a skill rule's press entry
///      (`On::Play`) for a player, nested like `play_card`. This is 「立即执行
///      乐队技能的（2）效果」 (mutsumi_never) and 「触发其技能的发动」 (pareo_far):
///      the card runs the skill's press entry directly. No `skillUsed` raise --
///      that is the player's
///      own press (`use_skill`), not a card running the body.
///      (b) the skill **attachment surface**:
///      `group_skill(player_id)` / `persona_skill(player_id)` name the bound
///      rule id, `group_skills(player_id)` lists every band attachment as
///      `(uid, id, extra)`, `add_group_skill(player_id, id, extra)` attaches one.
///      `extra` is 「拿取」's borrowed copy: 「相同乐队技能卡
///      的效果不可叠加」 (the hook dispatch skips an extra when a non-extra copy
///      of the same id is already attached) and 「不视为那个乐队的角色」
///      (`in_group` still reads only the character).
///      (c) `raise_bought(player_id, tile)` -- a card that hands a deed over
///      (tomoe_savior's 「从该玩家处收购该地契」) announces the acquisition so
///      the `bought` hook chain hears it.
///      (d) `plan::add_follower(player_id)` -- 「使你的下次主要移动结果对那些
///      玩家一起执行」 (sakiko_lead): the move's result is replayed for each
///      follower after the mover settles, in the recorded order (「你先触发结算，
///      此后其他玩家按行动顺序依次触发结算」).
/// v36: [CP点] is its own tile-mark category (`mark::CP_CATEGORY` / `mark::CP_KIND`),
///      owned by the `mark:cp` rule instance on the neutral board owner and never
///      by a player. The small writer API is `ctx::place_cp` / `ctx::count_cp` /
///      `ctx::clear_cp` plus the attached counts (`ctx::count_cp_from` per tile,
///      `ctx::cp_attached` in total) -- provenance is the placing card instance
///      (`TileMark.src`), not `TileMark.owner`. `HookKind::CpChanged` /
///      `TriggerKind::CpChanged` (`cpChanged`) fires whenever a card instance's
///      attached [CP点] count is written, so 「…时」 clauses on the count (the
///      该清CP了 graveyard rule) live in one event handler instead of at each
///      spend site. Mirrors v29's `crystalsChanged`.
/// v37: event rule instances (`docs/EVENTS.md`) -- the `rules/events` category.
///      `ctx::event_expire` / `event_is_active` / `event_deck_push` /
///      `event_banish` are the rule body's handles on the engine's event deck
///      and active list. An event's `CardDef` id is `event:<id>` and it is
///      bound on the neutral board owner (`BOARD_OWNER`) with `tile = -1` for
///      as long as it is active, the same shape as a `tile:*` instance.
/// v38: the two [CP点] kinds (user ruling 2026-10-07). **Tile marks** remain the
///      `mark:cp` owner's (`ctx::place_cp` / `count_cp` / `count_cp_from` /
///      `clear_cp`, plus `ctx::cp_src_at` for the mark's provenance lookup).
///      **On-card** [CP点] is `FieldCard::cp` -- 「自己[场上]N个[CP点]」, the CP
///      points attached to the card itself (the card rule's own stock,
///      crystals-like) -- written with `ctx::add_cp` / `add_cp_at` and read
///      with `ctx::cp_attached` (the running instance) / `cp_at` (another
///      instance). `cp_attached` changed meaning in place: it used to total
///      the tile marks carrying the instance as `src`, it now reads
///      `FieldCard::cp`. `HookKind::CpChanged` rides the on-card writes only.
///      The per-player 「自己[场上]」 counter `mark::CP_FIELD_TOK` is gone: the
///      card's own count replaces it.
/// v40: the **purchase surface** (`docs/PURCHASE.md`). `TriggerKind`s
///      `BuyGate` / `BuyAdd` / `BuyMul` / `BuySet` / `BuyAssign` (wire names
///      `buyGate` / `buyAdd` / `buyMul` / `buySet` / `buyAssign`) and the
///      [`BuyKind`] payload (`buy_kind` / `seller` / `price` / `deal_*`).
///      New tile props: [`prop::BUYABLE`], [`prop::BUY_HOUSES`],
///      [`prop::FORCE_MULT`] / [`prop::FORCE_FIXED`] /
///      [`prop::FORCE_STAYS_MORTGAGED`], [`prop::ANY_COLOR`] and the
///      `colorFor:` prefix. `ctx::buy_quotes` / `ctx::buy` / `ctx::acquire` /
///      `ctx::agent_offer` / `ctx::linger`. The retired props
///      `BUY_DISCOUNT` / `FREE_BUY` / `RAZE_ON_BUY` remain for one ABI (P5
///      deletes them and the lingering-turn flags together). SAVE_VERSION 3 → 4
///      (the lingering-instance field enters the save).
/// v41: the **removals**. `ctx::set_buy_discount` / `ctx::set_free_buy` /
///      `ctx::set_raze_on_buy` and the fields they wrote are gone,
///      replaced by `ctx::linger` + the `BuyAdd` / `BuyMul` / `BuySet` /
///      `BuyAssign` hooks. The old global / per-player colour writers and
///      their state key are gone, replaced by the `prop::ANY_COLOR` /
///      `prop::COLOR_FOR_PREFIX` tile
///      props. The retired props `BUY_DISCOUNT` / `FREE_BUY` / `RAZE_ON_BUY`
///      go with them.
/// v42: the payment command's **pre-split** stage and the terminal hooks
///      (`PIPELINE-AUDIT` Q2 / Q6). `TriggerKind`s `PayCombinedAdd` / `PayCombinedMul`
///      / `PayCombinedCancel` (wire names `payCombinedAdd` / `payCombinedMul` /
///      `payCombinedCancel`) run command-wide on the figure **before** any
///      「[分摊]」 divides it -- 「分摊前」 -- alongside the existing per-share
///      `PaySplitAdd` / `PaySplitMul` / `PaySplitChoose` / `PaySplitAt`. `ctx::pay_total` /
///      `ctx::split_pay` / `ctx::transfer_leg` drive them. Terminal
///      `<thing>Resolved` hooks: `tileResolved` (after `settleAfter`),
///      `moveResolved` (after a move and its settle), `bankruptResolved` (after
///      the leftover auctions). `HostRequest::Pay` carries `total_stage`, and
///      `HostRequest::PayTotal` is new. SAVE_VERSION unchanged (no save field).
/// v43: the **move-head / move-tail** pair (`SETTLE-STAGES.md` §7). `TriggerKind`s
///      `MoveBefore` / `MoveAfter` (wire names `moveBefore` / `moveAfter`).
///      `moveBefore` fires for every move -- walk or teleport, main or
///      card-driven, settling or not -- once its plan is fixed and before the
///      first step / the teleport; counteractions that cancel or alter the move
///      belong here (so it is a [`ChainKind`] too). `moveAfter` fires after
///      `passPlayer`, before `settleBefore`, for every completed move --
///      including a 「不触发结算」 one (only the settle stages are skipped);
///      「移动后」/「主要移动结束时」/「移动终点」 clauses land here. The
///      roll-specific `rollPlan` / `moveRoll` and the teleport-specific
///      `teleport` / `teleported` remain where they are; `moveResolved` remains the
///      final terminal after any settle. SAVE_VERSION unchanged.
/// v45: guard **condition** per guarded entry (docs/GUARDS.md G0). `On::Play` /
///      `On::Counteract` / `On::Hook` gain a trailing `pre: &'static str`
///      (`""` = none) and `ManifestOn` gains `pre: Option<String>` (the CEL
///      source, compiled host-side at ruleset build). No behavioural change
///      while no entry declares one. SAVE_VERSION unchanged (no save field).
/// v46: deleted guards + legacy audit (GUARDS.md G3/G4). `On::Counteract` /
///      `On::Hook` guards become `Option<fn>` (`None` = G4-deleted residual);
///      `ManifestOn` gains `has_guard` / `has_legacy`; `CardDef` gains a
///      `legacy` table and `export::OP_LEGACY_GUARD` (= 2) returns the
///      pre-migration guard for the `guard-audit` equivalence check. SAVE_VERSION
///      unchanged (no save field).
/// v47: opt-in field counteractions and mutually exclusive choice groups;
///      these share the hand-counteraction window without spending a field card.
/// v48: shared movement counteractions receive the preselected extension in
///      `COUNTERACT_MOVE_EXTENSION`; distance is chosen before its payment.
/// v49: **every** entry kind carries the guard condition (docs/GUARDS.md G0).
///      `On::Gate` / `On::AtEnd` / `On::RollPlan` / `On::Settle` gain the same
///      `pre: &'static str` + `Option<fn(i32) -> bool>` residual-guard pair
///      `On::Hook` has (so `On::Gate(kinds, pre, guard, body)` and
///      `On::RollPlan(pre, guard, body)` &c.). The host evaluates category →
///      condition → guard → body and fires the card flash only at body entry,
///      exactly as for hooks: an entry whose condition rejects runs no body and
///      emits no `card` event. `ManifestOn` is unchanged -- `pre` / `has_guard`
///      were already there and simply start carrying these kinds' values (a
///      pre-v49 module lied `has_guard: true` for them; v49 reports
///      `g.is_some()`). The window variables meaningful per kind are
///      documented in `docs/GUARDS.md` §4.2c. SAVE_VERSION unchanged (no save
///      field).
/// v50: the tile-body guest primitives (`rules/tiles/src/agent.rs` /
///      `circle.rs`). `ask_tiles` / `opt_tile` / `opt_price` / `opt_ai` -- the
///      tile ask with per-option labels, prices and an AI choice hint
///      (the agent-landing ask's `ask.view.prices` + `with_ai`); `PromptKind::TileId`
///      records the **tile id** (or -1 for 「不选」) in the answer log, so a
///      commit-pass re-run of a body whose options the just-committed host
///      effect reshaped still maps the pick to the same tile. `agent_offer` --
///      the chosen branch (buy `0` / build `1` / half-rent `2`) as one host
///      routine. `raise` --
///      guest-raised trigger points (`passRewardAffected` from `tile:circle`);
///      returns whether the link settled. `gain_typed` -- a bank print through
///      the money pipeline (`Pay::new(_, "gain")`) carrying the Pay's event
///      `typ` and a replacement log line (`Pay::text`). `prop_at` /
///      `set_prop_at` (one declared property of a field instance), layer counters
///      (`[除外]` layers, mirroring the other layer counters), `ai_agent_choice` (the AI's
///      preferred index among agent-offer tiles), `trigger::move_from`
///      (`t.Move.From`, the 移动起点). `ctx::agent_landing` and
///      `ctx::settle_circle_reward` are **removed** -- the tile bodies carry
///      the logic (`land_at_built_in` / `Play::circle_reward` remain the
///      `StubRules` fallback). SAVE_VERSION unchanged (no save field).
/// v51: **named counters + bound units + cross-card messages** (user ruling
///      2026-10-10). Every tile mark and player token is a unit of some card
///      instance's named counter, bound to a tile or a holder; destroying the
///      instance destroys its units. The two on-card counters keep their wire
///      names ([`counter::CP`], [`counter::CRYSTALS`]); anything else is a
///      name in `FieldCard::counters`. One `CounterChanged` hook replaces
///      `CpChanged` / `CrystalsChanged` (carries the counter name + signed
///      delta). The tile-mark surface is a filter
///      ([`MarkFilter`]) over `place_mark` (with [`Stack::Merge`] /
///      [`Stack::Fresh`]) / `count_marks` / `bump_mark` / `remove_marks` /
///      `mark_src_at` -- the CP helpers are gone. `On::Message` + `ctx::send`
///      is the generic cross-card channel (该清CP了 -> `mark:cp` places CP
///      units; the landing hook spends the placer's on-card CP). `tok` /
///      `add_tok` / `set_tok` become the holder-binding verbs of the creating
///      instance's counter. SAVE_VERSION 5 → 6 (marks/tokens carry
///      `instance`).
/// v52: `ManifestOn::label` -- an entry's rulebook clause label (「（1）」),
///      carried so the host's 「效果适用」 line can name which clause of a
///      multi-entry card ran. Filled from the `On` entry itself
///      (`On::label`, which moves with the entry); `None` = unlabelled.
///      Opaque display text -- nothing resolves it against the card text;
///      if the line needs prose, the entry carries a copy of it.
///      No host-function change; SAVE_VERSION unchanged.
/// v53: **purchase replies tell the truth** (HOST-06). `card_buy` / `buy` /
///      `acquire` / `agent_offer` / `card_build` / `card_offer_build` /
///      `card_mortgage` / `raise_bought` answer `1` only when the engine
///      actually committed (deed assigned / house raised / deed mortgaged /
///      hand-over landed) and `0` on every refusal (`BuyGate` / `buyBefore` /
///      `buildBefore` cancel, already owned, unpaid quote or money,
///      `why_not_build_on`, max houses). They used to answer `1`
///      unconditionally. `buy_quotes` (HOST-07) is the same hook-aware quote
///      `buy` charges (`BuyGate` / `BuyAdd` / `BuyMul` / `BuySet`), no longer
///      the native land+houses figure. `plan::set_no_build` /
///      `plan::set_build_anywhere` (HOST-08) have host bodies: `set_no_build`
///      is `set_can_build(false)` on the move being planned, and
///      `set_build_anywhere` opens builds past the landing-only rule. Dead
///      surface removed: the `set_bonus` import (HOST-09) and five unused
///      host linker names (HOST-10). No wire shape change for live imports;
///      SAVE_VERSION unchanged.
/// v54: `ManifestOn::presentation` -- how an entry shows up in the log.
///      `Visible` (default) flashes and writes a 「效果适用」 header.
///      `Silent` (`CardDef::silent`) is bookkeeping: runs its body, posts no
///      flash and no header (declare_cap / set_bounds / initial values /
///      latch clears). `QuietWhenEmpty` (`CardDef::quiet_when_empty`) runs
///      normally but **retracts** its header when the body produced nothing
///      visible (no children, no logs): the clause applied and changed
///      nothing (capped gain, 「已满」), so the player never sees a flash.
///      If the body produced anything, or the drive halted for a prompt (the
///      header is already on the stream), it behaves like `Visible`.
///      Nested activations announce normally. No host-function change;
///      SAVE_VERSION unchanged.
/// v55: `ManifestOn::flash` -- a declarative, per-entry [`Flash`] policy beside
///      the header mode: `Auto` (host classifies by scope, owner-self-only =>
///      no flash), `Always` (the rule says this entry always flashes, e.g. a
///      self-only effect the player must notice), `Never` (log entry remains,
///      no flash/pulse). Authored via `CardDef::flash`. The header modes of
///      v54 are unchanged; `CardDef::presentation` is the one lookup that
///      folds header + flash into [`EntryPresentation`].
///      Imperative flash from the body (like `ctx::log`): new host imports
///      `flash` / `no_flash` / `flash_card` / `flash_tile` /
///      `flash_card_keyed` (see `ctx::flash` and kin). Precedence:
///      `Never` > `ctx::no_flash` > `ctx::flash` / `Always` > `Auto`-by-scope.
///      A proactive `"flash"` event is parent-linked under the activation and
///      plays through the client's cardFlash path.
///      **Once-per-flash identity** (user, 2026-10-11): a flash is identified
///      by `(drive id, flash ordinal within the drive)` or an explicit key via
///      `ctx::flash_card_keyed`. Once posted to the stream, a re-run that
///      reaches the same identity MUST NOT post it again (unlike logs, which
///      replace-by-index and drop on halt). Identity lives in the drive's
///      persistent state and survives halt; the activation's own flash obeys
///      the same rule. Client-side, event ids dedupe a re-delivered frame.
///      SAVE_VERSION unchanged.
/// v56: **generic tile queries** replace the eight game-named kind/class host
///      ops (`is_deed` / `is_start` / `is_agent` / `is_shop` / `is_venue`
///      / `is_buyable` / `is_venue_for` / `is_color`). New imports:
///      `tile_is(tile, kind)` -- data `TileKind` by wire spelling;
///      `tile_attr(tile, attr)` -- i32 field read (`group`/`area`/`tier`/
///      `price`/`house`/`rent`/`index`);
///      `tile_in(player, tile, class)` -- ruleset class membership
///      (`buyable`/`shop`/`livehouse`/`color:<g>`), with per-player overrides
///      as generic class-membership props (`class:` / `classFor:<p>:`) plus the
///      existing `anyColor` / `colorFor:<p>` colour props. Game sugar
///      (`is_deed(t) = tile_is(t, "ring")`, …) lives in `bandori-std`.
///      `tile_group` / `tile_price` remain as generic field sugar.
///      Guest ABI otherwise unchanged; SAVE_VERSION unchanged.
/// v65: **import-module rename** -- the wasm import module and every guest
///      export / native `link_name` drop the game prefix: module `"bandori"` →
///      `"ruleset"`, `bandori_<name>` → `ruleset_<name>` (`ruleset_on`,
///      `ruleset_metadata`, `ruleset_abi_version`, `ruleset_manifest`, and
///      every `link_name = "ruleset_*"` host import). The Rust macros become
///      `ruleset!` / `ruleset_card!`. Wire-visible names change, so old modules
///      do not load on a new host (replays use archived engine bundles).
///      Semantics of every import/export are unchanged. SAVE_VERSION unchanged.
/// v64: **turn-clock props** -- `prop::BASE_TIME_SET` / `prop::BONUS_TIME_SET`
///      on a board-owned instance override the host turn clock (`base_time`
///      = 恢复时间, `bonus_time` = 免费时间) as each turn starts (元祖！邦多利酱).
///      Also the turn-clock **modifier hooks** `TriggerKind` / `HookKind`
///      `{TurnBonusTime = 99, TurnBaseTime = 100}`: raised as each turn starts
///      with `value` = the seconds the host would use (免费时间 refill / 恢复时间);
///      a hook rewrites it with `trigger::set_pay_amount`. The props then
///      override the hooked result. No new import, no SAVE_VERSION change.
/// v63: **roster metadata export** (`docs/CARDS.md` 「Card metadata」).
///      `CardDef::meta` carries the card's data-set row (name / rawName /
///      text / tags / derived / cell + a generic `fields` map for game slots
///      `band` / `owner`); the skill crates' `PersonaDef` / `GroupDef`
///      tables carry the character / band rows. A new guest export
///      `ruleset_metadata` (`export::METADATA`, `() -> i64` packed
///      postcard `meta::MetaDoc`) hands the whole roster to the host on
///      request -- **not** through `ManifestEntry` (the match path is
///      unchanged and unbloomed). Rows sort by `(kind, ordinal)` where the
///      ordinal comes from a shared generated table
///      (`bandori_std::roster_order`) so the rebuilt vectors equal the JSON
///      file order exactly even though the Rust crates register alphabetically.
///      Host: `Ruleset::metadata()` -> `meta::MetaDoc`; `GameData::load`
///      consumes the converted `Roster`. No host-function change; SAVE_VERSION
///      unchanged (no save field).
/// v62: **static card property + guest query** -- `prop::SONG` (`"song"`,
///      default 0): a song-title card marker (「卡名为歌名的卡」, Kizuna
///      Music's X). Declared with `.props(&[(prop::SONG, 1)])` on exactly the
///      twenty song cards; rules read it back through the new host import
///      `card_prop(id, key) -> i32` (`ctx::card_prop`), which is the generic
///      guest face of `CardRules::card_prop` (default 0). No game vocabulary
///      in the import. SAVE_VERSION unchanged.
/// v61: **legacy guard-audit scaffolding removed** (docs/GUARDS.md §5.1/§10
///      finished 2026-10-14). `CardDef::legacy` / `.legacy(...)`,
///      `ManifestOn::has_legacy`, `export::OP_LEGACY_GUARD`, the `guard-audit`
///      host feature, `cond_pre::legacy_audit` and `legacy_probe` are gone --
///      the G3 equivalence check ran once at removal time and the pre-migration
///      copies were deleted. `ManifestOn` loses `has_legacy` (postcard is not
///      self-describing, so this is the bump); guest wasm must be rebuilt with
///      the host. SAVE_VERSION / RECORD_VERSION unchanged (no save field).
/// v60: **exclusive-use gate** -- `TriggerKind::ExclusiveUse = 98` (also
///      `GateKind`): a CEL-only `On::Gate` question the band-skill cards answer
///      (CRYCHIC's 「variants may not use the base character's exclusives」).
///      The engine's default verdict is the generic exclusive rule (playable
///      iff `card.owner` is the actor's character or its base `variant_of`);
///      a closing gate entry denies the base case. Window: `actor`,
///      `trigger_card` (the exclusive's id hash), `name` (the card's `owner`
///      character name; `counter_is`), `value` (1 = owner is the actor's base
///      character), `step` (1 = owner is the actor's own character). No new
///      host import, no SAVE_VERSION change.
/// v59: `ManifestEntry::texts` -- per-entry **tile text** declarations
///      (`CardDef::texts`): the deed-popup description an entry speaks for,
///      as `(entry, slot, tile, key)`. `slot` is [`text_slot`] (settle /
///      pass); `tile` is `""` = the instance's own tile or a board tile name
///      for a tile-bound text on a tile-less instance; `key` is a `ui`
///      namespace i18n key (`deed.note.*`), `""` = suppress the slot. The
///      view composes per-tile speakers in body-precedence order (docs/TILES.md
///      「Tile text」). Trailing field on `ManifestEntry` (postcard is not
///      self-describing, so this is the bump); `MatchState::tile_texts` is a
///      view-only field and needs no SAVE_VERSION change.
/// v58: **status gates** -- `TriggerKind::{ActGate = 95, PayGate = 96,
///      MoveGate = 97}` (also `GateKind`): CEL-only `On::Gate` questions the
///      `status:*` pseudo cards answer; the host evaluates the condition
///      natively (`ctx::can_pay`'s host body asks `PayGate`). No new host
///      import, no SAVE_VERSION change.
/// v57: **closeout integration** -- one monotonic bump covering every addition
///      the five closeout batches made on top of v56:
///      `TriggerKind::ChargePaying = 78` / `HookKind::FlipEffect = 94` (fire-cost
///      substitution + flip-effect hook; `ctx::raise` is prompt-aware;
///      FlipEffect retired in v83 -- id 94 never reuse);
///      `join_redirect_target` / `set_join_used` (真奈 join retarget, CH-05);
///      and **raw money adjustment** -- `ctx::adjust_money` / `ctx::gain_raw` /
///      `ctx::lose_raw` (`HostRequest::AdjustMoney`, wire name
///      `adjust_money`). `money += delta` with a `log.gain` / `log.lose` line
///      and **no** payment pipeline: no `paySplitAdd` / `paySplitMul` / `paySplitChoose` /
///      `paySplitAt`, no `effect` [反击] window, no `payAfter`. 规则书 「不受其他
///      任何效果影响」 wants a [指定] split's amount to land as-is (ONE OF US's
///      「被分享方资金直接增加，不受其他任何效果影响」). Non-pausing. Positive
///      `delta` is a gain, negative a loss. `World::gain_money` / a raw
///      decrement under the hood. SAVE_VERSION unchanged by this batch (the
///      engine batch's SAVE_VERSION 6 → 7 is independent).
/// v66: **generic engine vocabulary** (phase 2, tiles). The engine treats
///      tile kinds as opaque string ids; rule meaning comes from the ruleset.
///      `CardDef::board_role` (`""` / `"tile"` / `"mark"` / `"status"`) and
///      `CardDef::governs` (tile kinds a `tile` rule settles) ride
///      [`ManifestEntry`] as trailing fields (postcard is not self-describing,
///      so this is the bump). `game-core` no longer carries a kind→rule table
///      or named tile-kind enum variants; `CardRules::tile_rule_id` /
///      `mark_rule_ids` / `status_rule_ids` supply the lists. Generic dice
///      rent: [`prop::RENT_DICE_MULT`] on a tile rule instance means
///      `count_owned_kind(owner, kind) × mult × 1d20` instead of the rent
///      table. No SAVE_VERSION change
///      (TileKind still serializes as the same raw string).
/// v67: **roster persona/group names** (phase 2). MetaRow kinds `"persona"` /
///      `"group"` (was `"character"` / `"band"`); meta key `"group"` (was
///      `"band"`). Player/field wire names `persona` / `skillOwner` /
///      `affiliations` / `groupSkill`. Host helpers `group_*` / `persona_*`
///      (was `band_*` / `character_*`). SAVE_VERSION 9 → 10 (persisted
///      player/field keys renamed). RECORD_VERSION unchanged.
/// v68: **charge resource + named counters + rule params** (phase 2). State
///      key `"charge"` (was `"fire"`); host imports `ruleset_charge_*`;
///      `MatchRulesData.rule_params` map (was `ringMultiplier`) with the
///      dice-rent unit under `rentUnitMult`; host `ruleset_rent_unit_mult` /
///      `ruleset_add_rent_unit_bonus` (was `ring_*`); `FieldCard` named
///      counters only (`counters` map, no `cp`/`crystals` fields);
///      `ruleset_group_pool(player, name)` (was `group_crystals`);
///      `StateVar.flags` for never-negative / engine-owned; trigger
///      `chargePaying` (was `firePaying`). SAVE_VERSION 10 → 11 (state key,
///      FieldCard shape, profile charge fields).
/// v69: neutral pass-reward / deed wire names. TriggerKind/HookKind/ChainKind
///      `circleAffected` -> `passRewardAffected` (numeric 26 unchanged; the
///      old spelling still parses). CEL `is_circle`/`is_ring`/`is_live_house`
///      -> `is_start`/`is_deed`/`is_venue` (old names remain aliases).
///      i18n keys `ask.circle.*` / `log.circle_*` -> `ask.pass_reward.*` /
///      `log.pass_reward_*` / `log.land_start`. SAVE_VERSION unchanged.
/// v70: **rule-defined kind ids** (phase 3). `TriggerKind` / `HookKind` /
///      `ChainKind` / `GateKind` are transparent `i32` newtypes; engine-generic
///      raise points are associated consts. Game-named kinds
///      (`PassRewardAffected` 26 / `FireSpent` 73 / `SkillUsed` 74 /
///      `HouseAdded` 75 / `ChargePaying` 78) leave the SDK and live as
///      ruleset-declared opaque ids (`bandori-std::kind`). Custom ids
///      serialize as decimal (`"26"`); no legacy aliases (`circleAffected`,
///      `crystalsChanged`, `firePaying`, …). CEL `is_start`/`is_deed`/
///      `is_venue`/`is_buyable` sugar removed in favour of `tile_is`/`tile_in`/
///      `tile_attr` only. SAVE_VERSION unchanged.
///      (Superseded in v90: the three *universal* raises of that batch --
///      marker-spend commit, field-card activation, house-count write -- moved
///      to the engine namespace as `markerSpent` / `cardUsed` / `houseAdded`.)
/// v71: **money rounds once after add/mul** (user ruling 2026-10-15,
///      OPEN-取整 family). Modifier cards (`减半` / `1.5倍` / `+x` / `×y` /
///      `paySplitAdd` / `paySplitMul` / `payCombined*` / `set_pay_factor`) transform the
///      running amount exactly and must not CeilTo-10 inside their own body;
///      `money_inner` / `pay_total` round a positive figure **up to a multiple
///      of 10 once** after every add/mul modifier has been processed. An
///      amount no modifier touched is left as the base formula produced it.
///      Printed 「向上取整10」 sites (agent half-rent, `[分摊]` share formula,
///      `HHW:爱心义演`) are that end-of-calc round and keep their CeilTo.
///      No new import; host semantics of `set_pay_amount` composition change.
///      SAVE_VERSION unchanged (no save field).
/// v72: **persona write** -- new host import `set_persona(player_id, name)`
///      writes `MatchPlayer::persona` (the name `persona_is` matches). Used by
///      character-swap cards (Two-in-One 「替换为sumimi的另一名角色」) so the
///      exclusive gate and `persona_is` see the new identity; skills are
///      swapped separately. SAVE_VERSION unchanged.
/// v73: **deed bodies** -- `OnKind::Mortgage` (9) / `OnKind::Redeem` (10)
///      (`On::Mortgage` / `On::Redeem`, same pre/guard/body shape as
///      `On::Settle`). A tile rule owns mortgage/redeem eligibility
///      (规则书 基础规则 5–6) and the 50% / 60% amounts, and calls
///      `ctx::apply_mortgage` / `ctx::apply_redeem` for the flag write +
///      direct money move (hooks + logging remain in that mechanism).
/// v74: **Action / Filter split** (WordPress `do_action` / `apply_filters`).
///      `On::Hook` is removed (no alias). `OnKind::Hook = 3` is renamed
///      `OnKind::Action` and **keeps id 3** (old compiled modules remain
///      loadable). New `OnKind::Filter = 11` (11, not 9 -- 9/10 are the v73
///      deed kinds) carries the modifier-pipeline / value-shaping kinds
///      (`TriggerKind::is_filter`). A Filter may only list filter kinds and an
///      Action only non-filter kinds; the check runs at ruleset build
///      (`On::check_kind_variant`). SAVE_VERSION unchanged.
/// v75: **pure Filter bodies**. `On::Filter` becomes
///      `fn(player_id, value) -> i32`; `On::FilterFlag` is
///      `fn(player_id, value) -> bool` (false = cancel/substitute);
///      `On::FilterValue` is `fn(player_id, FilterValue) -> FilterValue`.
///      Bodies may not prompt or emit mutating ctx effects; the host folds the
///      return. New filter kind `TriggerKind::DiceRollValue = 102` is raised
///      immediately before `DiceRollAfter` for roll reshaping (DiceRollAfter remains a
///      pure Action event). SAVE_VERSION unchanged.
/// v76: **DiceRollPlan / AtEnd fold**. `On::RollPlan` is
///      `On::FilterValue(&[TriggerKind::DiceRollPlan], …)`; `On::AtEnd` is
///      `On::Action(&[TriggerKind::AtEnd], …)` (one-shot via the existing
///      `schedule_turn_end` list). `OnKind::RollPlan = 5` / `OnKind::AtEnd = 4`
///      are retired -- never reuse those wire ids. New kind
///      `TriggerKind::AtEnd = 103` (wire `atEnd`). SAVE_VERSION unchanged.
/// v77: **Gate fold**. `On::Gate` is `On::FilterFlag` (yes/no: true = allowed)
///      or `On::FilterValue` (target / reason). All gate kinds join
///      `TriggerKind::is_filter()`. `OnKind::Gate = 6` is retired -- never
///      reuse. CEL-only status gates keep their native `admits_pre` /
///      `gate_open` path (condition is the whole answer). SAVE_VERSION unchanged.
/// v78: **purity + interactive pre-stages**. New kinds `FlipAfter = 104`
///      (post-flip Action; retired in v83 -- id 104 never reuse),
///      `PaySplitChooseBefore = 105` / `PayCombinedAddBefore = 106`
///      (interactive Action pre-stages raised immediately before the pure
///      `PaySplitChoose` / `PayCombinedAdd` filter stages, restoring the rulebook
///      prompt position). FilterValue gains an interned `tag_key` / `reason`
///      id and a host-supplied `recorded_roll` so dice-rolling Filter bodies
///      become pure. `card_sdk::pure::refuse_if_filter` covers every mutating
///      ctx call. SAVE_VERSION unchanged.
/// v79: **`Roll*` kind family renamed to `DiceRoll*`** (numeric ids unchanged).
///      `Roll(1) -> DiceRollBefore`, `RollAfter(49) -> DiceRollAfter`,
///      `RollPlan(67) -> DiceRollPlan`, `RollValue(102) -> DiceRollValue`.
///      `MoveRoll(2)` is **not** renamed (it is a [反击] ChainKind, a different
///      mechanism from Action/Filter). Wire names change too (`roll` ->
///      `diceRollBefore`, `rollAfter` -> `diceRollAfter`, `rollPlan` ->
///      `diceRollPlan`, `rollValue` -> `diceRollValue`) -- those strings ride
///      logs, the CEL kind vocabulary, and `anim.chainKind.*` locales, so this
///      is a version bump. Hard rename: no aliases (old records replay on
///      archived engines). SAVE_VERSION unchanged.
/// v80: **buy / build bodies** -- `OnKind::Buy` (12) / `OnKind::Build` (13)
///      (`On::Buy` / `On::Build`, same pre/guard/body shape as
///      `On::Mortgage` / `On::Redeem`). The body owns the order of steps,
///      eligibility and the quote, and calls generic mechanism primitives
///      (`ctx::raise` dispatching on `TriggerKind::is_filter`, `ctx::pay_buy` /
///      `ctx::pay_build` through the money pipeline, `ctx::assign_deed`,
///      `ctx::log`). `buyGate` is a `FilterFlag`, `buyAssign` a `FilterValue`
///      (deal owner/houses/mortgaged); `buyBefore` / `bought` / `buyAfter` /
///      `buildBefore` / `houseAdded` / `buildAfter` remain Actions. Force-buy
///      money remains a direct transfer outside effects (规则书 其他规则 6).
///      Ids 12/13, not 4/5/6 (retired) or 11 (`Filter`). Renumbered once past
///      the v79 DiceRoll* rename (the buy/build branch also claimed v79).
///      SAVE_VERSION unchanged.
/// v81: **`MoveRoll` kind renamed to `DiceRollResult`** (numeric id 2 unchanged).
///      Completes the v79 `DiceRoll*` family: `MoveRoll(2) -> DiceRollResult`
///      so every roll kind shares the `DiceRoll*` prefix
///      (`DiceRollBefore(1)` / `DiceRollResult(2)` / `DiceRollPlan(67)` /
///      `DiceRollAfter(49)` / `DiceRollValue(102)`). Wire name changes too
///      (`moveRoll` -> `diceRollResult`) -- that string rides logs, the CEL
///      kind vocabulary (`kind == DiceRollResult`), and `anim.chainKind.*`
///      locales, so this is a version bump. Hard rename: no aliases (old
///      records replay on archived engines). Still a [反击] ChainKind
///      (counteraction window), NOT an Action/Filter kind and still absent
///      from `TriggerKind::is_filter()`. The host/ctx API names
///      `set_move_roll` / `do_move_roll` / `trigger::move_roll` / CEL
///      `move_roll` are **not** kind names and are unchanged.
///      SAVE_VERSION unchanged.
/// v82: **`Pay*` kind family renamed to Combined/Split levels** (numeric ids
///      unchanged). Names name the pipeline LEVEL, not whether a 「[分摊]」
///      split happened: a payment COMMAND (e.g. 「其他玩家[分摊][支付]2000」)
///      first runs the Combined stages on the WHOLE figure before 「[分摊]」
///      divides it, then each settled leg/share runs the Split stages. A
///      single non-split payment runs both levels once each.
///      `PayTotalAdd(84) -> PayCombinedAdd`, `PayTotalMul(85) -> PayCombinedMul`,
///      `PayTotalCancel(86) -> PayCombinedCancel`;
///      `PayAdd(55) -> PaySplitAdd`, `PayMul(56) -> PaySplitMul`,
///      `PayChoose(52) -> PaySplitChoose`, `PayAt(57) -> PaySplitAt`;
///      pre-stage prompts `PayTotalAddAsk(106) -> PayCombinedAddBefore`,
///      `PayChooseAsk(105) -> PaySplitChooseBefore`.
///      Wire names change too (`payTotalAdd` -> `payCombinedAdd`,
///      `payTotalMul` -> `payCombinedMul`, `payTotalCancel` -> `payCombinedCancel`,
///      `payAdd` -> `paySplitAdd`, `payMul` -> `paySplitMul`,
///      `payChoose` -> `paySplitChoose`, `payAt` -> `paySplitAt`,
///      `payTotalAddAsk` -> `payCombinedAddBefore`,
///      `payChooseAsk` -> `paySplitChooseBefore`) -- those strings ride logs,
///      the CEL kind vocabulary, and `anim.chainKind.*` locales, so this is a
///      version bump. Hard rename: no aliases (old records replay on archived
///      engines). Not renamed: `Pay(9)` / `Paid(10)` / `PayAfter(48)` /
///      `PayGate(96)` / `ChargePaying(78)`, host fns (`ctx::pay_*`, `pay_buy`,
///      `pay_build`, `pay_leg`, `split_pay`, `set_pay_amount`,
///      `set_pay_target`), and CEL variables (`pay_is_rent`, `pay_*`).
///      Filter/Action classification unchanged: Combined*/Split* filters remain
///      filters; the two *Before prompts remain Actions.
///      SAVE_VERSION unchanged.
/// v83: **`FlipEffect` / `FlipAfter` retired; PP fans in the per-player state
///      dict**. Ids 94 (`flipEffect`) and 104 (`flipAfter`) are **retired --
///      never reuse**. Fan counters move to `ctx::state` keys `ppFansUp` /
///      `ppFansDown` (`skill_groups::fans`, docs/CARDS.md 「PP fans in the
///      per-player state dict」), owned by no card. Band clause （2） runs for
///      **every** down→up flip via the ruleset-defined Action kind `FansFlip`
///      (wire 107, `bandori_std::kind`; raised before the dict writes with
///      `value` = the effect's intended X). CEL reads the fans as
///      `slot('ppFansUp')` / `slot('ppFansDown')` (SLOT_NAMES).
///      SAVE_VERSION unchanged.
/// v84: **Dead trigger kinds retired**. Ids 16 (`stop`), 17 (`teleport`),
///      19 / 20 / 22 (the first three abnormal kinds) are **retired -- never
///      reuse**. None of them is raised by the engine and none has a live
///      listener. Not retired (still live at v84): `skillTeleport` (18),
///      the ruleset-declared id 21, `teleported` (66), `MoveKind::Teleport`, and
///      the `AbKind` wire ids. The CEL names for the retired kinds leave the
///      kind table (`Teleport` remains as `MoveKind`). SAVE_VERSION unchanged.
/// v85: **`SkillTeleport` retired; `move.by_skill` source tag**. Wire id 18
///      (`skillTeleport`) is **retired -- never reuse**. The only listener
///      (R:必然的联系（莉莎）) rides `MoveAfter` + cond
///      `move.kind == Teleport && move.by_skill` instead. `MoveCtx.by_skill`
///      is a generic "shaped or started by a `skill:` rule" tag (cond
///      `move.by_skill`); serde skip-if-false so saves remain stable.
///      `HostRequest::Teleport` now runs through `card_teleport` /
///      `teleport_as(resolve=false)` so it raises the move tail like every
///      other movement path. SAVE_VERSION unchanged.
/// v85 (source): **typed kind views** (`ActionKind` / `FilterKind`). The old
///      `HookKind` / `GateKind` aliases are retired (no alias remains). `On::Action`
///      takes `&'static [ActionKind]`, `On::Filter*` takes `&'static [FilterKind]`,
///      so a wrong-side kind const is a rustc error rather than a ruleset-build
///      failure. `TriggerKind` remains the single declaration source of truth
///      (all consts, `spelling`/`from_str`, `is_filter`); the typed sets mirror it
///      exactly. `ChainKind` remains the [`TriggerKind`] alias for
///      `On::Counteract`. WIRE and SAVE unchanged -- one shared i32 id space,
///      same name tables, no id renumbering. Source-only: ABI_VERSION remains 85.
/// v86: **display columns leave `ruleset_metadata`**. Printed text / display
///      strings remain authored on [`crate::meta::CardMeta`] /
///      `bandori_std::roster::{PersonaDef, GroupDef}` (the builder calls are
///      unchanged) but no longer ride the runtime metadata blob. Dropped
///      MetaRow keys: card `text` / `rawName` / `cell`; persona `display` /
///      `text` / `color` / `variantTag` / `art` / `costume` / `cnId`; group
///      `text` / `shortName` / `color` / `logo`. They are serialized at build
///      time into `webui/src/generated/display.json` by
///      `tools/export-display.mjs` (`docs/DISPLAY.md`). Keys that remain:
///      card `name` / `group` / `owner` / `derived` / `tags`; persona `name` /
///      `group` / `skill` / `exclusiveCards` / `preset` / `variantOf`; group
///      `name` / `skill` / `sharesExclusives`. Postcard field order of
///      [`crate::meta::MetaRow`] is unchanged (dropped columns are empty /
///      absent map pairs). SAVE_VERSION unchanged.
/// v87: **generic turn-flow / logging verbs** for status hosts: `end_turn_now`
///      (auto-skip latch), `teleport_as` (move-tail teleport with the `main`
///      flag and no abnormal window), `log_as` (typed log line). Additive
///      imports; SAVE_VERSION unchanged.
/// v88: **generic status / marker verbs**. The three named host grants
///      collapse to one kind-parameterized verb;
///      the turn-flow move-latch becomes `skip_main_move` / `clear_move_skip`;
///      charge-named marker imports become the generic `marker_*` family
///      (`gain_marker` / `spend_marker` / `add_marker_max`). `AbKind` is an
///      opaque newtype (wire codec only); the game names live in
///      `bandori-std`. `ActionKind`/`TriggerKind` lose the ruleset-named
///      consts (wire id 21 remains an opaque custom id). SAVE_VERSION unchanged.
/// v89: **status vocabulary leaves the SDK**. The kind-parameterized verb is
///      gone (rules compose `gate` + `state::*` + `raise_after`); state-key /
///      prop constants that named game statuses are deleted; `AbKind::as_str`
///      loses its name table (wire codec is `from_i32` / `as_i32` only).
///      `TriggerKind` id 21 serializes as decimal `"21"` (ruleset-declared).
///      New decision gates `OnBoardGate` (108) / `TargetableGate` (109).
///      `CardDef::gate_msgs` / `CardDef::exemption_prop` carry gate-refusal
///      i18n keys and the exempting prop key as data. `state::set_flags` +
///      `ctx::raise_after` are additive. SAVE_VERSION unchanged.

/// v90: **`MetaRow` sheds its two dead display slots**. `raw_name` and `cell`
///      have been empty since v86 moved display columns into
///      `webui/src/generated/display.json`; nothing read them. Dropped from
///      the postcard field order (a wire break -- that is the bump). The sheet
///      cell is now a comment on the card impl, and a row with no sheet name
///      is *named* after its cell (`#J11`) in `CardMeta::name`. SAVE_VERSION
///      unchanged.

/// v91: **namespaced trigger kinds**. `TriggerKind` is `{ ns: KindNs, id: i32 }`
///      (`KindNs::Engine` / `KindNs::Ruleset(key)`) instead of a bare `i32`;
///      `ActionKind` / `FilterKind` wrap it. A rule-defined kind can no longer
///      collide with a future engine id -- or with another ruleset's ids
///      (the key names the ruleset). Wire spelling: engine stays camelCase,
///      ruleset is `"r:{key}:{id}"` (e.g. `"r:bandori:21"`). Bare decimals
///      do not parse. Host masks are per-namespace (a second `u128` for ruleset
///      ids); a ruleset id outside `0..128` is a load error.
/// Three universal raises that Phase 1 still spelled as ruleset ids are
/// engine kinds, same numbers: **`markerSpent` (73)** any named-counter
/// spend commits; **`cardUsed` (74)** a field card's `On::Play` runs;
/// **`houseAdded` (75)** a deed's house count is written. The bandori
/// spellings `FireSpent` / `SkillUsed` / `HouseAdded` and
/// `"r:bandori:73/74/75"` are gone. What remains in `KindNs::Ruleset` is
/// genuinely ruleset vocabulary: `Exile` (21), `PassRewardAffected` (26),
/// `ChargePaying` (78), `FansFlip` (107).
/// v92: **CEL `kind` is a string**. `as_cel_i64` / `from_cel_i64` are dead;
///      the bit-pack encoding is gone. `trig_kind` / `raise` /
///      `declare_effect` / `trig_effect_kind` carry spellings (the
///      two-call out-buffer pattern for host-to-guest, pointer+len for
///      guest-to-host). `ManifestOn.triggers` is `Vec<String>` of spellings.
///      CEL `kind` compares spellings; the IR bakes `id_of(spelling)`.
///      `move.kind` stays `int` (`MoveKind` is a closed engine enum).
/// SAVE_VERSION unchanged (kinds are not in `Saved`).
/// RECORD_VERSION unchanged (the record codec does not carry kind ids).
/// v93: **idiomatic payloads + shared vocabulary**. `FilterValue` and its
///      `UNCHANGED` sentinel are gone. Per-kind payloads in
///      [`crate::payload`]: `GateValue` / `PlanValue` / `RollValue` /
///      `DealValue` / `PayAddValue` / `RedirectValue`, one `FilterPayload`
///      impl each. `On::FilterValue`'s body is a `FilterBody` arm (temporary
///      adapter until the Phase 3 `FilterEntry` drive). Gates move off
///      `On::FilterFlag` onto `FilterBody::Gate`; pure-decision
///      (`PaySplitChoose` / `PayCombinedCancel`) stays `fn(i32,i32)->bool`.
///      Intern-id tables (`TAG_FIRE_ROLL` / `TAG_PRE_ROLLPLAN` /
///      `REASON_POPPIN_LOCKED`) die -- bodies carry `MsgKey` strings.
///      Shared wire vocabulary lives in `game-kinds`: `Seat` / `Tile` /
///      `Payee` / `Dice` / `MoveKind` / `Dir` / `MsgKey`, one definition on
///      both sides of the hostcall. Flatten to `i32` only at the
///      `hostfns.rs` wasm-import edge. **SAVE_VERSION 11 -> 12** (`MoveCtx`
///      / `World.recorded_roll` / `TriggerBuy` sentinels become `Option`);
///      older saves are refused (replays ride archived engine bundles).
/// RECORD_VERSION unchanged (the record codec does not carry kind ids).
/// v94: **typed filter/action registration DSL**. `On::FilterFlag` /
///      `On::FilterValue` / `FilterBody` die; every filter registration is
///      `On::Filter(FilterEntry)` built by the `filter_kinds!` constructors
///      (`Filter::DiceRollPlan(pre, guard, body)`), every action is
///      `On::Action(ActionEntry)` from `action_kinds!`. One kind per entry --
///      multi-kind lists are split. `FilterEntry.body` is a type-erased
///      `*const ()` cast back only inside `payload::drive_*` (one per shape).
///      Downstream rulesets declare custom kinds with the same macros
///      (`bandori_std::kind::{Actions, Filters}`). `check_kind_variant` is
///      deleted -- the constructor type pins the side. Source-only for the
///      wire: `ManifestOn` is unchanged. SAVE_VERSION / RECORD_VERSION
///      unchanged.
/// v95: **named-field `On` entries + `counter_reason` locale keys**. The
///      remaining positional-tuple variants become `*Body` arms holding
///      `PlayEntry` / `CounteractEntry` / `RuleEntry` / `MessageEntry`; the
///      call spelling stays `On::Play(pre, why_not, body)` via `const fn`
///      constructors, with the 「…时」 clause attached by the
///      `.counter_reason(key)` builder (the entry field is `Option<MsgKey>`;
///      `None` replaces `""`). `ActionEntry`'s field moves with it. Display
///      text for the 「…时」 clause leaves Rust for
///      `cards:<crate>.…` / `counter.*` locale entries. Source + display export
///      only: `ManifestOn` is unchanged. SAVE_VERSION / RECORD_VERSION
///      unchanged.
/// v96: **`PaySplitMulBefore` interactive pre-stage**. New Action kind
///      `PaySplitMulBefore = 110` (wire `paySplitMulBefore`, hook-only like
///      `PaySplitChooseBefore = 105` / `PayCombinedAddBefore = 106`). Raised
///      per settled payment leg immediately before the pure `PaySplitMul`
///      filter fold -- after the whole `paySplitAdd` fold, before
///      `scale_settle_payment`. Payload: `player_id` = payer, `target` =
///      payee, `value` = the PaySplitMul input, the pay flag, `tile`,
///      `by_card`. The ask's `set_pay_amount` / `set_pay_target` fold into the
///      running figure; a value change sets `modified` so the end-of-calc
///      CeilTo-10 still fires. This is the writable stage between
///      `PaySplitAdd` and `PaySplitMul` (规则书 支付阶段 3 「支付计算后」).
///      Source-only for the wire: `ManifestOn` is unchanged. SAVE_VERSION /
///      RECORD_VERSION unchanged.
/// v97: **multi-select choice**. New import `ruleset_opt_count(n)` pushes how
///      many of the following options the ask wants; `PromptKind::ChoiceMulti
///      = 7` (wire `"multi"`) asks for exactly that many distinct options. The
///      ask answer is a **bitmask** (bit *i* = option *i*) in the same single
///      `i32` the answer log and `ruleset_ask` already carry -- no wire or
///      answer-log shape change. Guest API: `ctx::ask_pick_multi`. SAVE_VERSION
///      / RECORD_VERSION unchanged.
/// v98: **activation arguments (`declare`) + `ctx::query_guard`**
///      (docs/ACTIVATION.md).
///
///      *`declare`* -- a per-entry collector runs **before** the point of no
///      return and hands the body a **typed** argument, `T`, which is the
///      card's own postcard record (a pick, a tile, a card, …). `T = i32` is
///      the default, so the common "which of N" case keeps its spelling.
///      `export::ON` therefore stays at **four** params; `OP_DECLARE = 2`
///      returns packed `(ptr<<32)|len` of `postcard(T)` (or `EXIT_NEED_INPUT`,
///      or the new `EXIT_DECLINED`), the host stores `Vec<u8>`, and the body's
///      drive pulls it via `ctx::declared_arg::<T>()`. A buffer address cannot
///      collide with an `EXIT_*` sentinel. `ManifestOn::has_declare` (always
///      serialized) says whether the host must run it; `On::Filter` never
///      declares (rejected at ruleset build -- a filter is a pure value fold).
///      The body of a declaring entry is `fn(i32, T) -> Asked`; everyone else
///      keeps `fn(i32) -> Asked`. Guest API: `On::declare` / `On::declare_arg`.
///
///      *`query_guard`* -- one cross-card applicability query, by the entry id
///      the **target** declares (`On::id("decay")`, an ASCII slug: identity,
///      never display text). `ctx::query_guard(player_id, card_id, entry_id)`.
///      `entry::PRESS` is the reserved id of a card's press entry
///      (`On::Play`), resolved via `play_entry_preferring_gated`; declaring
///      that word is rejected at ruleset build. `can_fire` / `can_fire_any` /
///      `card_replayable` are gone -- the press question is
///      `query_guard(…, entry::PRESS)`. `ManifestOn::id` (always serialized)
///      carries the slug.
///
///      **Player-visible ordering:** for `On::Play` the `declare` prompt fires
///      *before* the `card` counteraction window (because `hand.remove` is the
///      commit and the window opens after it) -- the mode is part of declaring
///      the play, matching `COUNTERACT_MOVE_EXTENSION`'s two-step before
///      declaration. SAVE_VERSION / RECORD_VERSION unchanged.
pub const ABI_VERSION: i32 = 100;
/// v100: **slot sugar translates to the state field channel**. `ctx::slot` /
///      `set_slot` / `inc_slot` desugar to `state_get` / `state_set` /
///      `state_add` before the ABI, and `state::set_flags` rides `state_set`
///      field `FLAGS = 4`. `ruleset_slot` / `ruleset_set_slot` /
///      `ruleset_inc_slot` / `ruleset_state_set_flags` are gone.
///      SAVE_VERSION / RECORD_VERSION unchanged.
/// v99: **named-card pile movers collapse onto `Dest`**. `add_to_hand` /
///      `add_to_deck` / `add_to_deck_at` (`DeckPos`) die; the one
///      `add_to_dest(player_id, card, dest)` speaks the same wire `Dest`
///      codes as `send_to_dest` / `transfer_to_dest` (0 graveyard, 1 hand,
///      4/5/6 deck top / bottom / shuffle -- default shuffle). `Field` /
///      `Banished` are not piles and trap. `take_card`'s args reorder to
///      card-before-zone `take_card(player_id, card, src)`; `take_from_hand`
///      is `take_card(..., CardPile::Hand)`. Hostcalls: `ruleset_add_to_dest`
///      replaces the three adds; `ruleset_take_card` is
///      `(player_id, ptr, len, src)`. SAVE_VERSION / RECORD_VERSION unchanged.


/// How an entry presents itself in the log (`ManifestOn::presentation`, v54).
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Presentation {
    /// Flash + 「效果适用」 header (the default).
    Visible = 0,
    /// Bookkeeping: no flash, no header. `CardDef::silent`.
    Silent = 1,
    /// Flash only if the body produced something visible.
    /// `CardDef::quiet_when_empty`.
    QuietWhenEmpty = 2,
}

impl Presentation {
    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => Self::Silent,
            2 => Self::QuietWhenEmpty,
            _ => Self::Visible,
        }
    }
}

impl Default for Presentation {
    fn default() -> Self {
        Self::Visible
    }
}

/// Whether the activation flashes (`ManifestOn::flash`, v55).
///
/// `Auto` is today's host classification (owner-self-only => no flash);
/// `Always` / `Never` are the rule's own opinion and win over it. Body code
/// may still override at runtime via `ctx::flash` / `ctx::no_flash` under the
/// precedence `Never` > `ctx::no_flash` > `ctx::flash` / `Always` > `Auto`.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Flash {
    /// Classify by scope (owner-self-only => no flash). The default.
    Auto = 0,
    /// This entry always flashes, even when the effect is owner-self-only.
    Always = 1,
    /// Log entry remains; no flash / pulse, even when the effect is cross-player.
    Never = 2,
}

impl Flash {
    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => Self::Always,
            2 => Self::Never,
            _ => Self::Auto,
        }
    }
}

impl Default for Flash {
    fn default() -> Self {
        Self::Auto
    }
}

/// One entry's log presentation: the header mode plus the flash policy.
/// Returned by `CardDef::presentation` -- the one lookup that folds
/// `silent` / `quiet_when_empty` / `flash`. v55.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EntryPresentation {
    pub header: Presentation,
    pub flash: Flash,
}

/// Temporary trigger tag for the extension selected before source declaration.
/// The host scopes it to one counteraction body, never the resulting walk.
pub const COUNTERACT_MOVE_EXTENSION: &str = "counteract.moveExtension";

/// Tile-text slots (`CardDef::texts`, [`ManifestTileText::slot`], v59): which
/// half of a deed popup an entry speaks for. The settle slot is the 结算
/// description; the pass slot is the 经过 one. Mirrors the body's
/// `On::Settle` / `ActionKind::PassTile` split -- the text an entry declares is
/// the description that entry speaks for, so a rule that replaces one half
/// only replaces that half's text.
pub mod text_slot {
    /// The 结算 / landing description (`On::Settle`).
    pub const SETTLE: &str = "settle";
    /// The 经过 / pass description (`ActionKind::PassTile`).
    pub const PASS: &str = "pass";
}

/// Wasm import module name for every host function.
pub const IMPORT_MODULE: &str = "ruleset";

/// Reserved entry identifiers for cross-card applicability queries
/// (`ctx::query_guard`, docs/ACTIVATION.md §3). Lives in `abi` so both the
/// guest and the host see the same spelling.
pub mod entry {
    /// The card's **press** entry (`On::Play`): "may I use this?", which is
    /// what a skill button asks. Not a declared id -- it always resolves the
    /// entry the engine itself would pick (`play_entry_preferring_gated`), so
    /// a card with several press modes answers "is *any* press open" the way
    /// the engine picks a mode.
    ///
    /// A card may not declare this id (`On::id`; `build-ruleset` rejects it),
    /// so the word cannot be squatted.
    pub const PRESS: &str = "press";
}

/// Guest exports.
pub mod export {
    pub const ABI_VERSION: &str = "ruleset_abi_version";
    /// `() -> i64` packed `(ptr << 32) | len` pointing at a UTF-8 JSON manifest.
    pub const MANIFEST: &str = "ruleset_manifest";
    /// `(card: i32, entry: i32, op: i32, player_id: i32, arg: i32) -> i64` --
    /// call entry `entry` (an index into the card's manifest `on` list). `op`
    /// is [`OP_RUN`], [`OP_GUARD`] or [`OP_DECLARE`]. Returns 0, the guard's
    /// 0/1, or (for a `Play` gate) a packed `(ptr << 32) | len` postcard `Msg`
    /// reason with 0 meaning "playable". `arg` is the activation argument a
    /// prior [`OP_DECLARE`] collected (docs/ACTIVATION.md §2); ignored on
    /// [`OP_GUARD`], always 0 for a [`OP_RUN`] of a no-declare entry.
    pub const ON: &str = "ruleset_on";
    pub const OP_RUN: i32 = 0;
    pub const OP_GUARD: i32 = 1;
    /// Collect the activation argument. Returns the arg, [`crate::abi::EXIT_NEED_INPUT`],
    /// or [`crate::abi::EXIT_DECLINED`]. Only called when
    /// [`ManifestOn::has_declare`].
    pub const OP_DECLARE: i32 = 2;
    pub const MEMORY: &str = "memory";
    /// `() -> i64` packed `(ptr << 32) | len` pointing at a postcard
    /// [`crate::meta::MetaDoc`] (ABI v63). Built only when called; absent on
    /// pre-v63 modules (the host then reports empty roster metadata).
    pub const METADATA: &str = "ruleset_metadata";
    /// `() -> i64` packed postcard of the ruleset's tile-class table
    /// (`Vec<(String, bool, Vec<i32>)>`: name, requires_buyable, groups).
    /// Absent on older modules (the host then has an empty class table and
    /// only the generic `class*` props answer `tile_in`).
    pub const CLASS_DEFS: &str = "ruleset_class_defs";
    /// `() -> i64` packed postcard of the ruleset's kind hook-only flags
    /// (`Vec<(i32, bool)>`: ruleset kind id, hook-only). Absent on older
    /// modules (the host then treats every ruleset kind as opening a window).
    pub const KIND_FLAGS: &str = "ruleset_kind_flags";
}

/// The keys the engine's own game flow reads back (its turn rules and the
/// capability gates). Content is free to invent more -- a state key is just a
/// string. Mirrors `game_core::state::key`. Game-status key names live in
/// `bandori-std`, not here.
pub mod state_key {
    /// Latch set when an [异常移动效果] lands, cleared when the player's next
    /// [主要移动] completes. Longer-lived than the per-turn abnormal counter.
    pub const ABNORMAL_HIT: &str = "abnormalHit";
    /// Turn key of a live 「无法从手牌中使用[反击]」 lock (骰子已经掷下). When it
    /// equals `turn_key()`, every *other* seat is locked out of hand
    /// counteractions. Field counteractions (`counteractFromField`) are a
    /// separate path and are not gated here -- the clause names 「从手牌中」.
    pub const NO_COUNTERACT_TURN: &str = "noCounteractTurn";
    /// Hand size limit.
    pub const HAND_LIMIT: &str = "handLimit";
    /// The [`HAND_LIMIT`] value meaning 「手牌数没有上限」 (no limit). The
    /// key is an `i32` on the wire; the host reads this value as "no limit"
    /// (`MatchPlayer::hand_limit` -> `None`), never as a count.
    pub const NO_HAND_LIMIT: i32 = i32::MAX;
    /// The authoritative opening hand size (default 2; effects may lower it,
    /// minimum 0). Written at the before-match-start point, read by the opening
    /// draw. Replaces the old ignored `startHandMinus` slot.
    pub const START_HAND: &str = "startHand";
    /// Skill-system scratch.
    pub const SKILL_STATE: &str = "skillState";
    // 「[拥有者]不可盖房」 is no longer a per-player state latch: it is
    // `prop::NO_BUILD` on a rule instance -- a placed card's own instance, or a
    // `ctx::linger` instance for a hand card (`docs/PURCHASE.md`).
    // 「无法获取[CiRCLE奖励]」 is likewise `prop::NO_REWARD` on the source's
    // rule instance (`docs/TILES.md`).
}

/// Well-known tile-mark kinds. A mark's `kind` is its identity; the engine
/// responds to these two by name so a card can arm a gate without the engine
/// hardcoding the card's own name in the engine.
pub mod counter {
    /// On-card [CP点] (「自己[场上]N个[CP点]」). Storage `FieldCard::cp`.
    pub const CP: &str = "cp";
    /// 奇迹水晶. Storage `FieldCard::crystals`.
    pub const CRYSTALS: &str = "crystals";
}

pub mod mark {
    /// Carrying tiles cannot be named as a target (`target_tile` answers -1).
    pub const NO_TARGET: &str = "noTarget";

    /// Tile-mark **category**: [CP点], 「放置于路面上的指示物」
    /// (`data/rules.txt` 125). Mirrors `game_core::state::mark_category::CP`.
    /// The tile [CP点] are units of the standing `mark:cp` pseudo card's
    /// [`counter::CP`] counter, bound to tiles.
    pub const CP_CATEGORY: &str = "cp";
    /// The [CP点] mark's stable `kind`. Mirrors
    /// `game_core::state::mark_kind::CP`.
    pub const CP_KIND: &str = "mark:cp";

    /// Tile-mark **category**: a [指定] of one or more tiles by a field card /
    /// skill while it is in play (「指定…格子 / 地契 / 地块」). The mark is owned
    /// by the designating instance (`instance` = that card or skill uid) and
    /// dies with it (`destroy_instance_units`), so the label leaves the board
    /// when the designating rule does. `owner` is the designating seat (the
    /// label's colour); `card` on the row is the designating card id for
    /// display. Mirrors `game_core::state::mark_category::DESIGNATE`.
    pub const DESIGNATE_CATEGORY: &str = "designate";
    /// The designation mark's stable `kind`. Its display label comes from the
    /// designating card (`TileMark.card`), not from this string. Mirrors
    /// `game_core::state::mark_kind::DESIGNATE`.
    pub const DESIGNATE_KIND: &str = "mark:designate";
    /// Mark-filter sentinel: any owner / any src / any instance.
    pub const ANY: i32 = -2;
}

/// A filter over tile marks (the generic tile-mark API). Empty `kind` /
/// `category`, and [`mark::ANY`] for `owner` / `src` / `instance`, match
/// anything. Packed as postcard across the guest/host boundary.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MarkFilter {
    pub kind: String,
    pub category: String,
    /// Display owner seat, `-1` for a neutral mark, or [`mark::ANY`].
    pub owner: i32,
    /// Provenance instance uid, or [`mark::ANY`].
    pub src: i32,
    /// Owning counter's instance uid, or [`mark::ANY`].
    pub instance: i32,
}

impl MarkFilter {
    pub fn any() -> Self {
        Self {
            kind: String::new(),
            category: String::new(),
            owner: mark::ANY,
            src: mark::ANY,
            instance: mark::ANY,
        }
    }
    pub fn kind(mut self, k: &str) -> Self {
        self.kind = String::from(k);
        self
    }
    pub fn category(mut self, c: &str) -> Self {
        self.category = String::from(c);
        self
    }
    pub fn owner(mut self, o: i32) -> Self {
        self.owner = o;
        self
    }
    pub fn src(mut self, s: i32) -> Self {
        self.src = s;
        self
    }
    pub fn instance(mut self, i: i32) -> Self {
        self.instance = i;
        self
    }
}

/// How [`ctx::place_mark`] applies `count` to an existing matching row.
/// Wire codes on the `stack` argument (0 = [`Self::Merge`], 1 = [`Self::Fresh`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Stack {
    /// Stack onto the first match (same tile/kind/category/instance/src);
    /// create a row when there is none. The old `place_cp` behaviour.
    Merge = 0,
    /// Always push a fresh row. The old `add_mark` behaviour (tests count
    /// rows; kaoru's 3 「怪盗标记」, embers' copies, tae_police rabbits).
    Fresh = 1,
}

impl Stack {
    pub fn from_i32(v: i32) -> Self {
        if v == 1 {
            Self::Fresh
        } else {
            Self::Merge
        }
    }
}

/// Cross-card message payload (`ctx::send` / `On::Message`). A few typed ints
/// plus an optional name -- postcard-friendly.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Message {
    pub name: String,
    pub a: i32,
    pub b: i32,
    pub c: i32,
    pub tile: i32,
    pub seat: i32,
    pub text: String,
}

/// How `ctx::send` addresses a receiver.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Target {
    /// A specific field-card instance uid.
    Uid(i32),
    /// The instance of card id `card` held by `player` (first field copy).
    Card { player: i32, card: String },
    /// A standing board-owned rule by id: a tile rule (`tile:circle`), a mark
    /// owner (`mark:cp`), an active event, a status host. All live in the one
    /// `BOARD_OWNER` list; the id is what disambiguates.
    Board(String),
}

/// Named **card properties** -- the keys of a `CardDef`'s `props` map. A
/// property is a static fact about the card rule (`key -> i32`), not an effect
/// that runs at a trigger: the engine reads it back by key and never by
/// matching rulebook prose. Mirrors `game_core::state::prop` (the two crates
/// cannot share a definition; keep them in step).
///
/// Every key has a defined default of `0` when a card does not declare it.
pub mod prop {
    /// Offer this placed source in the hand-counteraction window. Default 0.
    pub const COUNTERACT_FROM_FIELD: &str = "counteractFromField";
    /// Nonzero alternatives with the same group are exclusive per seat/timing.
    pub const COUNTERACT_GROUP: &str = "counteractGroup";
    /// Fire-pot cost shown on a field counteraction's source-choice label.
    pub const COUNTERACT_CHARGE_COST: &str = "counteractChargeCost";
    /// Continuous 「手卡上限数量减1」 while the card
    /// sits on the field. Stamped onto the field instance at placement and
    /// gone with the card. `-1` cuts the owner's hand limit; a positive value
    /// lifts it; `0` (the default) does nothing.
    pub const HAND_LIMIT_DELTA: &str = "handLimitDelta";
    /// Bot-only **estimated execution cost** (user ruling 2026-10-07): what
    /// activating this card is expected to cost the player, in 资金. Read
    /// **only** by bots / autopilot as a reserve check -- never by legality.
    /// Constant for now; an X-dependent cost may later become a `rules-cond`
    /// expression. `0` (the default) means "unknown / assume free".
    pub const EST_COST: &str = "estCost";
    /// 「有[指定]目标」: `1` = this play names
    /// recipients, so 「取消其对目标之一的[指定]」 applies instead of 「抵消其
    /// 所有的效果」. The named set is the play's **other living players** (the
    /// 「[指定][使用者]以外的所有玩家」 / 「其他玩家[分摊]」 shape). Default `0`
    /// (names nobody).
    pub const DESIGNATES: &str = "designates";
    /// 真奈 join (「其反击卡中针对打出玩家自身的效果改为你」): `1` = this
    /// counter opens a join; self-targeted effects of later counters answering
    /// the **same link** are redirected to the declarer. Declaration-scoped.
    /// Default `0`.
    pub const SELF_REDIRECT: &str = "selfRedirect";
    /// 「此卡不受任何其他效果影响」: `1` = the placed instance is immune
    /// (`FieldCard.immune`). Stamped at placement from the card's static props
    /// so a `place_raw` / engine placement sees it without running the play
    /// body. Mirrors `game_core::state::prop::IMMUNE`.
    pub const IMMUNE: &str = "immune";
    /// Song-title card marker (「卡名为歌名的卡」, Kizuna Music's X): `1` =
    /// this card's printed name is a song title. A ruleset-level fact other
    /// rules read back via `ctx::card_prop` (`card_prop` host import); the
    /// engine itself never reads it. Default `0` (not a song).
    pub const SONG: &str = "song";
    /// Virtual **rent** house count (「房屋数视为…」).
    /// Presence is the override -- a house count of `0` is a legitimate value,
    /// so the read is `props.get`, not `unwrap_or(0)`. Real `st.houses` is
    /// untouched: build caps, raze, sale and asset value still see the standing
    /// houses. Two write surfaces, matching the two clause shapes:
    /// * `ctx::set_prop` on a placed card of the tile's **owner** -- 「你的所有
    ///   格子上的房屋数视为…」. Gone with the card.
    /// * `ctx::set_tile_prop` on the tile's rule instance -- a tile-scoped
    ///   override. The source arms and disarms it.
    /// The rent lookup (`pay_rent` / `rent_of` / `ctx::rent_houses_of`) reads
    /// it; `ctx::houses_of` remains real.
    pub const RENT_HOUSES: &str = "rentHouses";

    // ---------------------------------------------------------- tile data
    // Stamped onto a board-owned tile rule instance at bind time (from
    // `TileData`), read back by the settle bodies and the end-step buy/build
    // gates. Mirrors `game_core::state::prop`; see `docs/TILES.md`.

    /// Land price (houses are extra).
    pub const PRICE: &str = "price";
    /// Build cost per level.
    pub const HOUSE: &str = "house";
    /// Colour group. [`ALL_COLORS`] is 「该格获得所有颜色」.
    pub const GROUP: &str = "group";
    /// The tile's value that means 「该格获得所有颜色」.
    pub const ALL_COLORS: i32 = -2;
    /// 「每块地有标注的等级上限」 -- max houses (`rentLen - 1`).
    pub const BUILD_MAX: &str = "buildMax";
    /// Length of the rent table (levels = houses + 1).
    pub const RENT_LEN: &str = "rentLen";
    /// Rent at level N: `rent:0` … `rent:rentLen-1`.
    pub const RENT_PREFIX: &str = "rent:";
    /// Generic dice-rent multiplier (ABI v66). When set (`> 0`) on a tile rule
    /// instance, rent is `count_owned_kind(owner, kind) × mult × 1d20` instead
    /// of the rent table.
    /// TODO(规则书): the dice-rent table itself is not book text for every
    /// ruleset.
    pub const RENT_DICE_MULT: &str = "rentDiceMult";
    /// This tile's rule is the **start** square (the engine's generic
    /// "pass the start" / start-tile queries). Stamped from the tile rule's
    /// static props; the engine never matches a kind string.
    pub const START: &str = "start";
    /// `1` = this deed cannot be mortgaged (a tile-rule static prop). The
    /// engine reads it instead of matching a tile kind.
    pub const NO_MORTGAGE: &str = "noMortgage";

    // ------------------------------------------------- tile rule modifiers
    // What a card that bends a tile writes onto its rule instance, replacing
    // the engine flags these used to be.

    /// 「无法获取[CiRCLE奖励]」 on this tile's `tile:circle` instance.
    pub const NO_REWARD: &str = "noReward";
    /// Rent scale in milli-units (500 = x0.5). Replaces `rent_factor`.
    pub const RENT_FACTOR: &str = "rentFactor";
    /// Payment scale in milli-units (500 = x0.5). Replaces `pay_factor`.
    pub const PAY_FACTOR: &str = "payFactor";
    /// 「[拥有者]不可盖房」. Replaces the `noBuild` state key.
    pub const NO_BUILD: &str = "noBuild";
    /// 「不可在造价N及以上的格子上加盖房屋」 (卡池BUG) -- `why_not_build_on`
    /// refuses a build whose house cost is `>=` this, while the instance is in
    /// play. On a board-owned instance (`ctx::set_prop`); `0` disables.
    pub const NO_BUILD_ABOVE: &str = "noBuildAbove";
    /// Turn-clock override (v61): every turn start **sets** the host
    /// `base_time` (恢复时间) to this many seconds **plus one**; `0` = no
    /// override (the +1 is what lets "0 seconds" be expressed). On a
    /// board-owned instance (`tile < 0`); with several, the smallest wins.
    /// Use [`clock_secs`] to encode. SET, not refill -- it replaces the
    /// `turnBaseTime` refill/cap calculation for that turn start (元祖！邦多利酱).
    pub const BASE_TIME_SET: &str = "baseTimeSet";
    /// Turn-clock override (v61): as [`BASE_TIME_SET`], for the acting seat's
    /// `bonus_time` (免费时间): it is **set** to this value at turn start
    /// instead of refilled.
    pub const BONUS_TIME_SET: &str = "bonusTimeSet";
    /// Turn-clock override: every turn start the 恢复时间 **refill** added to
    /// `base_time` (the `TurnBaseTime` value) is this many seconds **plus
    /// one**; `0` = no override. Unlike [`BASE_TIME_SET`] the pool is not
    /// replaced -- only the per-turn increment, which the usual cap still
    /// applies to. Replaces any `TurnBaseTime` hook result; ignored when
    /// [`BASE_TIME_SET`] is armed. On a board-owned instance (`tile < 0`);
    /// with several, the smallest wins (元祖！邦多利酱).
    pub const BASE_TIME_REFILL_SET: &str = "baseTimeRefillSet";
    /// Encode `secs` for [`BASE_TIME_SET`] / [`BASE_TIME_REFILL_SET`] /
    /// [`BONUS_TIME_SET`].
    pub const fn clock_secs(secs: i32) -> i32 {
        secs + 1
    }

    // ------------------------------------------------------ purchase surface
    // v40 (`docs/PURCHASE.md`). Tile props that decide eligibility, price and
    // the deal.

    /// 「可购买格子」 (`data/rules.txt` line 19) -- `1` = this tile can be bought
    /// at all. Stamped from `TileData::is_buyable` at bind time; a rule
    /// instance may clear it (rana_parking's 「不可被抵押双倍支付购买」).
    pub const BUYABLE: &str = "buyable";
    /// Houses already standing on the tile, counted into the buy price
    /// (「购买格子地契和建造已有房子的资金总价」). `0` disables; a positive
    /// value overrides the real `st.houses` count for the quote.
    pub const BUY_HOUSES: &str = "buyHouses";
    /// Force-buy price scale in milli-units (2000 = ×2, the rulebook default
    /// 「两倍」). Replaces the hardcoded `2 *`.
    pub const FORCE_MULT: &str = "forceMult";
    /// 「此次购买的价格不受任何资金变动效果影响」 (`data/rules.txt` lines 92,
    /// 106) -- `1` = the force-buy / buy moves money **directly**, bypassing
    /// the pay pipeline.
    pub const FORCE_FIXED: &str = "forceFixed";
    /// 「获得的地契仍为抵押状态」 (`data/rules.txt` line 106) -- `1` = a
    /// force-buy leaves the deed mortgaged. Default `1` for Force (the
    /// rulebook's own words); `0` clears it.
    pub const FORCE_STAYS_MORTGAGED: &str = "forceStaysMortgaged";
    /// 「该格获得所有颜色」 / soyo 「所有颜色」 -- this tile counts as every
    /// colour group for agent-set membership. Mirrors [`ALL_COLORS`] as a
    /// tile prop rather than a `group` value.
    pub const ANY_COLOR: &str = "anyColor";
    /// Per-player colour override prefix: `colorFor:<p>` = the group tile `p`
    /// treats this tile as (`-1` clears, `-2` = all colours).
    pub const COLOR_FOR_PREFIX: &str = "colorFor:";
}

/// `t.Roll.Source` -- where a `diceRollBefore` / `diceRollResult` face came from. Read by
/// 「当你使用火罐进行掷骰时」 (寄于指尖的执念) and kin. A `diceRollBefore` / `diceRollResult`
/// trigger carries one; every other trigger reads [`roll_source::NONE`].
pub mod roll_source {
    /// Unattributed (the engine's own move roll, or a roll with no named source).
    pub const NONE: i32 = 0;
    /// Wire code for a marker-funded roll (the `charge` name is one caller;
    /// rules-layer names the clause). Frozen; the engine never branches on the
    /// game name.
    pub const CHARGE: i32 = 1;
    /// A hand/field card's own roll (`ctx::roll_ask` from a card body).
    pub const CARD: i32 = 2;
    /// A skill press's roll (`ctx::roll_ask` from a skill body).
    pub const SKILL: i32 = 3;
}

/// `i32_exit` status the host uses to abort a run that reached an unanswered prompt.
/// Never observed by the guest.
pub const EXIT_NEED_INPUT: i32 = 0x0B_A0_D0;

/// `OP_DECLARE` return: the player declined, the entry does not fire at all.
/// Sibling of [`EXIT_NEED_INPUT`]. Never a legal [`Activation::Arg`] -- `rt`
/// panics if one collides (docs/ACTIVATION.md §2).
pub const EXIT_DECLINED: i32 = 0x0B_A0_D1;

/// Prompt kinds, one per host ask shape.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptKind {
    /// One of the pushed options (`ask_pick`).
    Choice = 0,
    /// A yes / no question (`ask_yes`); the options are implicit.
    YesNo = 1,
    /// Pick a tile index (`ask_tile`).
    Tile = 2,
    /// Pick a player (`ask_player`).
    Player = 3,
    /// Pick a card id (`ask_card`).
    Card = 4,
    /// Pick a tile, answering with the **tile id** (or -1 for 「不选」) rather
    /// than the option index (`ask_tiles`). The id is what the answer log
    /// carries, so a commit-pass re-run of a body whose options list the
    /// just-committed host effect reshaped still maps the pick to the same
    /// tile.
    TileId = 5,
    /// Pick a tile the player then **walks** to (`ask_tile_walk`): answered
    /// like `Tile`, but the board lights the path as well as the tile.
    TileWalk = 6,
    /// Exactly `count` of the pushed options (`ask_pick_multi`). The answer is
    /// a **bitmask** (bit *i* = option *i*), carried in the single `i32` the
    /// answer log and `ruleset_ask` already speak. Distinctness is the mask's
    /// own shape -- the same bit cannot be set twice.
    ChoiceMulti = 7,
}

impl PromptKind {
    pub fn from_i32(v: i32) -> Option<Self> {
        Some(match v {
            0 => Self::Choice,
            1 => Self::YesNo,
            2 => Self::Tile,
            3 => Self::Player,
            4 => Self::Card,
            5 => Self::TileId,
            6 => Self::TileWalk,
            7 => Self::ChoiceMulti,
            _ => return None,
        })
    }

    /// The prompt-kind wire string (`"pick"`, `"yes"`, ...).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Choice => "pick",
            Self::YesNo => "yes",
            Self::Tile => "tile",
            Self::Player => "player",
            Self::Card => "card",
            // Same UI as `Tile` -- only the recorded answer differs.
            Self::TileId => "tile",
            Self::TileWalk => "tile",
            Self::ChoiceMulti => "multi",
        }
    }
}

/// How the player gets there -- a move is exactly one of these (walk the path
/// vs jump to the destination). One definition in `game-kinds`, shared with the
/// engine; re-exported here so guest call sites keep their path.
pub use game_kinds::{Dice, Dir, MoveKind, MsgKey, Payee, Seat, Tile};

bitflags::bitflags! {
    /// What a move resolves as it goes -- the settle axis, its own category
    /// beside [`MoveKind`]. An ordinary move settles both.
    ///
    /// The wire is a flat `i32`; [`Self::bits`] / [`Self::from_bits`] flatten it.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct Settle: i32 {
        /// The tiles the move goes through resolve their 经过 effects. A walk:
        /// every tile it steps on. A teleport: the destination counts as passed
        /// when the teleport actually moves the player (`From != to`), or when
        /// this bit is forced (「原地也算 经过」).
        const ROUTE = 1;
        /// The landing resolves: `settleBefore` -> `settle` ->
        /// `land` -> `settleAfter`. On a teleport this gates the destination's
        /// 经过 too (a move that does not resolve returns before the pass block).
        const DEST = 2;
    }
}

/// Which kind of purchase a buy trigger / quote is about (`docs/PURCHASE.md`).
/// Mirrors `game_core::engine::play::purchase::BuyKind`.
#[repr(i32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BuyKind {
    /// An ordinary land buy (the end step's offer, or `ctx::buy`).
    #[default]
    Land = 0,
    /// An agent offer's buy branch.
    Agent = 1,
    /// A card-driven buy (`ctx::buy` from a card body).
    Card = 2,
    /// 「强行购买」 -- a mortgaged deed bought out at 2×.
    Force = 3,
    /// 「收购」 -- a deed taken from its owner at the acquisition price.
    Acquire = 4,
    /// An auction win.
    Auction = 5,
}

impl BuyKind {
    pub fn from_i32(v: i32) -> Option<Self> {
        Some(match v {
            0 => Self::Land,
            1 => Self::Agent,
            2 => Self::Card,
            3 => Self::Force,
            4 => Self::Acquire,
            5 => Self::Auction,
            _ => return None,
        })
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Land => "land",
            Self::Agent => "agent",
            Self::Card => "card",
            Self::Force => "force",
            Self::Acquire => "acquire",
            Self::Auction => "auction",
        }
    }
}

/// An abnormal effect kind id (wire `i32`), carried on `abnormalGuard` /
/// `abnormal` triggers.
///
/// Opaque newtype over the wire id, the same shape as [`ActionKind`] /
/// [`FilterKind`]: this module is the **wire codec only** (`from_i32` /
/// `as_i32`). The game names of the kinds and their frozen webui wire
/// strings live in `bandori-std::abnormal`, not here.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
#[repr(transparent)]
pub struct AbKind(pub i32);

impl AbKind {
    /// Wire codec: `1..=7` are the abnormal kinds; anything else is none.
    pub fn from_i32(v: i32) -> Option<Self> {
        matches!(v, 1..=7).then_some(Self(v))
    }

    pub fn as_i32(self) -> i32 {
        self.0
    }
}

/// One of a player's card piles: hand / discard / draw, and the placed field
/// cards.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardPile {
    Hand = 0,
    Graveyard = 1,
    /// The draw pile, **top card first**.
    Deck = 2,
    /// Cards placed on the player's field, in placement order.
    Field = 3,
}

impl CardPile {
    pub fn from_i32(v: i32) -> Option<Self> {
        Some(match v {
            0 => Self::Hand,
            1 => Self::Graveyard,
            2 => Self::Deck,
            3 => Self::Field,
            _ => return None,
        })
    }
}

/// `passRewardAffected`'s `Trigger.value` -- which half of the CiRCLE reward was
/// taken. `0` is 「[获得]资金」, `1` is the card. The no-act path forces the
/// card, so a money-only clause cannot fire there.
///
/// **Must match the engine's `CIRCLE_REWARD_MONEY` / `CIRCLE_REWARD_CARD` in
/// `game-core/src/engine/play.rs`** (the same relation `Arg` has to
/// `game-core/src/msg.rs`): `game-core` is the source, this is the mirror.
/// Renumbering one means renumbering the other.
pub const REWARD_MONEY: i32 = 0;
pub const REWARD_CARD: i32 = 1;

/// Which id space a [`TriggerKind`] lives in.
///
/// Engine kinds are the fixed generic raise points; ruleset kinds are declared
/// by a named rule set and can never collide with a future engine id -- or
/// with a different rule set's ids.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize)]
pub enum KindNs {
    /// The engine's generic raise points (`0..`).
    #[default]
    Engine,
    /// A ruleset-declared namespace, keyed by the ruleset's stable name
    /// (`"bandori"`). Distinct rulesets cannot collide on the wire.
    Ruleset(&'static str),
}

impl KindNs {
    /// The `bandori-std` ruleset key (see `bandori_std::kind::RULESET_KEY`).
    pub const BANDORI: Self = Self::Ruleset("bandori");
}

// `&'static str` has no `Deserialize` that fits an owned postcard buffer, so
// the enum decodes through an owned wire form and interns the key (the known
// `BANDORI` key is matched first; anything else is leaked once per unique key).
impl<'de> serde::Deserialize<'de> for KindNs {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        enum Wire {
            Engine,
            Ruleset(String),
        }
        Ok(match Wire::deserialize(deserializer)? {
            Wire::Engine => KindNs::Engine,
            // Only declared ruleset keys construct a `KindNs`. An unknown key
            // cannot be given a `&'static str` without leaking, and the host
            // has no business inventing a namespace it was never told about.
            // A second ruleset declares its key next to `BANDORI`.
            Wire::Ruleset(key) => match key.as_str() {
                "bandori" => KindNs::BANDORI,
                other => {
                    return Err(serde::de::Error::custom(format_args!(
                        "unknown ruleset kind namespace {other:?}"
                    )))
                }
            },
        })
    }
}

/// Trigger kind: a namespaced wire id for every raise point.
///
/// Engine-generic kinds are named associated constants that serialize to
/// camelCase wire names; ruleset-declared kinds are opaque ids in a named
/// [`KindNs::Ruleset`] that serialize as `"r:{key}:{id}"` (e.g. `"r:bandori:21"`).
/// `spelling` and `from_str` map those two spellings. `is_filter` is the side
/// declaration for engine ids; the host treats `OnKind::Filter` /
/// `OnKind::Action` as authoritative for ruleset kinds (see [`Self::is_filter`]).
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct TriggerKind {
    /// The id space this id lives in.
    pub ns: KindNs,
    /// The id within that space.
    pub id: i32,
}

/// Action-kind view of the shared wire ids: event reactions (`On::Action`).
///
/// A listener may emit effects and does not rewrite the triggering value. Ids
/// are the same [`TriggerKind`] every kind view uses. Kind/variant agreement is a
/// compile error: an action entry takes these constants, never filter kinds.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct ActionKind(pub TriggerKind);

/// Filter-kind view of the shared wire ids: pure value transforms
/// (`On::Filter(FilterEntry)`).
///
/// Bodies cannot prompt or emit effects. The four shapes
/// ([`crate::kinds::FilterShape`]) are `fn(i32, i32) -> i32` (value),
/// `fn(i32, i32) -> bool` (flag), `fn(i32, GateValue) -> GateValue` (gate),
/// and `fn(i32, P) -> P` for a typed payload family. Bool polarity: gates
/// answer `Some(true)` = allowed; cancel stages answer `false` = cancel.
/// Ids are the same [`TriggerKind`] every kind view uses. Kind/variant
/// agreement is a compile error: a filter entry takes these constants (via
/// the `filter_kinds!` constructors), never action kinds.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct FilterKind(pub TriggerKind);

/// Counteraction-window selectors (`On::Counteract`). Same wire ids as
/// [`TriggerKind`]. A counteraction may prompt and may rewrite values its window
/// exposes. Dual-role ids also have an `ActionKind` constant when the same
/// raise is an action-listened event.
pub type ChainKind = TriggerKind;

impl ActionKind {
    /// Unset. Never raised; the parse fallback and Default.
    pub const None: Self = Self(TriggerKind::None);

    /// Action / Counteraction window. Before the move's dice are cast
    /// (行动阶段 8 移动掷骰前). Payload: the move, value = -1 as a no-roll-yet
    /// sentinel. A listener may emit effects; a counteraction may shape the roll.
    pub const DiceRollBefore: Self = Self(TriggerKind::DiceRollBefore);

    /// Action / Counteraction window. After the move's dice land and after DiceRollAfter
    /// (行动阶段 10 移动掷骰后), before the walk. Payload: the move, value = the
    /// face. A counteraction may prompt and reroll; the written face is read back.
    pub const DiceRollResult: Self = Self(TriggerKind::DiceRollResult);

    /// Action / Counteraction window. A turn begins (行动阶段 2 回合开始时), after
    /// the layer-count ticks. Payload: tile = the player's position.
    /// A listener may emit effects; a counteraction may answer the opening.
    pub const TurnStart: Self = Self(TriggerKind::TurnStart);

    /// Action / Counteraction window. One tile stepped over during a move
    /// (行动阶段 12 经过), after arrival, for every path tile including the
    /// destination. Payload: the move, tile = the tile stepped on.
    pub const Pass: Self = Self(TriggerKind::Pass);

    /// Action / Counteraction window. End-tile overlap (行动阶段 13) -- one raise
    /// per other player sharing the landing tile, at the move's end only.
    /// Payload: the move, tile = the landing tile, target = that player.
    pub const PassPlayer: Self = Self(TriggerKind::PassPlayer);

    /// Action / Counteraction window. Pre-settle window (行动阶段 14 触发结算前 /
    /// 移动终点). Payload: the move, tile = the tile about to settle.
    /// A relocation here redirects the settle, which re-runs this window at the
    /// new tile. A counteraction may cancel or reshape the upcoming settle.
    pub const SettleBefore: Self = Self(TriggerKind::SettleBefore);

    /// Action / Counteraction window. The settle's declaration (行动阶段 15
    /// 触发结算/结算时). Payload: the move, tile = the tile, target = the
    /// tile's owner (-1 = the bank). A counteraction may cancel the settle, in
    /// which case there is no body and no SettleAfter; TileResolved still fires.
    pub const Settle: Self = Self(TriggerKind::Settle);

    /// Action / Counteraction window. A deed mortgage has just applied
    /// (运营阶段 抵押地契). Payload: tile = the deed. A listener may emit effects.
    pub const Mortgage: Self = Self(TriggerKind::Mortgage);

    /// Action. A payment is settled -- what it actually is, after the counteraction
    /// chain and the modifier stages, before any money moves (支付阶段 5).
    /// Payload: player_id = payer, target = payee (-1 = the bank), value = amount,
    /// the pay flag (rent or not), tile, by_card. A listener may cancel the payment
    /// outright or rewrite its amount or payee.
    pub const Pay: Self = Self(TriggerKind::Pay);

    /// Action / Counteraction window. Money left a player (支付阶段 6–7), after the
    /// deduction commits. Payload: player_id = payer, target = payee,
    /// value = what moved, the pay flag, tile, by_card. Opens on a payer-side loss
    /// only; a pure gain raises PayAfter alone.
    pub const Paid: Self = Self(TriggerKind::Paid);

    /// Action / Counteraction window. A bankruptcy's asset cash-in is done, before
    /// the seat is cleared. Payload: none beyond the raise seat. The seat is
    /// already marked dead at BankruptBefore.
    pub const Bankrupt: Self = Self(TriggerKind::Bankrupt);

    /// Action / Counteraction window. A card is being played from hand, before its
    /// play body. Payload: card = the card id, cards = that id, by_card = its
    /// player. A counteraction may negate the play: the card still goes to its
    /// destination, but the body does not run. Also the kind of every declared
    /// counteraction link (seq / answers say which link it is).
    pub const Card: Self = Self(TriggerKind::Card);

    /// Action / Counteraction window. An event draw has revealed the event, before it
    /// resolves. Payload: card = the event id. A counteraction may negate the
    /// draw: the effect never resolves and the card is filed away.
    pub const Event: Self = Self(TriggerKind::Event);

    /// Action. An abnormal effect settled on the target. Payload: player_id =
    /// the causer, target = the victim, value = the abnormal kind, by_card.
    /// This reports what landed, self-applied included; declaration-side gates
    /// are AbnormalGuard and the Effect counteraction.
    pub const Abnormal: Self = Self(TriggerKind::Abnormal);

    /// Action. A single-target effect settled on the target. Payload: player_id =
    /// the causer, target, tile, card, by_card. Settlement hook only; the
    /// counteraction key for being hit by an effect is Effect. Declaration-side
    /// gates are Untargetable and Redirect.
    pub const Target: Self = Self(TriggerKind::Target);

    // Wire id 21 is the 「获得[除外]」 post-commit raise. The rule-facing name
    // is the ruleset-declared kind (`ActionKind::ruleset(RULESET_KEY, 21)` /
    // `"r:bandori:21"`); this module carries no game-named kind const.

    /// Action / Counteraction window. A player toggled skill state (the 1↔2
    /// toggle commit). Payload: player_id = who, value = the new state, by_card.
    pub const State: Self = Self(TriggerKind::State);

    /// Action / Counteraction window. A counteraction has been declared against the
    /// target by player_id. Payload: player_id = the declarer, target.
    /// Not raised by the current engine.
    pub const Counteracted: Self = Self(TriggerKind::Counteracted);

    /// Action / Counteraction window. A draw hit a player outside their own turn,
    /// before the cards move. Payload: player_id = target = the drawer,
    /// value = how many cards, by_card = the causer. A counteraction may block
    /// the draw. The per-draw after points remain DrewBefore / Drawn / Drew.
    pub const DrawOut: Self = Self(TriggerKind::DrawOut);

    /// Action / Counteraction window. A player played their second hand card of
    /// the turn. Payload: card, cards = that id, by_card = its player.
    pub const TwoCards: Self = Self(TriggerKind::TwoCards);

    /// Action / Counteraction window. Pre-half of TurnStart, before the turn
    /// opens. Payload: tile = the player's position.
    pub const TurnStartBefore: Self = Self(TriggerKind::TurnStartBefore);

    /// Action / Counteraction window. Pre-half of Pass, before the player arrives
    /// on each path tile. Payload: the move, tile = the tile about to be stepped on.
    pub const PassBefore: Self = Self(TriggerKind::PassBefore);

    /// Action / Counteraction window. Pre-half of Mortgage, before a deed
    /// mortgage applies. Payload: tile = the deed. A counteraction may veto it.
    pub const MortgageBefore: Self = Self(TriggerKind::MortgageBefore);

    /// Action / Counteraction window. Pre-half of Bankrupt, before the asset
    /// cash-in. Payload: value = the amount. The seat is already marked dead.
    pub const BankruptBefore: Self = Self(TriggerKind::BankruptBefore);

    /// Action / Counteraction window. Post-half of Card, after the play body and
    /// the card's destination. Payload: card, cards = that id, by_card.
    pub const CardAfter: Self = Self(TriggerKind::CardAfter);

    /// Action / Counteraction window. Post-half of Event, after the event resolves
    /// and is filed away. Payload: card = the event id.
    pub const EventAfter: Self = Self(TriggerKind::EventAfter);

    /// Action / Counteraction window. Post-half of SettleBefore / Settle, after
    /// the tile's land resolution (with or without the body). Payload: the
    /// move, tile = the tile, target = the tile's owner (-1 = the bank).
    pub const SettleAfter: Self = Self(TriggerKind::SettleAfter);

    /// Action / Counteraction window. Before a buy commits. Payload: tile only.
    /// A counteraction may veto the buy. Price and deal fields are not on this
    /// payload; the buy modifier stages carry them.
    pub const BuyBefore: Self = Self(TriggerKind::BuyBefore);

    /// Action / Counteraction window. After a buy resolves and the deed changes
    /// hands. Payload: tile; some paths also set by_card.
    pub const BuyAfter: Self = Self(TriggerKind::BuyAfter);

    /// Action / Counteraction window. Before building a house resolves.
    /// Payload: tile. A counteraction may veto the build.
    pub const BuildBefore: Self = Self(TriggerKind::BuildBefore);

    /// Action / Counteraction window. After a house count is written (not on a
    /// refund). Payload: tile.
    pub const BuildAfter: Self = Self(TriggerKind::BuildAfter);

    /// Action / Counteraction window. Before a hand discard resolves.
    /// Payload: card = the card id.
    pub const DiscardBefore: Self = Self(TriggerKind::DiscardBefore);

    /// Action / Counteraction window. After the discard is filed and the deck is
    /// maintained. Payload: card = the card id.
    pub const DiscardAfter: Self = Self(TriggerKind::DiscardAfter);

    /// Action / Counteraction window. A player ends their turn, before the turn
    /// is queued over. Distinct from TurnEndBefore, which is the turn-end
    /// pipeline's first step. Payload: none beyond the raise seat.
    pub const EndTurnBefore: Self = Self(TriggerKind::EndTurnBefore);

    /// Action / Counteraction window. After the player's turn has been queued
    /// over. Payload: none beyond the raise seat.
    pub const EndTurnAfter: Self = Self(TriggerKind::EndTurnAfter);

    /// Action / Counteraction window. Before a player forfeits.
    /// Payload: none beyond the raise seat.
    pub const LeaveBefore: Self = Self(TriggerKind::LeaveBefore);

    /// Action / Counteraction window. After a forfeiting player's seat is
    /// cleared. Payload: none beyond the raise seat.
    pub const LeaveAfter: Self = Self(TriggerKind::LeaveAfter);

    /// Action. A turn just ended (any player's). Payload: none beyond the raise
    /// seat. A listener may emit effects.
    pub const TurnEnd: Self = Self(TriggerKind::TurnEnd);

    /// Action. One card has been drawn and is still in hand. Payload: card = the card
    /// id, value = 1. This is the drawn card's own hook; the per-draw field-card
    /// points are DrewBefore / Drew.
    pub const Drawn: Self = Self(TriggerKind::Drawn);

    /// Action / Counteraction window. The player stepped on or stopped at a tile
    /// during a move (行动阶段 12 经过), including teleport lands.
    /// Payload: the move, tile. Field-card hook and chain link for the same step.
    pub const PassTile: Self = Self(TriggerKind::PassTile);

    /// Action. Every settled money move (支付阶段 6–7 资金变动), prints included.
    /// Payload: player_id = the payer (or source), target, value = what moved
    /// (0 on a cancelled payment), by_card, the pay flag, tile.
    pub const PayAfter: Self = Self(TriggerKind::PayAfter);

    /// Action. A move roll resolved (行动阶段 10 移动掷骰后); the face has landed.
    /// Payload: the move, value = the face. Documented exception to filter
    /// purity: a listener here may prompt, roll, and write the face (the trigger
    /// value is read back) -- the one Action allowed to reshape a roll. Prefer
    /// DiceRollValue when a pure reshape is enough.
    pub const DiceRollAfter: Self = Self(TriggerKind::DiceRollAfter);

    /// Action. A card's hand effect resolved and its destination is final.
    /// Payload: card, cards, by_card.
    pub const CardPlayed: Self = Self(TriggerKind::CardPlayed);

    /// Action. A targeting declaration named the player, before the Effect
    /// counteraction. Payload: target = the named player, tile, card,
    /// by_card = the causer.
    pub const Targeted: Self = Self(TriggerKind::Targeted);

    /// Action. First step of the turn-end pipeline (结束阶段前), before the AtEnd
    /// callbacks and the status wear-off. Payload: none beyond the raise seat.
    pub const TurnEndBefore: Self = Self(TriggerKind::TurnEndBefore);

    /// Action. After TurnEnd; the late scheduled turn-end callbacks run here.
    /// Payload: none beyond the raise seat.
    pub const TurnEndAfter: Self = Self(TriggerKind::TurnEndAfter);

    /// Action. This card just reached the discard pile. Payload: card = the card
    /// id.
    pub const Discarded: Self = Self(TriggerKind::Discarded);

    /// Action. Before match start (游戏开始前) -- raised once per player before
    /// the opening hands are drawn, and dispatched to every effect source: field
    /// cards including skills (per player, in field order) and the card ids in
    /// that player's piles and hands. Payload: none beyond the raise seat.
    /// Start positions and the initial hand size are decided here.
    pub const DeckBeforeGame: Self = Self(TriggerKind::DeckBeforeGame);

    /// Action. After match start (游戏开始时) -- raised once per player after the
    /// opening draw and mulligan, dispatched to every effect source the same way
    /// DeckBeforeGame is. Payload: none beyond the raise seat. Initial tokens and
    /// resources are created here.
    pub const DeckAtGameStart: Self = Self(TriggerKind::DeckAtGameStart);

    /// Action. After one card has been drawn (per single card; an N-card draw raises
    /// this N times). Payload: card = the card id, value = 1, cards = that id.
    /// Per-draw field-card point; the drawn card's own hook is Drawn.
    pub const Drew: Self = Self(TriggerKind::Drew);

    /// Action. A player's discard pile has been shuffled back into its deck.
    /// Payload: none beyond the raise seat.
    pub const Reshuffled: Self = Self(TriggerKind::Reshuffled);

    /// Action. A player became the owner of the tile (buy or auction).
    /// Payload: tile; some paths also set by_card or the buy record.
    pub const Bought: Self = Self(TriggerKind::Bought);

    /// Action / Counteraction window. The settle body point -- a field card
    /// placed on the tile may replace the tile's rule instances' effect.
    /// Payload: the move, tile = the tile, target = the tile's owner (-1 = the
    /// bank). Cancel here to mark the body as replaced.
    pub const SettleBody: Self = Self(TriggerKind::SettleBody);

    /// Action. A player is about to leave the game (bankrupt or forfeit), before
    /// the seat is cleared. Payload: none beyond the raise seat.
    pub const BeforeOut: Self = Self(TriggerKind::BeforeOut);
    /// Alias of [`Self::BeforeOut`] -- kinds.rs declares the constructor as
    /// `BeforeBankrupt`; the wire table still spells `beforeOut`.
    pub const BeforeBankrupt: Self = Self(TriggerKind::BeforeOut);

    /// Action. A teleport finished (after its settlement, or at once if it does
    /// not settle). Payload: the move, tile = the destination.
    pub const Teleported: Self = Self(TriggerKind::Teleported);

    /// Action / Counteraction window. A card effect is declared at someone. This is the counteraction
    /// key for being hit by another player's effect -- one stable declaration
    /// fact, not a union of outcome kinds. Raised when a link's recipients are
    /// named, before any settlement. Payload: the declaration's player_id /
    /// target / value / by_card / tile / pay flag as applicable, plus the
    /// effect-list name on the link. A counteraction may negate or spare.
    /// Settlement hooks (Target / Abnormal / Pay / Paid) fire later and cannot
    /// reconstruct this declaration; PayAfter reports a settled money move.
    pub const Effect: Self = Self(TriggerKind::Effect);

    /// Action / Counteraction window. Any named-counter spend just committed
    /// (`spend_marker`). Payload: `name` / `value`. Fire-pot is one name of
    /// many. A listener may emit effects.
    pub const MarkerSpent: Self = Self(TriggerKind::MarkerSpent);

    /// Action / Counteraction window. A field card's `On::Play` entry is about
    /// to run (a card living on the player's field rather than in hand).
    /// Payload: `card` / `cards` / `by_card`. A counteraction may negate the
    /// press. Dual-role: also a counteraction-window selector.
    pub const CardUsed: Self = Self(TriggerKind::CardUsed);

    /// Action / Counteraction window. A deed's house count has just been
    /// written. Payload: `tile` / `value`. A listener may emit effects.
    pub const HouseAdded: Self = Self(TriggerKind::HouseAdded);

    /// Action. A named on-card counter on a placed instance has just been written.
    /// Payload: card = the instance whose count moved, player_id = its owner,
    /// value = the signed change applied, name = the counter name.
    /// The count after the write is readable on that instance.
    pub const CounterChanged: Self = Self(TriggerKind::CounterChanged);

    /// Action. Before one card would be drawn (per single card; an N-card draw
    /// raises this N times). Payload: card = the card that would be drawn (the
    /// deck's top, or empty when the pile is dry), value = 1. This is the
    /// per-draw replacement point: a listener may cancel and perform its own
    /// look or pick -- whatever it adds to the hand is the replacement draw,
    /// and the after points (Drawn / Drew) fire for it. Opening hands do not
    /// raise it.
    pub const DrewBefore: Self = Self(TriggerKind::DrewBefore);

    /// Action. The tile's settlement is fully resolved, after SettleAfter,
    /// including when the settle is cancelled (the resolution is complete as
    /// nothing). Payload: the move, tile = the tile, target = the tile's owner
    /// (-1 = the bank).
    pub const TileResolved: Self = Self(TriggerKind::TileResolved);

    /// Action. The move (and any settlement it asked for) is fully resolved.
    /// Fires at the end of a walk or teleport, after the settle pipeline.
    /// Payload: the move, tile = the final position.
    pub const MoveResolved: Self = Self(TriggerKind::MoveResolved);

    /// Action. The bankruptcy is fully resolved -- cash-in done, the seat
    /// cleared, the leftover auctions finished. Payload: none beyond the
    /// raise seat.
    pub const BankruptResolved: Self = Self(TriggerKind::BankruptResolved);

    /// Action / Counteraction window. Before any move -- walk or teleport, main
    /// or card-driven, settling or not -- once the move's plan is fixed and
    /// before the first step or the teleport. Payload: the move, tile = the
    /// origin; for a walk, value = the resolved base distance. A counteraction
    /// here cancels or alters the move (and therefore its tail).
    pub const MoveBefore: Self = Self(TriggerKind::MoveBefore);

    /// Action / Counteraction window. After a completed move (移动后/主要移动
    /// 结束时, also the 移动终点 anchor). Fires after PassPlayer, before
    /// SettleBefore, for every completed move -- including one that does not
    /// settle. Payload: the move, tile = the destination.
    pub const MoveAfter: Self = Self(TriggerKind::MoveAfter);

    /// Action / Counteraction window. Before a marker spend moves (any token
    /// category). Payload: player_id, value = the negative delta,
    /// card = the marker name (not a card id). A counteraction here cancels the
    /// spend; nothing moves if the link is negated.
    pub const MarkerSpend: Self = Self(TriggerKind::MarkerSpend);

    /// Action / Counteraction window. Before a marker gain moves. Payload:
    /// player_id, value = the positive delta, card = the marker name.
    /// A counteraction may cancel the gain.
    pub const MarkerGain: Self = Self(TriggerKind::MarkerGain);

    /// Action. Scheduled turn-end callback. Raised only for the card a schedule
    /// entry names, at that turn's end (one-shot), inside the TurnEndBefore /
    /// TurnEndAfter windows. Payload is that window's trigger. A listener may
    /// emit effects.
    pub const AtEnd: Self = Self(TriggerKind::AtEnd);

    /// Action. Interactive pre-stage immediately before the pure PaySplitChoose
    /// filter, per settled payment leg. Payload: player_id = payer, target =
    /// payee, value = the running figure, the pay flag, tile, by_card.
    /// May prompt; the pure filter at PaySplitChoose then applies.
    pub const PaySplitChooseBefore: Self = Self(TriggerKind::PaySplitChooseBefore);

    /// Action. Interactive pre-stage immediately before the pure PayCombinedAdd
    /// filter, on the whole payment command. Payload: player_id = payer,
    /// target = payee, value = the command total, the pay flag, tile, by_card.
    /// May prompt; the pure filter at PayCombinedAdd then applies.
    pub const PayCombinedAddBefore: Self = Self(TriggerKind::PayCombinedAddBefore);

    /// Action. Interactive pre-stage immediately before the pure PaySplitMul
    /// filter, per settled payment leg (after the whole PaySplitAdd fold).
    /// Payload: player_id = payer, target = payee, value = the PaySplitMul
    /// input, the pay flag, tile, by_card. May prompt; the pure filter at
    /// PaySplitMul then applies. `set_pay_amount` / `set_pay_target` fold into
    /// the running figure.
    pub const PaySplitMulBefore: Self = Self(TriggerKind::PaySplitMulBefore);

    /// Engine-namespace kind. Same id as [`TriggerKind::engine`].
    pub const fn engine(id: i32) -> Self {
        Self(TriggerKind::engine(id))
    }

    /// Ruleset-namespace kind. Same id as [`TriggerKind::ruleset`].
    pub const fn ruleset(key: &'static str, id: i32) -> Self {
        Self(TriggerKind::ruleset(key, id))
    }

    /// View this id in the wire namespace.
    pub const fn as_trigger(self) -> TriggerKind {
        self.0
    }

    /// View a wire id as this side (no side check -- the consts pin the set).
    pub const fn from_trigger(k: TriggerKind) -> Self {
        Self(k)
    }

    /// Wire spelling (camelCase generic, or `"r:{key}:{id}"` for a ruleset-declared id).
    pub fn spelling(self) -> Cow<'static, str> {
        self.as_trigger().spelling()
    }

    /// Same test as [`TriggerKind::is_filter`], on the wire id. An `ActionKind`
    /// const is always `false` and a `FilterKind` const always `true`; a raw
    /// `Self(k)` still reports the wire fact.
    pub const fn is_filter(self) -> bool {
        self.as_trigger().is_filter()
    }
}

impl From<ActionKind> for TriggerKind {
    #[inline(always)]
    fn from(k: ActionKind) -> Self {
        k.as_trigger()
    }
}

impl From<TriggerKind> for ActionKind {
    #[inline(always)]
    fn from(k: TriggerKind) -> Self {
        ActionKind::from_trigger(k)
    }
}

impl FilterKind {
    /// Filter. A settled payment leg is about to settle its amount, after the
    /// Combined stages and any split divide. Payload: value = the running
    /// figure, target = payee, the pay flag. No prompt (use
    /// PaySplitChooseBefore). Body answers cancel: false stops the payment.
    /// An integer return is folded into the leg amount.
    pub const PaySplitChoose: Self = Self(TriggerKind::PaySplitChoose);

    /// Filter. A settled payment leg's first amount modifier (支付阶段 2 on each
    /// share, before any money moves). Payload: value = the running figure,
    /// target = payee. No prompt. Body returns the new figure; the host folds it
    /// into the leg amount.
    pub const PaySplitAdd: Self = Self(TriggerKind::PaySplitAdd);

    /// Filter. A settled payment leg's second amount modifier (支付阶段 4 on each
    /// share, after PaySplitAdd). Payload: value = the running figure
    /// (the PaySplitMul input), target = payee. No prompt (use
    /// PaySplitMulBefore). Body returns the new figure; the host folds it
    /// into the leg amount.
    pub const PaySplitMul: Self = Self(TriggerKind::PaySplitMul);

    /// Filter. A settled payment leg is about to pay (支付阶段 5 on each share,
    /// after PaySplitChoose, before the pay counteraction window). Payload:
    /// value = the running figure, target = payee. No prompt. Body returns the
    /// new figure; the host folds it into the leg amount.
    pub const PaySplitAt: Self = Self(TriggerKind::PaySplitAt);

    /// Filter. The move's plan pass (行动阶段 8, before the dice are rolled).
    /// Payload: the move. Body rewrites the plan struct (roller, dice, steps,
    /// direction, settle flag, tiles, factors, and so on); the host applies only
    /// the fields the body changed.
    pub const DiceRollPlan: Self = Self(TriggerKind::DiceRollPlan);

    /// Filter. An abnormal effect is about to hit the target. Payload: target =
    /// the victim, value = the abnormal kind, by_card = the causer; the raise
    /// seat is the causer. Body true = allowed, false = cancel the hit. The
    /// host folds the answers into one decision; if it lands, a later Abnormal
    /// reports it.
    pub const AbnormalGuard: Self = Self(TriggerKind::AbnormalGuard);

    /// Filter. Is the target untouchable by the causer's effects? Asked before
    /// naming them as a target, before an abnormal effect, and before a
    /// card-driven payment from or to them. Payload: player_id = the seat asked
    /// about, by_card = the causer. Body true = allowed, false = immune; the
    /// host folds the answers into one decision.
    pub const ImmuneAll: Self = Self(TriggerKind::ImmuneAll);

    /// Filter. The causer is about to name the seat as a target. Payload:
    /// player_id = the seat asked about, by_card = the causer. Body true =
    /// allowed, false = cannot name them; the host folds the answers into one
    /// decision.
    pub const Untargetable: Self = Self(TriggerKind::Untargetable);

    /// Filter. A single-target effect is about to hit the target; a placed card
    /// may take the hit instead. Payload: target, card, by_card = the causer.
    /// Body rewrites the target (or leaves it); the host folds that into who is
    /// hit. Asked before Untargetable.
    pub const Redirect: Self = Self(TriggerKind::Redirect);

    /// Filter. May the player buy the tile at all? Payload: tile, the buy
    /// record, and on the quote path value = the price. Body true = allowed,
    /// false = refuse (with a reason); the host folds the answers into one
    /// decision. Runs for every buy kind, forced buys included.
    pub const BuyGate: Self = Self(TriggerKind::BuyGate);

    /// Filter. The buy price's first modifier stage (fixed plus/minus).
    /// Payload: player_id, tile, value = the price, the buy record. Body
    /// returns the new figure; the host folds it into the price.
    pub const BuyAdd: Self = Self(TriggerKind::BuyAdd);

    /// Filter. The buy price's second modifier stage (multiply), after BuyAdd.
    /// Payload: player_id, tile, value = the price, the buy record. Body
    /// returns the new figure; the host folds it into the price.
    pub const BuyMul: Self = Self(TriggerKind::BuyMul);

    /// Filter. The buy price's third modifier stage (free or fixed price),
    /// after BuyMul. Payload: player_id, tile, value = the price, the buy
    /// record. Body returns the new figure; the host folds it into the price.
    /// Each stage floors the price at zero.
    pub const BuySet: Self = Self(TriggerKind::BuySet);

    /// Filter. The deal is committing, before Bought. Payload: tile, the buy
    /// record including deal owner / houses / mortgaged. Body rewrites those
    /// deal fields; the host applies only the fields the body changed.
    pub const BuyAssign: Self = Self(TriggerKind::BuyAssign);

    /// Filter. The payment command's Combined fixed plus/minus stage on the
    /// whole figure, before any split divide (支付阶段 2 on the command total).
    /// Payload: player_id = payer, target = payee, value = the command total,
    /// the pay flag, tile, by_card. No prompt (use PayCombinedAddBefore).
    /// Body returns the new figure; the host folds it into the command total.
    pub const PayCombinedAdd: Self = Self(TriggerKind::PayCombinedAdd);

    /// Filter. The payment command's Combined multiply stage (支付阶段 4 on the
    /// whole figure, after PayCombinedAdd). Payload: player_id = payer,
    /// target = payee, value = the command total, the pay flag, tile, by_card.
    /// Body returns the new figure; the host folds it into the command total.
    pub const PayCombinedMul: Self = Self(TriggerKind::PayCombinedMul);

    /// Filter. The payment command's Combined cancel (支付阶段 5 on the whole
    /// command rather than on one pair, after PayCombinedMul). Payload:
    /// player_id = payer, target = payee, value = the command total, the pay
    /// flag, tile, by_card. Body false drops the whole command: no leg runs and
    /// nothing moves. The host folds the answers into one decision.
    pub const PayCombinedCancel: Self = Self(TriggerKind::PayCombinedCancel);

    /// Filter. May the player use active skills and hand play effects?
    /// Evaluated natively wherever the question is asked, including pure reads.
    /// Payload: the acting seat. Body true = allowed, false = denied; the host
    /// folds the answers into one decision.
    pub const ActGate: Self = Self(TriggerKind::ActGate);

    /// Filter. May the player pay or receive payment? Evaluated natively like
    /// ActGate. Payload: the acting seat. Body true = allowed, false = denied;
    /// the host folds the answers into one decision.
    pub const PayGate: Self = Self(TriggerKind::PayGate);

    /// Filter. May the player make the main move? Evaluated natively like
    /// ActGate. Payload: the acting seat. Body true = allowed, false = denied;
    /// the host folds the answers into one decision.
    pub const MoveGate: Self = Self(TriggerKind::MoveGate);

    /// Filter. May the player use this exclusive resource? Payload: the
    /// exclusive card on the window, name = the owner, value / step as owner
    /// flags. Body true = allowed, false = denied; the host folds the answers
    /// into one decision.
    pub const ExclusiveUse: Self = Self(TriggerKind::ExclusiveUse);

    /// Filter. Turn-clock modifier: the bonus-time refill in seconds the acting
    /// seat gets as its turn starts. Payload: value = the refill. Body returns
    /// the new figure; the host folds it into the clock (then caps the total).
    pub const TurnBonusTime: Self = Self(TriggerKind::TurnBonusTime);

    /// Filter. Turn-clock modifier: the base-time refill in seconds the acting
    /// seat's turn adds at turn start. Payload: value = the refill. Body
    /// returns the new figure; the host folds it into the clock, capped at the
    /// larger of the configured initial pool and the hook's own value. A
    /// board-prop base-time set replaces the whole calculation at turn start.
    pub const TurnBaseTime: Self = Self(TriggerKind::TurnBaseTime);

    /// Filter. May the player play a hand card right now? Distinct from ActGate,
    /// which also gates active skills and is what skips a no-act
    /// turn at the start phase. A standing hand-size lock blocks plays without
    /// cancelling the rest of the turn. Payload: the acting seat. Body true =
    /// allowed, false = denied; the host folds the answers into one decision.
    pub const PlayGate: Self = Self(TriggerKind::PlayGate);

    /// Filter. Roll-value modifier, raised immediately before DiceRollAfter
    /// with the same payload (the move, value = the face). Body returns the new
    /// face; the host folds it into the roll. Pure: prefer this over the
    /// DiceRollAfter exception unless a prompt is required.
    pub const DiceRollValue: Self = Self(TriggerKind::DiceRollValue);

    /// Filter. May the named seat be treated as on the board right now?
    /// Evaluated natively like ActGate. Payload: the seat. Body true = allowed,
    /// false = denied; the host folds the answers into one decision.
    pub const OnBoardGate: Self = Self(TriggerKind::OnBoardGate);

    /// Filter. May this effect name the seat as a target? Evaluated natively
    /// like ActGate. Payload: the seat. Body true = allowed, false = denied;
    /// the host folds the answers into one decision.
    pub const TargetableGate: Self = Self(TriggerKind::TargetableGate);

    /// Engine-namespace kind. Same id as [`TriggerKind::engine`].
    pub const fn engine(id: i32) -> Self {
        Self(TriggerKind::engine(id))
    }

    /// Ruleset-namespace kind. Same id as [`TriggerKind::ruleset`].
    pub const fn ruleset(key: &'static str, id: i32) -> Self {
        Self(TriggerKind::ruleset(key, id))
    }

    /// View this id in the wire namespace.
    pub const fn as_trigger(self) -> TriggerKind {
        self.0
    }

    /// View a wire id as this side (no side check -- the consts pin the set).
    pub const fn from_trigger(k: TriggerKind) -> Self {
        Self(k)
    }

    /// Wire spelling (camelCase generic, or `"r:{key}:{id}"` for a ruleset-declared id).
    pub fn spelling(self) -> Cow<'static, str> {
        self.as_trigger().spelling()
    }

    /// Engine gate id for `gate_msgs` / `gate_skip_msgs` (the manifest's
    /// `(i32, key)` pairs). Engine gates only -- ruleset kinds have no gate id.
    pub const fn gate_id(self) -> i32 {
        self.0.id
    }

    /// Same test as [`TriggerKind::is_filter`], on the wire id. An `ActionKind`
    /// const is always `false` and a `FilterKind` const always `true`; a raw
    /// `Self(k)` still reports the wire fact.
    pub const fn is_filter(self) -> bool {
        self.as_trigger().is_filter()
    }
}

impl From<FilterKind> for TriggerKind {
    #[inline(always)]
    fn from(k: FilterKind) -> Self {
        k.as_trigger()
    }
}

impl From<TriggerKind> for FilterKind {
    #[inline(always)]
    fn from(k: TriggerKind) -> Self {
        FilterKind::from_trigger(k)
    }
}

#[allow(non_upper_case_globals)]
impl TriggerKind {
    /// Unset.
    pub const None: Self = Self::engine(0);

    /// `diceRollBefore`
    pub const DiceRollBefore: Self = Self::engine(1);

    /// `diceRollResult`
    pub const DiceRollResult: Self = Self::engine(2);

    /// `turnStart`
    pub const TurnStart: Self = Self::engine(3);

    /// `pass`
    pub const Pass: Self = Self::engine(4);

    /// `passPlayer`
    pub const PassPlayer: Self = Self::engine(5);

    /// `settleBefore`
    pub const SettleBefore: Self = Self::engine(6);

    /// `settle`
    pub const Settle: Self = Self::engine(7);

    /// `mortgage`
    pub const Mortgage: Self = Self::engine(8);

    /// `pay`
    pub const Pay: Self = Self::engine(9);

    /// `paid`
    pub const Paid: Self = Self::engine(10);

    /// `bankrupt`
    pub const Bankrupt: Self = Self::engine(11);

    /// `card`
    pub const Card: Self = Self::engine(12);

    /// `event`
    pub const Event: Self = Self::engine(13);

    /// `abnormal`
    pub const Abnormal: Self = Self::engine(14);

    /// `target`
    pub const Target: Self = Self::engine(15);

    // Wire id 16 retired; never reuse.

    // Wire id 17 retired; never reuse.

    // Wire id 18 retired; never reuse.

    // Wire id 19 retired; never reuse.

    // Wire id 20 retired; never reuse.

    // Wire id 21 is a ruleset-declared kind (the 「获得[除外]」 post-commit
    // raise). The rule-facing name lives in `bandori_std::kind`
    // (`TriggerKind::ruleset(RULESET_KEY, 21)` / wire string `"r:bandori:21"`);
    // this module carries no game-named kind const.

    // Wire id 22 retired; never reuse.

    /// `state`
    pub const State: Self = Self::engine(23);

    /// `counteracted`
    pub const Counteracted: Self = Self::engine(24);

    /// `drawOut`
    pub const DrawOut: Self = Self::engine(25);

    /// `twoCards`
    pub const TwoCards: Self = Self::engine(27);

    /// `turnStartBefore`
    pub const TurnStartBefore: Self = Self::engine(28);

    /// `passBefore`
    pub const PassBefore: Self = Self::engine(29);

    /// `mortgageBefore`
    pub const MortgageBefore: Self = Self::engine(30);

    /// `bankruptBefore`
    pub const BankruptBefore: Self = Self::engine(31);

    /// `cardAfter`
    pub const CardAfter: Self = Self::engine(32);

    /// `eventAfter`
    pub const EventAfter: Self = Self::engine(33);

    /// `settleAfter`
    pub const SettleAfter: Self = Self::engine(34);

    /// `buyBefore`
    pub const BuyBefore: Self = Self::engine(35);

    /// `buyAfter`
    pub const BuyAfter: Self = Self::engine(36);

    /// `buildBefore`
    pub const BuildBefore: Self = Self::engine(37);

    /// `buildAfter`
    pub const BuildAfter: Self = Self::engine(38);

    /// `discardBefore`
    pub const DiscardBefore: Self = Self::engine(39);

    /// `discardAfter`
    pub const DiscardAfter: Self = Self::engine(40);

    /// `endTurnBefore`
    pub const EndTurnBefore: Self = Self::engine(41);

    /// `endTurnAfter`
    pub const EndTurnAfter: Self = Self::engine(42);

    /// `leaveBefore`
    pub const LeaveBefore: Self = Self::engine(43);

    /// `leaveAfter`
    pub const LeaveAfter: Self = Self::engine(44);

    /// `turnEnd`
    pub const TurnEnd: Self = Self::engine(45);

    /// `drawn`
    pub const Drawn: Self = Self::engine(46);

    /// `passTile`
    pub const PassTile: Self = Self::engine(47);

    /// `payAfter`
    pub const PayAfter: Self = Self::engine(48);

    /// `diceRollAfter`
    pub const DiceRollAfter: Self = Self::engine(49);

    /// `cardPlayed`
    pub const CardPlayed: Self = Self::engine(50);

    /// `targeted`
    pub const Targeted: Self = Self::engine(51);

    /// `paySplitChoose`
    pub const PaySplitChoose: Self = Self::engine(52);

    /// `turnEndBefore`
    pub const TurnEndBefore: Self = Self::engine(53);

    /// `turnEndAfter`
    pub const TurnEndAfter: Self = Self::engine(54);

    /// `paySplitAdd`
    pub const PaySplitAdd: Self = Self::engine(55);

    /// `paySplitMul`
    pub const PaySplitMul: Self = Self::engine(56);

    /// `paySplitAt`
    pub const PaySplitAt: Self = Self::engine(57);

    /// `discarded`
    pub const Discarded: Self = Self::engine(58);

    /// `deckBeforeGame`
    pub const DeckBeforeGame: Self = Self::engine(59);

    /// `deckAtGameStart`
    pub const DeckAtGameStart: Self = Self::engine(60);

    /// `drew`
    pub const Drew: Self = Self::engine(61);

    /// `reshuffled`
    pub const Reshuffled: Self = Self::engine(62);

    /// `bought`
    pub const Bought: Self = Self::engine(63);

    /// `settleBody`
    pub const SettleBody: Self = Self::engine(64);

    /// `beforeOut`
    pub const BeforeOut: Self = Self::engine(65);
    /// Alias of [`Self::BeforeOut`] -- `kinds.rs` names the constructor
    /// `BeforeBankrupt`.
    pub const BeforeBankrupt: Self = Self::engine(65);

    /// `teleported`
    pub const Teleported: Self = Self::engine(66);

    /// `diceRollPlan`
    pub const DiceRollPlan: Self = Self::engine(67);

    /// `abnormalGuard`
    pub const AbnormalGuard: Self = Self::engine(68);

    /// `immuneAll`
    pub const ImmuneAll: Self = Self::engine(69);

    /// `untargetable`
    pub const Untargetable: Self = Self::engine(70);

    /// `redirect`
    pub const Redirect: Self = Self::engine(71);

    /// `effect`
    pub const Effect: Self = Self::engine(72);

    /// `markerSpent` -- any named-counter spend just committed (`spend_marker`).
    /// Payload: `name` / `value`. Fire-pot is one name of many.
    pub const MarkerSpent: Self = Self::engine(73);

    /// `cardUsed` -- a field card's `On::Play` entry is about to run (a card
    /// living on the player's field rather than in hand). Payload: `card` /
    /// `cards` / `by_card`.
    pub const CardUsed: Self = Self::engine(74);

    /// `houseAdded` -- a deed's house count has just been written.
    /// Payload: `tile` / `value`.
    pub const HouseAdded: Self = Self::engine(75);

    /// `counterChanged`
    pub const CounterChanged: Self = Self::engine(76);

    /// `drewBefore`
    pub const DrewBefore: Self = Self::engine(77);

    // Wire id 94 retired; never reuse.

    /// `buyGate`
    pub const BuyGate: Self = Self::engine(79);

    /// `buyAdd`
    pub const BuyAdd: Self = Self::engine(80);

    /// `buyMul`
    pub const BuyMul: Self = Self::engine(81);

    /// `buySet`
    pub const BuySet: Self = Self::engine(82);

    /// `buyAssign`
    pub const BuyAssign: Self = Self::engine(83);

    /// `payCombinedAdd`
    pub const PayCombinedAdd: Self = Self::engine(84);

    /// `payCombinedMul`
    pub const PayCombinedMul: Self = Self::engine(85);

    /// `payCombinedCancel`
    pub const PayCombinedCancel: Self = Self::engine(86);

    /// `tileResolved`
    pub const TileResolved: Self = Self::engine(87);

    /// `moveResolved`
    pub const MoveResolved: Self = Self::engine(88);

    /// `bankruptResolved`
    pub const BankruptResolved: Self = Self::engine(89);

    /// `moveBefore`
    pub const MoveBefore: Self = Self::engine(90);

    /// `moveAfter`
    pub const MoveAfter: Self = Self::engine(91);

    /// `markerSpend`
    pub const MarkerSpend: Self = Self::engine(92);

    /// `markerGain`
    pub const MarkerGain: Self = Self::engine(93);

    /// `actGate`
    pub const ActGate: Self = Self::engine(95);

    /// `payGate`
    pub const PayGate: Self = Self::engine(96);

    /// `moveGate`
    pub const MoveGate: Self = Self::engine(97);

    /// `exclusiveUse`
    pub const ExclusiveUse: Self = Self::engine(98);

    /// `turnBonusTime`
    pub const TurnBonusTime: Self = Self::engine(99);

    /// `turnBaseTime`
    pub const TurnBaseTime: Self = Self::engine(100);

    /// `playGate`
    pub const PlayGate: Self = Self::engine(101);

    /// `diceRollValue`
    pub const DiceRollValue: Self = Self::engine(102);

    /// `atEnd`
    pub const AtEnd: Self = Self::engine(103);

    // Wire id 104 retired; never reuse.

    /// `paySplitChooseBefore`
    pub const PaySplitChooseBefore: Self = Self::engine(105);

    /// `payCombinedAddBefore`
    pub const PayCombinedAddBefore: Self = Self::engine(106);

    /// `paySplitMulBefore`
    pub const PaySplitMulBefore: Self = Self::engine(110);

    /// `onBoardGate`
    pub const OnBoardGate: Self = Self::engine(108);

    /// `targetableGate`
    pub const TargetableGate: Self = Self::engine(109);

    /// Engine-namespace kind. The fixed generic raise points.
    pub const fn engine(id: i32) -> Self {
        Self {
            ns: KindNs::Engine,
            id,
        }
    }

    /// Ruleset-namespace kind. Rule-defined raise points, keyed by the
    /// ruleset's stable name; they cannot collide with a future engine id or
    /// with another ruleset's ids.
    pub const fn ruleset(key: &'static str, id: i32) -> Self {
        Self {
            ns: KindNs::Ruleset(key),
            id,
        }
    }

    /// Is this a filter kind? Filter kinds are modifier-pipeline /
    /// value-shaping stages whose result the host folds back (pay amount,
    /// buy price, turn clock, roll plan, charge-cost substitute), plus the
    /// decision gates (pure yes/no or target/reason folds).
    /// A filter entry may only list these; an action entry only non-filter kinds.
    ///
    /// For [`KindNs::Engine`] this reads the side declaration from
    /// `game_kinds` (the table `crates/` owns). For [`KindNs::Ruleset`] it
    /// answers `false`: a rule-defined kind's side is not in this table. The
    /// host must treat [`OnKind::Filter`] / [`OnKind::Action`] as authoritative
    /// for rule-defined kinds (a later phase emits a registration table).
    pub const fn is_filter(self) -> bool {
        if !matches!(self.ns, KindNs::Engine) {
            return false;
        }
        game_kinds::is_filter(self.id)
    }

    /// The trigger-kind wire spelling.
    ///
    /// Engine kinds keep their camelCase spelling from the `game_kinds` table;
    /// a ruleset-declared id is `"r:{key}:{id}"` (e.g. `"r:bandori:21"`),
    /// formatted -- no table. An unregistered engine id reads as `""`.
    pub fn spelling(self) -> Cow<'static, str> {
        match self.ns {
            KindNs::Ruleset(key) => Cow::Owned(format!("r:{key}:{}", self.id)),
            KindNs::Engine => Cow::Borrowed(game_kinds::wire(self.id)),
        }
    }

    /// Inverse of [`Self::spelling`]. Generic camelCase names and
    /// `"r:{key}:{id}"` parse; anything else is [`Self::None`].
    /// A bare decimal `"21"` is **not** accepted -- only the real spelling.
    pub fn from_str(s: &str) -> Self {
        if let Some(rest) = s.strip_prefix("r:") {
            if let Some((key, id)) = rest.split_once(':') {
                if !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()) {
                    let id = id.parse::<i32>().unwrap_or(0);
                    // Only declared ruleset keys parse; an unknown namespace is
                    // a miss rather than a leaked `&'static str` per call. A
                    // second ruleset declares its key next to `BANDORI`.
                    return match key {
                        "bandori" => Self::ruleset("bandori", id),
                        _ => Self::None,
                    };
                }
            }
            return Self::None;
        }
        match game_kinds::from_wire(s) {
            Some(id) => Self::engine(id),
            None => Self::None,
        }
    }
}

pub fn pack(ptr: u32, len: u32) -> i64 {
    (((ptr as u64) << 32) | len as u64) as i64
}

pub fn unpack(v: i64) -> (u32, u32) {
    let v = v as u64;
    ((v >> 32) as u32, (v & 0xFFFF_FFFF) as u32)
}

/// One entry of the guest card manifest (`rt::manifest`). The index in the
/// array is the card handle the host passes back. Shared with the host so the
/// two sides cannot disagree on the wire layout.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ManifestEntry {
    pub id: String,
    /// The card's entry points, in declaration order (the `entry` index the
    /// host passes back to `ruleset_on`).
    pub on: Vec<ManifestOn>,
    /// The card's declared static **properties** (`CardDef::props`), as
    /// `(key, value)` pairs sorted by key for deterministic wire bytes. The
    /// keys the engine reads are named in [`prop`]; a key a card does not
    /// declare reads as its default (`0`). Host side becomes a `BTreeMap`.
    #[serde(default)]
    pub props: Vec<(String, i32)>,
    /// Tile texts this card's entries speak (`CardDef::texts`). v59.
    /// Trailing field -- postcard is not self-describing, so adding it is an
    /// ABI bump.
    #[serde(default)]
    pub texts: Vec<ManifestTileText>,
    /// Board role of a standing rule (`CardDef::board_role`, v66): `""`
    /// (ordinary card), `"tile"`, `"mark"`, or `"status"`. Trailing field.
    #[serde(default)]
    pub board_role: String,
    /// Tile kinds this rule governs (`CardDef::governs`, v66) when
    /// [`Self::board_role`] is `"tile"`. Empty derives a single kind from the
    /// id's `tile:` suffix. Trailing field.
    #[serde(default)]
    pub governs: Vec<String>,
    /// Refusal message keys for the capability gates this rule closes
    /// (`CardDef::gate_msgs`, v89): `(gate kind i32, i18n key)`. Play path
    /// (`cannot_play`). Trailing field.
    #[serde(default)]
    pub gate_msgs: Vec<(i32, String)>,
    /// Card prop key that exempts a play from this rule's act gate
    /// (`CardDef::exemption_prop`, v89). Empty = none. Trailing field.
    #[serde(default)]
    pub exemption_prop: String,
    /// Turn-skip message keys (`CardDef::gate_skip_msgs`, v89):
    /// `(gate kind i32, i18n key)` -- the two skip keys.
    /// Trailing field.
    #[serde(default)]
    pub gate_skip_msgs: Vec<(i32, String)>,
}

/// One per-entry **tile text** declaration (`CardDef::texts`, v59): the
/// deed-popup description an entry speaks for.
///
/// * `entry` -- the manifest `on` index the text hangs off.
/// * `slot` -- [`text_slot`] (`"settle"` / `"pass"`).
/// * `tile` -- `""` = the instance's own tile (a `tile:*` rule bound to one
///   tile); otherwise a board tile *name* (`"CiRCLE"`, `"CiRCLE 咖啡厅"`) for a
///   tile-bound text on a tile-less instance (`event:*`).
/// * `key` -- i18n key in the `ui` namespace (`deed.note.*`); `""` suppresses
///   the slot (the effective text is nothing).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ManifestTileText {
    pub entry: i32,
    pub slot: String,
    pub tile: String,
    pub key: String,
}

/// One entry point in the manifest: what it is and which trigger kinds it
/// answers (empty for non-trigger entries).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ManifestOn {
    /// An [`OnKind`] as `i32`.
    pub kind: i32,
    /// `TriggerKind` wire **spellings** (`TriggerKind::spelling`): engine
    /// camelCase or `"r:{key}:{id}"`.
    pub triggers: Vec<String>,
    /// Guard **condition** source (docs/GUARDS.md §4.3), CEL over the §4.2
    /// window/candidate vocabulary. `None` = no condition (the guard alone
    /// decides). Compiled once at ruleset build (`RuleError::BadPre` on any
    /// parse / unknown-var / float error); the compiled lean form
    /// (`Cond::to_bytes(false)`) is what the runtime-only browser path loads.
    /// Never `skip_serializing_if`: postcard is not self-describing and would
    /// misalign the next field.
    pub pre: Option<String>,
    /// Does the residual wasm guard exist? G4 deletes a guard whose whole body
    /// moved into `pre`; the host then skips the `OP_GUARD` instantiation
    /// (`admits_pre`). `true` for every pre-G4 entry.
    pub has_guard: bool,
    /// Does this entry collect an activation argument before its body
    /// (`On::declare`, docs/ACTIVATION.md §2)? The host then runs
    /// [`export::OP_DECLARE`] before the point of no return, stores the
    /// postcard bytes, and serves them back on [`export::OP_RUN`] through
    /// `ruleset_declared_arg`. Never `skip_serializing_if`: postcard is not
    /// self-describing. v98. `On::Filter` is never `true`.
    pub has_declare: bool,
    /// Stable **internal** identifier for cross-card applicability queries
    /// (`ctx::query_guard`, docs/ACTIVATION.md §3). An ASCII slug matching
    /// `[A-Za-z0-9_][A-Za-z0-9_.:-]*`; `None` = not externally addressable.
    /// Never display text -- that is [`Self::label`]. v98.
    /// Never `skip_serializing_if`: postcard is not self-describing.
    pub id: Option<String>,
    /// Message names this [`OnKind::Message`] entry answers (empty otherwise).
    #[serde(default)]
    pub messages: Vec<String>,
    /// Opaque display text for the 「效果适用」 line -- 「（1）」 / 「（2）」 and
    /// kin -- so the line can name which clause of a multi-entry card ran.
    /// `None` = unlabelled (the line shows just the card name). v52.
    /// Not a key into the card text; source is [`On::label`] on the entry.
    #[serde(default)]
    pub label: Option<String>,
    /// How this entry presents itself in the log (`CardDef::silent` /
    /// `CardDef::quiet_when_empty`). See [`Presentation`]. v54.
    #[serde(default)]
    pub presentation: Presentation,
    /// Whether the activation flashes (`CardDef::flash`). See [`Flash`]. v55.
    #[serde(default)]
    pub flash: Flash,
}

/// What a card entry point is (`card_sdk::On`'s variants).
///
/// The wire mirror of `card_sdk::On`: each variant is one manifest entry
/// (`ManifestOn::kind`), and the entry's other fields carry its arguments.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnKind {
    /// `On::Play(pre, why_not, body)` -- play this card from hand (or press it
    /// as a skill via `invoke_skill`). `pre` is the gate's CEL condition,
    /// `why_not` is the playability query (a pure `Option<Msg>`;
    /// `None` = playable, `Some(why)` = blocked with a reason), `body` is the
    /// effect. The 「…时」 clause the counteraction prompt shows is a locale key
    /// attached with the `.counter_reason(key)` builder (absent when this entry
    /// is not a [反击] window). The host calls it between the
    /// [`TriggerKind::Card`] raise and `cardAfter`.
    Play = 0,
    // 1 was `CantPlay`, folded into `Play`'s gate.
    /// `On::Counteract(kinds, pre, guard, body)` -- a [反击] at those
    /// [`ChainKind`]s. `pre` (CEL) and `guard` (the residual pure `bool`
    /// query, `None` once G4 folded the whole body into `pre`) decide whether
    /// the card is offered in the hand window; `body` resolves against the
    /// answered link. The 「…时」 clause is a locale key attached with the
    /// `.counter_reason(key)` builder. Called at the counteraction offer, then
    /// again at LIFO resolution.
    Counteract = 2,
    /// `On::Action(Action::Kind(pre, guard, body))` -- an **event reaction** at
    /// one [`ActionKind`] (v74 rename of `On::Hook`; wire id 3 unchanged). Same
    /// `pre`/`guard`/`body` shape as [`Self::Counteract`], plus the optional
    /// `.counter_reason(key)` builder. The host calls it **automatically** for
    /// every placed card (placement order per player) whenever the kind is
    /// raised -- no window. May emit effects; does not modify the triggering
    /// value. Kinds must be non-filter (`TriggerKind::is_filter() == false`).
    Action = 3,
    // 4 was `AtEnd`, folded into `On::Action(Action::AtEnd(…))` in v76.
    // Wire id 4 is retired -- never reuse.
    // 5 was `DiceRollPlan`, folded into `On::Filter(Filter::DiceRollPlan(…))`
    // in v76. Wire id 5 is retired -- never reuse.
    // 6 was `Gate`, folded into `On::Filter(Filter::ActGate(…))` etc. at the
    // gate kinds in v77. Wire id 6 is retired -- never reuse.
    /// v31: a rule's **settle body** (`docs/TILES.md`) -- `On::Settle(pre, guard,
    /// body)`. Tile rules (`tile:*`) are one per board tile kind and this is what
    /// runs when that tile is 结算d, inside the settle chain (a
    /// [`TriggerKind::SettleBody`] cancel replaces it). Condition / residual
    /// guard (v49) decide whether the body governs this settle.
    Settle = 7,
    /// v50: `On::Message(names, pre, guard, body)` -- this card answers a
    /// cross-card message (`ctx::send`). `names` are the message names it
    /// answers; the same category → condition → residual guard → body layering
    /// as [`Self::Action`]. The handler reads the sender and payload from
    /// `ctx::message` and returns a reply value. Does **not** flash the card
    /// (internal plumbing); the handler may log.
    Message = 8,
    /// v73: a rule's **mortgage body** (`On::Mortgage(pre, guard, body)`) --
    /// what runs when the deed this rule instance governs is [抵押]d. The body
    /// owns eligibility (规则书 基础规则 5) and the payout; the engine keeps the
    /// flag write + direct money move (`ctx::apply_mortgage`).
    Mortgage = 9,
    /// v73: a rule's **redeem body** (`On::Redeem(pre, guard, body)`) --
    /// 规则书 基础规则 6. Same split as [`Self::Mortgage`].
    Redeem = 10,
    /// v74: `On::Filter(FilterEntry)` -- a **pure value / decision / gate /
    /// payload rewrite** at one modifier [`FilterKind`]
    /// (`TriggerKind::is_filter()`). v93: one kind per entry, built by the
    /// `filter_kinds!` constructors; the body shape is
    /// [`crate::kinds::FilterShape`]. Id 11, not 9 (9/10 are the v73 deed
    /// kinds).
    Filter = 11,
    /// v80: a rule's **buy body** (`On::Buy(pre, guard, body)`) -- what runs
    /// when the deed this rule instance governs is bought (settle offer,
    /// card-driven `ctx::buy`, agent branch, acquire / auction / card kinds).
    /// The body owns eligibility, the quote, and the order of steps; the
    /// engine keeps the money pipeline and deed write as `ctx` primitives.
    /// Force-buy remains a direct transfer outside effects (规则书 其他规则 6).
    Buy = 12,
    /// v80: a rule's **build body** (`On::Build(pre, guard, body)`) -- what
    /// runs when a house is built on the deed this rule instance governs.
    /// Same split as [`Self::Buy`].
    Build = 13,
}

/// Filter payloads live in [`crate::payload`] -- one typed struct per kind
/// family (`PlanValue` / `RollValue` / `DealValue` / `GateValue` /
/// `PayAddValue` / `RedirectValue`). The 28-field `FilterValue` bag and its
/// `UNCHANGED` sentinel are gone (ABI v92).
impl OnKind {
    pub fn from_i32(v: i32) -> Option<Self> {
        Some(match v {
            0 => Self::Play,
            2 => Self::Counteract,
            3 => Self::Action,
            // 4 / 5 / 6 retired (AtEnd / DiceRollPlan / Gate) -- see the enum docs.
            7 => Self::Settle,
            8 => Self::Message,
            9 => Self::Mortgage,
            10 => Self::Redeem,
            11 => Self::Filter,
            12 => Self::Buy,
            13 => Self::Build,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{KindNs, TriggerKind};

    /// ABI v85: wire id 18 (`skillTeleport`) is retired forever. Do not reuse
    /// the id or the spelling; listeners discriminate with `MoveAfter` + cond
    /// (`move.kind == Teleport && move.by_skill`).
    #[test]
    fn skill_teleport_id_18_is_retired_forever() {
        assert_eq!(
            TriggerKind::engine(18).spelling(),
            "",
            "id 18 must remain unnamed"
        );
        assert_eq!(
            TriggerKind::from_str("skillTeleport"),
            TriggerKind::None,
            "skillTeleport spelling is retired"
        );
        // Nearby ids: "teleport" (id 17) retired in v84, and live ids keep
        // working.
        assert_eq!(TriggerKind::from_str("teleport"), TriggerKind::None);
        assert_eq!(TriggerKind::MoveAfter.spelling(), "moveAfter");
    }

    /// ABI v90/v91: the two namespaces keep distinct wire spellings and the
    /// `"r:{key}:{id}"` form round-trips. A bare decimal is **not** a spelling.
    /// CEL `kind` is that spelling (v91); there is no int pack.
    #[test]
    fn namespaced_wire_spelling() {
        assert_eq!(
            TriggerKind::ruleset("bandori", 21).spelling(),
            "r:bandori:21"
        );
        assert_eq!(
            TriggerKind::ruleset("bandori", 107).spelling(),
            "r:bandori:107"
        );
        assert_eq!(
            TriggerKind::from_str("r:bandori:21"),
            TriggerKind::ruleset("bandori", 21)
        );
        // Bare decimals are not spellings -- only "r:{key}:{id}" parses.
        assert_eq!(TriggerKind::from_str("21"), TriggerKind::None);
        assert_eq!(
            TriggerKind::from_str("paySplitAdd"),
            TriggerKind::engine(55)
        );
        // Engine and ruleset with the same numeric id are different kinds.
        assert_ne!(TriggerKind::engine(21), TriggerKind::ruleset("bandori", 21));
        assert_eq!(TriggerKind::ruleset("bandori", 21).ns, KindNs::BANDORI);
        // Two rulesets with the same id are different kinds.
        assert_ne!(
            TriggerKind::ruleset("bandori", 21),
            TriggerKind::ruleset("other", 21)
        );
        assert!(
            !TriggerKind::ruleset("bandori", 55).is_filter(),
            "ruleset is never is_filter"
        );
    }
}
