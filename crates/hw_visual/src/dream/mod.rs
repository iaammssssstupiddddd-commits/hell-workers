mod components;
pub mod dream_bubble_material;
mod gain_visual;
mod particle;
mod ui_particle;

pub use components::{
    DreamParticle, DreamVisualState, DreamGainPopup, DreamGainUiParticle, DreamTrailGhost,
    DreamIconAbsorb,
};
pub use dream_bubble_material::{DreamBubbleMaterial, DreamBubbleUiMaterial};
pub use gain_visual::{dream_popup_spawn_system, dream_popup_update_system};
pub use particle::{
    ensure_dream_visual_state_system, dream_particle_spawn_system,
    rest_area_dream_particle_spawn_system, dream_particle_update_system,
};
pub use ui_particle::{
    dream_icon_absorb_system, ui_particle_merge_system, dream_trail_ghost_update_system,
    spawn_ui_particle, ui_particle_update_system,
};
