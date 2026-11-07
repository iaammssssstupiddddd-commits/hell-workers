//! Opt-in profiling receipts from real blueprint delivery branches.
//! This resource neither drives delivery nor grants lifecycle/release authority.
use bevy::prelude::*;
use hw_core::{WorldEpoch, logistics::ResourceType};

use crate::{Blueprint, BuildingType};

const CAP: usize = 4096;

#[derive(Resource, Default)]
pub struct BlueprintDeliveryReceipts {
    pub rows: Vec<BlueprintDeliveryReceipt>,
    pub failure: Option<&'static str>,
    total: usize,
}

#[derive(Clone, Debug)]
pub struct BlueprintDeliveryReceipt {
    pub worker: Entity,
    pub assignment: Entity,
    pub owner: Entity,
    pub item: Entity,
    pub world_epoch: u64,
    pub kind: BuildingType,
    pub resource: ResourceType,
    pub before_count: u32,
    pub after_count: u32,
    pub footprint: Vec<(i32, i32)>,
    pub wheelbarrow: bool,
}

impl BlueprintDeliveryReceipt {
    /// Queue after the existing item-consumption command, without changing it.
    pub fn queue(self, commands: &mut Commands) {
        commands.queue(move |world: &mut World| {
            if !world.contains_resource::<BlueprintDeliveryReceipts>() {
                return;
            }
            let valid = world.get_resource::<WorldEpoch>().map(|epoch| epoch.get())
                == Some(self.world_epoch)
                && world.get_entity(self.item).is_err()
                && world.get_entity(self.worker).is_ok()
                && self.item != self.owner
                && self.item != self.worker
                && self.before_count.checked_add(1) == Some(self.after_count)
                && !self.footprint.is_empty()
                && self.footprint.len() <= 64
                && world.get::<Blueprint>(self.owner).is_some_and(|blueprint| {
                    blueprint.kind == self.kind
                        && blueprint.occupied_grids == self.footprint
                        && blueprint
                            .delivered_materials
                            .get(&self.resource)
                            .is_some_and(|&count| count >= self.after_count)
                });
            let mut receipts = world.resource_mut::<BlueprintDeliveryReceipts>();
            if !valid {
                receipts
                    .failure
                    .get_or_insert("blueprint delivery commit context differs");
            } else if receipts.total >= CAP {
                receipts
                    .failure
                    .get_or_insert("blueprint delivery receipt cap exceeded");
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

    fn setup() -> (World, BlueprintDeliveryReceipt) {
        let mut world = World::new();
        world.init_resource::<WorldEpoch>();
        world.init_resource::<BlueprintDeliveryReceipts>();
        let mut blueprint = Blueprint::new(BuildingType::RestArea, vec![(2, 3)]);
        blueprint.deliver_material(ResourceType::Wood, 1);
        let owner = world.spawn(blueprint).id();
        let worker = world.spawn_empty().id();
        let item = world.spawn_empty().id();
        let receipt = BlueprintDeliveryReceipt {
            worker,
            assignment: owner,
            owner,
            item,
            world_epoch: world.resource::<WorldEpoch>().get(),
            kind: BuildingType::RestArea,
            resource: ResourceType::Wood,
            before_count: 0,
            after_count: 1,
            footprint: vec![(2, 3)],
            wheelbarrow: false,
        };
        (world, receipt)
    }

    fn apply(world: &mut World, receipt: BlueprintDeliveryReceipt, consume: bool) {
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, world);
        if consume {
            commands.entity(receipt.item).despawn();
        }
        receipt.queue(&mut commands);
        queue.apply(world);
    }

    #[test]
    fn real_item_consumption_is_required_before_receipt() {
        let (mut world, receipt) = setup();
        apply(&mut world, receipt.clone(), true);
        let trace = world.resource::<BlueprintDeliveryReceipts>();
        assert!(trace.failure.is_none());
        assert_eq!(trace.rows.len(), 1);
        assert_eq!(trace.rows[0].item, receipt.item);
        assert_eq!(trace.rows[0].after_count, 1);
    }

    #[test]
    fn missing_consumption_epoch_or_exact_delta_fails_closed() {
        for case in 0..5 {
            let (mut world, mut receipt) = setup();
            match case {
                1 => receipt.world_epoch += 1,
                2 => receipt.after_count = 2,
                3 => receipt.footprint = vec![(3, 3)],
                4 => {
                    world
                        .get_mut::<Blueprint>(receipt.owner)
                        .unwrap()
                        .delivered_materials
                        .clear();
                }
                _ => {}
            }
            apply(&mut world, receipt, case != 0);
            let trace = world.resource::<BlueprintDeliveryReceipts>();
            assert!(trace.rows.is_empty());
            assert!(trace.failure.is_some());
        }
    }

    #[test]
    fn receipt_cap_does_not_reset_when_rows_are_drained() {
        let (mut world, receipt) = setup();
        world.resource_mut::<BlueprintDeliveryReceipts>().total = CAP;
        apply(&mut world, receipt, true);
        let trace = world.resource::<BlueprintDeliveryReceipts>();
        assert!(trace.rows.is_empty());
        assert_eq!(
            trace.failure,
            Some("blueprint delivery receipt cap exceeded")
        );
    }

    #[test]
    fn absent_opt_in_resource_leaves_gameplay_unchanged() {
        let (mut world, receipt) = setup();
        world.remove_resource::<BlueprintDeliveryReceipts>();
        let owner = receipt.owner;
        apply(&mut world, receipt, true);
        assert!(!world.contains_resource::<BlueprintDeliveryReceipts>());
        assert_eq!(
            world.get::<Blueprint>(owner).unwrap().delivered_materials[&ResourceType::Wood],
            1
        );
    }
}
