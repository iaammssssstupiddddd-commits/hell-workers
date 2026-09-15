//! Shared, target-bound confirmation for construction cancellation.

use bevy::prelude::*;

use crate::UiIntent;
use crate::components::{MenuButton, UiInputBlocker};
use crate::setup::UiAssets;
use crate::theme::UiTheme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConstructionCancelConfirmation {
    pub target: Entity,
    pub epoch: u64,
    pub ticket: u64,
    pub selected: Option<Entity>,
    pub pinned: Option<Entity>,
    pub active_task: Option<Entity>,
    pub source_task: Option<Entity>,
}

#[derive(Resource, Default)]
pub struct ConstructionCancelState {
    pub pending: Option<ConstructionCancelConfirmation>,
    next_ticket: u64,
    pub target_label: String,
}

impl ConstructionCancelState {
    pub fn begin(
        &mut self,
        target: Entity,
        epoch: u64,
        selected: Option<Entity>,
        pinned: Option<Entity>,
        active_task: Option<Entity>,
        source_task: Option<Entity>,
    ) {
        self.target_label = "Soul Spa".to_string();
        self.next_ticket = self.next_ticket.wrapping_add(1).max(1);
        self.pending = Some(ConstructionCancelConfirmation {
            target,
            epoch,
            ticket: self.next_ticket,
            selected,
            pinned,
            active_task,
            source_task,
        });
    }
}

#[derive(Component)]
pub struct ConstructionCancelPanel;

#[derive(Component)]
pub struct ConfirmationText;

#[derive(Component, Clone, Copy)]
pub enum ConfirmationButton {
    Focus,
    Back,
    Confirm,
}

pub fn spawn_construction_cancel_panel(
    commands: &mut Commands,
    assets: &dyn UiAssets,
    theme: &UiTheme,
    parent: Entity,
) {
    let panel = commands
        .spawn((
            ConstructionCancelPanel,
            UiInputBlocker,
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            GlobalZIndex(100),
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                right: Val::Px(12.0),
                top: Val::Percent(20.0),
                width: Val::Px(360.0),
                max_width: Val::Percent(92.0),
                padding: UiRect::all(Val::Px(12.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(theme.colors.dialog_bg),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::default(),
                ConfirmationText,
                TextFont {
                    font: assets.font_ui().clone().into(),
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(theme.colors.text_primary_semantic),
            ));
            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(8.0),
                    row_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|row| {
                    for (kind, label) in [
                        (ConfirmationButton::Focus, "現地へ"),
                        (ConfirmationButton::Back, "戻る"),
                        (ConfirmationButton::Confirm, "建設を取り消す"),
                    ] {
                        row.spawn((
                            Button,
                            kind,
                            MenuButton(UiIntent::DismissConstructionCancel),
                            Node {
                                padding: UiRect::all(Val::Px(8.0)),
                                ..default()
                            },
                            BackgroundColor(theme.colors.button_default),
                        ))
                        .with_children(|button| {
                            button.spawn((
                                Text::new(label),
                                TextFont {
                                    font: assets.font_ui().clone().into(),
                                    font_size: FontSize::Px(14.0),
                                    ..default()
                                },
                                TextColor(theme.colors.text_primary_semantic),
                            ));
                        });
                    }
                });
        })
        .id();
    commands.entity(parent).add_child(panel);
}

pub fn update_construction_cancel_panel(
    state: Res<ConstructionCancelState>,
    mut roots: Query<&mut Node, With<ConstructionCancelPanel>>,
    mut text: Query<&mut Text, With<ConfirmationText>>,
    mut buttons: Query<(&ConfirmationButton, &mut MenuButton)>,
) {
    if !state.is_changed() {
        return;
    }
    for mut root in &mut roots {
        root.display = if state.pending.is_some() {
            Display::Flex
        } else {
            Display::None
        };
    }
    let Some(pending) = state.pending else {
        return;
    };
    for mut text in &mut text {
        text.0 = format!(
            "{} の建設を取り消しますか？\n搬入済みの Bone を返し、建設予定を削除します。選択・固定対象の変更や別画面を開くと確認を解除します。",
            state.target_label
        );
    }
    for (kind, mut button) in &mut buttons {
        button.0 = match kind {
            ConfirmationButton::Focus => UiIntent::FocusEntity(pending.target),
            ConfirmationButton::Back => UiIntent::DismissConstructionCancel,
            ConfirmationButton::Confirm => UiIntent::ConfirmSoulSpaConstructionCancel {
                target: pending.target,
                epoch: pending.epoch,
                ticket: pending.ticket,
            },
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_replace_hides_and_invalidates_construction_confirmation() {
        let mut world = World::new();
        let target = world.spawn_empty().id();
        let mut state = ConstructionCancelState::default();
        state.begin(target, 4, Some(target), None, None, None);
        world.insert_resource(state);
        let panel = world.spawn((ConstructionCancelPanel, Node::default())).id();
        let button = world
            .spawn(MenuButton(UiIntent::ConfirmSoulSpaConstructionCancel {
                target,
                epoch: 4,
                ticket: 1,
            }))
            .id();
        crate::reset_for_world_replace(&mut world);
        assert!(
            world
                .resource::<ConstructionCancelState>()
                .pending
                .is_none()
        );
        assert_eq!(world.get::<Node>(panel).unwrap().display, Display::None);
        assert!(matches!(
            world.get::<MenuButton>(button).unwrap().0,
            UiIntent::DismissConstructionCancel
        ));
    }
}
