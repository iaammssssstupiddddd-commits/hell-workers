use bevy::prelude::*;
use hw_world::zones::{AreaBounds, Yard};

#[derive(Clone)]
pub(crate) struct MixerCollectSandCandidate {
    pub(crate) mixer_entity: Entity,
    pub(crate) issued_by: Entity,
    pub(crate) mixer_pos: Vec2,
    pub(crate) owner_area: AreaBounds,
    pub(crate) yard_area: Option<Yard>,
    pub(crate) current_sand: u32,
    pub(crate) sand_inflight: u32,
}
