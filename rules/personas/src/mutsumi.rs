use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"若叶睦"#)
    .display(r#"若叶睦"#)
    .latin(r#"Wakaba Mutsumi"#)
    .group(r#"Ave Mujica"#)
    .color(r#"#779977"#)
    .skill(r#"天生的演员"#)
    .text(r#"[资源]：
（1）[火罐]：初始0，上限6。
[自动]：
（1）自己未处于[人偶模式]，回合开始前：获得1个[火罐]。
（2）自己从手牌使用卡牌时：失去所有[火罐]。
（3）自己进入[人偶模式]时：依次进行以下操作：
1. 将此角色卡换为上一位打出过手牌的玩家的角色卡。
2. 获得新角色的初始[火罐]。
3. 获得1层[睦状态]。
[主动]：
（1）自己未处于[人偶模式]，回合开始前：可使用6个[火罐]，自己[抽卡]1张。
（2）自己未处于[人偶模式]，回合开始时且手牌数为5：可选择进入[人偶模式]。
---
[火罐]：一种通用资源，初始数量、上限及使用方式由对应效果规定。
[抽卡]：从抽卡区抽取卡牌。
[人偶模式]：Ave Mujica团的特有词条，处于该状态时使用对应的人偶模式技能及乐队效果；具体处理见[规则书](../RULEBOOK.md)。
[睦状态]：回合结束时，如果自己的手牌数小于等于1，则退出[人偶模式]并减少1层。"#)
    .cn_id("")
    .art(r#"338"#)
    .costume(r#"338_event_297_story_01"#)
    .exclusive_cards(&[r#"表演的本能"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
