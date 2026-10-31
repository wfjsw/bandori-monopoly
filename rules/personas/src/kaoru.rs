use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"濑田薰"#)
    .display(r#"濑田薰"#)
    .group(r#"Hello, Happy World!"#)
    .color(r#"#AA33CC"#)
    .skill(r#"梦幻的王子殿下"#)
    .text(r#"（1）每次[经过]或被[经过]时，若场上不存在[怪盗标记]，获得1火罐（上限7）；若场上存在[怪盗标记]，则移除场上的一个[怪盗标记]（2）主要阶段可消耗7火罐在绝对距离最近的一名玩家场上放置3个[怪盗标记]，并使你的本次移动以微笑号为起点。（3）你非传送的主要移动不会经过场上拥有[怪盗标记]的玩家所拥有的格子。"#)
    .cn_id(r#"012"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"怪盗hello happy"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
