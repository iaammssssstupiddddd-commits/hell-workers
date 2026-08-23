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
    world_map::validate_world_map_candidate(candidate)?;
    construction::validate_construction_links(candidate)?;
    energy::validate_energy_links(candidate)?;
    zones_targets::validate_zone_links(candidate)?;
    zones_targets::validate_target_links(candidate)?;
    world_map::validate_natural_obstacle_positions(candidate)?;
    world_map::validate_world_map_tile_anchors(candidate)?;
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
