//! Host/guest ABI. Shared verbatim by the guest (ruleset.wasm) and the host
//! (`game-rules`). Bump [`ABI_VERSION`] on any incompatible change; the host
//! refuses rulesets built against a different version.

#[cfg(target_arch = "wasm32")]
use alloc::{string::String, vec::Vec};

/// Increment on any change to imports, exports, or their semantics.
/// v2: `play_card` (cross-module card calls); one module per card.
/// v6: full `TriggerKind` set + `trig_target` / `trig_tile` / `trig_value`.
/// v7: board/hand/status query + op wave (is_buyable ... in_band, spend_fire,
///     sweep_to_deck, trig_card_is; `Trigger.card`).
/// v8: `bandori_cant_play` export (`CardDef.cant_play`) + `add_to_deck_at`.
/// v9: `cant_move` (H.MoveWhyNot), `hand_size`, `is_ring`/`is_circle`/`is_live_house`.
/// v10: `trig_step` (`Trigger.step`) so a counteraction can tell which turn step
///      (0/1/2/3) it fired in, for kinds that aren't step-specific (e.g.
///      `mortgage` fires during both step 1 and step 3); paired
///      `*Before`/`*After` kinds filling in the missing half of every
///      trigger point, plus new hooks for buy/build/discard/end turn/leave.
/// v11: `trig_by_card` (`Trigger.by_card`) -- the player whose card caused this
///      trigger, or -1 when it was not card-caused. This is what `H.HitByOtherCard`
///      keys on (`by_card >= 0 && by_card != player_id`).
/// v12: `trig_set_pay_amount` -- a counteraction to a `pay`/`paid` trigger may rewrite
///      the pending amount (0 = cancel the payment, C# `t.Pay.cancel` / `PayCtx.amount`).
///      Also: `set_move_roll` now actually reaches the engine (the write-back from
///      the counteraction's trigger to `Trigger.value` was missing).
/// v13: `trig_pay_is_rent` (`t.Pay.IsRent`) -- is this `pay`/`paid` trigger rent,
///      as opposed to a buy/build/forced loss. Card-driven payments are never rent.
/// v14: `t.Move` context -- `trig_move_flags` (a [`MoveFlags`] bitset), plus
///      `trig_move_main` and `trig_move_dir`. A move-caused trigger (moveRoll /
///      pass / settle*) can now say what kind of move it was, and any number of
///      orthogonal modifiers on it. Replaces the bespoke `trig_move_fire_roll`.
/// v15: `trig_set_cancelled` / `trig_cancelled` (`Trigger.Cancelled`) -- a
///      counteraction may negate the trigger's effect outright (C# `trigger.Cancelled
///      = true`): the engine then skips the effect body (land / event / play)
///      but still runs the point's Before/After hooks.
/// v16: `trig_set_pay_target` -- a counteraction may also redirect the payee of a
///      pending `pay` (C# `PayCtx.to`; -1 = the bank). The transfer amount
///      follows `set_pay_amount`, so a reduced payment credits the payee less too.
/// v17: field-card (`Fx`) hooks + per-card crystals. New `TriggerKind`s
///      (`TurnEnd`, `Drawn`, `PassTile`, `PayAfter`, `RollAfter`, `CardPlayed`,
///      `Targeted`, `PayChoose`) are **hook points**, not [反击] points: the
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
///      `turn_main_steps`, `add_fire_max`, `card_replayable` (C# `H.CanReplay`).
///      `play_card` now returns the inner card's `Dest`, and the inner card runs
///      as itself (its own id and `Dest`, no longer the outer card's).
/// v21: `CardDef` standardized like triggers -- `id` + a table of `On` entry
///      points (`Play(gate, effect)`, `Counteract(kinds, guard, effect)`,
///      `Hook(kinds, effect)`, `AtEnd`). The manifest lists each entry with
///      its trigger kinds, so the host dispatches only to cards that declared
///      the kind at hand; one export `bandori_on(card, entry, op, player_id)`
///      replaces `bandori_play` / `_can_react` / `_react` / `_cant_play` (the
///      pre-v21 names, kept here as history).
/// v23: hook kinds from the C# call sites -- TurnEndBefore / TurnEndAfter,
///      PayAdd / PayMul / PayAt (the Money pipeline), Discarded, DeckBeforeGame /
///      DeckAtGameStart, Drew, Reshuffled, Bought, SettleBody, BeforeOut,
///      Teleported. Move payload `trig_move_remaining` / `trig_move_total` and
///      `MoveFlags::TELEPORT_WALK`. `schedule_turn_end` takes a mode (bit 1 =
///      the player's next turn, bit 2 = before the wear-off, C# `AtEnd`).
///      RollAfter now fires before the moveRoll [反击] window (C# order).
///      `RollPlan` is the point `On::RollPlan` (v22) is dispatched at.
/// v24: movement shaping -- `set_steps` / `set_reverse` / `set_signed` /
///      `set_stop_at` / `set_parity` / `set_resolve` / `set_settle_tile` /
///      `set_pay_factor` / `set_rent_factor` / `set_no_buy` / `set_can_build` /
///      `set_can_build` / `set_teleport_walk` / `set_min_roll` /
///      `set_extra_steps` / `set_more_steps` / `set_fire_roll` /
///      `set_no_circle_reward` / `set_settle_as_agent` and the `move_*` getters,
///      on the move being planned (guest: `ctx::plan::*`).
/// v25: card-driven `give_stay` / `give_stun` / `give_exile` / `teleport_to`
///      go through the C# `AbnormalGate`: the `abnormalGuard` hook (new kind),
///      then the `abnormal` [反击] window when another player caused it; a blocked
///      effect does not apply. `abnormal_count(player_id)` (C# `_abnormalTurn`),
///      `placed_tile(player_id, id)`, `play_doubled()` (C# `PlayCtx.Doubled`) and
///      `trig_cards` (the drawn cards on a `drew` trigger).
/// v26: the C# targeting pipeline -- `target(player_id, tile, single)` pauses the
///      run with a `Target` host request (`H.Target` / `H.TargetTile`): exile,
///      the `immuneAll` / `untargetable` / `redirect` guard hooks (new kinds),
///      the `_targeted` counter (`targeted_count`), then the `targeted` hooks
///      and the `target` [反击] window. `immuneAll` also gates card-driven
///      abnormal effects and payments (C# `ImmuneAll`). `sweep_to_deck`
///      became `shuffle_into_deck(player_id, hand, discard)`.
/// v27: the move model. `MoveFlags` is gone: a move is exactly one `MoveKind`
///      (Walk | Teleport), and what it resolves is a separate category,
///      `Settle` (ROUTE = the tiles it passes resolve [经过]; DEST = the landing
///      resolves [结算]). C# `TeleportWalk` becomes a Teleport whose destination
///      is computed from the roll; C# `Resolve = false` clears DEST. FIRE_ROLL
///      is card-owned state now (`plan::set_tag` / `trigger::move_tag`), not an
///      engine flag. `ctx::card_move` / `ctx::agent_landing` run a move or an
///      agent landing from inside a card; the plan gained `set_kind` / `set_tag`
///      / `set_start` / `set_teleport_to` and the Base/Dice roll tables
///      (`set_base_dice` / `add_base_dice` / `add_extra_dice` -- a flat add is a
///      `0`-sided term, which is what C# `Bonus` was).
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
///      `band_crystals` / `add_band_crystals` no longer touch the keyed state
///      `bandCrystals` (deleted, along with `MatchPlayer::band_crystals`): they
///      are sugar over the player's band-skill field card (`skill:<band>:<skill>`,
///      `FieldCard::band_skill`) -- the same instance `crystals` / `add_crystals`
///      touch from inside a band skill's own handler, so 「乐队卡 / 团卡」 crystal
///      text is one pool. `max` = 0 now means uncapped (it used to clamp the
///      count to 0); `add_band_crystals` returns the new count (it used to
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
///      `handLimitDelta` (C# `Card.HandLimitDelta`) and `playableStunned`
///      (C# `Card.PlayableStunned`). The host keeps them on `CardInfo`, the
///      engine queries `CardRules::card_prop(card, key)` (default 0) and
///      stamps the whole map onto the `FieldCard` at placement.
/// v31: tile rule instances (`docs/TILES.md`) -- `OnKind::Settle` (a rule's
///      settle body), the tile-data `prop` keys (`price` / `house` / `group` /
///      `buildMax` / `rentLen` / `rent:N` / `ringMult`), `place_card` on
///      `BOARD_OWNER`, and the `ctx::settle_*` primitives.
/// v32: the tile-bending surface replaces the engine's tile flags
///      (`docs/TILES.md` Phase 3). `tile_prop` / `set_tile_prop` read and write
///      a rule instance's props **by tile**; `settle_circle_reward` is the
///      [经过] CiRCLE reward (`tile:circle`'s Pass entry). `prop::NO_REWARD`
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
///      [`roll_source`]`::{NONE,FIRE,CARD,SKILL}`. `ctx::roll` is unchanged
///      (no window); a roll that [反击]s must answer goes through
///      `ctx::roll_ask` / `ctx::do_move_roll_ask`.
/// v34: `prop::RENT_HOUSES` (virtual rent-house-count, 「房屋数视为…」 -- gone
///      with a placed card) and `ctx::rent_houses_of` (the counted value the
///      rent lookup reads). `ctx::houses_of` stays real.
/// v35: the skill / band-skill / follow surface.
///      (a) `invoke_skill(player_id, id)` -- run a skill rule's press entry
///      (`On::Play`) for a player, nested like `play_card`. This is 「立即执行
///      乐队技能的（2）效果」 (mutsumi_never) and 「触发其技能的发动」 (pareo_far):
///      the C# direct method calls (`BandCrychic.TransformNow()` /
///      `SkillPareo -> Offer()`). No `skillUsed` raise -- that is the player's
///      own press (`use_skill`), not a card running the body.
///      (b) the skill **attachment surface** (C# `H._fx[i].bands` / `.skill`):
///      `band_skill(player_id)` / `character_skill(player_id)` name the bound
///      rule id, `band_skills(player_id)` lists every band attachment as
///      `(uid, id, extra)`, `add_band_skill(player_id, id, extra)` attaches one
///      (C# `H.MakeBand`). `extra` is 「拿取」's borrowed copy: 「相同乐队技能卡
///      的效果不可叠加」 (the hook dispatch skips an extra when a non-extra copy
///      of the same id is already attached) and 「不视为那个乐队的角色」
///      (`in_band` still reads only the character).
///      (c) `raise_bought(player_id, tile)` -- a card that hands a deed over
///      (tomoe_savior's 「从该玩家处收购该地契」) announces the acquisition so
///      the `bought` hook chain hears it (C# `f.Bought(i, t)`).
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
/// v38: the two [CP点] kinds (user ruling 2026-10-07). **Tile marks** stay the
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
///      `BUY_DISCOUNT` / `FREE_BUY` / `RAZE_ON_BUY` stay for one ABI (P5
///      deletes them and the `TurnCtx` flags together). SAVE_VERSION 3 → 4
///      (the `TurnCtx.lingering` field enters the save).
/// v41: the **removals**. `ctx::set_buy_discount` / `ctx::set_free_buy` /
///      `ctx::set_raze_on_buy` and the `TurnCtx` fields they wrote are gone,
///      replaced by `ctx::linger` + the `BuyAdd` / `BuyMul` / `BuySet` /
///      `BuyAssign` hooks. The old global / per-player colour writers and
///      their state key are gone, replaced by the `prop::ANY_COLOR` /
///      `prop::COLOR_FOR_PREFIX` tile
///      props. The retired props `BUY_DISCOUNT` / `FREE_BUY` / `RAZE_ON_BUY`
///      go with them.
/// v42: the payment command's **pre-split** stage and the terminal hooks
///      (`PIPELINE-AUDIT` Q2 / Q6). `TriggerKind`s `PayTotalAdd` / `PayTotalMul`
///      / `PayTotalCancel` (wire names `payTotalAdd` / `payTotalMul` /
///      `payTotalCancel`) run command-wide on the figure **before** any
///      「[分摊]」 divides it -- 「分摊前」 -- alongside the existing per-share
///      `PayAdd` / `PayMul` / `PayChoose` / `PayAt`. `ctx::pay_total` /
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
///      「移动后」/「主要移动结束时」/「[移动终点]」 clauses land here. The
///      roll-specific `rollPlan` / `moveRoll` and the teleport-specific
///      `teleport` / `teleported` stay where they are; `moveResolved` stays the
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
pub const ABI_VERSION: i32 = 47;

