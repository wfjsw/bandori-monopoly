use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"美竹兰"#)
    .display(r#"美竹兰"#)
    .group(r#"Afterglow"#)
    .color(r#"#EE0022"#)
    .skill(r#"叛逆的红挑染"#)
    .text(r#"（1）每三回合没有使用Afterglow角色的（2）技能获得一个[火罐]（初始1，上限1）（2）投掷移动步数前可使用一个[火罐]选择本次移动向后，并且向后移动[经过]CiRCLE时不获得CiRCLE奖励。"#)
    .cn_id(r#"006"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"像往常一样"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
