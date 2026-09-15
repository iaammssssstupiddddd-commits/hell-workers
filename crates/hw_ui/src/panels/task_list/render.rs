use super::types::{
    PendingTaskCancellation, TaskActionButton, TaskActionButtonKind, TaskDashboardActionState,
    TaskDashboardControl, TaskDashboardViewState, TaskEntry, TaskFilterMenu, TaskListDynamicNode,
    TaskPriorityAdjustment, TaskPriorityFilter, TaskPriorityTier, TaskSortDirection, TaskSortKey,
    TaskStatusFilter, TaskStatusSummary, TaskWorkTypeFilter, TaskWorkerFilter,
};
use super::types::{TaskListScroll, task_page_range};
use super::work_type_icon::{work_type_icon, work_type_label};
use crate::components::TaskListItem;
use crate::components::UiInputBlocker;
use crate::setup::UiAssets;
use crate::theme::UiTheme;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::ui_widgets::{ControlOrientation, ScrollArea, Scrollbar, ScrollbarThumb};
use hw_core::jobs::WorkType;

#[cfg(feature = "profiling")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TaskListRenderStats {
    pub input_rows: u32,
    pub visible_rows: u32,
    pub group_headers: u32,
}

#[cfg(feature = "profiling")]
type TaskListRenderResult = TaskListRenderStats;
#[cfg(not(feature = "profiling"))]
type TaskListRenderResult = ();

pub struct TaskListRenderInput<'a> {
    pub snapshot: &'a [TaskEntry],
    pub view_state: &'a TaskDashboardViewState,
    pub action_state: &'a TaskDashboardActionState,
    pub game_assets: &'a dyn UiAssets,
    pub theme: &'a UiTheme,
    pub scroll_position: Vec2,
}

