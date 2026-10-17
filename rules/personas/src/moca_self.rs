use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"青叶摩卡"#)
    .display(r#"青叶摩卡"#)
    .group(r#"Afterglow"#)
    .color(r#"#00CCAA"#)
    .skill(r#"我行我素"#)
    .text(r#"（1）每四回合没有使用Afterglow角色的（2）技能获得一个[火罐]（初始1，上限1）（2）当他人使用角色技能或打出手牌时，你可以消耗一个[火罐]传送到其所在格子（不触发结算且该传送本身不引发其他被动效果（如玩家重叠））"#)
    .cn_id(r#"007"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"0.5倍速"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
