//! `AG:(兰) 像往常一样` -- C# `CardRanAsUsual` (MatchHost.cs:1095-1160):
//!
//! 规则书（docs/rulebook/cards.json, id `AG:(兰) 像往常一样`）:
//! > (兰) 像往常一样：
//!
//! > （1）此卡可以当反击使用
//! >  
//! > （2）受到异常移动效果（包括你的技能）的回合结束前，回到起始地点并取消所有受到的效果（不进行任何结算）。
//!
//! arm a turn-end undo of abnormal movement (counteraction or hand play).

use bandori_std::ctx;
use card_sdk::abi::{ActionKind, ChainKind};
use card_sdk::{it, CardText};
use card_sdk::{key, Action, CardDef, Msg, On};

pub const RAN_AS_USUAL: CardDef = CardDef::new(
    "AG:(兰) 像往常一样",
    &[
        On::Play("", None, play),
        // G4: kind (Effect) is the category. The condition carries the whole
        // guard body: target rel + `effect.has(Abnormal)` + the once-per-turn
        // slot. Residual guard deleted.
        // `abnormal_gate` raises the `effect` chain for every landing,
        // self-applied included (「包括你的技能」); only the separate `ImmuneAll`
        // check still skips `by == player_id`.
        // TODO(规则书)（1）: the C# adds a once-per-turn gate
        //   (`!H.V(player_id, "asUsualTurn").Equals(H.TurnKey)`) that the passage
        //   does not name -- 「此卡可以当反击使用」 has no per-turn limit. Left in
        //   place (it narrows, never broadens) until the book rules on it.
        // 规则书（1）: 「此卡可以当反击使用」
        On::Counteract(
            &[ChainKind::Effect],
            "target == owner && effect.has(Abnormal) && slot('asUsualTurn') != turn_key",
            None,
            counteract),
        // The body-top applicability checks are the condition / residual guard
        // now: a seat still in the game (`owner.out == 0`) that actually took
        // an abnormal effect this turn (`at_end_something_to_undo`).
        On::Schedule("at_end", "owner.out == 0", Some(at_end_something_to_undo), at_end),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"像往常一样"#)
        .raw_name(r#"(兰) 像往常一样"#)
        .tags(&[r#"手"#, r#"反击"#])
        .text(CardText::items(&[
            it(r#"此卡可以当反击使用"#),
            it(r#"受到异常移动效果（包括你的技能）的回合结束前，回到起始地点并取消所有受到的效果（不进行任何结算）。"#),
        ]))
        // sheet cell C6
        .fields(&[("group", r#"Afterglow"#), ("owner", r#"美竹兰"#)]));

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）: 「此卡可以当反击使用」 -- the C# also plays it from hand (`Play`).
    arm(player_id);
    Ok(())
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    arm(player_id);
    Ok(())
}

fn arm(player_id: i32) {
    // C# `H.SetV(i, "asUsualTurn", H.TurnKey)` -- latch for the once-per-turn gate.
    ctx::set_slot(player_id, "asUsualTurn", ctx::turn_key());
    // 规则书（2）: 「受到异常移动效果（包括你的技能）的回合结束前，回到起始地点并取消所有受到的效果（不进行任何结算）」
    // -- the C# `Arm` logs and queues `Undo` on `H._turnCtx.AtEnd`.
    ctx::log(
        player_id,
        &Msg::new(key!("ran_as_usual_armed")).player_id("who", player_id),
    );
    // C# `H._turnCtx.AtEnd.Add(() => Undo(i))` -- the turn-end queue hook
    // (`ctx::before_turn_end` + `On::Schedule`). The card need not be in play.
    ctx::before_turn_end(player_id, "at_end");
}

/// C# `CardRanAsUsual.Undo`, queued on `H._turnCtx.AtEnd`. The scheduling half
/// is live, and so is the restore body below (its remaining gaps are its own
/// TODOs). Applicability is the entry's condition + [`at_end_something_to_undo`].
fn at_end(player_id: i32) -> card_sdk::Asked {
    // 规则书（2）: 「回到起始地点并取消所有受到的效果（不进行任何结算）」 --
    // restore what the turn-start snapshot recorded (C# `H._turnSnap[i]`):
    // the square, the layer counters, the money the turn cost and the board
    // ownership / house / mortgage arrays.
    // TODO(规则书)（起始地点）: 「起始地点」 is undefined (CiRCLE / the seat's opening
    //   tile / the pre-move position -- `docs/rulebook/spec/cards/ag.md`) -- we
    //   use the turn-start square.
    // TODO(规则书)（2）: the C# no-op gate also fires on the `Plan.Reverse` skill
    //   case, which a counter-only gate misses -- a reverse *skill* move is an
    //   abnormal move even though `_abnormalTurn` never counted it.
    let snap = ctx::turn_snap(player_id);
    if snap.pos >= 0 {
        // 规则书（2）: 「取消所有受到的效果（不进行任何结算）」 -- money goes back
        // to the turn-start figure as a raw move (no settle pipeline).
        let delta = snap.money - ctx::money_of(player_id);
        if delta != 0 {
            ctx::adjust_money(
                player_id,
                delta,
                &Msg::new(key!("ran_as_usual_undo"))
                    .player_id("who", player_id)
                    .tile("tile", snap.pos),
            );
        }
        // 规则书（2）: 「取消所有受到的效果」 -- deed ownership, houses and
        // mortgages taken off the turn-start board copy are put back.
        if snap.owners.len() == ctx::tile_count() as usize {
            for (t, &owner) in snap.owners.iter().enumerate() {
                ctx::set_owner(t as i32, owner);
            }
            for (t, &houses) in snap.houses.iter().enumerate() {
                ctx::set_houses(t as i32, houses);
            }
            for (t, &mortgaged) in snap.mortgaged.iter().enumerate() {
                ctx::set_mortgaged(t as i32, mortgaged);
            }
        }
        // 规则书（2）: 「回到起始地点」 -- the return settles nothing but is not
        // gate-free: `teleport_to` runs the abnormal gate, where the RAS band
        // guard refuses it as a second abnormal of the turn. `teleport_as` is
        // the card's own gate-free return.
        ctx::teleport_as(player_id, snap.pos, false, false);
    }
    ctx::state::set(player_id, bandori_std::state_key::STAY, snap.stay);
    ctx::state::set(player_id, bandori_std::state_key::STUN, snap.stun);
    ctx::state::set(player_id, bandori_std::state_key::EXILE, snap.exile);
    ctx::log(
        player_id,
        &Msg::new(key!("ran_as_usual_undo"))
            .player_id("who", player_id)
            .tile("tile", snap.pos),
    );
    Ok(())
}

/// `ctx::abnormal_count(player_id)` (C# `H._abnormalTurn[i]`) -- something
/// abnormal landed this turn, so there is something to undo. Derived turn
/// state outside the CEL schema (docs/GUARDS.md §4.2c): the residual guard of
/// [`at_end`].
fn at_end_something_to_undo(player_id: i32) -> bool {
    ctx::abnormal_count(player_id) > 0
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn at_end_kani(player_id: i32) -> card_sdk::Asked {
    at_end(player_id)
}
