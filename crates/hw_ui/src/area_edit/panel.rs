use crate::UiIntent;

use crate::components::{MenuButton, UiInputBlocker, UiInputState, UiTooltip};
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

impl AreaEditAction {
    fn label(self) -> &'static str {
        match self {
            Self::Undo => "元に戻す",
            Self::Redo => "やり直す",
            Self::Copy => "範囲をコピー",
            Self::Paste => "貼り付け",
            Self::Save1 => "枠1にサイズ保存",
            Self::Save2 => "枠2にサイズ保存",
            Self::Save3 => "枠3にサイズ保存",
            Self::Load1 => "枠1を適用",
            Self::Load2 => "枠2を適用",
            Self::Load3 => "枠3を適用",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AreaEditControl {
    pub action: AreaEditAction,
    /// Full target, dimensions and unavailable reason, displayed on hover.
    pub label: String,
    pub enabled: bool,
}

#[derive(Resource, Default)]
pub struct AreaEditPanelModel {
    pub visible: bool,
    pub target: Option<Entity>,
    pub summary: String,
    pub revision: u64,
    pub epoch: u64,
    pub controls: Vec<AreaEditControl>,
}

#[derive(Component)]
pub struct AreaEditPanel {
    font: Handle<Font>,
    content: Entity,
    scope: Option<(Option<Entity>, u64)>,
    pub details_expanded: bool,
}

#[derive(Component)]
pub struct AreaEditDetailsToggle;

#[derive(Component)]
pub struct AreaEditControlButton(pub AreaEditAction);

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
                top: Val::Px(64.0),
                width: Val::Px(300.0),
                max_width: Val::Percent(94.0),
                max_height: Val::Vh(60.0),
                flex_direction: FlexDirection::Row,
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            UiInputBlocker,
            bevy::ui::RelativeCursorPosition::default(),
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
                row_gap: Val::Px(6.0),
                ..default()
            },
            ScrollArea,
            ChildOf(root),
        ))
        .id();
    commands
        .spawn((
            Node {
                width: Val::Px(5.0),
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
    commands.entity(root).insert(AreaEditPanel {
        font,
        content,
        scope: None,
        details_expanded: false,
    });
}

fn spawn_text(
    parent: &mut ChildSpawnerCommands,
    text: &str,
    font: &Handle<Font>,
    size: f32,
    color: Color,
) {
    parent.spawn((
        Text::new(text),
        TextFont {
            font: font.clone().into(),
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    ));
}

fn button_node() -> Node {
    Node {
        min_height: Val::Px(28.0),
        min_width: Val::Px(0.0),
        flex_grow: 1.0,
        flex_basis: Val::Px(0.0),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        padding: UiRect::all(Val::Px(4.0)),
        ..default()
    }
}

pub fn update_area_edit_panel(
    mut commands: Commands,
    model: Res<AreaEditPanelModel>,
    theme: Res<UiTheme>,
    input: Res<UiInputState>,
    toggles: Query<Entity, With<AreaEditDetailsToggle>>,
    mut panels: Query<(Entity, &mut AreaEditPanel, &mut Node)>,
    children: Query<&Children>,
) {
    let toggle =
        !input.world_input_captured && toggles.iter().any(|entity| input.button_activated(entity));
    for (entity, mut panel, mut node) in &mut panels {
        let scope = model.visible.then_some((model.target, model.epoch));
        let scope_changed = panel.scope != scope;
        if scope_changed {
            panel.scope = scope;
            panel.details_expanded = false;
        } else if toggle && model.visible {
            panel.details_expanded = !panel.details_expanded;
        }
        if !model.is_changed() && !theme.is_changed() && !scope_changed && !panel.is_changed() {
            continue;
        }
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
            .insert(BackgroundColor(theme.colors.bg_surface));
        if !model.visible {
            continue;
        }
        commands.entity(panel.content).with_children(|parent| {
            spawn_text(
                parent,
                "作業範囲を編集",
                &panel.font,
                theme.typography.font_size_sm,
                theme.colors.text_accent_semantic,
            );
            spawn_text(
                parent,
                &model.summary,
                &panel.font,
                theme.typography.font_size_sm,
                theme.colors.text_primary_semantic,
            );
            spawn_text(
                parent,
                "地面をドラッグして指定・離すと適用\n範囲内で移動 / 辺・角でサイズ変更",
                &panel.font,
                theme.typography.font_size_xs,
                theme.colors.text_secondary_semantic,
            );
            parent
                .spawn(Node {
                    column_gap: Val::Px(4.0),
                    flex_shrink: 0.0,
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        Button,
                        button_node(),
                        AreaEditDetailsToggle,
                        BackgroundColor(theme.colors.button_default),
                    ))
                    .with_children(|button| {
                        spawn_text(
                            button,
                            if panel.details_expanded {
                                "詳細操作を閉じる"
                            } else {
                                "詳細操作を開く"
                            },
                            &panel.font,
                            theme.typography.font_size_sm,
                            theme.colors.text_primary_semantic,
                        );
                    });
                    row.spawn((
                        Button,
                        button_node(),
                        MenuButton(UiIntent::FinishAreaEdit),
                        BackgroundColor(theme.colors.button_default),
                        UiTooltip::new("適用済みの範囲を残して編集を終了します。"),
                    ))
                    .with_children(|button| {
                        spawn_text(
                            button,
                            "編集を終了",
                            &panel.font,
                            theme.typography.font_size_sm,
                            theme.colors.text_accent_semantic,
                        );
                    });
                });
            if !panel.details_expanded {
                return;
            }
            spawn_text(
                parent,
                "履歴・コピー / 保存枠はサイズのみ",
                &panel.font,
                theme.typography.font_size_xs,
                theme.colors.text_secondary_semantic,
            );
            for pair in model.controls.chunks(2) {
                parent
                    .spawn(Node {
                        column_gap: Val::Px(4.0),
                        flex_shrink: 0.0,
                        ..default()
                    })
                    .with_children(|row| {
                        for control in pair {
                            let mut button = row.spawn((
                                Button,
                                AreaEditControlButton(control.action),
                                button_node(),
                                Interaction::None,
                                UiTooltip::new(control.label.clone()),
                                BackgroundColor(if control.enabled {
                                    theme.colors.button_default
                                } else {
                                    theme.colors.bg_elevated
                                }),
                            ));
                            if control.enabled {
                                button.insert((
                                    Button,
                                    MenuButton(UiIntent::AreaEditControl {
                                        action: control.action,
                                        revision: model.revision,
                                        epoch: model.epoch,
                                    }),
                                ));
                            }
                            button.with_children(|button| {
                                spawn_text(
                                    button,
                                    control.action.label(),
                                    &panel.font,
                                    theme.typography.font_size_sm,
                                    if control.enabled {
                                        theme.colors.text_primary_semantic
                                    } else {
                                        theme.colors.text_muted
                                    },
                                );
                            });
                        }
                    });
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(mut commands: Commands, theme: Res<UiTheme>) {
        let root = commands.spawn(Node::default()).id();
        spawn_area_edit_panel(&mut commands, root, Handle::default(), &theme);
    }

    #[test]
    fn area_details_require_accepted_release_and_reset_when_target_changes() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<UiTheme>()
            .init_resource::<UiInputState>()
            .insert_resource(AreaEditPanelModel {
                visible: true,
                controls: vec![AreaEditControl {
                    action: AreaEditAction::Load1,
                    label: "枠1を適用: 未保存".into(),
                    enabled: false,
                }],
                ..default()
            })
            .add_systems(Startup, setup)
            .add_systems(Update, update_area_edit_panel);
        app.update();
        let root = app
            .world_mut()
            .query_filtered::<Entity, With<AreaEditPanel>>()
            .single(app.world())
            .unwrap();
        assert!(
            app.world()
                .get::<bevy::ui::RelativeCursorPosition>(root)
                .is_some()
        );
        let toggle = app
            .world_mut()
            .query_filtered::<Entity, With<AreaEditDetailsToggle>>()
            .single(app.world())
            .unwrap();
        *app.world_mut().get_mut::<Interaction>(toggle).unwrap() = Interaction::Pressed;
        app.update();
        assert!(
            !app.world()
                .get::<AreaEditPanel>(root)
                .unwrap()
                .details_expanded
        );
        app.world_mut()
            .resource_mut::<UiInputState>()
            .activated_buttons
            .insert(toggle);
        app.update();
        app.world_mut()
            .resource_mut::<UiInputState>()
            .activated_buttons
            .clear();
        assert!(
            app.world()
                .get::<AreaEditPanel>(root)
                .unwrap()
                .details_expanded
        );
        let control = app
            .world_mut()
            .query_filtered::<Entity, With<AreaEditControlButton>>()
            .single(app.world())
            .unwrap();
        assert!(app.world().get::<Button>(control).is_some());
        assert!(app.world().get::<MenuButton>(control).is_none());
        assert!(
            app.world()
                .get::<UiTooltip>(control)
                .unwrap()
                .text
                .contains("未保存")
        );
        app.world_mut()
            .resource_mut::<AreaEditPanelModel>()
            .revision += 1;
        app.update();
        assert!(
            app.world()
                .get::<AreaEditPanel>(root)
                .unwrap()
                .details_expanded
        );
        let target = app.world_mut().spawn_empty().id();
        app.world_mut().resource_mut::<AreaEditPanelModel>().target = Some(target);
        app.update();
        assert!(
            !app.world()
                .get::<AreaEditPanel>(root)
                .unwrap()
                .details_expanded
        );
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<AreaEditControlButton>>()
                .iter(app.world())
                .count(),
            0
        );
    }
}
