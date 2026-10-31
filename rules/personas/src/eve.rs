use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"若宫伊芙"#)
    .display(r#"若宫伊芙"#)
    .group(r#"Pastel✽Palettes"#)
    .color(r#"#DDBBFF"#)
    .skill(r#"天下统一"#)
    .text(r#"（1）游戏开始后获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得若宫伊芙的（2）技能
（2）当你因任意原因投掷前可选择将自己Y个正面[P✽P粉丝]变反并为此次投掷结果增加Yd4；如果自己是Pastel✽Palettes角色则将Y个其他乐队玩家拥有的反面[P✽P粉丝]变正，否则将所有Pastel✽Palettes角色的1个反面[P✽P粉丝]变正"#)
    .cn_id(r#"020"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"属于我的武士道！"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
