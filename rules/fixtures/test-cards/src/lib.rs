#![cfg_attr(target_arch = "wasm32", no_std)]

//! Test-only cards for the host's cross-module tests.

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use bandori_std::ctx::{self, trigger, CardPile};
use card_sdk::abi::{
    prop, roll_source, ActionKind, BuildCharge, ChainKind, FilterKind, RollPurpose, TriggerKind,
};
use card_sdk::serde;
use card_sdk::{
    key, Action, CardDef, Filter, GateValue, Msg, On, PayAddValue, PlanValue, RollValue,
};

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
/// Standing field hook: stuns the owner at `moveBefore` (after the move is
/// planned), so the walk continues while [晕眩] is already up at payout.
/// Models 「a counter's stun landed mid-turn」 without opening a new main move.
const STUN_SELF_MOVER: CardDef = CardDef::new(
    "TEST:stun_self_mover",
    &[On::Action(Action::MoveBefore(
        "actor == owner && card.placed",
        None,
        stun_self,
    ))],
);
const GUARD: CardDef = CardDef::new(
    "TEST:guard",
    &[
        On::Play("", None, guard_play),
        On::Filter(Filter::AbnormalGuard("", None, guard)),
    ],
);

const AIMER: CardDef = CardDef::new("TEST:aimer", &[On::Play("", None, aimer)])
    // A [指定] play: the join tests read `designations` and net_error's
    // clause 1 needs a designation to cancel (G06 §2.12, §6.4).
    .props(&[(prop::DESIGNATES, 1)]);
/// Targets its own seat through the full pipeline and logs the answer.
const TARGET_SELF: CardDef = CardDef::new("TEST:target_self", &[On::Play("", None, target_self)]);
/// Targets a tile the player picks (through `ctx::target_tile`) and logs it.
const TARGET_TILE: CardDef = CardDef::new("TEST:target_tile", &[On::Play("", None, target_tile)]);
/// Targets the actor and every other seat through `ctx::target_all` and logs
/// the seats that landed.
const TARGET_ALL: CardDef = CardDef::new("TEST:target_all", &[On::Play("", None, target_all)]);
/// Logs both nearest queries on a fixed board, for the query-matrix test.
const NEAREST: CardDef = CardDef::new("TEST:nearest", &[On::Play("", None, nearest)]);
/// Reads the bond waiver from inside a card run (the host-backed helpers cannot
/// run hostless in the test process): logs the exempt mask for grantor seats
/// 0..3 and the user's forward candidate mask.
const EXEMPTION_PROBE: CardDef =
    CardDef::new("TEST:exemption_probe", &[On::Play("", None, exemption_probe)]);
/// An effect-window counter that admits a self-designation and cancels it.
const TARGET_COUNTER: CardDef = CardDef::new(
    "TEST:target_counter",
    &[On::Counteract(
        &[ChainKind::Effect],
        "",
        Some(target_counter_yes),
        target_counter,
    )],
)
.props(&[(prop::DESIGNATES, 1)]);
/// A `targeted` hook that logs the raise (`target` / `tile` / `by`).
const TARGETED_PROBE: CardDef = CardDef::new(
    "TEST:targeted_probe",
    &[On::Action(Action::Targeted("", None, targeted_probe))],
);
/// G10 settle-outcome probe: `settleAfter` and `tileResolved` both log the
/// three facts and the paid value, so a test reads what the pipeline stamped
/// (`c` cancelled / `b` body replaced / `p` paid / `v` paid value).
const SETTLE_PROBE: CardDef = CardDef::new(
    "TEST:settle_probe",
    &[
        On::Action(Action::SettleAfter("", None, settle_probe)),
        On::Action(Action::TileResolved("", None, settle_probe)),
    ],
);

/// G10 world checkpoint: snapshot, change money, restore twice (idempotent),
/// drop. Answers nothing.
const SNAP_TWICE: CardDef = CardDef::new("TEST:snapTwice", &[On::Play("", None, snap_twice)]);
fn snap_twice(player_id: i32) -> card_sdk::Asked {
    let k = ctx::world_snapshot()?;
    ctx::adjust_money(player_id, 500, &Msg::new(key!("snap_twice_change")));
    ctx::world_restore(k)?;
    ctx::world_restore(k)?;
    ctx::world_snapshot_drop(k)?;
    Ok(())
}

/// G10 world checkpoint across a prompt: snapshot, change money, ask, then
/// restore -- the answer pass must still land the pre-prompt state.
const SNAP_ASK: CardDef = CardDef::new("TEST:snapAsk", &[On::Play("", None, snap_ask)]);
fn snap_ask(player_id: i32) -> card_sdk::Asked {
    let k = ctx::world_snapshot()?;
    ctx::adjust_money(player_id, 500, &Msg::new(key!("snap_ask_change")));
    let _ = ctx::ask_yes(
        player_id,
        &Msg::new(key!("snap_ask_q")),
        &Msg::new(key!("snap_ask_t")),
    )?;
    ctx::world_restore(k)?;
    ctx::world_snapshot_drop(k)?;
    Ok(())
}

/// G10 deferral: draw the event deck twice for the actor, back to back. Two
/// draws of the *same* card have to stay two records (§6.2).
const DRAW_TWICE: CardDef = CardDef::new("TEST:drawTwice", &[On::Play("", None, draw_twice)]);
fn draw_twice(player_id: i32) -> card_sdk::Asked {
    ctx::draw_event(player_id)?;
    ctx::draw_event(player_id)?;
    Ok(())
}

/// A **resolution** gate: the effect names the player and the chain forms, and
/// only what lands is voided.
const SHIELD: CardDef = CardDef::new(
    "TEST:shield",
    &[
        On::Play("", None, shield_play),
        On::Filter(Filter::ImmuneAll("", None, shield)),
    ],
);

/// A [反击] that answers an **effect declaration** and negates its activation --
/// the link never happened, so nothing settles. This is the Yu-Gi-Oh "negate
/// the activation" as against "negate the effect".
const COUNTER: CardDef = CardDef::new(
    "TEST:counter",
    &[On::Counteract(
        &[ChainKind::Effect],
        "",
        Some(counter_yes),
        counter,
    )],
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
    &[On::Counteract(
        &[ChainKind::Effect],
        "",
        Some(probe_yes),
        probe,
    )],
);

/// [反击] a counter's own play -- the 「新的时点」 of clause 89. Answers a
/// `card` link that is itself a counter (`trigger::seq() >= 2`), never a root
/// play (`seq` 0), and negates that counter's activation so its body does not
/// run. The declarer may answer their own link (the rulebook does not forbid
/// it).
/// [反击] the play's own `card` link (any declarer) and negates it. Unlike
/// `TEST:counter` (an effect link) this is the *activation* window itself, so
/// the play's outcome reads `Cancelled`.
const NEGATOR: CardDef = CardDef::new(
    "TEST:negator",
    &[On::Counteract(&[ChainKind::Card], "", Some(negator_yes), counter)],
);

fn negator_yes(player_id: i32) -> bool {
    trigger::by_card().is_some_and(|by| by != player_id)
}

const DENY: CardDef = CardDef::new(
    "TEST:deny",
    &[On::Counteract(&[ChainKind::Card], "", Some(deny_yes), deny)],
);

/// G06 §9.1 join tests -- an opener with `SELF_REDIRECT` (the 真奈 shape). Its
/// body runs the round and reports it: for each declarer the seat, whether it
/// shielded, and the designation it named. No declarer -> the opener's own draw
/// (the fallback).
const JOIN_OPEN: CardDef = CardDef::new(
    "TEST:joinOpen",
    &[On::Counteract(
        &[ChainKind::Effect],
        "target == owner",
        Some(always),
        join_open,
    )],
)
.props(&[(prop::SELF_REDIRECT, 1)]);

fn join_open(player_id: i32) -> card_sdk::Asked {
    let _ = ctx::join::open()?;
    let n = ctx::join::joiner_count();
    for i in 0..n {
        ctx::log(
            player_id,
            &Msg::new(key!("join_report"))
                .player_id("who", ctx::join::joiner(i))
                .i("shielded", ctx::join::joiner_shielded(i) as i64)
                .i(
                    "designation",
                    ctx::join::designation(i).unwrap_or(-1) as i64,
                ),
        );
    }
    if n == 0 {
        ctx::draw(player_id, 1)?;
    }
    Ok(())
}

/// The joining half of [`JOIN_OPEN`]: cancels the opener's designation
/// (「改为你」) and marks the shield through the legacy `set_join_used` alias.
const JOIN_SHIELD: CardDef = CardDef::new(
    "TEST:joinShield",
    &[On::Counteract(
        &[ChainKind::Effect],
        "target == owner",
        None,
        join_shield,
    )],
);

fn join_shield(player_id: i32) -> card_sdk::Asked {
    let to = ctx::join::opener().unwrap_or(player_id);
    ctx::cancel_designation(to);
    ctx::set_join_used();
    ctx::log(
        player_id,
        &Msg::new(key!("join_shield_done")).player_id("who", to),
    );
    Ok(())
}

/// A bare `pay` [反击] whose body is identifiable -- the kind dispatch half of
/// the mortis copy test (§2.15).
const PAY_COUNTER: CardDef = CardDef::new(
    "TEST:payCounter",
    &[On::Counteract(&[ChainKind::Pay], "", None, pay_counter)],
);

fn pay_counter(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("pay_counter_done")));
    Ok(())
}

/// A [反击] on the `pay` effect link that **negates** the payment: the
/// declared negation a later offer on the same link must see through
/// `trigger::pending_negation()`.
const PAY_NEGATE: CardDef = CardDef::new(
    "TEST:payNegate",
    &[
        On::Counteract(&[ChainKind::Effect], "chain_has(Pay)", None, pay_negate)
            .declares_negation(),
    ],
);

fn pay_negate(player_id: i32) -> card_sdk::Asked {
    trigger::negate_effect();
    ctx::log(player_id, &Msg::new(key!("pay_negate_done")));
    Ok(())
}

/// A **playable** card whose `Card` [反击] entry is permissive (`always`), so it
/// is a valid copy source once played -- the mortis copy test (§9.1 row 6/15).
/// Its `copied` link is logged: the own follow-up has an empty one, the copied
/// body names the source.
const CARD_COPY: CardDef = CardDef::new(
    "TEST:cardCopy",
    &[
        On::Play("", None, card_copy_play),
        On::Counteract(&[ChainKind::Card], "", Some(always), card_copy_counter),
    ],
);

fn card_copy_play(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("card_copy_play")));
    Ok(())
}

fn card_copy_counter(player_id: i32) -> card_sdk::Asked {
    let copied = trigger::link_copied().unwrap_or_default();
    ctx::log(
        player_id,
        &Msg::new(key!("card_copy_body")).card("copied", copied.as_str()),
    );
    Ok(())
}

/// A copy source whose counter entry sits at index 2 -- past the copying
/// card's own two entries -- and carries a label. The copied body's header
/// must take the **source's** label (design G06 §2.15).
const LABELED_COPY: CardDef = CardDef::new(
    "TEST:labeledCopy",
    &[
        On::Play("", None, card_copy_play),
        On::Filter(Filter::ImmuneAll("", None, pass_gate)),
        On::Counteract(&[ChainKind::Card], "", Some(always), labeled_copy_body).label("（2）"),
    ],
);

fn pass_gate(_player_id: i32, g: GateValue) -> GateValue {
    g
}

fn labeled_copy_body(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("labeled_copy_body")));
    Ok(())
}

/// A `[反击]` whose body places a card: a counteraction's placement is not
/// the answered play's own effect, so it must not mark that play as 「效果带有
/// 放置于场上的卡」 (design G06 §2.15).
const PLACING_COUNTER: CardDef = CardDef::new(
    "TEST:placingCounter",
    &[On::Counteract(
        &[ChainKind::Card],
        "",
        None,
        placing_counter,
    )],
);

fn placing_counter(player_id: i32) -> card_sdk::Asked {
    ctx::place_card(
        player_id,
        "TEST:shield",
        &Msg::new(key!("placing_counter_note")),
    );
    Ok(())
}

/// The same copy-source pair for a `Settle` link.
const SETTLE_COPY: CardDef = CardDef::new(
    "TEST:settleCopy",
    &[
        On::Play("", None, settle_copy_play),
        On::Counteract(&[ChainKind::Settle], "", Some(always), settle_copy_counter),
    ],
);

fn settle_copy_play(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("settle_copy_play")));
    Ok(())
}

fn settle_copy_counter(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("settle_copy_body")));
    Ok(())
}

/// The same copy-source pair for a `Pay` link.
const PAY_COPY: CardDef = CardDef::new(
    "TEST:payCopy",
    &[
        On::Play("", None, pay_copy_play),
        On::Counteract(&[ChainKind::Pay], "", Some(always), pay_copy_counter),
    ],
);

fn pay_copy_play(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("pay_copy_play")));
    Ok(())
}

fn pay_copy_counter(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("pay_copy_body")));
    Ok(())
}

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

fn target_self(player_id: i32) -> card_sdk::Asked {
    let got = ctx::target_self(player_id).unwrap_or(-1);
    ctx::log(
        player_id,
        &Msg::new(key!("target_self_done")).i("got", got as i64),
    );
    Ok(())
}

fn target_tile(player_id: i32) -> card_sdk::Asked {
    let tiles: Vec<i32> = (0..ctx::tile_count()).collect();
    let t = ctx::ask_tile(
        player_id,
        &Msg::new(key!("target_tile_title")),
        &Msg::new(key!("target_tile_ask")),
        &tiles,
    )?;
    let got = ctx::target_tile(t);
    ctx::log(
        player_id,
        &Msg::new(key!("target_tile_done"))
            .tile("tile", t)
            .i("got", got as i64),
    );
    Ok(())
}

/// A seat list as a bit mask (seat `s` = bit `s`), for a compact exact log.
fn seat_mask(seats: &[i32]) -> i64 {
    seats.iter().fold(0i64, |m, &p| m | (1i64 << p))
}

fn target_all(player_id: i32) -> card_sdk::Asked {
    let mut list = Vec::new();
    list.push(player_id);
    list.extend(ctx::others(player_id));
    let got = ctx::target_all(&list);
    ctx::log(
        player_id,
        &Msg::new(key!("target_all_done")).i("got", seat_mask(&got)),
    );
    Ok(())
}

fn nearest(player_id: i32) -> card_sdk::Asked {
    let fwd = ctx::nearest(player_id, ctx::DistanceKind::Forward);
    let short = ctx::nearest(player_id, ctx::DistanceKind::Shortest);
    let exempt = ctx::nearest_with_prop(
        player_id,
        ctx::DistanceKind::Forward,
        bandori_std::nearest::EXEMPTION,
    );
    ctx::log(
        player_id,
        &Msg::new(key!("nearest_done"))
            .i("fwd", seat_mask(&fwd))
            .i("short", seat_mask(&short))
            .i("exempt", seat_mask(&exempt)),
    );
    Ok(())
}

fn exemption_probe(player_id: i32) -> card_sdk::Asked {
    let n = ctx::player_count().min(4);
    let mut g = [0i64; 4];
    for grantor in 0..n {
        for p in 0..n {
            if bandori_std::nearest::exemption(grantor, p) {
                g[grantor as usize] |= 1i64 << p;
            }
        }
    }
    let cand = ctx::nearest_with_prop(
        player_id,
        ctx::DistanceKind::Forward,
        bandori_std::nearest::EXEMPTION,
    );
    ctx::log(
        player_id,
        &Msg::new(key!("exemption_probe_done"))
            .i("g0", g[0])
            .i("g1", g[1])
            .i("g2", g[2])
            .i("g3", g[3])
            .i("candidates", seat_mask(&cand)),
    );
    Ok(())
}

fn target_counter_yes(_player_id: i32) -> bool {
    trigger::target() >= 0
}

fn target_counter(player_id: i32) -> card_sdk::Asked {
    let who = trigger::target();
    ctx::cancel_designation(who);
    ctx::log(
        player_id,
        &Msg::new(key!("target_counter_done")).player_id("who", who),
    );
    Ok(())
}

fn targeted_probe(player_id: i32) -> card_sdk::Asked {
    ctx::log(
        player_id,
        &Msg::new(key!("targeted_probe_done"))
            .player_id("who", trigger::target())
            .tile("tile", trigger::tile())
            .i("by", trigger::by_card().unwrap_or(-1) as i64),
    );
    Ok(())
}

/// G10 settle fact: pay a *different* tile's rent (tile 6) from inside the
/// `settleAfter` hook, so the settle's own rent never committed (§6.1).
const PAY_OTHER_TILE: CardDef = CardDef::new(
    "TEST:payOtherTile",
    &[On::Action(Action::SettleAfter("", None, pay_other_tile))],
);
fn pay_other_tile(_player_id: i32) -> card_sdk::Asked {
    // Seat 1 is the landing payer of these fixtures.
    card_sdk::ctx::raw::asked(unsafe { card_sdk::ctx::raw::sys::pay_rent(1, 6, 0) })?;
    Ok(())
}

fn settle_probe(player_id: i32) -> card_sdk::Asked {
    ctx::log(
        player_id,
        &Msg::new(key!("settle_probe"))
            .tile("tile", trigger::tile())
            .i("c", trigger::settle_cancelled() as i64)
            .i("b", trigger::settle_body_replaced() as i64)
            .i("p", trigger::settle_paid() as i64)
            .i("v", trigger::settle_paid_value() as i64),
    );
    Ok(())
}

fn shield_play(player_id: i32) -> card_sdk::Asked {
    let Some(&target) = ctx::others(player_id).first() else {
        return Ok(());
    };
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(target, "TEST:shield", &Msg::new(key!("shield_note")));
    Ok(())
}

fn shield(player_id: i32, mut g: GateValue) -> GateValue {
    if trigger::player_id() == player_id && !trigger::cancelled() {
        ctx::log(player_id, &Msg::new(key!("shield_held")));
        g.allowed = Some(false); // deny
        return g;
    }
    g
}

