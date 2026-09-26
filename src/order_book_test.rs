#![cfg(test)]

use std::{assert_matches};
use crate::{order_book::{MBOMsg, OrderBook}, types::{Action, Side}};

#[test]
#[should_panic(expected = "cant add order with side None")]
fn order_book_apply_add_side_is_none_panics() {
    let mut order_book = OrderBook::new();
    let add_msg = MBOMsg{
        ts_event: 1,
        instrument_id: 2,
        action: Action::Add,
        side: Side::None,
        price: 10000,
        quantity: 20,
        order_id: 12,
        flags: 3,
        sequence: 1,
    };
    order_book.apply_event(&add_msg);
}

#[test]
#[should_panic(expected = "already exists")]
fn order_book_apply_add_panics() {
    let mut order_book = OrderBook::new();
    let msg = MBOMsg{
        ts_event: 1000000,
        instrument_id: 67,
        action: Action::Add,
        side: Side::Ask,
        price: 10000,
        quantity: 20,
        order_id: 12,
        flags: 3,
        sequence: 1,
    };
    order_book.apply_event(&msg);
    order_book.apply_event(&msg);
}

#[test]
fn order_book_apply_add() {
    let mut order_book = OrderBook::new();
    let msg = MBOMsg{
        ts_event: 1000000,
        instrument_id: 67,
        action: Action::Add,
        side: Side::Ask,
        price: 10000,
        quantity: 20,
        order_id: 12,
        flags: 3,
        sequence: 1,
    };
    order_book.apply_event(&msg);
    assert!(order_book.asks.contains_key(&10000), "asks should contain the added price level");
    assert!(order_book.order_refs.contains_key(&12), "order_refs should contain the added order");
    assert_eq!(order_book.asks.len(), 1, "asks should have one entry after add");
    assert_eq!(order_book.bids.len(), 0, "bids should be empty after add");
    assert_eq!(order_book.order_refs.get(&12).unwrap().clone(), (Side::Ask, 10000), "order_refs should contain the added order");
}

#[test]
#[should_panic]
fn order_book_apply_modify_non_existing_order_panics() {
    let mut order_book = OrderBook::new();
    let mdf_msg = MBOMsg{
        ts_event: 1,
        instrument_id:2,
        action: Action::Modify,
        side: Side::Ask,
        price: 10000,
        quantity: 20,
        order_id: 12,
        flags: 3,
        sequence: 1,
    };
    order_book.apply_event(&mdf_msg);
}

#[test]
#[should_panic(expected = "sides dont match")]
fn order_book_apply_modify_sides_dont_match_panics() {
    let mut order_book = OrderBook::new();
        let add_msg = MBOMsg{
            ts_event: 1,
            instrument_id: 2,
            action: Action::Add,
            side: Side::Ask,
            price: 10000,
            quantity: 20,
            order_id: 12,
            flags: 3,
            sequence: 1,
        };
        let mdf_msg = MBOMsg{
            ts_event: 1,
            instrument_id: 2,
            action: Action::Modify,
            side: Side::Bid,
            price: 10000,
            quantity: 20,
            order_id: 12,
            flags: 3,
            sequence: 1,
        };
        order_book.apply_event(&add_msg);
        order_book.apply_event(&mdf_msg);
}


#[test]
fn order_book_apply_modify() {
    let mut order_book = OrderBook::new();
    let add_msg = MBOMsg{
        ts_event: 1,
        instrument_id: 2,
        action: Action::Add,
        side: Side::Ask,
        price: 10000,
        quantity: 20,
        order_id: 12,
        flags: 3,
        sequence: 1,
    };
    order_book.apply_event(&add_msg);
    let mdf_msg = MBOMsg{
        ts_event: 1,
        instrument_id: 2,
        action: Action::Modify,
        side: Side::Ask,
        price: 20000,
        quantity: 20,
        order_id: 12,
        flags: 3,
        sequence: 1,
    };
    order_book.apply_event(&mdf_msg);
    assert_eq!(order_book.asks.len(), 1);
    assert_eq!(order_book.bids.len(), 0);
    assert_eq!(order_book.order_refs.get(&12).copied(), Some((Side::Ask, 20000)));
    assert_eq!(order_book.order_refs.len(), 1);
    assert_matches!(order_book.asks.get(&10000), None);
    let mdf_msg2 = MBOMsg{
        quantity: 10,
        ..mdf_msg
    };
    order_book.
}

#[test]
#[should_panic]
fn order_book_cancel_non_existing_panics() {
    let mut order_book = OrderBook::new();
    let cancel_msg = MBOMsg{
        ts_event: 1,
        instrument_id: 2,
        action: Action::Cancel,
        side: Side::Ask,
        price: 10000,
        quantity: 20,
        order_id: 12,
        flags: 3,
        sequence: 1,
    };
    order_book.apply_event(&cancel_msg);
}
