//! Real handler unit fixtures; not ordinary-world or native acceptance.
use super::*;
use crate::refine_observations::RefineCommitReceipts;
use hw_core::relationships::StoredIn;
use hw_jobs::{Building, RefineData, RefinePhase, StoredByMixer, mud_mixer::MudMixerStorage};

#[test]
fn real_refine_branch_receipt_observes_water_despawn_five_outputs_and_detached_done() {
    for observe in [false, true] {
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
                MudMixerStorage {
                    sand: 2,
                    rock: 3,
                    mud: 0,
                },
                Stockpile {
                    capacity: 5,
                    resource_type: Some(ResourceType::Water),
                },
                Designation {
                    work_type: WorkType::Refine,
                },
            ))
            .id();
        let water = app
            .world_mut()
            .spawn((
                Transform::default(),
                Visibility::Visible,
                ResourceItem(ResourceType::Water),
                StoredIn(owner),
            ))
            .id();
        let worker = spawn_task_execution_soul(
            app.world_mut(),
            AssignedTask::Refine(RefineData {
                mixer: owner,
                phase: RefinePhase::Refining { progress: 1. },
            }),
        );
        app.world_mut().entity_mut(worker).insert((
            WorkingOn(owner),
            ActiveTaskIdentity::new(owner, owner, WorkType::Refine),
        ));
        app.update();
        assert!(app.world().get_entity(water).is_err());
        assert!(app.world().get::<WorkingOn>(worker).is_none());
        let storage = app.world().get::<MudMixerStorage>(owner).unwrap();
        assert_eq!([storage.sand, storage.rock, storage.mud], [1, 2, 5]);
        assert!(
            matches!(app.world().get::<AssignedTask>(worker), Some(AssignedTask::Refine(
            RefineData { mixer, phase: RefinePhase::Done })) if *mixer == owner)
        );
        if observe {
            let receipts = app.world().resource::<RefineCommitReceipts>();
            assert!(receipts.failure.is_none(), "{:?}", receipts.failure);
            assert_eq!(receipts.rows.len(), 1);
            let row = &receipts.rows[0];
            assert_eq!(
                (row.worker, row.owner, row.water),
                (worker, owner, Some(water))
            );
            assert_eq!(row.before, [2, 3, 0]);
            assert_eq!(row.after, [1, 2, 5]);
            assert_eq!(row.outputs.len(), 5);
            for &item in &row.outputs {
                assert_eq!(
                    app.world().get::<ResourceItem>(item).unwrap().0,
                    ResourceType::StasisMud
                );
                assert_eq!(app.world().get::<StoredByMixer>(item).unwrap().0, owner);
                assert!(app.world().get::<StoredIn>(item).is_none());
            }
        } else {
            assert!(!app.world().contains_resource::<RefineCommitReceipts>());
        }
    }
}

#[test]
fn insufficient_materials_never_publish_a_refine_commit() {
    let mut app = task_execution_test_app();
    app.init_resource::<WorldEpoch>()
        .init_resource::<RefineCommitReceipts>();
    let owner = app
        .world_mut()
        .spawn((
            Transform::default(),
            MudMixerStorage::default(),
            Designation {
                work_type: WorkType::Refine,
            },
        ))
        .id();
    let worker = spawn_task_execution_soul(
        app.world_mut(),
        AssignedTask::Refine(RefineData {
            mixer: owner,
            phase: RefinePhase::Refining { progress: 1. },
        }),
    );
    app.world_mut().entity_mut(worker).insert((
        WorkingOn(owner),
        ActiveTaskIdentity::new(owner, owner, WorkType::Refine),
    ));
    app.update();
    let receipts = app.world().resource::<RefineCommitReceipts>();
    assert!(receipts.rows.is_empty());
    assert!(receipts.failure.is_none());
    assert_eq!(app.world().get::<MudMixerStorage>(owner).unwrap().mud, 0);
}
