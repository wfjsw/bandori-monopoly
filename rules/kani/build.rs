//! Generates `conf_sym_ops.rs` (HostOps -> `rules_kani::host::dispatch`).
//! Run on every build; output lands in OUT_DIR and is `include!`d by
//! `tests/conf_model.rs` so native card bodies can run against the symbolic
//! model without a Kani cfg.

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

const ARRAY_OPS: &[&str] = &[
    "roll", "roll_ask", "log", "effect", "flash", "no_flash", "flash_card", "flash_card_keyed",
    "flash_tile", "tile_count", "tile_named", "tile_owner", "player_pos",
    "tile_steps_ahead", "rent_of", "buy_price", "build_cost", "mortgage_value", "owned_count", "owned_at",     "tile_is", "tile_attr", "tile_in", "tile_group", "tile_price", "houses_of", "rent_houses_of",
    "set_houses", "add_house", "mortgaged_of", "set_mortgaged", "set_owner", "dist", "tile_forward", "neighbor",
    "players_on_count", "players_on_at", "player_count", "player_out", "others_count", "others_at", "money", "gain",
    "gain_typed", "pay", "pay_to", "pay_total", "pay_leg", "draw", "draw_event", "pay_rent",
    "offer_buy", "offer_force_buy", "offer_build", "apply_mortgage", "apply_redeem", "add_to_hand", "add_to_deck", "add_to_deck_at", "take_card", "cards_in",
    "to_discard", "hand_count", "hand_size", "discard_count", "deck_count", "discard_size", "discard_from_hand", "shuffle_into_deck",
    "place_card_at", "set_dest", "set_transfer_to_dest", "send_to_dest", "transfer_to_dest", "rent_unit_mult", "add_rent_unit_bonus", "teleport_to",
    "unplace_card", "event_expire", "event_is_active", "event_deck_push", "event_banish", "self_tile", "set_self_tile", "self_face_down",
    "set_self_face_down", "self_immune", "set_self_immune", "is_placed", "self_prop", "set_self_prop", "tile_prop", "set_tile_prop",
    "prop_at", "set_prop_at", "state_get", "state_set", "state_add", "slot", "set_slot", "inc_slot",
    "group_pool", "add_group_pool", "group_skill", "persona_skill", "group_skills", "add_group_skill", "invoke_skill", "raise_bought",
    "raise", "charge", "charge_max", "gain_charge", "give_stay", "give_stun", "give_exile", "give_extra_turn",
    "can_pay", "cant_move", "spend_charge", "stay_of", "stun_of", "exile_of", "turn_player", "round_no",
    "turn_key", "persona_is", "set_persona", "in_group", "opt_int", "opt_str", "opt_tile", "opt_price", "opt_ai",
    "ask", "trig_kind", "trig_player", "trig_target", "trig_tile", "trig_value", "trig_step", "trig_by_card",
    "trig_pay_is_rent", "trig_move_kind", "trig_move_resolve", "trig_move_tag", "trig_move_main", "trig_move_dir", "trig_move_remaining", "trig_move_from",
    "trig_move_total", "trig_cards", "trig_move_roll", "trig_roll_source", "trig_buy_kind", "trig_seller", "trig_price", "trig_set_price",
    "trig_deal_owner", "trig_set_deal_owner", "trig_deal_houses", "trig_set_deal_houses", "trig_deal_mortgaged", "trig_set_deal_mortgaged", "trig_set_reason", "trig_set_move_roll",
    "trig_set_pay_amount", "trig_set_pay_target", "trig_set_cancelled", "trig_set_negate_effect", "trig_set_spare", "trig_cancelled", "trig_seq", "trig_answers",
    "trig_effect_count", "trig_effect_kind", "trig_effect_target", "trig_effect_from", "trig_effect_tile", "trig_effect_value", "declare_effect", "trig_card_is",
    "play_card", "card_move", "card_replayable", "set_roller", "set_steps", "set_reverse", "set_signed", "set_stop_at",
    "set_parity", "set_resolve", "set_settle_tile", "set_pay_factor", "set_rent_factor", "set_no_buy", "set_no_build", "set_build_anywhere",
    "set_kind", "set_teleport_to", "set_start", "set_base_dice", "add_base_dice", "add_extra_dice", "move_stopped", "set_min_roll",
    "set_extra_steps", "set_more_steps", "set_tag", "set_settle_as_agent", "plan_add_follower", "move_stop_at", "move_parity", "move_resolve",
    "move_steps", "move_remaining", "move_total", "move_dir", "abnormal_count", "target", "targeted_count", "gains_this_turn",
    "placed_tile", "play_doubled", "set_play_doubled", "schedule_turn_end", "set_no_money_loss", "set_fixed_roll", "fixed_roll", "set_next_steps",
    "turn_main_steps", "add_charge_max", "card_build", "set_can_build", "clear_dice", "can_build_on", "card_buy", "buy_quotes",
    "ai_agent_choice", "buy", "acquire", "agent_offer", "linger", "card_immune", "card_mortgage", "card_text_mentions",
    "gain_fixed", "paid_in_settle", "place_card_on", "play_from_hand", "set_build_discount", "set_card_immune", "set_card_tile",
    "set_extreme", "turn_rolls", "turn_snap", "turn_start_pos", "unplace_card_named", "tok_names", "designations",
    "cancel_designation", "designation_cancelled", "gate", "card_settle_at", "card_offer_build", "do_move_roll", "placed_cards",
    "field_instances", "unplace_at", "tile_at", "set_tile_at", "is_face_down_at", "set_face_down_at", "is_immune_at", "set_immune_at",
    "set_build_cost_pct", "set_card_face_down", "card_face_down",
    "join_redirect_target", "set_join_used", "adjust_money", "card_prop",
];

