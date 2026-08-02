use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_familiar_ai::{
    FamiliarSettingsChangeOutcome, FamiliarSettingsChangeRequest, FamiliarSettingsChangeStatus,
    FamiliarSettingsRejection,
};
use hw_ui::UiIntent;
use hw_ui::components::{OperationDialog, OperationDialogState, UiInputState};

use crate::input_actions::{InputOverlay, PendingWorldInputCapture};

#[derive(SystemParam)]
pub(crate) struct FamiliarSettingsIntentCtx<'w, 's> {
    pub(crate) dialog_state: ResMut<'w, OperationDialogState>,
    ui_input_state: Res<'w, UiInputState>,
    operation_roots: Query<'w, 's, Entity, With<OperationDialog>>,
    requests: MessageWriter<'w, FamiliarSettingsChangeRequest>,
    outcomes: MessageWriter<'w, FamiliarSettingsChangeOutcome>,
}

impl FamiliarSettingsIntentCtx<'_, '_> {
    fn reject(&mut self, target: Entity, reason: FamiliarSettingsRejection) {
        self.outcomes.write(FamiliarSettingsChangeOutcome {
            target,
            status: FamiliarSettingsChangeStatus::Rejected {
                requested_patches: 1,
                reason,
            },
        });
    }

    fn operation_is_foreground(&self) -> bool {
        self.operation_roots
            .single()
            .is_ok_and(|root| self.ui_input_state.foreground_capture_root == Some(root))
    }
}

pub(crate) fn handle(
    intent: UiIntent,
    pending: &PendingWorldInputCapture,
    simulation_paused: bool,
    ctx: &mut FamiliarSettingsIntentCtx<'_, '_>,
) {
    match intent {
        UiIntent::ApplyFamiliarSettings { patch } => {
            let Some(target) = ctx.dialog_state.target else {
                ctx.reject(
                    Entity::PLACEHOLDER,
                    FamiliarSettingsRejection::PausedOrModal,
                );
                return;
            };
            let pending_is_operation_or_empty = pending
                .overlay()
                .is_none_or(|overlay| overlay == InputOverlay::OperationDialog);
            if simulation_paused || !pending_is_operation_or_empty || !ctx.operation_is_foreground()
            {
                ctx.reject(target, FamiliarSettingsRejection::PausedOrModal);
                return;
            }
            ctx.requests
                .write(FamiliarSettingsChangeRequest { target, patch });
        }
        UiIntent::ApplyFamiliarSettingsFor { target, patch } => {
            if simulation_paused
                || ctx.ui_input_state.world_input_captured
                || pending.overlay().is_some()
            {
                ctx.reject(target, FamiliarSettingsRejection::PausedOrModal);
                return;
            }
            ctx.requests
                .write(FamiliarSettingsChangeRequest { target, patch });
        }
        _ => {}
    }
}
