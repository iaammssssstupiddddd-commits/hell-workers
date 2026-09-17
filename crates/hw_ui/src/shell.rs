//! The map-first workspace. Opening a page never moves the camera or changes a pin.

use bevy::prelude::*;

use crate::UiIntent;
use crate::components::{EntityListPanel, InfoPanel, MenuButton, UiInputBlocker};
use crate::models::inspection::EntityInspectionViewModel;
use crate::panels::info_panel::InfoPanelPinState;
use crate::selection::SelectedEntity;
use crate::setup::UiAssets;
use crate::theme::{UiTheme, font_size_rem};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WorkspacePage {
    #[default]
    World,
    Management,
    Inspector,
    Display,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceAction {
    ToggleManagement,
    ToggleDisplay,
    OpenEntities,
    OpenBlockedTasks,
    InspectSelection,
    Back,
    Close,
}

#[derive(Resource, Default, Debug)]
pub struct UiShellState {
    pub page: WorkspacePage,
    temporary_target: Option<Entity>,
    return_to_management: bool,
    observed_selection: Option<Entity>,
    observed_pin: Option<Entity>,
    display_return: Option<WorkspacePage>,
}

impl UiShellState {
    pub fn inspected_entity(
        &self,
        selected: Option<Entity>,
        pinned: Option<Entity>,
    ) -> Option<Entity> {
        self.temporary_target.or(pinned).or(selected)
    }

    pub fn management_open(&self) -> bool {
        self.page == WorkspacePage::Management
    }

    pub fn can_go_back(&self) -> bool {
        self.page != WorkspacePage::World
    }

    pub fn observe_selection(&mut self, selected: Option<Entity>, pinned: Option<Entity>) {
        if self.observed_selection == selected && self.observed_pin == pinned {
            return;
        }
        let newly_selected = selected.is_some() && self.observed_selection != selected;
        let newly_pinned = pinned.is_some() && self.observed_pin != pinned;
        self.observed_selection = selected;
        self.observed_pin = pinned;
        self.temporary_target = None;
        // Reference cleanup must not navigate away from the player's chosen page.
        if self.page != WorkspacePage::Inspector && !newly_selected && !newly_pinned {
            return;
        }
        if selected.is_some() || pinned.is_some() {
            self.return_to_management |= self.management_open();
            self.page = WorkspacePage::Inspector;
        } else if self.page == WorkspacePage::Inspector {
            self.back();
        }
    }

    pub fn selection_changed(&self, selected: Option<Entity>, pinned: Option<Entity>) -> bool {
        self.observed_selection != selected || self.observed_pin != pinned
    }

    pub fn has_temporary_target(&self) -> bool {
        self.temporary_target.is_some()
    }

    pub fn observe_task_selection(&mut self, selected: Option<Entity>, pinned: Option<Entity>) {
        self.observed_selection = selected;
        self.observed_pin = pinned;
    }

    pub fn reopen_inspector(&mut self, selected: Option<Entity>, pinned: Option<Entity>) {
        self.return_to_management |= self.management_open();
        self.page = WorkspacePage::Inspector;
        self.temporary_target = None;
        self.observed_selection = selected;
        self.observed_pin = pinned;
    }

    pub fn apply(
        &mut self,
        action: WorkspaceAction,
        selected: Option<Entity>,
        pinned: Option<Entity>,
    ) {
        self.observe_selection(selected, pinned);
        match action {
            WorkspaceAction::ToggleDisplay => {
                if self.page == WorkspacePage::Display {
                    self.back();
                } else {
                    self.display_return = Some(self.page);
                    self.page = WorkspacePage::Display;
                }
            }
            WorkspaceAction::ToggleManagement if self.management_open() => self.close(),
            WorkspaceAction::ToggleManagement
            | WorkspaceAction::OpenEntities
            | WorkspaceAction::OpenBlockedTasks => {
                self.page = WorkspacePage::Management;
                self.temporary_target = None;
                self.return_to_management = false;
            }
            WorkspaceAction::InspectSelection => {
                if let Some(selected) = selected {
                    self.return_to_management |= self.management_open();
                    self.temporary_target = pinned.filter(|pin| *pin != selected).map(|_| selected);
                    self.page = WorkspacePage::Inspector;
                }
            }
            WorkspaceAction::Back => self.back(),
            WorkspaceAction::Close => self.close(),
        }
    }

    fn back(&mut self) {
        if self.page == WorkspacePage::Display {
            self.page = self.display_return.take().unwrap_or_default();
            return;
        }
        if self.temporary_target.take().is_some() {
            return;
        }
        if self.return_to_management && self.page == WorkspacePage::Inspector {
            self.page = WorkspacePage::Management;
            self.return_to_management = false;
        } else {
            self.close();
        }
    }

    fn close(&mut self) {
        self.page = WorkspacePage::World;
        self.temporary_target = None;
        self.return_to_management = false;
        self.display_return = None;
    }

    pub fn discard_missing_temporary_target(&mut self) {
        self.temporary_target = None;
    }
}

#[derive(Component)]
pub struct PopulationText;

#[derive(Component)]
pub struct CurrentSelectionChip;

#[derive(Component)]
pub struct CurrentSelectionLabel;

type WorkspaceRoots<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Node,
        Has<EntityListPanel>,
        Has<InfoPanel>,
        Has<CurrentSelectionChip>,
    ),
    Or<(
        With<EntityListPanel>,
        With<InfoPanel>,
        With<CurrentSelectionChip>,
    )>,
