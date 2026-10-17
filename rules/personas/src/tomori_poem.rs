use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"高松灯"#)
    .display(r#"高松灯"#)
    .group(r#"MyGO!!!!!"#)
    .color(r#"#77BBDD"#)
    .skill(r#"诗超绊"#)
    .text(r#"（1）每次[经过]任意RiNG时获得一个[火罐]（初始4，上限4）（2）当其他玩家的[移动终点]位于您前后5格内时可以选择使用4个火罐，在该玩家[触发结算]前强制将该玩家[传送]至自己所在的格子并[触发结算]，然后使其投掷1d20，若出目小于等于6则获得一层[停留]并在自己的下回合结束时[触发结算]；在此技能影响下的支付价格变为原本的一半。（3）当你的[移动终点]位于其他玩家的前后五格内时可以选择使用1个火罐，在[触发结算]前将自己[传送]至那名玩家所在的格子并[触发结算]。"#)
    .cn_id(r#"036"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"不再迷茫"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
