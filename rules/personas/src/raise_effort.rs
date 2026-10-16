use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"和奏瑞依"#)
    .display(r#"和奏瑞依"#)
    .group(r#"RAISE A SUILEN"#)
    .color(r#"#CC0000"#)
    .skill(r#"一次又一次竭尽全力"#)
    .text(r#"（1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）（2）进行任意掷骰后，可选择使用一个[火罐]再投一次骰子并择其一执行"#)
    .cn_id(r#"031"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"寄于指尖的执念"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
