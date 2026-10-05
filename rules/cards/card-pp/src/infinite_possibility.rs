//! `PP:[大和麻弥]可能性为∞` -- C# `CardInfinitePossibility`
//! (MatchHost.cs:8064-8123): placed when revealed while looking through the
//! deck; crystals feed the band card.
//!
//! 规则书（docs/rulebook/cards.json, id `PP:[大和麻弥]可能性为∞`）:
//! > [大和麻弥]可能性为∞：
//! > [特]：
//! > 观看卡组并观看到此卡时将此卡展示给所有玩家并将其放置在自己[场上]（次效果优先于其他后续效果，比如观看卡组后将一张牌加入手牌）。
//! > [持续]：
//! >
//! > （1）[拥有者]进行抽卡动作后如果此卡的[奇迹水晶]小于4则为此卡添加1个[奇迹水晶]，否则移除此卡的[奇迹水晶]并为[拥有者]的Pastel✽Palettes乐队卡添加1个[奇迹水晶]。
//! >
//! > （2）[共鸣]交换[拥有者]的抽卡区和弃卡区。
//!
//! Not a hand play (C# `Normal => false`, no `Play` override): the [特] places it
//! out of a deck inspection; the crystal growth runs on the `Drew` hook.

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const INFINITE_POSSIBILITY: CardDef = CardDef::new("PP:[大和麻弥]可能性为∞", &[
    On::Hook(&[HookKind::Drew], drew),
]);

// TODO(规则书): [特]「观看卡组并观看到此卡时将此卡展示给所有玩家并将其放置在自己[场上]
// （次效果优先于其他后续效果，比如观看卡组后将一张牌加入手牌）」 -- needs a
// deck-inspection hook (C# `H.Present` / the reveal-while-looking path) that
// interrupts the look, shows this card, and `H.PlaceCard`s it before the rest of
// the look resolves. Nothing in the vocabulary observes a deck inspection.
/// C# `CardInfinitePossibility.Drew` -- once per draw batch, grow the crystal
/// counter (cap 4); at the cap the crystals become one band crystal instead.
/// 规则书[持续]（1）: 「[拥有者]进行抽卡动作后如果此卡的[奇迹水晶]小于4则为此卡
/// 添加1个[奇迹水晶]，否则移除此卡的[奇迹水晶]并为[拥有者]的Pastel✽Palettes乐队卡添加1个
/// [奇迹水晶]」
fn drew(player_id: i32) {
    if !ctx::is_placed(player_id) || trigger::player_id() != player_id {
        return;
    }
    if trigger::value() <= 0 {
        return;
    }
    if ctx::crystals(player_id) < 4 {
        ctx::add_crystals(player_id, 1, 4);
    } else {
        let c = ctx::crystals(player_id);
        ctx::add_crystals(player_id, -c, 0);
        ctx::add_band_crystals(player_id, 1, i32::MAX);
        ctx::log(
            player_id,
            &Msg::new(key!("infinite_possibility_full")).player_id("who", player_id),
        );
    }
}

// TODO(规则书): [持续]（2）「[共鸣]交换[拥有者]的抽卡区和弃卡区」 -- needs
// H.TryResonance (discard 「PP:[衍生]共鸣」 from hand) and the Fx.Actions hook
// (C# `Card.Actions` offering 「[共鸣] 交换抽卡区和弃卡区」); the swap itself
// (`H.ShuffleAllIntoDeck`-style exchange of `H._hidden[Player].draw` and
// `.discard`) has no vocabulary either.