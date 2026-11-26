use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"佐藤益木"#)
    .display(r#"佐藤益木"#)
    .latin(r#"Sato Masuki"#)
    .group(r#"RAISE A SUILEN"#)
    .color(r#"#EEBB44"#)
    .skill(r#"与燃烧的红色一起驰骋"#)
    .text(r#"[资源]：
（1）[火罐]：初始0，上限3。
[自动]：
（1）自己每次领取完CiRCLE奖励后首次[经过]“白雪学园”或“银河拉面馆”之一时：获得1个[火罐]。
[主动]：
（1）自己的移动阶段前：可使用X个[火罐]，为本回合用于确定[主要移动]步数的[投掷]额外添加X个d20骰子。"#)
    .cn_id(r#"033"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"CRUSH ON THE DRUM!!!"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
