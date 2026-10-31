use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"鳰原令王那"#)
    .display(r#"鳰原令王那"#)
    .group(r#"RAISE A SUILEN"#)
    .color(r#"#FF99BB"#)
    .skill(r#"梦幻可爱♪女仆"#)
    .text(r#"（1）游戏开始时如果场上有“Pastel✽Palettes”角色则额外获得1个正面[P✽P粉丝]（2）你拥有格子上的房屋总数增加时可选择失去1PAREO标记（初始1，上限3），所有非自己的玩家分摊支付你“格子的房屋造价×格子上的房屋数”四分之一的资金（所有你拥有的格子中取最高值）"#)
    .cn_id(r#"034"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"渐渐远去的你"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
