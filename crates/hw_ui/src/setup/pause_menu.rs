//! 一時停止メニュー（Save / Load）

use super::UiAssets;
use crate::components::{MenuAction, MenuButton, PauseMenu, UiInputBlocker, UiInputCapture};
use crate::theme::UiTheme;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui::{FocusPolicy, RelativeCursorPosition};

fn spawn_menu_button(
    parent: &mut ChildSpawnerCommands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    label: &str,
    action: MenuAction,
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
            ZIndex(35),
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
            MenuAction::TogglePause,
        );
        spawn_menu_button(
            parent,
            game_assets,
            theme,
            "Save Game",
            MenuAction::SaveGame,
        );
        spawn_menu_button(
            parent,
            game_assets,
            theme,
            "Load Game",
            MenuAction::RequestLoadGame,
        );
        spawn_menu_button(
            parent,
            game_assets,
            theme,
            "Settings",
            MenuAction::ToggleSettings,
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::UiInputCapture;

    #[derive(Default)]
    struct TestAssets {
        font: Handle<Font>,
        image: Handle<Image>,
    }

    impl UiAssets for TestAssets {
        fn font_ui(&self) -> &Handle<Font> {
            &self.font
        }
        fn font_familiar(&self) -> &Handle<Font> {
            &self.font
        }
        fn font_soul_name(&self) -> &Handle<Font> {
            &self.font
        }
        fn icon_arrow_down(&self) -> &Handle<Image> {
            &self.image
        }
        fn icon_arrow_right(&self) -> &Handle<Image> {
            &self.image
        }
        fn icon_idle(&self) -> &Handle<Image> {
            &self.image
        }
        fn glow_circle(&self) -> &Handle<Image> {
            &self.image
        }
        fn icon_stress(&self) -> &Handle<Image> {
            &self.image
        }
        fn icon_fatigue(&self) -> &Handle<Image> {
            &self.image
        }
        fn icon_male(&self) -> &Handle<Image> {
            &self.image
        }
        fn icon_female(&self) -> &Handle<Image> {
            &self.image
        }
        fn icon_axe(&self) -> &Handle<Image> {
            &self.image
        }
        fn icon_pick(&self) -> &Handle<Image> {
            &self.image
        }
        fn icon_hammer(&self) -> &Handle<Image> {
            &self.image
        }
        fn icon_haul(&self) -> &Handle<Image> {
            &self.image
        }
        fn icon_bone_small(&self) -> &Handle<Image> {
            &self.image
        }
    }

    fn spawn_pause(mut commands: Commands, theme: Res<UiTheme>) {
        let parent = commands.spawn(Node::default()).id();
        spawn_pause_menu(&mut commands, &TestAssets::default(), &theme, parent);
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
    }
}