/// Wasm import module name for every host function.
pub const IMPORT_MODULE: &str = "bandori";

/// Guest exports.
pub mod export {
    pub const ABI_VERSION: &str = "bandori_abi_version";
    /// `() -> i64` packed `(ptr << 32) | len` pointing at a UTF-8 JSON manifest.
    pub const MANIFEST: &str = "bandori_manifest";
    /// `(card: i32, entry: i32, op: i32, player_id: i32) -> i64` -- call entry
    /// `entry` (an index into the card's manifest `on` list). `op` is
    /// [`OP_RUN`], [`OP_GUARD`] or [`OP_LEGACY_GUARD`]. Returns 0, the guard's
    /// 0/1, or (for a `Play` gate) a packed `(ptr << 32) | len` postcard `Msg`
    /// reason with 0 meaning "playable".
    pub const ON: &str = "bandori_on";
    pub const OP_RUN: i32 = 0;
    pub const OP_GUARD: i32 = 1;
    /// G3 migration audit (GUARDS.md §5.1): the card's pre-migration
    /// `legacy_*` guard. Only present while `ManifestOn::has_legacy`; the
    /// `guard-audit` host compares it against `pre ∧ guard` and panics on any
    /// mismatch. Traps / a missing entry mean "no legacy" and skip the check.
    pub const OP_LEGACY_GUARD: i32 = 2;
    pub const MEMORY: &str = "memory";
}

/// The keys the engine's own game flow reads back (its turn rules and the
/// status gates). Content is free to invent more -- a state key is just a
/// string. Mirrors `game_core::state::key`.
pub mod state_key {
    /// `[停留]` -- layers that wear off at end of turn.
    pub const STAY: &str = "stay";
    /// `[晕眩]` -- layers that wear off at end of turn.
    pub const STUN: &str = "stun";
    /// `[晕眩]` applied this turn, which only starts counting next turn.
    pub const STUN_START: &str = "stunStart";
    /// `[移除]` -- turns spent off the board.
    pub const EXILE: &str = "exile";
    /// Tile the exile returns to, or -1 for none.
    pub const EXILE_TO: &str = "exileTo";
    /// 「在[除外]层数归0后[传送]至该格子，视为当回合的主要移动」 -- the exile
    /// expiry teleport is that turn's main move (MyGO:无路矢). Read and consumed
    /// by the engine's exile tick.
    pub const EXILE_MAIN: &str = "exileMain";
    /// Fire pots held. Its `max` is the mandated cap, written by the character
    /// skill -- and that is the one number to show.
    pub const FIRE: &str = "fire";
    /// Layers of "may hold no hand cards".
    pub const NO_HAND: &str = "noHand";
    /// Layers of "cannot be stopped".
    pub const UNSTOPPABLE: &str = "unstoppable";
    /// Hand size limit.
    pub const HAND_LIMIT: &str = "handLimit";
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
/// hardcoding the card's own name (the C# checked `CountMarks(t, "高贵的微蓝")`).
pub mod mark {
    /// Carrying tiles cannot be named as a target (`H.TargetTile` answers -1).
    pub const NO_TARGET: &str = "noTarget";

    /// Tile-mark **category**: [CP点], 「放置于路面上的指示物」
    /// (`data/rules.txt` 125). Its own category, not a `kind` among the player
    /// marks. Mirrors `game_core::state::mark_category::CP`. This is the
    /// **tile** kind of [CP点]; the other kind is the on-card count
    /// (`FieldCard::cp`, 「自己[场上]N个[CP点]」) -- user ruling 2026-10-07.
    pub const CP_CATEGORY: &str = "cp";
    /// The [CP点] mark's stable `kind`. Its display label comes from
    /// [`CP_CATEGORY`] (「CP点」), not from this string. Mirrors
    /// `game_core::state::mark_kind::CP`.
    pub const CP_KIND: &str = "mark:cp";
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
    pub const COUNTERACT_FIRE_COST: &str = "counteractFireCost";
    /// Continuous 「手卡上限数量减1」 (C# `Card.HandLimitDelta`) while the card
    /// sits on the field. Stamped onto the field instance at placement and
    /// gone with the card. `-1` cuts the owner's hand limit; a positive value
    /// lifts it; `0` (the default) does nothing.
    pub const HAND_LIMIT_DELTA: &str = "handLimitDelta";
    /// 「可在眩晕时打出」 (C# `Card.PlayableStunned`): `1` = the card skips the
    /// stun gate when played from hand. The exile and no-hand gates have no
    /// such exception in the pool. Default `0` (blocked by stun).
    pub const PLAYABLE_STUNNED: &str = "playableStunned";
    /// Bot-only **estimated execution cost** (user ruling 2026-10-07): what
    /// activating this card is expected to cost the player, in 资金. Read
    /// **only** by bots / autopilot as a reserve check -- never by legality.
    /// Constant for now; an X-dependent cost may later become a `rules-cond`
    /// expression. `0` (the default) means "unknown / assume free".
    pub const EST_COST: &str = "estCost";
    /// 「有[指定]目标」 (C# `Card.Def.Targeting`): `1` = this play names
    /// recipients, so 「取消其对目标之一的[指定]」 applies instead of 「抵消其
    /// 所有的效果」. The named set is the play's **other living players** (the
    /// 「[指定][使用者]以外的所有玩家」 / 「其他玩家[分摊]」 shape). Default `0`
    /// (names nobody).
    pub const DESIGNATES: &str = "designates";
    /// Virtual **rent** house count (「房屋数视为…」, C# `H.RentHouses` + `boosted`).
    /// Presence is the override -- a house count of `0` is a legitimate value,
    /// so the read is `props.get`, not `unwrap_or(0)`. Real `st.houses` is
    /// untouched: build caps, raze, sale and asset value still see the standing
    /// houses. Two write surfaces, matching the two clause shapes:
    /// * `ctx::set_prop` on a placed card of the tile's **owner** -- 「你的所有
    ///   格子上的房屋数视为…」. Gone with the card.
    /// * `ctx::set_tile_prop` on the tile's rule instance -- a tile-scoped
    ///   override. The source arms and disarms it.
    /// The rent lookup (`pay_rent` / `rent_of` / `ctx::rent_houses_of`) reads
    /// it; `ctx::houses_of` stays real.
    pub const RENT_HOUSES: &str = "rentHouses";

    // ---------------------------------------------------------- tile data
    // Stamped onto a board-owned tile rule instance at bind time (from
    // `TileData`), read back by the settle bodies and the end-step buy/build
    // gates. Mirrors `game_core::state::prop`; see `docs/TILES.md`.

    /// Land price (houses are extra). `TileData.price`.
    pub const PRICE: &str = "price";
    /// Build cost per level. `TileData.house`.
    pub const HOUSE: &str = "house";
    /// Colour group (`TileData.group`). [`ALL_COLORS`] is 「该格获得所有颜色」.
    pub const GROUP: &str = "group";
    /// The tile's value that means 「该格获得所有颜色」.
    pub const ALL_COLORS: i32 = -2;
    /// 「每块地有标注的等级上限」 -- max houses. `TileData.rent.len() - 1`.
    pub const BUILD_MAX: &str = "buildMax";
    /// Length of the rent table (levels = houses + 1).
    pub const RENT_LEN: &str = "rentLen";
    /// Rent at level N: `rent:0` … `rent:rentLen-1` (`TileData.rent`).
    pub const RENT_PREFIX: &str = "rent:";
    /// RiNG rent multiplier (`match_rules.ring_multiplier`). TODO(规则书).
    pub const RING_MULT: &str = "ringMult";

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

/// `t.Roll.Source` -- where a `roll` / `moveRoll` face came from. Read by
/// 「当你使用火罐进行掷骰时」 (寄于指尖的执念) and kin. A `roll` / `moveRoll`
/// trigger carries one; every other trigger reads [`roll_source::NONE`].
pub mod roll_source {
    /// Unattributed (the engine's own move roll, or a roll with no named source).
    pub const NONE: i32 = 0;
    /// A [火罐]-funded roll (「使用火罐进行掷骰」).
    pub const FIRE: i32 = 1;
    /// A hand/field card's own roll (`ctx::roll_ask` from a card body).
    pub const CARD: i32 = 2;
    /// A skill press's roll (`ctx::roll_ask` from a skill body).
    pub const SKILL: i32 = 3;
}

/// `i32_exit` status the host uses to abort a run that reached an unanswered prompt.
/// Never observed by the guest.
pub const EXIT_NEED_INPUT: i32 = 0x0B_A0_D0;

/// Prompt kinds, mirroring `MatchPrompt.kind` in the C# (`MatchPrompt.cs`).
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptKind {
    /// One of the pushed options (`AskPick`).
    Choice = 0,
    /// A yes / no question (`AskYes`); the options are implicit.
    YesNo = 1,
    /// Pick a tile index (`AskTileOf`).
    Tile = 2,
    /// Pick a player. C# `H.AskSeat` has no kind of its own -- it is
    /// `AskPick` over player-name options -- so this one is ours.
    Player = 3,
    /// Pick a card id (`AskCard`).
    Card = 4,
}

impl PromptKind {
    pub fn from_i32(v: i32) -> Option<Self> {
        Some(match v {
            0 => Self::Choice,
            1 => Self::YesNo,
            2 => Self::Tile,
            3 => Self::Player,
            4 => Self::Card,
            _ => return None,
        })
    }

    /// The C# `MatchPrompt.kind` string (the C# names: `"pick"`, `"yes"`, ...).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Choice => "pick",
            Self::YesNo => "yes",
            Self::Tile => "tile",
            Self::Player => "player",
            Self::Card => "card",
        }
    }
}

