use bevy::prelude::*;
use hw_core::constants::TILE_SIZE;
use hw_energy::{SoulSpaPhase, SoulSpaSite, SoulSpaTile};
use hw_jobs::model::{Designation, TaskSlots, WorkType};
use hw_logistics::ResourceType;
use hw_logistics::types::ResourceItem;
use hw_spatial::{ResourceSpatialGrid, SpatialGridOps};

const PICKUP_RADIUS: f32 = TILE_SIZE * 1.5;

/// SoulSpaSite 周辺に搬入された Bone を検知し、`bones_delivered` を更新する。
/// 必要数を満たしたサイトを `SoulSpaPhase::Operational` に遷移させる。
pub fn soul_spa_delivery_sync_system(
    mut commands: Commands,
    mut q_sites: Query<(Entity, &Transform, &mut SoulSpaSite)>,
    q_resources: Query<(Entity, &Transform, &ResourceItem)>,
    resource_grid: Res<ResourceSpatialGrid>,
) {
    for (site_entity, site_transform, mut site) in q_sites.iter_mut() {
        if site.phase != SoulSpaPhase::Constructing {
            continue;
        }

        let site_pos = site_transform.translation.truncate();
        let nearby = resource_grid.get_nearby_in_radius(site_pos, PICKUP_RADIUS);

        let mut consumed = 0u32;
        for res_entity in nearby {
            let Ok((_, _, res_item)) = q_resources.get(res_entity) else {
                continue;
            };
            if res_item.0 != ResourceType::Bone {
                continue;
            }
            let still_needed = site
                .bones_required
                .saturating_sub(site.bones_delivered + consumed);
            if still_needed == 0 {
                break;
            }
            commands.entity(res_entity).try_despawn();
            consumed += 1;
        }

        if consumed == 0 {
            continue;
        }

        site.bones_delivered = (site.bones_delivered + consumed).min(site.bones_required);

        if site.bones_delivered >= site.bones_required {
            site.phase = SoulSpaPhase::Operational;
            info!(
                "SoulSpaSite {:?} construction complete → Operational",
                site_entity
            );
        }
    }
}

/// Operational に遷移した SoulSpaSite の各 SoulSpaTile に
/// `Designation(GeneratePower)` と `TaskSlots { max: 1 }` を挿入する。
pub fn soul_spa_tile_activate_system(
    mut commands: Commands,
    q_sites: Query<(Entity, &SoulSpaSite), Changed<SoulSpaSite>>,
    q_tiles: Query<(Entity, &SoulSpaTile)>,
) {
    for (site_entity, site) in q_sites.iter() {
        if site.phase != SoulSpaPhase::Operational {
            continue;
        }
        for (tile_entity, tile) in q_tiles.iter() {
            if tile.parent_site == site_entity {
                commands.entity(tile_entity).insert((
                    Designation {
                        work_type: WorkType::GeneratePower,
                    },
                    TaskSlots { max: 1 },
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operational_activation_uses_durable_parent_site_without_hierarchy() {
        let mut app = App::new();
        app.add_systems(Update, soul_spa_tile_activate_system);

        let site = app.world_mut().spawn(SoulSpaSite::default()).id();
        let other_site = app.world_mut().spawn(SoulSpaSite::default()).id();
        let tile = app
            .world_mut()
            .spawn(SoulSpaTile {
                parent_site: site,
                grid_pos: (4, 5),
            })
            .id();
        let other_tile = app
            .world_mut()
            .spawn(SoulSpaTile {
                parent_site: other_site,
                grid_pos: (6, 7),
            })
            .id();

        app.update();
        assert!(app.world().get::<Designation>(tile).is_none());

        app.world_mut().get_mut::<SoulSpaSite>(site).unwrap().phase = SoulSpaPhase::Operational;
        app.update();

        assert_eq!(
            app.world().get::<Designation>(tile).unwrap().work_type,
            WorkType::GeneratePower
        );
        assert_eq!(app.world().get::<TaskSlots>(tile).unwrap().max, 1);
        assert!(app.world().get::<Designation>(other_tile).is_none());
        assert!(app.world().get::<ChildOf>(tile).is_none());
    }
}
