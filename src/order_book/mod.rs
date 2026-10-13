pub mod order;
pub use crate::types::{OrderId, Price, Quantity, Side, Timestamp};
use dbn::{BidAskPair, MboMsg, Mbp10Msg};
pub use order::Order;

use crate::types::Action;
use std::assert_matches;
use std::backtrace::Backtrace;
use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap};
use std::fmt::{Display, Write};
use std::mem::replace;

const DISPLAY_WIDTH: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BookError {
    OrderIdNotFound(OrderId),
    OrderIndexNotFound(usize),
    DuplicateOrderId(OrderId),
    EnteringSideNone,
}

impl std::fmt::Display for BookError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OrderIdNotFound(id) => write!(f, "order {id:?} not found"),
            Self::OrderIndexNotFound(pos) => write!(f, "order at index {pos} not found"),
            Self::DuplicateOrderId(id) => write!(f, "duplicate order id {id:?}"),
            Self::EnteringSideNone => write!(f, "entering side is none"),
        }
    }
}

impl std::error::Error for BookError {}

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

    pub fn find(&self, id: OrderId) -> Result<&Order, BookError> {
        return self.orders.iter().find(|o| o.id == id).ok_or_else(|| BookError::OrderIdNotFound(id));
    }

    pub fn find_mut(&mut self, id: OrderId) -> Result<&mut Order, BookError> {
        return self.orders.iter_mut().find(|o| o.id == id).ok_or_else(|| BookError::OrderIdNotFound(id));
    }

    pub fn get_pos(&self, id: OrderId) -> Result<usize, BookError> {
        return self.orders.iter().position(|o| o.id == id).ok_or_else(|| BookError::OrderIdNotFound(id));
    }

    pub fn modify(&mut self, id: OrderId, new_qty: Quantity) -> Result<Order, BookError> {
        let old_order = self.find_mut(id)?;
        let new_order = Order{
            qty: new_qty,
            ..*old_order
        };
        let old_order  = replace(old_order, new_order);
        return Ok(old_order)
    }

    pub fn modify_idx(
        &mut self,
        order_pos: usize,
        new_qty: Quantity,
    ) -> Result<Order, BookError> {
        let old_order = self.orders.get_mut(order_pos).ok_or_else(|| BookError::OrderIndexNotFound(order_pos))?;
        let new_order = Order{
            qty: new_qty,
            ..*old_order
        };
        let old_order = replace(old_order, new_order);
        return Ok(old_order)
    }

    pub fn remove(&mut self, id: OrderId) -> Result<Order, BookError> {
        let pos = self.orders.iter().position(|o| o.id == id).ok_or_else(|| BookError::OrderIdNotFound(id))?;
        let order = self.orders.remove(pos);
        return Ok(order);
    }

    pub fn remove_idx(&mut self, pos: usize) -> Result<Order, BookError> {
        if pos >= self.orders.len() {
            return Err(BookError::OrderIndexNotFound(pos));
        }
        let order = self.orders.remove(pos);
        return Ok(order);
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

pub struct OrderBook {
    pub asks: BTreeMap<Price, PriceLevel>,
    pub bids: BTreeMap<Reverse<Price>, PriceLevel>,
    pub order_refs: HashMap<OrderId, (Side, Price)>,
    pub last_trade: (Price, Quantity),
    pub last_msg: MboMsg,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            asks: BTreeMap::new(),
            bids: BTreeMap::new(),
            order_refs: HashMap::new(),
            last_trade: (i64::MAX, u32::MAX),
            last_msg: MboMsg::default(),
        }
    }

    pub fn apply_event(&mut self, event: &MboMsg) -> Result<Option<Order>, BookError> {
        let side = event.side().unwrap();
        let action = event.action().unwrap();
        if event.price == dbn::UNDEF_PRICE && event.flags.is_tob() {
            match side {
                Side::Ask => {
                    self.asks.clear();
                    return Ok(None);
                }
                Side::Bid => {
                    self.bids.clear();
                    return Ok(None);
                }
                Side::None => return Err(BookError::EnteringSideNone)
            }

        }
        let removed = match action {
            Action::Add => {
                self.add(Order::from_event(event))?;
                None
            }
            Action::Modify => {
                self.modify(
                    event.order_id,
                    event.hd.ts_event,
                    side,
                    event.price,
                    event.size,
                )?;
                None
            }
            Action::Cancel => Some(self.cancel(event.order_id)?),
            Action::Clear => {
                self.clear();
                None
            }
            Action::Trade => {
                self.update_last(event.price, event.size);
                None
            }
            Action::Fill | Action::None => None,
        };
        self.last_msg = event.clone();
        Ok(removed)
    }

    fn update_last(&mut self, price: Price, qty: Quantity) {
        self.last_trade = (price, qty);
    }

    fn add(&mut self, order: Order) -> Result<(), BookError> {
        match order.side {
            Side::Ask => {
                self.asks
                    .entry(order.price)
                    .or_default()
                    .add(order);
            }
            Side::Bid => {
                self.bids
                    .entry(Reverse(order.price))
                    .or_default()
                    .add(order);
            }
            Side::None => return Err(BookError::EnteringSideNone),
        }
        if let Some(_) = self.order_refs.insert(order.id, (order.side, order.price)) {
            return Err(BookError::DuplicateOrderId(order.id))
        }

        Ok(())
    }

    fn modify(&mut self, id: OrderId, ts: Timestamp, side: Side, price: Price, qty: Quantity) -> Result<(), BookError> {
        let (old_side, old_price) = self.order_refs.get(&id).unwrap();
        assert_eq!(*old_side, side, "sides dont match");
        let level = match side {
            Side::Ask => self.asks.get_mut(old_price).unwrap(),
            Side::Bid => self.bids.get_mut(&Reverse(*old_price)).unwrap(),
            Side::None => return Err(BookError::EnteringSideNone),
        };
        let old_idx = level.get_pos(id)?;
        let old_order = level.orders.get(old_idx).unwrap();
        let lost_priority = old_order.loses_priority(price, qty);
        if lost_priority {
            let mut order = level.remove_idx(old_idx).unwrap();
            let level_empty = level.orders.is_empty();
            if level_empty {
                match side {
                    Side::Ask => {
                        self.asks.remove(old_price);
                    }
                    Side::Bid => {
                        self.bids.remove(&Reverse(*old_price));
                    }
                    Side::None => return Err(BookError::EnteringSideNone),
                }
            }
            order.ts = ts;
            order.price = price;
            order.qty = qty;
            let level = match side {
                Side::Ask => self.asks.entry(price).or_default(),
                Side::Bid => self.bids.entry(Reverse(price)).or_default(),
                Side::None => return Err(BookError::EnteringSideNone),
            };
            level.add(order);
        } else {
            level.modify_idx(old_idx, qty).unwrap();
        }
        self.order_refs.get_mut(&id).unwrap().1 = price;
        Ok(())
    }

    fn cancel(&mut self, id: OrderId) -> Result<Order, BookError> {
        let (old_side, old_price) = self.order_refs.get(&id).ok_or_else(|| BookError::OrderIdNotFound(id))?;
        let level = match old_side {
            Side::Ask => self.asks.get_mut(old_price),
            Side::Bid => self.bids.get_mut(&Reverse(*old_price)),
            Side::None => return Err(BookError::EnteringSideNone),
        }
        .unwrap();

        let order = level.remove(id)?;

        if level.orders.is_empty() {
            match old_side {
                Side::Ask => self.asks.remove(old_price),
                Side::Bid => self.bids.remove(&Reverse(*old_price)),
                Side::None => return Err(BookError::EnteringSideNone),
            };
        }
        self.order_refs
            .remove(&id)
            .ok_or_else(|| BookError::OrderIdNotFound(id))?;
        Ok(order)
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
                format!("ASKS | price: {} | qty: {}", price, level.total_qty())
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
        for (Reverse(price), level) in self.bids.iter().take(n) {
            writeln!(
                out,
                "{:<DISPLAY_WIDTH$}",
                format!("BIDS | price: {} | qty: {}", price, level.total_qty())
            )
            .unwrap();
        }
        writeln!(out, "{:-^DISPLAY_WIDTH$}", "").unwrap();

        return out;
    }

    pub fn mbp_10(&self) -> Mbp10Msg {
        let last = &self.last_msg;
        let mut levels: [BidAskPair; 10] = Default::default();
        let mut bids = self.bids.iter();
        let mut asks = self.asks.iter();

        for lvl in levels.iter_mut() {
            let bid_entry = bids.next().unwrap();
            let ask_entry = asks.next().unwrap();
            *lvl = BidAskPair {
                bid_px: bid_entry.0.0,
                ask_px: *ask_entry.0,
                bid_sz: bid_entry.1.total_qty(),
                ask_sz: ask_entry.1.total_qty(),
                bid_ct: bid_entry.1.orders.len() as u32,
                ask_ct: ask_entry.1.orders.len() as u32,
            }
        }

        Mbp10Msg {
            hd: last.hd,
            price: last.price,
            size: last.size,
            action: last.action,
            side: last.side,
            flags: last.flags,
            depth: u8::MAX,
            ts_recv: last.ts_recv,
            ts_in_delta: last.ts_in_delta,
            sequence: last.sequence,
            levels: levels,
        }
    }
}

