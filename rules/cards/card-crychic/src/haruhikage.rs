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

use card_sdk::abi::{TriggerKind, ChainKind, HookKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const HARUHIKAGE: CardDef = CardDef::new("CRYCHIC:春日影", &[
    On::Play(None, play),
    On::CounterAct(&[ChainKind::Effect, ChainKind::MoveRoll, ChainKind::SettleBefore], can_react, react),
    On::Hook(&[HookKind::Drawn], |_| true, on_drawn)]);

const ID: &str = "CRYCHIC:春日影";

/// `H.Within(seat, 5, includeSame: false)` -- other players within 5 tiles, not sharing yours.
fn within5(player_id: i32) -> Vec<i32> {
    let pos = ctx::player_pos(player_id);
    ctx::others(player_id)
        .into_iter()
        .filter(|&p| {
            let d = ctx::dist(pos, ctx::player_pos(p));
            d > 0 && d <= 5
        })
        .collect()
}

/// C# `H.AssetsOf` -- money + land (mortgaged at half) + houses.
fn assets_of(player_id: i32) -> i32 {
    let mut n = ctx::money(player_id);
    for t in ctx::owned_tiles(player_id) {
        n += if ctx::mortgaged_of(t) {
            ctx::tile_price(t) / 2
        } else {
            ctx::tile_price(t)
        };
        n += ctx::houses_of(t) * ctx::build_cost(t);
    }
    n
}

fn play(player_id: i32) {
    // 规则书（2）: 「使用一次Crychic角色的技能」 -- from hand the C# offers the
    // borrowed CRYCHIC skills (`H.CrychicChars` x `H.BorrowedActions`, then
    // `H.RunBorrowed`).
    ctx::log(player_id, &Msg::new(key!("haruhikage_play")).player_id("who", player_id));
    // 规则书（2）: 「使用一次Crychic角色的技能」 -- pick one of the five CRYCHIC
    // skills and run its `On::Play` entry. `ctx::play_card` is that run: the
    // skill is a card rule and this invokes it under its own id.
    const CRYCHIC: [&str; 5] = [
        "skill:高松灯（CRYCHIC）:跌跌撞撞...",
        "skill:椎名立希（CRYCHIC）:克服劣等感",
        "skill:长崎素世（CRYCHIC）:雨中祈晴",
        "skill:丰川祥子（CRYCHIC）:你愿意和我组建乐队吗？",
        "skill:若叶睦（CRYCHIC）:精致的人偶",
    ];
    let options: alloc::vec::Vec<Msg> = CRYCHIC
        .iter()
        .map(|id| Msg::new(key!("haruhikage_option")).card("card", *id))
        .collect();
    let k = ctx::ask_pick(
        player_id,
        &Msg::new(key!("haruhikage_title")),
        &Msg::new(key!("haruhikage_ask")),
        &options,
    );
    if let Some(&id) = CRYCHIC.get(k) {
        ctx::play_card(id, player_id);
    }
}

/// 规则书（1）[特]: 「若抽到此卡时你的总资产大于等于20000，可选择使其直接从抽牌堆打出，
/// 依次抽牌直至你的手牌数为6，若受到弃牌效果则中断此效果。」 -- C#
/// `CardHaruhikage.Drawn` -> `Special`.
fn on_drawn(player_id: i32) {
    if trigger::kind() != TriggerKind::Drawn || !trigger::card_is(ID) {
        return;
    }
    // 规则书（1）: 「若抽到此卡时你的总资产大于等于20000」 -- C# `H.AssetsOf(seat) < 20000`.
    if assets_of(player_id) < 20000 {
        return;
    }
    // 规则书（1）: 「可选择使其直接从抽牌堆打出」 -- C# `H.AskYes(..., aiYes: true)`.
    let yes = ctx::ask_yes(
        player_id,
        &Msg::new(key!("haruhikage_title")),
        &Msg::new(key!("haruhikage_special_ask")),
    );
    if !yes {
        return;
    }
    // The card was just drawn, so it is in hand (C# `hand.Remove(Id)`).
    if !ctx::take_from_hand(player_id, ID) {
        return;
    }
    ctx::log(player_id, &Msg::new(key!("haruhikage_special_play")).player_id("who", player_id).card("card", ID));
    // 规则书（1）: 「依次抽牌直至你的手牌数为6，若受到弃牌效果则中断此效果。」 -- C#
    // breaks when the hand did not grow (`hand.Count <= before`) or the player is out.
    for _ in 0..10 {
        if ctx::hand_size(player_id) >= 6 || ctx::player_out(player_id) {
            break;
        }
        let before = ctx::hand_size(player_id);
        ctx::draw(player_id, 1);
        if ctx::hand_size(player_id) <= before {
            break;
        }
    }
    // 规则书（1）: the [特] body finishes with the card in the discard pile.
    ctx::to_discard(player_id, ID);
}

fn can_react(player_id: i32) -> bool {
    match trigger::kind() {
        // 规则书（2）[反击]: 「使用一次Crychic角色的技能」 -- C# `t.Kind == "moveRoll"
        // && t.Seat == player` runs 椎名立希（CRYCHIC）'s skill.
        TriggerKind::MoveRoll => trigger::player_id() == player_id && trigger::move_roll().is_some(),
        // 规则书（2）[反击]: same, C# `t.Kind == "settleBefore" && t.Seat == seat &&
        // H.Within(player, 5, includeSame: false).Count > 0` runs 丰川祥子（CRYCHIC）'s
        // skill (needs a player to step toward).
        TriggerKind::SettleBefore => trigger::player_id() == player_id && !within5(player_id).is_empty(),
        // 规则书（2）[反击]: same, C# `t.Kind == "pay" && t.Pay.IsRent && t.Pay.to ==
        // player && t.Pay.from != player && t.Pay.tile >= 0 && !t.Pay.cancel` runs
        // 长崎素世（CRYCHIC）'s skill (cancels the rent).
        //
        // The payment is now declared as an `effect`; `pay_is_rent` on the link
        // is what tells it apart from a targeting or an abnormal.
        TriggerKind::Effect => {
            // on a payment effect `from` is the payer and `target` the payee.
            trigger::pay_is_rent()
                && trigger::player_id() != player_id
                && trigger::target() == player_id
                && trigger::tile() >= 0
                && trigger::value() > 0
        }
        _ => false,
    }
}

fn react(player_id: i32) {
    match trigger::kind() {
        TriggerKind::MoveRoll => reroll_skill(player_id),
        TriggerKind::SettleBefore => step_toward_skill(player_id),
        TriggerKind::Pay => cancel_pay_skill(player_id),
        _ => {}
    }
}

/// 椎名立希（CRYCHIC）的技能 -- one reroll of the move dice.
fn reroll_skill(player_id: i32) {
    // 规则书（2）: 「使用一次Crychic角色的技能」 -- C# `React` "moveRoll" branch:
    // `int num = Math.Max(0, H.DoMoveRoll(move)); move.Roll = num;`.
    // C# rerolls with `H.DoMoveRoll`, which sums the move's whole dice table.
    let x = ctx::do_move_roll(player_id);
    // 规则书（2）: 「使用一次Crychic角色的技能」 -- the new face becomes the move roll.
    trigger::set_move_roll(x);
    ctx::log(
        player_id,
        &Msg::new(key!("haruhikage_reroll")).player_id("who", player_id).i("n", x as i64),
    );
}

/// 丰川祥子（CRYCHIC）的技能 -- the endpoint steps 1 tile toward a player within 5.
fn step_toward_skill(player_id: i32) {
    let near = within5(player_id);
    if near.is_empty() {
        return;
    }
    // 规则书（2）: 「使用一次Crychic角色的技能」 -- C# `React` "settleBefore" branch
    // asks `终点向哪名玩家靠近 1 格？` over `H.Within(i, 5, includeSame: false)`.
    let who = ctx::ask_player(
        player_id,
        &Msg::new(key!("haruhikage_toward_title")),
        &Msg::new(key!("haruhikage_toward_ask")),
        &near,
    );
    // `H.StepToward(seat, target, why)` is a raw one-tile step toward the target
    // (it writes `State.players[i].pos`, no settle); the host's `teleport_to` is
    // that same raw write.
    let pos = ctx::player_pos(player_id);
    let target = ctx::player_pos(who);
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
    ctx::teleport_to(player_id, to);
    ctx::log(
        player_id,
        &Msg::new(key!("haruhikage_step")).player_id("who", player_id).tile("tile", to),
    );
}

/// 长崎素世（CRYCHIC）的技能 -- cancel the rent being paid to you.
fn cancel_pay_skill(player_id: i32) {
    // 规则书（2）: 「使用一次Crychic角色的技能」 -- C# `React` "pay" branch:
    // `trigger.Pay.cancel = true` plus `H.ExtraOf<SoyoTeleportFx>(i)` (teleport to
    // the mortgaged tile on the next main move).
    ctx::log(player_id, &Msg::new(key!("haruhikage_pay_note")).player_id("who", player_id));
    // 规则书（2）: cancel the rent being paid to you.
    trigger::set_pay_amount(0);
    // TODO(规则书)[judgement](ABI): `H.ExtraOf<SoyoTeleportFx>` (C# `SoyoTeleportFx.MoveBefore`:
    //   the clause under-specifies -- see the note above it
    //   `m.TeleportTo = t.Pay.tile`) -- needs the Fx.MoveBefore hook and a
    //   remembered tile (C# `H.SetV(i, "soyoC", tile + 1)`).
}
