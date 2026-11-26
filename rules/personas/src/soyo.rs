use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"长崎素世"#)
    .display(r#"长崎素世"#)
    .latin(r#"Nagasaki Soyo"#)
    .group(r#"MyGO!!!!!"#)
    .color(r#"#FFDD88"#)
    .skill(r#"通透的颜色"#)
    .text(r#"[资源]：
（1）[火罐]：初始1，上限1。
[自动]：
（1）自己每次[经过][CiRCLE]时：获得1个[火罐]。
[主动]：
（1）自己的移动[投掷]后：可使用1个[火罐]，使自己本回合的[移动终点]对自己视为对应颜色的[地产商]格子，持续至自己的下回合开始。若因此效果单次免除了至少1500资金的[支付]，则自己下次[经过]CiRCLE时不获得[火罐]。
获得1层“[移动终点]对自己视为对应颜色的[地产商]格子，如果本次移动进行了[结算]且原本[结算]流程包括分摊前需[支付]至少1500资金则获得一层[素世无火]状态。自己回合开始前减少1层。”的[素世]状态。（[素世无火]是“无视“通透的颜色”的[自动]（1）。自己[经过]CiRCLE时减少所有层数。”）"#)
    .cn_id(r#"039"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"混合的颜色"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
