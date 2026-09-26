use crate::order_book::{MBOMsg, Price, Quantity, Side, Timestamp};

#[derive(Clone, Copy, Debug)]
pub struct Order {
    pub id: u64,
    pub ts: Timestamp,
    pub side: Side,
    pub price: Price,
    pub qty: Quantity,
}

impl Order {
    pub fn new(id: u64, ts: u64, side: Side, price: i64, qty: u32) -> Self {
        Self {
            id,
            ts,
            side,
            price,
            qty,
        }
    }
    pub fn from_event(event: &MBOMsg) -> Self {
        Self {
            id: event.order_id,
            ts: event.ts_event,
            side: event.side,
            price: event.price,
            qty: event.quantity,
        }
    }
    pub fn loses_priority(&self, new_price: Price, new_qty: Quantity) -> bool {
        return self.price != new_price || self.qty < new_qty;
    }
}
