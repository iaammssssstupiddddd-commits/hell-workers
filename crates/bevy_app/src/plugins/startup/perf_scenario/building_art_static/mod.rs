//! Paused, mixed-building reference. This is NOT an active-production benchmark.
//! Production factories and domain projections run during setup only; inspection
//! never repairs a specimen once measurement readiness has been published.

pub(crate) mod active;
mod layout;
mod seed;

use std::collections::{HashMap, HashSet};

use bevy::{app::AppExit, ecs::system::SystemParam, prelude::*};
use hw_core::relationships::{RestAreaOccupants, StoredItems, TaskWorkers};
use hw_core::visual_mirror::{building::MudMixerVisualState, energy::PoweredVisualState};
use hw_energy::{SoulSpaPhase, SoulSpaSite, SoulSpaTile};
use hw_jobs::{Building, BuildingType, Door, DoorState};
use hw_visual::TopDownStructuralMaterial;
use hw_visual::visual3d::Building3dVisual;
use hw_world::{WorldMapRead, WorldMapWrite};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{PerfScenarioConfig, PerfWorkload, fixture::PerfScenarioApplied};
use crate::assets::door_asset_set::{
    DoorAssetAuthority, DoorAssetReadiness, DoorAssetReadinessState, ProductionDoorAssetPool,
    ProductionDoorMaterialPool,
};
use crate::plugins::startup::Building3dHandles;
use layout::Specimen;

pub(crate) use seed::{seed_building_art_static_system, setup_building_art_static_system};

/// Keep the paused fixture's calls out of the production systems' type sets.
/// LogicPlugin orders those exact system types; registering a second instance
/// in Update makes that ordering ambiguous even when this workload is disabled.
/// Cached calls retain change-detection/Local state through the setup frames.
pub(crate) fn settle_refine_visuals(world: &mut World) {
    world
        .run_system_cached(hw_jobs::sync_refine_activity_index_system)
        .expect("profiling setup has the production refine resources");
    world
        .run_system_cached(hw_jobs::visual_sync::sync_mud_mixer_active_system)
        .expect("profiling setup has the production visual resources");
    settle_completion_effects(world);
}

/// A paused reference must not retain the factories' one-shot completion text
/// or completion bounce forever. This is setup, never per-frame repair.
fn settle_completion_effects(world: &mut World) {
    use hw_visual::blueprint::{BuildingBounceEffect, CompletionText};
    let state = world.resource::<BuildingArtStaticState>();
    if state.phase != Phase::Seeded || state.completion_effects_settled {
        return;
    }
    let texts = world
        .query_filtered::<Entity, With<CompletionText>>()
        .iter(world)
        .collect::<Vec<_>>();
    for entity in texts {
        world.despawn(entity);
    }
    let owners = world
        .query_filtered::<(Entity, &mut Transform), With<BuildingBounceEffect>>()
        .iter_mut(world)
        .map(|(entity, mut transform)| {
            transform.scale = Vec3::ONE;
            entity
        })
        .collect::<Vec<_>>();
    for owner in owners {
        world.entity_mut(owner).remove::<BuildingBounceEffect>();
    }
    world
        .resource_mut::<BuildingArtStaticState>()
        .completion_effects_settled = true;
}

#[derive(Default, PartialEq, Eq, Debug)]
enum Phase {
    #[default]
    Inactive,
    Spawned,
    Seeded,
    Ready,
    Failed,
}

#[derive(Resource, Default)]
pub(crate) struct BuildingArtStaticState {
    phase: Phase,
    specs: Vec<Specimen>,
    actors: Vec<Entity>,
    owners: Vec<Entity>,
    evidence: Option<Value>,
    stable_frames: u64,
    completion_effects_settled: bool,
}

pub(crate) fn should_settle_building_art_static(
    config: Res<PerfScenarioConfig>,
    state: Res<BuildingArtStaticState>,
) -> bool {
    config.enabled()
        && matches!(
            config.workload(),
            PerfWorkload::BuildingArtStatic | PerfWorkload::BuildingArtActive
        )
        && matches!(state.phase, Phase::Spawned | Phase::Seeded)
}

