pub mod order;
pub use crate::types::{MBOMsg, OrderId, Price, Quantity, Side, Timestamp};
pub use order::Order;

use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Display;
use std::panic;
use crate::types::Action;

const DISPLAY_WIDTH: usize = 80;

#[derive(Debug)]
pub struct PriceLevel {
    pub orders: Vec<Order>,
    total_qty: Quantity,
}

impl PriceLevel {
    pub fn new() -> Self {
        Self {
            orders: Vec::new(),
            total_qty: 0,
        }
    }

    pub fn add(&mut self, order: Order) {
        self.total_qty += order.qty;
        self.orders.push(order);
    }

    fn get(&self, id: OrderId) -> Option<&Order> {
        return self.orders.iter().find(|o| o.id == id)
    }

    fn get_mut(&mut self, id: OrderId) -> Option<&mut Order> {
        return self.orders.iter_mut().find(|o| o.id == id)
    }

    fn get_pos(&self, id: OrderId) -> Option<usize> {
        return self.orders.iter().position(|o| o.id == id);
    }

    /// returns the order before modifying
    pub fn modify(&mut self, id: OrderId, new_price: Price, new_qty: Quantity) -> Option<Order> {
        let old_order = self.get_mut(id)?;
        let old_qty = old_order.qty;
        let old_cp = *old_order;
        *old_order = Order { price: new_price, qty: new_qty, ..old_order.clone() };
        self.total_qty = self.total_qty + new_qty - old_qty;
        return Some(old_cp)
    }

    pub fn modify_idx(&mut self, order_pos: usize, new_price: Price, new_qty: Quantity) -> Option<Order> {
        let old_order = self.orders.get_mut(order_pos)?;
        let old_qty = old_order.qty;
        let old_cp = *old_order;
        *old_order = Order { price: new_price, qty: new_qty, ..old_order.clone() };
        self.total_qty = (self.total_qty as i64 + new_qty as i64 - old_qty as i64) as u32;
        return Some(old_cp)
    }


    /// returns the removed order
    pub fn remove(&mut self, id: OrderId) -> Option<Order> {
        let pos = self.orders.iter().position(|o| o.id == id)?;
        let order = self.orders.remove(pos);
        self.total_qty -= order.qty;
        return Some(order)
    }

    /// returns the removed order
    fn remove_idx(&mut self, pos: usize) -> Option<Order> {
        if pos >= self.orders.len() {
            return None;
        }
        let order = self.orders.remove(pos);
        self.total_qty -= order.qty;
        return Some(order)
    }

    fn is_empty(&self) -> bool {
        return self.orders.is_empty()
    }
}


impl Default for PriceLevel {
    fn default() -> Self {
        Self {
            orders: Vec::new(),
            total_qty: Quantity::default(),
        }

    }
}

pub struct OrderBook {
    pub asks: BTreeMap<Price, PriceLevel>,
    pub bids: BTreeMap<Reverse<Price>, PriceLevel>,
    pub order_refs: HashMap<OrderId, (Side, Price)>,
    pub last: i64,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            asks: BTreeMap::new(),
            bids: BTreeMap::new(),
            order_refs: HashMap::new(),
            last: i64::MAX,
        }
    }

    pub fn apply_event(&mut self, event: &MBOMsg) {
        let side = event.side;
        let action = event.action;
        match action {
            Action::Add => {
                let order = Order::from_event(&event);
                self.add(order);
            },
            Action::Modify => self.modify(event.order_id, event.ts_event, side, event.price, event.quantity),
            Action::Cancel => self.cancel(event.order_id),
            Action::Clear => self.clear(),
            Action::Trade => {},
            Action::Fill => {},
            Action::None => {}
        };
    }

    fn add(&mut self, order: Order) {
        match order.side {
            Side::Ask => {
                self.asks.entry(order.price).or_default().add(order);
            }
            Side::Bid => {
                self.bids.entry(Reverse(order.price)).or_default().add(order);
            }
            Side::None => unreachable!("cant add order with side None")
        }
        let old_id = self.order_refs.insert(order.id, (order.side, order.price));
        if old_id.is_some() {
            panic!("order id {} already exists", order.id);
        }
    }

    fn modify(&mut self, id: OrderId, ts: Timestamp, side: Side, price: Price, qty: Quantity) {
        let (old_side, old_price) = self.order_refs.get(&id).unwrap();
        assert_eq!(*old_side, side, "sides dont match");
        let level = match side {
            Side::Ask => self.asks.get_mut(old_price).unwrap(),
            Side::Bid => self.bids.get_mut(&Reverse(*old_price)).unwrap(),
            Side::None => unreachable!("cant modify order with side None"),
        };
        let old_idx = level.get_pos(id).expect(&format!("orders: {:?}, id: {}", &level.orders, id));
        let old_order = level.orders.get(old_idx).unwrap();
        let lost_priority = old_order.loses_priority(price, qty);
        if lost_priority {
            let mut order = level.remove_idx(old_idx).unwrap();
            let level_empty = level.orders.is_empty();
            if level_empty {
                match side {
                    Side::Ask => {self.asks.remove(old_price);},
                    Side::Bid => {self.bids.remove(&Reverse(*old_price));},
                    Side::None => unreachable!("cant modify order with side None")
                }
            }
            order.ts = ts;
            order.price = price;
            order.qty = qty;
            let level = match side {
                Side::Ask => {
                    self.asks.entry(price).or_default()
                },
                Side::Bid => {

                    self.bids.entry(Reverse(price)).or_default()
                },
                Side::None => unreachable!("cant modify"),
            };
            level.add(order);
        } else {
            level.modify_idx(old_idx, price, qty);
        }
        self.order_refs.get_mut(&id).unwrap().1 = price;
    }

    fn cancel(&mut self, id: OrderId) {
        let (old_side, old_price) = self.order_refs.get(&id).unwrap();
        let level = match old_side {
            Side::Ask => self.asks.get_mut(old_price),
            Side::Bid => self.bids.get_mut(&Reverse(*old_price)),
            Side::None => unreachable!("cant cancel order with side None"),
        }.unwrap();

        level.remove(id).expect("order not in level");

        if level.orders.is_empty() {
            match old_side {
                Side::Ask => self.asks.remove(old_price),
                Side::Bid => self.bids.remove(&Reverse(*old_price)),
                Side::None => unreachable!("cant cancel order with side None"),
            };
        }
        self.order_refs.remove(&id).expect("order not in order_refs");
    }

    fn clear(&mut self, ) {
        self.asks.clear();
        self.bids.clear();
        self.order_refs.clear();
    }
}

impl Display for OrderBook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{:^DISPLAY_WIDTH$}", "OrderBook")?;
        writeln!(f, "{:-^DISPLAY_WIDTH$}", "")?;
        for (price, level) in self.asks.iter() {
            writeln!(f, "{:<DISPLAY_WIDTH$}", format!("ASKS | price: {} | qty: {}", price, level.total_qty))?;
        }
        writeln!(f, "{:-^DISPLAY_WIDTH$}", "")?;
        writeln!(f, "LAST | price: {} | qty: {}", self.last, 0)?;
        writeln!(f, "{:-^DISPLAY_WIDTH$}", "")?;
        for (price, level) in self.bids.iter().rev() {
            writeln!(f, "{:<DISPLAY_WIDTH$}", format!("BIDS | price: {} | qty: {}", price.0, level.total_qty))?;
        }
        writeln!(f, "{:-^DISPLAY_WIDTH$}", "")?;
        Ok(())
    }
}
