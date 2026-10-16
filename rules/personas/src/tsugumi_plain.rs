use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"羽泽鸫"#)
    .display(r#"羽泽鸫"#)
    .group(r#"Afterglow"#)
    .color(r#"#FFEE88"#)
    .skill(r#"伟大的平凡"#)
    .text(r#"（1）每三回合没有使用Afterglow角色的（2）技能获得一个[火罐]（初始1，上限1）（2）当你进行主要移动时，你可以失去一个[火罐]使本次移动反向，如果你这样做了，你的下回合开始时，进行一次双倍掷骰的移动，向后移动经过CiRCLE时不获得CiRCLE奖励。"#)
    .cn_id(r#"010"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"微小的『能做到』的事"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
