//! 使い魔のリクルート管理モジュール
//!
//! ロジックは `hw_ai::familiar_ai::decide::recruitment` へ移設済み。
//! 本ファイルは後方互換のための re-export のみを提供します。

pub use hw_ai::familiar_ai::decide::recruitment::{
    FamiliarRecruitmentContext, RecruitmentManager, RecruitmentOutcome, process_recruitment,
};
