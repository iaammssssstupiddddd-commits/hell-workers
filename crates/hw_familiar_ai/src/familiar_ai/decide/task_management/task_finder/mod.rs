//! タスク検索モジュール

mod filter;
mod score;

use bevy::prelude::*;
use hw_core::area::TaskArea;
use hw_core::relationships::ManagedTasks;
use hw_jobs::{TargetBlueprint, TaskDiagnosticInputRevisions, WorkType};
use hw_spatial::{DesignationSpatialGrid, TransportRequestSpatialGrid};
use hw_world::{WorldMap, Yard};
use std::collections::HashSet;

use crate::familiar_ai::decide::task_management::policy_score::{
    PolicyScoreContributions, transport_policy_units,
};
use crate::familiar_ai::decide::task_management::{
    FamiliarEvaluatorDiagnostics, FamiliarTaskAssignmentQueries,
};
use filter::{candidate_snapshot, collect_candidate_entities};
use score::score_candidate;

#[derive(Clone, Copy, Debug)]
pub struct DelegationCandidate {
    pub entity: Entity,
    pub work_type: WorkType,
    pub target_grid: (i32, i32),
    pub target_walkable: bool,
    pub skip_reachability_check: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct ScoredDelegationCandidate {
    pub candidate: DelegationCandidate,
    pub priority: i32,
    pub pos: Vec2,
    pub dist_sq: f32,
    pub policy_contributions: PolicyScoreContributions,
}

/// `collect_scored_candidates` に渡す Familiar 固有の検索コンテキスト。
pub struct FamiliarSearchContext<'a> {
    pub fam_entity: Entity,
    pub fam_pos: Vec2,
    pub task_area_opt: Option<&'a TaskArea>,
}

/// Familiar の候補母集団を構成する空間 index と所有集合。
pub struct FamiliarCandidateSources<'a> {
    pub designation_grid: &'a DesignationSpatialGrid,
    pub transport_request_grid: &'a TransportRequestSpatialGrid,
    pub managed_tasks: &'a ManagedTasks,
    pub world_map: &'a WorldMap,
}

fn include_in_global_designation_scan(work_type: WorkType, is_managed_by_yard: bool) -> bool {
    work_type == WorkType::Build || is_managed_by_yard
}

/// Familiar単位で委譲候補を収集し、スコア情報付きで返す
pub fn collect_scored_candidates(
    ctx: FamiliarSearchContext<'_>,
    queries: &FamiliarTaskAssignmentQueries,
    sources: FamiliarCandidateSources<'_>,
    q_target_blueprints: &Query<&TargetBlueprint>,
) -> Vec<ScoredDelegationCandidate> {
    collect_scored_candidates_internal(ctx, queries, sources, q_target_blueprints, None)
}

pub(crate) fn collect_scored_candidates_with_diagnostics(
    ctx: FamiliarSearchContext<'_>,
    queries: &FamiliarTaskAssignmentQueries,
    sources: FamiliarCandidateSources<'_>,
    q_target_blueprints: &Query<&TargetBlueprint>,
    diagnostics: &mut FamiliarEvaluatorDiagnostics,
    revisions: &TaskDiagnosticInputRevisions,
) -> Vec<ScoredDelegationCandidate> {
    collect_scored_candidates_internal(
        ctx,
        queries,
        sources,
        q_target_blueprints,
        Some((diagnostics, revisions)),
    )
}

