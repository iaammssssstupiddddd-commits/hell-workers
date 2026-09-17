//! Root adapters for the map HUD and the single workspace.
pub(crate) mod attention;
pub(crate) mod overlays;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_ui::components::LeftPanelMode;
use hw_ui::panels::task_list::{
    TaskDashboardActionState, TaskDashboardControl, TaskDashboardViewState, TaskListDirty,
    TaskStatusFilter,
};
use hw_ui::shell::{PopulationText, UiShellState, WorkspaceAction};

#[derive(SystemParam)]
pub(crate) struct WorkspaceIntentContext<'w> {
    shell: Option<ResMut<'w, UiShellState>>,
    mode: Option<ResMut<'w, LeftPanelMode>>,
    view: Option<ResMut<'w, TaskDashboardViewState>>,
    actions: Option<ResMut<'w, TaskDashboardActionState>>,
    dirty: Option<ResMut<'w, TaskListDirty>>,
    minimized: Option<ResMut<'w, hw_ui::list::EntityListMinimizeState>>,
    view_layer: Option<ResMut<'w, hw_ui::world_view::WorldViewState>>,
    construction_cancel:
        Option<ResMut<'w, hw_ui::panels::construction_cancel::ConstructionCancelState>>,
}

impl WorkspaceIntentContext<'_> {
    pub(crate) fn set_world_view(
        &mut self,
        view: hw_ui::world_view::WorldView,
        selected: Option<Entity>,
        pin: Option<Entity>,
    ) {
        if let Some(layer) = &mut self.view_layer {
            layer.manual = view;
        }
        if self
            .shell
            .as_ref()
            .is_some_and(|shell| shell.page == hw_ui::shell::WorkspacePage::Display)
        {
            self.apply(WorkspaceAction::Back, selected, pin);
        }
    }
    pub(crate) fn apply(
        &mut self,
        action: WorkspaceAction,
        selected: Option<Entity>,
        pin: Option<Entity>,
    ) {
        let Some(shell) = &mut self.shell else {
            return;
        };
        shell.apply(action, selected, pin);
        if let Some(cancel) = &mut self.construction_cancel {
            cancel.pending = None;
        }
        if matches!(
            action,
            WorkspaceAction::OpenEntities | WorkspaceAction::OpenBlockedTasks
        ) {
            if let Some(mode) = &mut self.mode {
                **mode = if action == WorkspaceAction::OpenEntities {
                    LeftPanelMode::EntityList
                } else {
                    LeftPanelMode::TaskList
                };
            }
            if let Some(minimized) = &mut self.minimized {
                minimized.minimized = false;
            }
        }
        if action == WorkspaceAction::OpenBlockedTasks
            && let Some(view) = &mut self.view
        {
            view.apply_control(TaskDashboardControl::ResetFilters);
            view.apply_control(TaskDashboardControl::SetStatus(TaskStatusFilter::Blocked));
        }
        if let Some(actions) = &mut self.actions {
            actions.confirmation = None;
            if action == WorkspaceAction::OpenBlockedTasks {
                actions.active_task = None;
            }
        }
        if let Some(dirty) = &mut self.dirty {
            dirty.mark_list();
        }
    }
}

type PopulationAdded = Or<(
    Added<hw_core::familiar::Familiar>,
    Added<crate::entities::damned_soul::DamnedSoul>,
)>;

pub(crate) fn population_hud_system(
    familiars: Query<(), With<hw_core::familiar::Familiar>>,
    souls: Query<(), With<crate::entities::damned_soul::DamnedSoul>>,
    added: Query<(), PopulationAdded>,
    mut removed_familiars: RemovedComponents<hw_core::familiar::Familiar>,
    mut removed_souls: RemovedComponents<crate::entities::damned_soul::DamnedSoul>,
    mut initialized: Local<bool>,
    mut text: Query<&mut Text, With<PopulationText>>,
) {
    let removed = removed_familiars.read().count() + removed_souls.read().count();
    if *initialized && added.is_empty() && removed == 0 {
        return;
    }
    *initialized = true;
    let value = format!(
        "使い魔 {} · 魂 {}",
        familiars.iter().count(),
        souls.iter().count()
    );
    for mut text in &mut text {
        if text.0 != value {
            text.0.clone_from(&value);
        }
    }
}

