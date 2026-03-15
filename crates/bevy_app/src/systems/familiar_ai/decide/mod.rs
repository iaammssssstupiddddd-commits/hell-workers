pub use hw_familiar_ai::familiar_ai::decide::FamiliarDecideOutput;

pub mod auto_gather_for_blueprint;
pub mod encouragement;
pub mod familiar_processor;
pub mod recruitment {
    pub use hw_familiar_ai::familiar_ai::decide::recruitment::{
        FamiliarRecruitmentContext, RecruitmentManager, RecruitmentOutcome, process_recruitment,
    };
}
pub mod scouting {
    pub use hw_familiar_ai::familiar_ai::decide::scouting::{
        FamiliarScoutingContext, scouting_logic,
    };
}
pub mod squad {
    pub use hw_familiar_ai::familiar_ai::decide::squad::SquadManager;
}
pub mod state_handlers {
    pub use hw_familiar_ai::familiar_ai::decide::state_handlers::{
        StateTransitionResult, idle, scouting, searching, supervising,
    };
}
pub mod supervising {
    pub use hw_familiar_ai::familiar_ai::decide::supervising::{
        FamiliarSupervisingContext, move_to_center, supervising_logic,
    };
}
pub mod task_delegation;
pub mod task_management {
    pub use hw_familiar_ai::familiar_ai::decide::task_management::{
        FamiliarTaskAssignmentQueries, IncomingDeliverySnapshot, ReservationShadow, TaskManager,
        take_reachable_with_cache_calls, take_source_selector_scan_snapshot,
    };
}

pub mod following {
    pub use hw_familiar_ai::familiar_ai::decide::following::*;
}
