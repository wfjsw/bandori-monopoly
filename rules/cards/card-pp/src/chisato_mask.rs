//! `PP:[白鹭千圣]微笑的铁假面` -- C# `CardChisatoMask`: stay in play, every other
//!
//! 规则书（docs/rulebook/cards.json, id `PP:[白鹭千圣]微笑的铁假面`）:
//! > [白鹭千圣]微笑的铁假面：
//! > [手]：
//! > 将此卡放置在[使用者]的[场地]，其他玩家[分摊][支付][使用者]2000资金。
//! > [持续]：
//! >
//! > （1）[拥有者]的弃卡区洗入抽卡区时其他玩家[分摊][支付][拥有者]500资金。
//! >
//! > （2）[共鸣]其他玩家[分摊][支付][拥有者]1500资金。
//!
//! player split-pays you 2,000. The reshuffle split runs on `Reshuffled`; the
//! [共鸣] branch is a second, larger split offered at play time.

use alloc::vec::Vec;

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const CHISATO_MASK: CardDef = CardDef::new("PP:[白鹭千圣]微笑的铁假面", &[
    On::Play(None, chisato_mask),
    On::Hook(&[HookKind::Reshuffled], reshuffled_guard, reshuffled)]);

fn chisato_mask(player_id: i32) {
    // 规则书[手]: 「将此卡放置在[使用者]的[场地]」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "PP:[白鹭千圣]微笑的铁假面", &Msg::new(key!("chisato_mask_note")));
    // 规则书[手]: 「其他玩家[分摊][支付][使用者]2000资金」
    split_pay(&ctx::others(player_id), player_id, 2000, &Msg::new(key!("chisato_mask_why")));
    // 规则书[持续]（2）: 「[共鸣]其他玩家[分摊][支付][拥有者]1500资金」
    if crate::resonance::try_resonance(player_id) {
        split_pay(&ctx::others(player_id), player_id, 1500, &Msg::new(key!("chisato_mask_resonance")));
    }
}

/// C# `CardChisatoMask.Reshuffled` -- when the owner's discard pile is shuffled
/// back into the draw pile, the other players split-pay the owner 500.
/// 规则书[持续]（1）: 「[拥有者]的弃卡区洗入抽卡区时其他玩家[分摊][支付][拥有者]500资金」
/// Pure guard for [`reshuffled`] -- the activation gate. `false`
/// means the card is not activated at all.
fn reshuffled_guard(player_id: i32) -> bool {
    ctx::is_placed(player_id) && trigger::player_id() == player_id
}

fn reshuffled(player_id: i32) {
    split_pay(&ctx::others(player_id), player_id, 500, &Msg::new(key!("chisato_mask_why")));
}

/// `H.SplitPay` -- every payer covers `ceil(ceil(total / n) / 10) * 10`.
fn split_pay(payers: &[i32], to: i32, total: i32, why: &Msg) {
    let list: Vec<i32> = payers.iter().copied().filter(|&p| p != to && !ctx::player_out(p)).collect();
    if list.is_empty() || total <= 0 {
        return;
    }
    let per = (total + list.len() as i32 - 1) / list.len() as i32;
    let share = (per + 9) / 10 * 10;
    for p in list {
        ctx::transfer(p, to, share, why);
    }
}