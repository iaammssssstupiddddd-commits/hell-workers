use bevy::prelude::*;

use crate::components::PauseMenu;

/// Menu visibility is independent of the player's simulation pause.
/// Only the root time owner may fill or consume `resume_speed`.
#[derive(Resource, Default, Debug)]
pub struct SystemMenuState {
    pub open: bool,
    pub resume_speed: Option<f32>,
}

type ActionAvailabilityButtons<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Option<&'static crate::components::MenuButton>,
        Has<crate::panels::task_list::TaskActionButton>,
        Has<crate::components::FamiliarMaxSoulAdjustButton>,
        Option<&'static mut crate::components::UiTooltip>,
        &'static mut BackgroundColor,
    ),
    With<Button>,
>;

/// Keep the unavailable reason on the live control, including hover tooltips.
pub fn show_paused_action_availability(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    theme: Res<crate::theme::UiTheme>,
    mut buttons: ActionAvailabilityButtons,
    mut was_paused: Local<bool>,
) {
    let pause_changed = *was_paused != time.is_paused();
    *was_paused = time.is_paused();
    const REASON: &str = "\n停止中は利用不可（Spaceで再開）";
    for (entity, menu, task, max_soul, tooltip, mut color) in &mut buttons {
        let blocked = time.is_paused()
            && (task || max_soul || menu.is_some_and(|menu| !menu.0.allowed_while_paused()));
        if blocked {
            color.0 = theme.colors.button_default.with_alpha(0.35);
        } else if pause_changed
            && (task || max_soul || menu.is_some_and(|menu| !menu.0.allowed_while_paused()))
        {
            color.0 = theme.colors.button_default;
        }
        if let Some(mut tooltip) = tooltip {
            if blocked && !tooltip.text.ends_with(REASON) {
                tooltip.text = format!("{}{REASON}", tooltip.text).into();
            } else if !blocked && tooltip.text.ends_with(REASON) {
                tooltip.text = tooltip.text.trim_end_matches(REASON).to_owned().into();
            }
        } else if blocked {
            commands
                .entity(entity)
                .insert(crate::components::UiTooltip::new(REASON));
        }
    }
}

pub fn update_pause_menu_visibility(
    menu: Res<SystemMenuState>,
    mut q_pause_menu: Query<&mut Node, With<PauseMenu>>,
) {
    let display = if menu.open {
        Display::Flex
    } else {
        Display::None
    };

    if let Ok(mut node) = q_pause_menu.single_mut() {
        node.display = display;
    }
}
