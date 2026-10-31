use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"丰川祥子（CRYCHIC）"#)
    .display(r#"丰川祥子（CRYCHIC）"#)
    .group(r#"CRYCHIC"#)
    .color(r#"#7799CC"#)
    .skill(r#"你愿意和我组建乐队吗？"#)
    .text(r#"（1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限2）（2）其他人的主要移动结束时，[触发结算]前若在你的前后5格内，可选择向你支付300资金并使自己的移动终点沿最短路径向你靠近1格。（3）你的主要移动结束时，[触发结算]前若前后五格内有别的玩家，你可消耗1火罐，选择其中一名玩家并使自己的移动终点沿最短路径向其靠近1格。（4）若你的乐队技能为“Ave Mujica”，你距离其他玩家5格以内时处于状态2，但只有洗牌时可以为你的乐队技能卡上添加奇迹水晶。"#)
    .cn_id("")
    .art(r#"341c"#)
    .costume(r#"341_jh_school_winter-2023"#)
    .exclusive_cards(&[r#"带领着大家"#])
    .preset(&[])
    .variant_of(r#"丰川祥子"#)
    .variant_tag(r#"c"#);
