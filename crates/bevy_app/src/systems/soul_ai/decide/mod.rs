pub use hw_soul_ai::soul_ai::decide::SoulDecideOutput;

pub mod drifting;
pub mod work;

pub mod escaping {
    pub use hw_soul_ai::soul_ai::decide::escaping::escaping_decision_system;
}
pub mod gathering_mgmt {
    pub use hw_soul_ai::soul_ai::decide::gathering_mgmt::{
        gathering_leave_decision, gathering_maintenance_decision, gathering_merge_decision,
        gathering_recruitment_decision,
    };
}
pub mod idle_behavior {
    pub use hw_soul_ai::soul_ai::decide::idle_behavior::idle_behavior_decision_system;
}
pub mod separation {
    pub use hw_soul_ai::soul_ai::decide::separation::*;
}
