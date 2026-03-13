//! 作業管理モジュール
//!
//! 魂へのタスク解除や自動割り当てロジックを管理します。

pub mod auto_build {
    pub use hw_soul_ai::soul_ai::decide::work::auto_build::*;
}
pub mod auto_refine {
    pub use hw_soul_ai::soul_ai::decide::work::auto_refine::*;
}
