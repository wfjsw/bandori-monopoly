#![cfg_attr(target_arch = "wasm32", no_std)]

//! Test-only cards for the host's cross-module tests.

use card_sdk::abi::{ChainKind, GateKind, HookKind};
use card_sdk::ctx::{self, trigger, CardPile};
use card_sdk::{key, CardDef, Msg, On};

/// Plays another card (in another module) in the middle of its own effect.
const RELAY: CardDef = CardDef::new("TEST:relay", &[On::Play("", None, relay)]);
/// Plays itself forever; the host must stop it at the depth limit.
const RECURSE: CardDef = CardDef::new("TEST:recurse", &[On::Play("", None, recurse)]);

/// Logs from both `play` and `counteract`, with no kind guard: the host must run its
/// `counteract` exactly once per play (at its own `card` trigger), not again at
/// `cardAfter` / `cardPlayed`.
const ECHO: CardDef = CardDef::new(
    "TEST:echo",
    &[
        On::Play("", None, echo_play),
        On::Counteract(&[ChainKind::Card], "", Some(never), echo_counteract),
    ],
);
/// Lists its player's hand through `cards_in` (the host->guest list) and logs
/// the count next to `hand_size`, so a test can check the two agree.
const LISTER: CardDef = CardDef::new("TEST:lister", &[On::Play("", None, lister)]);
/// Stuns the first other player through the [abnormal] gate, then logs how many
/// abnormal effects reached that player this turn.
const STUNNER: CardDef = CardDef::new("TEST:stunner", &[On::Play("", None, stunner)]);
/// Placed on the first other player's field; guards that player against every
/// abnormal effect (C# `IAbnormalGuard`).
const GUARD: CardDef = CardDef::new(
    "TEST:guard",
    &[
        On::Play("", None, guard_play),
        On::Gate(&[GateKind::AbnormalGuard], "", None, guard),
    ],
);

/// Targets the first other player (C# `H.Target`) and logs what it got and that
/// player's `_targeted` counter.
const AIMER: CardDef = CardDef::new("TEST:aimer", &[On::Play("", None, aimer)]);
/// Placed on the first other player's field; makes that player immune to other
/// players' effects (C# `ImmuneAll`). A **resolution** gate: the effect names
/// the player and the chain forms, and only what lands is voided.
const SHIELD: CardDef = CardDef::new(
    "TEST:shield",
    &[
        On::Play("", None, shield_play),
        On::Gate(&[GateKind::ImmuneAll], "", None, shield),
    ],
);

/// A [反击] that answers an **effect declaration** and negates its activation --
/// the link never happened, so nothing settles. This is the Yu-Gi-Oh "negate
/// the activation" as against "negate the effect".
const COUNTER: CardDef = CardDef::new(
    "TEST:counter",
    &[On::Counteract(&[ChainKind::Effect], "", Some(counter_yes), counter)],
);

/// [反击] any effect declaration -- deliberately loose (any declarer, any
/// recipient) so a chain test can line several responders up on one timing,
/// the triggering player included (clause 89's ring ends with them).
///
/// The body marks the answered link's `value` with how many probes have already
/// settled against it and pays the probe's player `100 * (that + 1)` from the
/// answered link's player. LIFO resolution is then readable in the money (the
/// newest probe settles first and takes the smallest cut) and shared-link
/// settlement in the log (`n` keeps counting on one link, `seq` says which).
const PROBE: CardDef = CardDef::new(
    "TEST:probe",
    &[On::Counteract(&[ChainKind::Effect], "", Some(probe_yes), probe)],
);

/// [反击] a counter's own play -- the 「新的时点」 of clause 89. Answers a
/// `card` link that is itself a counter (`trigger::seq() >= 2`), never a root
/// play (`seq` 0), and negates that counter's activation so its body does not
/// run.
const DENY: CardDef = CardDef::new(
    "TEST:deny",
    &[On::Counteract(&[ChainKind::Card], "", Some(deny_yes), deny)],
);

