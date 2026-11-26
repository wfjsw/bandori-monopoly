use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"凑友希那"#)
    .display(r#"凑友希那"#)
    .latin(r#"Minato Yukina"#)
    .group(r#"Roselia"#)
    .color(r#"#881188"#)
    .skill(r#"来练习吧"#)
    .text(r#"[资源]：
（1）[火罐]：初始0，上限1。
[自动]：
（1）自己初始获得“RiNG 4”格子，并从“RiNG 4”开始游戏。自己首次[经过]CiRCLE时，不获得[CiRCLE奖励]。
（2）自己的[移动终点]为任意RiNG时：获得1个[火罐]。自己不位于RiNG时，失去所有[火罐]。
[主动]：
（1）自己移动中[经过]首个任意RiNG格子时：可选择使自己停止移动。
（2）其他玩家移动[经过]自己时：可使用1个[火罐]，令那名玩家[强制停下]并[结算]。"#)
    .cn_id(r#"021"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"louder"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
