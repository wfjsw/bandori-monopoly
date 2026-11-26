use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"奥泽美咲"#)
    .display(r#"奥泽美咲"#)
    .latin(r#"Okusawa Misaki"#)
    .group(r#"Hello, Happy World!"#)
    .color(r#"#006699"#)
    .skill(r#"另一个我"#)
    .text(r#"[资源]：
（1）[火罐]：初始0，上限2。
[自动]：
（1）自己每次[经过]“弦卷集团”（#29）后：获得2个[火罐]。若自己[传送]时，CiRCLE位于自己正向移动的[移动起点]与[移动终点]之间，则在自己下次[经过]“弦卷集团”前，可将“弦卷集团”额外视为CiRCLE。
[主动]：
（1）自己的回合开始时：可使用1个[火罐]，使本次用于确定移动步数的[投掷]额外增加1d10，且此次移动改为[传送]到移动终点。
（2）自己使用[火罐]进行用于确定移动步数的[投掷]后、[结算]前：可使用1个[火罐]，获得1层在自己的下回合开始前移除的[除外]。"#)
    .cn_id(r#"015"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"（未命名）"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
