//! M5-only presentation gate layered on top of the frozen wall-density fixture.

use std::collections::{BTreeMap, HashMap, HashSet};

use bevy::ecs::system::SystemParam;
use bevy::mesh::{Mesh, Mesh3d};
use bevy::pbr::MeshMaterial3d;
use bevy::prelude::*;
use hw_visual::wall_connection::WallMeshFamily;
use hw_visual::{
    Building3dVisual, TopDownStructuralMaterial, Wall3dPresentationMode, Wall3dPresentationState,
};
use serde::Serialize;

use super::PerfWallPhase;
use super::wall_density_fixture::WallDensityFixtureState;
use super::{PerfScenarioConfig, PerfWallPresentation, PerfWorkload};
use crate::assets::wall_asset_set::{
    ProductionWallAssetPool, ProductionWallMaterialPool, WallAssetReadiness,
    WallProductionActivation, WallProductionActivationState, WallProductionFallbackReason,
};
use crate::plugins::startup::Building3dHandles;

const SIDECAR_SCHEMA_VERSION: u32 = 1;
const MAX_TRIANGLES_PER_PRODUCTION_MESH: usize = 350;

type WallPresentationQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Building3dVisual,
        &'static Mesh3d,
        &'static MeshMaterial3d<TopDownStructuralMaterial>,
        &'static Wall3dPresentationState,
    ),
>;

