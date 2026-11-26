use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"青叶摩卡"#)
    .display(r#"青叶摩卡"#)
    .latin(r#"Aoba Moca"#)
    .group(r#"Afterglow"#)
    .color(r#"#00CCAA"#)
    .skill(r#"我行我素"#)
    .text(r#"Band: Afterglow. Color: `#00CCAA`.
[资源]：
（1）[火罐]：初始1，上限1。
[自动]：
（1）自己每四回合没有使用Afterglow角色的[主动]（1）技能时：获得1个[火罐]。
[主动]：
（1）其他玩家使用角色技能或打出手牌时：自己可使用1个[火罐]，[传送]到那名玩家所在的格子。此次[传送]不触发[结算]，且[传送]本身不引发其他被动效果（如玩家[重叠]）。"#)
    .cn_id(r#"007"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"0.5倍速"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
