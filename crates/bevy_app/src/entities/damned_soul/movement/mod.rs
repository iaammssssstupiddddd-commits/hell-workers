//! ソウルの移動・パス追従・アニメーション

pub mod animation;
pub mod expression_events;

pub use animation::animation_system;
pub use expression_events::{
    apply_conversation_expression_event_system, update_conversation_expression_timer_system,
};
pub use hw_soul_ai::soul_movement;
