//! Player Help display model and UI-owned state.
//!
//! Stable identifier values and game-specific content are supplied by the root
//! application. This crate owns only the sealed presentation schema.

use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HelpSectionId(&'static str);

impl HelpSectionId {
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HelpTopicId(&'static str);

impl HelpTopicId {
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HelpEntryId(&'static str);

impl HelpEntryId {
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelpEntry {
    id: HelpEntryId,
    title: String,
    paragraphs: Vec<String>,
    shortcut: Option<String>,
}

impl HelpEntry {
    pub fn new(
        id: HelpEntryId,
        title: impl Into<String>,
        paragraphs: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            paragraphs: paragraphs.into_iter().map(Into::into).collect(),
            shortcut: None,
        }
    }

    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    pub const fn id(&self) -> HelpEntryId {
        self.id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn paragraphs(&self) -> &[String] {
        &self.paragraphs
    }

    pub fn shortcut(&self) -> Option<&str> {
        self.shortcut.as_deref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelpTopic {
    id: HelpTopicId,
    title: String,
    entries: Vec<HelpEntry>,
}

impl HelpTopic {
    pub fn new(
        id: HelpTopicId,
        title: impl Into<String>,
        entries: impl IntoIterator<Item = HelpEntry>,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            entries: entries.into_iter().collect(),
        }
    }

    pub const fn id(&self) -> HelpTopicId {
        self.id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn entries(&self) -> &[HelpEntry] {
        &self.entries
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelpSection {
    id: HelpSectionId,
    title: String,
    topics: Vec<HelpTopic>,
}

impl HelpSection {
    pub fn new(
        id: HelpSectionId,
        title: impl Into<String>,
        topics: impl IntoIterator<Item = HelpTopic>,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            topics: topics.into_iter().collect(),
        }
    }

    pub const fn id(&self) -> HelpSectionId {
        self.id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn topics(&self) -> &[HelpTopic] {
        &self.topics
    }
}

#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct HelpPanelContent {
    sections: Vec<HelpSection>,
}

impl HelpPanelContent {
    pub fn new(sections: impl IntoIterator<Item = HelpSection>) -> Self {
        Self {
            sections: sections.into_iter().collect(),
        }
    }

    pub fn sections(&self) -> &[HelpSection] {
        &self.sections
    }

    pub fn topics(&self) -> impl Iterator<Item = &HelpTopic> {
        self.sections.iter().flat_map(HelpSection::topics)
    }

    pub fn topic_ids(&self) -> impl Iterator<Item = HelpTopicId> + '_ {
        self.topics().map(HelpTopic::id)
    }

    pub fn first_topic_id(&self) -> Option<HelpTopicId> {
        self.topic_ids().next()
    }

    pub fn contains_topic(&self, topic: HelpTopicId) -> bool {
        self.topic_ids().any(|candidate| candidate == topic)
    }

    pub fn adjacent_topic(&self, current: HelpTopicId, step: HelpTopicStep) -> Option<HelpTopicId> {
        let topics: Vec<_> = self.topic_ids().collect();
        let index = topics.iter().position(|candidate| *candidate == current)?;
        let next = match step {
            HelpTopicStep::Previous => index.saturating_sub(1),
            HelpTopicStep::Next => (index + 1).min(topics.len().saturating_sub(1)),
        };
        topics.get(next).copied()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelpShortcutPair {
    first: String,
    second: String,
}

impl HelpShortcutPair {
    pub fn new(first: impl Into<String>, second: impl Into<String>) -> Self {
        Self {
            first: first.into(),
            second: second.into(),
        }
    }

    pub fn first(&self) -> &str {
        &self.first
    }

    pub fn second(&self) -> &str {
        &self.second
    }
}

/// Fixed shortcut-bearing controls that physically exist in the Help overlay.
///
/// Keeping this set next to [`HelpPanelChrome`] lets the root application map
/// input actions to concrete UI targets without an untyped "chrome" escape
/// hatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HelpChromeSlot {
    Close,
    TopicPrevious,
    TopicNext,
    PageUp,
    PageDown,
    DocumentStart,
    DocumentEnd,
}

impl HelpChromeSlot {
    pub const ALL: [Self; 7] = [
        Self::Close,
        Self::TopicPrevious,
        Self::TopicNext,
        Self::PageUp,
        Self::PageDown,
        Self::DocumentStart,
        Self::DocumentEnd,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Close => "close",
            Self::TopicPrevious => "topic-previous",
            Self::TopicNext => "topic-next",
            Self::PageUp => "page-up",
            Self::PageDown => "page-down",
            Self::DocumentStart => "document-start",
            Self::DocumentEnd => "document-end",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelpPanelCopySpec {
    pub launcher_label: &'static str,
    pub launcher_tooltip: &'static str,
    pub panel_title: &'static str,
    pub close_label: &'static str,
    pub topic_navigation_label: &'static str,
    pub page_navigation_label: &'static str,
    pub document_bounds_label: &'static str,
    pub shortcut_label: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelpPanelCopy {
    launcher_label: String,
    launcher_tooltip: String,
    panel_title: String,
    close_label: String,
    topic_navigation_label: String,
    page_navigation_label: String,
    document_bounds_label: String,
    shortcut_label: String,
}

impl HelpPanelCopy {
    pub fn new(spec: HelpPanelCopySpec) -> Self {
        Self {
            launcher_label: spec.launcher_label.to_string(),
            launcher_tooltip: spec.launcher_tooltip.to_string(),
            panel_title: spec.panel_title.to_string(),
            close_label: spec.close_label.to_string(),
            topic_navigation_label: spec.topic_navigation_label.to_string(),
            page_navigation_label: spec.page_navigation_label.to_string(),
            document_bounds_label: spec.document_bounds_label.to_string(),
            shortcut_label: spec.shortcut_label.to_string(),
        }
    }

    pub fn launcher_label(&self) -> &str {
        &self.launcher_label
    }

    pub fn launcher_tooltip(&self) -> &str {
        &self.launcher_tooltip
    }

    pub fn panel_title(&self) -> &str {
        &self.panel_title
    }

    pub fn close_label(&self) -> &str {
        &self.close_label
    }

    pub fn topic_navigation_label(&self) -> &str {
        &self.topic_navigation_label
    }

    pub fn page_navigation_label(&self) -> &str {
        &self.page_navigation_label
    }

    pub fn document_bounds_label(&self) -> &str {
        &self.document_bounds_label
    }

    pub fn shortcut_label(&self) -> &str {
        &self.shortcut_label
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelpPanelChrome {
    copy: HelpPanelCopy,
    launcher_shortcut: String,
    close_shortcuts: String,
    topic_navigation: HelpShortcutPair,
    page_navigation: HelpShortcutPair,
    document_bounds: HelpShortcutPair,
}

impl HelpPanelChrome {
    pub fn new(
        copy: HelpPanelCopy,
        launcher_shortcut: impl Into<String>,
        close_shortcuts: impl Into<String>,
        topic_navigation: HelpShortcutPair,
        page_navigation: HelpShortcutPair,
        document_bounds: HelpShortcutPair,
    ) -> Self {
        Self {
            copy,
            launcher_shortcut: launcher_shortcut.into(),
            close_shortcuts: close_shortcuts.into(),
            topic_navigation,
            page_navigation,
            document_bounds,
        }
    }

    pub fn copy(&self) -> &HelpPanelCopy {
        &self.copy
    }

    pub fn launcher_shortcut(&self) -> &str {
        &self.launcher_shortcut
    }

    pub fn close_shortcuts(&self) -> &str {
        &self.close_shortcuts
    }

    pub fn topic_navigation(&self) -> &HelpShortcutPair {
        &self.topic_navigation
    }

    pub fn page_navigation(&self) -> &HelpShortcutPair {
        &self.page_navigation
    }

    pub fn document_bounds(&self) -> &HelpShortcutPair {
        &self.document_bounds
    }

    pub fn shortcut(&self, slot: HelpChromeSlot) -> &str {
        match slot {
            HelpChromeSlot::Close => self.close_shortcuts(),
            HelpChromeSlot::TopicPrevious => self.topic_navigation().first(),
            HelpChromeSlot::TopicNext => self.topic_navigation().second(),
            HelpChromeSlot::PageUp => self.page_navigation().first(),
            HelpChromeSlot::PageDown => self.page_navigation().second(),
            HelpChromeSlot::DocumentStart => self.document_bounds().first(),
            HelpChromeSlot::DocumentEnd => self.document_bounds().second(),
        }
    }

    pub fn close_button_text(&self) -> String {
        format!("{} ({})", self.copy.close_label(), self.close_shortcuts())
    }

    pub fn footer_text(&self) -> String {
        format!(
            "{}: {}  {} / {}: {}  {} / {}: {}  {} / {}: {}",
            self.close_shortcuts(),
            self.copy.close_label(),
            self.topic_navigation().first(),
            self.topic_navigation().second(),
            self.copy.topic_navigation_label(),
            self.page_navigation().first(),
            self.page_navigation().second(),
            self.copy.page_navigation_label(),
            self.document_bounds().first(),
            self.document_bounds().second(),
            self.copy.document_bounds_label(),
        )
    }

    pub fn entry_shortcut_text(&self, shortcut: &str) -> String {
        format!("{}: {shortcut}", self.copy.shortcut_label())
    }
}

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HelpPanelState {
    pub open: bool,
    pub active_topic: Option<HelpTopicId>,
}

impl HelpPanelState {
    pub fn open_at(&mut self, topic: HelpTopicId) {
        self.open = true;
        self.active_topic = Some(topic);
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn select_topic(&mut self, topic: HelpTopicId) {
        self.active_topic = Some(topic);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelpTopicStep {
    Previous,
    Next,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelpScrollCommand {
    PageUp,
    PageDown,
    Start,
    End,
}

#[derive(Component, Default)]
pub struct HelpPanel;

#[derive(Component, Clone, Copy)]
pub struct HelpTopicButton(pub HelpTopicId);

#[derive(Component, Clone, Copy)]
pub struct HelpTopicBody(pub HelpTopicId);

#[derive(Component, Default)]
pub struct HelpScrollArea;

#[derive(Component, Default)]
pub struct HelpNavigationScrollArea;

#[cfg(test)]
mod tests {
    use super::*;

    const FIRST: HelpTopicId = HelpTopicId::new("first");
    const SECOND: HelpTopicId = HelpTopicId::new("second");

    fn content() -> HelpPanelContent {
        HelpPanelContent::new([HelpSection::new(
            HelpSectionId::new("section"),
            "Section",
            [
                HelpTopic::new(FIRST, "First", []),
                HelpTopic::new(SECOND, "Second", []),
            ],
        )])
    }

    #[test]
    fn topic_navigation_is_manifest_ordered_and_clamped() {
        let content = content();
        assert_eq!(
            content.adjacent_topic(FIRST, HelpTopicStep::Previous),
            Some(FIRST)
        );
        assert_eq!(
            content.adjacent_topic(FIRST, HelpTopicStep::Next),
            Some(SECOND)
        );
        assert_eq!(
            content.adjacent_topic(SECOND, HelpTopicStep::Next),
            Some(SECOND)
        );
    }

    #[test]
    fn opening_and_topic_selection_update_the_active_topic() {
        let mut state = HelpPanelState::default();
        state.open_at(FIRST);
        assert!(state.open);
        state.select_topic(SECOND);
        assert_eq!(state.active_topic, Some(SECOND));
    }
}
