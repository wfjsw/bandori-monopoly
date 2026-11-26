use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"椎名立希"#)
    .display(r#"椎名立希"#)
    .latin(r#"Shiina Taki"#)
    .group(r#"MyGO!!!!!"#)
    .color(r#"#7777AA"#)
    .skill(r#"决定练习日的会议"#)
    .text(r#"[资源]：
（1）[火罐]：初始0，上限5。
[自动]：
（1）任何玩家获得X层[停留]时：自己获得X个[火罐]。
[主动]：
（1）自己的运营阶段且不拥有[立希]时：可使用1个[火罐]，为自己场上的任意1张卡拥有[奇迹水晶]资源的卡添加1个[奇迹水晶]并获得1层“回合结束时减少1层的”[立希]状态。"#)
    .cn_id(r#"040"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"想认真去做"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
