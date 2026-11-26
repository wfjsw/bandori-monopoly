use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef = PersonaDef::new(r#"仓田真白"#)
    .display(r#"仓田真白"#)
    .latin(r#"Kurata Mashiro"#)
    .group(r#"Morfonica"#)
    .color(r#"#6677CC"#)
    .skill(r#"向后全速前进"#)
    .text(r#"[自动]：
（1）自己用于确定移动步数的[投掷]改为2d20
（2）自己进行移动前：使此次移动改为反向。"#)
    .cn_id(r#"026"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"（未命名）"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
