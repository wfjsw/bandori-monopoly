use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"广町七深"#)
    .display(r#"广町七深"#)
    .group(r#"Morfonica"#)
    .color(r#"#EE7744"#)
    .skill(r#"这是很普通的事吧？"#)
    .text(r#"（1）每次[经过]CiRCLE时获得一个[火罐]（初始2，上限2）（2）当有玩家在使用角色技能时，你可以支付一个火罐得到一个该角色的标记（当场上存活玩家数量大于等于3时：每种角色标记上限1，该技能对被动技能存在3回合CD）（3）在时机合适时候，你可以使用手中的角色标记，视为使用该角色的技能（尽力补齐资源消耗，比如双[火罐]消耗技能和翻转[P✽P粉丝]）。"#)
    .cn_id(r#"028"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"稍微努力了一下"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
