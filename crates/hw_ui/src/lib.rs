use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use std::collections::HashSet;

pub mod area_edit;
pub mod intents;
pub use intents::UiIntent;
pub mod components;
pub mod interaction;
pub mod list;
pub mod models;
pub mod notifications;
pub mod panels;
pub mod plugins;
pub mod setup;
pub mod text_input_intents;
pub use text_input_intents::TextInputIntent;
pub mod theme;
pub mod widgets;

pub struct HwUiPlugin;

impl Plugin for HwUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<UiIntent>()
            .add_message::<TextInputIntent>()
            .add_message::<notifications::UserFacingNotification>()
            .init_resource::<notifications::NotificationCenter>()
            .init_resource::<notifications::NotificationUiRuntime>();
    }
}

/// Clears UI-owned state that can retain simulation entity ids across a world
/// replacement while preserving the static UI tree and its node registry.
///
/// Root plugin facades call this through the save/load reset registry. This
/// crate deliberately does not know about that registry or the root app.
pub fn reset_for_world_replace(world: &mut World) {
    clear_message::<UiIntent>(world);
    clear_message::<TextInputIntent>(world);
    clear_message::<notifications::UserFacingNotification>(world);

    let mut transient_nodes = collect_dynamic_list_nodes(world);
    transient_nodes.extend(collect_task_list_nodes(world));
    transient_nodes.extend(collect_context_menu_nodes(world));
    if let Some(rename_state) = world.get_resource::<components::SoulRenameState>()
        && let Some(active) = rename_state.active
    {
        transient_nodes.insert(active.field_root);
    }
    if let Some(drag_state) = world.get_resource::<list::DragState>()
        && let Some(ghost) = drag_state.ghost_entity
    {
        transient_nodes.insert(ghost);
    }
    for entity in transient_nodes {
        if world.get_entity(entity).is_ok() {
            world.despawn(entity);
        }
    }

    clear_hover_action_targets(world);
    reset_existing_resource::<components::UiInputState>(world);
    reset_existing_resource::<components::SoulRenameState>(world);
    reset_existing_resource::<panels::info_panel::InfoPanelState>(world);
    reset_existing_resource::<panels::info_panel::InfoPanelPinState>(world);
    reset_existing_resource::<models::inspection::EntityInspectionViewModel>(world);
    reset_existing_resource::<list::EntityListViewModel>(world);
    reset_existing_resource::<list::EntityListNodeIndex>(world);
    reset_existing_resource::<list::DragState>(world);
    reset_existing_resource::<area_edit::AreaEditSession>(world);
    reset_existing_resource::<area_edit::AreaEditHistory>(world);
    reset_existing_resource::<area_edit::AreaEditClipboard>(world);
    reset_existing_resource::<interaction::TextFieldPendingAction>(world);
    reset_existing_resource::<selection::PlacementFeedbackState>(world);
    notifications::reset_for_world_replace(world);
    mark_entity_list_dirty(world);

    if world.contains_resource::<InputFocus>() {
        // A fresh resource drops both the active focus and buffered focus
        // transitions that could otherwise mention a removed rename field.
        world.insert_resource(InputFocus::default());
    }
}

fn clear_message<T: Message>(world: &mut World) {
    if let Some(mut messages) = world.get_resource_mut::<Messages<T>>() {
        messages.clear();
    }
}

fn collect_dynamic_list_nodes(world: &World) -> HashSet<Entity> {
    let Some(index) = world.get_resource::<list::EntityListNodeIndex>() else {
        return HashSet::new();
    };

    let mut nodes = HashSet::new();
    for section in index.familiar_sections.values() {
        nodes.insert(section.root);
        nodes.insert(section.header_text);
        nodes.insert(section.fold_icon);
        nodes.insert(section.members_container);
    }
    for rows in index.familiar_member_rows.values() {
        nodes.extend(rows.values().copied());
    }
    nodes.extend(index.familiar_empty_rows.values().copied());
    nodes.extend(index.unassigned_rows.values().copied());
    nodes
}

fn collect_task_list_nodes(world: &mut World) -> HashSet<Entity> {
    let mut query = world.query_filtered::<Entity, With<components::TaskListItem>>();
    query.iter(world).collect()
}

fn collect_context_menu_nodes(world: &mut World) -> HashSet<Entity> {
    let mut query = world.query_filtered::<Entity, With<components::ContextMenu>>();
    query.iter(world).collect()
}

fn clear_hover_action_targets(world: &mut World) {
    let mut query = world.query::<&mut components::HoverActionOverlay>();
    for mut overlay in query.iter_mut(world) {
        overlay.target = None;
    }
}

fn reset_existing_resource<T: Resource + Default>(world: &mut World) {
    if world.contains_resource::<T>() {
        world.insert_resource(T::default());
    }
}

