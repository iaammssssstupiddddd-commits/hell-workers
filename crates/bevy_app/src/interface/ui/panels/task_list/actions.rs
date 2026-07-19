//! Task dashboard capability resolution and owner-routed actions.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::jobs::WorkType;
use hw_core::relationships::TaskWorkers;
use hw_familiar_ai::AutoGatherDesignation;
use hw_jobs::{
    Blueprint, BlueprintCancelRequested, Designation, PlayerIssuedDesignation, Priority, Rock, Tree,
};
use hw_logistics::ResourceItem;
use hw_logistics::transport_request::{
    ManualTransportCloseContext, ManualTransportCloseResult, ManualTransportRequest,
    TransportRequest, TransportRequestFixedSource, TransportRequestKind,
    close_manual_transport_request,
};
use hw_ui::UiIntent;
use hw_ui::components::UiInputState;
use hw_ui::notifications::{NotificationRetention, NotificationSeverity, UserFacingNotification};
use hw_ui::panels::task_list::{
    PendingTaskCancellation, TaskActionButton, TaskActionButtonKind, TaskActionCapabilities,
    TaskCancelKind, TaskDashboardActionState, TaskListDirty, TaskPriorityAdjustment,
    TaskPriorityTier,
};

use crate::input_actions::{ForegroundUiGate, PendingWorldInputCapture};
use crate::systems::command::area_selection::cancel_single_designation;
use crate::systems::jobs::floor_construction::{
    FloorConstructionCancelRequested, FloorConstructionSite, FloorTileBlueprint,
};
use crate::systems::jobs::wall_construction::{
    WallConstructionCancelRequested, WallConstructionSite, WallTileBlueprint,
};

pub(super) struct TaskCapabilityRefs<'a> {
    pub designation: &'a Designation,
    pub has_priority: bool,
    pub player_issued: Option<&'a PlayerIssuedDesignation>,
    pub auto_gather: Option<&'a AutoGatherDesignation>,
    pub tree: Option<&'a Tree>,
    pub rock: Option<&'a Rock>,
    pub blueprint: Option<&'a Blueprint>,
    pub manual_transport: Option<&'a ManualTransportRequest>,
    pub fixed_source: Option<&'a TransportRequestFixedSource>,
    pub floor_tile: Option<&'a FloorTileBlueprint>,
    pub wall_tile: Option<&'a WallTileBlueprint>,
    pub transport_request: Option<&'a TransportRequest>,
}

pub(super) fn resolve_task_action_capabilities(
    refs: TaskCapabilityRefs<'_>,
) -> TaskActionCapabilities {
    let work_type = refs.designation.work_type;
    let manual_chop_or_mine = refs.player_issued.is_some()
        && refs.auto_gather.is_none()
        && refs.manual_transport.is_none()
        && refs.blueprint.is_none()
        && refs.transport_request.is_none()
        && matches!(
            (work_type, refs.tree.is_some(), refs.rock.is_some()),
            (WorkType::Chop, true, false) | (WorkType::Mine, false, true)
        );
    if manual_chop_or_mine {
        return TaskActionCapabilities {
            focus: true,
            priority: refs.has_priority,
            cancel: Some(TaskCancelKind::GenericDesignation),
        };
    }

    if refs.manual_transport.is_some()
        && refs.fixed_source.is_some()
        && refs.transport_request.is_some()
    {
        return TaskActionCapabilities {
            focus: true,
            priority: refs.has_priority,
            cancel: Some(TaskCancelKind::ManualTransportRequest),
        };
    }

    if refs.blueprint.is_some() {
        return TaskActionCapabilities {
            focus: true,
            priority: false,
            cancel: Some(TaskCancelKind::Blueprint),
        };
    }

    if let Some(tile) = refs.floor_tile {
        return TaskActionCapabilities {
            focus: true,
            priority: false,
            cancel: Some(TaskCancelKind::FloorSite(tile.parent_site)),
        };
    }
    if let Some(tile) = refs.wall_tile {
        return TaskActionCapabilities {
            focus: true,
            priority: false,
            cancel: Some(TaskCancelKind::WallSite(tile.parent_site)),
        };
    }
    if let Some(request) = refs.transport_request {
        let cancel = match request.kind {
            TransportRequestKind::DeliverToFloorConstruction => {
                Some(TaskCancelKind::FloorSite(request.anchor))
            }
            TransportRequestKind::DeliverToWallConstruction => {
                Some(TaskCancelKind::WallSite(request.anchor))
            }
            _ => None,
        };
        if cancel.is_some() {
            return TaskActionCapabilities {
                focus: true,
                priority: false,
                cancel,
            };
        }
    }

    TaskActionCapabilities::READ_ONLY
}

