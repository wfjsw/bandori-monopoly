use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"花园多惠"#)
    .display(r#"花园多惠"#)
    .group(r#"Poppin' Party"#)
    .color(r#"#0077DD"#)
    .skill(r#"花园警察，出警！"#)
    .text(r#"（1）每次你领取[CiRCLE奖励]时投掷3d20，如果结果对应的格子不是“星空齿科”则在结果对应的格子放置一个[多惠兔子]；当你经过有[多惠兔子]的格子时[移除]该格子上的所有[多惠兔子]并获得等量的[火罐]（初始2，上限4）
（2）在其他玩家使用角色/团技能的[主]效果或使用手牌的[手]效果时你可以使用4个[火罐]将其抵消，然后你获得500资金且被抵消的玩家获得2000资金"#)
    .cn_id(r#"002"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"寻找更好的声音"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
