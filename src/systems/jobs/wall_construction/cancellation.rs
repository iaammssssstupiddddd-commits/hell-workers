//! Wall construction cancellation system

use super::components::{TargetWallConstructionSite, WallConstructionCancelRequested};
use crate::assets::GameAssets;
use crate::constants::{TILE_SIZE, Z_ITEM_PICKUP};
use crate::entities::damned_soul::{DamnedSoul, Path};
use crate::relationships::WorkingOn;
use crate::systems::logistics::{Inventory, ResourceItem, ResourceType};
use crate::systems::soul_ai::execute::task_execution::context::TaskQueries;
use crate::systems::soul_ai::execute::task_execution::types::AssignedTask;
use crate::systems::soul_ai::helpers::work::unassign_task;
use crate::world::map::WorldMap;
use bevy::prelude::*;
use std::collections::HashSet;

#[derive(Clone, Copy)]
struct SiteTileSnapshot {
    entity: Entity,
    grid_pos: (i32, i32),
    wood_delivered: u32,
    mud_delivered: u32,
    spawned_wall: Option<Entity>,
}

fn spawn_refund_items(
    commands: &mut Commands,
    game_assets: &GameAssets,
    center: Vec2,
    resource_type: ResourceType,
    amount: u32,
) {
    if amount == 0 {
        return;
    }

    let image = match resource_type {
        ResourceType::Wood => game_assets.icon_wood_small.clone(),
        ResourceType::StasisMud => game_assets.icon_stasis_mud_small.clone(),
        _ => return,
    };

    let name = match resource_type {
        ResourceType::Wood => "Item (Wood, WallRefund)",
        ResourceType::StasisMud => "Item (StasisMud, WallRefund)",
        _ => return,
    };

    let columns = 8usize;
    for i in 0..amount as usize {
        let col = (i % columns) as f32;
        let row = (i / columns) as f32;
        let offset_x = (col - (columns as f32 - 1.0) * 0.5) * (TILE_SIZE * 0.18);
        let offset_y = row * (TILE_SIZE * 0.18);
        commands.spawn((
            ResourceItem(resource_type),
            Sprite {
                image: image.clone(),
                custom_size: Some(Vec2::splat(TILE_SIZE * 0.5)),
                ..default()
            },
            Transform::from_translation(Vec3::new(
                center.x + offset_x,
                center.y + offset_y,
                Z_ITEM_PICKUP,
            )),
            Name::new(name),
        ));
    }
}

/// Cancels wall construction sites marked with `WallConstructionCancelRequested`.
pub fn wall_construction_cancellation_system(
    mut commands: Commands,
    q_sites: Query<Entity, With<WallConstructionCancelRequested>>,
    q_entities: Query<Entity>,
    q_wall_requests: Query<(Entity, &TargetWallConstructionSite)>,
    mut q_souls: Query<
        (
            Entity,
            &Transform,
            &mut AssignedTask,
            &mut Path,
            &mut Inventory,
            Option<&WorkingOn>,
        ),
        With<DamnedSoul>,
    >,
    mut reservation_queries: TaskQueries,
    mut world_map: ResMut<WorldMap>,
    game_assets: Res<GameAssets>,
) {
    for site_entity in q_sites.iter() {
        let (site_material_center, site_tiles_total) = {
            let Ok((_site_transform, site, _)) =
                reservation_queries.storage.wall_sites.get(site_entity)
            else {
                continue;
            };
            (site.material_center, site.tiles_total)
        };

        let mut site_tiles: Vec<SiteTileSnapshot> = Vec::new();
        for tile_entity in q_entities.iter() {
            let Ok(tile) = reservation_queries.storage.wall_tiles.get_mut(tile_entity) else {
                continue;
            };
            if tile.parent_site != site_entity {
                continue;
            }
            site_tiles.push(SiteTileSnapshot {
                entity: tile_entity,
                grid_pos: tile.grid_pos,
                wood_delivered: tile.wood_delivered,
                mud_delivered: tile.mud_delivered,
                spawned_wall: tile.spawned_wall,
            });
        }

        let site_requests: Vec<Entity> = q_wall_requests
            .iter()
            .filter(|(_, target_site)| target_site.0 == site_entity)
            .map(|(request_entity, _)| request_entity)
            .collect();

        let mut related_targets: HashSet<Entity> =
            HashSet::with_capacity(site_tiles.len() + site_requests.len() + 1);
        related_targets.extend(site_tiles.iter().map(|tile| tile.entity));
        related_targets.extend(site_tiles.iter().filter_map(|tile| tile.spawned_wall));
        related_targets.extend(site_requests.iter().copied());
        related_targets.insert(site_entity);

        let mut released_workers = 0usize;
        for (
            soul_entity,
            soul_transform,
            mut assigned_task,
            mut path,
            mut inventory,
            working_on_opt,
        ) in q_souls.iter_mut()
        {
            let matches_site_task = assigned_task
                .get_target_entity()
                .is_some_and(|target| related_targets.contains(&target));
            let matches_working_on =
                working_on_opt.is_some_and(|working_on| related_targets.contains(&working_on.0));

            if !(matches_site_task || matches_working_on) {
                continue;
            }

            unassign_task(
                &mut commands,
                soul_entity,
                soul_transform.translation.truncate(),
                &mut assigned_task,
                &mut path,
                Some(&mut inventory),
                None,
                &mut reservation_queries,
                &world_map,
                true,
            );
            released_workers += 1;
        }

        let refunded_wood: u32 = site_tiles.iter().map(|tile| tile.wood_delivered).sum();
        let refunded_mud: u32 = site_tiles.iter().map(|tile| tile.mud_delivered).sum();
        spawn_refund_items(
            &mut commands,
            &game_assets,
            site_material_center,
            ResourceType::Wood,
            refunded_wood,
        );
        spawn_refund_items(
            &mut commands,
            &game_assets,
            site_material_center,
            ResourceType::StasisMud,
            refunded_mud,
        );

        for request_entity in site_requests {
            commands.entity(request_entity).try_despawn();
        }

        for tile in site_tiles {
            if let Some(wall_entity) = tile.spawned_wall {
                commands.entity(wall_entity).try_despawn();
            }

            if world_map
                .buildings
                .get(&tile.grid_pos)
                .copied()
                .is_some_and(|entity| entity == site_entity || Some(entity) == tile.spawned_wall)
            {
                world_map.buildings.remove(&tile.grid_pos);
            }

            world_map.remove_obstacle(tile.grid_pos.0, tile.grid_pos.1);
            commands.entity(tile.entity).try_despawn();
        }

        commands.entity(site_entity).try_despawn();

        info!(
            "WALL_CANCEL: Site {:?} cancelled (tiles: {}, workers: {}, refund wood: {}, refund mud: {})",
            site_entity, site_tiles_total, released_workers, refunded_wood, refunded_mud
        );
    }
}
