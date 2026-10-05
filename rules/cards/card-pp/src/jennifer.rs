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
//! The [持续] runs in the `PassTile` hook; the user is kept in a player slot
//! (stand-in for the C# per-card `Card.User`).

use card_sdk::abi::{TriggerKind, HookKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const JENNIFER: CardDef = CardDef::new("PP:找回珍妮弗", &[
    On::Play(jennifer),
    On::CantPlay(cant_play),
    On::Hook(&[HookKind::PassTile], pass_tile),
]);

/// Stand-in for C# `Card.User` (per-card Mem): the player who played the card,
/// stored on the owner's player while it is in play.
const SLOT_USER: &str = "jennifer_user";

fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书[手]: 「将此卡放置在除[使用者]以外的一名玩家的[场地]」 -- nobody to
    // host it otherwise. C# `CardJennifer.WhyNot` refuses the play with no other
    // players ("没有别的玩家").
    if ctx::others_count(player_id) == 0 {
        return Some(Msg::new(key!("jennifer_why_not")));
    }
    None
}

fn jennifer(player_id: i32) {
    // 规则书[手]: 「将此卡放置在除[使用者]以外的一名玩家的[场地]」 -- C#
    // `H.PickTarget(c, H.Others(seat), ...)` then `H.PlaceFromPlay(c, r.index)`
    // (owner = the target, user = the player).
    // `H.PickTarget` = the seat prompt, then the `H.Target` gate (C#
    // `Target(c, r.index, t); res.index = t.yes ? t.index : -1`); a failed
    // target means `c.Effective = false` and no placement. A `redirect` hook
    // (EXIST) may move the hit, and the card lands on that seat instead.
    let targets = ctx::others(player_id);
    if targets.is_empty() {
        return;
    }
    let target = ctx::ask_player(
        player_id,
        &Msg::new(key!("jennifer_title")),
        &Msg::new(key!("jennifer_ask")),
        &targets,
    );
    let hit = match ctx::target(target) {
        Some(h) => h,
        None => return,
    };
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(hit, "PP:找回珍妮弗", &Msg::new(key!("jennifer_note")).player_id("who", player_id));
    // C# `Card.User` -- the player who played it, for the [持续] below.
    ctx::set_slot(hit, SLOT_USER, player_id);
}

/// C# `CardJennifer.PassTile` -- the owner walks past 「偶像经纪公司」.
fn pass_tile(player_id: i32) {
    if trigger::kind() != TriggerKind::PassTile || !ctx::is_placed(player_id) {
        return;
    }
    // 规则书[持续]: 「[拥有者][经过]“偶像经纪公司”时」
    if trigger::player_id() != player_id {
        return;
    }
    let agency = ctx::tile_named("偶像经纪公司");
    if agency < 0 || trigger::tile() != agency {
        return;
    }
    let user = ctx::slot(player_id, SLOT_USER);
    // 规则书[持续]3: 「此卡[移除]」 -- C# `H.Unplace(this, "gone", ...)`.
    ctx::unplace_card(player_id);
    ctx::set_slot(player_id, SLOT_USER, -1);
    // 规则书[持续]1: 「[使用者][支付][拥有者]400资金」
    if user >= 0 && !ctx::player_out(user) {
        ctx::transfer(user, player_id, 400, &Msg::new(key!("jennifer_pay")));
    }
    // 规则书[持续]2: 「[使用者]获得1个正面的[P✽P粉丝]」 -- C# `H.GainFans(user, 1, up: true)`.
    if user >= 0 && !ctx::player_out(user) {
        ctx::add_tok(user, "P✽P粉丝(正)", 1, i32::MAX);
    }
    // 规则书[持续]2: 「[拥有者]将一个[P✽P粉丝]变为正面」 -- C# `H.FlipUp(owner, 1)`.
    let down = ctx::tok(player_id, "P✽P粉丝(反)");
    let flip = down.min(1);
    if flip > 0 {
        ctx::add_tok(player_id, "P✽P粉丝(反)", -flip, i32::MAX);
        ctx::add_tok(player_id, "P✽P粉丝(正)", flip, i32::MAX);
        let overflow = 1 - flip;
        if overflow > 0 && ctx::in_band(player_id, "Pastel✽Palettes") {
            ctx::add_band_crystals(player_id, overflow, 10);
        }
    }
    // 规则书[持续]3: 「将1张“魔法战队Pastel✽Ranger”加入[使用者]手卡」
    if user >= 0 && !ctx::player_out(user) {
        ctx::add_to_hand(user, "PP:[衍生]魔法战队Pastel✽Ranger");
        ctx::log(
            user,
            &Msg::new(key!("jennifer_ranger")).player_id("who", user),
        );
    }
}
// C# `Targeting => true; SingleTarget => true` -- `SingleTarget` is expressed
// by routing the pick through `ctx::target` (single-target, so a `redirect`
// hook may move the hit). `CardDef.targeting()` / `Normal` are not declared on
// the manifest yet; they only feed PLEASE CHOOSE's hand filter.