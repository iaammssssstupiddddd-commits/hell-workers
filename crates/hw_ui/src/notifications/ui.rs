use super::model::{
    NotificationCenter, NotificationEntry, NotificationHistoryButton, NotificationHistoryClose,
    NotificationHistoryKey, NotificationHistoryPanel, NotificationHistoryRow,
    NotificationHistoryScroll, NotificationSeverity, NotificationToastRoot, NotificationToastRow,
    NotificationToastSurface, NotificationUiAssets, NotificationUiRuntime, NotificationUnreadText,
};
use crate::components::UiInputBlocker;
use crate::theme::{UiTheme, font_size_rem};
use bevy::ecs::system::SystemParam;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::ui::{FocusPolicy, RelativeCursorPosition};
use bevy::ui_widgets::{ControlOrientation, ScrollArea, Scrollbar, ScrollbarThumb};

#[derive(Component)]
struct NotificationAgeText(super::NotificationEntryId);

fn elapsed_label(now: std::time::Duration, then: std::time::Duration) -> String {
    let seconds = now.saturating_sub(then).as_secs();
    if seconds < 60 {
        format!("{seconds}秒前")
    } else if seconds < 3600 {
        format!("{}分前", seconds / 60)
    } else {
        format!("{}時間前", seconds / 3600)
    }
}

type HistoryScrollQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut ScrollPosition,
        &'static ComputedNode,
        &'static UiGlobalTransform,
    ),
    With<NotificationHistoryScroll>,
>;
type HistoryPositionQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static NotificationHistoryKey,
        &'static ComputedNode,
        &'static UiGlobalTransform,
    ),
