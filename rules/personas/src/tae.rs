use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"花园多惠"#)
    .display(r#"花园多惠"#)
    .latin(r#"Hanazono Tae"#)
    .group(r#"Poppin' Party"#)
    .color(r#"#0077DD"#)
    .skill(r#"花园警察，出警！"#)
    .text(r#"[资源]：
（1）[火罐]：初始2，上限4。
[自动]：
（1）每次自己领取[CiRCLE奖励]时：[投掷]3d20。如果结果对应的格子不是“星空齿科”，则在该格子放置1个[多惠标记]。自己[经过]有[多惠标记]的格子时，移除该格子上的所有[多惠标记]，并获得X个[火罐]；X为此次移除的格子状态[多惠标记]数量。
[主动]：
（1）其他玩家发动角色或乐队技能的[主动]效果，或使用手牌的[手]效果时：可使用4个[火罐]，依次进行以下操作：
1. 抵消该效果。
2. 自己[获得]500资金。
3. 被抵消效果的玩家[获得]2000资金。"#)
    .cn_id(r#"002"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"寻找更好的声音"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
