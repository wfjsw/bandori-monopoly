use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"祐天寺若麦"#)
    .display(r#"祐天寺若麦"#)
    .group(r#"Ave Mujica"#)
    .color(r#"#AA4477"#)
    .skill(r#"大主播喵梦亲"#)
    .text(r#"状态1：（1）每次其他玩家向你支付资金时，你立即获得1火罐（初始0，上限5），并使那次支付的金额提高X*100，X为你拥有的火罐数。（2）任何时刻当你的火罐数到达上限时，你立即进入状态2。
状态2：（1）进入状态2时，你立即将场上全部正面朝上的手卡及事件卡翻为背面朝上，直到状态2结束为止，场上所有背面朝上的卡无法产生效果，且不受任何效果影响。你退出状态2后，被你翻面的卡自动翻回。"#)
    .cn_id("")
    .art(r#"340"#)
    .costume(r#"340_event_297_story_01"#)
    .exclusive_cards(&[r#"（未命名）"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
