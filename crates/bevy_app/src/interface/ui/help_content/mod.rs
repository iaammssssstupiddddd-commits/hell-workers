//! Root-owned, validated player Help catalog.

mod coverage;
mod manifest;
mod providers;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use hw_ui::help::{
    HelpPanelChrome, HelpPanelContent, HelpPanelCopy, HelpPanelCopySpec, HelpSection,
    HelpSectionId, HelpShortcutPair, HelpTopic,
};

use crate::input_actions::{InputAction, binding_labels_for_action};

use manifest::{FeatureSpec, HelpOwnerId, PlayerFeatureId};

pub(crate) type ProviderFn = fn() -> Result<HelpContribution, HelpCatalogError>;
type OrderedSectionTopics = BTreeMap<(u16, HelpSectionId), (&'static str, Vec<(u16, HelpTopic)>)>;

pub(crate) struct HelpContribution {
    feature: PlayerFeatureId,
    owner: HelpOwnerId,
    section_id: HelpSectionId,
    section_title: &'static str,
    topic: HelpTopic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HelpCatalogError(String);

impl HelpCatalogError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for HelpCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for HelpCatalogError {}

pub(crate) fn build_help_panel_content() -> Result<HelpPanelContent, HelpCatalogError> {
    build_from_specs(manifest::feature_specs())
}

pub(crate) fn build_help_panel_chrome() -> Result<HelpPanelChrome, HelpCatalogError> {
    let chrome = HelpPanelChrome::new(
        HelpPanelCopy::new(HelpPanelCopySpec {
            launcher_label: "Help",
            launcher_tooltip: "操作とゲームのヘルプ",
            panel_title: "Hell Workers ヘルプ",
            close_label: "閉じる",
            topic_navigation_label: "項目移動",
            page_navigation_label: "ページ移動",
            document_bounds_label: "先頭 / 末尾",
            shortcut_label: "操作",
        }),
        shortcut(InputAction::OpenHelp)?,
        shortcut(InputAction::CloseHelp)?,
        HelpShortcutPair::new(
            shortcut(InputAction::HelpPreviousTopic)?,
            shortcut(InputAction::HelpNextTopic)?,
        ),
        HelpShortcutPair::new(
            shortcut(InputAction::HelpPageUp)?,
            shortcut(InputAction::HelpPageDown)?,
        ),
        HelpShortcutPair::new(
            shortcut(InputAction::HelpHome)?,
            shortcut(InputAction::HelpEnd)?,
        ),
    );
    let copy = chrome.copy();
    if [
        copy.launcher_label(),
        copy.launcher_tooltip(),
        copy.panel_title(),
        copy.close_label(),
        copy.topic_navigation_label(),
        copy.page_navigation_label(),
        copy.document_bounds_label(),
        copy.shortcut_label(),
    ]
    .into_iter()
    .any(|text| text.trim().is_empty())
    {
        return Err(HelpCatalogError::new(
            "Help chrome copy must not contain empty text",
        ));
    }
    Ok(chrome)
}

fn build_from_specs(specs: Vec<FeatureSpec>) -> Result<HelpPanelContent, HelpCatalogError> {
    let mut contributions = Vec::with_capacity(specs.len());
    let mut seen_features = BTreeSet::new();

    for spec in specs {
        if !seen_features.insert(spec.feature) {
            return Err(HelpCatalogError::new(format!(
                "duplicate feature id: {}",
                spec.feature.as_str()
            )));
        }
        let contribution = (spec.provider)()?;
        if contribution.feature != spec.feature {
            return Err(HelpCatalogError::new(format!(
                "provider feature mismatch for {}",
                spec.feature.as_str()
            )));
        }
        if contribution.owner != spec.owner {
            return Err(HelpCatalogError::new(format!(
                "provider owner mismatch for {}",
                spec.feature.as_str()
            )));
        }
        contributions.push((spec, contribution));
    }

    let expected: BTreeSet<_> = PlayerFeatureId::ALL.iter().copied().collect();
    if seen_features != expected {
        return Err(HelpCatalogError::new(
            "manifest features and provider features differ",
        ));
    }

    contributions.sort_by_key(|(spec, _)| (spec.section_order, spec.topic_order, spec.feature));
    validate_contributions(&contributions)?;

    let mut section_topics = OrderedSectionTopics::new();
    for (spec, contribution) in contributions {
        let section = section_topics
            .entry((spec.section_order, contribution.section_id))
            .or_insert((contribution.section_title, Vec::new()));
        if section.0 != contribution.section_title {
            return Err(HelpCatalogError::new(format!(
                "section title mismatch: {}",
                contribution.section_id.as_str()
            )));
        }
        section.1.push((spec.topic_order, contribution.topic));
    }

    let sections = section_topics
        .into_iter()
        .map(|((_, section_id), (title, mut topics))| {
            topics.sort_by_key(|(order, topic)| (*order, topic.id()));
            HelpSection::new(
                section_id,
                title,
                topics.into_iter().map(|(_, topic)| topic),
            )
        })
        .collect::<Vec<_>>();
    let content = HelpPanelContent::new(sections);
    coverage::validate_surface_coverage(&content)?;
    Ok(content)
}

fn validate_contributions(
    contributions: &[(FeatureSpec, HelpContribution)],
) -> Result<(), HelpCatalogError> {
    let mut sections = BTreeMap::new();
    let mut section_orders = BTreeMap::new();
    let mut topic_orders = BTreeSet::new();
    let mut topics = BTreeSet::new();
    let mut entries = BTreeSet::new();

    for (spec, contribution) in contributions {
        if spec.feature.as_str().trim().is_empty()
            || spec.owner.as_str().trim().is_empty()
            || contribution.section_id.as_str().trim().is_empty()
            || contribution.section_title.trim().is_empty()
            || contribution.topic.id().as_str().trim().is_empty()
            || contribution.topic.title().trim().is_empty()
        {
            return Err(HelpCatalogError::new("catalog metadata must not be empty"));
        }
        if let Some((section_order, section_title)) = sections.insert(
            contribution.section_id,
            (spec.section_order, contribution.section_title),
        ) && (section_order != spec.section_order || section_title != contribution.section_title)
        {
            return Err(HelpCatalogError::new(format!(
                "section metadata mismatch: {}",
                contribution.section_id.as_str()
            )));
        }
        if let Some(existing_section) =
            section_orders.insert(spec.section_order, contribution.section_id)
            && existing_section != contribution.section_id
        {
            return Err(HelpCatalogError::new(format!(
                "duplicate section order: {}",
                spec.section_order
            )));
        }
        if !topic_orders.insert((contribution.section_id, spec.topic_order)) {
            return Err(HelpCatalogError::new(format!(
                "duplicate topic order in section {}: {}",
                contribution.section_id.as_str(),
                spec.topic_order
            )));
        }
        if !topics.insert(contribution.topic.id()) {
            return Err(HelpCatalogError::new(format!(
                "duplicate topic id: {}",
                contribution.topic.id().as_str()
            )));
        }
        if contribution.topic.entries().is_empty() {
            return Err(HelpCatalogError::new(format!(
                "topic has no entries: {}",
                contribution.topic.id().as_str()
            )));
        }
        for entry in contribution.topic.entries() {
            if entry.id().as_str().trim().is_empty()
                || entry.title().trim().is_empty()
                || entry.paragraphs().is_empty()
                || entry
                    .paragraphs()
                    .iter()
                    .any(|paragraph| paragraph.trim().is_empty())
                || entry
                    .shortcut()
                    .is_some_and(|shortcut| shortcut.trim().is_empty())
            {
                return Err(HelpCatalogError::new(format!(
                    "entry is incomplete: {}",
                    entry.id().as_str()
                )));
            }
            if !entries.insert(entry.id()) {
                return Err(HelpCatalogError::new(format!(
                    "duplicate entry id: {}",
                    entry.id().as_str()
                )));
            }
        }
    }
    Ok(())
}

pub(super) fn shortcut(action: InputAction) -> Result<String, HelpCatalogError> {
    let labels = binding_labels_for_action(action)
        .map_err(|error| HelpCatalogError::new(error.to_string()))?;
    if labels.is_empty() {
        return Err(HelpCatalogError::new(format!(
            "public action has no binding: {action:?}"
        )));
    }
    Ok(labels.join(" / "))
}
