use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"椎名立希（CRYCHIC）"#)
    .display(r#"椎名立希（CRYCHIC）"#)
    .latin(r#"Shiina Taki"#)
    .group(r#"CRYCHIC"#)
    .color(r#"#7777AA"#)
    .skill(r#"克服劣等感"#)
    .text(r#"[主动]：
（1）用于确定自己移动步数的[投掷]结果，严格小于其他玩家中上一次用于确定移动步数的[投掷]结果，且那次结果不大于20、自己本回合未使用技能时，获得一层[劣等感]状态
[劣等感]状态：在进行确定自己移动步数的[投掷]后可重新[投掷]1次，并使用第二次的结果，然后移除该状态。"#)
    .cn_id(r#"040"#)
    .art(r#"040c"#)
    .costume(r#"040_live_event_235_sr"#)
    .exclusive_cards(&[r#"即便比不上..."#])
    .preset(&[])
    .variant_of(r#"椎名立希"#)
    .variant_tag(r#"c"#);
