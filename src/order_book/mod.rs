pub mod order;
pub mod types;
use crate::order_book::order::Priority;
use dbn::{Action, BidAskPair, MboMsg, UNDEF_PRICE};
pub use order::Order;
pub use types::{OrderId, Price, Quantity, Side, Timestamp};

use std::cmp::Ordering;
use std::collections::btree_map::Entry as BEntry;
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap};
use std::fmt::{Display, Write};
use std::panic;

const DISPLAY_WIDTH: usize = 80;

#[derive(Debug)]
pub struct PriceLevel {
    pub orders: Vec<Order>,
}

impl PriceLevel {
    pub fn new() -> Self {
        Self { orders: Vec::new() }
    }

    pub fn add(&mut self, order: Order) {
        return self.orders.push(order);
    }

    pub fn find(&self, id: OrderId) -> Option<&Order> {
        return self.orders.iter().find(|o| o.id == id);
    }

    pub fn find_mut(&mut self, id: OrderId) -> Option<&mut Order> {
        return self.orders.iter_mut().find(|o| o.id == id);
    }

    pub fn get_pos(&self, id: OrderId) -> Option<usize> {
        return self.orders.iter().position(|o| o.id == id);
    }

    pub fn is_empty(&self) -> bool {
        return self.orders.is_empty();
    }

    pub fn total_qty(&self) -> Quantity {
        return self.orders.iter().map(|o| o.qty).sum();
    }
}

impl Default for PriceLevel {
    fn default() -> Self {
        Self { orders: Vec::new() }
    }
}

