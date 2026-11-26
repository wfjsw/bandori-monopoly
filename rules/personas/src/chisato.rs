use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"白鹭千圣"#)
    .display(r#"白鹭千圣"#)
    .latin(r#"Shirasagi Chisato"#)
    .group(r#"Pastel✽Palettes"#)
    .color(r#"#FFEEAA"#)
    .skill(r#"保持坦率的你"#)
    .text(r#"[自动]：
（1）游戏开始后，自己获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得白鹭千圣的[主动]（1）技能。
[主动]：
（1）自己每次[收取]资金时：可选择将自己Y个正面[P✽P粉丝]变反。若如此做，依次进行以下操作：
1. 此次[收取]的分摊前金额增加Y×100；
2. 如果自己是Pastel✽Palettes角色，则将Y个其他乐队玩家拥有的反面[P✽P粉丝]变正；否则，将所有Pastel✽Palettes角色的Y个反面[P✽P粉丝]变正。"#)
    .cn_id(r#"018"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"微笑的铁假面"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
