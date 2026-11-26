use super::*;

mod construction;
mod energy;
mod world_map;
mod zones_targets;

#[cfg(test)]
pub(super) use construction::validate_construction_links;
#[cfg(test)]
pub(super) use energy::validate_energy_links;
#[cfg(test)]
pub(super) use world_map::validate_world_map_candidate;

pub(super) fn validate(candidate: &World) -> Result<(), String> {
    use crate::systems::ventilation::climate::{AirLedger, ClimateSetting};
    let mut found = false;
    for entity in candidate.iter_entities() {
        if let Some(air) = entity.get::<AirLedger>() {
            if found || !air.valid() {
                return Err("室内空気の保存状態が重複または破損しています".into());
            }
            found = true;
        }
        if let Some(setting) = entity.get::<ClimateSetting>()
            && (setting.0 > 2
                || !entity.get::<Building>().is_some_and(|b| {
                    matches!(
                        b.kind,
                        hw_jobs::BuildingType::Equipment(
                            hw_jobs::Equipment::SupplyFan
                                | hw_jobs::Equipment::ExhaustFan
                                | hw_jobs::Equipment::Hood
                        )
                    )
                }))
        {
            return Err("送風設定の保存対象が不正です".into());
        }
    }
    crate::systems::waterworks::validate_candidate(candidate)?;
    world_map::validate_world_map_candidate(candidate)?;
    construction::validate_construction_links(candidate)?;
    energy::validate_energy_links(candidate)?;
    zones_targets::validate_zone_links(candidate)?;
    zones_targets::validate_target_links(candidate)?;
    world_map::validate_natural_obstacle_positions(candidate)?;
    Ok(())
}

fn validate_world_map_grid(field: &str, grid: (i32, i32)) -> Result<(), String> {
    if (0..MAP_WIDTH).contains(&grid.0) && (0..MAP_HEIGHT).contains(&grid.1) {
        Ok(())
    } else {
        Err(format!(
            "WorldMap.{field} contains out-of-bounds grid {grid:?}"
        ))
    }
}
