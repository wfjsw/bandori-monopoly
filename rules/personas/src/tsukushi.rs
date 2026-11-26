use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"二叶筑紫"#)
    .display(r#"二叶筑紫"#)
    .latin(r#"Futaba Tsukushi"#)
    .group(r#"Morfonica"#)
    .color(r#"#EE7788"#)
    .skill(r#"交给班长吧"#)
    .text(r#"[自动]：
（1）场上除自己以外的玩家均拥有1枚翻面后的[筑紫标记X]时：移除所有[筑紫标记X]，自己[获得]移除标记数量×800资金。每次发动此[自动]（1）效果时，单个标记的收益减少200（最低200）。
[主动]：
（1）自己获得资金或[抽卡]时：可改为[指定]场上除自己以外的1名角色进行1次该动作，并给予那名角色1个[筑紫标记X]。X记录因此效果获得的资金数量；若为[抽卡]，则记录2200。单轮结算（例如“登上武道馆”的结算）中，此效果最多发动1次。
（2）有角色获得资金或[抽卡]时：自己可将那名角色的[筑紫标记X]翻面，代替其进行1次该动作。以此效果获得的资金不能超过记录金额加300，超出部分由原角色获得。替代[抽卡]时，必须翻面记录金额至少为2200的标记。"#)
    .cn_id(r#"029"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"迷茫的庭园"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