/// Logs where the player ended up.
const MOVER: CardDef = CardDef::new("TEST:mover", &[On::Play("", None, mover)]);

/// A **dice** walk: unlike [`MOVER`] it leaves `plan.steps` unset, so
/// `run_planned_move` rolls the plan's dice table (the default 1d20) and raises
/// `diceRollPlan` / `diceRollValue` / `diceRollAfter` / `diceRollResult`.
/// It runs as a *side* move only when the caller marks `turn.plan.forced`
/// through `world_mut()` -- a `TEST:` id is not `event:`.
const DICE_MOVER: CardDef = CardDef::new("TEST:diceMover", &[On::Play("", None, dice_mover)]);

/// Places itself with one [奇迹水晶] and spends it inside the same effect -- the
/// AG:绯红之魂 (3) shape: 「不再拥有[奇迹水晶]时」 is a `counterChanged` handler
/// name-filtered to `counter::CRYSTALS`, so the write that empties the card is
/// what leaves the field. The spend site does not re-check the count.
const CRYSTAL: CardDef = CardDef::new(
    "TEST:named-counter",
    &[
        On::Play("", None, crystal_play),
        On::Action(Action::CounterChanged(
            "",
            Some(crystal_changed_guard),
            crystal_changed,
        )),
    ],
);

fn mover(player_id: i32) -> card_sdk::Asked {
    ctx::plan::set_steps(3);
    ctx::log(player_id, &Msg::new(key!("mover_planned")));
    ctx::run_planned_move(player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("mover_done")).i("pos", ctx::player_pos(player_id) as i64),
    );
    Ok(())
}

fn dice_mover(player_id: i32) -> card_sdk::Asked {
    // No `set_steps`: `run_planned_move` must roll the plan's dice. The landing does
    // not settle (the caller owns `resolve` for the main move it keeps).
    ctx::plan::set_kind(card_sdk::abi::MoveKind::Walk);
    ctx::plan::set_teleport_to(-1);
    ctx::plan::set_settles(false);
    ctx::run_planned_move(player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("mover_done")).i("pos", ctx::player_pos(player_id) as i64),
    );
    Ok(())
}

fn crystal_play(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "TEST:named-counter",
        &Msg::new(key!("crystal_note")),
    );
    ctx::set_crystals(1);
    // The decay tick: a write that takes the count to zero. `remove_crystal`
    // (was `decay`) no longer discards at 0 -- only the handler below is the
    // 「no crystals -> discard」 check.
    ctx::remove_crystal()?;
    Ok(())
}

fn crystal_changed_guard(player_id: i32) -> bool {
    ctx::is_placed()
        && trigger::player_id() == player_id
        && trigger::card_is("TEST:named-counter")
        && trigger::name() == bandori_std::counter::CRYSTALS
        && ctx::crystals() == 0
        // Only the write that did not raise the count speaks for the empty
        // state -- an earlier write in the same run must not speak for the
        // count the run ended on.
        && trigger::value() <= 0
}

fn crystal_changed(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Graveyard, card_sdk::abi::ZoneChangeCause::Effect);
    // Which write emptied it: a removal, or a write that landed on none.
    let reason = if trigger::value() < 0 {
        key!("named-counter-empty")
    } else {
        key!("named-counter-none")
    };
    ctx::log(player_id, &Msg::new(reason));
    Ok(())
}

/// Places itself and leaves **immediately** -- `send_to_dest` rather than
/// `set_dest` -- so the rest of the effect runs against a card that is already
/// gone. Logs `is_placed` right after, which is the whole distinction.
const DEST_NOW: CardDef = CardDef::new("TEST:fate_now", &[On::Play("", None, fate_now)]);

fn fate_now(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(player_id, "TEST:fate_now", &Msg::new(key!("fate_note")));
    ctx::send_to_dest(ctx::Dest::Graveyard, card_sdk::abi::ZoneChangeCause::Effect);
    ctx::log(
        player_id,
        &Msg::new(key!("fate_now_left")).i("placed", ctx::is_placed() as i64),
    );
    Ok(())
}

/// Places itself and names **another** player's discard -- 「将此卡放入[使用者]
/// 弃卡区」 when [使用者] is not the one holding it.
const DEST_TO: CardDef = CardDef::new("TEST:dest_to", &[On::Play("", None, dest_to)]);

fn dest_to(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(player_id, "TEST:dest_to", &Msg::new(key!("fate_note")));
    if let Some(&user) = ctx::others(player_id).first() {
        ctx::set_transfer_to_dest(user, ctx::Dest::Graveyard, card_sdk::abi::ZoneChangeCause::Effect);
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
    if let Some(&first) = hand.first() {
        if ctx::take_card(player_id, first.as_str(), CardPile::Hand) {
            ctx::to_discard(player_id, first.as_str());
            let found = ctx::cards_in(player_id, CardPile::Graveyard)
                .iter()
                .any(|c| *c == first);
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
    rules_status::stun::add(target, 1);
    ctx::log(
        player_id,
        &Msg::new(key!("stunner_done")).i("count", ctx::abnormal_count(target) as i64),
    );
    Ok(())
}

/// Stun the owner once the move is already planned (`moveBefore`).
fn stun_self(player_id: i32) -> card_sdk::Asked {
    rules_status::stun::add(player_id, 1);
    ctx::log(player_id, &Msg::new(key!("stunner_done")).i("count", 0));
    Ok(())
}

// ------------------------------------------------- G12 abnormal delivery
// Every deliverer below aims its effect at the first other player, so the
// `by != actor` half of the [不可阻挡] gate is what the tests exercise.

/// The first other seat, or -1.
fn first_other(player_id: i32) -> i32 {
    ctx::others(player_id).first().copied().unwrap_or(-1)
}

/// Received [传送]: the bare engine teleport (kind 4).
const TELEPORTER: CardDef = CardDef::new("TEST:teleporter", &[On::Play("", None, teleporter)]);

fn teleporter(player_id: i32) -> card_sdk::Asked {
    let target = first_other(player_id);
    if target < 0 {
        return Ok(());
    }
    let to = ctx::tile_steps_ahead(target, 3);
    if to < 0 {
        return Ok(());
    }
    ctx::teleport_to(target, to);
    ctx::log(
        player_id,
        &Msg::new(key!("teleporter_done")).i("pos", ctx::player_pos(target) as i64),
    );
    Ok(())
}

/// Received [强制移动]: four steps through `run_planned_move` (kind 5).
const FORCER: CardDef = CardDef::new("TEST:forcer", &[On::Play("", None, forcer)]);

fn forcer(player_id: i32) -> card_sdk::Asked {
    let target = first_other(player_id);
    if target < 0 {
        return Ok(());
    }
    ctx::plan::set_steps(4);
    let ran = ctx::run_planned_move(target);
    ctx::log(
        player_id,
        &Msg::new(key!("forcer_done"))
            .i("ran", ran as i64)
            .i("pos", ctx::player_pos(target) as i64),
    );
    Ok(())
}

/// Received [强制停下] (kind 6): the caller-side gate, then a marker.
const STOPPER: CardDef = CardDef::new("TEST:stopper", &[On::Play("", None, stopper)]);

fn stopper(player_id: i32) -> card_sdk::Asked {
    let target = first_other(player_id);
    if target < 0 {
        return Ok(());
    }
    let allowed = ctx::gate(target, bandori_std::kind::abnormal::Stop);
    if allowed {
        ctx::state::add(target, "TEST:stopped", 1);
    }
    ctx::log(
        player_id,
        &Msg::new(key!("stopper_done")).i("allowed", allowed as i64),
    );
    Ok(())
}

/// Received reverse (kind 7): the caller-side gate, then a marker.
const REVERSER: CardDef = CardDef::new("TEST:reverser", &[On::Play("", None, reverser)]);

fn reverser(player_id: i32) -> card_sdk::Asked {
    let target = first_other(player_id);
    if target < 0 {
        return Ok(());
    }
    let allowed = ctx::gate(target, bandori_std::kind::abnormal::Reverse);
    if allowed {
        ctx::state::add(target, "TEST:reversed", 1);
    }
    ctx::log(
        player_id,
        &Msg::new(key!("reverser_done")).i("allowed", allowed as i64),
    );
    Ok(())
}

/// Received [停留] layer: the status verb, which gates kind 1 itself.
const STAYER: CardDef = CardDef::new("TEST:stayer", &[On::Play("", None, stayer)]);

fn stayer(player_id: i32) -> card_sdk::Asked {
    let target = first_other(player_id);
    if target < 0 {
        return Ok(());
    }
    rules_status::stay::add(target, 1);
    ctx::log(
        player_id,
        &Msg::new(key!("stayer_done")).i("count", rules_status::stay::get(target) as i64),
    );
    Ok(())
}

/// Lands a [停留] on the actor **before the roll** (the in-move span:
/// `diceRollPlan` runs inside the main move's frame).
const PLAN_STAY: CardDef = CardDef::new(
    "TEST:planStay",
    &[
        On::Play("", None, plan_stay_place),
        On::Action(Action::DiceRollPlan(
            "actor == owner && card.placed",
            None,
            plan_stay,
        )),
    ],
);

fn plan_stay_place(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "TEST:planStay",
        &Msg::new(key!("plan_stay_note")),
    );
    Ok(())
}

fn plan_stay(player_id: i32) -> card_sdk::Asked {
    rules_status::stay::add(player_id, 1);
    ctx::log(player_id, &Msg::new(key!("plan_stay_done")));
    Ok(())
}

/// Lands a [停留] on the actor **after the walk**: `endTurnBefore` runs once
/// the move's frame has closed, before the `turnEnd` raises.
const END_STAY: CardDef = CardDef::new(
    "TEST:endStay",
    &[
        On::Play("", None, end_stay_place),
        On::Action(Action::EndTurnBefore(
            "actor == owner && card.placed",
            None,
            end_stay,
        )),
    ],
);

fn end_stay_place(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(player_id, "TEST:endStay", &Msg::new(key!("end_stay_note")));
    Ok(())
}

fn end_stay(player_id: i32) -> card_sdk::Asked {
    rules_status::stay::add(player_id, 1);
    ctx::log(player_id, &Msg::new(key!("end_stay_done")));
    Ok(())
}

/// Touches the first `PP:再次闪耀` on the board through its causer-aware
/// immunity read: one [奇迹水晶] when `is_immune_from_at(uid, by)` is false.
const IMMUNE_TOUCH: CardDef =
    CardDef::new("TEST:immuneTouch", &[On::Play("", None, immune_touch)]);

fn immune_touch(player_id: i32) -> card_sdk::Asked {
    let mut found = None;
    for s in 0..ctx::player_count() {
        if let Some((uid, _)) = ctx::field_instances(s)
            .into_iter()
            .find(|(_, id)| id.as_str() == "PP:再次闪耀")
        {
            found = Some(uid);
            break;
        }
    }
    let uid = found.unwrap_or(-1);
    let immune = uid >= 0 && ctx::is_immune_from_at(uid, player_id);
    if !immune && uid >= 0 {
        ctx::add_counter_at(uid, bandori_std::counter::CRYSTALS, 1, 0);
    }
    ctx::log(
        player_id,
        &Msg::new(key!("immune_touch_done"))
            .i("uid", uid as i64)
            .i("immune", immune as i64),
    );
    Ok(())
}

fn guard_play(player_id: i32) -> card_sdk::Asked {
    let Some(&target) = ctx::others(player_id).first() else {
        return Ok(());
    };
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(target, "TEST:guard", &Msg::new(key!("guard_note")));
    Ok(())
}

fn guard(player_id: i32, mut g: GateValue) -> GateValue {
    if trigger::target() == player_id && !trigger::cancelled() {
        ctx::log(player_id, &Msg::new(key!("guard_blocked")));
        g.allowed = Some(false); // deny
        return g;
    }
    g
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
        let reason = Msg::new(key!("probe_reason"));
        ctx::transfer(from, player_id, 100 * (seen + 1), &reason)?;
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

fn deny_yes(_player_id: i32) -> bool {
    // `trigger::` reads the **answered** link: a counter link (seq >= 2), not a
    // root play (seq 0). The rulebook does not forbid answering your own link
    // (ruling 2026-10-07: a counter's own round starts with its declarer), so
    // the declarer may deny their own counter.
    trigger::seq() >= 2
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

/// `PIPELINE-AUDIT` Q2 -- a 「分摊前」 probe: a placed card whose `payCombinedAdd`
/// hook cuts the **command total** by 500 (floor 0). With the pre-split stage
/// wired right, a 「[分摊][支付]2000」 across two payers charges 750 each
/// (ceil10(1500/2)); wired as a per-share stage it would charge 500 each.
const TOTAL_CUT: CardDef = CardDef::new(
    "TEST:totalCut",
    &[On::Filter(Filter::PayCombinedAdd(
        "",
        Some(total_cut_yes),
        total_cut,
    ))],
);

fn total_cut_yes(_player_id: i32) -> bool {
    ctx::is_placed()
}

fn total_cut(player_id: i32, value: i32) -> i32 {
    let out = (value - 500).max(0);
    ctx::log(
        player_id,
        &Msg::new(key!("total_cut_done"))
            .i("from", value as i64)
            .i("to", out as i64),
    );
    out
}

/// `G02` §9 row 19 -- a 分摊 command whose **command total** is 12,000:
/// every other live player is designated and the total divides by the
/// designated count (three payers of 4,000 each). No shipped 分摊 card prints
/// a total this large, and `pay_total_stage` (「一次性」) can only be seen on
/// one.
const SPLIT_12K: CardDef = CardDef::new("TEST:split12k", &[On::Play("", None, split_12k)]);

fn split_12k(player_id: i32) -> card_sdk::Asked {
    let others = ctx::others(player_id);
    if others.is_empty() {
        return Ok(());
    }
    ctx::split_pay(&others, player_id, 12_000, &Msg::new(key!("split12k_why")))?;
    Ok(())
}

/// `G02` §9 row 24 -- a placed card whose `paySplitAdd` filter tries to write
/// the payment shape too late. `set_pay_shape` returns whether the write
/// reaches the payment; the body logs it so a test can read the answer.
const SHAPE_LATE: CardDef = CardDef::new(
    "TEST:shapeLate",
    &[On::Filter(Filter::PaySplitAdd(
        "",
        Some(total_cut_yes),
        shape_late,
    ))],
);

fn shape_late(player_id: i32, v: PayAddValue) -> PayAddValue {
    let wrote = ctx::trigger::set_pay_shape(card_sdk::abi::PayShape::Fixed);
    ctx::log(
        player_id,
        &Msg::new(key!("shape_late_done")).i("wrote", wrote as i64),
    );
    v
}

/// `G02` §9 row 20 -- the raw 消耗 arm: a body that deletes money without a
/// payment command, so the `spendBefore` window is the only place a responder
/// can see it.
const LOSE_RAW: CardDef = CardDef::new("TEST:loseRaw", &[On::Play("", None, lose_raw)]);

fn lose_raw(player_id: i32) -> card_sdk::Asked {
    ctx::lose_raw(player_id, 1000, &Msg::new(key!("lose_raw_why")));
    Ok(())
}

/// `G02` §9 row 21 -- a **card-effect** fire spend (`ChargeSource::Other`), the
/// sibling of a skill cost. A placed `MyGO:（灯）不再迷茫` must not offer its
/// crystal swap for it.
const SPEND_FIRE: CardDef = CardDef::new("TEST:spendFire", &[On::Play("", None, spend_fire)]);

fn spend_fire(player_id: i32) -> card_sdk::Asked {
    bandori_std::fire::spend(player_id, 1, &Msg::new(key!("spend_fire_why")))?;
    Ok(())
}

/// `PIPELINE-AUDIT` B3 -- a `bankruptBefore` probe that tries to move money
/// **for the dying player**. Once the seat is marked dead before the window
/// (K2 / 规则书 L16+L81) the payment is refused and the probe's owner gains
/// nothing.
const DEAD_PAY: CardDef = CardDef::new(
    "TEST:deadPay",
    &[On::Action(Action::BankruptBefore(
        "",
        Some(dead_pay_yes),
        dead_pay,
    ))],
);

fn dead_pay_yes(player_id: i32) -> bool {
    // The instance's owner is a bystander; the trigger's `player_id` is the
    // seat going bankrupt.
    ctx::is_placed() && trigger::player_id() != player_id
}

fn dead_pay(player_id: i32) -> card_sdk::Asked {
    let dying = trigger::player_id();
    let got = ctx::transfer(dying, player_id, 1, &Msg::new(key!("dead_pay_reason")))?;
    ctx::log(
        player_id,
        &Msg::new(key!("dead_pay_done"))
            .i("dying", dying as i64)
            .i("got", got as i64),
    );
    Ok(())
}

/// `PIPELINE-AUDIT` Q4 -- 「A[支付]A」: the user pays themselves 5000. The
/// counteraction windows run; the settlement follows the affordability path.
const SELF_CHARGE: CardDef = CardDef::new("TEST:selfCharge", &[On::Play("", None, self_charge)]);

fn self_charge(player_id: i32) -> card_sdk::Asked {
    let got = ctx::transfer(
        player_id,
        player_id,
        5000,
        &Msg::new(key!("self_charge_reason")),
    )?;
    ctx::log(
        player_id,
        &Msg::new(key!("self_charge_done")).i("got", got as i64),
    );
    Ok(())
}

/// `PIPELINE-AUDIT` B2 -- a `paySplitAdd` probe that boosts **every** payment
/// through the pipeline by 100, with no target filter. Placed on a seat that
/// then goes bankrupt: after 规则书 L81 「所有其正在生效的卡，技能效果停止生效」
/// the boost must not reach anyone's money.
const PAY_ADD_ANY: CardDef = CardDef::new(
    "TEST:payAddAny",
    &[
        On::Play("", None, pay_add_any_play),
        On::Filter(Filter::PaySplitAdd("", Some(total_cut_yes), pay_add_any)),
    ],
);

fn pay_add_any_play(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "TEST:payAddAny",
        &Msg::new(key!("pay_add_any_note")),
    );
    Ok(())
}

fn pay_add_any(player_id: i32, mut v: PayAddValue) -> PayAddValue {
    let out = v.value + 100;
    ctx::log(
        player_id,
        &Msg::new(key!("pay_add_any_done")).i("n", v.value as i64),
    );
    PayAddValue { value: out, ..v }
}

/// `MONEY-01` -- the rent figure's own 「收费」 stages. `rentAdd` is the fixed
/// step (支付阶段 2) and `rentMul` the scale step (支付阶段 4): both sit before
/// the payment's stages, so a rent-shaped charge composes with them instead of
/// being rewritten by a payment-side body.
const RENT_ADD_100: CardDef = CardDef::new(
    "TEST:rentAdd100",
    &[On::Filter(Filter::RentAdd("card.placed", None, add_100))],
);

const RENT_MUL_HALF: CardDef = CardDef::new(
    "TEST:rentMulHalf",
    &[On::Filter(Filter::RentMul("card.placed", None, half))],
);

/// The settle's one `settleAdd` / `settleMul` window (`MONEY-01`): what the
/// settle costs at 支付阶段 2 / 4, composed once for every leg of the settle.
const SETTLE_ADD_100: CardDef = CardDef::new(
    "TEST:settleAdd100",
    &[On::Filter(Filter::SettleAdd("card.placed", None, add_100))],
);

/// A second `settleAdd` responder: two of them on one stage must **sum**.
const SETTLE_ADD_50: CardDef = CardDef::new(
    "TEST:settleAdd50",
    &[On::Filter(Filter::SettleAdd("card.placed", None, add_50))],
);

const SETTLE_MUL_HALF: CardDef = CardDef::new(
    "TEST:settleMulHalf",
    &[On::Filter(Filter::SettleMul("card.placed", None, half))],
);

/// The settle's 「地租」 twins: `settleRentAdd` (+100) and `settleRentMul`
/// (×0.5) shape the rent leg of a settle only, so a card-driven leg of the
/// same settle keeps its own figure.
const SETTLE_RENT_ADD_100: CardDef = CardDef::new(
    "TEST:settleRentAdd100",
    &[On::Filter(Filter::SettleRentAdd(
        "card.placed",
        None,
        add_100,
    ))],
);

const SETTLE_RENT_MUL_HALF: CardDef = CardDef::new(
    "TEST:settleRentMulHalf",
    &[On::Filter(Filter::SettleRentMul("card.placed", None, half))],
);

/// A second, non-rent leg of the settle: a body that pays 1000 on top.
const SETTLE_PAY_1000: CardDef = CardDef::new(
    "TEST:settlePay1000",
    &[On::Action(Action::SettleBody(
        "actor == owner && card.placed",
        None,
        settle_pay_1000,
    ))],
);

fn settle_pay_1000(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("settle_pay_1000_done")));
    ctx::pay(player_id, 1000, &Msg::new(key!("settle_pay_1000_reason")))?;
    Ok(())
}

/// MONEY-01 idempotence probe: arm a 支付减半 and then settle a tile whose
/// settle has to prompt, so the drive suspends and its prefix re-runs on the
/// resume. The 减半 must land **once**.
const FACTOR_SUSPEND: CardDef = CardDef::new("TEST:factorSusp", &[On::Play("", None, factor_susp)]);

fn factor_susp(player_id: i32) -> card_sdk::Asked {
    // The nested settles below happen after this body returns, so the halves
    // ride the turn plan's 「地租」 factor, which the settles' own move clones
    // carry.
    ctx::plan::set_rent_factor(500);
    // A prompt of its own: the body is re-executed from the start when the
    // drive resumes, replaying the `set_rent_factor` above. The 减半 must land
    // once.
    let _ = ctx::ask_yes(
        player_id,
        &Msg::new(key!("settle_ask_title")),
        &Msg::new(key!("settle_ask_text")),
    )?;
    // 购物中心 -- the test owns it for the other seat. Two settles: the second
    // one pays after the resume, where a doubled factor would show.
    ctx::settle_tile_at(1 - player_id, 1, true);
    ctx::settle_tile_at(1 - player_id, 1, true);
    Ok(())
}

/// A `SettleBody` hook that always asks -- a prompt purely to suspend the
/// arming body's drive mid-settle (MONEY-01 re-run probe).
const SETTLE_ASK: CardDef = CardDef::new(
    "TEST:settleAsk",
    &[On::Action(Action::SettleBody(
        "actor == 1",
        None,
        settle_ask,
    ))],
);

fn settle_ask(player_id: i32) -> card_sdk::Asked {
    let _ = ctx::ask_yes(
        player_id,
        &Msg::new(key!("settle_ask_title")),
        &Msg::new(key!("settle_ask_text")),
    )?;
    Ok(())
}

/// +50 on the stage input.
fn add_50(_player_id: i32, value: i32) -> i32 {
    value + 50
}

/// +100 on the stage input.
fn add_100(_player_id: i32, value: i32) -> i32 {
    value + 100
}

/// ÷2, rounded up (the rounding every halving body uses).
fn half(_player_id: i32, value: i32) -> i32 {
    value / 2 + value % 2
}

/// A 「不[触发结算]」 **teleport move** (`SETTLE-STAGES.md` §6 M6a / ruling R2):
/// `plan::set_kind(Teleport)` + `set_settles(false)` + `run_planned_move`, so the move
/// goes through `teleport_as` and must still raise `passTile` + `passPlayer` at
/// its destination. `to` is tiles ahead of the mover (default 5).
const TELE_NOSOLVE: CardDef =
    CardDef::new("TEST:tele_nosettle", &[On::Play("", None, tele_nosettle)]);

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
    ctx::plan::set_settles(false);
    ctx::run_planned_move(player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("mover_done")).i("pos", ctx::player_pos(player_id) as i64),
    );
    Ok(())
}

