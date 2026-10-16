use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"丰川祥子"#)
    .display(r#"丰川祥子"#)
    .group(r#"Ave Mujica"#)
    .color(r#"#7799CC"#)
    .skill(r#"请把你们的人生交给我"#)
    .text(r#"状态1： 经过其他玩家时，可将其所有层数的停留，眩晕转移至自己身上（仍正常完成本次移动），若如此做，每获得一层停留，眩晕，你获得1500资金。每次受到停留，眩晕，除外影响（并结算其影响），获得一个火罐（初始0，上限3），火罐数达到上限时可在回合开始时选择进入状态2。状态2：保留至多一张手牌并将其余置入弃牌堆。每回合开始时自动打出一张抽牌堆顶端的牌。"#)
    .cn_id("")
    .art(r#"341"#)
    .costume(r#"341_event_297_story_01"#)
    .exclusive_cards(&[r#"斩断留恋，忘却一切"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
