//! `AG:宣战布告` -- C# `CardDeclareWar` (MatchHost.cs:921-955):
//! [反击] when another player's card affects you, they pay you 500 and you draw.
//!
//! 规则书（docs/rulebook/cards.json, id `AG:宣战布告`）:
//! > 宣战布告：
//! > [手]：
//! > [反击]当你或你拥有的格子被其他玩家的卡效果影响时：[指定]那名玩家。被[指定]的玩家[支付][使用者]500资金且[使用者]抽1张卡。
//!

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const DECLARE_WAR: CardDef = CardDef::new("AG:宣战布告", &[
    On::React(&[TriggerKind::Pay, TriggerKind::Abnormal, TriggerKind::Target], can_react, react),
]);

fn can_react(player_id: i32) -> bool {
    // 规则书[反击]: 「当你或你拥有的格子被其他玩家的卡效果影响时」 -- C#
    // `H.HitByOtherCard(t, seat)`: `t.ByCard` is a different player and the trigger
    // is `target` / `pay` / `abnormal` aimed at you.
    // TODO(规则书): 「或你拥有的格子」 -- narrowed to the C#: `HitByOtherCard`
    // only keys on effects aimed at the player (`t.Target == seat` /
    // `t.Pay.from == seat`). A `H.TargetTile` on a tile you own does raise
    // `target` with `t.Target == you` (the owner), so the [指定] half of the
    // clause already lands here; other tile-affecting effects (house removal
    // and friends) raise nothing aimed at the owner and stay out of reach --
    // the same gap the C# has.
    let Some(by) = trigger::by_card().filter(|&by| by != player_id) else {
        return false;
    };
    match trigger::kind() {
        // C# `target`/`abnormal`: `t.Target == seat`, `!H.Out(t.ByCard)`.
        TriggerKind::Target | TriggerKind::Abnormal => {
            trigger::target() == player_id && !ctx::player_out(by)
        }
        // C# `pay`: `t.Pay.from == seat` (this player is the one paying).
        TriggerKind::Pay => trigger::player_id() == player_id && !ctx::player_out(by),
        _ => false,
    }
}

fn react(player_id: i32) {
    // C# `int byCard = c.Trigger.ByCard` -- the player whose card affected this
    // one, not `t.Seat`. On a `pay` trigger `trigger::player_id()` is the *payer*
    // (this player, `t.Pay.from`), so the 500 must come from `by_card`.
    let Some(by) = trigger::by_card() else {
        return;
    };
    if by == player_id {
        return;
    }
    // 规则书[反击]: 「[指定]那名玩家」 -- the designation is the card's player.
    // C# `H.Target(c, byCard, r)`: the full targeting gate (out / exile /
    // ImmuneAll / Untargetable / Redirect / the `target` [反击] window), and
    // `r.yes` gates everything below. `Some(hit)` is the player actually hit --
    // a `Redirect` hook may have moved it (`r.index` in the C#).
    let Some(hit) = ctx::target(by) else {
        return;
    };
    // 规则书[反击]: 「被[指定]的玩家[支付][使用者]500资金」 -- C#
    // `H.PayR(r.index, c.Seat, 500, ...)`: from the player actually targeted.
    let why = Msg::new(key!("declare_war_why")).card("card", "AG:宣战布告");
    ctx::transfer(hit, player_id, 500, &why);
    // 规则书[反击]: 「且[使用者]抽1张卡」
    ctx::draw(player_id, 1);
}