fn aimer(player_id: i32) -> card_sdk::Asked {
    let Some(&target) = ctx::others(player_id).first() else {
        return Ok(());
    };
    let got = ctx::target(target).unwrap_or(-1);
    ctx::log(
        player_id,
        &Msg::new(key!("aimer_done"))
            .i("got", got as i64)
            .i("count", ctx::targeted_count(target) as i64),
    );
    Ok(())
}

fn shield_play(player_id: i32) -> card_sdk::Asked {
    let Some(&target) = ctx::others(player_id).first() else {
        return Ok(());
    };
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(target, "TEST:shield", &Msg::new(key!("shield_note")));
    Ok(())
}

fn shield(player_id: i32) -> card_sdk::Asked {
    if trigger::player_id() == player_id && !trigger::cancelled() {
        trigger::set_cancelled();
        ctx::log(player_id, &Msg::new(key!("shield_held")));
    }
    Ok(())
}

/// Shapes a 3-step walk via the plan ops and runs it immediately with
/// `card_move` (C# `H.CardMove`): the run pauses, the engine walks, the effect
/// resumes. Logs where the player ended up.
const MOVER: CardDef = CardDef::new("TEST:mover", &[On::Play("", None, mover)]);

/// Places itself with one [奇迹水晶] and spends it inside the same effect -- the
/// AG:绯红之魂 (3) shape: 「不再拥有[奇迹水晶]时」 is a `crystalsChanged` handler,
/// so the write that empties the card is what leaves the field. The spend site
/// does not re-check the count.
const CRYSTAL: CardDef = CardDef::new(
    "TEST:crystal",
    &[
        On::Play("", None, crystal_play),
        On::Hook(&[HookKind::CrystalsChanged], "", Some(crystal_changed_guard), crystal_changed),
    ],
);

fn mover(player_id: i32) -> card_sdk::Asked {
    ctx::plan::set_steps(3);
    ctx::log(player_id, &Msg::new(key!("mover_planned")));
    ctx::card_move(player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("mover_done")).i("pos", ctx::player_pos(player_id) as i64),
    );
    Ok(())
}

fn crystal_play(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "TEST:crystal", &Msg::new(key!("crystal_note")));
    ctx::set_crystals(1);
    // The decay tick: a write that takes the count to zero. `decay` no longer
    // discards at 0 -- only the handler below is the 「no crystals -> discard」
    // check.
    ctx::decay()?;
    Ok(())
}

fn crystal_changed_guard(player_id: i32) -> bool {
    ctx::is_placed()
        && trigger::player_id() == player_id
        && trigger::card_is("TEST:crystal")
        && ctx::crystals() == 0
        // Only the write that did not raise the count speaks for the empty
        // state -- an earlier write in the same run must not speak for the
        // count the run ended on.
        && trigger::value() <= 0
}

fn crystal_changed(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Graveyard);
    // Which write emptied it: a removal, or a write that landed on none.
    let why = if trigger::value() < 0 {
        key!("crystal_empty")
    } else {
        key!("crystal_none")
    };
    ctx::log(player_id, &Msg::new(why));
    Ok(())
}

/// Places itself and leaves **immediately** -- `send_to_dest` rather than
/// `set_dest` -- so the rest of the effect runs against a card that is already
/// gone. Logs `is_placed` right after, which is the whole distinction.
const DEST_NOW: CardDef = CardDef::new("TEST:dest_now", &[On::Play("", None, dest_now)]);

fn dest_now(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "TEST:dest_now", &Msg::new(key!("dest_note")));
    ctx::send_to_dest(ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("dest_now_left")).i("placed", ctx::is_placed() as i64),
    );
    Ok(())
}

/// Places itself and names **another** player's discard -- 「将此卡放入[使用者]
/// 弃卡区」 when [使用者] is not the one holding it.
const DEST_TO: CardDef = CardDef::new("TEST:dest_to", &[On::Play("", None, dest_to)]);

fn dest_to(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "TEST:dest_to", &Msg::new(key!("dest_note")));
    if let Some(&user) = ctx::others(player_id).first() {
        ctx::set_transfer_to_dest(user, ctx::Dest::Graveyard);
    }
    Ok(())
}

fn never(_player: i32) -> bool {
    false
}

fn echo_play(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("echo_play")));
    Ok(())
}

fn echo_counteract(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("echo_counteract")));
    Ok(())
}

