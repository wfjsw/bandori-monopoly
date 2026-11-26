use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"牛込里美"#)
    .display(r#"牛込里美"#)
    .latin(r#"Ushigome Rimi"#)
    .group(r#"Poppin' Party"#)
    .color(r#"#FF55BB"#)
    .skill(r#"里美的决心"#)
    .text(r#"[资源]：
（1）[火罐]：初始1，上限3。
[自动]：
（1）自己在自己的回合中[经过]任意Live House颜色格子后，于该回合结束时获得1个[火罐]。
[主动]：
（1）自己的运营阶段：可使用3个[火罐]，依次进行以下操作：
1. 立刻进入移动阶段。
2. [传送]至任意与自己[绝对距离]最远的[地产商]，并进行[结算]。
3. 可选择盖房。"#)
    .cn_id(r#"003"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"我的心就像巧克力螺"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
