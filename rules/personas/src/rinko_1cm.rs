use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"白金燐子"#)
    .display(r#"白金燐子"#)
    .group(r#"Roselia"#)
    .color(r#"#BBBBBB"#)
    .skill(r#"即使1cm也要前进"#)
    .text(r#"（1）每次[经过]CiRCLE时获得三个[火罐]（初始3，上限3）（2）此技能可以正常使用或在[晕眩]状态下使用。你可以在移动掷骰前消耗X个火罐，使这回合移动掷骰的结果固定为X*6。本次移动不受异常移动效果影响。"#)
    .cn_id(r#"025"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"Ringing Bloom"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
