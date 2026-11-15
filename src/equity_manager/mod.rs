use crate::order_book::Price;
use dbn::{FIXED_PRICE_SCALE, MboMsg};

const LOGS_BUF_LIMIT: u32 = 200_000;

pub struct EquityManager {
    balance: Price,
    exposure: i32,
    logs_buf: Vec<MboMsg>,
}

impl EquityManager {
    pub fn new(starting_balance: u32) -> Self {
        Self {
            balance: starting_balance as Price * FIXED_PRICE_SCALE,
            exposure: 0,
            logs_buf: Vec::with_capacity(LOGS_BUF_LIMIT as _),
        }
    }
}
