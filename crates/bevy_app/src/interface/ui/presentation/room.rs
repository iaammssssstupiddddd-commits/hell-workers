use crate::systems::ventilation::climate::{ClimateSetting, DeviceReading, RoomClimate};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_jobs::{Building, RoomDetectionRole};
use hw_ui::models::inspection::EntityInspectionModel;
use hw_world::room_detection::{RoomFailureReason, build_detection_input, inspect_room};

#[derive(Default)]
pub(super) struct RoomInspectionCache {
    seed: Option<(i32, i32)>,
    text: String,
    enclosed: bool,
}

type ChangedBuildings<'w, 's> =
    Query<'w, 's, (), (With<Building>, Or<(Changed<Building>, Changed<Transform>)>)>;

type ClimateInspectionChanged = Or<(
    Changed<RoomClimate>,
    Changed<DeviceReading>,
    Changed<ClimateSetting>,
)>;

#[derive(SystemParam)]
pub struct RoomInspectionChanges<'w, 's> {
    climate: Query<'w, 's, (), ClimateInspectionChanged>,
    ventilation: Query<'w, 's, (), Changed<crate::systems::ventilation::RoomVentilation>>,
    vents: Query<'w, 's, (), Changed<crate::systems::ventilation::VentReading>>,
    tiles: Option<Res<'w, hw_world::RoomTileLookup>>,
    boundaries: Option<Res<'w, hw_world::RoomBoundaryLookup>>,
    changed: ChangedBuildings<'w, 's>,
    existing: Query<'w, 's, (), With<Building>>,
    removed: RemovedComponents<'w, 's, Building>,
    removed_transforms: RemovedComponents<'w, 's, Transform>,
}

impl RoomInspectionChanges<'_, '_> {
    pub fn changed(&mut self) -> bool {
        let removed = self.removed.read().count() > 0;
        let removed = self
            .removed_transforms
            .read()
            .filter(|entity| self.existing.contains(*entity))
            .count()
            > 0
            || removed;
        removed
            || !self.changed.is_empty()
            || !self.climate.is_empty()
            || !self.ventilation.is_empty()
            || !self.vents.is_empty()
            || self.boundaries.as_ref().is_some_and(|b| b.is_changed())
            || self.tiles.as_ref().is_some_and(|tiles| tiles.is_changed())
    }
}

#[derive(SystemParam)]
pub struct RoomInspection<'w, 's> {
    #[cfg(feature = "profiling")]
    probe: Option<ResMut<'w, crate::systems::ventilation::perf::AirPerf>>,
    climate: Query<'w, 's, &'static RoomClimate>,
    devices: Query<'w, 's, (&'static DeviceReading, Option<&'static ClimateSetting>)>,
    detection: Option<Res<'w, hw_world::RoomDetectionState>>,
    vents: Query<'w, 's, &'static crate::systems::ventilation::VentReading>,
    ventilation: Query<'w, 's, &'static crate::systems::ventilation::RoomVentilation>,
    tiles: Option<Res<'w, hw_world::RoomTileLookup>>,
    boundaries: Option<Res<'w, hw_world::RoomBoundaryLookup>>,
    buildings: Query<'w, 's, (Entity, &'static Building, &'static Transform)>,
    changed: ChangedBuildings<'w, 's>,
    existing: Query<'w, 's, (), With<Building>>,
    removed: RemovedComponents<'w, 's, Building>,
    removed_transforms: RemovedComponents<'w, 's, Transform>,
    cache: Local<'s, RoomInspectionCache>,
}

impl RoomInspection<'_, '_> {
    pub(super) fn append(
        &mut self,
        model: &mut Option<EntityInspectionModel>,
        _pin: Option<Entity>,
    ) {
        #[cfg(feature = "profiling")]
        let start = self
            .probe
            .as_ref()
            .map(|_| crate::systems::ventilation::perf::cpu_time_ns(false));
        self.append_content(model.as_mut());
        #[cfg(feature = "profiling")]
        if let Some((probe, start)) = self.probe.as_mut().zip(start) {
            probe.inspection_ns += crate::systems::ventilation::perf::cpu_time_ns(false) - start;
            if model.as_ref().is_some_and(|m| {
                Some(m.entity) == probe.pinned_vent
                    && _pin == probe.pinned_vent
                    && m.common_text.contains("外気経路あり")
            }) {
                probe.inspections += 1;
            }
        }
    }

