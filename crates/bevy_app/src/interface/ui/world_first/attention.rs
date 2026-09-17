//! Current terminal blockers, grouped only through verified construction owners.
use super::super::panels::task_list::TaskListState;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_jobs::construction::{
    FloorTileBlueprint, TargetFloorConstructionSite, TargetWallConstructionSite, WallTileBlueprint,
};
use hw_jobs::{
    Blueprint, FloorConstructionSite, TargetBlueprint, TargetSoulSpaSite, WallConstructionSite,
};
use hw_logistics::transport_request::{TransportRequest, TransportRequestKind};
use hw_ui::panels::task_list::TaskStatusSummary;

#[derive(Resource, Default, Debug, PartialEq, Eq)]
pub struct AttentionSummary {
    pub targets: Vec<Entity>,
    pub evaluating: usize,
}

type OwnerQuery<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static FloorTileBlueprint>,
        Option<&'static WallTileBlueprint>,
        Option<&'static TransportRequest>,
        Option<&'static TargetFloorConstructionSite>,
        Option<&'static TargetWallConstructionSite>,
        Option<&'static TargetBlueprint>,
        Option<&'static TargetSoulSpaSite>,
    ),
>;

type ChangedOwners = Or<(
    Changed<TargetFloorConstructionSite>,
    Changed<TargetWallConstructionSite>,
    Changed<TargetBlueprint>,
    Changed<TargetSoulSpaSite>,
    Added<FloorConstructionSite>,
    Added<WallConstructionSite>,
    Added<Blueprint>,
    Changed<hw_energy::SoulSpaSite>,
)>;

#[derive(SystemParam)]
pub(crate) struct AttentionInputs<'w, 's> {
    owners: OwnerQuery<'w, 's>,
    floors: Query<'w, 's, (), With<FloorConstructionSite>>,
    walls: Query<'w, 's, (), With<WallConstructionSite>>,
    blueprints: Query<'w, 's, (), With<Blueprint>>,
    spas: Query<'w, 's, &'static hw_energy::SoulSpaSite>,
    changed: Query<'w, 's, (), ChangedOwners>,
    removed_floor: RemovedComponents<'w, 's, FloorConstructionSite>,
    removed_wall: RemovedComponents<'w, 's, WallConstructionSite>,
    removed_blueprint: RemovedComponents<'w, 's, Blueprint>,
    removed_spa: RemovedComponents<'w, 's, hw_energy::SoulSpaSite>,
    removed_floor_target: RemovedComponents<'w, 's, TargetFloorConstructionSite>,
    removed_wall_target: RemovedComponents<'w, 's, TargetWallConstructionSite>,
    removed_blueprint_target: RemovedComponents<'w, 's, TargetBlueprint>,
    removed_spa_target: RemovedComponents<'w, 's, TargetSoulSpaSite>,
}

impl AttentionInputs<'_, '_> {
    fn owner(&self, entity: Entity) -> Entity {
        let Ok((floor, wall, request, floor_target, wall_target, blueprint_target, spa_target)) =
            self.owners.get(entity)
        else {
            return entity;
        };
        if let Some(floor) = floor
            && self.floors.contains(floor.parent_site)
        {
            return floor.parent_site;
        }
        if let Some(wall) = wall
            && self.walls.contains(wall.parent_site)
        {
            return wall.parent_site;
        }
        let Some(request) = request else {
            return entity;
        };
        let target = match request.kind {
            TransportRequestKind::DeliverToFloorConstruction => floor_target
                .map(|target| target.0)
                .filter(|target| self.floors.contains(*target)),
            TransportRequestKind::DeliverToWallConstruction => wall_target
                .map(|target| target.0)
                .filter(|target| self.walls.contains(*target)),
            TransportRequestKind::DeliverToBlueprint => blueprint_target
                .map(|target| target.0)
                .filter(|target| self.blueprints.contains(*target)),
            TransportRequestKind::DeliverToSoulSpa => {
                spa_target.map(|target| target.0).filter(|target| {
                    self.spas
                        .get(*target)
                        .is_ok_and(|site| site.phase == hw_energy::SoulSpaPhase::Constructing)
                })
            }
            _ => None,
        };
        target
            .filter(|target| *target == request.anchor)
            .unwrap_or(entity)
    }

