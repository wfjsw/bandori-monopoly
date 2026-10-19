use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"仓田真白"#)
    .display(r#"仓田真白"#)
    .group(r#"Morfonica"#)
    .color(r#"#6677CC"#)
    .skill(r#"向后全速前进"#)
    .text(r#"（1）[主动移动]时移动掷骰变为2d20（2）反方向移动"#)
    .cn_id(r#"026"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"（未命名）"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