fn lister(player_id: i32) -> card_sdk::Asked {
    let hand = ctx::cards_in(player_id, CardPile::Hand);
    ctx::log(
        player_id,
        &Msg::new(key!("lister_count"))
            .i("n", hand.len() as i64)
            .i("size", ctx::hand_size(player_id) as i64),
    );
    // Round-trip a real id: take the first card out of the hand and put it on
    // the discard pile, then confirm the discard listing shows it.
    if let Some(first) = hand.first() {
        if ctx::take_card(player_id, CardPile::Hand, first) {
            ctx::to_discard(player_id, first);
            let found = ctx::cards_in(player_id, CardPile::Discard)
                .iter()
                .any(|c| c == first);
            ctx::log(
                player_id,
                &Msg::new(key!("lister_moved")).i("found", found as i64),
            );
        }
    }
    Ok(())
}

fn stunner(player_id: i32) -> card_sdk::Asked {
    let Some(&target) = ctx::others(player_id).first() else {
        return Ok(());
    };
    ctx::give_stun(target, 1);
    ctx::log(
        player_id,
        &Msg::new(key!("stunner_done")).i("count", ctx::abnormal_count(target) as i64),
    );
    Ok(())
}

fn guard_play(player_id: i32) -> card_sdk::Asked {
    let Some(&target) = ctx::others(player_id).first() else {
        return Ok(());
    };
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(target, "TEST:guard", &Msg::new(key!("guard_note")));
    Ok(())
}

fn guard(player_id: i32) -> card_sdk::Asked {
    if trigger::target() == player_id && !trigger::cancelled() {
        trigger::set_cancelled();
        ctx::log(player_id, &Msg::new(key!("guard_blocked")));
    }
    Ok(())
}

fn relay(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("relay_start")));
    ctx::play_card("HHW:（育美）", player_id)?;
    ctx::log(player_id, &Msg::new(key!("relay_end")));
    Ok(())
}

fn recurse(player_id: i32) -> card_sdk::Asked {
    ctx::play_card("TEST:recurse", player_id)?;
    Ok(())
}

fn counter_yes(player_id: i32) -> bool {
    // Whole-list view: any effect another player's card declared at me.
    trigger::by_card().is_some_and(|by| by != player_id) && ctx::effect::hits(player_id)
}

fn counter(player_id: i32) -> card_sdk::Asked {
    // Negate the **activation**: the declaration never happened, so the
    // targeting never lands. (`negate_effect` would let it be seen as an effect
    // and settle to nothing -- the two are deliberately distinct.)
    trigger::set_cancelled();
    ctx::log(player_id, &Msg::new(key!("counter_fired")));
    Ok(())
}

fn probe_yes(_player_id: i32) -> bool {
    // Any effect link at all: the entry's `ChainKind::Effect` already says
    // which kind of timing this answers.
    true
}

fn probe(player_id: i32) -> card_sdk::Asked {
    // `trigger::` reads the **answered** link (the one this counter's body runs
    // against). `n` counts the probes that have already settled against that
    // same link; `seq` is 0 on an effect declaration and >= 2 on a counter, so
    // it says whether this answered X or another counter.
    //
    // The one mutable field a counter body shares with its siblings is the
    // answered link's `value` -- `trigger::set_pay_amount` is the host's
    // `Trigger.value` setter. The probe uses it as a settlement counter, so
    // LIFO order is readable in the money as well as the log: the newest probe
    // settles first and takes the smallest cut.
    let seen = trigger::value();
    trigger::set_pay_amount(seen + 1);
    let from = trigger::player_id();
    if from >= 0 && from != player_id {
        let why = Msg::new(key!("probe_why"));
        ctx::transfer(from, player_id, 100 * (seen + 1), &why)?;
    }
    ctx::log(
        player_id,
        &Msg::new(key!("probe_fired"))
            .player_id("who", player_id)
            .i("n", seen as i64)
            .i("seq", trigger::seq() as i64),
    );
    Ok(())
}

fn deny_yes(player_id: i32) -> bool {
    // `trigger::` reads the **answered** link: a counter link (seq >= 2), not a
    // root play (seq 0).
    trigger::player_id() != player_id && trigger::seq() >= 2
}