/// How the player gets there -- a move is exactly one of these (C# `MoveCtx.Teleport`
/// vs the walk loop). Mirrors `game_core::engine::move_ctx::MoveKind`.
#[repr(i32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MoveKind {
    /// The player steps along the board, tile by tile.
    #[default]
    Walk = 0,
    /// The player jumps straight to the destination (C# `TeleportMove`). A C#
    /// `TeleportWalk` (「视为 [传送]（只触发终点）」) is this kind with the
    /// destination derived from the roll instead of named.
    Teleport = 1,
}

impl MoveKind {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::Walk),
            1 => Some(Self::Teleport),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Walk => "walk",
            Self::Teleport => "teleport",
        }
    }
}

bitflags::bitflags! {
    /// What a move resolves as it goes -- the settle axis, its own category
    /// beside [`MoveKind`] (C# `m.Resolve` / `m.TeleportWalk` decomposed).
    /// An ordinary move settles both.
    ///
    /// The wire is a flat `i32`; [`Self::bits`] / [`Self::from_bits`] flatten it.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct Settle: i32 {
        /// The tiles the move goes through resolve their [经过] effects. A walk:
        /// every tile it steps on. A teleport: the destination counts as passed
        /// when the teleport actually moves the player (`From != to`), or when
        /// this bit is forced (C# `TeleportWalk`, 「原地也算 [经过]」).
        const ROUTE = 1;
        /// The landing resolves (C# `m.Resolve`): `settleBefore` -> `settle` ->
        /// `land` -> `settleAfter`. On a teleport this gates the destination's
        /// [经过] too (C# 24371 returns before the pass block when `!m.Resolve`).
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

/// An abnormal effect (C# `Abnormal.Kind`), carried on `abnormalGuard` /
/// `abnormal` triggers. C# `AbName` gives their names: [停留] / [晕眩] / [除外] /
/// [传送] / [强制移动] / [强制停下] / 反方向移动.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AbKind {
    Stay = 1,
    Stun = 2,
    Exile = 3,
    Teleport = 4,
    Forced = 5,
    Stop = 6,
    Reverse = 7,
}

impl AbKind {
    pub fn from_i32(v: i32) -> Option<Self> {
        Some(match v {
            1 => Self::Stay,
            2 => Self::Stun,
            3 => Self::Exile,
            4 => Self::Teleport,
            5 => Self::Forced,
            6 => Self::Stop,
            7 => Self::Reverse,
            _ => return None,
        })
    }

    /// The C# `Abnormal.Kind` string.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stay => "stay",
            Self::Stun => "stun",
            Self::Exile => "exile",
            Self::Teleport => "teleport",
            Self::Forced => "forced",
            Self::Stop => "stop",
            Self::Reverse => "reverse",
        }
    }
}

/// One of a player's card piles (C# `_hidden[s].hand` / `.discard` / `.draw`,
/// and the placed field cards).
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardPile {
    Hand = 0,
    Discard = 1,
    /// The draw pile, **top card first**.
    Deck = 2,
    /// Cards placed on the player's field, in placement order.
    Field = 3,
}

impl CardPile {
    pub fn from_i32(v: i32) -> Option<Self> {
        Some(match v {
            0 => Self::Hand,
            1 => Self::Discard,
            2 => Self::Deck,
            3 => Self::Field,
            _ => return None,
        })
    }
}

/// `circleAffected`'s `Trigger.value` -- which half of the CiRCLE reward was
/// taken. `0` is 「[获得]资金」, `1` is the card. The stunned path forces the
/// card, so a money-only clause cannot fire there.
///
/// **Must match the engine's `CIRCLE_REWARD_MONEY` / `CIRCLE_REWARD_CARD` in
/// `game-core/src/engine/play.rs`** (the same relation `Arg` has to
/// `game-core/src/msg.rs`): `game-core` is the source, this is the mirror.
/// Renumbering one means renumbering the other.
pub const REWARD_MONEY: i32 = 0;
pub const REWARD_CARD: i32 = 1;

