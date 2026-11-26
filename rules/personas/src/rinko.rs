use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"白金燐子"#)
    .display(r#"白金燐子"#)
    .latin(r#"Shirokane Rinko"#)
    .group(r#"Roselia"#)
    .color(r#"#BBBBBB"#)
    .skill(r#"即使1cm也要前进"#)
    .text(r#"[资源]：
（1）[火罐]：初始3，上限3。
[自动]：
（1）自己每次[经过]CiRCLE时：获得3个[火罐]。
[主动]：
（1）使用X个[火罐]，获得一层[霸体]状态，并立刻进入移动阶段，进行固定为X×6格的移动。此技能在正常状态或[眩晕]状态下均可使用。
[霸体]状态：不受[异常移动效果]影响，回合结束时移除该状态。"#)
    .cn_id(r#"025"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"Ringing Bloom"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
