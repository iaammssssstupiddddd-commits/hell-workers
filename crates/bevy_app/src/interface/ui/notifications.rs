use bevy::prelude::*;
use hw_ui::notifications::{NotificationRetention, NotificationSeverity, UserFacingNotification};

use crate::systems::save::{
    SaveLoadFailureKind, SaveLoadOperation, SaveLoadOutcome, SaveLoadResult,
};
use hw_energy::{
    PowerConsumerPolicyChangeOutcome, PowerConsumerPolicyChangeStatus,
    SoulSpaConstructionCancelOutcome, SoulSpaConstructionCancelResult, SoulSpaSlotsChangeOutcome,
    SoulSpaSlotsChangeStatus,
};
use hw_familiar_ai::{
    FamiliarSettingsChangeOutcome, FamiliarSettingsChangeStatus, FamiliarSettingsRejection,
};
use hw_jobs::{
    DeconstructionCancelOutcome, DeconstructionCancelResult, DeconstructionCommitOutcome,
    DeconstructionCommitResult, DeconstructionDesignationOutcome,
    DeconstructionDesignationRejectReason, DeconstructionDesignationResult,
    DeconstructionRejectReason,
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

pub(crate) fn adapt_familiar_settings_change_outcomes(
    mut outcomes: MessageReader<FamiliarSettingsChangeOutcome>,
    mut notifications: MessageWriter<UserFacingNotification>,
) {
    for outcome in outcomes.read().copied() {
        if let Some(notification) = familiar_settings_notification(outcome) {
            notifications.write(notification);
        }
    }
}

pub(crate) fn adapt_soul_spa_slots_change_outcomes(
    mut outcomes: MessageReader<SoulSpaSlotsChangeOutcome>,
    mut notifications: MessageWriter<UserFacingNotification>,
) {
    for outcome in outcomes.read().copied() {
        notifications.write(soul_spa_slots_notification(outcome));
    }
}

pub(crate) fn adapt_soul_spa_construction_cancel_outcomes(
    mut outcomes: MessageReader<SoulSpaConstructionCancelOutcome>,
    mut notifications: MessageWriter<UserFacingNotification>,
) {
    for outcome in outcomes.read().copied() {
        notifications.write(soul_spa_construction_cancel_notification(outcome));
    }
}

pub(crate) fn adapt_power_consumer_policy_change_outcomes(
    mut outcomes: MessageReader<PowerConsumerPolicyChangeOutcome>,
    mut notifications: MessageWriter<UserFacingNotification>,
) {
    for outcome in outcomes.read().copied() {
        notifications.write(power_consumer_policy_notification(outcome));
    }
}

pub(crate) fn adapt_deconstruction_designation_outcomes(
    mut outcomes: MessageReader<DeconstructionDesignationOutcome>,
    mut notifications: MessageWriter<UserFacingNotification>,
) {
    for outcome in outcomes.read().copied() {
        notifications.write(deconstruction_designation_notification(outcome));
    }
}

pub(crate) fn adapt_deconstruction_cancel_outcomes(
    mut outcomes: MessageReader<DeconstructionCancelOutcome>,
    mut notifications: MessageWriter<UserFacingNotification>,
) {
    for outcome in outcomes.read().copied() {
        notifications.write(deconstruction_cancel_notification(outcome));
    }
}

pub(crate) fn adapt_deconstruction_commit_outcomes(
    mut outcomes: MessageReader<DeconstructionCommitOutcome>,
    mut notifications: MessageWriter<UserFacingNotification>,
) {
    for outcome in outcomes.read().copied() {
        if let Some(notification) = deconstruction_commit_notification(outcome) {
            notifications.write(notification);
        }
    }
}

fn deconstruction_designation_notification(
    outcome: DeconstructionDesignationOutcome,
) -> UserFacingNotification {
    let (severity, title, body, result_key) = match outcome.result {
        DeconstructionDesignationResult::Designated { class, .. } => (
            NotificationSeverity::Success,
            "Deconstruction designated",
            format!(
                "A {:?} deconstruction order was created.",
                class.building_type()
            ),
            "designated",
        ),
        DeconstructionDesignationResult::Rejected(reason) => (
            NotificationSeverity::Warning,
            "Deconstruction not designated",
            deconstruction_designation_reject_label(reason).to_string(),
            deconstruction_designation_reject_key(reason),
        ),
    };
    UserFacingNotification::new(
        format!(
            "deconstruction-designation:{}:{}:{}",
            outcome.request_id,
            outcome.hit.map_or(0, Entity::to_bits),
            result_key,
        ),
        severity,
        title,
        body,
        NotificationRetention::ToastOnly,
    )
}

fn deconstruction_cancel_notification(
    outcome: DeconstructionCancelOutcome,
) -> UserFacingNotification {
    let (severity, title, body, result_key) = match outcome.result {
        DeconstructionCancelResult::Canceled => (
            NotificationSeverity::Success,
            "Deconstruction canceled",
            "The order and any assigned work were cleaned up safely.",
            "canceled",
        ),
        DeconstructionCancelResult::ClaimInProgress => (
            NotificationSeverity::Warning,
            "Deconstruction already finishing",
            "The building commit has already started and cannot be canceled.",
            "claim-in-progress",
        ),
        DeconstructionCancelResult::StaleWorld => (
            NotificationSeverity::Warning,
            "Deconstruction changed",
            "The world changed before the cancel request was applied.",
            "stale-world",
        ),
        DeconstructionCancelResult::StaleOrder => (
            NotificationSeverity::Warning,
            "Deconstruction changed",
            "The selected order is no longer available.",
            "stale-order",
        ),
    };
    UserFacingNotification::new(
        format!(
            "deconstruction-cancel:{}:{result_key}",
            outcome.order.to_bits()
        ),
        severity,
        title,
        body,
        NotificationRetention::ToastOnly,
    )
}

fn deconstruction_commit_notification(
    outcome: DeconstructionCommitOutcome,
) -> Option<UserFacingNotification> {
    let (severity, title, body, result_key) = match outcome.result {
        DeconstructionCommitResult::Committed => (
            NotificationSeverity::Success,
            "Deconstruction complete",
            "The building was removed and its recoverable contents were preserved.",
            "committed",
        ),
        DeconstructionCommitResult::OwnerMismatch => (
            NotificationSeverity::Error,
            "Deconstruction blocked",
            "The building owner state is inconsistent. The target was left unchanged.",
            "owner-mismatch",
        ),
        DeconstructionCommitResult::NoSafeRecovery => (
            NotificationSeverity::Warning,
            "Deconstruction blocked",
            "No safe recovery destination is available. The target was left unchanged.",
            "no-safe-recovery",
        ),
        DeconstructionCommitResult::InconsistentMixerInventory => (
            NotificationSeverity::Error,
            "Deconstruction blocked",
            "Mixer inventory is inconsistent. The target was left unchanged.",
            "mixer-inconsistent",
        ),
        DeconstructionCommitResult::Moving => (
            NotificationSeverity::Warning,
            "Deconstruction blocked",
            "The building is moving. The order remains available for retry.",
            "moving",
        ),
        DeconstructionCommitResult::UnsupportedTarget => (
            NotificationSeverity::Warning,
            "Deconstruction blocked",
            "Safe cleanup is unavailable for this target.",
            "unsupported",
        ),
        DeconstructionCommitResult::Canceled
        | DeconstructionCommitResult::Duplicate
        | DeconstructionCommitResult::StaleWorld
        | DeconstructionCommitResult::StaleIdentity
        | DeconstructionCommitResult::StaleTarget => return None,
    };
    Some(UserFacingNotification::new(
        format!(
            "deconstruction-commit:{}:{}:{result_key}",
            outcome.order.to_bits(),
            outcome.target.to_bits(),
        ),
        severity,
        title,
        body,
        NotificationRetention::ToastOnly,
    ))
}

const fn deconstruction_designation_reject_label(
    reason: DeconstructionDesignationRejectReason,
) -> &'static str {
    match reason {
        DeconstructionDesignationRejectReason::StaleWorld => {
            "The world changed; select the building again."
        }
        DeconstructionDesignationRejectReason::NoTarget => "No completed building was selected.",
        DeconstructionDesignationRejectReason::CleanupUnavailable => {
            "Safe cleanup is not available for that building."
        }
        DeconstructionDesignationRejectReason::Target(reason) => match reason {
            DeconstructionRejectReason::StaleTarget => "The selected building no longer exists.",
            DeconstructionRejectReason::UnsupportedTarget => {
                "The selected entity is not a supported completed building."
            }
            DeconstructionRejectReason::ConstructionInProgress => {
                "Construction is still in progress; use its cancel action instead."
            }
            DeconstructionRejectReason::Moving => "Wait until the building stops moving.",
            DeconstructionRejectReason::AlreadyDesignated => {
                "That building already has a deconstruction order."
            }
            DeconstructionRejectReason::OwnerMismatch => {
                "The selected building has an invalid owner state."
            }
        },
    }
}

