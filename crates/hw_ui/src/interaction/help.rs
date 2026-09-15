use bevy::prelude::*;

use crate::UiIntent;
use crate::help::{
    HelpPanel, HelpPanelContent, HelpPanelState, HelpScrollArea, HelpScrollCommand, HelpTopicBody,
    HelpTopicButton,
};
use crate::theme::UiTheme;
use bevy::ui_widgets::ScrollIntoView;

#[derive(Default)]
pub struct HelpReadingPositions {
    positions: std::collections::HashMap<crate::help::HelpTopicId, Vec2>,
    was_open: bool,
}

type SearchNavigationNodes<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Node,
        Option<&'static crate::help::HelpSearchResult>,
        Has<crate::help::HelpSearchEmpty>,
    ),
    Or<(
        With<crate::help::HelpSearchResult>,
        With<crate::help::HelpSearchEmpty>,
        With<HelpTopicButton>,
        With<crate::help::HelpNavigationHeading>,
    )>,
>;

pub fn sync_help_search(
    state: Res<HelpPanelState>,
    fields: Query<(&bevy::text::EditableText, &crate::widgets::TextFieldRole)>,
    mut nodes: SearchNavigationNodes,
    mut last_query: Local<Option<String>>,
    mut navigation: Query<&mut ScrollPosition, With<crate::help::HelpNavigationScrollArea>>,
) {
    if !state.open {
        return;
    }
    let Some((editable, _)) = fields
        .iter()
        .find(|(_, role)| **role == crate::widgets::TextFieldRole::HelpSearch)
    else {
        return;
    };
    let query = crate::widgets::editable_text_value(editable)
        .trim()
        .to_lowercase();
    if last_query.as_ref() == Some(&query) {
        return;
    }
    for mut position in &mut navigation {
        position.0 = Vec2::ZERO;
    }
    let terms: Vec<_> = query.split_whitespace().collect();
    let any_matches = nodes.iter().any(|(_, result, _)| {
        result.is_some_and(|result| search_matches(&result.searchable, &terms))
    });
    for (mut node, result, empty) in &mut nodes {
        let visible = if let Some(result) = result {
            !terms.is_empty() && search_matches(&result.searchable, &terms)
        } else if empty {
            !terms.is_empty() && !any_matches
        } else {
            terms.is_empty()
        };
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
    }
    *last_query = Some(query);
}

fn search_matches(text: &str, terms: &[&str]) -> bool {
    terms.iter().all(|term| text.contains(term))
}

