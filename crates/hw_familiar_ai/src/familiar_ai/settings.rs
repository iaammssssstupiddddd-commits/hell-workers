use std::collections::HashMap;

use bevy::prelude::*;
use hw_core::events::{FamiliarRosterReleasedVisualMessage, SoulTaskUnassignRequest};
use hw_core::familiar::{Familiar, FamiliarOperation, FamiliarPolicy, FamiliarSettingsPatch};
use hw_core::relationships::{CommandedBy, Commanding};

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FamiliarSettingsChangeRequest {
    pub target: Entity,
    pub patch: FamiliarSettingsPatch,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FamiliarSettingsChangeOutcome {
    pub target: Entity,
    pub status: FamiliarSettingsChangeStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FamiliarSettingsChangeStatus {
    Applied {
        requested_patches: usize,
        released_souls: usize,
        entered_all_work_disabled: bool,
    },
    Unchanged {
        requested_patches: usize,
    },
    Rejected {
        requested_patches: usize,
        reason: FamiliarSettingsRejection,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FamiliarSettingsRejection {
    StaleTarget,
    MissingOperation,
    MissingPolicy,
    PausedOrModal,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FamiliarSettingsApplySet;

fn stable_target_key(entity: Entity) -> (u32, u32) {
    (entity.index_u32(), entity.generation().to_bits())
}

fn grouped_requests(
    requests: &mut MessageReader<FamiliarSettingsChangeRequest>,
) -> Vec<(Entity, Vec<FamiliarSettingsPatch>)> {
    let mut by_target: HashMap<Entity, Vec<FamiliarSettingsPatch>> = HashMap::new();
    for request in requests.read() {
        by_target
            .entry(request.target)
            .or_default()
            .push(request.patch);
    }
    let mut grouped: Vec<_> = by_target.into_iter().collect();
    grouped.sort_unstable_by_key(|(target, _)| stable_target_key(*target));
    grouped
}

type FamiliarSettingsQuery<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static mut FamiliarOperation>,
        Option<&'static mut FamiliarPolicy>,
        Option<&'static Commanding>,
    ),
    With<Familiar>,
>;

pub fn apply_familiar_settings_change_requests_system(
    mut requests: MessageReader<FamiliarSettingsChangeRequest>,
    q_familiars: Query<(), With<Familiar>>,
    mut q_settings: FamiliarSettingsQuery,
    mut task_unassign_requests: MessageWriter<SoulTaskUnassignRequest>,
    mut release_visuals: MessageWriter<FamiliarRosterReleasedVisualMessage>,
    mut outcomes: MessageWriter<FamiliarSettingsChangeOutcome>,
    mut commands: Commands,
) {
    for (target, patches) in grouped_requests(&mut requests) {
        let requested_patches = patches.len();
        if !q_familiars.contains(target) {
            outcomes.write(FamiliarSettingsChangeOutcome {
                target,
                status: FamiliarSettingsChangeStatus::Rejected {
                    requested_patches,
                    reason: FamiliarSettingsRejection::StaleTarget,
                },
            });
            continue;
        }

        let Ok((operation, policy, commanding)) = q_settings.get_mut(target) else {
            outcomes.write(FamiliarSettingsChangeOutcome {
                target,
                status: FamiliarSettingsChangeStatus::Rejected {
                    requested_patches,
                    reason: FamiliarSettingsRejection::StaleTarget,
                },
            });
            continue;
        };
        let Some(mut operation) = operation else {
            outcomes.write(FamiliarSettingsChangeOutcome {
                target,
                status: FamiliarSettingsChangeStatus::Rejected {
                    requested_patches,
                    reason: FamiliarSettingsRejection::MissingOperation,
                },
            });
            continue;
        };
        let Some(mut policy) = policy else {
            outcomes.write(FamiliarSettingsChangeOutcome {
                target,
                status: FamiliarSettingsChangeStatus::Rejected {
                    requested_patches,
                    reason: FamiliarSettingsRejection::MissingPolicy,
                },
            });
            continue;
        };

        let current_operation = operation.clone();
        let current_policy = policy.clone();
        let mut next_operation = current_operation.clone();
        let mut next_policy = current_policy.clone();
        for patch in patches {
            patch.apply(&mut next_operation, &mut next_policy);
        }
        next_policy.normalize();

        let roster: Vec<Entity> = commanding
            .map(|commanding| commanding.iter().copied().collect())
            .unwrap_or_default();
        let released_souls = roster
            .len()
            .saturating_sub(next_operation.max_controlled_soul);
        let entered_all_work_disabled =
            !current_policy.all_work_disabled() && next_policy.all_work_disabled();
        let operation_changed = current_operation != next_operation;
        let policy_changed = current_policy != next_policy;

        if operation_changed {
            *operation = next_operation;
        }
        if policy_changed {
            *policy = next_policy;
        }

        if released_souls > 0 {
            for member in roster.iter().rev().take(released_souls).copied() {
                task_unassign_requests.write(SoulTaskUnassignRequest {
                    soul_entity: member,
                    emit_abandoned: false,
                });
                commands.entity(member).remove::<CommandedBy>();
            }
            release_visuals.write(FamiliarRosterReleasedVisualMessage {
                familiar_entity: target,
                released_souls,
            });
        }

        let status = if operation_changed || policy_changed || released_souls > 0 {
            FamiliarSettingsChangeStatus::Applied {
                requested_patches,
                released_souls,
                entered_all_work_disabled,
            }
        } else {
            FamiliarSettingsChangeStatus::Unchanged { requested_patches }
        };
        outcomes.write(FamiliarSettingsChangeOutcome { target, status });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::familiar::{FamiliarWorkPriority, FamiliarWorkRule};
    use hw_core::jobs::WorkType;
    use hw_core::relationships::{ManagedBy, ManagedTasks, WorkingOn};
    use hw_jobs::{ActiveTaskIdentity, AssignedTask, GatherData, GatherPhase};

    #[derive(Resource, Default)]
    struct Receipts {
        outcomes: Vec<FamiliarSettingsChangeOutcome>,
        unassigns: Vec<Entity>,
    }

    fn collect_receipts(
        mut outcomes: MessageReader<FamiliarSettingsChangeOutcome>,
        mut unassigns: MessageReader<SoulTaskUnassignRequest>,
        mut receipts: ResMut<Receipts>,
    ) {
        receipts.outcomes.extend(outcomes.read().copied());
        receipts
            .unassigns
            .extend(unassigns.read().map(|request| request.soul_entity));
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<FamiliarSettingsChangeRequest>()
            .add_message::<FamiliarSettingsChangeOutcome>()
            .add_message::<SoulTaskUnassignRequest>()
            .add_message::<FamiliarRosterReleasedVisualMessage>()
            .init_resource::<Receipts>()
            .add_systems(
                Update,
                (
                    apply_familiar_settings_change_requests_system,
                    ApplyDeferred,
                    collect_receipts,
                )
                    .chain(),
            );
        app
    }

    fn familiar(app: &mut App, max: usize) -> Entity {
        app.world_mut()
            .spawn((
                Familiar::default(),
                FamiliarOperation {
                    fatigue_threshold: 0.8,
                    max_controlled_soul: max,
                },
                FamiliarPolicy::default(),
            ))
            .id()
    }

    fn request(app: &mut App, target: Entity, patch: FamiliarSettingsPatch) {
        app.world_mut()
            .write_message(FamiliarSettingsChangeRequest { target, patch });
    }

    #[test]
    fn same_target_batch_replays_fifo_and_releases_only_for_final_max() {
        let mut app = app();
        let target = familiar(&mut app, 4);
        let souls = [
            app.world_mut().spawn(CommandedBy(target)).id(),
            app.world_mut().spawn(CommandedBy(target)).id(),
            app.world_mut().spawn(CommandedBy(target)).id(),
            app.world_mut().spawn(CommandedBy(target)).id(),
        ];
        app.world_mut().flush();

        request(
            &mut app,
            target,
            FamiliarSettingsPatch::AdjustMaxControlledSoul { delta: -2 },
        );
        request(
            &mut app,
            target,
            FamiliarSettingsPatch::AdjustMaxControlledSoul { delta: 2 },
        );
        app.update();

        assert_eq!(
            app.world()
                .get::<FamiliarOperation>(target)
                .unwrap()
                .max_controlled_soul,
            4
        );
        assert_eq!(
            app.world()
                .get::<Commanding>(target)
                .unwrap()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            souls
        );
        assert!(app.world().resource::<Receipts>().unassigns.is_empty());
        assert_eq!(
            app.world().resource::<Receipts>().outcomes,
            vec![FamiliarSettingsChangeOutcome {
                target,
                status: FamiliarSettingsChangeStatus::Unchanged {
                    requested_patches: 2,
                },
            }]
        );
    }

    #[test]
    fn max_decrease_releases_reverse_roster_once_and_commits_before_outcome() {
        let mut app = app();
        let target = familiar(&mut app, 4);
        let souls = [
            app.world_mut().spawn(CommandedBy(target)).id(),
            app.world_mut().spawn(CommandedBy(target)).id(),
            app.world_mut().spawn(CommandedBy(target)).id(),
            app.world_mut().spawn(CommandedBy(target)).id(),
        ];
        app.world_mut().flush();

        request(
            &mut app,
            target,
            FamiliarSettingsPatch::AdjustMaxControlledSoul { delta: -2 },
        );
        app.update();

        assert_eq!(
            app.world().resource::<Receipts>().unassigns,
            vec![souls[3], souls[2]]
        );
        assert!(app.world().get::<CommandedBy>(souls[3]).is_none());
        assert!(app.world().get::<CommandedBy>(souls[2]).is_none());
        assert_eq!(
            app.world()
                .get::<Commanding>(target)
                .unwrap()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            souls[..2]
        );
        assert_eq!(
            app.world().resource::<Receipts>().outcomes,
            vec![FamiliarSettingsChangeOutcome {
                target,
                status: FamiliarSettingsChangeStatus::Applied {
                    requested_patches: 1,
                    released_souls: 2,
                    entered_all_work_disabled: false,
                },
            }]
        );
    }

    #[test]
    fn settings_batch_updates_policy_and_reports_all_disabled_transition() {
        let mut app = app();
        let target = familiar(&mut app, 2);

        request(
            &mut app,
            target,
            FamiliarSettingsPatch::SetWorkPriority {
                work_type: WorkType::Haul,
                priority: FamiliarWorkPriority::High,
            },
        );
        request(
            &mut app,
            target,
            FamiliarSettingsPatch::SetAllWorkAllowed { allowed: false },
        );
        app.update();

        let policy = app.world().get::<FamiliarPolicy>(target).unwrap();
        assert!(policy.all_work_disabled());
        assert_eq!(
            policy.rule_for(WorkType::Haul),
            FamiliarWorkRule {
                allowed: false,
                priority: FamiliarWorkPriority::High,
            }
        );
        assert_eq!(
            app.world().resource::<Receipts>().outcomes[0].status,
            FamiliarSettingsChangeStatus::Applied {
                requested_patches: 2,
                released_souls: 0,
                entered_all_work_disabled: true,
            }
        );
    }

    #[test]
    fn policy_change_preserves_the_current_task_and_management_relationships() {
        let mut app = app();
        let target = familiar(&mut app, 2);
        let task = app.world_mut().spawn(ManagedBy(target)).id();
        let assigned = AssignedTask::Gather(GatherData {
            target: task,
            work_type: WorkType::Chop,
            phase: GatherPhase::Collecting { progress: 0.5 },
        });
        let identity = ActiveTaskIdentity::new(task, task, WorkType::Chop);
        let soul = app
            .world_mut()
            .spawn((
                CommandedBy(target),
                WorkingOn(task),
                assigned.clone(),
                identity,
            ))
            .id();
        app.world_mut().flush();

        request(
            &mut app,
            target,
            FamiliarSettingsPatch::SetWorkAllowed {
                work_type: WorkType::Chop,
                allowed: false,
            },
        );
        app.update();

        assert_eq!(
            app.world().get::<AssignedTask>(soul).unwrap().work_type(),
            assigned.work_type()
        );
        assert_eq!(
            app.world()
                .get::<ActiveTaskIdentity>(soul)
                .unwrap()
                .current_target_entity,
            task
        );
        assert_eq!(app.world().get::<WorkingOn>(soul).unwrap().0, task);
        assert_eq!(app.world().get::<CommandedBy>(soul).unwrap().0, target);
        assert!(
            app.world()
                .get::<ManagedTasks>(target)
                .unwrap()
                .contains(task)
        );
        assert!(
            !app.world()
                .get::<FamiliarPolicy>(target)
                .unwrap()
                .rule_for(WorkType::Chop)
                .allowed
        );
    }

    #[test]
    fn stale_and_missing_components_each_emit_one_terminal_rejection() {
        let mut app = app();
        let stale = app.world_mut().spawn_empty().id();
        assert!(app.world_mut().despawn(stale));
        let missing_operation = app
            .world_mut()
            .spawn((Familiar::default(), FamiliarPolicy::default()))
            .id();
        let missing_policy = app
            .world_mut()
            .spawn((Familiar::default(), FamiliarOperation::default()))
            .id();

        for target in [stale, missing_operation, missing_policy] {
            request(
                &mut app,
                target,
                FamiliarSettingsPatch::AdjustMaxControlledSoul { delta: 1 },
            );
        }
        app.update();

        let statuses: HashMap<_, _> = app
            .world()
            .resource::<Receipts>()
            .outcomes
            .iter()
            .map(|outcome| (outcome.target, outcome.status))
            .collect();
        assert_eq!(
            statuses[&stale],
            FamiliarSettingsChangeStatus::Rejected {
                requested_patches: 1,
                reason: FamiliarSettingsRejection::StaleTarget,
            }
        );
        assert_eq!(
            statuses[&missing_operation],
            FamiliarSettingsChangeStatus::Rejected {
                requested_patches: 1,
                reason: FamiliarSettingsRejection::MissingOperation,
            }
        );
        assert_eq!(
            statuses[&missing_policy],
            FamiliarSettingsChangeStatus::Rejected {
                requested_patches: 1,
                reason: FamiliarSettingsRejection::MissingPolicy,
            }
        );
    }
}
