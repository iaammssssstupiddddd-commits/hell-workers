pub mod movement;
pub mod soul_ai;
pub use movement::soul_movement;
pub use soul_ai::SoulAiCorePlugin;
pub use soul_ai::decide::drifting::{DriftingDecisionTimer, drifting_decision_system};
pub use soul_ai::decide::work::auto_build_diagnostics::BlueprintAutoBuildDiagnostics;
pub use soul_ai::helpers::work::{SoulDropCtx, is_soul_available_for_work, unassign_task};
