use bevy::prelude::*;

/// Mirror of `hw_jobs::BuildingType`. Independent enum defined in `hw_core`.
/// Synced by `on_building_added_sync_visual` (Observer) and
/// `sync_building_visual_system` (Changed<Building>) in `hw_jobs`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BuildingTypeVisual {
    #[default]
    Wall,
    Door,
    Floor,
    Tank,
    MudMixer,
    RestArea,
    Bridge,
    SandPile,
    BonePile,
    WheelbarrowParking,
    SoulSpa,
    OutdoorLamp,
}

/// Mirror of `hw_jobs::Building` carrying only the data `hw_visual` needs.
/// Inserted by `on_building_added_sync_visual` (Observer) and updated by
/// `sync_building_visual_system` (Changed<Building>) in `hw_jobs`.
#[derive(Component, Default)]
pub struct BuildingVisualState {
    pub kind: BuildingTypeVisual,
    pub is_provisional: bool,
}

/// Mirror of MudMixer's active state for `hw_visual`.
/// Inserted by `on_mud_mixer_storage_added` (Observer) and updated by
/// `sync_mud_mixer_active_system` (every Logic frame) in `hw_jobs`.
#[derive(Component, Default)]
pub struct MudMixerVisualState {
    pub is_active: bool,
}
