use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"户山香澄"#)
    .display(r#"户山香澄"#)
    .group(r#"Poppin' Party"#)
    .color(r#"#FF5522"#)
    .skill(r#"非凡之星"#)
    .text(r#"（1）其他玩家“星之鼓动山丘”上[结算]时获得一个[火罐]（初始0，上限1）（2）运营阶段可使用1个[火罐]，立刻进入移动阶段且本回合的[主要移动]改为[传送]到任意自己拥有的格子且可选择盖房"#)
    .cn_id(r#"001"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"大家我都喜欢哦"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