fn main() {
    gen_cond_irs();
    gen_body_dispatch();
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("conf_sym_ops.rs");
    let mut s = String::new();
    writeln!(s, "// Generated by build.rs. Do not edit.").unwrap();
    writeln!(s, "use card_sdk::kani_host::Op;").unwrap();
    writeln!(s, "use game_rules::HostErr;").unwrap();
    writeln!(s, "use game_rules::native_shims::HostOps;").unwrap();
    writeln!(s, "use rules_kani::host;").unwrap();
    writeln!(s, "pub struct SymOps;").unwrap();
    writeln!(s, "impl HostOps for SymOps {{").unwrap();
    for name in ARRAY_OPS {
        writeln!(s, "    fn op_{name}(&mut self, a: [i32; 6]) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::{name}, a)) }}").unwrap();
    }
    // Custom-arity methods (packing matches `card_sdk::ctx::sys_kani`).
    writeln!(s, "    fn op_place_mark(&mut self, tile: i32, kp: i32, kl: i32, cp: i32, cl: i32, owner: i32, src: i32, count: i32, stack: i32, np: i32, nl: i32) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::place_mark, [tile, kp, kl, cp, cl, owner])) }}").unwrap();
    writeln!(s, "    fn op_count_marks_f(&mut self, tile: i32, fp: i32, fl: i32) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::count_marks_f, [tile, fp, fl, 0, 0, 0])) }}").unwrap();
    writeln!(s, "    fn op_bump_mark_f(&mut self, tile: i32, fp: i32, fl: i32, delta: i32) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::bump_mark_f, [tile, fp, fl, delta, 0, 0])) }}").unwrap();
    writeln!(s, "    fn op_remove_marks_f(&mut self, tile: i32, fp: i32, fl: i32) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::remove_marks_f, [tile, fp, fl, 0, 0, 0])) }}").unwrap();
    writeln!(s, "    fn op_mark_src_at(&mut self, tile: i32, fp: i32, fl: i32) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::mark_src_at, [tile, fp, fl, 0, 0, 0])) }}").unwrap();
    writeln!(s, "    fn op_mark_instance_at(&mut self, tile: i32, fp: i32, fl: i32) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::mark_instance_at, [tile, fp, fl, 0, 0, 0])) }}").unwrap();
    writeln!(s, "    fn op_counter_self(&mut self, np: i32, nl: i32) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::counter_self, [np, nl, 0, 0, 0, 0])) }}").unwrap();
    writeln!(s, "    fn op_add_counter_self(&mut self, np: i32, nl: i32, n: i32, max: i32) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::add_counter_self, [np, nl, n, max, 0, 0])) }}").unwrap();
    writeln!(s, "    fn op_set_counter_self(&mut self, np: i32, nl: i32, n: i32) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::set_counter_self, [np, nl, n, 0, 0, 0])) }}").unwrap();
    writeln!(s, "    fn op_counter_at(&mut self, uid: i32, np: i32, nl: i32) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::counter_at, [uid, np, nl, 0, 0, 0])) }}").unwrap();
    writeln!(s, "    fn op_add_counter_at(&mut self, uid: i32, np: i32, nl: i32, n: i32, max: i32) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::add_counter_at, [uid, np, nl, n, max, 0])) }}").unwrap();
    writeln!(s, "    fn op_card_counter(&mut self, player_id: i32, cp: i32, cl: i32, np: i32, nl: i32) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::card_counter, [player_id, cp, cl, np, nl, 0])) }}").unwrap();
    writeln!(s, "    fn op_add_card_counter(&mut self, player_id: i32, cp: i32, cl: i32, np: i32, nl: i32, n: i32, max: i32) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::add_card_counter, [player_id, cp, cl, np, nl, n])) }}").unwrap();
    writeln!(s, "    fn op_count_held(&mut self, np: i32, nl: i32, player_id: i32) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::count_held, [np, nl, player_id, 0, 0, 0])) }}").unwrap();
    writeln!(s, "    fn op_add_held(&mut self, np: i32, nl: i32, player_id: i32, n: i32, max: i32) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::add_held, [np, nl, player_id, n, max, 0])) }}").unwrap();
    writeln!(s, "    fn op_count_held_name(&mut self, np: i32, nl: i32, player_id: i32) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::count_held_name, [np, nl, player_id, 0, 0, 0])) }}").unwrap();
    writeln!(s, "    fn op_set_held_name(&mut self, np: i32, nl: i32, player_id: i32, v: i32) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::set_held_name, [np, nl, player_id, v, 0, 0])) }}").unwrap();
    writeln!(s, "    fn op_move_units(&mut self, np: i32, nl: i32, from_tile: i32, from_player: i32, to_tile: i32, to_player: i32, n: i32) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::move_units, [np, nl, from_tile, from_player, to_tile, to_player])) }}").unwrap();
    writeln!(s, "    fn op_self_uid(&mut self) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::self_uid, [0, 0, 0, 0, 0, 0])) }}").unwrap();
    writeln!(s, "    fn op_send(&mut self, tp: i32, tl: i32, np: i32, nl: i32, pp: i32, pl: i32) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::send, [tp, tl, np, nl, pp, pl])) }}").unwrap();
    writeln!(s, "    fn op_msg_sender_uid(&mut self) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::msg_sender_uid, [0, 0, 0, 0, 0, 0])) }}").unwrap();
    writeln!(s, "    fn op_msg_sender_seat(&mut self) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::msg_sender_seat, [0, 0, 0, 0, 0, 0])) }}").unwrap();
    writeln!(s, "    fn op_msg_name(&mut self, p: i32, n: i32) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::msg_name, [p, n, 0, 0, 0, 0])) }}").unwrap();
    writeln!(s, "    fn op_msg_a(&mut self) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::msg_a, [0, 0, 0, 0, 0, 0])) }}").unwrap();
    writeln!(s, "    fn op_msg_b(&mut self) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::msg_b, [0, 0, 0, 0, 0, 0])) }}").unwrap();
    writeln!(s, "    fn op_msg_c(&mut self) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::msg_c, [0, 0, 0, 0, 0, 0])) }}").unwrap();
    writeln!(s, "    fn op_msg_tile(&mut self) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::msg_tile, [0, 0, 0, 0, 0, 0])) }}").unwrap();
    writeln!(s, "    fn op_msg_seat(&mut self) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::msg_seat, [0, 0, 0, 0, 0, 0])) }}").unwrap();
    writeln!(s, "    fn op_msg_text(&mut self, p: i32, n: i32) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::msg_text, [p, n, 0, 0, 0, 0])) }}").unwrap();
    writeln!(s, "    fn op_msg_reply(&mut self, v: i32) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::msg_reply, [v, 0, 0, 0, 0, 0])) }}").unwrap();
    writeln!(s, "    fn op_trig_name(&mut self, p: i32, n: i32) -> Result<i64, HostErr> {{ Ok(host::dispatch(Op::trig_name, [p, n, 0, 0, 0, 0])) }}").unwrap();
    writeln!(s, "}}").unwrap();
    fs::write(&out, s).unwrap();

    // Fail the build if `HostOps` grew a method SymOps does not generate.
    // The flash family (op_flash / op_no_flash / op_flash_card /
    // op_flash_card_keyed / op_flash_tile) sat in the trait but not in
    // ARRAY_OPS on the merge-base, so `impl HostOps for SymOps` was missing
    // five required methods -- a silent compile break until someone built
    // rules-kani. Keep ARRAY_OPS + the custom-arity list honest.
    let shims = fs::read_to_string(
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("../../crates/game-rules/src/native_shims.rs"),
    )
    .expect("read native_shims.rs");
    let trait_body = shims
        .split("pub trait HostOps {")
        .nth(1)
        .map(|s| s.split("\n}").next().unwrap_or(""))
        .unwrap_or("");
    // Custom-arity names already written above (not in ARRAY_OPS).
    let custom = [
        "op_place_mark", "op_count_marks_f", "op_bump_mark_f", "op_remove_marks_f",
        "op_mark_src_at", "op_mark_instance_at", "op_counter_self", "op_add_counter_self",
        "op_set_counter_self", "op_counter_at", "op_add_counter_at", "op_card_counter",
        "op_add_card_counter", "op_count_held", "op_add_held", "op_count_held_name",
        "op_set_held_name", "op_move_units", "op_self_uid", "op_send", "op_msg_sender_uid",
        "op_msg_sender_seat", "op_msg_name", "op_msg_a", "op_msg_b", "op_msg_c", "op_msg_tile",
        "op_msg_seat", "op_msg_text", "op_msg_reply", "op_trig_name",
    ];
    let mut generated: Vec<String> = ARRAY_OPS.iter().map(|n| format!("op_{n}")).collect();
    generated.extend(custom.iter().map(|s| s.to_string()));
    let mut missing = Vec::new();
    let mut start = 0usize;
    while let Some(rel) = trait_body[start..].find("fn op_") {
        let at = start + rel + 3;
        let rest = &trait_body[at..];
        let end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        let name = &rest[..end];
        if !generated.iter().any(|g| g == name) {
            missing.push(name.to_string());
        }
        start = at + end;
    }
    if !missing.is_empty() {
        panic!(
            "rules-kani/build.rs: HostOps methods missing from ARRAY_OPS/custom list: {missing:?}. \
             Add them to ARRAY_OPS (or the custom-arity block) so SymOps implements HostOps."
        );
    }
}
/// Guard IR table: derived from the guard condition strings on the `On::*`
/// entries (`rules/{cards,skills,tiles,events,fixtures}`), regenerated on every
/// build so it can never drift. Output goes to `$OUT_DIR/cond_irs.rs` and is
/// `include!`d by `src/generated/mod.rs`.
fn gen_cond_irs() {
    let rules_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .parent()
        .unwrap()
        .to_path_buf();
    for d in ["cards", "skills", "tiles", "events", "fixtures"] {
        println!("cargo:rerun-if-changed={}", rules_dir.join(d).display());
    }
    let (src, _n) = rules_cond::gen_ir::generate(&rules_dir);
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("cond_irs.rs");
    fs::write(out, src).unwrap();
}

