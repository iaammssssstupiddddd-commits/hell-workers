//! Opt-in cancellation transaction witnesses. Never drives or repairs gameplay.
use bevy::prelude::*;
use hw_core::{
    WorldEpoch,
    relationships::{StoredIn, WorkingOn},
    soul::DamnedSoul,
};
use hw_jobs::{ActiveTaskIdentity, AssignedTask, Blueprint, BuildingType, TargetBlueprint};
use hw_logistics::{
    PendingBelongsToBlueprint, ResourceItem, ResourceType, transport_request::TransportRequest,
};
use serde_json::{Value, json};

use super::task_targets_blueprint;

const LIMIT: usize = 4096;
const PENDING_LIMIT: usize = 64;
const WAIT_FRAMES: u32 = 300;

#[derive(Resource, Default)]
pub(crate) struct CancellationReceipts {
    pending: Vec<CancellationDraft>,
    total: usize,
    pub rows: Vec<Value>,
    pub failure: Option<String>,
}

pub(super) struct CancellationDraft {
    owner: Entity,
    kind: BuildingType,
    epoch: Option<u64>,
    footprint: Vec<(i32, i32)>,
    delivered: Vec<(ResourceType, u32)>,
    actors: Vec<Entity>,
    requests: Vec<Entity>,
    storages: Vec<Entity>,
    stockpile_grids: Vec<(i32, i32)>,
    released_items: Vec<Entity>,
    refunds: Vec<(Entity, ResourceType)>,
    overflow: bool,
    waited: u32,
    transaction_index: usize,
}

fn bounded_push<T>(values: &mut Vec<T>, value: T, overflow: &mut bool) {
    if values.len() < LIMIT {
        values.push(value);
    } else {
        *overflow = true;
    }
}

impl CancellationDraft {
    pub(super) fn new(
        owner: Entity,
        blueprint: &Blueprint,
        epoch: Option<&WorldEpoch>,
    ) -> Option<Self> {
        if !matches!(
            blueprint.kind,
            BuildingType::Tank
                | BuildingType::MudMixer
                | BuildingType::RestArea
                | BuildingType::Door
                | BuildingType::WheelbarrowParking
                | BuildingType::SandPile
                | BuildingType::BonePile
                | BuildingType::OutdoorLamp
        ) {
            return None;
        }
        let mut delivered: Vec<_> = blueprint
            .delivered_materials
            .iter()
            .map(|(&k, &v)| (k, v))
            .collect();
        delivered.sort_unstable_by_key(|(kind, _)| super::resource_order(*kind));
        Some(Self {
            owner,
            kind: blueprint.kind,
            epoch: epoch.map(|epoch| epoch.get()),
            footprint: blueprint
                .occupied_grids
                .iter()
                .copied()
                .take(LIMIT)
                .collect(),
            delivered,
            actors: Vec::new(),
            requests: Vec::new(),
            storages: Vec::new(),
            stockpile_grids: Vec::new(),
            released_items: Vec::new(),
            refunds: Vec::new(),
            overflow: blueprint.occupied_grids.len() > LIMIT,
            waited: 0,
            transaction_index: 0,
        })
    }

    pub(super) fn actor(&mut self, entity: Entity) {
        bounded_push(&mut self.actors, entity, &mut self.overflow);
    }
    pub(super) fn request(&mut self, entity: Entity) {
        bounded_push(&mut self.requests, entity, &mut self.overflow);
    }
    pub(super) fn storage(&mut self, entity: Entity) {
        bounded_push(&mut self.storages, entity, &mut self.overflow);
    }
    pub(super) fn stockpile_grid(&mut self, grid: (i32, i32)) {
        bounded_push(&mut self.stockpile_grids, grid, &mut self.overflow);
    }
    pub(super) fn released_item(&mut self, entity: Entity) {
        bounded_push(&mut self.released_items, entity, &mut self.overflow);
    }
    pub(super) fn refund(&mut self, entity: Entity, kind: ResourceType) {
        bounded_push(&mut self.refunds, (entity, kind), &mut self.overflow);
    }

