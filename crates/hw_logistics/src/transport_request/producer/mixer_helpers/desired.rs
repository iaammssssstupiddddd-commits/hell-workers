use std::collections::HashSet;

use bevy::prelude::*;

use hw_core::constants::{BUCKET_CAPACITY, MUD_MIXER_CAPACITY};
use hw_core::relationships::{StoredItems, TaskWorkers};
use hw_jobs::MovePlanned;
use hw_jobs::mud_mixer::MudMixerStorage;
use hw_world::zones::{AreaBounds, Yard};

use crate::resource_cache::SharedResourceCache;
use crate::transport_request::producer::find_owner_for_position;
use crate::types::ResourceType;
use crate::zone::Stockpile;

type MixerQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static MudMixerStorage,
        Option<&'static TaskWorkers>,
        Option<&'static MovePlanned>,
    ),
>;

pub(crate) type StockpilesDetailedQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static Stockpile,
        Option<&'static StoredItems>,
    ),
>;

pub(crate) struct MixerInflightContext<'a> {
    pub haul_cache: &'a SharedResourceCache,
    pub water_inflight_by_mixer: &'a std::collections::HashMap<Entity, u32>,
    pub sand_inflight_by_mixer: &'a std::collections::HashMap<Entity, u32>,
}

pub(crate) fn compute_mixer_desired_requests(
    q_mixers: &MixerQuery,
    desired_requests: &mut std::collections::HashMap<(Entity, ResourceType), (Entity, u32, Vec2)>,
    active_mixers: &mut HashSet<Entity>,
    all_owners: &[(Entity, AreaBounds)],
    active_yards: &[(Entity, Yard)],
    q_stockpiles_detailed: &StockpilesDetailedQuery,
    inflight: MixerInflightContext<'_>,
) {
    for (mixer_entity, mixer_transform, storage, _workers_opt, move_planned_opt) in q_mixers.iter()
    {
        if move_planned_opt.is_some() {
            continue;
        }
        active_mixers.insert(mixer_entity);

        let mixer_pos = mixer_transform.translation.truncate();
        let Some((fam_entity, _owner_area)) =
            find_owner_for_position(mixer_pos, all_owners, active_yards)
        else {
            continue;
        };

        for resource_type in [ResourceType::Sand, ResourceType::Rock] {
            let current = match resource_type {
                ResourceType::Sand => storage.sand,
                ResourceType::Rock => storage.rock,
                _ => 0,
            };

            let inflight_count = if resource_type == ResourceType::Sand {
                *inflight
                    .sand_inflight_by_mixer
                    .get(&mixer_entity)
                    .unwrap_or(&0)
            } else {
                0
            };
            let _ = inflight
                .haul_cache
                .get_mixer_destination_reservation(mixer_entity, resource_type);
            let needed = MUD_MIXER_CAPACITY.saturating_sub(current + inflight_count);
            if needed > 0 {
                desired_requests.insert(
                    (mixer_entity, resource_type),
                    (fam_entity, needed.max(1), mixer_pos),
                );
            }
        }

        let water_inflight_tasks = *inflight
            .water_inflight_by_mixer
            .get(&mixer_entity)
            .unwrap_or(&0);
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