pub fn handle_help_navigation_system(
    mut intents: MessageReader<UiIntent>,
    content: Res<HelpPanelContent>,
    mut state: ResMut<HelpPanelState>,
    mut scroll_area: Query<(&Node, &ComputedNode, &mut ScrollPosition), With<HelpScrollArea>>,
    mut reading: Local<HelpReadingPositions>,
    entries: Query<(Entity, &crate::help::HelpEntryBody)>,
    mut commands: Commands,
) {
    if state.active_topic.is_none() {
        reading.positions.clear();
    }
    if state.open
        && !reading.was_open
        && let Some(saved) = state
            .active_topic
            .and_then(|topic| reading.positions.get(&topic))
            .copied()
        && let Ok((_, _, mut position)) = scroll_area.single_mut()
    {
        position.0 = saved;
    }
    reading.was_open = state.open;
    for intent in intents.read().copied() {
        if !state.open {
            continue;
        }

        match intent {
            UiIntent::SelectHelpEntry(entry) => {
                if let Some(topic) = content.entry_topic(entry)
                    && let Ok((_, _, mut position)) = scroll_area.single_mut()
                    && let Some((entity, _)) = entries.iter().find(|(_, body)| body.0 == entry)
                {
                    change_topic(&mut state, topic, &mut position, &mut reading);
                    commands.entity(entity).insert(HelpEntryJump);
                }
            }
            UiIntent::SelectHelpTopic(topic) if content.contains_topic(topic) => {
                if let Ok((_, _, mut position)) = scroll_area.single_mut() {
                    change_topic(&mut state, topic, &mut position, &mut reading);
                }
            }
            UiIntent::StepHelpTopic(step) => {
                if let Some(current) = state.active_topic
                    && let Some(next) = content.adjacent_topic(current, step)
                    && let Ok((_, _, mut position)) = scroll_area.single_mut()
                {
                    change_topic(&mut state, next, &mut position, &mut reading);
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
    if state.open
        && let Some(topic) = state.active_topic
        && let Ok((_, _, position)) = scroll_area.single()
    {
        reading.positions.insert(topic, position.0);
    }
}

#[derive(Component)]
pub struct HelpEntryJump;

pub fn apply_help_entry_jump(
    state: Res<HelpPanelState>,
    entries: Query<(Entity, &ComputedNode), With<HelpEntryJump>>,
    mut commands: Commands,
) {
    for (entity, computed) in &entries {
        if state.open && computed.size().y > 0.0 {
            commands.trigger(ScrollIntoView { entity });
        }
        commands.entity(entity).remove::<HelpEntryJump>();
    }
}

fn change_topic(
    state: &mut HelpPanelState,
    next: crate::help::HelpTopicId,
    position: &mut ScrollPosition,
    reading: &mut HelpReadingPositions,
) {
    if state.active_topic == Some(next) {
        return;
    }
    if let Some(previous) = state.active_topic {
        reading.positions.insert(previous, position.0);
    }
    state.select_topic(next);
    position.0 = reading.positions.get(&next).copied().unwrap_or_default();
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
    mut topics: Query<(&HelpTopicBody, &mut Node), Without<HelpTopicButton>>,
    mut buttons: Query<(
        Entity,
        &HelpTopicButton,
        &Interaction,
        &Node,
        &mut BackgroundColor,
    )>,
) {
    for (topic, mut node) in &mut topics {
        node.display = if state.active_topic == Some(topic.0) {
            Display::Flex
        } else {
            Display::None
        };
    }

    for (entity, topic, interaction, node, mut color) in &mut buttons {
        let selected = state.active_topic == Some(topic.0);
        color.0 = help_topic_button_color(selected, *interaction, &theme);
        if selected && state.is_changed() && node.display != Display::None {
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
    fn help_topic_switch_restores_each_reading_position() {
        let mut state = HelpPanelState::default();
        state.open_at(FIRST);
        let mut reading = HelpReadingPositions::default();
        let mut position = ScrollPosition(Vec2::new(0.0, 120.0));
        change_topic(&mut state, SECOND, &mut position, &mut reading);
        assert_eq!(position.0, Vec2::ZERO);
        position.y = 240.0;
        change_topic(&mut state, FIRST, &mut position, &mut reading);
        assert_eq!(position.y, 120.0);
        change_topic(&mut state, SECOND, &mut position, &mut reading);
        assert_eq!(position.y, 240.0);
        change_topic(&mut state, SECOND, &mut position, &mut reading);
        assert_eq!(position.y, 240.0);
    }

    #[test]
    fn help_search_matches_body_and_all_terms_and_restores_navigation() {
        use crate::help::{HelpNavigationHeading, HelpSearchEmpty, HelpSearchResult};
        use crate::widgets::TextFieldRole;
        use bevy::text::EditableText;
        let mut app = App::new();
        app.init_resource::<HelpPanelState>()
            .add_systems(Update, sync_help_search);
        let field = app
            .world_mut()
            .spawn((EditableText::new("BONE 搬入"), TextFieldRole::HelpSearch))
            .id();
        let matched = app
            .world_mut()
            .spawn((
                Node::default(),
                HelpSearchResult {
                    entry: HelpEntryId::new("bone"),
                    searchable: "bone の搬入を確認".into(),
                },
            ))
            .id();
        let other = app
            .world_mut()
            .spawn((
                Node::default(),
                HelpSearchResult {
                    entry: HelpEntryId::new("other"),
                    searchable: "bone を回収".into(),
                },
            ))
            .id();
        let heading = app
            .world_mut()
            .spawn((Node::default(), HelpNavigationHeading))
            .id();
        let empty = app
            .world_mut()
            .spawn((Node::default(), HelpSearchEmpty))
            .id();
        app.world_mut()
            .resource_mut::<HelpPanelState>()
            .open_at(FIRST);
        app.update();
        assert_eq!(
            app.world().get::<Node>(matched).unwrap().display,
            Display::Flex
        );
        assert_eq!(
            app.world().get::<Node>(other).unwrap().display,
            Display::None
        );
        assert_eq!(
            app.world().get::<Node>(heading).unwrap().display,
            Display::None
        );
        assert_eq!(
            app.world().get::<Node>(empty).unwrap().display,
            Display::None
        );
        app.world_mut()
            .entity_mut(field)
            .insert(EditableText::new("unknown"));
        app.update();
        assert_eq!(
            app.world().get::<Node>(empty).unwrap().display,
            Display::Flex
        );
        app.world_mut()
            .get_mut::<EditableText>(field)
            .unwrap()
            .clear();
        app.update();
        assert_eq!(
            app.world().get::<Node>(heading).unwrap().display,
            Display::Flex
        );
        assert_eq!(
            app.world().get::<Node>(matched).unwrap().display,
            Display::None
        );
    }

    #[test]
    fn help_entry_navigation_requires_open_help_and_a_catalog_entry() {
        let mut app = App::new();
        app.add_message::<UiIntent>()
            .insert_resource(test_content())
            .init_resource::<HelpPanelState>()
            .add_systems(Update, handle_help_navigation_system);
        app.world_mut().spawn((
            Node::default(),
            ComputedNode::default(),
            ScrollPosition::default(),
            HelpScrollArea,
        ));
        let entry = HelpEntryId::new("first-entry");
        let body = app
            .world_mut()
            .spawn(crate::help::HelpEntryBody(entry))
            .id();
        app.world_mut()
            .resource_mut::<Messages<UiIntent>>()
            .write(UiIntent::SelectHelpEntry(entry));
        app.update();
        assert!(app.world().get::<HelpEntryJump>(body).is_none());
        app.world_mut()
            .resource_mut::<HelpPanelState>()
            .open_at(SECOND);
        app.world_mut()
            .resource_mut::<Messages<UiIntent>>()
            .write(UiIntent::SelectHelpEntry(HelpEntryId::new("missing")));
        app.update();
        assert_eq!(
            app.world().resource::<HelpPanelState>().active_topic,
            Some(SECOND)
        );
        app.world_mut()
            .resource_mut::<Messages<UiIntent>>()
            .write(UiIntent::SelectHelpEntry(entry));
        app.update();
        assert_eq!(
            app.world().resource::<HelpPanelState>().active_topic,
            Some(FIRST)
        );
        assert!(app.world().get::<HelpEntryJump>(body).is_some());
    }

    #[test]
    fn help_reopen_restores_position_lost_while_hidden_and_reset_forgets_it() {
        let mut app = App::new();
        app.add_message::<UiIntent>()
            .insert_resource(test_content())
            .init_resource::<HelpPanelState>()
            .add_systems(Update, handle_help_navigation_system);
        let scroll = app
            .world_mut()
            .spawn((
                Node::default(),
                ComputedNode::default(),
                ScrollPosition(Vec2::new(0.0, 180.0)),
                HelpScrollArea,
            ))
            .id();
        app.world_mut()
            .resource_mut::<HelpPanelState>()
            .open_at(FIRST);
        app.update();
        app.world_mut().resource_mut::<HelpPanelState>().close();
        app.update();
        app.world_mut().get_mut::<ScrollPosition>(scroll).unwrap().0 = Vec2::ZERO;
        app.world_mut()
            .resource_mut::<HelpPanelState>()
            .open_at(FIRST);
        app.update();
        assert_eq!(app.world().get::<ScrollPosition>(scroll).unwrap().y, 180.0);
        *app.world_mut().resource_mut::<HelpPanelState>() = HelpPanelState::default();
        app.update();
        app.world_mut().get_mut::<ScrollPosition>(scroll).unwrap().0 = Vec2::ZERO;
        app.world_mut()
            .resource_mut::<HelpPanelState>()
            .open_at(FIRST);
        app.update();
        assert_eq!(app.world().get::<ScrollPosition>(scroll).unwrap().y, 0.0);
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
