use std::collections::HashMap;

use bevy::prelude::*;

use hw_core::relationships::TaskWorkers;
use hw_jobs::mud_mixer::TargetMixer;

use crate::transport_request::producer::upsert;
use crate::transport_request::{TransportRequest, TransportRequestKind};
use crate::types::ResourceType;

type MixerRequestsQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Option<&'static TargetMixer>,
        &'static TransportRequest,
        Option<&'static TaskWorkers>,
        upsert::ExistingRequestRuntime<'static>,
    ),
    Or<(With<TargetMixer>, Added<TransportRequest>)>,
>;

pub(crate) fn collect_inflight_mixer_requests(
    q_mixer_requests: &MixerRequestsQuery,
) -> (HashMap<Entity, u32>, HashMap<Entity, u32>) {
    let mut water_inflight_by_mixer = HashMap::<Entity, u32>::new();
    let mut sand_inflight_by_mixer = HashMap::<Entity, u32>::new();

    for (_, _, request, workers_opt, _) in q_mixer_requests.iter() {
        let workers = workers_opt.map(|w| w.len() as u32).unwrap_or(0);
        if workers == 0 {
            continue;
        }

        match (request.kind, request.resource_type) {
            (TransportRequestKind::DeliverWaterToMixer, _) => {
                *water_inflight_by_mixer.entry(request.anchor).or_insert(0) += workers;
            }
            (TransportRequestKind::DeliverToMixerSolid, ResourceType::Sand) => {
                *sand_inflight_by_mixer.entry(request.anchor).or_insert(0) += workers;
            }
            _ => {}
        }
    }

    (water_inflight_by_mixer, sand_inflight_by_mixer)
}
