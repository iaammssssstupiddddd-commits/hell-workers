// タスクリストのオーケストレーション

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_ui::components::{LeftPanelMode, TaskListBody};
use hw_ui::panels::info_panel::InfoPanelPinState;
use hw_ui::panels::task_list::{TaskDashboardActionState, TaskDashboardViewState};
use hw_ui::theme::UiTheme;

#[cfg(feature = "profiling")]
use super::view_model::{TaskDashboardPerfMetrics, TaskDashboardTimingMetrics};
use super::{TaskListDirty, view_model::TaskListState};
#[cfg(feature = "profiling")]
use std::time::Instant;

const TASK_DASHBOARD_ROW_OVERDRAW: usize = 2;

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskDashboardViewport {
    resident_row_limit: usize,
}

impl Default for TaskDashboardViewport {
    fn default() -> Self {
        Self {
            resident_row_limit: 1,
        }
    }
}

fn maximum_resident_rows(window_height: f32, theme: &UiTheme) -> usize {
    let row_height = theme.sizes.soul_item_height.max(1.0);
    let max_panel_height =
        window_height.max(0.0) * theme.sizes.entity_list_max_height_percent / 100.0;
    (max_panel_height / row_height).ceil() as usize + TASK_DASHBOARD_ROW_OVERDRAW
}

fn clear_task_list_body(commands: &mut Commands, body_entity: Entity) {
    commands.entity(body_entity).despawn_children();
}

pub fn sync_task_dashboard_viewport_system(
    window: Query<&Window, With<PrimaryWindow>>,
    theme: Res<UiTheme>,
    mut viewport: ResMut<TaskDashboardViewport>,
    mut dirty: ResMut<TaskListDirty>,
) {
    let Ok(window) = window.single() else {
        return;
    };
    let resident_row_limit = maximum_resident_rows(window.height(), &theme).max(1);
    if viewport.resident_row_limit != resident_row_limit {
        viewport.resident_row_limit = resident_row_limit;
        dirty.mark_list();
    }
}

#[cfg(feature = "profiling")]
struct TaskDashboardTimingGuard<'a> {
    metrics: Option<&'a mut TaskDashboardTimingMetrics>,
    started: Instant,
}

#[cfg(feature = "profiling")]
impl<'a> TaskDashboardTimingGuard<'a> {
    fn new(metrics: Option<&'a mut TaskDashboardTimingMetrics>) -> Self {
        Self {
            metrics,
            started: Instant::now(),
        }
    }
}

#[cfg(feature = "profiling")]
impl Drop for TaskDashboardTimingGuard<'_> {
    fn drop(&mut self) {
        let Some(metrics) = self.metrics.as_deref_mut() else {
            return;
        };
        if !metrics.active {
            return;
        }
        let elapsed_ns = u64::try_from(self.started.elapsed().as_nanos()).unwrap_or(u64::MAX);
        metrics.system_invocations = metrics.system_invocations.saturating_add(1);
        metrics.total_elapsed_ns = metrics.total_elapsed_ns.saturating_add(elapsed_ns);
    }
}

#[derive(SystemParam)]
pub struct TaskListRenderState<'w> {
    game_assets: Res<'w, crate::assets::GameAssets>,
    theme: Res<'w, UiTheme>,
    mode: Res<'w, LeftPanelMode>,
    state: Res<'w, TaskListState>,
    view_state: Res<'w, TaskDashboardViewState>,
    action_state: Res<'w, TaskDashboardActionState>,
    pin_state: Res<'w, InfoPanelPinState>,
    viewport: Res<'w, TaskDashboardViewport>,
}

