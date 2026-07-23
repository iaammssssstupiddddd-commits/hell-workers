use super::*;
use crate::soul_ai::execute::task_assignment_apply::apply_task_assignment_requests_system;
use hw_core::events::OnTaskAssigned;
use hw_core::relationships::{DeliveringTo, LoadedIn, StoredIn};
use hw_core::soul::IdleState;
use hw_jobs::events::TaskAssignmentRequest;
use hw_jobs::{HaulWithWheelbarrowData, HaulWithWheelbarrowPhase};
use hw_logistics::Wheelbarrow;
use hw_logistics::transport_request::TransportPriority;
use hw_logistics::transport_request::WheelbarrowDestination;
use hw_logistics::zone::{StockpileAcceptance, StockpilePolicy};
use hw_logistics::{
    StockpilePolicyChangeOutcome, StockpilePolicyChangeRequest, StockpilePolicyPatch,
    apply_stockpile_policy_change_requests_system,
};

fn restrictive_policy(capacity: usize) -> StockpilePolicy {
    StockpilePolicy {
        acceptance: StockpileAcceptance::Only(ResourceType::Rock),
        inbound_priority: TransportPriority::Critical,
        target_amount: 0,
        allow_export: false,
    }
    .normalized_for_capacity(capacity)
}

fn spawn_dropping_soul(world: &mut World, item: Entity, stockpile: Entity) -> Entity {
    let assignment = world.spawn_empty().id();
    world
        .spawn((
            Transform::default(),
            DamnedSoul::default(),
            AssignedTask::Haul(HaulData {
                item,
                stockpile,
                phase: HaulPhase::Dropping,
            }),
            Destination(Vec2::ZERO),
            Path::default(),
            Inventory(Some(item)),
            ActiveTaskIdentity::new(assignment, stockpile, WorkType::Haul),
            WorkingOn(stockpile),
        ))
        .id()
}

#[test]
fn typed_policy_change_grandfathers_committed_haul_and_blocks_the_next_unreserved_haul() {
    let mut app = task_execution_test_app();
    app.add_message::<TaskAssignmentRequest>()
        .add_message::<OnTaskAssigned>()
        .add_message::<StockpilePolicyChangeRequest>()
        .add_message::<StockpilePolicyChangeOutcome>()
        .add_systems(
            Update,
            apply_task_assignment_requests_system.after(collect_task_notification_messages),
        )
        .add_systems(
            Update,
            apply_stockpile_policy_change_requests_system.before(task_execution_system),
        );
    let stockpile = app
        .world_mut()
        .spawn((
            Transform::default(),
            Stockpile {
                capacity: 2,
                resource_type: None,
            },
            StockpilePolicy::for_capacity(2),
        ))
        .id();
    let item = app
        .world_mut()
        .spawn((
            Transform::default(),
            Visibility::Visible,
            ResourceItem(ResourceType::Wood),
            DeliveringTo(stockpile),
        ))
        .id();
    let familiar = app.world_mut().spawn_empty().id();
    let assignment = app.world_mut().spawn_empty().id();
    let soul = app
        .world_mut()
        .spawn((
            Transform::default(),
            Visibility::Visible,
            DamnedSoul::default(),
            AssignedTask::None,
            Destination(Vec2::ZERO),
            Path::default(),
            Inventory::default(),
            IdleState::default(),
        ))
        .id();
    app.world_mut().write_message(TaskAssignmentRequest {
        familiar_entity: familiar,
        worker_entity: soul,
        task_entity: assignment,
        work_type: WorkType::Haul,
        task_pos: Vec2::ZERO,
        assigned_task: AssignedTask::Haul(HaulData {
            item,
            stockpile,
            phase: HaulPhase::GoingToItem,
        }),
        reservation_ops: Vec::new(),
        already_commanded: true,
    });

    app.update();

    assert_eq!(
        app.world().get::<DeliveringTo>(item).map(|target| target.0),
        Some(stockpile)
    );
    *app.world_mut()
        .get_mut::<AssignedTask>(soul)
        .expect("assigned task") = AssignedTask::Haul(HaulData {
        item,
        stockpile,
        phase: HaulPhase::Dropping,
    });
    *app.world_mut()
        .get_mut::<Inventory>(soul)
        .expect("inventory") = Inventory(Some(item));
    let restrictive = restrictive_policy(2);
    app.world_mut().write_message(StockpilePolicyChangeRequest {
        targets: vec![stockpile],
        patch: StockpilePolicyPatch {
            acceptance: Some(restrictive.acceptance),
            inbound_priority: Some(restrictive.inbound_priority),
            target_amount: Some(restrictive.target_amount),
            allow_export: Some(restrictive.allow_export),
        },
    });

    app.update();

    assert_eq!(
        app.world().get::<StockpilePolicy>(stockpile),
        Some(&restrictive)
    );
    assert_eq!(
        app.world().get::<StoredIn>(item).map(|stored| stored.0),
        Some(stockpile)
    );
    assert!(app.world().get::<DeliveringTo>(item).is_none());
    assert!(matches!(
        app.world().get::<AssignedTask>(soul),
        Some(AssignedTask::None)
    ));
    let receipts = app.world().resource::<TaskNotificationReceipts>();
    assert_eq!(receipts.completed_domain.len(), 1);
    assert!(receipts.abandoned.is_empty());

    let next_item = app
        .world_mut()
        .spawn((
            Transform::default(),
            Visibility::Visible,
            ResourceItem(ResourceType::Wood),
        ))
        .id();
    let next_soul = spawn_dropping_soul(app.world_mut(), next_item, stockpile);

    app.update();

    assert!(app.world().get::<StoredIn>(next_item).is_none());
    assert!(matches!(
        app.world().get::<AssignedTask>(next_soul),
        Some(AssignedTask::None)
    ));
    let receipts = app.world().resource::<TaskNotificationReceipts>();
    assert_eq!(receipts.completed_domain.len(), 1);
    assert!(receipts.abandoned.is_empty());
}

