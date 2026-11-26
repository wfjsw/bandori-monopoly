use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"月岛麻里奈"#)
    .display(r#"月岛麻里奈"#)
    .latin(r#"Tsukishima Marina"#)
    .group(r#"CiRCLE"#)
    .color(r#"#7E6A69"#)
    .skill(r#"礼物还有好多好多哟"#)
    .text(r#"[自动]：
（1）其他玩家[经过]CiRCLE时：那名玩家可[支付]自己1次500资金。若如此做，依次进行以下操作：
1. 自己[投掷]1d10。
2. 若结果至少为6，那名玩家[获得]1200资金。"#)
    .cn_id("")
    .art(r#"201"#)
    .costume(r#"201_casual"#)
    .exclusive_cards(&[r#"今天也要加油工作喔"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
