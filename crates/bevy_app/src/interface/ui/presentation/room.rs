use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_jobs::{Building, RoomDetectionRole};
use hw_ui::models::inspection::EntityInspectionModel;
use hw_world::room_detection::{RoomFailureReason, build_detection_input, inspect_room};

#[derive(Default)]
pub(super) struct RoomInspectionCache {
    seed: Option<(i32, i32)>,
    text: String,
}

type ChangedBuildings<'w, 's> =
    Query<'w, 's, (), (With<Building>, Or<(Changed<Building>, Changed<Transform>)>)>;

#[derive(SystemParam)]
pub struct RoomInspectionChanges<'w, 's> {
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
        removed || !self.changed.is_empty()
    }
}

#[derive(SystemParam)]
pub struct RoomInspection<'w, 's> {
    buildings: Query<'w, 's, (Entity, &'static Building, &'static Transform)>,
    changed: ChangedBuildings<'w, 's>,
    existing: Query<'w, 's, (), With<Building>>,
    removed: RemovedComponents<'w, 's, Building>,
    removed_transforms: RemovedComponents<'w, 's, Transform>,
    cache: Local<'s, RoomInspectionCache>,
}

impl RoomInspection<'_, '_> {
    pub(super) fn append(&mut self, model: Option<&mut EntityInspectionModel>) {
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
            self.cache.text = seed.map_or_else(String::new, |seed| {
                let tiles = hw_world::room_systems::collect_building_tiles(&self.buildings);
                match inspect_room(seed, &build_detection_input(&tiles)) {
                    Ok(room) => format!(
                        "部屋の条件: 成立（床 {}、扉 {}）",
                        room.tiles.len(),
                        room.door_tiles.len()
                    ),
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
        }
        if let Some(model) = model
            && !self.cache.text.is_empty()
        {
            if !model.common_text.is_empty() {
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
        app.world_mut().despawn(wall);
        app.update();
        assert!(text(&app).contains("不成立"));
        assert!(text(&app).contains("(4, 5)"));
        spawn(&mut app, hw_jobs::BuildingType::Wall, 4, 5);
        app.update();
        assert!(text(&app).contains("成立（床 1、扉 1）"));
    }
}