fn deny(player_id: i32) -> card_sdk::Asked {
    // Negate the countered counter's **activation**: its body will not run.
    trigger::set_cancelled();
    ctx::log(
        player_id,
        &Msg::new(key!("deny_fired")).player_id("who", player_id),
    );
    Ok(())
}

/// `PIPELINE-AUDIT` Q2 -- a 「分摊前」 probe: a placed card whose `payTotalAdd`
/// hook cuts the **command total** by 500 (floor 0). With the pre-split stage
/// wired right, a 「[分摊][支付]2000」 across two payers charges 750 each
/// (ceil10(1500/2)); wired as a per-share stage it would charge 500 each.
const TOTAL_CUT: CardDef = CardDef::new(
    "TEST:totalCut",
    &[On::Hook(&[HookKind::PayTotalAdd], "", Some(total_cut_yes), total_cut)],
);

fn total_cut_yes(_player_id: i32) -> bool {
    ctx::is_placed()
}

fn total_cut(player_id: i32) -> card_sdk::Asked {
    let amount = trigger::value();
    trigger::set_pay_amount((amount - 500).max(0));
    ctx::log(
        player_id,
        &Msg::new(key!("total_cut_done")).i("from", amount as i64).i(
            "to",
            trigger::value() as i64,
        ),
    );
    Ok(())
}

/// `PIPELINE-AUDIT` B3 -- a `bankruptBefore` probe that tries to move money
/// **for the dying player**. Once the seat is marked dead before the window
/// (K2 / 规则书 L16+L81) the payment is refused and the probe's owner gains
/// nothing.
const DEAD_PAY: CardDef = CardDef::new(
    "TEST:deadPay",
    &[On::Hook(&[HookKind::BankruptBefore], "", Some(dead_pay_yes), dead_pay)],
);

fn dead_pay_yes(player_id: i32) -> bool {
    // The instance's owner is a bystander; the trigger's `player_id` is the
    // seat going bankrupt.
    ctx::is_placed() && trigger::player_id() != player_id
}

fn dead_pay(player_id: i32) -> card_sdk::Asked {
    let dying = trigger::player_id();
    let got = ctx::transfer(dying, player_id, 1, &Msg::new(key!("dead_pay_why")))?;
    ctx::log(
        player_id,
        &Msg::new(key!("dead_pay_done")).i("dying", dying as i64).i("got", got as i64),
    );
    Ok(())
}

/// `PIPELINE-AUDIT` Q4 -- 「A[支付]A」: the user pays themselves 5000. The
/// counteraction windows run; the settlement follows the affordability path.
const SELF_CHARGE: CardDef = CardDef::new("TEST:selfCharge", &[On::Play("", None, self_charge)]);

fn self_charge(player_id: i32) -> card_sdk::Asked {
    let got = ctx::transfer(player_id, player_id, 5000, &Msg::new(key!("self_charge_why")))?;
    ctx::log(
        player_id,
        &Msg::new(key!("self_charge_done")).i("got", got as i64),
    );
    Ok(())
}

/// `PIPELINE-AUDIT` B2 -- a `payAdd` probe that boosts **every** payment
/// through the pipeline by 100, with no target filter. Placed on a seat that
/// then goes bankrupt: after 规则书 L81 「所有其正在生效的卡，技能效果停止生效」
/// the boost must not reach anyone's money.
const PAY_ADD_ANY: CardDef = CardDef::new(
    "TEST:payAddAny",
    &[
        On::Play("", None, pay_add_any_play),
        On::Hook(&[HookKind::PayAdd], "", Some(total_cut_yes), pay_add_any)],
);

fn pay_add_any_play(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "TEST:payAddAny", &Msg::new(key!("pay_add_any_note")));
    Ok(())
}

fn pay_add_any(player_id: i32) -> card_sdk::Asked {
    let amount = trigger::value();
    trigger::set_pay_amount(amount + 100);
    ctx::log(
        player_id,
        &Msg::new(key!("pay_add_any_done")).i("n", amount as i64),
    );
    Ok(())
}

