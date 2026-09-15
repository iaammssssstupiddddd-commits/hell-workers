//! Focus follows the visible modal tree, including controls outside its scroll viewport.
use bevy::ecs::system::SystemParam;
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui_widgets::{Checkbox, ScrollIntoView, Slider};
use hw_core::WorldEpoch;
use hw_ui::UiIntent;
use hw_ui::components::MenuButton;
use hw_ui::interaction::button_activation::{ButtonActivationQueries, ButtonActivationState};
use hw_ui::theme::UiTheme;

use crate::input_actions::{InputAction, PendingWorldInputCapture, ResolvedInputFrame};

#[derive(Resource, Default)]
pub(super) struct ModalFocusState {
    epoch: WorldEpoch,
    stack: Vec<(Entity, Option<Entity>)>,
    last_focus: Option<Entity>,
    ring: Option<(Entity, Option<Outline>)>,
}

type Controls<'w, 's> = Query<
    'w,
    's,
    Entity,
    Or<(
        With<Button>,
        With<EditableText>,
        With<Slider>,
        With<Checkbox>,
    )>,
>;

#[derive(SystemParam)]
pub(super) struct ModalTree<'w, 's> {
    activation: ButtonActivationQueries<'w, 's>,
    controls: Controls<'w, 's>,
    children: Query<'w, 's, &'static Children>,
    buttons: Query<'w, 's, &'static MenuButton>,
    outlines: Query<'w, 's, Option<&'static Outline>>,
}

impl ModalTree<'_, '_> {
    fn controls(&self, root: Entity) -> Vec<Entity> {
        let mut pending = vec![root];
        let mut controls = Vec::new();
        while let Some(entity) = pending.pop() {
            if !self.activation.visible(entity, Some(root)) {
                continue;
            }
            if self.controls.contains(entity) {
                controls.push(entity);
            }
            if let Ok(children) = self.children.get(entity) {
                pending.extend(children.iter().rev());
            }
        }
        controls
    }

    fn initial(&self, controls: &[Entity]) -> Option<Entity> {
        controls
            .iter()
            .copied()
            .find(|entity| {
                self.buttons.get(*entity).is_ok_and(|button| {
                    matches!(
                        button.0,
                        UiIntent::CancelSaveCatalogConfirm
                            | UiIntent::CancelLoadConfirm
                            | UiIntent::CloseSaveCatalog
                            | UiIntent::CloseSettings
                            | UiIntent::CloseHelp
                            | UiIntent::CloseDialog
                    )
                })
            })
            .or_else(|| controls.first().copied())
    }
}

pub(super) fn navigate_modal(
    frame: Res<ResolvedInputFrame>,
    tree: ModalTree,
    epoch: Res<WorldEpoch>,
    mut focus: ResMut<InputFocus>,
    mut activation: ResMut<ButtonActivationState>,
    mut commands: Commands,
) {
    let Some(root) = tree.activation.foreground() else {
        return;
    };
    let controls = tree.controls(root);
    for action in frame.actions() {
        match action {
            InputAction::ModalFocusNext | InputAction::ModalFocusPrevious => {
                if controls.is_empty() {
                    continue;
                }
                let index = focus
                    .get()
                    .and_then(|entity| controls.iter().position(|item| *item == entity));
                let next = match (index, action) {
                    (Some(index), InputAction::ModalFocusPrevious) => {
                        (index + controls.len() - 1) % controls.len()
                    }
                    (Some(index), _) => (index + 1) % controls.len(),
                    (None, InputAction::ModalFocusPrevious) => controls.len() - 1,
                    (None, _) => 0,
                };
                focus.set(controls[next], FocusCause::Navigated);
                commands.trigger(ScrollIntoView {
                    entity: controls[next],
                });
            }
            InputAction::ModalActivate => {
                if let Some(entity) = focus.get() {
                    tree.activation
                        .queue_keyboard(entity, *epoch, &mut activation);
                }
            }
            _ => {}
        }
    }
}

