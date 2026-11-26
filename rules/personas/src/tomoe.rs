use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"宇田川巴"#)
    .display(r#"宇田川巴"#)
    .latin(r#"Udagawa Tomoe"#)
    .group(r#"Afterglow"#)
    .color(r#"#BB0033"#)
    .skill(r#"豚骨酱油拉面大姐"#)
    .text(r#"Band: Afterglow. Color: `#BB0033`.
[资源]：
（1）[火罐]：初始1，上限1。
[自动]：
（1）自己每四回合没有使用Afterglow角色的[主动]（1）技能时：获得1个[火罐]。
[主动]：
（1）自己的移动阶段开始前，若自己不位于“银河拉面馆”：可使用1个[火罐]，依次进行以下操作：
1. 使此次移动的起点向[绝对距离]“银河拉面馆”更近的方向移动10格。
2. 若上述操作使起点向前移动并超过“银河拉面馆”，自己补充1个[火罐]。
（2）自己位于“银河拉面馆”时：可选择[停留]1回合，并获得一个[巴]状态。
[巴]状态：无法使用[主动]（2）效果，经过CiRCLE时移除。"#)
    .cn_id(r#"009"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"商店街的救世主"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
