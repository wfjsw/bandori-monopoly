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
//! another player's field; when the owner passes 偶像经纪公司 the user pays 400, the
//! user gains a positive P✽P fan, the owner flips one up, the card is removed and
//! a Pastel✽Ranger goes to the user's hand. The steps have no exit condition: a
//! user bankrupted by the 400 still gets the fan and the Ranger.

use bandori_std::ctx;
use card_sdk::abi::ActionKind;
use card_sdk::{key, Action, CardDef, Msg, On};
use card_sdk::{para, sec, steps, CardText};
use skill_groups::fans;

pub const JENNIFER: CardDef = CardDef::new(
    "PP:找回珍妮弗",
    &[
        On::Play("", Some(cant_play), jennifer),
        // 规则书[持续]: 「[拥有者][经过]“偶像经纪公司”时」
        On::Action(Action::PassTile(
            "card.placed && actor == owner && tile.id == tile_named('偶像经纪公司')",
            None,
            pass_tile,
        )),
    ],
)
.meta(
    &card_sdk::meta::CardMeta::new()
        .name(r#"找回珍妮弗"#)
        .raw_name(r#"找回珍妮弗"#)
        .tags(&[r#"手"#, r#"持续"#])
        .text(CardText::of(&[
            sec(
                r#"[手]："#,
                &[para(r#"将此卡放置在除[使用者]以外的一名玩家的[场地]。"#)],
            ),
            sec(
                r#"[持续]："#,
                &[
                    para(r#"[拥有者][经过]“偶像经纪公司”时依次进行以下操作："#),
                    steps(&[
                        r#"[使用者][支付][拥有者]400资金；"#,
                        r#"[使用者]获得1个正面的[P✽P粉丝]，[拥有者]将一个[P✽P粉丝]变为正面；"#,
                        r#"此卡[移除]，然后将1张“魔法战队Pastel✽Ranger”加入[使用者]手卡。"#,
                    ]),
                ],
            ),
        ]))
        // sheet cell D13
        .fields(&[("group", r#"Pastel✽Palettes"#), ("owner", "")]),
);

/// `fever.rs`'s `fever.user` shape -- per instance, so copies do not collide.
const PROP_USER: &str = "jennifer_user";

fn cant_play(player_id: i32) -> Option<Msg> {
    // TODO(规则书)（将此卡放置在除[使用者]以外的一名玩家的[场地]）: 规则书[手]: 「将此卡放置在除[使用者]以外的一名玩家的[场地]」 --
    // nobody to host it otherwise. 书中无此打出限制， 已出局的玩家也算「一名玩家」 --
    // 现在无人在局时整张卡被拒（others_count 滤掉出局者）
    if ctx::others_count(player_id) == 0 {
        return Some(Msg::new(key!("jennifer_blocked")));
    }
    None
}

fn jennifer(player_id: i32) -> card_sdk::Asked {
    // A `redirect` hook (EXIST) may move the hit, and the card lands on that seat
    // instead.
    let targets = ctx::others(player_id);
    if targets.is_empty() {
        return Ok(());
    }
    let target = ctx::ask_player(
        player_id,
        &Msg::new(key!("jennifer_title")),
        &Msg::new(key!("jennifer_ask")).card("card", "PP:找回珍妮弗"),
        &targets,
    )?;
    let hit = match ctx::target(target) {
        Some(h) => h,
        None => return Ok(()),
    };
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(
        hit,
        "PP:找回珍妮弗",
        &Msg::new(key!("jennifer_note")).player_id("who", player_id),
    );
    ctx::set_prop(PROP_USER, player_id + 1);
    Ok(())
}

fn pass_tile(player_id: i32) -> card_sdk::Asked {
    // `card.placed && actor == owner && tile.id == tile_named('偶像经纪公司')`
    // is the condition.
    //
    // 规则书[持续]: 「依次进行以下操作」 -- the three steps run in printed order.
    // 1 pay, 2 fans, 3 [移除] *then* the Pastel Ranger. The card is removed in
    // step 3, after the money and the fans, not before them.
    let user = ctx::prop(PROP_USER) - 1;
    // 规则书[持续]: 「依次进行以下操作」 -- the three steps have no exit
    // condition, so a user already out (or bankrupted by step 1's 400) still
    // gets steps 2 and 3. Only a missing user prop (`< 0`) skips them.
    // 1. 「[使用者][支付][拥有者]400资金」
    if user >= 0 {
        ctx::transfer(user, player_id, 400, &Msg::new(key!("jennifer_pay")))?;
    }
    // 2.
    if user >= 0 {
        let _ = fans::grant(user, 1, 0)?;
    }
    // 2. X = 1; the band card applies clause （2） when 1 > the owner's face-downs.
    let _ = fans::flip_up(player_id, 1)?;
    // 3.
    ctx::send_to_dest(ctx::Dest::Banished, card_sdk::abi::ZoneChangeCause::Effect);
    if user >= 0 {
        ctx::add_to_dest(
            user,
            "PP:[衍生]魔法战队Pastel✽Ranger",
            ctx::Dest::Hand,
            false,
        );
        ctx::log(
            user,
            &Msg::new(key!("jennifer_ranger")).player_id("who", user),
        );
    }
    Ok(())
}
// `CardDef.targeting()` / `Normal` are not declared on the manifest yet; they
// only feed PLEASE CHOOSE's hand filter.

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn jennifer_kani(player_id: i32) -> card_sdk::Asked {
    jennifer(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn pass_tile_kani(player_id: i32) -> card_sdk::Asked {
    pass_tile(player_id)
}