>;

/// Only this system writes workspace root visibility. Content owners can continue
/// refreshing their snapshots while either page is closed.
pub fn workspace_visibility_system(
    shell: Res<UiShellState>,
    model: Res<EntityInspectionViewModel>,
    selected: Res<SelectedEntity>,
    pin: Res<InfoPanelPinState>,
    mut roots: WorkspaceRoots,
) {
    for (mut node, management, inspector, chip) in &mut roots {
        let visible = (management && shell.management_open())
            || (inspector && shell.page == WorkspacePage::Inspector && model.model.is_some())
            || (chip
                && pin.entity.is_some()
                && selected.0.is_some()
                && selected.0 != pin.entity
                && shell.temporary_target.is_none());
        let display = if visible {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
}

pub fn spawn_workspace_navigation(
    parent: &mut ChildSpawnerCommands,
    assets: &dyn UiAssets,
    theme: &UiTheme,
    inspector: bool,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            justify_content: JustifyContent::SpaceBetween,
            column_gap: Val::Px(8.0),
            margin: UiRect::bottom(Val::Px(6.0)),
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|row| {
            workspace_button(row, assets, theme, "戻る", WorkspaceAction::Back);
            workspace_button(row, assets, theme, "閉じる", WorkspaceAction::Close);
        });
    if inspector {
        let chip = workspace_button(
            parent,
            assets,
            theme,
            "現在の選択 → 詳細へ",
            WorkspaceAction::InspectSelection,
        );
        parent.commands().entity(chip).insert(CurrentSelectionChip);
    }
}

pub(crate) fn workspace_button(
    parent: &mut ChildSpawnerCommands,
    assets: &dyn UiAssets,
    theme: &UiTheme,
    label: &str,
    action: WorkspaceAction,
) -> Entity {
    parent
        .spawn((
            Button,
            Node {
                min_height: Val::Px(32.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(theme.colors.button_default),
            MenuButton(UiIntent::Workspace(action)),
            UiInputBlocker,
        ))
        .with_children(|button| {
            let mut label_node = button.spawn((
                Text::new(label),
                TextFont {
                    font: assets.font_ui().clone().into(),
                    font_size: font_size_rem(14.0),
                    ..default()
                },
                TextColor(theme.colors.text_primary_semantic),
            ));
            if action == WorkspaceAction::InspectSelection {
                label_node.insert(CurrentSelectionLabel);
            }
        })
        .id()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn management_detail_returns_without_replacing_the_pin() {
        let a = Entity::from_raw_u32(1).unwrap();
        let b = Entity::from_raw_u32(2).unwrap();
        let mut shell = UiShellState::default();
        shell.observe_selection(Some(a), Some(a));
        shell.apply(WorkspaceAction::OpenEntities, Some(a), Some(a));
        shell.observe_selection(Some(b), Some(a));
        assert_eq!(shell.inspected_entity(Some(b), Some(a)), Some(a));
        shell.apply(WorkspaceAction::InspectSelection, Some(b), Some(a));
        assert_eq!(shell.inspected_entity(Some(b), Some(a)), Some(b));
        shell.apply(WorkspaceAction::Back, Some(b), Some(a));
        assert_eq!(shell.inspected_entity(Some(b), Some(a)), Some(a));
        shell.apply(WorkspaceAction::Back, Some(b), Some(a));
        assert!(shell.management_open());
    }

    #[test]
    fn closing_keeps_selection_and_does_not_reopen_on_refresh() {
        let selected = Some(Entity::from_raw_u32(1).unwrap());
        let mut shell = UiShellState::default();
        shell.observe_selection(selected, None);
        shell.apply(WorkspaceAction::Close, selected, None);
        shell.observe_selection(selected, None);
        assert!(!shell.can_go_back());
        assert_eq!(shell.inspected_entity(selected, None), selected);
    }

    #[test]
    fn deleted_selection_or_pin_does_not_reopen_a_closed_workspace() {
        let a = Some(Entity::from_raw_u32(1).unwrap());
        let b = Some(Entity::from_raw_u32(2).unwrap());
        for (selected, pin) in [(a, None), (None, b)] {
            let mut shell = UiShellState::default();
            shell.observe_selection(a, b);
            shell.apply(WorkspaceAction::Close, a, b);
            shell.observe_selection(selected, pin);
            assert_eq!(shell.page, WorkspacePage::World);
        }
    }

    #[test]
    fn reference_cleanup_preserves_management_and_display_pages() {
        let a = Some(Entity::from_raw_u32(1).unwrap());
        let b = Some(Entity::from_raw_u32(2).unwrap());
        for action in [
            WorkspaceAction::OpenEntities,
            WorkspaceAction::ToggleDisplay,
        ] {
            for (selected, pin) in [(a, None), (None, b)] {
                let mut shell = UiShellState::default();
                shell.observe_selection(a, b);
                shell.apply(action, a, b);
                let page = shell.page;
                shell.observe_selection(selected, pin);
                assert_eq!(shell.page, page);
            }
        }
    }
}
