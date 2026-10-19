use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"若叶睦（CRYCHIC）"#)
    .display(r#"若叶睦（CRYCHIC）"#)
    .group(r#"CRYCHIC"#)
    .color(r#"#779977"#)
    .skill(r#"精致的人偶"#)
    .text(r#"（1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）（2）开局时指定一名其他玩家，你的移动阶段开始时，可消耗1火罐，使本次的移动掷骰结果为那名玩家上一次的移动掷骰结果；那名玩家破产时，可重新指定一名玩家。（3）乐队技能为Ave Mujica时，任意时刻手牌大于等于3时进入状态2，严格小于3时进入状态1，但只有洗牌时可以为你的乐队技能卡上添加奇迹水晶。"#)
    .cn_id("")
    .art(r#"338c"#)
    .costume(r#"338_school_winter-2023"#)
    .exclusive_cards(&[r#"从没有觉得..."#])
    .preset(&[])
    .variant_of(r#"若叶睦"#)
    .variant_tag(r#"c"#);
