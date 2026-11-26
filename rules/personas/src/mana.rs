use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"纯田真奈"#)
    .display(r#"纯田真奈"#)
    .latin(r#"Sumida Mana"#)
    .group(r#"Sumimi"#)
    .color(r#"#CF9E34"#)
    .skill(r#"甜甜圈爱好者"#)
    .text(r#"[资源]：
（1）[火罐]：初始1，上限2。
[自动]：
（1）自己每次[经过]CiRCLE时：获得1个[火罐]。
[主动]：
（1）自己[经过]“购物中心”、“便利店”、“快餐店”、“羽泽咖啡厅”或“山吹面包房”时：可使用1个[火罐]，依次进行以下操作：
1. 向对应格子进行1次[支付]。可向已抵押的格子正常[支付]其原本收费；若为无主格子，改为[消耗]对应收费金额。
2. 获得一层[甜甜圈]状态。
[甜甜圈]状态：自己的[消耗]与[支付]资金减半。在获得该状态后的下个回合结束时移除该状态。"#)
    .cn_id("")
    .art(r#"229"#)
    .costume(r#"229_sumimi"#)
    .exclusive_cards(&[r#"歌唱大赛5连冠"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
