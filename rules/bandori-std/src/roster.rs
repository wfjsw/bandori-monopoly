//! Game roster metadata (ABI v86): the typed character / band
//! rows and the card field keys, beside the rule code.
//!
//! `card-sdk` stays vocabulary-free -- [`card_sdk::meta::CardMeta`] carries the
//! shared card-row shape plus a generic `fields` map, and this module names the
//! keys this game puts in that map (`meta_key`). The typed
//! [`PersonaDef`] / [`GroupDef`] tables live with their skill rules
//! (`rules/personas`, `rules/skills/skill-groups`); the shared
//! ordinal tables that restore data-file order live in [`roster_order`].
//!
//! Display-only columns (rule text, art ids, accent colours, sheet cells, ...)
//! stay as fields on the defs -- that is the authoring surface -- but they no
//! longer ride [`MetaRow`] into `ruleset_metadata`. They are exported at build
//! time into `webui/src/generated/display.json` by `tools/export-display.mjs`
//! (`docs/DISPLAY.md`).

use alloc::string::String;
use alloc::vec::Vec;

use card_sdk::meta::{MetaRow, ToMetaRow};

/// Field / list / flag keys this game writes into [`card_sdk::meta::MetaRow`]'s
/// generic maps. Mirrored by the host converter in `game-rules`
/// (`roster_from_meta`).
///
/// Display-only columns are **not** listed here: those never enter `MetaRow`
/// (see `docs/DISPLAY.md`).
pub mod meta_key {
    /// Card row: the band the card belongs to (`group`).
    pub const GROUP: &str = "group";
    /// Card row: the exclusive owner character name (`owner`);
    /// `""` = not exclusive.
    pub const OWNER: &str = "owner";
    /// Character / band row: skill name (`skill`).
    pub const SKILL: &str = "skill";
    /// Character row: base character of a variant (`variantOf`).
    pub const VARIANT_OF: &str = "variantOf";
    /// Character row: exclusive card names (`exclusiveCards`).
    pub const EXCLUSIVE_CARDS: &str = "exclusiveCards";
    /// Character row: starting-deck preset card names (`preset`).
    pub const PRESET: &str = "preset";
    /// Band flag: members may carry each other's exclusives (`sharesExclusives`).
    pub const SHARES_EXCLUSIVES: &str = "sharesExclusives";
}

/// One playable character from `the roster`, beside its skill rule
/// (`rules/personas`). Byte-exact text: use raw string literals.
#[derive(Clone, Copy)]
pub struct PersonaDef {
    pub name: &'static str,
    pub display: &'static str,
    pub group: &'static str,
    pub color: &'static str,
    pub skill: &'static str,
    pub text: &'static str,
    pub cn_id: &'static str,
    pub art: &'static str,
    pub costume: &'static str,
    pub exclusive_cards: &'static [&'static str],
    pub preset: &'static [&'static str],
    pub variant_of: &'static str,
    pub variant_tag: &'static str,
}

impl PersonaDef {
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            display: name,
            group: "",
            color: "",
            skill: "",
            text: "",
            cn_id: "",
            art: "",
            costume: "",
            exclusive_cards: &[],
            preset: &[],
            variant_of: "",
            variant_tag: "",
        }
    }

    pub const fn display(mut self, display: &'static str) -> Self {
        self.display = display;
        self
    }
    pub const fn group(mut self, group: &'static str) -> Self {
        self.group = group;
        self
    }
    pub const fn color(mut self, color: &'static str) -> Self {
        self.color = color;
        self
    }
    pub const fn skill(mut self, skill: &'static str) -> Self {
        self.skill = skill;
        self
    }
    pub const fn text(mut self, text: &'static str) -> Self {
        self.text = text;
        self
    }
    pub const fn cn_id(mut self, cn_id: &'static str) -> Self {
        self.cn_id = cn_id;
        self
    }
    pub const fn art(mut self, art: &'static str) -> Self {
        self.art = art;
        self
    }
    pub const fn costume(mut self, costume: &'static str) -> Self {
        self.costume = costume;
        self
    }
    pub const fn exclusive_cards(mut self, cards: &'static [&'static str]) -> Self {
        self.exclusive_cards = cards;
        self
    }
    pub const fn preset(mut self, preset: &'static [&'static str]) -> Self {
        self.preset = preset;
        self
    }
    pub const fn variant_of(mut self, variant_of: &'static str) -> Self {
        self.variant_of = variant_of;
        self
    }
    pub const fn variant_tag(mut self, variant_tag: &'static str) -> Self {
        self.variant_tag = variant_tag;
        self
    }
}

