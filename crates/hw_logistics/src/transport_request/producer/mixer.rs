//! MudMixer auto-haul system

use super::mixer_helpers;

use bevy::prelude::*;

use hw_core::area::TaskArea;
use hw_core::familiar::ActiveCommand;
use hw_core::relationships::TaskWorkers;
use hw_jobs::mud_mixer::{MudMixerStorage, TargetMixer};
use hw_world::zones::Yard;

use crate::resource_cache::SharedResourceCache;
use crate::transport_request::TransportRequest;
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
        Option<&'static hw_jobs::MovePlanned>,
    ),
>;

type MixerRequestQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static TargetMixer,
        &'static TransportRequest,
        Option<&'static hw_jobs::Designation>,
        Option<&'static TaskWorkers>,
    ),
>;

pub fn mud_mixer_auto_haul_system(
    mut commands: Commands,
    haul_cache: Res<SharedResourceCache>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_yards: Query<(Entity, &Yard)>,
    q_mixers: MixerQuery,
    q_mixer_requests: MixerRequestQuery,
    q_stockpiles_detailed: Query<(
        Entity,
        &Transform,
        &Stockpile,
        Option<&hw_core::relationships::StoredItems>,
    )>,
) {
    let active_familiars = mixer_helpers::collect_active_familiars(&q_familiars);
    let active_yards = mixer_helpers::collect_active_yards(&q_yards);
    let all_owners = super::collect_all_area_owners(&active_familiars, &active_yards);

    let (water_inflight_by_mixer, sand_inflight_by_mixer) =
        mixer_helpers::collect_inflight_mixer_requests(&q_mixer_requests);

    let mut desired_requests =
        std::collections::HashMap::<(Entity, ResourceType), (Entity, u32, Vec2)>::new();
    let mut active_mixers = std::collections::HashSet::<Entity>::new();

    mixer_helpers::compute_mixer_desired_requests(
        &q_mixers,
        &mut desired_requests,
        &mut active_mixers,
        &all_owners,
        &active_yards,
        &haul_cache,
        &q_stockpiles_detailed,
        &water_inflight_by_mixer,
        &sand_inflight_by_mixer,
    );

    mixer_helpers::upsert_mixer_requests(
        &mut commands,
        &q_mixer_requests,
        &desired_requests,
        &active_mixers,
    );
}
