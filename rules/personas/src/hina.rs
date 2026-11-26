use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"冰川日菜"#)
    .display(r#"冰川日菜"#)
    .latin(r#"Hikawa Hina"#)
    .group(r#"Pastel✽Palettes"#)
    .color(r#"#55DDEE"#)
    .skill(r#"日菜抽中的大奖"#)
    .text(r#"[自动]：
（1）游戏开始后，自己获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得冰川日菜的[自动]（2）技能。
（2）每回合开始时：自己必须[投掷]1d4，根据最终[投掷]结果获得以下对应角色的[主动]（1）技能，持续至下回合开始前：
- 结果为1：丸山彩；
- 结果为2：白鹭千圣；
- 结果为3：若宫伊芙；
- 结果为4：大和麻弥。"#)
    .cn_id(r#"017"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"会发出怎样的声音呢？"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