impl Display for OrderBook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const LEVELS: usize = 10;
        writeln!(f, "{}", self.write_n_levels(LEVELS))
    }
}

#[cfg(test)]
mod price_level_tests {
    use super::*;

    #[test]
    fn empty_level_has_no_orders() {
        let mut level = PriceLevel::new();

        assert!(level.orders.is_empty());
        assert_eq!(level.find(1), Err(BookError::OrderIdNotFound(1)));
        assert_eq!(level.find_mut(1), Err(BookError::OrderIdNotFound(1)));
        assert_eq!(level.get_pos(1), Err(BookError::OrderIdNotFound(1)));
    }

    #[test]
    fn added_orders_are_findable_in_insertion_order() {
        let mut level = PriceLevel::new();
        let orders = [
            Order::new(1, 100, Side::Ask, 1_000, 10),
            Order::new(2, 100, Side::Ask, 2_000, 10),
            Order::new(3, 100, Side::Ask, 3_000, 10),
        ];

        for order in orders {
            level.add(order);
        }

        assert_eq!(level.orders.as_slice(), orders.as_slice());

        for (pos, order) in orders.iter().enumerate() {
            assert_eq!(level.find(order.id), Ok(order));
            assert_eq!(level.get_pos(order.id), Ok(pos));
        }

        assert_eq!(level.find(99), Err(BookError::OrderIdNotFound(99)));
        assert_eq!(level.find_mut(99), Err(BookError::OrderIdNotFound(99)));
        assert_eq!(level.get_pos(99), Err(BookError::OrderIdNotFound(99)));
    }

