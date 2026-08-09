//! Active-play autosave scheduler (Track C2 M3).

use std::time::Duration;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::game_state::TaskMode;
use hw_core::{GameSettings, SaveSlotId};
use hw_ui::area_edit::AreaEditSession;
use hw_ui::components::UiInputState;

use super::catalog::{
    AutosaveGenerationLimit, SaveCatalog, SaveExpectedTarget, SaveFileRevision, SaveStorageRoot,
    read_save_file_revision,
};
use super::catalog_ui::SaveCatalogUi;
use super::saving::expected_target_from_revision;
use super::state::{
    SaveLoadFailureKind, SaveLoadOutcome, SaveLoadOutcomeSource, SaveLoadRequest, SaveLoadResult,
    SaveLoadState, SaveRecoveryMode, SaveRequestOrigin,
};
use crate::app_contexts::TaskContext;

#[derive(Resource, Debug, Clone)]
pub struct AutosaveScheduler {
    pub elapsed_active: Duration,
    pub due: bool,
    pub round_robin_cursor: u8,
}

impl Default for AutosaveScheduler {
    fn default() -> Self {
        Self {
            elapsed_active: Duration::ZERO,
            due: false,
            round_robin_cursor: 1,
        }
    }
}

impl AutosaveScheduler {
    pub fn reset_timer(&mut self) {
        self.elapsed_active = Duration::ZERO;
        self.due = false;
    }

    pub fn interval_for(settings: &GameSettings) -> Duration {
        Duration::from_secs(u64::from(settings.normalized_autosave_interval_minutes()) * 60)
    }

