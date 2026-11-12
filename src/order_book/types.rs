pub use dbn::{
    Action, FIXED_PRICE_SCALE, FlagSet, MboMsg, Mbp10Msg, Side,
    decode::{DecodeRecordRef, dbn::Decoder},
};

pub type Price = i64;
pub type Quantity = u32;
pub type OrderId = u64;
pub type Timestamp = u64;
pub type InstrumentId = u32;
pub type Sequence = u32;
pub type OrderRef = (u64, Side);
