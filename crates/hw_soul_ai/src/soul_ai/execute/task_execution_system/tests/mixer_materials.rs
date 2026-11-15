//! Real handlers in unit fixtures; no ordinary-world or release authority.
use super::*;
use crate::refine_observations::RefineCommitReceipts;
use hw_core::{
    constants::MUD_MIXER_CAPACITY, logistics::WheelbarrowDestination, relationships::LoadedIn,
};
use hw_jobs::{
    Building, HaulToMixerData, HaulToMixerPhase, HaulWithWheelbarrowData, HaulWithWheelbarrowPhase,
    mud_mixer::MudMixerStorage,
};
use hw_logistics::Wheelbarrow;

fn app_owner(observe: bool, sand: u32, rock: u32) -> (App, Entity) {
    let mut app = task_execution_test_app();
    app.init_resource::<WorldEpoch>();
    if observe {
        app.init_resource::<RefineCommitReceipts>();
    }
    let owner = app
        .world_mut()
        .spawn((
            Transform::default(),
            Building {
                kind: BuildingType::MudMixer,
                is_provisional: false,
            },
            MudMixerStorage { sand, rock, mud: 0 },
        ))
        .id();
    (app, owner)
}

#[test]
fn actual_hand_material_conversion_preserves_full_and_success_branches() {
    for observe in [false, true] {
        for resource in [ResourceType::Sand, ResourceType::Rock] {
            for full in [false, true] {
                let n = if full { MUD_MIXER_CAPACITY } else { 0 };
                let (mut app, owner) = app_owner(observe, n, n);
                let item = app
                    .world_mut()
                    .spawn((
                        Transform::default(),
                        Visibility::Hidden,
                        ResourceItem(resource),
                    ))
                    .id();
                let assignment = app.world_mut().spawn_empty().id();
                let worker = spawn_task_execution_soul(
                    app.world_mut(),
                    AssignedTask::HaulToMixer(HaulToMixerData {
                        item,
                        mixer: owner,
                        resource_type: resource,
                        phase: HaulToMixerPhase::Delivering,
                    }),
                );
                app.world_mut().entity_mut(worker).insert((
                    Inventory(Some(item)),
                    WorkingOn(item),
                    ActiveTaskIdentity::new(assignment, item, WorkType::HaulToMixer),
                ));
                app.update();
                assert_eq!(app.world().get_entity(item).is_err(), !full);
                let storage = app.world().get::<MudMixerStorage>(owner).unwrap();
                assert_eq!(
                    if resource == ResourceType::Sand {
                        storage.sand
                    } else {
                        storage.rock
                    },
                    if full { n } else { 1 }
                );
                assert!(app.world().get::<Inventory>(worker).unwrap().0.is_none());
                if observe {
                    let rows = app.world().resource::<RefineCommitReceipts>();
                    assert!(rows.failure.is_none(), "{:?}", rows.failure);
                    assert_eq!(rows.materials.len(), usize::from(!full));
                    if !full {
                        assert_eq!(rows.materials[0].item, item);
                    }
                }
            }
        }
    }
}

#[test]
fn actual_wheelbarrow_material_conversion_records_only_accepted_items() {
    for observe in [false, true] {
        for available in [0, 1, 2] {
            let before = MUD_MIXER_CAPACITY - available;
            let (mut app, owner) = app_owner(observe, before, 0);
            let wheelbarrow = app
                .world_mut()
                .spawn((Transform::default(), Wheelbarrow { capacity: 3 }))
                .id();
            let items: Vec<_> = (0..2)
                .map(|_| {
                    app.world_mut()
                        .spawn((
                            Transform::default(),
                            Visibility::Hidden,
                            ResourceItem(ResourceType::Sand),
                            LoadedIn(wheelbarrow),
                        ))
                        .id()
                })
                .collect();
            let assignment = app.world_mut().spawn_empty().id();
            let worker = spawn_task_execution_soul(
                app.world_mut(),
                AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
                    wheelbarrow,
                    source_pos: Vec2::ZERO,
                    destination: WheelbarrowDestination::Mixer {
                        entity: owner,
                        resource_type: ResourceType::Sand,
                    },
                    collect_source: None,
                    collect_amount: 0,
                    collect_resource_type: None,
                    items: items.clone(),
                    phase: HaulWithWheelbarrowPhase::Unloading,
                }),
            );
            app.world_mut().entity_mut(worker).insert((
                Inventory(Some(wheelbarrow)),
                WorkingOn(owner),
                ActiveTaskIdentity::new(assignment, owner, WorkType::WheelbarrowHaul),
            ));
            app.update();
            assert_eq!(
                app.world().get::<MudMixerStorage>(owner).unwrap().sand,
                MUD_MIXER_CAPACITY
            );
            assert_eq!(
                items
                    .iter()
                    .filter(|&&e| app.world().get_entity(e).is_err())
                    .count(),
                available as usize
            );
            if observe {
                let rows = app.world().resource::<RefineCommitReceipts>();
                assert!(rows.failure.is_none(), "{:?}", rows.failure);
                assert_eq!(rows.materials.len(), available as usize);
                for (index, row) in rows.materials.iter().enumerate() {
                    assert_eq!(
                        (row.before[0], row.after[0]),
                        (before + index as u32, before + index as u32 + 1)
                    );
                    assert_eq!(row.loaded_before, Some(wheelbarrow));
                }
            }
        }
    }
}

#[test]
fn material_receipt_rejects_foreign_or_malformed_branch_without_repair() {
    use bevy::ecs::world::CommandQueue;
    for case in 0..12 {
        let (mut app, owner) = app_owner(true, 0, 0);
        let item = app
            .world_mut()
            .spawn((
                Transform::default(),
                Visibility::Hidden,
                ResourceItem(ResourceType::Sand),
            ))
            .id();
        let assignment = app.world_mut().spawn_empty().id();
        let worker = spawn_task_execution_soul(
            app.world_mut(),
            AssignedTask::HaulToMixer(HaulToMixerData {
                item,
                mixer: owner,
                resource_type: ResourceType::Sand,
                phase: HaulToMixerPhase::Delivering,
            }),
        );
        app.world_mut().entity_mut(worker).insert((
            Inventory(Some(item)),
            WorkingOn(item),
            ActiveTaskIdentity::new(assignment, item, WorkType::HaulToMixer),
        ));
        app.update();
        let mut row = app.world().resource::<RefineCommitReceipts>().materials[0].clone();
        match case {
            0 => row.world_epoch += 1,
            1 => row.owner = worker,
            2 => row.item = app.world_mut().spawn_empty().id(),
            3 => row.actual_resource = None,
            4 => row.stored_before = Some(owner),
            5 => row.loaded_before = Some(owner),
            6 => row.carried_before = None,
            7 => row.work_type = WorkType::Refine,
            8 => row.after[0] += 1,
            9 => {
                row.before[0] = u32::MAX;
                row.after[0] = 0;
            }
            10 => row.resource = ResourceType::Water,
            _ => {
                app.world_mut()
                    .get_mut::<Building>(owner)
                    .unwrap()
                    .is_provisional = true;
            }
        }
        let mut queue = CommandQueue::default();
        row.queue(&mut Commands::new(&mut queue, app.world()));
        queue.apply(app.world_mut());
        assert!(
            app.world()
                .resource::<RefineCommitReceipts>()
                .failure
                .is_some()
        );
        assert_eq!(app.world().get::<MudMixerStorage>(owner).unwrap().sand, 1);
    }
}
