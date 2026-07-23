use bevy::prelude::MessageWriter;
use hw_ui::UiIntent;
use hw_ui::components::MenuAction;

pub(super) fn handle_pressed_action(action: MenuAction, ui_intents: &mut MessageWriter<UiIntent>) {
    match action {
        MenuAction::InspectEntity(entity) => {
            ui_intents.write(UiIntent::InspectEntity(entity));
        }
        MenuAction::ClearInspectPin => {
            ui_intents.write(UiIntent::ClearInspectPin);
        }
        MenuAction::ToggleArchitect => {
            ui_intents.write(UiIntent::ToggleArchitect);
        }
        MenuAction::ToggleOrders => {
            ui_intents.write(UiIntent::ToggleOrders);
        }
        MenuAction::ToggleZones => {
            ui_intents.write(UiIntent::ToggleZones);
        }
        MenuAction::ToggleDream => {
            ui_intents.write(UiIntent::ToggleDream);
        }
        MenuAction::ToggleSettings => {
            ui_intents.write(UiIntent::ToggleSettings);
        }
        MenuAction::CloseSettings => {
            ui_intents.write(UiIntent::CloseSettings);
        }
        MenuAction::SetDefaultTimeSpeed(speed) => {
            ui_intents.write(UiIntent::SetDefaultTimeSpeed(speed));
        }
        MenuAction::SelectBuild(kind) => {
            ui_intents.write(UiIntent::SelectBuild(kind));
        }
        MenuAction::SelectFloorPlace => {
            ui_intents.write(UiIntent::SelectFloorPlace);
        }
        MenuAction::SelectZone(kind) => {
            ui_intents.write(UiIntent::SelectZone(kind));
        }
        MenuAction::RemoveZone(kind) => {
            ui_intents.write(UiIntent::RemoveZone(kind));
        }
        MenuAction::SelectTaskMode(mode) => {
            ui_intents.write(UiIntent::SelectTaskMode(mode));
        }
        MenuAction::SelectAreaTask => {
            ui_intents.write(UiIntent::SelectAreaTask);
        }
        MenuAction::SelectDreamPlanting => {
            ui_intents.write(UiIntent::SelectDreamPlanting);
        }
        MenuAction::OpenOperationDialog => {
            ui_intents.write(UiIntent::OpenOperationDialog);
        }
        MenuAction::CloseDialog => {
            ui_intents.write(UiIntent::CloseDialog);
        }
        MenuAction::AdjustFatigueThreshold(delta) => {
            ui_intents.write(UiIntent::AdjustFatigueThreshold(delta));
        }
        MenuAction::AdjustMaxControlledSoul(delta) => {
            ui_intents.write(UiIntent::AdjustMaxControlledSoul(delta));
        }
        MenuAction::AdjustMaxControlledSoulFor(entity, delta) => {
            ui_intents.write(UiIntent::AdjustMaxControlledSoulFor(entity, delta));
        }
        MenuAction::SetTimeSpeed(speed) => {
            ui_intents.write(UiIntent::SetTimeSpeed(speed));
        }
        MenuAction::TogglePause => {
            ui_intents.write(UiIntent::TogglePause);
        }
        MenuAction::SaveGame => {
            ui_intents.write(UiIntent::SaveGame);
        }
        MenuAction::RequestLoadGame => {
            ui_intents.write(UiIntent::RequestLoadGame);
        }
        MenuAction::ConfirmLoadGame => {
            ui_intents.write(UiIntent::ConfirmLoadGame);
        }
        MenuAction::CancelLoadConfirm => {
            ui_intents.write(UiIntent::CancelLoadConfirm);
        }
        MenuAction::ToggleDoorLock(entity) => {
            ui_intents.write(UiIntent::ToggleDoorLock(entity));
        }
        MenuAction::SelectArchitectCategory(kind) => {
            ui_intents.write(UiIntent::SelectArchitectCategory(kind));
        }
        MenuAction::MovePlantBuilding(entity) => {
            ui_intents.write(UiIntent::MovePlantBuilding(entity));
        }
        MenuAction::ApplyStockpilePolicy { target, patch } => {
            ui_intents.write(UiIntent::ApplyStockpilePolicy { target, patch });
        }
        MenuAction::BeginStockpilePolicyRangeEdit { patch } => {
            ui_intents.write(UiIntent::BeginStockpilePolicyRangeEdit { patch });
        }
        MenuAction::AdjustTaskPriority {
            entity,
            expected_work_type,
            adjustment,
        } => {
            ui_intents.write(UiIntent::AdjustTaskPriority {
                entity,
                expected_work_type,
                adjustment,
            });
        }
        MenuAction::CancelTask {
            entity,
            expected_work_type,
            expected_kind,
        } => {
            ui_intents.write(UiIntent::CancelTask {
                entity,
                expected_work_type,
                expected_kind,
            });
        }
        MenuAction::SetUiScale(_)
        | MenuAction::SetCameraPanSpeed(_)
        | MenuAction::SetCameraMousePanEnabled(_)
        | MenuAction::SetDebugGizmosEnabled(_)
        | MenuAction::SetFpsDisplayEnabled(_) => {
            // Slider/Checkbox observer 経由で発行される
        }
    }
}
