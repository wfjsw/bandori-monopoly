use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"八幡海铃"#)
    .display(r#"八幡海铃"#)
    .group(r#"Ave Mujica"#)
    .color(r#"#335566"#)
    .skill(r#"熟练的支援贝斯手"#)
    .text(r#"（1）状态1：其他玩家可以在他们的回合开始时向你支付400资金指定一个属于你的格子，使其立即对前后一格内的一名玩家进行一次半价[结算]，并获得相当于你获取数额的资金；直到这两名玩家的下回合结束，若你仍在状态1，他们不会被该效果再次收款。每两个回合没有玩家发动该技能，你在回合开始时获得1火罐（初始0，上限4），若达到上限，可在你的回合开始时选择进入状态2。（2） 状态2： 进入状态2时你可消耗X个火罐，指定其他人的X个地契，每张地契向对应玩家支付100+n*100资金（n为对应格子上的房屋数），直到状态2结束为止，你与对方均分那些地契收取的资金；每通过此法消耗4个火罐，你抽1张卡。"#)
    .cn_id("")
    .art(r#"339"#)
    .costume(r#"339_event_297_story_01"#)
    .exclusive_cards(&[r#"（未命名）"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
