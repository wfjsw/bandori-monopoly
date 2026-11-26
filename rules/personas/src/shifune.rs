use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"都筑诗船"#)
    .display(r#"都筑诗船"#)
    .latin(r#"Tsuzuki Shifune"#)
    .group(r#"CiRCLE"#)
    .color(r#"#77DD77"#)
    .skill(r#"尽力了吗"#)
    .text(r#"[资源]：
（1）[火罐]：初始1，上限2。
[自动]：
（1）自己每次[经过]CiRCLE时：获得1个[火罐]。
（2）游戏开始时：自己获得“Space”格子。
[主动]：
（1）自己的运营阶段：可使用1个[火罐]，使自己与1名其他玩家获得一层[尽力了吗]状态。
[尽力了吗]状态：
1.下一次用于确定移动步数的[投掷]减少[投掷]1d6的结果。若该次确定移动步数的[投掷]结果不大于0，则视为在原地进行1次[结算]。进行确定移动步数的[投掷]后减少一层该状态并获得一层[尽力了]状态和1层[SPACE的考验]状态。
[尽力了]状态：
1.下一次用于确定移动步数的[投掷]增加[投掷]1d6的结果。进行确定移动步数的[投掷]后减少一层该状态并获得1层[SPACE的考验]状态。
[SPACE的考验]状态：
1. [结算]流程中，除购买地契外，[支付]或[消耗]的资金减半；
2. 若使用者以外的拥有该状态的玩家在进行确定移动步数的[投掷]后的[结算]中加盖房屋，使用者[获得]等于该层房屋原本造价的资金。
3. [结算后]减少1层。"#)
    .cn_id("")
    .art(r#"215"#)
    .costume(r#"215_casual"#)
    .exclusive_cards(&[r#"Parking Space"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
