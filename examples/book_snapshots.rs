use backtest_l3::OrderBook;
use dbn::{
    MboMsg,
    decode::{DecodeRecord, dbn::Decoder},
};

fn main() {
    let mut book = OrderBook::new();
    let args: Vec<String> = std::env::args().collect();
    dbg!(&args);
    let mut data = Decoder::from_zstd_file(&args[1]).expect("file not found");

    let mut i = 0;
    while let Some(msg) = data.decode_record::<MboMsg>().unwrap() {
        book.apply_event(&msg).unwrap();

        if i % 10_000_000 == 0 {
            println!("{}", book);
        }

        if i == 100_000_000 {
            break;
        }

        i += 1;
    }
}
