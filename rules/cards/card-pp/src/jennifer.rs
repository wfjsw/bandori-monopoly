//! `PP:找回珍妮弗` -- C# `CardJennifer` (MatchHost.cs:7699-7765): place on
//! another player's field; when the owner passes 偶像经纪公司 the user pays 400.
//!
//! 规则书（docs/rulebook/cards.json, id `PP:找回珍妮弗`）:
//! > 找回珍妮弗：
//! > [手]：
//! > 将此卡放置在除[使用者]以外的一名玩家的[场地]。
//! > [持续]：
//! > [拥有者][经过]“偶像经纪公司”时依次进行以下操作：
//! > 1. [使用者][支付][拥有者]400资金；
//! > 2. [使用者]获得1个正面的[P✽P粉丝]，[拥有者]将一个[P✽P粉丝]变为正面；
//! > 3. 此卡[移除]，然后将1张“魔法战队Pastel✽Ranger”加入[使用者]手卡。
//!
//! The [持续] needs the PassTile hook and per-card Mem for the user (below).

use card_sdk::{ctx, key, CardDef, Msg};

pub const JENNIFER: CardDef = CardDef {
    id: "PP:找回珍妮弗",
    play: Some(jennifer),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

fn why_not(seat: i32) -> Option<Msg> {
    // 规则书[手]: 「将此卡放置在除[使用者]以外的一名玩家的[场地]」 -- nobody to
    // host it otherwise. C# `CardJennifer.WhyNot` refuses the play with no other
    // players ("没有别的玩家").
    if ctx::others_count(seat) == 0 {
        return Some(Msg::new(key!("jennifer_why_not")));
    }
    None
}

fn jennifer(seat: i32) {
    // 规则书[手]: 「将此卡放置在除[使用者]以外的一名玩家的[场地]」 -- C#
    // `H.PickTarget(c, H.Others(seat), ...)` then `H.PlaceFromPlay(c, r.index)`
    // (owner = the target, user = the player).
    // `H.PickTarget` also runs the `H.Target` targeting gate (Untargetable
    // check) -- no such gate in the vocabulary yet.
    let targets = ctx::others(seat);
    if targets.is_empty() {
        return;
    }
    let target = ctx::ask_seat(
        seat,
        &Msg::new(key!("jennifer_title")),
        &Msg::new(key!("jennifer_ask")),
        &targets,
    );
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(target, "PP:找回珍妮弗", &Msg::new(key!("jennifer_note")).seat("who", seat));
    // TODO(规则书): [持续]「[拥有者][经过]“偶像经纪公司”时依次进行以下操作：1. [使用者]
    // [支付][拥有者]400资金；2. [使用者]获得1个正面的[P✽P粉丝]，[拥有者]将一个[P✽P粉丝]
    // 变为正面；3. 此卡[移除]，然后将1张“魔法战队Pastel✽Ranger”加入[使用者]手卡」 --
    // needs the Fx.PassTile hook (C# `Card.PassTile(MoveCtx, int)` on the owner
    // passing 「偶像经纪公司」) and per-card Mem for the user (C# `Card.User`) so the
    // 400 flows user -> owner (`H.PayR(user, owner, 400, ...)`), the fans move as
    // `H.GainFans(user, 1, up: true, ...)` + `H.FlipUp(owner, 1, ...)`, and
    // 「魔法战队Pastel✽Ranger」 goes to the user's hand (`H.AddToHand(user, ...)`).
    // C# `Targeting => true; SingleTarget => true` -- CardDef has no targeting
    // flags yet.
}