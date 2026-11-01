use dbn::{Action, Mbp10Msg, RecordHeader, Side};

use crate::{order_book::OrderBook, types::{MboMsg, Price, Quantity}};

pub struct Mbp10Converter {
    applied_mbo: Vec<MboMsg>,
    book: OrderBook,
    last: Mbp10Msg,
}

impl Mbp10Converter {
    pub fn new() -> Self {
        Self {
            book: OrderBook::new(),
            applied_mbo: Vec::new(),
            last: Mbp10Msg::default(),
        }
    }

    fn calc_depth(&self, msg_price: Price, msg_action: Action) -> Option<u8> {
        match msg_action {
            Action::Add|Action::Cancel|Action::Trade|Action::Modify => {
                let mut asks = self.book.asks.keys();
                let mut bids = self.book.bids.keys().rev();

                for depth in 0..10 {
                    let ask = asks.next();
                    let bid = bids.next();

                    if ask.is_none() && bid.is_none() {
                        return None;
                    }
                    if ask == Some(&msg_price) || bid == Some(&msg_price) {
                        return Some(depth);
                    }
                }
                None
            },
            Action::Clear => None,
            Action::None|Action::Fill => unreachable!(),
        }
    }

    pub fn apply_msg(&mut self, msg: &MboMsg) {
        // mbo: 403, mbp: 338
        let action = msg.action().unwrap();

        if action != Action::None {
            self.last = Mbp10Msg{
                action: msg.action,
                side: msg.side,
                price: msg.price,
                size: msg.size,
                // depth: self.calc_depth(msg.price, action).unwrap_or_else(|| u8::MAX),
                ..self.last
            };
        }
        if matches!(action, Action::Cancel|Action::Modify|Action::Trade) {
            self.last.depth = self.calc_depth(msg.price, action).unwrap_or_else(|| u8::MAX);
        }

        self.book.apply_event(msg);
        self.applied_mbo.push(*msg);

        if action == Action::Add {
            self.last.depth = self.calc_depth(msg.price, action).unwrap_or_else(|| u8::MAX);
        }
    }

    pub fn current_mbp10(&self) -> Mbp10Msg {
        let Some(last_mbo) = self.applied_mbo.last() else {
            return Mbp10Msg::default()
        };
        let hd = RecordHeader::new::<Mbp10Msg>(last_mbo.hd.rtype, last_mbo.hd.publisher_id, last_mbo.hd.instrument_id, last_mbo.hd.ts_event);
        let levels = self.book.get_n_levels(10).try_into().unwrap();
        Mbp10Msg {
            hd,
            flags: last_mbo.flags,
            ts_recv: last_mbo.ts_recv,
            ts_in_delta: last_mbo.ts_in_delta,
            sequence: last_mbo.sequence,
            levels,
            ..self.last
        }
    }
}
