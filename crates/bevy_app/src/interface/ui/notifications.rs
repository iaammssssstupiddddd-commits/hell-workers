use bevy::prelude::*;
use hw_ui::notifications::{NotificationRetention, NotificationSeverity, UserFacingNotification};

use crate::systems::save::{
    SaveLoadFailureKind, SaveLoadOperation, SaveLoadOutcome, SaveLoadResult,
};
use hw_logistics::StockpilePolicyChangeOutcome;

pub(crate) fn adapt_save_load_outcomes(
    mut outcomes: MessageReader<SaveLoadOutcome>,
    mut notifications: MessageWriter<UserFacingNotification>,
) {
    for outcome in outcomes.read() {
        notifications.write(notification_from_outcome(outcome));
    }
}

pub(crate) fn adapt_stockpile_policy_change_outcomes(
    mut outcomes: MessageReader<StockpilePolicyChangeOutcome>,
    mut notifications: MessageWriter<UserFacingNotification>,
) {
    for outcome in outcomes.read().copied() {
        notifications.write(stockpile_policy_notification(outcome));
    }
}

fn stockpile_policy_notification(outcome: StockpilePolicyChangeOutcome) -> UserFacingNotification {
    let (severity, title) = if outcome.eligible() == 0 {
        (
            NotificationSeverity::Warning,
            "Stockpile policy not applied",
        )
    } else if outcome.has_adjustments_or_skips() {
        (
            NotificationSeverity::Warning,
            "Stockpile policy partially applied",
        )
    } else if outcome.applied == 0 {
        (NotificationSeverity::Info, "Stockpile policy unchanged")
    } else {
        (NotificationSeverity::Success, "Stockpile policy updated")
    };

    let mut details = vec![format!(
        "Changed {} managed cell(s); {} already matched.",
        outcome.applied, outcome.unchanged
    )];
    if outcome.skipped_stale > 0 {
        details.push(format!(
            "Skipped {} cell(s) that no longer exist.",
            outcome.skipped_stale
        ));
    }
    if outcome.skipped_unmanaged > 0 {
        details.push(format!(
            "Skipped {} unsupported or special storage target(s).",
            outcome.skipped_unmanaged
        ));
    }
    if outcome.target_clamped > 0 {
        details.push(format!(
            "Clamped target amount to capacity on {} cell(s).",
            outcome.target_clamped
        ));
    }
    if outcome.requested > outcome.unique {
        details.push(format!(
            "Ignored {} duplicate target(s).",
            outcome.requested - outcome.unique
        ));
    }

    UserFacingNotification::new(
        format!(
            "stockpile_policy:{}:{}:{}:{}:{}:{}:{}",
            outcome.applied,
            outcome.unchanged,
            outcome.skipped_stale,
            outcome.skipped_unmanaged,
            outcome.target_clamped,
            outcome.requested,
            outcome.unique,
        ),
        severity,
        title,
        details.join(" "),
        NotificationRetention::ToastOnly,
    )
}

fn notification_from_outcome(outcome: &SaveLoadOutcome) -> UserFacingNotification {
    let target = safe_target(&outcome.target);
    let (severity, title, body) = match outcome.result {
        SaveLoadResult::Succeeded => match outcome.operation {
            SaveLoadOperation::Save => (
                NotificationSeverity::Success,
                "Game saved",
                format!("Saved {target}."),
            ),
            SaveLoadOperation::Load => (
                NotificationSeverity::Success,
                "Game loaded",
                format!("Loaded {target}."),
            ),
        },
        SaveLoadResult::Failed(SaveLoadFailureKind::SaveSerialize) => (
            NotificationSeverity::Error,
            "Save failed",
            format!("Could not prepare save data for {target}."),
        ),
        SaveLoadResult::Failed(SaveLoadFailureKind::SaveWrite) => (
            NotificationSeverity::Error,
            "Save failed",
            format!("Could not write {target}."),
        ),
        SaveLoadResult::Failed(SaveLoadFailureKind::LoadNotFound) => (
            NotificationSeverity::Warning,
            "Save not found",
            format!("{target} does not exist."),
        ),
        SaveLoadResult::Failed(SaveLoadFailureKind::LoadRead) => (
            NotificationSeverity::Error,
            "Load failed",
            format!("Could not read {target}."),
        ),
        SaveLoadResult::Failed(SaveLoadFailureKind::UnsupportedFormat) => (
            NotificationSeverity::Error,
            "Unsupported save",
            format!("{target} uses an unsupported save format."),
        ),
        SaveLoadResult::Failed(SaveLoadFailureKind::InvalidData) => (
            NotificationSeverity::Error,
            "Invalid save data",
            format!("{target} is invalid or damaged."),
        ),
        SaveLoadResult::Failed(SaveLoadFailureKind::SeedMismatch) => (
            NotificationSeverity::Error,
            "World seed mismatch",
            format!("{target} belongs to a different generated world."),
        ),
        SaveLoadResult::Failed(SaveLoadFailureKind::MissingPrerequisite) => (
            NotificationSeverity::Error,
            "Load unavailable",
            format!("The current session cannot prepare {target} for loading."),
        ),
        SaveLoadResult::Failed(SaveLoadFailureKind::ApplyRecovered) => (
            NotificationSeverity::Warning,
            "Load failed; world restored",
            format!("Could not apply {target}. The previous world was restored."),
        ),
        SaveLoadResult::Failed(SaveLoadFailureKind::RecoveryFailed) => (
            NotificationSeverity::Error,
            "Load recovery failed",
            format!("Could not load {target}, and the previous world could not be restored."),
        ),
    };

    UserFacingNotification::new(
        format!(
            "save_load:{}:{}:{}",
            outcome.operation.key_part(),
            target,
            outcome.result.key_part()
        ),
        severity,
        title,
        body,
        NotificationRetention::Important,
    )
}