    fn append_content(&mut self, model: Option<&mut EntityInspectionModel>) {
        let removed = self.removed.read().count() > 0;
        let removed = self
            .removed_transforms
            .read()
            .filter(|entity| self.existing.contains(*entity))
            .count()
            > 0
            || removed;
        let seed = model.as_ref().and_then(|model| {
            let (_, building, transform) = self.buildings.get(model.entity).ok()?;
            (building.kind.room_detection_role(building.is_provisional) == RoomDetectionRole::Floor)
                .then(|| hw_world::WorldMap::world_to_grid(transform.translation.truncate()))
        });
        if seed != self.cache.seed || removed || !self.changed.is_empty() {
            self.cache.seed = seed;
            let mut enclosed = false;
            self.cache.text = seed.map_or_else(String::new, |seed| {
                let tiles = hw_world::room_systems::collect_building_tiles(&self.buildings);
                match inspect_room(seed, &build_detection_input(&tiles)) {
                    Ok(room) => {
                        enclosed = true;
                        format!(
                            "部屋の条件: 成立（床 {}、扉 {}）",
                            room.tiles.len(),
                            room.door_tiles.len()
                        )
                    }
                    Err(failure) => {
                        let reason = match failure.reason {
                            RoomFailureReason::NoFloor => {
                                "有効な床がない。床上の障害物・未完成箇所を確認"
                            }
                            RoomFailureReason::TooLarge => {
                                "床の連結範囲が上限を超えている。壁で区切る"
                            }
                            RoomFailureReason::MapBoundary => {
                                "マップ外へ開いている。内側を壁で囲む"
                            }
                            RoomFailureReason::OpenBoundary => {
                                "境界が開いている。該当位置に壁または扉を設置"
                            }
                            RoomFailureReason::NoDoor => "扉がない。境界に扉を設置",
                        };
                        format!(
                            "部屋の条件: 不成立\n{reason}\n確認位置: ({}, {})",
                            failure.grid.0, failure.grid.1
                        )
                    }
                }
            });
            self.cache.enclosed = enclosed;
        }
        let Some(model) = model else { return };
        let Ok((_, building, transform)) = self.buildings.get(model.entity) else {
            return;
        };
        if let hw_jobs::BuildingType::Equipment(kind) = building.kind {
            if let Ok((reading, setting)) = self.devices.get(model.entity) {
                model
                    .common_text
                    .push_str(&format!("\n{}　{}", kind.copy().0, reading.0));
                if matches!(
                    kind,
                    hw_jobs::Equipment::SupplyFan
                        | hw_jobs::Equipment::ExhaustFan
                        | hw_jobs::Equipment::Hood
                ) {
                    model.common_text.push_str(&format!(
                        "\n風量設定 {}（同じ建築カードで本体をクリックして切替）",
                        setting.copied().unwrap_or_default().label()
                    ));
                }
                let grid = hw_world::WorldMap::world_to_grid(transform.translation.xy());
                if let Some(room) = self.tiles.as_ref().and_then(|t| t.tile_to_room.get(&grid))
                    && let Ok(climate) = self.climate.get(*room)
                {
                    model.common_text.push_str(&format!("\n{}", climate.text()));
                }
            }
            return;
        }
        if building.kind == hw_jobs::BuildingType::ScreamingVent && !building.is_provisional {
            let (x, y) = hw_world::WorldMap::world_to_grid(transform.translation.truncate());
            if self
                .detection
                .as_ref()
                .is_some_and(|state| !state.dirty_tiles.is_empty())
            {
                model.common_text.push_str("自然換気　部屋を再判定中");
                return;
            }
            let rooms = self
                .boundaries
                .as_deref()
                .map_or(&[][..], |b| b.rooms_at((x, y)));
            let Some(reading) = self.vents.get(model.entity).ok() else {
                model.common_text.push_str("自然換気　部屋を再判定中");
                return;
            };
            use std::fmt::Write;
            write!(model.common_text, "{reading}").expect("writing to a String");
            if rooms.is_empty() {
                model.common_text.push_str("\n隣接する成立室なし");
                return;
            }
            let Some(tiles) = self.tiles.as_deref() else {
                return;
            };
            let mut shown = Vec::new();
            for (direction, adjacent) in [
                ("西", (x - 1, y)),
                ("東", (x + 1, y)),
                ("北", (x, y + 1)),
                ("南", (x, y - 1)),
            ] {
                if let Some(room) = tiles.tile_to_room.get(&adjacent)
                    && rooms.contains(room)
                    && !shown.contains(room)
                    && let Ok(ventilation) = self.ventilation.get(*room)
                {
                    shown.push(*room);
                    model.common_text.push_str(&format!(
                        "\n{direction}の部屋　{}\n{}",
                        ventilation.state_label(),
                        ventilation.balance()
                    ));
                }
            }
        } else if !self.cache.text.is_empty() {
            if !model.common_text.is_empty() {
                model.common_text.push('\n');
            }
            if self.cache.enclosed
                && let Some(seed) = seed
                && let Some(room) = self
                    .tiles
                    .as_ref()
                    .and_then(|tiles| tiles.tile_to_room.get(&seed))
                && let Ok(ventilation) = self.ventilation.get(*room)
            {
                model.common_text.push_str(&ventilation.description());
                if let Ok(climate) = self.climate.get(*room) {
                    model.common_text.push_str(&format!("\n{}", climate.text()));
                }
                model.common_text.push('\n');
            }
            model.common_text.push_str(&self.cache.text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface::selection::SelectedEntity;
    use hw_ui::models::inspection::EntityInspectionViewModel;
    use hw_ui::panels::info_panel::InfoPanelPinState;

    #[test]
    fn pinned_and_selected_vent_reads_current_flow_after_reconnection() {
        use crate::systems::ventilation::VentReading;
        use hw_infra::ventilation::OpeningFlow;
        let mut app = crate::test_support::minimal_app();
        app.init_resource::<SelectedEntity>()
            .init_resource::<InfoPanelPinState>()
            .init_resource::<EntityInspectionViewModel>()
            .init_resource::<hw_spatial::FamiliarSpatialGrid>()
            .add_systems(
                Update,
                super::super::update_entity_inspection_view_model_system,
            );
        let e = app
            .world_mut()
            .spawn((
                Building {
                    kind: hw_jobs::BuildingType::ScreamingVent,
                    is_provisional: false,
                },
                Transform::default(),
                VentReading::Unconnected,
            ))
            .id();
        app.world_mut().resource_mut::<InfoPanelPinState>().entity = Some(e);
        let text = |app: &App| {
            app.world()
                .resource::<EntityInspectionViewModel>()
                .model
                .as_ref()
                .unwrap()
                .common_text
                .clone()
        };
        app.update();
        assert!(text(&app).contains("通風面が未接続"));
        for (net, line) in [
            (0.004, "双方向交換　各 0.20"),
            (-0.125, "通風 東→西 0.12"),
            (0.25, "通風 西→東 0.25"),
        ] {
            app.world_mut().entity_mut(e).insert(VentReading::Passage {
                kind: "外壁開口",
                a: "西",
                b: "東",
                flow: OpeningFlow {
                    net,
                    exchange_each_way: 0.2,
                    connected_to_outdoors: true,
                },
            });
            app.update();
            assert!(text(&app).contains(line));
        }
        app.world_mut().resource_mut::<InfoPanelPinState>().entity = None;
        app.world_mut().resource_mut::<SelectedEntity>().0 = Some(e);
        app.world_mut()
            .entity_mut(e)
            .insert(VentReading::Unconnected);
        app.update();
        assert!(text(&app).contains("通風面が未接続"));
    }

    #[test]
    fn floor_inspection_refreshes_after_boundary_removal_without_room_entities() {
        let mut app = crate::test_support::minimal_app();
        app.init_resource::<SelectedEntity>()
            .init_resource::<InfoPanelPinState>()
            .init_resource::<EntityInspectionViewModel>()
            .init_resource::<hw_spatial::FamiliarSpatialGrid>()
            .add_systems(
                Update,
                super::super::update_entity_inspection_view_model_system,
            );
        let spawn = |app: &mut App, kind, x, y| {
            app.world_mut()
                .spawn((
                    Building {
                        kind,
                        is_provisional: false,
                    },
                    Transform::from_translation(
                        hw_world::WorldMap::grid_to_world(x, y).extend(0.0),
                    ),
                ))
                .id()
        };
        let floor = spawn(&mut app, hw_jobs::BuildingType::Floor, 5, 5);
        let wall = spawn(&mut app, hw_jobs::BuildingType::Wall, 4, 5);
        spawn(&mut app, hw_jobs::BuildingType::Wall, 6, 5);
        spawn(&mut app, hw_jobs::BuildingType::Wall, 5, 4);
        spawn(&mut app, hw_jobs::BuildingType::Door, 5, 6);
        app.world_mut().resource_mut::<SelectedEntity>().0 = Some(floor);
        app.update();
        let text = |app: &App| {
            app.world()
                .resource::<EntityInspectionViewModel>()
                .model
                .as_ref()
                .unwrap()
                .common_text
                .clone()
        };
        assert!(text(&app).contains("成立（床 1、扉 1）"));
        app.update();
        assert!(text(&app).contains("成立（床 1、扉 1）"));
        // Room detection may still have the old lookup while the simulation is paused.
        let old = app
            .world_mut()
            .spawn(crate::systems::ventilation::RoomVentilation(
                hw_infra::ventilation::Ventilation::for_floor_tiles(1),
            ))
            .id();
        app.init_resource::<hw_world::RoomTileLookup>()
            .world_mut()
            .resource_mut::<hw_world::RoomTileLookup>()
            .tile_to_room
            .insert((5, 5), old);
        app.update();
        assert!(text(&app).contains("外気交換なし"));
        app.world_mut().despawn(wall);
        app.update();
        assert!(text(&app).contains("不成立"));
        assert!(!text(&app).contains("自然換気"));
        assert!(text(&app).contains("(4, 5)"));
        spawn(&mut app, hw_jobs::BuildingType::Wall, 4, 5);
        app.update();
        assert!(text(&app).contains("成立（床 1、扉 1）"));
    }
}
