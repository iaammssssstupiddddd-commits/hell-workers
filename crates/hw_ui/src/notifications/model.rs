use bevy::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Duration;

pub const NOTIFICATION_DEDUPE_WINDOW: Duration = Duration::from_secs(2);
pub const NOTIFICATION_TOAST_LIFETIME: Duration = Duration::from_secs(4);
pub const MAX_ACTIVE_TOASTS: usize = 1;
pub const MAX_NOTIFICATION_HISTORY: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NotificationKey(String);

impl NotificationKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for NotificationKey {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for NotificationKey {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationSeverity {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationRetention {
    ToastOnly,
    Important,
}

impl NotificationRetention {
    pub(super) fn merge(self, incoming: Self) -> Self {
        if self == Self::Important || incoming == Self::Important {
            Self::Important
        } else {
            Self::ToastOnly
        }
    }
}

/// Explicit actions are rendered only inside notification history.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationAction {
    RetrySettingsSave { attempt: u64 },
}

#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub struct UserFacingNotification {
    pub key: NotificationKey,
    pub severity: NotificationSeverity,
    pub title: String,
    pub body: String,
    pub retention: NotificationRetention,
    pub action: Option<NotificationAction>,
}

impl UserFacingNotification {
    pub fn new(
        key: impl Into<NotificationKey>,
        severity: NotificationSeverity,
        title: impl Into<String>,
        body: impl Into<String>,
        retention: NotificationRetention,
    ) -> Self {
        Self {
            key: key.into(),
            severity,
            title: title.into(),
            body: body.into(),
            retention,
            action: None,
        }
    }
    pub fn with_action(mut self, action: NotificationAction) -> Self {
        self.action = Some(action);
        self
    }
}

pub type NotificationEntryId = u64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotificationEntry {
    pub id: NotificationEntryId,
    pub key: NotificationKey,
    pub severity: NotificationSeverity,
    pub title: String,
    pub body: String,
    pub retention: NotificationRetention,
    pub action: Option<NotificationAction>,
    pub first_seen: Duration,
    pub last_seen: Duration,
    pub repeat_count: u32,
    pub(super) expires_at: Duration,
}

#[derive(Resource, Debug)]
pub struct NotificationCenter {
    pub(super) entries: HashMap<NotificationEntryId, NotificationEntry>,
    pub(super) toasts: VecDeque<NotificationEntryId>,
    pub(super) history: VecDeque<NotificationEntryId>,
    pub(super) history_open: bool,
    pub(super) unread: HashSet<NotificationEntryId>,
    pub(super) next_id: NotificationEntryId,
    pub(super) revision: u64,
    pub(super) toast_lifetime: Duration,
}

impl NotificationCenter {
    pub fn toast_lifetime(&self) -> Duration {
        self.toast_lifetime
    }

    pub fn set_toast_lifetime(&mut self, lifetime: Duration) {
        self.toast_lifetime = lifetime;
    }

    pub(super) fn entry(&self, id: NotificationEntryId) -> Option<&NotificationEntry> {
        self.entries.get(&id)
    }

    pub fn retain_settings_retry_attempt(&mut self, current_attempt: Option<u64>) {
        let mut changed = false;
        for entry in self.entries.values_mut() {
            if matches!(entry.action, Some(NotificationAction::RetrySettingsSave { attempt }) if Some(attempt) != current_attempt)
            {
                entry.action = None;
                changed = true;
            }
        }
        if changed {
            self.revision = self.revision.wrapping_add(1);
        }
    }
}

impl Default for NotificationCenter {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            toasts: VecDeque::new(),
            history: VecDeque::new(),
            history_open: false,
            unread: HashSet::new(),
            next_id: 1,
            revision: 0,
            toast_lifetime: NOTIFICATION_TOAST_LIFETIME,
        }
    }
}

impl NotificationCenter {
    pub fn toast_entries(&self) -> impl DoubleEndedIterator<Item = &NotificationEntry> {
        self.toasts.iter().filter_map(|id| self.entries.get(id))
    }

    pub fn history_entries(&self) -> impl DoubleEndedIterator<Item = &NotificationEntry> {
        self.history.iter().filter_map(|id| self.entries.get(id))
    }

    pub fn toast_count(&self) -> usize {
        self.toasts.len()
    }

    pub fn history_count(&self) -> usize {
        self.history.len()
    }

    pub fn history_open(&self) -> bool {
        self.history_open
    }

    pub fn unread_count(&self) -> usize {
        self.unread.len()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }
}

#[derive(Resource, Default, Debug)]
pub struct NotificationUiRuntime {
    pub(super) rendered_revision: Option<u64>,
    pub(super) age_second: Option<u64>,
    pub(super) history_open: bool,
    pub(super) scroll_anchor: Option<(NotificationEntryId, f32)>,
}

#[derive(Resource, Clone)]
pub struct NotificationUiAssets {
    pub font: Handle<Font>,
}

#[derive(Component)]
pub struct NotificationToastSurface;

#[derive(Component)]
pub struct NotificationToastRoot;

#[derive(Component)]
pub struct NotificationToastRow;

#[derive(Component)]
pub struct NotificationHistoryRow;

#[derive(Component)]
pub struct NotificationHistoryButton;

#[derive(Component)]
pub struct NotificationHistoryPanel;

#[derive(Component)]
pub struct NotificationHistoryScroll;

#[derive(Component)]
pub struct NotificationHistoryKey(pub NotificationEntryId);

#[derive(Component)]
pub struct NotificationHistoryClose;

#[derive(Component)]
pub struct NotificationUnreadText;
