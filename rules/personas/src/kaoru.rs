use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"濑田薰"#)
    .display(r#"濑田薰"#)
    .latin(r#"Seta Kaoru"#)
    .group(r#"Hello, Happy World!"#)
    .color(r#"#AA33CC"#)
    .skill(r#"梦幻的王子殿下"#)
    .text(r#"[资源]：
（1）[火罐]：初始0，上限7。
[自动]：
（1）自己每次[经过]或被[经过]时：若场上不存在[怪盗标记]，则获得1个[火罐]；若场上存在[怪盗标记]，则移除场上的1个[怪盗标记]。
（2）自己非[传送]的[主要移动]不会[经过]移动阶段开始时场上拥有[怪盗标记]的玩家所拥有的格子。
[主动]：
（1）自己的运营阶段：可使用7个[火罐]，依次进行以下操作：
1. 在与自己[绝对距离]最近的1名玩家场上放置3个[怪盗标记]。
2. 使此次移动以“微笑号”为起点。"#)
    .cn_id(r#"012"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"怪盗hello happy"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
