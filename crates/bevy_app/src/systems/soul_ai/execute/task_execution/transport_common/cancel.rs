//! 運搬中断 facade — 実装は hw_soul_ai に移設済み。

pub use hw_soul_ai::soul_ai::execute::task_execution::transport_common::cancel::{
    cancel_haul_to_blueprint, cancel_haul_to_mixer, cancel_haul_to_mixer_before_pickup,
    cancel_haul_to_stockpile, drop_bucket_with_cleanup,
};