/// Q3 (`PIPELINE-AUDIT`, user ruling 2026-10-07): a `paySplitAdd` hook that drives
/// a **1000**-shaped payment to **-500**, with no floor. The reverse child
/// (amount 500) does not match the guard and settles normally -- so one
/// reversal, not a flip-flop loop. Placed like `TEST:totalCut`.
const PAY_OVERCUT: CardDef = CardDef::new(
    "TEST:payOvercut",
    &[On::Filter(Filter::PaySplitAdd(
        "",
        Some(total_cut_yes),
        pay_overcut,
    ))],
);

fn pay_overcut(player_id: i32, v: PayAddValue) -> PayAddValue {
    if v.value == 1000 {
        ctx::log(
            player_id,
            &Msg::new(key!("pay_overcut_done"))
                .i("from", v.value as i64)
                .i("to", -500),
        );
        PayAddValue { value: -500, ..v }
    } else {
        v
    }
}

/// Q3 two-sided: the user pays the next player 1000.
const XFER_1000: CardDef = CardDef::new("TEST:xfer1000", &[On::Play("", None, xfer_1000)]);

fn xfer_1000(player_id: i32) -> card_sdk::Asked {
    let n = ctx::player_count();
    let to = (player_id + 1) % n;
    ctx::transfer(player_id, to, 1000, &Msg::new(key!("xfer_1000_reason")))?;
    Ok(())
}

/// Q3 one-sided 「[获得]」: the user gains 1000 from the bank.
const GAIN_1000: CardDef = CardDef::new("TEST:gain1000", &[On::Play("", None, gain_1000)]);

fn gain_1000(player_id: i32) -> card_sdk::Asked {
    ctx::gain(player_id, 1000, &Msg::new(key!("gain_1000_reason")))?;
    Ok(())
}

/// Q3 one-sided 「[消耗]」: the user loses 1000 to the bank.
const LOSE_1000: CardDef = CardDef::new("TEST:lose1000", &[On::Play("", None, lose_1000)]);

fn lose_1000(player_id: i32) -> card_sdk::Asked {
    ctx::pay(player_id, 1000, &Msg::new(key!("lose_1000_reason")))?;
    Ok(())
}

/// Marker window (user ruling 2026-10-07): a [反击] that answers `markerSpend`
/// and cancels the spend -- nothing moves. No shipped card listens to the
/// window yet; this fixture pins the shape.
const MARKER_DENY: CardDef = CardDef::new(
    "TEST:markerDeny",
    &[On::Counteract(
        &[ChainKind::MarkerSpend],
        "",
        Some(marker_deny_yes),
        marker_deny,
    )],
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
    ctx::add_held_by_name(player_id, "TEST:tok", 5, 99)?;
    let moved = ctx::add_held_by_name(player_id, "TEST:tok", -3, 99)?;
    ctx::log(
        player_id,
        &Msg::new(key!("marker_spend_done")).n("moved", moved as i64),
    );
    Ok(())
}

/// Tags every move plan `fireRoll` at `rollPlan`. G03 moved the 「使用火罐进行
/// 移动掷骰」 key onto `roll_source` (`TEST:fireRecast` supplies that), so the
/// tag is now only a marker a test can assert on.
const CHARGE_ROLL: CardDef = CardDef::new(
    "TEST:fireRoll",
    &[
        On::Play("", None, fire_roll_play),
        On::Filter(Filter::DiceRollPlan("", None, fire_roll_plan)),
    ],
);

fn fire_roll_play(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "TEST:fireRoll",
        &Msg::new(key!("fire_roll_note")),
    );
    Ok(())
}

fn fire_roll_plan(_player_id: i32, v: PlanValue) -> PlanValue {
    // Pure PlanValue write instead of the old in-body `plan::set_tag`
    // (refuse_if_filter'd).
    PlanValue {
        tag: Some(("fireRoll", 1)),
        ..v
    }
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
    &[On::Counteract(
        &[ChainKind::Effect],
        "actor == owner && value >= 999999",
        Some(pre_reject_guard),
        pre_reject_body,
    )],
);

fn pre_reject_guard(_player_id: i32) -> bool {
    // Residual guard: would accept. The condition rejects first.
    true
}

fn pre_reject_body(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("pre_reject_ran")));
    Ok(())
}

/// Guard **condition** accepts (`actor == owner` -- the `condition::MINE` sugar):
/// the guard runs. Nothing rejected twice -- the guard does not re-check
/// `actor == owner`.
const PRE_ACCEPT: CardDef = CardDef::new(
    "TEST:preAccept",
    &[On::Counteract(
        &[ChainKind::Effect],
        card_sdk::condition::MINE,
        Some(pre_accept_guard),
        pre_accept_body,
    )],
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
        On::Filter(Filter::PaySplitAdd(
            "false",
            Some(pre_hook_guard),
            pre_hook_body,
        )),
    ],
);

fn pre_hook_place(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(player_id, "TEST:preHook", &Msg::new(key!("pre_hook_note")));
    Ok(())
}

fn pre_hook_guard(_player_id: i32) -> bool {
    true
}

fn pre_hook_body(player_id: i32, mut v: PayAddValue) -> PayAddValue {
    ctx::log(player_id, &Msg::new(key!("pre_hook_ran")));
    PayAddValue {
        value: v.value,
        ..v
    }
}

// v49 (docs/GUARDS.md §4.2c): `On::Gate` / `On::Schedule` / `On::RollPlan` /
// `On::Settle` carry the same `condition` + residual-guard pair. Each fixture below
// declares `condition: "false"` on one of those kinds, so the entry must run no body
// (and so emit no `card` flash) whenever the kind is asked.

/// `On::Gate` with a rejecting condition: a gate raise must not reach the body
/// (and so must not claim the answer).
const PRE_GATE: CardDef = CardDef::new(
    "TEST:preGate",
    &[
        On::Play("", None, pre_gate_place),
        On::Filter(Filter::ImmuneAll("false", None, pre_gate_body)),
    ],
);

fn pre_gate_place(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(player_id, "TEST:preGate", &Msg::new(key!("pre_gate_note")));
    Ok(())
}

fn pre_gate_body(_player_id: i32, mut g: GateValue) -> GateValue {
    g.allowed = Some(false); // deny
    g
}

/// `On::RollPlan` with a rejecting condition: the move being planned must not
/// reach the routine.
const PRE_ROLLPLAN: CardDef = CardDef::new(
    "TEST:preRollPlan",
    &[
        On::Play("", None, pre_rollplan_place),
        On::Filter(Filter::DiceRollPlan("false", None, pre_rollplan_body)),
    ],
);

fn pre_rollplan_place(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "TEST:preRollPlan",
        &Msg::new(key!("pre_rollplan_note")),
    );
    Ok(())
}

fn pre_rollplan_body(player_id: i32, v: PlanValue) -> PlanValue {
    // Pure PlanValue write instead of the old in-body `plan::set_tag`. The log
    // is allowed (not a mutation).
    ctx::log(player_id, &Msg::new(key!("pre_rollplan_ran")));
    PlanValue {
        tag: Some(("preRollPlan", 1)),
        ..v
    }
}

/// `On::Schedule` with a rejecting condition: the scheduled turn-end callback
/// must not run.
const PRE_ATEMD: CardDef = CardDef::new(
    "TEST:preAtEnd",
    &[
        On::Play("", None, pre_at_end_place),
        On::Schedule("pre_at_end_body", "false", None, pre_at_end_body),
    ],
);

fn pre_at_end_place(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "TEST:preAtEnd",
        &Msg::new(key!("pre_at_end_note")),
    );
    ctx::at_turn_end(player_id, "pre_at_end_body");
    Ok(())
}

fn pre_at_end_body(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("pre_at_end_ran")));
    Ok(())
}

/// `On::Settle` with a rejecting condition: the settle body must not govern
/// the settle (no body, no flash).
const PRE_SETTLE: CardDef = CardDef::new(
    "TEST:preSettle",
    &[On::Settle("false", None, pre_settle_body)],
);

fn pre_settle_body(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("pre_settle_ran")));
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
        On::Action(Action::PassTile("", Some(pass_tele_guard), pass_tele_body)),
    ],
);

fn pass_tele_place(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "TEST:pass_tele",
        &Msg::new(key!("pass_tele_note")),
    );
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
        On::Action(Action::Settle("", Some(tile_hook_yes), tile_hook)),
    ],
);

fn tile_hook_place(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "TEST:tileHook",
        &Msg::new(key!("tile_hook_note")),
    );
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
    &[On::Counteract(
        &[ChainKind::Card],
        "",
        Some(deny_play_yes),
        deny_play,
    )],
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

/// A `passTile` hook that only announces itself (a `"card"` flash plus one log
/// line) and lets the walk continue. Pins the lazy-flush order: the walk's
/// `roll`/`move` event covering the hook's tile must land **before** the flash,
/// and the remainder of the walk continues from that tile.
///
/// Guard: fires on the tile named by `TEST.pass_flash.tile` (set by the test;
/// unset/`-1` never fires), once per walk (`TEST.pass_flash.done`).
const PASS_FLASH: CardDef = CardDef::new(
    "TEST:pass_flash",
    &[
        On::Play("", None, pass_flash_place),
        On::Action(Action::PassTile(
            "",
            Some(pass_flash_guard),
            pass_flash_body,
        )),
    ],
);

fn pass_flash_place(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "TEST:pass_flash",
        &Msg::new(key!("pass_flash_note")),
    );
    Ok(())
}

fn pass_flash_guard(player_id: i32) -> bool {
    ctx::is_placed()
        && trigger::player_id() == player_id
        && trigger::tile() == ctx::state::get(player_id, "TEST.pass_flash.tile")
        && ctx::state::get(player_id, "TEST.pass_flash.done") == 0
}

fn pass_flash_body(player_id: i32) -> card_sdk::Asked {
    ctx::state::set(player_id, "TEST.pass_flash.done", 1);
    ctx::log(player_id, &Msg::new(key!("pass_flash_ran")));
    Ok(())
}

/// [`PASS_FLASH`]'s pre-half: a `passBefore` hook that only announces itself.
/// The flush it forces covers steps up to the **previous** tile (`passBefore`
/// fires before the step onto `next`).
///
/// Guard: `TEST.pass_before_flash.tile` / `TEST.pass_before_flash.done`, the
/// same shape as [`PASS_FLASH`].
const PASS_BEFORE_FLASH: CardDef = CardDef::new(
    "TEST:pass_before_flash",
    &[
        On::Play("", None, pass_before_flash_place),
        On::Action(Action::PassBefore(
            "",
            Some(pass_before_flash_guard),
            pass_before_flash_body,
        )),
    ],
);

fn pass_before_flash_place(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "TEST:pass_before_flash",
        &Msg::new(key!("pass_flash_note")),
    );
    Ok(())
}

fn pass_before_flash_guard(player_id: i32) -> bool {
    ctx::is_placed()
        && trigger::player_id() == player_id
        && trigger::tile() == ctx::state::get(player_id, "TEST.pass_before_flash.tile")
        && ctx::state::get(player_id, "TEST.pass_before_flash.done") == 0
}

fn pass_before_flash_body(player_id: i32) -> card_sdk::Asked {
    ctx::state::set(player_id, "TEST.pass_before_flash.done", 1);
    ctx::log(player_id, &Msg::new(key!("pass_flash_ran")));
    Ok(())
}

// -- cross-card messages (v50) ------------------------------------------------

/// Answers `"ping"` with `a + b`, logging the sender. The worked example of
/// `On::Message` + `ctx::send`.
const PING: CardDef = CardDef::new(
    "TEST:ping",
    &[
        On::Message(&["ping"], "", None, on_ping),
        On::Play("", None, ping_send),
    ],
);

fn on_ping(player_id: i32) -> card_sdk::Asked {
    let sum = ctx::message::a() + ctx::message::b();
    ctx::message::reply(sum);
    ctx::log(player_id, &Msg::new(key!("ping_got")).i("n", sum as i64));
    Ok(())
}

