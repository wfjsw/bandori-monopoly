use ruleset_core::roster::PersonaDef;

/// Roster character row for this skill's owner (ABI v63).
pub const CHARACTER: PersonaDef =
PersonaDef::new(r#"朝日六花"#)
    .display(r#"朝日六花"#)
    .group(r#"RAISE A SUILEN"#)
    .color(r#"#AAEE22"#)
    .skill(r#"瞄准目标"#)
    .text(r#"（1）你购买的第一个非“旭汤澡堂”或任意“Live House”格子获得“Live House”的颜色（2）在你有初始“Live House”格子时你拥有的“旭汤澡堂”格子获得“Live House”的颜色"#)
    .cn_id(r#"032"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"追逐梦想的步伐"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");
