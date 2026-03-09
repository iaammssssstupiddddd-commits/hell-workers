use std::collections::HashSet;

use bevy::prelude::*;

use crate::relationships::TaskWorkers;
use crate::systems::familiar_ai::perceive::resource_sync::SharedResourceCache;
use crate::systems::jobs::MudMixerStorage;
use crate::systems::logistics::ResourceType;
use crate::systems::soul_ai::execute::task_execution::move_plant::MovePlanned;
use crate::systems::world::zones::{AreaBounds, Yard};
use hw_core::constants::{BUCKET_CAPACITY, MUD_MIXER_CAPACITY};

use super::types::MixerCollectSandCandidate;

pub(crate) fn compute_mixer_desired_requests(
    q_mixers: &Query<(
        Entity,
        &Transform,
        &MudMixerStorage,
        Option<&TaskWorkers>,
        Option<&MovePlanned>,
    )>,
    desired_requests: &mut std::collections::HashMap<(Entity, ResourceType), (Entity, u32, Vec2)>,
    active_mixers: &mut HashSet<Entity>,
    all_owners: &[(Entity, AreaBounds)],
    active_yards: &[(Entity, Yard)],
    haul_cache: &SharedResourceCache,
    q_stockpiles_detailed: &Query<(
        Entity,
        &Transform,
        &crate::systems::logistics::Stockpile,
        Option<&crate::relationships::StoredItems>,
    )>,
    water_inflight_by_mixer: &std::collections::HashMap<Entity, u32>,
    sand_inflight_by_mixer: &std::collections::HashMap<Entity, u32>,
    collect_sand_demanders: &HashSet<Entity>,
    collect_sand_tasking: &HashSet<Entity>,
    collect_sand_candidates: &mut Vec<MixerCollectSandCandidate>,
) {
    for (mixer_entity, mixer_transform, storage, _workers_opt, move_planned_opt) in q_mixers.iter()
    {
        if move_planned_opt.is_some() {
            continue;
        }
        active_mixers.insert(mixer_entity);

        let mixer_pos = mixer_transform.translation.truncate();
        let Some((fam_entity, owner_area)) =
            crate::systems::logistics::transport_request::producer::find_owner_for_position(
                mixer_pos,
                all_owners,
                active_yards,
            )
        else {
            continue;
        };
        let yard_area = crate::systems::logistics::transport_request::producer::find_owner_yard(
            mixer_pos,
            active_yards,
        )
        .map(|(_, yard)| yard);

        for resource_type in [ResourceType::Sand, ResourceType::Rock] {
            let current = match resource_type {
                ResourceType::Sand => storage.sand,
                ResourceType::Rock => storage.rock,
                _ => 0,
            };

            let _ = haul_cache.get_mixer_destination_reservation(mixer_entity, resource_type);
            let needed = MUD_MIXER_CAPACITY.saturating_sub(current);
            if needed > 0 {
                desired_requests.insert(
                    (mixer_entity, resource_type),
                    (fam_entity, needed.max(1), mixer_pos),
                );
            }
        }

        let sand_inflight_tasks = *sand_inflight_by_mixer.get(&mixer_entity).unwrap_or(&0);
        let has_collect_sand_demand = collect_sand_demanders.contains(&fam_entity);
        let has_collect_sand_task = collect_sand_tasking.contains(&fam_entity);
        if has_collect_sand_demand
            && !has_collect_sand_task
            && storage.sand + sand_inflight_tasks < 2
        {
            collect_sand_candidates.push(MixerCollectSandCandidate {
                mixer_entity,
                issued_by: fam_entity,
                mixer_pos,
                owner_area: owner_area.clone(),
                yard_area: yard_area.cloned(),
                current_sand: storage.sand,
                sand_inflight: sand_inflight_tasks,
            });
        }

        let water_inflight_tasks = *water_inflight_by_mixer.get(&mixer_entity).unwrap_or(&0);
        let water_inflight = water_inflight_tasks * BUCKET_CAPACITY;
        let (water_current, water_capacity) =
            if let Ok((_, _, stock, stored_opt)) = q_stockpiles_detailed.get(mixer_entity) {
                if stock.resource_type == Some(ResourceType::Water) {
                    (
                        stored_opt.map(|s| s.len()).unwrap_or(0) as u32,
                        stock.capacity as u32,
                    )
                } else {
                    (0, MUD_MIXER_CAPACITY)
                }
            } else {
                (0, MUD_MIXER_CAPACITY)
            };
        let projected_water = water_current
            .saturating_add(water_inflight)
            .min(water_capacity);
        let missing_water = water_capacity.saturating_sub(projected_water);
        if missing_water > 0 {
            let desired_slots = missing_water.div_ceil(BUCKET_CAPACITY).max(1);
            desired_requests.insert(
                (mixer_entity, ResourceType::Water),
                (fam_entity, desired_slots, mixer_pos),
            );
        }
    }
}
