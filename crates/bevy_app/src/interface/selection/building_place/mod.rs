//! Building placement system (root shell)
//!
//! Root shell: entity spawn + `WorldMap` occupancy update + `GameAssets` + `BuildContext` に依存。
//! hw_ui / hw_jobs crate への移設には WorldMap / GameAssets の抽象化が必要であり、
//! 現段階では意図的に root に残す。純バリデーション API は hw_ui::selection::placement を参照。
mod companion;
mod flow;
#[cfg(feature = "profiling")]
pub(crate) mod observed_outcomes;
mod placement;

use crate::app_contexts::{
    BuildContext, CompanionParentKind, CompanionPlacementKind, CompanionPlacementState,
};
use crate::assets::GameAssets;
use crate::interface::ui::UiInputState;
use crate::systems::jobs::{Blueprint, Building, BuildingType};
use crate::world::map::{RIVER_Y_MIN, WorldMap, WorldMapWrite};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::time::Real;
use hw_ui::camera::MainCamera;
use hw_ui::selection::{PlacementFeedbackState, PlacementTileRejection};
use hw_world::zones::{Site, Yard};

use super::placement_geometry::building_spawn_pos;
use companion::make_companion_placement;
use flow::handle_companion_flow;
#[cfg(feature = "profiling")]
pub(crate) use placement::try_place_bucket_storage_companion;
use placement::{place_building_blueprint, validate_building_blueprint_placement};

#[derive(SystemParam)]
pub struct BuildPlaceInput<'w, 's> {
    keys: Res<'w, ButtonInput<KeyCode>>,
    pub buttons: Res<'w, ButtonInput<MouseButton>>,
    pub q_window: Query<'w, 's, &'static Window, With<bevy::window::PrimaryWindow>>,
    pub q_camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<MainCamera>>,
    pub ui_input_state: Res<'w, UiInputState>,
}

type DemolitionFilter = Or<(
    With<hw_jobs::DeconstructionPending>,
    With<hw_jobs::DeconstructionOrders>,
)>;

#[derive(SystemParam)]
pub(crate) struct WallRetrofitEligibility<'w, 's> {
    rooms: Option<Res<'w, hw_world::RoomTileLookup>>,
    demolition: Query<'w, 's, (), DemolitionFilter>,
    wall_tiles: Query<'w, 's, &'static hw_jobs::construction::WallTileBlueprint>,
}

impl WallRetrofitEligibility<'_, '_> {
    pub(crate) fn room_check(
        &self,
        kind: BuildingType,
        grid: (i32, i32),
    ) -> Option<hw_ui::selection::PlacementValidation> {
        use hw_ui::selection::{PlacementRejectReason, PlacementValidation};
        (matches!(kind,BuildingType::Equipment(e) if e.indoor())
            && !self
                .rooms
                .as_ref()
                .is_some_and(|r| r.tile_to_room.contains_key(&grid)))
        .then(|| PlacementValidation::rejected_at(PlacementRejectReason::RequiresRoomFloor, grid))
    }
    pub(crate) fn available(&self, owner: Option<Entity>) -> bool {
        owner.is_some_and(|wall| {
            !self.demolition.contains(wall)
                && !self
                    .wall_tiles
                    .iter()
                    .any(|tile| tile.spawned_wall == Some(wall))
        })
    }
}

#[derive(SystemParam)]
pub struct BuildingStateQueries<'w, 's> {
    retrofit: WallRetrofitEligibility<'w, 's>,
    settings: Query<'w, 's, &'static crate::systems::ventilation::climate::ClimateSetting>,
    pub q_blueprints_by_entity: Query<'w, 's, &'static Blueprint>,
    pub q_sites: Query<'w, 's, &'static Site>,
    pub q_yards: Query<'w, 's, &'static Yard>,
    pub q_buildings: Query<'w, 's, &'static Building>,
}

#[derive(SystemParam)]
pub struct BuildPlaceState<'w, 's> {
    pub build_context: ResMut<'w, BuildContext>,
    contracts: Query<'w, 's, &'static mut crate::systems::waterworks::WaterworksContract>,
    targets: Query<'w, 's, (), With<crate::systems::waterworks::WaterworksTarget>>,
    next_mode: ResMut<'w, NextState<hw_core::game_state::PlayMode>>,
    pub companion_state: ResMut<'w, CompanionPlacementState>,
    pub game_assets: Res<'w, GameAssets>,
    pub real_time: Res<'w, Time<Real>>,
    pub placement_feedback: ResMut<'w, PlacementFeedbackState>,
}

