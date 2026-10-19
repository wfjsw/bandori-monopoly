use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"北泽育美"#)
    .display(r#"北泽育美"#)
    .group(r#"Hello, Happy World!"#)
    .color(r#"#FF9922"#)
    .skill(r#"全垒打！"#)
    .text(r#"（1）每个回合在北泽精肉店生成一个可乐饼。你经过格子上的可乐饼时可以将其转移到自己场上（持有上限10），其他角色经过格子上可乐饼时可以将其转移到自己场上并向你支付50*X资金，X为该角色持有的可乐饼数量。你将可乐饼转移至自己场上时可用其替换掉一个其他不位于[持续]卡上的标记。角色每拥有一个可乐饼，移动时多投掷1个1d2。
育美经过其他角色时，可以把自己身上的可乐饼转移给该角色。育美可在主要阶段消耗4个可乐饼兑换1000资金，其他角色拥有4个可乐饼时自动移除所有可乐饼停留一回合。"#)
    .cn_id(r#"013"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"（未命名）"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
