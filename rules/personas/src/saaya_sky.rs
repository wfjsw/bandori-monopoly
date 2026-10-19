use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"山吹沙绫"#)
    .display(r#"山吹沙绫"#)
    .group(r#"Poppin' Party"#)
    .color(r#"#FFCC11"#)
    .skill(r#"焕然一新的天空中"#)
    .text(r#"（1）其他玩家一次[消耗]或[支付]至少1000资金且自己不拥有saaya标记时可让那名玩家获得200资金且自己获得1个saaya标记和1个[火罐]（初始3，上限5）；拥有saaya标记时投掷移动骰时失去1个saaya标记，此次投掷结果减1d10（计算后小等于0则移动到下一个可购买格子）
（2）[消耗]或[支付]资金时可使用5个[火罐]，此次资金变动减少5000（最少0）。"#)
    .cn_id(r#"004"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"总有一天要给这片天空命名"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
