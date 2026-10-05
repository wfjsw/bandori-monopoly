//! `PPP:仓库里的Random Star` -- C# `CardRandomStar` (MatchHost.cs:8919-8970):
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:仓库里的Random Star`）:
//! > 仓库里的Random Star：
//! >
//! > （1）将此卡放置自身场上并将弃牌堆和手牌洗入卡组，然后将一张“拍卖撤下来了”放置在卡组底端
//! >
//! > （2）[经过]“流星堂”时可使用2星星贴纸在“流星堂”强制停下并[结算]
//! >
//! the card stays in play; the 流星堂 stop needs the Fx.PassTile hook plus the
//! AbnormalGate forced-stop gate (TODO in source).

use card_sdk::{ctx, key, CardDef, Msg};

pub const RANDOM_STAR: CardDef = CardDef {
    id: "PPP:仓库里的Random Star",
    play: Some(random_star),
    can_react: None,
    react: None,
    why_not: None,
};

fn random_star(seat: i32) {
    // 规则书（1）: 「将此卡放置自身场上」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "PPP:仓库里的Random Star", &Msg::new(key!("random_star_note")));
    ctx::log(seat, &Msg::new(key!("random_star_placed")).seat("who", seat));
    // 规则书（1）: 「并将弃牌堆和手牌洗入卡组」 -- C# `H.ShuffleAllIntoDeck(seat, hand:
    // true, discard: true)`.
    ctx::sweep_to_deck(seat);
    // 规则书（1）: 「然后将一张“拍卖撤下来了”放置在卡组底端」 -- C#
    // `H.AddToDeck(seat, "PPP:[衍生]拍卖撤下来了", "bottom")`.
    ctx::add_to_deck_at(seat, "PPP:[衍生]拍卖撤下来了", ctx::DeckPos::Bottom);
    ctx::log(seat, &Msg::new(key!("random_star_swept")).seat("who", seat));
    // TODO(规则书)（2）: 「[经过]“流星堂”时可使用2星星贴纸在“流星堂”强制停下并[结算]」
    //   -- needs the Fx.PassTile hook plus the H.AbnormalGate forced-stop gate
    //   (C# `Abnormal{Kind = "stop"}` / `m.Stopped` / `m.Resolve`).
}