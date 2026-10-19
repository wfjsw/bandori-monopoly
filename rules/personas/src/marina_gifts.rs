use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"月岛麻里奈"#)
    .display(r#"月岛麻里奈"#)
    .group(r#"CiRCLE"#)
    .color(r#"#7E6A69"#)
    .skill(r#"礼物还有好多好多哟"#)
    .text(r#"其他玩家经过CiRCLE时，可向你支付一次500资金，你投掷一次1d10，如果投掷结果至少为6，那名玩家获得1200资金。"#)
    .cn_id("")
    .art(r#"201"#)
    .costume(r#"201_casual"#)
    .exclusive_cards(&[r#"今天也要加油工作喔"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
