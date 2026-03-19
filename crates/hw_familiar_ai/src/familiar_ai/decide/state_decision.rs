//! 使い魔 AI の状態判断 dispatch core + Bevy System（facade）
//!
//! `FamiliarCommand` / `FamiliarAiState` / 分隊人数から「どのサブ処理に進むか」を
//! pure function で決定する。
//!
//! # 設計原則
//! - `determine_decision_path` は hw_core 型のみを使う pure function。
//! - `FamiliarStateDecisionResult` は `familiar_ai_state_system` が MessageWriter に変換するデータ。
//! - lens 構築と MessageWriter 呼び出しは `familiar_ai_state_system` が担う。

mod path;
mod result;
mod system;

pub use self::path::{FamiliarDecisionPath, determine_decision_path};
pub use self::result::FamiliarStateDecisionResult;
pub use self::system::{FamiliarAiStateDecisionParams, familiar_ai_state_system};
