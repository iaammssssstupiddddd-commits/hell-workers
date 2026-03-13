//! root adapter: FamiliarDelegationContext と process_task_delegation_and_movement は
//! hw_familiar_ai::familiar_ai::decide::delegation_context へ移動済み。
//!
//! squad / recruitment の pure helper は hw_familiar_ai 側に定義済みで先頭の `pub use` から re-export する。

pub use hw_familiar_ai::familiar_ai::decide::delegation_context::{
    FamiliarDelegationContext, FamiliarRecruitmentContext, FamiliarSquadContext,
    RecruitmentOutcome, SquadManagementOutcome, finalize_state_transitions,
    process_recruitment, process_squad_management, process_task_delegation_and_movement,
};