/// Trigger kinds a counteraction can be checked against.
///
/// The values are a wire enum: the host fills them at the same points the
/// engine raises its `Trigger`s. Kinds the engine does not raise yet still exist
/// here so a card's `can_counteract` can state its real condition; it simply
/// never sees that kind until the engine raises it (TODO in `game-core`).
#[repr(i32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TriggerKind {
    #[default]
    /// Unset. `from_str`'s fallback and the `Default` -- never raised.
    None = 0,
    /// The main move's pre-roll point (行动阶段 8 「移动掷骰前」) -- raised before
    /// the d20 is cast, with `value = -1` as a "no roll yet" sentinel so a
    /// counter matching [`Self::Roll`] / [`Self::MoveRoll`] stays dormant before
    /// the dice exist. Payload: `@m` the move.
    Roll = 1,
    /// The move roll resolved (行动阶段 9–10 「移动掷骰时/后」) -- after the dice,
    /// before the walk. `value` is the face; a counteraction may reroll it with
    /// `set_move_roll` (`t.value` is read back). Payload: `@m` the move.
    MoveRoll = 2,
    /// A turn begins (行动阶段 2 「回合开始时」) -- after the exile/stun status
    /// ticks, so a hook sees the turn as it opens. `turnStartBefore` is the
    /// pre-half. Payload: `tile` = the player's position.
    TurnStart = 3,
    /// One tile stepped over during a move (行动阶段 12 「[经过]」) -- raised after
    /// the player arrives on `tile`, for **every** path tile (CiRCLE and the
    /// destination), not just the end. `passBefore` is the pre-half;
    /// [`Self::PassTile`] is the Fx hook for the same step.
    Pass = 4,
    /// End-tile [重叠] (行动阶段 13) -- one raise per other player sharing the
    /// landing tile. `target` = that player. Raised at the move's end, never
    /// mid-walk (a mid-route pass is [`Self::PassTile`]).
    PassPlayer = 5,
    /// 行动阶段 14 「[触发结算]前」/「[移动终点]」 -- the pre-settle window. A
    /// relocation here redirects the settle (it re-runs this window at the new
    /// tile, `SETTLE-STAGES.md` §7 Q7). Payload: `tile` = the tile about to settle.
    SettleBefore = 6,
    /// 行动阶段 15 opening 「触发结算」/「[结算]时」 -- the settle's declaration
    /// and [反击] window. `set_cancelled()` = 「本次结算」 never happened: no
    /// body, no `settleAfter`; [`Self::TileResolved`] still fires. `target` =
    /// the tile's owner (-1 = the bank).
    Settle = 7,
    /// A deed was just mortgaged, after it applied (运营阶段 「抵押地契」).
    /// `mortgageBefore` is the pre-half that can block. Payload: `tile` = the deed.
    Mortgage = 8,
    /// A payment's settlement hook -- what the payment actually is, after the
    /// [反击] chain and the modifier stages, before any money moves (规则书
    /// 支付阶段 5). `player_id` = payer, `target` = payee (-1 = the bank),
    /// `value` = amount. `set_cancelled()` stops the payment outright (`NEGATION-AUDIT`
    /// V5). The [反击] key for a payment is [`Self::Effect`] (`kind: "pay"`).
    Pay = 9,
    /// Money left a player (规则书 支付阶段 6–7) -- after the deduction commits.
    /// `player_id` = payer, `target` = payee, `value` = what moved. The [反击]
    /// window 「[消耗]或[支付]」/「被…收取资金」 opens here, on a payer-side loss
    /// only -- a print (`gain`) raises [`Self::PayAfter`] alone.
    Paid = 10,
    /// A bankruptcy's asset cash-in is done, before the seat is cleared.
    /// `bankruptBefore` is the pre-half (before cash-in); [`Self::BankruptResolved`]
    /// the terminal. The seat is already marked dead before `bankruptBefore` (B3).
    Bankrupt = 11,
    /// A card is being played from hand -- before its `play` body. `card` = the
    /// card id, `by_card` = its player. `set_cancelled()` negates the play: the
    /// card still goes to its `Dest`, but the body does not run. Also the kind of
    /// every declared [反击] link (`seq` / `answers` say which link it is).
    Card = 12,
    /// An event was drawn and revealed -- before it resolves. `card` = the event
    /// id. `set_cancelled()` negates the draw: the effect never resolves and the
    /// card is filed away. `eventAfter` is the post-half.
    Event = 13,
    /// An abnormal effect settled on `target` (`t.value` is the [`AbKind`]).
    /// The declaration gates are [`GateKind::AbnormalGuard`] + the [`Self::Effect`]
    /// [反击]; this hook reports what actually landed, self-applied included.
    Abnormal = 14,
    /// A single-target effect settled on `target`. Now a settlement hook: the
    /// [反击] key for 「被…效果影响」 is [`Self::Effect`]. Declaration-side gates:
    /// [`GateKind::Untargetable`] / [`GateKind::Redirect`].
    Target = 15,
    /// Wire name `stop`: a player was forced to stop (「强制停下」,
    /// [`AbKind::Stop`]). Not raised by the current engine; kept for wire
    /// compatibility.
    Stop = 16,
    /// Wire name `teleport`: a teleport was performed. Not raised by the current
    /// engine; kept for wire compatibility. The teleport-specific points today
    /// are [`Self::MoveBefore`] (before) and [`Self::Teleported`] (after).
    Teleport = 17,
    /// Wire name `skillTeleport`: 「当你使用技能进行传送后」 (R:必然的联系（莉莎）).
    /// Not raised by the current engine; kept for wire compatibility.
    SkillTeleport = 18,
    /// Wire name `stun`: a [晕眩] layer landed. Not raised by the current engine
    /// (the landing reports as [`Self::Abnormal`] with `t.value` = [`AbKind::Stun`]);
    /// kept for wire compatibility.
    Stun = 19,
    /// Wire name `stay`: a [停留] layer landed. Not raised by the current engine
    /// (see [`Self::Stun`]); kept for wire compatibility.
    Stay = 20,
    /// Wire name `exile`: 「任意玩家获得[除外]…时」 -- an [除外] layer was granted
    /// by any path (火种燃尽之后会怎么样呢？ listens here). Raised from the grant
    /// log, `value = 1`. Distinct from [`Self::Abnormal`], which is the landing
    /// itself.
    Exile = 21,
    /// Wire name `forced`: a [强制移动] landed. Not raised by the current engine
    /// (see [`Self::Stun`]); kept for wire compatibility.
    Forced = 22,
    /// Wire name `state`: 「当有其他玩家切换状态时」 -- a player toggled skill
    /// state (Mujica:欢迎来到ave mujica的世界). Not raised by the current
    /// engine; kept for wire compatibility.
    State = 23,
    /// Wire name `counteracted`: 「有玩家对你使用[反击]后」 -- a [反击] was
    /// declared against `target` by `player_id` (Mujica:无法将视线移开). Not
    /// raised by the current engine; kept for wire compatibility.
    Counteracted = 24,
    /// Wire name `drawOut`: 「回合外受到抽卡效果时」 -- a draw hit you outside
    /// your own turn (CRYCHIC:优雅的呐喊). Not raised by the current engine;
    /// kept for wire compatibility. The per-draw points today are
    /// [`Self::DrewBefore`] / [`Self::Drawn`] / [`Self::Drew`].
    DrawOut = 25,
    /// The CiRCLE reward was picked but not paid out (「[获得]资金」 vs the card).
    /// `value` = which half ([`REWARD_MONEY`] / [`REWARD_CARD`]); the stunned
    /// path forces the card. `set_cancelled()` skips the payout.
    CircleAffected = 26,
    /// Wire name `twoCards`: 「当有人同一回合内打出两张卡时」 (Sumimi:Here the
    /// world). Not raised by the current engine; kept for wire compatibility.
    TwoCards = 27,
    /// v10: the missing pre-half of `TurnStart`.
    TurnStartBefore = 28,
    /// v10: the missing pre-half of `Pass`.
    PassBefore = 29,
    /// v10: the missing pre-half of `Mortgage`.
    MortgageBefore = 30,
    /// v10: the missing pre-half of `Bankrupt`.
    BankruptBefore = 31,
    /// v10: the missing post-half of `Card`.
    CardAfter = 32,
    /// v10: the missing post-half of `Event`.
    EventAfter = 33,
    /// v10: the missing post-half of `SettleBefore`/`Settle` (fires after
    /// `land()` resolves).
    SettleAfter = 34,
    /// v10: new action hook -- before a buy resolves.
    BuyBefore = 35,
    /// v10: new action hook -- after a buy resolves.
    BuyAfter = 36,
    /// v10: new action hook -- before building a house resolves.
    BuildBefore = 37,
    /// v10: new action hook -- after building a house resolves.
    BuildAfter = 38,
    /// v10: new action hook -- before a hand discard resolves.
    DiscardBefore = 39,
    /// v10: new action hook -- after a hand discard resolves.
    DiscardAfter = 40,
    /// v10: new action hook -- before ending the turn.
    EndTurnBefore = 41,
    /// v10: new action hook -- after ending the turn.
    EndTurnAfter = 42,
    /// v10: new action hook -- before a player forfeits.
    LeaveBefore = 43,
    /// v10: new action hook -- after a player forfeits.
    LeaveAfter = 44,

    // v17: field-card hook points. These are NOT [反击] points -- the
    // engine runs every placed card's `counteract` against them automatically. They
    // are distinct kinds from the counteraction kinds above so a card can tell a
    // field effect from a hand counteraction by its kind alone.
    /// A turn just ended (any player's).
    TurnEnd = 45,
    /// **One card was drawn** (per single card; `t.card` names it, still in
    /// hand). This is the drawn card's *own* hook; the per-draw field-card
    /// points are [`Self::DrewBefore`] / [`Self::Drew`].
    Drawn = 46,
    /// The player passed/stopped on a tile during a move (行动阶段 12 「[经过]」).
    PassTile = 47,
    /// A payment settled (规则书 支付阶段 6–7 「资金变动」).
    PayAfter = 48,
    /// A move roll resolved (行动阶段 10 「移动掷骰后」).
    RollAfter = 49,
    /// A card's hand effect resolved.
    CardPlayed = 50,
    /// The player was targeted.
    Targeted = 51,
    /// The player is about to pay (may modify/decline).
    PayChoose = 52,
    /// v23: first step of a turn end (结束阶段前), before the `On::AtEnd`
    /// callbacks and the status wear-off.
    TurnEndBefore = 53,
    /// v23: after [`Self::TurnEnd`]; the scheduled turn-end callbacks run here.
    TurnEndAfter = 54,
    /// v23: a payment's amount, first modifier pass (规则书 支付阶段 2, before
    /// any money moves).
    PayAdd = 55,
    /// v23: second modifier pass (规则书 支付阶段 4), after [`Self::PayAdd`].
    PayMul = 56,
    /// v23: after [`Self::PayChoose`], before the `pay` [反击] window
    /// (规则书 支付阶段 5).
    PayAt = 57,
    /// v23: this card (named on `t.card`) just went to the discard pile.
    Discarded = 58,
    /// v23: **before match start** (「游戏开始前」) -- raised once per
    /// player before the opening hands are drawn, and dispatched to *every*
    /// effect source: field cards including skills (per player, in field order)
    /// and the card ids in that player's piles/hands. Start positions and the
    /// authoritative initial hand size are decided here.
    DeckBeforeGame = 59,
    /// v23: **after match start** (「游戏开始时」) -- raised once per
    /// player after the opening draw and mulligan, dispatched to every effect
    /// source as [`Self::DeckBeforeGame`] is. Initial tokens/resources (fire
    /// pots 「初始N」, P✽P fans) are created here.
    DeckAtGameStart = 60,
    /// v23: **after one card was drawn** (per single card; an
    /// N-card draw raises this N times, each payload naming one card). This is
    /// the per-draw field-card point; the drawn card's own hook is
    /// [`Self::Drawn`].
    Drew = 61,
    /// v23: a player's discard pile was shuffled back into its deck.
    Reshuffled = 62,
    /// v23: a player became the owner of `t.tile` (buy or auction).
    Bought = 63,
    /// v23/31: the **settle body** point -- a field card placed on the tile may
    /// replace the tile's rule instances' effect: do it and call
    /// `trigger::set_cancelled()`. Was `SettleInstead`; renamed when the body
    /// became the tile's rule instances (`docs/TILES.md`).
    SettleBody = 64,
    /// v23: a player is about to leave the game (bankrupt or forfeit).
    BeforeOut = 65,
    /// v23: a teleport finished (after its settlement, or at once if it does not
    /// settle).
    Teleported = 66,
    /// v23: the main move's `RollPlan` pass (行动阶段 8, before the dice are
    /// rolled). The host runs every placed card's `On::RollPlan` here.
    RollPlan = 67,
    /// v25: an abnormal effect is about to hit
    /// `t.target` (`trigger::abnormal_kind()` says which). A field card blocks it
    /// with `trigger::set_cancelled()`. Then, if someone else caused it, the
    /// `effect` [反击] window opens on the declaration; a landing is reported by
    /// [`Self::Abnormal`].
    AbnormalGuard = 68,
    /// v26: is `t.player_id` untouchable by `t.by_card`'s
    /// effects? A field card claims it with `trigger::set_cancelled()`. Asked
    /// before targeting `t.player_id`, before an abnormal effect, and before a
    /// card-driven payment from/to `t.player_id`.
    ImmuneAll = 69,
    /// v26: `t.by_card`'s card is about to
    /// target `t.player_id`. Block with `trigger::set_cancelled()`.
    Untargetable = 70,
    /// v26: a single-target card is about to target
    /// `t.target`; a field card takes the hit with `trigger::set_target(player_id)`.
    Redirect = 71,
    /// v27: **a card effect is declared at someone**. This is the [反击] key for
    /// 「被其他玩家的卡效果影响」 -- one stable fact, not a union of outcome
    /// kinds. It is raised when a link's recipients are named, before any
    /// settlement, and carries the link's effect list (`ctx::effect`).
    ///
    /// The old `Target` / `Abnormal` / `Pay` windows are settlement hooks now:
    /// they fire as the effect settles and can no longer reconstruct this
    /// clause. A counter that means "an effect hit me" listens here and asks
    /// `effect::`; one that means "a payment settled" listens at [`HookKind::PayAfter`].
    Effect = 72,
    /// v28: a house was just added to
    /// `t.tile` (now `t.value` houses) by `t.player_id`.
    HouseAdded = 75,
    /// v28: `t.player_id` just spent `t.value` fire
    /// (「每当你消耗火罐时」). Fires after the spend commits.
    FireSpent = 73,
    /// v28: `t.player_id` just used their character skill
    /// (「使用自己原有的技能（2）时」). `t.card` is the skill rule's id.
    SkillUsed = 74,
    /// v29: a placed card's [奇迹水晶] count was just written (set or add).
    /// `t.card` is the card whose count moved, `t.player_id` its owner, and
    /// `t.value` is the **change applied** (0 = a write that landed on the same
    /// count, e.g. a card placed with none). The count after the write is
    /// `ctx::crystals()` on the instance itself.
    ///
    /// The 「此卡上不再拥有[奇迹水晶]时」 clauses (AG:绯红之魂 (3) and kin) listen
    /// here instead of testing the count at each spend site, so a count that is
    /// emptied by *any* path still leaves the field.
    CrystalsChanged = 76,
    /// v30: **before one card is drawn** (per single card; an N-card draw raises
    /// this N times). `t.card` is the card that would be drawn (the deck's top,
    /// or empty when the pile is dry). This is the per-draw *replacement* point:
    /// a hook that wants to replace the draw calls `trigger::set_cancelled()`
    /// and performs its own look/pick -- whatever it adds to the hand is the
    /// replacement draw, and the after points ([`Self::Drawn`] / [`Self::Drew`])
    /// fire for it (「此次加手视为抽卡动作」). Opening hands do not raise it.
    DrewBefore = 77,
    /// v36: a card instance's attached [CP点] count was just written (placed or
    /// cleared). `t.card` is the card the marks are attached to, `t.player_id`
    /// its owner, and `t.value` is the **change applied** to the attached count
    /// (negative when a [CP点] left). The count after the write is
    /// `ctx::cp_attached()` on the instance itself.
    ///
    /// The 「该清CP了 should be graveyarded as soon as the attached on-card cp
    /// mark is empty」 rule (user ruling 2026-10-07) listens here rather than
    /// re-checking at each spend site, so a count emptied by *any* write --
    /// a settle, another effect's removal -- leaves the field just the same.
    /// Mirrors [`Self::CrystalsChanged`], which is the same shape for [奇迹水晶].
    CpChanged = 78,

    // v40: the purchase surface (`docs/PURCHASE.md`).
    /// May `t.player_id` buy `t.tile` at all? A placed card
    /// refuses with `trigger::set_cancelled()` plus a reason. Runs for every
    /// [`BuyKind`], Force included (Poppin's hill lock).
    BuyGate = 79,
    /// The buy price's first modifier stage (fixed ±), before
    /// [`Self::BuyMul`] and [`Self::BuySet`]. `t.value` / `set_price`.
    BuyAdd = 80,
    /// Second modifier stage (×), after [`Self::BuyAdd`].
    BuyMul = 81,
    /// Third modifier stage (free / fixed price), after
    /// [`Self::BuyMul`]. Each stage floors the price at 0.
    BuySet = 82,
    /// The deal is committing, before the `bought` hook.
    /// Rewrites `deal_owner` / `deal_houses` / `deal_mortgaged`.
    BuyAssign = 83,

    // v42: the command-wide **pre-split** payment stage (`PIPELINE-AUDIT` Q2)
    // and the terminal `<thing>Resolved` hooks (Q6).
    /// The payment command's **pre-split** fixed ± stage
    /// (「分摊前资金减少/增加」, 规则书 支付阶段 2 applied to the command total).
    /// Runs once per payment command, on the figure **before** any 「[分摊]」
    /// divides it into shares; a single-pair payment's command total is its own
    /// amount. `t.value` / `set_pay_amount` rewrite the total. Composes with
    /// [`Self::PayTotalMul`] and [`Self::PayTotalCancel`] (add → mul → cancel).
    /// The per-share counterparts are [`Self::PayAdd`] / [`Self::PayMul`] /
    /// [`Self::PayChoose`] / [`Self::PayAt`], which run on each settled leg.
    PayTotalAdd = 84,
    /// The pre-split × stage (规则书 支付阶段 4), after
    /// [`Self::PayTotalAdd`]. `t.value` / `set_pay_amount`.
    PayTotalMul = 85,
    /// The pre-split cancel (规则书 支付阶段 5's
    /// 「取消支付」 on the command rather than on one pair), after
    /// [`Self::PayTotalMul`]. `trigger::set_cancelled()` drops the whole
    /// command: no leg runs and nothing moves.
    PayTotalCancel = 86,
    /// Terminal: the tile's settlement is fully resolved (after
    /// [`Self::SettleAfter`], including when the settle was cancelled -- the
    /// resolution is complete as nothing). `PIPELINE-AUDIT` Q6.
    TileResolved = 87,
    /// Terminal: the move (and any settlement it asked for) is fully resolved.
    /// Fires at the end of a walk / teleport, after the settle pipeline.
    MoveResolved = 88,
    /// Terminal: the bankruptcy is fully resolved -- cash-in done, the seat
    /// cleared, the leftover auctions finished. `PIPELINE-AUDIT` K11.
    BankruptResolved = 89,

    // v43: the move-head / move-tail pair (`SETTLE-STAGES.md` §7).
    /// **Before any move** -- walk or teleport, main or card-driven, settling or
    /// not -- once the move's plan is fixed and before the first step / the
    /// teleport. This is where a counteraction cancels or alters the move
    /// (`SETTLE-STAGES.md` §7 「移动前」). The roll-specific `rollPlan` /
    /// `moveRoll` and the teleport-specific `teleport` stay inside a main move's
    /// own head; this is the generic move point.
    /// For a walk, `value` is the resolved base distance and `move_total` /
    /// `move_remaining` include existing extra steps, before any are walked.
    MoveBefore = 90,
    /// **After a completed move** -- 「移动后」/「主要移动结束时」 and the
    /// 「[移动终点]」 condition anchor. Fires after `passPlayer`, before
    /// `settleBefore`, for every completed move -- including a 「不触发结算」
    /// one (其他规则注意事项 1.2: 「是否[结算]」 gates only the settle). The
    /// teleport-specific `teleported` and the final `moveResolved` stay where
    /// they are; this is the move-end point the tail effects belong on.
    MoveAfter = 91,

    // v44: marker spend / gain counteraction windows (user ruling 2026-10-07).
    /// **Before a marker spend** (火罐 / 奇迹水晶 / P✽P粉丝 / any token) moves.
    /// A counteraction here cancels the spend; nothing is spent if the link is
    /// negated. Marker *costs* keep today's timing otherwise: they are spent as
    /// the effect resolves and a whole-effect negation before the body already
    /// prevents them.
    MarkerSpend = 92,
    /// **Before a marker gain** moves. Same window shape as [`Self::MarkerSpend`].
    MarkerGain = 93,
}

