use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"要乐奈"#)
    .display(r#"要乐奈"#)
    .group(r#"MyGO!!!!!"#)
    .color(r#"#77DD77"#)
    .skill(r#"投币式停车场的猫"#)
    .text(r#"（1）每次[经过]CiRCLE时获得一个[火罐]（初始3，上限3）（2）可花费3个火罐传送至space代替本回合的移动，该次传送不可进行地契购买。在space上结算时，若space已属于其他玩家，免除付款并抽1张卡；若自己为space的拥有者，则可选择将该次结算改为在space格子上添加一个“抹茶芭菲”，每个将使其收费增加500资金，其他玩家在space触发结算时可将自己拥有的一个“抹茶芭菲”转移到该格上以免除当次付款 （3）你与其他玩家重合时，获得其800资金并使那个玩家获得一个“抹茶芭菲”；拥有“抹茶芭菲”的玩家经过任意“RiNG”时可使用一个“抹茶芭菲”获得400资金"#)
    .cn_id(r#"038"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"有趣的女人"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
