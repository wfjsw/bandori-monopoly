//! `CRYCHIC:如果能一直持续下去...` -- C# `CardForever`: drain the band skill
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:如果能一直持续下去...`）:
//! > 如果能一直持续下去...： 
//!
//! > （1）[手] 移除你乐队技能卡上的奇迹水晶，获得2000+500*X资金，X为移除的奇迹水晶数量。
//!
//! > （2）[持续] 若你的手牌大于等于7，此卡立即置入弃牌堆。
//!
//! crystals for 2,000 + 500·X, then stays in play as [持续] until a draw
//! leaves the owner holding 7+ cards.

use card_sdk::abi::HookKind;
use card_sdk::ctx;
use card_sdk::{key, CardDef, Msg, On};

pub const FOREVER: CardDef = CardDef::new(
    "CRYCHIC:如果能一直持续下去...",
    &[
        On::Play(None, forever),
        // 规则书（2）[持续]: 「若你的手牌大于等于7，此卡立即置入弃牌堆。」 -- a
        // continuous condition, so the check runs at every point a hand size
        // moves (draw, discard-from-hand, play-from-hand) and at the turn
        // boundaries as a backstop. `Drew` alone only sees draws.
        On::Hook(
            &[
                HookKind::Drew,
                HookKind::DiscardAfter,
                HookKind::CardPlayed,
                HookKind::TurnEndBefore,
                HookKind::TurnEnd,
            ],
            |_| true,
            on_drew,
        ),
    ],
);

const ID: &str = "CRYCHIC:如果能一直持续下去...";

fn forever(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）[手]: 「移除你乐队技能卡上的奇迹水晶，获得2000+500*X资金，X为移除的奇迹水晶数量。」
    let x = ctx::band_crystals(player_id);
    if x > 0 {
        ctx::add_band_crystals(player_id, -x, 0); // 规则书（1）[手]: 「移除你乐队技能卡上的奇迹水晶」
    }
    ctx::gain(
        player_id,
        2000 + 500 * x,
        &Msg::new(key!("forever_why")).i("x", x as i64),
    )?;
    // 规则书（1）[手]: 「获得2000+500*X资金，X为移除的奇迹水晶数量」
    // 规则书（2）[持续]: the card lives in play until a draw leaves 7+ in hand
    // (`on_drew` below).
    ctx::set_dest(ctx::Dest::Field); // 规则书（2）[持续]: the card lives in play
    ctx::place_card(player_id, ID, &Msg::new(key!("forever_note")));
    Ok(())
}

/// 规则书（2）[持续]: 「若你的手牌大于等于7，此卡立即置入弃牌堆。」 -- a
/// continuous [持续] condition: whenever the owner's hand is 7+, the card goes
/// straight to the discard pile. C# `CardForever.Drew` only looked at the
/// owner's own draws; the sheet says 「立即」, so every hand-size movement
/// (and the turn boundaries as a backstop) re-checks.
fn on_drew(player_id: i32) -> card_sdk::Asked {
    if !ctx::is_placed() {
        return Ok(());
    }
    // 规则书（2）[持续]: 「若你的手牌大于等于7」 -- C# `H._hidden[Player].hand.Count < 7`.
    if ctx::hand_size(player_id) < 7 {
        return Ok(());
    }
    // 规则书（2）[持续]: 「此卡立即置入弃牌堆」 -- C# `H.Unplace(this, "discard", "手牌有 7 张以上")`.
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("forever_discard")).player_id("who", player_id),
    );
    Ok(())
}
