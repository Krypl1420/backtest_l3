use dbn::{
    Mbp10Msg,
    decode::{DecodeRecord, dbn::Decoder},
};

fn main() {
    let data = Decoder::from_zstd_file("test_data/glbx-mdp3-20260701.mbp-10.dbn.zst")
        .expect("file not found");

    let mut index = 0;
    let data = data.decode_records::<Mbp10Msg>().unwrap();
    for (i, msg) in data.iter().enumerate() {
        if !msg.flags.is_snapshot() {
            index = i;
            break;
        }
    }
    println!("{index}");
}
