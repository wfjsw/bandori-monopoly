use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"宇田川巴"#)
    .display(r#"宇田川巴"#)
    .group(r#"Afterglow"#)
    .color(r#"#BB0033"#)
    .skill(r#"豚骨酱油拉面大姐"#)
    .text(r#"（1）每四回合没有使用Afterglow角色的（2）技能获得一个[火罐]（初始1，上限1）（2）移动阶段前，你可以使用一个[火罐]，使此次移动的起点向绝对距离“银河拉面馆”更近的方向移动10格。若你因此效果导致起点向前移动并超过了“银河拉面馆”，你补充一个火罐。（3）若你位于银河拉面馆上，你无法使用（2）效果，你可以选择停留一回合，此效果每圈限一次"#)
    .cn_id(r#"009"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"商店街的救世主"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