fn ping_send(player_id: i32) -> card_sdk::Asked {
    // Address the first `TEST:ping` on our own field (the placed copy). A
    // board pseudo card with no `On::Message` answers 0.
    let reply = ctx::send(
        &card_sdk::abi::Target::Card {
            player: player_id,
            card: "TEST:ping".into(),
        },
        "ping",
        &card_sdk::abi::Message {
            name: "ping".into(),
            arg0: 2,
            arg1: 3,
            ..Default::default()
        },
    )?;
    ctx::log(
        player_id,
        &Msg::new(key!("ping_reply")).i("n", reply as i64),
    );
    Ok(())
}

/// Declares `"ping"` but its `condition` rejects every sender -- never answers.
const PING_REJECT: CardDef = CardDef::new(
    "TEST:pingReject",
    &[On::Message(
        &["ping"],
        "actor == owner",
        None,
        on_ping_reject,
    )],
);

fn on_ping_reject(player_id: i32) -> card_sdk::Asked {
    ctx::message::reply(-99);
    ctx::log(player_id, &Msg::new(key!("ping_got")));
    Ok(())
}

/// A body that rolls (a `"dice"` event) and then pays (a host request that
/// pauses and re-runs). Exactly one dice event must land.
const ROLL_THEN_PAY: CardDef =
    CardDef::new("TEST:rollThenPay", &[On::Play("", None, roll_then_pay)]);

fn roll_then_pay(player_id: i32) -> card_sdk::Asked {
    let _ = ctx::roll(player_id, 1, 6, RollPurpose::Effect, roll_source::CARD)?;
    ctx::pay(player_id, 100, &Msg::new(key!("roll_then_pay_reason")))?;
    Ok(())
}

/// A `deckBeforeGame` hook that places itself -- the pile->field transition
/// must not dispatch it twice (two headers for one body).
const DECK_ONCE: CardDef = CardDef::new(
    "TEST:deckOnce",
    &[On::Action(Action::DeckBeforeGame("", None, deck_once))],
);

fn deck_once(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Effect);
    ctx::place_card(
        player_id,
        "TEST:deckOnce",
        &Msg::new(key!("deck_once_note")),
    );
    Ok(())
}

/// A field card that bends the turn clock through the modifier hooks:
/// 免费时间 refill +7 s, 恢复时间 doubled.
const CLOCK_HOOK: CardDef = CardDef::new(
    "TEST:clockHook",
    &[
        On::Play("", None, clock_hook_play),
        On::Filter(Filter::TurnBonusTime("", None, clock_bonus)),
        On::Filter(Filter::TurnBaseTime("", None, clock_base)),
    ],
);

fn clock_hook_play(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "TEST:clockHook",
        &Msg::new(key!("deck_once_note")),
    );
    Ok(())
}

fn clock_bonus(_player_id: i32, value: i32) -> i32 {
    value + 7
}

fn clock_base(_player_id: i32, value: i32) -> i32 {
    value * 2
}

/// Reads money, pays (host effect changes money), reads money again.
/// Pins HostRequest replay semantics: the second read sees the post-pay
/// amount (replay re-run rebases onto live+host_effects) or the pre-pay
/// amount (learn pass keeps its first-pass snapshot).
const READ_HOST_READ: CardDef =
    CardDef::new("TEST:readHostRead", &[On::Play("", None, read_host_read)]);

fn read_host_read(player_id: i32) -> card_sdk::Asked {
    let houses = |p: i32| {
        ctx::owned_tiles(p)
            .into_iter()
            .filter(|&t| ctx::is_live_house_for(p, t))
            .count() as i64
    };
    let h0 = houses(player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("rhr_before")).i("houses", h0).i("n", 0),
    );
    // Host effect: teleport to the agent tile; its settle may buy a livehouse.
    ctx::plan::set_teleport_to(ctx::tile_named("地产商"));
    let _ = ctx::run_planned_move(player_id);
    let h1 = houses(player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("rhr_after")).i("houses", h1).i("n", 0),
    );
    Ok(())
}

/// STACK-01 regression: a placed card whose `SettleBody` re-settles the same
/// tile while its crystal counter lasts. N nested settles, then stop. The
/// engine's work stack must run the chain on a small thread stack.
const SETTLE_NEST: CardDef = CardDef::new(
    "TEST:settleNest",
    &[On::Action(Action::SettleBody(
        "actor == owner && card.placed && card.counter('crystals') > 0",
        None,
        settle_nest,
    ))],
);

fn settle_nest(player_id: i32) -> card_sdk::Asked {
    let n = ctx::crystals();
    if n <= 0 {
        return Ok(());
    }
    ctx::set_crystals(n - 1);
    let at = ctx::trigger::tile();
    if at < 0 {
        return Ok(());
    }
    ctx::settle_tile_at(player_id, at, true);
    Ok(())
}

/// STACK-01: a SettleBody hook whose body logs before a nested settle, then
/// after it (the tail). A sibling hook logs too. The test pins that the tail
/// runs exactly once and the sibling is not re-run.
const SETTLE_TAIL: CardDef = CardDef::new(
    "TEST:settleTail",
    &[On::Action(Action::SettleBody(
        "actor == owner && card.placed",
        None,
        settle_tail,
    ))],
);

fn settle_tail(player_id: i32) -> card_sdk::Asked {
    // Only on the tile this card sits on (tile 5 in the test) -- not every
    // settle the owner makes.
    if ctx::trigger::tile() != 5 {
        return Ok(());
    }
    ctx::log(player_id, &Msg::new(key!("tail_before")));
    // Settle a *different* tile so this is a 1-level nest (the tail must run
    // exactly once). Tile 0 has no settleTail instance.
    // The nested settle is by the *other* seat: a counteract condition sees
    // the move's tile, not the nested settle's, so `actor` is what tells the
    // inner settle apart from the outer one.
    ctx::settle_tile_at(1 - player_id, 0, true);
    ctx::log(player_id, &Msg::new(key!("tail_after")));
    Ok(())
}

/// Sibling hook on SettleBody -- must run once per settle, not again on resume.
const SETTLE_SIB: CardDef = CardDef::new(
    "TEST:settleSib",
    &[On::Action(Action::SettleBody(
        "actor == owner && card.placed",
        None,
        settle_sib,
    ))],
);

fn settle_sib(player_id: i32) -> card_sdk::Asked {
    if ctx::trigger::tile() != 5 {
        return Ok(());
    }
    ctx::log(player_id, &Msg::new(key!("sib_once")));
    Ok(())
}

/// A `Settle` [反击] that cancels the settle outright. Only offered for the
/// *other* seat's settle (`actor == 1`; the tail nests for the other player),
/// so the outer settle of the same turn is left alone. Answers `Settle`, not
/// `SettleBefore` -- the latter is only raised by a real walk, never by a
/// card-driven `settle_tile_at`. A settle cancelled at its declaration never
/// raises `settleAdd` / `settleMul`; the outer settle's stored 支付价格 shaping
/// must survive it.
const SETTLE_CANCEL: CardDef = CardDef::new(
    "TEST:settleCancel",
    &[On::Counteract(
        &[ChainKind::Settle],
        "actor == 1",
        None,
        settle_cancel,
    )],
)
.props(&[(card_sdk::abi::prop::COUNTERACT_FROM_FIELD, 1)]);

fn settle_cancel(player_id: i32) -> card_sdk::Asked {
    ctx::log(
        player_id,
        &Msg::new(key!("cancel_at")).i("tile", ctx::trigger::tile() as i64),
    );
    ctx::trigger::set_cancelled();
    Ok(())
}

/// Silent-entry presentation (`On::silent`). Three fixtures pin the
/// contract:
/// * `TEST:silentPair` -- a silent bookkeeping hook beside a flashing sibling
///   on the same settle: only the sibling posts a `"card"` event.
/// * `TEST:silentNest` -- a silent body that nests a real activation: the
///   nested card still flashes.
/// * `TEST:silentPay` -- a silent body that pays: the debug guard records a
///   violation (`game_rules::take_silent_visible`).
const SILENT_PAIR: CardDef = CardDef::new(
    "TEST:silentPair",
    &[
        On::Play("", None, silent_pair_place),
        On::Action(Action::Settle("", Some(silent_hook_yes), silent_pair_book)).silent(),
        On::Action(Action::Settle("", Some(silent_hook_yes), silent_pair_flash)),
    ],
);

const SILENT_NEST: CardDef = CardDef::new(
    "TEST:silentNest",
    &[
        On::Play("", None, silent_nest_place),
        On::Action(Action::Settle("", Some(silent_hook_yes), silent_nest_body)).silent(),
    ],
);

const SILENT_PAY: CardDef = CardDef::new(
    "TEST:silentPay",
    &[
        On::Play("", None, silent_pay_place),
        On::Action(Action::Settle("", Some(silent_hook_yes), silent_pay_body)).silent(),
    ],
);

fn silent_pair_place(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "TEST:silentPair",
        &Msg::new(key!("silent_pair_note")),
    );
    Ok(())
}

fn silent_nest_place(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "TEST:silentNest",
        &Msg::new(key!("silent_nest_note")),
    );
    Ok(())
}

fn silent_pay_place(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "TEST:silentPay",
        &Msg::new(key!("silent_pay_note")),
    );
    Ok(())
}

/// Shared settle-hook guard: the holder's own land, while the card is placed.
fn silent_hook_yes(player_id: i32) -> bool {
    ctx::is_placed() && trigger::player_id() == player_id
}

/// Pure bookkeeping: a declaration write with no log and no host effect.
fn silent_pair_book(player_id: i32) -> card_sdk::Asked {
    ctx::state::set(player_id, "TEST.silentPair.declared", 1);
    Ok(())
}

/// The flashing sibling: one narration line under its own header.
fn silent_pair_flash(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("silent_pair_flash")));
    Ok(())
}

/// A silent body that nests a real activation (`TEST:echo`'s play).
fn silent_nest_body(player_id: i32) -> card_sdk::Asked {
    ctx::state::set(player_id, "TEST.silentNest.declared", 1);
    ctx::play_card("TEST:echo", player_id)?;
    Ok(())
}

/// A silent body that pays -- the debug guard must fire.
fn silent_pay_body(player_id: i32) -> card_sdk::Asked {
    ctx::state::set(player_id, "TEST.silentPay.declared", 1);
    ctx::gain(player_id, 100, &Msg::new(key!("silent_pay_reason")))?;
    Ok(())
}

/// QuietWhenEmpty (`On::quiet_when_empty`): the entry runs but its header
/// is retracted when the body produced nothing visible.
/// * entry 1 (`TEST.quiet.noop`) — body writes scratch state only → retracted.
/// * entry 2 (`TEST.quiet.log`) — body logs → keeps its header.
const SILENT_QUIET: CardDef = CardDef::new(
    "TEST:silentQuiet",
    &[
        On::Play("", None, silent_quiet_place),
        On::Action(Action::Settle("", Some(silent_hook_yes), quiet_noop)).quiet_when_empty(),
        On::Action(Action::TurnEnd(card_sdk::condition::MINE, None, quiet_log)).quiet_when_empty(),
    ],
);

fn silent_quiet_place(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "TEST:silentQuiet",
        &Msg::new(key!("silent_quiet_note")),
    );
    Ok(())
}

fn quiet_noop(player_id: i32) -> card_sdk::Asked {
    ctx::state::set(player_id, "TEST.quiet.noop", 1);
    Ok(())
}

fn quiet_log(player_id: i32) -> card_sdk::Asked {
    if ctx::state::get(player_id, "TEST.quiet.said") != 0 {
        return Ok(());
    }
    ctx::state::set(player_id, "TEST.quiet.said", 1);
    ctx::log(player_id, &Msg::new(key!("quiet_log_line")));
    Ok(())
}

/// QuietWhenEmpty bodies whose only **visible** effect is a log vec the
/// commit flush turns into after-hooks -- no `ctx::log` / pay / draw.
/// `body_empty` must count those too or the 「效果适用」 header is retracted
/// and then `counterChanged` / the named-raise hooks fire on an invisible
/// clause. The header must also **flush** on those rows: the write/raise is
/// the first visible outcome, so Ruling 2a posts the clause name above it.
/// * `TEST:quietCounter` -- only a counter write (`ctx::add_counter_at`, the
///   path that does **not** auto-narrate `log.outcome.counter`).
/// * `TEST:quietRaise` -- only a named raise (`ctx::raise_after`).
const QUIET_COUNTER: CardDef = CardDef::new(
    "TEST:quietCounter",
    &[
        On::Play("", None, quiet_counter_place),
        On::Action(Action::Settle(
            "",
            Some(silent_hook_yes),
            quiet_counter_body,
        ))
        .quiet_when_empty(),
    ],
);

const QUIET_RAISE: CardDef = CardDef::new(
    "TEST:quietRaise",
    &[
        On::Play("", None, quiet_raise_place),
        On::Action(Action::Settle("", Some(silent_hook_yes), quiet_raise_body)).quiet_when_empty(),
    ],
);

fn quiet_counter_place(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "TEST:quietCounter",
        &Msg::new(key!("quiet_counter_note")),
    );
    Ok(())
}

/// Only the counter write. `ctx::add_counter` / `set_counter` auto-narrate
/// `log.outcome.counter` (so `logged` would see them); `add_counter_at` is the
/// silent write, and its `counter_log` row is what raises `counterChanged`
/// after commit -- and what flushes the lazy header.
fn quiet_counter_body(_player_id: i32) -> card_sdk::Asked {
    ctx::add_counter_at(ctx::self_uid(), "quietTick", 1, 0);
    Ok(())
}

fn quiet_raise_place(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "TEST:quietRaise",
        &Msg::new(key!("quiet_raise_note")),
    );
    Ok(())
}

/// Only a post-commit named raise -- no log line of its own. The raise row
/// flushes the lazy header.
fn quiet_raise_body(player_id: i32) -> card_sdk::Asked {
    ctx::raise_after(player_id, "TEST.quietRaise.tick", 1);
    Ok(())
}

/// Ruling 2a: an optional prompt that is declined leaves no header, no flash,
/// no log. The lazy header only posts when a visible outcome follows.
const SILENT_ASK: CardDef = CardDef::new(
    "TEST:silentAsk",
    &[
        On::Play("", None, silent_ask_place),
        On::Action(Action::Settle("", Some(silent_hook_yes), silent_ask_body)),
    ],
);

fn silent_ask_place(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "TEST:silentAsk",
        &Msg::new(key!("silent_ask_note")),
    );
    Ok(())
}

/// Optional offer: decline = no trace; accept = one log line (so the header
/// appears).
fn silent_ask_body(player_id: i32) -> card_sdk::Asked {
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("silent_ask_title")),
        &Msg::new(key!("silent_ask_text")),
    )? {
        return Ok(());
    }
    ctx::log(player_id, &Msg::new(key!("silent_ask_taken")));
    Ok(())
}

// -- flash policy (v55) ------------------------------------------------------

use card_sdk::abi::Flash;

/// `Flash::Always` on an owner-self-only effect (gain to self): the player
/// should still see the flash.
const FLASH_ALWAYS: CardDef = CardDef::new(
    "TEST:flashAlways",
    &[On::Play("", None, flash_always).flash(Flash::Always)],
);

fn flash_always(player_id: i32) -> card_sdk::Asked {
    ctx::gain(player_id, 1, &Msg::new(key!("flash_always_gain")))?;
    Ok(())
}

/// `Flash::Never` on a cross-player effect (a pay leg to another seat): the
/// log entry stays, no flash / pulse.
const FLASH_NEVER: CardDef = CardDef::new(
    "TEST:flashNever",
    &[On::Play("", None, flash_never).flash(Flash::Never)],
);

fn flash_never(player_id: i32) -> card_sdk::Asked {
    let other = ctx::others(player_id).first().copied().unwrap_or(-1);
    ctx::pay_leg(player_id, other, 1, &Msg::new(key!("flash_never_pay")))?;
    Ok(())
}

/// `Flash::Always` **and** `QuietWhenEmpty` on a no-op body. `Always` is the
/// declarative spelling of `ctx::flash()` -- "cue me" -- so it must survive
/// the quiet retraction the same way the imperative cue does. Otherwise the
/// entry promises "always flashes" and then shows nothing at all.
const FLASH_ALWAYS_QUIET: CardDef = CardDef::new(
    "TEST:flashAlwaysQuiet",
    &[On::Play("", None, flash_always_quiet)
        .flash(Flash::Always)
        .quiet_when_empty()],
);

/// Scratch state only -- nothing visible, so the body counts as empty.
fn flash_always_quiet(player_id: i32) -> card_sdk::Asked {
    ctx::state::set(player_id, "TEST.flashAlwaysQuiet.noop", 1);
    Ok(())
}

/// Two settle hooks on the same trigger kind, `Always` then trailing `Auto`.
/// One drive, one flash decision: the fold must keep the `Always` (a trailing
/// `Auto` sibling must not drop it). Both bodies are no-ops, so `Auto` alone
/// would not flash -- the `Always` entry's declarative cue is the only reason
/// the drive shows anything (and the only header it posts).
const FLASH_FOLD_ALWAYS: CardDef = CardDef::new(
    "TEST:flashFoldAlways",
    &[
        On::Play("", None, flash_fold_always_place),
        On::Action(Action::Settle("", Some(silent_hook_yes), flash_fold_noop)).flash(Flash::Always),
        On::Action(Action::Settle("", Some(silent_hook_yes), flash_fold_noop)),
    ],
);

/// Two settle hooks on the same trigger kind, `Never` then trailing `Always`.
/// The fold's `Never` must suppress the whole drive -- a later `Always`
/// sibling must not lift it. The `Always` sibling is the one that posts the
/// header (declarative cue on a no-op body); it must not flash.
const FLASH_FOLD_NEVER: CardDef = CardDef::new(
    "TEST:flashFoldNever",
    &[
        On::Play("", None, flash_fold_never_place),
        On::Action(Action::Settle("", Some(silent_hook_yes), flash_fold_noop)).flash(Flash::Never),
        On::Action(Action::Settle("", Some(silent_hook_yes), flash_fold_noop)).flash(Flash::Always),
    ],
);

