//! Bounded save-phase timings for perf contracts and diagnostics.

use bevy::prelude::*;

/// Latest save transaction phase timings (nanoseconds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SaveTransactionSample {
    pub body_bytes: usize,
    pub serialize_ns: u64,
    pub write_file_sync_ns: u64,
    pub commit_directory_sync_ns: u64,
    pub total_ns: u64,
}

#[derive(Resource, Debug, Default)]
pub struct SaveTransactionMetrics {
    pub last: Option<SaveTransactionSample>,
}

impl SaveTransactionMetrics {
    pub fn record(&mut self, sample: SaveTransactionSample) {
        self.last = Some(sample);
    }
}