    #[test]
    fn mutable_lookup_updates_only_the_matching_order() {
        let mut level = PriceLevel::new();
        let first = Order::new(1, 100, Side::Ask, 1_000, 10);
        let second = Order::new(2, 100, Side::Ask, 1_000, 10);

        level.add(first);
        level.add(second);

        level.find_mut(2).expect("order 2 should exist").qty = 5;

        assert_eq!(level.find(1), Ok(&first));
        assert_eq!(
            level.find(2),
            Ok(&Order { qty: 5, ..second }),
        );
        assert_eq!(level.get_pos(2), Ok(1));
    }

    #[test]
    fn correct_total_qty() {
        let mut level = PriceLevel::new();
        let order1 = Order::new(1, 100, Side::Ask, 1_000, 200);
        let order2 = Order::new(2, 100, Side::Ask, 1_000, 50);

        level.add(order1);
        level.add(order2);

        assert_eq!(level.total_qty(), 250);
        assert_eq!(level.orders.len(), 2);

        let original_order = level.modify(1, 100).expect("order 1 should be modified");
        assert_eq!(original_order, order1);

        assert_eq!(level.total_qty(), 150);
        assert_eq!(level.orders.len(), 2);

        level.remove(2).expect("order 2 should be removed");

        assert_eq!(level.total_qty(), 100);
        assert_eq!(level.orders.len(), 1);
    }
}


#[cfg(test)]
mod book_tests {
    use dbn::{FlagSet, MboMsg, RecordHeader, rtype};

    use crate::{
        order_book::{Order, OrderBook},
        types::{Action, OrderId, Price, Quantity, Side},
    };

    fn add(order_id: OrderId, action: Action, side: Side, price: Price, size: Quantity) -> MboMsg {
        MboMsg {
            hd: RecordHeader::default::<MboMsg>(rtype::MBO),
            order_id,
            price,
            size,
            flags: FlagSet::default(),
            channel_id: 0,
            action: action as _,
            side: side as _,
            ts_recv: 0,
            ts_in_delta: 0,
            sequence: 0,
        }
    }

    fn modify(id: OrderId, side: Side, price: Price, qty: Quantity) -> MboMsg {
        MboMsg {
            hd: RecordHeader::default::<MboMsg>(rtype::MBO),
            order_id: id,
            side: side as _,
            price,
            size: qty,
            flags: FlagSet::default(),
            channel_id: 0,
            action: Action::Modify as _,
            ts_recv: 0,
            ts_in_delta: 0,
            sequence: 0,
        }
    }

    fn cancel(id: OrderId, side: Side) -> MboMsg {
        MboMsg {
            hd: RecordHeader::default::<MboMsg>(rtype::MBO),
            order_id: id,
            side: side as _,
            price: 0,
            size: 0,
            flags: FlagSet::default(),
            channel_id: 0,
            action: Action::Cancel as _,
            ts_recv: 0,
            ts_in_delta: 0,
            sequence: 0,
        }
    }

    #[test]
    fn level_is_droped_if_no_order_left() {
        let mut book = OrderBook::new();
        const ORDER_ID: OrderId = 0;
        const PRICE: Price = 1000;
        const QTY: Quantity = 10;
        let add_event = add(ORDER_ID, Action::Add, Side::Ask, PRICE, QTY);
        let cancel_event = cancel(ORDER_ID, Side::Ask);
        book.apply_event(&add_event).unwrap();

        let price_level = book.asks.get(&PRICE).unwrap();
        assert_eq!(book.asks.len(), 1);
        assert_eq!(price_level.orders.len(), 1);
        assert_eq!(
            price_level.orders.get(0).copied(),
            Some(Order::from_event(&add_event))
        );

        book.apply_event(&cancel_event).unwrap();

        assert_eq!(book.asks.len(), 0);
    }

    // #[test]
    // fn book_add() {
    //     let mut book = OrderBook::new();
    //     book.apply_event(&add(0, Action::Add, Side::Ask, 1000, 10)).unwrap();
    //     book.apply_event(&add(0, Action::Add, Side::Ask, 1000, 10)).unwrap();
    //     assert_eq!(book.asks.len(), 1);
    //     assert_eq!(book.bids.len(), 0);
    //     assert_eq!(book.order_refs.len(), 1);
    // }
}
