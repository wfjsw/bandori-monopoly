use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"宇田川亚子"#)
    .display(r#"宇田川亚子"#)
    .latin(r#"Udagawa Ako"#)
    .group(r#"Roselia"#)
    .color(r#"#DD0088"#)
    .skill(r#"对帅气的憧憬"#)
    .text(r#"[自动]：
（1）自己[经过]CiRCLE领取[CiRCLE奖励]时，如果自己的场上存在背面朝上的卡：此次领取奖励改为将自己场上背面朝上的卡加入自己手牌。
（2）自己场上背面朝上的卡为[反击]卡时，可在符合条件时直接打出该卡。
[主动]：
（1）自己[经过]CiRCLE并选择[抽卡]奖励时：可依次进行以下操作：
1. 观看抽卡区顶部至多2张卡。
2. 选择其中1张，加入自己手牌。
3. 将剩余的卡放入弃卡区，或花费500资金，将其背面朝上放置在自己的场上。"#)
    .cn_id(r#"024"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"黑暗大魔姬亚子"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
