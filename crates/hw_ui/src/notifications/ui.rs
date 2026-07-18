use super::model::{
    NotificationCenter, NotificationEntry, NotificationHistoryButton, NotificationHistoryPanel,
    NotificationHistoryRow, NotificationSeverity, NotificationToastRoot, NotificationToastRow,
    NotificationToastSurface, NotificationUiAssets, NotificationUiRuntime, NotificationUnreadText,
};
use crate::components::UiInputBlocker;
use crate::theme::{UiTheme, font_size_rem};
use bevy::ecs::system::SystemParam;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui::{FocusPolicy, RelativeCursorPosition};

type ToastRootQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static mut Node),
    (
        With<NotificationToastRoot>,
        Without<NotificationHistoryPanel>,
    ),
>;
type HistoryPanelQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static mut Node),
    (
        With<NotificationHistoryPanel>,
        Without<NotificationToastRoot>,
    ),
>;

#[derive(SystemParam)]
pub struct NotificationUiQueries<'w, 's> {
    toast_root: ToastRootQuery<'w, 's>,
    history_panel: HistoryPanelQuery<'w, 's>,
    unread_text: Query<'w, 's, &'static mut Text, With<NotificationUnreadText>>,
    toast_rows: Query<'w, 's, Entity, With<NotificationToastRow>>,
    history_rows: Query<'w, 's, Entity, With<NotificationHistoryRow>>,
}

pub(crate) fn spawn_notification_ui(
    commands: &mut Commands,
    font: Handle<Font>,
    theme: &UiTheme,
    top_right_parent: Entity,
    overlay_parent: Entity,
) {
    commands.insert_resource(NotificationUiAssets { font: font.clone() });

    let toast_root = commands
        .spawn((
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                right: Val::Px(24.0),
                top: Val::Px(24.0),
                width: Val::Px(380.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                row_gap: Val::Px(8.0),
                ..default()
            },
            ZIndex(45),
            Pickable::IGNORE,
            FocusPolicy::Pass,
            NotificationToastSurface,
            NotificationToastRoot,
            Name::new("Notification Toast Stack"),
        ))
        .id();
    commands.entity(overlay_parent).add_child(toast_root);

    let history_button = commands
        .spawn((
            Button,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(theme.spacing.panel_margin_x),
                top: Val::Px(theme.sizes.time_control_top + 170.0),
                min_width: Val::Px(104.0),
                height: Val::Px(30.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(theme.colors.button_default),
            BorderColor::all(theme.colors.border_default),
            RelativeCursorPosition::default(),
            UiInputBlocker,
            NotificationHistoryButton,
            Name::new("Notification History Button"),
        ))
        .id();
    commands.entity(history_button).with_children(|button| {
        button.spawn((
            Text::new("通知"),
            TextFont {
                font: font.clone().into(),
                font_size: font_size_rem(theme.typography.font_size_sm),
                ..default()
            },
            TextColor(theme.colors.text_primary_semantic),
            NotificationUnreadText,
        ));
    });
    commands.entity(top_right_parent).add_child(history_button);

    let history_panel = commands
        .spawn((
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                right: Val::Px(theme.spacing.panel_margin_x),
                top: Val::Px(theme.sizes.time_control_top + 206.0),
                width: Val::Px(420.0),
                max_height: Val::Px(520.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(theme.colors.bg_overlay),
            BorderColor::all(theme.colors.border_accent),
            RelativeCursorPosition::default(),
            UiInputBlocker,
            FocusPolicy::Block,
            ZIndex(46),
            NotificationHistoryPanel,
            Name::new("Notification History Panel"),
        ))
        .id();
    commands.entity(history_panel).with_children(|panel| {
        panel.spawn((
            Text::new("重要な通知"),
            TextFont {
                font: font.into(),
                font_size: font_size_rem(theme.typography.font_size_md),
                ..default()
            },
            TextColor(theme.colors.text_accent_semantic),
            Node {
                margin: UiRect::bottom(Val::Px(4.0)),
                ..default()
            },
        ));
    });
    commands.entity(overlay_parent).add_child(history_panel);
}

pub fn present_notifications_system(
    mut commands: Commands,
    center: Res<NotificationCenter>,
    assets: Option<Res<NotificationUiAssets>>,
    theme: Res<UiTheme>,
    mut runtime: ResMut<NotificationUiRuntime>,
    mut queries: NotificationUiQueries,
) {
    if runtime.rendered_revision == Some(center.revision()) {
        return;
    }
    let Some(assets) = assets else {
        return;
    };
    let Ok((toast_root_entity, mut toast_root_node)) = queries.toast_root.single_mut() else {
        return;
    };
    let Ok((history_panel_entity, mut history_panel_node)) = queries.history_panel.single_mut()
    else {
        return;
    };

    for entity in &queries.toast_rows {
        commands.entity(entity).despawn();
    }
    for entity in &queries.history_rows {
        commands.entity(entity).despawn();
    }

    toast_root_node.display = if center.toast_count() == 0 {
        Display::None
    } else {
        Display::Flex
    };
    history_panel_node.display = if center.history_open() {
        Display::Flex
    } else {
        Display::None
    };
    if let Ok(mut text) = queries.unread_text.single_mut() {
        text.0 = if center.unread_count() == 0 {
            "通知".to_string()
        } else {
            format!("通知 ({})", center.unread_count())
        };
    }

    for entry in center.toast_entries().rev() {
        spawn_toast_row(
            &mut commands,
            toast_root_entity,
            entry,
            &assets.font,
            &theme,
        );
    }
    if center.history_count() == 0 {
        spawn_history_empty_row(&mut commands, history_panel_entity, &assets.font, &theme);
    } else {
        for entry in center.history_entries().rev() {
            spawn_history_row(
                &mut commands,
                history_panel_entity,
                entry,
                &assets.font,
                &theme,
            );
        }
    }

    runtime.rendered_revision = Some(center.revision());
}

fn spawn_toast_row(
    commands: &mut Commands,
    parent: Entity,
    entry: &NotificationEntry,
    font: &Handle<Font>,
    theme: &UiTheme,
) {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::left(Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(theme.colors.bg_overlay),
            BorderColor::all(severity_color(entry.severity, theme)),
            Pickable::IGNORE,
            FocusPolicy::Pass,
            NotificationToastSurface,
            NotificationToastRow,
            Name::new("Notification Toast"),
        ))
        .id();
    commands.entity(row).with_children(|row| {
        row.spawn((
            Text::new(entry_title(entry)),
            TextFont {
                font: font.clone().into(),
                font_size: font_size_rem(theme.typography.font_size_md),
                ..default()
            },
            TextColor(severity_color(entry.severity, theme)),
            Pickable::IGNORE,
            FocusPolicy::Pass,
            NotificationToastSurface,
        ));
        if !entry.body.is_empty() {
            row.spawn((
                Text::new(entry.body.clone()),
                TextFont {
                    font: font.clone().into(),
                    font_size: font_size_rem(theme.typography.font_size_sm),
                    ..default()
                },
                TextColor(theme.colors.text_primary_semantic),
                Node {
                    margin: UiRect::top(Val::Px(2.0)),
                    ..default()
                },
                Pickable::IGNORE,
                FocusPolicy::Pass,
                NotificationToastSurface,
            ));
        }
    });
    commands.entity(parent).add_child(row);
}

fn spawn_history_row(
    commands: &mut Commands,
    parent: Entity,
    entry: &NotificationEntry,
    font: &Handle<Font>,
    theme: &UiTheme,
) {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::left(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(theme.colors.bg_elevated),
            BorderColor::all(severity_color(entry.severity, theme)),
            NotificationHistoryRow,
            Name::new("Notification History Row"),
        ))
        .id();
    commands.entity(row).with_children(|row| {
        row.spawn((
            Text::new(entry_title(entry)),
            TextFont {
                font: font.clone().into(),
                font_size: font_size_rem(theme.typography.font_size_sm),
                ..default()
            },
            TextColor(severity_color(entry.severity, theme)),
        ));
        if !entry.body.is_empty() {
            row.spawn((
                Text::new(entry.body.clone()),
                TextFont {
                    font: font.clone().into(),
                    font_size: font_size_rem(theme.typography.font_size_xs),
                    ..default()
                },
                TextColor(theme.colors.text_secondary_semantic),
            ));
        }
    });
    commands.entity(parent).add_child(row);
}

