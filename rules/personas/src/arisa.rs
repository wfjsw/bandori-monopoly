use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"市谷有咲"#)
    .display(r#"市谷有咲"#)
    .latin(r#"Ichigaya Arisa"#)
    .group(r#"Poppin' Party"#)
    .color(r#"#AA66DD"#)
    .skill(r#"盆栽爱好者"#)
    .text(r#"[资源]：
（1）[火罐]：初始0，上限2。
[自动]：
（1）每次有其他玩家在“流星堂”抽取事件时：自己获得1个[火罐]。
（2）回合结束时，如果自己本回合的[主要移动][经过]“流星堂”，且[移动终点]不为“流星堂”：自己抽取1个视为在“流星堂”抽取的事件，并获得1个[火罐]。
[主动]：
（1）自己[经过]“流星堂”时：可使用2个[火罐]，为自己的乐队技能卡添加1个[奇迹水晶]。"#)
    .cn_id(r#"005"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"等等等一下"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