#[derive(Debug)]
pub struct OrderBook {
    pub asks: BTreeMap<Price, PriceLevel>,
    pub bids: BTreeMap<Price, PriceLevel>,
    order_refs: HashMap<OrderId, (Side, Price)>,
    pub last_trade: (Price, Quantity),
    pub last_msg: MboMsg,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            asks: BTreeMap::new(),
            bids: BTreeMap::new(),
            order_refs: HashMap::new(),
            last_trade: (Price::MAX, Quantity::MAX),
            last_msg: MboMsg::default(),
        }
    }

    pub fn apply_event(&mut self, mbo: &MboMsg) -> Option<Order> {
        self.last_msg = *mbo;
        if mbo.price == UNDEF_PRICE && mbo.flags.is_tob() {
            self.get_half_book(mbo.side().unwrap()).clear();
        }
        match mbo.action().unwrap() {
            Action::Add => self.add(mbo),
            Action::Modify => self.modify(mbo),
            Action::Cancel => return self.cancel(mbo),
            Action::Clear => self.clear(),
            Action::Trade => {
                if mbo.side().unwrap() != Side::None {
                    self.update_last(mbo.price, mbo.size)
                }
            }
            Action::Fill => (),
            Action::None => (),
        };
        None
    }

    fn add(&mut self, mbo: &MboMsg) {
        if mbo.flags.is_tob() {
            let levels = self.get_half_book(mbo.side().unwrap());
            levels.clear();
            let order = Order::from_event(mbo);
            levels.entry(mbo.price).or_default().add(order);
            self.order_refs.clear();
        } else {
            let Entry::Vacant(entry) = self.order_refs.entry(mbo.order_id) else {
                panic!("duplicate id");
            };
            entry.insert((mbo.side().unwrap(), mbo.price));
            let order = Order::from_event(mbo);
            let level = self
                .get_half_book(mbo.side().unwrap())
                .entry(mbo.price)
                .or_default();
            level.add(order);
        }
        self.order_refs
            .insert(mbo.order_id, (mbo.side().unwrap(), mbo.price));
    }

    fn modify(&mut self, mbo: &MboMsg) {
        let Some(&(order_side, order_price)) = self.order_refs.get(&mbo.order_id) else {
            return self.add(&mbo);
        };
        assert_eq!(order_side, mbo.side().unwrap());
        let levels = self.get_half_book(order_side);
        let level = levels.get_mut(&order_price).unwrap();
        let order_idx = level.get_pos(mbo.order_id).unwrap();
        let order = level.orders.get_mut(order_idx).unwrap();
        let priority = order.loses_priority(mbo.price, mbo.size);
        order.price = mbo.price;
        order.qty = mbo.size;

        match priority {
            Priority::Kept => (),
            Priority::LostSize => {
                let order = level.orders.remove(order_idx);
                level.add(order);
            }
            Priority::LostPrice => {
                let BEntry::Occupied(mut entry) = levels.entry(order_price) else {
                    panic!("level not found");
                };
                let level = entry.get_mut();
                let order = level.orders.remove(order_idx);
                if level.is_empty() {
                    entry.remove();
                    assert!(!levels.contains_key(&order_price));
                }
                levels.entry(mbo.price).or_default().add(order);
                self.order_refs
                    .insert(mbo.order_id, (order_side, mbo.price));
            }
        }
    }

    fn cancel(&mut self, mbo: &MboMsg) -> Option<Order> {
        let levels = self.get_half_book(mbo.side().unwrap());
        let level = levels.get_mut(&mbo.price).unwrap();
        let order_idx = level.get_pos(mbo.order_id).unwrap();
        let order = level.find_mut(mbo.order_id).unwrap();
        let old_order = match order.qty.cmp(&mbo.size) {
            Ordering::Less => panic!("canceling bigger size than availible"),
            Ordering::Equal => {
                let order = level.orders.remove(order_idx);
                if level.is_empty() {
                    levels.remove(&mbo.price).unwrap();
                }
                self.order_refs.remove(&mbo.order_id);
                Some(order)
            }
            Ordering::Greater => {
                order.qty -= mbo.size;
                None
            }
        };
        old_order
    }

    fn get_half_book(&mut self, side: Side) -> &mut BTreeMap<Price, PriceLevel> {
        match side {
            Side::Ask => &mut self.asks,
            Side::Bid => &mut self.bids,
            Side::None => unreachable!(),
        }
    }

    fn update_last(&mut self, price: Price, qty: Quantity) {
        self.last_trade = (price, qty);
    }

    fn clear(&mut self) {
        self.asks.clear();
        self.bids.clear();
        self.order_refs.clear();
    }

    fn write_n_levels(&self, n: usize) -> String {
        let mut out = String::new();
        let asks: Vec<_> = self.asks.iter().take(n).collect();

        writeln!(out, "{:-^DISPLAY_WIDTH$}", "").unwrap();
        for (price, level) in asks.into_iter().rev() {
            writeln!(
                out,
                "{:<DISPLAY_WIDTH$}",
                format!(
                    "ASKS | price: {} | qty: {} | orders: {}",
                    price,
                    level.total_qty(),
                    level.orders.len()
                )
            )
            .unwrap();
        }
        writeln!(out, "{:-^DISPLAY_WIDTH$}", "").unwrap();
        writeln!(
            out,
            "{:<DISPLAY_WIDTH$}",
            format!(
                "LAST | price: {} | qty: {}",
                self.last_trade.0, self.last_trade.1
            )
        )
        .unwrap();
        writeln!(out, "{:-^DISPLAY_WIDTH$}", "").unwrap();
        for (price, level) in self.bids.iter().rev().take(n) {
            writeln!(
                out,
                "{:<DISPLAY_WIDTH$}",
                format!(
                    "BIDS | price: {} | qty: {} | orders: {}",
                    price,
                    level.total_qty(),
                    level.orders.len()
                )
            )
            .unwrap();
        }
        writeln!(out, "{:-^DISPLAY_WIDTH$}", "").unwrap();

        return out;
    }

    pub fn get_n_levels(&self, n: usize) -> Vec<BidAskPair> {
        let mut levels: Vec<BidAskPair> = Vec::with_capacity(n);
        let mut bids = self.bids.iter().rev();
        let mut asks = self.asks.iter();

        for _ in 0..n {
            let level = levels.push_mut(BidAskPair::default());
            if let Some(ask_level) = asks.next() {
                level.ask_px = *ask_level.0;
                level.ask_sz = ask_level.1.total_qty();
                level.ask_ct = ask_level.1.orders.len() as _;
            }
            if let Some(bid_level) = bids.next() {
                level.bid_px = *bid_level.0;
                level.bid_sz = bid_level.1.total_qty();
                level.bid_ct = bid_level.1.orders.len() as _;
            }
        }
        return levels;
    }
}

impl Display for OrderBook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const LEVELS: usize = 10;
        writeln!(f, "{}", self.write_n_levels(LEVELS))
    }
}

#[cfg(test)]
mod book_tests {
    use super::*;
    use dbn::{RecordHeader, rtype};

