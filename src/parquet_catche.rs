use crate::types::{MBOMsg, Side, Action};

use arrow::array::{Int64Array, LargeStringArray, UInt8Array, UInt32Array, UInt64Array};
use parquet::arrow::ProjectionMask;
use parquet::arrow::arrow_reader::{ParquetRecordBatchReader, ParquetRecordBatchReaderBuilder};
use parquet::file::metadata::ParquetMetaData;
use std::fs::File;
use std::sync::Arc;

const DEFAULT_BATCH_SIZE: usize = 10_000_000;

const MASK: [&str; 9] = [
    "ts_event",
    "instrument_id",
    "action",
    "side",
    "price",
    "size",
    "order_id",
    "flags",
    "sequence",
];

pub struct DataCatcher {
    pub data: Vec<MBOMsg>,
    metadata: Arc<ParquetMetaData>,
    idx: usize,
    reader: ParquetRecordBatchReader,
}

impl DataCatcher {
    pub fn new(path: &str) -> Self {
        Self::with_batch_size(path, DEFAULT_BATCH_SIZE)
    }

    pub fn with_batch_size(path: &str, batch_size: usize) -> Self {
        let builder = Self::get_builder(path);

        let mask = ProjectionMask::columns(
            builder.parquet_schema(),
            MASK
        );

        let metadata = builder.metadata().clone();

        let reader = builder
            .with_projection(mask)
            .with_batch_size(batch_size)
            .build()
            .unwrap();

        Self { reader, data: Vec::with_capacity(batch_size), idx: 0, metadata}
    }

    fn get_builder(path: &str) -> ParquetRecordBatchReaderBuilder<File> {
        let file = File::open(path).unwrap();
        let builder = ParquetRecordBatchReaderBuilder::try_new(file).unwrap();
        return builder;
    }

    pub fn new_batch(&mut self) -> bool {
        self.idx = 0;
        let last = self.data.last().cloned();
        self.data.clear();
        if let Some(some_last) = last {
            self.data.push(some_last);
        }

        let batch = match self.reader.next() {
            Some(Ok(batch)) => batch,
            Some(Err(e)) => panic!("unexpected error: {}", e),
            None => return false,
        };

        let schema = batch.schema();
        let column = |name: &str| {
            let index = schema
                .index_of(name)
                .unwrap_or_else(|_| panic!("projected batch is missing column `{name}`"));
            batch.column(index)
        };
        let downcast = |name: &str| column(name).as_any();

        let ts_events = downcast("ts_event").downcast_ref::<UInt64Array>().unwrap();
        let instrument_id = downcast("instrument_id").downcast_ref::<UInt32Array>().unwrap();
        let action = downcast("action").downcast_ref::<LargeStringArray>().unwrap();
        let side = downcast("side").downcast_ref::<LargeStringArray>().unwrap();
        let price = downcast("price").downcast_ref::<Int64Array>().unwrap();
        let quantity = downcast("size").downcast_ref::<UInt32Array>().unwrap();
        let order_id = downcast("order_id").downcast_ref::<UInt64Array>().unwrap();
        let flags = downcast("flags").downcast_ref::<UInt8Array>().unwrap();
        let sequence = downcast("sequence").downcast_ref::<UInt32Array>().unwrap();

        let ts_events = ts_events.values();
        let instrument_id = instrument_id.values();
        let price = price.values();
        let quantity = quantity.values();
        let order_id = order_id.values();
        let flags = flags.values();
        let sequence = sequence.values();

        let num_rows = batch.num_rows();
        self.data.reserve(num_rows);

        for i in 0..num_rows {
            let msg = MBOMsg {
                ts_event: ts_events[i],
                instrument_id: instrument_id[i],
                action: Action::from_u8(action.value(i).as_bytes()[0]),
                side: Side::from_u8(side.value(i).as_bytes()[0]),
                price: price[i],
                quantity: quantity[i],
                order_id: order_id[i],
                flags: flags[i],
                sequence: sequence[i],
            };
            self.data.push(msg);
        }
        return true
    }
}

impl Iterator for DataCatcher {
    type Item = (MBOMsg, MBOMsg);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.idx + 1 < self.data.len() {
                let item = (self.data[self.idx], self.data[self.idx + 1]);
                self.idx += 1;
                return Some(item);
            }
            if !self.new_batch() {
                return None;
            }
        }
    }
}
