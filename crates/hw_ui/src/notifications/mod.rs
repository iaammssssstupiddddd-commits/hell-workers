mod model;
mod reducer;
mod ui;

use bevy::prelude::*;

pub use model::{
    MAX_ACTIVE_TOASTS, MAX_NOTIFICATION_HISTORY, NOTIFICATION_DEDUPE_WINDOW,
    NOTIFICATION_TOAST_LIFETIME, NotificationCenter, NotificationEntry, NotificationEntryId,
    NotificationHistoryButton, NotificationHistoryPanel, NotificationHistoryRow, NotificationKey,
    NotificationRetention, NotificationSeverity, NotificationToastRoot, NotificationToastRow,
    NotificationToastSurface, NotificationUiAssets, NotificationUiRuntime, NotificationUnreadText,
    UserFacingNotification,
};
pub use reducer::{apply_notification_ui_state_system, reduce_notifications_system};
pub use ui::present_notifications_system;
pub(crate) use ui::spawn_notification_ui;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NotificationSystemSet {
    Adapt,
    Reduce,
    Present,
}

pub(crate) fn reset_for_world_replace(world: &mut World) {
    let mut dynamic_rows = world
        .query_filtered::<Entity, Or<(With<NotificationToastRow>, With<NotificationHistoryRow>)>>();
    let rows: Vec<_> = dynamic_rows.iter(world).collect();
    for entity in rows {
        if world.get_entity(entity).is_ok() {
            world.despawn(entity);
        }
    }

    if world.contains_resource::<NotificationCenter>() {
        world.insert_resource(NotificationCenter::default());
    }
    if world.contains_resource::<NotificationUiRuntime>() {
        world.insert_resource(NotificationUiRuntime::default());
    }

    let mut toast_roots = world.query_filtered::<&mut Node, With<NotificationToastRoot>>();
    for mut node in toast_roots.iter_mut(world) {
        node.display = Display::None;
    }
    let mut history_panels = world.query_filtered::<&mut Node, With<NotificationHistoryPanel>>();
    for mut node in history_panels.iter_mut(world) {
        node.display = Display::None;
    }
    let mut unread_labels = world.query_filtered::<&mut Text, With<NotificationUnreadText>>();
    for mut text in unread_labels.iter_mut(world) {
        text.0 = "通知".to_string();
    }
}
