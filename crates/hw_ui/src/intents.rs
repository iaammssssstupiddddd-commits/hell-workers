use bevy::prelude::{Entity, Message, Vec2};
use hw_core::game_state::{TaskMode, TimeSpeed};
use hw_core::jobs::WorkType;
use hw_jobs::{BuildingCategory, BuildingType};
use hw_logistics::{StockpilePolicyPatch, zone::ZoneType};

use crate::panels::task_list::{TaskCancelKind, TaskPriorityAdjustment};

/// Copyable target descriptor resolved by the root adapter into concrete stockpile entities.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum StockpilePolicyEditTarget {
    Single(Entity),
    Area { min: Vec2, max: Vec2 },
}

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
    ApplyStockpilePolicy {
        target: StockpilePolicyEditTarget,
        patch: StockpilePolicyPatch,
    },
    BeginStockpilePolicyRangeEdit {
        patch: StockpilePolicyPatch,
    },
    AdjustTaskPriority {
        entity: Entity,
        expected_work_type: WorkType,
        adjustment: TaskPriorityAdjustment,
    },
    CancelTask {
        entity: Entity,
        expected_work_type: WorkType,
        expected_kind: TaskCancelKind,
    },
}

impl UiIntent {
    pub const fn is_specialized(&self) -> bool {
        matches!(
            self,
            Self::AdjustTaskPriority { .. } | Self::CancelTask { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_task_dashboard_actions_have_a_specialized_consumer() {
        assert!(
            UiIntent::AdjustTaskPriority {
                entity: Entity::PLACEHOLDER,
                expected_work_type: WorkType::Chop,
                adjustment: TaskPriorityAdjustment::Increase,
            }
            .is_specialized()
        );
        assert!(
            UiIntent::CancelTask {
                entity: Entity::PLACEHOLDER,
                expected_work_type: WorkType::Chop,
                expected_kind: TaskCancelKind::GenericDesignation,
            }
            .is_specialized()
        );
        assert!(!UiIntent::ToggleDoorLock(Entity::PLACEHOLDER).is_specialized());
        assert!(!UiIntent::SelectArchitectCategory(Some(BuildingCategory::Plant)).is_specialized());
        assert!(!UiIntent::MovePlantBuilding(Entity::PLACEHOLDER).is_specialized());
        assert!(
            !UiIntent::ApplyStockpilePolicy {
                target: StockpilePolicyEditTarget::Single(Entity::PLACEHOLDER),
                patch: StockpilePolicyPatch::default(),
            }
            .is_specialized()
        );
        assert!(
            !UiIntent::BeginStockpilePolicyRangeEdit {
                patch: StockpilePolicyPatch::default(),
            }
            .is_specialized()
        );
    }
}
