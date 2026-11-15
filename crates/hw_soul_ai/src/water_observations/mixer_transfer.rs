//! Passive branch-local quantity conversion. Never fixes or supplies water.
use super::TankPourReceipts;
use bevy::prelude::*;
use hw_core::{
    WorldEpoch,
    constants::BUCKET_CAPACITY,
    relationships::{DeliveringTo, LoadedIn, StoredIn, StoredItems, WorkingOn},
};
use hw_jobs::{
    ActiveTaskIdentity, AssignedTask, Blueprint, BucketTransportDestination, BucketTransportPhase,
    BucketTransportSource, Building, BuildingType, WorkType,
};
use hw_logistics::{
    BelongsTo, Inventory, ResourceItem, ResourceType, Stockpile,
    transport_request::{TransportRequest, TransportRequestKind},
};

#[derive(Clone, Debug)]
pub struct TransferContext {
    pub worker: Entity,
    pub assignment: Entity,
    pub target: Entity,
    pub tank: Entity,
    pub mixer: Entity,
    pub bucket: Entity,
    pub carried_before: Option<Entity>,
    pub world_epoch: u64,
    pub work_type: WorkType,
}

impl TransferContext {
    fn valid(&self, world: &World) -> bool {
        world.get_resource::<WorldEpoch>().map(|e| e.get()) == Some(self.world_epoch)
            && self.assignment == self.target
            && self.carried_before == Some(self.bucket)
            && self.work_type == WorkType::HaulWaterToMixer
            && [self.worker, self.target, self.tank, self.mixer, self.bucket]
                .into_iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                == 5
            && world
                .get::<hw_core::soul::DamnedSoul>(self.worker)
                .is_some()
            && [
                (self.tank, BuildingType::Tank),
                (self.mixer, BuildingType::MudMixer),
            ]
            .into_iter()
            .all(|(entity, kind)| {
                world.get::<Blueprint>(entity).is_none()
                    && world
                        .get::<Building>(entity)
                        .is_some_and(|b| b.kind == kind && !b.is_provisional)
            })
            && world
                .get::<Stockpile>(self.tank)
                .is_some_and(|s| s.capacity == 50 && s.resource_type == Some(ResourceType::Water))
            && world
                .get::<hw_jobs::mud_mixer::MudMixerStorage>(self.mixer)
                .is_some()
            && world
                .get::<BelongsTo>(self.bucket)
                .is_some_and(|r| r.0 == self.tank)
            && world.get::<LoadedIn>(self.bucket).is_none()
            && world.get::<StoredIn>(self.bucket).is_none()
            && world.get::<TransportRequest>(self.target).is_some_and(|r| {
                r.kind == TransportRequestKind::DeliverWaterToMixer
                    && r.anchor == self.mixer
                    && r.resource_type == ResourceType::Water
            })
    }

    fn active(&self, world: &World, returning: bool) -> bool {
        world.get::<Inventory>(self.worker).is_some_and(|i| i.0 == Some(self.bucket))
            && world.get::<WorkingOn>(self.worker).is_some_and(|r| r.0 == self.target)
            && world.get::<ActiveTaskIdentity>(self.worker).is_some_and(|i|
                i.assignment_entity == self.assignment && i.current_target_entity == self.target
                    && i.current_work_type == self.work_type && i.matches_working_on(Some(self.target)))
            && world.get::<AssignedTask>(self.worker).is_some_and(|task| matches!(task,
                AssignedTask::BucketTransport(data) if data.bucket == self.bucket
                    && matches!(data.source, BucketTransportSource::Tank {tank,..} if tank == self.tank)
                    && matches!(data.destination, BucketTransportDestination::Mixer(m) if m == self.mixer)
                    && if returning { matches!(data.phase,BucketTransportPhase::ReturningBucket) }
                       else { matches!(data.phase,BucketTransportPhase::GoingToDestination) }))
    }
}

#[derive(Clone, Debug)]
pub struct TankTakeReceipt {
    pub context: TransferContext,
    pub bucket_before: Option<ResourceType>,
    pub inputs: Vec<Entity>,
    pub inputs_before: Vec<WaterInputObservation>,
    /// The product currently consumes on Found OR Unreachable, but not Deferred.
    /// Preserve that observation; it must not be relabeled a successful route.
    pub route_found: bool,
}

#[derive(Clone, Debug)]
pub struct WaterInputObservation {
    pub entity: Entity,
    pub resource: ResourceType,
    pub stored_in: Option<Entity>,
    pub loaded_in: Option<Entity>,
}

