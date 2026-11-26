use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"和奏瑞依"#)
    .display(r#"和奏瑞依"#)
    .latin(r#"Wakana Rei"#)
    .group(r#"RAISE A SUILEN"#)
    .color(r#"#CC0000"#)
    .skill(r#"一次又一次竭尽全力"#)
    .text(r#"[资源]：
（1）[火罐]：初始1，上限1。
[自动]：
（1）自己每次[经过]CiRCLE时：获得1个[火罐]。
[主动]：
（1）自己进行任意[投掷]后：可使用1个[火罐]，再[投掷]1次，并选择两次结果之一执行。"#)
    .cn_id(r#"031"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"寄于指尖的执念"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
