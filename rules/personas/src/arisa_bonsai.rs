use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"市谷有咲"#)
    .display(r#"市谷有咲"#)
    .group(r#"Poppin' Party"#)
    .color(r#"#AA66DD"#)
    .skill(r#"盆栽爱好者"#)
    .text(r#"（1）每次有其他玩家在“流星堂”抽取事件时获得一个[火罐]（初始0，上限2）（2）回合结束时如果本回合的[主要移动][经过]“流星堂”且[移动终点]不为“流星堂”时抽取一个视为在“流星堂”抽取的事件并获得一个[火罐]。（3）[经过]“流星堂”时可使用2个[火罐]为自己的团卡添加1个[奇迹水晶]。"#)
    .cn_id(r#"005"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"等等等一下"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
