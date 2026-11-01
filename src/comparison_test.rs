#![cfg(test)]

use std::{iter::Peekable, vec::IntoIter};
use dbn::{
    BidAskPair, MboMsg, Mbp10Msg, decode::{DecodeRecord, dbn::Decoder},
};

use crate::OrderBook;

fn read_data() -> (Peekable<IntoIter<MboMsg>>, Peekable<IntoIter<Mbp10Msg>>) {
    let mbo_decoder = Decoder::from_zstd_file("test_data/glbx-mdp3-20260701.mbo.dbn.zst").expect("failed to open mbo.dbn");
    let mbp_decoder = Decoder::from_zstd_file("test_data/glbx-mdp3-20260701.mbp-10.dbn.zst").expect("failed to open mbp-10.dbn");

    let mbo_data = mbo_decoder
        .decode_records::<MboMsg>()
        .expect("invalid mbo file")
        .into_iter()
        .peekable();
    let mbp_data = mbp_decoder
        .decode_records::<Mbp10Msg>()
        .expect("invalid mbp file")
        .into_iter()
        .peekable();

    (mbo_data, mbp_data)
}


#[test]
#[ignore = "slow test; run manually"]
fn main() {
    let mut book  = OrderBook::new();
    let (mut mbo_data, mbp_data) = read_data();
    let mut mbo_entry = mbo_data.next().unwrap();

    while mbo_data.peek().is_some_and(|entry| entry.flags.is_snapshot()) {
        mbo_entry = mbo_data.next().unwrap();
        book.apply_event(&mbo_entry);
    }

    for mbp in mbp_data {
        if mbp.flags.is_snapshot() {
            continue;
        }

        while !(mbp.hd.ts_event == mbo_entry.hd.ts_event
        && mbp.ts_recv == mbo_entry.ts_recv
        && mbp.sequence == mbo_entry.sequence
        && mbp.flags.is_last() == mbo_entry.flags.is_last()) {
            assert!(mbo_entry.hd.ts_event <= mbp.hd.ts_event, "mbo_entry: {:?}, mbp: {:?}", mbo_entry.hd.ts_event, mbp.hd.ts_event);
            let entry = mbo_data.next().unwrap();
            book.apply_event(&entry);
            mbo_entry = book.last_msg.clone();
        }

        let levels: [BidAskPair; 10] = book.get_n_levels(10).try_into().unwrap();
        assert_eq!(mbp.levels, levels);
    }
}