    pub(super) fn queue(mut self, commands: &mut Commands) {
        // This command follows the original refund spawns and owner despawn.
        commands.queue(move |world: &mut World| {
            let Some(mut trace) = world.get_resource_mut::<CancellationReceipts>() else {
                return;
            };
            if self.overflow || trace.total >= LIMIT || trace.pending.len() >= PENDING_LIMIT {
                trace
                    .failure
                    .get_or_insert("cancellation witness cap exceeded".into());
            } else {
                self.transaction_index = trace.total;
                trace.total += 1;
                trace.pending.push(self);
            }
        });
    }

    fn ready(&self, world: &mut World) -> Result<bool, &'static str> {
        let current = world.get_resource::<WorldEpoch>().map(|epoch| epoch.get());
        if self.epoch.is_none() || self.epoch != current {
            return Err("cancellation witness world epoch differs");
        }
        let Some(map) = world.get_resource::<hw_world::WorldMap>() else {
            return Err("cancellation witness map missing");
        };
        if world.get_entity(self.owner).is_ok()
            || self
                .requests
                .iter()
                .chain(&self.storages)
                .any(|&entity| world.get_entity(entity).is_ok())
            || map
                .building_entries()
                .any(|(_, &owner)| owner == self.owner)
            || map
                .stockpile_entries()
                .any(|(_, owner)| self.storages.contains(owner))
        {
            return Ok(false);
        }
        if self.released_items.iter().any(|&entity| {
            world.get_entity(entity).is_err()
                || world.get::<StoredIn>(entity).is_some()
                || world.get::<Visibility>(entity) != Some(&Visibility::Visible)
        }) {
            return Ok(false);
        }
        for &entity in &self.actors {
            let Some(task) = world.get::<AssignedTask>(entity) else {
                return Err("cancellation witness actor missing");
            };
            if task_targets_blueprint(task, self.owner)
                || world.get::<WorkingOn>(entity).is_some_and(|target| {
                    target.0 == self.owner || self.requests.contains(&target.0)
                })
                || world
                    .get::<ActiveTaskIdentity>(entity)
                    .is_some_and(|target| {
                        target.current_target_entity == self.owner
                            || self.requests.contains(&target.current_target_entity)
                            || target.assignment_entity == self.owner
                            || self.requests.contains(&target.assignment_entity)
                    })
            {
                return Ok(false);
            }
        }
        let mut tasks = world.query_filtered::<(
            Option<&AssignedTask>,
            Option<&WorkingOn>,
            Option<&ActiveTaskIdentity>,
        ), With<DamnedSoul>>();
        if tasks.iter(world).any(|(task, working, identity)| {
            task.is_some_and(|task| task_targets_blueprint(task, self.owner))
                || working.is_some_and(|target| self.closed_target(target.0))
                || identity.is_some_and(|target| {
                    self.closed_target(target.current_target_entity)
                        || self.closed_target(target.assignment_entity)
                })
        }) {
            return Ok(false);
        }
        let mut requests = world.query::<(&TransportRequest, Option<&TargetBlueprint>)>();
        if requests.iter(world).any(|(request, target)| {
            request.anchor == self.owner || target.is_some_and(|target| target.0 == self.owner)
        }) {
            return Ok(false);
        }
        let mut pending = world.query::<&PendingBelongsToBlueprint>();
        if pending.iter(world).any(|pending| pending.0 == self.owner) {
            return Ok(false);
        }
        let expected = self
            .delivered
            .iter()
            .try_fold(0_u64, |sum, (_, count)| sum.checked_add(u64::from(*count)))
            .ok_or("cancellation refund amount overflow")?;
        if expected != self.refunds.len() as u64 {
            return Err("cancellation refund spawn count differs");
        }
        for &(kind, count) in &self.delivered {
            if self
                .refunds
                .iter()
                .filter(|(_, value)| *value == kind)
                .count() as u64
                != u64::from(count)
            {
                return Err("cancellation refund type counts differ");
            }
        }
        if self.refunds.iter().any(|(entity, kind)| {
            world
                .get::<ResourceItem>(*entity)
                .is_none_or(|item| item.0 != *kind)
        }) {
            return Err("cancellation refund entity missing or changed");
        }
        Ok(true)
    }

    fn closed_target(&self, entity: Entity) -> bool {
        entity == self.owner || self.requests.contains(&entity) || self.storages.contains(&entity)
    }

    fn row(&self, world: &World) -> Value {
        let map = world.resource::<hw_world::WorldMap>();
        let ids = |values: &[Entity]| {
            values
                .iter()
                .map(|entity| format!("{entity:?}"))
                .collect::<Vec<_>>()
        };
        json!({"producer": "blueprint_cancellation_system", "operation": "BlueprintCancellationApplied", "result": "OwnerCleanupApplied",
            "transaction_index": self.transaction_index, "world_epoch": self.epoch, "owner": format!("{:?}", self.owner),
            "owner_bits": self.owner.to_bits(),
            "kind": format!("{:?}", self.kind), "footprint": self.footprint,
            "footprint_walkability": self.footprint.iter().map(|&(x,y)| map.is_walkable(x,y)).collect::<Vec<_>>(),
            "delivered_materials": self.delivered.iter().map(|(kind,count)| json!({"kind": format!("{kind:?}"), "count": count})).collect::<Vec<_>>(),
            "refund_items": self.refunds.iter().map(|(entity,kind)| json!({"entity": format!("{entity:?}"), "kind": format!("{kind:?}"), "count": 1})).collect::<Vec<_>>(),
            "released_actors": ids(&self.actors), "closed_requests": ids(&self.requests), "removed_storages": ids(&self.storages),
            "released_items": ids(&self.released_items), "cleared_stockpile_grids": self.stockpile_grids})
    }
}

