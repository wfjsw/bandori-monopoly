use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"桐谷透子"#)
    .display(r#"桐谷透子"#)
    .group(r#"Morfonica"#)
    .color(r#"#EE6666"#)
    .skill(r#"天上天下，唯我独尊"#)
    .text(r#"（1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）（2）一回合一次，在你的经营阶段，你可以消耗一个火罐并指定你的一个地块，指定前后各一格范围内（不包括该格子本身）的所有其他玩家支付你X，X为此地的地租的一半/指定玩家数（向上取整百）。"#)
    .cn_id(r#"027"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"（未命名）"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