fn flash_fold_always_place(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "TEST:flashFoldAlways",
        &Msg::new(key!("flash_fold_note")),
    );
    Ok(())
}

fn flash_fold_never_place(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "TEST:flashFoldNever",
        &Msg::new(key!("flash_fold_note")),
    );
    Ok(())
}

/// Three settle hooks on one drive, each body a `ctx::log` so each flushes its
/// own 「效果适用」 header. Entry 0 is `Flash::Always` (the fold's winning
/// opinion); the other two are default `Auto`. One drive, one flash decision,
/// **every** header it posted -- all three must carry the folded `true`. A
/// trailing sibling's header must not keep the `flush_header` `already`
/// default-off.
const FLASH_HEADER_IDS: CardDef = CardDef::new(
    "TEST:flashHeaderIds",
    &[
        On::Play("", None, flash_header_ids_place),
        On::Action(Action::Settle(
            "",
            Some(silent_hook_yes),
            flash_header_ids_log,
        ))
        .flash(Flash::Always),
        On::Action(Action::Settle(
            "",
            Some(silent_hook_yes),
            flash_header_ids_log,
        )),
        On::Action(Action::Settle(
            "",
            Some(silent_hook_yes),
            flash_header_ids_log,
        )),
    ],
);

fn flash_header_ids_place(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "TEST:flashHeaderIds",
        &Msg::new(key!("flash_header_ids_note")),
    );
    Ok(())
}

/// A single log line: the first visible outcome for this entry, so the lazy
/// header flushes here (and only here -- no extra host request to piggyback on).
fn flash_header_ids_log(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("flash_header_ids_line")));
    Ok(())
}

/// Pure no-op: nothing visible, so `Auto` would not flash and the body posts
/// no second header. Enough to make the drive run both entries.
fn flash_fold_noop(_player_id: i32) -> card_sdk::Asked {
    Ok(())
}

/// `ctx::flash()` then `ask_yes`: exactly ONE flash, posted **before** the
/// prompt. The body re-runs after the answer and must not flash again.
const FLASH_THEN_ASK: CardDef =
    CardDef::new("TEST:flashThenAsk", &[On::Play("", None, flash_then_ask)]);

fn flash_then_ask(player_id: i32) -> card_sdk::Asked {
    ctx::flash();
    ctx::log(player_id, &Msg::new(key!("flash_then_ask_mark")));
    let yes = ctx::ask_yes(
        player_id,
        &Msg::new(key!("flash_ask_title")),
        &Msg::new(key!("flash_ask_text")),
    )?;
    if yes {
        ctx::log(player_id, &Msg::new(key!("flash_ask_yes")));
    } else {
        ctx::log(player_id, &Msg::new(key!("flash_ask_no")));
    }
    Ok(())
}

/// Two prompts in a row, one `ctx::flash()` before each: one flash per
/// distinct ordinal, none doubled across the three passes.
const FLASH_TWO_ASKS: CardDef =
    CardDef::new("TEST:flashTwoAsks", &[On::Play("", None, flash_two_asks)]);

fn flash_two_asks(player_id: i32) -> card_sdk::Asked {
    ctx::flash_card("TEST:flashAlways", &Msg::new(key!("flash_two_cue1")));
    ctx::log(player_id, &Msg::new(key!("flash_two_mark1")));
    let _ = ctx::ask_yes(
        player_id,
        &Msg::new(key!("flash_ask_title")),
        &Msg::new(key!("flash_ask_text")),
    )?;
    ctx::flash_card("TEST:flashAlways", &Msg::new(key!("flash_two_cue2")));
    ctx::log(player_id, &Msg::new(key!("flash_two_mark2")));
    let _ = ctx::ask_yes(
        player_id,
        &Msg::new(key!("flash_ask_title")),
        &Msg::new(key!("flash_ask_text")),
    )?;
    Ok(())
}

/// `ctx::flash_card_keyed` with the same key twice in one drive: flashes once.
const FLASH_KEYED_TWICE: CardDef = CardDef::new(
    "TEST:flashKeyedTwice",
    &[On::Play("", None, flash_keyed_twice)],
);

fn flash_keyed_twice(player_id: i32) -> card_sdk::Asked {
    ctx::flash_card_keyed("cue", "TEST:flashAlways", &Msg::new(key!("flash_keyed_a")));
    ctx::log(player_id, &Msg::new(key!("flash_keyed_mark1")));
    ctx::flash_card_keyed("cue", "TEST:flashAlways", &Msg::new(key!("flash_keyed_b")));
    ctx::log(player_id, &Msg::new(key!("flash_keyed_mark2")));
    Ok(())
}

/// `ctx::flash_card` posts a parent-linked `"flash"` event **before** later
/// effects (code order).
const FLASH_CARD_EARLY: CardDef = CardDef::new(
    "TEST:flashCardEarly",
    &[On::Play("", None, flash_card_early)],
);

fn flash_card_early(player_id: i32) -> card_sdk::Asked {
    ctx::flash_card("TEST:flashAlways", &Msg::new(key!("flash_card_cue")));
    ctx::log(player_id, &Msg::new(key!("flash_card_after")));
    Ok(())
}

/// `ctx::no_flash` suppresses an otherwise-flashing cross-player pay.
const FLASH_SUPPRESS: CardDef =
    CardDef::new("TEST:flashSuppress", &[On::Play("", None, flash_suppress)]);

fn flash_suppress(player_id: i32) -> card_sdk::Asked {
    ctx::no_flash();
    let other = ctx::others(player_id).first().copied().unwrap_or(-1);
    ctx::pay_leg(player_id, other, 1, &Msg::new(key!("flash_suppress_pay")))?;
    Ok(())
}

/// Owner-self-only play (gain to self) with default `Flash::Auto`: no flash.
const FLASH_AUTO_SELF: CardDef =
    CardDef::new("TEST:flashAutoSelf", &[On::Play("", None, flash_auto_self)]);

fn flash_auto_self(player_id: i32) -> card_sdk::Asked {
    ctx::gain(player_id, 1, &Msg::new(key!("flash_auto_gain")))?;
    Ok(())
}

/// Nested drive: the outer body flashes then plays an inner card that also
/// flashes. The two flash sets are independent.
const FLASH_OUTER: CardDef = CardDef::new("TEST:flashOuter", &[On::Play("", None, flash_outer)]);

fn flash_outer(player_id: i32) -> card_sdk::Asked {
    ctx::flash_card("TEST:flashAlways", &Msg::new(key!("flash_outer_cue")));
    ctx::play_card("TEST:flashInner", player_id)?;
    Ok(())
}

/// The nested drive's own flash (played by [`FLASH_OUTER`]).
const FLASH_INNER: CardDef = CardDef::new("TEST:flashInner", &[On::Play("", None, flash_inner)]);

fn flash_inner(player_id: i32) -> card_sdk::Asked {
    ctx::flash_card("TEST:flashAlways", &Msg::new(key!("flash_inner_cue")));
    Ok(())
}

/// Downstream custom **filter** kind fixture: registers
/// `bandori_std::kind::Filters::MyRewrite` (a ruleset-ns filter declared via
/// `filter_kinds!`). Proves the declaration → registration → dispatch path for
/// a rule-defined filter. No game semantics.
const CUSTOM_FILTER: CardDef = CardDef::new(
    "TEST:customFilter",
    &[
        On::Play("", None, custom_filter_play),
        On::Filter(bandori_std::kind::Filters::MyRewrite("", None, my_rewrite)),
    ],
);

fn custom_filter_play(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "TEST:customFilter",
        &Msg::new(key!("custom_filter_note")),
    );
    Ok(())
}

fn my_rewrite(_player_id: i32, v: PlanValue) -> PlanValue {
    PlanValue {
        min_roll: Some(1),
        ..v
    }
}

// ---------------------------------------------------------------------------
// Activation arguments (`declare`, docs/ACTIVATION.md §2).
// ---------------------------------------------------------------------------

/// Always declines at the activation-argument collector. The body has **no
/// decline path** and logs unconditionally, so silence is proof it never ran
/// (and the card must still be in hand).
const DECLINE: CardDef = CardDef::new(
    "TEST:decline",
    &[On::Play("", None, decline_body).declare(decline_declare)],
);

fn decline_declare(_player_id: i32) -> card_sdk::Declared {
    Ok(card_sdk::Activation::Decline)
}

fn decline_body(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("decline_body")));
    Ok(())
}

/// Asks "use this?" at the collector; a yes carries the arg 7 to the body,
/// which logs it. The body has no decline path.
const PICK: CardDef = CardDef::new(
    "TEST:pick",
    &[On::Play("", None, pick_unused).declare_arg(pick_declare, pick_body)],
);

const fn pick_unused(_player_id: i32) -> card_sdk::Asked {
    Ok(())
}

fn pick_declare(player_id: i32) -> card_sdk::Declared {
    if ctx::ask_yes(
        player_id,
        &Msg::new(key!("pick_title")),
        &Msg::new(key!("pick_ask")),
    )? {
        Ok(card_sdk::Activation::Arg(7))
    } else {
        Ok(card_sdk::Activation::Decline)
    }
}

fn pick_body(player_id: i32, arg: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("pick_body")).i("arg", arg as i64));
    Ok(())
}

/// Sibling of [`DECLINE`] on the same timing: a decline on one entry must not
/// stop the next entry. Both fire at `card` (this one is the counter).
const DECLINE_SIB: CardDef = CardDef::new(
    "TEST:decline_sib",
    &[
        On::Play("", None, decline_sib_play),
        On::Counteract(&[ChainKind::Card], "", Some(never), decline_sib_counter),
    ],
);

fn decline_sib_play(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("decline_sib_play")));
    Ok(())
}

fn decline_sib_counter(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("decline_sib_counter")));
    Ok(())
}

/// A card whose `On::Play` gate is closed: `ctx::query_guard(…, ctx::entry::PRESS)` /
/// `ctx::can_fire` must answer false for it.
const GATE_CLOSED: CardDef = CardDef::new(
    "TEST:gate_closed",
    &[On::Play(
        "",
        Some(|_p| Some(Msg::new(key!("gate_closed_reason")))),
        gate_closed_body,
    )],
);

fn gate_closed_body(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("gate_closed_body")));
    Ok(())
}

/// A counteract whose residual guard **would prompt**. Eligibility probes
/// (`can_counteract_scoped`) must refuse with `RuleError::GuardPrompted`
/// rather than asking the player mid-scan (docs/ACTIVATION.md §3).
const ASKING_GUARD: CardDef = CardDef::new(
    "TEST:asking_guard",
    &[
        On::Play("", None, asking_guard_play),
        On::Counteract(
            &[ChainKind::Card],
            "",
            Some(asking_guard),
            asking_guard_counter,
        ),
    ],
);

fn asking_guard_play(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("asking_guard_play")));
    Ok(())
}

fn asking_guard(player_id: i32) -> bool {
    // A residual guard is a pure query; asking from one is a probe error.
    ctx::ask_yes(
        player_id,
        &Msg::new(key!("asking_guard_title")),
        &Msg::new(key!("asking_guard_ask")),
    )
    .unwrap_or(true)
}

fn asking_guard_counter(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("asking_guard_counter")));
    Ok(())
}

/// A [反击] whose activation argument is collected at the **offer** layer
/// (docs/ACTIVATION.md §2): `OP_DECLARE` runs in `finish_offer`, before the
/// declaration commits. Declining there means **no counter is declared at
/// all** -- the card stays in hand and the body never runs. The body has no
/// decline path and logs unconditionally, so silence is proof it never ran.
const PICK_COUNTER: CardDef = CardDef::new(
    "TEST:pick_counter",
    &[
        On::Counteract(&[ChainKind::Card], "", Some(always), pick_counter_unused)
            .declare_arg(pick_counter_declare, pick_counter_body),
    ],
);

fn always(_player_id: i32) -> bool {
    true
}

const fn pick_counter_unused(_player_id: i32) -> card_sdk::Asked {
    Ok(())
}

fn pick_counter_declare(player_id: i32) -> card_sdk::Declared {
    if ctx::ask_yes(
        player_id,
        &Msg::new(key!("pick_c_title")),
        &Msg::new(key!("pick_c_ask")),
    )? {
        Ok(card_sdk::Activation::Arg(7))
    } else {
        Ok(card_sdk::Activation::Decline)
    }
}

fn pick_counter_body(player_id: i32, arg: i32) -> card_sdk::Asked {
    ctx::log(
        player_id,
        &Msg::new(key!("pick_c_body")).i("arg", arg as i64),
    );
    Ok(())
}

/// A [反击] whose collector reads the **trigger it is declaring against**. The
/// collector only accepts when `trigger::kind()` is `Card`; a `Run` bound to
/// `Trigger::default()` (kind `None`) would decline and the body would never
/// run. Pins that a [反击]'s `OP_DECLARE` sees the real trigger
/// (docs/ACTIVATION.md §2) -- `detour` needs it the moment its pick filters on
/// the roll.
const TRIGGER_SEEN: CardDef = CardDef::new(
    "TEST:trigger_seen",
    &[
        On::Counteract(&[ChainKind::Card], "", Some(always), trigger_seen_unused)
            .declare_arg(trigger_seen_declare, trigger_seen_body),
    ],
);

const fn trigger_seen_unused(_player_id: i32) -> card_sdk::Asked {
    Ok(())
}

fn trigger_seen_declare(_player_id: i32) -> card_sdk::Declared {
    if ctx::trigger::kind() == card_sdk::abi::TriggerKind::Card {
        Ok(card_sdk::Activation::Arg(1))
    } else {
        Ok(card_sdk::Activation::Decline)
    }
}

fn trigger_seen_body(player_id: i32, arg: i32) -> card_sdk::Asked {
    // When mortis copies this entry, the link names the source -- the
    // G06 §9.1 copy test reads it back through this log.
    let copied = trigger::link_copied().unwrap_or_default();
    ctx::log(
        player_id,
        &Msg::new(key!("trigger_seen_body"))
            .i("arg", arg as i64)
            .card("copied", copied.as_str()),
    );
    Ok(())
}

/// A placed card whose hook's `OP_DECLARE` collector **traps**. A trapping
/// collector must never commit and must never cost the card
/// (docs/ACTIVATION.md §2 "Trap severity"): the body never runs and the card
/// stays in play. Propagating the trap into the body-trap arm would
/// `DEST_GRAVEYARD` a card whose body never ran. The body has no decline path
/// and logs unconditionally, so silence is proof it never ran.
const TRAP_DECLARE: CardDef = CardDef::new(
    "TEST:trap_declare",
    &[
        On::Play("", None, trap_declare_play),
        On::Action(Action::TurnEnd("card.placed", None, trap_declare_body))
            .declare(trap_declare_collect),
    ],
);

fn trap_declare_play(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "TEST:trap_declare",
        &Msg::new(key!("trap_declare_note")),
    );
    Ok(())
}

/// Deliberately panics into a wasm trap: the collector blows up before the
/// activation commits.
fn trap_declare_collect(_player_id: i32) -> card_sdk::Declared {
    panic!("TEST:trap_declare collector trapped");
}

fn trap_declare_body(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("trap_declare_body")));
    Ok(())
}

// ---------------------------------------------------------------------------
// Rev-2: a card-authored struct arg, and named entry ids for `query_guard`.
// ---------------------------------------------------------------------------

/// The card's own activation argument -- private to this file, postcard on
/// the wire. Two distinct fields so the test can prove more than one `i32`
/// crossed (a plain `Arg(i32)` could not carry both).
#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
struct StructArg {
    pick: i32,
    tile: i32,
    seat: i32,
}

/// A play whose `declare_arg` returns a **struct**. Prompt-free, so the
/// whole interaction is the play and the body logs every field. The body has
/// no decline path.
const STRUCT_ARG: CardDef = CardDef::new(
    "TEST:struct_arg",
    &[On::Play("", None, struct_arg_unused).declare_arg(struct_arg_declare, struct_arg_body)],
);

const fn struct_arg_unused(_player_id: i32) -> card_sdk::Asked {
    Ok(())
}

fn struct_arg_declare(_player_id: i32) -> card_sdk::Declared<StructArg> {
    Ok(card_sdk::Activation::Arg(StructArg {
        pick: 3,
        tile: 7,
        seat: -1,
    }))
}

fn struct_arg_body(player_id: i32, arg: StructArg) -> card_sdk::Asked {
    ctx::log(
        player_id,
        &Msg::new(key!("struct_arg_body"))
            .i("pick", arg.pick as i64)
            .i("tile", arg.tile as i64)
            .i("seat", arg.seat as i64),
    );
    Ok(())
}

/// A hook carrying a **named entry id** whose guard admits. No `condition` -- this
/// fixture is about the *guard* and the id lookup, not about a placement
/// condition (`query_guard` routes through `admits`, so a `card.placed`
/// condition would correctly reject an unplaced card and mask the guard).
const NAMED_OPEN: CardDef = CardDef::new(
    "TEST:named_open",
    &[
        On::Play("", None, named_open_play),
        On::Action(Action::TurnEnd("", Some(always), named_open_tick)).id("decay"),
    ],
);

fn named_open_play(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(player_id, "TEST:named_open", &Msg::new(key!("named_note")));
    Ok(())
}

fn named_open_tick(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("named_open_tick")));
    Ok(())
}

/// A hook carrying the same named id whose guard refuses -- the negative for
/// `query_guard`'s slug lookup.
const NAMED_SHUT: CardDef = CardDef::new(
    "TEST:named_shut",
    &[
        On::Play("", None, named_shut_play),
        On::Action(Action::TurnEnd("", Some(never), named_shut_tick)).id("decay"),
    ],
);

fn named_shut_play(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(player_id, "TEST:named_shut", &Msg::new(key!("named_note")));
    Ok(())
}

fn named_shut_tick(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("named_shut_tick")));
    Ok(())
}

