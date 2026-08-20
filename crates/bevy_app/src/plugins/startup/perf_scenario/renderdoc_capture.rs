//! Deterministic RenderDoc capture for the frozen RtT-light profiling fixture.

use super::super::rtt_composite::{
    RTT_COMPOSITE_BIND_SET_OR_SPACE, RTT_COMPOSITE_SCENE_SAMPLER_BINDING,
    RTT_COMPOSITE_SCENE_TEXTURE_BINDING,
};
use super::*;
use bevy::asset::{AssetId, LoadState};
use bevy::camera::NormalizedRenderTarget;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::diagnostic::FrameCount;
use bevy::ecs::system::SystemParam;
use bevy::light::NotShadowCaster;
use bevy::render::camera::ExtractedCamera;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::{CachedPipelineState, PipelineCache, PipelineDescriptor};
use bevy::render::renderer::{RenderAdapterInfo, RenderDevice};
use bevy::render::texture::GpuImage;
use bevy::render::view::window::ExtractedWindows;
use bevy::render::{Render, RenderApp, RenderSystems};
use bevy::shader::{Shader, ShaderCacheError};
use libloading::Library;
use serde::Serialize;
use std::ffi::{CStr, CString, c_void};
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::{Arc, Mutex};

const RENDERDOC_SETTLE_FRAMES: u32 = 4;
const RENDERDOC_GATE_TIMEOUT_FRAMES: u32 = 600;
const RENDERDOC_CHECKPOINT_NAME: &str = "indoor-light-fixture-ready-v1";
const RENDERDOC_REQUESTED_API_VERSION: &str = "1.6.0";
const RENDERDOC_RUNTIME_CHECKPOINT_SCHEMA_VERSION: u32 = 4;
const RENDERDOC_SELECTOR_STRATEGY: &str = "wgpu_device_null_window";
const SIMULATION_TICK_SOURCE: &str = "perf_capture.fixed_update_tick";
const RTT_SCENE_LABEL: &str = "hell-workers-rtt-scene";
const TOPDOWN_STRUCTURAL_SHADER: &str =
    include_str!("../../../../../../assets/shaders/topdown_structural_material.wgsl");