fn mark_entity_list_dirty(world: &mut World) {
    if let Some(mut dirty) = world.get_resource_mut::<list::EntityListDirty>() {
        dirty.mark_structure();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_replace_reset_clears_entity_bearing_ui_state() {
        let mut world = World::new();
        let stale_simulation_entity = world.spawn_empty().id();
        let rename_field = world.spawn_empty().id();
        let drag_ghost = world.spawn_empty().id();
        let list_row = world.spawn_empty().id();
        let toast_row = world.spawn(notifications::NotificationToastRow).id();
        let history_row = world.spawn(notifications::NotificationHistoryRow).id();
        let toast_root = world
            .spawn((
                Node {
                    display: Display::Flex,
                    ..default()
                },
                notifications::NotificationToastRoot,
            ))
            .id();
        let history_panel = world
            .spawn((
                Node {
                    display: Display::Flex,
                    ..default()
                },
                notifications::NotificationHistoryPanel,
            ))
            .id();
        let unread_text = world
            .spawn((Text::new("通知 (1)"), notifications::NotificationUnreadText))
            .id();

        world.insert_resource(components::SoulRenameState {
            active: Some(components::SoulRenameActive {
                target: stale_simulation_entity,
                field_root: rename_field,
            }),
        });
        world.insert_resource(panels::info_panel::InfoPanelPinState {
            entity: Some(stale_simulation_entity),
        });
        world.insert_resource(list::DragState {
            active_soul: Some(stale_simulation_entity),
            ghost_entity: Some(drag_ghost),
            ..default()
        });
        let mut node_index = list::EntityListNodeIndex::default();
        node_index
            .unassigned_rows
            .insert(stale_simulation_entity, list_row);
        world.insert_resource(node_index);
        world.insert_resource(list::EntityListDirty::default());
        world.init_resource::<Messages<UiIntent>>();
        world.init_resource::<Messages<TextInputIntent>>();
        world.init_resource::<Messages<notifications::UserFacingNotification>>();
        world.init_resource::<notifications::NotificationCenter>();
        world.init_resource::<notifications::NotificationUiRuntime>();
        let mut placement_feedback = selection::PlacementFeedbackState::default();
        placement_feedback.show_recent_rejection(
            selection::PlacementRejectReason::OutOfBounds,
            (0, 0),
            std::time::Duration::ZERO,
        );
        world.insert_resource(placement_feedback);
        world
            .resource_mut::<Messages<UiIntent>>()
            .write(UiIntent::InspectEntity(stale_simulation_entity));
        world
            .resource_mut::<Messages<TextInputIntent>>()
            .write(TextInputIntent::RenameSoul {
                entity: stale_simulation_entity,
                name: "stale".to_string(),
            });
        let notification = notifications::UserFacingNotification::new(
            "stale",
            notifications::NotificationSeverity::Warning,
            "stale",
            "stale",
            notifications::NotificationRetention::Important,
        );
        world
            .resource_mut::<Messages<notifications::UserFacingNotification>>()
            .write(notification.clone());
        world
            .resource_mut::<notifications::NotificationCenter>()
            .push(notification, std::time::Duration::ZERO);

        reset_for_world_replace(&mut world);

        assert!(
            world
                .resource::<components::SoulRenameState>()
                .active
                .is_none()
        );
        assert!(
            world
                .resource::<panels::info_panel::InfoPanelPinState>()
                .entity
                .is_none()
        );
        assert!(world.resource::<list::DragState>().active_soul.is_none());
        assert!(world.get_entity(rename_field).is_err());
        assert!(world.get_entity(drag_ghost).is_err());
        assert!(world.get_entity(list_row).is_err());
        assert!(world.get_entity(toast_row).is_err());
        assert!(world.get_entity(history_row).is_err());
        assert!(
            world
                .resource::<list::EntityListNodeIndex>()
                .unassigned_rows
                .is_empty()
        );
        assert!(
            world
                .resource::<list::EntityListDirty>()
                .needs_structure_sync()
        );
        assert!(world.resource::<Messages<UiIntent>>().is_empty());
        assert!(world.resource::<Messages<TextInputIntent>>().is_empty());
        assert!(
            world
                .resource::<Messages<notifications::UserFacingNotification>>()
                .is_empty()
        );
        assert_eq!(
            world
                .resource::<notifications::NotificationCenter>()
                .toast_count(),
            0
        );
        assert_eq!(
            world
                .resource::<notifications::NotificationCenter>()
                .history_count(),
            0
        );
        assert_eq!(
            world.get::<Node>(toast_root).unwrap().display,
            Display::None
        );
        assert_eq!(
            world.get::<Node>(history_panel).unwrap().display,
            Display::None
        );
        assert_eq!(world.get::<Text>(unread_text).unwrap().0, "通知");
        assert!(
            world
                .resource::<selection::PlacementFeedbackState>()
                .visible(std::time::Duration::ZERO)
                .is_none()
        );
    }
}
pub mod camera;
pub mod selection;
