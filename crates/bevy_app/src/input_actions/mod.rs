mod bindings;
mod context;
mod model;
mod resolver;

#[cfg(test)]
mod tests;

use bevy::input_focus::InputFocusSystems;
use bevy::prelude::*;
use hw_ui::UiIntent;

use crate::systems::GameSystemSet;

pub use context::InputContextSnapshot;
use model::InputConflictLane;
pub use model::{InputAction, InputActionFamily, InputChord, InputModifiers};
pub(crate) use resolver::resolve_input_frame_system;
pub use resolver::{ResolvedInputFrame, resolve_input_chords};

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum InputPreUpdateSet {
    Resolve,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum InputResolutionSet {
    PointerIngress,
    Consume,
}

pub(crate) fn configure_input_resolution_sets(app: &mut App) {
    app.configure_sets(
        PreUpdate,
        InputPreUpdateSet::Resolve
            .after(InputFocusSystems::Dispatch)
            .after(hw_ui::interaction::text_input_focus_sync_system),
    );
    app.configure_sets(
        Update,
        (
            InputResolutionSet::PointerIngress,
            InputResolutionSet::Consume,
        )
            .chain()
            .in_set(GameSystemSet::Input),
    );
}

pub(crate) fn input_action_to_ui_intent_system(
    resolved_frame: Res<ResolvedInputFrame>,
    mut ui_intents: MessageWriter<UiIntent>,
) {
    for action in resolved_frame.actions() {
        if let Some(intent) = ui_intent_for_action(*action) {
            ui_intents.write(intent);
        }
    }
}

fn ui_intent_for_action(action: InputAction) -> Option<UiIntent> {
    match action {
        InputAction::SaveGame => Some(UiIntent::SaveGame),
        InputAction::RequestLoadGame => Some(UiIntent::RequestLoadGame),
        InputAction::CycleElevation => None,
    }
}
