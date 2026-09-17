//! ボトムバー UI

use super::UiAssets;
use crate::components::{
    MenuAction, MenuButton, UiInputBlocker, UiNodeRegistry, UiSlot, UiTooltip,
};
use crate::help::HelpPanelChrome;
use crate::theme::UiTheme;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::ui::{BackgroundGradient, ColorStop, LinearGradient};

/// ボトムバーをスポーン
pub fn spawn_bottom_bar(
    commands: &mut Commands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    parent_entity: Entity,
    ui_nodes: &mut UiNodeRegistry,
    help_chrome: &HelpPanelChrome,
) {
    let bottom_bar = commands
        .spawn((
            Node {
                width: Val::Auto,
                height: Val::Px(44.0),
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                bottom: Val::Px(12.0),
                column_gap: Val::Px(4.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Start,
                padding: UiRect::all(Val::Px(theme.spacing.bottom_bar_padding)),
                border: UiRect::all(Val::Px(theme.sizes.panel_border_width)),
                border_radius: BorderRadius::all(Val::Px(theme.sizes.panel_corner_radius)),
                ..default()
            },
            BackgroundGradient::from(LinearGradient {
                angle: std::f32::consts::FRAC_PI_2,
                stops: vec![
                    ColorStop::new(theme.panels.bottom_bar.top, Val::Percent(0.0)),
                    ColorStop::new(theme.panels.bottom_bar.bottom, Val::Percent(100.0)),
                ],
                ..default()
            }),
            BorderColor::all(theme.colors.panel_accent_control_bar),
            RelativeCursorPosition::default(),
            UiInputBlocker,
        ))
        .id();
    commands.entity(parent_entity).add_child(bottom_bar);

    commands.entity(bottom_bar).with_children(|parent| {
        let buttons = [
            (
                "建てる",
                "建築物の種類を選び、地図に配置します",
                MenuAction::ToggleArchitect,
                Some("B"),
            ),
            (
                "作業を指示",
                "伐採・採掘・運搬などの仕事を指定します",
                MenuAction::ToggleOrders,
                None,
            ),
            (
                "範囲を設定",
                "保管場所やYardの範囲を設定します",
                MenuAction::ToggleZones,
                Some("Z"),
            ),
            (
                "管理",
                "使い魔・魂・仕事の一覧を開きます",
                MenuAction::Workspace(crate::shell::WorkspaceAction::ToggleManagement),
                None,
            ),
            (
                "表示",
                "担当範囲・保管場所・給電・部屋を地図で確認します",
                MenuAction::Workspace(crate::shell::WorkspaceAction::ToggleDisplay),
                None,
            ),
            ("Dream", "Dreamメニュー切替", MenuAction::ToggleDream, None),
        ];

        for (label, tooltip, action, shortcut) in buttons {
            spawn_bottom_bar_button(parent, game_assets, theme, label, tooltip, action, shortcut);
        }
        spawn_bottom_bar_button(
            parent,
            game_assets,
            theme,
            help_chrome.copy().launcher_label(),
            help_chrome.copy().launcher_tooltip(),
            MenuAction::OpenHelp { opener: None },
            Some(help_chrome.launcher_shortcut()),
        );

        // Mode Display
        let mode_text = parent
            .spawn((
                Text::new(""),
                TextFont {
                    font: game_assets.font_ui().clone().into(),
                    font_size: crate::theme::font_size_rem(theme.typography.font_size_md), // Semantic
                    weight: FontWeight::BOLD,
                    ..default()
                },
                TextColor(theme.colors.text_primary_semantic),
                TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
                BackgroundColor(theme.colors.bg_surface),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(8.0),
                    bottom: Val::Px(theme.spacing.bottom_bar_height + 8.0),
                    max_width: Val::Vw(94.0),
                    display: Display::None,
                    padding: UiRect::all(Val::Px(6.0)),
                    ..default()
                },
                UiSlot::ModeText,
            ))
            .id();
        ui_nodes.set_slot(UiSlot::ModeText, mode_text);
    });
}

fn spawn_bottom_bar_button(
    parent: &mut ChildSpawnerCommands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    label: &str,
    tooltip: &str,
    action: MenuAction,
    shortcut: Option<&str>,
) {
    parent
        .spawn((
            Button,
            Node {
                min_width: Val::Px(76.0),
                height: Val::Px(36.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
                flex_direction: FlexDirection::RowReverse,
                column_gap: Val::Px(6.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme.colors.button_default),
            MenuButton(action),
            match shortcut {
                Some(shortcut) => UiTooltip::with_shortcut(tooltip.to_owned(), shortcut.to_owned()),
                None => UiTooltip::new(tooltip.to_owned()),
            },
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                TextFont {
                    font: game_assets.font_ui().clone().into(),
                    font_size: crate::theme::font_size_rem(theme.typography.font_size_base),
                    weight: FontWeight::SEMIBOLD,
                    ..default()
                },
                TextColor(theme.colors.text_primary_semantic),
            ));
            let icon = match action {
                MenuAction::ToggleArchitect => Some(game_assets.icon_hammer()),
                MenuAction::ToggleOrders => Some(game_assets.icon_axe()),
                MenuAction::ToggleZones => Some(game_assets.icon_haul()),
                MenuAction::Workspace(crate::shell::WorkspaceAction::ToggleManagement) => {
                    Some(game_assets.icon_idle())
                }
                MenuAction::Workspace(crate::shell::WorkspaceAction::ToggleDisplay) => {
                    Some(game_assets.icon_arrow_down())
                }
                MenuAction::ToggleDream => Some(game_assets.glow_circle()),
                _ => None,
            };
            if let Some(icon) = icon {
                button.spawn((
                    ImageNode::new(icon.clone()),
                    Node {
                        width: Val::Px(16.0),
                        height: Val::Px(16.0),
                        flex_shrink: 0.0,
                        ..default()
                    },
                ));
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::test_support::{TestAssets, sentinel_help_chrome};

    fn spawn_bar(
        mut commands: Commands,
        theme: Res<UiTheme>,
        mut ui_nodes: ResMut<UiNodeRegistry>,
    ) {
        let parent = commands.spawn(Node::default()).id();
        spawn_bottom_bar(
            &mut commands,
            &TestAssets::default(),
            &theme,
            parent,
            &mut ui_nodes,
            &sentinel_help_chrome(),
        );
    }

    #[test]
    fn help_launcher_uses_injected_label_tooltip_and_shortcut() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<UiTheme>()
            .init_resource::<UiNodeRegistry>()
            .add_systems(Startup, spawn_bar);
        app.update();

        let mut help_buttons = app
            .world_mut()
            .query::<(&MenuButton, &UiTooltip, &Children)>();
        let (tooltip_text, shortcut, label_entity) = help_buttons
            .iter(app.world())
            .find_map(|(button, tooltip, children)| {
                matches!(button.0, MenuAction::OpenHelp { .. }).then(|| {
                    (
                        tooltip.text.to_string(),
                        tooltip.shortcut.as_deref().map(str::to_owned),
                        children[0],
                    )
                })
            })
            .expect("bottom bar Help launcher");
        assert_eq!(tooltip_text, "Injected Help Tooltip");
        assert_eq!(shortcut.as_deref(), Some("Ctrl+F1"));
        assert_eq!(
            app.world().get::<Text>(label_entity).unwrap().0,
            "Injected Help"
        );
    }
}