pub fn rebuild_task_list_ui(
    parent: &mut ChildSpawnerCommands,
    input: TaskListRenderInput<'_>,
) -> TaskListRenderResult {
    let TaskListRenderInput {
        snapshot,
        view_state,
        action_state,
        game_assets,
        theme,
        scroll_position,
    } = input;
    spawn_toolbar(parent, view_state, game_assets, theme);

    let visible = view_state.visible_entries(snapshot);
    let range = task_page_range(view_state.page_index, visible.len());
    #[cfg(feature = "profiling")]
    let mut stats = TaskListRenderStats {
        input_rows: u32::try_from(snapshot.len()).unwrap_or(u32::MAX),
        visible_rows: u32::try_from(visible.len()).unwrap_or(u32::MAX),
        group_headers: 0,
    };
    parent
        .spawn((
            TaskListDynamicNode,
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                ..default()
            },
        ))
        .with_children(|row| {
            let scroll = row
                .spawn((
                    TaskListDynamicNode,
                    TaskListScroll,
                    ScrollArea,
                    // Preserve the previous offset when ordinary progress rebuilds the rows.
                    ScrollPosition(scroll_position),
                    UiInputBlocker,
                    RelativeCursorPosition::default(),
                    Node {
                        flex_grow: 1.0,
                        min_width: Val::Px(0.0),
                        min_height: Val::Px(0.0),
                        flex_direction: FlexDirection::Column,
                        overflow: Overflow::scroll_y(),
                        ..default()
                    },
                ))
                .with_children(|parent| {
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
                                font_size: crate::theme::font_size_rem(
                                    theme.typography.font_size_small,
                                ),
                                ..default()
                            },
                            TextColor(theme.colors.empty_text),
                        ));
                        return;
                    }

                    let grouped = view_state.sort_key == TaskSortKey::WorkType;
                    let mut previous_work_type = None;
                    for entry in visible[range.clone()].iter().copied() {
                        if grouped && previous_work_type != Some(entry.work_type) {
                            let count = visible
                                .iter()
                                .filter(|candidate| candidate.work_type == entry.work_type)
                                .count();
                            spawn_group_header(parent, entry.work_type, count, game_assets, theme);
                            #[cfg(feature = "profiling")]
                            {
                                stats.group_headers = stats.group_headers.saturating_add(1);
                            }
                            previous_work_type = Some(entry.work_type);
                        }
                        spawn_task_row(
                            parent,
                            entry,
                            action_state.active_task == Some(entry.entity),
                            action_state,
                            game_assets,
                            theme,
                        );
                    }
                })
                .id();
            row.spawn((
                TaskListDynamicNode,
                Node {
                    width: Val::Px(6.0),
                    ..default()
                },
                Scrollbar::new(scroll, ControlOrientation::Vertical, 20.0),
            ))
            .with_children(|bar| {
                bar.spawn((
                    TaskListDynamicNode,
                    ScrollbarThumb {
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        border: UiRect::ZERO,
                    },
                    BackgroundColor(theme.colors.text_muted),
                ));
            });
        });
    parent
        .spawn((
            TaskListDynamicNode,
            Node {
                width: Val::Percent(100.0),
                flex_shrink: 0.0,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(3.0),
                row_gap: Val::Px(3.0),
                ..default()
            },
        ))
        .with_children(|footer| {
            for (control, label) in [
                (TaskDashboardControl::FirstPage, "|<"),
                (TaskDashboardControl::PreviousPage, "<"),
                (TaskDashboardControl::NextPage, ">"),
                (TaskDashboardControl::LastPage, ">|"),
            ] {
                spawn_control(footer, control, label, game_assets, theme);
            }
            footer.spawn((
                TaskListDynamicNode,
                Text::new(format!(
                    "{}–{} / {}",
                    if visible.is_empty() {
                        0
                    } else {
                        range.start + 1
                    },
                    range.end,
                    visible.len()
                )),
                TextFont {
                    font: game_assets.font_ui().clone().into(),
                    font_size: crate::theme::font_size_rem(theme.typography.font_size_small),
                    ..default()
                },
                TextColor(theme.colors.text_primary_semantic),
            ));
        });
    #[cfg(feature = "profiling")]
    return stats;
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
                flex_shrink: 0.0,
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
            spawn_control(
                toolbar,
                TaskDashboardControl::ResetFilters,
                "全解除",
                game_assets,
                theme,
            );
        });
    if let Some(menu) = state.filter_menu {
        spawn_filter_choices(parent, menu, game_assets, theme);
    }
}

