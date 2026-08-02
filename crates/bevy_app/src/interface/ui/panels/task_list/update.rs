// タスクリストのオーケストレーション

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_ui::components::{LeftPanelMode, TaskListBody};
use hw_ui::panels::info_panel::InfoPanelPinState;
use hw_ui::panels::task_list::{TaskDashboardActionState, TaskDashboardViewState};
use hw_ui::theme::UiTheme;

#[cfg(feature = "profiling")]
use super::view_model::{TaskDashboardPerfMetrics, TaskDashboardTimingMetrics};
use super::{TaskListDirty, view_model::TaskListState};
#[cfg(feature = "profiling")]
use std::time::Instant;

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

    if let Ok(children) = children_query.get(body_entity) {
        #[cfg(feature = "profiling")]
        if let Some(perf_metrics) = perf_metrics.as_deref_mut() {
            perf_metrics.despawn_roots_requested = perf_metrics
                .despawn_roots_requested
                .saturating_add(u32::try_from(children.len()).unwrap_or(u32::MAX));
        }
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }

    #[cfg(feature = "profiling")]
    let mut render_stats = hw_ui::panels::task_list::TaskListRenderStats::default();
    commands.entity(body_entity).with_children(|parent| {
        #[cfg(feature = "profiling")]
        {
            render_stats = hw_ui::panels::task_list::rebuild_task_list_ui(
                parent,
                &render_state.state.snapshot,
                &render_state.view_state,
                render_state.pin_state.entity,
                &render_state.action_state,
                &*render_state.game_assets,
                &render_state.theme,
            );
        }
        #[cfg(not(feature = "profiling"))]
        hw_ui::panels::task_list::rebuild_task_list_ui(
            parent,
            &render_state.state.snapshot,
            &render_state.view_state,
            render_state.pin_state.entity,
            &render_state.action_state,
            &*render_state.game_assets,
            &render_state.theme,
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