const fn deconstruction_designation_reject_key(
    reason: DeconstructionDesignationRejectReason,
) -> &'static str {
    match reason {
        DeconstructionDesignationRejectReason::StaleWorld => "stale-world",
        DeconstructionDesignationRejectReason::NoTarget => "no-target",
        DeconstructionDesignationRejectReason::CleanupUnavailable => "cleanup-unavailable",
        DeconstructionDesignationRejectReason::Target(reason) => match reason {
            DeconstructionRejectReason::StaleTarget => "stale-target",
            DeconstructionRejectReason::UnsupportedTarget => "unsupported-target",
            DeconstructionRejectReason::ConstructionInProgress => "construction-in-progress",
            DeconstructionRejectReason::Moving => "moving",
            DeconstructionRejectReason::AlreadyDesignated => "already-designated",
            DeconstructionRejectReason::OwnerMismatch => "owner-mismatch",
        },
    }
}

fn power_consumer_policy_notification(
    outcome: PowerConsumerPolicyChangeOutcome,
) -> UserFacingNotification {
    let target = outcome.target.to_bits();
    let (severity, title, body, status_key) = match outcome.status {
        PowerConsumerPolicyChangeStatus::Applied { previous, applied } if previous == applied => (
            NotificationSeverity::Info,
            "Power priority unchanged",
            format!("Priority remains {applied:?}."),
            format!("unchanged:{applied:?}"),
        ),
        PowerConsumerPolicyChangeStatus::Applied { applied, .. } => (
            NotificationSeverity::Success,
            "Power priority updated",
            format!("Priority set to {applied:?}."),
            format!("applied:{applied:?}"),
        ),
        PowerConsumerPolicyChangeStatus::StaleTarget => (
            NotificationSeverity::Warning,
            "Power priority not changed",
            "The selected consumer no longer exists.".to_string(),
            "stale".to_string(),
        ),
        PowerConsumerPolicyChangeStatus::UnsupportedTarget => (
            NotificationSeverity::Warning,
            "Power priority not changed",
            "The selected entity does not consume power.".to_string(),
            "unsupported".to_string(),
        ),
        PowerConsumerPolicyChangeStatus::MissingPolicy => (
            NotificationSeverity::Error,
            "Power priority unavailable",
            "The consumer is missing its durable power policy.".to_string(),
            "missing-policy".to_string(),
        ),
    };

    UserFacingNotification::new(
        format!("power-consumer-priority:{target}:{status_key}"),
        severity,
        title,
        body,
        NotificationRetention::ToastOnly,
    )
}

