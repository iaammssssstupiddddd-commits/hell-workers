//! A button commits only after a matching release; Interaction remains visual state.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::ui::{InteractionDisabled, UiStack};
use bevy::window::PrimaryWindow;
use hw_core::WorldEpoch;

use crate::UiIntent;
use crate::components::*;
use crate::list::DragState;
use crate::models::EntityInspectionViewModel;
use crate::panels::task_list::{TaskActionButton, TaskDashboardControl, TaskDashboardViewState};

#[derive(Clone, Debug, PartialEq)]
enum ButtonIdentity {
    Menu(UiIntent),
    TaskAction(TaskActionButton),
    TaskControl(TaskDashboardControl, Option<TaskDashboardViewState>),
    Soul(Entity),
    Familiar(Entity),
    Task(Entity),
    MaxSoul(Entity, i8),
    Section(EntityListSectionType),
    Tab(LeftPanelMode),
    Rename(Option<Entity>),
    Other,
}

#[derive(Clone, Debug, PartialEq)]
struct ArmedButton {
    entity: Entity,
    identity: ButtonIdentity,
    foreground: Option<Entity>,
    epoch: WorldEpoch,
}

#[derive(Resource, Default)]
pub struct ButtonActivationState {
    pointer: Option<ArmedButton>,
    keyboard: Option<ArmedButton>,
}

type ButtonMetadata<'w, 's> = Query<
    'w,
    's,
    (
        &'static Interaction,
        (
            Option<&'static MenuButton>,
            Option<&'static TaskActionButton>,
            Option<&'static TaskDashboardControl>,
        ),
        (
            Option<&'static SoulListItem>,
            Option<&'static FamiliarListItem>,
            Option<&'static TaskListItem>,
        ),
        (
            Option<&'static FamiliarMaxSoulAdjustButton>,
            Option<&'static SectionToggle>,
            Option<&'static LeftPanelTabButton>,
            Has<SoulRenameButton>,
        ),
    ),
    With<Button>,
>;

type UiHierarchy<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static Node>,
        Option<&'static Visibility>,
        Option<&'static ChildOf>,
        Has<InteractionDisabled>,
    ),
>;

#[derive(SystemParam)]
pub struct ButtonActivationQueries<'w, 's> {
    time: Option<Res<'w, Time<Virtual>>>,
    buttons: ButtonMetadata<'w, 's>,
    hierarchy: UiHierarchy<'w, 's>,
    roots: Query<'w, 's, (Entity, Option<&'static GlobalZIndex>), With<UiInputCapture>>,
    dashboard: Option<Res<'w, TaskDashboardViewState>>,
    inspection: Option<Res<'w, EntityInspectionViewModel>>,
}

impl ButtonActivationQueries<'_, '_> {
    pub fn visible(&self, entity: Entity, foreground: Option<Entity>) -> bool {
        if self.time.as_ref().is_some_and(|time| time.is_paused())
            && let Ok((_, (menu, task, _), _, (max_soul, _, _, _))) = self.buttons.get(entity)
            && (menu.is_some_and(|menu| !menu.0.allowed_while_paused())
                || task.is_some()
                || max_soul.is_some())
        {
            return false;
        }
        let mut current = entity;
        let mut in_foreground = foreground.is_none();
        for _ in 0..128 {
            let Ok((node, visibility, parent, disabled)) = self.hierarchy.get(current) else {
                return false;
            };
            if disabled
                || node.is_some_and(|node| node.display == Display::None)
                || visibility == Some(&Visibility::Hidden)
            {
                return false;
            }
            in_foreground |= Some(current) == foreground;
            let Some(parent) = parent else {
                return in_foreground;
            };
            current = parent.parent();
        }
        false
    }

    pub fn foreground(&self) -> Option<Entity> {
        self.roots
            .iter()
            .filter(|(entity, _)| self.visible(*entity, None))
            .max_by_key(|(_, layer)| layer.map_or(0, |layer| layer.0))
            .map(|(entity, _)| entity)
    }

    fn snapshot(
        &self,
        entity: Entity,
        foreground: Option<Entity>,
        epoch: WorldEpoch,
    ) -> Option<ArmedButton> {
        if !self.visible(entity, foreground) {
            return None;
        }
        let (
            _,
            (menu, task_action, control),
            (soul, familiar, task),
            (max_soul, section, tab, rename),
        ) = self.buttons.get(entity).ok()?;
        let identity = if let Some(menu) = menu {
            ButtonIdentity::Menu(menu.0)
        } else if let Some(action) = task_action {
            ButtonIdentity::TaskAction(*action)
        } else if let Some(control) = control {
            ButtonIdentity::TaskControl(*control, self.dashboard.as_deref().cloned())
        } else if let Some(soul) = soul {
            ButtonIdentity::Soul(soul.0)
        } else if let Some(familiar) = familiar {
            ButtonIdentity::Familiar(familiar.0)
        } else if let Some(task) = task {
            ButtonIdentity::Task(task.0)
        } else if let Some(button) = max_soul {
            ButtonIdentity::MaxSoul(button.familiar, button.delta)
        } else if let Some(section) = section {
            ButtonIdentity::Section(section.0)
        } else if let Some(tab) = tab {
            ButtonIdentity::Tab(tab.0)
        } else if rename {
            ButtonIdentity::Rename(
                self.inspection
                    .as_ref()
                    .and_then(|view| view.model.as_ref())
                    .map(|model| model.entity),
            )
        } else {
            ButtonIdentity::Other
        };
        Some(ArmedButton {
            entity,
            identity,
            foreground,
            epoch,
        })
    }

