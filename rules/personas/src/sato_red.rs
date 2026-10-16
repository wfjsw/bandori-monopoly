use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"佐藤益木"#)
    .display(r#"佐藤益木"#)
    .group(r#"RAISE A SUILEN"#)
    .color(r#"#EEBB44"#)
    .skill(r#"与燃烧的红色一起驰骋"#)
    .text(r#"（1）每次[经过]“白雪学园”或“银河拉面馆”时获得1个[火罐]（初始0，上限2）（2）移动阶段前可使用X个[火罐]，本回合主要移动掷骰额外添加Xd10"#)
    .cn_id(r#"033"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"CRUSH ON THE DRUM!!!"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
