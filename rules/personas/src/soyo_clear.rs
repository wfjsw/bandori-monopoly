use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"长崎素世"#)
    .display(r#"长崎素世"#)
    .group(r#"MyGO!!!!!"#)
    .color(r#"#FFDD88"#)
    .skill(r#"通透的颜色"#)
    .text(r#"（1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）（2）移动掷骰后可消耗一个火罐，使自己本回合的[移动终点]对你视为对应颜色的地产商格子，持续至你的下回合开始；若以此法单次免除了至少1500资金的[支付]，则你下次经过CiRCLE时不获得火罐。"#)
    .cn_id(r#"039"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"混合的颜色"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
