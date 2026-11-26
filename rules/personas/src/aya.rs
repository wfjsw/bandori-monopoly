use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"丸山彩"#)
    .display(r#"丸山彩"#)
    .latin(r#"Maruyama Aya"#)
    .group(r#"Pastel✽Palettes"#)
    .color(r#"#FF88BB"#)
    .skill(r#"With~"#)
    .text(r#"[自动]：
（1）游戏开始后，自己获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得丸山彩的[主动]（1）技能。
[主动]：
（1）自己每次需要[支付]资金时：可选择将自己Y个正面[P✽P粉丝]变反。若如此做，依次进行以下操作：
1. 此次[支付]的分摊前金额减少Y×100（最低0）；
2. 如果自己是Pastel✽Palettes角色，则将Y个其他乐队玩家拥有的反面[P✽P粉丝]变正；否则，将所有Pastel✽Palettes角色的Y个反面[P✽P粉丝]变正。"#)
    .cn_id(r#"016"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"憧憬的前方"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
