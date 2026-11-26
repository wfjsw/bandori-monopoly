use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"若叶睦（CRYCHIC）"#)
    .display(r#"若叶睦（CRYCHIC）"#)
    .latin(r#"Wakaba Mutsumi"#)
    .group(r#"CRYCHIC"#)
    .color(r#"#779977"#)
    .skill(r#"精致的人偶"#)
    .text(r#"[资源]：
（1）[火罐]：初始1，上限1。
[自动]：
（1）自己每次[经过]CiRCLE时：获得1个[火罐]。
（2）游戏开始时：自己[指定]1名其他玩家。那名玩家[破产]时，自己可重新[指定]1名玩家。
（3）自己的乐队技能为“Ave Mujica”时：回合结束时手牌数至少为3则进入[人偶模式]，严格小于3则退出[人偶模式]；只有洗牌时才能为自己的乐队技能卡添加[奇迹水晶]。
[主动]：
（1）自己的移动阶段开始时：可使用1个[火罐]，使此次用于确定移动步数的[投掷]结果，等于此角色[自动]（2）[指定]玩家上一次用于确定移动步数的[投掷]结果。"#)
    .cn_id("")
    .art(r#"338c"#)
    .costume(r#"338_school_winter-2023"#)
    .exclusive_cards(&[r#"从没有觉得..."#])
    .preset(&[])
    .variant_of(r#"若叶睦"#)
    .variant_tag(r#"c"#);
