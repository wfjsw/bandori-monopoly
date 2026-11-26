use bandori_std::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"三角初华（Sumimi）"#)
    .display(r#"三角初华（Sumimi）"#)
    .latin(r#"Misumi Uika"#)
    .group(r#"Sumimi"#)
    .color(r#"#BB9955"#)
    .skill(r#"成为偶像"#)
    .text(r#"[资源]：
（1）[火罐]：初始2，上限2。
[自动]：
（1）自己每次[经过]CiRCLE时：获得1个[火罐]。
[主动]：
（1）回合开始时：自己可使用1个[火罐]，获得一层[sumimi初华标记X]，X为与“主要街道”颜色相同的任意1个格子的编号。
其他玩家[经过][sumimi初华标记X]对应的格子时，获得一层[sumimi初华标记X（2）]状态。
[sumimi初华标记X（2）]状态：到达[移动终点]后，依次进行以下操作：
1. 自己[支付][sumimi初华标记X]的拥有者Y×30资金；Y为自己[经过][sumimi初华标记X]对应的格子后移动的剩余格数。
2. 此状态减少1层，并将对应的[sumimi初华标记X]减少1层（如果有）。
[sumimi初华标记X]：获得时使自己[火罐]上限-1，移除时使自己[火罐]上限+1,自己被由自己以外的消耗火罐效果影响时移除等量的该标记代替[火罐]消耗。"#)
    .cn_id("")
    .art(r#"337s"#)
    .costume(r#"337_sumimi"#)
    .exclusive_cards(&[r#"儿时玩伴的鼓励"#])
    .preset(&[])
    .variant_of(r#"三角初华"#)
    .variant_tag("");