fn safe_target(target: &str) -> &str {
    if target.is_empty()
        || target.len() > 96
        || target.contains(['/', '\\'])
        || target.chars().any(char::is_control)
    {
        "Current save"
    } else {
        target
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAILURES: [SaveLoadFailureKind; 10] = [
        SaveLoadFailureKind::SaveSerialize,
        SaveLoadFailureKind::SaveWrite,
        SaveLoadFailureKind::LoadNotFound,
        SaveLoadFailureKind::LoadRead,
        SaveLoadFailureKind::UnsupportedFormat,
        SaveLoadFailureKind::InvalidData,
        SaveLoadFailureKind::SeedMismatch,
        SaveLoadFailureKind::MissingPrerequisite,
        SaveLoadFailureKind::ApplyRecovered,
        SaveLoadFailureKind::RecoveryFailed,
    ];

    #[test]
    fn every_terminal_result_maps_to_important_safe_ui_text() {
        for result in std::iter::once(SaveLoadResult::Succeeded)
            .chain(FAILURES.into_iter().map(SaveLoadResult::Failed))
        {
            let notification = notification_from_outcome(&SaveLoadOutcome {
                operation: SaveLoadOperation::Load,
                target: "/private/user/secret.ron\nraw error".to_owned(),
                result,
            });

            assert_eq!(notification.retention, NotificationRetention::Important);
            assert!(!notification.body.contains("/private"));
            assert!(!notification.body.contains("raw error"));
            assert!(notification.body.contains("Current save"));
        }
    }

    #[test]
    fn severity_and_dedupe_key_keep_distinct_terminal_meanings() {
        let success = notification_from_outcome(&SaveLoadOutcome {
            operation: SaveLoadOperation::Save,
            target: "world.scn.ron".to_owned(),
            result: SaveLoadResult::Succeeded,
        });
        let missing = notification_from_outcome(&SaveLoadOutcome {
            operation: SaveLoadOperation::Load,
            target: "world.scn.ron".to_owned(),
            result: SaveLoadResult::Failed(SaveLoadFailureKind::LoadNotFound),
        });
        let recovered = notification_from_outcome(&SaveLoadOutcome {
            operation: SaveLoadOperation::Load,
            target: "world.scn.ron".to_owned(),
            result: SaveLoadResult::Failed(SaveLoadFailureKind::ApplyRecovered),
        });

        assert_eq!(success.severity, NotificationSeverity::Success);
        assert_eq!(missing.severity, NotificationSeverity::Warning);
        assert_eq!(recovered.severity, NotificationSeverity::Warning);
        assert_ne!(success.key, missing.key);
        assert_ne!(missing.key, recovered.key);
    }

    #[test]
    fn stockpile_policy_outcomes_distinguish_success_partial_and_no_target() {
        let success = stockpile_policy_notification(StockpilePolicyChangeOutcome {
            requested: 2,
            unique: 2,
            applied: 2,
            unchanged: 0,
            skipped_stale: 0,
            skipped_unmanaged: 0,
            target_clamped: 0,
        });
        let partial = stockpile_policy_notification(StockpilePolicyChangeOutcome {
            requested: 3,
            unique: 3,
            applied: 1,
            unchanged: 0,
            skipped_stale: 1,
            skipped_unmanaged: 1,
            target_clamped: 1,
        });
        let none = stockpile_policy_notification(StockpilePolicyChangeOutcome {
            requested: 1,
            unique: 1,
            applied: 0,
            unchanged: 0,
            skipped_stale: 0,
            skipped_unmanaged: 1,
            target_clamped: 0,
        });

        assert_eq!(success.severity, NotificationSeverity::Success);
        assert_eq!(partial.severity, NotificationSeverity::Warning);
        assert_eq!(none.severity, NotificationSeverity::Warning);
        assert_eq!(success.retention, NotificationRetention::ToastOnly);
        assert!(partial.body.contains("unsupported or special storage"));
        assert!(partial.body.contains("Clamped target amount"));
    }
}