    pub fn queue_keyboard(
        &self,
        entity: Entity,
        epoch: WorldEpoch,
        state: &mut ButtonActivationState,
    ) {
        let foreground = self.foreground();
        state.keyboard = foreground.and_then(|_| self.snapshot(entity, foreground, epoch));
    }
}

#[derive(SystemParam)]
pub struct ButtonActivationInput<'w, 's> {
    mouse: Res<'w, ButtonInput<MouseButton>>,
    focus: Option<Res<'w, bevy::input_focus::InputFocus>>,
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    stack: Res<'w, UiStack>,
    epoch: Res<'w, WorldEpoch>,
    drag: Option<Res<'w, DragState>>,
}

pub fn collect_button_activations(
    input: ButtonActivationInput,
    queries: ButtonActivationQueries,
    mut state: ResMut<ButtonActivationState>,
    mut ui: ResMut<UiInputState>,
) {
    ui.activated_buttons.clear();
    let focused = input.windows.single().is_ok_and(|window| window.focused);
    let cursor = input
        .windows
        .single()
        .ok()
        .and_then(Window::cursor_position)
        .is_some();
    let foreground = queries.foreground();
    if !focused {
        state.pointer = None;
        state.keyboard = None;
        return;
    }
    if let Some(armed) = state.keyboard.take()
        && !input.mouse.pressed(MouseButton::Left)
        && !input.mouse.just_released(MouseButton::Left)
        && input.focus.as_ref().and_then(|focus| focus.get()) == Some(armed.entity)
        && queries
            .snapshot(armed.entity, foreground, *input.epoch)
            .as_ref()
            == Some(&armed)
    {
        ui.activated_buttons.insert(armed.entity);
    }

    let dragging = input.drag.as_ref().is_some_and(|drag| drag.is_dragging());
    if !cursor || dragging || ui.world_pointer_claimed {
        state.pointer = None;
        return;
    }
    if input.mouse.just_pressed(MouseButton::Left) {
        state.pointer = input.stack.uinodes.iter().rev().find_map(|entity| {
            let (interaction, ..) = queries.buttons.get(*entity).ok()?;
            (*interaction == Interaction::Pressed)
                .then(|| queries.snapshot(*entity, foreground, *input.epoch))
                .flatten()
        });
    }
    if let Some(armed) = state.pointer.as_ref() {
        let same = queries
            .snapshot(armed.entity, foreground, *input.epoch)
            .as_ref()
            == Some(armed);
        let over = queries
            .buttons
            .get(armed.entity)
            .is_ok_and(|(interaction, ..)| *interaction != Interaction::None);
        if !same || !over {
            state.pointer = None;
        }
    }
    if input.mouse.just_released(MouseButton::Left) {
        if let Some(armed) = state.pointer.take() {
            ui.activated_buttons.insert(armed.entity);
        }
    } else if !input.mouse.pressed(MouseButton::Left) {
        state.pointer = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input_focus::InputFocus;

    fn app() -> (App, Entity, Entity) {
        let mut app = App::new();
        app.init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<UiStack>()
            .init_resource::<WorldEpoch>()
            .init_resource::<DragState>()
            .init_resource::<InputFocus>()
            .init_resource::<ButtonActivationState>()
            .init_resource::<UiInputState>()
            .add_systems(Update, collect_button_activations);
        let mut window = Window {
            focused: true,
            ..default()
        };
        window.set_cursor_position(Some(Vec2::new(20.0, 20.0)));
        let window = app.world_mut().spawn((window, PrimaryWindow)).id();
        let button = app
            .world_mut()
            .spawn((Button, Interaction::Hovered, MenuButton(UiIntent::SaveGame)))
            .id();
        app.world_mut()
            .resource_mut::<UiStack>()
            .uinodes
            .push(button);
        (app, button, window)
    }

    fn press(app: &mut App, button: Entity) {
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        *app.world_mut().get_mut::<Interaction>(button).unwrap() = Interaction::Pressed;
        app.update();
        assert!(
            !app.world()
                .resource::<UiInputState>()
                .button_activated(button)
        );
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
    }

    fn release(app: &mut App, button: Entity) -> bool {
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        if let Some(mut interaction) = app.world_mut().get_mut::<Interaction>(button) {
            *interaction = Interaction::Hovered;
        }
        app.update();
        app.world()
            .resource::<UiInputState>()
            .button_activated(button)
    }

    #[test]
    fn matching_release_commits_once_and_holding_does_not_repeat() {
        let (mut app, button, _) = app();
        press(&mut app, button);
        app.update();
        assert!(
            !app.world()
                .resource::<UiInputState>()
                .button_activated(button)
        );
        assert!(release(&mut app, button));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        app.update();
        assert!(
            !app.world()
                .resource::<UiInputState>()
                .button_activated(button)
        );
    }

    #[test]
    fn leaving_then_returning_before_release_does_not_rearm() {
        let (mut app, button, _) = app();
        press(&mut app, button);
        *app.world_mut().get_mut::<Interaction>(button).unwrap() = Interaction::None;
        app.update();
        assert!(!release(&mut app, button));
    }

    #[test]
    fn changed_payload_epoch_foreground_disabled_or_deleted_target_cancels() {
        for cause in 0..5 {
            let (mut app, button, _) = app();
            press(&mut app, button);
            match cause {
                0 => {
                    app.world_mut().get_mut::<MenuButton>(button).unwrap().0 =
                        UiIntent::RequestLoadGame;
                }
                1 => app.world_mut().resource_mut::<WorldEpoch>().advance(),
                2 => {
                    app.world_mut()
                        .spawn((Node::default(), UiInputCapture, GlobalZIndex(100)));
                }
                3 => {
                    app.world_mut()
                        .entity_mut(button)
                        .insert(InteractionDisabled);
                }
                _ => {
                    app.world_mut().despawn(button);
                }
            }
            assert!(!release(&mut app, button), "cause {cause}");
        }
    }

    #[test]
    fn focus_loss_cursor_loss_drag_and_world_claim_cancel_release() {
        for cause in 0..4 {
            let (mut app, button, window) = app();
            press(&mut app, button);
            match cause {
                0 => app.world_mut().get_mut::<Window>(window).unwrap().focused = false,
                1 => app
                    .world_mut()
                    .get_mut::<Window>(window)
                    .unwrap()
                    .set_cursor_position(None),
                2 => app.world_mut().resource_mut::<DragState>().active_soul = Some(button),
                _ => {
                    app.world_mut()
                        .resource_mut::<UiInputState>()
                        .world_pointer_claimed = true
                }
            }
            assert!(!release(&mut app, button), "cause {cause}");
        }
    }

    #[test]
    fn only_frontmost_button_can_arm_and_hidden_ancestor_blocks_it() {
        let (mut app, behind, _) = app();
        let front = app.world_mut().spawn((Button, Interaction::Pressed)).id();
        app.world_mut()
            .resource_mut::<UiStack>()
            .uinodes
            .push(front);
        press(&mut app, behind);
        assert!(!release(&mut app, behind));
        assert!(
            app.world()
                .resource::<UiInputState>()
                .button_activated(front)
        );
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        let parent = app
            .world_mut()
            .spawn(Node {
                display: Display::None,
                ..default()
            })
            .id();
        app.world_mut().entity_mut(front).insert(ChildOf(parent));
        press(&mut app, front);
        assert!(!release(&mut app, front));
    }

    #[test]
    fn queued_keyboard_activation_requires_unchanged_focus_and_payload() {
        for cause in 0..4 {
            let (mut app, button, _) = app();
            let root = app
                .world_mut()
                .spawn((Node::default(), UiInputCapture))
                .id();
            app.world_mut().entity_mut(button).insert(ChildOf(root));
            app.insert_resource(InputFocus::from_entity(button));
            app.world_mut()
                .resource_mut::<ButtonActivationState>()
                .keyboard = Some(ArmedButton {
                entity: button,
                identity: ButtonIdentity::Menu(UiIntent::SaveGame),
                foreground: Some(root),
                epoch: WorldEpoch::default(),
            });
            match cause {
                1 => app.world_mut().resource_mut::<InputFocus>().clear(),
                2 => {
                    app.world_mut().get_mut::<MenuButton>(button).unwrap().0 =
                        UiIntent::RequestLoadGame
                }
                3 => app
                    .world_mut()
                    .resource_mut::<ButtonInput<MouseButton>>()
                    .press(MouseButton::Left),
                _ => {}
            }
            app.update();
            assert_eq!(
                app.world()
                    .resource::<UiInputState>()
                    .button_activated(button),
                cause == 0
            );
            app.update();
            assert!(
                !app.world()
                    .resource::<UiInputState>()
                    .button_activated(button)
            );
        }
    }
}
