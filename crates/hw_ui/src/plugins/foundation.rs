use crate::components::{
    PlacementFailureTooltip, SectionFolded, SoulRenameState, UiInputState, UiNodeRegistry,
    UnassignedFolded,
};
use crate::interaction::{
    TextFieldPendingAction, apply_text_field_pending_action_system, on_text_field_focus_gained,
    on_text_field_focus_lost, on_text_field_keyboard_input,
    reset_text_input_consumed_keyboard_system, text_input_focus_sync_system,
};
use crate::list::search::EntityListSearchState;
use crate::theme::UiTheme;
use bevy::input_focus::InputFocusSystems;
use bevy::prelude::*;

pub struct UiFoundationPlugin;

impl Plugin for UiFoundationPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<SectionFolded>();
        app.register_type::<UnassignedFolded>();
        app.init_resource::<UiInputState>();
        app.init_resource::<PlacementFailureTooltip>();
        app.init_resource::<UiNodeRegistry>();
        app.init_resource::<UiTheme>();
        app.init_resource::<SoulRenameState>();
        app.init_resource::<EntityListSearchState>();
        app.init_resource::<TextFieldPendingAction>();
        app.add_observer(on_text_field_keyboard_input)
            .add_observer(on_text_field_focus_gained)
            .add_observer(on_text_field_focus_lost)
            .add_systems(
                PreUpdate,
                reset_text_input_consumed_keyboard_system.before(InputFocusSystems::Dispatch),
            )
            .add_systems(
                PreUpdate,
                (
                    text_input_focus_sync_system,
                    apply_text_field_pending_action_system.after(text_input_focus_sync_system),
                )
                    .after(InputFocusSystems::Dispatch),
            );
    }
}