#[derive(SystemParam)]
pub(crate) struct WallDensityPresentationParams<'w, 's> {
    visuals: WallPresentationQuery<'w, 's>,
    meshes: Res<'w, Assets<Mesh>>,
    materials: Res<'w, Assets<TopDownStructuralMaterial>>,
    fallback: Res<'w, Building3dHandles>,
    production_assets: Res<'w, ProductionWallAssetPool>,
    production_materials: Res<'w, ProductionWallMaterialPool>,
    readiness: Res<'w, WallAssetReadiness>,
    activation: Res<'w, WallProductionActivation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct WallDensityPresentationEvidence {
    schema_version: u32,
    pub(super) expected_mode: &'static str,
    phase: &'static str,
    target_wall_count: usize,
    visual_count: usize,
    production_count: usize,
    fallback_count: usize,
    pub(super) active_mesh_count: usize,
    active_material_count: usize,
    active_mesh_material_pair_count: usize,
    resident_production_mesh_count: usize,
    resident_production_material_count: usize,
    resident_fallback_mesh_count: usize,
    resident_fallback_material_count: usize,
    total_wall_mesh_pool_count: usize,
    total_wall_material_pool_count: usize,
    production_materials_lit: bool,
    max_production_mesh_triangles: usize,
    production_mesh_triangles: Vec<usize>,
    family_counts: BTreeMap<&'static str, usize>,
    rotation_counts: BTreeMap<u8, usize>,
    session_id: u64,
    readiness_revision: u64,
    decision_revision: u64,
    presentation_revision: u64,
    pub(super) asset_set_generation: u64,
    pub(super) authority: crate::assets::wall_asset_set::WallAssetAuthority,
    pub(super) manifest_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WallDensityPresentationReadiness {
    Pending,
    Ready(Box<WallDensityPresentationEvidence>),
}

pub(crate) fn inspect_wall_density_presentation(
    config: &PerfScenarioConfig,
    fixture: &WallDensityFixtureState,
    params: &WallDensityPresentationParams<'_, '_>,
) -> Result<WallDensityPresentationReadiness, String> {
    let Some(expected) = config.wall_presentation() else {
        return Ok(WallDensityPresentationReadiness::Pending);
    };
    if config.workload != PerfWorkload::WallDensity {
        return Err("Wall presentation evidence requires wall-density".to_string());
    }
    let evidence = fixture.renderdoc_evidence()?;
    let activation_identity = match (&params.activation.state, expected) {
        (
            WallProductionActivationState::ReadyToApply {
                asset_set_generation,
                authority,
                manifest_sha256,
                ..
            },
            PerfWallPresentation::Production,
        ) => (*asset_set_generation, *authority, manifest_sha256.clone()),
        (WallProductionActivationState::Fallback(WallProductionFallbackReason::Loading), _) => {
            return Ok(WallDensityPresentationReadiness::Pending);
        }
        (
            WallProductionActivationState::Fallback(
                WallProductionFallbackReason::TopologyUnavailable,
            ),
            PerfWallPresentation::Production,
        ) => return Ok(WallDensityPresentationReadiness::Pending),
        (
            WallProductionActivationState::Fallback(
                WallProductionFallbackReason::CandidateDisabled,
            ),
            PerfWallPresentation::FallbackControl,
        ) => {
            let resolved = params.production_assets.resolved.as_ref().ok_or_else(|| {
                "fallback control has no resolved production asset pool".to_string()
            })?;
            (
                resolved.identity.asset_set_generation,
                resolved.identity.authority,
                resolved.identity.manifest_sha256.clone(),
            )
        }
        (state, _) => {
            return Err(format!(
                "Wall presentation activation cannot satisfy {}: {state:?}",
                expected.as_str()
            ));
        }
    };

    let resolved = params
        .production_assets
        .resolved
        .as_ref()
        .ok_or_else(|| "production Wall asset pool is unresolved".to_string())?;
    if resolved.identity.asset_set_generation != activation_identity.0
        || resolved.identity.authority != activation_identity.1
        || resolved.identity.manifest_sha256 != activation_identity.2
        || params.production_materials.identity.as_ref() != Some(&resolved.identity)
    {
        return Err("Wall presentation asset identities differ".to_string());
    }

    let target_entities: HashSet<_> = evidence.target_entities.iter().copied().collect();
    let structural_visual_count = params.visuals.iter().count();
    if structural_visual_count != evidence.target_wall_count {
        return Err(format!(
            "Wall presentation world contains {structural_visual_count} structural visuals for {} targets",
            evidence.target_wall_count
        ));
    }
    let mut owners = HashMap::<Entity, usize>::new();
    let mut production_count = 0usize;
    let mut fallback_count = 0usize;
    let mut active_meshes = HashSet::new();
    let mut active_materials = HashSet::new();
    let mut active_pairs = HashSet::new();
    let mut family_counts = BTreeMap::new();
    let mut rotation_counts = BTreeMap::new();
    let mut presentation_revisions = HashSet::new();
    for (visual, mesh, material, state) in &params.visuals {
        if !target_entities.contains(&visual.owner) {
            continue;
        }
        *owners.entry(visual.owner).or_default() += 1;
        match state.mode {
            Wall3dPresentationMode::Production => production_count += 1,
            Wall3dPresentationMode::Fallback => fallback_count += 1,
        }
        active_meshes.insert(mesh.0.id());
        active_materials.insert(material.0.id());
        active_pairs.insert((mesh.0.id(), material.0.id()));
        *family_counts
            .entry(family_name(state.topology.family))
            .or_default() += 1;
        *rotation_counts
            .entry(state.topology.quarter_turns_y.get())
            .or_default() += 1;
        presentation_revisions.insert(state.activation_revision);
    }
    if owners.len() != evidence.target_wall_count || owners.values().any(|count| *count != 1) {
        return Err(format!(
            "Wall presentation owner coverage differs: owners={}/{}, duplicates={}",
            owners.len(),
            evidence.target_wall_count,
            owners.values().filter(|count| **count != 1).count()
        ));
    }
    if presentation_revisions.len() != 1 {
        return Err("Wall presentation revisions are mixed".to_string());
    }
    let expected_count = evidence.target_wall_count;
    match expected {
        PerfWallPresentation::Production
            if production_count == 0 && fallback_count == expected_count =>
        {
            return Ok(WallDensityPresentationReadiness::Pending);
        }
        PerfWallPresentation::Production
            if production_count != expected_count || fallback_count != 0 =>
        {
            return Err("Wall production presentation is mixed".to_string());
        }
        PerfWallPresentation::FallbackControl
            if fallback_count != expected_count || production_count != 0 =>
        {
            return Err("Wall fallback-control presentation is mixed".to_string());
        }
        _ => {}
    }

    let expected_material = match (expected, evidence.phase) {
        (PerfWallPresentation::Production, PerfWallPhase::Completed) => {
            params.production_materials.complete.as_ref()
        }
        (PerfWallPresentation::Production, PerfWallPhase::Provisional) => {
            params.production_materials.provisional.as_ref()
        }
        (PerfWallPresentation::FallbackControl, PerfWallPhase::Completed) => {
            Some(&params.fallback.wall_material)
        }
        (PerfWallPresentation::FallbackControl, PerfWallPhase::Provisional) => {
            Some(&params.fallback.wall_provisional_material)
        }
    }
    .ok_or_else(|| "expected Wall material is absent".to_string())?;
    let expected_meshes = match expected {
        PerfWallPresentation::Production => resolved
            .meshes
            .iter()
            .map(|handle| handle.id())
            .collect::<HashSet<_>>(),
        PerfWallPresentation::FallbackControl => HashSet::from([params.fallback.wall_mesh.id()]),
    };
    if !active_meshes.iter().all(|id| expected_meshes.contains(id))
        || active_materials != HashSet::from([expected_material.id()])
    {
        return Err("Wall presentation active handles differ".to_string());
    }

    let production_mesh_triangles = resolved
        .meshes
        .iter()
        .map(|handle| {
            let mesh = params
                .meshes
                .get(handle)
                .ok_or_else(|| "production Wall mesh is not resident".to_string())?;
            let index_count = mesh
                .indices()
                .map_or_else(|| mesh.count_vertices(), |indices| indices.len());
            if index_count % 3 != 0 {
                return Err("production Wall mesh is not triangular".to_string());
            }
            Ok(index_count / 3)
        })
        .collect::<Result<Vec<_>, String>>()?;
    let max_triangles = production_mesh_triangles.iter().copied().max().unwrap_or(0);
    if max_triangles > MAX_TRIANGLES_PER_PRODUCTION_MESH {
        return Err(format!(
            "production Wall mesh exceeds triangle budget: {max_triangles}"
        ));
    }
    let production_material_handles = [
        params.production_materials.complete.as_ref(),
        params.production_materials.provisional.as_ref(),
    ];
    let resident_production_material_count = production_material_handles
        .iter()
        .filter(|handle| handle.is_some_and(|handle| params.materials.contains(handle.id())))
        .count();
    let resident_fallback_material_count = [
        &params.fallback.wall_material,
        &params.fallback.wall_provisional_material,
    ]
    .into_iter()
    .filter(|handle| params.materials.contains(handle.id()))
    .count();
    let resident_fallback_mesh_count =
        usize::from(params.meshes.contains(params.fallback.wall_mesh.id()));
    let resident_production_mesh_count = resolved
        .meshes
        .iter()
        .filter(|handle| params.meshes.contains(handle.id()))
        .count();
    let production_materials_lit = production_material_handles.iter().all(|handle| {
        handle
            .and_then(|handle| params.materials.get(handle))
            .is_some_and(|material| !material.base.unlit)
    });
    let total_wall_mesh_pool_count = resolved
        .meshes
        .iter()
        .map(|handle| handle.id())
        .chain(std::iter::once(params.fallback.wall_mesh.id()))
        .collect::<HashSet<_>>()
        .len();
    let total_wall_material_pool_count = production_material_handles
        .into_iter()
        .flatten()
        .map(|handle| handle.id())
        .chain([
            params.fallback.wall_material.id(),
            params.fallback.wall_provisional_material.id(),
        ])
        .collect::<HashSet<_>>()
        .len();
    if resident_production_mesh_count != 6
        || resident_production_material_count != 2
        || resident_fallback_mesh_count != 1
        || resident_fallback_material_count != 2
        || total_wall_mesh_pool_count != 7
        || total_wall_material_pool_count != 4
        || !production_materials_lit
    {
        return Err("Wall presentation finite pool residency differs".to_string());
    }
    let presentation_revision = *presentation_revisions
        .iter()
        .next()
        .expect("one presentation revision was established");
    Ok(WallDensityPresentationReadiness::Ready(Box::new(
        WallDensityPresentationEvidence {
            schema_version: SIDECAR_SCHEMA_VERSION,
            expected_mode: expected.as_str(),
            phase: evidence.phase.as_str(),
            target_wall_count: expected_count,
            visual_count: structural_visual_count,
            production_count,
            fallback_count,
            active_mesh_count: active_meshes.len(),
            active_material_count: active_materials.len(),
            active_mesh_material_pair_count: active_pairs.len(),
            resident_production_mesh_count,
            resident_production_material_count,
            resident_fallback_mesh_count,
            resident_fallback_material_count,
            total_wall_mesh_pool_count,
            total_wall_material_pool_count,
            production_materials_lit,
            max_production_mesh_triangles: max_triangles,
            production_mesh_triangles,
            family_counts,
            rotation_counts,
            session_id: params.readiness.session_id,
            readiness_revision: params.readiness.activation_revision,
            decision_revision: params.activation.decision_revision,
            presentation_revision,
            asset_set_generation: activation_identity.0,
            authority: activation_identity.1,
            manifest_sha256: activation_identity.2,
        },
    )))
}

pub(crate) fn presentation_sidecar(
    initial: &WallDensityPresentationEvidence,
    final_evidence: &WallDensityPresentationEvidence,
) -> Result<serde_json::Value, String> {
    if initial != final_evidence {
        return Err("Wall presentation changed during the measurement session".to_string());
    }
    Ok(serde_json::json!({
        "schema_version": SIDECAR_SCHEMA_VERSION,
        "stable": true,
        "initial": initial,
        "final": final_evidence,
    }))
}

const fn family_name(family: WallMeshFamily) -> &'static str {
    match family {
        WallMeshFamily::Isolated => "isolated",
        WallMeshFamily::End => "end",
        WallMeshFamily::Straight => "straight",
        WallMeshFamily::Corner => "corner",
        WallMeshFamily::TJunction => "t_junction",
        WallMeshFamily::Cross => "cross",
    }
}

#[cfg(test)]
const fn expected_active_counts(
    presentation: PerfWallPresentation,
    _phase: PerfWallPhase,
) -> (usize, usize) {
    match presentation {
        PerfWallPresentation::Production => (6, 1),
        PerfWallPresentation::FallbackControl => (1, 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formal_modes_have_bounded_active_pools() {
        for phase in [PerfWallPhase::Completed, PerfWallPhase::Provisional] {
            assert_eq!(
                expected_active_counts(PerfWallPresentation::Production, phase),
                (6, 1)
            );
            assert_eq!(
                expected_active_counts(PerfWallPresentation::FallbackControl, phase),
                (1, 1)
            );
        }
    }
}