fn collect_scored_candidates_internal(
    ctx: FamiliarSearchContext<'_>,
    queries: &FamiliarTaskAssignmentQueries,
    sources: FamiliarCandidateSources<'_>,
    q_target_blueprints: &Query<&TargetBlueprint>,
    mut diagnostics: Option<(
        &mut FamiliarEvaluatorDiagnostics,
        &TaskDiagnosticInputRevisions,
    )>,
) -> Vec<ScoredDelegationCandidate> {
    let all_yards: Vec<Yard> = queries.yards.iter().cloned().collect();

    let mut candidates = collect_candidate_entities(
        ctx.task_area_opt,
        &all_yards,
        sources.managed_tasks,
        sources.designation_grid,
        sources.transport_request_grid,
    );

    let mut seen: HashSet<Entity> = candidates.iter().copied().collect();
    for (entity, _, designation, managed_by_opt, _, _, _, _) in
        queries.designation.designations.iter()
    {
        let is_managed_by_yard =
            managed_by_opt.is_some_and(|managed_by| queries.yards.get(managed_by.0).is_ok());

        if include_in_global_designation_scan(designation.work_type, is_managed_by_yard)
            && seen.insert(entity)
        {
            candidates.push(entity);
        }
    }

    let mut valid_candidates = Vec::new();
    for entity in candidates {
        // Stale spatial entries are not members of the current candidate
        // universe and must not create a diagnostic row.
        if queries.designation.designations.get(entity).is_err() {
            continue;
        }
        if let Some((diagnostics, revisions)) = diagnostics.as_mut() {
            diagnostics.observe_applicable(entity, revisions);
        }

        let snapshot = match candidate_snapshot(
            ctx.fam_entity,
            entity,
            ctx.task_area_opt,
            &all_yards,
            sources.managed_tasks,
            sources.world_map,
            queries,
        ) {
            Ok(snapshot) => snapshot,
            Err(reason) => {
                if let Some((diagnostics, _)) = diagnostics.as_mut() {
                    diagnostics.reject(entity, reason);
                }
                continue;
            }
        };

        let work_type = snapshot.work_type;
        if work_type == WorkType::HaulWaterToMixer {
            debug!("TASK_FINDER: HaulWaterToMixer {:?} passed filter", entity);
        }

        let priority = match score_candidate(
            entity,
            work_type,
            snapshot.base_priority,
            snapshot.in_stockpile_none,
            queries,
            q_target_blueprints,
        ) {
            Ok(priority) => priority,
            Err(reason) => {
                if let Some((diagnostics, _)) = diagnostics.as_mut() {
                    diagnostics.reject(entity, reason);
                }
                continue;
            }
        };

        let dist_sq = snapshot.pos.distance_squared(ctx.fam_pos);

        if work_type == WorkType::HaulWaterToMixer {
            debug!(
                "TASK_FINDER: HaulWaterToMixer {:?} scored priority={} dist_sq={}",
                entity, priority, dist_sq
            );
        }

        valid_candidates.push(ScoredDelegationCandidate {
            candidate: DelegationCandidate {
                entity,
                work_type,
                target_grid: snapshot.target_grid,
                target_walkable: snapshot.target_walkable,
                skip_reachability_check: snapshot.skip_reachability_check,
            },
            priority,
            pos: snapshot.pos,
            dist_sq,
            policy_contributions: queries.receiver_policy_tiers.get(entity).map_or_else(
                |_| PolicyScoreContributions::default(),
                |tier| PolicyScoreContributions::new(transport_policy_units(tier.0), 0),
            ),
        });
    }

    if valid_candidates.is_empty() {
        debug!("TASK_FINDER: {:?} has no candidates", ctx.fam_entity);
    }

    valid_candidates
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::events::ResourceReservationRequest;
    use hw_core::familiar::Familiar;
    use hw_core::relationships::ManagedBy;
    use hw_jobs::events::TaskAssignmentRequest;
    use hw_jobs::{Blueprint, BuildingType, Designation, Priority, Rock, TaskSlots, Tree};
    use hw_logistics::SharedResourceCache;

    #[derive(Resource, Default)]
    struct CandidateProbe(Vec<Entity>);

    #[derive(Resource, Default)]
    struct MembershipProbe(
        std::collections::HashMap<Entity, (u16, Option<hw_jobs::TaskDiagnosticClass>)>,
    );

    fn capture_candidates(
        queries: FamiliarTaskAssignmentQueries,
        designation_grid: Res<DesignationSpatialGrid>,
        transport_request_grid: Res<TransportRequestSpatialGrid>,
        q_target_blueprints: Query<&TargetBlueprint>,
        familiars: Query<(Entity, &Transform, Option<&TaskArea>, &ManagedTasks), With<Familiar>>,
        mut probe: ResMut<CandidateProbe>,
    ) {
        let Ok((fam_entity, transform, task_area_opt, managed_tasks)) = familiars.single() else {
            return;
        };
        probe.0 = collect_scored_candidates(
            FamiliarSearchContext {
                fam_entity,
                fam_pos: transform.translation.truncate(),
                task_area_opt,
            },
            &queries,
            FamiliarCandidateSources {
                designation_grid: &designation_grid,
                transport_request_grid: &transport_request_grid,
                managed_tasks,
                world_map: queries.read.world_map.as_ref(),
            },
            &q_target_blueprints,
        )
        .into_iter()
        .map(|candidate| candidate.candidate.entity)
        .collect();
    }

    fn capture_candidate_membership(
        queries: FamiliarTaskAssignmentQueries,
        designation_grid: Res<DesignationSpatialGrid>,
        transport_request_grid: Res<TransportRequestSpatialGrid>,
        q_target_blueprints: Query<&TargetBlueprint>,
        familiars: Query<(Entity, &Transform, Option<&TaskArea>, &ManagedTasks), With<Familiar>>,
        revisions: Res<TaskDiagnosticInputRevisions>,
        mut probe: ResMut<MembershipProbe>,
    ) {
        let mut cycle = super::super::FamiliarTaskDiagnosticCycle::new(1, &revisions);
        for (fam_entity, transform, task_area_opt, managed_tasks) in &familiars {
            cycle.begin_evaluator();
            let mut diagnostics = FamiliarEvaluatorDiagnostics::new(1);
            collect_scored_candidates_with_diagnostics(
                FamiliarSearchContext {
                    fam_entity,
                    fam_pos: transform.translation.truncate(),
                    task_area_opt,
                },
                &queries,
                FamiliarCandidateSources {
                    designation_grid: &designation_grid,
                    transport_request_grid: &transport_request_grid,
                    managed_tasks,
                    world_map: queries.read.world_map.as_ref(),
                },
                &q_target_blueprints,
                &mut diagnostics,
                &revisions,
            );
            cycle.finish_evaluator(diagnostics);
        }

        let mut published = super::super::FamiliarTaskCandidateDiagnostics::default();
        published.publish(cycle);
        probe.0 = queries
            .designation
            .designations
            .iter()
            .filter_map(|(entity, ..)| {
                published.record(entity).map(|record| {
                    (
                        entity,
                        (
                            record.coverage.applicable_evaluators,
                            record.counters.representative(),
                        ),
                    )
                })
            })
            .collect();
    }

    #[test]
    fn yard_owned_gather_designations_use_global_scan() {
        assert!(include_in_global_designation_scan(WorkType::Chop, true));
        assert!(include_in_global_designation_scan(WorkType::Mine, true));
        assert!(!include_in_global_designation_scan(WorkType::Chop, false));
    }

    #[test]
    fn build_designations_keep_global_scan_fallback() {
        assert!(include_in_global_designation_scan(WorkType::Build, false));
    }

    #[test]
    fn yard_owned_remote_gather_designations_reach_familiar_candidates() {
        let mut app = App::new();
        app.init_resource::<WorldMap>()
            .init_resource::<SharedResourceCache>()
            .init_resource::<hw_logistics::transport_request::WheelbarrowArbitrationDiagnostics>()
            .init_resource::<DesignationSpatialGrid>()
            .init_resource::<TransportRequestSpatialGrid>()
            .init_resource::<CandidateProbe>()
            .add_message::<ResourceReservationRequest>()
            .add_message::<TaskAssignmentRequest>()
            .add_systems(Update, capture_candidates);

        let yard = app
            .world_mut()
            .spawn(Yard {
                min: Vec2::ZERO,
                max: Vec2::splat(32.0),
            })
            .id();
        app.world_mut().spawn((
            Familiar::default(),
            Transform::default(),
            ManagedTasks::default(),
        ));
        let chop = app
            .world_mut()
            .spawn((
                Transform::from_xyz(640.0, 640.0, 0.0),
                Designation {
                    work_type: WorkType::Chop,
                },
                ManagedBy(yard),
                TaskSlots::new(1),
                Priority::default(),
                Tree,
            ))
            .id();
        let mine = app
            .world_mut()
            .spawn((
                Transform::from_xyz(672.0, 640.0, 0.0),
                Designation {
                    work_type: WorkType::Mine,
                },
                ManagedBy(yard),
                TaskSlots::new(1),
                Priority::default(),
                Rock,
            ))
            .id();

        app.update();

        let candidates = &app.world().resource::<CandidateProbe>().0;
        assert!(candidates.contains(&chop));
        assert!(candidates.contains(&mine));
    }

    #[test]
    fn diagnostic_membership_uses_each_real_candidate_universe() {
        let mut app = App::new();
        app.init_resource::<WorldMap>()
            .init_resource::<SharedResourceCache>()
            .init_resource::<hw_logistics::transport_request::WheelbarrowArbitrationDiagnostics>()
            .init_resource::<DesignationSpatialGrid>()
            .init_resource::<TransportRequestSpatialGrid>()
            .init_resource::<TaskDiagnosticInputRevisions>()
            .init_resource::<MembershipProbe>()
            .add_message::<ResourceReservationRequest>()
            .add_message::<TaskAssignmentRequest>()
            .add_systems(Update, capture_candidate_membership);

        let first_area = TaskArea::from_points(Vec2::ZERO, Vec2::splat(128.0));
        let second_area = TaskArea::from_points(Vec2::splat(1024.0), Vec2::splat(1152.0));
        app.world_mut().spawn((
            Familiar::default(),
            Transform::from_xyz(32.0, 32.0, 0.0),
            first_area,
            ManagedTasks::default(),
        ));
        app.world_mut().spawn((
            Familiar::default(),
            Transform::from_xyz(1056.0, 1056.0, 0.0),
            second_area,
            ManagedTasks::default(),
        ));

        let local = app
            .world_mut()
            .spawn((
                Transform::from_xyz(64.0, 64.0, 0.0),
                Designation {
                    work_type: WorkType::Chop,
                },
                Tree,
            ))
            .id();
        app.world_mut()
            .resource_mut::<DesignationSpatialGrid>()
            .data_mut()
            .insert(local, Vec2::splat(64.0));

        let build = app
            .world_mut()
            .spawn((
                Transform::from_xyz(512.0, 512.0, 0.0),
                Designation {
                    work_type: WorkType::Build,
                },
                Blueprint::new(BuildingType::Wall, vec![(16, 16)]),
            ))
            .id();
        let yard = app
            .world_mut()
            .spawn(Yard {
                min: Vec2::splat(480.0),
                max: Vec2::splat(544.0),
            })
            .id();
        let yard_owned = app
            .world_mut()
            .spawn((
                Transform::from_xyz(520.0, 520.0, 0.0),
                Designation {
                    work_type: WorkType::Chop,
                },
                ManagedBy(yard),
                Tree,
            ))
            .id();

        app.update();

        let membership = &app.world().resource::<MembershipProbe>().0;
        assert_eq!(membership.get(&local).map(|entry| entry.0), Some(1));
        assert_eq!(membership.get(&build).map(|entry| entry.0), Some(2));
        assert_eq!(membership.get(&yard_owned).map(|entry| entry.0), Some(2));
        assert_eq!(
            membership.get(&build).and_then(|entry| entry.1),
            Some(hw_jobs::TaskDiagnosticClass::DependencyWaiting)
        );
    }
}
