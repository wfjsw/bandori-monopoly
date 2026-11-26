use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"美竹兰"#)
    .display(r#"美竹兰"#)
    .latin(r#"Mitake Ran"#)
    .group(r#"Afterglow"#)
    .color(r#"#EE0022"#)
    .skill(r#"叛逆的红挑染"#)
    .text(r#"Band: Afterglow. Color: `#EE0022`.
[资源]：
（1）[火罐]：初始1，上限1。
[自动]：
（1）自己每四回合没有使用Afterglow角色的[主动]（1）技能时：获得1个[火罐]。
[主动]：
（1）自己进行用于确定本次移动步数的[投掷]前：可使用1个[火罐]，选择本次移动反向。因该技能的效果反向移动[经过]CiRCLE时，不获得[CiRCLE奖励]。"#)
    .cn_id(r#"006"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"像往常一样"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
