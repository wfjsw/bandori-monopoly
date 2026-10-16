use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"千早爱音"#)
    .display(r#"千早爱音"#)
    .group(r#"MyGO!!!!!"#)
    .color(r#"#FF8899"#)
    .skill(r#"重新开始"#)
    .text(r#"（1）每次[经过]CiRCLE时获得一个[火罐]（初始2，上限3）（2）在受到[停留][传送]效果影响并结算该效果后，可以消耗2个火罐，抽一张卡"#)
    .cn_id(r#"037"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"Anon Tokyo"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
