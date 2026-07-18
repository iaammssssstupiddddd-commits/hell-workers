use super::model::{
    MAX_ACTIVE_TOASTS, MAX_NOTIFICATION_HISTORY, NOTIFICATION_DEDUPE_WINDOW,
    NOTIFICATION_TOAST_LIFETIME, NotificationCenter, NotificationEntry, NotificationEntryId,
    NotificationHistoryButton, NotificationRetention, UserFacingNotification,
};
use crate::components::UiInputState;
use crate::interaction::update_interaction_color;
use crate::theme::UiTheme;
use bevy::prelude::*;
use bevy::time::Real;
use std::time::Duration;

type NotificationHistoryButtonQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static mut BackgroundColor),
    (Changed<Interaction>, With<NotificationHistoryButton>),
>;

impl NotificationCenter {
    pub fn push(&mut self, incoming: UserFacingNotification, now: Duration) {
        if let Some(id) = self.find_dedupe_candidate(&incoming, now) {
            self.coalesce(id, incoming, now);
        } else {
            self.append(incoming, now);
        }
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn expire(&mut self, now: Duration) {
        let expired: Vec<_> = self
            .toasts
            .iter()
            .copied()
            .filter(|id| {
                self.entries
                    .get(id)
                    .is_some_and(|entry| entry.expires_at <= now)
            })
            .collect();
        if expired.is_empty() {
            return;
        }

        self.toasts.retain(|id| !expired.contains(id));
        for id in expired {
            self.remove_if_unreferenced(id);
        }
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn toggle_history(&mut self) {
        self.history_open = !self.history_open;
        if self.history_open {
            self.unread.clear();
        }
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn close_history(&mut self) {
        if !self.history_open {
            return;
        }
        self.history_open = false;
        self.revision = self.revision.wrapping_add(1);
    }

    fn find_dedupe_candidate(
        &self,
        incoming: &UserFacingNotification,
        now: Duration,
    ) -> Option<NotificationEntryId> {
        self.entries
            .values()
            .filter(|entry| entry.key == incoming.key)
            .filter(|entry| now.saturating_sub(entry.last_seen) <= NOTIFICATION_DEDUPE_WINDOW)
            .max_by_key(|entry| entry.last_seen)
            .map(|entry| entry.id)
    }

    fn coalesce(
        &mut self,
        id: NotificationEntryId,
        incoming: UserFacingNotification,
        now: Duration,
    ) {
        let is_important = {
            let entry = self
                .entries
                .get_mut(&id)
                .expect("dedupe candidate must refer to an entry");
            entry.severity = incoming.severity;
            entry.title = incoming.title;
            entry.body = incoming.body;
            entry.retention = entry.retention.merge(incoming.retention);
            entry.last_seen = now;
            entry.expires_at = now + NOTIFICATION_TOAST_LIFETIME;
            entry.repeat_count = entry.repeat_count.saturating_add(1);
            entry.retention == NotificationRetention::Important
        };

        self.move_to_back_of_toasts(id);
        if is_important {
            self.move_to_back_of_history(id);
            if !self.history_open {
                self.unread.insert(id);
            }
        }
        self.enforce_limits();
    }

    fn append(&mut self, incoming: UserFacingNotification, now: Duration) {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let retention = incoming.retention;
        self.entries.insert(
            id,
            NotificationEntry {
                id,
                key: incoming.key,
                severity: incoming.severity,
                title: incoming.title,
                body: incoming.body,
                retention,
                first_seen: now,
                last_seen: now,
                repeat_count: 1,
                expires_at: now + NOTIFICATION_TOAST_LIFETIME,
            },
        );
        self.toasts.push_back(id);
        if retention == NotificationRetention::Important {
            self.history.push_back(id);
            if !self.history_open {
                self.unread.insert(id);
            }
        }
        self.enforce_limits();
    }

    fn move_to_back_of_toasts(&mut self, id: NotificationEntryId) {
        self.toasts.retain(|candidate| *candidate != id);
        self.toasts.push_back(id);
    }

    fn move_to_back_of_history(&mut self, id: NotificationEntryId) {
        self.history.retain(|candidate| *candidate != id);
        self.history.push_back(id);
    }

    fn enforce_limits(&mut self) {
        while self.toasts.len() > MAX_ACTIVE_TOASTS {
            if let Some(id) = self.toasts.pop_front() {
                self.remove_if_unreferenced(id);
            }
        }
        while self.history.len() > MAX_NOTIFICATION_HISTORY {
            if let Some(id) = self.history.pop_front() {
                self.unread.remove(&id);
                self.remove_if_unreferenced(id);
            }
        }
    }

    fn remove_if_unreferenced(&mut self, id: NotificationEntryId) {
        if !self.toasts.contains(&id) && !self.history.contains(&id) {
            self.entries.remove(&id);
        }
    }
}

pub fn reduce_notifications_system(
    mut notifications: MessageReader<UserFacingNotification>,
    real_time: Res<Time<Real>>,
    mut center: ResMut<NotificationCenter>,
) {
    let now = real_time.elapsed();
    center.expire(now);
    for notification in notifications.read().cloned() {
        center.push(notification, now);
    }
}

pub fn apply_notification_ui_state_system(
    mut buttons: NotificationHistoryButtonQuery,
    mut button_nodes: Query<&mut Node, With<NotificationHistoryButton>>,
    input_state: Res<UiInputState>,
    theme: Res<UiTheme>,
    mut center: ResMut<NotificationCenter>,
) {
    for mut node in &mut button_nodes {
        node.display = if input_state.world_input_captured {
            Display::None
        } else {
            Display::Flex
        };
    }
    if input_state.world_input_capture_started || input_state.world_input_captured {
        center.close_history();
        return;
    }

    for (interaction, mut color) in &mut buttons {
        update_interaction_color(*interaction, &mut color, &theme);
        if *interaction == Interaction::Pressed {
            center.toggle_history();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::{NotificationRetention, NotificationSeverity};

    fn notification(
        key: impl Into<String>,
        retention: NotificationRetention,
    ) -> UserFacingNotification {
        let key = key.into();
        UserFacingNotification::new(
            key.clone(),
            NotificationSeverity::Info,
            format!("title {key}"),
            format!("body {key}"),
            retention,
        )
    }

    #[test]
    fn same_key_inside_window_coalesces_and_extends_expiry() {
        let mut center = NotificationCenter::default();
        center.push(
            notification("same", NotificationRetention::ToastOnly),
            Duration::ZERO,
        );
        center.push(
            UserFacingNotification::new(
                "same",
                NotificationSeverity::Warning,
                "updated",
                "new body",
                NotificationRetention::Important,
            ),
            Duration::from_secs(2),
        );

        assert_eq!(center.toast_count(), 1);
        assert_eq!(center.history_count(), 1);
        let entry = center.toast_entries().next().unwrap();
        assert_eq!(entry.repeat_count, 2);
        assert_eq!(entry.title, "updated");
        assert_eq!(entry.severity, NotificationSeverity::Warning);

        center.expire(Duration::from_secs(5));
        assert_eq!(center.toast_count(), 1);
        center.expire(Duration::from_secs(6));
        assert_eq!(center.toast_count(), 0);
        assert_eq!(center.history_count(), 1);
    }

    #[test]
    fn same_key_outside_window_creates_a_new_entry() {
        let mut center = NotificationCenter::default();
        center.push(
            notification("same", NotificationRetention::Important),
            Duration::ZERO,
        );
        center.push(
            notification("same", NotificationRetention::Important),
            Duration::from_secs(2) + Duration::from_nanos(1),
        );

        assert_eq!(center.toast_count(), 2);
        assert_eq!(center.history_count(), 2);
    }

    #[test]
    fn toast_and_history_limits_evict_oldest_entries() {
        let mut center = NotificationCenter::default();
        for index in 0..70 {
            center.push(
                notification(format!("key-{index}"), NotificationRetention::Important),
                Duration::from_secs(index),
            );
        }

        assert_eq!(center.toast_count(), MAX_ACTIVE_TOASTS);
        assert_eq!(center.history_count(), MAX_NOTIFICATION_HISTORY);
        assert_eq!(center.unread_count(), MAX_NOTIFICATION_HISTORY);
        assert_eq!(
            center.history_entries().next().unwrap().key.as_str(),
            "key-6"
        );
    }

    #[test]
    fn toast_only_does_not_enter_history_and_open_marks_history_read() {
        let mut center = NotificationCenter::default();
        center.push(
            notification("transient", NotificationRetention::ToastOnly),
            Duration::ZERO,
        );
        center.push(
            notification("important", NotificationRetention::Important),
            Duration::ZERO,
        );

        assert_eq!(center.history_count(), 1);
        assert_eq!(center.unread_count(), 1);
        center.toggle_history();
        assert!(center.history_open());
        assert_eq!(center.unread_count(), 0);
        center.close_history();
        assert!(!center.history_open());
    }

    #[test]
    fn unchanged_expiry_check_does_not_dirty_center() {
        let mut center = NotificationCenter::default();
        center.push(
            notification("still-live", NotificationRetention::ToastOnly),
            Duration::ZERO,
        );
        let revision = center.revision();

        center.expire(Duration::from_secs(1));

        assert_eq!(center.revision(), revision);
    }

    #[test]
    fn foreground_capture_closes_history_and_hides_its_button() {
        let mut app = App::new();
        let mut center = NotificationCenter::default();
        center.toggle_history();
        app.add_plugins(MinimalPlugins)
            .init_resource::<UiTheme>()
            .insert_resource(center)
            .insert_resource(UiInputState {
                world_input_captured: true,
                world_input_capture_started: true,
                ..default()
            })
            .add_systems(Update, apply_notification_ui_state_system);
        let button = app
            .world_mut()
            .spawn((
                Interaction::None,
                BackgroundColor::default(),
                Node::default(),
                NotificationHistoryButton,
            ))
            .id();

        app.update();

        assert!(!app.world().resource::<NotificationCenter>().history_open());
        assert_eq!(
            app.world().get::<Node>(button).unwrap().display,
            Display::None
        );

        *app.world_mut().resource_mut::<UiInputState>() = UiInputState::default();
        app.update();
        assert_eq!(
            app.world().get::<Node>(button).unwrap().display,
            Display::Flex
        );
    }
}
