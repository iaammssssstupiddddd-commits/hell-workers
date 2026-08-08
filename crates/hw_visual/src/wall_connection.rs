use crate::handles::WallVisualHandles;
use crate::layer::VisualLayerKind;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::visual_mirror::building::{BuildingTypeVisual, BuildingVisualState};
use hw_core::visual_mirror::construction::BlueprintVisualState;
use hw_world::{WorldMap, WorldMapRead};
use std::collections::HashSet;

/// Runtime wake-up for wall/door removals whose visual mirror disappears
/// before the regular `Changed<BuildingVisualState>` query can observe it.
#[derive(Resource, Debug, Default)]
pub struct WallConnectionDirty {
    removed_grids: HashSet<(i32, i32)>,
}

impl WallConnectionDirty {
    pub fn mark_removed<I>(&mut self, grids: I)
    where
        I: IntoIterator<Item = (i32, i32)>,
    {
        self.removed_grids.extend(grids);
    }

    fn take_removed(&mut self) -> HashSet<(i32, i32)> {
        std::mem::take(&mut self.removed_grids)
    }
}

type ChangedBuildingQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Transform, &'static BuildingVisualState),
    Or<(Added<BuildingVisualState>, Changed<BuildingVisualState>)>,
>;

type WallCheckQuery<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static BuildingVisualState>,
        Option<&'static BlueprintVisualState>,
    ),
    Or<(With<BuildingVisualState>, With<BlueprintVisualState>)>,
>;

#[derive(SystemParam)]
pub struct WallConnectionQueries<'w, 's> {
    q_children: Query<'w, 's, &'static Children>,
    q_visual_layers: Query<'w, 's, (&'static VisualLayerKind, &'static mut Sprite)>,
    q_blueprint_sprites: Query<'w, 's, &'static mut Sprite, Without<VisualLayerKind>>,
}

/// 壁の接続更新を行うシステム
pub fn wall_connections_system(
    wall_handles: Res<WallVisualHandles>,
    world_map: WorldMapRead,
    q_new_buildings: ChangedBuildingQuery,
    q_new_blueprints: Query<
        (Entity, &Transform, &BlueprintVisualState),
        Added<BlueprintVisualState>,
    >,
    q_walls_check: WallCheckQuery,
    mut dirty: ResMut<WallConnectionDirty>,
    mut queries: WallConnectionQueries,
) {
    let mut update_targets = HashSet::new();

    for (x, y) in dirty.take_removed() {
        add_neighbors_to_update(x, y, &mut update_targets);
    }

    for (_entity, transform, building_visual) in q_new_buildings.iter() {
        if matches!(
            building_visual.kind,
            BuildingTypeVisual::Wall | BuildingTypeVisual::Door
        ) {
            let (x, y) = WorldMap::world_to_grid(transform.translation.truncate());
            add_neighbors_to_update(x, y, &mut update_targets);
        }
    }

    for (_entity, _transform, state) in q_new_blueprints.iter() {
        if state.is_wall_or_door {
            for &(gx, gy) in &state.occupied_grids {
                add_neighbors_to_update(gx, gy, &mut update_targets);
            }
        }
    }

    if update_targets.is_empty() {
        return;
    }

    for (gx, gy) in update_targets {
        if let Some(entity) = world_map.building_entity((gx, gy))
            && is_wall(gx, gy, world_map.as_ref(), &q_walls_check)
        {
            let is_plain_wall = q_walls_check.get(entity).ok().is_some_and(
                |(building_visual_opt, blueprint_opt)| {
                    building_visual_opt.is_some_and(|v| v.kind == BuildingTypeVisual::Wall)
                        || blueprint_opt.is_some_and(|s| s.is_plain_wall)
                },
            );
            if !is_plain_wall {
                continue;
            }

            // 完成した Building は Sprite を VisualLayerKind::Struct 子エンティティに持つ
            let mut updated = false;
            if let Ok(children) = queries.q_children.get(entity) {
                for child in children.iter() {
                    if let Ok((kind, mut sprite)) = queries.q_visual_layers.get_mut(child)
                        && *kind == VisualLayerKind::Struct
                    {
                        update_wall_sprite(
                            entity,
                            gx,
                            gy,
                            &mut sprite,
                            world_map.as_ref(),
                            &q_walls_check,
                            &wall_handles,
                        );
                        updated = true;
                        break;
                    }
                }
            }
            // Blueprint エンティティは Sprite を直接持つ
            if !updated && let Ok(mut sprite) = queries.q_blueprint_sprites.get_mut(entity) {
                update_wall_sprite(
                    entity,
                    gx,
                    gy,
                    &mut sprite,
                    world_map.as_ref(),
                    &q_walls_check,
                    &wall_handles,
                );
            }
        }
    }
}

