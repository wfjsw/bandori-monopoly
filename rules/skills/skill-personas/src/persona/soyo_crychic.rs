use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"长崎素世（CRYCHIC）"#)
    .display(r#"长崎素世（CRYCHIC）"#)
    .group(r#"CRYCHIC"#)
    .color(r#"#FFDD88"#)
    .skill(r#"雨中祈晴"#)
    .text(r#"当其他玩家在属于你的格子上触发结算时，你可取消那次支付，使你下回合的主要移动变为传送至触发此技能的那格。"#)
    .cn_id(r#"039"#)
    .art(r#"039c"#)
    .costume(r#"039_live_event_235_ur"#)
    .exclusive_cards(&[r#"回到曾经"#])
    .preset(&[])
    .variant_of(r#"长崎素世"#)
    .variant_tag(r#"c"#);
