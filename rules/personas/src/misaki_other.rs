use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"奥泽美咲"#)
    .display(r#"奥泽美咲"#)
    .group(r#"Hello, Happy World!"#)
    .color(r#"#006699"#)
    .skill(r#"另一个我"#)
    .text(r#"（1）每次经过“弦卷集团”（#29）格子后获得2火罐（初始0，上限2），若你在传送时CiRCLE位于你正向移动的移动起点和移动终点之间，则你下次经过“弦卷集团”前可将“弦卷集团”格子额外视为CiRCLE。（2）你的回合开始时，你可以消耗一个火罐，使你的移动掷骰额外获得一个1d10，且你的移动视为[传送]。（3）使用火罐进行移动掷骰后，触发结算前，你可以消耗1火罐，获得一层在你的下回合开始前移除的[除外]。"#)
    .cn_id(r#"015"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"（未命名）"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
