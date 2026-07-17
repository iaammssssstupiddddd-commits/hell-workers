use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::game_state::{PlayMode, TaskMode};
use hw_ui::area_edit::AreaEditSession;
use hw_ui::components::MenuState;

use super::{InputAction, ResolvedInputFrame};
use crate::app_contexts::{
    BuildContext, CompanionPlacementState, MoveContext, MovePlacementState, TaskContext,
    ZoneContext,
};
use crate::entities::damned_soul::Destination;
use crate::entities::familiar::{ActiveCommand, Familiar};
use crate::interface::selection::building_move::clear_move_states;
use crate::systems::command::zone_placement::ZoneRemovalPreviewState;
use crate::systems::command::zone_placement::removal_preview::clear_removal_preview;
use crate::world::map::WorldMap;

/// All mutable owner state that must be cleared together when an active mode ends.
#[derive(SystemParam)]
pub struct ActiveModeCleanupParams<'w, 's> {
    pub(crate) commands: Commands<'w, 's>,
    pub(crate) menu_state: ResMut<'w, MenuState>,
    pub(crate) next_play_mode: ResMut<'w, NextState<PlayMode>>,
    pub(crate) build_context: ResMut<'w, BuildContext>,
    pub(crate) move_context: ResMut<'w, MoveContext>,
    pub(crate) move_placement_state: ResMut<'w, MovePlacementState>,
    pub(crate) zone_context: ResMut<'w, ZoneContext>,
    pub(crate) task_context: ResMut<'w, TaskContext>,
    pub(crate) companion_state: ResMut<'w, CompanionPlacementState>,
    pub(crate) area_edit_session: ResMut<'w, AreaEditSession>,
    zone_removal_preview: ResMut<'w, ZoneRemovalPreviewState>,
    world_map: Res<'w, WorldMap>,
    q_familiar_state:
        Query<'w, 's, (&'static mut ActiveCommand, &'static mut Destination), With<Familiar>>,
    q_sprites: Query<'w, 's, &'static mut Sprite>,
}

impl ActiveModeCleanupParams<'_, '_> {
    pub(crate) fn has_active_owner_state(&self, current_play_mode: &PlayMode) -> bool {
        current_play_mode != &PlayMode::Normal
            || matches!(
                &*self.next_play_mode,
                NextState::Pending(mode) | NextState::PendingIfNeq(mode)
                    if mode != &PlayMode::Normal
            )
            || self.build_context.0.is_some()
            || self.move_context.0.is_some()
            || self.move_placement_state.0.is_some()
            || self.zone_context.0.is_some()
            || self.task_context.0 != TaskMode::None
            || self.companion_state.0.is_some()
            || self.area_edit_session.active_drag.is_some()
            || self.area_edit_session.dream_planting_preview_seed.is_some()
            || self.zone_removal_preview.is_active()
    }

    fn restore_active_area_edit_drag(&mut self) {
        if let Some(active_drag) = self.area_edit_session.active_drag.take() {
            self.commands
                .entity(active_drag.familiar_entity)
                .insert(active_drag.original_area);
            if let Ok((mut active_command, mut destination)) =
                self.q_familiar_state.get_mut(active_drag.familiar_entity)
            {
                active_command.command = active_drag.original_command;
                destination.0 = active_drag.original_destination;
            }
        }
    }

    /// Rolls back only an uncommitted pointer gesture while preserving its mode owner.
    pub(crate) fn rollback_in_progress_gesture(&mut self) {
        self.restore_active_area_edit_drag();

        self.task_context.0 = match self.task_context.0 {
            TaskMode::DesignateChop(Some(_)) => TaskMode::DesignateChop(None),
            TaskMode::DesignateMine(Some(_)) => TaskMode::DesignateMine(None),
            TaskMode::DesignateHaul(Some(_)) => TaskMode::DesignateHaul(None),
            TaskMode::CancelDesignation(Some(_)) => TaskMode::CancelDesignation(None),
            TaskMode::AreaSelection(Some(_)) => TaskMode::AreaSelection(None),
            TaskMode::AssignTask(Some(_)) => TaskMode::AssignTask(None),
            TaskMode::ZonePlacement(kind, Some(_)) => TaskMode::ZonePlacement(kind, None),
            TaskMode::ZoneRemoval(kind, Some(_)) => TaskMode::ZoneRemoval(kind, None),
            TaskMode::FloorPlace(Some(_)) => TaskMode::FloorPlace(None),
            TaskMode::WallPlace(Some(_)) => TaskMode::WallPlace(None),
            TaskMode::DreamPlanting(Some(_)) => TaskMode::DreamPlanting(None),
            mode => mode,
        };

        self.area_edit_session.dream_planting_preview_seed = None;
        if self.zone_removal_preview.is_active() {
            clear_removal_preview(
                &self.world_map,
                &mut self.q_sprites,
                &mut self.zone_removal_preview,
            );
        }
    }

