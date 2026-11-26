use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"冰川纱夜"#)
    .display(r#"冰川纱夜"#)
    .latin(r#"Hikawa Sayo"#)
    .group(r#"Roselia"#)
    .color(r#"#00AABB"#)
    .skill(r#"踏上荆棘之路的觉悟"#)
    .text(r#"[资源]：
（1）[火罐]：初始10，上限10。
[自动]：
（1）自己每次[经过]CiRCLE时：获得2个[火罐]。自己每次被其他玩家[经过]时，获得1个[火罐]。
（2）第一个玩家被淘汰后：自己[经过]CiRCLE时获得的[火罐]数量增加1。
[主动]：
（1）自己进行确定[主要移动]步数的[投掷]后：可使用6个[火罐]，将本次非[传送]的[主要移动]增加1格或2格。每回合限1次。"#)
    .cn_id(r#"022"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"弹奏弹奏弹奏，继续弹奏"#])
    .preset(&[r#"R:（纱夜）弹奏弹奏弹奏，继续弹奏"#, r#"R:学生会的检查"#, r#"R:曲奇时间"#, r#"R:NFO"#, r#"R:live前的准备"#, r#"R:蓝玫瑰的骄傲"#, r#"R:Sprechchor"#, r#"通用:登上武道馆"#, r#"通用:网络链接异常"#, r#"通用:尽力后的收获"#])
    .variant_of("")
    .variant_tag("");
