use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"高松灯（CRYCHIC）"#)
    .display(r#"高松灯（CRYCHIC）"#)
    .latin(r#"Takamatsu Tomori"#)
    .group(r#"CRYCHIC"#)
    .color(r#"#77BBDD"#)
    .skill(r#"跌跌撞撞..."#)
    .text(r#"[资源]：
（1）[火罐]：初始1，上限1。
[自动]：
（1）自己每次[经过]CiRCLE时：获得1个[火罐]。
（2）用于确定自己移动步数的[投掷]结果小于等于6时：立即获得1层[停留]，并在回合结束时[结算]。
[主动]：
（1）自己的回合内任意时刻：可使用1个[火罐]，移除自己1层[停留]。"#)
    .cn_id(r#"036"#)
    .art(r#"036c"#)
    .costume(r#"036_live_event_235_ur"#)
    .exclusive_cards(&[r#"内心的呐喊"#])
    .preset(&[])
    .variant_of(r#"高松灯"#)
    .variant_tag(r#"c"#);
