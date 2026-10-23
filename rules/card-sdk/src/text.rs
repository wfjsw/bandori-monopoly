//! Rule text as sections and bulletpoints -- the display shape of a card's
//! (or an event's) effect text.
//!
//! The rulebook prints structure as text conventions: section labels on a line
//! of their own (`[手]：`, `[特]：`, `[持续]`, `状态1：`), numbered effects inline
//! as `（1）…（2）…`, sub-steps as `1. …` lines. Encoding that in one flat
//! `&'static str` leaves every consumer to re-derive the structure with regex
//! heuristics (and to guess whether a `（2）` opens an effect or names one).
//! [`CardText`] carries the structure instead; the markers are **positional**,
//! so they cannot be mistaken for cross-references.
//!
//! [`CardText::flat`] renders the rulebook string form. That is what rides
//! [`crate::meta::MetaRow::text`] -- the rule queries (`card_text_mentions`,
//! 「效果包含…」) substring-search it -- and what display-only consumers that
//! want prose rather than blocks read. The invariant is that every authored
//! run of prose appears in `flat()` as a contiguous substring; separators and
//! `（N）` / `N.` markers may differ from the sheet's exact spacing.
//!
//! Generic shape: labels are plain strings, so this module holds no game
//! vocabulary. Authoring stays `const fn`-friendly (see [`sec`] / [`para`] /
//! [`items`] / [`steps`] / [`it`] / [`it_sub`]).
//!
//! A `const fn` cannot return `'static` slices it builds inside its own body
//! (the temporaries would be locals), so the single-block shortcuts on
//! [`CardText`] are **variants** carrying the caller's `'static` slice
//! directly, and [`CardText::of`] is the general multi-section form.

// String/Vec come from `alloc` on guest builds and from std on the host.
#[cfg(any(target_arch = "wasm32", feature = "guest"))]
use alloc::string::{String, ToString};
#[cfg(any(target_arch = "wasm32", feature = "guest"))]
use alloc::vec::Vec;

