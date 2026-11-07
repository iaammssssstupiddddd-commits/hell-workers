//! Unit fixtures exercise real handlers, not native/lifecycle acceptance.
use super::*;
use hw_core::{WorldEpoch, logistics::WheelbarrowDestination, relationships::LoadedIn};
use hw_jobs::delivery_observations::BlueprintDeliveryReceipts;
use hw_jobs::{
    HaulToBlueprintData, HaulToBpPhase, HaulWithWheelbarrowData, HaulWithWheelbarrowPhase,
};
use hw_logistics::Wheelbarrow;

fn observed_app() -> App {
    let mut app = task_execution_test_app();
    app.init_resource::<WorldEpoch>()
        .init_resource::<BlueprintDeliveryReceipts>();
    app
}

#[test]
fn hand_delivery_records_consumption_with_and_without_build_chain() {
    for chain in [false, true] {
        let mut app = observed_app();
        let mut blueprint = Blueprint::new(BuildingType::RestArea, vec![(2, 3)]);
        let before = if chain { 4 } else { 0 };
        blueprint.deliver_material(ResourceType::Wood, before);
        let owner = app
            .world_mut()
            .spawn((
                Transform::default(),
                blueprint,
                Designation {
                    work_type: WorkType::Build,
                },
            ))
            .id();
        let item = app
            .world_mut()
            .spawn((Transform::default(), ResourceItem(ResourceType::Wood)))
            .id();
        let assignment = app.world_mut().spawn_empty().id();
        let worker = spawn_task_execution_soul(
            app.world_mut(),
            AssignedTask::HaulToBlueprint(HaulToBlueprintData {
                item,
                blueprint: owner,
                phase: HaulToBpPhase::Delivering,
            }),
        );
        app.world_mut().entity_mut(worker).insert((
            Inventory(Some(item)),
            ActiveTaskIdentity::new(assignment, assignment, WorkType::Haul),
            WorkingOn(assignment),
        ));
        app.update();
        let trace = app.world().resource::<BlueprintDeliveryReceipts>();
        assert!(trace.failure.is_none(), "{:?}", trace.failure);
        assert_eq!(trace.rows.len(), 1);
        let row = &trace.rows[0];
        assert_eq!(
            (row.worker, row.assignment, row.owner, row.item),
            (worker, assignment, owner, item)
        );
        assert_eq!(
            (row.before_count, row.after_count, row.wheelbarrow),
            (before, before + 1, false)
        );
        assert!(app.world().get_entity(item).is_err());
        assert_eq!(app.world().get::<Inventory>(worker).unwrap().0, None);
        if chain {
            assert!(matches!(
                app.world().get::<AssignedTask>(worker),
                Some(AssignedTask::Build(_))
            ));
        } else {
            assert!(matches!(
                app.world().get::<AssignedTask>(worker),
                Some(AssignedTask::None)
            ));
        }
    }
}

#[test]
fn wheelbarrow_delivery_records_each_real_consumed_item_once() {
    let mut app = observed_app();
    let owner = app
        .world_mut()
        .spawn((
            Transform::default(),
            Blueprint::new(BuildingType::RestArea, vec![(2, 3)]),
        ))
        .id();
    let wheelbarrow = app
        .world_mut()
        .spawn((Transform::default(), Wheelbarrow { capacity: 3 }))
        .id();
    let items: Vec<_> = (0..2)
        .map(|_| {
            app.world_mut()
                .spawn((
                    Transform::default(),
                    ResourceItem(ResourceType::Wood),
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
            destination: WheelbarrowDestination::Blueprint(owner),
            collect_source: None,
            collect_amount: 0,
            collect_resource_type: None,
            items: items.clone(),
            phase: HaulWithWheelbarrowPhase::Unloading,
        }),
    );
    app.world_mut().entity_mut(worker).insert((
        Inventory(Some(wheelbarrow)),
        ActiveTaskIdentity::new(assignment, owner, WorkType::WheelbarrowHaul),
        WorkingOn(owner),
    ));
    app.update();
    let trace = app.world().resource::<BlueprintDeliveryReceipts>();
    assert!(trace.failure.is_none(), "{:?}", trace.failure);
    assert_eq!(trace.rows.len(), 2);
    for (index, row) in trace.rows.iter().enumerate() {
        assert_eq!(
            (row.worker, row.assignment, row.owner, row.item),
            (worker, assignment, owner, items[index])
        );
        assert_eq!(
            (row.before_count, row.after_count, row.wheelbarrow),
            (index as u32, index as u32 + 1, true)
        );
        assert!(app.world().get_entity(items[index]).is_err());
    }
    assert_eq!(
        app.world()
            .get::<Blueprint>(owner)
            .unwrap()
            .delivered_materials[&ResourceType::Wood],
        2
    );
}