fn add_neighbors_to_update(x: i32, y: i32, targets: &mut HashSet<(i32, i32)>) {
    targets.insert((x, y));
    targets.insert((x, y + 1));
    targets.insert((x, y - 1));
    targets.insert((x + 1, y));
    targets.insert((x - 1, y));
}

fn update_wall_sprite(
    wall_entity: Entity,
    x: i32,
    y: i32,
    sprite: &mut Sprite,
    world_map: &WorldMap,
    q_walls_check: &WallCheckQuery<'_, '_>,
    wall_handles: &WallVisualHandles,
) {
    let up = is_wall(x, y + 1, world_map, q_walls_check);
    let down = is_wall(x, y - 1, world_map, q_walls_check);
    let left = is_wall(x - 1, y, world_map, q_walls_check);
    let right = is_wall(x + 1, y, world_map, q_walls_check);

    let is_provisional = is_provisional_wall(wall_entity, q_walls_check);

    let (texture, flip_x, flip_y) = if is_provisional {
        match (up, down, left, right) {
            (false, false, false, false) => (wall_handles.stone_isolated.clone(), false, false),
            (false, false, true, false) => {
                (wall_handles.stone_horizontal_left.clone(), false, false)
            }
            (false, false, false, true) => {
                (wall_handles.stone_horizontal_right.clone(), false, false)
            }
            (false, false, true, true) => {
                (wall_handles.stone_horizontal_both.clone(), false, false)
            }
            (true, false, false, false) => (wall_handles.stone_vertical_top.clone(), false, false),
            (false, true, false, false) => {
                (wall_handles.stone_vertical_bottom.clone(), false, false)
            }
            (true, true, false, false) => (wall_handles.stone_vertical_both.clone(), false, false),
            (true, false, true, false) => (wall_handles.stone_corner_tl.clone(), false, false),
            (true, false, false, true) => (wall_handles.stone_corner_tr.clone(), false, false),
            (false, true, true, false) => (wall_handles.stone_corner_bl.clone(), false, false),
            (false, true, false, true) => (wall_handles.stone_corner_br.clone(), false, false),
            (true, true, true, false) => (wall_handles.stone_t_left.clone(), false, false),
            (true, true, false, true) => (wall_handles.stone_t_right.clone(), false, false),
            (true, false, true, true) => (wall_handles.stone_t_up.clone(), false, false),
            (false, true, true, true) => (wall_handles.stone_t_down.clone(), false, false),
            (true, true, true, true) => (wall_handles.stone_cross.clone(), false, false),
        }
    } else {
        match (up, down, left, right) {
            (false, false, false, false) => (wall_handles.mud_isolated.clone(), false, false),
            (false, false, true, false) => (wall_handles.mud_end_right.clone(), false, false),
            (false, false, false, true) => (wall_handles.mud_end_left.clone(), false, false),
            (false, false, true, true) => (wall_handles.mud_horizontal.clone(), false, false),
            (true, false, false, false) => (wall_handles.mud_end_bottom.clone(), false, false),
            (false, true, false, false) => (wall_handles.mud_end_top.clone(), false, false),
            (true, true, false, false) => (wall_handles.mud_vertical.clone(), false, false),
            (true, false, true, false) => (wall_handles.mud_corner_tl.clone(), false, false),
            (true, false, false, true) => (wall_handles.mud_corner_tr.clone(), false, false),
            (false, true, true, false) => (wall_handles.mud_corner_bl.clone(), false, false),
            (false, true, false, true) => (wall_handles.mud_corner_br.clone(), false, false),
            (true, true, true, false) => (wall_handles.mud_t_left.clone(), false, false),
            (true, true, false, true) => (wall_handles.mud_t_right.clone(), false, false),
            (true, false, true, true) => (wall_handles.mud_t_up.clone(), false, false),
            (false, true, true, true) => (wall_handles.mud_t_down.clone(), false, false),
            (true, true, true, true) => (wall_handles.mud_cross.clone(), false, false),
        }
    };

    sprite.image = texture;
    sprite.flip_x = flip_x;
    sprite.flip_y = flip_y;
    sprite.color = if is_provisional {
        Color::srgba(1.0, 0.75, 0.4, 0.85)
    } else {
        Color::WHITE
    };
}

