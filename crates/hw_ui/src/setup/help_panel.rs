//! Player Help overlay.

use super::UiAssets;
use crate::components::{MenuButton, UiInputBlocker, UiInputCapture};
use crate::help::{
    HelpNavigationScrollArea, HelpPanel, HelpPanelChrome, HelpPanelContent, HelpScrollArea,
    HelpTopicBody, HelpTopicButton,
};
use crate::intents::UiIntent;
use crate::overlay::HELP_LAYER;
use crate::theme::UiTheme;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui::{FocusPolicy, RelativeCursorPosition};
use bevy::ui_widgets::{ControlOrientation, ScrollArea, Scrollbar, ScrollbarThumb};

pub fn spawn_help_panel(
    commands: &mut Commands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    parent_entity: Entity,
    content: &HelpPanelContent,
    chrome: &HelpPanelChrome,
) {
    let root = commands
        .spawn((
            Node {
                display: Display::None,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                left: Val::ZERO,
                top: Val::ZERO,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(24.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.015, 0.01, 0.025, 0.88)),
            FocusPolicy::Block,
            Pickable::default(),
            UiInputCapture,
            HelpPanel,
            HELP_LAYER,
            Name::new("Help Capture"),
        ))
        .id();
    commands.entity(parent_entity).add_child(root);

    let panel = commands
        .spawn((
            Node {
                width: Val::Percent(92.0),
                max_width: Val::Px(1_080.0),
                height: Val::Percent(90.0),
                max_height: Val::Px(720.0),
                min_height: Val::Px(360.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(16.0)),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(theme.colors.dialog_bg),
            BorderColor::all(theme.colors.dialog_border),
            RelativeCursorPosition::default(),
            Interaction::default(),
            UiInputBlocker,
            Name::new("Help Panel"),
        ))
        .id();
    commands.entity(root).add_child(panel);

    commands.entity(panel).with_children(|parent| {
        spawn_header(parent, game_assets, theme, chrome);

        parent
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(14.0),
                ..default()
            })
            .with_children(|body| {
                spawn_navigation(body, game_assets, theme, content);
                spawn_content(body, game_assets, theme, content, chrome);
            });

        parent.spawn((
            Text::new(chrome.footer_text()),
            TextFont {
                font: game_assets.font_ui().clone().into(),
                font_size: FontSize::Px(theme.typography.font_size_xs),
                ..default()
            },
            TextColor(theme.colors.text_muted),
            TextLayout::new(Justify::Center, LineBreak::WordOrCharacter),
            Node {
                width: Val::Percent(100.0),
                ..default()
            },
        ));
    });
}

