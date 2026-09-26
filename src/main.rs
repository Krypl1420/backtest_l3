mod order_book_test;
mod price_level_test;
mod order_book;
mod parquet_catche;
mod types;

use order_book::{OrderBook};
use parquet_catche::{DataCatcher};

fn main() {
    let catcher = DataCatcher::new("parquets/mbo.parquet");
    let mut book = OrderBook::new();
    let mut i = 0;
    for msg in catcher {
        book.apply_event(&msg.0);
        if i % 10_000_000 == 0 {
            println!("{}", book);
        }
        i += 1;
    }
    println!("{}", book);
}
