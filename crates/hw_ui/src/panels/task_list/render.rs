use super::types::{
    PendingTaskCancellation, TaskActionButton, TaskActionButtonKind, TaskDashboardActionState,
    TaskDashboardControl, TaskDashboardViewState, TaskEntry, TaskListDynamicNode,
    TaskPriorityAdjustment, TaskPriorityFilter, TaskPriorityTier, TaskSortDirection, TaskSortKey,
    TaskStatusFilter, TaskStatusSummary, TaskWorkTypeFilter, TaskWorkerFilter,
};
use super::work_type_icon::{work_type_icon, work_type_label};
use crate::components::TaskListItem;
use crate::setup::UiAssets;
use crate::theme::UiTheme;
use bevy::prelude::*;
use hw_core::jobs::WorkType;

pub fn rebuild_task_list_ui(
    parent: &mut ChildSpawnerCommands,
    snapshot: &[TaskEntry],
    view_state: &TaskDashboardViewState,
    pinned_entity: Option<Entity>,
    action_state: &TaskDashboardActionState,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
) {
    spawn_toolbar(parent, view_state, game_assets, theme);

    let visible = view_state.visible_entries(snapshot);
    if visible.is_empty() {
        parent.spawn((
            TaskListDynamicNode,
            Text::new(if snapshot.is_empty() {
                "No designations"
            } else {
                "No matching designations"
            }),
            TextFont {
                font: game_assets.font_ui().clone().into(),
                font_size: crate::theme::font_size_rem(theme.typography.font_size_small),
                ..default()
            },
            TextColor(theme.colors.empty_text),
        ));
        return;
    }

    let grouped = view_state.sort_key == TaskSortKey::WorkType;
    let mut previous_work_type = None;
    for entry in visible.iter().copied() {
        if grouped && previous_work_type != Some(entry.work_type) {
            let count = visible
                .iter()
                .filter(|candidate| candidate.work_type == entry.work_type)
                .count();
            spawn_group_header(parent, entry.work_type, count, game_assets, theme);
            previous_work_type = Some(entry.work_type);
        }
        spawn_task_row(
            parent,
            entry,
            pinned_entity == Some(entry.entity),
            action_state,
            game_assets,
            theme,
        );
    }
}

fn spawn_toolbar(
    parent: &mut ChildSpawnerCommands,
    state: &TaskDashboardViewState,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
) {
    parent
        .spawn((
            TaskListDynamicNode,
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(3.0),
                row_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(3.0)),
                ..default()
            },
        ))
        .with_children(|toolbar| {
            spawn_control(
                toolbar,
                TaskDashboardControl::WorkTypeFilter,
                &format!("Type: {}", work_type_filter_label(state.work_type)),
                game_assets,
                theme,
            );
            spawn_control(
                toolbar,
                TaskDashboardControl::StatusFilter,
                &format!("State: {}", status_filter_label(state.status)),
                game_assets,
                theme,
            );
            spawn_control(
                toolbar,
                TaskDashboardControl::PriorityFilter,
                &format!("Priority: {}", priority_filter_label(state.priority)),
                game_assets,
                theme,
            );
            spawn_control(
                toolbar,
                TaskDashboardControl::WorkerFilter,
                &format!("Workers: {}", worker_filter_label(state.workers)),
                game_assets,
                theme,
            );
            spawn_control(
                toolbar,
                TaskDashboardControl::SortKey,
                &format!("Sort: {}", sort_key_label(state.sort_key)),
                game_assets,
                theme,
            );
            spawn_control(
                toolbar,
                TaskDashboardControl::SortDirection,
                match state.direction {
                    TaskSortDirection::Ascending => "Order: Asc",
                    TaskSortDirection::Descending => "Order: Desc",
                },
                game_assets,
                theme,
            );
        });
}

