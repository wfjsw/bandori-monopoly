//! Profile one recorded match frame: state build, tile-text compose, extras,
//! and JSON serialization. Throwaway harness for the 20 Hz investigation.
//!
//! cargo run --release -p rules-worker --example prof_view -- <rec.json> <frac>

use std::path::Path;
use std::time::Instant;

use game_core::engine::compose_tile_texts;
use game_core::record::{decode_record, Replayer};

fn main() {
    let mut args = std::env::args().skip(1);
    let rec_path = args.next().expect("record path");
    let frac: f64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(0.5);

    let ctx = rules_worker::Ctx::load(
        Path::new("/home/monopoly/bm-live/data"),
        Path::new("/home/monopoly/bm-live/dist/cards"),
    );
    let bytes = std::fs::read(&rec_path).expect("read record");
    let rec = decode_record(&bytes).expect("decode record");
    let stamp = ctx.stamp();
    let mut rp =
        Replayer::new_with_stamp(ctx.data.clone(), ctx.rules.clone(), &rec, &stamp, true).expect("replayer");
    let target = (rp.total_ticks() as f64 * frac) as u64;
    eprintln!("total_ticks={} target={}", rp.total_ticks(), target);
    let t0 = Instant::now();
    rp.seek(target).expect("seek");
    eprintln!("seek took {:?}", t0.elapsed());

    let m = rp.match_ref();
    let member = 1;

    let t = Instant::now();
    let _ = m.state();
    println!("state {:?}", t.elapsed());

    let t = Instant::now();
    let _ = compose_tile_texts(m.world(), &ctx.data, &*ctx.rules);
    println!("tile_texts {:?}", t.elapsed());

    let t = Instant::now();
    let extras = m.view_extras(member);
    println!("view_extras {:?} ({} bytes)", t.elapsed(), extras.to_string().len());

    macro_rules! timed { ($name:literal, $e:expr) => {{ let t = Instant::now(); let v = $e; println!("{} {:?} ({} bytes)", $name, t.elapsed(), serde_json::to_string(&v).map(|s| s.len()).unwrap_or(0)); v }} }
    let state = timed!("state2", m.state());
    let hand = timed!("hand_of", m.hand_of(member));
    let notes = timed!("hand_notes", m.hand_notes_of(member));
    let draw = timed!("draw_of", m.draw_of(member));
    let played = timed!("played_context", m.played_context());
    let quotes = timed!("tile_quotes", m.tile_quotes(member));
    let inspection = timed!("inspection", m.inspection_of(member));
    let frame = serde_json::json!({
        "state": state,
        "hand": hand,
        "handNotes": notes,
        "draw": draw,
        "you": member,
        "playerId": state.player_of(member),
        "played": played.0,
        "cardsPlayed": played.1,
        "tileQuotes": quotes,
        "inspection": inspection,
        "aiAnswer": extras.get("aiAnswer"),
        "playable": extras.get("playable"),
        "aiPlayable": extras.get("aiPlayable"),
        "estCost": extras.get("estCost"),
        "skills": extras.get("skills"),
    });

    let t = Instant::now();
    let json = serde_json::to_string(&frame).expect("serialize");
    println!("frame serialize {:?} ({} bytes)", t.elapsed(), json.len());
}
