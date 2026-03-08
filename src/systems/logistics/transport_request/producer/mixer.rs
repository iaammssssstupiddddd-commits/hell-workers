//! MudMixer auto-haul system
//!
//! Automatically creates haul tasks for materials needed by MudMixer.

#[path = "mixer_helpers.rs"]
mod mixer_helpers;

use bevy::prelude::*;

use crate::entities::familiar::{ActiveCommand};
use crate::events::DesignationRequest;
use crate::relationships::{ManagedBy, TaskWorkers};
use crate::systems::command::TaskArea;
use crate::systems::familiar_ai::perceive::resource_sync::SharedResourceCache;
use crate::systems::jobs::{Designation, MudMixerStorage, TargetMixer};
use crate::systems::logistics::transport_request::{TransportDemand, TransportRequest};
use crate::systems::logistics::{ResourceType, Stockpile};
use crate::systems::soul_ai::execute::task_execution::move_plant::MovePlanned;
use crate::systems::world::zones::Yard;
use crate::world::map::WorldMapRead;

/// MudMixer への自動資材運搬タスク生成システム
pub fn mud_mixer_auto_haul_system(
    mut commands: Commands,
    mut designation_writer: MessageWriter<DesignationRequest>,
    haul_cache: Res<SharedResourceCache>,
    world_map: WorldMapRead,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_yards: Query<(Entity, &Yard)>,
    q_mixers: Query<(
        Entity,
        &Transform,
        &MudMixerStorage,
        Option<&TaskWorkers>,
        Option<&MovePlanned>,
    )>,
    q_mixer_requests: Query<(
        Entity,
        &TargetMixer,
        &TransportRequest,
        Option<&Designation>,
        Option<&TaskWorkers>,
    )>,
    q_stockpiles_detailed: Query<(
        Entity,
        &Transform,
        &Stockpile,
        Option<&crate::relationships::StoredItems>,
    )>,
    q_sand_piles: Query<
        (
            Entity,
            &Transform,
            Option<&Designation>,
            Option<&TaskWorkers>,
        ),
        With<crate::systems::jobs::SandPile>,
    >,
    q_task_state: Query<(Option<&Designation>, Option<&TaskWorkers>)>,
    q_collect_sand_tasks: Query<(&Designation, &ManagedBy, Option<&TaskWorkers>)>,
    q_requests_for_demand: Query<(&TransportRequest, Option<&TaskWorkers>, Option<&TransportDemand>)>,
) {
    let active_familiars = mixer_helpers::collect_active_familiars(&q_familiars);
    let active_yards = mixer_helpers::collect_active_yards(&q_yards);
    let all_owners = super::collect_all_area_owners(&active_familiars, &active_yards);

    let (collect_sand_demanders, collect_sand_tasking) =
        mixer_helpers::collect_collect_sand_familiar_states(&q_requests_for_demand, &q_collect_sand_tasks);
    let (water_inflight_by_mixer, sand_inflight_by_mixer) =
        mixer_helpers::collect_inflight_mixer_requests(&q_mixer_requests);

    let mut desired_requests =
        std::collections::HashMap::<(Entity, ResourceType), (Entity, u32, Vec2)>::new();
    let mut active_mixers = std::collections::HashSet::<Entity>::new();
    let mut collect_sand_candidates = Vec::<mixer_helpers::MixerCollectSandCandidate>::new();

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
        &collect_sand_demanders,
        &collect_sand_tasking,
        &mut collect_sand_candidates,
    );

    for candidate in collect_sand_candidates.iter() {
        mixer_helpers::issue_collect_sand_if_needed(
            &mut designation_writer,
            candidate,
            &q_sand_piles,
            &q_task_state,
            world_map.as_ref(),
        );
    }

    mixer_helpers::upsert_mixer_requests(
        &mut commands,
        &q_mixer_requests,
        &desired_requests,
        &active_mixers,
    );
}
