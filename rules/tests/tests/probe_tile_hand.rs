//! Throwaway probe: does a `tile:` rule instance ever land in a hand?

mod common;

use common::Table;
use game_core::state::BotMentality;

#[test]
fn probe_tile_in_hand() {
    let mut t = Table::new(&["弦卷心", "奥泽美咲", "北泽育美", "松原花音"]);
    for k in 0..4 {
        t.make_bot(k, BotMentality::Standard);
    }
    let mut hits = 0;
    for tick in 0..4000 {
        for k in 0..4 {
            let hand = t.hand(k);
            for c in &hand {
                if c.starts_with("tile:") {
                    hits += 1;
                    eprintln!("HIT tick={tick} player={k} card={c} hand={hand:?}");
                    let ev = t.m.state().events.clone();
                    for e in ev.iter().rev().take(40) {
                        eprintln!("  ev {} {:?}", e.r#type, e.card);
                    }
                }
            }
        }
        if tick % 500 == 0 {
            let st = t.m.state();
            eprintln!(
                "tick {tick} phase={} step={} prompt={} hands={:?}",
                st.phase,
                st.step,
                st.prompt.id,
                (0..4).map(|k| t.hand(k).len()).collect::<Vec<_>>()
            );
        }
        if hits >= 3 {
            break;
        }
        bot_core::sim::pump_bots(&mut t.m);
        t.m.tick(1.0);
        if t.m.state().phase != "play" {
            eprintln!("game over at tick {tick}");
            break;
        }
    }
    eprintln!("hits={hits}");
    for k in 0..4 {
        eprintln!("final hand p{k}: {:?}", t.hand(k));
        eprintln!("final gy   p{k}: {:?}", t.m.world().hidden[k].graveyard);
    }
    eprintln!("board_discard: {:?}", t.m.world().st.board_discard);
    assert_eq!(hits, 0, "a tile rule instance reached a hand");
}
