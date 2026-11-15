//! Actual River filling branch observations, not proof of river path selection.
use super::TankPourReceipts;
use bevy::prelude::*;
use hw_core::{
    WorldEpoch,
    relationships::{DeliveringTo, LoadedIn, StoredIn, WorkingOn},
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
pub struct RiverFillReceipt {
    pub worker: Entity,
    pub assignment: Entity,
    pub target: Entity,
    pub tank: Entity,
    pub bucket: Entity,
    pub world_epoch: u64,
    pub bucket_before: Option<ResourceType>,
}

impl RiverFillReceipt {
    /// Called only after the existing River branch changed the bucket on Found.
    /// No world mutation other than bounded diagnostic storage is permitted.
    pub fn queue(self, commands: &mut Commands) {
        commands.queue(move |world: &mut World| {
            if !world.contains_resource::<TankPourReceipts>() { return; }
            let valid = world.get_resource::<WorldEpoch>().map(|e| e.get()) == Some(self.world_epoch)
                && self.bucket_before == Some(ResourceType::BucketEmpty)
                && self.assignment == self.target
                && [self.worker, self.target, self.tank, self.bucket].into_iter()
                    .collect::<std::collections::HashSet<_>>().len() == 4
                && world.get::<hw_core::soul::DamnedSoul>(self.worker).is_some()
                && world.get::<WorkingOn>(self.worker).is_some_and(|r| r.0 == self.target)
                && world.get::<ActiveTaskIdentity>(self.worker).is_some_and(|i|
                    i.assignment_entity == self.assignment && i.current_target_entity == self.target
                        && i.current_work_type == WorkType::GatherWater
                        && i.matches_working_on(Some(self.target)))
                && world.get::<TransportRequest>(self.target).is_some_and(|r|
                    r.kind == TransportRequestKind::GatherWaterToTank && r.anchor == self.tank
                        && r.resource_type == ResourceType::Water)
                && world.get::<AssignedTask>(self.worker).is_some_and(|task| matches!(task,
                    AssignedTask::BucketTransport(data) if data.bucket == self.bucket
                        && matches!(data.source, BucketTransportSource::River)
                        && matches!(data.destination, BucketTransportDestination::Tank(t) if t == self.tank)
                        && matches!(data.phase, BucketTransportPhase::GoingToDestination)))
                && world.get::<Inventory>(self.worker).is_some_and(|i| i.0 == Some(self.bucket))
                && world.get::<ResourceItem>(self.bucket).is_some_and(|i| i.0 == ResourceType::BucketWater)
                && world.get::<BelongsTo>(self.bucket).is_some_and(|r| r.0 == self.tank)
                && world.get::<DeliveringTo>(self.bucket).is_some_and(|r| r.0 == self.tank)
                && world.get::<LoadedIn>(self.bucket).is_none()
                && world.get::<StoredIn>(self.bucket).is_none()
                && world.get::<Blueprint>(self.tank).is_none()
                && world.get::<Building>(self.tank).is_some_and(|b| b.kind == BuildingType::Tank && !b.is_provisional)
                && world.get::<Stockpile>(self.tank).is_some_and(|s|
                    s.capacity == 50 && s.resource_type == Some(ResourceType::Water));
            let mut receipts = world.resource_mut::<TankPourReceipts>();
            if !valid {
                receipts.failure.get_or_insert("River fill post-commit context differs");
            } else if receipts.total >= 4096 {
                receipts.failure.get_or_insert("Water receipt cap exceeded");
            } else {
                receipts.total += 1;
                receipts.river_fills.push(self);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::world::CommandQueue;
    use hw_jobs::BucketTransportData;

    fn fixture() -> (World, RiverFillReceipt) {
        let mut world = World::new();
        world.init_resource::<WorldEpoch>();
        world.init_resource::<TankPourReceipts>();
        let tank = world
            .spawn((
                Building {
                    kind: BuildingType::Tank,
                    is_provisional: false,
                },
                Stockpile {
                    capacity: 50,
                    resource_type: Some(ResourceType::Water),
                },
            ))
            .id();
        let request = world
            .spawn(TransportRequest {
                kind: TransportRequestKind::GatherWaterToTank,
                anchor: tank,
                resource_type: ResourceType::Water,
                issued_by: tank,
                priority: hw_logistics::transport_request::TransportPriority::Normal,
                stockpile_group: vec![],
            })
            .id();
        let bucket = world
            .spawn((
                ResourceItem(ResourceType::BucketWater),
                BelongsTo(tank),
                DeliveringTo(tank),
            ))
            .id();
        let worker = world
            .spawn((
                hw_core::soul::DamnedSoul::default(),
                Inventory(Some(bucket)),
                WorkingOn(request),
                ActiveTaskIdentity::new(request, request, WorkType::GatherWater),
                AssignedTask::BucketTransport(BucketTransportData {
                    bucket,
                    source: BucketTransportSource::River,
                    destination: BucketTransportDestination::Tank(tank),
                    amount: 1,
                    phase: BucketTransportPhase::GoingToDestination,
                }),
            ))
            .id();
        (
            world,
            RiverFillReceipt {
                worker,
                assignment: request,
                target: request,
                tank,
                bucket,
                world_epoch: 0,
                bucket_before: Some(ResourceType::BucketEmpty),
            },
        )
    }

    fn apply(world: &mut World, receipt: RiverFillReceipt) {
        let mut queue = CommandQueue::default();
        receipt.queue(&mut Commands::new(&mut queue, world));
        queue.apply(world);
    }

    #[test]
    fn exact_river_fill_is_bounded_and_optional_without_repair() {
        let (mut world, receipt) = fixture();
        apply(&mut world, receipt.clone());
        assert!(world.resource::<TankPourReceipts>().failure.is_none());
        assert_eq!(world.resource::<TankPourReceipts>().river_fills.len(), 1);
        world.resource_mut::<TankPourReceipts>().total = 4096;
        apply(&mut world, receipt.clone());
        assert_eq!(
            world.resource::<TankPourReceipts>().failure,
            Some("Water receipt cap exceeded")
        );
        world.remove_resource::<TankPourReceipts>();
        apply(&mut world, receipt.clone());
        assert_eq!(
            world.get::<ResourceItem>(receipt.bucket).unwrap().0,
            ResourceType::BucketWater
        );
    }

    #[test]
    fn river_fill_rejects_foreign_missing_or_unapplied_context() {
        for case in 0..12 {
            let (mut world, mut receipt) = fixture();
            match case {
                0 => receipt.world_epoch += 1,
                1 => receipt.bucket_before = Some(ResourceType::BucketWater),
                2 => receipt.assignment = receipt.bucket,
                3 => {
                    world.entity_mut(receipt.worker).remove::<WorkingOn>();
                }
                4 => {
                    world.entity_mut(receipt.bucket).remove::<BelongsTo>();
                }
                5 => {
                    world
                        .entity_mut(receipt.bucket)
                        .insert(DeliveringTo(receipt.worker));
                }
                6 => {
                    world
                        .entity_mut(receipt.bucket)
                        .insert(StoredIn(receipt.tank));
                }
                7 => {
                    world.entity_mut(receipt.worker).insert(Inventory(None));
                }
                8 => {
                    world
                        .get_mut::<TransportRequest>(receipt.target)
                        .unwrap()
                        .resource_type = ResourceType::BucketWater;
                }
                9 => {
                    world.entity_mut(receipt.worker).insert(AssignedTask::None);
                }
                10 => {
                    world
                        .entity_mut(receipt.bucket)
                        .insert(ResourceItem(ResourceType::BucketEmpty));
                }
                _ => {
                    world.entity_mut(receipt.tank).remove::<Building>();
                }
            }
            apply(&mut world, receipt);
            assert_eq!(
                world.resource::<TankPourReceipts>().failure,
                Some("River fill post-commit context differs")
            );
            assert!(world.resource::<TankPourReceipts>().river_fills.is_empty());
        }
    }
}
