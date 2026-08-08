use bevy::ecs::system::{ParamSet, SystemParam};
use bevy::prelude::*;
use hw_ui::UiIntent;

use super::handlers;
use super::handlers::familiar_settings::FamiliarSettingsIntentCtx;
use super::intent_context::{
    IntentDomainActionCtx, IntentFamiliarQueries, IntentModeCtx, IntentSelectionCtx,
    IntentUiQueries,
};
use crate::input_actions::PendingWorldInputCapture;
use crate::interface::ui::help_controller::{HelpPauseGuard, HelpScrollAreas, handle_help_intent};

#[derive(SystemParam)]
pub(crate) struct IntentHelpCtx<'w, 's> {
    pending: Res<'w, PendingWorldInputCapture>,
    content: Res<'w, hw_ui::help::HelpPanelContent>,
    state: ResMut<'w, hw_ui::help::HelpPanelState>,
    guard: ResMut<'w, HelpPauseGuard>,
    scroll_areas: HelpScrollAreas<'w, 's>,
}

#[derive(SystemParam)]
pub(crate) struct IntentSettingsCtx<'w> {
    settings: ResMut<'w, hw_core::GameSettings>,
    debug_visible: ResMut<'w, crate::DebugVisible>,
    config_store: ResMut<'w, GizmoConfigStore>,
}

#[derive(SystemParam)]
pub(crate) struct IntentAuxCtx<'w, 's> {
    settings: IntentSettingsCtx<'w>,
    help: IntentHelpCtx<'w, 's>,
}

