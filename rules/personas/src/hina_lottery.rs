use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"冰川日菜"#)
    .display(r#"冰川日菜"#)
    .group(r#"Pastel✽Palettes"#)
    .color(r#"#55DDEE"#)
    .skill(r#"日菜抽中的大奖"#)
    .text(r#"（1）游戏开始后获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得冰川日菜的（2）技能
（2）每回合开始时必须进行1次投掷1d4，根据最后投掷的结果在下回合开始前获得以下角色的（2）技能：结果为1是丸山彩；结果为2是白鹭千圣；结果为3是若宫伊芙；结果为4是大和麻弥。"#)
    .cn_id(r#"017"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"会发出怎样的声音呢？"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
