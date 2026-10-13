use dbn::{
    MboMsg,
    decode::{DecodeRecord, dbn::Decoder},
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    dbg!(&args);
    let mut data = Decoder::from_zstd_file(&args[1]).expect("file not found");

    let mut tob_flags = 0;
    while let Some(msg) = data.decode_record::<MboMsg>().unwrap() {
        if msg.flags.is_tob() {
            tob_flags += 1;
            if tob_flags % 1000 == 0 {
                println!("{}", tob_flags);
            }
        }
    }
}
