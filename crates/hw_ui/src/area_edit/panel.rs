use crate::UiIntent;
use crate::components::{MenuButton, UiInputBlocker};
use crate::theme::UiTheme;
use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AreaEditAction {
    Undo,
    Redo,
    Copy,
    Paste,
    Save1,
    Save2,
    Save3,
    Load1,
    Load2,
    Load3,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AreaEditControl {
    pub action: AreaEditAction,
    pub label: String,
    pub enabled: bool,
}

#[derive(Resource, Default)]
pub struct AreaEditPanelModel {
    pub visible: bool,
    pub revision: u64,
    pub epoch: u64,
    pub controls: Vec<AreaEditControl>,
}

#[derive(Component)]
pub struct AreaEditPanel {
    font: Handle<Font>,
    content: Entity,
}

pub fn spawn_area_edit_panel(
    commands: &mut Commands,
    parent: Entity,
    font: Handle<Font>,
    theme: &UiTheme,
) {
    use bevy::ui_widgets::{ControlOrientation, ScrollArea, Scrollbar, ScrollbarThumb};
    let root = commands
        .spawn((
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                right: Val::Px(12.0),
                bottom: Val::Px(100.0),
                width: Val::Px(500.0),
                max_width: Val::Percent(94.0),
                max_height: Val::Vh(35.0),
                flex_direction: FlexDirection::Row,
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            UiInputBlocker,
            Interaction::None,
            GlobalZIndex(35),
            ChildOf(parent),
            Name::new("Area Edit Controls"),
        ))
        .id();
    let content = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
                row_gap: Val::Px(4.0),
                ..default()
            },
            ScrollArea,
            ChildOf(root),
        ))
        .id();
    commands
        .spawn((
            Node {
                width: Val::Px(7.0),
                ..default()
            },
            Scrollbar::new(content, ControlOrientation::Vertical, 20.0),
            ChildOf(root),
        ))
        .with_children(|bar| {
            bar.spawn((
                ScrollbarThumb {
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    border: UiRect::ZERO,
                },
                BackgroundColor(theme.colors.text_muted),
            ));
        });
    commands
        .entity(root)
        .insert(AreaEditPanel { font, content });
}

pub fn update_area_edit_panel(
    mut commands: Commands,
    model: Res<AreaEditPanelModel>,
    theme: Res<UiTheme>,
    mut panels: Query<(Entity, &AreaEditPanel, &mut Node)>,
    children: Query<&Children>,
) {
    if !model.is_changed() && !theme.is_changed() {
        return;
    }
    for (entity, panel, mut node) in &mut panels {
        node.display = if model.visible {
            Display::Flex
        } else {
            Display::None
        };
        if let Ok(children) = children.get(panel.content) {
            for child in children.iter() {
                commands.entity(child).try_despawn();
            }
        }
        commands
            .entity(entity)
            .insert(BackgroundColor(theme.colors.tooltip_bg));
        if !model.visible {
            continue;
        }
        commands.entity(panel.content).with_children(|parent| {
            for control in &model.controls {
                let mut row = parent.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        min_height: Val::Px(26.0),
                        padding: UiRect::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(theme.colors.button_default),
                ));
                if control.enabled {
                    row.insert((
                        Button,
                        MenuButton(UiIntent::AreaEditControl {
                            action: control.action,
                            revision: model.revision,
                            epoch: model.epoch,
                        }),
                    ));
                }
                row.with_children(|parent| {
                    parent.spawn((
                        Text::new(&control.label),
                        TextFont {
                            font: panel.font.clone().into(),
                            font_size: FontSize::Px(theme.typography.font_size_sm),
                            ..default()
                        },
                        TextColor(theme.colors.text_primary_semantic),
                    ));
                });
            }
        });
    }
}