    pub(crate) fn cancel_active_mode(&mut self) {
        self.restore_active_area_edit_drag();
        self.area_edit_session.dream_planting_preview_seed = None;

        clear_removal_preview(
            &self.world_map,
            &mut self.q_sprites,
            &mut self.zone_removal_preview,
        );
        clear_move_states(
            &mut self.move_context,
            &mut self.move_placement_state,
            &mut self.companion_state,
        );
        self.build_context.0 = None;
        self.zone_context.0 = None;
        self.task_context.0 = TaskMode::None;
        *self.menu_state = MenuState::Hidden;
        self.next_play_mode.set(PlayMode::Normal);
    }

    pub(crate) fn close_open_menu(&mut self) {
        *self.menu_state = MenuState::Hidden;
    }
}

pub(crate) fn cancel_or_close_input_action_system(
    resolved_frame: Res<ResolvedInputFrame>,
    mut cleanup: ActiveModeCleanupParams,
) {
    if resolved_frame.contains(InputAction::CancelActiveMode) {
        cleanup.cancel_active_mode();
    } else if resolved_frame.contains(InputAction::CloseOpenMenu) {
        cleanup.close_open_menu();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_contexts::{CompanionPlacementState, PendingMovePlacement};
    use crate::entities::familiar::FamiliarCommand;
    use crate::input_actions::InputModifiers;
    use crate::systems::command::TaskArea;
    use crate::test_support::minimal_app;
    use hw_ui::area_edit::{AreaEditDrag, AreaEditOperation};

    fn cleanup_app(action: InputAction) -> App {
        let mut app = minimal_app();
        app.init_resource::<ResolvedInputFrame>()
            .init_resource::<MenuState>()
            .init_resource::<NextState<PlayMode>>()
            .init_resource::<BuildContext>()
            .init_resource::<MoveContext>()
            .init_resource::<MovePlacementState>()
            .init_resource::<ZoneContext>()
            .init_resource::<TaskContext>()
            .init_resource::<CompanionPlacementState>()
            .init_resource::<AreaEditSession>()
            .init_resource::<ZoneRemovalPreviewState>()
            .init_resource::<WorldMap>()
            .add_systems(Update, cancel_or_close_input_action_system);
        app.world_mut()
            .resource_mut::<ResolvedInputFrame>()
            .replace(InputModifiers::default(), vec![action], None, true);
        app
    }

    #[test]
    fn active_mode_cancel_restores_area_drag_and_clears_all_owner_state() {
        let mut app = cleanup_app(InputAction::CancelActiveMode);
        let original_area = TaskArea::from_points(Vec2::ZERO, Vec2::splat(2.0));
        let changed_area = TaskArea::from_points(Vec2::splat(4.0), Vec2::splat(8.0));
        let familiar = app
            .world_mut()
            .spawn((
                Familiar::default(),
                ActiveCommand {
                    command: FamiliarCommand::Patrol,
                },
                Destination(Vec2::splat(9.0)),
                changed_area,
            ))
            .id();
        app.world_mut()
            .resource_mut::<AreaEditSession>()
            .active_drag = Some(AreaEditDrag {
            familiar_entity: familiar,
            operation: AreaEditOperation::Move,
            original_area: original_area.clone(),
            original_destination: Vec2::ONE,
            original_command: FamiliarCommand::Idle,
            drag_start: Vec2::ZERO,
        });
        app.world_mut().resource_mut::<BuildContext>().0 =
            Some(crate::systems::jobs::BuildingType::Tank);
        app.world_mut().resource_mut::<MoveContext>().0 = Some(familiar);
        app.world_mut().resource_mut::<MovePlacementState>().0 = Some(PendingMovePlacement {
            building: familiar,
            destination_grid: (4, 5),
        });
        *app.world_mut().resource_mut::<MenuState>() = MenuState::Architect;
        app.world_mut().resource_mut::<TaskContext>().0 = TaskMode::AreaSelection(Some(Vec2::ZERO));

        app.update();

        assert_eq!(
            app.world().entity(familiar).get::<TaskArea>(),
            Some(&original_area)
        );
        assert_eq!(
            app.world().entity(familiar).get::<Destination>().unwrap().0,
            Vec2::ONE
        );
        assert_eq!(
            app.world()
                .entity(familiar)
                .get::<ActiveCommand>()
                .unwrap()
                .command,
            FamiliarCommand::Idle
        );
        assert!(
            app.world()
                .resource::<AreaEditSession>()
                .active_drag
                .is_none()
        );
        assert!(app.world().resource::<MoveContext>().0.is_none());
        assert!(app.world().resource::<MovePlacementState>().0.is_none());
        assert_eq!(*app.world().resource::<MenuState>(), MenuState::Hidden);
        assert_eq!(app.world().resource::<TaskContext>().0, TaskMode::None);
        assert!(matches!(
            *app.world().resource::<NextState<PlayMode>>(),
            NextState::Pending(PlayMode::Normal)
        ));
    }

    #[test]
    fn close_open_menu_does_not_cancel_background_mode_state() {
        let mut app = cleanup_app(InputAction::CloseOpenMenu);
        *app.world_mut().resource_mut::<MenuState>() = MenuState::Zones;
        app.world_mut().resource_mut::<TaskContext>().0 = TaskMode::SelectBuildTarget;

        app.update();

        assert_eq!(*app.world().resource::<MenuState>(), MenuState::Hidden);
        assert_eq!(
            app.world().resource::<TaskContext>().0,
            TaskMode::SelectBuildTarget
        );
    }

    fn cancel_if_owner_state_is_active(
        play_mode: Res<State<PlayMode>>,
        mut cleanup: ActiveModeCleanupParams,
    ) {
        if cleanup.has_active_owner_state(play_mode.get()) {
            cleanup.cancel_active_mode();
        }
    }

    fn rollback_gesture(mut cleanup: ActiveModeCleanupParams) {
        cleanup.rollback_in_progress_gesture();
    }

    #[test]
    fn capture_rollback_preserves_mode_owner_and_committed_dream_request() {
        let mut app = cleanup_app(InputAction::CloseOpenMenu);
        app.add_systems(Update, rollback_gesture);
        app.world_mut().resource_mut::<TaskContext>().0 =
            TaskMode::DreamPlanting(Some(Vec2::splat(3.0)));
        {
            let mut session = app.world_mut().resource_mut::<AreaEditSession>();
            session.dream_planting_preview_seed = Some(41);
            session.pending_dream_planting = Some((Vec2::ZERO, Vec2::ONE, 17));
        }

        app.update();

        assert_eq!(
            app.world().resource::<TaskContext>().0,
            TaskMode::DreamPlanting(None)
        );
        let session = app.world().resource::<AreaEditSession>();
        assert_eq!(session.dream_planting_preview_seed, None);
        assert_eq!(
            session.pending_dream_planting,
            Some((Vec2::ZERO, Vec2::ONE, 17))
        );
        assert!(matches!(
            *app.world().resource::<NextState<PlayMode>>(),
            NextState::Unchanged
        ));
    }

    #[test]
    fn ui_cleanup_detects_task_owner_while_play_mode_is_normal() {
        let mut app = cleanup_app(InputAction::CloseOpenMenu);
        app.insert_resource(State::new(PlayMode::Normal));
        app.world_mut().resource_mut::<TaskContext>().0 = TaskMode::DesignateChop(None);
        app.add_systems(Update, cancel_if_owner_state_is_active);

        app.update();

        assert_eq!(app.world().resource::<TaskContext>().0, TaskMode::None);
        assert!(matches!(
            *app.world().resource::<NextState<PlayMode>>(),
            NextState::Pending(PlayMode::Normal)
        ));
    }

    #[test]
    fn ui_cleanup_replaces_pending_non_normal_transition() {
        let mut app = cleanup_app(InputAction::CloseOpenMenu);
        app.insert_resource(State::new(PlayMode::Normal));
        app.world_mut()
            .resource_mut::<NextState<PlayMode>>()
            .set(PlayMode::BuildingPlace);
        app.add_systems(Update, cancel_if_owner_state_is_active);

        app.update();

        assert!(matches!(
            *app.world().resource::<NextState<PlayMode>>(),
            NextState::Pending(PlayMode::Normal)
        ));
    }
}
