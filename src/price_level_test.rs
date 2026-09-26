#![cfg(test)]

use crate::{order_book::{MBOMsg, OrderBook, PriceLevel}, types::{Action, Side}};

#[test]
fn price_level_add() {
    let mut price_level = PriceLevel::new();
    price_level.add(Order { id: 1, ts: 0, side: Side::Buy, price: 100, qty: 10 });
}
