use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"广町七深"#)
    .display(r#"广町七深"#)
    .latin(r#"Hiromachi Nanami"#)
    .group(r#"Morfonica"#)
    .color(r#"#EE7744"#)
    .skill(r#"这是很普通的事吧？"#)
    .text(r#"[自动]：
（1）自己每次[经过]CiRCLE时：获得1个[火罐]（初始2，上限2）。
[主动]：
（1）有玩家使用角色技能时：自己可使用1个[火罐]，获得1个记录该角色的[模仿标记]。场上[存活]玩家数量至少为3时，每种被模仿角色对应的[模仿标记]上限1，且此技能对[自动]技能存在3回合冷却。
（2）时机合适时：自己可使用持有的[模仿标记]，视为使用该标记记录的角色的技能。若该技能需要使用[火罐]，则使用自己拥有数量与所需数量中的较小值；其他所需资源（例如资金和翻转[P✽P粉丝]）仍须按该技能的要求使用。"#)
    .cn_id(r#"028"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"稍微努力了一下"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