    fn event(
        action: Action,
        order_id: OrderId,
        price: Price,
        side: Side,
        size: Quantity,
    ) -> MboMsg {
        let mut hd = RecordHeader::default::<MboMsg>(rtype::MBO);
        hd.ts_event = 42;
        MboMsg {
            hd,
            order_id,
            price,
            size,
            action: action as _,
            side: side as _,
            ..MboMsg::default()
        }
    }

    #[test]
    fn new_book_values() {
        let book = OrderBook::new();

        assert!(book.bids.is_empty());
        assert!(book.asks.is_empty());
        assert!(book.order_refs.is_empty());
        assert_eq!(book.last_trade, (Price::MAX, Quantity::MAX));
    }

    #[test]
    fn add_creates_orders_on_both_sides() {
        let mut book = OrderBook::new();
        let bid = event(Action::Add, 1, 100, Side::Bid, 10);
        let ask = event(Action::Add, 2, 101, Side::Ask, 20);

        assert_eq!(book.apply_event(&bid), None);
        assert_eq!(book.apply_event(&ask), None);

        assert_eq!(book.bids[&100].orders, vec![Order::from_event(&bid)]);
        assert_eq!(book.asks[&101].orders, vec![Order::from_event(&ask)]);
        assert_eq!(book.order_refs.len(), 2);
        assert_eq!(book.order_refs[&1], (Side::Bid, 100));
        assert_eq!(book.order_refs[&2], (Side::Ask, 101));
        assert_eq!(book.last_msg.order_id, ask.order_id);
        assert_eq!(book.last_msg.hd.ts_event, ask.hd.ts_event);
    }

    #[test]
    fn orders_at_same_price_keep_insertion_order_and_sum_quantity() {
        let mut book = OrderBook::new();
        let first = event(Action::Add, 1, 100, Side::Bid, 10);
        let second = event(Action::Add, 2, 100, Side::Bid, 20);
        book.apply_event(&first);
        book.apply_event(&second);

        assert_eq!(book.bids.len(), 1);
        assert_eq!(book.bids[&100].total_qty(), 30);
        assert_eq!(
            book.bids[&100].orders,
            vec![Order::from_event(&first), Order::from_event(&second)]
        );
    }

    #[test]
    #[should_panic(expected = "duplicate id")]
    fn adding_duplicate_order_id_panics() {
        let mut book = OrderBook::new();
        book.apply_event(&event(Action::Add, 1, 100, Side::Bid, 10));
        book.apply_event(&event(Action::Add, 1, 101, Side::Ask, 20));
    }

    #[test]
    fn reducing_quantity_keeps_queue_priority() {
        let mut book = OrderBook::new();
        book.apply_event(&event(Action::Add, 1, 100, Side::Bid, 10));
        book.apply_event(&event(Action::Add, 2, 100, Side::Bid, 20));

        assert_eq!(
            book.apply_event(&event(Action::Modify, 1, 100, Side::Bid, 5)),
            None
        );

        let level = &book.bids[&100];
        assert_eq!(level.orders[0].id, 1);
        assert_eq!(level.orders[0].qty, 5);
        assert_eq!(level.orders[1].id, 2);
        assert_eq!(level.total_qty(), 25);
        assert_eq!(book.order_refs[&1], (Side::Bid, 100));
    }

    #[test]
    fn increasing_quantity_moves_order_to_back_of_queue() {
        let mut book = OrderBook::new();
        book.apply_event(&event(Action::Add, 1, 100, Side::Bid, 10));
        book.apply_event(&event(Action::Add, 2, 100, Side::Bid, 20));

        book.apply_event(&event(Action::Modify, 1, 100, Side::Bid, 15));

        let level = &book.bids[&100];
        assert_eq!(level.orders[0].id, 2);
        assert_eq!(level.orders[1].id, 1);
        assert_eq!(level.orders[1].qty, 15);
        assert_eq!(level.total_qty(), 35);
    }

