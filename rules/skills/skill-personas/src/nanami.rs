//! `skill:广町七深:这是很普通的事吧？`
//!
//! 规则书（skill sheet, 广町七深）:
//! > （1）每次[经过]CiRCLE时获得一个[火罐]（初始2，上限2）
//! > （2）当有玩家在使用角色技能时，你可以支付一个火罐得到一个该角色的标记（当场上
//! > 存活玩家数量大于等于3时：每种角色标记上限1，该技能对被动技能存在3回合CD）
//! > （3）在时机合适时候，你可以使用手中的角色标记，视为使用该角色的技能（尽力补齐
//! > 资源消耗，比如双[火罐]消耗技能和翻转[P✽P粉丝]）。
//!
//! （2） 「该角色的标记」 is a counter named for the character whose skill is
//! running -- which the `skillUsed` trigger carries as the skill's id, so the
//! mark name is `模仿标记:<skill id>` (the 「模仿标记」 namespace NNM also spends).
//! 「每种角色标记上限1」 is a per-kind cap, and
//! 「该技能对被动技能存在3回合CD」 is a per-kind cooldown.
//!
//! （3） 「视为使用该角色的技能」 is `ctx::play_card(skill_id, player_id)?`, the
//! same invocation haruhikage uses; the skill id is the mark name minus the
//! `模仿标记:` prefix (or the legacy `角色标记:`: replays and saves still hold
//! it, so every read accepts both).
//!
//! 「若该技能需要使用[火罐]，则使用自己拥有数量与所需数量中的较小值」 is a
//! fire **substitution**: only `min(held, needed)` of this player's own pots are
//! spent (pre-paid before the invocation, the shortfall bridged by providerless
//! temp pots so the invoked body's own charge lands), and every other resource
//! (money, [P✽P粉丝] flips) is left to the body in full. The needed count is
//! read from the table below; skills that choose X within the held count
//! (rinko / masking / umiri) need no substitution, so they map to 0.

use bandori_std::ctx::{self, state};
use card_sdk::abi::ActionKind;
use card_sdk::ctx::NameId;
use card_sdk::{key, Action, CardDef, Msg, On};

/// The 「模仿标记」 namespace every consumer agrees on (NNM's `模仿标记:*` too).
/// A mark's name is `模仿标记:<skill id>`, so (3) can hand the id to `play_card`.
const MARK_PREFIX: &str = "模仿标记:";

/// The pre-rename prefix. Replays and saves still hold `角色标记:*` marks, so
/// every read takes either prefix; writes always use [`MARK_PREFIX`].
const LEGACY_PREFIX: &str = "角色标记:";

/// The skill id a mark stands for, under the new prefix or the legacy one
/// (`模仿标记:<skill id>` / `角色标记:<skill id>` -> `<skill id>`).
fn mark_skill(name: NameId) -> &'static str {
    let s = name.as_str();
    s.strip_prefix(MARK_PREFIX)
        .or_else(|| s.strip_prefix(LEGACY_PREFIX))
        .unwrap_or(s)
}

/// Held mark names, both prefixes (new writing convention first, legacy
/// after), so a save made before the rename still counts.
fn held_marks(player_id: i32) -> alloc::vec::Vec<NameId> {
    let mut names = ctx::held_names(player_id, MARK_PREFIX);
    names.extend(ctx::held_names(player_id, LEGACY_PREFIX));
    names
}

/// Fire the named skill needs, for the （3） substitution. A **residual**: skills
/// charge fire inside their body and no cost metadata is declarable on a
/// persona sheet (`On::Play.cost` exists for held tokens, but the faces here
/// spend in-body), so the count is transcribed. `0` = the skill picks X within
/// the held count and needs no substitution (rinko / masking / umiri), or it
/// spends no fire at all.
fn fire_need(id: &str) -> i32 {
    match id {
        "skill:千早爱音:重新开始" | "skill:市谷有咲:盆栽爱好者" => 2,
        "skill:濑田薰:梦幻的王子殿下" => 7,
        "skill:冰川纱夜:踏上荆棘之路的觉悟" => 6,
        "skill:山吹沙绫:焕然一新的天空中" => 5,
        "skill:花园多惠:花园警察，出警！" => 4,
        "skill:要乐奈:投币式停车场的猫" | "skill:牛込里美:里美的决心" => 3,
        "skill:上原绯玛丽:大家一起迈出新的一步"
        | "skill:户山香澄:非凡之星"
        | "skill:和奏瑞依:一次又一次竭尽全力"
        | "skill:今井莉莎:慈爱女神"
        | "skill:纯田真奈:甜甜圈爱好者"
        | "skill:奥泽美咲:另一个我"
        | "skill:青叶摩卡:我行我素"
        | "skill:若叶睦（CRYCHIC）:精致的人偶"
        | "skill:美竹兰:叛逆的红挑染"
        | "skill:丰川祥子（CRYCHIC）:你愿意和我组建乐队吗？"
        | "skill:都筑诗船:尽力了吗"
        | "skill:长崎素世:通透的颜色"
        | "skill:椎名立希:决定练习日的会议"
        | "skill:桐谷透子:天上天下，唯我独尊"
        | "skill:宇田川巴:豚骨酱油拉面大姐"
        | "skill:高松灯（CRYCHIC）:跌跌撞撞..."
        | "skill:羽泽鸫:伟大的平凡"
        | "skill:三角初华（Sumimi）:成为偶像"
        | "skill:凑友希那:来练习吧" => 1,
        _ => 0,
    }
}

