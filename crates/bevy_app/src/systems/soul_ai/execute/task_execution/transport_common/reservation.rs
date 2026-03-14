//! 運搬予約 facade — 実装は hw_soul_ai に移設済み。

pub use hw_soul_ai::soul_ai::execute::task_execution::transport_common::reservation::{
    record_picked_source, record_stored_destination, release_destination,
    release_mixer_destination, release_source,
};
