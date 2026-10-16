use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"都筑诗船"#)
    .display(r#"都筑诗船"#)
    .group(r#"CiRCLE"#)
    .color(r#"#77DD77"#)
    .skill(r#"尽力了吗"#)
    .text(r#"（1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限2）（2）你的主要阶段中可消耗1火罐使你与一名其他玩家接下来的两次移动掷骰先后-1d6与+1d6，若由于此效果导致移动数为0，则视为原地进行一次结算。在此技能效果影响下触发结算时，除购买地契外支付或消耗的资金减半，若其他玩家在此技能效果影响下触发结算时加盖了房屋，你获得相当于那层房屋造价原价的资金。（3）初始获得“Space”。"#)
    .cn_id("")
    .art(r#"215"#)
    .costume(r#"215_casual"#)
    .exclusive_cards(&[r#"Parking Space"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
