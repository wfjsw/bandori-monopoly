use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"山吹沙绫"#)
    .display(r#"山吹沙绫"#)
    .latin(r#"Yamabuki Saaya"#)
    .group(r#"Poppin' Party"#)
    .color(r#"#FFCC11"#)
    .skill(r#"焕然一新的天空中"#)
    .text(r#"[资源]：
（1）[火罐]：初始3，上限5。
[主动]：
（1）其他玩家一次[消耗]或[支付]至少1000资金，且自己不拥有[saaya标记]时：可依次进行以下操作：
1. 那名玩家[获得]200资金。
2. 自己获得1层[saaya标记]和1个[火罐]。
（2）自己[消耗]或[支付]资金时：可使用5个[火罐]，此次资金变动减少5000（最低0）。
---
[火罐]：一种通用资源，初始数量、上限及使用方式由对应效果规定。
[消耗]：直接失去资金，等同于删钱。
[支付]：玩家之间资金转移的付款方用语。
[获得]：直接获得资金，等同于印钱。
[投掷]：按指定的骰子数量与面数掷骰；1d10表示1个10面骰。
[可购买格子]：非CiRCLE，CiRCLE 咖啡厅，江户川乐器店，流星堂，或[地产商]的格子。
[saaya标记]：移动[投掷]的结果减少[投掷]1d10的结果。如果计算后的结果小于等于0，则移动到下一个[可购买格子]。移动[投掷]后减少1层。"#)
    .cn_id(r#"004"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"总有一天要给这片天空命名"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