/// A body that asks `query_guard` about the fixtures above and logs every
/// verdict in one line. The host test reads those booleans -- a guest API
/// cannot be probed from the host side without a card calling it.
const QUERY: CardDef = CardDef::new("TEST:query", &[On::Play("", None, query_body)]);

fn query_body(player_id: i32) -> card_sdk::Asked {
    let open = ctx::query_guard(player_id, "TEST:named_open", "decay");
    let shut = ctx::query_guard(player_id, "TEST:named_shut", "decay");
    let wrong = ctx::query_guard(player_id, "TEST:named_open", "nope");
    let unnamed = ctx::query_guard(player_id, "TEST:echo", "decay");
    let press_open = ctx::query_guard(player_id, "TEST:echo", ctx::entry::PRESS);
    let press_shut = ctx::query_guard(player_id, "TEST:gate_closed", ctx::entry::PRESS);
    ctx::log(
        player_id,
        &Msg::new(key!("query_body"))
            .i("open", open as i64)
            .i("shut", shut as i64)
            .i("wrong", wrong as i64)
            .i("unnamed", unnamed as i64)
            .i("press_open", press_open as i64)
            .i("press_shut", press_shut as i64),
    );
    Ok(())
}

// ------------------------------------------------------------- G03 roll ladder

/// G03: logs every roll stage (kind id, face, purpose, source, rerolled,
/// unused face, cast count) and bends the face at Add (+1) and Value (×2), so
/// the stage order and the fold-once contract are visible from the log.
const ROLL_TRACE: CardDef = CardDef::new(
    "TEST:rollTrace",
    &[
        On::Play("", None, roll_trace_play),
        On::Action(Action::DiceRollCast("card.placed", None, roll_trace_log)),
        On::Action(Action::DiceRollBefore("card.placed", None, roll_trace_log)),
        On::Filter(Filter::DiceRollAdd("card.placed", None, roll_trace_add)),
        On::Filter(Filter::DiceRollValue("card.placed", None, roll_trace_value)),
        On::Action(Action::DiceRollAfter("card.placed", None, roll_trace_log)),
        On::Action(Action::DiceRollResult("card.placed", None, roll_trace_log)),
        On::Action(Action::DiceRollSettled("card.placed", None, roll_trace_log)),
    ],
);

fn roll_trace_play(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "TEST:rollTrace",
        &Msg::new(key!("roll_trace_note")),
    );
    Ok(())
}

fn roll_trace_log(player_id: i32) -> card_sdk::Asked {
    let p = trigger::roll_payload().unwrap_or_default();
    ctx::log(
        player_id,
        &Msg::new(key!("roll_trace"))
            .i("kind", trigger::kind().id as i64)
            .i("face", trigger::value() as i64)
            .i("purpose", p.purpose as i64)
            .i("source", p.source as i64)
            .i("rerolled", p.rerolled as i64)
            .i("unused", p.unused_face as i64)
            .i("casts", p.casts.len() as i64),
    );
    Ok(())
}

fn roll_trace_add(_player_id: i32, v: RollValue) -> RollValue {
    RollValue {
        value: v.value + 1,
        ..v
    }
}

fn roll_trace_value(_player_id: i32, v: RollValue) -> RollValue {
    RollValue {
        value: v.value * 2,
        ..v
    }
}

/// G03: at the move's After, recast with a fire source and pick the new face
/// (the Result window must then carry `roll_source == 1`).
const FIRE_RECAST: CardDef = CardDef::new(
    "TEST:fireRecast",
    &[
        On::Play("", None, fire_recast_play),
        On::Action(Action::DiceRollAfter("card.placed", None, fire_recast)),
    ],
);

fn fire_recast_play(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "TEST:fireRecast",
        &Msg::new(key!("fire_recast_note")),
    );
    Ok(())
}

fn fire_recast(player_id: i32) -> card_sdk::Asked {
    let id = trigger::roll_id();
    let _ = ctx::roll_recast(id, roll_source::CHARGE)?;
    ctx::roll_pick(id, 1);
    ctx::log(
        player_id,
        &Msg::new(key!("fire_recast_note"))
            .i("id", id as i64)
            .i("unused", ctx::roll_unused_face(id).unwrap_or(-1) as i64)
            .i("casts", ctx::roll_faces(id).len() as i64),
    );
    Ok(())
}

/// G03: at the move's After, reroll with a fire source and let the player
/// pick (keep old / keep new) -- the keep-point body must see `unused_face`.
const FIRE_REROLL: CardDef = CardDef::new(
    "TEST:fireReroll",
    &[
        On::Play("", None, fire_recast_play),
        On::Action(Action::DiceRollAfter("card.placed", None, fire_reroll)),
    ],
);

fn fire_reroll(player_id: i32) -> card_sdk::Asked {
    let title = Msg::new(key!("fire_recast_note"));
    let old = Msg::new(key!("fire_keep_old"));
    let new = Msg::new(key!("fire_keep_new"));
    let r = ctx::roll_reroll(player_id, &title, &title, &old, &new, roll_source::CHARGE)?;
    ctx::log(
        player_id,
        &Msg::new(key!("fire_recast_note"))
            .i("chosen", r.chosen_index as i64)
            .i("unused", r.unused_face.map(|v| v as i64).unwrap_or(-1)),
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// G05: a clause a player may take at the plan pass
// ---------------------------------------------------------------------------

/// An offer raised **before the dice** at the dual `DiceRollPlan` kind. The
/// offer is the entry's `declare_arg`, so declining runs no body and leaves no
/// flash and no log. Accepting claims the move (`plan::claim`) and pins three
/// steps, which a second copy's guard reads to stand itself down.
const PLAN_OFFER: CardDef = CardDef::new(
    "TEST:planOffer",
    &[
        On::Play("", None, plan_offer_place),
        On::Action(Action::DiceRollPlan(
            "card.placed",
            Some(plan_offer_can),
            plan_offer_unused,
        ))
        .declare_arg(plan_offer_ask, plan_offer_body),
    ],
);

fn plan_offer_place(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "TEST:planOffer",
        &Msg::new(key!("plan_offer_note")),
    );
    Ok(())
}

/// Another offer's claim on this move stands this one down without asking.
fn plan_offer_can(_player_id: i32) -> bool {
    !ctx::plan::is_claimed()
}

/// The constructor's body slot; `.declare_arg` replaces it.
const fn plan_offer_unused(_player_id: i32) -> card_sdk::Asked {
    Ok(())
}

fn plan_offer_ask(player_id: i32) -> card_sdk::Declared {
    if ctx::ask_yes(
        player_id,
        &Msg::new(key!("plan_offer_title")),
        &Msg::new(key!("plan_offer_ask")),
    )? {
        Ok(card_sdk::Activation::Arg(7))
    } else {
        Ok(card_sdk::Activation::Decline)
    }
}

fn plan_offer_body(player_id: i32, arg: i32) -> card_sdk::Asked {
    ctx::plan::claim();
    ctx::plan::set_steps(3);
    ctx::log(
        player_id,
        &Msg::new(key!("plan_offer_body")).i("arg", arg as i64),
    );
    Ok(())
}

/// Declines from the collector with no prompt at all: its body logs, so the
/// presence of `plan_decline_body` is proof the body ran.
const PLAN_DECLINE: CardDef = CardDef::new(
    "TEST:planDecline",
    &[
        On::Play("", None, plan_decline_place),
        On::Action(Action::DiceRollPlan(
            "card.placed",
            None,
            plan_decline_unused,
        ))
        .declare_arg(plan_decline_ask, plan_decline_body),
    ],
);

fn plan_decline_place(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "TEST:planDecline",
        &Msg::new(key!("plan_decline_note")),
    );
    Ok(())
}

const fn plan_decline_unused(_player_id: i32) -> card_sdk::Asked {
    Ok(())
}

const fn plan_decline_ask(_player_id: i32) -> card_sdk::Declared {
    Ok(card_sdk::Activation::Decline)
}

fn plan_decline_body(player_id: i32, _arg: i32) -> card_sdk::Asked {
    ctx::plan::set_steps(5);
    ctx::log(player_id, &Msg::new(key!("plan_decline_body")));
    Ok(())
}

/// An action listing the dual kind, spelled raw (`ActionKind(TriggerKind)`):
/// the typed constructors only accept the action side. Proves the loader
/// admits it on an `On::Action` entry.
const DUAL_ACTION: CardDef = CardDef::new(
    "TEST:dualAction",
    &[On::Action(card_sdk::kinds::action(
        ActionKind(TriggerKind::DiceRollPlan),
        "",
        None,
        dual_action_body,
    ))],
);

const fn dual_action_body(_player_id: i32) -> card_sdk::Asked {
    Ok(())
}

// ---------------------------------------------------------------------------
// Inspection / draw points (G11)
// ---------------------------------------------------------------------------

/// Logs when it is turned up in a look. `placed` separates the deck instance
/// from a field copy: an `inspected` raise goes to the turned card itself, so
/// a field copy placed alongside must stay silent.
const INSPECT_PROBE: CardDef = CardDef::new(
    "TEST:inspectProbe",
    &[On::Action(Action::Inspected("", None, inspect_probe))],
);

fn inspect_probe(player_id: i32) -> card_sdk::Asked {
    ctx::log(
        player_id,
        &Msg::new(key!("inspect_probe"))
            .i("position", ctx::trigger::value() as i64)
            .i("placed", ctx::is_placed() as i64)
            .i("by", ctx::trigger::by_card().unwrap_or(-1) as i64),
    );
    Ok(())
}

/// Turns up the top two cards privately and logs how many it held **after**
/// the begin returned -- the line proves the `inspected` raises ran inside the
/// call.
const LOOK_AT_TWO: CardDef = CardDef::new("TEST:lookAtTwo", &[On::Play("", None, look_at_two)]);

fn look_at_two(player_id: i32) -> card_sdk::Asked {
    let held = ctx::inspection_begin(player_id, player_id, 2, false)?;
    ctx::log(
        player_id,
        &Msg::new(key!("look_returned")).i("held", held as i64),
    );
    ctx::inspection_end(player_id, false);
    Ok(())
}

/// Opens a **public** look, then publishes the deck while the frame is still
/// open, and halts: the published list already names the held card, so the
/// view must show it once, not twice.
const LOOK_REVEAL: CardDef = CardDef::new("TEST:lookReveal", &[On::Play("", None, look_reveal)]);

fn look_reveal(player_id: i32) -> card_sdk::Asked {
    ctx::inspection_begin(player_id, player_id, 1, true)?;
    ctx::reveal_pile(player_id, CardPile::Deck, &Msg::new(key!("look_reveal")));
    // Keep the frame open past the commit net's reach.
    let _ = ctx::ask_yes(
        player_id,
        &Msg::new(key!("look_reveal")),
        &Msg::new(key!("look_reveal")),
    )?;
    Ok(())
}

/// Opens a **public** look, publishes the deck, closes the look and only then
/// keeps the top card into the hand, then halts: the published list must follow
/// the pile (the kept card is no longer a deck entry), not stay frozen at the
/// reveal.
const LOOK_KEEP: CardDef = CardDef::new("TEST:lookKeep", &[On::Play("", None, look_keep)]);

fn look_keep(player_id: i32) -> card_sdk::Asked {
    ctx::inspection_begin(player_id, player_id, 2, true)?;
    ctx::reveal_pile(player_id, CardPile::Deck, &Msg::new(key!("look_keep")));
    ctx::inspection_end(player_id, false);
    let top = ctx::cards_in(player_id, CardPile::Deck);
    if let Some(card) = top.first() {
        let id = card.as_str().to_string();
        if ctx::take_card(player_id, &id, CardPile::Deck) {
            ctx::add_to_dest(player_id, &id, ctx::Dest::Hand, false);
        }
    }
    // Two halts: the first shows the begin (the only part applied live), the
    // second the committed end + keep.
    let _ = ctx::ask_yes(
        player_id,
        &Msg::new(key!("look_keep")),
        &Msg::new(key!("look_keep")),
    )?;
    let _ = ctx::ask_yes(
        player_id,
        &Msg::new(key!("look_keep")),
        &Msg::new(key!("look_keep")),
    )?;
    Ok(())
}

/// Opens a private look and returns **without** ending it: the commit net must
/// put the cards back (design G11 §5.4).
const LOOK_LEAK: CardDef = CardDef::new("TEST:lookLeak", &[On::Play("", None, look_leak)]);

fn look_leak(player_id: i32) -> card_sdk::Asked {
    ctx::inspection_begin(player_id, player_id, 2, false)?;
    Ok(())
}

/// Draws two cards in **one** draw instruction.
const DRAW_TWO: CardDef = CardDef::new("TEST:drawTwo", &[On::Play("", None, draw_two)]);

fn draw_two(player_id: i32) -> card_sdk::Asked {
    ctx::draw(player_id, 2)?;
    Ok(())
}

/// A field probe for both draw points: `drew` logs per card, `drawAfter` logs
/// once with the batch length and its two first ids.
const DRAW_AFTER_PROBE: CardDef = CardDef::new(
    "TEST:drawAfterProbe",
    &[
        On::Action(Action::DrawAfter("", None, draw_after_probe)),
        On::Action(Action::Drew("", None, drew_probe)),
    ],
);

fn draw_after_probe(player_id: i32) -> card_sdk::Asked {
    let cards = ctx::trigger::cards();
    ctx::log(
        player_id,
        &Msg::new(key!("draw_after_probe"))
            .i("value", ctx::trigger::value() as i64)
            .card("first", cards.first().map(|c| c.as_str()).unwrap_or(""))
            .card("second", cards.get(1).map(|c| c.as_str()).unwrap_or("")),
    );
    Ok(())
}

fn drew_probe(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("drew_probe")));
    Ok(())
}

// ---------------------------------------------------------------------------
// Invocation (G05 §6.1 adopts G13's surface)
// ---------------------------------------------------------------------------

/// The invoked target: a `PRESS` Play and a named `"two"` action entry, both
/// logging what the **invocation frame** says -- `by_instance` is the caller's
/// instance, which is G05's only addition to G13's frame.
const INVOKED: CardDef = CardDef::new(
    "TEST:invoked",
    &[
        On::Play("", None, invoked_body),
        On::Action(Action::TurnEnd("", None, invoked_body)).id("two"),
    ],
);

fn invoked_body(player_id: i32) -> card_sdk::Asked {
    let by = ctx::invoke().map_or(-1, |f| f.by_instance);
    let entry = ctx::invoke().map_or_else(String::new, |f| f.entry.as_str().into());
    ctx::log(
        player_id,
        &Msg::new(key!("invoked_frame"))
            .i("by", by as i64)
            .i("self", ctx::self_uid() as i64)
            .i("instance", ctx::invoke().map_or(-1, |f| f.instance) as i64)
            .card("card", &trigger_card())
            .card("entry", &entry)
            .i("paid", ctx::invoke_paid() as i64),
    );
    Ok(())
}

/// Invokes `TEST:invoked`'s `"two"` while it is itself placed, then again from a
/// hand play -- the caller binding must be its instance first and `-1` after.
const INVOKER: CardDef = CardDef::new(
    "TEST:invoker",
    &[
        On::Play("", None, invoker_play),
        On::Action(Action::TurnEnd("", None, invoker_body)),
    ],
);

fn invoker_play(player_id: i32) -> card_sdk::Asked {
    // Before the placement the run has no instance: `by_instance` is `-1`
    // (G13 §4.1, the hand-play case).
    ctx::invoke_entry(player_id, "TEST:invoked", "two")?;
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(player_id, "TEST:invoker", &Msg::new(key!("invoker_note")));
    Ok(())
}

fn invoker_body(player_id: i32) -> card_sdk::Asked {
    // A placed card's hook: `by_instance` is this instance.
    ctx::invoke_entry(player_id, "TEST:invoked", "two")?;
    Ok(())
}

/// A raw **house write** (+1) on the caller's current tile -- raises the
/// `effect` window over the tile (`houseAdd`, G09 5.4).
const HOUSE_ADD: CardDef = CardDef::new("TEST:house_add", &[On::Play("", None, house_add)]);

fn house_add(player_id: i32) -> card_sdk::Asked {
    let t = ctx::tile_at(ctx::player_pos(player_id));
    if t < 0 {
        return Ok(());
    }
    let n = ctx::add_house(t, 1);
    ctx::log(
        player_id,
        &Msg::new(key!("house_wrote")).tile("tile", t).i("n", n as i64),
    );
    Ok(())
}

/// A raw **house write** (-1) -- raises `houseRemove` and, on a fall,
/// `houseRemoved` (G09 5.4/5.5).
const HOUSE_REMOVE: CardDef =
    CardDef::new("TEST:house_remove", &[On::Play("", None, house_remove)]);

fn house_remove(player_id: i32) -> card_sdk::Asked {
    let t = ctx::tile_at(ctx::player_pos(player_id));
    if t < 0 {
        return Ok(());
    }
    let n = ctx::add_house(t, -1);
    ctx::log(
        player_id,
        &Msg::new(key!("house_wrote")).tile("tile", t).i("n", n as i64),
    );
    Ok(())
}

/// A raw **owner write** on the caller's current tile (owner -> seat 1).
const OWNER_SET: CardDef = CardDef::new("TEST:owner_set", &[On::Play("", None, owner_set)]);

fn owner_set(player_id: i32) -> card_sdk::Asked {
    let t = ctx::tile_at(ctx::player_pos(player_id));
    if t < 0 {
        return Ok(());
    }
    ctx::set_owner(t, 1);
    ctx::log(player_id, &Msg::new(key!("owner_wrote")).tile("tile", t));
    Ok(())
}

/// `card_build_as` with a `Free` charge and no owner test.
const BUILD_FREE: CardDef = CardDef::new("TEST:build_free", &[On::Play("", None, build_free)]);

