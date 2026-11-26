use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"今井莉莎"#)
    .display(r#"今井莉莎"#)
    .latin(r#"Imai Lisa"#)
    .group(r#"Roselia"#)
    .color(r#"#DD2200"#)
    .skill(r#"慈爱女神"#)
    .text(r#"[资源]：
（1）[火罐]：初始2，上限2。
[自动]：
（1）自己每次[经过]CiRCLE时：获得1个[火罐]。
（2）自己提供的每个临时[火罐]，在提供后自己的第二次回合开始前失去。其他玩家使用自己提供的临时[火罐]时，需[支付]自己600资金；被“冲榜”类效果被动消耗时无需[支付]。
[主动]：
（1）自己的移动阶段前：可使用1个[火罐]，将此次移动改为[传送]到与自己[正向距离]最近的其他角色所在格子并[结算]（包括同格的其他角色；并列最近时由自己选择。同格[传送]不触发[经过]）。
此次[传送]的[结算]前，自己可选择依次进行以下操作：
- 额外使用1个[火罐]。
- 使与自己位于同一格子的另一名角色获得1个可超过上限的临时[火罐]。
若如此做，此次[传送]不触发任何[结算]。"#)
    .cn_id(r#"023"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"必然的联系"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
