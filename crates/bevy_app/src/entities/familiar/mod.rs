//! 使い魔エンティティ

mod animation;
mod components;
mod range_indicator;
mod spawn;

pub use animation::familiar_animation_system;
pub use components::{
    ActiveCommand, Familiar, FamiliarColorAllocator, FamiliarCommand, FamiliarOperation,
    FamiliarType,
};
pub use hw_familiar_ai::familiar_movement;
pub use range_indicator::update_familiar_range_indicator;
pub use spawn::{FamiliarSpawnEvent, familiar_spawning_system, spawn_familiar};