fn is_provisional_wall(entity: Entity, q_walls_check: &WallCheckQuery<'_, '_>) -> bool {
    q_walls_check
        .get(entity)
        .ok()
        .and_then(|(visual_opt, _)| visual_opt)
        .is_some_and(|v| v.kind == BuildingTypeVisual::Wall && v.is_provisional)
}

fn is_wall(x: i32, y: i32, world_map: &WorldMap, q_walls_check: &WallCheckQuery<'_, '_>) -> bool {
    if let Some(entity) = world_map.building_entity((x, y))
        && let Ok((building_visual_opt, blueprint_opt)) = q_walls_check.get(entity)
    {
        if let Some(v) = building_visual_opt {
            return matches!(v.kind, BuildingTypeVisual::Wall | BuildingTypeVisual::Door);
        }
        if let Some(s) = blueprint_opt {
            return s.is_wall_or_door;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_handles(
        mud_isolated: Handle<Image>,
        mud_end_right: Handle<Image>,
    ) -> WallVisualHandles {
        let unused = Handle::default();
        WallVisualHandles {
            stone_isolated: unused.clone(),
            stone_horizontal_left: unused.clone(),
            stone_horizontal_right: unused.clone(),
            stone_horizontal_both: unused.clone(),
            stone_vertical_top: unused.clone(),
            stone_vertical_bottom: unused.clone(),
            stone_vertical_both: unused.clone(),
            stone_corner_tl: unused.clone(),
            stone_corner_tr: unused.clone(),
            stone_corner_bl: unused.clone(),
            stone_corner_br: unused.clone(),
            stone_t_up: unused.clone(),
            stone_t_down: unused.clone(),
            stone_t_left: unused.clone(),
            stone_t_right: unused.clone(),
            stone_cross: unused.clone(),
            door_closed: unused.clone(),
            door_open: unused.clone(),
            mud_isolated,
            mud_horizontal: unused.clone(),
            mud_vertical: unused.clone(),
            mud_corner_tl: unused.clone(),
            mud_corner_tr: unused.clone(),
            mud_corner_bl: unused.clone(),
            mud_corner_br: unused.clone(),
            mud_t_up: unused.clone(),
            mud_t_down: unused.clone(),
            mud_t_left: unused.clone(),
            mud_t_right: unused.clone(),
            mud_cross: unused.clone(),
            mud_end_top: unused.clone(),
            mud_end_bottom: unused.clone(),
            mud_end_left: unused.clone(),
            mud_end_right,
            mud_floor: unused,
        }
    }

    fn spawn_completed_wall(app: &mut App, grid: (i32, i32)) -> (Entity, Entity) {
        let wall = app
            .world_mut()
            .spawn((
                Transform::from_translation(WorldMap::grid_to_world(grid.0, grid.1).extend(0.0)),
                BuildingVisualState {
                    kind: BuildingTypeVisual::Wall,
                    is_provisional: false,
                },
            ))
            .id();
        let sprite = app
            .world_mut()
            .spawn((VisualLayerKind::Struct, Sprite::default(), ChildOf(wall)))
            .id();
        app.world_mut()
            .resource_mut::<WorldMap>()
            .set_building(grid, wall);
        (wall, sprite)
    }

    #[test]
    fn removed_wall_dirty_refreshes_surviving_neighbor_sprite() {
        let mut images = Assets::<Image>::default();
        let isolated = images.add(Image::default());
        let connected = images.add(Image::default());

        let mut app = App::new();
        app.init_resource::<WorldMap>()
            .init_resource::<WallConnectionDirty>()
            .insert_resource(test_handles(isolated.clone(), connected.clone()))
            .add_systems(Update, wall_connections_system);

        let removed_grid = (10, 10);
        let survivor_grid = (11, 10);
        let (removed_wall, _) = spawn_completed_wall(&mut app, removed_grid);
        let (_, survivor_sprite) = spawn_completed_wall(&mut app, survivor_grid);

        app.update();
        assert_eq!(
            app.world().get::<Sprite>(survivor_sprite).unwrap().image,
            connected
        );

        assert!(
            app.world_mut()
                .resource_mut::<WorldMap>()
                .clear_building_if_owned(removed_grid, removed_wall)
        );
        app.world_mut().despawn(removed_wall);
        app.world_mut()
            .resource_mut::<WallConnectionDirty>()
            .mark_removed([removed_grid]);

        app.update();
        assert_eq!(
            app.world().get::<Sprite>(survivor_sprite).unwrap().image,
            isolated
        );
    }
}