    fn changed(&mut self) -> bool {
        // Drain every reader, including when an earlier input was removed.
        let removals = [
            self.removed_floor.read().count(),
            self.removed_wall.read().count(),
            self.removed_blueprint.read().count(),
            self.removed_spa.read().count(),
            self.removed_floor_target.read().count(),
            self.removed_wall_target.read().count(),
            self.removed_blueprint_target.read().count(),
            self.removed_spa_target.read().count(),
        ];
        !self.changed.is_empty() || removals.into_iter().any(|count| count > 0)
    }
}

pub(crate) fn update_attention_summary(
    tasks: Res<TaskListState>,
    mut inputs: AttentionInputs,
    mut summary: ResMut<AttentionSummary>,
) {
    let owners_changed = inputs.changed();
    if !tasks.is_changed() && !owners_changed {
        return;
    }
    let mut next = AttentionSummary::default();
    for entry in &tasks.snapshot {
        match entry.status {
            TaskStatusSummary::Blocked(_) => next.targets.push(inputs.owner(entry.entity)),
            TaskStatusSummary::PendingEvaluation => next.evaluating += 1,
            _ => {}
        }
    }
    next.targets.sort_unstable();
    next.targets.dedup();
    if *summary != next {
        *summary = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_ui::panels::task_list::{TaskActionCapabilities, TaskBlockerReason, TaskEntry};

    fn entry(entity: Entity, status: TaskStatusSummary) -> TaskEntry {
        TaskEntry {
            entity,
            work_type: hw_core::jobs::WorkType::Build,
            description: String::new(),
            priority: 0,
            worker_count: 0,
            status,
            actions: TaskActionCapabilities::READ_ONLY,
            related_owner: None,
            related_anchor: None,
        }
    }

    #[test]
    fn attention_groups_verified_owners_and_keeps_evaluation_separate() {
        let mut app = App::new();
        app.init_resource::<TaskListState>()
            .init_resource::<AttentionSummary>()
            .add_systems(Update, update_attention_summary);
        let site = app
            .world_mut()
            .spawn(FloorConstructionSite::new(
                hw_core::area::TaskArea::from_points(Vec2::ZERO, Vec2::splat(64.0)),
                Vec2::ZERO,
                2,
            ))
            .id();
        let first = app
            .world_mut()
            .spawn(FloorTileBlueprint::new(site, (0, 0)))
            .id();
        let second = app
            .world_mut()
            .spawn(FloorTileBlueprint::new(site, (1, 0)))
            .id();
        let request = app
            .world_mut()
            .spawn((
                TransportRequest {
                    kind: TransportRequestKind::DeliverToFloorConstruction,
                    anchor: site,
                    issued_by: site,
                    resource_type: hw_logistics::ResourceType::Bone,
                    priority: default(),
                    stockpile_group: Vec::new(),
                },
                TargetFloorConstructionSite(site),
            ))
            .id();
        let pending = app.world_mut().spawn_empty().id();
        let unrelated = app.world_mut().spawn_empty().id();
        let blocked = TaskStatusSummary::Blocked(TaskBlockerReason::MissingResourceOrSource);
        let mut unrelated_entry = entry(unrelated, blocked);
        // A generic related anchor is not proof of common construction ownership.
        unrelated_entry.related_anchor = Some(site);
        app.world_mut().resource_mut::<TaskListState>().snapshot = vec![
            entry(first, blocked),
            entry(second, blocked),
            entry(request, blocked),
            entry(pending, TaskStatusSummary::PendingEvaluation),
            unrelated_entry,
        ];
        app.update();
        let summary = app.world().resource::<AttentionSummary>();
        assert_eq!(summary.targets.len(), 2);
        assert!(summary.targets.contains(&site));
        assert!(summary.targets.contains(&unrelated));
        assert_eq!(summary.evaluating, 1);
        app.world_mut().despawn(site);
        app.update();
        let summary = app.world().resource::<AttentionSummary>();
        assert!(!summary.targets.contains(&site));
        assert_eq!(summary.targets.len(), 4);
        app.world_mut()
            .resource_mut::<TaskListState>()
            .snapshot
            .clear();
        app.update();
        assert_eq!(
            *app.world().resource::<AttentionSummary>(),
            AttentionSummary::default()
        );
    }
}
