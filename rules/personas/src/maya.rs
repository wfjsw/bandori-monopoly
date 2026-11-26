use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"大和麻弥"#)
    .display(r#"大和麻弥"#)
    .latin(r#"Yamato Maya"#)
    .group(r#"Pastel✽Palettes"#)
    .color(r#"#99DD88"#)
    .skill(r#"朝阳照耀的片刻"#)
    .text(r#"[自动]：
（1）游戏开始后，自己获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得大和麻弥的[主动]（1）技能。
[主动]：
（1）自己[抽卡]时：可选择将自己Y个正面[P✽P粉丝]变反。若如此做，依次进行以下操作：
1. 此次[抽卡]改为观看自己卡组顶端Y+1张卡（卡组数量不足Y+1则观看全部卡组），然后选择其中1张卡加入自己手牌（不公开，此次加入手牌视为[抽卡]动作），将剩余观看的卡洗回自己卡组；
2. 如果自己是Pastel✽Palettes角色，则将Y个其他乐队玩家拥有的反面[P✽P粉丝]变正；否则，将所有Pastel✽Palettes角色的Y个反面[P✽P粉丝]变正。"#)
    .cn_id(r#"019"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"可能性为∞"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