/// The mark name for a skill id.
fn mark_name(id: &str) -> alloc::string::String {
    alloc::format!("{MARK_PREFIX}{id}")
}

/// Per-kind cooldown, keyed `skill.nanamiOrdinary.cd:<skill id>`.
fn cd_key(id: &str) -> alloc::string::String {
    alloc::format!("skill.nanamiOrdinary.cd:{id}")
}

pub const NANAMI: CardDef = CardDef::new(
    "skill:广町七深:这是很普通的事吧？",
    &[
        On::Play("", Some(can_use), use_skill),
        On::Action(Action::TurnStartBefore("", None, declare_cap)).silent(),
        On::Action(Action::DeckAtGameStart("", None, declare_cap)).silent(),
        On::Action(Action::Pass(bandori_std::guards::OWN_START, None, on_pass)),
        // （2）'s applicability is the condition + residual: another player's
        // skill use, a skill id on the trigger, a free pot, and (at 3+ alive)
        // no mark of that kind yet. The CD is a dynamic per-skill state key
        // (`skill.nanamiOrdinary.cd:<skill id>`), so it stays the residual
        // (docs/GUARDS.md §4.2). A reject runs no body and so posts no flash.
        // The declared id is what 稍微努力了一下 (3) invokes for 「你的（2）技能」 --
        // `entry::PRESS` would be this skill's first Play, which is (3).
        On::Action(Action::CardUsed(
            "actor != owner && trigger_card != 0 && charge(owner) >= 1",
            Some(on_skill_applies),
            on_skill,
        ))
        .id("2"),
        On::Action(Action::TurnEnd(card_sdk::condition::MINE, None, tick)).quiet_when_empty(),
    ],
)
.rule()
// 「（初始2，上限2）」
.props(&[
    (bandori_std::fire::FIRE_INITIAL, 2),
    (bandori_std::fire::FIRE_CAP, 2),
]);

/// Residual guard for [`on_skill`]: the CD and the 「每种角色标记上限1」 cap.
/// Pure reads -- the same instant as the body (`run_hook` runs both on one
/// store). `actor != owner && trigger_card != 0 && charge(owner) >= 1` is the
/// condition.
fn on_skill_applies(player_id: i32) -> bool {
    let Some(id) = ctx::trigger::cards().into_iter().next() else {
        return false;
    };
    if id == "" {
        return false;
    }
    // 「该技能对被动技能存在3回合CD」 -- a per-kind cooldown.
    if state::get(player_id, &cd_key(id.as_str())) > 0 {
        return false;
    }
    // 「当场上存活玩家数量大于等于3时：每种角色标记上限1」
    let alive = (0..ctx::player_count())
        .filter(|&p| !ctx::player_out(p))
        .count() as i32;
    if alive >= 3 && ctx::held_count(player_id, &mark_name(id.as_str())) >= 1 {
        return false;
    }
    true
}

/// 「初始2，上限2」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id);
    Ok(())
}

/// （1）「每次[经过]CiRCLE时获得一个[火罐]」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    bandori_std::fire::gain(player_id, 1, &Msg::new(key!("nanami_gain")))?;
    Ok(())
}

