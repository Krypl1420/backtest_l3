const A_UINT8: u8 = 'A' as u8;
const B_UINT8: u8 = 'B' as u8;
const N_UINT8: u8 = 'N' as u8;
const M_UINT8: u8 = 'M' as u8;
const C_UINT8: u8 = 'C' as u8;
const R_UINT8: u8 = 'R' as u8;
const T_UINT8: u8 = 'T' as u8;
const F_UINT8: u8 = 'F' as u8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Side {
    Ask,
    Bid,
    None
}

impl Side {
    pub fn from_char(c: char) -> Self {
        match c {
            'A' => Self::Ask,
            'B' => Self::Bid,
            'N' => Self::None,
            _ => unreachable!(),
        }
    }

    pub fn from_u8(c: u8) -> Self {
        match c {
            A_UINT8 => Self::Ask,
            B_UINT8 => Self::Bid,
            N_UINT8 => Self::None,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Add,
    Modify,
    Cancel,
    Clear,
    Trade,
    Fill,
    None,
}

impl Action {
    pub fn from_char(c: char) -> Self {
        match c {
            'A' => Self::Add,
            'M' => Self::Modify,
            'C' => Self::Cancel,
            'R' => Self::Clear,
            'T' => Self::Trade,
            'F' => Self::Fill,
            'N' => Self::None,
            _ => unreachable!(),
        }
    }

    pub fn from_u8(i: u8) -> Self {
        match i {
            A_UINT8 => Self::Add,
            M_UINT8 => Self::Modify,
            C_UINT8 => Self::Cancel,
            R_UINT8 => Self::Clear,
            T_UINT8 => Self::Trade,
            F_UINT8 => Self::Fill,
            N_UINT8 => Self::None,
            _ => unreachable!(),
        }
    }
}

pub type OrderRef = (u64, Side);

#[derive(Clone, Copy, Debug)]
pub struct MBOMsg {
    pub ts_event: Timestamp,
    pub instrument_id: InstrumentId,
    pub action: Action,
    pub side: Side,
    pub price: Price,
    pub quantity: Quantity,
    pub order_id: OrderId,
    pub flags: u8,
    pub sequence: u32,
}

pub struct Flags {
    last: bool,
    tob: bool,
    snapshot: bool,
    mbp: bool,
    bad_ts_recv: bool,
    maybe_bad_book: bool,
    publisher_specific: bool
}

impl Flags {
    pub fn new(last: bool, tob: bool, snapshot: bool, mbp: bool, bad_ts_recv: bool, maybe_bad_book: bool, publisher_specific: bool) -> Self {
        Self {
            last,
            tob,
            snapshot,
            mbp,
            bad_ts_recv,
            maybe_bad_book,
            publisher_specific,
        }
    }

    pub fn from_u8(flags: u8) -> Self {
        Self {
            last: flags & 128 != 0,
            tob: flags & 64 != 0,
            snapshot: flags & 32 != 0,
            mbp: flags & 16 != 0,
            bad_ts_recv: flags & 8 != 0,
            maybe_bad_book: flags & 4 != 0,
            publisher_specific: flags & 2 != 0,
        }
    }
}

pub type Price = i64;
pub type Quantity = u32;
pub type OrderId = u64;
pub type Timestamp = u64;
pub type InstrumentId = u32;
