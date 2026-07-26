//! 一時停止メニュー（Save / Load）

use super::UiAssets;
use crate::components::{
    MenuAction, MenuButton, PauseMenu, UiInputBlocker, UiInputCapture, UiTooltip,
};
use crate::help::HelpPanelChrome;
use crate::overlay::PAUSE_LAYER;
use crate::theme::UiTheme;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui::{FocusPolicy, RelativeCursorPosition};

fn spawn_menu_button(
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
                width: Val::Percent(100.0),
                height: Val::Px(36.0),
                margin: UiRect::bottom(Val::Px(8.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(theme.colors.button_default),
            BorderColor::all(theme.colors.dialog_border),
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
                    font_size: FontSize::Px(theme.typography.font_size_base),
                    ..default()
                },
                TextColor(theme.colors.text_primary_semantic),
            ));
        });
}

/// 一時停止中に表示する Save / Load メニューをスポーンする。
pub fn spawn_pause_menu(
    commands: &mut Commands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    parent_entity: Entity,
    help_chrome: &HelpPanelChrome,
) {
    let pause_menu = commands
        .spawn((
            Node {
                display: Display::None,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                ..default()
            },
            FocusPolicy::Block,
            Pickable::default(),
            UiInputCapture,
            PauseMenu,
            PAUSE_LAYER,
            Name::new("Pause Capture"),
        ))
        .id();
    commands.entity(parent_entity).add_child(pause_menu);

    let panel = commands
        .spawn((
            Node {
                width: Val::Px(260.0),
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(45.0),
                margin: UiRect::left(Val::Px(-130.0)),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(16.0)),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                row_gap: Val::Px(4.0),
                ..default()
            },
            BackgroundColor(theme.colors.dialog_bg),
            BorderColor::all(theme.colors.dialog_border),
            Interaction::default(),
            RelativeCursorPosition::default(),
            UiInputBlocker,
            Name::new("Pause Panel"),
        ))
        .id();
    commands.entity(pause_menu).add_child(panel);

    commands.entity(panel).with_children(|parent| {
        parent.spawn((
            Text::new("Paused"),
            TextFont {
                font: game_assets.font_ui().clone().into(),
                font_size: FontSize::Px(theme.typography.font_size_xl),
                ..default()
            },
            TextColor(theme.colors.text_accent),
            Node {
                margin: UiRect::bottom(Val::Px(12.0)),
                align_self: AlignSelf::Center,
                ..default()
            },
        ));

        spawn_menu_button(
            parent,
            game_assets,
            theme,
            "Resume",
            "Resume",
            MenuAction::TogglePause,
            None,
        );
        spawn_menu_button(
            parent,
            game_assets,
            theme,
            "Save Game",
            "Save Game",
            MenuAction::SaveGame,
            None,
        );
        spawn_menu_button(
            parent,
            game_assets,
            theme,
            "Load Game",
            "Load Game",
            MenuAction::RequestLoadGame,
            None,
        );
        spawn_menu_button(
            parent,
            game_assets,
            theme,
            "Settings",
            "Settings",
            MenuAction::ToggleSettings,
            None,
        );
        spawn_menu_button(
            parent,
            game_assets,
            theme,
            help_chrome.copy().launcher_label(),
            help_chrome.copy().launcher_tooltip(),
            MenuAction::OpenHelp { opener: None },
            Some(help_chrome.launcher_shortcut()),
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::UiInputCapture;
    use crate::setup::test_support::{TestAssets, sentinel_help_chrome};

    fn spawn_pause(mut commands: Commands, theme: Res<UiTheme>) {
        let parent = commands.spawn(Node::default()).id();
        let chrome = sentinel_help_chrome();
        spawn_pause_menu(
            &mut commands,
            &TestAssets::default(),
            &theme,
            parent,
            &chrome,
        );
    }

    #[test]
    fn pause_uses_blocking_viewport_root_with_resume_inside_panel() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<UiTheme>()
            .add_systems(Startup, spawn_pause);

        app.update();

        let mut roots = app.world_mut().query_filtered::<
            (Entity, &Node, &FocusPolicy, &Pickable, &Children),
            (With<PauseMenu>, With<UiInputCapture>),
        >();
        let (root, node, focus, pickable, children) = roots.single(app.world()).unwrap();
        assert_eq!(node.display, Display::None);
        assert_eq!(node.width, Val::Percent(100.0));
        assert_eq!(node.height, Val::Percent(100.0));
        assert_eq!(*focus, FocusPolicy::Block);
        assert_eq!(*pickable, Pickable::default());
        assert_eq!(children.len(), 1);

        let panel = children[0];
        assert!(app.world().entity(panel).contains::<UiInputBlocker>());
        assert_eq!(
            app.world().entity(panel).get::<ChildOf>().unwrap().parent(),
            root
        );
        let mut buttons = app.world_mut().query::<&MenuButton>();
        assert!(
            buttons
                .iter(app.world())
                .any(|button| matches!(button.0, MenuAction::TogglePause))
        );

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
            .expect("Pause menu Help launcher");
        assert_eq!(tooltip_text, "Injected Help Tooltip");
        assert_eq!(shortcut.as_deref(), Some("Ctrl+F1"));
        assert_eq!(
            app.world().get::<Text>(label_entity).unwrap().0,
            "Injected Help"
        );
    }
}
