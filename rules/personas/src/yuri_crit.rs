use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"八潮瑠唯"#)
    .display(r#"八潮瑠唯"#)
    .group(r#"Morfonica"#)
    .color(r#"#669988"#)
    .skill(r#"正论暴击"#)
    .text(r#"（1）如果你的（2）技能或角色卡被复制，当复制者使x重置为0时，对方失去该技能（2）（抵押或赎回以外）当你即将获得或失去资金（在所有其他资金量改动效果生效后并结果不等于0）时，roll1d20，若结果小于等于X，将X重置为0，此次获得资金翻倍或无需失去资金（收款或付款方同样受到影响），若结果大于X，X+1。X初始为0"#)
    .cn_id(r#"030"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"正论恶魔"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