/// Runs after presentation so hidden buttons never retain focus for another input frame.
pub(super) fn sync_modal_focus(
    tree: ModalTree,
    pending: Res<PendingWorldInputCapture>,
    epoch: Res<WorldEpoch>,
    theme: Res<UiTheme>,
    mut state: ResMut<ModalFocusState>,
    mut focus: ResMut<InputFocus>,
    mut commands: Commands,
) {
    if state.epoch != *epoch {
        state.stack.clear();
        state.last_focus = None;
        state.epoch = *epoch;
        focus.clear();
    }
    let root = tree.activation.foreground();
    if state.stack.last().map(|item| item.0) != root {
        if let Some(index) =
            root.and_then(|root| state.stack.iter().position(|item| item.0 == root))
        {
            let restore = state.stack.get(index + 1).and_then(|item| item.1);
            state.stack.truncate(index + 1);
            focus.clear();
            if let Some(entity) = restore.filter(|entity| tree.activation.visible(*entity, root)) {
                focus.set(entity, FocusCause::Navigated);
            }
        } else if let Some(root) = root {
            let opener = pending
                .foreground_opener(Some(root))
                .or_else(|| focus.get())
                .or(state.last_focus);
            state.stack.push((root, opener));
            focus.clear();
        } else {
            let restore = state.stack.first().and_then(|item| item.1);
            state.stack.clear();
            focus.clear();
            if let Some(entity) = restore.filter(|entity| tree.activation.visible(*entity, None)) {
                focus.set(entity, FocusCause::Navigated);
            }
        }
    }
    if let Some(root) = root {
        let controls = tree.controls(root);
        if !focus.get().is_some_and(|entity| controls.contains(&entity)) {
            focus.clear();
            if let Some(entity) = tree.initial(&controls) {
                focus.set(entity, FocusCause::Navigated);
                commands.trigger(ScrollIntoView { entity });
            }
        }
    }
    let next = root.and(focus.get());
    state.last_focus = focus.get();
    if state.ring.as_ref().map(|item| item.0) != next {
        if let Some((entity, original)) = state.ring.take()
            && let Ok(mut entity) = commands.get_entity(entity)
        {
            if let Some(original) = original {
                entity.insert(original);
            } else {
                entity.remove::<Outline>();
            }
        }
        if let Some(entity) = next {
            let original = tree.outlines.get(entity).ok().flatten().copied();
            state.ring = Some((entity, original));
            commands.entity(entity).insert(Outline::new(
                Val::Px(2.0),
                Val::Px(2.0),
                theme.colors.accent_sulfur,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ui::InteractionDisabled;
    use hw_ui::components::UiInputCapture;

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<WorldEpoch>()
            .init_resource::<UiTheme>()
            .init_resource::<InputFocus>()
            .init_resource::<ModalFocusState>()
            .init_resource::<PendingWorldInputCapture>()
            .init_resource::<ResolvedInputFrame>()
            .init_resource::<ButtonActivationState>()
            .add_systems(Update, navigate_modal)
            .add_systems(PostUpdate, sync_modal_focus);
        app
    }

    fn action(app: &mut App, action: InputAction) {
        app.world_mut()
            .resource_mut::<ResolvedInputFrame>()
            .replace(default(), vec![action], None, true);
        app.update();
        app.world_mut()
            .resource_mut::<ResolvedInputFrame>()
            .replace(default(), vec![], None, false);
    }

    #[test]
    fn focus_cycles_tree_order_skipping_hidden_and_disabled_and_scrolls_offscreen_controls() {
        let mut app = app();
        let root = app
            .world_mut()
            .spawn((Node::default(), UiInputCapture))
            .id();
        let first = app.world_mut().spawn((Button, ChildOf(root))).id();
        let hidden = app
            .world_mut()
            .spawn((
                Node {
                    display: Display::None,
                    ..default()
                },
                ChildOf(root),
            ))
            .id();
        app.world_mut().spawn((Button, ChildOf(hidden)));
        app.world_mut()
            .spawn((Button, InteractionDisabled, ChildOf(root)));
        let last = app.world_mut().spawn((Button, ChildOf(root))).id();
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(first));
        action(&mut app, InputAction::ModalFocusPrevious);
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(last));
        assert!(app.world().get::<Outline>(last).is_some());
        assert!(app.world().get::<Outline>(first).is_none());
        action(&mut app, InputAction::ModalFocusNext);
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(first));
    }

    #[test]
    fn confirmation_prefers_back_and_modal_close_restores_only_live_opener() {
        for delete_opener in [false, true] {
            let mut app = app();
            let opener = app.world_mut().spawn(Button).id();
            app.insert_resource(InputFocus::from_entity(opener));
            let root = app
                .world_mut()
                .spawn((Node::default(), UiInputCapture))
                .id();
            app.world_mut().spawn((Button, ChildOf(root)));
            let back = app
                .world_mut()
                .spawn((
                    Button,
                    MenuButton(UiIntent::CancelSaveCatalogConfirm),
                    ChildOf(root),
                ))
                .id();
            app.update();
            assert_eq!(app.world().resource::<InputFocus>().get(), Some(back));
            if delete_opener {
                app.world_mut().despawn(opener);
            }
            app.world_mut().get_mut::<Node>(root).unwrap().display = Display::None;
            app.update();
            assert_eq!(
                app.world().resource::<InputFocus>().get(),
                (!delete_opener).then_some(opener)
            );
        }
    }

    #[test]
    fn nested_overlay_restores_parent_focus_and_world_replace_forgets_return_stack() {
        let mut app = app();
        let parent = app
            .world_mut()
            .spawn((Node::default(), UiInputCapture, GlobalZIndex(1)))
            .id();
        let button = app.world_mut().spawn((Button, ChildOf(parent))).id();
        app.update();
        let child = app
            .world_mut()
            .spawn((Node::default(), UiInputCapture, GlobalZIndex(2)))
            .id();
        app.world_mut().spawn((Button, ChildOf(child)));
        // Capture clears focus before the newly opened root is presented.
        app.world_mut().resource_mut::<InputFocus>().clear();
        app.update();
        app.world_mut().get_mut::<Node>(child).unwrap().display = Display::None;
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(button));
        app.world_mut().resource_mut::<WorldEpoch>().advance();
        app.world_mut().get_mut::<Node>(parent).unwrap().display = Display::None;
        app.update();
        assert!(app.world().resource::<InputFocus>().get().is_none());
        assert!(app.world().resource::<ModalFocusState>().stack.is_empty());
    }
}
