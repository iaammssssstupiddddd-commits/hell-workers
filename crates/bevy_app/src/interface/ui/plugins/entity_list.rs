use crate::interface::ui::list::change_detection::detect_entity_list_changes;
use crate::interface::ui::list::dirty::EntityListDirty;
use crate::interface::ui::{
    DragState, EntityListMinimizeState, EntityListNodeIndex, EntityListResizeState,
    EntityListViewModel, build_entity_list_view_model_system, entity_list_drag_drop_system,
    entity_list_interaction_system, entity_list_minimize_toggle_system,
    entity_list_resize_cursor_system, entity_list_resize_system,
    entity_list_scroll_hint_visibility_system, entity_list_scroll_system,
    entity_list_section_toggle_system, entity_list_tab_focus_system,
    entity_list_visual_feedback_system, sync_entity_list_from_view_model_system,
    sync_entity_list_value_rows_system, update_unassigned_arrow_icon_system,
};
use crate::interface::ui::interaction::handle_ui_intent;
use crate::systems::GameSystemSet;
use crate::systems::command::task_area_edit_cursor_system;
use bevy::prelude::*;

pub type UiEntityListPlugin = hw_ui::plugins::entity_list::UiEntityListPlugin;

pub fn ui_entity_list_plugin() -> UiEntityListPlugin {
    UiEntityListPlugin::new(register_ui_entity_list_plugin_systems)
}

fn register_ui_entity_list_plugin_systems(app: &mut App) {
    app.init_resource::<EntityListViewModel>();
    app.init_resource::<EntityListNodeIndex>();
    app.init_resource::<DragState>();
    app.init_resource::<EntityListMinimizeState>();
    app.init_resource::<EntityListResizeState>();
    app.init_resource::<EntityListDirty>();
    app.add_systems(
        Update,
        (
            entity_list_section_toggle_system,
            entity_list_interaction_system.before(handle_ui_intent),
            entity_list_drag_drop_system,
            entity_list_visual_feedback_system,
            entity_list_scroll_system,
            entity_list_scroll_hint_visibility_system,
            entity_list_tab_focus_system,
            entity_list_minimize_toggle_system,
            entity_list_resize_system,
            entity_list_resize_cursor_system.after(entity_list_resize_system),
            task_area_edit_cursor_system.after(entity_list_resize_cursor_system),
            update_unassigned_arrow_icon_system,
        )
            .in_set(GameSystemSet::Interface),
    )
    .add_systems(
        Update,
        detect_entity_list_changes.in_set(GameSystemSet::Interface),
    )
    .add_systems(
        Update,
        (
            build_entity_list_view_model_system,
            sync_entity_list_from_view_model_system,
        )
            .chain()
            .run_if(|dirty: Res<EntityListDirty>| {
                dirty.needs_structure_sync() || dirty.needs_value_sync_only()
            })
            .after(detect_entity_list_changes)
            .in_set(GameSystemSet::Interface),
    )
    .add_systems(
        Update,
        sync_entity_list_value_rows_system
            .run_if(|dirty: Res<EntityListDirty>| dirty.needs_value_sync_only())
            .after(detect_entity_list_changes)
            .in_set(GameSystemSet::Interface),
    );
}
