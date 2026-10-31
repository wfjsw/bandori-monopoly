use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"三角初华"#)
    .display(r#"三角初华"#)
    .group(r#"Ave Mujica"#)
    .color(r#"#BB9955"#)
    .skill(r#"Imprisoned XII"#)
    .text(r#"状态1：移动改为10+1d10，每当你经过CiRCLE的回合结束后，若你自从上一次经过CiRCLE后未在任何[回忆地块]（小豆岛，武道馆，旧古河庭园）触发结算则可在你的下回合开始时进入状态2。若满足前述条件，你可在你经过CiRCLE的回合结束后立刻获得一个新的回合。
状态2：（火罐上限3）获得[不可阻挡]，并且在此状态下主动移动经过任何玩家都将向其收取200资金，每次成功收取后可选择在[触发结算]前额外移动1d6（最多4次）"#)
    .cn_id("")
    .art(r#"337"#)
    .costume(r#"337_event_297_story_01"#)
    .exclusive_cards(&[r#"我，无畏悲伤"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
