use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"高松灯"#)
    .display(r#"高松灯"#)
    .latin(r#"Takamatsu Tomori"#)
    .group(r#"MyGO!!!!!"#)
    .color(r#"#77BBDD"#)
    .skill(r#"诗超绊"#)
    .text(r#"[资源]：
（1）[火罐]：初始4，上限4。
[自动]：
（1）自己[经过]任意RiNG时：获得1个[火罐]。
[主动]：
（1）其他玩家的[移动终点]与自己所在格子的[绝对距离]不多于5时：可使用4个[火罐]并依次进行以下操作：
  1. 在那名玩家[结算]前，将其[传送]至自己所在的格子并[结算]。
  2. 令其[投掷]1d20，若结果小于等于6，则那名玩家在获得1层“回合结束后获得1层[停留]和1层[重力]并减少1层。”的[延迟[停留]，[重力]]状态。（[重力]是“拥有此状态时即使同时拥有[停留]也可正常[结算]，且[结算]流程中的分摊后[支付]变成0.5倍。回合结束时减少1层。”）
（2）自己的[移动终点]与任意其他玩家所在格子的[绝对距离]不多于5时：可使用4个[火罐]，在自己的[结算]前，将自己[传送]至任意满足上述条件的玩家所在的格子并[结算]。"#)
    .cn_id(r#"036"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"不再迷茫"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
