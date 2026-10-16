use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"椎名立希（CRYCHIC）"#)
    .display(r#"椎名立希（CRYCHIC）"#)
    .group(r#"CRYCHIC"#)
    .color(r#"#7777AA"#)
    .skill(r#"克服劣等感"#)
    .text(r#"每当你的移动掷骰结果严格小于你以外玩家中上一次进行的移动掷骰结果且那次移动掷骰结果不大于20，若你本回合未使用技能，你下回合可在移动前重骰一次并选择第二次的结果使用。"#)
    .cn_id(r#"040"#)
    .art(r#"040c"#)
    .costume(r#"040_live_event_235_sr"#)
    .exclusive_cards(&[r#"即便比不上..."#])
    .preset(&[])
    .variant_of(r#"椎名立希"#)
    .variant_tag(r#"c"#);
