use bevy::prelude::*;

use hw_core::relationships::TaskWorkers;
use hw_jobs::WorkType;
use hw_jobs::mud_mixer::TargetMixer;

use crate::transport_request::producer::upsert::{self, SemanticRequestSpec, SpawnRequestSpec};
use crate::transport_request::{TransportPriority, TransportRequest, TransportRequestKind};
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

pub(crate) fn upsert_mixer_requests(
    commands: &mut Commands,
    q_mixer_requests: &MixerRequestsQuery,
    desired_requests: &std::collections::HashMap<(Entity, ResourceType), (Entity, u32, Vec2)>,
    active_mixers: &std::collections::HashSet<Entity>,
) {
    let mut seen_existing_keys = std::collections::HashSet::<(Entity, ResourceType)>::new();

    upsert_mixer_requests_by_kind(
        commands,
        q_mixer_requests,
        desired_requests,
        active_mixers,
        &mut seen_existing_keys,
        TransportRequestKind::DeliverWaterToMixer,
    );
    upsert_mixer_requests_by_kind(
        commands,
        q_mixer_requests,
        desired_requests,
        active_mixers,
        &mut seen_existing_keys,
        TransportRequestKind::DeliverToMixerSolid,
    );

    for (key, (issued_by, slots, mixer_pos)) in desired_requests.iter() {
        if seen_existing_keys.contains(key) {
            continue;
        }

        let (work_type, kind, name) = mixer_request_profile(key.1);
        upsert::spawn_transport_request(
            commands,
            SpawnRequestSpec {
                name,
                key: *key,
                site_pos: *mixer_pos,
                issued_by: *issued_by,
                desired_slots: *slots,
                priority: 5,
                target: TargetMixer(key.0),
                kind,
                work_type,
            },
        );
    }
}

fn upsert_mixer_requests_by_kind(
    commands: &mut Commands,
    q_mixer_requests: &MixerRequestsQuery,
    desired_requests: &std::collections::HashMap<(Entity, ResourceType), (Entity, u32, Vec2)>,
    active_mixers: &std::collections::HashSet<Entity>,
    seen_existing_keys: &mut std::collections::HashSet<(Entity, ResourceType)>,
    expected_kind: TransportRequestKind,
) {
    for (request_entity, target_mixer, request, workers_opt, current) in q_mixer_requests.iter() {
        if request.kind != expected_kind {
            continue;
        }
        let key = (request.anchor, request.resource_type);
        if !mixer_request_resource_matches(key.1, expected_kind) {
            continue;
        }

        let workers = workers_opt.map(|w| w.len()).unwrap_or(0);
        if !upsert::process_duplicate_key(
            commands,
            request_entity,
            workers,
            seen_existing_keys,
            key,
        ) {
            continue;
        }

        if let Some((issued_by, slots, mixer_pos)) = desired_requests.get(&key) {
            let (work_type, kind, _) = mixer_request_profile(key.1);
            if target_mixer.is_none_or(|current| current.0 != key.0) {
                commands
                    .entity(request_entity)
                    .try_insert(TargetMixer(key.0));
            }
            upsert::update_request_runtime_if_needed(
                commands,
                request_entity,
                request,
                current,
                SemanticRequestSpec {
                    key,
                    site_pos: *mixer_pos,
                    issued_by: *issued_by,
                    desired_slots: *slots,
                    inflight: super::super::to_u32_saturating(workers),
                    priority: 5,
                    transport_priority: TransportPriority::Normal,
                    kind,
                    work_type,
                    state: upsert::request_state_for_workers(workers),
                },
            );
            continue;
        }

        if workers == 0 {
            if !active_mixers.contains(&request.anchor) {
                commands.entity(request_entity).try_despawn();
            } else {
                upsert::disable_request_if_needed(commands, request_entity, current, None);
            }
        }
    }
}

fn mixer_request_profile(
    resource_type: ResourceType,
) -> (WorkType, TransportRequestKind, &'static str) {
    if resource_type == ResourceType::Water {
        (
            WorkType::HaulWaterToMixer,
            TransportRequestKind::DeliverWaterToMixer,
            "TransportRequest::DeliverWaterToMixer",
        )
    } else {
        (
            WorkType::HaulToMixer,
            TransportRequestKind::DeliverToMixerSolid,
            "TransportRequest::DeliverToMixerSolid",
        )
    }
}

fn mixer_request_resource_matches(resource_type: ResourceType, kind: TransportRequestKind) -> bool {
    match kind {
        TransportRequestKind::DeliverWaterToMixer => resource_type == ResourceType::Water,
        TransportRequestKind::DeliverToMixerSolid => {
            matches!(resource_type, ResourceType::Sand | ResourceType::Rock)
        }
        _ => false,
    }
}
