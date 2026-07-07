use bevy::prelude::Entity;
use bevy::prelude::Message;
use hw_core::game_state::{TaskMode, TimeSpeed};
use hw_jobs::{BuildingCategory, BuildingType};
use hw_logistics::zone::ZoneType;

#[derive(Message, Copy, Clone, Debug)]
pub enum UiIntent {
    ToggleArchitect,
    ToggleZones,
    ToggleOrders,
    ToggleDream,
    ToggleSettings,
    CloseSettings,
    SetUiScale(f32),
    SetCameraPanSpeed(f32),
    SetCameraMousePanEnabled(bool),
    SetDefaultTimeSpeed(TimeSpeed),
    SetDebugGizmosEnabled(bool),
    SetFpsDisplayEnabled(bool),
    InspectEntity(Entity),
    ClearInspectPin,
    SelectBuild(BuildingType),
    SelectFloorPlace,
    SelectZone(ZoneType),
    RemoveZone(ZoneType),
    SelectTaskMode(TaskMode),
    SelectAreaTask,
    SelectDreamPlanting,
    ToggleDoorLock(Entity),
    OpenOperationDialog,
    AdjustFatigueThreshold(f32),
    AdjustMaxControlledSoul(isize),
    AdjustMaxControlledSoulFor(Entity, isize),
    CloseDialog,
    SetTimeSpeed(TimeSpeed),
    TogglePause,
    SaveGame,
    RequestLoadGame,
    ConfirmLoadGame,
    CancelLoadConfirm,
    SelectArchitectCategory(Option<BuildingCategory>),
    MovePlantBuilding(Entity),
}

impl UiIntent {
    pub const fn is_specialized(&self) -> bool {
        matches!(
            self,
            Self::ToggleDoorLock(_) | Self::SelectArchitectCategory(_) | Self::MovePlantBuilding(_)
        )
    }
}
