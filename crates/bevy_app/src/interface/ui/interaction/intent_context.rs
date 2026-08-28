use bevy::ecs::system::SystemParam;
use bevy::input_focus::InputFocus;
use bevy::prelude::*;

use crate::entities::familiar::{Familiar, FamiliarOperation, FamiliarPolicy};
use crate::input_actions::ActiveModeCleanupParams;
use crate::interface::selection::SelectedEntity;
use crate::interface::ui::InfoPanelPinState;
use crate::systems::command::TaskArea;
use crate::systems::save::{
    SaveCatalog, SaveCatalogUi, SaveDialogSession, SaveLoadState, SaveRecoveryMode, SaveStorageRoot,
};
use crate::systems::settings::SettingsStorageRoot;
use crate::world::map::GeneratedWorldLayoutResource;
use hw_core::game_state::PlayMode;
use hw_jobs::{Building, BuildingCategory, DeconstructionPending};
use hw_ui::components::{ArchitectCategoryState, OperationDialog};

#[derive(SystemParam)]
pub(crate) struct IntentModeCtx<'w, 's> {
    pub(crate) cleanup: ActiveModeCleanupParams<'w, 's>,
    pub(crate) play_mode: Res<'w, State<PlayMode>>,
    pub(crate) time: ResMut<'w, Time<Virtual>>,
}

impl IntentModeCtx<'_, '_> {
    pub(crate) fn cancel_active_mode_if_needed(&mut self) {
        if self.cleanup.has_active_owner_state(self.play_mode.get()) {
            self.cleanup.cancel_active_mode();
        }
    }
}

/// UI-owned mode and selection actions used by the generic intent handler.
#[derive(SystemParam)]
pub(crate) struct IntentActionCtx<'w, 's> {
    architect_category: ResMut<'w, ArchitectCategoryState>,
    q_buildings: Query<'w, 's, &'static Building, Without<DeconstructionPending>>,
}

impl IntentActionCtx<'_, '_> {
    pub(crate) fn toggle_architect_category(&mut self, category: Option<BuildingCategory>) {
        self.architect_category.0 = if self.architect_category.0 == category {
            None
        } else {
            category
        };
    }

    pub(crate) fn is_move_plant_target(&self, entity: Entity) -> bool {
        self.q_buildings
            .get(entity)
            .is_ok_and(|building| building.kind.is_player_movable())
    }
}

#[derive(SystemParam)]
pub(crate) struct IntentSelectionCtx<'w> {
    pub(crate) selected_entity: ResMut<'w, SelectedEntity>,
    pub(crate) info_panel_pin: ResMut<'w, InfoPanelPinState>,
    pub(crate) resolved_frame: Res<'w, crate::input_actions::ResolvedInputFrame>,
}

type FamiliarSettingsTargetQuery<'w, 's> = Query<
    'w,
    's,
    (),
    (
        With<Familiar>,
        With<FamiliarOperation>,
        With<FamiliarPolicy>,
    ),
>;

#[derive(SystemParam)]
pub(crate) struct IntentFamiliarQueries<'w, 's> {
    pub(crate) q_familiar_settings: FamiliarSettingsTargetQuery<'w, 's>,
    pub(crate) q_familiars_for_area:
        Query<'w, 's, (Entity, Option<&'static TaskArea>), With<Familiar>>,
}

#[derive(SystemParam)]
pub(crate) struct IntentUiQueries<'w, 's> {
    // ダイアログはそれぞれ独立したエンティティだが、&mut Node の 2 クエリは
    // Without で disjoint を明示しないと B0001（クエリ競合 panic）になる。
    pub(crate) q_dialog: Query<'w, 's, &'static mut Node, With<OperationDialog>>,
    pub(crate) q_operation_scroll:
        Query<'w, 's, &'static mut ScrollPosition, With<hw_ui::components::OperationDialogScroll>>,
    pub(crate) input_focus: ResMut<'w, InputFocus>,
    pub(crate) save_load_state: ResMut<'w, SaveLoadState>,
    pub(crate) save_storage_root: Res<'w, SaveStorageRoot>,
    pub(crate) settings_storage_root: Res<'w, SettingsStorageRoot>,
    pub(crate) save_catalog: ResMut<'w, SaveCatalog>,
    pub(crate) save_catalog_ui: ResMut<'w, SaveCatalogUi>,
    pub(crate) save_dialog_session: ResMut<'w, SaveDialogSession>,
    pub(crate) save_recovery: Res<'w, SaveRecoveryMode>,
    pub(crate) worldgen_layout: Option<Res<'w, GeneratedWorldLayoutResource>>,
}

impl IntentUiQueries<'_, '_> {
    pub(crate) fn worldgen_seed(&self) -> Option<u64> {
        self.worldgen_layout
            .as_ref()
            .map(|layout| layout.master_seed)
    }
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
