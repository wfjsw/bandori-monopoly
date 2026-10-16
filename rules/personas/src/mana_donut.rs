use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"纯田真奈"#)
    .display(r#"纯田真奈"#)
    .group(r#"Sumimi"#)
    .color(r#"#CF9E34"#)
    .skill(r#"甜甜圈爱好者"#)
    .text(r#"（1）每次[经过]CiRCLE时获得1个[火罐]（初始1，上限2）（2）经过“购物中心”，“便利店”，“快餐店”，“羽泽咖啡厅”，“山吹面包房”时，在触发结算前可选择消耗一个火罐并向对应格子进行一次支付（可向抵押格子正常支付其原本收费，若为无主格子则为消耗对应收费资金），之后，直到你的下个回合结束，你的资金消耗减半。"#)
    .cn_id("")
    .art(r#"229"#)
    .costume(r#"229_sumimi"#)
    .exclusive_cards(&[r#"歌唱大赛5连冠"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