type SelectionNames<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static hw_core::familiar::Familiar>,
        Option<&'static crate::entities::damned_soul::SoulIdentity>,
        Option<&'static Name>,
    ),
>;

pub(crate) fn current_selection_label_system(
    selected: Res<crate::interface::selection::SelectedEntity>,
    names: SelectionNames,
    mut labels: Query<&mut Text, With<hw_ui::shell::CurrentSelectionLabel>>,
) {
    let name = selected
        .0
        .and_then(|entity| names.get(entity).ok())
        .map(|(familiar, soul, name)| {
            familiar
                .map(|f| f.name.as_str())
                .or_else(|| soul.map(|s| s.name.as_str()))
                .or_else(|| name.map(Name::as_str))
                .unwrap_or("対象")
        })
        .unwrap_or("対象");
    let value = format!("現在選択: {name} → 詳細へ");
    for mut label in &mut labels {
        if label.0 != value {
            label.0.clone_from(&value);
        }
    }
}

pub(crate) fn tool_workspace_system(
    play: Res<State<hw_core::game_state::PlayMode>>,
    next: Res<NextState<hw_core::game_state::PlayMode>>,
    menu: Res<hw_ui::components::MenuState>,
    task: Res<crate::app_contexts::TaskContext>,
    selected: Res<crate::interface::selection::SelectedEntity>,
    pin: Res<hw_ui::panels::info_panel::InfoPanelPinState>,
    mut workspace: WorkspaceIntentContext,
) {
    use hw_core::game_state::{PlayMode, TaskMode};
    use hw_ui::components::MenuState;
    let mode = match &*next {
        NextState::Pending(mode) | NextState::PendingIfNeq(mode) => mode,
        NextState::Unchanged => play.get(),
    };
    if workspace
        .shell
        .as_ref()
        .is_some_and(|shell| shell.can_go_back())
        && (mode != &PlayMode::Normal
            || task.0 != TaskMode::None
            || matches!(
                *menu,
                MenuState::Architect | MenuState::Orders | MenuState::Zones | MenuState::Dream
            ))
    {
        workspace.apply(WorkspaceAction::Close, selected.0, pin.entity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn blocked_entry_resets_filters_and_pending_confirmation() {
        let mut world = World::new();
        world.init_resource::<UiShellState>();
        world.init_resource::<LeftPanelMode>();
        world.init_resource::<TaskDashboardViewState>();
        world.init_resource::<hw_ui::panels::construction_cancel::ConstructionCancelState>();
        let target = world.spawn_empty().id();
        world
            .resource_mut::<hw_ui::panels::construction_cancel::ConstructionCancelState>()
            .begin(target, 0, None, None, None, None);
        world
            .resource_mut::<TaskDashboardViewState>()
            .apply_control(TaskDashboardControl::SetStatus(TaskStatusFilter::Working));
        world
            .run_system_once(|mut context: WorkspaceIntentContext| {
                context.apply(WorkspaceAction::OpenBlockedTasks, None, None)
            })
            .unwrap();
        assert!(world.resource::<UiShellState>().management_open());
        assert_eq!(*world.resource::<LeftPanelMode>(), LeftPanelMode::TaskList);
        assert!(
            world
                .resource::<hw_ui::panels::construction_cancel::ConstructionCancelState>()
                .pending
                .is_none()
        );
        assert_eq!(
            world.resource::<TaskDashboardViewState>().status,
            TaskStatusFilter::Blocked
        );
    }
}