fn build_free(player_id: i32) -> card_sdk::Asked {
    let t = ctx::tile_at(ctx::player_pos(player_id));
    if t < 0 {
        return Ok(());
    }
    let ok = ctx::card_build_as(player_id, t, BuildCharge::Free, 0, 0, 0, false);
    ctx::log(
        player_id,
        &Msg::new(key!("build_tried")).tile("tile", t).i("ok", ok as i64),
    );
    Ok(())
}

/// `card_build_as` with a `Fixed` charge of 100 and the owner test.
const BUILD_FIXED: CardDef = CardDef::new("TEST:build_fixed", &[On::Play("", None, build_fixed)]);

fn build_fixed(player_id: i32) -> card_sdk::Asked {
    let t = ctx::tile_at(ctx::player_pos(player_id));
    if t < 0 {
        return Ok(());
    }
    let ok = ctx::card_build_as(player_id, t, BuildCharge::Fixed, 100, 0, 0, true);
    ctx::log(
        player_id,
        &Msg::new(key!("build_tried")).tile("tile", t).i("ok", ok as i64),
    );
    Ok(())
}

/// `card_build_as` with a `Scaled` charge of 1/2 of the effective price.
const BUILD_SCALED: CardDef =
    CardDef::new("TEST:build_scaled", &[On::Play("", None, build_scaled)]);

fn build_scaled(player_id: i32) -> card_sdk::Asked {
    let t = ctx::tile_at(ctx::player_pos(player_id));
    if t < 0 {
        return Ok(());
    }
    let ok = ctx::card_build_as(player_id, t, BuildCharge::Scaled, 0, 1, 2, true);
    ctx::log(
        player_id,
        &Msg::new(key!("build_tried")).tile("tile", t).i("ok", ok as i64),
    );
    Ok(())
}

/// Build-hook order probe: places itself, then logs at `buildBefore` /
/// `houseAdded` / `buildAfter` so a test can pin the order.
const BUILD_PROBE: CardDef = CardDef::new(
    "TEST:build_probe",
    &[
        On::Play("", None, build_probe_play),
        On::Action(Action::BuildBefore("card.placed", None, build_probe_before)),
        On::Action(Action::HouseAdded("card.placed", None, build_probe_added)),
        On::Action(Action::BuildAfter("card.placed", None, build_probe_after)),
    ],
);

fn build_probe_play(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(player_id, "TEST:build_probe", &Msg::new(key!("build_probe_note")));
    Ok(())
}

fn build_probe_before(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("probe_build_before")));
    Ok(())
}

fn build_probe_added(player_id: i32) -> card_sdk::Asked {
    ctx::log(
        player_id,
        &Msg::new(key!("probe_house_added"))
            .tile("tile", trigger::tile())
            .i("value", trigger::value() as i64),
    );
    Ok(())
}

fn build_probe_after(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("probe_build_after")));
    Ok(())
}

/// Logs every `houseRemoved` (`tile` / `value`).
const HOUSE_REMOVED_PROBE: CardDef = CardDef::new(
    "TEST:house_removed_probe",
    &[
        On::Play("", None, house_removed_probe_play),
        On::Action(Action::HouseRemoved("card.placed", None, house_removed_probe)),
    ],
);

fn house_removed_probe_play(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        player_id,
        "TEST:house_removed_probe",
        &Msg::new(key!("house_removed_probe_note")),
    );
    Ok(())
}

fn house_removed_probe(player_id: i32) -> card_sdk::Asked {
    ctx::log(
        player_id,
        &Msg::new(key!("house_removed_seen"))
            .tile("tile", trigger::tile())
            .i("value", trigger::value() as i64),
    );
    Ok(())
}

/// A [反击] on the tile-effect window that arms `SUPPRESS_TILE_EFFECTS` for
/// the governing tile (the marina gesture).
const TILE_EFFECT_COUNTER: CardDef = CardDef::new(
    "TEST:tile_effect_counter",
    &[On::Counteract(
        &[ChainKind::TileEffectBefore],
        "",
        None,
        tile_effect_counter,
    )],
);

fn tile_effect_counter(player_id: i32) -> card_sdk::Asked {
    let t = trigger::tile();
    ctx::plan::set_tag(card_sdk::abi::tag::SUPPRESS_TILE_EFFECTS, t);
    ctx::log(
        player_id,
        &Msg::new(key!("tile_effect_suppressed")).tile("tile", t),
    );
    Ok(())
}

/// Places itself on the caller's current tile, and counteracts a
/// `tileEffectBefore` window to bind its own slot to 弦卷集团 -- the swap lands
/// *inside* the window, after the settle frame has already pinned its tile, so
/// the tile now stands where this counter sits.
const TILE_SWAP_COUNTER: CardDef = CardDef::new(
    "TEST:tile_swap_counter",
    &[
        On::Play("", None, tile_swap_counter_play),
        On::Counteract(&[ChainKind::TileEffectBefore], "", None, tile_swap_counter),
    ],
)
.props(&[(card_sdk::abi::prop::COUNTERACT_FROM_FIELD, 1)]);

fn tile_swap_counter_play(player_id: i32) -> card_sdk::Asked {
    let here = ctx::tile_at(ctx::player_pos(player_id));
    if here < 0 {
        return Ok(());
    }
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card_on(
        player_id,
        here,
        "TEST:tile_swap_counter",
        &Msg::new(key!("tile_swap_counter_note")),
    );
    Ok(())
}

fn tile_swap_counter(player_id: i32) -> card_sdk::Asked {
    let group = ctx::tile_named("弦卷集团");
    if group < 0 {
        return Ok(());
    }
    ctx::set_swapped_tile(group);
    ctx::log(
        player_id,
        &Msg::new(key!("tile_swap_counter_done")).tile("to", group),
    );
    Ok(())
}

/// Settles the caller's current tile without moving (`settle_tile_at`). Unlike
/// a roll landing there is no walk `tileEffectBefore` window before the settle
/// frame, so a counter answering the window answers the frame's own.
const SETTLE_HERE: CardDef = CardDef::new("TEST:settle_here", &[On::Play("", None, settle_here)]);

fn settle_here(player_id: i32) -> card_sdk::Asked {
    let here = ctx::tile_at(ctx::player_pos(player_id));
    if here < 0 {
        return Ok(());
    }
    ctx::settle_tile_at(player_id, here, false);
    Ok(())
}

/// Places itself on the caller's current tile and binds the swap to
/// 弦卷集团 (`set_swapped_tile`).
const SWAP_PROBE: CardDef = CardDef::new("TEST:swap_probe", &[On::Play("", None, swap_probe)]);

fn swap_probe(player_id: i32) -> card_sdk::Asked {
    let here = ctx::tile_at(ctx::player_pos(player_id));
    let group = ctx::tile_named("弦卷集团");
    if here < 0 || group < 0 {
        return Ok(());
    }
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card_on(player_id, here, "TEST:swap_probe", &Msg::new(key!("swap_probe_note")));
    ctx::set_swapped_tile(group);
    Ok(())
}

/// Logs the settle's governing tile and physical slot (dispatch check).
const TILE_ID_PROBE: CardDef = CardDef::new(
    "TEST:tile_id_probe",
    &[
        On::Play("", None, tile_id_probe_play),
        On::Action(Action::Settle("", None, tile_id_probe)),
    ],
);

fn tile_id_probe_play(player_id: i32) -> card_sdk::Asked {
    let here = ctx::tile_at(ctx::player_pos(player_id));
    if here < 0 {
        return Ok(());
    }
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card_on(player_id, here, "TEST:tile_id_probe", &Msg::new(key!("tile_id_probe_note")));
    Ok(())
}

fn tile_id_probe(player_id: i32) -> card_sdk::Asked {
    ctx::log(
        player_id,
        &Msg::new(key!("tile_id_seen"))
            .tile("tile", trigger::tile())
            .i("position", trigger::position() as i64),
    );
    Ok(())
}

/// Logs the `Paid` settle's move path (length + every governed tile).
const PATH_PROBE: CardDef = CardDef::new(
    "TEST:path_probe",
    &[
        On::Play("", None, path_probe_play),
        On::Action(Action::Paid("card.placed", None, path_probe)),
    ],
);

fn path_probe_play(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(player_id, "TEST:path_probe", &Msg::new(key!("path_probe_note")));
    Ok(())
}

fn path_probe(player_id: i32) -> card_sdk::Asked {
    let n = trigger::move_path_length();
    let mut m = Msg::new(key!("path_seen")).i("n", n as i64);
    for i in 0..n {
        m = m.tile("tile", trigger::move_path(i));
    }
    ctx::log(player_id, &m);
    Ok(())
}

/// A raw **house set** to 1 on the caller's current tile (`houseSet`, G09 5.4).
const HOUSE_SET: CardDef = CardDef::new("TEST:house_set", &[On::Play("", None, house_set)]);

fn house_set(player_id: i32) -> card_sdk::Asked {
    let t = ctx::tile_at(ctx::player_pos(player_id));
    if t < 0 {
        return Ok(());
    }
    ctx::set_houses(t, 1);
    ctx::log(player_id, &Msg::new(key!("house_wrote")).tile("tile", t));
    Ok(())
}

/// Vetoes any build at `buildBefore` (`set_cancelled`).
const BUILD_VETO: CardDef = CardDef::new(
    "TEST:build_veto",
    &[
        On::Play("", None, build_veto_play),
        On::Action(Action::BuildBefore("card.placed", None, build_veto)),
    ],
);

fn build_veto_play(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(player_id, "TEST:build_veto", &Msg::new(key!("build_veto_note")));
    Ok(())
}

fn build_veto(_player_id: i32) -> card_sdk::Asked {
    ctx::trigger::set_cancelled();
    Ok(())
}

/// Charges `who` a full rent on the caller's current tile without a settle --
/// raises `Paid`, so a path probe can show a move-less rent has length 0.
const RENT_CHARGE: CardDef =
    CardDef::new("TEST:rent_charge", &[On::Play("", None, rent_charge)]);

fn rent_charge(player_id: i32) -> card_sdk::Asked {
    let t = ctx::tile_at(ctx::player_pos(player_id));
    if t < 0 {
        return Ok(());
    }
    // Charge the *other* seat -- a self-charge is not the shape a rent probe
    // wants to see.
    ctx::charge_rent(1 - player_id, t, 1_000)?;
    Ok(())
}

/// G13 §10.2: the target instance is bound as `self_uid` inside the body and
/// the caller's instance is back afterwards.
const INVOKE_BIND: CardDef = CardDef::new(
    "TEST:invoke_bind",
    &[On::Action(Action::TurnEnd("", None, invoke_bind))],
);

fn invoke_bind(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("bind_before")).i("self", ctx::self_uid() as i64));
    ctx::invoke_entry(player_id, "TEST:invoked", "two")?;
    ctx::log(player_id, &Msg::new(key!("bind_after")).i("self", ctx::self_uid() as i64));
    Ok(())
}

/// G13 §10.2: `trigger.card` names the target inside the body and is restored
/// to the caller after.
const INVOKE_SEED: CardDef = CardDef::new(
    "TEST:invoke_seed",
    &[On::Action(Action::CardPlayed("", None, invoke_seed))],
);

/// The trigger's card (`trigger.cards`): the invoked card inside the invoke
/// frame, the played card in the caller's `cardPlayed` hook.
fn trigger_card() -> String {
    ctx::trigger::cards()
        .into_iter()
        .next()
        .map_or_else(String::new, |c| c.as_str().into())
}

fn invoke_seed(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new(key!("seed_before")).card("card", &trigger_card()));
    ctx::invoke_entry(player_id, "TEST:invoked", "two")?;
    ctx::log(player_id, &Msg::new(key!("seed_after")).card("card", &trigger_card()));
    Ok(())
}

/// G13 §10.2: an unknown entry is a no-op (`DEST_UNSET`), not a trap.
const INVOKE_BAD_ENTRY: CardDef = CardDef::new(
    "TEST:invoke_bad_entry",
    &[On::Action(Action::TurnEnd("", None, invoke_bad_entry))],
);

fn invoke_bad_entry(player_id: i32) -> card_sdk::Asked {
    let r = ctx::invoke_entry(player_id, "TEST:invoked", "nope");
    ctx::log(player_id, &Msg::new(key!("bad_entry")).i("ok", r.is_ok() as i64));
    Ok(())
}

/// G13 §10.2: an unknown card traps.
const INVOKE_BAD_CARD: CardDef = CardDef::new(
    "TEST:invoke_bad_card",
    &[On::Action(Action::TurnEnd("", None, invoke_bad_card))],
);

fn invoke_bad_card(player_id: i32) -> card_sdk::Asked {
    ctx::invoke_entry(player_id, "TEST:nope", "two")?;
    Ok(())
}

/// G13 §10.2: a `Filter` entry is not invocable (its body shape differs).
const INVOKE_FILTER: CardDef = CardDef::new(
    "TEST:invoke_filter",
    &[
        On::Filter(Filter::PaySplitMul("", None, identity)).id("f"),
        On::Action(Action::TurnEnd("", None, invoke_filter)),
    ],
);

fn identity(_player_id: i32, value: i32) -> i32 {
    value
}

fn invoke_filter(player_id: i32) -> card_sdk::Asked {
    ctx::invoke_entry(player_id, "TEST:invoke_filter", "f")?;
    Ok(())
}

/// G13 §10.2: `invoke_skill` resolves `entry::PRESS`.
const INVOKE_SKILL_PROBE: CardDef = CardDef::new(
    "TEST:invoke_skill",
    &[On::Action(Action::TurnEnd("", None, invoke_skill_probe))],
);

fn invoke_skill_probe(player_id: i32) -> card_sdk::Asked {
    ctx::invoke_skill(player_id, "TEST:invoked")?;
    Ok(())
}

/// G13 §10.2: `play_card` also lands on the first Play, not on a named entry.
const PLAY_CARD_PROBE: CardDef = CardDef::new(
    "TEST:play_card_probe",
    &[On::Action(Action::TurnEnd("", None, play_card_probe))],
);

fn play_card_probe(player_id: i32) -> card_sdk::Asked {
    ctx::play_card("TEST:invoked", player_id)?;
    Ok(())
}

/// Zone-family hook probe: every zone kind logs the payload so a test can
/// read the pair, the cause and the two bits. Its play places itself and
/// records the instance for `TEST:drop`; `TEST.zoner.dest` (a `Dest` wire
/// code + 1) makes its `zoneLeft` rewrite the destination.
const ZONER: CardDef = CardDef::new(
    "TEST:zoner",
    &[
        On::Play("", None, zoner_play),
        On::Action(Action::ZoneLeft("", None, zoner_left)),
        On::Action(Action::ZoneLeftAfter("", None, zoner_left_after)),
        On::Action(Action::CardFaceFlipped("", None, zoner_flipped)),
        On::Action(Action::CardFaceFlippedAfter("", None, zoner_flipped_after)),
        On::Action(Action::HandChanged("", None, zoner_hand_changed)),
        On::Action(Action::CardAttached("", None, zoner_attached)),
        On::Action(Action::CardDetached("", None, zoner_detached)),
        On::Action(Action::Discarded("", None, zoner_discarded)),
        On::Action(Action::OpeningHandDealt("", None, zoner_dealt)),
        On::Action(Action::CardAfter("", None, zoner_outcome)),
        On::Action(Action::CardPlayed("", None, zoner_outcome)),
    ],
);

fn zoner_play(player_id: i32) -> card_sdk::Asked {
    let me = ctx::place_self_to_field(
        player_id,
        "TEST:zoner",
        &Msg::new(key!("zoner_play")),
        card_sdk::abi::ZoneChangeCause::Play);
    ctx::state::set(player_id, "TEST.drop.uid", me);
    Ok(())
}

fn zoner_left(player_id: i32) -> card_sdk::Asked {
    zoner_log("zoner_left", player_id);
    let dest = ctx::state::get(player_id, "TEST.zoner.dest");
    if dest > 0 {
        bandori_std::zones::set_leave_destination(ctx::Dest::from_i32(dest));
    }
    Ok(())
}

fn zoner_left_after(player_id: i32) -> card_sdk::Asked {
    zoner_log("zoner_left_after", player_id);
    Ok(())
}

fn zoner_flipped(player_id: i32) -> card_sdk::Asked {
    zoner_log("zoner_flipped", player_id);
    Ok(())
}

fn zoner_flipped_after(player_id: i32) -> card_sdk::Asked {
    zoner_log("zoner_flipped_after", player_id);
    Ok(())
}

fn zoner_hand_changed(player_id: i32) -> card_sdk::Asked {
    zoner_log("zoner_hand_changed", player_id);
    Ok(())
}

fn zoner_attached(player_id: i32) -> card_sdk::Asked {
    zoner_log("zoner_attached", player_id);
    Ok(())
}

fn zoner_detached(player_id: i32) -> card_sdk::Asked {
    zoner_log("zoner_detached", player_id);
    Ok(())
}

fn zoner_discarded(player_id: i32) -> card_sdk::Asked {
    zoner_log("zoner_discarded", player_id);
    Ok(())
}

fn zoner_outcome(player_id: i32) -> card_sdk::Asked {
    let outcome = trigger::play_outcome().map_or(-1, |o| o.as_i32());
    ctx::log(player_id, &Msg::new("zoner_outcome").i("outcome", outcome as i64));
    Ok(())
}

fn zoner_dealt(player_id: i32) -> card_sdk::Asked {
    zoner_log("zoner_dealt", player_id);
    Ok(())
}

/// One log line per zone raise: the hook name, the instance, the pair, the
/// cause and the two bits.
fn zoner_log(what: &str, player_id: i32) {
    let zone = trigger::zone().unwrap_or_default();
    ctx::log(
        player_id,
        &Msg::new(what)
            .i("uid", trigger::instance() as i64)
            .i("from", zone.from.as_i32() as i64)
            .i("to", zone.to.as_i32() as i64)
            .i("cause", zone.cause.as_i32() as i64)
            .i("face", trigger::value() as i64)
            .i("wasted", zone.wasted as i64)
            .i("counted", zone.counted as i64),
    );
}