/// （2）「当有玩家在使用角色技能时，你可以支付一个火罐得到一个该角色的标记」.
/// `actor != owner && trigger_card != 0 && charge(owner) >= 1` plus
/// [`on_skill_applies`] decide applicability; the body is the offer.
fn on_skill(player_id: i32) -> card_sdk::Asked {
    let Some(id) = ctx::trigger::cards().into_iter().next() else {
        debug_assert!(
            false,
            "on_skill_applies admitted but the trigger has no skill id"
        );
        return Ok(());
    };
    if id == "" {
        debug_assert!(false, "on_skill_applies admitted but the skill id is empty");
        return Ok(());
    }
    let cd = cd_key(id.as_str());
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("nanami_title")),
        &Msg::new(key!("nanami_ask")),
    )? {
        return Ok(());
    }
    if !bandori_std::fire::spend_skill(player_id, 1, &Msg::new(key!("nanami_spend")))? {
        return Ok(());
    }
    ctx::add_held_by_name(player_id, &mark_name(id.as_str()), 1, i32::MAX)?;
    state::set(player_id, &cd, 3);
    ctx::log(
        player_id,
        &Msg::new(key!("nanami_mark")).card("card", id.as_str()),
    );
    Ok(())
}

/// 「3回合CD」 counts down.
fn tick(player_id: i32) -> card_sdk::Asked {
    for name in held_marks(player_id) {
        let cd = cd_key(mark_skill(name));
        let left = state::get(player_id, &cd);
        if left > 0 {
            state::set(player_id, &cd, left - 1);
            ctx::log(
                player_id,
                &Msg::new(key!("nanami_cd")).i("n", (left - 1) as i64),
            );
        }
    }
    Ok(())
}

/// （3） 「你可以使用手中的角色标记，视为使用该角色的技能」.
fn can_use(player_id: i32) -> Option<Msg> {
    if bandori_std::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if held_marks(player_id).is_empty() {
        return Some(Msg::new(key!("nanami_no_mark")));
    }
    None
}

/// （3）「视为使用该角色的技能」 -- `ctx::play_card` runs that skill's `On::Play`.
fn use_skill(player_id: i32) -> card_sdk::Asked {
    let marks = held_marks(player_id);
    if marks.is_empty() {
        return Ok(());
    }
    let pick = ctx::ask_choice(
        player_id,
        &Msg::new(key!("nanami_title")),
        &Msg::new(key!("nanami_which")),
        &marks
            .iter()
            .map(|m| Msg::new(key!("nanami_option")).card("card", mark_skill(*m)))
            .collect::<alloc::vec::Vec<_>>(),
    )?;
    let Some(&mark) = marks.get(pick) else {
        return Ok(());
    };
    let id = mark_skill(mark);
    // 「若该技能需要使用[火罐]，则使用自己拥有数量与所需数量中的较小值」 --
    // pre-pay `min(held, needed)` of this player's own pots, then bridge the
    // invoked body's full charge with providerless temp pots so it draws the
    // bridge first (net own spend = the minimum). No `needed` (0) leaves the
    // body's charge untouched. Other resources are the body's, untouched here.
    //
    // The mark is removed *after* the play: a pause inside the invoked body
    // replays this body from the top, and an already-removed mark would empty
    // `marks` and return early. On the replay the pre-pay and play are recorded
    // answers, so nothing is charged twice.
    let needed = fire_need(id);
    if needed > 0 {
        let pre = bandori_std::fire::get(player_id).min(needed);
        if pre > 0
            && !bandori_std::fire::spend_skill(player_id, pre, &Msg::new(key!("nanami_spend")))?
        {
            return Ok(());
        }
        bandori_std::fire::temp::grant(player_id, needed);
    }
    let temp_before = bandori_std::fire::temp::get(player_id);
    ctx::play_card(id, player_id)?;
    // The body aborted before its charge (or a [奇迹水晶] substituted it): drop
    // the bridge we added, keeping any pots a provider already lent.
    let left = bandori_std::fire::temp::get(player_id) - (temp_before - needed);
    if needed > 0 && left > 0 {
        ctx::add_held_by_name(player_id, bandori_std::fire::TEMP_KEY, -left, i32::MAX)?;
    }
    ctx::add_held_by_name(player_id, mark.as_str(), -1, i32::MAX)?;
    ctx::log(player_id, &Msg::new(key!("nanami_used")).card("card", id));
    Ok(())
}

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn use_skill_kani(player_id: i32) -> card_sdk::Asked {
    use_skill(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn declare_cap_kani(player_id: i32) -> card_sdk::Asked {
    declare_cap(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pass_kani(player_id: i32) -> card_sdk::Asked {
    on_pass(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_skill_kani(player_id: i32) -> card_sdk::Asked {
    on_skill(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn tick_kani(player_id: i32) -> card_sdk::Asked {
    tick(player_id)
}