fn soul_spa_slots_notification(outcome: SoulSpaSlotsChangeOutcome) -> UserFacingNotification {
    let target = outcome.target.to_bits();
    let (severity, title, body, status_key) = match outcome.status {
        SoulSpaSlotsChangeStatus::Applied {
            requested,
            applied,
            clamped: false,
        } => (
            NotificationSeverity::Success,
            "Soul Spa slots updated",
            format!("Active slots set to {applied}."),
            format!("applied:{requested}:{applied}"),
        ),
        SoulSpaSlotsChangeStatus::Applied {
            requested,
            applied,
            clamped: true,
        } => (
            NotificationSeverity::Warning,
            "Soul Spa slots adjusted",
            format!("Requested {requested}; active slots were clamped to {applied}."),
            format!("clamped:{requested}:{applied}"),
        ),
        SoulSpaSlotsChangeStatus::StaleTarget => (
            NotificationSeverity::Warning,
            "Soul Spa slots not changed",
            "The selected Soul Spa no longer exists.".to_string(),
            "stale".to_string(),
        ),
        SoulSpaSlotsChangeStatus::UnsupportedTarget => (
            NotificationSeverity::Warning,
            "Soul Spa slots not changed",
            "The selected entity is not a Soul Spa.".to_string(),
            "unsupported".to_string(),
        ),
        SoulSpaSlotsChangeStatus::PhaseUnavailable => (
            NotificationSeverity::Warning,
            "Soul Spa slots unavailable",
            "Active slots can be changed after construction is complete.".to_string(),
            "phase".to_string(),
        ),
    };

    UserFacingNotification::new(
        format!("soul-spa-slots:{target}:{status_key}"),
        severity,
        title,
        body,
        NotificationRetention::ToastOnly,
    )
}

