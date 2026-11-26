use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"上原绯玛丽"#)
    .display(r#"上原绯玛丽"#)
    .latin(r#"Uehara Himari"#)
    .group(r#"Afterglow"#)
    .color(r#"#FF9999"#)
    .skill(r#"大家一起迈出新的一步"#)
    .text(r#"Band: Afterglow. Color: `#FF9999`.
[资源]：
（1）[火罐]：初始1，上限1。
[自动]：
（1）自己每四回合没有使用Afterglow角色的[主动]（1）技能时：获得1个[火罐]。
[主动]：
（1）自己的移动阶段前：可使用1个[火罐]，选择此次移动仅在格子序号为单数的格子上进行，或仅在格子序号为双数的格子上进行。"#)
    .cn_id(r#"008"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"如果并非没问题"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
