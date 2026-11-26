use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"长崎素世（CRYCHIC）"#)
    .display(r#"长崎素世（CRYCHIC）"#)
    .latin(r#"Nagasaki Soyo"#)
    .group(r#"CRYCHIC"#)
    .color(r#"#FFDD88"#)
    .skill(r#"雨中祈晴"#)
    .text(r#"[主动]：
（1）其他玩家在自己拥有的格子上[结算]时：可取消那次[支付]，使自己的下回合[主要移动]改为[传送]至触发此技能的格子。"#)
    .cn_id(r#"039"#)
    .art(r#"039c"#)
    .costume(r#"039_live_event_235_ur"#)
    .exclusive_cards(&[r#"回到曾经"#])
    .preset(&[])
    .variant_of(r#"长崎素世"#)
    .variant_tag(r#"c"#);