/// Runs in Last, after ordinary deferred commands and task unassignment.
/// Missing completion is bounded and fails closed; it never performs cleanup.
pub(crate) fn finalize_cancellation_receipts(world: &mut World) {
    world.resource_scope(|world, mut trace: Mut<CancellationReceipts>| {
        let pending = std::mem::take(&mut trace.pending);
        for mut draft in pending {
            if trace.failure.is_some() {
                break;
            }
            match draft.ready(world) {
                Ok(true) => {
                    trace.rows.push(draft.row(world));
                }
                Ok(false) if draft.waited < WAIT_FRAMES => {
                    draft.waited += 1;
                    trace.pending.push(draft);
                }
                Ok(false) => {
                    trace.failure =
                        Some("cancellation cleanup did not reach terminal state".into());
                }
                Err(error) => {
                    trace.failure = Some(error.into());
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::super::blueprint_cancellation_system;
    use super::*;
    use bevy::ecs::schedule::ApplyDeferred;
    use hw_core::events::SoulTaskUnassignRequest;
    use hw_jobs::{BuildData, BuildPhase};
    use hw_logistics::ResourceItemVisualHandles;

    fn app(tracing: bool) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<hw_world::WorldMap>()
            .init_resource::<WorldEpoch>()
            .insert_resource(ResourceItemVisualHandles {
                icon_bone_small: default(),
                icon_wood_small: default(),
                icon_rock_small: default(),
                icon_sand_small: default(),
                icon_stasis_mud_small: default(),
            })
            .add_message::<SoulTaskUnassignRequest>()
            .add_systems(
                Update,
                (blueprint_cancellation_system, ApplyDeferred).chain(),
            );
        if tracing {
            app.init_resource::<CancellationReceipts>()
                .add_systems(Last, finalize_cancellation_receipts);
        }
        app
    }

    fn blueprint(app: &mut App, materials: &[(ResourceType, u32)]) -> Entity {
        let mut blueprint =
            Blueprint::new(BuildingType::Tank, vec![(2, 3), (3, 3), (2, 4), (3, 4)]);
        blueprint.delivered_materials = materials.iter().copied().collect();
        let footprint = blueprint.occupied_grids.clone();
        let owner = app
            .world_mut()
            .spawn((
                Transform::default(),
                blueprint,
                hw_jobs::BlueprintCancelRequested,
            ))
            .id();
        app.world_mut()
            .resource_mut::<hw_world::WorldMap>()
            .reserve_building_footprint(BuildingType::Tank, owner, footprint);
        owner
    }

    #[test]
    fn cancellation_records_real_refund_ids_once_after_deferred_cleanup() {
        let mut app = app(true);
        let owner = blueprint(
            &mut app,
            &[(ResourceType::Wood, 2), (ResourceType::Rock, 1)],
        );
        let storage = app.world_mut().spawn(PendingBelongsToBlueprint(owner)).id();
        app.world_mut()
            .resource_mut::<hw_world::WorldMap>()
            .register_stockpile_tile((4, 4), storage);
        let item = app
            .world_mut()
            .spawn((
                ResourceItem(ResourceType::BucketEmpty),
                StoredIn(storage),
                Visibility::Hidden,
            ))
            .id();
        app.update();
        let trace = app.world().resource::<CancellationReceipts>();
        assert_eq!(trace.failure, None);
        assert_eq!(trace.rows.len(), 1);
        let row = &trace.rows[0];
        assert_eq!(row["operation"], "BlueprintCancellationApplied");
        assert_eq!(row["owner"], format!("{owner:?}"));
        assert_eq!(row["refund_items"].as_array().unwrap().len(), 3);
        let mut actual = app.world_mut().query::<(Entity, &ResourceItem)>();
        for (entity, kind) in actual
            .iter(app.world())
            .filter(|(entity, _)| *entity != item)
        {
            assert!(
                app.world().resource::<CancellationReceipts>().rows[0]["refund_items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|value| value["entity"] == format!("{entity:?}")
                        && value["kind"] == format!("{:?}", kind.0))
            );
        }
        assert!(app.world().get_entity(owner).is_err());
        assert!(app.world().get_entity(storage).is_err());
        assert!(app.world().get::<StoredIn>(item).is_none());
        assert_eq!(
            app.world().get::<Visibility>(item),
            Some(&Visibility::Visible)
        );
        assert_eq!(
            app.world()
                .resource::<hw_world::WorldMap>()
                .stockpile_entity((4, 4)),
            None
        );
        app.update();
        assert_eq!(app.world().resource::<CancellationReceipts>().rows.len(), 1);
    }

    #[test]
    fn request_and_despawn_alone_do_not_report_actor_cleanup() {
        let mut app = app(true);
        let owner = blueprint(&mut app, &[]);
        let actor = app
            .world_mut()
            .spawn((
                DamnedSoul::default(),
                AssignedTask::Build(BuildData {
                    blueprint: owner,
                    phase: BuildPhase::GoingToBlueprint,
                }),
                WorkingOn(owner),
            ))
            .id();
        app.update();
        assert!(app.world().get_entity(owner).is_err());
        assert!(
            app.world()
                .resource::<CancellationReceipts>()
                .rows
                .is_empty()
        );
        assert!(matches!(
            app.world().get::<AssignedTask>(actor),
            Some(AssignedTask::Build(_))
        ));
        // Model the separate owner completing its work; the witness never does this.
        app.world_mut()
            .entity_mut(actor)
            .insert(AssignedTask::None)
            .remove::<WorkingOn>();
        app.update();
        assert_eq!(app.world().resource::<CancellationReceipts>().rows.len(), 1);
    }

    #[test]
    fn stale_epoch_and_missing_refund_fail_closed() {
        for stale in [false, true] {
            let mut app = app(true);
            let owner = blueprint(&mut app, &[(ResourceType::Wood, 1)]);
            app.world_mut().run_schedule(Update);
            if stale {
                app.world_mut().resource_mut::<WorldEpoch>().advance();
            } else {
                let mut items = app.world_mut().query::<(Entity, &ResourceItem)>();
                let item = items.iter(app.world()).next().unwrap().0;
                app.world_mut().despawn(item);
            }
            finalize_cancellation_receipts(app.world_mut());
            let trace = app.world().resource::<CancellationReceipts>();
            assert!(trace.rows.is_empty());
            assert!(trace.failure.is_some());
            assert!(app.world().get_entity(owner).is_err());
        }
    }

    #[test]
    fn actor_identity_is_checked_even_without_an_assigned_task_component() {
        let mut app = app(true);
        let owner = blueprint(&mut app, &[]);
        let actor = app
            .world_mut()
            .spawn((
                DamnedSoul::default(),
                ActiveTaskIdentity::new(owner, owner, hw_jobs::WorkType::Build),
            ))
            .id();
        app.update();
        assert!(app.world().get_entity(owner).is_err());
        assert!(
            app.world()
                .resource::<CancellationReceipts>()
                .rows
                .is_empty()
        );
        assert!(app.world().get::<ActiveTaskIdentity>(actor).is_some());
        app.world_mut()
            .entity_mut(actor)
            .remove::<ActiveTaskIdentity>();
        app.update();
        assert_eq!(app.world().resource::<CancellationReceipts>().rows.len(), 1);
    }

    #[test]
    fn working_on_is_checked_even_without_an_assigned_task_component() {
        let mut app = app(true);
        let owner = blueprint(&mut app, &[]);
        app.world_mut().run_schedule(Update);
        let actor = app.world_mut().spawn(DamnedSoul::default()).id();
        // Inject a stale relation in the unit fixture: normal despawn hooks
        // already remove WorkingOn(owner), so ordinary insertion cannot retain it.
        app.world_mut()
            .entity_mut(actor)
            .insert_with_relationship_hook_mode(
                WorkingOn(owner),
                bevy::ecs::relationship::RelationshipHookMode::Skip,
            );
        finalize_cancellation_receipts(app.world_mut());
        assert!(app.world().get_entity(owner).is_err());
        assert!(
            app.world()
                .resource::<CancellationReceipts>()
                .rows
                .is_empty()
        );
        assert!(app.world().get::<WorkingOn>(actor).is_some());
        app.world_mut().entity_mut(actor).remove::<WorkingOn>();
        app.update();
        assert_eq!(app.world().resource::<CancellationReceipts>().rows.len(), 1);
    }

    #[test]
    fn unsupported_refund_is_not_synthesized() {
        let mut app = app(true);
        blueprint(&mut app, &[(ResourceType::Water, 1)]);
        app.update();
        let trace = app.world().resource::<CancellationReceipts>();
        assert!(trace.rows.is_empty());
        assert_eq!(
            trace.failure.as_deref(),
            Some("cancellation refund spawn count differs")
        );
    }

    #[test]
    fn ordinary_cancellation_never_creates_the_opt_in_resource() {
        let mut app = app(false);
        let owner = blueprint(&mut app, &[(ResourceType::Wood, 1)]);
        app.update();
        assert!(app.world().get_entity(owner).is_err());
        assert!(!app.world().contains_resource::<CancellationReceipts>());
        let mut items = app.world_mut().query::<&ResourceItem>();
        assert_eq!(items.iter(app.world()).count(), 1);
    }

    #[test]
    fn cap_and_cleanup_deadline_never_create_success() {
        let mut app = app(true);
        app.world_mut().resource_mut::<CancellationReceipts>().total = LIMIT;
        blueprint(&mut app, &[]);
        app.update();
        assert!(
            app.world()
                .resource::<CancellationReceipts>()
                .rows
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<CancellationReceipts>()
                .failure
                .is_some()
        );
        let mut app = self::app(true);
        let owner = blueprint(&mut app, &[]);
        app.world_mut().spawn((
            DamnedSoul::default(),
            AssignedTask::Build(BuildData {
                blueprint: owner,
                phase: BuildPhase::GoingToBlueprint,
            }),
        ));
        app.world_mut().run_schedule(Update);
        for _ in 0..=WAIT_FRAMES {
            finalize_cancellation_receipts(app.world_mut());
        }
        assert!(
            app.world()
                .resource::<CancellationReceipts>()
                .rows
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<CancellationReceipts>()
                .failure
                .is_some()
        );
    }

    #[test]
    fn transaction_indices_remain_unique_across_drains() {
        let mut app = app(true);
        blueprint(&mut app, &[]);
        app.update();
        assert_eq!(
            app.world_mut()
                .resource_mut::<CancellationReceipts>()
                .rows
                .remove(0)["transaction_index"],
            0
        );
        blueprint(&mut app, &[]);
        app.update();
        assert_eq!(
            app.world().resource::<CancellationReceipts>().rows[0]["transaction_index"],
            1
        );
        let unsupported = Blueprint::new(BuildingType::Bridge, vec![(2, 3)]);
        assert!(
            CancellationDraft::new(
                Entity::PLACEHOLDER,
                &unsupported,
                Some(&WorldEpoch::default())
            )
            .is_none()
        );
    }
}
