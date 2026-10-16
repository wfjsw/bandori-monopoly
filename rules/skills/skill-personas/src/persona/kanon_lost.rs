use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"松原花音"#)
    .display(r#"松原花音"#)
    .group(r#"Hello, Happy World!"#)
    .color(r#"#44DDFF"#)
    .skill(r#"真正的迷子"#)
    .text(r#"(1) 游戏开始起点为#30弦卷豪宅 (2) 移动阶段进行两次移动掷骰并将它们的结果相减作为你的移动格数 (3) 如果三个回合内没有拿过Circle奖励，则下回合移动投掷改为1d20+1d4"#)
    .cn_id(r#"014"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"Wacha Mocha 啪嗒进行曲"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