impl TriggerKind {
    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => Self::Roll,
            2 => Self::MoveRoll,
            3 => Self::TurnStart,
            4 => Self::Pass,
            5 => Self::PassPlayer,
            6 => Self::SettleBefore,
            7 => Self::Settle,
            8 => Self::Mortgage,
            9 => Self::Pay,
            10 => Self::Paid,
            11 => Self::Bankrupt,
            12 => Self::Card,
            13 => Self::Event,
            14 => Self::Abnormal,
            15 => Self::Target,
            16 => Self::Stop,
            17 => Self::Teleport,
            18 => Self::SkillTeleport,
            19 => Self::Stun,
            20 => Self::Stay,
            21 => Self::Exile,
            22 => Self::Forced,
            23 => Self::State,
            24 => Self::Counteracted,
            25 => Self::DrawOut,
            26 => Self::CircleAffected,
            27 => Self::TwoCards,
            28 => Self::TurnStartBefore,
            29 => Self::PassBefore,
            30 => Self::MortgageBefore,
            31 => Self::BankruptBefore,
            32 => Self::CardAfter,
            33 => Self::EventAfter,
            34 => Self::SettleAfter,
            35 => Self::BuyBefore,
            36 => Self::BuyAfter,
            37 => Self::BuildBefore,
            38 => Self::BuildAfter,
            39 => Self::DiscardBefore,
            40 => Self::DiscardAfter,
            41 => Self::EndTurnBefore,
            42 => Self::EndTurnAfter,
            43 => Self::LeaveBefore,
            44 => Self::LeaveAfter,
            45 => Self::TurnEnd,
            46 => Self::Drawn,
            47 => Self::PassTile,
            48 => Self::PayAfter,
            49 => Self::RollAfter,
            50 => Self::CardPlayed,
            51 => Self::Targeted,
            52 => Self::PayChoose,
            53 => Self::TurnEndBefore,
            54 => Self::TurnEndAfter,
            55 => Self::PayAdd,
            56 => Self::PayMul,
            57 => Self::PayAt,
            58 => Self::Discarded,
            59 => Self::DeckBeforeGame,
            60 => Self::DeckAtGameStart,
            61 => Self::Drew,
            62 => Self::Reshuffled,
            63 => Self::Bought,
            64 => Self::SettleBody,
            65 => Self::BeforeOut,
            66 => Self::Teleported,
            67 => Self::RollPlan,
            68 => Self::AbnormalGuard,
            69 => Self::ImmuneAll,
            70 => Self::Untargetable,
            71 => Self::Redirect,
            72 => Self::Effect,
            75 => Self::HouseAdded,
            73 => Self::FireSpent,
            74 => Self::SkillUsed,
            76 => Self::CrystalsChanged,
            77 => Self::DrewBefore,
            78 => Self::CpChanged,
            79 => Self::BuyGate,
            80 => Self::BuyAdd,
            81 => Self::BuyMul,
            82 => Self::BuySet,
            83 => Self::BuyAssign,
            84 => Self::PayTotalAdd,
            85 => Self::PayTotalMul,
            86 => Self::PayTotalCancel,
            87 => Self::TileResolved,
            88 => Self::MoveResolved,
            89 => Self::BankruptResolved,
            90 => Self::MoveBefore,
            91 => Self::MoveAfter,
            92 => Self::MarkerSpend,
            93 => Self::MarkerGain,
            _ => Self::None,
        }
    }

    /// The C# `Trigger.Kind` string.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Roll => "roll",
            Self::MoveRoll => "moveRoll",
            Self::TurnStart => "turnStart",
            Self::Pass => "pass",
            Self::PassPlayer => "passPlayer",
            Self::SettleBefore => "settleBefore",
            Self::Settle => "settle",
            Self::Mortgage => "mortgage",
            Self::Pay => "pay",
            Self::Paid => "paid",
            Self::Bankrupt => "bankrupt",
            Self::Card => "card",
            Self::Event => "event",
            Self::Abnormal => "abnormal",
            Self::Target => "target",
            Self::Stop => "stop",
            Self::Teleport => "teleport",
            Self::SkillTeleport => "skillTeleport",
            Self::Stun => "stun",
            Self::Stay => "stay",
            Self::Exile => "exile",
            Self::Forced => "forced",
            Self::State => "state",
            Self::Counteracted => "counteracted",
            Self::DrawOut => "drawOut",
            Self::CircleAffected => "circleAffected",
            Self::TwoCards => "twoCards",
            Self::TurnStartBefore => "turnStartBefore",
            Self::PassBefore => "passBefore",
            Self::MortgageBefore => "mortgageBefore",
            Self::BankruptBefore => "bankruptBefore",
            Self::CardAfter => "cardAfter",
            Self::EventAfter => "eventAfter",
            Self::SettleAfter => "settleAfter",
            Self::BuyBefore => "buyBefore",
            Self::BuyAfter => "buyAfter",
            Self::BuildBefore => "buildBefore",
            Self::BuildAfter => "buildAfter",
            Self::DiscardBefore => "discardBefore",
            Self::DiscardAfter => "discardAfter",
            Self::EndTurnBefore => "endTurnBefore",
            Self::EndTurnAfter => "endTurnAfter",
            Self::LeaveBefore => "leaveBefore",
            Self::LeaveAfter => "leaveAfter",
            Self::TurnEnd => "turnEnd",
            Self::Drawn => "drawn",
            Self::PassTile => "passTile",
            Self::PayAfter => "payAfter",
            Self::RollAfter => "rollAfter",
            Self::CardPlayed => "cardPlayed",
            Self::Targeted => "targeted",
            Self::PayChoose => "payChoose",
            Self::TurnEndBefore => "turnEndBefore",
            Self::TurnEndAfter => "turnEndAfter",
            Self::PayAdd => "payAdd",
            Self::PayMul => "payMul",
            Self::PayAt => "payAt",
            Self::Discarded => "discarded",
            Self::DeckBeforeGame => "deckBeforeGame",
            Self::DeckAtGameStart => "deckAtGameStart",
            Self::Drew => "drew",
            Self::Reshuffled => "reshuffled",
            Self::Bought => "bought",
            Self::SettleBody => "settleBody",
            Self::BeforeOut => "beforeOut",
            Self::Teleported => "teleported",
            Self::RollPlan => "rollPlan",
            Self::AbnormalGuard => "abnormalGuard",
            Self::ImmuneAll => "immuneAll",
            Self::Untargetable => "untargetable",
            Self::Redirect => "redirect",
            Self::Effect => "effect",
            Self::HouseAdded => "houseAdded",
            Self::FireSpent => "fireSpent",
            Self::SkillUsed => "skillUsed",
            Self::CrystalsChanged => "crystalsChanged",
            Self::DrewBefore => "drewBefore",
            Self::CpChanged => "cpChanged",
            Self::BuyGate => "buyGate",
            Self::BuyAdd => "buyAdd",
            Self::BuyMul => "buyMul",
            Self::BuySet => "buySet",
            Self::BuyAssign => "buyAssign",
            Self::PayTotalAdd => "payTotalAdd",
            Self::PayTotalMul => "payTotalMul",
            Self::PayTotalCancel => "payTotalCancel",
            Self::TileResolved => "tileResolved",
            Self::MoveResolved => "moveResolved",
            Self::BankruptResolved => "bankruptResolved",
            Self::MoveBefore => "moveBefore",
            Self::MoveAfter => "moveAfter",
            Self::MarkerSpend => "markerSpend",
            Self::MarkerGain => "markerGain",
        }
    }

    /// Inverse of [`as_str`] (`game-rules` maps engine trigger kinds through here).
    pub fn from_str(s: &str) -> Self {
        match s {
            "roll" => Self::Roll,
            "moveRoll" => Self::MoveRoll,
            "turnStart" => Self::TurnStart,
            "pass" => Self::Pass,
            "passPlayer" => Self::PassPlayer,
            "settleBefore" => Self::SettleBefore,
            "settle" => Self::Settle,
            "mortgage" => Self::Mortgage,
            "pay" => Self::Pay,
            "paid" => Self::Paid,
            "bankrupt" => Self::Bankrupt,
            "card" => Self::Card,
            "event" => Self::Event,
            "abnormal" => Self::Abnormal,
            "target" => Self::Target,
            "stop" => Self::Stop,
            "teleport" => Self::Teleport,
            "skillTeleport" => Self::SkillTeleport,
            "stun" => Self::Stun,
            "stay" => Self::Stay,
            "exile" => Self::Exile,
            "forced" => Self::Forced,
            "state" => Self::State,
            "counteracted" => Self::Counteracted,
            "drawOut" => Self::DrawOut,
            "circleAffected" => Self::CircleAffected,
            "twoCards" => Self::TwoCards,
            "turnStartBefore" => Self::TurnStartBefore,
            "passBefore" => Self::PassBefore,
            "mortgageBefore" => Self::MortgageBefore,
            "bankruptBefore" => Self::BankruptBefore,
            "cardAfter" => Self::CardAfter,
            "eventAfter" => Self::EventAfter,
            "settleAfter" => Self::SettleAfter,
            "buyBefore" => Self::BuyBefore,
            "buyAfter" => Self::BuyAfter,
            "buildBefore" => Self::BuildBefore,
            "buildAfter" => Self::BuildAfter,
            "discardBefore" => Self::DiscardBefore,
            "discardAfter" => Self::DiscardAfter,
            "endTurnBefore" => Self::EndTurnBefore,
            "endTurnAfter" => Self::EndTurnAfter,
            "leaveBefore" => Self::LeaveBefore,
            "leaveAfter" => Self::LeaveAfter,
            "turnEnd" => Self::TurnEnd,
            "drawn" => Self::Drawn,
            "passTile" => Self::PassTile,
            "payAfter" => Self::PayAfter,
            "rollAfter" => Self::RollAfter,
            "cardPlayed" => Self::CardPlayed,
            "targeted" => Self::Targeted,
            "payChoose" => Self::PayChoose,
            "turnEndBefore" => Self::TurnEndBefore,
            "turnEndAfter" => Self::TurnEndAfter,
            "payAdd" => Self::PayAdd,
            "payMul" => Self::PayMul,
            "payAt" => Self::PayAt,
            "discarded" => Self::Discarded,
            "deckBeforeGame" => Self::DeckBeforeGame,
            "deckAtGameStart" => Self::DeckAtGameStart,
            "drew" => Self::Drew,
            "reshuffled" => Self::Reshuffled,
            "bought" => Self::Bought,
            "settleBody" => Self::SettleBody,
            "beforeOut" => Self::BeforeOut,
            "teleported" => Self::Teleported,
            "rollPlan" => Self::RollPlan,
            "abnormalGuard" => Self::AbnormalGuard,
            "immuneAll" => Self::ImmuneAll,
            "untargetable" => Self::Untargetable,
            "redirect" => Self::Redirect,
            "effect" => Self::Effect,
            "houseAdded" => Self::HouseAdded,
            "fireSpent" => Self::FireSpent,
            "skillUsed" => Self::SkillUsed,
            "crystalsChanged" => Self::CrystalsChanged,
            "drewBefore" => Self::DrewBefore,
            "cpChanged" => Self::CpChanged,
            "buyGate" => Self::BuyGate,
            "buyAdd" => Self::BuyAdd,
            "buyMul" => Self::BuyMul,
            "buySet" => Self::BuySet,
            "buyAssign" => Self::BuyAssign,
            "payTotalAdd" => Self::PayTotalAdd,
            "payTotalMul" => Self::PayTotalMul,
            "payTotalCancel" => Self::PayTotalCancel,
            "tileResolved" => Self::TileResolved,
            "moveResolved" => Self::MoveResolved,
            "bankruptResolved" => Self::BankruptResolved,
            "moveBefore" => Self::MoveBefore,
            "moveAfter" => Self::MoveAfter,
            "markerSpend" => Self::MarkerSpend,
            "markerGain" => Self::MarkerGain,
            _ => Self::None,
        }
    }

    /// Which of the three vocabularies this event belongs to. Derived from the
    /// typed views below -- there is no second list to keep in step.
    pub const fn role(self) -> EventRole {
        let v = self as i32;
        if GateKind::from_i32(v).is_some() {
            EventRole::Gate
        } else if ChainKind::from_i32(v).is_some() {
            EventRole::Link
        } else {
            EventRole::Hook
        }
    }
}