/// Places itself, records the instance and attaches the first hand card to
/// itself face-down. It hears `zoneLeft` too, so a cascade's order shows.
const HOLDER: CardDef = CardDef::new(
    "TEST:holder",
    &[
        On::Play("", None, holder_play),
        On::Action(Action::ZoneLeft("", None, zoner_left)),
    ],
);

fn holder_play(player_id: i32) -> card_sdk::Asked {
    let me = ctx::place_self_to_field(
        player_id,
        "TEST:holder",
        &Msg::new(key!("holder_play")),
        card_sdk::abi::ZoneChangeCause::Play);
    ctx::state::set(player_id, "TEST.drop.uid", me);
    let held = ctx::cards_in(player_id, CardPile::Hand);
    if let Some(card) = held.first() {
        ctx::attach_card(
            me,
            card.as_str(),
            card_sdk::abi::ZoneChangeCause::Effect,
            true,
        );
    }
    Ok(())
}

/// Places itself, then makes a child of itself (`created_by`); the child's own
/// body runs, up to depth 2, so a grandchild recurses. It hears `zoneLeft` so
/// the creator cascade's order and cause show.
const CREATOR: CardDef = CardDef::new(
    "TEST:creator",
    &[
        On::Play("", None, creator_play),
        On::Action(Action::ZoneLeft("", None, zoner_left)),
    ],
);

fn creator_play(player_id: i32) -> card_sdk::Asked {
    let me = ctx::place_self_to_field(
        player_id,
        "TEST:creator",
        &Msg::new(key!("creator_play")),
        card_sdk::abi::ZoneChangeCause::Play);
    // A nested `play_card` of the same card is this card creating a child: the
    // caller left its own instance in `TEST.creator.maker`.
    let maker = ctx::state::get(player_id, "TEST.creator.maker");
    if maker > 0 {
        ctx::set_created_by_at(me, maker);
    }
    let depth = ctx::state::get(player_id, "TEST.creator.depth");
    if depth == 0 {
        ctx::state::set(player_id, "TEST.drop.uid", me);
    }
    ctx::state::set(player_id, "TEST.creator.depth", depth + 1);
    if depth < 2 {
        ctx::state::set(player_id, "TEST.creator.maker", me);
        ctx::play_card("TEST:creator", player_id)?;
    }
    Ok(())
}

/// Takes the instance recorded at `TEST.drop.uid` off the field to the
/// destination code at `TEST.drop.dest` (unset/`0` = the Graveyard), cause
/// `Effect`.
const DROP: CardDef = CardDef::new("TEST:drop", &[On::Play("", None, drop_play)]);

fn drop_play(player_id: i32) -> card_sdk::Asked {
    let uid = ctx::state::get(player_id, "TEST.drop.uid");
    let dest = ctx::state::get(player_id, "TEST.drop.dest");
    ctx::unplace_at(
        uid,
        ctx::Dest::from_i32(dest),
        card_sdk::abi::ZoneChangeCause::Effect,
    );
    Ok(())
}

/// Declares a target (so an effect window opens) and logs its own play
/// outcome from `cardAfter` / `cardPlayed`: a negated play still announces
/// itself, `Cancelled`.
const OUTCOMER: CardDef = CardDef::new("TEST:outcomer", &[On::Play("", None, outcomer_play)]);

fn outcomer_play(player_id: i32) -> card_sdk::Asked {
    if let Some(&target) = ctx::others(player_id).first() {
        ctx::target(target);
    }
    Ok(())
}

/// A placed bystander that logs every `cardPlayed` outcome it hears.
const OBSERVER: CardDef = CardDef::new(
    "TEST:observer",
    &[On::Action(Action::CardPlayed("", None, observer_outcome))],
);

fn observer_outcome(player_id: i32) -> card_sdk::Asked {
    let outcome = trigger::play_outcome().map_or(-1, |o| o.as_i32());
    ctx::log(
        player_id,
        &Msg::new("observer_outcome").i("outcome", outcome as i64),
    );
    Ok(())
}

/// Marks itself never-effective, then leaves to the Graveyard: the `discarded`
/// raise must carry `wasted`.
const WASTER: CardDef = CardDef::new(
    "TEST:waster",
    &[
        On::Play("", None, waster_play),
        On::Action(Action::Discarded("", None, zoner_discarded)),
    ],
);

fn waster_play(player_id: i32) -> card_sdk::Asked {
    let me = ctx::place_self_to_field(
        player_id,
        "TEST:waster",
        &Msg::new(key!("waster_play")),
        card_sdk::abi::ZoneChangeCause::Play);
    ctx::state::set(player_id, "TEST.waster.uid", me);
    ctx::set_ineffective_at(me, true);
    ctx::unplace_at(
        me,
        ctx::Dest::Graveyard,
        card_sdk::abi::ZoneChangeCause::Effect,
    );
    Ok(())
}

/// Sets the waste bit on its own **hand play** (`self_uid()` is `-1` there),
/// reads it back and leaves: the bit must be the play's, not a row's, and the
/// `discarded` raise must carry it. Probes the setter/getter pair.
const HAND_WASTER: CardDef =
    CardDef::new("TEST:handwaster", &[On::Play("", None, hand_waster_play)]);

fn hand_waster_play(player_id: i32) -> card_sdk::Asked {
    ctx::set_ineffective_at(-1, true);
    ctx::log(
        player_id,
        &Msg::new("hand_wasted_read").i("read", ctx::is_ineffective_at(-1) as i64),
    );
    ctx::set_dest(
        ctx::Dest::Graveyard,
        card_sdk::abi::ZoneChangeCause::Effect,
    );
    Ok(())
}

/// Inert filler for the cycle probe.
const CYC: CardDef = CardDef::new("TEST:cyc", &[]);

/// Places itself and a `TEST:cyc`, attaches the cyc (row re-parent), then
/// attaches itself back onto the cyc -- a cycle, which must refuse. The two
/// answers are logged.
const CYCLER: CardDef = CardDef::new("TEST:cycler", &[On::Play("", None, cycler_play)]);

fn cycler_play(player_id: i32) -> card_sdk::Asked {
    let me = ctx::place_self_to_field(
        player_id,
        "TEST:cycler",
        &Msg::new(key!("cycler_play")),
        card_sdk::abi::ZoneChangeCause::Play);
    let other = ctx::place_card_at(player_id, "TEST:cyc", &Msg::new(key!("cyc_place")));
    let first = ctx::attach_card(
        me,
        "TEST:cyc",
        card_sdk::abi::ZoneChangeCause::Effect,
        false,
    );
    let second = ctx::attach_card(
        other,
        "TEST:cycler",
        card_sdk::abi::ZoneChangeCause::Effect,
        false,
    );
    ctx::log(
        player_id,
        &Msg::new("cycler_attach")
            .i("first", first as i64)
            .i("second", second as i64),
    );
    Ok(())
}

/// A board-owned rule: places itself on tile 0 at the neutral board owner,
/// records the instance and attaches the first hand card face-down.
const BOARDER: CardDef = CardDef::new("TEST:brd", &[On::Play("", None, brd_play)]);

fn brd_play(player_id: i32) -> card_sdk::Asked {
    // `-1` is the neutral board owner (the rule sits on the tile, not a seat).
    let me = ctx::place_card_on(-1, 0, "TEST:brd", &Msg::new(key!("brd_place")));
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Effect);
    ctx::state::set(player_id, "TEST.drop.uid", me);
    let held = ctx::cards_in(player_id, CardPile::Hand);
    if let Some(card) = held.first() {
        ctx::attach_card(
            me,
            card.as_str(),
            card_sdk::abi::ZoneChangeCause::Effect,
            true,
        );
    }
    Ok(())
}

/// Leaves the recorded board rule to the Graveyard.
const BOARD_OFF: CardDef = CardDef::new("TEST:brdOff", &[On::Play("", None, drop_play)]);

/// Places itself and flips down then up: its own flip hooks must hear it
/// while it is face-down.
const FLIPPER: CardDef = CardDef::new(
    "TEST:flipper",
    &[
        On::Play("", None, flipper_play),
        On::Action(Action::CardFaceFlipped("", None, zoner_flipped)),
        On::Action(Action::CardFaceFlippedAfter("", None, zoner_flipped_after)),
    ],
);

fn flipper_play(player_id: i32) -> card_sdk::Asked {
    let me = ctx::place_self_to_field(
        player_id,
        "TEST:flipper",
        &Msg::new(key!("flipper_play")),
        card_sdk::abi::ZoneChangeCause::Play);
    ctx::set_face_down_at(me, true, card_sdk::abi::ZoneChangeCause::Effect);
    ctx::set_face_down_at(me, false, card_sdk::abi::ZoneChangeCause::Effect);
    Ok(())
}

/// Rehomes tile 0 to player 1 (`ctx::rehome_card`).
const REHOMER: CardDef = CardDef::new("TEST:rehomer", &[On::Play("", None, rehomer_play)]);

fn rehomer_play(player_id: i32) -> card_sdk::Asked {
    let other = if player_id == 0 { 1 } else { 0 };
    ctx::rehome_card(0, other);
    Ok(())
}

/// Logs the player's earliest-owned tile (`ctx::earliest_owned_tile`).
const TILE_PROBE: CardDef = CardDef::new("TEST:tileProbe", &[On::Play("", None, tile_probe_play)]);

fn tile_probe_play(player_id: i32) -> card_sdk::Asked {
    let tile = ctx::earliest_owned_tile(player_id);
    ctx::log(player_id, &Msg::new("tile_probe").i("tile", tile as i64));
    Ok(())
}

/// Draws one card (the `counted` hand arrival).
const DRAWER: CardDef = CardDef::new("TEST:drawer", &[On::Play("", None, drawer_play)]);

fn drawer_play(player_id: i32) -> card_sdk::Asked {
    ctx::draw(player_id, 1)?;
    Ok(())
}

/// Tutors `TEST:zoner` from the draw pile into the hand (an uncounted
/// arrival).
const TUTOR: CardDef = CardDef::new("TEST:tutor", &[On::Play("", None, tutor_play)]);

fn tutor_play(player_id: i32) -> card_sdk::Asked {
    ctx::add_to_dest(player_id, "TEST:zoner", ctx::Dest::Hand, false);
    Ok(())
}

/// Turns the player's last field instance face-down (`ctx::set_face_down_at`,
/// so its flip hooks run). The prober does not place itself.
const FLIP_DOWN: CardDef = CardDef::new("TEST:flipDown", &[On::Play("", None, flip_down_play)]);

fn flip_down_play(player_id: i32) -> card_sdk::Asked {
    if let Some(&(uid, _)) = ctx::field_instances(player_id).last() {
        ctx::set_face_down_at(uid, true, card_sdk::abi::ZoneChangeCause::Effect);
    }
    Ok(())
}

/// Turns the player's last field instance face-up.
const FLIP_UP: CardDef = CardDef::new("TEST:flipUp", &[On::Play("", None, flip_up_play)]);

fn flip_up_play(player_id: i32) -> card_sdk::Asked {
    if let Some(&(uid, _)) = ctx::field_instances(player_id).last() {
        ctx::set_face_down_at(uid, false, card_sdk::abi::ZoneChangeCause::Effect);
    }
    Ok(())
}

/// Leaves the player's last field instance to the Graveyard (`ctx::unplace_at`).
const DUMPER: CardDef = CardDef::new("TEST:dumper", &[On::Play("", None, dumper_play)]);

fn dumper_play(player_id: i32) -> card_sdk::Asked {
    ctx::log(player_id, &Msg::new("dbg.dumper").n("uids", ctx::field_instances(player_id).iter().map(|(u, _)| *u).len() as i64).n("pick", ctx::field_instances(player_id).last().map_or(-1, |(u, _)| *u) as i64));
    if let Some(&(uid, _)) = ctx::field_instances(player_id).last() {
        ctx::unplace_at(
            uid,
            ctx::Dest::Graveyard,
            card_sdk::abi::ZoneChangeCause::Effect,
        );
    }
    Ok(())
}

/// Tutors `PPP:[衍生]拍卖撤下来了` into the hand (an uncounted arrival into
/// another card's hook).
const TUTOR_AUCTION: CardDef = CardDef::new("TEST:tutorAuction", &[On::Play("", None, tutor_auction_play)]);

fn tutor_auction_play(player_id: i32) -> card_sdk::Asked {
    ctx::add_to_dest(player_id, "PPP:[衍生]拍卖撤下来了", ctx::Dest::Hand, false);
    Ok(())
}

card_sdk::ruleset!(&[
    RELAY,
    RECURSE,
    ECHO,
    LISTER,
    STUNNER,
    STUN_SELF_MOVER,
    TELEPORTER,
    FORCER,
    STOPPER,
    REVERSER,
    STAYER,
    PLAN_STAY,
    END_STAY,
    IMMUNE_TOUCH,
    GUARD,
    AIMER,
    TARGET_SELF,
    TARGET_TILE,
    TARGET_ALL,
    NEAREST,
    EXEMPTION_PROBE,
    TARGET_COUNTER,
    TARGETED_PROBE,
    SETTLE_PROBE,
    PAY_OTHER_TILE,
    SNAP_TWICE,
    SNAP_ASK,
    DRAW_TWICE,
    SHIELD,
    MOVER,
    DICE_MOVER,
    COUNTER,
    PROBE,
    DENY,
    NEGATOR,
    JOIN_OPEN,
    JOIN_SHIELD,
    PAY_COUNTER,
    PAY_NEGATE,
    CARD_COPY,
    PLACING_COUNTER,
    LABELED_COPY,
    SETTLE_COPY,
    PAY_COPY,
    CRYSTAL,
    DEST_NOW,
    DEST_TO,
    TOTAL_CUT,
    DEAD_PAY,
    SELF_CHARGE,
    PAY_ADD_ANY,
    TELE_NOSOLVE,
    RENT_ADD_100,
    RENT_MUL_HALF,
    SETTLE_ADD_100,
    SETTLE_ADD_50,
    SETTLE_MUL_HALF,
    FACTOR_SUSPEND,
    SETTLE_ASK,
    PAY_OVERCUT,
    XFER_1000,
    GAIN_1000,
    LOSE_1000,
    MARKER_DENY,
    MARKER_SPEND,
    CHARGE_ROLL,
    PRE_REJECT,
    PRE_ACCEPT,
    PRE_PLAY,
    PRE_HOOK,
    PRE_GATE,
    PRE_ROLLPLAN,
    PRE_ATEMD,
    PRE_SETTLE,
    SPLIT_12K,
    SHAPE_LATE,
    LOSE_RAW,
    SPEND_FIRE,
    PASS_TELE,
    TILE_HOOK,
    DENY_PLAY,
    PASS_FLASH,
    PASS_BEFORE_FLASH,
    PING,
    PING_REJECT,
    ROLL_THEN_PAY,
    DECK_ONCE,
    CLOCK_HOOK,
    READ_HOST_READ,
    SETTLE_NEST,
    SETTLE_TAIL,
    SETTLE_SIB,
    SETTLE_CANCEL,
    SETTLE_RENT_ADD_100,
    SETTLE_RENT_MUL_HALF,
    SETTLE_PAY_1000,
    SILENT_PAIR,
    SILENT_NEST,
    SILENT_PAY,
    SILENT_QUIET,
    SILENT_ASK,
    QUIET_COUNTER,
    QUIET_RAISE,
    FLASH_ALWAYS,
    FLASH_NEVER,
    FLASH_ALWAYS_QUIET,
    FLASH_FOLD_ALWAYS,
    FLASH_FOLD_NEVER,
    FLASH_HEADER_IDS,
    FLASH_THEN_ASK,
    FLASH_TWO_ASKS,
    FLASH_KEYED_TWICE,
    FLASH_CARD_EARLY,
    FLASH_SUPPRESS,
    FLASH_AUTO_SELF,
    FLASH_OUTER,
    FLASH_INNER,
    CUSTOM_FILTER,
    DECLINE,
    PICK,
    DECLINE_SIB,
    GATE_CLOSED,
    ASKING_GUARD,
    PICK_COUNTER,
    TRIGGER_SEEN,
    TRAP_DECLARE,
    STRUCT_ARG,
    NAMED_OPEN,
    NAMED_SHUT,
    QUERY,
    ROLL_TRACE,
    FIRE_RECAST,
    FIRE_REROLL,
    PLAN_OFFER,
    PLAN_DECLINE,
    DUAL_ACTION,
    INVOKED,
    INVOKER,
    INVOKE_BIND,
    INVOKE_SEED,
    INVOKE_BAD_ENTRY,
    INVOKE_BAD_CARD,
    INVOKE_FILTER,
    INVOKE_SKILL_PROBE,
    PLAY_CARD_PROBE,
    INSPECT_PROBE,
    LOOK_AT_TWO,
    LOOK_REVEAL,
    LOOK_KEEP,
    LOOK_LEAK,
    DRAW_TWO,
    DRAW_AFTER_PROBE,
    HOUSE_ADD,
    HOUSE_REMOVE,
    OWNER_SET,
    BUILD_FREE,
    BUILD_FIXED,
    BUILD_SCALED,
    BUILD_PROBE,
    HOUSE_REMOVED_PROBE,
    TILE_EFFECT_COUNTER,
    TILE_SWAP_COUNTER,
    SETTLE_HERE,
    SWAP_PROBE,
    TILE_ID_PROBE,
    PATH_PROBE,
    HOUSE_SET,
    BUILD_VETO,
    RENT_CHARGE,
    ZONER,
    HOLDER,
    CREATOR,
    DROP,
    WASTER,
    HAND_WASTER,
    OUTCOMER,
    OBSERVER,
    CYC,
    CYCLER,
    BOARDER,
    BOARD_OFF,
    FLIPPER,
    REHOMER,
    TILE_PROBE,
    DRAWER,
    TUTOR,
    FLIP_DOWN,
    FLIP_UP,
    DUMPER,
    TUTOR_AUCTION
]);
