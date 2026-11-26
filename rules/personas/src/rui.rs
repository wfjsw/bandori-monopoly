use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"八潮瑠唯"#)
    .display(r#"八潮瑠唯"#)
    .latin(r#"Yashio Rui"#)
    .group(r#"Morfonica"#)
    .color(r#"#669988"#)
    .skill(r#"正论暴击"#)
    .text(r#"X初始为0。
[自动]：
（1）自己的[自动]（2）技能或角色卡被复制后，复制者将X重置为0时：那名玩家失去该技能。
（2）自己即将获得或失去资金时（抵押或赎回除外），在所有其他资金量改动效果生效后、结果不等于0时：[投掷]1d20，并根据结果进行以下操作：
1. 结果小于等于X：将X重置为0，此次获得的资金翻倍或无需失去资金，收款或付款方同样受到影响；
2. 结果大于X：X增加1。"#)
    .cn_id(r#"030"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"正论恶魔"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
