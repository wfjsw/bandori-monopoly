use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"鳰原令王那"#)
    .display(r#"鳰原令王那"#)
    .latin(r#"Nyubara Reona"#)
    .group(r#"RAISE A SUILEN"#)
    .color(r#"#FF99BB"#)
    .skill(r#"梦幻可爱♪女仆"#)
    .text(r#"[资源]：
（1）[PAREO标记]：初始1，上限3。
[自动]：
（1）游戏开始时，如果场上有Pastel✽Palettes角色：自己额外获得1个正面[P✽P粉丝]。
[主动]：
（1）自己拥有格子上的房屋总数增加时：可使用1个[PAREO标记]，所有其他玩家[分摊][支付]自己X资金。X为自己拥有的所有格子中“格子的房屋造价×格子上的房屋数”的最高值的四分之一。"#)
    .cn_id(r#"034"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"渐渐远去的你"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
