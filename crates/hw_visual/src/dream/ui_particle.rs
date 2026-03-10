mod icon;
mod merge;
mod trail;
mod update;

pub use icon::dream_icon_absorb_system;
pub use merge::ui_particle_merge_system;
pub use trail::dream_trail_ghost_update_system;
pub use update::{spawn_ui_particle, ui_particle_update_system};
