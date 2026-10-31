use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"上原绯玛丽"#)
    .display(r#"上原绯玛丽"#)
    .group(r#"Afterglow"#)
    .color(r#"#FF9999"#)
    .skill(r#"大家一起迈出新的一步"#)
    .text(r#"（1）每四回合没有使用Afterglow角色的（2）技能获得一个[火罐]（初始1，上限1）（2）移动阶段前，你可以消耗一个火罐指定你的此次移动仅在单/双数格上进行。"#)
    .cn_id(r#"008"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"如果并非没问题"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
