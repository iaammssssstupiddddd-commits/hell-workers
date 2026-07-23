use crate::interface::ui::panels::task_list::{
    TaskListDirty, TaskListState, detect_task_list_changed_components,
    detect_task_list_removed_components, update_task_list_state_system,
};
use crate::interface::ui::panels::task_list::{
    left_panel_tab_system, left_panel_visibility_system, task_dashboard_action_state_sync_system,
    task_dashboard_control_system, task_list_click_system, task_list_update_system,
    task_list_visual_feedback_system,
};
use crate::interface::ui::{
    InfoPanelNodes, InfoPanelPinState, InfoPanelState, info_panel_system,
    presentation::EntityInspectionViewModel, update_entity_inspection_view_model_system,
};
use crate::systems::GameSystemSet;
use bevy::prelude::*;
use bevy::time::Real;
use hw_ui::components::{LeftPanelMode, SoulRenameState};
use hw_ui::interaction::{soul_rename_button_system, soul_rename_cleanup_system};

const INSPECTION_REFRESH_INTERVAL_SECS: f32 = 0.1;

/// The selected/pinned inspector is dynamic, but rebuilding its strings every
/// render frame is unnecessary. Selection and pin changes wake immediately;
/// steady inspection refreshes run from real time so pausing simulation does
/// not freeze the panel.
#[derive(Resource)]
struct InspectionRefreshCadence {
    timer: Timer,
    due: bool,
}

impl Default for InspectionRefreshCadence {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(INSPECTION_REFRESH_INTERVAL_SECS, TimerMode::Repeating),
            due: true,
        }
    }
}

fn advance_inspection_refresh_cadence_system(
    time: Res<Time<Real>>,
    mut cadence: ResMut<InspectionRefreshCadence>,
) {
    cadence.due = cadence.timer.tick(time.delta()).just_finished();
}

fn inspection_refresh_should_run(
    selected: Res<crate::interface::selection::SelectedEntity>,
    pin_state: Res<InfoPanelPinState>,
    rename_state: Res<SoulRenameState>,
    cadence: Res<InspectionRefreshCadence>,
    changed_stockpiles: Query<(), Changed<hw_logistics::StockpilePolicy>>,
) -> bool {
    let inspected_policy_changed = pin_state
        .entity
        .or(selected.0)
        .is_some_and(|entity| changed_stockpiles.get(entity).is_ok());
    selected.is_changed()
        || pin_state.is_changed()
        || rename_state.is_changed()
        || inspected_policy_changed
        || (cadence.due && (selected.0.is_some() || pin_state.entity.is_some()))
}

pub type UiInfoPanelPlugin = hw_ui::plugins::info_panel::UiInfoPanelPlugin;

pub fn ui_info_panel_plugin() -> UiInfoPanelPlugin {
    UiInfoPanelPlugin::new(register_ui_info_panel_plugin_systems)
}

