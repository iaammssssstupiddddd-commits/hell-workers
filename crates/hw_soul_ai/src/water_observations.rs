//! Opt-in observations of actual Tank pours; never a water/task producer.
use bevy::prelude::*;
use hw_core::{
    WorldEpoch,
    constants::BUCKET_CAPACITY,
    relationships::{LoadedIn, StoredIn, StoredItems},
};
use hw_jobs::{Blueprint, Building, BuildingType};
use hw_logistics::{Inventory, ResourceItem, ResourceType, Stockpile};

mod river_fill;
pub use river_fill::RiverFillReceipt;

#[derive(Resource, Default)]
pub struct TankPourReceipts {
    pub rows: Vec<TankPourReceipt>,
    pub river_fills: Vec<RiverFillReceipt>,
    pub failure: Option<&'static str>,
    total: usize,
}

#[derive(Clone, Debug)]
pub struct TankPourReceipt {
    pub worker: Entity,
    pub assignment: Entity,
    pub target: Entity,
    pub gather_water: bool,
    pub tank: Entity,
    pub bucket: Entity,
    pub world_epoch: u64,
    /// The actual type observed immediately before the existing branch writes it.
    pub bucket_before: Option<ResourceType>,
    pub river_source: bool,
    pub outputs: Vec<Entity>,
}

