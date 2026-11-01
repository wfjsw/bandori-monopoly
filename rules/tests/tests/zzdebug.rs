mod common;
use common::*;
use game_core::state::Counter;

fn decline_all(t: &mut Table) {
    for _ in 0..10 {
        if t.prompt().is_none() {
            return;
        }
        t.decline();
    }
}

fn add_token(t: &mut Table, who: usize, name: &str, value: i32) {
    t.m.world_mut().st.players[who].tokens.push(Counter {
        name: name.into(),
        value,
        instance: None,
        max: 0,
    });
}

#[test]
fn dbg_nnm() {
    let mut t = Table::new(&["广町七深", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    t.place_raw(0, "Mor:（NNM）稍微努力了一下");
    eprintln!("after place_raw: {:?}", t.field(0));
    add_token(&mut t, 0, "角色标记:skill:户山香澄:非凡之星", 1);
    t.set_fire(0, 1, 3);
    t.skill(0, "Mor:（NNM）稍微努力了一下").unwrap();
    eprintln!("after skill: {:?}", t.field(0));
    t.answer_one(0).unwrap();
    t.answer_one(2).unwrap();
    eprintln!("after (3): {:?}", t.field(0));
    eprintln!("prompt: {}", t.dump_prompt());
    t.answer_one(1).unwrap();
    decline_all(&mut t);
    eprintln!("after decline: {:?}", t.field(0));
}

#[test]
fn dbg_starry() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "Mor:蝴蝶飞舞的星月夜").unwrap();
    t.answer_one(1).unwrap();
    t.set_pos(0, 50);
    t.dice(&[3, 7]);
    t.roll(0).unwrap();
    eprintln!("prompt: {}", t.dump_prompt());
    t.answer_one(0).unwrap();
    eprintln!("prompt2: {}", t.dump_prompt());
    decline_all(&mut t);
    eprintln!("pos: {}", t.pos(0));
}