fn register_ui_info_panel_plugin_systems(app: &mut App) {
    app.init_resource::<InfoPanelState>();
    app.init_resource::<InfoPanelPinState>();
    app.init_resource::<InfoPanelNodes>();
    app.init_resource::<LeftPanelMode>();
    app.init_resource::<EntityInspectionViewModel>();
    app.init_resource::<InspectionRefreshCadence>();
    app.init_resource::<TaskListDirty>();
    app.init_resource::<TaskListState>();
    app.add_systems(
        PreUpdate,
        (
            detect_task_list_changed_components,
            detect_task_list_removed_components,
            update_task_list_state_system,
        )
            .chain(),
    );
    app.add_systems(
        Update,
        advance_inspection_refresh_cadence_system.in_set(GameSystemSet::Interface),
    );
    app.add_systems(
        Update,
        (
            left_panel_tab_system,
            left_panel_visibility_system.after(left_panel_tab_system),
            task_dashboard_action_state_sync_system.after(left_panel_tab_system),
            task_dashboard_control_system.after(task_dashboard_action_state_sync_system),
            task_list_update_system
                .after(task_dashboard_action_state_sync_system)
                .after(task_dashboard_control_system),
            task_list_click_system,
            task_list_visual_feedback_system.after(task_list_click_system),
            soul_rename_button_system::<crate::assets::GameAssets>,
            soul_rename_cleanup_system,
        )
            .in_set(GameSystemSet::Interface),
    );
    app.add_systems(
        Update,
        (
            update_entity_inspection_view_model_system
                .after(hw_logistics::apply_stockpile_policy_change_requests_system),
            info_panel_system::<crate::assets::GameAssets>
                .after(update_entity_inspection_view_model_system)
                .after(crate::interface::ui::menu_visibility_system)
                .before(crate::interface::ui::update_mode_text_system),
        )
            .chain()
            .run_if(inspection_refresh_should_run)
            .after(advance_inspection_refresh_cadence_system)
            .in_set(GameSystemSet::Interface),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface::selection::SelectedEntity;
    use crate::test_support::minimal_app;
    use bevy::time::TimeUpdateStrategy;
    use hw_core::relationships::ParkedAt;
    use hw_logistics::transport_request::arbitration::WheelbarrowArbitrationRuntime;
    use hw_logistics::transport_request::producer::active_unit_cache::{
        CachedActiveYards, CachedStockpileGroups, update_cached_active_yards_system,
        update_cached_stockpile_groups_system,
    };
    use hw_logistics::transport_request::producer::task_area::task_area_auto_haul_system;
    use hw_logistics::transport_request::{
        TransportRequestMetrics, WheelbarrowArbitrationDiagnostics, wheelbarrow_arbitration_system,
    };
    use hw_logistics::{
        BelongsTo, ResourceItem, ResourceType, SharedResourceCache, Stockpile, StockpilePolicy,
        StockpilePolicyChangeOutcome, StockpilePolicyChangeRequest, StockpilePolicyPatch,
        Wheelbarrow, apply_stockpile_policy_change_requests_system,
    };
    use hw_spatial::{FamiliarSpatialGrid, SpatialGridOps, StockpileSpatialGrid};
    use hw_world::Yard;

    const MEASURE_TICKS: u64 = 40;

    #[derive(Debug, Default, PartialEq, Eq)]
    struct SteadyStateTotals {
        task_area_groups: u64,
        task_area_free_items_scanned: u64,
        task_area_items_matched: u64,
        wheelbarrow_leases_granted: u64,
        wheelbarrow_eligible_requests: u64,
        wheelbarrow_bucket_items: u64,
        wheelbarrow_candidates_after_top_k: u64,
        wheelbarrow_items_deduped: u64,
        wheelbarrow_candidates_dropped_by_dedup: u64,
        group_rebuilds: u64,
        arbitration_rebuilds: u64,
    }

    impl SteadyStateTotals {
        fn sample(&mut self, metrics: &TransportRequestMetrics) {
            self.task_area_groups += u64::from(metrics.task_area_groups);
            self.task_area_free_items_scanned += u64::from(metrics.task_area_free_items_scanned);
            self.task_area_items_matched += u64::from(metrics.task_area_items_matched);
            self.wheelbarrow_leases_granted +=
                u64::from(metrics.wheelbarrow_leases_granted_this_frame);
            self.wheelbarrow_eligible_requests +=
                u64::from(metrics.wheelbarrow_arb_eligible_requests);
            self.wheelbarrow_bucket_items += u64::from(metrics.wheelbarrow_arb_bucket_items_total);
            self.wheelbarrow_candidates_after_top_k +=
                u64::from(metrics.wheelbarrow_arb_candidates_after_topk);
            self.wheelbarrow_items_deduped += u64::from(metrics.wheelbarrow_arb_items_deduped);
            self.wheelbarrow_candidates_dropped_by_dedup +=
                u64::from(metrics.wheelbarrow_arb_candidates_dropped_by_dedup);
        }
    }

    fn steady_state_fixture() -> (App, Entity) {
        let mut app = minimal_app();
        app.insert_resource(Time::<Fixed>::from_hz(60.0))
            .insert_resource(TimeUpdateStrategy::FixedTimesteps(1))
            .init_resource::<SelectedEntity>()
            .init_resource::<InfoPanelPinState>()
            .init_resource::<EntityInspectionViewModel>()
            .init_resource::<InspectionRefreshCadence>()
            .init_resource::<SoulRenameState>()
            .init_resource::<FamiliarSpatialGrid>()
            .init_resource::<StockpileSpatialGrid>()
            .init_resource::<CachedActiveYards>()
            .init_resource::<CachedStockpileGroups>()
            .init_resource::<TransportRequestMetrics>()
            .init_resource::<SharedResourceCache>()
            .init_resource::<WheelbarrowArbitrationRuntime>()
            .init_resource::<WheelbarrowArbitrationDiagnostics>()
            .add_message::<StockpilePolicyChangeRequest>()
            .add_message::<StockpilePolicyChangeOutcome>()
            .add_systems(
                Update,
                (
                    apply_stockpile_policy_change_requests_system,
                    update_cached_active_yards_system,
                    update_cached_stockpile_groups_system,
                    task_area_auto_haul_system,
                    ApplyDeferred,
                    wheelbarrow_arbitration_system,
                )
                    .chain(),
            )
            .add_systems(Update, advance_inspection_refresh_cadence_system)
            .add_systems(
                Update,
                update_entity_inspection_view_model_system
                    .run_if(inspection_refresh_should_run)
                    .after(advance_inspection_refresh_cadence_system)
                    .after(wheelbarrow_arbitration_system),
            );

        let yard = app
            .world_mut()
            .spawn(Yard {
                min: Vec2::splat(-16.0),
                max: Vec2::splat(16.0),
            })
            .id();
        let stockpile = app
            .world_mut()
            .spawn((
                Transform::default(),
                Stockpile {
                    capacity: 3,
                    resource_type: None,
                },
                StockpilePolicy::for_capacity(3),
                BelongsTo(yard),
            ))
            .id();
        app.world_mut().spawn((
            Transform::default(),
            Visibility::Visible,
            ResourceItem(ResourceType::Wood),
        ));
        let parking = app.world_mut().spawn_empty().id();
        app.world_mut().spawn((
            Transform::default(),
            Wheelbarrow { capacity: 10 },
            ParkedAt(parking),
        ));
        app.world_mut()
            .resource_mut::<StockpileSpatialGrid>()
            .insert(stockpile, Vec2::ZERO);

        (app, stockpile)
    }

    fn run_steady_state_scenario(exercise_policy_ui: bool) -> SteadyStateTotals {
        let (mut app, stockpile) = steady_state_fixture();

        // The request spawn and one-time pending marker are lifecycle changes. Let both settle
        // before comparing the unchanged steady-state window.
        for _ in 0..3 {
            app.update();
        }
        let group_generation_before = app.world().resource::<CachedStockpileGroups>().generation();
        let arbitration_generation_before = app
            .world()
            .resource::<WheelbarrowArbitrationDiagnostics>()
            .header()
            .expect("initial arbitration diagnostics")
            .generation;
        let mut totals = SteadyStateTotals::default();

        for tick in 0..MEASURE_TICKS {
            if exercise_policy_ui && tick == 5 {
                app.world_mut().resource_mut::<SelectedEntity>().0 = Some(stockpile);
                app.world_mut().write_message(StockpilePolicyChangeRequest {
                    targets: vec![stockpile],
                    patch: StockpilePolicyPatch::default(),
                });
            }
            if exercise_policy_ui && tick == 20 {
                app.world_mut().resource_mut::<SelectedEntity>().0 = None;
            }

            app.update();

            if exercise_policy_ui && tick == 5 {
                assert_eq!(
                    app.world()
                        .resource::<EntityInspectionViewModel>()
                        .model
                        .as_ref()
                        .map(|model| model.entity),
                    Some(stockpile)
                );
                assert!(
                    !app.world()
                        .entity(stockpile)
                        .get_ref::<StockpilePolicy>()
                        .expect("managed stockpile policy")
                        .is_changed(),
                    "a no-op editor request must not dirty producer/arbitration inputs"
                );
            }
            if exercise_policy_ui && tick == 20 {
                assert!(
                    app.world()
                        .resource::<EntityInspectionViewModel>()
                        .model
                        .is_none()
                );
            }

            totals.sample(app.world().resource::<TransportRequestMetrics>());
        }

        totals.group_rebuilds = app
            .world()
            .resource::<CachedStockpileGroups>()
            .generation()
            .wrapping_sub(group_generation_before);
        totals.arbitration_rebuilds = app
            .world()
            .resource::<WheelbarrowArbitrationDiagnostics>()
            .header()
            .expect("steady-state arbitration diagnostics")
            .generation
            .wrapping_sub(arbitration_generation_before);
        totals
    }

    #[test]
    fn policy_panel_open_close_does_not_add_producer_or_arbitration_work() {
        let control = run_steady_state_scenario(false);
        let policy_ui = run_steady_state_scenario(true);

        assert_eq!(policy_ui, control);
        assert_eq!(control.group_rebuilds, 0);
        assert_eq!(control.task_area_groups, MEASURE_TICKS);
        assert_eq!(control.task_area_free_items_scanned, MEASURE_TICKS);
        assert_eq!(control.task_area_items_matched, MEASURE_TICKS);
        assert!(control.arbitration_rebuilds >= 1);
        assert!(control.wheelbarrow_eligible_requests > 0);
    }
}
