use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"冰川纱夜"#)
    .display(r#"冰川纱夜"#)
    .group(r#"Roselia"#)
    .color(r#"#00AABB"#)
    .skill(r#"踏上荆棘之路的觉悟"#)
    .text(r#"（1）每次[经过]CiRCLE时获得2个[火罐]，每次被别的玩家[经过]时获得1个[火罐]（初始10，上限10）（2）一回合一次，你可以在进入移动阶段时消耗6个[火罐]将非[传送]的主要移动添加1或2格（3）第一个玩家被淘汰，经过Circle时获得的火罐加1"#)
    .cn_id(r#"022"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"弹奏弹奏弹奏，继续弹奏"#])
    .preset(&[r#"R:（纱夜）弹奏弹奏弹奏，继续弹奏"#, r#"R:学生会的检查"#, r#"R:曲奇时间"#, r#"R:NFO"#, r#"R:live前的准备"#, r#"R:蓝玫瑰的骄傲"#, r#"R:Sprechchor"#, r#"通用:登上武道馆"#, r#"通用:网络链接异常"#, r#"通用:尽力后的收获"#])
    .variant_of("")
    .variant_tag("");