    pub fn tick_active(&mut self, delta: Duration, interval: Duration) {
        if self.due {
            return;
        }
        self.elapsed_active = self.elapsed_active.saturating_add(delta);
        if self.elapsed_active >= interval {
            self.elapsed_active = interval;
            self.due = true;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutosaveEligibility {
    Eligible,
    Ineligible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutosaveEligibilityContext {
    pub recovery: SaveRecoveryMode,
    pub virtual_paused: bool,
    pub foreground_capture: bool,
    pub catalog_open: bool,
    pub pending_request: bool,
    pub snapshot_eligible: bool,
}

pub fn evaluate_autosave_eligibility(context: AutosaveEligibilityContext) -> AutosaveEligibility {
    if context.recovery != SaveRecoveryMode::Healthy
        || context.virtual_paused
        || context.foreground_capture
        || context.catalog_open
        || context.pending_request
        || !context.snapshot_eligible
    {
        AutosaveEligibility::Ineligible
    } else {
        AutosaveEligibility::Eligible
    }
}

fn snapshot_is_eligible(
    task_context: Option<&TaskContext>,
    area_edit_session: Option<&AreaEditSession>,
) -> bool {
    task_context.is_none_or(|context| context.0 == TaskMode::None)
        && area_edit_session.is_none_or(|session| {
            !session.is_dragging() && session.pending_dream_planting.is_none()
        })
}

fn eligibility_context(
    recovery: SaveRecoveryMode,
    virtual_paused: bool,
    ui_input_state: &UiInputState,
    catalog_ui: &SaveCatalogUi,
    save_state: &SaveLoadState,
    task_context: Option<&TaskContext>,
    area_edit_session: Option<&AreaEditSession>,
) -> AutosaveEligibilityContext {
    AutosaveEligibilityContext {
        recovery,
        virtual_paused,
        foreground_capture: ui_input_state.world_input_captured
            || ui_input_state.world_input_capture_started,
        catalog_open: catalog_ui.is_open(),
        pending_request: !save_state.is_idle(),
        snapshot_eligible: snapshot_is_eligible(task_context, area_edit_session),
    }
}

#[derive(SystemParam)]
pub(crate) struct AutosaveRuntimeParams<'w> {
    settings: Res<'w, GameSettings>,
    recovery: Res<'w, SaveRecoveryMode>,
    virtual_time: Res<'w, Time<Virtual>>,
    ui_input_state: Res<'w, UiInputState>,
    catalog_ui: Res<'w, SaveCatalogUi>,
    task_context: Option<Res<'w, TaskContext>>,
    area_edit_session: Option<Res<'w, AreaEditSession>>,
}

impl AutosaveRuntimeParams<'_> {
    fn eligibility(&self, save_state: &SaveLoadState) -> AutosaveEligibility {
        evaluate_autosave_eligibility(eligibility_context(
            *self.recovery,
            self.virtual_time.is_paused(),
            &self.ui_input_state,
            &self.catalog_ui,
            save_state,
            self.task_context.as_deref(),
            self.area_edit_session.as_deref(),
        ))
    }
}

pub fn select_autosave_slot(
    catalog: &SaveCatalog,
    generations: AutosaveGenerationLimit,
    round_robin_cursor: u8,
) -> (SaveSlotId, SaveExpectedTarget, bool) {
    let active: Vec<SaveSlotId> = (1..=generations.clamped())
        .filter_map(SaveSlotId::autosave)
        .collect();

    for slot in &active {
        let missing = catalog
            .entry(*slot)
            .map(|entry| !entry.revision.exists)
            .unwrap_or(true);
        if missing {
            return (*slot, SaveExpectedTarget::Absent, false);
        }
    }

    let all_mtime_trusted = active.iter().all(|slot| {
        catalog
            .entry(*slot)
            .and_then(|entry| entry.modified)
            .is_some()
    });

    if all_mtime_trusted {
        let oldest = active
            .iter()
            .min_by_key(|slot| {
                let entry = catalog.entry(**slot).expect("active slot scanned");
                (entry.modified, slot.autosave_generation())
            })
            .copied()
            .unwrap_or(SaveSlotId::Autosave1);
        let revision = catalog
            .entry(oldest)
            .map(|entry| entry.revision.clone())
            .unwrap_or_else(SaveFileRevision::absent);
        return (oldest, expected_target_from_revision(revision), false);
    }

    let cursor = if (1..=generations.clamped()).contains(&round_robin_cursor) {
        round_robin_cursor
    } else {
        1
    };
    let slot = SaveSlotId::autosave(cursor).unwrap_or(SaveSlotId::Autosave1);
    let revision = catalog
        .entry(slot)
        .map(|entry| entry.revision.clone())
        .unwrap_or_else(SaveFileRevision::absent);
    (slot, expected_target_from_revision(revision), true)
}

pub(crate) fn advance_round_robin(scheduler: &mut AutosaveScheduler, generations: u8) {
    let next = scheduler.round_robin_cursor % generations + 1;
    scheduler.round_robin_cursor = next;
}

/// Advances the active-play timer. Does not issue requests.
pub(crate) fn tick_autosave_timer_system(
    time: Res<Time<Real>>,
    save_state: Res<SaveLoadState>,
    runtime: AutosaveRuntimeParams,
    mut scheduler: ResMut<AutosaveScheduler>,
) {
    if !runtime.settings.autosave_enabled {
        return;
    }
    if runtime.eligibility(&save_state) == AutosaveEligibility::Ineligible {
        return;
    }
    let interval = AutosaveScheduler::interval_for(&runtime.settings);
    scheduler.tick_active(time.delta(), interval);
}

/// Issues at most one autosave request after manual producers have run.
pub(crate) fn autosave_scheduler_system(
    runtime: AutosaveRuntimeParams,
    catalog: Res<SaveCatalog>,
    root: Res<SaveStorageRoot>,
    mut save_state: ResMut<SaveLoadState>,
    scheduler: Res<AutosaveScheduler>,
) {
    if !runtime.settings.autosave_enabled || !scheduler.due {
        return;
    }
    if runtime.eligibility(&save_state) == AutosaveEligibility::Ineligible {
        // Keep due; do not emit terminal failure.
        return;
    }

    let generations = AutosaveGenerationLimit(runtime.settings.normalized_autosave_generations());
    let (slot, expected, uses_round_robin) =
        select_autosave_slot(&catalog, generations, scheduler.round_robin_cursor);

    // Authoritative re-read at issue time.
    let path = root.resolve(slot);
    let expected = match read_save_file_revision(&path) {
        Ok(revision) => expected_target_from_revision(revision),
        Err(_) => expected,
    };

    if save_state.try_set(SaveLoadRequest::Save {
        origin: SaveRequestOrigin::Autosave,
        slot,
        expected_target: expected,
    }) {
        // Timer consumption happens on terminal outcomes.
        if uses_round_robin {
            // Advance only after successful commit; stash intent via cursor leave.
            let _ = uses_round_robin;
        }
        let _ = generations;
    }
}

pub(crate) fn consume_autosave_timer_on_outcomes(
    mut outcomes: MessageReader<SaveLoadOutcome>,
    mut scheduler: ResMut<AutosaveScheduler>,
    settings: Res<GameSettings>,
) {
    for outcome in outcomes.read() {
        match outcome.source {
            SaveLoadOutcomeSource::Autosave => match outcome.result {
                SaveLoadResult::Succeeded => {
                    scheduler.reset_timer();
                    advance_round_robin(&mut scheduler, settings.normalized_autosave_generations());
                }
                SaveLoadResult::Failed(SaveLoadFailureKind::CommittedDurabilityUncertain) => {
                    scheduler.reset_timer();
                    advance_round_robin(&mut scheduler, settings.normalized_autosave_generations());
                }
                SaveLoadResult::Failed(
                    SaveLoadFailureKind::SaveSerialize
                    | SaveLoadFailureKind::SaveWrite
                    | SaveLoadFailureKind::OverwriteConfirmationRequired
                    | SaveLoadFailureKind::RequestRejected,
                ) => {
                    scheduler.reset_timer();
                }
                _ => {}
            },
            SaveLoadOutcomeSource::Manual(_) => {
                if outcome.operation != super::state::SaveLoadOperation::Save {
                    continue;
                }
                match outcome.result {
                    SaveLoadResult::Succeeded
                    | SaveLoadResult::Failed(SaveLoadFailureKind::CommittedDurabilityUncertain) => {
                        scheduler.reset_timer();
                    }
                    _ => {}
                }
            }
            SaveLoadOutcomeSource::Load(_) => {
                if matches!(outcome.result, SaveLoadResult::Succeeded) {
                    scheduler.reset_timer();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::save::catalog::{
        SaveCatalogEntry, SaveContentStatus, SaveSlotCapabilities,
    };
    use hw_core::SaveSlotRole;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn entry(slot: SaveSlotId, exists: bool, modified: Option<SystemTime>) -> SaveCatalogEntry {
        SaveCatalogEntry {
            slot,
            role: SaveSlotRole::Autosave,
            capabilities: SaveSlotCapabilities {
                can_manual_save: false,
                can_scheduler_save: true,
                can_load: exists,
            },
            content: if exists {
                SaveContentStatus::CurrentV1 { worldgen_seed: 1 }
            } else {
                SaveContentStatus::Empty
            },
            revision: if exists {
                SaveFileRevision {
                    exists: true,
                    length: 10,
                    modified,
                    file_identity: Some(1),
                    prefix_fingerprint: 1,
                }
            } else {
                SaveFileRevision::absent()
            },
            file_size: if exists { 10 } else { 0 },
            modified,
            modified_unavailable: false,
        }
    }

    #[test]
    fn prefers_missing_active_slot_before_rotation() {
        let mut catalog = SaveCatalog::default();
        catalog.replace_entries(
            vec![
                entry(SaveSlotId::Autosave1, true, Some(UNIX_EPOCH)),
                entry(SaveSlotId::Autosave2, false, None),
                entry(SaveSlotId::Autosave3, true, Some(UNIX_EPOCH)),
            ],
            0,
        );
        let (slot, expected, rr) = select_autosave_slot(&catalog, AutosaveGenerationLimit(3), 1);
        assert_eq!(slot, SaveSlotId::Autosave2);
        assert_eq!(expected, SaveExpectedTarget::Absent);
        assert!(!rr);
    }

    #[test]
    fn eligibility_requires_healthy_unpaused_idle_without_catalog() {
        let eligible = AutosaveEligibilityContext {
            recovery: SaveRecoveryMode::Healthy,
            virtual_paused: false,
            foreground_capture: false,
            catalog_open: false,
            pending_request: false,
            snapshot_eligible: true,
        };
        assert_eq!(
            evaluate_autosave_eligibility(eligible),
            AutosaveEligibility::Eligible
        );
        assert_eq!(
            evaluate_autosave_eligibility(AutosaveEligibilityContext {
                recovery: SaveRecoveryMode::RecoveryFailed,
                ..eligible
            }),
            AutosaveEligibility::Ineligible
        );
        assert_eq!(
            evaluate_autosave_eligibility(AutosaveEligibilityContext {
                virtual_paused: true,
                ..eligible
            }),
            AutosaveEligibility::Ineligible
        );
        assert_eq!(
            evaluate_autosave_eligibility(AutosaveEligibilityContext {
                foreground_capture: true,
                ..eligible
            }),
            AutosaveEligibility::Ineligible
        );
        assert_eq!(
            evaluate_autosave_eligibility(AutosaveEligibilityContext {
                snapshot_eligible: false,
                ..eligible
            }),
            AutosaveEligibility::Ineligible
        );
    }

    #[test]
    fn timer_saturates_to_one_due() {
        let mut scheduler = AutosaveScheduler::default();
        let interval = Duration::from_secs(60);
        scheduler.tick_active(Duration::from_secs(30), interval);
        assert!(!scheduler.due);
        scheduler.tick_active(Duration::from_secs(40), interval);
        assert!(scheduler.due);
        assert_eq!(scheduler.elapsed_active, interval);
        scheduler.tick_active(Duration::from_secs(100), interval);
        assert!(scheduler.due);
        assert_eq!(scheduler.elapsed_active, interval);
    }

    #[test]
    fn manual_save_failure_does_not_consume_autosave_timer() {
        let mut app = crate::test_support::minimal_app();
        app.init_resource::<AutosaveScheduler>()
            .init_resource::<GameSettings>()
            .add_message::<SaveLoadOutcome>()
            .add_systems(Update, consume_autosave_timer_on_outcomes);
        {
            let mut scheduler = app.world_mut().resource_mut::<AutosaveScheduler>();
            scheduler.due = true;
            scheduler.elapsed_active = Duration::from_secs(120);
        }
        app.world_mut().write_message(SaveLoadOutcome {
            operation: super::super::state::SaveLoadOperation::Save,
            target: "Manual slot 1".to_string(),
            result: SaveLoadResult::Failed(SaveLoadFailureKind::SaveWrite),
            source: SaveLoadOutcomeSource::Manual(SaveRequestOrigin::ManualCatalog {
                dialog_session: 1,
            }),
        });
        app.update();
        let scheduler = app.world().resource::<AutosaveScheduler>();
        assert!(scheduler.due);
        assert_eq!(scheduler.elapsed_active, Duration::from_secs(120));
    }

    #[test]
    fn due_autosave_stays_pending_while_paused_or_foreground_capture_is_active() {
        let mut app = crate::test_support::minimal_app();
        app.init_resource::<AutosaveScheduler>()
            .init_resource::<GameSettings>()
            .init_resource::<SaveRecoveryMode>()
            .init_resource::<SaveCatalogUi>()
            .init_resource::<SaveCatalog>()
            .init_resource::<SaveStorageRoot>()
            .init_resource::<SaveLoadState>()
            .init_resource::<UiInputState>()
            .init_resource::<Time<Virtual>>()
            .add_systems(Update, autosave_scheduler_system);
        app.world_mut().resource_mut::<AutosaveScheduler>().due = true;
        app.world_mut().resource_mut::<Time<Virtual>>().pause();

        app.update();

        assert!(app.world().resource::<SaveLoadState>().is_idle());
        assert!(app.world().resource::<AutosaveScheduler>().due);

        app.world_mut().resource_mut::<Time<Virtual>>().unpause();
        app.world_mut()
            .resource_mut::<UiInputState>()
            .world_input_captured = true;
        app.update();

        assert!(app.world().resource::<SaveLoadState>().is_idle());
        assert!(app.world().resource::<AutosaveScheduler>().due);
    }
}
