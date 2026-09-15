use bevy::prelude::{Entity, Message, Vec2};
use hw_core::familiar::FamiliarSettingsPatch;
use hw_core::game_state::{TaskMode, TimeSpeed};
use hw_core::jobs::WorkType;
use hw_jobs::{BuildingCategory, BuildingType};
use hw_logistics::{StockpilePolicyPatch, zone::ZoneType};

use crate::help::{HelpScrollCommand, HelpTopicId, HelpTopicStep};
use crate::panels::task_list::{TaskCancelKind, TaskPriorityAdjustment};
use crate::power::PowerPriorityValue;

/// Copyable target descriptor resolved by the root adapter into concrete stockpile entities.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum StockpilePolicyEditTarget {
    Single(Entity),
    Area { min: Vec2, max: Vec2 },
}

#[derive(Message, Copy, Clone, Debug, PartialEq)]
pub enum UiIntent {
    OpenHelp {
        opener: Option<Entity>,
    },
    CloseHelp,
    StartWorkGuide,
    EndWorkGuide,
    SelectHelpTopic(HelpTopicId),
    SelectHelpEntry(crate::help::HelpEntryId),
    StepHelpTopic(HelpTopicStep),
    ScrollHelp(HelpScrollCommand),
    ToggleArchitect,
    ToggleZones,
    ToggleOrders,
    ToggleDream,
    ToggleSettings,
    CloseSettings,
    RetrySettingsSave {
        attempt: u64,
    },
    SetUiScale(f32),
    SetCameraPanSpeed(f32),
    SetCameraMousePanEnabled(bool),
    SetDefaultTimeSpeed(TimeSpeed),
    SetDebugGizmosEnabled(bool),
    SetFpsDisplayEnabled(bool),
    SetPowerPriorityEnabled(bool),
    SetAutosaveEnabled(bool),
    SetAutosaveIntervalMinutes(u32),
    SetAutosaveGenerations(u8),
    SetNotificationDuration(u8),
    InspectEntity(Entity),
    FocusEntity(Entity),
    ToggleFamiliarIdlePatrol(Entity),
    ClearInspectPin,
    SelectBuild(BuildingType),
    SelectFloorPlace,
    SelectZone(ZoneType),
    RemoveZone(ZoneType),
    SelectTaskMode(TaskMode),
    SelectAreaTask,
    AreaEditControl {
        action: crate::area_edit::panel::AreaEditAction,
        revision: u64,
        epoch: u64,
    },
    SelectDreamPlanting,
    ToggleDoorLock(Entity),
    OpenOperationDialog {
        opener: Option<Entity>,
        target: Entity,
    },
    ApplyFamiliarSettings {
        patch: FamiliarSettingsPatch,
    },
    ApplyFamiliarSettingsFor {
        target: Entity,
        patch: FamiliarSettingsPatch,
    },
    CloseDialog,
    SetTimeSpeed(TimeSpeed),
    TogglePause,
    ToggleSystemMenu,
    SaveGame,
    RequestLoadGame,
    CancelLoadConfirm,
    SelectSaveCatalogSlot {
        slot: hw_core::SaveSlotId,
        session: u64,
    },
    ConfirmSaveCatalogSlot {
        slot: hw_core::SaveSlotId,
        session: u64,
    },
    SelectLoadCatalogSlot {
        slot: hw_core::SaveSlotId,
        session: u64,
    },
    ConfirmLoadCatalogSlot {
        slot: hw_core::SaveSlotId,
        session: u64,
    },
    CancelSaveCatalogConfirm,
    CloseSaveCatalog,
    SelectArchitectCategory(Option<BuildingCategory>),
    MovePlantBuilding(Entity),
    ApplyStockpilePolicy {
        target: StockpilePolicyEditTarget,
        patch: StockpilePolicyPatch,
    },
    SetSoulSpaActiveSlots {
        target: Entity,
        active_slots: u32,
    },
    DismissConstructionCancel,
    ConfirmSoulSpaConstructionCancel {
        target: Entity,
        epoch: u64,
        ticket: u64,
    },
    CancelSoulSpaConstruction {
        source_task: Option<Entity>,
        target: Entity,
    },
    SetPowerConsumerPriority {
        target: Entity,
        priority: PowerPriorityValue,
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
    /// Explicit pause boundary, shared by visible controls and root consumers.
    pub const fn allowed_while_paused(&self) -> bool {
        match self {
            Self::SelectTaskMode(
                TaskMode::DesignateChop(_)
                | TaskMode::DesignateMine(_)
                | TaskMode::AreaSelection(_),
            )
            | Self::ApplyStockpilePolicy {
                target: StockpilePolicyEditTarget::Single(_),
                ..
            } => true,
            Self::SelectTaskMode(_)
            | Self::SelectBuild(_)
            | Self::SelectFloorPlace
            | Self::SelectZone(_)
            | Self::RemoveZone(_)
            | Self::SelectDreamPlanting
            | Self::MovePlantBuilding(_)
            | Self::ToggleFamiliarIdlePatrol(_)
            | Self::ApplyFamiliarSettings { .. }
            | Self::ApplyFamiliarSettingsFor { .. }
            | Self::ApplyStockpilePolicy {
                target: StockpilePolicyEditTarget::Area { .. },
                ..
            }
            | Self::BeginStockpilePolicyRangeEdit { .. }
            | Self::SetSoulSpaActiveSlots { .. }
            | Self::SetPowerConsumerPriority { .. }
            | Self::CancelSoulSpaConstruction { .. }
            | Self::ConfirmSoulSpaConstructionCancel { .. }
            | Self::AdjustTaskPriority { .. }
            | Self::CancelTask { .. } => false,
            Self::StartWorkGuide
            | Self::EndWorkGuide
            | Self::OpenHelp { .. }
            | Self::CloseHelp
            | Self::SelectHelpTopic(_)
            | Self::SelectHelpEntry(_)
            | Self::StepHelpTopic(_)
            | Self::ScrollHelp(_)
            | Self::ToggleArchitect
            | Self::ToggleZones
            | Self::ToggleOrders
            | Self::ToggleDream
            | Self::ToggleSettings
            | Self::CloseSettings
            | Self::RetrySettingsSave { .. }
            | Self::SetUiScale(_)
            | Self::SetCameraPanSpeed(_)
            | Self::SetCameraMousePanEnabled(_)
            | Self::SetDefaultTimeSpeed(_)
            | Self::SetDebugGizmosEnabled(_)
            | Self::SetFpsDisplayEnabled(_)
            | Self::SetPowerPriorityEnabled(_)
            | Self::SetAutosaveEnabled(_)
            | Self::SetAutosaveIntervalMinutes(_)
            | Self::SetAutosaveGenerations(_)
            | Self::SetNotificationDuration(_)
            | Self::InspectEntity(_)
            | Self::FocusEntity(_)
            | Self::ClearInspectPin
            | Self::SelectAreaTask
            | Self::AreaEditControl { .. }
            | Self::ToggleDoorLock(_)
            | Self::OpenOperationDialog { .. }
            | Self::CloseDialog
            | Self::SetTimeSpeed(_)
            | Self::TogglePause
            | Self::ToggleSystemMenu
            | Self::SaveGame
            | Self::RequestLoadGame
            | Self::CancelLoadConfirm
            | Self::SelectSaveCatalogSlot { .. }
            | Self::ConfirmSaveCatalogSlot { .. }
            | Self::SelectLoadCatalogSlot { .. }
            | Self::ConfirmLoadCatalogSlot { .. }
            | Self::CancelSaveCatalogConfirm
            | Self::CloseSaveCatalog
            | Self::SelectArchitectCategory(_)
            | Self::DismissConstructionCancel => true,
        }
    }

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