>;

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
    unread_text: Query<
        'w,
        's,
        &'static mut Text,
        (With<NotificationUnreadText>, Without<NotificationAgeText>),
    >,
    age_text: Query<
        'w,
        's,
        (&'static NotificationAgeText, &'static mut Text),
        Without<NotificationUnreadText>,
    >,
    toast_rows: Query<'w, 's, Entity, With<NotificationToastRow>>,
    history_rows: Query<'w, 's, Entity, With<NotificationHistoryRow>>,
    history_scroll: HistoryScrollQuery<'w, 's>,
    history_positions: HistoryPositionQuery<'w, 's>,
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
                top: Val::Percent(30.0),
                width: Val::Px(420.0),
                max_height: Val::Percent(60.0),
                max_width: Val::Percent(92.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
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
                font: font.clone().into(),
                font_size: font_size_rem(theme.typography.font_size_md),
                ..default()
            },
            TextColor(theme.colors.text_accent_semantic),
            Node {
                margin: UiRect::bottom(Val::Px(4.0)),
                flex_shrink: 0.0,
                ..default()
            },
        ));
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(0.0),
                flex_grow: 1.0,
                ..default()
            })
            .with_children(|row| {
                let scroll = row
                    .spawn((
                        Node {
                            flex_grow: 1.0,
                            min_width: Val::Px(0.0),
                            min_height: Val::Px(0.0),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(6.0),
                            overflow: Overflow::scroll_y(),
                            ..default()
                        },
                        ScrollArea,
                        NotificationHistoryScroll,
                        UiInputBlocker,
                        RelativeCursorPosition::default(),
                    ))
                    .id();
                row.spawn((
                    Node {
                        width: Val::Px(6.0),
                        ..default()
                    },
                    Scrollbar::new(scroll, ControlOrientation::Vertical, 20.0),
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
            });
        panel
            .spawn((
                Button,
                NotificationHistoryClose,
                Node {
                    height: Val::Px(32.0),
                    flex_shrink: 0.0,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(theme.colors.button_default),
            ))
            .with_children(|button| {
                button.spawn((
                    Text::new("閉じる"),
                    TextFont {
                        font: font.clone().into(),
                        font_size: font_size_rem(theme.typography.font_size_sm),
                        ..default()
                    },
                    TextColor(theme.colors.text_primary_semantic),
                ));
            });
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
    real_time: Res<Time<Real>>,
) {
    let now = real_time.elapsed();
    if runtime.age_second != Some(now.as_secs()) {
        for (marker, mut text) in &mut queries.age_text {
            if let Some(entry) = center.entry(marker.0) {
                text.0 = elapsed_label(now, entry.last_seen);
            }
        }
        runtime.age_second = Some(now.as_secs());
    }
    if runtime.rendered_revision == Some(center.revision()) {
        return;
    }
    let Some(assets) = assets else {
        return;
    };
    let Ok((toast_root_entity, mut toast_root_node)) = queries.toast_root.single_mut() else {
        return;
    };
    let Ok((_, mut history_panel_node)) = queries.history_panel.single_mut() else {
        return;
    };
    let Ok((history_body, mut scroll, computed, transform)) = queries.history_scroll.single_mut()
    else {
        return;
    };
    if center.history_open() && !runtime.history_open {
        scroll.0 = Vec2::ZERO;
        runtime.scroll_anchor = None;
    } else if center.history_open() {
        let top = transform.translation.y - computed.size().y * 0.5;
        runtime.scroll_anchor = queries
            .history_positions
            .iter()
            .filter_map(|(key, node, position)| {
                let row_top = position.translation.y - node.size().y * 0.5;
                let row_bottom = position.translation.y + node.size().y * 0.5;
                (row_bottom > top).then_some((key.0, row_top - top))
            })
            .min_by(|left, right| left.1.total_cmp(&right.1));
    } else {
        runtime.scroll_anchor = None;
    }
    runtime.history_open = center.history_open();

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
        spawn_history_empty_row(&mut commands, history_body, &assets.font, &theme);
    } else {
        for entry in center.history_entries().rev() {
            spawn_history_row(
                &mut commands,
                history_body,
                entry,
                &assets.font,
                &theme,
                now,
            );
        }
    }

    runtime.rendered_revision = Some(center.revision());
}

/// Preserve the first visible entry across insertion and variable-height row updates.
/// Layout has to resolve the new rows before their offset can be compared.
pub fn restore_notification_scroll_anchor_system(
    mut runtime: ResMut<NotificationUiRuntime>,
    mut scrolls: HistoryScrollQuery,
    rows: HistoryPositionQuery,
) {
    let Some((id, old_offset)) = runtime.scroll_anchor.take() else {
        return;
    };
    let Ok((_, mut scroll, computed, transform)) = scrolls.single_mut() else {
        return;
    };
    let delta = rows
        .iter()
        .find(|(key, _, _)| key.0 == id)
        .map_or(0.0, |(_, row, position)| {
            let top = transform.translation.y - computed.size().y * 0.5;
            let offset = position.translation.y - row.size().y * 0.5 - top;
            (offset - old_offset) * computed.inverse_scale_factor()
        });
    let max_scroll =
        (computed.content_size().y - computed.size().y).max(0.0) * computed.inverse_scale_factor();
    scroll.0.y = (scroll.0.y + delta).clamp(0.0, max_scroll);
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
    now: std::time::Duration,
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
            NotificationHistoryKey(entry.id),
            Name::new("Notification History Row"),
        ))
        .id();
    commands.entity(row).with_children(|row| {
        row.spawn((
            NotificationAgeText(entry.id),
            Text::new(elapsed_label(now, entry.last_seen)),
            TextFont {
                font: font.clone().into(),
                font_size: font_size_rem(theme.typography.font_size_xs),
                ..default()
            },
            TextColor(theme.colors.text_secondary_semantic),
        ));
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
        if let Some(super::NotificationAction::RetrySettingsSave { attempt }) = entry.action {
            row.spawn((
                Button,
                crate::components::MenuButton(crate::UiIntent::RetrySettingsSave { attempt }),
                Node {
                    margin: UiRect::top(Val::Px(6.0)),
                    padding: UiRect::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(theme.colors.button_default),
            ))
            .with_children(|button| {
                button.spawn((
                    Text::new("現在の設定を再保存"),
                    TextFont {
                        font: font.clone().into(),
                        font_size: font_size_rem(theme.typography.font_size_sm),
                        ..default()
                    },
                    TextColor(theme.colors.text_primary_semantic),
                ));
            });
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

    #[test]
    fn history_elapsed_time_updates_without_rebuilding_or_scrolling() {
        let mut app = App::new();
        app.init_resource::<Time<Real>>()
            .init_resource::<UiTheme>()
            .init_resource::<NotificationCenter>()
            .init_resource::<NotificationUiRuntime>()
            .add_systems(Startup, setup_notification_test_ui)
            .add_systems(Update, present_notifications_system);
        app.update();
        app.world_mut()
            .resource_mut::<NotificationCenter>()
            .toggle_history();
        app.update();
        let age = app
            .world_mut()
            .query_filtered::<Entity, With<NotificationAgeText>>()
            .single(app.world())
            .unwrap();
        let scroll = app
            .world_mut()
            .query_filtered::<Entity, With<NotificationHistoryScroll>>()
            .single(app.world())
            .unwrap();
        app.world_mut()
            .get_mut::<ScrollPosition>(scroll)
            .unwrap()
            .0
            .y = 35.0;
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(std::time::Duration::from_secs(65));
        app.update();
        assert_eq!(app.world().get::<Text>(age).unwrap().0, "1分前");
        assert_eq!(app.world().get::<ScrollPosition>(scroll).unwrap().0.y, 35.0);
    }

    #[test]
    fn retry_action_is_rendered_only_in_history() {
        let mut world = World::new();
        let toast = world.spawn_empty().id();
        let history = world.spawn_empty().id();
        let mut center = NotificationCenter::default();
        center.push(
            super::super::UserFacingNotification::new(
                "save",
                NotificationSeverity::Error,
                "failed",
                "retry",
                super::super::NotificationRetention::Important,
            )
            .with_action(super::super::NotificationAction::RetrySettingsSave { attempt: 7 }),
            std::time::Duration::ZERO,
        );
        let entry = center.history_entries().next().unwrap().clone();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, &world);
            spawn_toast_row(
                &mut commands,
                toast,
                &entry,
                &Handle::default(),
                &UiTheme::default(),
            );
            spawn_history_row(
                &mut commands,
                history,
                &entry,
                &Handle::default(),
                &UiTheme::default(),
                std::time::Duration::ZERO,
            );
        }
        queue.apply(&mut world);
        let mut buttons = world.query::<(Entity, &crate::components::MenuButton)>();
        let (button, action) = buttons.single(&world).unwrap();
        assert!(matches!(
            action.0,
            crate::UiIntent::RetrySettingsSave { attempt: 7 }
        ));
        let row = world.get::<ChildOf>(button).unwrap().parent();
        assert!(world.get::<NotificationHistoryRow>(row).is_some());
        assert_eq!(world.get::<ChildOf>(row).unwrap().parent(), history);
    }

    #[test]
    fn history_anchor_preserves_offset_and_clamps_when_entry_was_evicted() {
        let mut app = App::new();
        app.init_resource::<NotificationUiRuntime>()
            .add_systems(Update, restore_notification_scroll_anchor_system);
        let scroll = app
            .world_mut()
            .spawn((
                NotificationHistoryScroll,
                ScrollPosition(Vec2::new(0.0, 20.0)),
                ComputedNode {
                    size: Vec2::splat(100.0),
                    content_size: Vec2::new(100.0, 400.0),
                    inverse_scale_factor: 0.5,
                    ..default()
                },
                UiGlobalTransform::default(),
            ))
            .id();
        let row = app
            .world_mut()
            .spawn((
                NotificationHistoryKey(7),
                ComputedNode {
                    size: Vec2::new(100.0, 20.0),
                    ..default()
                },
                UiGlobalTransform::default(),
            ))
            .id();
        app.world_mut()
            .resource_mut::<NotificationUiRuntime>()
            .scroll_anchor = Some((7, -10.0));
        app.update();
        assert_eq!(app.world().get::<ScrollPosition>(scroll).unwrap().0.y, 45.0);
        assert!(
            app.world()
                .resource::<NotificationUiRuntime>()
                .scroll_anchor
                .is_none()
        );

        app.world_mut().despawn(row);
        app.world_mut()
            .get_mut::<ScrollPosition>(scroll)
            .unwrap()
            .0
            .y = 250.0;
        app.world_mut()
            .resource_mut::<NotificationUiRuntime>()
            .scroll_anchor = Some((7, -10.0));
        app.update();
        assert_eq!(
            app.world().get::<ScrollPosition>(scroll).unwrap().0.y,
            150.0
        );
    }
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
