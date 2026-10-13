use dbn::MboMsg;

use crate::{order_book::{Price, Quantity, Side, Timestamp}, types::OrderId};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Order {
    pub id: u64,
    pub ts: Timestamp,
    pub side: Side,
    pub price: Price,
    pub qty: Quantity,
}

impl Order {
    pub fn new(id: OrderId, ts: Timestamp, side: Side, price: Price, qty: Quantity) -> Self {
        Self {
            id,
            ts,
            side,
            price,
            qty,
        }
    }
    pub fn from_event(event: &MboMsg) -> Self {
        let side = event.side().unwrap();
        Self {
            id: event.order_id,
            ts: event.hd.ts_event,
            side: side,
            price: event.price,
            qty: event.size,
        }
    }
    pub fn loses_priority(&self, new_price: Price, new_qty: Quantity) -> bool {
        return self.price != new_price || self.qty < new_qty;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_or_reduced_quantity_keeps_priority() {
        let order = Order::new(1, 100, Side::Bid, 1_000, 10);

        assert!(!order.loses_priority(1_000, 10));
        assert!(!order.loses_priority(1_000, 5));
        assert!(!order.loses_priority(1_000, 0));
    }

    #[test]
    fn increased_quantity_loses_priority() {
        let order = Order::new(1, 100, Side::Bid, 1_000, 10);

        assert!(order.loses_priority(1_000, 11));
    }

    #[test]
    fn price_change_loses_priority_regardless_of_quantity() {
        let order = Order::new(1, 100, Side::Bid, 1_000, 10);

        for price in [999, 1_001] {
            for qty in [5, 10, 11] {
                assert!(order.loses_priority(price, qty));
            }
        }
    }
}