fn spawn_filter_choices(
    parent: &mut ChildSpawnerCommands,
    menu: TaskFilterMenu,
    assets: &dyn UiAssets,
    theme: &UiTheme,
) {
    let options: Vec<(TaskDashboardControl, String)> = match menu {
        TaskFilterMenu::WorkType => std::iter::once(TaskWorkTypeFilter::All)
            .chain(
                super::work_type_icon::player_reachable_work_types().map(TaskWorkTypeFilter::Only),
            )
            .map(|value| {
                (
                    TaskDashboardControl::SetWorkType(value),
                    work_type_filter_label(value).to_owned(),
                )
            })
            .collect(),
        TaskFilterMenu::Status => [
            TaskStatusFilter::All,
            TaskStatusFilter::Working,
            TaskStatusFilter::Blocked,
            TaskStatusFilter::Pending,
        ]
        .into_iter()
        .map(|value| {
            (
                TaskDashboardControl::SetStatus(value),
                status_filter_label(value).to_owned(),
            )
        })
        .collect(),
        TaskFilterMenu::Priority => [
            TaskPriorityFilter::All,
            TaskPriorityFilter::Normal,
            TaskPriorityFilter::High,
            TaskPriorityFilter::Critical,
        ]
        .into_iter()
        .map(|value| {
            (
                TaskDashboardControl::SetPriority(value),
                priority_filter_label(value).to_owned(),
            )
        })
        .collect(),
        TaskFilterMenu::Workers => [
            TaskWorkerFilter::All,
            TaskWorkerFilter::Assigned,
            TaskWorkerFilter::Unassigned,
        ]
        .into_iter()
        .map(|value| {
            (
                TaskDashboardControl::SetWorkers(value),
                worker_filter_label(value).to_owned(),
            )
        })
        .collect(),
    };
    parent
        .spawn((
            TaskListDynamicNode,
            Node {
                max_height: Val::Px(140.0),
                min_height: Val::Px(0.0),
                flex_shrink: 0.0,
                width: Val::Percent(100.0),
                ..default()
            },
        ))
        .with_children(|row| {
            let scroll = row
                .spawn((
                    ScrollArea,
                    UiInputBlocker,
                    RelativeCursorPosition::default(),
                    Node {
                        flex_grow: 1.0,
                        min_width: Val::Px(0.0),
                        min_height: Val::Px(0.0),
                        flex_direction: FlexDirection::Column,
                        overflow: Overflow::scroll_y(),
                        ..default()
                    },
                    Name::new("Task Filter Choices"),
                ))
                .with_children(|choices| {
                    for (control, label) in options {
                        spawn_control(choices, control, &label, assets, theme);
                    }
                })
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
                flex_shrink: 0.0,
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
    is_active: bool,
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

            if is_active {
                wrapper
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: Val::Px(4.0),
                        ..default()
                    })
                    .with_children(|bar| {
                        for (kind, target, label) in [
                            (
                                super::TaskRelatedKind::Owner,
                                entry.related_owner,
                                "担当の詳細",
                            ),
                            (
                                super::TaskRelatedKind::Anchor,
                                entry.related_anchor,
                                "運搬の関連先",
                            ),
                        ] {
                            if let Some(target) = target {
                                bar.spawn((
                                    Button,
                                    TaskActionButton {
                                        target: entry.entity,
                                        expected_work_type: entry.work_type,
                                        kind: TaskActionButtonKind::InspectRelated { kind, target },
                                    },
                                    Node {
                                        padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                                        ..default()
                                    },
                                    BackgroundColor(theme.colors.button_default),
                                ))
                                .with_children(|button| {
                                    button.spawn((
                                        Text::new(label),
                                        TextFont {
                                            font: game_assets.font_ui().clone().into(),
                                            font_size: FontSize::Px(theme.typography.font_size_xs),
                                            ..default()
                                        },
                                        TextColor(theme.colors.text_primary_semantic),
                                    ));
                                });
                            }
                        }
                        crate::list::spawn::spawn_entity_focus_button(
                            bar,
                            entry.entity,
                            game_assets,
                            theme,
                            Node::default(),
                        );
                        bar.spawn((
                            Button,
                            crate::components::MenuButton(
                                crate::components::MenuAction::InspectEntity(entry.entity),
                            ),
                            Node {
                                padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(theme.colors.button_default),
                        ))
                        .with_children(|button| {
                            button.spawn((
                                Text::new("詳細を固定"),
                                TextFont {
                                    font: game_assets.font_ui().clone().into(),
                                    font_size: FontSize::Px(theme.typography.font_size_xs),
                                    ..default()
                                },
                                TextColor(theme.colors.text_primary_semantic),
                            ));
                        });
                    });
            }
            if is_active && entry.actions.has_actions() {
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
                        | super::types::TaskCancelKind::WallSite(_)
                        | super::types::TaskCancelKind::SoulSpaSite(_) => "Confirm cancel site",
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

    #[test]
    fn every_task_is_reachable_and_shrinking_lists_clamp_the_page() {
        for total in [0_usize, 1, 20, 21, 200, 661] {
            let pages = total.max(1).div_ceil(20);
            let actual: Vec<_> = (0..pages)
                .flat_map(|page| task_page_range(page, total))
                .collect();
            assert_eq!(actual, (0..total).collect::<Vec<_>>());
            assert!(task_page_range(usize::MAX, total).len() <= 20);
        }
        assert_eq!(task_page_range(9, 180), 160..180);
    }
}