/// Per-card body trampolines + `call_body_idx` (Kani). Derived from
/// `invoke::BAND_START` / `invoke::RULESET` order on every build so it can
/// never drift. Output goes to `$OUT_DIR/body_dispatch_fn.rs` and is
/// `include!`d by `src/invoke.rs` (never committed — same rule as
/// `cond_irs.rs`).
///
/// Why: one shared `code*` call site (`body_of(on)(player)`) makes CBMC's
/// function-pointer removal expand an IF-chain over every address-taken
/// `fn(i32) -> Asked` (874–4313 targets). Each card gets its own
/// `#[inline(never)]` trampoline loading a constant `&CardDef` so each
/// function's points-to set is that card's entries only.
fn gen_body_dispatch() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let rules_dir = manifest.parent().unwrap().to_path_buf();
    for d in ["cards", "skills", "tiles", "events", "tile_marks"] {
        println!("cargo:rerun-if-changed={}", rules_dir.join(d).display());
    }
    println!(
        "cargo:rerun-if-changed={}",
        manifest.join("src/invoke.rs").display()
    );

    // RULESET order (same as `invoke::RULESET` cfg order). Only crates
    // enabled by cargo features are emitted — a `band-pp` build links 19
    // cards, not 285, so CBMC's `fn(i32)->Asked` set is tens not 874.
    const ALL_BANDS: &[(&str, &str)] = &[
        ("card_ag", "band-ag"),
        ("card_crychic", "band-crychic"),
        ("card_general", "band-general"),
        ("card_hhw", "band-hhw"),
        ("card_morfonica", "band-morfonica"),
        ("card_mujica", "band-mujica"),
        ("card_mygo", "band-mygo"),
        ("card_pp", "band-pp"),
        ("card_ppp", "band-ppp"),
        ("card_ras", "band-ras"),
        ("card_roselia", "band-roselia"),
        ("card_sumimi", "band-sumimi"),
        ("skill_groups", "skills-groups"),
        ("skill_personas", "skills-personas"),
        ("rules_tiles", "tiles"),
        ("rules_tile_marks", "tiles"),
        ("rules_events", "events"),
    ];
    let enabled: Vec<&str> = ALL_BANDS
        .iter()
        .filter(|(_, feat)| env::var_os(&format!("CARGO_FEATURE_{}", feat.replace('-', "_").to_uppercase())).is_some())
        .map(|(c, _)| *c)
        .collect();
    let GROUPS: Vec<&str> = enabled;

    // Band lengths come from each crate's CARDS array (not invoke.rs —
    // BAND_START itself is generated here for the enabled subset).
    let mut starts = vec![0usize];
    for crate_name in &GROUPS {
        let cards = parse_cards(&rules_dir, crate_name);
        let next = *starts.last().unwrap() + cards.len();
        starts.push(next);
    }
    if starts[GROUPS.len()] == 0 {
        panic!("rules-kani/build.rs: no card crates enabled (features)");
    }

    // Emit BAND_START for the enabled subset.
    {
        let mut bs = String::new();
        let _ = writeln!(bs, "// Generated by build.rs. Do not edit.");
        let _ = writeln!(bs, "pub const BAND_START: [usize; {}] = [", GROUPS.len() + 1);
        for s in &starts {
            let _ = writeln!(bs, "    {s},");
        }
        bs.push_str("];\n");
        let _ = writeln!(bs, "pub const ENABLED_BANDS: usize = {};", GROUPS.len());
        // Unrolled band select (zero-loop). Length follows the enabled set.
        bs.push_str("/// Greatest b with BAND_START[b] <= idx (unrolled).\n");
        bs.push_str("#[inline(always)]\n");
        bs.push_str("pub fn band_of(idx: usize) -> usize {\n");
        bs.push_str("    let mut lo = 0usize;\n");
        for b in 1..GROUPS.len() + 1 {
            let _ = writeln!(bs, "    if BAND_START[{b}] <= idx {{ lo = {b}; }}");
        }
        bs.push_str("    lo\n");
        bs.push_str("}\n");
        fs::write(
            PathBuf::from(env::var("OUT_DIR").unwrap()).join("band_start.rs"),
            bs,
        )
        .unwrap();
    }
    // Emit feature-aware card id table.
    gen_card_ids(&rules_dir, &GROUPS, &starts);

    let mut s = String::new();
    s.push_str("// Generated by build.rs (gen_body_dispatch). Do not edit.\n");
    s.push_str("// Named-const body trampolines + call_body_idx for Kani.\n\n");
    // Each trampoline **names** the pub const (`&card_pp::same_dream::
    // SAME_DREAM`), not `CARDS[i]` (a table of address-taken fns). CBMC
    // resolves one target from a single const initialiser — the pilot
    // invokers (`git show 91fe463:rules/kani/src/invoke.rs`) proved this.
    for (i, crate_name) in GROUPS.iter().enumerate() {
        let base = starts[i];
        let count = starts[i + 1] - starts[i];
        let cards = parse_cards(&rules_dir, crate_name);
        assert_eq!(
            cards.len(),
            count,
            "{crate_name}: CARDS has {} entries, BAND_START says {count}",
            cards.len()
        );
        for off in 0..count {
            let idx = base + off;
            let (module, konst) = &cards[off];
            let path = format!("{crate_name}::{module}::{konst}");
            // Integer body-id match → DIRECT calls through the cfg(kani)
            // `NAME_kani` wrappers (same-module). A shared `code*` /
            // `f(player)` call makes CBMC expand a ~379-target IF-chain over
            // every address-taken `fn(i32) -> Asked` in the binary (ppcost
            // root cause: BMC never finishes; goto 8.5 MB but symex walks
            // every arm). Direct calls are one body.
            let bodies = parse_on_bodies(crate_name, &rules_dir, module, konst);
            // Empty `On` is legal (e.g. `PP:[衍生]共鸣` is fuel-only).
            let _ = writeln!(
                s,
                "/// `{path}` — integer body-id match, direct calls (no code* IF-chain)."
            );
            s.push_str("#[cfg(kani)]\n#[inline(never)]\n");
            let _ = writeln!(
                s,
                "fn call_card_{idx}(entry_index: usize, player: i32) -> Asked {{"
            );
            if bodies.is_empty() {
                let _ = writeln!(s, "    let _ = (entry_index, player);");
                let _ = writeln!(s, "    fail_entry_oob({idx}, entry_index)");
            } else {
                // Read the module source once to check for `NAME_kani` wrappers.
                let mod_src = {
                    let p = crate_dir(&rules_dir, crate_name)
                        .join("src")
                        .join(format!("{module}.rs"));
                    std::fs::read_to_string(&p).unwrap_or_default()
                };
                s.push_str("    match entry_index {\n");
                for (ei, body) in bodies.iter().enumerate() {
                    // Self-healing: if `NAME_kani` is missing (a new On:: body
                    // without its wrapper), fall back to calling the plain body
                    // directly. The wrapper is an optimisation (direct call),
                    // not a semantic requirement — the plain body has the same
                    // signature. `wrapper_check` test still flags the gap.
                    let wrapper = format!("{body}_kani");
                    let target = if mod_src.contains(&format!("pub fn {wrapper}(")) {
                        format!("{crate_name}::{module}::{wrapper}")
                    } else {
                        format!("{crate_name}::{module}::{body}")
                    };
                    let _ = writeln!(s, "        {ei} => {target}(player),");
                }
                let _ = writeln!(s, "        _ => fail_entry_oob({idx}, entry_index),");
                s.push_str("    }\n");
            }
            s.push_str("}\n\n");
        }
    }
    s.push_str("/// Per-card body dispatch (Kani). Integer match → named-const trampoline.\n");
    s.push_str("///\n");
    s.push_str("/// `call_body_idx_c::<IDX>` is the preferred entry: the const generic\n");
    s.push_str("/// monomorphizes to ONE trampoline so CBMC never walks the other cards'\n");
    s.push_str("/// bodies (a runtime `match idx` keeps every arm live — the ppcost\n");
    s.push_str("/// residual after the code* cut). `call_body_idx` stays for call sites\n");
    s.push_str("/// that only have a runtime index.\n");
    s.push_str("#[cfg(kani)]\n#[inline(always)]\n");
    s.push_str("fn call_body_idx(idx: usize, entry_index: usize, player: i32) -> Asked {\n");
    s.push_str("    match idx {\n");
    for (i, _c) in GROUPS.iter().enumerate() {
        let base = starts[i];
        let count = starts[i + 1] - starts[i];
        for off in 0..count {
            let idx = base + off;
            let _ = writeln!(s, "        {idx} => call_card_{idx}(entry_index, player),");
        }
    }
    s.push_str("        _ => fail_unknown_idx(idx),\n");
    s.push_str("    }\n");
    s.push_str("}\n");
    s.push_str("\n/// Const-generic body dispatch: `IDX` folds to a single trampoline.\n");
    s.push_str("#[cfg(kani)]\n#[inline(always)]\n");
    s.push_str("fn call_body_idx_c<const IDX: usize>(entry_index: usize, player: i32) -> Asked {\n");
    s.push_str("    match IDX {\n");
    for (i, _c) in GROUPS.iter().enumerate() {
        let base = starts[i];
        let count = starts[i + 1] - starts[i];
        for off in 0..count {
            let idx = base + off;
            let _ = writeln!(s, "        {idx} => call_card_{idx}(entry_index, player),");
        }
    }
    s.push_str("        _ => fail_unknown_idx(IDX),\n");
    s.push_str("    }\n");
    s.push_str("}\n");

    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("body_dispatch_fn.rs");
    fs::write(out, s).unwrap();
}