fn soul_spa_construction_cancel_notification(
    outcome: SoulSpaConstructionCancelOutcome,
) -> UserFacingNotification {
    let target = outcome.target.to_bits();
    let (severity, title, body, result_key) = match outcome.result {
        SoulSpaConstructionCancelResult::Canceled { refunded_bones } => (
            NotificationSeverity::Success,
            "Soul Spa construction canceled",
            format!("Refunded {refunded_bones} delivered Bone."),
            format!("canceled:{refunded_bones}"),
        ),
        SoulSpaConstructionCancelResult::Paused => (
            NotificationSeverity::Warning,
            "Soul Spa cancellation paused",
            "Resume the simulation before canceling construction.".to_string(),
            "paused".to_string(),
        ),
        SoulSpaConstructionCancelResult::StaleTarget => (
            NotificationSeverity::Warning,
            "Soul Spa cancellation expired",
            "The selected Soul Spa no longer exists.".to_string(),
            "stale".to_string(),
        ),
        SoulSpaConstructionCancelResult::PhaseUnavailable => (
            NotificationSeverity::Warning,
            "Soul Spa construction not canceled",
            "Only a Soul Spa still under construction can be canceled.".to_string(),
            "phase".to_string(),
        ),
        SoulSpaConstructionCancelResult::OwnerMismatch => (
            NotificationSeverity::Error,
            "Soul Spa construction not canceled",
            "The construction footprint changed; no resources were modified.".to_string(),
            "owner-mismatch".to_string(),
        ),
        SoulSpaConstructionCancelResult::ActiveTaskMismatch => (
            NotificationSeverity::Error,
            "Soul Spa construction not canceled",
            "Related hauling work changed; retry after the task state settles.".to_string(),
            "task-mismatch".to_string(),
        ),
    };

    UserFacingNotification::new(
        format!("soul-spa-construction-cancel:{target}:{result_key}"),
        severity,
        title,
        body,
        NotificationRetention::ToastOnly,
    )
}

