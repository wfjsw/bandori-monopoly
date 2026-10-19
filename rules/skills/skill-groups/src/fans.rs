//! P✽P粉丝 facade -- the fan-counter API every card body already calls.
//!
//! The kind-owned host is [`rules_markers::pp_fans`] (`mark:pp_fans` in
//! `rules/markers`); this module re-exports its verbs so call sites keep the
//! `skill_groups::fans::*` signatures (zero churn). See MARKERS.md §7 for the
//! ownership model and the two-affiliation split.

pub use rules_markers::pp_fans::{
    already_granted, down, flip_down, flip_up, grant, initial_grant_once, total, up, DOWN, UP,
};