use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"凑友希那"#)
    .display(r#"凑友希那"#)
    .group(r#"Roselia"#)
    .color(r#"#881188"#)
    .skill(r#"来练习吧"#)
    .text(r#"（1）初始获得“RiNG 4”格子，从“RiNG 4”格子开始游戏，首次经过CiRCLE不获得经过奖励（2）如果移动时[经过]了RiNG，可以在[经过]的第一个RiNG停止移动（3）移动终点为任意“RiNG”时，获得一个火罐（上限1）。任何时刻当你不位于RiNG时，失去所有的火罐。当其他玩家移动[经过]您时，您可以选择使用一个[火罐]令该玩家强制停下并触发结算。"#)
    .cn_id(r#"021"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"louder"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