fn spawn_history_empty_row(
    commands: &mut Commands,
    parent: Entity,
    font: &Handle<Font>,
    theme: &UiTheme,
) {
    let row = commands
        .spawn((
            Text::new("重要な通知はありません"),
            TextFont {
                font: font.clone().into(),
                font_size: font_size_rem(theme.typography.font_size_sm),
                ..default()
            },
            TextColor(theme.colors.text_secondary_semantic),
            NotificationHistoryRow,
        ))
        .id();
    commands.entity(parent).add_child(row);
}

fn entry_title(entry: &NotificationEntry) -> String {
    if entry.repeat_count > 1 {
        format!("{} (x{})", entry.title, entry.repeat_count)
    } else {
        entry.title.clone()
    }
}

fn severity_color(severity: NotificationSeverity, theme: &UiTheme) -> Color {
    match severity {
        NotificationSeverity::Info => theme.colors.status_info,
        NotificationSeverity::Success => theme.colors.status_healthy,
        NotificationSeverity::Warning => theme.colors.status_warning,
        NotificationSeverity::Error => theme.colors.status_danger,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::{
        NotificationRetention, NotificationSeverity, UserFacingNotification,
    };

    fn setup_notification_test_ui(
        mut commands: Commands,
        theme: Res<UiTheme>,
        mut center: ResMut<NotificationCenter>,
    ) {
        let top_right = commands.spawn_empty().id();
        let overlay = commands.spawn_empty().id();
        spawn_notification_ui(&mut commands, Handle::default(), &theme, top_right, overlay);
        center.push(
            UserFacingNotification::new(
                "test",
                NotificationSeverity::Info,
                "Test",
                "Body",
                NotificationRetention::Important,
            ),
            std::time::Duration::ZERO,
        );
    }

    #[test]
    fn toast_descendants_are_pick_through_and_unchanged_frames_keep_rows() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<UiTheme>()
            .init_resource::<NotificationCenter>()
            .init_resource::<NotificationUiRuntime>()
            .add_systems(Startup, setup_notification_test_ui)
            .add_systems(Update, present_notifications_system);

        app.update();

        let mut surfaces = app.world_mut().query::<(
            Entity,
            &Pickable,
            &FocusPolicy,
            Option<&NotificationToastRow>,
        )>();
        let surface_entities: Vec<_> = surfaces
            .iter(app.world())
            .map(|(entity, pickable, focus, _)| {
                assert_eq!(*pickable, Pickable::IGNORE);
                assert_eq!(*focus, FocusPolicy::Pass);
                entity
            })
            .collect();
        assert!(surface_entities.len() >= 3);

        let row_before = app
            .world_mut()
            .query_filtered::<Entity, With<NotificationToastRow>>()
            .single(app.world())
            .unwrap();
        app.update();
        let row_after = app
            .world_mut()
            .query_filtered::<Entity, With<NotificationToastRow>>()
            .single(app.world())
            .unwrap();
        assert_eq!(row_before, row_after);
    }
}
