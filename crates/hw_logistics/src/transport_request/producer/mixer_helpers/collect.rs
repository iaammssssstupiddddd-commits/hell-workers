use std::collections::HashMap;

use bevy::prelude::*;

use hw_core::area::TaskArea;
use hw_core::familiar::{ActiveCommand, FamiliarCommand};
use hw_core::relationships::TaskWorkers;
use hw_jobs::Designation;
use hw_jobs::mud_mixer::TargetMixer;
use hw_world::zones::{AreaBounds, Yard};

use crate::transport_request::{TransportRequest, TransportRequestKind};
use crate::types::ResourceType;

type MixerRequestsQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static TargetMixer,
        &'static TransportRequest,
        Option<&'static Designation>,
        Option<&'static TaskWorkers>,
    ),
>;

pub(crate) fn collect_active_familiars(
    q_familiars: &Query<(Entity, &ActiveCommand, &TaskArea)>,
) -> Vec<(Entity, AreaBounds)> {
    q_familiars
        .iter()
        .filter(|(_, active_command, _)| !matches!(active_command.command, FamiliarCommand::Idle))
        .map(|(entity, _, area)| (entity, area.bounds()))
        .collect()
}

pub(crate) fn collect_active_yards(q_yards: &Query<(Entity, &Yard)>) -> Vec<(Entity, Yard)> {
    q_yards
        .iter()
        .map(|(entity, yard)| (entity, yard.clone()))
        .collect()
}

pub(crate) fn collect_inflight_mixer_requests(
    q_mixer_requests: &MixerRequestsQuery,
) -> (HashMap<Entity, u32>, HashMap<Entity, u32>) {
    let mut water_inflight_by_mixer = HashMap::<Entity, u32>::new();
    let mut sand_inflight_by_mixer = HashMap::<Entity, u32>::new();

    for (_, target_mixer, request, _, workers_opt) in q_mixer_requests.iter() {
        let workers = workers_opt.map(|w| w.len() as u32).unwrap_or(0);
        if workers == 0 {
            continue;
        }

        match (request.kind, request.resource_type) {
            (TransportRequestKind::DeliverWaterToMixer, _) => {
                *water_inflight_by_mixer.entry(target_mixer.0).or_insert(0) += workers;
            }
            (TransportRequestKind::DeliverToMixerSolid, ResourceType::Sand) => {
                *sand_inflight_by_mixer.entry(target_mixer.0).or_insert(0) += workers;
            }
            _ => {}
        }
    }

    (water_inflight_by_mixer, sand_inflight_by_mixer)
}