/// A 「不[触发结算]」 **teleport move** (`SETTLE-STAGES.md` §6 M6a / ruling R2):
/// `plan::set_kind(Teleport)` + `set_resolve(false)` + `card_move`, so the move
/// goes through `teleport_as` and must still raise `passTile` + `passPlayer` at
/// its destination. `to` is tiles ahead of the mover (default 5).
const TELE_NOSOLVE: CardDef = CardDef::new("TEST:tele_nosettle", &[On::Play("", None, tele_nosettle)]);

fn tele_nosettle(player_id: i32) -> card_sdk::Asked {
    let n = ctx::tile_count();
    if n <= 0 {
        return Ok(());
    }
    let to = ctx::tile_steps_ahead(player_id, 5);
    if to < 0 {
        return Ok(());
    }
    ctx::plan::set_kind(card_sdk::abi::MoveKind::Teleport);
    ctx::plan::set_teleport_to(to);
    ctx::plan::set_resolve(false);
    ctx::card_move(player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("mover_done")).i("pos", ctx::player_pos(player_id) as i64),
    );
    Ok(())
}

/// Q3 (`PIPELINE-AUDIT`, user ruling 2026-10-07): a `payAdd` hook that drives
/// a **1000**-shaped payment to **-500**, with no floor. The reverse child
/// (amount 500) does not match the guard and settles normally -- so one
/// reversal, not a flip-flop loop. Placed like `TEST:totalCut`.
const PAY_OVERCUT: CardDef = CardDef::new(
    "TEST:payOvercut",
    &[On::Hook(&[HookKind::PayAdd], "", Some(total_cut_yes), pay_overcut)],
);

fn pay_overcut(player_id: i32) -> card_sdk::Asked {
    let amount = trigger::value();
    if amount == 1000 {
        trigger::set_pay_amount(-500);
        ctx::log(
            player_id,
            &Msg::new(key!("pay_overcut_done"))
                .i("from", amount as i64)
                .i("to", -500),
        );
    }
    Ok(())
}

/// Q3 two-sided: the user pays the next player 1000.
const XFER_1000: CardDef = CardDef::new("TEST:xfer1000", &[On::Play("", None, xfer_1000)]);

fn xfer_1000(player_id: i32) -> card_sdk::Asked {
    let n = ctx::player_count();
    let to = (player_id + 1) % n;
    ctx::transfer(player_id, to, 1000, &Msg::new(key!("xfer_1000_why")))?;
    Ok(())
}

/// Q3 one-sided 「[获得]」: the user gains 1000 from the bank.
const GAIN_1000: CardDef = CardDef::new("TEST:gain1000", &[On::Play("", None, gain_1000)]);

fn gain_1000(player_id: i32) -> card_sdk::Asked {
    ctx::gain(player_id, 1000, &Msg::new(key!("gain_1000_why")))?;
    Ok(())
}

/// Q3 one-sided 「[消耗]」: the user loses 1000 to the bank.
const LOSE_1000: CardDef = CardDef::new("TEST:lose1000", &[On::Play("", None, lose_1000)]);

fn lose_1000(player_id: i32) -> card_sdk::Asked {
    ctx::pay(player_id, 1000, &Msg::new(key!("lose_1000_why")))?;
    Ok(())
}

/// Marker window (user ruling 2026-10-07): a [反击] that answers `markerSpend`
/// and cancels the spend -- nothing moves. No shipped card listens to the
/// window yet; this fixture pins the shape.
const MARKER_DENY: CardDef = CardDef::new(
    "TEST:markerDeny",
    &[On::Counteract(&[ChainKind::MarkerSpend], "", Some(marker_deny_yes), marker_deny)],
);

fn marker_deny_yes(player_id: i32) -> bool {
    // Answer **another** seat's marker spend (the usual counteraction shape).
    trigger::player_id() != player_id
}

fn marker_deny(player_id: i32) -> card_sdk::Asked {
    trigger::set_cancelled();
    ctx::log(
        player_id,
        &Msg::new(key!("marker_deny_done")).n("n", trigger::value() as i64),
    );
    Ok(())
}

/// Spends 3 of the user's own counter named 「TEST:tok」 (created first).
const MARKER_SPEND: CardDef = CardDef::new("TEST:markerSpend", &[On::Play("", None, marker_spend)]);

