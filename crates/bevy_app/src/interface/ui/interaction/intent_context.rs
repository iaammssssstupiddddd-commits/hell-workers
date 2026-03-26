use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::app_contexts::{BuildContext, TaskContext, ZoneContext};
use crate::entities::familiar::{Familiar, FamiliarOperation};
use crate::interface::selection::SelectedEntity;
use crate::interface::ui::{EntityListNodeIndex, InfoPanelPinState};
use crate::systems::command::TaskArea;
use crate::systems::familiar_ai::FamiliarAiState;
use hw_core::game_state::PlayMode;
use hw_core::relationships::Commanding;
use hw_ui::components::{MenuState, OperationDialog};

#[derive(SystemParam)]
pub(crate) struct IntentModeCtx<'w> {
    pub(crate) menu_state: ResMut<'w, MenuState>,
    pub(crate) next_play_mode: ResMut<'w, NextState<PlayMode>>,
    pub(crate) build_context: ResMut<'w, BuildContext>,
    pub(crate) zone_context: ResMut<'w, ZoneContext>,
    pub(crate) task_context: ResMut<'w, TaskContext>,
}

#[derive(SystemParam)]
pub(crate) struct IntentSelectionCtx<'w> {
    pub(crate) selected_entity: ResMut<'w, SelectedEntity>,
    pub(crate) info_panel_pin: ResMut<'w, InfoPanelPinState>,
    pub(crate) node_index: Res<'w, EntityListNodeIndex>,
}

#[derive(SystemParam)]
pub(crate) struct IntentFamiliarQueries<'w, 's> {
    pub(crate) q_familiar_ops: Query<'w, 's, &'static mut FamiliarOperation>,
    pub(crate) q_familiar_meta: Query<
        'w,
        's,
        (
            &'static Familiar,
            &'static FamiliarAiState,
            Option<&'static Commanding>,
        ),
    >,
    pub(crate) q_familiars_for_area:
        Query<'w, 's, (Entity, Option<&'static TaskArea>), With<Familiar>>,
}

#[derive(SystemParam)]
pub(crate) struct IntentUiQueries<'w, 's> {
    pub(crate) q_dialog: Query<'w, 's, &'static mut Node, With<OperationDialog>>,
    pub(crate) q_text: Query<'w, 's, &'static mut Text>,
}

pub(crate) fn ensure_familiar_selected(
    selected_entity: &mut ResMut<SelectedEntity>,
    q_familiars_for_area: &Query<(Entity, Option<&TaskArea>), With<Familiar>>,
    _mode_label: &str,
) {
    let selected_is_familiar = selected_entity
        .0
        .is_some_and(|entity| q_familiars_for_area.get(entity).is_ok());

    if selected_is_familiar {
        return;
    }

    let mut familiars: Vec<(Entity, bool)> = q_familiars_for_area
        .iter()
        .map(|(entity, area_opt)| (entity, area_opt.is_some()))
        .collect();
    familiars.sort_by_key(|(entity, _)| entity.index());

    let fallback = familiars
        .iter()
        .find(|(_, has_area)| !*has_area)
        .map(|(entity, _)| *entity)
        .or_else(|| familiars.first().map(|(entity, _)| *entity));

    if let Some(familiar_entity) = fallback {
        selected_entity.0 = Some(familiar_entity);
    }
}
