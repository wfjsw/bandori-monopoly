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
//! out of a deck inspection, and every [持续] line needs an Fx hook the ABI
//! lacks (below).

use card_sdk::CardDef;

pub const INFINITE_POSSIBILITY: CardDef = CardDef {
    id: "PP:[大和麻弥]可能性为∞",
    play: None,
    can_react: None,
    react: None,
    why_not: None,
};

// TODO(规则书): [特]「观看卡组并观看到此卡时将此卡展示给所有玩家并将其放置在自己[场上]
// （次效果优先于其他后续效果，比如观看卡组后将一张牌加入手牌）」 -- needs a
// deck-inspection hook (C# `H.Present` / the reveal-while-looking path) that
// interrupts the look, shows this card, and `H.PlaceCard`s it before the rest of
// the look resolves. Nothing in the vocabulary observes a deck inspection.
// TODO(规则书): [持续]（1）「[拥有者]进行抽卡动作后如果此卡的[奇迹水晶]小于4则为此卡
// 添加1个[奇迹水晶]，否则移除此卡的[奇迹水晶]并为[拥有者]的Pastel✽Palettes乐队卡添加1个
// [奇迹水晶]」 -- needs the Fx.Drew hook (C# `Card.Drew(int, List<string>)`) and the
// per-card crystal counter (C# `Card.Crystals`, cap 4 via `NoteText`); the swap
// is `AddCrystals(-Crystals, "满了")` + `H.AddBandCrystals(Seat, 1, ...)`.
// TODO(规则书): [持续]（2）「[共鸣]交换[拥有者]的抽卡区和弃卡区」 -- needs
// H.TryResonance (discard 「PP:[衍生]共鸣」 from hand) and the Fx.Actions hook
// (C# `Card.Actions` offering 「[共鸣] 交换抽卡区和弃卡区」); the swap itself
// (`H.ShuffleAllIntoDeck`-style exchange of `H._hidden[Seat].draw` and
// `.discard`) has no vocabulary either.