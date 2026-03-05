use bevy::prelude::*;

use crate::systems::jobs::{TargetMixer, WorkType};
use crate::systems::logistics::ResourceType;
use crate::systems::logistics::transport_request::{
    TransportRequest, TransportRequestKind,
};

pub(crate) fn upsert_mixer_requests(
    commands: &mut Commands,
    q_mixer_requests: &Query<(
        Entity,
        &TargetMixer,
        &TransportRequest,
        Option<&crate::systems::jobs::Designation>,
        Option<&crate::relationships::TaskWorkers>,
    )>,
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
        crate::systems::logistics::transport_request::producer::upsert::spawn_transport_request_with_work_type(
            commands,
            name,
            *key,
            *mixer_pos,
            *issued_by,
            *slots,
            5,
            TargetMixer(key.0),
            kind,
            work_type,
        );
    }
}

fn upsert_mixer_requests_by_kind(
    commands: &mut Commands,
    q_mixer_requests: &Query<
        (
            Entity,
            &TargetMixer,
            &TransportRequest,
            Option<&crate::systems::jobs::Designation>,
            Option<&crate::relationships::TaskWorkers>,
        ),
    >,
    desired_requests: &std::collections::HashMap<(Entity, ResourceType), (Entity, u32, Vec2)>,
    active_mixers: &std::collections::HashSet<Entity>,
    seen_existing_keys: &mut std::collections::HashSet<(Entity, ResourceType)>,
    expected_kind: TransportRequestKind,
) {
    for (request_entity, target_mixer, request, _designation, workers_opt) in q_mixer_requests.iter() {
        if request.kind != expected_kind {
            continue;
        }
        let key = (target_mixer.0, request.resource_type);
        if !mixer_request_resource_matches(key.1, expected_kind) {
            continue;
        }

        let workers = workers_opt.map(|w| w.len()).unwrap_or(0);
        if !crate::systems::logistics::transport_request::producer::upsert::process_duplicate_key(
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
            crate::systems::logistics::transport_request::producer::upsert::upsert_transport_request_with_work_type(
                commands,
                request_entity,
                key,
                *mixer_pos,
                *issued_by,
                *slots,
                0,
                5,
                TargetMixer(key.0),
                kind,
                work_type,
            );
            continue;
        }

        if workers == 0 {
            if !active_mixers.contains(&target_mixer.0) {
                commands.entity(request_entity).try_despawn();
            } else {
                crate::systems::logistics::transport_request::producer::upsert::disable_request(
                    commands,
                    request_entity,
                );
            }
        }
    }
}

fn mixer_request_profile(resource_type: ResourceType) -> (WorkType, TransportRequestKind, &'static str) {
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