fn fail(state: &mut BuildingArtStaticState, exit: &mut MessageWriter<AppExit>, reason: String) {
    error!("BUILDING_ART_STATIC: {reason}");
    state.phase = Phase::Failed;
    exit.write(AppExit::error());
}

type BuildingQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Building,
        &'static Transform,
        Option<&'static Door>,
        Option<&'static StoredItems>,
        Option<&'static RestAreaOccupants>,
        Option<&'static MudMixerVisualState>,
        Option<&'static PoweredVisualState>,
    ),
>;

type VisualQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Building3dVisual,
        &'static Mesh3d,
        &'static MeshMaterial3d<TopDownStructuralMaterial>,
        &'static ViewVisibility,
    ),
>;
type SpriteQuery<'w, 's> = Query<
    'w,
    's,
    (&'static ChildOf, &'static Sprite, &'static ViewVisibility),
    With<hw_visual::layer::VisualLayerKind>,
>;
type CompletionEffects<'w, 's> = Query<
    'w,
    's,
    (),
    Or<(
        With<hw_visual::blueprint::CompletionText>,
        With<hw_visual::blueprint::BuildingBounceEffect>,
    )>,
>;

#[derive(SystemParam)]
pub(crate) struct InspectParams<'w, 's> {
    config: Res<'w, PerfScenarioConfig>,
    state: ResMut<'w, BuildingArtStaticState>,
    applied: ResMut<'w, PerfScenarioApplied>,
    world_map: WorldMapRead<'w>,
    buildings: BuildingQuery<'w, 's>,
    visuals: VisualQuery<'w, 's>,
    sprites: SpriteQuery<'w, 's>,
    meshes: Res<'w, Assets<Mesh>>,
    materials: Res<'w, Assets<TopDownStructuralMaterial>>,
    images: Res<'w, Assets<Image>>,
    handles: Res<'w, Building3dHandles>,
    door_pool: Res<'w, ProductionDoorAssetPool>,
    door_material: Res<'w, ProductionDoorMaterialPool>,
    door_readiness: Res<'w, DoorAssetReadiness>,
    stockpiles: Query<'w, 's, &'static hw_logistics::Stockpile>,
    companions: Query<'w, 's, &'static hw_logistics::BelongsTo, With<hw_logistics::BucketStorage>>,
    camera: Query<'w, 's, &'static Transform, With<hw_ui::camera::MainCamera>>,
    completion_effects: CompletionEffects<'w, 's>,
    spas: Query<'w, 's, &'static SoulSpaSite>,
    tiles: Query<'w, 's, (&'static SoulSpaTile, Option<&'static TaskWorkers>)>,
    time: Res<'w, Time<Virtual>>,
    exit: MessageWriter<'w, AppExit>,
}

pub(crate) fn inspect_building_art_static_system(mut params: InspectParams) {
    if !params.config.enabled()
        || !matches!(
            params.config.workload(),
            PerfWorkload::BuildingArtStatic | PerfWorkload::BuildingArtActive
        )
        || !matches!(params.state.phase, Phase::Seeded | Phase::Ready)
        || (params.config.workload() == PerfWorkload::BuildingArtActive
            && params.state.phase == Phase::Ready)
    {
        return;
    }
    match inspect(&params) {
        Ok(None) => {
            if params.state.phase == Phase::Ready {
                fail(
                    &mut params.state,
                    &mut params.exit,
                    "resident/visible presentation disappeared after readiness".into(),
                );
            }
        }
        Ok(Some(evidence)) => {
            if params.state.evidence.is_none() {
                params.state.evidence = Some(evidence);
                params.state.phase = Phase::Ready;
                params.applied.workload =
                    params.config.workload() == PerfWorkload::BuildingArtStatic;
            }
            params.state.stable_frames += 1;
        }
        Err(reason) => fail(&mut params.state, &mut params.exit, reason),
    }
}

fn inspect(params: &InspectParams) -> Result<Option<Value>, String> {
    if !params.time.is_paused() {
        return Err("static fixture Virtual Time is not paused".into());
    }
    if !params.state.completion_effects_settled || !params.completion_effects.is_empty() {
        return Err("static fixture has unfinished completion effects".into());
    }
    let camera = params
        .camera
        .single()
        .map_err(|_| "main camera count differs")?;
    if camera.translation.truncate() != hw_world::WorldMap::grid_to_world(46, 37)
        || camera.scale != Vec3::splat(layout::CAMERA_SCALE)
    {
        return Err(format!(
            "static fixture camera changed: translation={:?}, scale={:?}",
            camera.translation, camera.scale
        ));
    }
    let Some(door_assets) = params.door_pool.resolved.as_ref() else {
        return Ok(None);
    };
    match &params.door_readiness.state {
        DoorAssetReadinessState::Loading => return Ok(None),
        DoorAssetReadinessState::Eligible(identity)
            if identity == &door_assets.identity
                && identity.authority == DoorAssetAuthority::ReleaseApproved
                && identity.asset_set_generation == 7 => {}
        _ => return Err("Door reference requires the approved generation 7 release".into()),
    }
    let Some(door_material) = params.door_material.material.as_ref() else {
        return Ok(None);
    };
    if params.door_material.identity.as_ref() != Some(&door_assets.identity) {
        return Ok(None);
    }
    let copies = layout::copies(params.config.size());
    if params.buildings.iter().count() != copies * 13 || params.state.owners.len() != copies * 9 {
        return Err("target/support building inventory differs".into());
    }
    let mut visuals = HashMap::<Entity, Vec<_>>::new();
    for (entity, visual, mesh, material, visibility) in &params.visuals {
        visuals
            .entry(visual.owner)
            .or_default()
            .push((entity, mesh, material, visibility));
    }
    let mut sprites = HashMap::<Entity, Vec<_>>::new();
    for (parent, sprite, visibility) in &params.sprites {
        sprites
            .entry(parent.parent())
            .or_default()
            .push((sprite, visibility));
    }
    let mut active_meshes = HashSet::new();
    // Full JSON is allocated only for the initial checkpoint, never per frame.
    let mut records = params.state.evidence.is_none().then(Vec::new);
    for (spec, &owner) in params.state.specs.iter().zip(&params.state.owners) {
        let (building, transform, door, water, resting, mixer, lamp) = params
            .buildings
            .get(owner)
            .map_err(|_| format!("missing {:?}/{}", spec.kind, spec.ordinal))?;
        if building.kind != spec.kind
            || building.is_provisional
            || transform.translation.truncate() != spec.center
            || spec
                .tiles
                .iter()
                .any(|&grid| params.world_map.building_entity(grid) != Some(owner))
        {
            return Err(format!(
                "owner/footprint mismatch: {:?}/{}",
                spec.kind, spec.ordinal
            ));
        }
        let structural = !matches!(
            spec.kind,
            BuildingType::SandPile
                | BuildingType::BonePile
                | BuildingType::WheelbarrowParking
                | BuildingType::OutdoorLamp
        );
        let owner_visuals = visuals.get(&owner).map(Vec::as_slice).unwrap_or_default();
        if owner_visuals.len() != usize::from(structural) {
            return Err(format!(
                "visual count differs: {:?}/{}",
                spec.kind, spec.ordinal
            ));
        }
        for (_, mesh, material, visibility) in owner_visuals {
            let expected_mesh = match spec.kind {
                BuildingType::Door => &door_assets.meshes[0],
                _ => &params.handles.equipment_2x2_mesh,
            };
            let expected_material = match spec.kind {
                BuildingType::Door => door_material,
                BuildingType::Tank if spec.quarter() == 1 => &params.handles.tank_partial_material,
                BuildingType::Tank if spec.quarter() >= 2 => &params.handles.tank_full_material,
                BuildingType::MudMixer if spec.quarter() >= 2 => {
                    &params.handles.mixer_active_material
                }
                BuildingType::MudMixer => &params.handles.mixer_idle_material,
                _ => &params.handles.equipment_material,
            };
            if &mesh.0 != expected_mesh
                || &material.0 != expected_material
                || params.meshes.get(&mesh.0).is_none()
                || params.materials.get(&material.0).is_none()
                || !visibility.get()
            {
                return Ok(None);
            }
            active_meshes.insert(mesh.id());
        }
        let owner_sprites = sprites.get(&owner).map(Vec::as_slice).unwrap_or_default();
        if owner_sprites.len() != usize::from(!structural) {
            return Err("foreground presentation count differs".into());
        }
        for (sprite, visibility) in owner_sprites {
            if params.images.get(&sprite.image).is_none() || !visibility.get() {
                return Ok(None);
            }
        }
        match spec.kind {
            BuildingType::Tank => {
                let (x, y) = spec.companion();
                for grid in [(x, y), (x + 1, y)] {
                    let storage = params
                        .world_map
                        .stockpile_entity(grid)
                        .ok_or("missing Tank companion")?;
                    if params
                        .companions
                        .get(storage)
                        .map_or(true, |parent| parent.0 != owner)
                    {
                        return Err("Tank companion owner differs".into());
                    }
                }
                let actual = water.map_or(0, StoredItems::len);
                if actual != spec.water_count()
                    || params
                        .stockpiles
                        .get(owner)
                        .map_or(true, |stockpile| stockpile.capacity != 50)
                {
                    return Err("Tank water count differs".into());
                }
            }
            BuildingType::MudMixer => {
                let active = spec.quarter() >= 2;
                if mixer.is_none_or(|m| m.is_active != active) {
                    return Ok(None);
                }
            }
            BuildingType::RestArea => {
                let actual = resting.map_or(0, RestAreaOccupants::len);
                if actual != spec.rest_count() {
                    return Err("RestArea occupancy differs".into());
                }
            }
            BuildingType::SoulSpa => {
                let spa = params.spas.get(owner).map_err(|_| "missing SoulSpaSite")?;
                if spa.phase != SoulSpaPhase::Operational {
                    return Ok(None);
                }
                let mut mask = 0u8;
                let mut count = 0;
                for (tile, workers) in &params.tiles {
                    if tile.parent_site != owner {
                        continue;
                    }
                    count += 1;
                    let index = spec
                        .tiles
                        .iter()
                        .position(|&grid| grid == tile.grid_pos)
                        .ok_or("Spa tile outside footprint")?;
                    if workers.is_some_and(|w| !w.is_empty()) {
                        mask |= 1 << index;
                    }
                }
                if count != 4 || mask != spec.spa_mask() {
                    return Err("Spa tile count/mask differs".into());
                }
            }
            BuildingType::OutdoorLamp => {
                let powered = spec.quarter() >= 2;
                if lamp.is_none_or(|l| l.is_powered != powered) {
                    return Ok(None);
                }
            }
            BuildingType::Door => {
                for grid in spec.supports() {
                    for (entity, kind) in [
                        (params.world_map.building_entity(grid), BuildingType::Wall),
                        (params.world_map.floor_entity(grid), BuildingType::Floor),
                    ] {
                        let entity = entity.ok_or("missing Door support")?;
                        if params
                            .buildings
                            .get(entity)
                            .map_or(true, |(b, ..)| b.kind != kind || b.is_provisional)
                        {
                            return Err("Door support differs".into());
                        }
                    }
                }
                if door.is_none_or(|d| d.state != DoorState::Closed) {
                    return Err("Door must remain closed".into());
                }
            }
            _ => {}
        };
        if let Some(records) = records.as_mut() {
            let state = match spec.kind {
                BuildingType::Tank => json!({"stored_water": spec.water_count(), "capacity": 50}),
                BuildingType::MudMixer => json!({"refining": spec.quarter() >= 2}),
                BuildingType::RestArea => json!({"occupants": spec.rest_count()}),
                BuildingType::SoulSpa => json!({"operational": true, "mask": spec.spa_mask()}),
                BuildingType::OutdoorLamp => json!({"powered": spec.quarter() >= 2}),
                BuildingType::Door => json!({"state": "Closed", "axis": "EastWest"}),
                _ => json!({"state": "Static"}),
            };
            records.push(json!({"kind": format!("{:?}", spec.kind), "ordinal": spec.ordinal,
                "anchor": spec.anchor, "tiles": spec.tiles, "center": [spec.center.x, spec.center.y], "state": state}));
        }
    }
    if active_meshes.len() != 2 {
        return Err("legacy shared mesh count differs".into());
    }
    let Some(records) = records else {
        return Ok(Some(Value::Null));
    };
    Ok(Some(
        json!({"records": records, "target_count": params.state.specs.len(),
        "target_structural_roots": layout::copies(params.config.size()) * 5,
        "target_foreground_owners": layout::copies(params.config.size()) * 4,
        "target_active_unique_meshes": active_meshes.len(), "souls": params.state.actors.len(), "completion_effects": 0}),
    ))
}

impl BuildingArtStaticState {
    pub(super) fn write_sidecar(&self, config: &PerfScenarioConfig) -> std::io::Result<()> {
        if config.workload() != PerfWorkload::BuildingArtStatic {
            return Ok(());
        }
        if self.phase != Phase::Ready || self.stable_frames < 2 {
            return Err(std::io::Error::other(
                "building-art-static fixture did not remain ready",
            ));
        }
        let evidence = self
            .evidence
            .as_ref()
            .ok_or_else(|| std::io::Error::other("missing fixture evidence"))?;
        let bytes = serde_json::to_vec(evidence)?;
        let summary = json!({"schema_version": 1, "contract_id": "building-art-static-nine-v2",
            "evidence_kind": "paused-static-only", "active_simulation_evidence": false,
            "camera_scale": layout::CAMERA_SCALE, "stable_frames": self.stable_frames,
            "layout_sha256": hw_infra::lighting::digest_hex(&Sha256::digest(&bytes).into()), "initial": evidence, "final": evidence});
        let directory = super::output::perf_output_directory(config);
        std::fs::create_dir_all(&directory)?;
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join("building_art_static.json"))?;
        serde_json::to_writer_pretty(file, &summary).map_err(std::io::Error::other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_visual::blueprint::{BuildingBounceEffect, CompletionText};
    use hw_visual::floating_text::FloatingText;

    #[test]
    fn completion_effects_settle_once_and_never_repair_measured_state() {
        let mut world = World::new();
        world.init_resource::<BuildingArtStaticState>();
        let owner = world
            .spawn((
                BuildingBounceEffect::completion(),
                Transform::from_scale(Vec3::splat(1.2)),
            ))
            .id();
        let text = world
            .spawn(CompletionText {
                floating_text: FloatingText {
                    lifetime: 1.0,
                    config: default(),
                },
            })
            .id();
        settle_completion_effects(&mut world);
        assert!(world.get::<BuildingBounceEffect>(owner).is_some());
        assert!(world.get_entity(text).is_ok());

        world.resource_mut::<BuildingArtStaticState>().phase = Phase::Seeded;
        settle_completion_effects(&mut world);
        assert!(world.get::<BuildingBounceEffect>(owner).is_none());
        assert_eq!(world.get::<Transform>(owner).unwrap().scale, Vec3::ONE);
        assert!(world.get_entity(text).is_err());

        world
            .entity_mut(owner)
            .insert(BuildingBounceEffect::completion());
        settle_completion_effects(&mut world);
        assert!(
            world.get::<BuildingBounceEffect>(owner).is_some(),
            "a later transient must remain observable to the fail-closed inspector"
        );
    }
}
