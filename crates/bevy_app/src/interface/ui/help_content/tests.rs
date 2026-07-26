use super::*;

fn familiar_commands_with_blocked_entry() -> Result<HelpContribution, HelpCatalogError> {
    let mut contribution = providers::familiars::familiar_commands()?;
    let topic = &contribution.topic;
    let mut entries = topic.entries().to_vec();
    entries.push(hw_ui::help::HelpEntry::new(
        hw_ui::help::HelpEntryId::new("familiar-build"),
        "Build",
        ["blocked fixture"],
    ));
    contribution.topic = HelpTopic::new(topic.id(), topic.title(), entries);
    Ok(contribution)
}

fn normalized_approval_snapshot(content: &HelpPanelContent) -> String {
    let feature_by_topic: BTreeMap<hw_ui::help::HelpTopicId, (PlayerFeatureId, HelpOwnerId)> =
        manifest::feature_specs()
            .into_iter()
            .map(|spec| {
                let contribution = (spec.provider)().expect("production provider");
                (contribution.topic.id(), (spec.feature, spec.owner))
            })
            .collect();
    let mut lines = Vec::new();
    let chrome = build_help_panel_chrome().expect("production Help chrome");
    let copy = chrome.copy();

    lines.push(format!(
        "launcher|label={:?}|tooltip={:?}|shortcut={:?}",
        copy.launcher_label(),
        copy.launcher_tooltip(),
        chrome.launcher_shortcut()
    ));
    lines.push(format!(
        "chrome-copy|panel-title={:?}|close-label={:?}|topic-navigation-label={:?}|page-navigation-label={:?}|document-bounds-label={:?}|shortcut-label={:?}",
        copy.panel_title(),
        copy.close_label(),
        copy.topic_navigation_label(),
        copy.page_navigation_label(),
        copy.document_bounds_label(),
        copy.shortcut_label(),
    ));
    lines.push(format!(
        "chrome-render|close-button={:?}|footer={:?}|shortcut-sample={:?}",
        chrome.close_button_text(),
        chrome.footer_text(),
        chrome.entry_shortcut_text("<binding>"),
    ));
    for slot in hw_ui::help::HelpChromeSlot::ALL {
        lines.push(format!(
            "chrome|slot={:?}|shortcut={:?}",
            slot.as_str(),
            chrome.shortcut(slot)
        ));
    }

    for section in content.sections() {
        lines.push(format!(
            "section|id={:?}|title={:?}",
            section.id().as_str(),
            section.title()
        ));
        for topic in section.topics() {
            let (feature, owner) = feature_by_topic[&topic.id()];
            lines.push(format!(
                "topic|feature={:?}|owner={:?}|section={:?}|id={:?}|title={:?}",
                feature.as_str(),
                owner.as_str(),
                section.id().as_str(),
                topic.id().as_str(),
                topic.title()
            ));
            for entry in topic.entries() {
                lines.push(format!(
                    "entry|topic={:?}|id={:?}|title={:?}|paragraphs={:?}|shortcut={:?}",
                    topic.id().as_str(),
                    entry.id().as_str(),
                    entry.title(),
                    entry.paragraphs(),
                    entry.shortcut()
                ));
            }
        }
    }

    lines.extend(coverage::normalized_approval_manifest());
    format!("{}\n", lines.join("\n"))
}

#[test]
fn production_catalog_is_valid_and_provider_order_independent() {
    let forward = build_help_panel_content().expect("production Help catalog must be valid");
    let mut reversed_specs = manifest::feature_specs();
    reversed_specs.reverse();
    let reversed = build_from_specs(reversed_specs).expect("reordered providers must validate");

    assert_eq!(
        normalized_approval_snapshot(&forward),
        normalized_approval_snapshot(&reversed)
    );
    assert_eq!(forward.topic_ids().count(), PlayerFeatureId::ALL.len());
}

#[test]
fn exact_snapshot_approves_all_player_visible_help_copy_and_coverage() {
    let content = build_help_panel_content().expect("catalog");
    let actual = normalized_approval_snapshot(&content);
    assert_eq!(actual, include_str!("coverage_approval.snap"));
}

#[test]
#[ignore = "explicitly regenerates the reviewed Help approval snapshot"]
fn regenerate_help_approval_snapshot() {
    let content = build_help_panel_content().expect("catalog");
    let actual = normalized_approval_snapshot(&content);
    let snapshot_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/interface/ui/help_content/coverage_approval.snap");
    std::fs::write(&snapshot_path, &actual).unwrap_or_else(|error| {
        panic!(
            "failed to update Help approval snapshot {}: {error}",
            snapshot_path.display()
        )
    });
}

#[test]
fn help_shortcut_is_generated_from_the_canonical_binding() {
    assert_eq!(shortcut(InputAction::OpenHelp).as_deref(), Ok("F1"));
}

#[test]
fn coverage_rejects_blocked_entries() {
    let mut blocked_specs = manifest::feature_specs();
    blocked_specs
        .iter_mut()
        .find(|spec| spec.feature == PlayerFeatureId::FamiliarCommands)
        .expect("Familiar command feature")
        .provider = familiar_commands_with_blocked_entry;
    let blocked_error = build_from_specs(blocked_specs).unwrap_err().to_string();
    assert!(blocked_error.contains("blocked surface is present"));
}

#[test]
fn validator_rejects_ambiguous_section_and_topic_ordering() {
    let mut split_section = manifest::feature_specs();
    split_section[2].section_order += 1;
    assert!(
        build_from_specs(split_section)
            .unwrap_err()
            .to_string()
            .contains("section metadata mismatch")
    );

    let mut duplicate_topic_order = manifest::feature_specs();
    duplicate_topic_order[2].topic_order = duplicate_topic_order[1].topic_order;
    assert!(
        build_from_specs(duplicate_topic_order)
            .unwrap_err()
            .to_string()
            .contains("duplicate topic order")
    );
}