    #[test]
    fn changing_price_moves_order_and_removes_empty_level() {
        let mut book = OrderBook::new();
        book.apply_event(&event(Action::Add, 1, 101, Side::Ask, 10));
        book.apply_event(&event(Action::Add, 2, 102, Side::Ask, 20));

        book.apply_event(&event(Action::Modify, 1, 102, Side::Ask, 10));

        assert!(!book.asks.contains_key(&101));
        let level = &book.asks[&102];
        assert_eq!(level.orders.len(), 2);
        assert_eq!(level.orders[0].id, 2);
        assert_eq!(level.orders[1].id, 1);
        assert_eq!(level.orders[1].price, 102);
        assert_eq!(book.order_refs[&1], (Side::Ask, 102));
    }

    #[test]
    fn modifying_unknown_order_adds_it() {
        let mut book = OrderBook::new();
        let msg = event(Action::Modify, 1, 101, Side::Ask, 10);

        assert_eq!(book.apply_event(&msg), None);

        assert_eq!(book.asks[&101].orders, vec![Order::from_event(&msg)]);
        assert_eq!(book.order_refs[&1], (Side::Ask, 101));
    }

    #[test]
    fn partial_cancel_reduces_quantity_without_removing_order() {
        let mut book = OrderBook::new();
        book.apply_event(&event(Action::Add, 1, 100, Side::Bid, 10));

        assert_eq!(
            book.apply_event(&event(Action::Cancel, 1, 100, Side::Bid, 4)),
            None
        );

        assert_eq!(book.bids[&100].orders.len(), 1);
        assert_eq!(book.bids[&100].orders[0].qty, 6);
        assert_eq!(book.order_refs[&1], (Side::Bid, 100));
    }

    #[test]
    fn full_cancel_returns_order_and_removes_empty_level() {
        let mut book = OrderBook::new();
        let msg = event(Action::Add, 1, 101, Side::Ask, 10);
        book.apply_event(&msg);

        let removed = book.apply_event(&event(Action::Cancel, 1, 101, Side::Ask, 10));

        assert_eq!(removed, Some(Order::from_event(&msg)));
        assert!(book.asks.is_empty());
        assert!(book.order_refs.is_empty());
    }

    #[test]
    fn full_cancel_at_shared_level_removes_only_target_order_and_reference() {
        let mut book = OrderBook::new();
        let first = event(Action::Add, 1, 100, Side::Bid, 10);
        let second = event(Action::Add, 2, 100, Side::Bid, 20);
        book.apply_event(&first);
        book.apply_event(&second);

        let removed = book.apply_event(&event(Action::Cancel, 1, 100, Side::Bid, 10));

        assert_eq!(removed, Some(Order::from_event(&first)));
        assert_eq!(book.bids[&100].orders, vec![Order::from_event(&second)]);
        assert!(
            !book.order_refs.contains_key(&1),
            "{book}, {:?}",
            book.order_refs
        );
        assert_eq!(book.order_refs[&2], (Side::Bid, 100));
    }

    #[test]
    #[should_panic(expected = "canceling bigger size than availible")]
    fn canceling_more_than_available_panics() {
        let mut book = OrderBook::new();
        book.apply_event(&event(Action::Add, 1, 100, Side::Bid, 10));
        book.apply_event(&event(Action::Cancel, 1, 100, Side::Bid, 11));
    }

    #[test]
    fn clear_removes_both_sides_and_all_order_references() {
        let mut book = OrderBook::new();
        book.apply_event(&event(Action::Add, 1, 100, Side::Bid, 10));
        book.apply_event(&event(Action::Add, 2, 101, Side::Ask, 20));

        assert_eq!(
            book.apply_event(&event(Action::Clear, 0, 0, Side::None, 0)),
            None
        );

        assert!(book.bids.is_empty());
        assert!(book.asks.is_empty());
        assert!(book.order_refs.is_empty());
    }

    #[test]
    fn trade_updates_last_trade_without_changing_resting_orders() {
        let mut book = OrderBook::new();
        let resting = event(Action::Add, 1, 100, Side::Bid, 10);
        book.apply_event(&resting);

        assert_eq!(
            book.apply_event(&event(Action::Trade, 0, 101, Side::Ask, 3)),
            None
        );

        assert_eq!(book.last_trade, (101, 3));
        assert_eq!(book.bids[&100].orders, vec![Order::from_event(&resting)]);
        assert!(book.asks.is_empty());
        assert_eq!(book.order_refs.len(), 1);

        book.apply_event(&event(Action::Trade, 0, 102, Side::None, 5));
        assert_eq!(book.last_trade, (101, 3));
    }
}