impl ToMetaRow for PersonaDef {
    /// Runtime-only row. `text` is emitted: the ruleset queries it at runtime
    /// (`card_text_mentions` over `skill:<owner>:<skill>`, 「技能中含有…」).
    /// Display columns (`display` / `color` / `variantTag` / `art` / `costume`
    /// / `cnId`) stay on the def and go to `display.json` (`docs/DISPLAY.md`).
    fn to_meta_row(&self) -> MetaRow {
        MetaRow {
            kind: alloc_string("persona"),
            ordinal: 0,
            id: alloc_string(self.name),
            name: alloc_string(self.name),
            text: alloc_string(self.text),
            tags: Vec::new(),
            derived: false,
            fields: Vec::from([
                (alloc_string(meta_key::GROUP), alloc_string(self.group)),
                (alloc_string(meta_key::SKILL), alloc_string(self.skill)),
                (
                    alloc_string(meta_key::VARIANT_OF),
                    alloc_string(self.variant_of),
                ),
            ]),
            lists: Vec::from([
                (
                    alloc_string(meta_key::EXCLUSIVE_CARDS),
                    self.exclusive_cards
                        .iter()
                        .map(|s| alloc_string(s))
                        .collect(),
                ),
                (
                    alloc_string(meta_key::PRESET),
                    self.preset.iter().map(|s| alloc_string(s)).collect(),
                ),
            ]),
            flags: Vec::new(),
        }
    }
}

/// One band roster row, beside its skill rule
/// (`rules/skills/skill-groups`). Byte-exact text: use raw string literals.
#[derive(Clone, Copy)]
pub struct GroupDef {
    pub name: &'static str,
    pub short_name: &'static str,
    pub color: &'static str,
    pub skill: &'static str,
    pub text: &'static str,
    pub logo: &'static str,
    pub shares_exclusives: bool,
}

impl GroupDef {
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            short_name: "",
            color: "",
            skill: "",
            text: "",
            logo: "",
            shares_exclusives: false,
        }
    }

    pub const fn short_name(mut self, short_name: &'static str) -> Self {
        self.short_name = short_name;
        self
    }
    pub const fn color(mut self, color: &'static str) -> Self {
        self.color = color;
        self
    }
    pub const fn skill(mut self, skill: &'static str) -> Self {
        self.skill = skill;
        self
    }
    pub const fn text(mut self, text: &'static str) -> Self {
        self.text = text;
        self
    }
    pub const fn logo(mut self, logo: &'static str) -> Self {
        self.logo = logo;
        self
    }
    pub const fn shares_exclusives(mut self, shares: bool) -> Self {
        self.shares_exclusives = shares;
        self
    }
}

impl ToMetaRow for GroupDef {
    /// Runtime-only row. `text` is emitted: the ruleset queries it at runtime
    /// (`card_text_mentions` over `skill:<owner>:<skill>`, 「技能中含有…」).
    /// Display columns (`shortName` / `color` / `logo`) stay on the def and go
    /// to `display.json` (`docs/DISPLAY.md`).
    fn to_meta_row(&self) -> MetaRow {
        MetaRow {
            kind: alloc_string("group"),
            ordinal: 0,
            id: alloc_string(self.name),
            name: alloc_string(self.name),
            text: alloc_string(self.text),
            tags: Vec::new(),
            derived: false,
            fields: Vec::from([
                (alloc_string(meta_key::SKILL), alloc_string(self.skill)),
            ]),
            lists: Vec::new(),
            flags: Vec::from([(
                alloc_string(meta_key::SHARES_EXCLUSIVES),
                self.shares_exclusives,
            )]),
        }
    }
}

/// `alloc`/`std` bridge for the `String` literals the row builder needs.
fn alloc_string(s: &str) -> String {
    s.into()
}