use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"松原花音"#)
    .display(r#"松原花音"#)
    .latin(r#"Matsubara Kanon"#)
    .group(r#"Hello, Happy World!"#)
    .color(r#"#44DDFF"#)
    .skill(r#"真正的迷子"#)
    .text(r#"[自动]：
（1）游戏开始时：自己的起点为“弦卷豪宅”（30号格子）。
（2）自己的移动阶段：进行2次用于确定移动步数的[投掷]，将两次结果相减，作为自己的移动格数（若为负数则反向移动）。
（3）如果自己在三个回合内没有领取[CiRCLE奖励]：获得[呼诶诶]状态。
[呼诶诶]状态：用于确定移动步数的[投掷]改为1d20+1d4。领取CiRCLE奖励后移除该状态。"#)
    .cn_id(r#"014"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"Wacha Mocha 啪嗒进行曲"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
