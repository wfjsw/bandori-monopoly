use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"桐谷透子"#)
    .display(r#"桐谷透子"#)
    .latin(r#"Kirigaya Toko"#)
    .group(r#"Morfonica"#)
    .color(r#"#EE6666"#)
    .skill(r#"天上天下，唯我独尊"#)
    .text(r#"[资源]：
（1）[火罐]：初始1，上限1。
[自动]：
（1）自己每次[经过][CiRCLE]时：获得1个[火罐]。
[主动]：
（1）自己的经营阶段：可使用1个[火罐]，[指定]自己拥有的1个格子。该格子前后各1格范围内（不包括该格子本身）的所有其他玩家被[指定]，各[支付]自己X资金。X为该格子的地租的一半÷被[指定]玩家数量（向上取整百）。每回合限1次。"#)
    .cn_id(r#"027"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"（未命名）"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