/// Which vocabulary an event kind belongs to.
///
/// [`TriggerKind`] is the *wire* enum: every event the engine raises, and what
/// crosses the guest boundary. Cards do not declare against it -- they declare
/// against one of the three typed views, and the type says what the event is
/// for:
///
/// * [`ChainKind`] -- a chain link answers it, so a [反击] window opens.
/// * [`HookKind`] -- a field-card (`Fx`) settlement point; placed cards run
///   automatically, no window.
/// * [`GateKind`] -- a question posed to placed cards at declaration or at
///   resolution (`may this effect name S?` / `does anything land on S?`).
///
/// The `matches!` list that used to be `is_hook_only` is now
/// [`TriggerKind::role`] reading these types.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventRole {
    /// Opens a [反击] window -- a chain link can answer it.
    Link = 0,
    /// Field-card hook only -- no window.
    Hook = 1,
    /// A query to placed cards -- no window.
    Gate = 2,
}

macro_rules! declare_kinds {
    ($(#[$doc:meta])* $name:ident { $($(#[$vdoc:meta])* $variant:ident = $val:expr,)* }) => {
        $(#[$doc])*
        #[repr(i32)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $name {
            $($(#[$vdoc])* $variant = $val,)*
        }

        impl $name {
            /// The wire value, shared with [`TriggerKind`].
            pub const fn as_i32(self) -> i32 {
                self as i32
            }

            pub const fn from_i32(v: i32) -> Option<Self> {
                Some(match v {
                    $($val => Self::$variant,)*
                    _ => return None,
                })
            }
        }

        impl PartialEq<TriggerKind> for $name {
            fn eq(&self, other: &TriggerKind) -> bool {
                *self as i32 == *other as i32
            }
        }

        impl PartialEq<$name> for TriggerKind {
            fn eq(&self, other: &$name) -> bool {
                *self as i32 == *other as i32
            }
        }
    };
}

declare_kinds! {
    /// What a chain link answers: the [反击] selectors.
    ///
    /// A card declares these on `On::Counteract`. These are the moments at which a
    /// counter may be played -- an effect being *declared*, not an outcome
    /// having settled. [`HookKind`] and [`GateKind`] values are deliberately
    /// absent: a counteraction cannot be offered at a settlement hook or at a gate.
    ChainKind {
        /// See [`TriggerKind::Roll`] -- as a chain link a counter may block the
        /// roll (the dice are not cast yet).
        Roll = 1,
        /// See [`TriggerKind::MoveRoll`] -- the [反击] that rerolls the dice
        /// (`set_move_roll`) answers here.
        MoveRoll = 2,
        /// See [`TriggerKind::TurnStart`] -- a counter at the turn's opening
        /// (after the status ticks).
        TurnStart = 3,
        /// See [`TriggerKind::Pass`] -- a counter to 「当你经过…时」 at each
        /// traversed tile (the mid-route counterpart of [`Self::PassTile`]).
        Pass = 4,
        /// See [`TriggerKind::PassPlayer`] -- the end-tile [重叠] [反击]
        /// (`target` = the player overlapped).
        PassPlayer = 5,
        /// See [`TriggerKind::SettleBefore`] -- the 「[触发结算]前」 counter
        /// (行动阶段 14).
        SettleBefore = 6,
        /// See [`TriggerKind::Settle`] -- 「[结算]时」 (行动阶段 15 opening).
        /// `set_cancelled()` = the settle never happened (no body, no
        /// `settleAfter`); `tileResolved` still fires.
        Settle = 7,
        /// v32: the settle **body** is its own chain link (`docs/TILES.md`), so
        /// 「replace the body」 (「将本次结算改为…」) and 「the settle never
        /// happened」 are two separate [反击] targets. Cancelling `Settle` skips
        /// the body *and* `settleAfter`; cancelling `SettleBody` skips only the
        /// body and `settleAfter` still runs.
        SettleBody = 64,
        /// See [`TriggerKind::Mortgage`] -- a counter after the mortgage applied.
        Mortgage = 8,
        /// See [`TriggerKind::Paid`] -- 「[消耗]或[支付]」/「被…收取资金」: the
        /// window opens on a payer-side loss, after the money moved.
        Paid = 10,
        /// See [`TriggerKind::Bankrupt`] -- a counter after the cash-in, before
        /// the seat is cleared.
        Bankrupt = 11,
        /// See [`TriggerKind::Card`] -- the play's [反击] window; also the kind of
        /// every declared counter link (a counter to a counter is answered here).
        Card = 12,
        /// See [`TriggerKind::Event`] -- a counter to the event draw, before it
        /// resolves.
        Event = 13,
        /// See [`TriggerKind::Stop`] -- wire name `stop`; not raised by the
        /// current engine, kept for wire compatibility.
        Stop = 16,
        /// See [`TriggerKind::Teleport`] -- not raised by the current engine;
        /// kept for wire compatibility.
        Teleport = 17,
        /// See [`TriggerKind::SkillTeleport`] -- 「当你使用技能进行传送后」;
        /// not raised by the current engine, kept for wire compatibility.
        SkillTeleport = 18,
        /// See [`TriggerKind::Stun`] -- not raised by the current engine; kept
        /// for wire compatibility.
        Stun = 19,
        /// See [`TriggerKind::Stay`] -- not raised by the current engine; kept
        /// for wire compatibility.
        Stay = 20,
        /// See [`TriggerKind::Exile`] -- 「任意玩家获得[除外]…时」, raised from
        /// the grant log (not the landing, which is [`TriggerKind::Abnormal`]).
        Exile = 21,
        /// See [`TriggerKind::Forced`] -- not raised by the current engine; kept
        /// for wire compatibility.
        Forced = 22,
        /// See [`TriggerKind::State`] -- 「当有其他玩家切换状态时」; not raised
        /// by the current engine, kept for wire compatibility.
        State = 23,
        /// See [`TriggerKind::Counteracted`] -- 「有玩家对你使用[反击]后」; not
        /// raised by the current engine, kept for wire compatibility.
        Counteracted = 24,
        /// See [`TriggerKind::DrawOut`] -- 「回合外受到抽卡效果时」; not raised
        /// by the current engine, kept for wire compatibility.
        DrawOut = 25,
        /// See [`TriggerKind::CircleAffected`] -- a counter to the CiRCLE reward
        /// pick, before the payout.
        CircleAffected = 26,
        /// See [`TriggerKind::TwoCards`] -- 「当有人同一回合内打出两张卡时」; not
        /// raised by the current engine, kept for wire compatibility.
        TwoCards = 27,
        /// See [`TriggerKind::TurnStartBefore`] -- the 「回合开始前」 half
        /// (行动阶段 1), before the status ticks.
        TurnStartBefore = 28,
        /// See [`TriggerKind::PassBefore`] -- the pre-half of each [经过] step
        /// (行动阶段 12), before the player arrives.
        PassBefore = 29,
        /// See [`TriggerKind::MortgageBefore`] -- a counter that blocks the
        /// mortgage, before any guard.
        MortgageBefore = 30,
        /// See [`TriggerKind::BankruptBefore`] -- a counter before the asset
        /// cash-in (the seat is already dead -- B3).
        BankruptBefore = 31,
        /// See [`TriggerKind::CardAfter`] -- a counter after the play's `Dest`
        /// handling. No [反击] key of its own for 「被…效果影响」: that is
        /// [`Self::Effect`].
        CardAfter = 32,
        /// See [`TriggerKind::EventAfter`] -- a counter after the event is filed
        /// away.
        EventAfter = 33,
        /// See [`TriggerKind::SettleAfter`] -- 「[结算]后」/「[触发结算]后」
        /// (行动阶段 16's 「结算后」 half).
        SettleAfter = 34,
        /// See [`TriggerKind::BuyBefore`] -- a counter before a buy resolves
        /// (`docs/PURCHASE.md`).
        BuyBefore = 35,
        /// See [`TriggerKind::BuyAfter`] -- a counter after the deed changes hands.
        BuyAfter = 36,
        /// See [`TriggerKind::BuildBefore`] -- a counter before a house build pays.
        BuildBefore = 37,
        /// See [`TriggerKind::BuildAfter`] -- a counter after the house commits.
        BuildAfter = 38,
        /// See [`TriggerKind::DiscardBefore`] -- a counter before a hand discard.
        DiscardBefore = 39,
        /// See [`TriggerKind::DiscardAfter`] -- a counter after the card is in the pile.
        DiscardAfter = 40,
        /// See [`TriggerKind::EndTurnBefore`] -- a counter at the player's
        /// end-turn command (auto-skips raise only [`Self::EndTurnAfter`]).
        EndTurnBefore = 41,
        /// See [`TriggerKind::EndTurnAfter`] -- a counter on any turn end.
        EndTurnAfter = 42,
        /// See [`TriggerKind::LeaveBefore`] -- a counter before a forfeit.
        LeaveBefore = 43,
        /// See [`TriggerKind::LeaveAfter`] -- a counter after the player is cleared.
        LeaveAfter = 44,
        /// The effect declaration itself -- the [反击] key for 「被…效果影响」.
        Effect = 72,
        /// See [`TriggerKind::HouseAdded`] -- 「盖房」, after the house count is written.
        HouseAdded = 75,
        /// See [`TriggerKind::FireSpent`] -- 「每当你消耗火罐时」, after the spend commits.
        FireSpent = 73,
        /// See [`TriggerKind::SkillUsed`] -- 「使用自己原有的技能（2）时」, after the press.
        SkillUsed = 74,
        /// v43: the move head (`SETTLE-STAGES.md` §7 「移动前」) -- a counteraction
        /// that cancels or alters the move answers here, once the plan is fixed
        /// and before the first step / the teleport.
        MoveBefore = 90,
        /// v43: the move tail (行动阶段 13 「移动后」/「主要移动结束时」) --
        /// after `passPlayer`, before `settleBefore`, for every completed move.
        MoveAfter = 91,
        /// v43: one [经过] step (`SETTLE-STAGES.md` §4 M4) -- the chain
        /// counterpart of [`TriggerKind::PassTile`], so a [反击] that answers
        /// 「当你经过…时」 can target a mid-route pass rather than the end-tile
        /// [重叠]. Same moment as [`Self::Pass`]; `PassBefore` is the pre-half.
        PassTile = 47,
        /// v44: a marker spend (user ruling 2026-10-07) -- its own [反击]
        /// window, opened **before** the markers move. No shipped card listens
        /// yet; a fixture in `rules/fixtures/test-cards` pins the shape.
        MarkerSpend = 92,
        /// v44: a marker gain, same window shape as [`Self::MarkerSpend`].
        MarkerGain = 93,
    }
}

declare_kinds! {
    /// What a placed card can hook: everything except the [`GateKind`] questions.
    ///
    /// A card declares these on `On::Hook`. The engine runs every *placed*
    /// card's entry against them automatically -- no declaration, no prompt.
    ///
    /// This **overlaps** [`ChainKind`] on purpose. A kind may be both a [反击]
    /// point and a hook point -- `SettleBefore` is a moment at which a hand card
    /// can be played *and* a field card can counteract -- and the two declarations
    /// are distinct entries (`On::Counteract` vs `On::Hook`). The engine's dispatch
    /// has always worked this way; the types now say so. Kinds that are *only*
    /// hooks (the settlement / field-card points) simply have no [`ChainKind`]
    /// counterpart.
    HookKind {
        /// See [`TriggerKind::Roll`] -- a field card runs here automatically.
        Roll = 1,
        /// See [`TriggerKind::MoveRoll`].
        MoveRoll = 2,
        /// See [`TriggerKind::TurnStart`].
        TurnStart = 3,
        /// See [`TriggerKind::Pass`] -- each [经过] step.
        Pass = 4,
        /// See [`TriggerKind::PassPlayer`] -- the end-tile [重叠].
        PassPlayer = 5,
        /// See [`TriggerKind::SettleBefore`] -- 「[触发结算]前」 (行动阶段 14).
        SettleBefore = 6,
        /// See [`TriggerKind::Settle`] -- 「[结算]时」 (行动阶段 15 opening).
        Settle = 7,
        /// See [`TriggerKind::Mortgage`].
        Mortgage = 8,
        /// See [`TriggerKind::Pay`] -- settlement hook only; a counteraction
        /// cannot be offered here (the payment's [反击] key is
        /// [`TriggerKind::Effect`]).
        Pay = 9,
        /// See [`TriggerKind::Paid`].
        Paid = 10,
        /// See [`TriggerKind::Bankrupt`].
        Bankrupt = 11,
        /// See [`TriggerKind::Card`].
        Card = 12,
        /// See [`TriggerKind::Event`].
        Event = 13,
        /// See [`TriggerKind::Abnormal`] -- settlement hook only (the [反击]
        /// key is [`TriggerKind::Effect`] with `kind: "abnormal"`).
        Abnormal = 14,
        /// See [`TriggerKind::Target`] -- settlement hook only (same: the [反击]
        /// key is [`TriggerKind::Effect`]).
        Target = 15,
        /// See [`TriggerKind::Stop`] -- not raised by the current engine; kept
        /// for wire compatibility.
        Stop = 16,
        /// See [`TriggerKind::Teleport`] -- not raised by the current engine;
        /// kept for wire compatibility.
        Teleport = 17,
        /// See [`TriggerKind::SkillTeleport`] -- not raised by the current
        /// engine; kept for wire compatibility.
        SkillTeleport = 18,
        /// See [`TriggerKind::Stun`] -- not raised by the current engine; kept
        /// for wire compatibility.
        Stun = 19,
        /// See [`TriggerKind::Stay`] -- not raised by the current engine; kept
        /// for wire compatibility.
        Stay = 20,
        /// See [`TriggerKind::Exile`] -- 「任意玩家获得[除外]…时」.
        Exile = 21,
        /// See [`TriggerKind::Forced`] -- not raised by the current engine; kept
        /// for wire compatibility.
        Forced = 22,
        /// See [`TriggerKind::State`] -- not raised by the current engine; kept
        /// for wire compatibility.
        State = 23,
        /// See [`TriggerKind::Counteracted`] -- not raised by the current
        /// engine; kept for wire compatibility.
        Counteracted = 24,
        /// See [`TriggerKind::DrawOut`] -- not raised by the current engine;
        /// kept for wire compatibility.
        DrawOut = 25,
        /// See [`TriggerKind::CircleAffected`].
        CircleAffected = 26,
        /// See [`TriggerKind::TwoCards`] -- not raised by the current engine;
        /// kept for wire compatibility.
        TwoCards = 27,
        /// See [`TriggerKind::TurnStartBefore`] -- 「回合开始前」 (行动阶段 1).
        TurnStartBefore = 28,
        /// See [`TriggerKind::PassBefore`].
        PassBefore = 29,
        /// See [`TriggerKind::MortgageBefore`].
        MortgageBefore = 30,
        /// See [`TriggerKind::BankruptBefore`].
        BankruptBefore = 31,
        /// See [`TriggerKind::CardAfter`].
        CardAfter = 32,
        /// See [`TriggerKind::EventAfter`].
        EventAfter = 33,
        /// See [`TriggerKind::SettleAfter`] -- 「[结算]后」/「[触发结算]后」.
        SettleAfter = 34,
        /// See [`TriggerKind::BuyBefore`].
        BuyBefore = 35,
        /// See [`TriggerKind::BuyAfter`].
        BuyAfter = 36,
        /// See [`TriggerKind::BuildBefore`].
        BuildBefore = 37,
        /// See [`TriggerKind::BuildAfter`].
        BuildAfter = 38,
        /// See [`TriggerKind::DiscardBefore`].
        DiscardBefore = 39,
        /// See [`TriggerKind::DiscardAfter`].
        DiscardAfter = 40,
        /// See [`TriggerKind::EndTurnBefore`].
        EndTurnBefore = 41,
        /// See [`TriggerKind::EndTurnAfter`].
        EndTurnAfter = 42,
        /// See [`TriggerKind::LeaveBefore`].
        LeaveBefore = 43,
        /// See [`TriggerKind::LeaveAfter`].
        LeaveAfter = 44,
        /// See [`TriggerKind::TurnEnd`] -- a turn just ended
        /// (any player's). Hook-only: no [`ChainKind`] counterpart.
        TurnEnd = 45,
        /// See [`TriggerKind::Drawn`] -- the drawn card's own hook (it is still
        /// in hand, named on `t.card`). Hook-only.
        Drawn = 46,
        /// See [`TriggerKind::PassTile`] -- the [经过] step (行动阶段 12).
        /// Also a [`ChainKind`] (v43).
        PassTile = 47,
        /// See [`TriggerKind::PayAfter`] -- the 「资金变动」 hook (规则书 支付阶段 7).
        /// Hook-only.
        PayAfter = 48,
        /// See [`TriggerKind::RollAfter`] -- a hook that may
        /// rewrite the face (`set_move_roll`). Hook-only.
        RollAfter = 49,
        /// See [`TriggerKind::CardPlayed`] -- a card's hand
        /// effect resolved. Hook-only.
        CardPlayed = 50,
        /// See [`TriggerKind::Targeted`] -- the player was named.
        /// Hook-only.
        Targeted = 51,
        /// See [`TriggerKind::PayChoose`] -- may modify/decline
        /// the payment. Hook-only.
        PayChoose = 52,
        /// See [`TriggerKind::TurnEndBefore`] -- first step of a turn end,
        /// before the `On::AtEnd` callbacks and the status wear-off. Hook-only.
        TurnEndBefore = 53,
        /// See [`TriggerKind::TurnEndAfter`] -- the scheduled turn-end callbacks
        /// run here. Hook-only.
        TurnEndAfter = 54,
        /// See [`TriggerKind::PayAdd`] -- 支付阶段 2, fixed ± on each share.
        /// Hook-only.
        PayAdd = 55,
        /// See [`TriggerKind::PayMul`] -- 支付阶段 4, × on each share. Hook-only.
        PayMul = 56,
        /// See [`TriggerKind::PayAt`] -- after `PayChoose`, before the `pay`
        /// [反击] window. Hook-only.
        PayAt = 57,
        /// See [`TriggerKind::Discarded`] -- this card (on
        /// `t.card`) just went to the discard pile. Hook-only.
        Discarded = 58,
        /// See [`TriggerKind::DeckBeforeGame`] -- 「游戏开始前」, dispatched to
        /// every effect source. Hook-only.
        DeckBeforeGame = 59,
        /// See [`TriggerKind::DeckAtGameStart`] -- 「游戏开始时」. Hook-only.
        DeckAtGameStart = 60,
        /// See [`TriggerKind::Drew`] -- the per-draw field-card point (one raise
        /// per single card). Hook-only.
        Drew = 61,
        /// See [`TriggerKind::Reshuffled`] -- a discard pile was shuffled back.
        /// Hook-only.
        Reshuffled = 62,
        /// See [`TriggerKind::Bought`] -- a player became the owner
        /// of `t.tile` (buy or auction). Hook-only.
        Bought = 63,
        /// The settle **body** (`docs/TILES.md`) -- a field card placed on the
        /// tile may `set_cancelled()` here to *replace* the body (Parking Space,
        /// 笑容大游行). The same kind is also a [`ChainKind`] (v32), so a hand
        /// card may [反击] the body as its own link.
        SettleBody = 64,
        /// See [`TriggerKind::BeforeOut`] -- a player is about to leave the game.
        /// Hook-only.
        BeforeOut = 65,
        /// See [`TriggerKind::Teleported`] -- a teleport finished. Hook-only
        /// (the teleport's [反击] key is [`TriggerKind::MoveBefore`]).
        Teleported = 66,
        /// See [`TriggerKind::RollPlan`] -- the main-move `RollPlan` pass, before
        /// the dice. Hook-only (`On::RollPlan` is its own entry).
        RollPlan = 67,
        /// See [`TriggerKind::FireSpent`] -- 「每当你消耗火罐时」.
        FireSpent = 73,
        /// See [`TriggerKind::SkillUsed`] -- 「使用自己原有的技能（2）时」.
        SkillUsed = 74,
        /// See [`TriggerKind::HouseAdded`] -- a house was just added to `t.tile`.
        HouseAdded = 75,
        /// See [`TriggerKind::CrystalsChanged`] -- a placed card's [奇迹水晶]
        /// count was just written. Hook-only.
        CrystalsChanged = 76,
        /// See [`TriggerKind::DrewBefore`] -- the per-draw *replacement* point
        /// (「此次加手视为抽卡动作」). Hook-only.
        DrewBefore = 77,
        /// See [`TriggerKind::CpChanged`] -- a card instance's on-card [CP点]
        /// count was just written. Hook-only.
        CpChanged = 78,
        /// v40: the buy price's first modifier stage (`docs/PURCHASE.md`) --
        /// fixed ±, before [`Self::BuyMul`] / [`Self::BuySet`]. `t.value` /
        /// `set_price` on the run; each stage floors the price at 0.
        BuyAdd = 80,
        /// v40: the buy price's × stage, after [`Self::BuyAdd`].
        BuyMul = 81,
        /// v40: the buy price's free / fixed stage, after [`Self::BuyMul`].
        BuySet = 82,
        /// v40: the deal is committing (before `bought`); rewrite
        /// `deal_owner` / `deal_houses` / `deal_mortgaged`.
        BuyAssign = 83,
        /// v42: the payment command's **pre-split** fixed ± stage (「分摊前」,
        /// 规则书 支付阶段 2 on the command total) -- see
        /// [`TriggerKind::PayTotalAdd`]. Command-wide, before any 「[分摊]」
        /// divides it; the per-share stages are `PayAdd` / `PayMul` /
        /// `PayChoose` / `PayAt`.
        PayTotalAdd = 84,
        /// v42: the pre-split × stage (规则书 支付阶段 4), after
        /// [`Self::PayTotalAdd`].
        PayTotalMul = 85,
        /// v42: the pre-split cancel (规则书 支付阶段 5's 「取消支付」 on the
        /// command), after [`Self::PayTotalMul`]; `set_cancelled()` drops the
        /// whole command.
        PayTotalCancel = 86,
        /// v42: terminal -- the tile's settlement is fully resolved (also when
        /// it was cancelled). See [`TriggerKind::TileResolved`].
        TileResolved = 87,
        /// v42: terminal -- the move (and any settle it asked for) is done.
        /// See [`TriggerKind::MoveResolved`].
        MoveResolved = 88,
        /// v42: terminal -- the bankruptcy is fully resolved. See
        /// [`TriggerKind::BankruptResolved`].
        BankruptResolved = 89,
        /// v43: the move head (`SETTLE-STAGES.md` §7 「移动前」) -- every move
        /// (walk or teleport, main or card-driven, settling or not) once its
        /// plan is fixed and before the first step / the teleport. Also a
        /// [`ChainKind`]: counteractions that cancel or alter the move go here.
        MoveBefore = 90,
        /// v43: the move tail (行动阶段 13 「移动后」/「主要移动结束时」) --
        /// after `passPlayer`, before `settleBefore`, for every completed move
        /// including a 「不触发结算」 one. Also a [`ChainKind`].
        MoveAfter = 91,
        // PassTile (47) was already a HookKind; v43 makes it a [`ChainKind`]
        // too (see ChainKind).
    }
}

declare_kinds! {
    /// Questions posed to placed cards, at declaration or at resolution.
    ///
    /// A card declares these on `On::Gate`. These are not occurrences -- nothing
    /// "happened" -- so they are not triggers and cannot be [反击]'d. They ask
    /// a placed card whether an effect may name someone, or whether anything
    /// lands on them.
    GateKind {
        /// An abnormal effect is about to hit `t.target`; block with
        /// `trigger::set_cancelled()`. Then the `effect` [反击] window opens on
        /// the declaration (if someone else caused it), and a landing is
        /// reported by the [`TriggerKind::Abnormal`] hook -- that hook is the
        /// outcome and cannot be [反击]'d.
        AbnormalGuard = 68,
        /// Is `t.player_id` untouchable by `t.by_card`'s effects? Asked at
        /// **resolution** -- the effect is named, the chain forms, and it lands
        /// as nothing. Claim with `trigger::set_cancelled()`.
        ImmuneAll = 69,
        /// `t.by_card`'s card is about to name `t.player_id`. Asked at
        /// **declaration** -- block and the effect cannot name them at all, so
        /// no chain forms against them. Claim with `trigger::set_cancelled()`.
        Untargetable = 70,
        /// A single-target effect is about to name `t.target`; a placed card
        /// takes the hit with `trigger::set_target(player_id)`. **Declaration
        /// re-naming**: the recipient set is settled before the chain opens, so
        /// whoever holds the name is the one who answers.
        Redirect = 71,
        /// v40: may `t.player_id` buy `t.tile` at all (`docs/PURCHASE.md`)?
        /// A placed card refuses with `trigger::set_cancelled()` plus a
        /// reason. Runs for every [`BuyKind`], Force included.
        BuyGate = 79,
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
    /// host passes back to `bandori_on`).
    pub on: Vec<ManifestOn>,
    /// The card's declared static **properties** (`CardDef::props`), as
    /// `(key, value)` pairs sorted by key for deterministic wire bytes. The
    /// keys the engine reads are named in [`prop`]; a key a card does not
    /// declare reads as its default (`0`). Host side becomes a `BTreeMap`.
    #[serde(default)]
    pub props: Vec<(String, i32)>,
}

/// One entry point in the manifest: what it is and which trigger kinds it
/// answers (empty for non-trigger entries).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ManifestOn {
    /// An [`OnKind`] as `i32`.
    pub kind: i32,
    /// `TriggerKind`s as `i32`.
    pub triggers: Vec<i32>,
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
    /// Does this entry keep a `legacy_*` audit guard (G3, docs/GUARDS.md §5.1)?
    /// The `guard-audit` host feature calls [`export::OP_LEGACY_GUARD`] and
    /// panics on any mismatch with `pre ∧ guard`. Deleted once the card's audit
    /// is clean.
    pub has_legacy: bool,
}

/// What a card entry point is (`card_sdk::On`'s variants).
///
/// The wire mirror of `card_sdk::On`: each variant is one manifest entry
/// (`ManifestOn::kind`), and the entry's other fields carry its arguments.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnKind {
    /// `On::Play(cond, gate, body)` -- play this card from hand (or press it as
    /// a skill via `invoke_skill`). `cond` is the gate's CEL condition, `gate`
    /// is the playability query (a pure `Option<Msg>`; `None` = playable,
    /// `Some(why)` = blocked with a reason), `body` is the effect. The host
    /// calls it between the [`TriggerKind::Card`] raise and `cardAfter`.
    Play = 0,
    // 1 was `CantPlay`, folded into `Play`'s gate.
    /// `On::Counteract(kinds, cond, guard, body)` -- a [反击] at those
    /// [`ChainKind`]s. `cond` (CEL) and `guard` (the residual pure `bool` query,
    /// `None` once G4 folded the whole body into `cond`) decide whether the card
    /// is offered in the hand window; `body` resolves against the answered link.
    /// Called at the counteraction offer, then again at LIFO resolution.
    Counteract = 2,
    /// `On::Hook(kinds, cond, guard, body)` -- a field-card (`Fx`) hook at those
    /// [`HookKind`]s. Same `cond`/`guard`/`body` shape as [`Self::Counteract`].
    /// The host calls it **automatically** for every placed card (placement
    /// order per player) whenever one of the kinds is raised -- no window.
    Hook = 3,
    /// `On::AtEnd(body)` -- what `ctx::at_turn_end` / `ctx::at_next_turn_end`
    /// schedules: `body` runs once at that turn end. `cond`/`guard` are carried
    /// by the scheduling call, not by the entry.
    AtEnd = 4,
    /// `On::RollPlan(body)` -- this card has a movement routine.
    /// The host calls it on the main-move [`TriggerKind::RollPlan`] pass, before
    /// the dice, so `body` can shape `turn.plan`.
    RollPlan = 5,
    /// `On::Gate(kinds, body)` -- a question at those [`GateKind`]s. Not guarded:
    /// no `cond`/`guard` fields. `body` answers with `set_cancelled` /
    /// `set_target` / `set_reason`; the host asks every placed card at
    /// declaration or at resolution, per the kind.
    Gate = 6,
    /// v31: a rule's **settle body** (`docs/TILES.md`) -- `On::Settle(body)`. Tile
    /// rules (`tile:*`) are one per board tile kind and this is what runs when
    /// that tile is [结算]d, inside the settle chain (a [`TriggerKind::SettleBody`]
    /// cancel replaces it).
    Settle = 7,
}

impl OnKind {
    pub fn from_i32(v: i32) -> Option<Self> {
        Some(match v {
            0 => Self::Play,
            2 => Self::Counteract,
            3 => Self::Hook,
            4 => Self::AtEnd,
            5 => Self::RollPlan,
            6 => Self::Gate,
            7 => Self::Settle,
            _ => return None,
        })
    }
}
