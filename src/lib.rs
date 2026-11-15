pub mod data;
pub mod order_book;
pub mod equity_manager;

pub use order_book::OrderBook;
pub use equity_manager::EquityManager;

use std::fmt::Debug;
use time::OffsetDateTime;

use dbn::{MboMsg, Result as DbnResult};
use std::iter::Peekable;

pub struct Backtester<I: Iterator<Item = DbnResult<MboMsg>>> {
    replay_book: OrderBook,
    exec_book: OrderBook,
    mbo_data: Peekable<I>,
    broker: EquityManager,
}

impl<I: Iterator<Item = DbnResult<MboMsg>>> Backtester<I> {
    pub fn new(starting_balance: u32, mbo_data: I) -> Self {
        Self {
            replay_book: OrderBook::new(),
            exec_book: OrderBook::new(),
            mbo_data: mbo_data.peekable(),
            broker: EquityManager::new(starting_balance),
        }
    }

    pub fn apply_last(&mut self) -> DbnResult<()> {
        while let Some(entry) = self.mbo_data.next().transpose()? {
            self.replay_book.apply_event(&entry);
            if entry.flags.is_last() {
                return Ok(());
            }
        }
        panic!("mbo data exhausted")
    }

    pub fn wait_for_dt<T: PartialOrd<OffsetDateTime> + Debug>(
        &mut self,
        stop_time: &T,
    ) -> DbnResult<()> {
        if self.replay_book.last_msg() != MboMsg::default()
            && *stop_time < self.replay_book.last_msg().hd.ts_event().unwrap()
        {
            assert_eq!(
                *stop_time,
                self.replay_book.last_msg().hd.ts_event().unwrap(),
                "waiting for ts from the past, it's backtest not time machine bro"
            );
        }
        loop {
            let Some(entry) = self.mbo_data.peek() else {
                assert_eq!(
                    *stop_time,
                    self.replay_book.last_msg().hd.ts_event().unwrap(),
                    "mbo data exhausted"
                );
                break;
            };
            if *stop_time < entry.as_ref().unwrap().hd.ts_event().unwrap() {
                break;
            }
            let entry = self.mbo_data.next().expect("mbo data was exhausted")?;
            self.replay_book.apply_event(&entry);
        }
        Ok(())
    }

    pub fn wait_for_ts(&mut self, stop_time: u64) -> DbnResult<()> {
        if self.replay_book.last_msg() != MboMsg::default()
            && stop_time < self.replay_book.last_msg().hd.ts_event
        {
            assert_eq!(
                stop_time,
                self.replay_book.last_msg().hd.ts_event,
                "waiting for ts from the past, it's backtest not time machine bro"
            );
        }
        loop {
            let Some(entry) = self.mbo_data.peek() else {
                assert_eq!(
                    stop_time,
                    self.replay_book.last_msg().hd.ts_event,
                    "mbo data exhausted"
                );
                break;
            };
            if stop_time < entry.as_ref().unwrap().hd.ts_event {
                break;
            }
            let entry = self.mbo_data.next().expect("mbo data was exhausted")?;
            self.replay_book.apply_event(&entry);
        }
        Ok(())
    }

    pub fn consume_snapshots(&mut self) -> DbnResult<()> {
        while let Some(entry) = self.mbo_data.peek().as_ref() {
            if !entry
                .as_ref()
                .expect("mbo data was exhausted")
                .flags
                .is_snapshot()
            {
                break;
            }
            let entry = self.mbo_data.next().unwrap()?;
            self.replay_book.apply_event(&entry);
        }
        Ok(())
    }
}

#[cfg(test)]
mod backtester_comparison_tests {
    use super::Backtester;
    use dbn::{
        BidAskPair, MboMsg, Mbp10Msg,
        decode::{DecodeRecord, DecodeRecordRef, dbn::Decoder},
    };
    use std::{iter::Peekable, vec::IntoIter};

    fn mbp_data() -> Peekable<IntoIter<Mbp10Msg>> {
        let mbp_decoder = Decoder::from_zstd_file("test_data/glbx-mdp3-20260701.mbp-10.dbn.zst")
            .expect("failed to open mbp-10.dbn");

        let mbp_data = mbp_decoder
            .decode_records::<Mbp10Msg>()
            .expect("invalid mbp file")
            .into_iter()
            .peekable();

        mbp_data
    }

    #[test]
    #[ignore = "slow test"]
    fn wait_for_ts_book_comparison() {
        let mut data = Decoder::from_zstd_file("test_data/glbx-mdp3-20260701.mbo.dbn.zst")
            .expect("file not found");
        let mbo_data = data
            .decode_buf_iter()
            .map(|record| record.and_then(|record| record.try_get::<MboMsg>().copied()));
        let mut backtester = Backtester::new(10_000, mbo_data);
        backtester.consume_snapshots().unwrap();
        let mut mbp_data = mbp_data();
        while let Some(mbp_entry) = mbp_data.next() {
            if mbp_entry.flags.is_snapshot()
                || !mbp_entry.flags.is_last()
                || (mbp_data.peek().is_some()
                    && mbp_data.peek().unwrap().hd.ts_event <= mbp_entry.hd.ts_event)
            {
                continue;
            }
            backtester.wait_for_ts(mbp_entry.hd.ts_event).unwrap();
            let levels: [BidAskPair; 10] =
                backtester.replay_book.get_n_levels(10).try_into().unwrap();
            assert_eq!(
                mbp_entry.levels,
                levels,
                "\nmbp_entry: {:?},\n mbo: {:?}\n",
                mbp_entry.hd,
                backtester.replay_book.last_msg().hd
            );
        }
    }

    #[test]
    #[ignore = "slow test"]
    fn wait_for_dt_book_comparison() {
        let mut data = Decoder::from_zstd_file("test_data/glbx-mdp3-20260701.mbo.dbn.zst")
            .expect("file not found");
        let mbo_data = data
            .decode_buf_iter()
            .map(|record| record.and_then(|record| record.try_get::<MboMsg>().copied()));
        let mut backtester = Backtester::new(10_000, mbo_data);
        backtester.consume_snapshots().unwrap();
        let mut mbp_data = mbp_data();
        while let Some(mbp_entry) = mbp_data.next() {
            if mbp_entry.flags.is_snapshot()
                || !mbp_entry.flags.is_last()
                || (mbp_data.peek().is_some()
                    && mbp_data.peek().unwrap().hd.ts_event <= mbp_entry.hd.ts_event)
            {
                continue;
            }
            backtester
                .wait_for_dt(&mbp_entry.hd.ts_event().unwrap())
                .unwrap();
            let levels: [BidAskPair; 10] =
                backtester.replay_book.get_n_levels(10).try_into().unwrap();
            assert_eq!(
                mbp_entry.levels,
                levels,
                "\nmbp_entry: {:?},\n mbo: {:?}\n",
                mbp_entry.hd,
                backtester.replay_book.last_msg().hd
            );
        }
    }
}
