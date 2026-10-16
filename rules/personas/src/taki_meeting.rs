use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"椎名立希"#)
    .display(r#"椎名立希"#)
    .group(r#"MyGO!!!!!"#)
    .color(r#"#7777AA"#)
    .skill(r#"决定练习日的会议"#)
    .text(r#"（1）场上每有玩家获得一层[停留]时，你获得一个[火罐]（初始0，上限5）（2）运营阶段可消耗1个[火罐]为你场上的任何一张卡添加一个[奇迹水晶]，每回合限一次。"#)
    .cn_id(r#"040"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"想认真去做"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
