use bevy::ecs::system::{ParamSet, SystemParam};
use bevy::prelude::*;
use hw_ui::UiIntent;

use super::handlers;
use super::intent_context::{
    IntentDomainActionCtx, IntentFamiliarQueries, IntentModeCtx, IntentSelectionCtx,
    IntentUiQueries,
};
use crate::FamiliarOperationMaxSoulChangedEvent;

#[derive(SystemParam)]
pub(crate) struct IntentSettingsCtx<'w> {
    settings: ResMut<'w, hw_core::GameSettings>,
    debug_visible: ResMut<'w, crate::DebugVisible>,
    config_store: ResMut<'w, GizmoConfigStore>,
}

pub(crate) fn handle_ui_intent(
    mut ui_intents: MessageReader<UiIntent>,
    mut action_contexts: ParamSet<(IntentModeCtx, IntentDomainActionCtx)>,
    mut selection_ctx: IntentSelectionCtx,
    mut familiar_queries: IntentFamiliarQueries,
    mut ui_queries: IntentUiQueries,
    mut ev_max_soul_changed: MessageWriter<FamiliarOperationMaxSoulChangedEvent>,
    mut settings_ctx: IntentSettingsCtx,
) {
    for intent in ui_intents.read().cloned() {
        let should_save_settings = match intent {
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
            UiIntent::OpenOperationDialog | UiIntent::CloseDialog => {
                let can_open_operation = selection_ctx.selected_entity.0.is_some_and(|entity| {
                    familiar_queries.q_familiars_for_area.get(entity).is_ok()
                });
                handlers::handle_dialog(intent, can_open_operation, &mut ui_queries);
                false
            }
            UiIntent::AdjustFatigueThreshold(_)
            | UiIntent::AdjustMaxControlledSoul(_)
            | UiIntent::AdjustMaxControlledSoulFor(..) => {
                handlers::handle_familiar_settings(
                    intent,
                    &mut selection_ctx,
                    &mut familiar_queries,
                    &mut ui_queries,
                    &mut ev_max_soul_changed,
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
            | UiIntent::SetFpsDisplayEnabled(_) => {
                let mut mode_ctx = action_contexts.p0();
                handlers::handle_settings(
                    intent,
                    &mut settings_ctx.settings,
                    &mut mode_ctx.cleanup.menu_state,
                    &mut settings_ctx.debug_visible,
                    &mut settings_ctx.config_store,
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
            UiIntent::AdjustTaskPriority { .. } | UiIntent::CancelTask { .. } => false,
        };

        handlers::save_if_requested(should_save_settings, &settings_ctx.settings);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_contexts::{
        BuildContext, CompanionPlacementState, MoveContext, MovePlacementState, TaskContext,
        ZoneContext,
    };
    use crate::input_actions::{InputModifiers, ResolvedInputFrame};
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
    use hw_ui::components::{ArchitectCategoryState, MenuState};
    use hw_world::{DoorVisualHandles, WorldMap};

    #[test]
    fn handler_system_params_are_conflict_free() {
        let mut app = minimal_app();
        let mut system = IntoSystem::into_system(handle_ui_intent);

        system.initialize(app.world_mut());
    }

    fn domain_action_app() -> App {
        let mut app = minimal_app();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .add_message::<UiIntent>()
            .add_message::<FamiliarOperationMaxSoulChangedEvent>()
            .add_message::<hw_logistics::StockpilePolicyChangeRequest>()
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
}
