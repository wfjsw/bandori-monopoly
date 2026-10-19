use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"弦卷心"#)
    .display(r#"弦卷心"#)
    .group(r#"Hello, Happy World!"#)
    .color(r#"#FFEE22"#)
    .skill(r#"弦卷集团"#)
    .text(r#"（1）开局时获得1000资金（2）[经过]CiRCLE时额外获得1500资金"#)
    .cn_id(r#"011"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"前往笑容集结的地方！"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
