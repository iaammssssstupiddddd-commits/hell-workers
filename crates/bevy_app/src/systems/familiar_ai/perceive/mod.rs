pub mod resource_sync;
pub mod state_detection {
    pub use hw_familiar_ai::familiar_ai::perceive::state_detection::{
        FamiliarAiStateHistory, detect_command_changes_system, detect_state_changes_system,
        determine_transition_reason,
    };
}
