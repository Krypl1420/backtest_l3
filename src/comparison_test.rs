#![cfg(test)]

use std::{iter::Peekable, vec::IntoIter};
use crate::{mbp10_converter::Mbp10Converter};
use dbn::{
    MboMsg, Mbp10Msg, decode::{DecodeRecord, dbn::Decoder},
};

fn red_text(s: String) -> String {
    format!("\x1b[31m{}\x1b[0m", s)
}

fn debug_info(mbp: &Mbp10Msg, mbo_conv: &Mbp10Msg) -> String {
    macro_rules! field {
        ($name:literal, $m:ident => $e:expr) => {
            (
                $name,
                format!("{:?}", { let $m = mbp; $e }),
                format!("{:?}", { let $m = mbo_conv; $e }),
            )
        };
    }

    let fields = [
        field!("ts_event",      m => m.hd.ts_event),
        field!("ts_recv",       m => m.ts_recv),
        field!("publisher_id",  m => m.hd.publisher_id),
        field!("instrument_id", m => m.hd.instrument_id),
        field!("action",        m => m.action()),
        field!("side",          m => m.side()),
        field!("depth",         m => m.depth),
        field!("price",         m => m.price),
        field!("size",          m => m.size),
        field!("flags",         m => m.flags.raw()),
        field!("sequence",      m => m.sequence),
        field!("levels",        m => m.levels),
    ];


    let mut info_string = String::new();
    info_string.push_str(&format!("ts_event -> mbp: {:?} mbo: {:?}\n", mbp.hd.ts_event(), mbo_conv.hd.ts_event()));
    for (name, a, b) in &fields {
        if a == b {
            info_string.push_str(&format!("{name}: {a} == {b}\n"));
        } else {
            info_string.push_str(&red_text(format!("{name}: {a} != {b}   <-- MISMATCH\n")));
        }
    }
    info_string
}

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
    let mut converter = Mbp10Converter::new();
    let (mut mbo_data, mbp_data) = read_data();

    while mbo_data.peek().is_some_and(|entry| entry.flags.is_snapshot()) {
        let entry = mbo_data.next().unwrap();
        converter.apply_msg(&entry);
    }

    let mut mbo_conv = converter.current_mbp10();
    for mbp in mbp_data {
        if mbp.flags.is_snapshot() {
            continue;
        }

        while !(mbp.hd.ts_event == mbo_conv.hd.ts_event
        && mbp.ts_recv == mbo_conv.ts_recv
        && mbp.sequence == mbo_conv.sequence
        && mbp.flags.is_last() == mbo_conv.flags.is_last()) {
            assert!(mbo_conv.hd.ts_event <= mbp.hd.ts_event, "mbo_conv: {:?}, mbp: {:?}", mbo_conv.hd.ts_event, mbp.hd.ts_event);
            let entry = mbo_data.next().unwrap();
            converter.apply_msg(&entry);
            mbo_conv = converter.current_mbp10();
        }

        // assert_eq!(mbp.hd.ts_event, mbo_conv.hd.ts_event, "\n{}", debug_info(&mbp, &mbo_conv));
        // assert_eq!(mbp.ts_recv, mbo_conv.ts_recv, "\n{}", debug_info(&mbp, &mbo_conv));
        // assert_eq!(mbp.sequence, mbo_conv.sequence, "\n{}", debug_info(&mbp, &mbo_conv));
        // assert_eq!(mbp.flags, mbo_conv.flags, "\n{}", debug_info(&mbp, &mbo_conv));
        // assert_eq!(mbp.hd.instrument_id, mbo_conv.hd.instrument_id, "\n{}", debug_info(&mbp, &mbo_conv));
        // if mbp.side().unwrap() != Side::None {
        //     assert_eq!(mbp.side, mbo_conv.side, "\n{}", debug_info(&mbp, &mbo_conv));
        // }
        // assert_eq!(mbp.action, mbo_conv.action, "\n{}", debug_info(&mbp, &mbo_conv));
        // // assert_eq!(mbp.depth, mbo_conv.depth, "\n{}", debug_info(&mbp, &mbo_conv));
        // assert_eq!(mbp.price, mbo_conv.price, "\n{}", debug_info(&mbp, &mbo_conv));
        assert_eq!(mbp.levels, mbo_conv.levels, "\n{}", debug_info(&mbp, &mbo_conv));
    }
}
