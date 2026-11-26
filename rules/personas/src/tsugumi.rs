use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"羽泽鸫"#)
    .display(r#"羽泽鸫"#)
    .latin(r#"Hazawa Tsugumi"#)
    .group(r#"Afterglow"#)
    .color(r#"#FFEE88"#)
    .skill(r#"伟大的平凡"#)
    .text(r#"Band: Afterglow. Color: `#FFEE88`.
[资源]：
（1）[火罐]：初始1，上限1。
[自动]：
（1）自己每四回合没有使用Afterglow角色的[主动]（1）技能时：获得1个[火罐]。
[主动]：
（1）自己进行[主要移动]时：可使用1个[火罐]，使本次移动反向。若如此做，在自己的下回合开始时进行1次双倍[投掷]的移动。
因该技能的效果反向移动[经过]CiRCLE时，不获得[CiRCLE奖励]。"#)
    .cn_id(r#"010"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"微小的『能做到』的事"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
