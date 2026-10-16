use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"高松灯（CRYCHIC）"#)
    .display(r#"高松灯（CRYCHIC）"#)
    .group(r#"CRYCHIC"#)
    .color(r#"#77BBDD"#)
    .skill(r#"跌跌撞撞..."#)
    .text(r#"（1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）（2）你回合内的任意时刻，可消耗1火罐清除自身一层[停留]（3）当你的移动掷骰小于等于6时，立刻获得一层[停留]并在回合结束时触发结算。"#)
    .cn_id(r#"036"#)
    .art(r#"036c"#)
    .costume(r#"036_live_event_235_ur"#)
    .exclusive_cards(&[r#"内心的呐喊"#])
    .preset(&[])
    .variant_of(r#"高松灯"#)
    .variant_tag(r#"c"#);
