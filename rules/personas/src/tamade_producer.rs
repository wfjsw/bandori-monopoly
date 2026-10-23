use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef = PersonaDef::new(r#"珠手知由"#)
    .display(r#"珠手知由"#)
    .group(r#"RAISE A SUILEN"#)
    .color(r#"#00BBFF"#)
    .skill(r#"天才制作人"#)
    .text(r#"加盖房屋时半价，如果你上回合进行过房屋加盖，本回合加盖房屋变为免费"#)
    .cn_id(r#"035"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"演奏我的音乐吧"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
