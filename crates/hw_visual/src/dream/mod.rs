mod components;
pub mod dream_bubble_material;
mod gain_visual;
mod handles;
mod particle;
#[cfg(feature = "profiling")]
mod perf;
mod ui_handles;
mod ui_particle;

pub use components::{
    DreamGainPopup, DreamGainUiParticle, DreamIconAbsorb, DreamParticle, DreamTrailGhost,
    DreamVisualState,
};
pub use dream_bubble_material::{DreamBubbleMaterial, DreamBubbleUiMaterial};
pub use gain_visual::{
    DreamPresentationLedger, dream_popup_spawn_system, dream_popup_update_system,
    ingest_dream_transfers_system,
};
pub use handles::{DreamBubbleHandles, init_dream_bubble_handles};
pub use particle::{
    dream_particle_spawn_system, dream_particle_update_system, ensure_dream_visual_state_system,
    rest_area_dream_particle_spawn_system,
};
#[cfg(feature = "profiling")]
pub use perf::{
    DreamUiPerfControl, DreamUiPerfMetrics, DreamUiPerfParticle,
    maintain_dream_ui_perf_burst_system, observe_dream_ui_perf_system,
};
pub use ui_handles::{
    DreamBubbleUiHandles, DreamUiMaterialBucket, alpha_to_bucket, apply_ui_material_bucket,
    bucket_material_index, color_to_bucket, init_dream_bubble_ui_handles, mass_to_bucket,
};
#[cfg(feature = "profiling")]
pub(crate) use ui_particle::spawn_ui_particle_with_rng;
pub use ui_particle::{
    dream_icon_absorb_system, dream_trail_ghost_update_system, spawn_ui_particle,
    ui_particle_merge_system, ui_particle_update_system,
};
