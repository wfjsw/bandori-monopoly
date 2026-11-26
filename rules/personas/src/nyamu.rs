use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"祐天寺若麦"#)
    .display(r#"祐天寺若麦"#)
    .latin(r#"Yutenji Nyamu"#)
    .group(r#"Ave Mujica"#)
    .color(r#"#AA4477"#)
    .skill(r#"大主播喵梦亲"#)
    .text(r#"[资源]：
（1）[火罐]：初始0，上限5。
[自动]：
（1）自己未处于[人偶模式]，每次其他玩家[支付]自己资金时：立即获得1个[火罐]，并使那次[支付]的金额增加X×100；X为自己拥有的[火罐]数量。
（2）自己未处于[人偶模式]，任何时刻[火罐]达到上限时：立即进入[人偶模式]。
（3）自己进入[人偶模式]时：立即将场上全部正面朝上的手卡及事件卡翻为背面朝上。直到[人偶模式]结束，场上所有背面朝上的卡无法产生效果，且不受任何效果影响。自己退出[人偶模式]后，由自己翻面的卡自动翻回。"#)
    .cn_id("")
    .art(r#"340"#)
    .costume(r#"340_event_297_story_01"#)
    .exclusive_cards(&[r#"（未命名）"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
