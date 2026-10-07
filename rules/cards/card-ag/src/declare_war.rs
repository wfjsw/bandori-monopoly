//! `AG:宣战布告` -- C# `CardDeclareWar` (MatchHost.cs:921-955):
//!
//! 规则书（docs/rulebook/cards.json, id `AG:宣战布告`）:
//! > 宣战布告：
//! > [手]：
//! > [反击]当你或你拥有的格子被其他玩家的卡效果影响时：[指定]那名玩家。被[指定]的玩家[支付][使用者]500资金且[使用者]抽1张卡。
//!
//! [反击] when another player's card affects you, they pay you 500 and you draw.

use card_sdk::abi::ChainKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const DECLARE_WAR: CardDef = CardDef::new(
    "AG:宣战布告",
    &[On::Counteract(&[ChainKind::Effect], can_counteract, counteract)],
);

fn can_counteract(player_id: i32) -> bool {
    // 规则书[反击]: 「当你或你拥有的格子被其他玩家的卡效果影响时」
    //
    // One condition on the *effect*, and it now reads as one: any effect another
    // player's card declared at me. It used to be reconstructed by unioning
    // `Target`/`Abnormal` (via `t.Target`) with `Pay` (via `t.Pay.from`) and
    // matching on two different fields -- the union this refactor removes.
    // `effect::hits` covers both because a payment touches its payer *and* its
    // payee.
    //
    // 「或你拥有的格子」 -- the declaration names the owner when a tile-affecting
    // effect aims at a tile, so the [指定] half lands here; effects that touch a
    // tile without naming its owner (house removal and friends) declare nothing
    // aimed at the owner and stay out of reach -- the same gap the C# has.
    let Some(by) = trigger::by_card().filter(|&by| by != player_id) else {
        return false;
    };
    if ctx::player_out(by) {
        return false;
    }
    if ctx::effect::hits(player_id) {
        return true;
    }
    // 「或你拥有的格子」 -- an effect aimed at a tile this player owns.
    (0..ctx::effect::count()).any(|i| {
        let t = ctx::effect::tile(i);
        t >= 0 && ctx::tile_owner(t) == player_id
    })
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // C# `int byCard = c.Trigger.ByCard` -- the player whose card affected this
    // one, not `t.Seat`. On a `pay` trigger `trigger::player_id()` is the *payer*
    // (this player, `t.Pay.from`), so the 500 must come from `by_card`.
    let Some(by) = trigger::by_card() else {
        return Ok(());
    };
    if by == player_id {
        return Ok(());
    }
    // 规则书[反击]: 「[指定]那名玩家」 -- the designation is the card's player.
    // C# `H.Target(c, byCard, r)`: the full targeting gate (out / exile /
    // ImmuneAll / Untargetable / Redirect / the `target` [反击] window), and
    // `r.yes` gates everything below. `Some(hit)` is the player actually hit --
    // a `Redirect` hook may have moved it (`r.index` in the C#).
    let Some(hit) = ctx::target(by) else {
        return Ok(());
    };
    // 规则书[反击]: 「被[指定]的玩家[支付][使用者]500资金」 -- C#
    // `H.PayR(r.index, c.Seat, 500, ...)`: from the player actually targeted.
    let why = Msg::new(key!("declare_war_why")).card("card", "AG:宣战布告");
    ctx::transfer(hit, player_id, 500, &why)?;
    // 规则书[反击]: 「且[使用者]抽1张卡」
    ctx::draw(player_id, 1)?;
    Ok(())
}
