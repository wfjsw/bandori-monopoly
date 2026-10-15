//! A pending prompt survives save → restore (server restart): same id, still answerable.

mod common;
use common::*;
use game_core::engine::Match;
use game_core::net::NetMessage;

#[test]
fn buy_prompt_answerable_after_restore() {
    let mut t = Table::vanilla(2);
    let land = tile("购物中心");
    t.set_money(0, 10_000);
    t.begin_turn(0);
    t.set_pos(0, (land + 59) % 60);
    t.dice(&[1]);
    t.roll(0).unwrap();
    let p = t.expect_prompt();
    let json = t.m.save();
    let mut m2 = Match::restore(data(), rules(), &json).unwrap();
    assert_eq!(m2.state().prompt.id, p.id);
    for _ in 0..50 {
        m2.tick(0.1);
    }
    assert_eq!(m2.state().prompt.id, p.id, "tick changed prompt");
    let r = m2.act(1, &NetMessage { prompt: p.id, value: 0, ..NetMessage::act("answer") });
    assert!(r.is_ok(), "{:?}", r.err().map(|e| e.key().to_string()));
}
