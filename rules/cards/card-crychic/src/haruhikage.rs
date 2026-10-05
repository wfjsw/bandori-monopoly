//! `CRYCHIC:春日影` -- C# `CardHaruhikage` (MatchHost.cs:2584-2707): [反击] that
//! runs a CRYCHIC character skill (reroll / step toward / cancel a rent).
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:春日影`）:
//! > 春日影
//! > ：
//! > （1）[特] 若抽到此卡时你的总资产大于等于20000，可选择使其直接从抽牌堆打出，依次抽牌直至你的手牌数为6，若受到弃牌效果则中断此效果。
//! > （2）（此卡可作为[反击]打出）使用一次Crychic角色的技能
//!

use alloc::vec::Vec;

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const HARUHIKAGE: CardDef = CardDef {
    id: "CRYCHIC:春日影",
    play: Some(play),
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

/// `H.Within(seat, 5, includeSame: false)` -- other seats within 5 tiles, not sharing yours.
fn within5(seat: i32) -> Vec<i32> {
    let pos = ctx::seat_pos(seat);
    ctx::others(seat)
        .into_iter()
        .filter(|&p| {
            let d = ctx::dist(pos, ctx::seat_pos(p));
            d > 0 && d <= 5
        })
        .collect()
}

fn play(seat: i32) {
    // 规则书（2）: 「使用一次Crychic角色的技能」 -- from hand the C# offers the
    // borrowed CRYCHIC skills (`H.CrychicChars` x `H.BorrowedActions`, then
    // `H.RunBorrowed`).
    ctx::log(seat, &Msg::new(key!("haruhikage_play")).seat("who", seat));
    // TODO(ABI): （2） 「使用一次Crychic角色的技能」 -- needs the skill-attachment
    //   surface (`H.BorrowedActions` / `H.RunBorrowed` / `H.ExtraOf` skills) so
    //   the player can pick and run one of 高松灯/椎名立希/丰川祥子/若叶睦/长崎素世
    //   （CRYCHIC）'s skills. The [反击] path below already runs the three skills
    //   the C# `React` wires to its trigger windows.
    // TODO(规则书): the C# `WhyNot` refuses the card when no CRYCHIC skill is
    //   usable (`现在没有能使用的 CRYCHIC 角色技能`) -- needs the `WhyNot` hook.
    // TODO(规则书)（1）: 「[特] 若抽到此卡时你的总资产大于等于20000，可选择使其直接从抽牌堆
    //   打出，依次抽牌直至你的手牌数为6，若受到弃牌效果则中断此效果。」 -- needs the
    //   Fx.Drawn hook (C# `CardHaruhikage.Drawn` -> `Special`), an asset total
    //   query (C# `H.AssetsOf`: money + deeds + buildings), and hand-size
    //   queries. `ctx::draw(seat, 1)` in a loop is ready once the hand count is.
}

fn can_react(seat: i32) -> bool {
    match trigger::kind() {
        // 规则书（2）[反击]: 「使用一次Crychic角色的技能」 -- C# `t.Kind == "moveRoll"
        // && t.Seat == seat` runs 椎名立希（CRYCHIC）'s skill.
        TriggerKind::MoveRoll => trigger::seat() == seat && trigger::move_roll().is_some(),
        // 规则书（2）[反击]: same, C# `t.Kind == "settleBefore" && t.Seat == seat &&
        // H.Within(seat, 5, includeSame: false).Count > 0` runs 丰川祥子（CRYCHIC）'s
        // skill (needs a player to step toward).
        TriggerKind::SettleBefore => trigger::seat() == seat && !within5(seat).is_empty(),
        // 规则书（2）[反击]: same, C# `t.Kind == "pay" && t.Pay.IsRent && t.Pay.to ==
        // seat && t.Pay.from != seat && t.Pay.tile >= 0 && !t.Pay.cancel` runs
        // 长崎素世（CRYCHIC）'s skill (cancels the rent).
        TriggerKind::Pay => {
            // on pay triggers `t.Pay.from == trigger::seat()`, `t.Pay.to == trigger::target()`
            trigger::seat() != seat
                && trigger::target() == seat
                && trigger::tile() >= 0
                && trigger::value() > 0
            // C# also wants `t.Pay.IsRent` and `!t.Pay.cancel`; neither is in the
            // trigger payload yet (TODO(ABI)), so any inbound payment looks rent-like.
        }
        _ => false,
    }
}

