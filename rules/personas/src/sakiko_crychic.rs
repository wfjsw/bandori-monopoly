use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"丰川祥子（CRYCHIC）"#)
    .display(r#"丰川祥子（CRYCHIC）"#)
    .latin(r#"Toyokawa Sakiko"#)
    .group(r#"CRYCHIC"#)
    .color(r#"#7799CC"#)
    .skill(r#"你愿意和我组建乐队吗？"#)
    .text(r#"[资源]：
（1）[火罐]：初始1，上限2。
[自动]：
（1）自己每次[经过]CiRCLE时：获得1个[火罐]。
（2）其他玩家的[主要移动]结束时、[结算]前，且那名玩家位于自己前后5格内：那名玩家可选择[支付]自己300资金，使其[移动终点]沿最短路径向自己靠近1格。
（3）自己的乐队技能为“Ave Mujica”时：自己与其他玩家的距离在5格以内则处于[人偶模式]，但只有洗牌时才能为自己的乐队技能卡添加[奇迹水晶]。
[主动]：
（1）自己的[主要移动]结束时、[结算]前，若自己前后5格内有其他玩家：可使用1个[火罐]，选择其中1名玩家，使自己的[移动终点]沿最短路径向其靠近1格。"#)
    .cn_id("")
    .art(r#"341c"#)
    .costume(r#"341_jh_school_winter-2023"#)
    .exclusive_cards(&[r#"带领着大家"#])
    .preset(&[])
    .variant_of(r#"丰川祥子"#)
    .variant_tag(r#"c"#);
