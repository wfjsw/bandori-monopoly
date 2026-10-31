use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"宇田川亚子"#)
    .display(r#"宇田川亚子"#)
    .group(r#"Roselia"#)
    .color(r#"#DD0088"#)
    .skill(r#"对帅气的憧憬"#)
    .text(r#"（1）每次经过CiRCLE时，若选择抽卡，则可以看抽牌堆顶至多两张卡，选择其中1张加入手牌，剩余的卡放入弃牌堆或花费500资金将其背面朝上放置在自己场上。（2）经过CiRCLE领取奖励时，若自己场上存在背面朝上的卡，则本次领取奖励被替换为将自己场上背面朝上的卡拿取至手中。（3）若自己场上背面朝上放置的卡为[反击]卡，可在符合条件时将其直接打出。"#)
    .cn_id(r#"024"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"黑暗大魔姬亚子"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