pub fn task_list_update_system(
    mut commands: Commands,
    render_state: TaskListRenderState,
    mut dirty: ResMut<TaskListDirty>,
    body_query: Query<Entity, With<TaskListBody>>,
    children_query: Query<&Children>,
    #[cfg(feature = "profiling")] mut perf_metrics: Option<ResMut<TaskDashboardPerfMetrics>>,
    #[cfg(feature = "profiling")] mut timing_metrics: Option<ResMut<TaskDashboardTimingMetrics>>,
) {
    #[cfg(feature = "profiling")]
    let _timing_guard = TaskDashboardTimingGuard::new(timing_metrics.as_deref_mut());

    if *render_state.mode != LeftPanelMode::TaskList {
        return;
    }

    if !dirty.list_dirty() {
        return;
    }

    let Ok(body_entity) = body_query.single() else {
        return;
    };

    if let Ok(_children) = children_query.get(body_entity) {
        #[cfg(feature = "profiling")]
        if let Some(perf_metrics) = perf_metrics.as_deref_mut() {
            perf_metrics.despawn_roots_requested = perf_metrics
                .despawn_roots_requested
                .saturating_add(u32::try_from(_children.len()).unwrap_or(u32::MAX));
        }
        clear_task_list_body(&mut commands, body_entity);
    }

    #[cfg(feature = "profiling")]
    let mut render_stats = hw_ui::panels::task_list::TaskListRenderStats::default();
    commands.entity(body_entity).with_children(|parent| {
        #[cfg(feature = "profiling")]
        {
            render_stats = hw_ui::panels::task_list::rebuild_task_list_ui(
                parent,
                hw_ui::panels::task_list::TaskListRenderInput {
                    snapshot: &render_state.state.snapshot,
                    view_state: &render_state.view_state,
                    pinned_entity: render_state.pin_state.entity,
                    action_state: &render_state.action_state,
                    game_assets: &*render_state.game_assets,
                    theme: &render_state.theme,
                    resident_row_limit: render_state.viewport.resident_row_limit,
                },
            );
        }
        #[cfg(not(feature = "profiling"))]
        hw_ui::panels::task_list::rebuild_task_list_ui(
            parent,
            hw_ui::panels::task_list::TaskListRenderInput {
                snapshot: &render_state.state.snapshot,
                view_state: &render_state.view_state,
                pinned_entity: render_state.pin_state.entity,
                action_state: &render_state.action_state,
                game_assets: &*render_state.game_assets,
                theme: &render_state.theme,
                resident_row_limit: render_state.viewport.resident_row_limit,
            },
        );
    });
    #[cfg(feature = "profiling")]
    if let Some(perf_metrics) = perf_metrics.as_deref_mut() {
        perf_metrics.render_rebuilds = perf_metrics.render_rebuilds.saturating_add(1);
        perf_metrics.render_input_rows = perf_metrics
            .render_input_rows
            .saturating_add(render_stats.input_rows);
        perf_metrics.render_visible_rows = perf_metrics
            .render_visible_rows
            .saturating_add(render_stats.visible_rows);
        perf_metrics.render_group_headers = perf_metrics
            .render_group_headers
            .saturating_add(render_stats.group_headers);
    }
    dirty.clear_list();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Component)]
    struct TestBody;

    #[derive(Component)]
    struct TestChild;

    fn queue_child_despawn(mut commands: Commands, child_query: Query<Entity, With<TestChild>>) {
        for child in &child_query {
            commands.entity(child).despawn();
        }
    }

    fn queue_body_cleanup(mut commands: Commands, body_query: Query<Entity, With<TestBody>>) {
        let body = body_query.single().expect("test body should exist");
        clear_task_list_body(&mut commands, body);
    }

    #[test]
    fn resident_limit_covers_the_maximum_panel_height_with_overdraw() {
        let theme = UiTheme::default();

        assert_eq!(maximum_resident_rows(1_080.0, &theme), 40);
        assert_eq!(maximum_resident_rows(2_160.0, &theme), 78);
    }

    #[test]
    fn body_cleanup_resolves_children_after_an_earlier_deferred_despawn() {
        let mut app = App::new();
        app.set_error_handler(bevy::ecs::error::panic);
        app.add_systems(
            Update,
            (queue_child_despawn, queue_body_cleanup).chain_ignore_deferred(),
        );

        let body = app.world_mut().spawn(TestBody).id();
        let child = app.world_mut().spawn((TestChild, ChildOf(body))).id();

        app.update();

        assert!(app.world().get_entity(body).is_ok());
        assert!(app.world().get_entity(child).is_err());
        assert!(app.world().get::<Children>(body).is_none());
    }
}
