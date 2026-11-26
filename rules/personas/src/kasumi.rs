use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"户山香澄"#)
    .display(r#"户山香澄"#)
    .latin(r#"Toyama Kasumi"#)
    .group(r#"Poppin' Party"#)
    .color(r#"#FF5522"#)
    .skill(r#"非凡之星"#)
    .text(r#"[资源]：
（1）[火罐]：初始0，上限1。
[自动]：
（1）其他玩家在“星之鼓动山丘”上[结算]时：自己获得1个[火罐]。
[主动]：
（1）自己的运营阶段：可使用1个[火罐]，依次进行以下操作：
1. 立刻进入移动阶段，将本回合的[主要移动]改为[传送]到任意自己拥有的格子。
2. 可选择盖房。"#)
    .cn_id(r#"001"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"大家我都喜欢哦"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
