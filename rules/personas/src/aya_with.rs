use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"丸山彩"#)
    .display(r#"丸山彩"#)
    .group(r#"Pastel✽Palettes"#)
    .color(r#"#FF88BB"#)
    .skill(r#"With~"#)
    .text(r#"（1）游戏开始后获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得丸山彩的（2）技能
（2）自己每次需要支付资金时可选择将自己Y个正面[P✽P粉丝]变反，此次支付的分摊前资金减少Y×100（最少0）；如果自己是Pastel✽Palettes角色则将Y个其他乐队玩家拥有的反面[P✽P粉丝]变正，否则将所有Pastel✽Palettes角色的1个反面[P✽P粉丝]变正"#)
    .cn_id(r#"016"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"憧憬的前方"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
