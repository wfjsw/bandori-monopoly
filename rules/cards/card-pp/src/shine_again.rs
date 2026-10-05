//! `PP:再次闪耀` -- C# `CardShineAgain` (MatchHost.cs:7145-7243): stay in play;
//! while short on cash the owner may record a colour and take a rescue.
//!
//! 规则书（docs/rulebook/cards.json, id `PP:再次闪耀`）:
//! > 再次闪耀：
//! > [特]：
//! > 此卡不受除拥有此卡的玩家以外的玩家的效果影响。
//! > [手]：
//! > 将此卡放置在[使用者]的[场地]。
//! > [持续]：
//! >
//! > （1）[反击][拥有者][消耗]或[支付]并当前资金不够时：记录一个此卡未记录且[拥有者]拥有同色地契的颜色，[拥有者][获得]“[拥有者]拥有的1个同色地契的购买价格”÷5的资金和1个正面[P✽P粉丝]。
//! >
//! > （2）[共鸣]记录一个此卡未记录的颜色，[拥有者]获得1个正面[P✽P粉丝]。
//!
//! The rescue runs in the `PayChoose` hook; the recorded colours live in a
//! player slot (stand-in for the C# per-card `HashSet<int> _colors`).

use alloc::vec::Vec;

use card_sdk::abi::{TriggerKind, HookKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const SHINE_AGAIN: CardDef = CardDef::new("PP:再次闪耀", &[
    On::Play(shine_again),
    On::Hook(&[HookKind::PayChoose], pay_choose),
]);

/// Stand-in for C# `CardShineAgain._colors` (per-card Mem): a bitset of the
/// colour groups already recorded, kept on the owner's player while in play.
const SLOT_COLORS: &str = "shine_again_colors";

fn shine_again(player_id: i32) {
    // 规则书[手]: 「将此卡放置在[使用者]的[场地]」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "PP:再次闪耀", &Msg::new(key!("shine_again_note")));
    // 规则书[持续]（1）: 「记录一个此卡未记录的颜色」 -- the recorded set starts
    // empty (C# `new HashSet<int>()` on the card).
    ctx::set_slot(player_id, SLOT_COLORS, 0);
    // TODO(规则书): [特]「此卡不受除拥有此卡的玩家以外的玩家的效果影响」 -- needs the
    // C# `Card.Immune` flag so other players' effects skip this field card
    // (`public override bool Immune => true`).
}

/// Colour groups of buyable tiles the owner holds, minus the recorded set
/// (C# `CardShineAgain.Colors(owned: true)`).
fn unrecorded_owned_colors(player_id: i32) -> Vec<i32> {
    let recorded = ctx::slot(player_id, SLOT_COLORS);
    let mut out: Vec<i32> = Vec::new();
    for t in ctx::owned_tiles(player_id) {
        if !ctx::is_buyable(t) {
            continue;
        }
        let g = ctx::tile_group(t);
        if g < 0 || out.contains(&g) {
            continue;
        }
        if recorded & (1 << g) != 0 {
            continue;
        }
        out.push(g);
    }
    out
}

/// C# `CardShineAgain.PayChoose` -- while the owner cannot cover a payment,
/// record a colour and take that colour's top deed price / 5 plus a fan.
fn pay_choose(player_id: i32) {
    if trigger::kind() != TriggerKind::PayChoose || !ctx::is_placed(player_id) {
        return;
    }
    if trigger::player_id() != player_id || trigger::value() <= 0 {
        return;
    }
    // 规则书[持续]（1）: 「当前资金不够时」 -- C# `Me.money >= p.amount` refuses.
    if ctx::money(player_id) >= trigger::value() {
        return;
    }
    let colors = unrecorded_owned_colors(player_id);
    if colors.is_empty() {
        return;
    }
    // 规则书[持续]（1）: 「记录一个此卡未记录且[拥有者]拥有同色地契的颜色」
    let mut options: Vec<Msg> = Vec::new();
    for &g in &colors {
        options.push(Msg::new(key!("shine_again_color")).i("g", g as i64));
    }
    let pick = ctx::ask_pick(
        player_id,
        &Msg::new(key!("shine_again_title")),
        &Msg::new(key!("shine_again_ask")).i("n", trigger::value() as i64),
        &options,
    );
    let g = colors[pick.min(colors.len() - 1)];
    ctx::set_slot(player_id, SLOT_COLORS, ctx::slot(player_id, SLOT_COLORS) | (1 << g));
    // 规则书[持续]（1）: 「[拥有者]拥有的1个同色地契的购买价格」÷5 -- C# `Max(price)`
    // over the owner's deeds in that colour (`H._tiles[t].price`).
    let mut best = 0;
    for t in ctx::owned_tiles(player_id) {
        if ctx::is_buyable(t) && ctx::tile_group(t) == g {
            let p = ctx::tile_price(t);
            if p > best {
                best = p;
            }
        }
    }
    // 规则书[持续]（1）: 「[拥有者][获得]……资金和1个正面[P✽P粉丝]」 -- C#
    // `H.GainFans(Seat, 1, up: true)` + `H.GainR(Seat, maxPrice / 5)`.
    ctx::add_tok(player_id, "P✽P粉丝(正)", 1, i32::MAX);
    let money = best / 5;
    if money > 0 {
        ctx::gain(player_id, money, &Msg::new(key!("shine_again_why")).i("n", money as i64));
    }
    // TODO(规则书): [持续]（2）「[共鸣]记录一个此卡未记录的颜色，[拥有者]获得1个正面
    // [P✽P粉丝]」 -- needs H.TryResonance (discard 「PP:[衍生]共鸣」 from hand) and the
    // Fx.Actions hook (C# `Card.Actions` offering 「[共鸣] 再次闪耀」) plus the same
    // colour set; the fan is `H.GainFans(Seat, 1, up: true, ...)`.
}