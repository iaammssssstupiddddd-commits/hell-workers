//! Deterministic RenderDoc capture for the frozen RtT-light profiling fixture.

use super::super::rtt_composite::{
    RTT_COMPOSITE_BIND_SET_OR_SPACE, RTT_COMPOSITE_SCENE_SAMPLER_BINDING,
    RTT_COMPOSITE_SCENE_TEXTURE_BINDING,
};
use super::*;
use bevy::asset::AssetId;
use bevy::camera::NormalizedRenderTarget;
use bevy::diagnostic::FrameCount;
use bevy::render::camera::ExtractedCamera;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::{CachedPipelineState, PipelineCache};
use bevy::render::renderer::{RenderAdapterInfo, RenderDevice};
use bevy::render::texture::GpuImage;
use bevy::render::view::window::ExtractedWindows;
use bevy::render::{Render, RenderApp, RenderSystems};
use bevy::world_serialization::{WorldInstance, WorldInstanceSpawner};
use libloading::Library;
use serde::Serialize;
use std::ffi::{CStr, CString, c_void};
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::{Arc, Mutex};

const RENDERDOC_SETTLE_FRAMES: u32 = 4;
const RENDERDOC_CHECKPOINT_NAME: &str = "indoor-light-fixture-ready-v1";
const RENDERDOC_REQUESTED_API_VERSION: &str = "1.6.0";
const RENDERDOC_RUNTIME_CHECKPOINT_SCHEMA_VERSION: u32 = 3;
const RENDERDOC_SELECTOR_STRATEGY: &str = "wgpu_device_null_window";
const SIMULATION_TICK_SOURCE: &str = "perf_capture.fixed_update_tick";
const RTT_SCENE_LABEL: &str = "hell-workers-rtt-scene";

type SoulWorldInstancesQuery<'w, 's> =
    Query<'w, 's, &'static WorldInstance, Or<(With<SoulProxy3d>, With<SoulShadowProxy3d>)>>;

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
    typed_emitter_components: u32,
    eligible_supplied_emitters: u32,
    indoor_mask_cells: u32,
    indoor_mask_checksum: String,
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
    active: Option<(StableRenderDocCheckpoint, GpuReadySignature, u64, u64)>,
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
    config: Res<'w, PerfScenarioConfig>,
    applied: Res<'w, PerfScenarioApplied>,
    capture: Res<'w, PerfCapture>,
    checksum_queries: PerfChecksumQueries<'w, 's>,
    virtual_time: Res<'w, Time<Virtual>>,
    rtt_runtime: Res<'w, RttRuntime>,
    render_environment: Res<'w, PerfRenderEnvironmentEvidence>,
    indoor_light_fixture: Res<'w, IndoorLightFixtureState>,
    indoor_light_runtime: Res<'w, crate::systems::lighting::IndoorLightRuntime>,
    room_lookup: Res<'w, hw_world::RoomTileLookup>,
    world_instance_spawner: Res<'w, WorldInstanceSpawner>,
    soul_world_instances: SoulWorldInstancesQuery<'w, 's>,
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
    pipelines: Res<'w, PipelineCache>,
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

