//! Opt-in, bounded receipts from the real refinement completion branch.
//! No task driver, material mutation, or acceptance/release authority lives here.
use bevy::prelude::*;
use hw_core::{WorldEpoch, constants::STASIS_MUD_OUTPUT};
use hw_jobs::{
    ActiveTaskIdentity, AssignedTask, Blueprint, Building, BuildingType, RefineData, RefinePhase,
    StoredByMixer, WorkType, mud_mixer::MudMixerStorage,
};
use hw_logistics::{ResourceItem, ResourceType};

const CAP: usize = 4096;

#[derive(Resource, Default)]
pub struct RefineCommitReceipts {
    pub rows: Vec<RefineCommitReceipt>,
    pub failure: Option<&'static str>,
    total: usize,
}

#[derive(Clone, Debug)]
pub struct RefineCommitReceipt {
    pub worker: Entity,
    pub assignment: Entity,
    pub owner: Entity,
    pub world_epoch: u64,
    pub water: Option<Entity>,
    pub water_count: u32,
    pub outputs: Vec<Entity>,
    /// Sand, Rock, StasisMud counters, never physical Sand/Rock item identities.
    pub before: [u32; 3],
    pub after: [u32; 3],
}

impl RefineCommitReceipt {
    /// Runs after the existing deferred consumption/spawn commands, read-only.
    pub fn queue(self, commands: &mut Commands) {
        commands.queue(move |world: &mut World| {
            if !world.contains_resource::<RefineCommitReceipts>() {
                return;
            }
            let unique: std::collections::HashSet<_> = self.outputs.iter().copied().collect();
            let valid = world.get_resource::<WorldEpoch>().map(|epoch| epoch.get()) == Some(self.world_epoch)
                && self.before[0].checked_sub(1) == Some(self.after[0])
                && self.before[1].checked_sub(1) == Some(self.after[1])
                && self.before[2].checked_add(STASIS_MUD_OUTPUT) == Some(self.after[2])
                && self.water_count > 0
                && self.water.is_some_and(|water| water != self.owner && water != self.worker
                    && world.get_entity(water).is_err() && !unique.contains(&water))
                && self.owner != self.worker
                && world.get::<hw_core::soul::DamnedSoul>(self.worker).is_some()
                && world.get::<Blueprint>(self.owner).is_none()
                && world.get::<Building>(self.owner).is_some_and(|building|
                    building.kind == BuildingType::MudMixer && !building.is_provisional)
                // Other deliveries may run in this frame. These are branch-local
                // counters; the later domain snapshot records the final values.
                && world.get::<MudMixerStorage>(self.owner).is_some()
                && world.get::<ActiveTaskIdentity>(self.worker).is_some_and(|identity|
                    identity.assignment_entity == self.assignment && identity.current_target_entity == self.owner
                    && identity.current_work_type == WorkType::Refine && identity.matches_working_on(None))
                && world.get::<hw_core::relationships::WorkingOn>(self.worker).is_none()
                && world.get::<hw_core::relationships::TaskWorkers>(self.owner)
                    .is_none_or(|workers| !workers.iter().any(|worker| *worker == self.worker))
                && matches!(world.get::<AssignedTask>(self.worker),
                    Some(AssignedTask::Refine(RefineData { mixer, phase: RefinePhase::Done })) if *mixer == self.owner)
                && self.outputs.len() == STASIS_MUD_OUTPUT as usize && unique.len() == self.outputs.len()
                && self.outputs.iter().all(|&entity| entity != self.owner && entity != self.worker
                    && world.get::<ResourceItem>(entity).is_some_and(|item| item.0 == ResourceType::StasisMud)
                    && world.get::<StoredByMixer>(entity).is_some_and(|stored| stored.0 == self.owner)
                    && world.get::<hw_core::relationships::StoredIn>(entity).is_none()
                    && world.get::<hw_core::relationships::LoadedIn>(entity).is_none());
            let mut receipts = world.resource_mut::<RefineCommitReceipts>();
            if !valid {
                receipts.failure.get_or_insert("refine commit context differs");
            } else if receipts.total >= CAP {
                receipts.failure.get_or_insert("refine commit receipt cap exceeded");
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

    fn fixture() -> (World, RefineCommitReceipt) {
        let mut world = World::new();
        world.init_resource::<WorldEpoch>();
        world.init_resource::<RefineCommitReceipts>();
        let owner = world
            .spawn((
                Building {
                    kind: BuildingType::MudMixer,
                    is_provisional: false,
                },
                MudMixerStorage {
                    sand: 1,
                    rock: 2,
                    mud: STASIS_MUD_OUTPUT,
                },
            ))
            .id();
        let assignment = world.spawn_empty().id();
        let mut identity = ActiveTaskIdentity::new(assignment, owner, WorkType::Refine);
        identity.detach_from_working_on();
        let worker = world
            .spawn((
                identity,
                hw_core::soul::DamnedSoul::default(),
                AssignedTask::Refine(RefineData {
                    mixer: owner,
                    phase: RefinePhase::Done,
                }),
            ))
            .id();
        let water = world.spawn_empty().id();
        world.despawn(water);
        let outputs = (0..STASIS_MUD_OUTPUT)
            .map(|_| {
                world
                    .spawn((ResourceItem(ResourceType::StasisMud), StoredByMixer(owner)))
                    .id()
            })
            .collect();
        let receipt = RefineCommitReceipt {
            worker,
            assignment,
            owner,
            world_epoch: 0,
            water: Some(water),
            water_count: 1,
            outputs,
            before: [2, 3, 0],
            after: [1, 2, STASIS_MUD_OUTPUT],
        };
        (world, receipt)
    }

    fn apply(world: &mut World, receipt: RefineCommitReceipt) {
        let mut queue = CommandQueue::default();
        receipt.queue(&mut Commands::new(&mut queue, world));
        queue.apply(world);
    }

    #[test]
    fn refine_receipt_requires_real_post_commit_items_counters_and_task() {
        let (mut world, receipt) = fixture();
        apply(&mut world, receipt.clone());
        let trace = world.resource::<RefineCommitReceipts>();
        assert!(trace.failure.is_none());
        assert_eq!(trace.rows.len(), 1);
        assert_eq!(trace.rows[0].outputs, receipt.outputs);
        assert_eq!(world.get::<MudMixerStorage>(receipt.owner).unwrap().sand, 1);
    }

    #[test]
    fn refine_receipt_rejects_missing_or_foreign_commit_without_repairing_world() {
        for case in 0..15 {
            let (mut world, mut receipt) = fixture();
            match case {
                0 => receipt.water = None,
                1 => receipt.water = Some(world.spawn_empty().id()),
                2 => receipt.world_epoch += 1,
                3 => receipt.after[0] += 1,
                4 => {
                    receipt.outputs.pop().unwrap();
                }
                5 => receipt.outputs[1] = receipt.outputs[0],
                6 => {
                    world
                        .entity_mut(receipt.outputs[0])
                        .insert(ResourceItem(ResourceType::Water));
                }
                7 => {
                    world
                        .entity_mut(receipt.outputs[0])
                        .insert(StoredByMixer(receipt.worker));
                }
                8 => {
                    world.entity_mut(receipt.worker).insert(AssignedTask::None);
                }
                9 => receipt.assignment = receipt.owner,
                10 => {
                    world
                        .entity_mut(receipt.owner)
                        .get_mut::<Building>()
                        .unwrap()
                        .is_provisional = true;
                }
                11 => receipt.water_count = 0,
                12 => {
                    world
                        .entity_mut(receipt.outputs[0])
                        .insert(hw_core::relationships::StoredIn(receipt.owner));
                }
                13 => {
                    world
                        .entity_mut(receipt.outputs[0])
                        .insert(hw_core::relationships::LoadedIn(receipt.owner));
                }
                _ => {
                    world
                        .entity_mut(receipt.worker)
                        .insert(hw_core::relationships::WorkingOn(receipt.owner));
                }
            }
            let owner = receipt.owner;
            apply(&mut world, receipt);
            let trace = world.resource::<RefineCommitReceipts>();
            assert_eq!(
                trace.failure,
                Some("refine commit context differs"),
                "case {case}"
            );
            assert!(trace.rows.is_empty());
            assert_eq!(world.get::<MudMixerStorage>(owner).unwrap().sand, 1);
        }
    }

    #[test]
    fn refine_receipt_is_inert_without_opt_in_and_cap_survives_drain() {
        let (mut world, receipt) = fixture();
        world.remove_resource::<RefineCommitReceipts>();
        apply(&mut world, receipt.clone());
        assert!(!world.contains_resource::<RefineCommitReceipts>());
        world.init_resource::<RefineCommitReceipts>();
        world.resource_mut::<RefineCommitReceipts>().total = CAP;
        apply(&mut world, receipt);
        let trace = world.resource::<RefineCommitReceipts>();
        assert_eq!(trace.failure, Some("refine commit receipt cap exceeded"));
        assert!(trace.rows.is_empty());
    }
}
