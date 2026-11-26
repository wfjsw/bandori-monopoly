use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"千早爱音"#)
    .display(r#"千早爱音"#)
    .latin(r#"Chihaya Anon"#)
    .group(r#"MyGO!!!!!"#)
    .color(r#"#FF8899"#)
    .skill(r#"重新开始"#)
    .text(r#"[资源]：
（1）[火罐]：初始2，上限3。
[自动]：
（1）自己每次[经过][CiRCLE]时：获得1个[火罐]。
[主动]：
（1）自己回合结束时自己的[停留]减少层数时：可使用2个[火罐]，自己[抽卡]1张。
（2）自己[传送]后：可使用2个[火罐]，自己[抽卡]1张。"#)
    .cn_id(r#"037"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"Anon Tokyo"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
