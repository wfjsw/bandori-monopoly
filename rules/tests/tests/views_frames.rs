//! The shared-base quote path (`tile_quotes_base` + `tile_quotes_viewer`) that
//! `views()` uses: one seat-independent pass per batch, one cheap overlay per
//! seat. The overlay must reproduce the single-call answer exactly.

mod common;

use common::{characters, data, Table};

fn ten() -> Vec<String> {
    characters().into_iter().take(10).collect()
}

fn boards() -> Vec<usize> {
    (0..data().tiles.len())
        .filter(|&i| data().tiles[i].is_buyable())
        .collect()
}

/// Own / mortgage / build a spread of tiles: every third stays unowned, the
/// rest go round-robin, every fourth is mortgaged and every fifth built.
fn arrange(t: &mut Table) {
    let n = t.n;
    for (k, &tile) in boards().iter().enumerate() {
        if k % 3 == 2 {
            continue;
        }
        let w = t.m.world_mut();
        w.set_tile_owner(tile as i32, (k % n) as i32);
        if k % 4 == 0 {
            w.st.mortgaged[tile] = true;
        }
        if k % 5 == 0 {
            w.st.houses[tile] = 1;
        }
    }
}

#[test]
fn the_viewer_overlay_reproduces_the_single_call() {
    let chars = ten();
    let cs: Vec<&str> = chars.iter().map(String::as_str).collect();
    let mut t = Table::new(&cs);
    let base = t.m.tile_quotes_base();
    for member in 1..=t.n as i32 {
        assert_eq!(t.m.tile_quotes(member), t.m.tile_quotes_from(&base, member), "before any deed");
    }
    arrange(&mut t);
    let base = t.m.tile_quotes_base();
    for member in 1..=t.n as i32 {
        assert_eq!(t.m.tile_quotes(member), t.m.tile_quotes_from(&base, member), "member {member}");
    }
}

#[test]
fn an_unknown_member_keeps_the_seat_independent_reading() {
    let chars = ten();
    let cs: Vec<&str> = chars.iter().map(String::as_str).collect();
    let mut t = Table::new(&cs);
    arrange(&mut t);
    let base = t.m.tile_quotes_base();
    assert_eq!(t.m.tile_quotes(-1), base);
    assert_eq!(t.m.tile_quotes_from(&base, -1), base);
}

#[test]
fn bench_frame_work() {
    let chars = ten();
    let cs: Vec<&str> = chars.iter().map(String::as_str).collect();
    let t = Table::new(&cs);
    let m = &t.m;
    let n = 10;
    for who in 1..=n {
        let _ = m.view_extras_typed(who);
    }
    let t0 = std::time::Instant::now();
    let base = m.tile_quotes_base();
    let base_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let t0 = std::time::Instant::now();
    for who in 1..=n {
        let _ = m.tile_quotes_from(&base, who);
    }
    let new_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let t0 = std::time::Instant::now();
    for who in 1..=n {
        let _ = m.tile_quotes(who);
    }
    let old_ms = t0.elapsed().as_secs_f64() * 1000.0;
    println!("quotes: base={base_ms:.2}ms old(10 seats)={old_ms:.2}ms new(base+10)={new_ms:.2}ms");
}


