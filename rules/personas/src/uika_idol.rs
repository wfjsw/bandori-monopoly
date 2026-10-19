use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"三角初华（Sumimi）"#)
    .display(r#"三角初华（Sumimi）"#)
    .group(r#"Sumimi"#)
    .color(r#"#BB9955"#)
    .skill(r#"成为偶像"#)
    .text(r#"（1）每次[经过]CiRCLE时获得1个[火罐]（初始2，上限2）（2）回合开始时可将一个火罐放置在与“主要街道”颜色相同的任一格，其他玩家经过该格且到达移动终点后向你支付X*30资金并移除那个火罐，X为对方经过该格后移动的剩余格数。"#)
    .cn_id("")
    .art(r#"337s"#)
    .costume(r#"337_sumimi"#)
    .exclusive_cards(&[r#"儿时玩伴的鼓励"#])
    .preset(&[])
    .variant_of(r#"三角初华"#)
    .variant_tag("");