impl TankTakeReceipt {
    pub fn queue(self, commands: &mut Commands) {
        commands.queue(move |world: &mut World| {
            if !world.contains_resource::<TankPourReceipts>() {
                return;
            }
            let c = &self.context;
            let valid = c.valid(world)
                && c.active(world, false)
                && self.bucket_before == Some(ResourceType::BucketEmpty)
                && !self.inputs.is_empty()
                && self.inputs.len() <= BUCKET_CAPACITY as usize
                && self.inputs_before.len() == self.inputs.len()
                && self
                    .inputs_before
                    .iter()
                    .zip(&self.inputs)
                    .all(|(item, &entity)| {
                        item.entity == entity
                            && item.resource == ResourceType::Water
                            && item.stored_in == Some(c.tank)
                            && item.loaded_in.is_none()
                    })
                && self
                    .inputs
                    .iter()
                    .copied()
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    == self.inputs.len()
                && self.inputs.iter().all(|&e| world.get_entity(e).is_err())
                && world.get::<AssignedTask>(c.worker).is_some_and(|task| {
                    matches!(task,
                    AssignedTask::BucketTransport(data) if data.amount as usize == self.inputs.len()
                        && matches!(data.source, BucketTransportSource::Tank {needs_fill:true,..}))
                })
                && world
                    .get::<ResourceItem>(c.bucket)
                    .is_some_and(|r| r.0 == ResourceType::BucketWater)
                && world
                    .get::<DeliveringTo>(c.bucket)
                    .is_some_and(|r| r.0 == c.mixer)
                && world
                    .get::<StoredItems>(c.tank)
                    .is_none_or(|items| !items.iter().any(|e| self.inputs.contains(&e)));
            let mut rows = world.resource_mut::<TankPourReceipts>();
            if !valid {
                rows.failure
                    .get_or_insert("Tank take post-commit context differs");
            } else if rows.total >= 4096 {
                rows.failure.get_or_insert("Water receipt cap exceeded");
            } else {
                rows.total += 1;
                rows.tank_takes.push(self);
            }
        });
    }
}

#[derive(Clone, Debug)]
pub struct MixerPourReceipt {
    pub context: TransferContext,
    pub bucket_before: Option<ResourceType>,
    pub amount: u32,
    pub before_count: usize,
    pub capacity: usize,
    pub outputs: Vec<Entity>,
    pub returning: bool,
}

impl MixerPourReceipt {
    pub fn queue(self, commands: &mut Commands) {
        commands.queue(move |world: &mut World| {
            if !world.contains_resource::<TankPourReceipts>() {
                return;
            }
            let c = &self.context;
            let valid = c.valid(world)
                && self.bucket_before == Some(ResourceType::BucketWater)
                && self.amount > 0
                && self.amount <= BUCKET_CAPACITY
                && self.before_count < self.capacity
                && self.outputs.len()
                    == (self.amount as usize).min(self.capacity.saturating_sub(self.before_count))
                && self
                    .outputs
                    .iter()
                    .copied()
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    == self.outputs.len()
                && world.get::<Stockpile>(c.mixer).is_some_and(|s| {
                    s.capacity == self.capacity && s.resource_type == Some(ResourceType::Water)
                })
                && self.outputs.iter().all(|&e| {
                    ![c.worker,c.target,c.tank,c.mixer,c.bucket].contains(&e) && world
                        .get::<ResourceItem>(e)
                        .is_some_and(|r| r.0 == ResourceType::Water)
                        && world.get::<StoredIn>(e).is_some_and(|r| r.0 == c.mixer)
                        && world.get::<LoadedIn>(e).is_none()
                        && world
                            .get::<StoredItems>(c.mixer)
                            .is_some_and(|items| items.iter().any(|i| i == e))
                })
                && world
                    .get::<ResourceItem>(c.bucket)
                    .is_some_and(|r| r.0 == ResourceType::BucketEmpty)
                && world.get::<DeliveringTo>(c.bucket).is_none()
                && if self.returning {
                    c.active(world, true) && world.get::<AssignedTask>(c.worker).is_some_and(|task|
                        matches!(task,AssignedTask::BucketTransport(data) if data.amount == self.amount))
                } else {
                    matches!(
                        world.get::<AssignedTask>(c.worker),
                        Some(AssignedTask::None)
                    ) && world
                        .get::<Inventory>(c.worker)
                        .is_some_and(|i| i.0.is_none())
                        && world.get::<WorkingOn>(c.worker).is_none()
                        && world.get::<ActiveTaskIdentity>(c.worker).is_none()
                };
            let mut rows = world.resource_mut::<TankPourReceipts>();
            if !valid {
                rows.failure
                    .get_or_insert("Mixer pour post-commit context differs");
            } else if rows.total >= 4096 {
                rows.failure.get_or_insert("Water receipt cap exceeded");
            } else {
                rows.total += 1;
                rows.mixer_pours.push(self);
            }
        });
    }
}