fn familiar_settings_notification(
    outcome: FamiliarSettingsChangeOutcome,
) -> Option<UserFacingNotification> {
    let target = outcome.target.to_bits();
    let (key, severity, title, body) = match outcome.status {
        FamiliarSettingsChangeStatus::Applied {
            released_souls,
            entered_all_work_disabled,
            ..
        } if entered_all_work_disabled => (
            format!("familiar-settings:{target}:all-disabled:{released_souls}"),
            NotificationSeverity::Warning,
            "Familiar work disabled",
            if released_souls > 0 {
                format!(
                    "No new work will be assigned. Released {released_souls} excess Soul(s); current work and self-maintenance continue."
                )
            } else {
                "No new work will be assigned. Current work and self-maintenance continue."
                    .to_string()
            },
        ),
        FamiliarSettingsChangeStatus::Applied { released_souls, .. } if released_souls > 0 => (
            format!("familiar-settings:{target}:released:{released_souls}"),
            NotificationSeverity::Info,
            "Familiar roster reduced",
            format!("Released {released_souls} excess Soul(s)."),
        ),
        FamiliarSettingsChangeStatus::Applied { .. }
        | FamiliarSettingsChangeStatus::Unchanged { .. } => return None,
        FamiliarSettingsChangeStatus::Rejected { reason, .. } => {
            let (severity, title, body, reason_key) = match reason {
                FamiliarSettingsRejection::StaleTarget => (
                    NotificationSeverity::Warning,
                    "Familiar setting not applied",
                    "The selected Familiar no longer exists.",
                    "stale",
                ),
                FamiliarSettingsRejection::PausedOrModal => (
                    NotificationSeverity::Warning,
                    "Familiar setting not applied",
                    "Close the foreground menu or resume the simulation before editing.",
                    "blocked",
                ),
                FamiliarSettingsRejection::MissingOperation => (
                    NotificationSeverity::Error,
                    "Familiar setting unavailable",
                    "The Familiar is missing durable operation settings.",
                    "missing-operation",
                ),
                FamiliarSettingsRejection::MissingPolicy => (
                    NotificationSeverity::Error,
                    "Familiar setting unavailable",
                    "The Familiar is missing its durable work policy.",
                    "missing-policy",
                ),
            };
            (
                format!("familiar-settings:{target}:rejected:{reason_key}"),
                severity,
                title,
                body.to_string(),
            )
        }
    };

    Some(UserFacingNotification::new(
        key,
        severity,
        title,
        body,
        NotificationRetention::ToastOnly,
    ))
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

    #[test]
    fn ordinary_familiar_settings_commits_do_not_spam_toasts() {
        let target = Entity::PLACEHOLDER;
        assert!(
            familiar_settings_notification(FamiliarSettingsChangeOutcome {
                target,
                status: FamiliarSettingsChangeStatus::Applied {
                    requested_patches: 1,
                    released_souls: 0,
                    entered_all_work_disabled: false,
                },
            })
            .is_none()
        );
        assert!(
            familiar_settings_notification(FamiliarSettingsChangeOutcome {
                target,
                status: FamiliarSettingsChangeStatus::Unchanged {
                    requested_patches: 2,
                },
            })
            .is_none()
        );
    }

    #[test]
    fn familiar_settings_warn_for_all_disabled_and_report_released_roster_once() {
        let target = Entity::PLACEHOLDER;
        let disabled = familiar_settings_notification(FamiliarSettingsChangeOutcome {
            target,
            status: FamiliarSettingsChangeStatus::Applied {
                requested_patches: 1,
                released_souls: 0,
                entered_all_work_disabled: true,
            },
        })
        .unwrap();
        let released = familiar_settings_notification(FamiliarSettingsChangeOutcome {
            target,
            status: FamiliarSettingsChangeStatus::Applied {
                requested_patches: 1,
                released_souls: 2,
                entered_all_work_disabled: false,
            },
        })
        .unwrap();
        let combined = familiar_settings_notification(FamiliarSettingsChangeOutcome {
            target,
            status: FamiliarSettingsChangeStatus::Applied {
                requested_patches: 2,
                released_souls: 2,
                entered_all_work_disabled: true,
            },
        })
        .unwrap();

        assert_eq!(disabled.severity, NotificationSeverity::Warning);
        assert_eq!(released.severity, NotificationSeverity::Info);
        assert_eq!(combined.severity, NotificationSeverity::Warning);
        assert_eq!(disabled.retention, NotificationRetention::ToastOnly);
        assert!(released.body.contains("2 excess Soul"));
        assert!(combined.body.contains("2 excess Soul"));
        assert_ne!(disabled.key, released.key);
    }

    #[test]
    fn familiar_settings_rejections_use_warning_or_error_by_recoverability() {
        let target = Entity::PLACEHOLDER;
        for (reason, expected) in [
            (
                FamiliarSettingsRejection::StaleTarget,
                NotificationSeverity::Warning,
            ),
            (
                FamiliarSettingsRejection::PausedOrModal,
                NotificationSeverity::Warning,
            ),
            (
                FamiliarSettingsRejection::MissingOperation,
                NotificationSeverity::Error,
            ),
            (
                FamiliarSettingsRejection::MissingPolicy,
                NotificationSeverity::Error,
            ),
        ] {
            let notification = familiar_settings_notification(FamiliarSettingsChangeOutcome {
                target,
                status: FamiliarSettingsChangeStatus::Rejected {
                    requested_patches: 1,
                    reason,
                },
            })
            .unwrap();
            assert_eq!(notification.severity, expected);
            assert_eq!(notification.retention, NotificationRetention::ToastOnly);
        }
    }

    #[test]
    fn deconstruction_terminal_notifications_keep_success_and_recovery_failures_distinct() {
        let order = Entity::from_raw_u32(10).expect("order");
        let target = Entity::from_raw_u32(11).expect("target");
        let canceled = deconstruction_cancel_notification(DeconstructionCancelOutcome {
            order,
            target: Some(target),
            result: DeconstructionCancelResult::Canceled,
        });
        let blocked = deconstruction_commit_notification(DeconstructionCommitOutcome {
            worker: Entity::PLACEHOLDER,
            order,
            target,
            result: DeconstructionCommitResult::NoSafeRecovery,
        })
        .unwrap();
        let internal = deconstruction_commit_notification(DeconstructionCommitOutcome {
            worker: Entity::PLACEHOLDER,
            order,
            target,
            result: DeconstructionCommitResult::Duplicate,
        });

        assert_eq!(canceled.severity, NotificationSeverity::Success);
        assert_eq!(blocked.severity, NotificationSeverity::Warning);
        assert!(blocked.body.contains("left unchanged"));
        assert!(internal.is_none());
        assert_ne!(canceled.key, blocked.key);
    }

    #[test]
    fn soul_spa_construction_cancel_notifications_preserve_terminal_meaning() {
        let target = Entity::from_raw_u32(12).expect("target");
        let notification = |result| {
            soul_spa_construction_cancel_notification(SoulSpaConstructionCancelOutcome {
                target,
                result,
            })
        };

        let canceled =
            notification(SoulSpaConstructionCancelResult::Canceled { refunded_bones: 7 });
        let paused = notification(SoulSpaConstructionCancelResult::Paused);
        let stale = notification(SoulSpaConstructionCancelResult::StaleTarget);
        let phase = notification(SoulSpaConstructionCancelResult::PhaseUnavailable);
        let owner = notification(SoulSpaConstructionCancelResult::OwnerMismatch);
        let task = notification(SoulSpaConstructionCancelResult::ActiveTaskMismatch);

        assert_eq!(canceled.severity, NotificationSeverity::Success);
        assert!(canceled.body.contains('7'));
        assert_eq!(paused.severity, NotificationSeverity::Warning);
        assert!(paused.body.contains("Resume"));
        assert_eq!(stale.severity, NotificationSeverity::Warning);
        assert_eq!(phase.severity, NotificationSeverity::Warning);
        assert_eq!(owner.severity, NotificationSeverity::Error);
        assert_eq!(task.severity, NotificationSeverity::Error);
        for notification in [&canceled, &paused, &stale, &phase, &owner, &task] {
            assert_eq!(notification.retention, NotificationRetention::ToastOnly);
        }
        let keys = [
            &canceled.key,
            &paused.key,
            &stale.key,
            &phase.key,
            &owner.key,
            &task.key,
        ]
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
        assert_eq!(keys.len(), 6);
    }
}
