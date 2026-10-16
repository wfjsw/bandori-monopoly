use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"今井莉莎"#)
    .display(r#"今井莉莎"#)
    .group(r#"Roselia"#)
    .color(r#"#DD2200"#)
    .skill(r#"慈爱女神"#)
    .text(r#"（1）每次[经过]CiRCLE时获得1个[火罐]（初始2，上限2）（2）移动阶段前，你可以消耗一个火罐，将此次移动变为传送到距离你正向距离最近的角色所在的格子，触发结算。本次传送触发结算前，你可以额外消耗一个火罐或使与你在同一个格子上的另一个角色获得一个可超过上限的临时火罐，若你这样做，此次传送不触发任何结算。（3）你提供的临时火罐将会在两回合后失去，使用你提供的临时火罐需要支付你600资金（被冲榜类效果被动消耗无需支付）。"#)
    .cn_id(r#"023"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"必然的联系"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