fn marker_spend(player_id: i32) -> card_sdk::Asked {
    ctx::add_tok(player_id, "TEST:tok", 5, 99)?;
    let moved = ctx::add_tok(player_id, "TEST:tok", -3, 99)?;
    ctx::log(
        player_id,
        &Msg::new(key!("marker_spend_done")).n("moved", moved as i64),
    );
    Ok(())
}

/// Tags every move plan `fireRoll` at `rollPlan`, so a plain `roll` looks like
/// a 「使用火罐进行移动掷骰」. HHW:（美咲）'s `can_counteract` keys on
/// `move_tag("fireRoll")` and never reads the pot -- the 0-fire regression.
const FIRE_ROLL: CardDef = CardDef::new(
    "TEST:fireRoll",
    &[
        On::Play("", None, fire_roll_play),
        On::Hook(&[HookKind::RollPlan], "", None, fire_roll_plan),
    ],
);

fn fire_roll_play(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "TEST:fireRoll", &Msg::new(key!("fire_roll_note")));
    Ok(())
}

fn fire_roll_plan(_player_id: i32) -> card_sdk::Asked {
    ctx::plan::set_tag("fireRoll", 1);
    Ok(())
}

// --------------------------------------------------------------- conditions
// docs/GUARDS.md G0/G2 fixture cards: a condition is the layer-1 prefilter
// evaluated natively before the wasm guard is instantiated. Nothing rejected
// twice -- these guards do not restate their condition.

/// Guard **condition** rejects (`actor == owner && value >= 999999` is never
/// true in the tests): the guard must be skipped entirely, so a counteraction
/// is never offered even though the guard would say yes. See
/// `crates/game-rules/tests/guard_pre.rs`.
const PRE_REJECT: CardDef = CardDef::new(
    "TEST:preReject",
    &[On::Counteract(&[ChainKind::Effect], "actor == owner && value >= 999999", Some(pre_reject_guard), pre_reject_body,)],
);

fn pre_reject_guard(_player_id: i32) -> bool {
    // Residual guard: would accept. The condition rejects first.
    true
}

fn pre_reject_body(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("pre_reject_ran")));
    Ok(())
}

/// Guard **condition** accepts (`actor == owner` -- the `pre::MINE` sugar):
/// the guard runs. Nothing rejected twice -- the guard does not re-check
/// `actor == owner`.
const PRE_ACCEPT: CardDef = CardDef::new(
    "TEST:preAccept",
    &[On::Counteract(&[ChainKind::Effect], card_sdk::pre::MINE, Some(pre_accept_guard), pre_accept_body,)],
);

fn pre_accept_guard(_player_id: i32) -> bool {
    true
}

fn pre_accept_body(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("pre_accept_ran")));
    Ok(())
}

/// Play **gate** with a rejecting condition: `cant_play` must answer
/// `err.play_pre` without instantiating the gate.
const PRE_PLAY: CardDef = CardDef::new(
    "TEST:prePlay",
    &[On::Play(
        "owner.money >= 999999",
        Some(pre_play_gate),
        pre_play_body,
    )],
);

fn pre_play_gate(_player_id: i32) -> Option<Msg> {
    // Residual gate: would allow. The condition rejects first.
    None
}

fn pre_play_body(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("pre_play_ran")));
    Ok(())
}

/// Field hook with a rejecting condition: `run_hook` must not fire the body.
const PRE_HOOK: CardDef = CardDef::new(
    "TEST:preHook",
    &[
        On::Play("", None, pre_hook_place),
        On::Hook(&[HookKind::PayAdd], "false", Some(pre_hook_guard), pre_hook_body,),
    ],
);

fn pre_hook_place(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "TEST:preHook", &Msg::new(key!("pre_hook_note")));
    Ok(())
}

fn pre_hook_guard(_player_id: i32) -> bool {
    true
}

fn pre_hook_body(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("pre_hook_ran")));
    Ok(())
}

