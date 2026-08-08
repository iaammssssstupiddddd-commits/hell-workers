use bevy::ecs::system::SystemParam;
use bevy::input_focus::InputFocus;
use bevy::prelude::*;

use crate::entities::familiar::{Familiar, FamiliarOperation, FamiliarPolicy};
use crate::input_actions::ActiveModeCleanupParams;
use crate::interface::selection::SelectedEntity;
use crate::interface::ui::InfoPanelPinState;
use crate::systems::command::TaskArea;
use crate::systems::save::{SaveLoadState, SavePath};
use hw_core::game_state::PlayMode;
use hw_core::world::DoorState;
use hw_energy::{
    PowerConsumer, PowerConsumerPolicy, PowerConsumerPolicyChangeOutcome,
    PowerConsumerPolicyChangeStatus, PowerPriority, SoulSpaConstructionCancelOutcome,
    SoulSpaConstructionCancelRequest, SoulSpaConstructionCancelResult, SoulSpaPhase, SoulSpaSite,
    SoulSpaSlotsChangeOutcome, SoulSpaSlotsChangeStatus,
};
use hw_jobs::{Building, BuildingCategory, DeconstructionPending, Door};
use hw_logistics::{StockpilePolicyChangeRequest, StockpilePolicyPatch};
use hw_spatial::StockpileSpatialGrid;
use hw_ui::components::{ArchitectCategoryState, LoadConfirmDialog, OperationDialog};
use hw_ui::intents::StockpilePolicyEditTarget;
use hw_ui::power::PowerPriorityValue;
use hw_world::{DoorVisualHandles, WorldMap, WorldMapWrite, apply_door_state};

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

/// Domain-side validation and mutation used by the generic UI intent handler.
///
/// This is placed in a `ParamSet` with `IntentModeCtx`: both need WorldMap and
/// mutable Sprite access, but individual intents borrow only one side at a
/// time.
#[derive(SystemParam)]
pub(crate) struct IntentDomainActionCtx<'w, 's> {
    architect_category: ResMut<'w, ArchitectCategoryState>,
    q_buildings: Query<'w, 's, &'static Building, Without<DeconstructionPending>>,
    q_doors: Query<'w, 's, (&'static Transform, &'static mut Door, &'static mut Sprite)>,
    world_map: WorldMapWrite<'w>,
    door_visual_handles: Res<'w, DoorVisualHandles>,
    stockpile_grid: Res<'w, StockpileSpatialGrid>,
    stockpile_policy_requests: MessageWriter<'w, StockpilePolicyChangeRequest>,
    soul_spa_slot_outcomes: MessageWriter<'w, SoulSpaSlotsChangeOutcome>,
    soul_spa_cancel_requests: MessageWriter<'w, SoulSpaConstructionCancelRequest>,
    soul_spa_cancel_outcomes: MessageWriter<'w, SoulSpaConstructionCancelOutcome>,
    power_consumer_policy_outcomes: MessageWriter<'w, PowerConsumerPolicyChangeOutcome>,
    q_soul_spas: Query<'w, 's, &'static mut SoulSpaSite>,
    q_power_consumers: Query<'w, 's, Option<&'static mut PowerConsumerPolicy>, With<PowerConsumer>>,
    q_entities: Query<'w, 's, ()>,
}

impl IntentDomainActionCtx<'_, '_> {
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
            .is_ok_and(|building| building.kind.category() == BuildingCategory::Plant)
    }

    pub(crate) fn toggle_door_lock(&mut self, entity: Entity) {
        let Ok((transform, mut door, mut sprite)) = self.q_doors.get_mut(entity) else {
            return;
        };
        let door_grid = WorldMap::world_to_grid(transform.translation.truncate());
        let next_state = if door.state == DoorState::Locked {
            DoorState::Closed
        } else {
            DoorState::Locked
        };
        apply_door_state(
            &mut door,
            &mut sprite,
            &mut self.world_map,
            &self.door_visual_handles,
            door_grid,
            next_state,
        );
    }

    pub(crate) fn request_stockpile_policy_change(
        &mut self,
        target: StockpilePolicyEditTarget,
        patch: StockpilePolicyPatch,
    ) {
        let targets =
            crate::systems::command::resolve_stockpile_policy_targets(target, &self.stockpile_grid);
        self.stockpile_policy_requests
            .write(StockpilePolicyChangeRequest { targets, patch });
    }

    pub(crate) fn set_soul_spa_active_slots(&mut self, target: Entity, requested: u32) {
        let status = match self.q_soul_spas.get_mut(target) {
            Ok(site) if site.phase != SoulSpaPhase::Operational => {
                SoulSpaSlotsChangeStatus::PhaseUnavailable
            }
            Ok(mut site) => {
                let applied = SoulSpaSite::clamped_active_slots(requested);
                if site.active_slots != applied {
                    site.set_active_slots(applied);
                }
                SoulSpaSlotsChangeStatus::Applied {
                    requested,
                    applied,
                    clamped: requested != applied,
                }
            }
            Err(_) if self.q_entities.get(target).is_ok() => {
                SoulSpaSlotsChangeStatus::UnsupportedTarget
            }
            Err(_) => SoulSpaSlotsChangeStatus::StaleTarget,
        };
        self.soul_spa_slot_outcomes
            .write(SoulSpaSlotsChangeOutcome { target, status });
    }

    pub(crate) fn cancel_soul_spa_construction(&mut self, target: Entity, paused: bool) {
        if paused {
            self.soul_spa_cancel_outcomes
                .write(SoulSpaConstructionCancelOutcome {
                    target,
                    result: SoulSpaConstructionCancelResult::Paused,
                });
        } else {
            self.soul_spa_cancel_requests
                .write(SoulSpaConstructionCancelRequest { target });
        }
    }

    pub(crate) fn set_power_consumer_priority(
        &mut self,
        target: Entity,
        requested: PowerPriorityValue,
    ) {
        let requested = match requested {
            PowerPriorityValue::Low => PowerPriority::Low,
            PowerPriorityValue::Normal => PowerPriority::Normal,
            PowerPriorityValue::High => PowerPriority::High,
        };
        let status = match self.q_power_consumers.get_mut(target) {
            Ok(Some(mut policy)) => {
                let previous = policy.priority;
                if previous != requested {
                    policy.priority = requested;
                }
                PowerConsumerPolicyChangeStatus::Applied {
                    previous,
                    applied: requested,
                }
            }
            Ok(None) => PowerConsumerPolicyChangeStatus::MissingPolicy,
            Err(_) if self.q_entities.get(target).is_ok() => {
                PowerConsumerPolicyChangeStatus::UnsupportedTarget
            }
            Err(_) => PowerConsumerPolicyChangeStatus::StaleTarget,
        };
        self.power_consumer_policy_outcomes
            .write(PowerConsumerPolicyChangeOutcome { target, status });
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
    pub(crate) q_dialog:
        Query<'w, 's, &'static mut Node, (With<OperationDialog>, Without<LoadConfirmDialog>)>,
    pub(crate) q_load_confirm:
        Query<'w, 's, &'static mut Node, (With<LoadConfirmDialog>, Without<OperationDialog>)>,
    pub(crate) q_operation_scroll:
        Query<'w, 's, &'static mut ScrollPosition, With<hw_ui::components::OperationDialogScroll>>,
    pub(crate) input_focus: ResMut<'w, InputFocus>,
    pub(crate) save_load_state: ResMut<'w, SaveLoadState>,
    pub(crate) save_path: Res<'w, SavePath>,
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