/// Parse every `On::*` constructor in a card source and return the body fn
/// name (last argument) in declaration order. Matches
/// `tools/kani/add_kani_wrappers.py` so `NAME_kani` exists for each.
/// Strip `//` line comments and `/* */` block comments so `On::` in doc
/// comments is not mistaken for a constructor.
fn strip_rust_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

fn parse_on_bodies(
    crate_name: &str,
    rules_dir: &PathBuf,
    module: &str,
    konst: &str,
) -> Vec<String> {
    let lib = crate_dir(rules_dir, crate_name)
        .join("src")
        .join(format!("{module}.rs"));
    let raw = fs::read_to_string(&lib).unwrap_or_else(|e| panic!("read {lib:?}: {e}"));
    let src = strip_rust_comments(&raw);
    let src = src.as_str();
    let mut bodies = Vec::new();
    let mut search_from = 0;
    while let Some(rel) = src[search_from..].find("On::") {
        let start = search_from + rel;
        let after = start + 4;
        let mut name_end = after;
        while name_end < src.len()
            && (src.as_bytes()[name_end].is_ascii_alphanumeric() || src.as_bytes()[name_end] == b'_')
        {
            name_end += 1;
        }
        let _ = &src[after..name_end]; // variant name
        // skip whitespace to '('
        let mut p = name_end;
        while p < src.len() && src.as_bytes()[p].is_ascii_whitespace() {
            p += 1;
        }
        if p >= src.len() || src.as_bytes()[p] != b'(' {
            search_from = name_end;
            continue;
        }
        // balanced parens
        let mut depth = 0usize;
        let mut j = p;
        while j < src.len() {
            let c = src.as_bytes()[j];
            if c == b'(' {
                depth += 1;
            } else if c == b')' {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            j += 1;
        }
        let inner = &src[p + 1..j];
        // split top-level commas; last part is the body ident
        let mut parts: Vec<&str> = Vec::new();
        let mut depth2 = 0i32;
        let mut cur = 0usize;
        for (k, c) in inner.char_indices() {
            match c {
                '(' | '[' | '{' => depth2 += 1,
                ')' | ']' | '}' => depth2 -= 1,
                ',' if depth2 == 0 => {
                    parts.push(inner[cur..k].trim());
                    cur = k + 1;
                }
                _ => {}
            }
        }
        parts.push(inner[cur..].trim());
        // Trailing comma in `On::Hook(…, body,)` leaves an empty last part —
        // take the last **non-empty** ident instead (class B: aya_with's
        // `at_start,` / `on_pay,` trailing commas made the dispatch emit
        // `fail_entry_oob` for every entry).
        if let Some(last) = parts.iter().rev().find(|p| !p.trim().is_empty()) {
            let last = last.trim();
            if last
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
                && last.chars().next().map(|c| c.is_ascii_alphabetic() || c == '_') == Some(true)
            {
                bodies.push(last.to_string());
            }
        }
        search_from = j + 1;
    }
    bodies
}

/// Parse `pub const BAND_START: [usize; 18] = [ … ];` from `invoke.rs`.
fn parse_band_start(invoke: &str) -> Vec<usize> {
    let key = "pub const BAND_START:";
    let at = invoke.find(key).expect("BAND_START not found in invoke.rs");
    let rest = &invoke[at..];
    // Skip the type `[usize; 18]` — take the `= [` initializer.
    let eq = rest.find('=').expect("BAND_START =");
    let open = rest[eq..].find('[').expect("BAND_START init array");
    let open = eq + open;
    let close = rest[open..].find(']').expect("BAND_START init close");
    let body = &rest[open + 1..open + close];
    body.split(',')
        .map(|x| x.trim())
        .filter(|x| !x.is_empty())
        .map(|x| x.parse::<usize>().expect("BAND_START entry"))
        .collect()
}

/// Rust crate name → source dir under `rules/`.
fn crate_dir(rules_dir: &PathBuf, crate_name: &str) -> PathBuf {
    match crate_name {
        "rules_tile_marks" => rules_dir.join("tile_marks"),
        "rules_tiles" => rules_dir.join("tiles"),
        "rules_events" => rules_dir.join("events"),
        "skill_groups" => rules_dir.join("skills/skill-groups"),
        "skill_personas" => rules_dir.join("skills/skill-personas"),
        other => rules_dir.join("cards").join(other.replace('_', "-")),
    }
}

/// Parse a band crate's `lib.rs`: `use mod::CONST` + `CARDS = &[CONST, …]`.
fn parse_cards(rules_dir: &PathBuf, crate_name: &str) -> Vec<(String, String)> {
    let lib = crate_dir(rules_dir, crate_name).join("src/lib.rs");
    let t = fs::read_to_string(&lib).unwrap_or_else(|e| panic!("read {lib:?}: {e}"));
    let mut uses: std::collections::HashMap<String, String> = Default::default();
    for line in t.lines() {
        let line = line.trim().trim_start_matches("pub ").trim();
        let Some(rest) = line.strip_prefix("use ") else {
            continue;
        };
        let body = rest.trim_end_matches(';').trim();
        let Some((mod_name, items)) = body.split_once("::") else {
            continue;
        };
        let items = items.trim().trim_start_matches('{').trim_end_matches('}');
        for item in items.split(',') {
            let name = item.trim().split(" as ").next().unwrap_or("").trim();
            if !name.is_empty() {
                uses.insert(name.to_string(), mod_name.trim().to_string());
            }
        }
    }
    let body = cards_array_body(&t).unwrap_or_else(|| panic!("{crate_name}: CARDS array not found"));
    let mut out = Vec::new();
    for item in body.split(',') {
        let item = item.split("//").next().unwrap_or("").trim();
        if item.is_empty() {
            continue;
        }
        let module = uses
            .get(item)
            .unwrap_or_else(|| panic!("{crate_name}: no `use mod::{item}` for CARDS entry"));
        out.push((module.clone(), item.to_string()));
    }
    out
}

fn cards_array_body(t: &str) -> Option<String> {
    let at = t.find("CARDS:")?;
    let rest = &t[at..];
    let eq = rest.find('=')?;
    let open = rest[eq..].find("&[")?;
    let open = eq + open + 2;
    let close = rest[open..].find("];")?;
    Some(rest[open..open + close].to_string())
}

/// Feature-aware `card_ids.rs` (CARD_COUNT / IDS / card_id_const / card_id_opt).
fn gen_card_ids(rules_dir: &PathBuf, bands: &[&str], starts: &[usize]) {
    let mut ids: Vec<(String, String)> = Vec::new(); // (id string, path comment)
    for (i, crate_name) in bands.iter().enumerate() {
        let cards = parse_cards(rules_dir, crate_name);
        let count = starts[i + 1] - starts[i];
        assert_eq!(cards.len(), count, "{crate_name} CARDS vs BAND_START");
        for (module, konst) in &cards {
            // Recover the CardDef id string from the const's `CardDef::new("…"`
            let lib = crate_dir(rules_dir, crate_name).join("src").join(format!("{module}.rs"));
            let src = fs::read_to_string(&lib).unwrap_or_else(|e| panic!("read {lib:?}: {e}"));
            let id = extract_card_id(&src, konst)
                .unwrap_or_else(|| panic!("{crate_name}::{module}::{konst}: CardDef::new id not found"));
            ids.push((id, format!("{crate_name}::{module}::{konst}")));
        }
    }
    let mut s = String::new();
    s.push_str("// Generated by build.rs (gen_card_ids). Do not edit.\n");
    s.push_str("// Feature-aware id → index map for the enabled crate set.\n\n");
    let _ = writeln!(s, "pub const CARD_COUNT: usize = {};", ids.len());
    s.push_str("pub static IDS: &[&str] = &[\n");
    for (id, _) in &ids {
        let _ = writeln!(s, "    {id:?},");
    }
    s.push_str("];\n\n");
    s.push_str("pub fn card_id_opt(id: &str) -> Option<usize> {\n");
    s.push_str("    let mut i = 0usize;\n");
    s.push_str("    while i < IDS.len() {\n");
    s.push_str("        if IDS[i] == id {\n");
    s.push_str("            return Some(i);\n");
    s.push_str("        }\n");
    s.push_str("        i += 1;\n");
    s.push_str("    }\n");
    s.push_str("    None\n");
    s.push_str("}\n\n");
    // `card_id_const` — const fn, byte-compare over the enabled IDS
    // (`match` on `&str` is not const-stable). `card_id!` in
    // `generated/mod.rs` expands to `const ID: usize = card_id_const($s)`.
    s.push_str("pub const fn const_str_eq(a: &[u8], b: &[u8]) -> bool {\n");
    s.push_str("    if a.len() != b.len() { return false; }\n");
    s.push_str("    let mut i = 0;\n");
    s.push_str("    while i < a.len() {\n");
    s.push_str("        if a[i] != b[i] { return false; }\n");
    s.push_str("        i += 1;\n");
    s.push_str("    }\n");
    s.push_str("    true\n");
    s.push_str("}\n\n");
    s.push_str("pub const fn card_id_const(id: &str) -> usize {\n");
    s.push_str("    let b = id.as_bytes();\n");
    s.push_str("    let mut i = 0;\n");
    s.push_str("    while i < IDS.len() {\n");
    s.push_str("        if const_str_eq(b, IDS[i].as_bytes()) { return i; }\n");
    s.push_str("        i += 1;\n");
    s.push_str("    }\n");
    s.push_str("    panic!(\"card_id!: id not in the enabled feature set\")\n");
    s.push_str("}\n");
    fs::write(
        PathBuf::from(env::var("OUT_DIR").unwrap()).join("card_ids.rs"),
        s,
    )
    .unwrap();
}

/// Pull the card id out of a module source: `CardDef::new("…")` or
/// `CardDef::new(ID)` where `const ID: &str = "…"`.
fn extract_card_id(src: &str, konst: &str) -> Option<String> {
    let at = src.find(konst)?;
    let rest = &src[at..];
    let new = rest.find("CardDef::new(")?;
    let rest = &rest[new + "CardDef::new(".len()..];
    let rest = rest.trim_start();
    if rest.starts_with('"') {
        let rest = &rest[1..];
        let end = rest.find('"')?;
        return Some(rest[..end].to_string());
    }
    // `CardDef::new(ID, …)` — resolve `const ID: &str = "…"` in this file.
    let ident_end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    let ident = &rest[..ident_end];
    if ident.is_empty() {
        return None;
    }
    // Find `const IDENT: &str = "…"` (or `pub const`).
    let mut search = src;
    while let Some(pos) = search.find(ident) {
        let line_start = search[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let line_end = search[pos..].find('\n').map(|i| pos + i).unwrap_or(search.len());
        let line = &search[line_start..line_end];
        if line.contains("const") && line.contains("&str") {
            if let Some(q) = line.find('"') {
                let rest = &line[q + 1..];
                let end = rest.find('"')?;
                return Some(rest[..end].to_string());
            }
        }
        search = &search[pos + ident.len()..];
    }
    None
}
