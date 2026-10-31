use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"大和麻弥"#)
    .display(r#"大和麻弥"#)
    .group(r#"Pastel✽Palettes"#)
    .color(r#"#99DD88"#)
    .skill(r#"朝阳照耀的片刻"#)
    .text(r#"（1）游戏开始后获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得大和麻弥的（2）技能
（2）自己抽卡时时可选择将自己Y个正面[P✽P粉丝]变反，此次抽卡改为观看卡组顶端Y+1张卡（卡组数量不足Y+1则观看全部卡组），然后选择一张牌（不公开）加入手牌（此次加手视为抽卡动作），剩余观看的牌洗回卡组；如果自己是Pastel✽Palettes角色则将Y个其他乐队玩家拥有的反面[P✽P粉丝]变正，否则将所有Pastel✽Palettes角色的1个反面[P✽P粉丝]变正"#)
    .cn_id(r#"019"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"可能性为∞"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
