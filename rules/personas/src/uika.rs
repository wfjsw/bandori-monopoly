use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"三角初华"#)
    .display(r#"三角初华"#)
    .latin(r#"Misumi Uika"#)
    .group(r#"Ave Mujica"#)
    .color(r#"#BB9955"#)
    .skill(r#"Imprisoned XII"#)
    .text(r#"[资源]：
（1）[火罐]：在[人偶模式]下上限3。
[自动]：
（1）自己未处于[人偶模式]时：移动步数改为10加上[投掷]1d10的结果。
（2）自己处于[人偶模式]时：获得[不可阻挡]。自己进行[主要移动]并[经过]任意玩家时，那名玩家[支付]自己200资金。
[主动]：
（1）自己未处于[人偶模式]，且自己[经过]CiRCLE的回合结束后，自上一次[经过]CiRCLE以来未在任何[回忆地块]（小豆岛、武道馆、旧古河庭园）[结算]时：可在自己的下回合开始时进入[人偶模式]。
（2）自己未处于[人偶模式]并满足此角色[主动]（1）的前述条件时：可在自己[经过]CiRCLE的回合结束后，立刻获得1个新的回合。
（3）自己处于[人偶模式]，每次通过此角色[自动]（2）成功[收取]资金后：可选择在[结算]前额外移动[投掷]1d6的结果所对应的步数。此效果每回合最多使用4次。"#)
    .cn_id("")
    .art(r#"337"#)
    .costume(r#"337_event_297_story_01"#)
    .exclusive_cards(&[r#"我，无畏悲伤"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