/// A `passTile` hook that teleports the mover 3 tiles ahead with a bare
/// `ctx::teleport_to` (no settle, no `teleport` event), once per walk. Pins
/// the walk-announce / `passTile` order: the walk's `roll`/`move` event must be
/// published **before** the hook jumps the piece, or the client animates the
/// walk from the pre-jump tile -- the reported "move starts from the location
/// prior to the teleport" bug.
const PASS_TELE: CardDef = CardDef::new(
    "TEST:pass_tele",
    &[
        On::Play("", None, pass_tele_place),
        On::Hook(&[HookKind::PassTile], "", Some(pass_tele_guard), pass_tele_body),
    ],
);

fn pass_tele_place(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "TEST:pass_tele", &Msg::new(key!("pass_tele_note")));
    Ok(())
}

fn pass_tele_guard(player_id: i32) -> bool {
    ctx::is_placed()
        && trigger::player_id() == player_id
        // Once per walk: the jump itself re-raises `passTile` at the destination.
        && ctx::state::get(player_id, "TEST.pass_tele.done") == 0
}

fn pass_tele_body(player_id: i32) -> card_sdk::Asked {
    let n = ctx::tile_count();
    if n <= 0 {
        return Ok(());
    }
    let to = ctx::tile_steps_ahead(player_id, 3);
    if to < 0 {
        return Ok(());
    }
    ctx::state::set(player_id, "TEST.pass_tele.done", 1);
    // A bare `ctx::teleport_to` (no settle, no `teleport` event) -- the
    // walk-announce / `passTile` order shows up as the walk event's id vs
    // this log line's id: HEAD publishes the walk before `passTile`, 808f0b3
    // published it after.
    ctx::teleport_to(player_id, to);
    ctx::log(
        player_id,
        &Msg::new(key!("pass_tele_done")).i("pos", ctx::player_pos(player_id) as i64),
    );
    Ok(())
}

/// A field card with a hook on the tile it stands beside: its `settle` clause
/// fires for **its holder** landing anywhere. Another player's land is a guard
/// reject, so the body never runs and nothing about the card is announced --
/// the card-activation event must say the same.
const TILE_HOOK: CardDef = CardDef::new(
    "TEST:tileHook",
    &[
        On::Play("", None, tile_hook_place),
        On::Hook(&[HookKind::Settle], "", Some(tile_hook_yes), tile_hook),
    ],
);

fn tile_hook_place(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "TEST:tileHook", &Msg::new(key!("tile_hook_note")));
    Ok(())
}

fn tile_hook_yes(player_id: i32) -> bool {
    ctx::is_placed() && trigger::player_id() == player_id
}

fn tile_hook(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("tile_hook_ran")));
    Ok(())
}

/// [反击] a root play itself -- 「将其抵消」. Answers the `card` link at its own
/// declaration (`seq == 0`, the root; a counter's link is `seq >= 2`) and
/// negates the **activation**, so the played card's body never runs.
/// (`TEST:deny` is the same for a *counter's* play.)
const DENY_PLAY: CardDef = CardDef::new(
    "TEST:denyPlay",
    &[On::Counteract(&[ChainKind::Card], "", Some(deny_play_yes), deny_play)],
);

fn deny_play_yes(player_id: i32) -> bool {
    trigger::player_id() != player_id && trigger::seq() == 0
}

fn deny_play(player_id: i32) -> card_sdk::Asked {
    trigger::set_cancelled();
    ctx::log(
        player_id,
        &Msg::new(key!("deny_play_fired")).player_id("who", player_id),
    );
    Ok(())
}

card_sdk::bandori_ruleset!(&[
    RELAY, RECURSE, ECHO, LISTER, STUNNER, GUARD, AIMER, SHIELD, MOVER, COUNTER, PROBE, DENY,
    CRYSTAL, DEST_NOW, DEST_TO, TOTAL_CUT, DEAD_PAY, SELF_CHARGE, PAY_ADD_ANY, TELE_NOSOLVE,
    PAY_OVERCUT, XFER_1000, GAIN_1000, LOSE_1000, MARKER_DENY, MARKER_SPEND, FIRE_ROLL,
    PRE_REJECT, PRE_ACCEPT, PRE_PLAY, PRE_HOOK, PASS_TELE, TILE_HOOK, DENY_PLAY
]);
