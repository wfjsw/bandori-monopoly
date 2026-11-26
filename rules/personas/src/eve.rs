use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"若宫伊芙"#)
    .display(r#"若宫伊芙"#)
    .latin(r#"Wakamiya Eve"#)
    .group(r#"Pastel✽Palettes"#)
    .color(r#"#DDBBFF"#)
    .skill(r#"天下统一"#)
    .text(r#"[自动]：
（1）游戏开始后，自己获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得若宫伊芙的[主动]（1）技能。
[主动]：
（1）自己因任意原因[投掷]前：可选择将自己Y个正面[P✽P粉丝]变反。若如此做，依次进行以下操作：
1. 为此次[投掷]额外增加Y个d4骰子；
2. 如果自己是Pastel✽Palettes角色，则将Y个其他乐队玩家拥有的反面[P✽P粉丝]变正；否则，将所有Pastel✽Palettes角色的Y个反面[P✽P粉丝]变正。"#)
    .cn_id(r#"020"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"属于我的武士道！"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
