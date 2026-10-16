use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"若叶睦"#)
    .display(r#"若叶睦"#)
    .group(r#"Ave Mujica"#)
    .color(r#"#779977"#)
    .skill(r#"天生的演员"#)
    .text(r#"状态1：你的每6回合不打出任何手牌，在回合开始前抽1张牌。若你的回合开始时手牌数为5，可选择进入状态2。状态2：进入该状态后，你从角色卡堆拿取一张上一位打出过手牌的玩家的角色卡将其覆盖于自己角色卡上并获得其初始火罐，直至退出状态2；若回合结束时你的手牌小于等于1，退出状态2。"#)
    .cn_id("")
    .art(r#"338"#)
    .costume(r#"338_event_297_story_01"#)
    .exclusive_cards(&[r#"表演的本能"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