const RENDERDOC_RECEIVER_SHADER_PATHS: [&str; 8] = [
    "shaders/topdown_structural_material.wgsl",
    "shaders/topdown_structural_material_prepass.wgsl",
    "shaders/terrain_surface_material.wgsl",
    "shaders/terrain_surface_material_lod1_lite.wgsl",
    "shaders/terrain_surface_material_lod2.wgsl",
    "shaders/terrain_surface_material_prepass.wgsl",
    "shaders/shadow_style.wgsl",
    "shaders/indoor_light_field.wgsl",
];
const RENDERDOC_RECEIVER_FRAGMENT_SHADER_PATHS: [&str; 4] = [
    RENDERDOC_RECEIVER_SHADER_PATHS[0],
    RENDERDOC_RECEIVER_SHADER_PATHS[2],
    RENDERDOC_RECEIVER_SHADER_PATHS[3],
    RENDERDOC_RECEIVER_SHADER_PATHS[4],
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CpuCheckpointSignature {
    checksum: u64,
    scene_target: AssetId<Image>,
    mask_target: Option<AssetId<Image>>,
    render_inventory: PerfRenderInventory,
    p02_presentation: Option<PerfP02Presentation>,
}

#[derive(Clone, Debug)]
struct StableRenderDocCheckpoint {
    generation: u64,
    simulation_tick: u64,
    scene_target: AssetId<Image>,
    mask_target: Option<AssetId<Image>>,
    render_inventory: PerfRenderInventory,
    p02_presentation: Option<PerfP02Presentation>,
    runtime_field: Option<RuntimeFieldEvidence>,
    gpu_light_field: Option<RuntimeGpuLightFieldEvidence>,
    cross_consumer: Option<RuntimeCrossConsumerEvidence>,
    receiver_fragment_shaders: Option<[AssetId<Shader>; 4]>,
    receiver_import_shaders: Option<[(AssetId<Shader>, Shader); 2]>,
    fixture: RuntimeFixtureEvidence,
}

#[derive(Clone, Debug, Serialize)]
struct RuntimeFixtureEvidence {
    fixture_checksum: &'static str,
    rooms: usize,
    completed_floors: usize,
    completed_walls: usize,
    doors: usize,
    supplied_lamp_candidates: usize,
    unsupplied_lamp_candidates: usize,
}

#[derive(Clone, Debug, Serialize)]
struct RuntimeFieldEvidence {
    world_epoch: u64,
    field_revision: u64,
    field_checksum: String,
    typed_emitter_components: u32,
    eligible_supplied_emitters: u32,
    indoor_mask_cells: u32,
    indoor_mask_checksum: String,
}

#[derive(Clone, Debug, Serialize)]
struct RuntimeGpuLightFieldEvidence {
    schema_version: u32,
    availability: &'static str,
    field_image_count: u32,
    field_handle_count: u32,
    logical_payload_bytes: u64,
    staging_bytes: u64,
    upload_count: u64,
    uploads_per_changed_revision: u64,
    changed_revision_samples: u64,
    steady_updates: u64,
    steady_uploads: u64,
    steady_scoped_allocation_events: u64,
    steady_scoped_allocation_bytes: u64,
    upload_allocation_events: u64,
    upload_allocation_bytes: u64,
    old_epoch_uploads: u64,
    uploaded_epoch: u64,
    uploaded_revision: u64,
    gpu_checksum: String,
    receiver_pipeline_count: u32,
    receiver_material_count: u32,
    receiver_binding_count: u32,
    shared_field_image: bool,
    point_light_count_increment: u32,
    spot_light_count_increment: u32,
    shadow_map_count_increment: u32,
    local_light_pass_increment: u32,
    mask_pass_count: u32,
    duplicate_2d_pass_count: u32,
    cpu_golden_vectors_pass: bool,
    pixel_probes_pass: bool,
    field_texture_label: &'static str,
    field_width: u16,
    field_height: u16,
    pixel_probe_x: u16,
    pixel_probe_y: u16,
    pixel_probe_expected_rgba: [u8; 4],
}

#[derive(Clone, Debug, Serialize)]
struct RuntimeCrossConsumerEvidence {
    schema_version: u32,
    availability: &'static str,
    world_epoch: u64,
    field_revision: u64,
    field_checksum: String,
    gpu_uploaded_epoch: u64,
    gpu_uploaded_revision: u64,
    gpu_checksum: String,
    soul_recovery_steps: u32,
    soul_count: u32,
    soul_sample_count: u64,
    soul_effect_count: u64,
    soul_mask_or_stale_effects: u64,
    room_count: u32,
    room_state_count: u32,
    room_world_epoch_match_count: u32,
    room_field_revision_match_count: u32,
    room_topology_match_count: u32,
    revision_epoch_consistency: bool,
}

#[derive(Resource, Clone, Default, ExtractResource)]
pub(crate) struct RenderDocCheckpointMailbox(Option<StableRenderDocCheckpoint>);

#[derive(Clone, Debug)]
struct RenderDocCaptureResult {
    checkpoint: StableRenderDocCheckpoint,
    render_frame_index: u64,
    capture_begin_frame: u64,
    capture_end_frame: u64,
    frame_count_before_capture: u64,
    ready_frame_ordinal: u32,
    pre_capture_signature: GpuReadySignature,
    post_capture_signature: GpuReadySignature,
    returned_api_version: String,
    capture_path: PathBuf,
}

#[derive(Clone, Debug)]
enum RenderDocBridgeState {
    Waiting,
    Capturing,
    Captured(Box<RenderDocCaptureResult>),
    Failed(String),
    Finished,
}

#[derive(Resource, Clone)]
pub(crate) struct RenderDocBridge(Arc<Mutex<RenderDocBridgeState>>);

impl RenderDocBridge {
    fn snapshot(&self) -> RenderDocBridgeState {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn replace(&self, state: RenderDocBridgeState) {
        *self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = state;
    }

    fn try_begin_capture(&self) -> bool {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !matches!(*state, RenderDocBridgeState::Waiting) {
            return false;
        }
        *state = RenderDocBridgeState::Capturing;
        true
    }
}

#[derive(Resource, Default)]
pub(crate) struct RenderDocMainState {
    previous: Option<CpuCheckpointSignature>,
    stable_updates: u8,
    next_generation: u64,
    gpu_measurement_started: bool,
    receiver_probes_spawned: bool,
}

#[derive(Resource)]
struct RenderDocReceiverShaders {
    handles: [Handle<Shader>; RENDERDOC_RECEIVER_SHADER_PATHS.len()],
}

impl RenderDocReceiverShaders {
    fn loaded(&self, asset_server: &AssetServer) -> Result<bool, String> {
        for (path, handle) in RENDERDOC_RECEIVER_SHADER_PATHS
            .iter()
            .zip(self.handles.iter())
        {
            match asset_server.get_load_state(handle.id()) {
                Some(LoadState::Loaded) => {}
                Some(LoadState::Failed(error)) => {
                    return Err(format!(
                        "RenderDoc receiver shader failed to load: {path}: {error}"
                    ));
                }
                Some(LoadState::NotLoaded | LoadState::Loading) | None => return Ok(false),
            }
        }
        Ok(true)
    }

    fn fragment_shader_ids(&self) -> [AssetId<Shader>; 4] {
        [
            self.handles[0].id(),
            self.handles[2].id(),
            self.handles[3].id(),
            self.handles[4].id(),
        ]
    }

    fn import_shader_snapshots(
        &self,
        shaders: &Assets<Shader>,
    ) -> Result<[(AssetId<Shader>, Shader); 2], String> {
        let snapshot = |index: usize| {
            let handle = &self.handles[index];
            shaders
                .get(handle)
                .cloned()
                .map(|shader| (handle.id(), shader))
                .ok_or_else(|| {
                    format!(
                        "RenderDoc receiver import shader asset is unavailable: {}",
                        RENDERDOC_RECEIVER_SHADER_PATHS[index]
                    )
                })
        };
        Ok([snapshot(6)?, snapshot(7)?])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
struct GpuReadySignature {
    pipeline_count: usize,
    primary_window: Entity,
    scene_camera_count: usize,
    mask_camera_count: usize,
    window_camera_count: usize,
}

#[derive(Resource, Default)]
struct RenderDocRenderState {
    generation: Option<u64>,
    ready_signature: Option<GpuReadySignature>,
    ready_frames: u32,
    waiting_reason: Option<String>,
    waiting_frames: u32,
    active: Option<(StableRenderDocCheckpoint, GpuReadySignature, u64, u64)>,
}

enum GpuCaptureGate {
    Ready(GpuReadySignature),
    Waiting(String),
}

type GetApiFn =
    unsafe extern "C" fn(renderdoc_sys::RENDERDOC_Version, *mut *mut c_void) -> std::os::raw::c_int;

struct RequiredRenderDocFns {
    get_num_captures: unsafe extern "C" fn() -> u32,
    get_capture: unsafe extern "C" fn(u32, *mut std::os::raw::c_char, *mut u32, *mut u64) -> u32,
    is_frame_capturing: unsafe extern "C" fn() -> u32,
}

struct LoadedRenderDoc {
    _library: Library,
    functions: RequiredRenderDocFns,
    returned_api_version: String,
}

// RenderDoc exposes a process-global, thread-safe function table. The library
// handle is retained for at least as long as every copied function pointer.
unsafe impl Send for LoadedRenderDoc {}
// See the safety argument above; RenderDoc explicitly supports API calls from
// the render thread while its injected module remains loaded.
unsafe impl Sync for LoadedRenderDoc {}

#[derive(Resource)]
struct RenderDocApi {
    loaded: Result<LoadedRenderDoc, String>,
}

#[derive(SystemParam)]
pub(crate) struct RenderDocCheckpointParams<'w, 's> {
    commands: Commands<'w, 's>,
    config: Res<'w, PerfScenarioConfig>,
    applied: Res<'w, PerfScenarioApplied>,
    capture: Res<'w, PerfCapture>,
    checksum_queries: PerfChecksumQueries<'w, 's>,
    virtual_time: Res<'w, Time<Virtual>>,
    rtt_runtime: Res<'w, RttRuntime>,
    render_environment: Res<'w, PerfRenderEnvironmentEvidence>,
    indoor_light_fixture: Res<'w, IndoorLightFixtureState>,
    asset_server: Res<'w, AssetServer>,
    shaders: Res<'w, Assets<Shader>>,
    receiver_shaders: Res<'w, RenderDocReceiverShaders>,
    indoor_light_runtime: Res<'w, crate::systems::lighting::IndoorLightRuntime>,
    world_epoch: Res<'w, hw_core::WorldEpoch>,
    indoor_light_texture:
        ResMut<'w, crate::systems::visual::indoor_light_texture::IndoorLightTexture>,
    indoor_light_lifecycle_probe:
        ResMut<'w, crate::systems::lighting::IndoorLightingLifecycleProbe>,
    images: ResMut<'w, Assets<Image>>,
    building_3d_handles: Res<'w, crate::plugins::startup::Building3dHandles>,
    terrain_3d_handles: Res<'w, crate::plugins::startup::Terrain3dHandles>,
    structural_materials: Res<'w, Assets<hw_visual::TopDownStructuralMaterial>>,
    terrain_materials: Res<'w, Assets<hw_visual::TerrainSurfaceMaterial>>,
    terrain_materials_lod1_lite: Res<'w, Assets<hw_visual::TerrainSurfaceMaterialLod1Lite>>,
    terrain_materials_lod2: Res<'w, Assets<hw_visual::TerrainSurfaceMaterialLod2>>,
    terrain_chunks: Query<
        'w,
        's,
        (
            &'static Mesh3d,
            &'static Transform,
            &'static RenderLayers,
            &'static ViewVisibility,
        ),
        With<crate::world::map::TerrainChunk>,
    >,
    room_lookup: Res<'w, hw_world::RoomTileLookup>,
    cross_consumer_observation:
        Res<'w, crate::systems::lighting::IndoorLightCrossConsumerObservation>,
    room_illumination_states: Query<
        'w,
        's,
        (
            &'static hw_world::Room,
            &'static crate::systems::lighting::RoomIlluminationState,
        ),
    >,
}

#[derive(SystemParam)]
struct RenderDocRenderParams<'w, 's> {
    mailbox: Res<'w, RenderDocCheckpointMailbox>,
    bridge: Res<'w, RenderDocBridge>,
    api: Res<'w, RenderDocApi>,
    images: Res<'w, RenderAssets<GpuImage>>,
    windows: Res<'w, ExtractedWindows>,
    adapter: Res<'w, RenderAdapterInfo>,
    device: Res<'w, RenderDevice>,
    cameras: Query<'w, 's, &'static ExtractedCamera>,
    pipelines: ResMut<'w, PipelineCache>,
    frame_count: Res<'w, FrameCount>,
}

pub(crate) fn install(app: &mut App) {
    let enabled = app
        .world()
        .get_resource::<PerfScenarioConfig>()
        .is_some_and(PerfScenarioConfig::renderdoc_capture_enabled);
    let bridge = RenderDocBridge(Arc::new(Mutex::new(RenderDocBridgeState::Waiting)));
    app.insert_resource(bridge.clone())
        .init_resource::<RenderDocCheckpointMailbox>()
        .init_resource::<RenderDocMainState>();
    let receiver_shader_handles = {
        let asset_server = app.world().resource::<AssetServer>();
        RENDERDOC_RECEIVER_SHADER_PATHS.map(|path| asset_server.load(path))
    };
    app.insert_resource(RenderDocReceiverShaders {
        handles: receiver_shader_handles,
    });
    if !enabled {
        return;
    }

    app.add_plugins(ExtractResourcePlugin::<RenderDocCheckpointMailbox>::default());
    let capture_template = match std::env::var("HW_RENDERDOC_CAPTURE_TEMPLATE") {
        Ok(value) if !value.is_empty() => PathBuf::from(value),
        _ => {
            bridge.replace(RenderDocBridgeState::Failed(
                "HW_RENDERDOC_CAPTURE_TEMPLATE is required".to_string(),
            ));
            return;
        }
    };
    let loaded = load_renderdoc(&capture_template);
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        bridge.replace(RenderDocBridgeState::Failed(
            "RenderApp is unavailable for RenderDoc capture".to_string(),
        ));
        return;
    };
    render_app
        .insert_resource(bridge)
        .insert_resource(RenderDocApi { loaded })
        .init_resource::<RenderDocRenderState>()
        .add_systems(
            Render,
            begin_renderdoc_frame
                .after(RenderSystems::Prepare)
                .before(RenderSystems::Render),
        )
        .add_systems(
            Render,
            finish_renderdoc_frame
                .after(RenderSystems::Render)
                .before(RenderSystems::Cleanup),
        );
}

fn topdown_structural_light_is_applied_before_post_processing() -> bool {
    let sample = TOPDOWN_STRUCTURAL_SHADER.find("let local_light = sample_indoor_light_field(");
    let apply = TOPDOWN_STRUCTURAL_SHADER
        .find("out.color.rgb + pbr_input.material.base_color.rgb * local_light");
    let post = TOPDOWN_STRUCTURAL_SHADER
        .find("out.color = main_pass_post_lighting_processing(pbr_input, out.color);");
    matches!((sample, apply, post), (Some(sample), Some(apply), Some(post)) if sample < apply && apply < post)
}

pub(crate) fn arm_renderdoc_checkpoint_system(
    mut params: RenderDocCheckpointParams,
    mut mailbox: ResMut<RenderDocCheckpointMailbox>,
    mut state: ResMut<RenderDocMainState>,
    bridge: Res<RenderDocBridge>,
) {
    if !params.config.renderdoc_capture_enabled() || mailbox.0.is_some() {
        return;
    }
    if matches!(bridge.snapshot(), RenderDocBridgeState::Failed(_)) {
        return;
    }
    if !params.applied.complete() {
        return;
    }
    if !params.virtual_time.is_paused() {
        bridge.replace(RenderDocBridgeState::Failed(
            "virtual time was not paused at the RenderDoc checkpoint".to_string(),
        ));
        return;
    }
    match params.render_environment.snapshot() {
        PerfRenderEnvironmentState::Pending => return,
        PerfRenderEnvironmentState::Ready(_) => {}
        PerfRenderEnvironmentState::Disabled => {
            bridge.replace(RenderDocBridgeState::Failed(
                "renderer evidence was disabled for RenderDoc capture".to_string(),
            ));
            return;
        }
        PerfRenderEnvironmentState::Failed(reason) => {
            bridge.replace(RenderDocBridgeState::Failed(reason));
            return;
        }
    }

    let Some(selection) = params.config.rtt_light_selection() else {
        bridge.replace(RenderDocBridgeState::Failed(
            "RenderDoc capture is missing the RtT-light selection".to_string(),
        ));
        return;
    };
    if selection.uses_gpu_light_field() && !state.receiver_probes_spawned {
        let Some((mesh, source_transform, render_layers, _)) = params
            .terrain_chunks
            .iter()
            .find(|(_, _, _, visibility)| visibility.get())
        else {
            return;
        };
        let mut probe_transform = *source_transform;
        probe_transform.translation.y -= 0.25;
        params.commands.spawn((
            Name::new("PerfRenderDocTerrainLod1LiteReceiverProbe"),
            Mesh3d(mesh.0.clone()),
            MeshMaterial3d(params.terrain_3d_handles.lod1_lite.clone()),
            probe_transform,
            render_layers.clone(),
            NoFrustumCulling,
            NotShadowCaster,
        ));
        probe_transform.translation.y -= 0.25;
        params.commands.spawn((
            Name::new("PerfRenderDocTerrainLod2ReceiverProbe"),
            Mesh3d(mesh.0.clone()),
            MeshMaterial3d(params.terrain_3d_handles.lod2.clone()),
            probe_transform,
            render_layers.clone(),
            NoFrustumCulling,
            NotShadowCaster,
        ));
        state.receiver_probes_spawned = true;
        return;
    }
    let receiver_import_shaders = if selection.uses_gpu_light_field() {
        match params.receiver_shaders.loaded(&params.asset_server) {
            Ok(true) => {}
            Ok(false) => return,
            Err(reason) => {
                bridge.replace(RenderDocBridgeState::Failed(reason));
                return;
            }
        }
        match params
            .receiver_shaders
            .import_shader_snapshots(&params.shaders)
        {
            Ok(shaders) => Some(shaders),
            Err(reason) => {
                bridge.replace(RenderDocBridgeState::Failed(reason));
                return;
            }
        }
    } else {
        None
    };
    let checksum = calculate_checksum(&params.checksum_queries);
    if checksum.souls != params.config.soul_count as usize
        || checksum.familiars != params.config.familiar_count as usize
    {
        return;
    }
    let render_inventory = calculate_render_inventory(&params.checksum_queries);
    if let Err(reason) = validate_medium_inventory(selection.stage_id(), render_inventory) {
        bridge.replace(RenderDocBridgeState::Failed(reason));
        return;
    }
    let p02_presentation = selection
        .uses_p02_presentation()
        .then(|| calculate_p02_presentation(&params.checksum_queries));
    let runtime_field = if selection.uses_runtime_field() {
        if params.indoor_light_runtime.availability()
            != crate::systems::lighting::IndoorLightAvailability::Available
        {
            return;
        }
        let tiles = params.room_lookup.mask_signature().canonical_tiles();
        let Some(indoor_mask_cells) = u32::try_from(tiles.len()).ok() else {
            bridge.replace(RenderDocBridgeState::Failed(
                "P04 Room mask cell count exceeds u32".to_string(),
            ));
            return;
        };
        if params.indoor_light_runtime.indoor_mask_cells() != Some(indoor_mask_cells) {
            bridge.replace(RenderDocBridgeState::Failed(
                "P04 runtime mask differs from canonical Room membership".to_string(),
            ));
            return;
        }
        let Some(field_checksum) = params.indoor_light_runtime.field_checksum_hex() else {
            bridge.replace(RenderDocBridgeState::Failed(
                "P04 runtime field has no published checksum".to_string(),
            ));
            return;
        };
        Some(RuntimeFieldEvidence {
            world_epoch: params.world_epoch.get(),
            field_revision: params.indoor_light_runtime.output_revision(),
            field_checksum,
            typed_emitter_components: params.indoor_light_runtime.typed_emitter_components(),
            eligible_supplied_emitters: params.indoor_light_runtime.eligible_supplied_emitters(),
            indoor_mask_cells,
            indoor_mask_checksum: super::output::canonical_room_mask_checksum(tiles),
        })
    } else {
        None
    };
    let gpu_light_field = if selection.uses_gpu_light_field() {
        if !state.gpu_measurement_started {
            state.gpu_measurement_started = true;
            state.previous = None;
            state.stable_updates = 0;
            crate::systems::visual::indoor_light_texture::collect_renderdoc_steady_window(
                &params.indoor_light_runtime,
                *params.world_epoch,
                &mut params.indoor_light_lifecycle_probe,
                &mut params.indoor_light_texture,
                &mut params.images,
            );
        }
        if params
            .indoor_light_texture
            .metrics()
            .changed_revision_samples
            < 1
            || params.indoor_light_texture.metrics().steady_updates
                < crate::systems::visual::indoor_light_texture::STEADY_UPDATE_CALLS
        {
            return;
        }
        match gpu_light_field_evidence(&params) {
            Ok(evidence) => Some(evidence),
            Err(reason) => {
                bridge.replace(RenderDocBridgeState::Failed(reason));
                return;
            }
        }
    } else {
        None
    };
    let cross_consumer = if selection.stage_id() == "p08" {
        let Some(runtime_field) = runtime_field.as_ref() else {
            bridge.replace(RenderDocBridgeState::Failed(
                "P08 cross-consumer checkpoint has no runtime field".to_string(),
            ));
            return;
        };
        let Some(gpu_light_field) = gpu_light_field.as_ref() else {
            bridge.replace(RenderDocBridgeState::Failed(
                "P08 cross-consumer checkpoint has no GPU field".to_string(),
            ));
            return;
        };
        match cross_consumer_evidence(&params, runtime_field, gpu_light_field) {
            Ok(evidence) => Some(evidence),
            Err(reason) => {
                bridge.replace(RenderDocBridgeState::Failed(reason));
                return;
            }
        }
    } else {
        None
    };
    let signature = CpuCheckpointSignature {
        checksum: checksum.value,
        scene_target: params.rtt_runtime.scene.id(),
        mask_target: None,
        render_inventory,
        p02_presentation,
    };
    if state.previous == Some(signature) {
        state.stable_updates = state.stable_updates.saturating_add(1);
    } else {
        state.previous = Some(signature);
        state.stable_updates = 1;
    }
    if state.stable_updates < 2 {
        return;
    }
    let Some(observation) = params.indoor_light_fixture.observation.as_ref() else {
        return;
    };
    let fixture = RuntimeFixtureEvidence {
        fixture_checksum: observation.layout_checksum,
        rooms: observation.rooms,
        completed_floors: observation.floors,
        completed_walls: observation.walls,
        doors: observation.doors,
        supplied_lamp_candidates: observation.main_supplied_count,
        unsupplied_lamp_candidates: observation.control_shed_count,
    };

    state.next_generation = state.next_generation.saturating_add(1);
    mailbox.0 = Some(StableRenderDocCheckpoint {
        generation: state.next_generation,
        simulation_tick: params.capture.fixed_update_tick(),
        scene_target: signature.scene_target,
        mask_target: signature.mask_target,
        render_inventory,
        p02_presentation,
        runtime_field,
        gpu_light_field,
        cross_consumer,
        receiver_fragment_shaders: selection
            .uses_gpu_light_field()
            .then(|| params.receiver_shaders.fragment_shader_ids()),
        receiver_import_shaders,
        fixture,
    });
    eprintln!("PERF_RENDERDOC: CPU checkpoint ready; waiting for GPU settle");
}

pub(crate) fn poll_renderdoc_capture_system(
    config: Res<PerfScenarioConfig>,
    bridge: Res<RenderDocBridge>,
    mut exit: MessageWriter<AppExit>,
) {
    if !config.renderdoc_capture_enabled() {
        return;
    }
    match bridge.snapshot() {
        RenderDocBridgeState::Captured(result) => {
            let Some(output_dir) = config.output_dir.as_ref() else {
                bridge.replace(RenderDocBridgeState::Failed(
                    "RenderDoc capture has no output directory".to_string(),
                ));
                exit.write(AppExit::error());
                return;
            };
            let Some(selection) = config.rtt_light_selection() else {
                bridge.replace(RenderDocBridgeState::Failed(
                    "RenderDoc capture requires an RtT-light selection".to_string(),
                ));
                exit.write(AppExit::error());
                return;
            };
            if let Err(error) = write_runtime_checkpoint(
                output_dir,
                &result,
                selection.contract_id(),
                selection.stage_id(),
            ) {
                error!("PERF_RENDERDOC: failed to write checkpoint: {error}");
                bridge.replace(RenderDocBridgeState::Failed(error.to_string()));
                exit.write(AppExit::error());
                return;
            }
            bridge.replace(RenderDocBridgeState::Finished);
            eprintln!("PERF_RENDERDOC: capture completed");
            exit.write(AppExit::Success);
        }
        RenderDocBridgeState::Failed(reason) => {
            error!("PERF_RENDERDOC: {reason}");
            bridge.replace(RenderDocBridgeState::Finished);
            exit.write(AppExit::error());
        }
        RenderDocBridgeState::Waiting
        | RenderDocBridgeState::Capturing
        | RenderDocBridgeState::Finished => {}
    }
}

fn begin_renderdoc_frame(
    mut params: RenderDocRenderParams,
    mut state: ResMut<RenderDocRenderState>,
) {
    if state.active.is_some() {
        return;
    }
    let Some(checkpoint) = params.mailbox.0.as_ref() else {
        return;
    };
    if state.generation != Some(checkpoint.generation) {
        if let Some(imports) = checkpoint.receiver_import_shaders.as_ref() {
            for (id, shader) in imports {
                params.pipelines.set_shader(*id, shader.clone());
            }
        }
        state.generation = Some(checkpoint.generation);
        state.ready_signature = None;
        state.ready_frames = 0;
        state.waiting_reason = None;
        state.waiting_frames = 0;
    }
    let signature = match gpu_ready_signature(&params, checkpoint) {
        Ok(GpuCaptureGate::Ready(value)) => {
            state.waiting_reason = None;
            state.waiting_frames = 0;
            value
        }
        Ok(GpuCaptureGate::Waiting(reason)) => {
            if observe_gpu_waiting(&mut state, reason.clone()) {
                params.bridge.replace(RenderDocBridgeState::Failed(format!(
                    "GPU capture gate remained unavailable for {RENDERDOC_GATE_TIMEOUT_FRAMES} frames: {reason}"
                )));
            }
            return;
        }
        Err(reason) => {
            params.bridge.replace(RenderDocBridgeState::Failed(reason));
            return;
        }
    };
    if !observe_gpu_ready_signature(&mut state, signature) {
        return;
    }
    let api = match &params.api.loaded {
        Ok(value) => value,
        Err(reason) => {
            params
                .bridge
                .replace(RenderDocBridgeState::Failed(reason.clone()));
            return;
        }
    };
    if !params.bridge.try_begin_capture() {
        return;
    }
    if let Err(reason) = api.start_capture(&params.device) {
        params.bridge.replace(RenderDocBridgeState::Failed(reason));
        return;
    }
    let begin_frame = u64::from(params.frame_count.0);
    state.active = Some((checkpoint.clone(), signature, begin_frame, begin_frame));
}

fn observe_gpu_ready_signature(
    state: &mut RenderDocRenderState,
    signature: GpuReadySignature,
) -> bool {
    match state.ready_signature {
        None => state.ready_signature = Some(signature),
        Some(previous) if previous != signature => {
            state.ready_signature = Some(signature);
            state.ready_frames = 0;
        }
        Some(_) => {}
    }
    state.ready_frames = state.ready_frames.saturating_add(1);
    state.ready_frames >= RENDERDOC_SETTLE_FRAMES
}

fn observe_gpu_waiting(state: &mut RenderDocRenderState, reason: String) -> bool {
    state.ready_signature = None;
    state.ready_frames = 0;
    state.waiting_reason = Some(reason);
    state.waiting_frames = state.waiting_frames.saturating_add(1);
    state.waiting_frames >= RENDERDOC_GATE_TIMEOUT_FRAMES
}

fn finish_renderdoc_frame(params: RenderDocRenderParams, mut state: ResMut<RenderDocRenderState>) {
    let Some((checkpoint, expected_signature, capture_begin_frame, frame_count_before)) =
        state.active.take()
    else {
        return;
    };
    let capture_end_frame = u64::from(params.frame_count.0);
    let api = match &params.api.loaded {
        Ok(value) => value,
        Err(reason) => {
            params
                .bridge
                .replace(RenderDocBridgeState::Failed(reason.clone()));
            return;
        }
    };
    let current_signature = gpu_signature(&params, &checkpoint, false);
    if !matches!(
        current_signature,
        Ok(GpuCaptureGate::Ready(signature)) if signature == expected_signature
    ) {
        let _ = api.stop_without_publish(&params.device);
        params.bridge.replace(RenderDocBridgeState::Failed(
            "GPU capture gate changed during the captured render frame".to_string(),
        ));
        return;
    }
    match api.end_capture(&params.device) {
        Ok(capture_path) => {
            let returned_api_version = match &params.api.loaded {
                Ok(loaded) => loaded.returned_api_version.clone(),
                Err(reason) => {
                    params
                        .bridge
                        .replace(RenderDocBridgeState::Failed(reason.clone()));
                    return;
                }
            };
            let post_signature = match gpu_signature(&params, &checkpoint, false) {
                Ok(GpuCaptureGate::Ready(value)) => value,
                Ok(GpuCaptureGate::Waiting(reason)) => {
                    params.bridge.replace(RenderDocBridgeState::Failed(format!(
                        "GPU capture gate was unavailable after the captured frame: {reason}"
                    )));
                    return;
                }
                Err(reason) => {
                    params.bridge.replace(RenderDocBridgeState::Failed(reason));
                    return;
                }
            };
            params
                .bridge
                .replace(RenderDocBridgeState::Captured(Box::new(
                    RenderDocCaptureResult {
                        checkpoint,
                        render_frame_index: capture_end_frame,
                        capture_begin_frame,
                        capture_end_frame,
                        frame_count_before_capture: frame_count_before,
                        ready_frame_ordinal: RENDERDOC_SETTLE_FRAMES,
                        pre_capture_signature: expected_signature,
                        post_capture_signature: post_signature,
                        returned_api_version,
                        capture_path,
                    },
                )))
        }
        Err(reason) => params.bridge.replace(RenderDocBridgeState::Failed(reason)),
    }
}

fn gpu_ready_signature(
    params: &RenderDocRenderParams,
    checkpoint: &StableRenderDocCheckpoint,
) -> Result<GpuCaptureGate, String> {
    gpu_signature(params, checkpoint, true)
}

fn gpu_signature(
    params: &RenderDocRenderParams,
    checkpoint: &StableRenderDocCheckpoint,
    before_render: bool,
) -> Result<GpuCaptureGate, String> {
    if params.adapter.backend != wgpu::Backend::Vulkan {
        return Err(format!(
            "RenderDoc capture requires Vulkan; observed {:?}",
            params.adapter.backend
        ));
    }
    if params.windows.windows.len() != 1 {
        return Err(format!(
            "RenderDoc wildcard capture requires exactly one window; observed {}",
            params.windows.windows.len()
        ));
    }
    let Some(primary) = params.windows.primary else {
        return Ok(GpuCaptureGate::Waiting(
            "primary window is unavailable".to_string(),
        ));
    };
    let Some(window) = params.windows.windows.get(&primary) else {
        return Ok(GpuCaptureGate::Waiting(
            "primary extracted window is unavailable".to_string(),
        ));
    };
    if window.swap_chain_texture_view.is_none() {
        return Ok(GpuCaptureGate::Waiting(
            "primary swapchain texture view is unavailable".to_string(),
        ));
    }
    if before_render && window.swap_chain_texture.is_none() {
        return Ok(GpuCaptureGate::Waiting(
            "primary swapchain texture is unavailable before render".to_string(),
        ));
    }
    if !before_render && window.swap_chain_texture.is_some() {
        return Err("primary swapchain image was not presented by the captured frame".to_string());
    }
    let Some(scene) = params.images.get(checkpoint.scene_target) else {
        return Ok(GpuCaptureGate::Waiting(
            "RtT scene texture is unavailable on the GPU".to_string(),
        ));
    };
    if scene.texture_descriptor.label != Some(RTT_SCENE_LABEL) {
        return Err("RtT GPU texture labels differ from the RenderDoc contract".to_string());
    }
    if checkpoint.mask_target.is_some() {
        return Err("P01 RenderDoc checkpoint unexpectedly retained a mask target".to_string());
    }
    let mut pipeline_count = 0;
    for pipeline in params.pipelines.pipelines() {
        pipeline_count += usize::from(matches!(&pipeline.state, CachedPipelineState::Ok(_)));
    }
    if pipeline_count == 0 {
        return Ok(GpuCaptureGate::Waiting(
            "no resident render pipeline is available".to_string(),
        ));
    }
    if checkpoint.gpu_light_field.is_some() {
        let Some(receiver_fragment_shaders) = checkpoint.receiver_fragment_shaders else {
            return Err("P06 GPU checkpoint is missing receiver fragment shader IDs".to_string());
        };
        for (path, shader_id) in RENDERDOC_RECEIVER_FRAGMENT_SHADER_PATHS
            .iter()
            .zip(receiver_fragment_shaders)
        {
            let mut matched_descriptor = false;
            let mut resident = false;
            let mut waiting_state = None;
            let mut permanent_error = None;
            for pipeline in params.pipelines.pipelines() {
                let PipelineDescriptor::RenderPipelineDescriptor(descriptor) = &pipeline.descriptor
                else {
                    continue;
                };
                let Some(fragment) = descriptor.fragment.as_ref() else {
                    continue;
                };
                if fragment.shader.id() != shader_id {
                    continue;
                }
                matched_descriptor = true;
                match &pipeline.state {
                    CachedPipelineState::Ok(_) => resident = true,
                    CachedPipelineState::Queued => waiting_state = Some("queued"),
                    CachedPipelineState::Creating(_) => waiting_state = Some("creating"),
                    CachedPipelineState::Err(
                        ShaderCacheError::ShaderNotLoaded(_)
                        | ShaderCacheError::ShaderImportNotYetAvailable,
                    ) => waiting_state = Some("waiting for a shader dependency"),
                    CachedPipelineState::Err(error) => {
                        permanent_error = Some(format!("{error:?}"));
                    }
                }
            }
            if resident {
                continue;
            }
            if let Some(error) = permanent_error {
                return Err(format!(
                    "RenderDoc receiver pipeline compilation failed for {path}: {error}"
                ));
            }
            let state = if matched_descriptor {
                waiting_state.unwrap_or("not resident")
            } else {
                "not queued"
            };
            return Ok(GpuCaptureGate::Waiting(format!(
                "RenderDoc receiver pipeline for {path} is {state}"
            )));
        }
    }

    let mut scene_camera_count = 0;
    let mut mask_camera_count = 0;
    let mut window_camera_count = 0;
    for camera in &params.cameras {
        match camera.target.as_ref() {
            Some(NormalizedRenderTarget::Image(target))
                if target.handle.id() == checkpoint.scene_target =>
            {
                scene_camera_count += 1;
            }
            Some(NormalizedRenderTarget::Image(target))
                if Some(target.handle.id()) == checkpoint.mask_target =>
            {
                mask_camera_count += 1;
            }
            Some(NormalizedRenderTarget::Window(_)) => window_camera_count += 1,
            _ => {}
        }
    }
    if scene_camera_count != 1 || mask_camera_count != 0 || window_camera_count == 0 {
        return Ok(GpuCaptureGate::Waiting(
            "RenderDoc camera topology is not ready".to_string(),
        ));
    }
    Ok(GpuCaptureGate::Ready(GpuReadySignature {
        pipeline_count,
        primary_window: primary,
        scene_camera_count,
        mask_camera_count,
        window_camera_count,
    }))
}

fn validate_medium_inventory(stage_id: &str, inventory: PerfRenderInventory) -> Result<(), String> {
    let expected = PerfRenderInventory {
        scene_target_count: 1,
        mask_target_count: usize::from(stage_id == "current"),
        camera_3d_rtt_count: if stage_id == "current" { 2 } else { 1 },
        camera_2d_count: if matches!(
            stage_id,
            "p02" | "p03" | "p04" | "p05" | "p06" | "p07" | "p08"
        ) {
            2
        } else {
            3
        },
        layer_2d_pass_count: if matches!(
            stage_id,
            "p02" | "p03" | "p04" | "p05" | "p06" | "p07" | "p08"
        ) {
            1
        } else {
            2
        },
        soul_proxy_3d: if matches!(
            stage_id,
            "p02" | "p03" | "p04" | "p05" | "p06" | "p07" | "p08"
        ) {
            0
        } else {
            200
        },
        soul_mask_proxy_3d: if stage_id == "current" { 200 } else { 0 },
        soul_shadow_proxy_3d: if matches!(
            stage_id,
            "p02" | "p03" | "p04" | "p05" | "p06" | "p07" | "p08"
        ) {
            0
        } else {
            200
        },
        familiar_proxy_3d: if matches!(
            stage_id,
            "p02" | "p03" | "p04" | "p05" | "p06" | "p07" | "p08"
        ) {
            0
        } else {
            12
        },
    };
    if inventory == expected {
        Ok(())
    } else {
        Err(format!(
            "{stage_id} medium RenderDoc inventory differs: observed={inventory:?} expected={expected:?}"
        ))
    }
}

impl LoadedRenderDoc {
    fn start_capture(&self, device: &RenderDevice) -> Result<(), String> {
        let functions = &self.functions;
        // SAFETY: wgpu documents this method as RenderDoc
        // StartFrameCapture(device, NULL). The render-device selector removes
        // the undefined multi-device wildcard used by a NULL/NULL call.
        unsafe {
            if (functions.get_num_captures)() != 0 {
                return Err("RenderDoc capture count was nonzero before arming".to_string());
            }
            if (functions.is_frame_capturing)() != 0 {
                return Err("RenderDoc was already capturing before arming".to_string());
            }
            device.wgpu_device().start_graphics_debugger_capture();
            if (functions.is_frame_capturing)() != 1 {
                return Err("RenderDoc StartFrameCapture did not become active".to_string());
            }
        }
        Ok(())
    }

    fn stop_without_publish(&self, device: &RenderDevice) -> Result<(), String> {
        // SAFETY: This matches the preceding device-selected start. The
        // surrounding helper treats the resulting temporary capture as failed
        // diagnostic evidence and never publishes it.
        unsafe { device.wgpu_device().stop_graphics_debugger_capture() };
        // SAFETY: Function pointer was negotiated from the retained table.
        if unsafe { (self.functions.is_frame_capturing)() } == 0 {
            Ok(())
        } else {
            Err("RenderDoc capture remained active after abort".to_string())
        }
    }

    fn end_capture(&self, device: &RenderDevice) -> Result<PathBuf, String> {
        let functions = &self.functions;
        // SAFETY: This matches the preceding device-selected wgpu start.
        unsafe {
            device.wgpu_device().stop_graphics_debugger_capture();
            if (functions.is_frame_capturing)() != 0 {
                return Err("RenderDoc EndFrameCapture failed".to_string());
            }
            if (functions.get_num_captures)() != 1 {
                return Err("RenderDoc did not produce exactly one capture".to_string());
            }
            let mut path_length = 0_u32;
            let mut timestamp = 0_u64;
            if (functions.get_capture)(0, ptr::null_mut(), &mut path_length, &mut timestamp) != 1
                || path_length == 0
                || path_length > 32_768
            {
                return Err("RenderDoc GetCapture length query failed".to_string());
            }
            let mut buffer = vec![0_i8; path_length as usize];
            if (functions.get_capture)(0, buffer.as_mut_ptr(), &mut path_length, &mut timestamp)
                != 1
            {
                return Err("RenderDoc GetCapture path query failed".to_string());
            }
            let path = CStr::from_ptr(buffer.as_ptr())
                .to_str()
                .map_err(|_| "RenderDoc capture path is not UTF-8")?;
            Ok(PathBuf::from(path))
        }
    }
}

#[cfg(target_os = "linux")]
fn load_renderdoc(capture_template: &Path) -> Result<LoadedRenderDoc, String> {
    use libloading::os::unix::{Library as UnixLibrary, RTLD_NOW};
    const RTLD_NOLOAD: std::os::raw::c_int = 0x4;

    let requested_library =
        std::env::var("HW_RENDERDOC_LIBRARY").map_err(|_| "HW_RENDERDOC_LIBRARY is required")?;
    // SAFETY: RTLD_NOLOAD returns a handle only for the RenderDoc module that
    // renderdoccmd already injected into this process.
    let library: Library =
        unsafe { UnixLibrary::open(Some(requested_library.as_str()), RTLD_NOW | RTLD_NOLOAD) }
            .map_err(|error| format!("RenderDoc was not injected: {error}"))?
            .into();
    let mut raw_api = ptr::null_mut::<c_void>();
    let result = {
        // SAFETY: The symbol has RenderDoc's documented C ABI and is used only
        // while the owning Library remains alive.
        let get_api = unsafe { library.get::<GetApiFn>(b"RENDERDOC_GetAPI\0") }
            .map_err(|error| format!("RENDERDOC_GetAPI is missing: {error}"))?;
        // SAFETY: raw_api is a valid out pointer for the requested API table.
        unsafe { get_api(renderdoc_sys::eRENDERDOC_API_Version_1_6_0, &mut raw_api) }
    };
    if result != 1 || raw_api.is_null() {
        return Err(format!("RENDERDOC_GetAPI(1.6.0) failed: result={result}"));
    }
    // SAFETY: A successful GetAPI call for 1.6 returns this exact table type.
    let api = unsafe { *raw_api.cast::<renderdoc_sys::RENDERDOC_API_1_6_0>() };
    let get_api_version = api.GetAPIVersion.ok_or("GetAPIVersion is null")?;
    let mut major = 0;
    let mut minor = 0;
    let mut patch = 0;
    // SAFETY: Function pointer was checked for null and arguments are valid.
    unsafe { get_api_version(&mut major, &mut minor, &mut patch) };
    if !is_compatible_renderdoc_api_version(major, minor, patch) {
        return Err(format!(
            "incompatible RenderDoc App API {major}.{minor}.{patch}"
        ));
    }
    let returned_api_version = format!("{major}.{minor}.{patch}");
    // SAFETY: These union fields are aliases retained for API compatibility;
    // the negotiated 1.6 table initializes the capture-path variants.
    let set_capture_path = unsafe { api.__bindgen_anon_2.SetCaptureFilePathTemplate }
        .ok_or("SetCaptureFilePathTemplate is null")?;
    // SAFETY: See the union-field justification above.
    let get_capture_path = unsafe { api.__bindgen_anon_3.GetCaptureFilePathTemplate }
        .ok_or("GetCaptureFilePathTemplate is null")?;
    let template = capture_template
        .to_str()
        .ok_or("RenderDoc capture template is not UTF-8")?;
    let template = CString::new(template).map_err(|_| "capture template contains NUL")?;
    // SAFETY: The C string lives through the call and RenderDoc copies it.
    unsafe { set_capture_path(template.as_ptr()) };
    // SAFETY: Function pointer was checked and returns RenderDoc-owned storage.
    let round_trip = unsafe { get_capture_path() };
    if round_trip.is_null()
        // SAFETY: Non-null pointer is a RenderDoc-owned NUL-terminated string.
        || unsafe { CStr::from_ptr(round_trip) }.to_bytes() != template.as_bytes()
    {
        return Err("RenderDoc capture template round-trip failed".to_string());
    }
    let functions = RequiredRenderDocFns {
        get_num_captures: api.GetNumCaptures.ok_or("GetNumCaptures is null")?,
        get_capture: api.GetCapture.ok_or("GetCapture is null")?,
        is_frame_capturing: api.IsFrameCapturing.ok_or("IsFrameCapturing is null")?,
    };
    Ok(LoadedRenderDoc {
        _library: library,
        functions,
        returned_api_version,
    })
}

const fn is_compatible_renderdoc_api_version(major: i32, minor: i32, patch: i32) -> bool {
    major == 1 && (minor > 6 || (minor == 6 && patch >= 0))
}

#[cfg(not(target_os = "linux"))]
fn load_renderdoc(_capture_template: &Path) -> Result<LoadedRenderDoc, String> {
    Err("formal RenderDoc capture is currently supported only on Linux".to_string())
}

#[derive(Serialize)]
struct RuntimeCheckpointFile<'a> {
    schema_version: u32,
    status: &'static str,
    contract_id: String,
    stage_id: String,
    generation: u64,
    checkpoint: RuntimeCheckpoint,
    render_inventory: RuntimeRenderInventory,
    #[serde(skip_serializing_if = "Option::is_none")]
    p02_presentation: Option<RuntimeP02Presentation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    runtime_field: Option<RuntimeFieldEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gpu_light_field: Option<RuntimeGpuLightFieldEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cross_consumer: Option<RuntimeCrossConsumerEvidence>,
    render_resources: RuntimeRenderResources,
    fixture: RuntimeFixtureEvidence,
    capture_path: &'a Path,
    requested_renderdoc_api_version: &'static str,
    returned_renderdoc_api_version: String,
    selector: RuntimeSelectorEvidence,
    gpu_ready: RuntimeGpuReadyEvidence,
    capture_artifact: RuntimeCaptureArtifact,
}

#[derive(Serialize)]
struct RuntimeSelectorEvidence {
    strategy: &'static str,
    device_selector: &'static str,
    window_selector: &'static str,
    window_count: usize,
    primary_window: u64,
}

#[derive(Serialize)]
struct RuntimeGpuReadyEvidence {
    pre_capture: RuntimeGpuReadySignature,
    post_capture: RuntimeGpuReadySignature,
}

#[derive(Serialize)]
struct RuntimeGpuReadySignature {
    pipeline_count: usize,
    scene_camera_count: usize,
    mask_camera_count: usize,
    window_camera_count: usize,
}

#[derive(Serialize)]
struct RuntimeCaptureArtifact {
    sha256: String,
    bytes: u64,
}

#[derive(Serialize)]
struct RuntimeCheckpoint {
    name: &'static str,
    simulation_tick: u64,
    simulation_tick_source: &'static str,
    settle_frames: u32,
    ready_frame_ordinal: u32,
    capture_frame: u32,
    capture_begin_frame: u64,
    capture_end_frame: u64,
    render_frame_index: u64,
    frame_count_before_capture: u64,
}

#[derive(Serialize)]
struct RuntimeRenderInventory {
    scene_target_count: usize,
    mask_target_count: usize,
    camera_3d_rtt_count: usize,
    camera_2d_count: usize,
    layer_2d_pass_count: usize,
    soul_proxy_3d: usize,
    soul_mask_proxy_3d: usize,
    soul_shadow_proxy_3d: usize,
    familiar_proxy_3d: usize,
}

#[derive(Serialize)]
struct RuntimeP02Presentation {
    layer_2d_camera_count: usize,
    layer_2d_pass_count: usize,
    building_count: usize,
    duplicate_presentation_count: usize,
    building_exactly_one_presentation: bool,
    soul_count: usize,
    soul_billboard_count: usize,
    familiar_3d_count: usize,
    state_and_bounce_probes_pass: bool,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct RuntimeCompositeTextureBinding {
    target: &'static str,
    stage: &'static str,
    fixed_bind_set_or_space: u32,
    fixed_bind_number: u32,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct RuntimeCompositeSamplerBinding {
    stage: &'static str,
    fixed_bind_set_or_space: u32,
    fixed_bind_number: u32,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct RuntimeRenderResources {
    scene_target_label: &'static str,
    mask_target_label: Option<&'static str>,
    composite_draw_count: u32,
    composite_texture_bindings: Vec<RuntimeCompositeTextureBinding>,
    composite_sampler_bindings: Vec<RuntimeCompositeSamplerBinding>,
}

impl From<PerfRenderInventory> for RuntimeRenderInventory {
    fn from(value: PerfRenderInventory) -> Self {
        Self {
            scene_target_count: value.scene_target_count,
            mask_target_count: value.mask_target_count,
            camera_3d_rtt_count: value.camera_3d_rtt_count,
            camera_2d_count: value.camera_2d_count,
            layer_2d_pass_count: value.layer_2d_pass_count,
            soul_proxy_3d: value.soul_proxy_3d,
            soul_mask_proxy_3d: value.soul_mask_proxy_3d,
            soul_shadow_proxy_3d: value.soul_shadow_proxy_3d,
            familiar_proxy_3d: value.familiar_proxy_3d,
        }
    }
}

impl From<PerfP02Presentation> for RuntimeP02Presentation {
    fn from(value: PerfP02Presentation) -> Self {
        Self {
            layer_2d_camera_count: value.layer_2d_camera_count,
            layer_2d_pass_count: value.layer_2d_pass_count,
            building_count: value.building_count,
            duplicate_presentation_count: value.duplicate_presentation_count,
            building_exactly_one_presentation: value.building_exactly_one_presentation,
            soul_count: value.soul_count,
            soul_billboard_count: value.soul_billboard_count,
            familiar_3d_count: value.familiar_3d_count,
            state_and_bounce_probes_pass: value.state_and_bounce_probes_pass,
        }
    }
}

fn p01_composite_render_resources() -> RuntimeRenderResources {
    RuntimeRenderResources {
        scene_target_label: RTT_SCENE_LABEL,
        mask_target_label: None,
        composite_draw_count: 1,
        composite_texture_bindings: vec![RuntimeCompositeTextureBinding {
            target: "scene_target",
            stage: "fragment",
            fixed_bind_set_or_space: RTT_COMPOSITE_BIND_SET_OR_SPACE,
            fixed_bind_number: RTT_COMPOSITE_SCENE_TEXTURE_BINDING,
        }],
        composite_sampler_bindings: vec![RuntimeCompositeSamplerBinding {
            stage: "fragment",
            fixed_bind_set_or_space: RTT_COMPOSITE_BIND_SET_OR_SPACE,
            fixed_bind_number: RTT_COMPOSITE_SCENE_SAMPLER_BINDING,
        }],
    }
}

fn p06_cpu_golden_vectors_pass(snapshot: &hw_infra::lighting::FieldSnapshot) -> bool {
    let packed = hw_infra::lighting::pack_rgba8_linear(snapshot);
    let quantize = |value: u16| {
        u8::try_from((u32::from(value) * 255 + 32_767) / 65_535)
            .expect("UNORM16 quantization fits u8")
    };
    let payload_matches = snapshot
        .cells()
        .iter()
        .zip(snapshot.indoor_mask_bytes())
        .zip(packed.chunks_exact(4))
        .all(|((cell, mask), pixel)| {
            pixel
                == [
                    quantize(cell.r),
                    quantize(cell.g),
                    quantize(cell.b),
                    if *mask == 0 { 0 } else { 255 },
                ]
        });
    let mapping_matches = [(0, 0), (99, 0), (0, 99), (99, 99), (50, 50)]
        .into_iter()
        .all(|grid| WorldMap::world_to_grid(WorldMap::grid_to_world(grid.0, grid.1)) == grid);
    payload_matches && mapping_matches
}

fn gpu_light_field_evidence(
    params: &RenderDocCheckpointParams<'_, '_>,
) -> Result<RuntimeGpuLightFieldEvidence, String> {
    let texture = params.indoor_light_texture.as_ref();
    let image_handle = texture.handle();
    let structural_handles = [
        &params.building_3d_handles.wall_material,
        &params.building_3d_handles.wall_provisional_material,
        &params.building_3d_handles.floor_material,
        &params.building_3d_handles.bridge_material,
        &params.building_3d_handles.door_closed_material,
        &params.building_3d_handles.door_open_material,
        &params.building_3d_handles.door_locked_material,
        &params.building_3d_handles.equipment_material,
        &params.building_3d_handles.tank_partial_material,
        &params.building_3d_handles.tank_full_material,
        &params.building_3d_handles.mixer_idle_material,
        &params.building_3d_handles.mixer_active_material,
    ];
    let structural_receivers_match = structural_handles.iter().all(|handle| {
        params
            .structural_materials
            .get(*handle)
            .and_then(|material| material.extension.indoor_light_field.as_ref())
            == Some(image_handle)
    });
    let terrain_receivers_match = params
        .terrain_materials
        .get(&params.terrain_3d_handles.lod1)
        .and_then(|material| material.extension.indoor_light_field.as_ref())
        == Some(image_handle)
        && params
            .terrain_materials_lod1_lite
            .get(&params.terrain_3d_handles.lod1_lite)
            .and_then(|material| material.extension.indoor_light_field.as_ref())
            == Some(image_handle)
        && params
            .terrain_materials_lod2
            .get(&params.terrain_3d_handles.lod2)
            .and_then(|material| material.extension.indoor_light_field.as_ref())
            == Some(image_handle);
    if !structural_receivers_match || !terrain_receivers_match {
        return Err("P06 receiver materials do not share the owned Light Field image".to_string());
    }
    let Some(uploaded_epoch) = texture.uploaded_epoch() else {
        return Err("P06 Light Field image has no uploaded epoch".to_string());
    };
    let Some(uploaded_revision) = texture.uploaded_revision() else {
        return Err("GPU Light Field image has no uploaded revision".to_string());
    };
    let Some(gpu_checksum) = texture.uploaded_checksum() else {
        return Err("P06 Light Field image has no uploaded checksum".to_string());
    };
    if Some(uploaded_epoch) != params.indoor_light_runtime.published_epoch()
        || texture.uploaded_revision() != Some(params.indoor_light_runtime.output_revision())
        || Some(gpu_checksum) != params.indoor_light_runtime.field_checksum_hex().as_deref()
    {
        return Err("P06 GPU Light Field differs from the current CPU publication".to_string());
    }
    let metrics = texture.metrics();
    let Some(snapshot) = params
        .indoor_light_runtime
        .snapshot_for_epoch(*params.world_epoch)
    else {
        return Err("P06 CPU golden probe has no current snapshot".to_string());
    };
    let packed = hw_infra::lighting::pack_rgba8_linear(snapshot);
    let Some((probe_index, probe_pixel)) = packed
        .chunks_exact(4)
        .enumerate()
        .filter(|(_, pixel)| pixel[3] != 0)
        .max_by_key(|(_, pixel)| u16::from(pixel[0]) + u16::from(pixel[1]) + u16::from(pixel[2]))
        .filter(|(_, pixel)| pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0)
    else {
        return Err("P06 CPU golden probe has no illuminated pixel".to_string());
    };
    let dimensions = snapshot.dimensions();
    let width = usize::from(dimensions.width());
    let pixel_probe_expected_rgba = probe_pixel
        .try_into()
        .expect("Light Field probe pixel contains four bytes");
    let uploads_per_changed_revision = if metrics.changed_revision_samples == 0 {
        0
    } else {
        metrics
            .upload_count
            .div_ceil(metrics.changed_revision_samples)
    };
    Ok(RuntimeGpuLightFieldEvidence {
        schema_version: 1,
        availability: "available",
        field_image_count: 1,
        field_handle_count: 1,
        logical_payload_bytes: metrics.logical_payload_bytes,
        staging_bytes: metrics.staging_bytes,
        upload_count: metrics.upload_count,
        uploads_per_changed_revision,
        changed_revision_samples: metrics.changed_revision_samples,
        steady_updates: metrics.steady_updates,
        steady_uploads: metrics.steady_uploads,
        steady_scoped_allocation_events: 0,
        steady_scoped_allocation_bytes: 0,
        upload_allocation_events: metrics.upload_allocation_events,
        upload_allocation_bytes: metrics.upload_allocation_bytes,
        old_epoch_uploads: metrics.old_epoch_uploads,
        uploaded_epoch,
        uploaded_revision,
        gpu_checksum: gpu_checksum.to_string(),
        receiver_pipeline_count: 4,
        receiver_material_count: u32::try_from(structural_handles.len() + 3)
            .expect("P06 receiver count fits u32"),
        receiver_binding_count: 1,
        shared_field_image: true,
        point_light_count_increment: 0,
        spot_light_count_increment: 0,
        shadow_map_count_increment: 0,
        local_light_pass_increment: 0,
        mask_pass_count: 0,
        duplicate_2d_pass_count: 0,
        cpu_golden_vectors_pass: p06_cpu_golden_vectors_pass(snapshot)
            && topdown_structural_light_is_applied_before_post_processing(),
        pixel_probes_pass: false,
        field_texture_label:
            crate::systems::visual::indoor_light_texture::LIGHT_FIELD_TEXTURE_LABEL,
        field_width: dimensions.width(),
        field_height: dimensions.height(),
        pixel_probe_x: u16::try_from(probe_index % width).expect("probe x fits u16"),
        pixel_probe_y: u16::try_from(probe_index / width).expect("probe y fits u16"),
        pixel_probe_expected_rgba,
    })
}

fn cross_consumer_evidence(
    params: &RenderDocCheckpointParams<'_, '_>,
    runtime_field: &RuntimeFieldEvidence,
    gpu_light_field: &RuntimeGpuLightFieldEvidence,
) -> Result<RuntimeCrossConsumerEvidence, String> {
    let observation = *params.cross_consumer_observation;
    let expected_epoch = params.world_epoch.get();
    let expected_revision = params.indoor_light_runtime.output_revision();
    let expected_souls = params.config.soul_count;
    let expected_samples =
        u64::from(expected_souls).saturating_mul(u64::from(observation.recovery_steps()));
    let topology_revision = params.room_lookup.topology_signature().revision();
    let expected_rooms = params
        .indoor_light_fixture
        .observation
        .as_ref()
        .map(|fixture| fixture.rooms)
        .ok_or_else(|| "P08 cross-consumer checkpoint has no fixture observation".to_string())?;

    let mut room_state_count = 0_u32;
    let mut room_world_epoch_match_count = 0_u32;
    let mut room_field_revision_match_count = 0_u32;
    let mut room_topology_match_count = 0_u32;
    for (room, state) in &params.room_illumination_states {
        room_state_count = room_state_count.saturating_add(1);
        if state.world_epoch() == expected_epoch {
            room_world_epoch_match_count = room_world_epoch_match_count.saturating_add(1);
        }
        if state.field_revision() == expected_revision {
            room_field_revision_match_count = room_field_revision_match_count.saturating_add(1);
        }
        if state.room_topology_revision() == topology_revision
            && state.room_tile_signature() == &room.tile_signature
        {
            room_topology_match_count = room_topology_match_count.saturating_add(1);
        }
    }
    let room_count = u32::try_from(expected_rooms)
        .map_err(|_| "P08 fixture Room count exceeds u32".to_string())?;
    let revision_epoch_consistency = runtime_field.world_epoch == expected_epoch
        && runtime_field.field_revision == expected_revision
        && runtime_field.field_checksum == gpu_light_field.gpu_checksum
        && gpu_light_field.uploaded_epoch == expected_epoch
        && gpu_light_field.uploaded_revision == expected_revision
        && observation.world_epoch() == Some(expected_epoch)
        && observation.field_revision() == Some(expected_revision)
        && observation.recovery_steps() > 0
        && observation.soul_count() == expected_souls
        && observation.sample_count() == expected_samples
        && observation.mask_or_stale_effects() == 0
        && room_state_count == room_count
        && room_world_epoch_match_count == room_count
        && room_field_revision_match_count == room_count
        && room_topology_match_count == room_count;
    if !revision_epoch_consistency {
        return Err(format!(
            "P08 cross-consumer epoch/revision mismatch: epoch={expected_epoch} revision={expected_revision} souls={}/{expected_souls} samples={}/{expected_samples} rooms={room_state_count}/{room_count}",
            observation.soul_count(),
            observation.sample_count(),
        ));
    }

    Ok(RuntimeCrossConsumerEvidence {
        schema_version: 1,
        availability: "available",
        world_epoch: expected_epoch,
        field_revision: expected_revision,
        field_checksum: runtime_field.field_checksum.clone(),
        gpu_uploaded_epoch: gpu_light_field.uploaded_epoch,
        gpu_uploaded_revision: gpu_light_field.uploaded_revision,
        gpu_checksum: gpu_light_field.gpu_checksum.clone(),
        soul_recovery_steps: observation.recovery_steps(),
        soul_count: observation.soul_count(),
        soul_sample_count: observation.sample_count(),
        soul_effect_count: observation.effect_count(),
        soul_mask_or_stale_effects: observation.mask_or_stale_effects(),
        room_count,
        room_state_count,
        room_world_epoch_match_count,
        room_field_revision_match_count,
        room_topology_match_count,
        revision_epoch_consistency,
    })
}

fn sha256_file(path: &Path) -> Result<String, std::io::Error> {
    subprocess_sha256(path)
}

#[cfg(target_os = "linux")]
fn subprocess_sha256(path: &Path) -> Result<String, std::io::Error> {
    use std::process::Command;
    let output = Command::new("sha256sum").arg(path).output()?;
    if !output.status.success() {
        return Err(std::io::Error::other("sha256sum failed"));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let digest = text.split_whitespace().next().unwrap_or_default();
    if digest.len() != 64 {
        return Err(std::io::Error::other("sha256sum returned invalid digest"));
    }
    Ok(digest.to_string())
}

#[cfg(not(target_os = "linux"))]
fn subprocess_sha256(path: &Path) -> Result<String, std::io::Error> {
    let _ = path;
    Err(std::io::Error::other(
        "sha256 capture digest requires Linux sha256sum",
    ))
}

fn write_runtime_checkpoint(
    output_dir: &Path,
    result: &RenderDocCaptureResult,
    contract_id: &str,
    stage_id: &str,
) -> std::io::Result<()> {
    let metadata = std::fs::metadata(&result.capture_path)?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(std::io::Error::other(
            "RenderDoc reported an empty or missing capture",
        ));
    }
    let capture_bytes = metadata.len();
    let capture_sha256 = sha256_file(&result.capture_path)?;
    std::fs::create_dir_all(output_dir)?;
    let destination = output_dir.join("renderdoc-checkpoint.json");
    let temporary = output_dir.join(format!(".renderdoc-checkpoint.{}.tmp", std::process::id()));
    let signature = |value: GpuReadySignature| RuntimeGpuReadySignature {
        pipeline_count: value.pipeline_count,
        scene_camera_count: value.scene_camera_count,
        mask_camera_count: value.mask_camera_count,
        window_camera_count: value.window_camera_count,
    };
    let file = RuntimeCheckpointFile {
        schema_version: RENDERDOC_RUNTIME_CHECKPOINT_SCHEMA_VERSION,
        status: "valid",
        contract_id: contract_id.to_string(),
        stage_id: stage_id.to_string(),
        generation: result.checkpoint.generation,
        checkpoint: RuntimeCheckpoint {
            name: RENDERDOC_CHECKPOINT_NAME,
            simulation_tick: result.checkpoint.simulation_tick,
            simulation_tick_source: SIMULATION_TICK_SOURCE,
            settle_frames: RENDERDOC_SETTLE_FRAMES,
            ready_frame_ordinal: result.ready_frame_ordinal,
            capture_frame: RENDERDOC_SETTLE_FRAMES,
            capture_begin_frame: result.capture_begin_frame,
            capture_end_frame: result.capture_end_frame,
            render_frame_index: result.render_frame_index,
            frame_count_before_capture: result.frame_count_before_capture,
        },
        render_inventory: result.checkpoint.render_inventory.into(),
        p02_presentation: result.checkpoint.p02_presentation.map(Into::into),
        runtime_field: result.checkpoint.runtime_field.clone(),
        gpu_light_field: result.checkpoint.gpu_light_field.clone(),
        cross_consumer: result.checkpoint.cross_consumer.clone(),
        render_resources: p01_composite_render_resources(),
        fixture: result.checkpoint.fixture.clone(),
        capture_path: &result.capture_path,
        requested_renderdoc_api_version: RENDERDOC_REQUESTED_API_VERSION,
        returned_renderdoc_api_version: result.returned_api_version.clone(),
        selector: RuntimeSelectorEvidence {
            strategy: RENDERDOC_SELECTOR_STRATEGY,
            device_selector: "wgpu::Device::start_graphics_debugger_capture",
            window_selector: "null",
            window_count: 1,
            primary_window: result.pre_capture_signature.primary_window.to_bits(),
        },
        gpu_ready: RuntimeGpuReadyEvidence {
            pre_capture: signature(result.pre_capture_signature),
            post_capture: signature(result.post_capture_signature),
        },
        capture_artifact: RuntimeCaptureArtifact {
            sha256: capture_sha256,
            bytes: capture_bytes,
        },
    };
    let mut bytes = serde_json::to_vec_pretty(&file).map_err(std::io::Error::other)?;
    bytes.push(b'\n');
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(temporary, destination)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderdoc_api_requires_compatible_major_one_prefix() {
        assert!(is_compatible_renderdoc_api_version(1, 6, 0));
        assert!(is_compatible_renderdoc_api_version(1, 7, 2));
        assert!(!is_compatible_renderdoc_api_version(1, 5, 9));
        assert!(!is_compatible_renderdoc_api_version(2, 0, 0));
    }

    #[test]
    fn current_medium_inventory_is_exact() {
        let inventory = PerfRenderInventory {
            scene_target_count: 1,
            mask_target_count: 1,
            camera_3d_rtt_count: 2,
            camera_2d_count: 3,
            layer_2d_pass_count: 2,
            soul_proxy_3d: 200,
            soul_mask_proxy_3d: 200,
            soul_shadow_proxy_3d: 200,
            familiar_proxy_3d: 12,
        };
        assert!(validate_medium_inventory("current", inventory).is_ok());
    }

    #[test]
    fn p01_medium_inventory_requires_scene_only_topology() {
        let inventory = PerfRenderInventory {
            scene_target_count: 1,
            mask_target_count: 0,
            camera_3d_rtt_count: 1,
            camera_2d_count: 3,
            layer_2d_pass_count: 2,
            soul_proxy_3d: 200,
            soul_mask_proxy_3d: 0,
            soul_shadow_proxy_3d: 200,
            familiar_proxy_3d: 12,
        };
        assert!(validate_medium_inventory("p01", inventory).is_ok());
        assert!(validate_medium_inventory("current", inventory).is_err());
    }

    #[test]
    fn p04_medium_inventory_preserves_p02_presentation_topology() {
        let inventory = PerfRenderInventory {
            scene_target_count: 1,
            mask_target_count: 0,
            camera_3d_rtt_count: 1,
            camera_2d_count: 2,
            layer_2d_pass_count: 1,
            soul_proxy_3d: 0,
            soul_mask_proxy_3d: 0,
            soul_shadow_proxy_3d: 0,
            familiar_proxy_3d: 0,
        };
        assert!(validate_medium_inventory("p04", inventory).is_ok());
        assert!(validate_medium_inventory("p06", inventory).is_ok());
        assert!(validate_medium_inventory("p01", inventory).is_err());
    }

    #[test]
    fn p06_linear_probe_pins_shader_order_before_post_processing() {
        assert!(topdown_structural_light_is_applied_before_post_processing());
    }

    #[test]
    fn p06_receivers_import_the_named_light_field_module() {
        let receiver_sources = [
            TOPDOWN_STRUCTURAL_SHADER,
            include_str!("../../../../../../assets/shaders/terrain_surface_material.wgsl"),
            include_str!(
                "../../../../../../assets/shaders/terrain_surface_material_lod1_lite.wgsl"
            ),
            include_str!("../../../../../../assets/shaders/terrain_surface_material_lod2.wgsl"),
        ];

        for source in receiver_sources {
            assert!(
                source.contains(
                    "#import hell_workers::indoor_light_field::sample_indoor_light_field"
                )
            );
            assert!(!source.contains("\"shaders/indoor_light_field.wgsl\""));
        }
    }

    #[test]
    fn p08_receivers_remove_soul_projectors_but_keep_directional_shadow_style() {
        let receiver_sources = [
            TOPDOWN_STRUCTURAL_SHADER,
            include_str!(
                "../../../../../../assets/shaders/topdown_structural_material_prepass.wgsl"
            ),
            include_str!("../../../../../../assets/shaders/terrain_surface_material.wgsl"),
            include_str!(
                "../../../../../../assets/shaders/terrain_surface_material_lod1_lite.wgsl"
            ),
            include_str!("../../../../../../assets/shaders/terrain_surface_material_lod2.wgsl"),
            include_str!("../../../../../../assets/shaders/terrain_surface_material_prepass.wgsl"),
            include_str!("../../../../../../assets/shaders/shadow_style.wgsl"),
        ];

        for source in receiver_sources {
            assert!(!source.contains("soul_shadow_projector"));
            assert!(!source.contains("soul_projected_shadow"));
        }

        let shadow_style = include_str!("../../../../../../assets/shaders/shadow_style.wgsl");
        assert!(shadow_style.contains("fn directional_shadow_visibility("));
        assert!(shadow_style.contains("fn apply_directional_shadow_style("));
    }

    #[test]
    fn p01_composite_binding_contract_is_exact() {
        let resources = p01_composite_render_resources();
        assert_eq!(resources.composite_draw_count, 1);
        assert_eq!(
            resources.composite_texture_bindings,
            vec![RuntimeCompositeTextureBinding {
                target: "scene_target",
                stage: "fragment",
                fixed_bind_set_or_space: 2,
                fixed_bind_number: 1,
            }]
        );
        assert_eq!(
            resources.composite_sampler_bindings,
            vec![RuntimeCompositeSamplerBinding {
                stage: "fragment",
                fixed_bind_set_or_space: 2,
                fixed_bind_number: 2,
            }]
        );
    }

    #[test]
    fn renderdoc_bridge_claims_capture_once() {
        let bridge = RenderDocBridge(Arc::new(Mutex::new(RenderDocBridgeState::Waiting)));

        assert!(bridge.try_begin_capture());
        assert!(matches!(bridge.snapshot(), RenderDocBridgeState::Capturing));
        assert!(!bridge.try_begin_capture());

        bridge.replace(RenderDocBridgeState::Failed("done".to_string()));
        assert!(!bridge.try_begin_capture());
    }

    #[test]
    fn renderdoc_settle_restarts_when_resident_pipeline_set_changes() {
        let signature = |pipeline_count| GpuReadySignature {
            pipeline_count,
            primary_window: Entity::from_bits(1),
            scene_camera_count: 1,
            mask_camera_count: 0,
            window_camera_count: 1,
        };
        let mut state = RenderDocRenderState::default();

        assert!(!observe_gpu_ready_signature(&mut state, signature(10)));
        assert!(!observe_gpu_ready_signature(&mut state, signature(10)));
        assert!(!observe_gpu_ready_signature(&mut state, signature(11)));
        assert_eq!(state.ready_frames, 1);
        assert!(!observe_gpu_ready_signature(&mut state, signature(11)));
        assert!(!observe_gpu_ready_signature(&mut state, signature(11)));
        assert!(observe_gpu_ready_signature(&mut state, signature(11)));
    }

    #[test]
    fn renderdoc_wait_watchdog_does_not_restart_when_reason_changes() {
        let mut state = RenderDocRenderState::default();

        for frame in 1..RENDERDOC_GATE_TIMEOUT_FRAMES {
            let reason = if frame % 2 == 0 { "queued" } else { "creating" };
            assert!(!observe_gpu_waiting(&mut state, reason.to_string()));
        }
        assert!(observe_gpu_waiting(
            &mut state,
            "waiting for a shader dependency".to_string()
        ));
        assert_eq!(state.waiting_frames, RENDERDOC_GATE_TIMEOUT_FRAMES);
        assert_eq!(
            state.waiting_reason.as_deref(),
            Some("waiting for a shader dependency")
        );
    }
}
