//! Work helper shell.
//!
//! `is_soul_available_for_work` と `unassign_task` は hw_ai に移設済み。
//! このファイルは後方互換のための re-export シェル。

pub use hw_ai::soul_ai::helpers::work::is_soul_available_for_work;
pub use hw_ai::soul_ai::helpers::work::unassign_task;
