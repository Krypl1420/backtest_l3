use dbn::{
    MboMsg,
    decode::{DecodeRecord, dbn::Decoder},
};

fn main() {
    let mut data = Decoder::from_zstd_file("test_data/glbx-mdp3-20260701.mbo.dbn.zst").expect("file not found");

    let mut tob_flags = 0;
    while let Some(msg) = data.decode_record::<MboMsg>().unwrap() {
        if msg.flags.is_tob() {
            tob_flags += 1;
            if tob_flags % 1000 == 0 {
                println!("{}", tob_flags);
            }
        }
    }
    println!("{tob_flags}");
}
