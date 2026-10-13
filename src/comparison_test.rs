#![cfg(test)]

use crate::order_book::OrderBook;
use dbn::{
    MboMsg, Mbp10Msg,
    decode::{DecodeRecord, dbn::Decoder},
};

#[test]
#[ignore = "slow test; run manually"]
fn main() {
    let mut order_book = OrderBook::new();
    let mbo_decoder = Decoder::from_zstd_file("test_data/glbx-mdp3-20260701.mbo.dbn.zst").unwrap();
    let mbp_decoder =
        Decoder::from_zstd_file("test_data/glbx-mdp3-20260701.mbp-10.dbn.zst").unwrap();

    let mut mbo_data = mbo_decoder
        .decode_records::<MboMsg>()
        .unwrap()
        .into_iter()
        .peekable();
    let mbp_data = mbp_decoder
        .decode_records::<Mbp10Msg>()
        .unwrap()
        .into_iter()
        .peekable();

    while mbo_data
        .peek()
        .is_some_and(|entry| entry.flags.is_snapshot())
    {
        let entry = mbo_data.next().unwrap();
        order_book.apply_event(&entry).unwrap();
    }

    for i in mbp_data {
        if i.flags.is_snapshot() {
            assert_eq!(order_book.mbp_10().levels, i.levels);
            continue;
        }

        loop {
            let mbo = mbo_data.next().unwrap_or_else(|| {
                panic!(
                    "MBO stream exhausted: MBP {i:?}, last MBO {:?}",
                    order_book.last_msg
                )
            });

            assert!(
                mbo.ts_recv <= i.ts_recv,
                "Passed unmatched MBP event: MBP {i:?}, MBO {mbo:?}"
            );
            order_book.apply_event(&mbo).unwrap();

            if mbo.hd.instrument_id == i.hd.instrument_id
                && mbo.hd.ts_event == i.hd.ts_event
                && mbo.sequence == i.sequence
                && mbo.ts_recv == i.ts_recv
                && mbo.flags.is_last() == i.flags.is_last()
            {
                break;
            }
        }

        let mbp = order_book.mbp_10();

        // A terminal MBO no-op marks the completed update but does not carry
        // the price, size, action, or side reported by MBP.
        if order_book.last_msg.action().unwrap() != dbn::Action::None {
            assert_eq!(mbp.price, i.price);
            assert_eq!(mbp.size, i.size);
            assert_eq!(mbp.action, i.action);
            assert_eq!(mbp.side, i.side);
        }
        assert_eq!(mbp.flags, i.flags);
        assert_eq!(mbp.ts_recv, i.ts_recv);
        assert_eq!(mbp.ts_in_delta, i.ts_in_delta);
        assert_eq!(mbp.sequence, i.sequence);
        assert_eq!(mbp.levels, i.levels);
    }
}
