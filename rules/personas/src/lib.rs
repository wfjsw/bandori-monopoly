//! Roster rows (`PersonaDef`), one file per persona, named by the character --
//! metadata only, kept apart from the skill rules in `skills/skill-personas`.
//! Collected in [`roster_table`].

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

pub mod ako;
pub mod anon;
pub mod arisa;
pub mod aya;
pub mod chisato;
pub mod chuchu;
pub mod eve;
pub mod hagumi;
pub mod himari;
pub mod hina;
pub mod kanon;
pub mod kaoru;
pub mod kasumi;
pub mod kokoro;
pub mod layer;
pub mod lisa;
pub mod lock;
pub mod mana;
pub mod marina;
pub mod mashiro;
pub mod masking;
pub mod maya;
pub mod misaki;
pub mod moca;
pub mod mutsumi;
pub mod mutsumi_crychic;
pub mod nanami;
pub mod nyamu;
pub mod pareo;
pub mod ran;
pub mod rana;
pub mod rimi;
pub mod rinko;
pub mod roster_table;
pub mod rui;
pub mod saaya;
pub mod sakiko;
pub mod sakiko_crychic;
pub mod sayo;
pub mod soyo;
pub mod soyo_crychic;
pub mod tae;
pub mod taki;
pub mod taki_crychic;
pub mod toko;
pub mod tomoe;
pub mod tomori;
pub mod tomori_crychic;
pub mod tsugumi;
pub mod tsukushi;
pub mod tukushi;
pub mod uika;
pub mod uika_sumimi;
pub mod umiri;
pub mod yukina;