pub(super) struct PlacementQueries<'a, 'w, 's> {
    retrofit: &'a WallRetrofitEligibility<'w, 's>,
    pub q_buildings: &'a Query<'w, 's, &'static Building>,
    pub q_blueprints_by_entity: &'a Query<'w, 's, &'static Blueprint>,
    pub q_sites: &'a Query<'w, 's, &'static Site>,
    pub q_yards: &'a Query<'w, 's, &'static Yard>,
}

fn record_placement_outcome(
    feedback: &mut PlacementFeedbackState,
    outcome: Result<(), PlacementTileRejection>,
    attempted_grid: (i32, i32),
    now: std::time::Duration,
) {
    match outcome {
        Ok(()) => feedback.block_live_feedback_at(attempted_grid),
        Err(rejection) => {
            feedback.show_recent_rejection(rejection.reason, rejection.grid, now);
        }
    }
}

pub fn blueprint_placement(
    input: BuildPlaceInput,
    mut world_map: WorldMapWrite,
    mut state: BuildPlaceState,
    queries: BuildingStateQueries,
    mut commands: Commands,
    mut last_pipe: Local<Option<(i32, i32)>>,
) {
    if input.ui_input_state.world_input_blocked() {
        *last_pipe = None;
        return;
    }

    let pipe =
        state.build_context.kind == Some(BuildingType::Equipment(hw_jobs::Equipment::Conduit));
    let started = input.buttons.just_pressed(MouseButton::Left);
    let dragging_pipe = pipe && input.buttons.pressed(MouseButton::Left);
    if started || !dragging_pipe {
        *last_pipe = None;
    }
    if !(started || dragging_pipe) {
        return;
    }

    let Some(world_pos) = hw_ui::camera::world_cursor_pos(&input.q_window, &input.q_camera) else {
        return;
    };
    let grid = WorldMap::world_to_grid(world_pos);
    let now = state.real_time.elapsed();

    let pq = PlacementQueries {
        retrofit: &queries.retrofit,
        q_buildings: &queries.q_buildings,
        q_blueprints_by_entity: &queries.q_blueprints_by_entity,
        q_sites: &queries.q_sites,
        q_yards: &queries.q_yards,
    };

    if pipe {
        let start = last_pipe.unwrap_or(grid);
        if *last_pipe == Some(grid) {
            return;
        }
        *last_pipe = Some(grid);
        let erase =
            input.keys.pressed(KeyCode::ShiftLeft) || input.keys.pressed(KeyCode::ShiftRight);
        let kind = BuildingType::Equipment(hw_jobs::Equipment::Conduit);
        let mut tile = start;
        loop {
            if erase {
                if let Some(&target) = world_map.conduits.get(&tile) {
                    if queries.q_blueprints_by_entity.get(target).is_ok() {
                        commands
                            .entity(target)
                            .try_insert(hw_jobs::BlueprintCancelRequested);
                    } else if queries
                        .q_buildings
                        .get(target)
                        .is_ok_and(|b| b.kind == kind)
                    {
                        world_map.conduits.remove(&tile);
                        commands.entity(target).try_despawn();
                    }
                }
            } else if !world_map.conduits.contains_key(&tile) {
                let result = place_building_blueprint(
                    &mut commands,
                    &mut world_map,
                    &state.game_assets,
                    kind,
                    tile,
                    &pq,
                );
                record_placement_outcome(
                    &mut state.placement_feedback,
                    result.map(|_| ()),
                    tile,
                    now,
                );
            }
            if tile == grid {
                break;
            }
            if tile.0 != grid.0 {
                tile.0 += (grid.0 - tile.0).signum();
            } else {
                tile.1 += (grid.1 - tile.1).signum();
            }
        }
        return;
    }
    if let Some(
        kind @ BuildingType::Equipment(
            hw_jobs::Equipment::SupplyFan
            | hw_jobs::Equipment::ExhaustFan
            | hw_jobs::Equipment::Hood,
        ),
    ) = state.build_context.kind
        && let Some(target) = world_map.building_entity(grid)
        && queries
            .q_buildings
            .get(target)
            .is_ok_and(|b| b.kind == kind && !b.is_provisional)
    {
        let setting = queries.settings.get(target).copied().unwrap_or_default();
        commands
            .entity(target)
            .insert(crate::systems::ventilation::climate::ClimateSetting(
                (setting.0 + 1) % 3,
            ));
        return;
    }
    if let Some(district) = state.build_context.contract_district {
        if !state.targets.is_empty()
            || !state.contracts.single().is_ok_and(|record| {
                record.outcome == crate::systems::waterworks::ContractOutcome::Active
                    && record.district == district
            })
        {
            return;
        }
        let geometry = if state.companion_state.0.is_some() {
            super::placement_geometry::bucket_storage_geometry(grid)
        } else {
            super::placement_geometry::live_building_geometry(&world_map, BuildingType::Tank, grid)
        };
        if let Some(rejection) = district.validate(&geometry.occupied_grids).rejection(grid) {
            record_placement_outcome(&mut state.placement_feedback, Err(rejection), grid, now);
            return;
        }
    }

    // companion 配置中は通常建築を抑止
    if let Some(result) = handle_companion_flow(
        &mut state.companion_state,
        &mut commands,
        &mut world_map,
        &state.game_assets,
        &pq,
        world_pos,
        grid,
    ) {
        if let Ok(entity) = result
            && state.build_context.contract_district.is_some()
            && let Ok(mut contract) = state.contracts.single_mut()
        {
            contract.designation = contract.designation.checked_add(1).unwrap_or(1);
            commands
                .entity(entity)
                .insert(crate::systems::waterworks::WaterworksTarget(
                    contract.designation,
                ));
            *state.build_context = BuildContext::default();
            state.next_mode.set(hw_core::game_state::PlayMode::Normal);
        }
        record_placement_outcome(&mut state.placement_feedback, result.map(|_| ()), grid, now);
        return;
    }

    let Some(building_type) = state.build_context.kind else {
        return;
    };
    let spawn_pos = building_spawn_pos(building_type, grid, RIVER_Y_MIN);

    if building_type == BuildingType::Tank {
        let validation =
            validate_building_blueprint_placement(&world_map, building_type, grid, &pq);
        if !validation.can_place {
            let rejection = validation
                .rejection(grid)
                .expect("rejected Tank placement must carry a reason");
            #[cfg(feature = "profiling")]
            observed_outcomes::record(
                &mut commands,
                serde_json::json!({"kind": "Tank",
                "grid": grid, "status": "Rejected", "reason": format!("{:?}", rejection.reason)}),
            );
            record_placement_outcome(&mut state.placement_feedback, Err(rejection), grid, now);
            return;
        }
        state.companion_state.0 = Some(make_companion_placement(
            CompanionParentKind::Tank,
            grid,
            CompanionPlacementKind::BucketStorage,
            spawn_pos,
        ));
        #[cfg(feature = "profiling")]
        observed_outcomes::record(
            &mut commands,
            serde_json::json!({"kind": "Tank",
            "grid": grid, "status": "CompanionPending"}),
        );
        record_placement_outcome(&mut state.placement_feedback, Ok(()), grid, now);
    } else {
        let result = place_building_blueprint(
            &mut commands,
            &mut world_map,
            &state.game_assets,
            building_type,
            grid,
            &pq,
        );
        #[cfg(feature = "profiling")]
        if matches!(
            building_type,
            BuildingType::MudMixer
                | BuildingType::RestArea
                | BuildingType::Door
                | BuildingType::WheelbarrowParking
                | BuildingType::SandPile
                | BuildingType::BonePile
                | BuildingType::OutdoorLamp
        ) {
            let row = match &result {
                Ok((owner, footprint, _)) => {
                    serde_json::json!({"kind": format!("{building_type:?}"),
                    "grid": grid, "status": "BlueprintCommitted", "owner": format!("{owner:?}"),
                    "footprint": footprint})
                }
                Err(rejection) => serde_json::json!({"kind": format!("{building_type:?}"),
                    "grid": grid, "status": "Rejected", "reason": format!("{:?}", rejection.reason)}),
            };
            observed_outcomes::record(&mut commands, row);
        }
        record_placement_outcome(&mut state.placement_feedback, result.map(|_| ()), grid, now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_ui::selection::{PlacementRejectReason, PlacementValidation};

    #[test]
    fn successful_building_outcome_arms_same_anchor_feedback_blocker() {
        let anchor = (3, 4);
        let mut feedback = PlacementFeedbackState::default();

        record_placement_outcome(&mut feedback, Ok(()), anchor, std::time::Duration::ZERO);
        feedback.set_live_building_validation(
            &PlacementValidation::rejected(PlacementRejectReason::OccupiedByBuilding),
            anchor,
        );

        assert!(feedback.visible(std::time::Duration::ZERO).is_none());
    }

    #[test]
    fn rejected_building_outcome_remains_visible() {
        let anchor = (3, 4);
        let mut feedback = PlacementFeedbackState::default();

        record_placement_outcome(
            &mut feedback,
            Err(PlacementTileRejection {
                grid: anchor,
                reason: PlacementRejectReason::OccupiedByBuilding,
            }),
            anchor,
            std::time::Duration::ZERO,
        );

        assert_eq!(
            feedback.visible(std::time::Duration::ZERO).unwrap().reason,
            PlacementRejectReason::OccupiedByBuilding
        );
    }
}
