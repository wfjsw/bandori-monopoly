use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"二叶筑紫"#)
    .display(r#"二叶筑紫"#)
    .group(r#"Morfonica"#)
    .color(r#"#EE7788"#)
    .skill(r#"交给班长吧"#)
    .text(r#"（1）当你获得资金或者抽卡时，可以改为指定场上除你以外的一个角色进行一次该动作，发送给对方一个X（占位）标记并记录获得因此效果获得资金的数量，若为抽卡效果则记录2200。单轮结算（比如登上武道馆的结算）中此效果最多触发一次（2）当有角色获得金钱或抽卡时，你可以将对方的X标记翻面并代替其进行一次该动作。以此效果获得的金钱数量不能超过记录数字+300，超出部分由原主获得。此效果对“抽牌”动作发动时必须翻面记录数字为2200及以上的标记。（3）当场上除你之外的玩家都拥有一枚翻面后的X标记时，移除所有X标记，你获得移除标记数量*800的资金（每次发动（3）技能时单个标记的收益减200，最低单标记收益200）。"#)
    .cn_id(r#"029"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"迷茫的庭园"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