impl TankPourReceipt {
    /// Queue after water spawn and bucket cleanup commands. Do not repair state.
    pub fn queue(self, commands: &mut Commands) {
        commands.queue(move |world: &mut World| {
            if !world.contains_resource::<TankPourReceipts>() {
                return;
            }
            let unique: std::collections::HashSet<_> = self.outputs.iter().copied().collect();
            let valid = world.get_resource::<WorldEpoch>().map(|epoch| epoch.get())
                == Some(self.world_epoch)
                && self.bucket_before == Some(ResourceType::BucketWater)
                && self.river_source
                && self.gather_water
                && self.target == self.assignment
                && self.target != self.bucket
                && self.target != self.worker && self.target != self.tank
                && world.get::<hw_logistics::transport_request::TransportRequest>(self.target).is_some_and(|request| {
                    request.kind == hw_logistics::transport_request::TransportRequestKind::GatherWaterToTank
                        && request.anchor == self.tank && request.resource_type == ResourceType::Water
                })
                && self.worker != self.tank
                && self.bucket != self.worker
                && self.bucket != self.tank
                && world
                    .get::<hw_core::soul::DamnedSoul>(self.worker)
                    .is_some()
                && world.get::<Blueprint>(self.tank).is_none()
                && world
                    .get::<Building>(self.tank)
                    .is_some_and(|b| b.kind == BuildingType::Tank && !b.is_provisional)
                && world.get::<Stockpile>(self.tank).is_some_and(|s| {
                    s.resource_type == Some(ResourceType::Water) && s.capacity == 50
                })
                && world
                    .get::<ResourceItem>(self.bucket)
                    .is_some_and(|r| r.0 == ResourceType::BucketEmpty)
                && world.get::<LoadedIn>(self.bucket).is_none()
                && world.get::<StoredIn>(self.bucket).is_none()
                && world
                    .get::<hw_core::relationships::DeliveringTo>(self.bucket)
                    .is_none()
                && world
                    .get::<Inventory>(self.worker)
                    .is_some_and(|i| i.0 != Some(self.bucket))
                && self.outputs.len() == BUCKET_CAPACITY as usize
                && unique.len() == self.outputs.len()
                && self.outputs.iter().all(|&entity| {
                    entity != self.worker
                        && entity != self.tank
                        && entity != self.bucket
                        && world
                            .get::<ResourceItem>(entity)
                            .is_some_and(|r| r.0 == ResourceType::Water)
                        && world
                            .get::<StoredIn>(entity)
                            .is_some_and(|r| r.0 == self.tank)
                        && world.get::<LoadedIn>(entity).is_none()
                        && world
                            .get::<StoredItems>(self.tank)
                            .is_some_and(|items| items.iter().any(|item| item == entity))
                });
            let mut receipts = world.resource_mut::<TankPourReceipts>();
            if !valid {
                receipts
                    .failure
                    .get_or_insert("Tank pour post-commit context differs");
            } else if receipts.total >= 4096 {
                receipts
                    .failure
                    .get_or_insert("Tank pour receipt cap exceeded");
            } else {
                receipts.total += 1;
                receipts.rows.push(self);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::world::CommandQueue;

    fn fixture() -> (World, TankPourReceipt) {
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
                    resource_type: Some(ResourceType::Water),
                    capacity: 50,
                },
            ))
            .id();
        let bucket = world.spawn(ResourceItem(ResourceType::BucketEmpty)).id();
        let worker = world
            .spawn((hw_core::soul::DamnedSoul::default(), Inventory(None)))
            .id();
        let outputs = (0..BUCKET_CAPACITY)
            .map(|_| {
                world
                    .spawn((ResourceItem(ResourceType::Water), StoredIn(tank)))
                    .id()
            })
            .collect();
        let assignment = world
            .spawn(hw_logistics::transport_request::TransportRequest {
                kind: hw_logistics::transport_request::TransportRequestKind::GatherWaterToTank,
                anchor: tank,
                resource_type: ResourceType::Water,
                issued_by: worker,
                priority: hw_logistics::transport_request::TransportPriority::Normal,
                stockpile_group: Vec::new(),
            })
            .id();
        (
            world,
            TankPourReceipt {
                worker,
                assignment,
                target: assignment,
                gather_water: true,
                tank,
                bucket,
                world_epoch: 0,
                bucket_before: Some(ResourceType::BucketWater),
                river_source: true,
                outputs,
            },
        )
    }

    fn apply(world: &mut World, receipt: TankPourReceipt) {
        let mut queue = CommandQueue::default();
        receipt.queue(&mut Commands::new(&mut queue, world));
        queue.apply(world);
    }

    #[test]
    fn tank_pour_observes_only_real_post_commit_items_and_sticky_cap() {
        let (mut world, receipt) = fixture();
        apply(&mut world, receipt.clone());
        assert!(world.resource::<TankPourReceipts>().failure.is_none());
        assert_eq!(
            world.resource::<TankPourReceipts>().rows[0].outputs,
            receipt.outputs
        );
        world.resource_mut::<TankPourReceipts>().rows.clear();
        world.resource_mut::<TankPourReceipts>().total = 4096;
        apply(&mut world, receipt);
        assert_eq!(
            world.resource::<TankPourReceipts>().failure,
            Some("Tank pour receipt cap exceeded")
        );
    }

    #[test]
    fn tank_pour_rejects_missing_foreign_or_invented_commit_without_repair() {
        for case in 0..17 {
            let (mut world, mut receipt) = fixture();
            match case {
                0 => receipt.world_epoch += 1,
                1 => receipt.bucket_before = None,
                2 => receipt.bucket_before = Some(ResourceType::BucketEmpty),
                3 => receipt.outputs[1] = receipt.outputs[0],
                4 => {
                    world
                        .entity_mut(receipt.outputs[0])
                        .insert(StoredIn(receipt.worker));
                }
                5 => {
                    world
                        .entity_mut(receipt.outputs[0])
                        .insert(ResourceItem(ResourceType::Rock));
                }
                6 => {
                    world
                        .entity_mut(receipt.bucket)
                        .insert(ResourceItem(ResourceType::BucketWater));
                }
                7 => {
                    world
                        .entity_mut(receipt.worker)
                        .insert(Inventory(Some(receipt.bucket)));
                }
                8 => {
                    world.entity_mut(receipt.tank).remove::<Building>();
                }
                9 => receipt.target = receipt.tank,
                10 => receipt.gather_water = false,
                11 => {
                    receipt.outputs.pop();
                }
                12 => {
                    world
                        .entity_mut(receipt.target)
                        .remove::<hw_logistics::transport_request::TransportRequest>();
                }
                13 => {
                    world
                        .get_mut::<hw_logistics::transport_request::TransportRequest>(
                            receipt.target,
                        )
                        .unwrap()
                        .anchor = receipt.worker;
                }
                14 => {
                    world
                        .get_mut::<hw_logistics::transport_request::TransportRequest>(
                            receipt.target,
                        )
                        .unwrap()
                        .resource_type = ResourceType::Wood;
                }
                15 => receipt.assignment = receipt.bucket,
                _ => {
                    world
                        .get_mut::<hw_logistics::transport_request::TransportRequest>(
                            receipt.target,
                        )
                        .unwrap()
                        .resource_type = ResourceType::BucketWater
                }
            }
            let tank = receipt.tank;
            apply(&mut world, receipt);
            assert!(
                world.resource::<TankPourReceipts>().rows.is_empty(),
                "case {case}"
            );
            assert_eq!(
                world.resource::<TankPourReceipts>().failure,
                Some("Tank pour post-commit context differs")
            );
            assert_eq!(world.get::<Stockpile>(tank).unwrap().capacity, 50);
        }
        let (mut world, receipt) = fixture();
        world.remove_resource::<TankPourReceipts>();
        let count = world.entities().len();
        apply(&mut world, receipt);
        assert_eq!(world.entities().len(), count);
        assert!(!world.contains_resource::<TankPourReceipts>());
    }
}