fn spawn_control(
    parent: &mut ChildSpawnerCommands,
    control: TaskDashboardControl,
    label: &str,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
) {
    parent
        .spawn((
            Button,
            control,
            Node {
                padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(theme.colors.button_default),
        ))
        .with_child((
            Text::new(label),
            TextFont {
                font: game_assets.font_ui().clone().into(),
                font_size: crate::theme::font_size_rem(theme.typography.font_size_xs),
                ..default()
            },
            TextColor(theme.colors.text_secondary),
        ));
}

fn spawn_group_header(
    parent: &mut ChildSpawnerCommands,
    work_type: WorkType,
    count: usize,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
) {
    let (header_icon, header_color) = work_type_icon(&work_type, game_assets, theme);
    parent
        .spawn((
            TaskListDynamicNode,
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                margin: UiRect {
                    top: Val::Px(4.0),
                    bottom: Val::Px(2.0),
                    ..default()
                },
                padding: UiRect::horizontal(Val::Px(6.0)),
                column_gap: Val::Px(4.0),
                ..default()
            },
        ))
        .with_children(|row| {
            row.spawn((
                ImageNode {
                    image: header_icon,
                    color: header_color,
                    ..default()
                },
                Node {
                    width: Val::Px(theme.sizes.icon_size),
                    height: Val::Px(theme.sizes.icon_size),
                    ..default()
                },
            ));
            row.spawn((
                Text::new(format!("{} ({count})", work_type_label(&work_type))),
                TextFont {
                    font: game_assets.font_ui().clone().into(),
                    font_size: crate::theme::font_size_rem(theme.typography.font_size_xs),
                    weight: FontWeight::SEMIBOLD,
                    ..default()
                },
                TextColor(theme.colors.text_secondary_semantic),
            ));
        });
}

fn spawn_task_row(
    parent: &mut ChildSpawnerCommands,
    entry: &TaskEntry,
    is_pinned: bool,
    action_state: &TaskDashboardActionState,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
) {
    let (item_icon, item_color) = work_type_icon(&entry.work_type, game_assets, theme);
    let desc_color = match entry.priority_tier() {
        TaskPriorityTier::Normal => theme.colors.text_primary,
        TaskPriorityTier::High => theme.colors.accent_ember,
        TaskPriorityTier::Critical => theme.colors.status_danger,
    };
    let status_color = task_status_color(entry.status, theme);

    parent
        .spawn((
            TaskListDynamicNode,
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
        ))
        .with_children(|wrapper| {
            wrapper
                .spawn((
                    Button,
                    Node {
                        width: Val::Percent(100.0),
                        min_height: Val::Px(theme.sizes.soul_item_height),
                        flex_shrink: 0.0,
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(4.0),
                        border: UiRect::left(Val::Px(0.0)),
                        ..default()
                    },
                    BorderColor::all(Color::NONE),
                    BackgroundColor(theme.colors.list_item_default),
                    TaskListItem(entry.entity),
                ))
                .with_children(|button| {
                    button.spawn((
                        ImageNode {
                            image: item_icon,
                            color: item_color,
                            ..default()
                        },
                        Node {
                            width: Val::Px(theme.sizes.icon_size),
                            height: Val::Px(theme.sizes.icon_size),
                            ..default()
                        },
                    ));
                    button
                        .spawn(Node {
                            flex_grow: 1.0,
                            flex_direction: FlexDirection::Column,
                            ..default()
                        })
                        .with_children(|text_column| {
                            text_column.spawn((
                                Text::new(&entry.description),
                                TextFont {
                                    font: game_assets.font_ui().clone().into(),
                                    font_size: crate::theme::font_size_rem(
                                        theme.typography.font_size_item,
                                    ),
                                    ..default()
                                },
                                TextColor(desc_color),
                            ));
                            text_column.spawn((
                                Text::new(entry.status.label()),
                                TextFont {
                                    font: game_assets.font_ui().clone().into(),
                                    font_size: crate::theme::font_size_rem(
                                        theme.typography.font_size_xs,
                                    ),
                                    ..default()
                                },
                                TextColor(status_color),
                            ));
                        });
                    if entry.worker_count > 0 {
                        button.spawn((
                            Text::new(format!("\u{00d7}{}", entry.worker_count)),
                            TextFont {
                                font: game_assets.font_ui().clone().into(),
                                font_size: crate::theme::font_size_rem(
                                    theme.typography.font_size_small,
                                ),
                                ..default()
                            },
                            TextColor(theme.colors.text_secondary),
                        ));
                    }
                });

            if is_pinned && entry.actions.has_actions() {
                spawn_action_bar(wrapper, entry, action_state, game_assets, theme);
            }
        });
}

fn task_status_color(status: TaskStatusSummary, theme: &UiTheme) -> Color {
    match status {
        TaskStatusSummary::Working => theme.colors.status_healthy,
        TaskStatusSummary::Blocked(_) => theme.colors.status_warning,
        TaskStatusSummary::PendingEvaluation => theme.colors.status_info,
    }
}