#[test]
fn unreserved_haul_obeys_the_current_policy() {
    let mut app = task_execution_test_app();
    let stockpile = app
        .world_mut()
        .spawn((
            Transform::default(),
            Stockpile {
                capacity: 2,
                resource_type: None,
            },
            restrictive_policy(2),
        ))
        .id();
    let item = app
        .world_mut()
        .spawn((
            Transform::default(),
            Visibility::Visible,
            ResourceItem(ResourceType::Wood),
        ))
        .id();
    let soul = spawn_dropping_soul(app.world_mut(), item, stockpile);

    app.update();

    assert!(app.world().get::<StoredIn>(item).is_none());
    assert!(matches!(
        app.world().get::<AssignedTask>(soul),
        Some(AssignedTask::None)
    ));
    let receipts = app.world().resource::<TaskNotificationReceipts>();
    assert!(receipts.completed_domain.is_empty());
    assert!(receipts.abandoned.is_empty());
}

#[test]
fn committed_haul_still_retryably_stops_when_physically_full() {
    let mut app = task_execution_test_app();
    let stockpile = app
        .world_mut()
        .spawn((
            Transform::default(),
            Stockpile {
                capacity: 1,
                resource_type: Some(ResourceType::Wood),
            },
            restrictive_policy(1),
        ))
        .id();
    app.world_mut().spawn((
        Transform::default(),
        Visibility::Visible,
        ResourceItem(ResourceType::Wood),
        StoredIn(stockpile),
    ));
    let item = app
        .world_mut()
        .spawn((
            Transform::default(),
            Visibility::Visible,
            ResourceItem(ResourceType::Wood),
            DeliveringTo(stockpile),
        ))
        .id();
    let soul = spawn_dropping_soul(app.world_mut(), item, stockpile);

    app.update();

    assert!(app.world().get::<StoredIn>(item).is_none());
    assert!(matches!(
        app.world().get::<AssignedTask>(soul),
        Some(AssignedTask::None)
    ));
    let receipts = app.world().resource::<TaskNotificationReceipts>();
    assert!(receipts.completed_domain.is_empty());
    assert!(receipts.abandoned.is_empty());
}

#[test]
fn wheelbarrow_unload_preserves_committed_items_and_rechecks_unreserved_remainder() {
    let mut app = task_execution_test_app();
    let stockpile = app
        .world_mut()
        .spawn((
            Transform::default(),
            Stockpile {
                capacity: 3,
                resource_type: None,
            },
            StockpilePolicy::for_capacity(3),
        ))
        .id();
    let wheelbarrow = app
        .world_mut()
        .spawn((Transform::default(), Wheelbarrow { capacity: 3 }))
        .id();
    let committed_items: Vec<Entity> = (0..2)
        .map(|_| {
            app.world_mut()
                .spawn((
                    Transform::default(),
                    Visibility::Hidden,
                    ResourceItem(ResourceType::Wood),
                    LoadedIn(wheelbarrow),
                    DeliveringTo(stockpile),
                ))
                .id()
        })
        .collect();
    let unreserved_item = app
        .world_mut()
        .spawn((
            Transform::default(),
            Visibility::Hidden,
            ResourceItem(ResourceType::Wood),
            LoadedIn(wheelbarrow),
        ))
        .id();
    let mut items = committed_items.clone();
    items.push(unreserved_item);
    let assignment = app.world_mut().spawn_empty().id();
    let soul = app
        .world_mut()
        .spawn((
            Transform::default(),
            DamnedSoul::default(),
            AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
                wheelbarrow,
                source_pos: Vec2::ZERO,
                destination: WheelbarrowDestination::Stockpile(stockpile),
                collect_source: None,
                collect_amount: 0,
                collect_resource_type: None,
                items,
                phase: HaulWithWheelbarrowPhase::Unloading,
            }),
            Destination(Vec2::ZERO),
            Path::default(),
            Inventory(Some(wheelbarrow)),
            ActiveTaskIdentity::new(assignment, stockpile, WorkType::WheelbarrowHaul),
            WorkingOn(stockpile),
        ))
        .id();
    *app.world_mut()
        .get_mut::<StockpilePolicy>(stockpile)
        .expect("stockpile policy") = restrictive_policy(3);

    app.update();

    for item in committed_items {
        assert_eq!(
            app.world().get::<StoredIn>(item).map(|stored| stored.0),
            Some(stockpile)
        );
        assert!(app.world().get::<LoadedIn>(item).is_none());
        assert!(app.world().get::<DeliveringTo>(item).is_none());
    }
    assert!(app.world().get::<StoredIn>(unreserved_item).is_none());
    assert!(app.world().get::<LoadedIn>(unreserved_item).is_none());
    assert!(app.world().get::<DeliveringTo>(unreserved_item).is_none());
    assert_eq!(
        app.world().get::<Visibility>(unreserved_item),
        Some(&Visibility::Visible),
        "the policy-rejected remainder must return to the ground"
    );
    assert!(matches!(
        app.world().get::<AssignedTask>(soul),
        Some(AssignedTask::None)
    ));
    let receipts = app.world().resource::<TaskNotificationReceipts>();
    assert_eq!(receipts.completed_domain.len(), 1);
    assert!(receipts.abandoned.is_empty());
}
