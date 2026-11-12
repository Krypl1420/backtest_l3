pub mod data;
pub mod order_book;

pub use order_book::OrderBook;

use std::fmt::Debug;
use time::OffsetDateTime;

#[cfg(test)]
mod comparison_test;

use dbn::{MboMsg, Result as DbnResult};
use std::iter::Peekable;

pub struct Backtester<I: Iterator<Item = DbnResult<MboMsg>>> {
    book: OrderBook,
    mbo_data: Peekable<I>,
}

impl<I: Iterator<Item = DbnResult<MboMsg>>> Backtester<I> {
    pub fn new(mbo_data: I) -> Self {
        Self {
            book: OrderBook::new(),
            mbo_data: mbo_data.peekable(),
        }
    }

    pub fn apply_events(&mut self) -> DbnResult<()> {
        while let Some(entry) = self.mbo_data.next().transpose()? {
            self.book.apply_event(&entry);
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
        if self.book.last_msg != MboMsg::default()
            && *stop_time < self.book.last_msg.hd.ts_event().unwrap()
        {
            assert_eq!(
                *stop_time,
                self.book.last_msg.hd.ts_event().unwrap(),
                "waiting for ts from the past, it's backtest not time machine bro"
            );
        }
        loop {
            let Some(entry) = self.mbo_data.peek() else {
                assert_eq!(
                    *stop_time,
                    self.book.last_msg.hd.ts_event().unwrap(),
                    "mbo data exhausted"
                );
                break;
            };
            if *stop_time < entry.as_ref().unwrap().hd.ts_event().unwrap() {
                break;
            }
            let entry = self.mbo_data.next().expect("mbo data was exhausted")?;
            self.book.apply_event(&entry);
        }
        Ok(())
    }

    pub fn wait_for_ts(&mut self, stop_time: u64) -> DbnResult<()> {
        if self.book.last_msg != MboMsg::default() && stop_time < self.book.last_msg.hd.ts_event {
            assert_eq!(
                stop_time, self.book.last_msg.hd.ts_event,
                "waiting for ts from the past, it's backtest not time machine bro"
            );
        }
        loop {
            let Some(entry) = self.mbo_data.peek() else {
                assert_eq!(
                    stop_time, self.book.last_msg.hd.ts_event,
                    "mbo data exhausted"
                );
                break;
            };
            if stop_time < entry.as_ref().unwrap().hd.ts_event {
                break;
            }
            let entry = self.mbo_data.next().expect("mbo data was exhausted")?;
            self.book.apply_event(&entry);
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
            self.book.apply_event(&entry);
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
        let mut backtester = Backtester::new(mbo_data);
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
            let levels: [BidAskPair; 10] = backtester.book.get_n_levels(10).try_into().unwrap();
            assert_eq!(
                mbp_entry.levels, levels,
                "\nmbp_entry: {:?},\n mbo: {:?}\n",
                mbp_entry.hd, backtester.book.last_msg.hd
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
        let mut backtester = Backtester::new(mbo_data);
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
            let levels: [BidAskPair; 10] = backtester.book.get_n_levels(10).try_into().unwrap();
            assert_eq!(
                mbp_entry.levels, levels,
                "\nmbp_entry: {:?},\n mbo: {:?}\n",
                mbp_entry.hd, backtester.book.last_msg.hd
            );
        }
    }
}
