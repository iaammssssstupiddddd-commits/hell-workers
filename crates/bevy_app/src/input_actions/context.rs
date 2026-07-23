use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::game_state::{PlayMode, TaskMode};
use hw_ui::components::{
    LoadConfirmDialog, MenuState, OperationDialog, SettingsPanel, UiInputState,
};

use crate::app_contexts::TaskContext;
use crate::entities::familiar::Familiar;
use crate::interface::selection::SelectedEntity;

use super::capture::PendingWorldInputCapture;

/// The visually highest overlay that owns keyboard input for the frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputOverlay {
    LoadConfirm,
    Settings,
    Pause,
    OperationDialog,
}

/// Frame-local state used by the pure input resolver.
#[derive(Debug, Clone, PartialEq)]
pub struct InputContextSnapshot {
    pub text_input_blocks_keybinds: bool,
    pub has_in_progress_gesture: bool,
    pub top_overlay: Option<InputOverlay>,
    pub simulation_paused: bool,
    pub logic_shortcuts_enabled: bool,
    pub play_mode: PlayMode,
    pub task_mode: TaskMode,
    pub menu_state: MenuState,
    pub has_selected_familiar: bool,
    pub pending_play_mode: Option<PlayMode>,
    pub debug_visible: bool,
}

impl Default for InputContextSnapshot {
    fn default() -> Self {
        Self {
            text_input_blocks_keybinds: false,
            has_in_progress_gesture: false,
            top_overlay: None,
            simulation_paused: false,
            logic_shortcuts_enabled: true,
            play_mode: PlayMode::default(),
            task_mode: TaskMode::default(),
            menu_state: MenuState::default(),
            has_selected_familiar: false,
            pending_play_mode: None,
            debug_visible: false,
        }
    }
}

impl InputContextSnapshot {
    pub(crate) fn familiar_shortcuts_enabled(&self) -> bool {
        self.logic_shortcuts_enabled
            && self.play_mode == PlayMode::Normal
            && self.pending_play_mode.is_none()
            && self.has_selected_familiar
            && familiar_compatible_task_mode(self.task_mode)
    }

    pub(crate) fn active_mode(&self) -> bool {
        self.play_mode != PlayMode::Normal
            || self.task_mode != TaskMode::None
            || self
                .pending_play_mode
                .as_ref()
                .is_some_and(|mode| mode != &PlayMode::Normal)
    }

    pub(crate) fn open_menu(&self) -> bool {
        self.play_mode == PlayMode::Normal
            && self.pending_play_mode.is_none()
            && !matches!(self.menu_state, MenuState::Hidden | MenuState::Settings)
    }
}

fn familiar_compatible_task_mode(task_mode: TaskMode) -> bool {
    matches!(
        task_mode,
        TaskMode::None
            | TaskMode::DesignateChop(None)
            | TaskMode::DesignateMine(None)
            | TaskMode::DesignateHaul(None)
            | TaskMode::CancelDesignation(None)
            | TaskMode::SelectBuildTarget
    )
}

#[derive(SystemParam)]
pub(crate) struct InputContextParams<'w, 's> {
    ui_input_state: Res<'w, UiInputState>,
    time: Res<'w, Time<Virtual>>,
    play_mode: Res<'w, State<PlayMode>>,
    next_play_mode: Res<'w, NextState<PlayMode>>,
    task_context: Res<'w, TaskContext>,
    menu_state: Res<'w, MenuState>,
    selected: Res<'w, SelectedEntity>,
    debug_visible: Res<'w, crate::DebugVisible>,
    pending_capture: Option<Res<'w, PendingWorldInputCapture>>,
    q_familiars: Query<'w, 's, (), With<Familiar>>,
    q_load_confirm: Query<'w, 's, &'static Node, With<LoadConfirmDialog>>,
    q_settings: Query<'w, 's, &'static Node, With<SettingsPanel>>,
    q_operation_dialog: Query<'w, 's, &'static Node, With<OperationDialog>>,
}

impl InputContextParams<'_, '_> {
    pub(crate) fn snapshot(
        &self,
        has_active_area_edit_drag: bool,
    ) -> (InputContextSnapshot, Option<Entity>) {
        let play_mode = self.play_mode.get().clone();
        let pending_play_mode = match &*self.next_play_mode {
            NextState::Pending(mode) | NextState::PendingIfNeq(mode) if mode != &play_mode => {
                Some(mode.clone())
            }
            NextState::Pending(_) | NextState::PendingIfNeq(_) | NextState::Unchanged => None,
        };
        let selected_familiar = self
            .selected
            .0
            .filter(|entity| self.q_familiars.get(*entity).is_ok());
        let simulation_paused = self.time.is_paused();
        let has_in_progress_gesture =
            has_active_area_edit_drag || task_mode_has_in_progress_gesture(self.task_context.0);
        let visible_overlay = if query_is_visible(&self.q_load_confirm) {
            Some(InputOverlay::LoadConfirm)
        } else if query_is_visible(&self.q_settings) {
            Some(InputOverlay::Settings)
        } else if simulation_paused {
            Some(InputOverlay::Pause)
        } else if query_is_visible(&self.q_operation_dialog) {
            Some(InputOverlay::OperationDialog)
        } else {
            None
        };
        let pending_overlay = self
            .pending_capture
            .as_ref()
            .and_then(|pending| pending.overlay());
        let top_overlay = match (pending_overlay, visible_overlay) {
            (Some(pending), Some(visible)) if pending.priority() < visible.priority() => {
                Some(visible)
            }
            (Some(pending), _) => Some(pending),
            (None, visible) => visible,
        };

        (
            InputContextSnapshot {
                text_input_blocks_keybinds: hw_ui::interaction::text_input_blocks_keybinds(
                    &self.ui_input_state,
                ),
                has_in_progress_gesture,
                top_overlay,
                simulation_paused,
                logic_shortcuts_enabled: !simulation_paused,
                play_mode,
                task_mode: self.task_context.0,
                menu_state: *self.menu_state,
                has_selected_familiar: selected_familiar.is_some(),
                pending_play_mode,
                debug_visible: self.debug_visible.0,
            },
            selected_familiar,
        )
    }
}

fn task_mode_has_in_progress_gesture(task_mode: TaskMode) -> bool {
    matches!(
        task_mode,
        TaskMode::DesignateChop(Some(_))
            | TaskMode::DesignateMine(Some(_))
            | TaskMode::DesignateHaul(Some(_))
            | TaskMode::CancelDesignation(Some(_))
            | TaskMode::AreaSelection(Some(_))
            | TaskMode::AssignTask(Some(_))
            | TaskMode::ZonePlacement(_, Some(_))
            | TaskMode::ZoneRemoval(_, Some(_))
            | TaskMode::FloorPlace(Some(_))
            | TaskMode::WallPlace(Some(_))
            | TaskMode::DreamPlanting(Some(_))
            | TaskMode::StockpilePolicyEdit(Some(_))
            | TaskMode::SoulSpaPlace(Some(_))
    )
}

fn query_is_visible<T: Component>(query: &Query<&Node, With<T>>) -> bool {
    query
        .single()
        .is_ok_and(|node| node.display != Display::None)
}