pub fn task_dashboard_action_button_system(
    interactions: Query<(Entity, &Interaction, &TaskActionButton), Changed<Interaction>>,
    time: Res<Time<Virtual>>,
    foreground_gate: ForegroundUiGate,
    mut action_state: ResMut<TaskDashboardActionState>,
    mut dirty: ResMut<TaskListDirty>,
    mut intents: MessageWriter<UiIntent>,
) {
    for (button_entity, interaction, action) in &interactions {
        if *interaction != Interaction::Pressed
            || time.is_paused()
            || !foreground_gate.allows(button_entity)
        {
            continue;
        }

        match action.kind {
            TaskActionButtonKind::AdjustPriority(adjustment) => {
                if action_state.confirmation.take().is_some() {
                    dirty.mark_list();
                }
                intents.write(UiIntent::AdjustTaskPriority {
                    entity: action.target,
                    expected_work_type: action.expected_work_type,
                    adjustment,
                });
            }
            TaskActionButtonKind::Cancel(kind) => {
                let pending = PendingTaskCancellation {
                    target: action.target,
                    expected_work_type: action.expected_work_type,
                    kind,
                };
                if action_state.confirmation == Some(pending) {
                    action_state.confirmation = None;
                    intents.write(UiIntent::CancelTask {
                        entity: action.target,
                        expected_work_type: action.expected_work_type,
                        expected_kind: kind,
                    });
                } else {
                    action_state.confirmation = Some(pending);
                }
                dirty.mark_list();
            }
        }
    }
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskActionOutcome {
    pub entity: Entity,
    pub action: TaskActionKind,
    pub result: TaskActionResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskActionKind {
    AdjustPriority(TaskPriorityAdjustment),
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskActionResult {
    PriorityChanged(TaskPriorityTier),
    CancellationRequested,
    MalformedRequestClosed,
    Stale,
    Unsupported,
    Paused,
    Captured,
}

type TaskActionTargetQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Designation,
        Option<&'static mut Priority>,
        Option<&'static PlayerIssuedDesignation>,
        Option<&'static AutoGatherDesignation>,
        Option<&'static Tree>,
        Option<&'static Rock>,
        Option<&'static Blueprint>,
        Option<&'static ManualTransportRequest>,
        Option<&'static TransportRequestFixedSource>,
        Option<&'static TaskWorkers>,
        Option<&'static ResourceItem>,
        Option<&'static FloorTileBlueprint>,
        Option<&'static WallTileBlueprint>,
        Option<&'static TransportRequest>,
    ),
>;

#[derive(SystemParam)]
pub struct TaskActionApplyQueries<'w, 's> {
    targets: TaskActionTargetQuery<'w, 's>,
    floor_sites: Query<'w, 's, (), With<FloorConstructionSite>>,
    wall_sites: Query<'w, 's, (), With<WallConstructionSite>>,
}

pub fn apply_task_action_intents_system(
    mut commands: Commands,
    mut intents: MessageReader<UiIntent>,
    mut outcomes: MessageWriter<TaskActionOutcome>,
    time: Res<Time<Virtual>>,
    ui_input_state: Res<UiInputState>,
    pending_capture: Res<PendingWorldInputCapture>,
    mut queries: TaskActionApplyQueries,
) {
    for intent in intents.read().copied() {
        let request = match intent {
            UiIntent::AdjustTaskPriority {
                entity,
                expected_work_type,
                adjustment,
            } => Some((
                entity,
                expected_work_type,
                TaskActionRequest::AdjustPriority(adjustment),
            )),
            UiIntent::CancelTask {
                entity,
                expected_work_type,
                expected_kind,
            } => Some((
                entity,
                expected_work_type,
                TaskActionRequest::Cancel(expected_kind),
            )),
            _ => None,
        };
        let Some((entity, expected_work_type, request)) = request else {
            continue;
        };
        let action = request.action_kind();

        let result = if time.is_paused() {
            TaskActionResult::Paused
        } else if ui_input_state.world_input_captured || pending_capture.overlay().is_some() {
            TaskActionResult::Captured
        } else {
            apply_live_task_action(
                &mut commands,
                &mut queries,
                entity,
                expected_work_type,
                request,
            )
        };
        outcomes.write(TaskActionOutcome {
            entity,
            action,
            result,
        });
    }
}

#[derive(Debug, Clone, Copy)]
enum TaskActionRequest {
    AdjustPriority(TaskPriorityAdjustment),
    Cancel(TaskCancelKind),
}

impl TaskActionRequest {
    const fn action_kind(self) -> TaskActionKind {
        match self {
            Self::AdjustPriority(adjustment) => TaskActionKind::AdjustPriority(adjustment),
            Self::Cancel(_) => TaskActionKind::Cancel,
        }
    }
}

fn apply_live_task_action(
    commands: &mut Commands,
    queries: &mut TaskActionApplyQueries<'_, '_>,
    entity: Entity,
    expected_work_type: WorkType,
    request: TaskActionRequest,
) -> TaskActionResult {
    let Ok((
        designation,
        mut priority,
        player_issued,
        auto_gather,
        tree,
        rock,
        blueprint,
        manual_transport,
        fixed_source,
        workers,
        resource_item,
        floor_tile,
        wall_tile,
        transport_request,
    )) = queries.targets.get_mut(entity)
    else {
        return TaskActionResult::Stale;
    };
    if designation.work_type != expected_work_type {
        return TaskActionResult::Stale;
    }

    let capabilities = resolve_task_action_capabilities(TaskCapabilityRefs {
        designation,
        has_priority: priority.is_some(),
        player_issued,
        auto_gather,
        tree,
        rock,
        blueprint,
        manual_transport,
        fixed_source,
        floor_tile,
        wall_tile,
        transport_request,
    });

    match request {
        TaskActionRequest::AdjustPriority(adjustment) => {
            if !capabilities.priority {
                return TaskActionResult::Unsupported;
            }
            let Some(priority) = priority.as_deref_mut() else {
                return TaskActionResult::Stale;
            };
            let next = TaskPriorityTier::from_priority(priority.0).adjusted_value(adjustment);
            priority.0 = next;
            TaskActionResult::PriorityChanged(TaskPriorityTier::from_priority(next))
        }
        TaskActionRequest::Cancel(expected_kind) => {
            if capabilities.cancel != Some(expected_kind) {
                return TaskActionResult::Stale;
            }
            match expected_kind {
                TaskCancelKind::GenericDesignation => {
                    cancel_single_designation(commands, entity, workers, false, false, None);
                    TaskActionResult::CancellationRequested
                }
                TaskCancelKind::ManualTransportRequest => {
                    match close_manual_transport_request(
                        commands,
                        ManualTransportCloseContext {
                            request_entity: entity,
                            manual: manual_transport,
                            fixed_source,
                            workers,
                            resource_item,
                        },
                    ) {
                        ManualTransportCloseResult::Closed => {
                            TaskActionResult::CancellationRequested
                        }
                        ManualTransportCloseResult::MalformedClosed => {
                            TaskActionResult::MalformedRequestClosed
                        }
                        ManualTransportCloseResult::Unsupported => TaskActionResult::Unsupported,
                    }
                }
                TaskCancelKind::Blueprint => {
                    commands.entity(entity).try_insert(BlueprintCancelRequested);
                    TaskActionResult::CancellationRequested
                }
                TaskCancelKind::FloorSite(site) => {
                    if queries.floor_sites.get(site).is_err() {
                        return TaskActionResult::Stale;
                    }
                    commands
                        .entity(site)
                        .try_insert(FloorConstructionCancelRequested);
                    TaskActionResult::CancellationRequested
                }
                TaskCancelKind::WallSite(site) => {
                    if queries.wall_sites.get(site).is_err() {
                        return TaskActionResult::Stale;
                    }
                    commands
                        .entity(site)
                        .try_insert(WallConstructionCancelRequested);
                    TaskActionResult::CancellationRequested
                }
            }
        }
    }
}

pub fn adapt_task_action_outcomes(
    mut outcomes: MessageReader<TaskActionOutcome>,
    mut notifications: MessageWriter<UserFacingNotification>,
) {
    for outcome in outcomes.read() {
        let (severity, title, body) = match outcome.result {
            TaskActionResult::PriorityChanged(tier) => (
                NotificationSeverity::Success,
                "Priority changed",
                format!("Task priority is now {}.", tier.label()),
            ),
            TaskActionResult::CancellationRequested => (
                NotificationSeverity::Success,
                "Cancellation requested",
                "The task owner will finish cleanup.".to_string(),
            ),
            TaskActionResult::MalformedRequestClosed => (
                NotificationSeverity::Warning,
                "Task closed",
                "Incomplete transport data was cleaned up safely.".to_string(),
            ),
            TaskActionResult::Stale => (
                NotificationSeverity::Warning,
                "Task changed",
                "The selected task is no longer available.".to_string(),
            ),
            TaskActionResult::Unsupported => (
                NotificationSeverity::Warning,
                "Action unavailable",
                "This task is read-only.".to_string(),
            ),
            TaskActionResult::Paused => (
                NotificationSeverity::Warning,
                "Action paused",
                "Resume the simulation before changing tasks.".to_string(),
            ),
            TaskActionResult::Captured => (
                NotificationSeverity::Warning,
                "Action blocked",
                "Close the active dialog before changing tasks.".to_string(),
            ),
        };
        notifications.write(UserFacingNotification::new(
            format!(
                "task-action:{}:{}:{}:{}",
                outcome.entity.index_u32(),
                outcome.entity.generation().to_bits(),
                action_key(outcome.action),
                result_key(outcome.result),
            ),
            severity,
            title,
            body,
            NotificationRetention::ToastOnly,
        ));
    }
}

const fn action_key(action: TaskActionKind) -> &'static str {
    match action {
        TaskActionKind::AdjustPriority(TaskPriorityAdjustment::Decrease) => "priority-down",
        TaskActionKind::AdjustPriority(TaskPriorityAdjustment::Increase) => "priority-up",
        TaskActionKind::Cancel => "cancel",
    }
}

const fn result_key(result: TaskActionResult) -> &'static str {
    match result {
        TaskActionResult::PriorityChanged(TaskPriorityTier::Normal) => "priority-normal",
        TaskActionResult::PriorityChanged(TaskPriorityTier::High) => "priority-high",
        TaskActionResult::PriorityChanged(TaskPriorityTier::Critical) => "priority-critical",
        TaskActionResult::CancellationRequested => "cancel-requested",
        TaskActionResult::MalformedRequestClosed => "malformed-closed",
        TaskActionResult::Stale => "stale",
        TaskActionResult::Unsupported => "unsupported",
        TaskActionResult::Paused => "paused",
        TaskActionResult::Captured => "captured",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct OutcomeReceipts(Vec<TaskActionOutcome>);

    #[derive(Resource, Default)]
    struct IntentReceiptCount(usize);

    fn collect_outcomes(
        mut outcomes: MessageReader<TaskActionOutcome>,
        mut receipts: ResMut<OutcomeReceipts>,
    ) {
        receipts.0.extend(outcomes.read().copied());
    }

    fn count_intents(
        mut intents: MessageReader<UiIntent>,
        mut receipts: ResMut<IntentReceiptCount>,
    ) {
        receipts.0 += intents.read().count();
    }

    fn empty_capability_refs(designation: &Designation) -> TaskCapabilityRefs<'_> {
        TaskCapabilityRefs {
            designation,
            has_priority: false,
            player_issued: None,
            auto_gather: None,
            tree: None,
            rock: None,
            blueprint: None,
            manual_transport: None,
            fixed_source: None,
            floor_tile: None,
            wall_tile: None,
            transport_request: None,
        }
    }

    #[test]
    fn task_dashboard_action_capability_allow_list_rejects_unmarked_designations() {
        let chop = Designation {
            work_type: WorkType::Chop,
        };
        let tree = Tree;
        let player = PlayerIssuedDesignation;

        let allowed = resolve_task_action_capabilities(TaskCapabilityRefs {
            designation: &chop,
            has_priority: true,
            player_issued: Some(&player),
            auto_gather: None,
            tree: Some(&tree),
            rock: None,
            blueprint: None,
            manual_transport: None,
            fixed_source: None,
            floor_tile: None,
            wall_tile: None,
            transport_request: None,
        });
        assert!(allowed.priority);
        assert_eq!(allowed.cancel, Some(TaskCancelKind::GenericDesignation));

        let denied = resolve_task_action_capabilities(TaskCapabilityRefs {
            player_issued: None,
            ..TaskCapabilityRefs {
                designation: &chop,
                has_priority: true,
                player_issued: Some(&player),
                auto_gather: None,
                tree: Some(&tree),
                rock: None,
                blueprint: None,
                manual_transport: None,
                fixed_source: None,
                floor_tile: None,
                wall_tile: None,
                transport_request: None,
            }
        });
        assert_eq!(denied, TaskActionCapabilities::READ_ONLY);

        let auto_gather = AutoGatherDesignation {
            owner: Entity::PLACEHOLDER,
            resource_type: hw_core::logistics::ResourceType::Wood,
        };
        let denied_auto = resolve_task_action_capabilities(TaskCapabilityRefs {
            designation: &chop,
            has_priority: true,
            player_issued: Some(&player),
            auto_gather: Some(&auto_gather),
            tree: Some(&tree),
            rock: None,
            blueprint: None,
            manual_transport: None,
            fixed_source: None,
            floor_tile: None,
            wall_tile: None,
            transport_request: None,
        });
        assert_eq!(denied_auto, TaskActionCapabilities::READ_ONLY);
    }

    #[test]
    fn task_dashboard_action_capabilities_match_the_owner_matrix() {
        let player = PlayerIssuedDesignation;
        let rock = Rock;
        let manual_mine = Designation {
            work_type: WorkType::Mine,
        };
        assert_eq!(
            resolve_task_action_capabilities(TaskCapabilityRefs {
                has_priority: true,
                player_issued: Some(&player),
                rock: Some(&rock),
                ..empty_capability_refs(&manual_mine)
            }),
            TaskActionCapabilities {
                focus: true,
                priority: true,
                cancel: Some(TaskCancelKind::GenericDesignation),
            }
        );

        let request = TransportRequest {
            kind: TransportRequestKind::DepositToStockpile,
            anchor: Entity::PLACEHOLDER,
            resource_type: hw_core::logistics::ResourceType::Wood,
            issued_by: Entity::PLACEHOLDER,
            priority: hw_logistics::transport_request::TransportPriority::Normal,
            stockpile_group: Vec::new(),
        };
        let manual_transport = ManualTransportRequest;
        let fixed_source = TransportRequestFixedSource(Entity::PLACEHOLDER);
        let haul = Designation {
            work_type: WorkType::Haul,
        };
        assert_eq!(
            resolve_task_action_capabilities(TaskCapabilityRefs {
                has_priority: true,
                manual_transport: Some(&manual_transport),
                fixed_source: Some(&fixed_source),
                transport_request: Some(&request),
                ..empty_capability_refs(&haul)
            }),
            TaskActionCapabilities {
                focus: true,
                priority: true,
                cancel: Some(TaskCancelKind::ManualTransportRequest),
            }
        );
        assert_eq!(
            resolve_task_action_capabilities(TaskCapabilityRefs {
                has_priority: true,
                transport_request: Some(&request),
                ..empty_capability_refs(&haul)
            }),
            TaskActionCapabilities::READ_ONLY,
            "producer-owned transport requests stay read-only",
        );

        let blueprint = Blueprint::new(hw_jobs::BuildingType::Wall, Vec::new());
        let build = Designation {
            work_type: WorkType::Build,
        };
        assert_eq!(
            resolve_task_action_capabilities(TaskCapabilityRefs {
                has_priority: true,
                blueprint: Some(&blueprint),
                ..empty_capability_refs(&build)
            }),
            TaskActionCapabilities {
                focus: true,
                priority: false,
                cancel: Some(TaskCancelKind::Blueprint),
            },
            "Blueprint priority remains read-only",
        );

        let floor_site = Entity::from_raw_u32(41).expect("valid floor site");
        let floor_tile = FloorTileBlueprint::new(floor_site, (1, 2));
        assert_eq!(
            resolve_task_action_capabilities(TaskCapabilityRefs {
                floor_tile: Some(&floor_tile),
                ..empty_capability_refs(&build)
            })
            .cancel,
            Some(TaskCancelKind::FloorSite(floor_site))
        );
        let wall_site = Entity::from_raw_u32(42).expect("valid wall site");
        let wall_tile = WallTileBlueprint::new(wall_site, (3, 4));
        assert_eq!(
            resolve_task_action_capabilities(TaskCapabilityRefs {
                wall_tile: Some(&wall_tile),
                ..empty_capability_refs(&build)
            })
            .cancel,
            Some(TaskCancelKind::WallSite(wall_site))
        );
        let floor_request = TransportRequest {
            kind: TransportRequestKind::DeliverToFloorConstruction,
            anchor: floor_site,
            ..request.clone()
        };
        assert_eq!(
            resolve_task_action_capabilities(TaskCapabilityRefs {
                transport_request: Some(&floor_request),
                ..empty_capability_refs(&haul)
            })
            .cancel,
            Some(TaskCancelKind::FloorSite(floor_site))
        );
        let wall_request = TransportRequest {
            kind: TransportRequestKind::DeliverToWallConstruction,
            anchor: wall_site,
            ..request.clone()
        };
        assert_eq!(
            resolve_task_action_capabilities(TaskCapabilityRefs {
                transport_request: Some(&wall_request),
                ..empty_capability_refs(&haul)
            })
            .cancel,
            Some(TaskCancelKind::WallSite(wall_site))
        );

        for work_type in [WorkType::Move, WorkType::GeneratePower] {
            let designation = Designation { work_type };
            assert_eq!(
                resolve_task_action_capabilities(TaskCapabilityRefs {
                    has_priority: true,
                    player_issued: Some(&player),
                    ..empty_capability_refs(&designation)
                }),
                TaskActionCapabilities::READ_ONLY,
            );
        }
    }

    #[test]
    fn task_dashboard_captured_or_paused_action_press_leaves_no_intent_or_confirmation() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<UiInputState>()
            .init_resource::<PendingWorldInputCapture>()
            .init_resource::<TaskDashboardActionState>()
            .init_resource::<TaskListDirty>()
            .init_resource::<IntentReceiptCount>()
            .add_message::<UiIntent>()
            .add_systems(
                Update,
                (task_dashboard_action_button_system, count_intents).chain(),
            );

        let target = Entity::from_raw_u32(23).expect("valid test target");
        let capture_root = app.world_mut().spawn_empty().id();
        {
            let mut input = app.world_mut().resource_mut::<UiInputState>();
            input.world_input_captured = true;
            input.foreground_capture_root = Some(capture_root);
        }
        app.world_mut().spawn((
            Interaction::Pressed,
            TaskActionButton {
                target,
                expected_work_type: WorkType::Chop,
                kind: TaskActionButtonKind::Cancel(TaskCancelKind::GenericDesignation),
            },
        ));

        app.update();
        app.world_mut()
            .resource_mut::<UiInputState>()
            .world_input_captured = false;
        app.update();

        assert_eq!(app.world().resource::<IntentReceiptCount>().0, 0);
        assert!(
            app.world()
                .resource::<TaskDashboardActionState>()
                .confirmation
                .is_none()
        );

        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        app.world_mut().spawn((
            Interaction::Pressed,
            TaskActionButton {
                target,
                expected_work_type: WorkType::Chop,
                kind: TaskActionButtonKind::Cancel(TaskCancelKind::GenericDesignation),
            },
        ));
        app.update();
        app.world_mut().resource_mut::<Time<Virtual>>().unpause();
        app.update();

        assert_eq!(app.world().resource::<IntentReceiptCount>().0, 0);
        assert!(
            app.world()
                .resource::<TaskDashboardActionState>()
                .confirmation
                .is_none()
        );
    }

    #[test]
    fn task_dashboard_cancel_intents_route_through_task_owners() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<UiInputState>()
            .init_resource::<PendingWorldInputCapture>()
            .init_resource::<OutcomeReceipts>()
            .add_message::<UiIntent>()
            .add_message::<TaskActionOutcome>()
            .add_message::<hw_core::events::SoulTaskUnassignRequest>()
            .add_systems(
                Update,
                (apply_task_action_intents_system, collect_outcomes).chain(),
            );

        let owner = app.world_mut().spawn_empty().id();
        let generic = app
            .world_mut()
            .spawn((
                Designation {
                    work_type: WorkType::Chop,
                },
                Tree,
                PlayerIssuedDesignation,
                Priority(10),
                hw_jobs::TaskSlots::new(1),
                hw_core::relationships::ManagedBy(owner),
            ))
            .id();

        let source = app
            .world_mut()
            .spawn(hw_logistics::transport_request::ManualHaulPinnedSource)
            .id();
        let manual = app
            .world_mut()
            .spawn((
                Designation {
                    work_type: WorkType::Haul,
                },
                Priority(5),
                ManualTransportRequest,
                TransportRequestFixedSource(source),
                TransportRequest {
                    kind: TransportRequestKind::DepositToStockpile,
                    anchor: owner,
                    resource_type: hw_core::logistics::ResourceType::Wood,
                    issued_by: owner,
                    priority: hw_logistics::transport_request::TransportPriority::Normal,
                    stockpile_group: Vec::new(),
                },
            ))
            .id();

        app.world_mut().write_message(UiIntent::CancelTask {
            entity: generic,
            expected_work_type: WorkType::Chop,
            expected_kind: TaskCancelKind::GenericDesignation,
        });
        app.world_mut().write_message(UiIntent::CancelTask {
            entity: manual,
            expected_work_type: WorkType::Haul,
            expected_kind: TaskCancelKind::ManualTransportRequest,
        });
        app.update();

        assert!(app.world().get_entity(generic).is_ok());
        assert!(app.world().get::<Designation>(generic).is_none());
        assert!(app.world().get::<Priority>(generic).is_none());
        assert!(
            app.world()
                .get::<PlayerIssuedDesignation>(generic)
                .is_none()
        );
        assert!(app.world().get::<hw_jobs::TaskSlots>(generic).is_none());
        assert!(
            app.world()
                .get::<hw_core::relationships::ManagedBy>(generic)
                .is_none()
        );

        assert!(app.world().get_entity(manual).is_err());
        assert!(
            app.world()
                .get::<hw_logistics::transport_request::ManualHaulPinnedSource>(source)
                .is_none()
        );
        assert_eq!(
            app.world().resource::<OutcomeReceipts>().0,
            vec![
                TaskActionOutcome {
                    entity: generic,
                    action: TaskActionKind::Cancel,
                    result: TaskActionResult::CancellationRequested,
                },
                TaskActionOutcome {
                    entity: manual,
                    action: TaskActionKind::Cancel,
                    result: TaskActionResult::CancellationRequested,
                },
            ]
        );
    }

    #[test]
    fn task_dashboard_action_applies_priority_only_after_live_revalidation() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<UiInputState>()
            .init_resource::<PendingWorldInputCapture>()
            .init_resource::<OutcomeReceipts>()
            .add_message::<UiIntent>()
            .add_message::<TaskActionOutcome>()
            .add_systems(
                Update,
                (apply_task_action_intents_system, collect_outcomes).chain(),
            );

        let task = app
            .world_mut()
            .spawn((
                Designation {
                    work_type: WorkType::Chop,
                },
                Tree,
                PlayerIssuedDesignation,
                Priority(2),
            ))
            .id();
        app.world_mut().write_message(UiIntent::AdjustTaskPriority {
            entity: task,
            expected_work_type: WorkType::Chop,
            adjustment: TaskPriorityAdjustment::Increase,
        });

        app.update();

        assert_eq!(app.world().get::<Priority>(task).unwrap().0, 5);
        assert_eq!(
            app.world().resource::<OutcomeReceipts>().0,
            vec![TaskActionOutcome {
                entity: task,
                action: TaskActionKind::AdjustPriority(TaskPriorityAdjustment::Increase),
                result: TaskActionResult::PriorityChanged(TaskPriorityTier::High),
            }]
        );

        for (adjustment, expected_priority, expected_tier) in [
            (
                TaskPriorityAdjustment::Increase,
                10,
                TaskPriorityTier::Critical,
            ),
            (TaskPriorityAdjustment::Decrease, 5, TaskPriorityTier::High),
            (
                TaskPriorityAdjustment::Decrease,
                0,
                TaskPriorityTier::Normal,
            ),
            (TaskPriorityAdjustment::Increase, 5, TaskPriorityTier::High),
        ] {
            app.world_mut().write_message(UiIntent::AdjustTaskPriority {
                entity: task,
                expected_work_type: WorkType::Chop,
                adjustment,
            });
            app.update();
            assert_eq!(
                app.world().get::<Priority>(task).unwrap().0,
                expected_priority
            );
            assert_eq!(
                app.world()
                    .resource::<OutcomeReceipts>()
                    .0
                    .last()
                    .unwrap()
                    .result,
                TaskActionResult::PriorityChanged(expected_tier)
            );
        }

        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        app.world_mut().write_message(UiIntent::AdjustTaskPriority {
            entity: task,
            expected_work_type: WorkType::Chop,
            adjustment: TaskPriorityAdjustment::Increase,
        });
        app.update();
        assert_eq!(app.world().get::<Priority>(task).unwrap().0, 5);
        assert_eq!(
            app.world()
                .resource::<OutcomeReceipts>()
                .0
                .last()
                .unwrap()
                .result,
            TaskActionResult::Paused
        );

        app.world_mut().resource_mut::<Time<Virtual>>().unpause();
        app.world_mut()
            .resource_mut::<UiInputState>()
            .world_input_captured = true;
        app.world_mut().write_message(UiIntent::AdjustTaskPriority {
            entity: task,
            expected_work_type: WorkType::Chop,
            adjustment: TaskPriorityAdjustment::Increase,
        });
        app.update();
        assert_eq!(app.world().get::<Priority>(task).unwrap().0, 5);
        assert_eq!(
            app.world()
                .resource::<OutcomeReceipts>()
                .0
                .last()
                .unwrap()
                .result,
            TaskActionResult::Captured
        );

        app.world_mut()
            .resource_mut::<UiInputState>()
            .world_input_captured = false;
        let outcomes_before = app.world().resource::<OutcomeReceipts>().0.len();
        app.update();
        assert_eq!(app.world().get::<Priority>(task).unwrap().0, 5);
        assert_eq!(
            app.world().resource::<OutcomeReceipts>().0.len(),
            outcomes_before,
            "rejected intents must be drained instead of applying later",
        );
    }
}