/// Rule text as sections and bulletpoints. Display shape; [`CardText::flat`]
/// is the rulebook string the rule queries search.
///
/// ```ignore
/// CardText::of(&[
///     sec("[手]：", &[
///         para("依次进行以下效果："),
///         steps(&["为[使用者]的乐队卡添加2个[奇迹水晶]；", "…"]),
///     ]),
///     sec("[持续]：", &[items(&[it("手卡上限数量减1。"), it("…")])]),
/// ])
/// ```
#[derive(Clone, Copy)]
pub enum CardText {
    /// No text at all.
    Empty,
    /// A single unlabelled paragraph -- the shape of a card whose rule text is
    /// one sentence with no sections or bulletpoints.
    Para(&'static str),
    /// A single unlabelled numbered list.
    Items(&'static [TextItem]),
    /// The general form: an explicit section list.
    Sections(&'static [TextSection]),
}

/// One labelled block of effect text. `label` is the line the rulebook prints
/// above its effects (`[手]：`, `[特]：`, `[持续]`, `状态1：`); empty for the
/// unlabelled lead before the first label, or for a text with no labels at all.
#[derive(Clone, Copy)]
pub struct TextSection {
    pub label: &'static str,
    pub blocks: &'static [TextBlock],
}

/// A paragraph, a numbered-effect list, or a standalone sub-step list.
#[derive(Clone, Copy)]
pub enum TextBlock {
    /// Free paragraph (intro / tail prose).
    Para(&'static str),
    /// Numbered effects `（1）（2）…`. The marker is **positional** -- do not
    /// include it in the strings. Items number from 1; a new [`TextBlock::Items`]
    /// block (e.g. under a new section label) restarts at 1.
    Items(&'static [TextItem]),
    /// Standalone sub-steps `1. 2. 3.` (marker positional). The
    /// 「依次进行以下操作：」 lists that sit at section level rather than
    /// nested under a numbered effect.
    Steps(&'static [&'static str]),
}

/// One numbered effect, with its optional nested `1. 2.` sub-steps.
#[derive(Clone, Copy)]
pub struct TextItem {
    /// Effect body without its `（N）` marker. Cross-references (「使用（2）效果」,
    /// 「角色的（2）技能」) stay in this prose verbatim -- they name effects, they
    /// do not open them.
    pub text: &'static str,
    /// Nested `1. 2.` sub-steps under this effect (marker positional).
    pub sub: &'static [&'static str],
}

/// One section: `label` on its own line above `blocks`.
pub const fn sec(label: &'static str, blocks: &'static [TextBlock]) -> TextSection {
    TextSection { label, blocks }
}

/// A paragraph block.
pub const fn para(text: &'static str) -> TextBlock {
    TextBlock::Para(text)
}

/// A numbered-effects block.
pub const fn items(items: &'static [TextItem]) -> TextBlock {
    TextBlock::Items(items)
}

/// A standalone sub-step list block.
pub const fn steps(steps: &'static [&'static str]) -> TextBlock {
    TextBlock::Steps(steps)
}

/// One numbered effect, no sub-steps.
pub const fn it(text: &'static str) -> TextItem {
    TextItem { text, sub: &[] }
}

/// One numbered effect with nested sub-steps.
pub const fn it_sub(text: &'static str, sub: &'static [&'static str]) -> TextItem {
    TextItem { text, sub }
}

impl CardText {
    /// No text at all.
    pub const EMPTY: Self = Self::Empty;

    /// The general form: an explicit section list.
    pub const fn of(sections: &'static [TextSection]) -> Self {
        Self::Sections(sections)
    }

    /// A single unlabelled paragraph.
    pub const fn para(text: &'static str) -> Self {
        Self::Para(text)
    }

    /// A single unlabelled numbered list.
    pub const fn items(list: &'static [TextItem]) -> Self {
        Self::Items(list)
    }

    /// True when there is nothing to show.
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Empty => true,
            Self::Para(t) => t.is_empty(),
            Self::Items(list) => list.is_empty(),
            Self::Sections(secs) => secs.iter().all(|s| s.label.is_empty() && s.blocks.is_empty()),
        }
    }

    /// The rulebook string form. See the module docs for the invariant: every
    /// authored prose run appears here as a contiguous substring.
    pub fn flat(&self) -> String {
        let mut out = String::new();
        match self {
            Self::Empty => {}
            Self::Para(text) => out.push_str(text),
            Self::Items(list) => flat_items(&mut out, list),
            Self::Sections(secs) => {
                for section in secs.iter() {
                    flat_section(&mut out, section);
                }
            }
        }
        out
    }

    /// Render-order pieces for display export: a `Label` before each labelled
    /// section's blocks, then that section's [`TextBlock`]s. The shortcut
    /// variants yield a single unlabelled section.
    pub fn pieces(&self) -> Vec<TextPiece> {
        let mut out = Vec::new();
        match self {
            Self::Empty => {}
            Self::Para(text) => out.push(TextPiece::Block(TextBlock::Para(text))),
            Self::Items(list) => out.push(TextPiece::Block(TextBlock::Items(list))),
            Self::Sections(secs) => {
                for section in secs.iter() {
                    if !section.label.is_empty() {
                        out.push(TextPiece::Label(section.label));
                    }
                    for block in section.blocks.iter() {
                        out.push(TextPiece::Block(*block));
                    }
                }
            }
        }
        out
    }
}

impl Default for CardText {
    fn default() -> Self {
        Self::Empty
    }
}

/// One display piece: a section label line, or a block of prose / bullets.
#[derive(Clone, Copy)]
pub enum TextPiece {
    Label(&'static str),
    Block(TextBlock),
}

fn flat_section(out: &mut String, section: &TextSection) {
    if !section.label.is_empty() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(section.label);
    }
    for block in section.blocks.iter() {
        match block {
            TextBlock::Para(text) => {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(text);
            }
            TextBlock::Items(list) => flat_items(out, list),
            TextBlock::Steps(list) => {
                for (n, step) in list.iter().enumerate() {
                    out.push('\n');
                    out.push_str(&(n + 1).to_string());
                    out.push_str(". ");
                    out.push_str(step);
                }
            }
        }
    }
}

fn flat_items(out: &mut String, list: &[TextItem]) {
    for (n, item) in list.iter().enumerate() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push('（');
        out.push_str(&(n + 1).to_string());
        out.push('）');
        out.push_str(item.text);
        for (m, sub) in item.sub.iter().enumerate() {
            out.push('\n');
            out.push_str(&(m + 1).to_string());
            out.push_str(". ");
            out.push_str(sub);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // `&'static` slices must be promoted at the const item, so the fixtures are
    // consts -- the same shape a card def uses (`const CARD: CardDef = …`).

    const SIMPLE: CardText = CardText::para("获得2个PAREO标记，然后视为你的房屋总数增加。");

    const UNLABELLED: CardText = CardText::items(&[
        it("每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）"),
        it("进行任意掷骰后，可选择使用一个[火罐]再投一次骰子并择其一执行"),
    ]);

    const STEPS: CardText = CardText::of(&[sec(
        "[手]：",
        &[
            para("依次进行以下效果："),
            steps(&[
                "为[使用者]的Pastel✽Palettes乐队卡添加2个[奇迹水晶]；",
                "为[使用者]所有效果包含[奇迹水晶]的卡添加1个[奇迹水晶]，如果[共鸣]则改为添加2个。",
            ]),
        ],
    )]);

    const NESTED: CardText = CardText::of(&[sec(
        "[持续]：",
        &[items(&[
            it("手卡上限数量减1。"),
            it("回合结束时添加1个[奇迹水晶]。"),
            it_sub(
                "此卡拥有至少5个[奇迹水晶]时根据此卡上的标记进行一下操作随后进入弃卡区：",
                &[
                    "每个单色效果为获得1层状态“下次盖房的价格减少1000（可溢出），盖房后减少1层”；",
                    "每个双色效果为将自己的所有反面[P✽P粉丝]变正。",
                ],
            ),
        ])],
    )]);

    const XREF: CardText = CardText::items(&[
        it("如果你的（2）技能或角色卡被复制，当复制者使x重置为0时，对方失去该技能"),
        it("（抵押或赎回以外）当你即将获得或失去资金时翻倍"),
    ]);

    const TWO_STATES: CardText = CardText::of(&[
        sec("状态1：", &[items(&[it("进入状态2。")])]),
        sec("状态2：", &[items(&[it("保留至多一张手牌。"), it("每回合开始时打出一张。")])]),
    ]);

    const PROSE: CardText = CardText::of(&[sec(
        "[手]：",
        &[para("为所有效果包含[奇迹水晶]的卡添加1个。")],
    )]);

    #[test]
    fn flat_simple_para() {
        assert_eq!(SIMPLE.flat(), "获得2个PAREO标记，然后视为你的房屋总数增加。");
    }

    #[test]
    fn flat_unlabelled_items() {
        assert_eq!(
            UNLABELLED.flat(),
            "（1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）\n\
             （2）进行任意掷骰后，可选择使用一个[火罐]再投一次骰子并择其一执行"
        );
    }

    #[test]
    fn flat_labelled_section_with_steps() {
        assert_eq!(
            STEPS.flat(),
            "[手]：\n\
             依次进行以下效果：\n\
             1. 为[使用者]的Pastel✽Palettes乐队卡添加2个[奇迹水晶]；\n\
             2. 为[使用者]所有效果包含[奇迹水晶]的卡添加1个[奇迹水晶]，如果[共鸣]则改为添加2个。"
        );
    }

    #[test]
    fn flat_items_with_nested_sub_steps() {
        assert_eq!(
            NESTED.flat(),
            "[持续]：\n\
             （1）手卡上限数量减1。\n\
             （2）回合结束时添加1个[奇迹水晶]。\n\
             （3）此卡拥有至少5个[奇迹水晶]时根据此卡上的标记进行一下操作随后进入弃卡区：\n\
             1. 每个单色效果为获得1层状态“下次盖房的价格减少1000（可溢出），盖房后减少1层”；\n\
             2. 每个双色效果为将自己的所有反面[P✽P粉丝]变正。"
        );
    }

    #[test]
    fn flat_keeps_cross_references_in_item_prose() {
        // 「（2）技能」 names an effect; it stays inside item 1's prose and is
        // not re-emitted as a list marker of its own.
        let flat = XREF.flat();
        assert!(flat.contains("如果你的（2）技能或角色卡被复制"));
        assert!(flat.contains("（1）如果你的（2）技能"));
        assert_eq!(flat.matches("（1）").count(), 1);
        assert_eq!(flat.matches("（2）").count(), 2); // one xref + one item marker
    }

    #[test]
    fn flat_restarts_numbering_per_items_block() {
        assert_eq!(
            TWO_STATES.flat(),
            "状态1：\n（1）进入状态2。\n状态2：\n（1）保留至多一张手牌。\n（2）每回合开始时打出一张。"
        );
    }

    #[test]
    fn flat_empty() {
        assert_eq!(CardText::EMPTY.flat(), "");
        assert!(CardText::EMPTY.is_empty());
    }

    #[test]
    fn flat_keeps_prose_contiguous() {
        // The invariant the rule queries rely on: no piece of authored prose
        // may be split or reordered by the rendering.
        assert!(PROSE.flat().contains("效果包含[奇迹水晶]的卡"));
    }

    #[test]
    fn pieces_match_render_order() {
        let p = STEPS.pieces();
        assert_eq!(p.len(), 3);
        assert!(matches!(p[0], TextPiece::Label("[手]：")));
        assert!(matches!(p[1], TextPiece::Block(TextBlock::Para(_))));
        assert!(matches!(p[2], TextPiece::Block(TextBlock::Steps(_))));
    }
}