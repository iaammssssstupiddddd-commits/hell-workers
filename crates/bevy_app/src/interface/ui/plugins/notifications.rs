use crate::systems::GameSystemSet;
use bevy::prelude::*;
use hw_ui::notifications::{
    NotificationSystemSet, apply_notification_ui_state_system, present_notifications_system,
    reduce_notifications_system,
};

pub struct UiNotificationsPlugin;

impl Plugin for UiNotificationsPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            Update,
            (
                NotificationSystemSet::Adapt,
                NotificationSystemSet::Reduce,
                NotificationSystemSet::Present,
            )
                .chain()
                .in_set(GameSystemSet::Interface),
        )
        .add_systems(
            Update,
            (
                crate::interface::ui::notifications::adapt_save_load_outcomes,
                crate::interface::ui::notifications::adapt_stockpile_policy_change_outcomes,
                crate::interface::ui::panels::task_list::adapt_task_action_outcomes,
            )
                .in_set(NotificationSystemSet::Adapt),
        )
        .add_systems(
            Update,
            (
                reduce_notifications_system,
                apply_notification_ui_state_system,
            )
                .chain()
                .in_set(NotificationSystemSet::Reduce),
        )
        .add_systems(
            Update,
            present_notifications_system.in_set(NotificationSystemSet::Present),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface::ui::panels::task_list::{
        TaskActionKind, TaskActionOutcome, TaskActionResult,
    };
    use crate::systems::save::{
        SaveLoadFailureKind, SaveLoadOperation, SaveLoadOutcome, SaveLoadResult,
    };
    use hw_ui::HwUiPlugin;
    use hw_ui::components::UiInputState;
    use hw_ui::notifications::{
        NotificationCenter, NotificationRetention, NotificationSeverity, UserFacingNotification,
    };
    use hw_ui::panels::task_list::TaskPriorityTier;
    use hw_ui::theme::UiTheme;

    #[derive(Resource, Default)]
    struct PresentTrace(Vec<usize>);

    fn adapt(mut notifications: MessageWriter<UserFacingNotification>) {
        notifications.write(UserFacingNotification::new(
            "same-update",
            NotificationSeverity::Success,
            "Saved",
            "Current save",
            NotificationRetention::Important,
        ));
    }

    fn trace_present(center: Res<NotificationCenter>, mut trace: ResMut<PresentTrace>) {
        trace.0.push(center.history_count());
    }

    #[test]
    fn adapt_reduce_and_present_run_in_the_same_update_in_order() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, HwUiPlugin, UiNotificationsPlugin))
            .add_message::<SaveLoadOutcome>()
            .add_message::<TaskActionOutcome>()
            .add_message::<hw_logistics::StockpilePolicyChangeOutcome>()
            .init_resource::<UiTheme>()
            .init_resource::<UiInputState>()
            .init_resource::<PresentTrace>()
            .add_systems(Update, adapt.in_set(NotificationSystemSet::Adapt))
            .add_systems(Update, trace_present.in_set(NotificationSystemSet::Present));

        app.update();

        assert_eq!(app.world().resource::<PresentTrace>().0, vec![1]);
    }

    #[test]
    fn identical_save_load_outcomes_coalesce_through_the_real_adapter() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, HwUiPlugin, UiNotificationsPlugin))
            .add_message::<SaveLoadOutcome>()
            .add_message::<TaskActionOutcome>()
            .add_message::<hw_logistics::StockpilePolicyChangeOutcome>()
            .init_resource::<UiTheme>()
            .init_resource::<UiInputState>();
        let outcome = SaveLoadOutcome {
            operation: SaveLoadOperation::Load,
            target: "world.scn.ron".to_owned(),
            result: SaveLoadResult::Failed(SaveLoadFailureKind::LoadNotFound),
        };
        app.world_mut().write_message(outcome.clone());
        app.world_mut().write_message(outcome);

        app.update();

        let center = app.world().resource::<NotificationCenter>();
        assert_eq!(center.history_count(), 1);
        assert_eq!(center.history_entries().next().unwrap().repeat_count, 2);
    }

    #[test]
    fn task_action_outcome_becomes_one_toast_without_history() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, HwUiPlugin, UiNotificationsPlugin))
            .add_message::<SaveLoadOutcome>()
            .add_message::<TaskActionOutcome>()
            .add_message::<hw_logistics::StockpilePolicyChangeOutcome>()
            .init_resource::<UiTheme>()
            .init_resource::<UiInputState>();
        let entity = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(TaskActionOutcome {
            entity,
            action: TaskActionKind::AdjustPriority(
                hw_ui::panels::task_list::TaskPriorityAdjustment::Increase,
            ),
            result: TaskActionResult::PriorityChanged(TaskPriorityTier::High),
        });

        app.update();

        let center = app.world().resource::<NotificationCenter>();
        assert_eq!(center.toast_count(), 1);
        assert_eq!(center.history_count(), 0);
        assert_eq!(
            center.toast_entries().next().unwrap().retention,
            NotificationRetention::ToastOnly
        );
    }

    #[test]
    fn stockpile_policy_outcome_becomes_a_warning_toast_in_the_same_update() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, HwUiPlugin, UiNotificationsPlugin))
            .add_message::<SaveLoadOutcome>()
            .add_message::<TaskActionOutcome>()
            .add_message::<hw_logistics::StockpilePolicyChangeOutcome>()
            .init_resource::<UiTheme>()
            .init_resource::<UiInputState>();
        app.world_mut()
            .write_message(hw_logistics::StockpilePolicyChangeOutcome {
                requested: 2,
                unique: 2,
                applied: 1,
                unchanged: 0,
                skipped_stale: 0,
                skipped_unmanaged: 1,
                target_clamped: 0,
            });

        app.update();

        let center = app.world().resource::<NotificationCenter>();
        let toast = center.toast_entries().next().expect("warning toast");
        assert_eq!(toast.severity, NotificationSeverity::Warning);
        assert_eq!(toast.retention, NotificationRetention::ToastOnly);
        assert!(toast.body.contains("unsupported or special storage"));
    }
}
