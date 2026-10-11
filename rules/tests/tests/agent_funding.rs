//! Land and dealer buy/build offers may be funded by mortgaging other deeds.
mod common;

use common::*;

fn land_at_dealer(t: &mut Table) {
    t.set_pos(0, tile("主要街道") - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    assert_eq!(t.expect_prompt().kind, "tile");
}

#[test]
fn dealer_purchase_can_be_funded_by_mortgage() {
    let mut t = Table::vanilla(2);
    let target = tile("偶像经纪公司");
    let fund = tile("购物中心");
    t.own(0, &[fund]);
    t.set_money(0, 100);
    land_at_dealer(&mut t);
    t.answer_tile(0, target).unwrap();
    let prompt = t.expect_prompt();
    assert_eq!(prompt.kind, "mortgage");
    assert_eq!(prompt.bid, 1500);
    assert!(prompt.items.contains(&fund.to_string()));
    t.answer_items(0, &[&fund.to_string()]).unwrap();
    assert_eq!(t.owner(target), Some(0));
    assert!(t.mortgaged(fund));
    assert_eq!(t.money(0), 0, "100 cash + 1500 mortgage - 1600 purchase");
}

#[test]
fn dealer_build_can_mortgage_another_deed_but_not_the_target() {
    let mut t = Table::vanilla(2);
    let target = tile("偶像经纪公司");
    let fund = tile("天文馆");
    t.own(0, &[target, fund]);
    t.set_money(0, 0);
    land_at_dealer(&mut t);
    t.answer_tile(0, target).unwrap();
    let prompt = t.expect_prompt();
    assert_eq!(prompt.kind, "mortgage");
    assert!(!prompt.items.contains(&target.to_string()));
    assert!(prompt.items.contains(&fund.to_string()));
    t.answer_items(0, &[&fund.to_string()]).unwrap();
    assert_eq!(t.houses(target), 1);
    assert!(!t.mortgaged(target));
    assert!(t.mortgaged(fund));
    assert_eq!(t.money(0), 300);
}

#[test]
fn dealer_mortgage_can_be_cancelled_without_charging_or_bankruptcy() {
    let mut t = Table::vanilla(2);
    let target = tile("偶像经纪公司");
    let fund = tile("购物中心");
    t.own(0, &[fund]);
    t.set_money(0, 100);
    land_at_dealer(&mut t);
    t.answer_tile(0, target).unwrap();
    assert_eq!(t.expect_prompt().kind, "mortgage");
    // Submitting too few deeds is rejected; cancelling is a separate answer.
    assert!(t.answer_items(0, &[]).is_err());
    t.answer_one(1).unwrap();
    assert_eq!(t.owner(target), None);
    assert!(!t.mortgaged(fund));
    assert_eq!(t.money(0), 100);
    assert!(!t.p(0).out());
}

#[test]
fn dealer_build_cannot_use_its_own_deed_as_the_only_funding() {
    let mut t = Table::vanilla(2);
    let target = tile("购物中心");
    t.own(0, &[target]);
    t.set_money(0, 500);
    land_at_dealer(&mut t);
    t.answer_tile(0, target).unwrap();
    assert!(
        t.prompt().is_none(),
        "cannot raise 1500 without mortgaging the target"
    );
    assert_eq!(t.houses(target), 0);
    assert!(!t.mortgaged(target));
    assert_eq!(t.money(0), 500);
    assert!(!t.p(0).out());
}

#[test]
fn dealer_cash_purchase_does_not_ask_for_a_mortgage() {
    let mut t = Table::vanilla(2);
    let target = tile("偶像经纪公司");
    t.set_money(0, 2000);
    land_at_dealer(&mut t);
    t.answer_tile(0, target).unwrap();
    assert!(t.prompt().is_none());
    assert_eq!(t.owner(target), Some(0));
    assert_eq!(t.money(0), 400);
}

#[test]
fn mandatory_rent_mortgage_cannot_be_cancelled() {
    let mut t = Table::vanilla(2);
    let target = tile("购物中心");
    let fund = tile("天文馆");
    t.own(1, &[target]);
    t.own(0, &[fund]);
    t.set_money(0, 0);
    t.set_pos(0, target - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    let prompt = t.expect_prompt();
    assert_eq!(prompt.kind, "mortgage");
    assert!(prompt.options.is_empty());
    assert!(
        t.answer_one(1).is_err(),
        "a mandatory rent cannot be declined"
    );
    t.answer_items(0, &[&fund.to_string()]).unwrap();
    assert_eq!(t.money(0), 1000);
    assert_eq!(t.money(1), 10300);
}

fn land_on_property(t: &mut Table, target: usize) {
    t.set_pos(0, target - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    assert!(t.prompt().is_none());
    assert_eq!(t.step(), 4);
}

#[test]
fn ordinary_purchase_gate_and_commit_include_mortgage_funds() {
    let mut t = Table::vanilla(2);
    let target = tile("偶像经纪公司");
    let fund = tile("购物中心");
    t.own(0, &[fund]);
    t.set_money(0, 100);
    land_on_property(&mut t, target);
    assert!(t.m.state().can_buy_here, "the purchase button is enabled");
    t.buy(0).unwrap();
    assert_eq!(t.expect_prompt().kind, "mortgage");
    assert_eq!(t.expect_prompt().bid, 1500);
    t.answer_items(0, &[&fund.to_string()]).unwrap();
    assert_eq!(t.owner(target), Some(0));
    assert!(t.mortgaged(fund));
    assert_eq!(t.money(0), 0);
}

#[test]
fn ordinary_purchase_mortgage_can_be_cancelled() {
    let mut t = Table::vanilla(2);
    let target = tile("偶像经纪公司");
    let fund = tile("购物中心");
    t.own(0, &[fund]);
    t.set_money(0, 100);
    land_on_property(&mut t, target);
    t.buy(0).unwrap();
    t.answer_one(1).unwrap();
    assert_eq!(t.owner(target), None);
    assert!(!t.mortgaged(fund));
    assert_eq!(t.money(0), 100);
    assert!(!t.p(0).out());
}

#[test]
fn ordinary_purchase_rejects_cash_plus_deeds_shortfall() {
    let mut t = Table::vanilla(2);
    let target = tile("偶像经纪公司");
    t.own(0, &[tile("小豆岛")]);
    t.set_money(0, 100);
    land_on_property(&mut t, target);
    assert!(!t.m.state().can_buy_here);
    assert!(t.buy(0).is_err());
    assert_eq!(t.owner(target), None);
    assert_eq!(t.money(0), 100);
}

#[test]
fn ordinary_build_gate_excludes_the_deed_being_upgraded() {
    let mut t = Table::vanilla(2);
    let target = tile("购物中心");
    t.own(0, &[target]);
    t.set_money(0, 500);
    land_on_property(&mut t, target);
    assert!(
        !t.m.state().can_build_here,
        "cannot fund construction with its own deed"
    );
    assert!(t.build(0).is_err());
    assert!(!t.mortgaged(target));
}

#[test]
fn ordinary_build_can_be_funded_by_another_deed() {
    let mut t = Table::vanilla(2);
    let target = tile("购物中心");
    let fund = tile("天文馆");
    t.own(0, &[target, fund]);
    t.set_money(0, 1000);
    land_on_property(&mut t, target);
    assert!(t.m.state().can_build_here);
    t.build(0).unwrap();
    let prompt = t.expect_prompt();
    assert_eq!(prompt.kind, "mortgage");
    assert!(!prompt.items.contains(&target.to_string()));
    t.answer_items(0, &[&fund.to_string()]).unwrap();
    assert_eq!(t.houses(target), 1);
    assert!(t.mortgaged(fund));
    assert!(!t.mortgaged(target));
    assert_eq!(t.money(0), 300);
}

#[test]
fn non_main_landing_offers_a_mortgage_funded_purchase() {
    let mut t = Table::vanilla(2);
    let target = tile("偶像经纪公司");
    let fund = tile("购物中心");
    t.own(0, &[fund]);
    t.set_money(0, 100);
    t.set_pos(0, target - 3);
    // Use the event-style side move, so landing offers the purchase immediately.
    t.m.world_mut().turn.plan.forced = true;
    t.give_play(0, "TEST:mover").unwrap();
    assert_eq!(t.expect_prompt().title.key(), "ask.buy.title");
    t.answer_one(0).unwrap();
    assert_eq!(t.expect_prompt().kind, "mortgage");
    t.answer_items(0, &[&fund.to_string()]).unwrap();
    assert_eq!(t.owner(target), Some(0));
    assert!(t.mortgaged(fund));
    assert_eq!(t.money(0), 0);
}

#[test]
fn force_purchase_keeps_its_existing_cash_gate() {
    let mut t = Table::vanilla(2);
    let target = tile("偶像经纪公司");
    let fund = tile("购物中心");
    t.own(1, &[target]);
    t.set_mortgaged(target, true);
    t.own(0, &[fund]);
    t.set_money(0, 2000);
    land_on_property(&mut t, target);
    assert_eq!(t.owner(target), Some(1));
    assert_eq!(t.money(0), 2000);
    assert!(!t.mortgaged(fund));
}
