use bevy::prelude::*;

use crate::UiIntent;
use crate::help::{
    HelpPanel, HelpPanelContent, HelpPanelState, HelpScrollArea, HelpScrollCommand, HelpTopicBody,
    HelpTopicButton,
};
use crate::theme::UiTheme;
use bevy::ui_widgets::ScrollIntoView;

pub fn handle_help_navigation_system(
    mut intents: MessageReader<UiIntent>,
    content: Res<HelpPanelContent>,
    mut state: ResMut<HelpPanelState>,
    mut scroll_area: Query<(&Node, &ComputedNode, &mut ScrollPosition), With<HelpScrollArea>>,
) {
    let mut reset_scroll = false;
    for intent in intents.read().copied() {
        if !state.open {
            continue;
        }

        match intent {
            UiIntent::SelectHelpTopic(topic) if content.contains_topic(topic) => {
                state.select_topic(topic);
                reset_scroll = true;
            }
            UiIntent::StepHelpTopic(step) => {
                if let Some(current) = state.active_topic
                    && let Some(next) = content.adjacent_topic(current, step)
                {
                    state.select_topic(next);
                    reset_scroll = true;
                }
            }
            UiIntent::ScrollHelp(command) => {
                if let Ok((node, computed, mut position)) = scroll_area.single_mut() {
                    apply_scroll_command(node, computed, &mut position, command);
                }
            }
            _ => {}
        }
    }

    if reset_scroll && let Ok((_, _, mut position)) = scroll_area.single_mut() {
        position.0 = Vec2::ZERO;
    }
}

fn apply_scroll_command(
    node: &Node,
    computed: &ComputedNode,
    position: &mut ScrollPosition,
    command: HelpScrollCommand,
) {
    let viewport_height = computed.size().y * computed.inverse_scale_factor();
    let content_height = computed.content_size().y * computed.inverse_scale_factor();
    let max_y = (content_height - viewport_height).max(0.0);
    position.y = match command {
        HelpScrollCommand::PageUp => position.y - viewport_height,
        HelpScrollCommand::PageDown => position.y + viewport_height,
        HelpScrollCommand::Start => 0.0,
        HelpScrollCommand::End => max_y,
    }
    .clamp(0.0, max_y);

    if node.overflow.y != OverflowAxis::Scroll {
        position.y = 0.0;
    }
}

pub fn update_help_panel_visibility_system(
    state: Res<HelpPanelState>,
    mut root: Query<&mut Node, With<HelpPanel>>,
) {
    let display = if state.open {
        Display::Flex
    } else {
        Display::None
    };
    if let Ok(mut node) = root.single_mut() {
        node.display = display;
    }
}

pub fn update_help_topic_presentation_system(
    state: Res<HelpPanelState>,
    theme: Res<UiTheme>,
    mut commands: Commands,
    mut topics: Query<(&HelpTopicBody, &mut Node)>,
    mut buttons: Query<(Entity, &HelpTopicButton, &Interaction, &mut BackgroundColor)>,
) {
    for (topic, mut node) in &mut topics {
        node.display = if state.active_topic == Some(topic.0) {
            Display::Flex
        } else {
            Display::None
        };
    }

    for (entity, topic, interaction, mut color) in &mut buttons {
        let selected = state.active_topic == Some(topic.0);
        color.0 = help_topic_button_color(selected, *interaction, &theme);
        if selected && state.is_changed() {
            commands.trigger(ScrollIntoView { entity });
        }
    }
}

fn help_topic_button_color(selected: bool, interaction: Interaction, theme: &UiTheme) -> Color {
    if selected {
        match interaction {
            Interaction::Pressed => theme.colors.button_pressed,
            Interaction::Hovered => theme.colors.list_item_selected_hover,
            Interaction::None => theme.colors.list_item_selected,
        }
    } else {
        match interaction {
            Interaction::Pressed => theme.colors.button_pressed,
            Interaction::Hovered => theme.colors.button_hover,
            Interaction::None => theme.colors.button_default,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::help::{
        HelpEntry, HelpEntryId, HelpSection, HelpSectionId, HelpTopic, HelpTopicId, HelpTopicStep,
    };

    const FIRST: HelpTopicId = HelpTopicId::new("first");
    const SECOND: HelpTopicId = HelpTopicId::new("second");

    fn test_content() -> HelpPanelContent {
        HelpPanelContent::new([HelpSection::new(
            HelpSectionId::new("section"),
            "Section",
            [
                HelpTopic::new(
                    FIRST,
                    "First",
                    [HelpEntry::new(
                        HelpEntryId::new("first-entry"),
                        "Entry",
                        ["Body"],
                    )],
                ),
                HelpTopic::new(SECOND, "Second", []),
            ],
        )])
    }

    #[test]
    fn selected_and_unselected_topics_preserve_hover_feedback() {
        let theme = UiTheme::default();
        assert_eq!(
            help_topic_button_color(true, Interaction::Hovered, &theme),
            theme.colors.list_item_selected_hover
        );
        assert_eq!(
            help_topic_button_color(false, Interaction::Hovered, &theme),
            theme.colors.button_hover
        );
        assert_eq!(
            help_topic_button_color(false, Interaction::None, &theme),
            theme.colors.button_default
        );
    }

    #[test]
    fn closed_help_ignores_navigation_and_invalid_topics() {
        let mut app = App::new();
        app.add_message::<UiIntent>()
            .insert_resource(test_content())
            .init_resource::<HelpPanelState>()
            .add_systems(Update, handle_help_navigation_system);
        app.world_mut().spawn((
            Node {
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ComputedNode::default(),
            ScrollPosition::default(),
            HelpScrollArea,
        ));

        app.world_mut()
            .resource_mut::<Messages<UiIntent>>()
            .write(UiIntent::SelectHelpTopic(SECOND));
        app.update();
        assert_eq!(app.world().resource::<HelpPanelState>().active_topic, None);

        app.world_mut()
            .resource_mut::<HelpPanelState>()
            .open_at(FIRST);
        app.world_mut()
            .resource_mut::<Messages<UiIntent>>()
            .write(UiIntent::SelectHelpTopic(HelpTopicId::new("unknown")));
        app.update();
        assert_eq!(
            app.world().resource::<HelpPanelState>().active_topic,
            Some(FIRST)
        );
    }

    #[test]
    fn arrow_navigation_uses_content_order_and_resets_scroll() {
        let mut app = App::new();
        app.add_message::<UiIntent>()
            .insert_resource(test_content())
            .init_resource::<HelpPanelState>()
            .add_systems(Update, handle_help_navigation_system);
        app.world_mut().spawn((
            Node {
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ComputedNode::default(),
            ScrollPosition(Vec2::new(0.0, 120.0)),
            HelpScrollArea,
        ));
        app.world_mut()
            .resource_mut::<HelpPanelState>()
            .open_at(FIRST);
        app.world_mut()
            .resource_mut::<Messages<UiIntent>>()
            .write(UiIntent::StepHelpTopic(HelpTopicStep::Next));

        app.update();

        assert_eq!(
            app.world().resource::<HelpPanelState>().active_topic,
            Some(SECOND)
        );
        let position = app
            .world_mut()
            .query_filtered::<&ScrollPosition, With<HelpScrollArea>>()
            .single(app.world())
            .unwrap();
        assert_eq!(position.0, Vec2::ZERO);
    }
}