fn spawn_action_bar(
    parent: &mut ChildSpawnerCommands,
    entry: &TaskEntry,
    action_state: &TaskDashboardActionState,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::FlexEnd,
            column_gap: Val::Px(3.0),
            padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
            ..default()
        })
        .with_children(|bar| {
            if entry.actions.priority {
                spawn_action_button(
                    bar,
                    TaskActionButton {
                        target: entry.entity,
                        expected_work_type: entry.work_type,
                        kind: TaskActionButtonKind::AdjustPriority(
                            TaskPriorityAdjustment::Decrease,
                        ),
                    },
                    "Priority -",
                    theme.colors.button_default,
                    game_assets,
                    theme,
                );
                spawn_action_button(
                    bar,
                    TaskActionButton {
                        target: entry.entity,
                        expected_work_type: entry.work_type,
                        kind: TaskActionButtonKind::AdjustPriority(
                            TaskPriorityAdjustment::Increase,
                        ),
                    },
                    "Priority +",
                    theme.colors.button_default,
                    game_assets,
                    theme,
                );
            }
            if let Some(kind) = entry.actions.cancel {
                let pending = PendingTaskCancellation {
                    target: entry.entity,
                    expected_work_type: entry.work_type,
                    kind,
                };
                let label = if action_state.confirmation == Some(pending) {
                    match kind {
                        super::types::TaskCancelKind::FloorSite(_)
                        | super::types::TaskCancelKind::WallSite(_) => "Confirm cancel site",
                        _ => "Confirm cancel",
                    }
                } else {
                    kind.label()
                };
                spawn_action_button(
                    bar,
                    TaskActionButton {
                        target: entry.entity,
                        expected_work_type: entry.work_type,
                        kind: TaskActionButtonKind::Cancel(kind),
                    },
                    label,
                    theme.colors.status_danger,
                    game_assets,
                    theme,
                );
            }
        });
}

fn spawn_action_button(
    parent: &mut ChildSpawnerCommands,
    action: TaskActionButton,
    label: &str,
    background: Color,
    game_assets: &dyn UiAssets,
    theme: &UiTheme,
) {
    parent
        .spawn((
            Button,
            action,
            Node {
                padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(background),
        ))
        .with_child((
            Text::new(label),
            TextFont {
                font: game_assets.font_ui().clone().into(),
                font_size: crate::theme::font_size_rem(theme.typography.font_size_xs),
                ..default()
            },
            TextColor(theme.colors.text_primary),
        ));
}

fn work_type_filter_label(filter: TaskWorkTypeFilter) -> &'static str {
    match filter {
        TaskWorkTypeFilter::All => "All",
        TaskWorkTypeFilter::Only(work_type) => work_type_label(&work_type),
    }
}

const fn status_filter_label(filter: TaskStatusFilter) -> &'static str {
    match filter {
        TaskStatusFilter::All => "All",
        TaskStatusFilter::Working => "Working",
        TaskStatusFilter::Blocked => "Blocked",
        TaskStatusFilter::Pending => "Pending",
    }
}

const fn priority_filter_label(filter: TaskPriorityFilter) -> &'static str {
    match filter {
        TaskPriorityFilter::All => "All",
        TaskPriorityFilter::Normal => "Normal",
        TaskPriorityFilter::High => "High",
        TaskPriorityFilter::Critical => "Critical",
    }
}

const fn worker_filter_label(filter: TaskWorkerFilter) -> &'static str {
    match filter {
        TaskWorkerFilter::All => "All",
        TaskWorkerFilter::Assigned => "Assigned",
        TaskWorkerFilter::Unassigned => "Unassigned",
    }
}

const fn sort_key_label(key: TaskSortKey) -> &'static str {
    match key {
        TaskSortKey::WorkType => "Type",
        TaskSortKey::Status => "State",
        TaskSortKey::Priority => "Priority",
        TaskSortKey::WorkerCount => "Workers",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_statuses_use_distinct_semantic_theme_colors() {
        let theme = UiTheme::default();
        let working = task_status_color(TaskStatusSummary::Working, &theme);
        let blocked = task_status_color(
            TaskStatusSummary::Blocked(super::super::types::TaskBlockerReason::Unreachable),
            &theme,
        );
        let pending = task_status_color(TaskStatusSummary::PendingEvaluation, &theme);

        assert_eq!(working, theme.colors.status_healthy);
        assert_eq!(blocked, theme.colors.status_warning);
        assert_eq!(pending, theme.colors.status_info);
        assert_ne!(working, blocked);
        assert_ne!(working, pending);
        assert_ne!(blocked, pending);
    }
}