pub(crate) fn handle_ui_intent(
    mut ui_intents: MessageReader<UiIntent>,
    mut action_contexts: ParamSet<(IntentModeCtx, IntentDomainActionCtx)>,
    mut selection_ctx: IntentSelectionCtx,
    familiar_queries: IntentFamiliarQueries,
    mut ui_queries: IntentUiQueries,
    mut familiar_settings_ctx: FamiliarSettingsIntentCtx,
    mut aux_ctx: IntentAuxCtx,
) {
    for intent in ui_intents.read().cloned() {
        let should_save_settings = match intent {
            UiIntent::OpenHelp { .. } | UiIntent::CloseHelp => {
                let mut mode_ctx = action_contexts.p0();
                handle_help_intent(
                    intent,
                    &aux_ctx.help.pending,
                    &aux_ctx.help.content,
                    &mut aux_ctx.help.state,
                    &mut aux_ctx.help.guard,
                    &mut mode_ctx.time,
                    &mut aux_ctx.help.scroll_areas,
                );
                false
            }
            UiIntent::SelectHelpTopic(_) | UiIntent::StepHelpTopic(_) | UiIntent::ScrollHelp(_) => {
                false
            }
            UiIntent::InspectEntity(_) | UiIntent::ClearInspectPin => {
                handlers::handle_selection(intent, &mut selection_ctx);
                false
            }
            UiIntent::ToggleArchitect
            | UiIntent::ToggleOrders
            | UiIntent::ToggleZones
            | UiIntent::ToggleDream => {
                handlers::handle_toggle(intent, &mut action_contexts.p0());
                false
            }
            UiIntent::SelectBuild(_)
            | UiIntent::SelectFloorPlace
            | UiIntent::SelectZone(_)
            | UiIntent::RemoveZone(_)
            | UiIntent::SelectTaskMode(_)
            | UiIntent::SelectAreaTask
            | UiIntent::SelectDreamPlanting
            | UiIntent::BeginStockpilePolicyRangeEdit { .. } => {
                handlers::handle_mode_select(
                    intent,
                    &mut action_contexts.p0(),
                    &mut selection_ctx,
                    &familiar_queries,
                );
                false
            }
            UiIntent::OpenOperationDialog { .. } | UiIntent::CloseDialog => {
                handlers::handle_dialog(
                    intent,
                    &aux_ctx.help.pending,
                    &familiar_queries,
                    &mut familiar_settings_ctx.dialog_state,
                    &mut ui_queries,
                );
                false
            }
            UiIntent::ApplyFamiliarSettings { .. } | UiIntent::ApplyFamiliarSettingsFor { .. } => {
                let simulation_paused = action_contexts.p0().time.is_paused();
                handlers::handle_familiar_settings(
                    intent,
                    &aux_ctx.help.pending,
                    simulation_paused,
                    &mut familiar_settings_ctx,
                );
                false
            }
            UiIntent::TogglePause | UiIntent::SetTimeSpeed(_) => {
                handlers::handle_time(
                    intent,
                    &mut action_contexts.p0().time,
                    &mut ui_queries.input_focus,
                );
                false
            }
            UiIntent::SaveGame
            | UiIntent::RequestLoadGame
            | UiIntent::ConfirmLoadGame
            | UiIntent::CancelLoadConfirm => {
                handlers::handle_save_game(intent, &mut ui_queries);
                false
            }
            UiIntent::ToggleSettings
            | UiIntent::CloseSettings
            | UiIntent::SetUiScale(_)
            | UiIntent::SetCameraPanSpeed(_)
            | UiIntent::SetCameraMousePanEnabled(_)
            | UiIntent::SetDefaultTimeSpeed(_)
            | UiIntent::SetDebugGizmosEnabled(_)
            | UiIntent::SetFpsDisplayEnabled(_)
            | UiIntent::SetPowerPriorityEnabled(_) => {
                let mut mode_ctx = action_contexts.p0();
                handlers::handle_settings(
                    intent,
                    &mut aux_ctx.settings.settings,
                    &mut mode_ctx.cleanup.menu_state,
                    &mut aux_ctx.settings.debug_visible,
                    &mut aux_ctx.settings.config_store,
                    &mut ui_queries.input_focus,
                )
            }
            UiIntent::ToggleDoorLock(entity) => {
                action_contexts.p1().toggle_door_lock(entity);
                false
            }
            UiIntent::SelectArchitectCategory(category) => {
                action_contexts.p1().toggle_architect_category(category);
                false
            }
            UiIntent::MovePlantBuilding(entity) => {
                let target_is_valid = !selection_ctx.resolved_frame.pointer_selection_suppressed()
                    && action_contexts.p1().is_move_plant_target(entity);
                if target_is_valid {
                    let mut mode_ctx = action_contexts.p0();
                    mode_ctx.cancel_active_mode_if_needed();
                    selection_ctx.selected_entity.0 = Some(entity);
                    mode_ctx.cleanup.move_context.0 = Some(entity);
                    mode_ctx.cleanup.move_placement_state.0 = None;
                    mode_ctx.cleanup.companion_state.0 = None;
                    mode_ctx
                        .cleanup
                        .next_play_mode
                        .set(hw_core::game_state::PlayMode::BuildingMove);
                }
                false
            }
            UiIntent::ApplyStockpilePolicy { target, patch } => {
                action_contexts
                    .p1()
                    .request_stockpile_policy_change(target, patch);
                false
            }
            UiIntent::SetSoulSpaActiveSlots {
                target,
                active_slots,
            } => {
                action_contexts
                    .p1()
                    .set_soul_spa_active_slots(target, active_slots);
                false
            }
            UiIntent::CancelSoulSpaConstruction { target } => {
                let paused = action_contexts.p0().time.is_paused();
                action_contexts
                    .p1()
                    .cancel_soul_spa_construction(target, paused);
                false
            }
            UiIntent::SetPowerConsumerPriority { target, priority } => {
                action_contexts
                    .p1()
                    .set_power_consumer_priority(target, priority);
                false
            }
            UiIntent::AdjustTaskPriority { .. } | UiIntent::CancelTask { .. } => false,
        };

        handlers::save_if_requested(should_save_settings, &aux_ctx.settings.settings);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_contexts::{
        BuildContext, CompanionPlacementState, MoveContext, MovePlacementState, TaskContext,
        ZoneContext,
    };
    use crate::entities::familiar::{Familiar, FamiliarOperation, FamiliarPolicy};
    use crate::input_actions::{
        InputModifiers, ResolvedInputFrame, request_capture_from_menu_buttons_system,
        reset_pending_world_input_capture_system,
    };
    use crate::interface::selection::SelectedEntity;
    use crate::interface::ui::{EntityListNodeIndex, InfoPanelPinState};
    use crate::systems::command::{StockpilePolicyRangeEditState, ZoneRemovalPreviewState};
    use crate::systems::save::{SaveLoadState, SavePath};
    use crate::test_support::minimal_app;
    use bevy::ecs::system::{IntoSystem, System};
    use bevy::input_focus::InputFocus;
    use hw_core::game_state::{PlayMode, TaskMode};
    use hw_jobs::{Building, BuildingCategory, BuildingType, Door};
    use hw_spatial::SpatialGridOps;
    use hw_spatial::StockpileSpatialGrid;
    use hw_ui::StockpilePolicyEditTarget;
    use hw_ui::area_edit::AreaEditSession;
    use hw_ui::components::{
        ArchitectCategoryState, MenuState, OperationDialog, OperationDialogState, UiInputCapture,
        UiInputState,
    };
    use hw_world::{DoorVisualHandles, WorldMap};

    #[test]
    fn handler_system_params_are_conflict_free() {
        let mut app = minimal_app();
        app.add_message::<hw_energy::SoulSpaConstructionCancelRequest>();
        app.add_message::<hw_energy::SoulSpaConstructionCancelOutcome>();
        let mut system = IntoSystem::into_system(handle_ui_intent);

        system.initialize(app.world_mut());
    }

    fn domain_action_app() -> App {
        let mut app = minimal_app();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .add_message::<UiIntent>()
            .add_message::<hw_familiar_ai::FamiliarSettingsChangeRequest>()
            .add_message::<hw_familiar_ai::FamiliarSettingsChangeOutcome>()
            .add_message::<hw_logistics::StockpilePolicyChangeRequest>()
            .add_message::<hw_energy::SoulSpaSlotsChangeOutcome>()
            .add_message::<hw_energy::SoulSpaConstructionCancelRequest>()
            .add_message::<hw_energy::SoulSpaConstructionCancelOutcome>()
            .add_message::<hw_energy::PowerConsumerPolicyChangeOutcome>()
            .init_state::<PlayMode>()
            .init_resource::<BuildContext>()
            .init_resource::<MoveContext>()
            .init_resource::<MovePlacementState>()
            .init_resource::<ZoneContext>()
            .init_resource::<TaskContext>()
            .init_resource::<CompanionPlacementState>()
            .init_resource::<AreaEditSession>()
            .init_resource::<ZoneRemovalPreviewState>()
            .init_resource::<StockpilePolicyRangeEditState>()
            .init_resource::<StockpileSpatialGrid>()
            .init_resource::<WorldMap>()
            .init_resource::<MenuState>()
            .init_resource::<SelectedEntity>()
            .init_resource::<InfoPanelPinState>()
            .init_resource::<EntityListNodeIndex>()
            .init_resource::<ResolvedInputFrame>()
            .init_resource::<InputFocus>()
            .init_resource::<SaveLoadState>()
            .init_resource::<SavePath>()
            .init_resource::<hw_core::GameSettings>()
            .init_resource::<crate::DebugVisible>()
            .init_resource::<GizmoConfigStore>()
            .init_resource::<ArchitectCategoryState>()
            .init_resource::<PendingWorldInputCapture>()
            .init_resource::<hw_ui::components::UiInputState>()
            .init_resource::<hw_ui::components::OperationDialogState>()
            .init_resource::<hw_ui::help::HelpPanelState>()
            .init_resource::<HelpPauseGuard>()
            .insert_resource(hw_ui::help::HelpPanelContent::new([
                hw_ui::help::HelpSection::new(
                    hw_ui::help::HelpSectionId::new("test"),
                    "Test",
                    [hw_ui::help::HelpTopic::new(
                        hw_ui::help::HelpTopicId::new("test"),
                        "Test",
                        [],
                    )],
                ),
            ]))
            .insert_resource(DoorVisualHandles {
                door_open: Handle::default(),
                door_closed: Handle::default(),
            })
            .add_systems(Update, handle_ui_intent);
        app.update();
        app
    }

    fn write_intent(app: &mut App, intent: UiIntent) {
        app.world_mut()
            .resource_mut::<Messages<UiIntent>>()
            .write(intent);
    }

    fn spawn_building(app: &mut App, kind: BuildingType) -> Entity {
        app.world_mut()
            .spawn(Building {
                kind,
                is_provisional: false,
            })
            .id()
    }

    #[derive(Resource, Default)]
    struct StockpilePolicyRequests(Vec<hw_logistics::StockpilePolicyChangeRequest>);

    fn collect_stockpile_policy_requests(
        mut requests: MessageReader<hw_logistics::StockpilePolicyChangeRequest>,
        mut receipts: ResMut<StockpilePolicyRequests>,
    ) {
        receipts.0.extend(requests.read().cloned());
    }

    #[derive(Resource, Default)]
    struct FamiliarSettingsReceipts {
        requests: Vec<hw_familiar_ai::FamiliarSettingsChangeRequest>,
        outcomes: Vec<hw_familiar_ai::FamiliarSettingsChangeOutcome>,
    }

    fn collect_familiar_settings_receipts(
        mut requests: MessageReader<hw_familiar_ai::FamiliarSettingsChangeRequest>,
        mut outcomes: MessageReader<hw_familiar_ai::FamiliarSettingsChangeOutcome>,
        mut receipts: ResMut<FamiliarSettingsReceipts>,
    ) {
        receipts.requests.extend(requests.read().copied());
        receipts.outcomes.extend(outcomes.read().copied());
    }

    #[derive(Resource, Default)]
    struct SoulSpaSlotReceipts(Vec<hw_energy::SoulSpaSlotsChangeOutcome>);

    fn collect_soul_spa_slot_receipts(
        mut outcomes: MessageReader<hw_energy::SoulSpaSlotsChangeOutcome>,
        mut receipts: ResMut<SoulSpaSlotReceipts>,
    ) {
        receipts.0.extend(outcomes.read().copied());
    }

    #[derive(Resource, Default)]
    struct SoulSpaCancelReceipts {
        requests: Vec<hw_energy::SoulSpaConstructionCancelRequest>,
        outcomes: Vec<hw_energy::SoulSpaConstructionCancelOutcome>,
    }

    fn collect_soul_spa_cancel_receipts(
        mut requests: MessageReader<hw_energy::SoulSpaConstructionCancelRequest>,
        mut outcomes: MessageReader<hw_energy::SoulSpaConstructionCancelOutcome>,
        mut receipts: ResMut<SoulSpaCancelReceipts>,
    ) {
        receipts.requests.extend(requests.read().copied());
        receipts.outcomes.extend(outcomes.read().copied());
    }

    #[derive(Resource, Default)]
    struct PowerPolicyReceipts(Vec<hw_energy::PowerConsumerPolicyChangeOutcome>);

    fn collect_power_policy_receipts(
        mut outcomes: MessageReader<hw_energy::PowerConsumerPolicyChangeOutcome>,
        mut receipts: ResMut<PowerPolicyReceipts>,
    ) {
        receipts.0.extend(outcomes.read().copied());
    }

    #[test]
    fn soul_spa_slot_intents_are_terminal_clamped_and_do_not_kick_workers() {
        use hw_core::relationships::{TaskWorkers, WorkingOn};
        use hw_energy::{
            SOUL_SPA_MAX_ACTIVE_SLOTS, SoulSpaPhase, SoulSpaSite, SoulSpaSlotsChangeOutcome,
            SoulSpaSlotsChangeStatus, SoulSpaTile,
        };

        let mut app = domain_action_app();
        app.init_resource::<SoulSpaSlotReceipts>().add_systems(
            Update,
            collect_soul_spa_slot_receipts.after(handle_ui_intent),
        );
        let site = app
            .world_mut()
            .spawn(SoulSpaSite {
                phase: SoulSpaPhase::Operational,
                ..default()
            })
            .id();
        let tile = app
            .world_mut()
            .spawn(SoulSpaTile {
                parent_site: site,
                grid_pos: (0, 0),
            })
            .id();
        let worker = app.world_mut().spawn(WorkingOn(tile)).id();
        let constructing = app.world_mut().spawn(SoulSpaSite::default()).id();
        let unsupported = app.world_mut().spawn_empty().id();
        let stale = app.world_mut().spawn_empty().id();
        app.world_mut().despawn(stale);

        write_intent(
            &mut app,
            UiIntent::SetSoulSpaActiveSlots {
                target: site,
                active_slots: u32::MAX,
            },
        );
        write_intent(
            &mut app,
            UiIntent::SetSoulSpaActiveSlots {
                target: site,
                active_slots: 2,
            },
        );
        write_intent(
            &mut app,
            UiIntent::SetSoulSpaActiveSlots {
                target: constructing,
                active_slots: 1,
            },
        );
        write_intent(
            &mut app,
            UiIntent::SetSoulSpaActiveSlots {
                target: unsupported,
                active_slots: 1,
            },
        );
        write_intent(
            &mut app,
            UiIntent::SetSoulSpaActiveSlots {
                target: stale,
                active_slots: 1,
            },
        );

        app.update();

        assert_eq!(
            app.world().get::<SoulSpaSite>(site).unwrap().active_slots,
            2
        );
        assert_eq!(
            app.world()
                .get::<WorkingOn>(worker)
                .map(|working| working.0),
            Some(tile)
        );
        assert_eq!(app.world().get::<TaskWorkers>(tile).unwrap().len(), 1);
        assert_eq!(
            app.world()
                .get::<SoulSpaSite>(constructing)
                .unwrap()
                .active_slots,
            SOUL_SPA_MAX_ACTIVE_SLOTS
        );
        assert_eq!(
            app.world().resource::<SoulSpaSlotReceipts>().0,
            vec![
                SoulSpaSlotsChangeOutcome {
                    target: site,
                    status: SoulSpaSlotsChangeStatus::Applied {
                        requested: u32::MAX,
                        applied: SOUL_SPA_MAX_ACTIVE_SLOTS,
                        clamped: true,
                    },
                },
                SoulSpaSlotsChangeOutcome {
                    target: site,
                    status: SoulSpaSlotsChangeStatus::Applied {
                        requested: 2,
                        applied: 2,
                        clamped: false,
                    },
                },
                SoulSpaSlotsChangeOutcome {
                    target: constructing,
                    status: SoulSpaSlotsChangeStatus::PhaseUnavailable,
                },
                SoulSpaSlotsChangeOutcome {
                    target: unsupported,
                    status: SoulSpaSlotsChangeStatus::UnsupportedTarget,
                },
                SoulSpaSlotsChangeOutcome {
                    target: stale,
                    status: SoulSpaSlotsChangeStatus::StaleTarget,
                },
            ]
        );
    }

    #[test]
    fn soul_spa_construction_cancel_intent_routes_to_the_domain_owner() {
        let mut app = domain_action_app();
        app.init_resource::<SoulSpaCancelReceipts>().add_systems(
            Update,
            collect_soul_spa_cancel_receipts.after(handle_ui_intent),
        );
        let target = app.world_mut().spawn_empty().id();

        write_intent(&mut app, UiIntent::CancelSoulSpaConstruction { target });
        app.update();

        assert_eq!(
            app.world().resource::<SoulSpaCancelReceipts>().requests,
            vec![hw_energy::SoulSpaConstructionCancelRequest { target }]
        );
        assert!(
            app.world()
                .resource::<SoulSpaCancelReceipts>()
                .outcomes
                .is_empty()
        );
    }

    #[test]
    fn paused_soul_spa_cancel_is_rejected_once_without_buffering_a_request() {
        let mut app = domain_action_app();
        app.init_resource::<SoulSpaCancelReceipts>().add_systems(
            Update,
            collect_soul_spa_cancel_receipts.after(handle_ui_intent),
        );
        let target = app.world_mut().spawn_empty().id();
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        write_intent(&mut app, UiIntent::CancelSoulSpaConstruction { target });

        for _ in 0..3 {
            app.update();
        }
        app.world_mut().resource_mut::<Time<Virtual>>().unpause();
        app.update();

        let receipts = app.world().resource::<SoulSpaCancelReceipts>();
        assert!(receipts.requests.is_empty());
        assert_eq!(
            receipts.outcomes,
            vec![hw_energy::SoulSpaConstructionCancelOutcome {
                target,
                result: hw_energy::SoulSpaConstructionCancelResult::Paused,
            }]
        );
    }

    #[derive(Resource, Default)]
    struct ChangedSoulSpaCount(usize);

    fn count_changed_soul_spas(
        q_changed: Query<(), Changed<hw_energy::SoulSpaSite>>,
        mut count: ResMut<ChangedSoulSpaCount>,
    ) {
        count.0 += q_changed.iter().count();
    }

    #[test]
    fn exact_same_soul_spa_slot_intent_does_not_dirty_the_site() {
        use hw_energy::{SoulSpaPhase, SoulSpaSite};

        let mut app = domain_action_app();
        app.init_resource::<ChangedSoulSpaCount>()
            .add_systems(Update, count_changed_soul_spas.after(handle_ui_intent));
        let site = app
            .world_mut()
            .spawn(SoulSpaSite {
                phase: SoulSpaPhase::Operational,
                active_slots: 4,
                ..default()
            })
            .id();
        app.update();
        app.world_mut().resource_mut::<ChangedSoulSpaCount>().0 = 0;

        write_intent(
            &mut app,
            UiIntent::SetSoulSpaActiveSlots {
                target: site,
                active_slots: 4,
            },
        );
        app.update();

        assert_eq!(app.world().resource::<ChangedSoulSpaCount>().0, 0);
    }

    #[test]
    fn power_priority_intents_revalidate_target_and_never_repair_missing_policy() {
        use hw_energy::{
            PowerConsumer, PowerConsumerPolicy, PowerConsumerPolicyChangeOutcome,
            PowerConsumerPolicyChangeStatus, PowerPriority,
        };
        use hw_ui::power::PowerPriorityValue;

        let mut app = domain_action_app();
        app.init_resource::<PowerPolicyReceipts>().add_systems(
            Update,
            collect_power_policy_receipts.after(handle_ui_intent),
        );
        let consumer = app
            .world_mut()
            .spawn((
                PowerConsumer { demand: 1.0 },
                PowerConsumerPolicy {
                    priority: PowerPriority::Normal,
                },
            ))
            .id();
        let missing_policy = app.world_mut().spawn(PowerConsumer { demand: 1.0 }).id();
        app.world_mut()
            .entity_mut(missing_policy)
            .remove::<PowerConsumerPolicy>();
        let unsupported = app.world_mut().spawn_empty().id();
        let stale = app.world_mut().spawn_empty().id();
        app.world_mut().despawn(stale);

        for target in [consumer, missing_policy, unsupported, stale] {
            write_intent(
                &mut app,
                UiIntent::SetPowerConsumerPriority {
                    target,
                    priority: PowerPriorityValue::High,
                },
            );
        }
        app.update();

        assert_eq!(
            app.world().get::<PowerConsumerPolicy>(consumer),
            Some(&PowerConsumerPolicy {
                priority: PowerPriority::High,
            })
        );
        assert!(
            app.world()
                .get::<PowerConsumerPolicy>(missing_policy)
                .is_none()
        );
        assert_eq!(
            app.world().resource::<PowerPolicyReceipts>().0,
            vec![
                PowerConsumerPolicyChangeOutcome {
                    target: consumer,
                    status: PowerConsumerPolicyChangeStatus::Applied {
                        previous: PowerPriority::Normal,
                        applied: PowerPriority::High,
                    },
                },
                PowerConsumerPolicyChangeOutcome {
                    target: missing_policy,
                    status: PowerConsumerPolicyChangeStatus::MissingPolicy,
                },
                PowerConsumerPolicyChangeOutcome {
                    target: unsupported,
                    status: PowerConsumerPolicyChangeStatus::UnsupportedTarget,
                },
                PowerConsumerPolicyChangeOutcome {
                    target: stale,
                    status: PowerConsumerPolicyChangeStatus::StaleTarget,
                },
            ]
        );
    }

    #[test]
    fn operation_capture_latches_exact_target_independent_of_live_selection() {
        let mut app = domain_action_app();
        app.init_resource::<FamiliarSettingsReceipts>()
            .add_systems(
                Update,
                (
                    reset_pending_world_input_capture_system,
                    request_capture_from_menu_buttons_system,
                )
                    .chain()
                    .before(handle_ui_intent),
            )
            .add_systems(
                Update,
                collect_familiar_settings_receipts.after(handle_ui_intent),
            );
        let target_a = app
            .world_mut()
            .spawn((
                Familiar::default(),
                FamiliarOperation::default(),
                FamiliarPolicy::default(),
            ))
            .id();
        let target_b = app
            .world_mut()
            .spawn((
                Familiar::default(),
                FamiliarOperation::default(),
                FamiliarPolicy::default(),
            ))
            .id();
        app.world_mut().resource_mut::<SelectedEntity>().0 = Some(target_b);
        let root = app
            .world_mut()
            .spawn((
                Node {
                    display: Display::None,
                    ..default()
                },
                UiInputCapture,
                OperationDialog,
            ))
            .id();
        let opener = app
            .world_mut()
            .spawn((
                Interaction::Pressed,
                Button,
                hw_ui::components::MenuButton(UiIntent::OpenOperationDialog {
                    opener: None,
                    target: target_a,
                }),
            ))
            .id();
        write_intent(
            &mut app,
            UiIntent::OpenOperationDialog {
                opener: Some(opener),
                target: target_a,
            },
        );

        app.update();

        assert_eq!(
            app.world().resource::<OperationDialogState>().target,
            Some(target_a)
        );
        assert_eq!(
            app.world().get::<Node>(root).unwrap().display,
            Display::Flex
        );
        assert_eq!(
            app.world().resource::<SelectedEntity>().0,
            Some(target_b),
            "opening A must not depend on or rewrite the live selection"
        );

        {
            let mut ui_state = app.world_mut().resource_mut::<UiInputState>();
            ui_state.world_input_captured = true;
            ui_state.foreground_capture_root = Some(root);
        }
        write_intent(
            &mut app,
            UiIntent::ApplyFamiliarSettings {
                patch: hw_core::familiar::FamiliarSettingsPatch::SetAllWorkAllowed {
                    allowed: false,
                },
            },
        );
        app.update();

        assert_eq!(
            app.world().resource::<FamiliarSettingsReceipts>().requests,
            vec![hw_familiar_ai::FamiliarSettingsChangeRequest {
                target: target_a,
                patch: hw_core::familiar::FamiliarSettingsPatch::SetAllWorkAllowed {
                    allowed: false,
                },
            }]
        );
    }

    #[test]
    fn paused_or_captured_explicit_settings_intents_emit_rejection_without_request() {
        let mut app = domain_action_app();
        app.init_resource::<FamiliarSettingsReceipts>().add_systems(
            Update,
            collect_familiar_settings_receipts.after(handle_ui_intent),
        );
        let target = app.world_mut().spawn(Familiar::default()).id();
        let patch = hw_core::familiar::FamiliarSettingsPatch::AdjustMaxControlledSoul { delta: 1 };

        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        write_intent(
            &mut app,
            UiIntent::ApplyFamiliarSettingsFor { target, patch },
        );
        app.update();

        app.world_mut().resource_mut::<Time<Virtual>>().unpause();
        app.world_mut()
            .resource_mut::<UiInputState>()
            .world_input_captured = true;
        write_intent(
            &mut app,
            UiIntent::ApplyFamiliarSettingsFor { target, patch },
        );
        app.update();

        let receipts = app.world().resource::<FamiliarSettingsReceipts>();
        assert!(receipts.requests.is_empty());
        assert_eq!(receipts.outcomes.len(), 2);
        assert!(receipts.outcomes.iter().all(|outcome| {
            outcome.target == target
                && matches!(
                    outcome.status,
                    hw_familiar_ai::FamiliarSettingsChangeStatus::Rejected {
                        requested_patches: 1,
                        reason: hw_familiar_ai::FamiliarSettingsRejection::PausedOrModal,
                    }
                )
        }));
    }

    #[test]
    fn single_and_area_stockpile_policy_intents_share_the_same_domain_request_type() {
        let mut app = domain_action_app();
        app.init_resource::<StockpilePolicyRequests>().add_systems(
            Update,
            collect_stockpile_policy_requests.after(handle_ui_intent),
        );
        let left = app.world_mut().spawn_empty().id();
        let right = app.world_mut().spawn_empty().id();
        {
            let mut grid = app.world_mut().resource_mut::<StockpileSpatialGrid>();
            grid.insert(right, Vec2::new(16.0, 0.0));
            grid.insert(left, Vec2::ZERO);
        }
        let patch = hw_logistics::StockpilePolicyPatch {
            allow_export: Some(false),
            ..default()
        };

        write_intent(
            &mut app,
            UiIntent::ApplyStockpilePolicy {
                target: StockpilePolicyEditTarget::Single(right),
                patch,
            },
        );
        write_intent(
            &mut app,
            UiIntent::ApplyStockpilePolicy {
                target: StockpilePolicyEditTarget::Area {
                    min: Vec2::splat(32.0),
                    max: Vec2::splat(-1.0),
                },
                patch,
            },
        );
        app.update();

        assert_eq!(
            app.world().resource::<StockpilePolicyRequests>().0,
            vec![
                hw_logistics::StockpilePolicyChangeRequest {
                    targets: vec![right],
                    patch,
                },
                hw_logistics::StockpilePolicyChangeRequest {
                    targets: vec![left, right],
                    patch,
                },
            ]
        );
    }

    #[test]
    fn begin_stockpile_policy_range_intent_owns_mode_and_patch() {
        let mut app = domain_action_app();
        let patch = hw_logistics::StockpilePolicyPatch {
            target_amount: Some(4),
            ..default()
        };
        write_intent(&mut app, UiIntent::BeginStockpilePolicyRangeEdit { patch });

        app.update();

        assert_eq!(
            app.world().resource::<TaskContext>().0,
            TaskMode::StockpilePolicyEdit(None)
        );
        assert_eq!(
            app.world()
                .resource::<StockpilePolicyRangeEditState>()
                .patch,
            Some(patch)
        );
        assert!(matches!(
            *app.world().resource::<NextState<PlayMode>>(),
            NextState::Pending(PlayMode::TaskDesignation)
                | NextState::PendingIfNeq(PlayMode::TaskDesignation)
        ));
    }

    #[test]
    fn blocked_familiar_build_intent_is_a_strict_no_op() {
        let mut app = domain_action_app();
        let existing_mode = TaskMode::DesignateMine(None);
        app.world_mut().resource_mut::<TaskContext>().0 = existing_mode;
        *app.world_mut().resource_mut::<MenuState>() = MenuState::Orders;
        let familiar = app.world_mut().spawn(Familiar::default()).id();

        write_intent(
            &mut app,
            UiIntent::SelectTaskMode(TaskMode::SelectBuildTarget),
        );
        app.update();

        assert_eq!(app.world().resource::<TaskContext>().0, existing_mode);
        assert_eq!(*app.world().resource::<MenuState>(), MenuState::Orders);
        assert!(app.world().resource::<SelectedEntity>().0.is_none());
        assert!(matches!(
            *app.world().resource::<NextState<PlayMode>>(),
            NextState::Unchanged
        ));
        assert!(app.world().get_entity(familiar).is_ok());
    }

    #[test]
    fn deconstruction_task_mode_intent_enters_task_designation() {
        let mut app = domain_action_app();

        write_intent(
            &mut app,
            UiIntent::SelectTaskMode(TaskMode::DesignateDeconstruct(None)),
        );
        app.update();

        assert_eq!(
            app.world().resource::<TaskContext>().0,
            TaskMode::DesignateDeconstruct(None)
        );
        assert!(matches!(
            *app.world().resource::<NextState<PlayMode>>(),
            NextState::Pending(PlayMode::TaskDesignation)
                | NextState::PendingIfNeq(PlayMode::TaskDesignation)
        ));
        assert_eq!(*app.world().resource::<MenuState>(), MenuState::Hidden);
    }

    #[test]
    fn move_plant_intent_rejects_despawned_or_non_plant_target() {
        let mut app = domain_action_app();
        let wall = spawn_building(&mut app, BuildingType::Wall);
        write_intent(&mut app, UiIntent::MovePlantBuilding(wall));
        app.update();

        assert!(app.world().resource::<SelectedEntity>().0.is_none());
        assert!(app.world().resource::<MoveContext>().0.is_none());

        let stale = spawn_building(&mut app, BuildingType::Tank);
        assert!(app.world_mut().despawn(stale));
        write_intent(&mut app, UiIntent::MovePlantBuilding(stale));
        app.update();

        assert!(app.world().resource::<SelectedEntity>().0.is_none());
        assert!(app.world().resource::<MoveContext>().0.is_none());

        let pending = spawn_building(&mut app, BuildingType::Tank);
        let order = app.world_mut().spawn_empty().id();
        app.world_mut()
            .entity_mut(pending)
            .insert(hw_jobs::DeconstructionPending { order });
        write_intent(&mut app, UiIntent::MovePlantBuilding(pending));
        app.update();

        assert!(app.world().resource::<SelectedEntity>().0.is_none());
        assert!(app.world().resource::<MoveContext>().0.is_none());
    }

    #[test]
    fn pointer_suppression_blocks_move_plant_intent() {
        let mut app = domain_action_app();
        let plant = spawn_building(&mut app, BuildingType::Tank);
        app.world_mut()
            .resource_mut::<ResolvedInputFrame>()
            .replace(InputModifiers::default(), Vec::new(), None, true);
        write_intent(&mut app, UiIntent::MovePlantBuilding(plant));

        app.update();
        assert!(app.world().resource::<MoveContext>().0.is_none());

        app.world_mut()
            .resource_mut::<ResolvedInputFrame>()
            .replace(InputModifiers::default(), Vec::new(), None, false);
        app.update();

        assert!(app.world().resource::<MoveContext>().0.is_none());
        assert!(app.world().resource::<SelectedEntity>().0.is_none());
    }

    #[test]
    fn move_plant_intent_is_consumed_once() {
        let mut app = domain_action_app();
        let plant = spawn_building(&mut app, BuildingType::MudMixer);
        app.world_mut().resource_mut::<BuildContext>().0 = Some(BuildingType::Wall);
        app.world_mut().resource_mut::<TaskContext>().0 = TaskMode::DesignateChop(None);
        *app.world_mut().resource_mut::<MenuState>() = MenuState::Architect;
        write_intent(&mut app, UiIntent::MovePlantBuilding(plant));

        app.update();

        assert_eq!(app.world().resource::<SelectedEntity>().0, Some(plant));
        assert_eq!(app.world().resource::<MoveContext>().0, Some(plant));
        assert!(app.world().resource::<BuildContext>().0.is_none());
        assert_eq!(app.world().resource::<TaskContext>().0, TaskMode::None);
        assert_eq!(*app.world().resource::<MenuState>(), MenuState::Hidden);
        assert!(matches!(
            *app.world().resource::<NextState<PlayMode>>(),
            NextState::Pending(PlayMode::BuildingMove)
                | NextState::PendingIfNeq(PlayMode::BuildingMove)
        ));

        app.world_mut().resource_mut::<MoveContext>().0 = None;
        app.world_mut().resource_mut::<SelectedEntity>().0 = None;
        app.update();

        assert!(app.world().resource::<MoveContext>().0.is_none());
        assert!(app.world().resource::<SelectedEntity>().0.is_none());
    }

    #[test]
    fn move_action_cleanup_precedes_mode_and_menu_update() {
        let mut app = domain_action_app();
        let plant = spawn_building(&mut app, BuildingType::SoulSpa);
        app.world_mut().resource_mut::<BuildContext>().0 = Some(BuildingType::Tank);
        app.world_mut().resource_mut::<TaskContext>().0 = TaskMode::FloorPlace(Some(Vec2::ZERO));
        *app.world_mut().resource_mut::<MenuState>() = MenuState::Architect;
        write_intent(&mut app, UiIntent::MovePlantBuilding(plant));

        app.update();

        assert!(app.world().resource::<BuildContext>().0.is_none());
        assert_eq!(app.world().resource::<TaskContext>().0, TaskMode::None);
        assert_eq!(*app.world().resource::<MenuState>(), MenuState::Hidden);
        assert_eq!(app.world().resource::<SelectedEntity>().0, Some(plant));
        assert_eq!(app.world().resource::<MoveContext>().0, Some(plant));
        assert!(matches!(
            *app.world().resource::<NextState<PlayMode>>(),
            NextState::Pending(PlayMode::BuildingMove)
                | NextState::PendingIfNeq(PlayMode::BuildingMove)
        ));
    }

    #[test]
    fn door_and_architect_actions_have_single_intent_consumer() {
        let mut app = domain_action_app();
        let grid = (5, 5);
        let world = WorldMap::grid_to_world(grid.0, grid.1);
        let door = app
            .world_mut()
            .spawn((
                Door::default(),
                Transform::from_translation(world.extend(0.0)),
                Sprite::default(),
            ))
            .id();
        app.world_mut().resource_mut::<WorldMap>().register_door(
            grid,
            door,
            hw_core::world::DoorState::Closed,
        );
        write_intent(&mut app, UiIntent::ToggleDoorLock(door));
        write_intent(
            &mut app,
            UiIntent::SelectArchitectCategory(Some(BuildingCategory::Plant)),
        );

        app.update();

        assert_eq!(
            app.world().get::<Door>(door).unwrap().state,
            hw_core::world::DoorState::Locked
        );
        assert_eq!(
            app.world().resource::<ArchitectCategoryState>().0,
            Some(BuildingCategory::Plant)
        );
    }

    #[test]
    fn power_priority_checkbox_reaches_the_next_logic_allocation() {
        use bevy::ui_widgets::ValueChange;
        use hw_energy::{
            ConsumesFrom, GeneratesFor, PowerAllocationMode, PowerConsumer, PowerConsumerPolicy,
            PowerGenerator, PowerGrid, PowerPriority, PowerShedReason, PowerSupplyState,
        };
        use hw_ui::components::{SettingsCheckboxMarker, SettingsField};

        use crate::systems::energy::grid_recalc::{
            EnergyUpdateDirty, energy_grid_recalc_should_run, grid_recalc_system,
            sync_power_allocation_mode_from_settings_system,
        };

        let mut app = domain_action_app();
        app.init_resource::<PowerAllocationMode>()
            .init_resource::<EnergyUpdateDirty>()
            .add_observer(crate::systems::settings::on_settings_checkbox_value_change)
            .add_systems(
                Update,
                (
                    sync_power_allocation_mode_from_settings_system,
                    grid_recalc_system.run_if(energy_grid_recalc_should_run),
                    ApplyDeferred,
                )
                    .chain()
                    .after(handle_ui_intent),
            );
        let checkbox = app
            .world_mut()
            .spawn(SettingsCheckboxMarker(SettingsField::PowerPriority))
            .id();
        let grid = app.world_mut().spawn(PowerGrid::default()).id();
        app.world_mut().spawn((
            PowerGenerator {
                current_output: 0.6,
                output_per_soul: 1.0,
            },
            GeneratesFor(grid),
        ));
        let high = app
            .world_mut()
            .spawn((
                PowerConsumer { demand: 0.6 },
                PowerConsumerPolicy {
                    priority: PowerPriority::High,
                },
                Transform::from_xyz(0.0, 0.0, 0.0),
                ConsumesFrom(grid),
            ))
            .id();
        let low = app
            .world_mut()
            .spawn((
                PowerConsumer { demand: 0.6 },
                PowerConsumerPolicy {
                    priority: PowerPriority::Low,
                },
                Transform::from_xyz(16.0, 0.0, 0.0),
                ConsumesFrom(grid),
            ))
            .id();
        app.world_mut()
            .resource_mut::<EnergyUpdateDirty>()
            .grid_recalc_due = true;
        app.update();
        assert_eq!(
            app.world().get::<PowerSupplyState>(high),
            Some(&PowerSupplyState::Supplied)
        );

        app.world_mut().commands().trigger(ValueChange {
            source: checkbox,
            value: false,
            is_final: true,
        });
        app.world_mut().flush();
        app.update();

        assert!(
            !app.world()
                .resource::<hw_core::GameSettings>()
                .power_priority_enabled
        );
        assert_eq!(
            *app.world().resource::<PowerAllocationMode>(),
            PowerAllocationMode::LegacyAllOrNone
        );
        for consumer in [high, low] {
            assert_eq!(
                app.world().get::<PowerSupplyState>(consumer),
                Some(&PowerSupplyState::Shed {
                    reason: PowerShedReason::LegacyGlobalDeficit,
                })
            );
        }

        app.world_mut().commands().trigger(ValueChange {
            source: checkbox,
            value: true,
            is_final: true,
        });
        app.world_mut().flush();
        app.update();

        assert!(
            app.world()
                .resource::<hw_core::GameSettings>()
                .power_priority_enabled
        );
        assert_eq!(
            *app.world().resource::<PowerAllocationMode>(),
            PowerAllocationMode::PriorityPrefix
        );
        assert_eq!(
            app.world().get::<PowerSupplyState>(high),
            Some(&PowerSupplyState::Supplied),
            "returning from Legacy must rebuild Priority as a cold start"
        );
        assert!(matches!(
            app.world().get::<PowerSupplyState>(low),
            Some(PowerSupplyState::Shed {
                reason: PowerShedReason::InsufficientGeneration,
            })
        ));
    }
}
