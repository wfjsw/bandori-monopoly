use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"白鹭千圣"#)
    .display(r#"白鹭千圣"#)
    .group(r#"Pastel✽Palettes"#)
    .color(r#"#FFEEAA"#)
    .skill(r#"保持坦率的你"#)
    .text(r#"（1）游戏开始后获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得白鹭千圣的（2）技能
（2）自己每次收取资金时可选择将自己Y个正面[P✽P粉丝]变反，此次获得的分摊前数量增加Y×100；如果自己是Pastel✽Palettes角色则将Y个其他乐队玩家拥有的反面[P✽P粉丝]变正，否则将所有Pastel✽Palettes角色的1个反面[P✽P粉丝]变正"#)
    .cn_id(r#"018"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"微笑的铁假面"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