fn spawn_header(
    parent: &mut ChildSpawnerCommands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    chrome: &HelpPanelChrome,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|header| {
            header.spawn((
                Text::new(chrome.copy().panel_title()),
                TextFont {
                    font: game_assets.font_ui().clone().into(),
                    font_size: FontSize::Px(theme.typography.font_size_xl),
                    weight: FontWeight::BOLD,
                    ..default()
                },
                TextColor(theme.colors.text_accent),
            ));
            header
                .spawn((
                    Button,
                    Node {
                        min_width: Val::Px(92.0),
                        height: Val::Px(34.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        padding: UiRect::horizontal(Val::Px(10.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(theme.colors.button_default),
                    BorderColor::all(theme.colors.dialog_border),
                    MenuButton(UiIntent::CloseHelp),
                ))
                .with_children(|button| {
                    button.spawn((
                        Text::new(chrome.close_button_text()),
                        TextFont {
                            font: game_assets.font_ui().clone().into(),
                            font_size: FontSize::Px(theme.typography.font_size_sm),
                            ..default()
                        },
                        TextColor(theme.colors.text_primary_semantic),
                    ));
                });
        });
}

fn spawn_navigation(
    parent: &mut ChildSpawnerCommands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    content: &HelpPanelContent,
) {
    parent
        .spawn((
            Node {
                width: Val::Percent(30.0),
                min_width: Val::Px(210.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(theme.colors.bg_surface),
            BorderColor::all(theme.colors.border_default),
            RelativeCursorPosition::default(),
            UiInputBlocker,
            ScrollArea,
            HelpNavigationScrollArea,
        ))
        .with_children(|navigation| {
            for section in content.sections() {
                navigation.spawn((
                    Text::new(section.title()),
                    TextFont {
                        font: game_assets.font_ui().clone().into(),
                        font_size: FontSize::Px(theme.typography.font_size_sm),
                        weight: FontWeight::BOLD,
                        ..default()
                    },
                    TextColor(theme.colors.text_accent_semantic),
                    Node {
                        margin: UiRect::axes(Val::Px(5.0), Val::Px(4.0)),
                        ..default()
                    },
                ));
                for topic in section.topics() {
                    navigation
                        .spawn((
                            Button,
                            Node {
                                width: Val::Percent(100.0),
                                min_height: Val::Px(30.0),
                                justify_content: JustifyContent::FlexStart,
                                align_items: AlignItems::Center,
                                padding: UiRect::horizontal(Val::Px(8.0)),
                                margin: UiRect::bottom(Val::Px(3.0)),
                                border_radius: BorderRadius::all(Val::Px(3.0)),
                                ..default()
                            },
                            BackgroundColor(theme.colors.button_default),
                            MenuButton(UiIntent::SelectHelpTopic(topic.id())),
                            HelpTopicButton(topic.id()),
                        ))
                        .with_children(|button| {
                            button.spawn((
                                Text::new(topic.title()),
                                TextFont {
                                    font: game_assets.font_ui().clone().into(),
                                    font_size: FontSize::Px(theme.typography.font_size_xs),
                                    ..default()
                                },
                                TextColor(theme.colors.text_primary_semantic),
                                TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
                            ));
                        });
                }
            }
        });
}

fn spawn_content(
    parent: &mut ChildSpawnerCommands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    content: &HelpPanelContent,
    chrome: &HelpPanelChrome,
) {
    parent
        .spawn(Node {
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            min_height: Val::Px(0.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            ..default()
        })
        .with_children(|row| {
            let scroll_area = row
                .spawn((
                    Node {
                        flex_grow: 1.0,
                        min_width: Val::Px(0.0),
                        min_height: Val::Px(0.0),
                        height: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(12.0)),
                        overflow: Overflow::scroll_y(),
                        ..default()
                    },
                    BackgroundColor(theme.colors.bg_elevated),
                    RelativeCursorPosition::default(),
                    UiInputBlocker,
                    ScrollArea,
                    HelpScrollArea,
                    Name::new("Help Content Scroll Area"),
                ))
                .id();

            row.commands().entity(scroll_area).with_children(|scroll| {
                for (index, topic) in content.topics().enumerate() {
                    scroll
                        .spawn((
                            Node {
                                display: if index == 0 {
                                    Display::Flex
                                } else {
                                    Display::None
                                },
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(12.0),
                                ..default()
                            },
                            HelpTopicBody(topic.id()),
                        ))
                        .with_children(|topic_parent| {
                            topic_parent.spawn((
                                Text::new(topic.title()),
                                TextFont {
                                    font: game_assets.font_ui().clone().into(),
                                    font_size: FontSize::Px(theme.typography.font_size_lg),
                                    weight: FontWeight::BOLD,
                                    ..default()
                                },
                                TextColor(theme.colors.text_accent),
                                TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
                            ));

                            for entry in topic.entries() {
                                spawn_entry(topic_parent, game_assets, theme, entry, chrome);
                            }
                        });
                }
            });

            row.spawn((
                Node {
                    width: Val::Px(6.0),
                    height: Val::Percent(100.0),
                    margin: UiRect::left(Val::Px(4.0)),
                    ..default()
                },
                Scrollbar::new(scroll_area, ControlOrientation::Vertical, 20.0),
            ))
            .with_children(|scrollbar| {
                scrollbar.spawn((
                    ScrollbarThumb {
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        border: UiRect::ZERO,
                    },
                    BackgroundColor(theme.colors.text_muted),
                ));
            });
        });
}

fn spawn_entry(
    parent: &mut ChildSpawnerCommands,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
    entry: &crate::help::HelpEntry,
    chrome: &HelpPanelChrome,
) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                row_gap: Val::Px(5.0),
                ..default()
            },
            BackgroundColor(theme.colors.bg_surface),
            BorderColor::all(theme.colors.border_default),
        ))
        .with_children(|entry_parent| {
            entry_parent.spawn((
                Text::new(entry.title()),
                TextFont {
                    font: game_assets.font_ui().clone().into(),
                    font_size: FontSize::Px(theme.typography.font_size_base),
                    weight: FontWeight::SEMIBOLD,
                    ..default()
                },
                TextColor(theme.colors.text_primary_semantic),
                TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
            ));
            if let Some(shortcut) = entry.shortcut() {
                entry_parent.spawn((
                    Text::new(chrome.entry_shortcut_text(shortcut)),
                    TextFont {
                        font: game_assets.font_ui().clone().into(),
                        font_size: FontSize::Px(theme.typography.font_size_xs),
                        weight: FontWeight::BOLD,
                        ..default()
                    },
                    TextColor(theme.colors.accent_soul_bright),
                    TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
                ));
            }
            for paragraph in entry.paragraphs() {
                entry_parent.spawn((
                    Text::new(paragraph),
                    TextFont {
                        font: game_assets.font_ui().clone().into(),
                        font_size: FontSize::Px(theme.typography.font_size_sm),
                        ..default()
                    },
                    TextColor(theme.colors.text_secondary_semantic),
                    TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
                    Node {
                        width: Val::Percent(100.0),
                        ..default()
                    },
                ));
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::help::{HelpEntry, HelpEntryId, HelpSection, HelpSectionId, HelpTopic, HelpTopicId};
    use crate::setup::test_support::{TestAssets, sentinel_help_chrome};

    fn spawn_help(mut commands: Commands, theme: Res<UiTheme>) {
        let parent = commands.spawn(Node::default()).id();
        let content = HelpPanelContent::new([HelpSection::new(
            HelpSectionId::new("section"),
            "Section",
            [HelpTopic::new(
                HelpTopicId::new("topic"),
                "Topic",
                [HelpEntry::new(HelpEntryId::new("entry"), "Entry", ["Body"])
                    .with_shortcut("Ctrl+K")],
            )],
        )]);
        spawn_help_panel(
            &mut commands,
            &TestAssets::default(),
            &theme,
            parent,
            &content,
            &sentinel_help_chrome(),
        );
    }

    #[test]
    fn help_uses_hidden_capture_standard_scroll_and_injected_chrome_copy() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<UiTheme>()
            .add_systems(Startup, spawn_help);
        app.update();

        let mut roots = app.world_mut().query_filtered::<
            (&Node, &FocusPolicy, &Pickable, &GlobalZIndex),
            (With<HelpPanel>, With<UiInputCapture>),
        >();
        let (node, focus, pickable, layer) = roots.single(app.world()).unwrap();
        assert_eq!(node.display, Display::None);
        assert_eq!(*focus, FocusPolicy::Block);
        assert_eq!(*pickable, Pickable::default());
        assert_eq!(*layer, HELP_LAYER);
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, (With<ScrollArea>, With<HelpScrollArea>)>()
                .iter(app.world())
                .count(),
            1
        );

        let mut text_query = app.world_mut().query::<&Text>();
        let texts: Vec<_> = text_query
            .iter(app.world())
            .map(|text| text.0.as_str())
            .collect();
        assert!(texts.contains(&"Injected Help Title"));
        assert!(texts.contains(&"Injected Close (Ctrl+F1 / Esc)"));
        assert!(texts.contains(
            &"Ctrl+F1 / Esc: Injected Close  PrevTopic / NextTopic: Injected Topics  \
              PrevPage / NextPage: Injected Pages  DocumentStart / DocumentEnd: Injected Bounds"
        ));
        assert!(texts.contains(&"Injected Shortcut: Ctrl+K"));
    }
}
