use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"牛込里美"#)
    .display(r#"牛込里美"#)
    .group(r#"Poppin' Party"#)
    .color(r#"#FF55BB"#)
    .skill(r#"里美的决心"#)
    .text(r#"（1）你的回合中[经过]任意Live House颜色格子后在回合结束时获得1个[火罐]（初始1，上限3）（2）运营阶段可选择使用3个[火罐]立刻进入移动阶段，[传送]至任意与你绝对距离最远的地产商并[结算]且可选择盖房"#)
    .cn_id(r#"003"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"我的心就像巧克力螺"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
