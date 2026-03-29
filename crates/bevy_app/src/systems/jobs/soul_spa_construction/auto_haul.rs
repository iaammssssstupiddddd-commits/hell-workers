use bevy::prelude::*;
use hw_core::area::TaskArea;
use hw_core::constants::FLOOR_CONSTRUCTION_PRIORITY;
use hw_core::familiar::{ActiveCommand, FamiliarCommand};
use hw_core::relationships::TaskWorkers;
use hw_energy::{SoulSpaPhase, SoulSpaSite};
use hw_jobs::TargetSoulSpaSite;
use hw_logistics::transport_request::producer::{
    RequestSyncSpec, collect_all_area_owners, find_owner, sync_construction_requests,
};
use hw_logistics::transport_request::{TransportRequest, TransportRequestKind};
use hw_logistics::ResourceType;
use hw_world::zones::{AreaBounds, Yard};
use std::collections::HashMap;

/// Constructing フェーズの SoulSpaSite に Bone の TransportRequest を自動生成。
pub fn soul_spa_auto_haul_system(
    mut commands: Commands,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_yards: Query<(Entity, &Yard)>,
    q_sites: Query<(Entity, &Transform, &SoulSpaSite)>,
    q_existing: Query<(
        Entity,
        &TargetSoulSpaSite,
        &TransportRequest,
        Option<&TaskWorkers>,
    )>,
) {
    let active_familiars: Vec<(Entity, AreaBounds)> = q_familiars
        .iter()
        .filter(|(_, active_cmd, _)| !matches!(active_cmd.command, FamiliarCommand::Idle))
        .map(|(e, _, area)| (e, area.bounds()))
        .collect();

    let active_yards: Vec<(Entity, Yard)> = q_yards.iter().map(|(e, y)| (e, y.clone())).collect();
    let all_owners = collect_all_area_owners(&active_familiars, &active_yards);

    if all_owners.is_empty() {
        return;
    }

    let mut desired_requests: HashMap<(Entity, ResourceType), (Entity, u32, Vec2)> =
        HashMap::new();

    for (site_entity, transform, site) in q_sites.iter() {
        if site.phase != SoulSpaPhase::Constructing {
            continue;
        }

        let site_pos = transform.translation.truncate();
        let Some((fam_entity, _)) = find_owner(site_pos, &all_owners) else {
            continue;
        };

        // Count in-flight deliveries for this site
        let in_flight: u32 = q_existing
            .iter()
            .filter(|(_, target, req, workers)| {
                target.0 == site_entity
                    && req.kind == TransportRequestKind::DeliverToSoulSpa
                    && workers.map(|w| w.len()).unwrap_or(0) > 0
            })
            .map(|(_, _, _, workers)| workers.map(|w| w.len() as u32).unwrap_or(0))
            .sum();

        let remaining = site
            .bones_required
            .saturating_sub(site.bones_delivered)
            .saturating_sub(in_flight);

        if remaining == 0 {
            continue;
        }

        desired_requests.insert(
            (site_entity, ResourceType::Bone),
            (fam_entity, remaining.min(4), site_pos),
        );
    }

    sync_construction_requests(
        &mut commands,
        &q_existing,
        &desired_requests,
        RequestSyncSpec {
            expected_kind: TransportRequestKind::DeliverToSoulSpa,
            request_name: "TransportReq(SoulSpa Bone)",
            request_kind: TransportRequestKind::DeliverToSoulSpa,
        },
        |target: &TargetSoulSpaSite| target.0,
        TargetSoulSpaSite,
        |_| FLOOR_CONSTRUCTION_PRIORITY,
    );
}