pub(crate) fn arm_renderdoc_checkpoint_system(
    params: RenderDocCheckpointParams,
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
    let expected_instances = if selection.uses_p02_presentation() {
        0
    } else {
        params.config.soul_count as usize * 2
    };
    if params.soul_world_instances.iter().count() != expected_instances
        || !params
            .soul_world_instances
            .iter()
            .all(|instance| params.world_instance_spawner.instance_is_ready(**instance))
    {
        return;
    }

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
        Some(RuntimeFieldEvidence {
            typed_emitter_components: params.indoor_light_runtime.typed_emitter_components(),
            eligible_supplied_emitters: params.indoor_light_runtime.eligible_supplied_emitters(),
            indoor_mask_cells,
            indoor_mask_checksum: super::output::canonical_room_mask_checksum(tiles),
        })
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

fn begin_renderdoc_frame(params: RenderDocRenderParams, mut state: ResMut<RenderDocRenderState>) {
    if state.active.is_some() {
        return;
    }
    let Some(checkpoint) = params.mailbox.0.as_ref() else {
        return;
    };
    if state.generation != Some(checkpoint.generation) {
        state.generation = Some(checkpoint.generation);
        state.ready_signature = None;
        state.ready_frames = 0;
    }
    let signature = match gpu_ready_signature(&params, checkpoint) {
        Ok(Some(value)) => value,
        Ok(None) => return,
        Err(reason) => {
            params.bridge.replace(RenderDocBridgeState::Failed(reason));
            return;
        }
    };
    match state.ready_signature {
        None => state.ready_signature = Some(signature),
        Some(previous) if previous != signature => {
            params.bridge.replace(RenderDocBridgeState::Failed(
                "GPU capture gate changed after the settle window began".to_string(),
            ));
            return;
        }
        Some(_) => {}
    }
    state.ready_frames = state.ready_frames.saturating_add(1);
    if state.ready_frames < RENDERDOC_SETTLE_FRAMES {
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
    if current_signature != Ok(Some(expected_signature)) {
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
                Ok(Some(value)) => value,
                Ok(None) => {
                    params.bridge.replace(RenderDocBridgeState::Failed(
                        "GPU capture gate was unavailable after the captured frame".to_string(),
                    ));
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
) -> Result<Option<GpuReadySignature>, String> {
    gpu_signature(params, checkpoint, true)
}

fn gpu_signature(
    params: &RenderDocRenderParams,
    checkpoint: &StableRenderDocCheckpoint,
    before_render: bool,
) -> Result<Option<GpuReadySignature>, String> {
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
        return Ok(None);
    };
    let Some(window) = params.windows.windows.get(&primary) else {
        return Ok(None);
    };
    if window.swap_chain_texture_view.is_none() {
        return Ok(None);
    }
    if before_render && window.swap_chain_texture.is_none() {
        return Ok(None);
    }
    if !before_render && window.swap_chain_texture.is_some() {
        return Err("primary swapchain image was not presented by the captured frame".to_string());
    }
    let Some(scene) = params.images.get(checkpoint.scene_target) else {
        return Ok(None);
    };
    if scene.texture_descriptor.label != Some(RTT_SCENE_LABEL) {
        return Err("RtT GPU texture labels differ from the RenderDoc contract".to_string());
    }
    if checkpoint.mask_target.is_some() {
        return Err("P01 RenderDoc checkpoint unexpectedly retained a mask target".to_string());
    }
    if params.pipelines.waiting_pipelines().next().is_some() {
        return Ok(None);
    }
    let mut pipeline_count = 0;
    for pipeline in params.pipelines.pipelines() {
        pipeline_count += 1;
        match &pipeline.state {
            CachedPipelineState::Ok(_) => {}
            CachedPipelineState::Queued | CachedPipelineState::Creating(_) => return Ok(None),
            CachedPipelineState::Err(error) => {
                return Err(format!("render pipeline compilation failed: {error:?}"));
            }
        }
    }
    if pipeline_count == 0 {
        return Ok(None);
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
        return Ok(None);
    }
    Ok(Some(GpuReadySignature {
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
        camera_2d_count: if matches!(stage_id, "p02" | "p03" | "p04") {
            2
        } else {
            3
        },
        layer_2d_pass_count: if matches!(stage_id, "p02" | "p03" | "p04") {
            1
        } else {
            2
        },
        soul_proxy_3d: if matches!(stage_id, "p02" | "p03" | "p04") {
            0
        } else {
            200
        },
        soul_mask_proxy_3d: if stage_id == "current" { 200 } else { 0 },
        soul_shadow_proxy_3d: if matches!(stage_id, "p02" | "p03" | "p04") {
            0
        } else {
            200
        },
        familiar_proxy_3d: if matches!(stage_id, "p02" | "p03" | "p04") {
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
        assert!(validate_medium_inventory("p01", inventory).is_err());
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
}