fn react(seat: i32) {
    match trigger::kind() {
        TriggerKind::MoveRoll => reroll_skill(seat),
        TriggerKind::SettleBefore => step_toward_skill(seat),
        TriggerKind::Pay => cancel_pay_skill(seat),
        _ => {}
    }
}

/// 椎名立希（CRYCHIC）的技能 -- one reroll of the move dice.
fn reroll_skill(seat: i32) {
    // 规则书（2）: 「使用一次Crychic角色的技能」 -- C# `React` "moveRoll" branch:
    // `int num = Math.Max(0, H.DoMoveRoll(move)); move.Roll = num;`.
    // TODO: C# rerolls with H.DoMoveRoll (honours the move's dice plan / bonuses).
    let x = ctx::roll(seat, 1, 20);
    // 规则书（2）: 「使用一次Crychic角色的技能」 -- the new face becomes the move roll.
    trigger::set_move_roll(x);
    ctx::log(
        seat,
        &Msg::new(key!("haruhikage_reroll")).seat("who", seat).i("n", x as i64),
    );
}

/// 丰川祥子（CRYCHIC）的技能 -- the endpoint steps 1 tile toward a player within 5.
fn step_toward_skill(seat: i32) {
    let near = within5(seat);
    if near.is_empty() {
        return;
    }
    // 规则书（2）: 「使用一次Crychic角色的技能」 -- C# `React` "settleBefore" branch
    // asks `终点向哪名玩家靠近 1 格？` over `H.Within(i, 5, includeSame: false)`.
    let who = ctx::ask_seat(
        seat,
        &Msg::new(key!("haruhikage_toward_title")),
        &Msg::new(key!("haruhikage_toward_ask")),
        &near,
    );
    // `H.StepToward(seat, target, why)` is a raw one-tile step toward the target
    // (it writes `State.seats[i].pos`, no settle); the host's `teleport_to` is
    // that same raw write.
    let pos = ctx::seat_pos(seat);
    let target = ctx::seat_pos(who);
    let n = ctx::tile_count();
    if n <= 0 {
        return;
    }
    let fwd = (target - pos).rem_euclid(n);
    if fwd == 0 {
        return;
    }
    let dir = if fwd <= n - fwd { 1 } else { -1 };
    let to = (pos + dir).rem_euclid(n);
    // 规则书（2）: 「使用一次Crychic角色的技能」 -- the endpoint moves 1 tile closer.
    ctx::teleport_to(seat, to);
    ctx::log(
        seat,
        &Msg::new(key!("haruhikage_step")).seat("who", seat).tile("tile", to),
    );
}

/// 长崎素世（CRYCHIC）的技能 -- cancel the rent being paid to you.
fn cancel_pay_skill(seat: i32) {
    // 规则书（2）: 「使用一次Crychic角色的技能」 -- C# `React` "pay" branch:
    // `trigger.Pay.cancel = true` plus `H.ExtraOf<SoyoTeleportFx>(i)` (teleport to
    // the mortgaged tile on the next main move).
    ctx::log(seat, &Msg::new(key!("haruhikage_pay_note")).seat("who", seat));
    // TODO(ABI): the rent cancel (C# `c.Trigger.Pay.cancel = true`) -- needs a
    //   pay-cancel op on the pay trigger.
    // TODO(ABI): `H.ExtraOf<SoyoTeleportFx>` (C# `SoyoTeleportFx.MoveBefore`:
    //   `m.TeleportTo = t.Pay.tile`) -- needs the Fx.MoveBefore hook and a
    //   remembered tile (C# `H.SetV(i, "soyoC", tile + 1)`).
}