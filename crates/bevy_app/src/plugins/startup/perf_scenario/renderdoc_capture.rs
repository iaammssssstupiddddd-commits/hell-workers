//! Deterministic RenderDoc capture for the frozen RtT-light profiling fixture.

use super::super::rtt_composite::{
    RTT_COMPOSITE_BIND_SET_OR_SPACE, RTT_COMPOSITE_MASK_SAMPLER_BINDING,
    RTT_COMPOSITE_MASK_TEXTURE_BINDING, RTT_COMPOSITE_SCENE_SAMPLER_BINDING,
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
use bevy::render::renderer::RenderAdapterInfo;
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
const RENDERDOC_API_VERSION: &str = "1.6.0";
const RENDERDOC_RUNTIME_CHECKPOINT_SCHEMA_VERSION: u32 = 2;
const RTT_SCENE_LABEL: &str = "hell-workers-rtt-scene";
const RTT_MASK_LABEL: &str = "hell-workers-rtt-soul-mask";

type SoulWorldInstancesQuery<'w, 's> = Query<
    'w,
    's,
    &'static WorldInstance,
    Or<(
        With<SoulProxy3d>,
        With<SoulMaskProxy3d>,
        With<SoulShadowProxy3d>,
    )>,
>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CpuCheckpointSignature {
    checksum: u64,
    scene_target: AssetId<Image>,
    mask_target: AssetId<Image>,
    render_inventory: PerfRenderInventory,
}

#[derive(Clone, Debug)]
struct StableRenderDocCheckpoint {
    generation: u64,
    simulation_tick: u64,
    scene_target: AssetId<Image>,
    mask_target: AssetId<Image>,
    render_inventory: PerfRenderInventory,
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

#[derive(Resource, Clone, Default, ExtractResource)]
pub(crate) struct RenderDocCheckpointMailbox(Option<StableRenderDocCheckpoint>);

#[derive(Clone, Debug)]
struct RenderDocCaptureResult {
    checkpoint: StableRenderDocCheckpoint,
    render_frame_index: u64,
    capture_path: PathBuf,
}

#[derive(Clone, Debug)]
enum RenderDocBridgeState {
    Waiting,
    Capturing,
    Captured(RenderDocCaptureResult),
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
}

#[derive(Resource, Default)]
pub(crate) struct RenderDocMainState {
    previous: Option<CpuCheckpointSignature>,
    stable_updates: u8,
    next_generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    active: Option<(StableRenderDocCheckpoint, GpuReadySignature, u64)>,
}

type GetApiFn =
    unsafe extern "C" fn(renderdoc_sys::RENDERDOC_Version, *mut *mut c_void) -> std::os::raw::c_int;

struct RequiredRenderDocFns {
    get_num_captures: unsafe extern "C" fn() -> u32,
    get_capture: unsafe extern "C" fn(u32, *mut std::os::raw::c_char, *mut u32, *mut u64) -> u32,
    start_frame_capture: unsafe extern "C" fn(
        renderdoc_sys::RENDERDOC_DevicePointer,
        renderdoc_sys::RENDERDOC_WindowHandle,
    ),
    is_frame_capturing: unsafe extern "C" fn() -> u32,
    end_frame_capture: unsafe extern "C" fn(
        renderdoc_sys::RENDERDOC_DevicePointer,
        renderdoc_sys::RENDERDOC_WindowHandle,
    ) -> u32,
    discard_frame_capture: unsafe extern "C" fn(
        renderdoc_sys::RENDERDOC_DevicePointer,
        renderdoc_sys::RENDERDOC_WindowHandle,
    ) -> u32,
}

struct LoadedRenderDoc {
    _library: Library,
    functions: RequiredRenderDocFns,
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
    checksum_queries: PerfChecksumQueries<'w, 's>,
    virtual_time: Res<'w, Time<Virtual>>,
    rtt_runtime: Res<'w, RttRuntime>,
    render_environment: Res<'w, PerfRenderEnvironmentEvidence>,
    indoor_light_fixture: Res<'w, IndoorLightFixtureState>,
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

    let expected_instances = params.config.soul_count as usize * 3;
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
    if let Err(reason) = validate_current_medium_inventory(render_inventory) {
        bridge.replace(RenderDocBridgeState::Failed(reason));
        return;
    }
    let signature = CpuCheckpointSignature {
        checksum: checksum.value,
        scene_target: params.rtt_runtime.scene.id(),
        mask_target: params.rtt_runtime.soul_mask.id(),
        render_inventory,
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
        simulation_tick: 0,
        scene_target: signature.scene_target,
        mask_target: signature.mask_target,
        render_inventory,
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
            if let Err(error) = write_runtime_checkpoint(output_dir, &result) {
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
    if let Err(reason) = api.start_capture() {
        params.bridge.replace(RenderDocBridgeState::Failed(reason));
        return;
    }
    params.bridge.replace(RenderDocBridgeState::Capturing);
    state.active = Some((
        checkpoint.clone(),
        signature,
        u64::from(params.frame_count.0),
    ));
}

fn finish_renderdoc_frame(params: RenderDocRenderParams, mut state: ResMut<RenderDocRenderState>) {
    let Some((checkpoint, expected_signature, render_frame_index)) = state.active.take() else {
        return;
    };
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
        let _ = api.discard_capture();
        params.bridge.replace(RenderDocBridgeState::Failed(
            "GPU capture gate changed during the captured render frame".to_string(),
        ));
        return;
    }
    match api.end_capture() {
        Ok(capture_path) => {
            params
                .bridge
                .replace(RenderDocBridgeState::Captured(RenderDocCaptureResult {
                    checkpoint,
                    render_frame_index,
                    capture_path,
                }))
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
    let Some(mask) = params.images.get(checkpoint.mask_target) else {
        return Ok(None);
    };
    if scene.texture_descriptor.label != Some(RTT_SCENE_LABEL)
        || mask.texture_descriptor.label != Some(RTT_MASK_LABEL)
    {
        return Err("RtT GPU texture labels differ from the RenderDoc contract".to_string());
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
                if target.handle.id() == checkpoint.mask_target =>
            {
                mask_camera_count += 1;
            }
            Some(NormalizedRenderTarget::Window(_)) => window_camera_count += 1,
            _ => {}
        }
    }
    if scene_camera_count != 1 || mask_camera_count != 1 || window_camera_count == 0 {
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

fn validate_current_medium_inventory(inventory: PerfRenderInventory) -> Result<(), String> {
    let expected = PerfRenderInventory {
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
    if inventory == expected {
        Ok(())
    } else {
        Err(format!(
            "current medium RenderDoc inventory differs: observed={inventory:?} expected={expected:?}"
        ))
    }
}

impl LoadedRenderDoc {
    fn start_capture(&self) -> Result<(), String> {
        let functions = &self.functions;
        // SAFETY: All function pointers were negotiated from the retained 1.6
        // API table, and null device/window mean the single active window.
        unsafe {
            if (functions.get_num_captures)() != 0 {
                return Err("RenderDoc capture count was nonzero before arming".to_string());
            }
            if (functions.is_frame_capturing)() != 0 {
                return Err("RenderDoc was already capturing before arming".to_string());
            }
            (functions.start_frame_capture)(ptr::null_mut(), ptr::null_mut());
            if (functions.is_frame_capturing)() != 1 {
                return Err("RenderDoc StartFrameCapture did not become active".to_string());
            }
        }
        Ok(())
    }

    fn discard_capture(&self) -> Result<(), String> {
        // SAFETY: The function pointer belongs to the retained API table and
        // null handles select the only active captured window.
        let result =
            unsafe { (self.functions.discard_frame_capture)(ptr::null_mut(), ptr::null_mut()) };
        if result == 1 {
            Ok(())
        } else {
            Err("RenderDoc DiscardFrameCapture failed".to_string())
        }
    }

    fn end_capture(&self) -> Result<PathBuf, String> {
        let functions = &self.functions;
        // SAFETY: The function pointers belong to the retained API table and
        // null handles select the only active captured window.
        unsafe {
            if (functions.end_frame_capture)(ptr::null_mut(), ptr::null_mut()) != 1
                || (functions.is_frame_capturing)() != 0
            {
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
    if (major, minor, patch) != (1, 6, 0) {
        return Err(format!(
            "unexpected RenderDoc App API {major}.{minor}.{patch}"
        ));
    }
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
        start_frame_capture: api.StartFrameCapture.ok_or("StartFrameCapture is null")?,
        is_frame_capturing: api.IsFrameCapturing.ok_or("IsFrameCapturing is null")?,
        end_frame_capture: api.EndFrameCapture.ok_or("EndFrameCapture is null")?,
        discard_frame_capture: api
            .DiscardFrameCapture
            .ok_or("DiscardFrameCapture is null")?,
    };
    Ok(LoadedRenderDoc {
        _library: library,
        functions,
    })
}

#[cfg(not(target_os = "linux"))]
fn load_renderdoc(_capture_template: &Path) -> Result<LoadedRenderDoc, String> {
    Err("formal RenderDoc capture is currently supported only on Linux".to_string())
}

#[derive(Serialize)]
struct RuntimeCheckpointFile<'a> {
    schema_version: u32,
    status: &'static str,
    checkpoint: RuntimeCheckpoint,
    render_inventory: RuntimeRenderInventory,
    render_resources: RuntimeRenderResources,
    fixture: RuntimeFixtureEvidence,
    capture_path: &'a Path,
    renderdoc_api_version: &'static str,
}

#[derive(Serialize)]
struct RuntimeCheckpoint {
    name: &'static str,
    simulation_tick: u64,
    settle_frames: u32,
    capture_frame: u32,
    render_frame_index: u64,
    validated_frames: u32,
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
    mask_target_label: &'static str,
    composite_draw_count: u32,
    composite_texture_bindings: [RuntimeCompositeTextureBinding; 2],
    composite_sampler_bindings: [RuntimeCompositeSamplerBinding; 2],
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

fn current_composite_render_resources() -> RuntimeRenderResources {
    RuntimeRenderResources {
        scene_target_label: RTT_SCENE_LABEL,
        mask_target_label: RTT_MASK_LABEL,
        composite_draw_count: 1,
        composite_texture_bindings: [
            RuntimeCompositeTextureBinding {
                target: "scene_target",
                stage: "fragment",
                fixed_bind_set_or_space: RTT_COMPOSITE_BIND_SET_OR_SPACE,
                fixed_bind_number: RTT_COMPOSITE_SCENE_TEXTURE_BINDING,
            },
            RuntimeCompositeTextureBinding {
                target: "mask_target",
                stage: "fragment",
                fixed_bind_set_or_space: RTT_COMPOSITE_BIND_SET_OR_SPACE,
                fixed_bind_number: RTT_COMPOSITE_MASK_TEXTURE_BINDING,
            },
        ],
        composite_sampler_bindings: [
            RuntimeCompositeSamplerBinding {
                stage: "fragment",
                fixed_bind_set_or_space: RTT_COMPOSITE_BIND_SET_OR_SPACE,
                fixed_bind_number: RTT_COMPOSITE_SCENE_SAMPLER_BINDING,
            },
            RuntimeCompositeSamplerBinding {
                stage: "fragment",
                fixed_bind_set_or_space: RTT_COMPOSITE_BIND_SET_OR_SPACE,
                fixed_bind_number: RTT_COMPOSITE_MASK_SAMPLER_BINDING,
            },
        ],
    }
}

fn write_runtime_checkpoint(
    output_dir: &Path,
    result: &RenderDocCaptureResult,
) -> std::io::Result<()> {
    let metadata = std::fs::metadata(&result.capture_path)?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(std::io::Error::other(
            "RenderDoc reported an empty or missing capture",
        ));
    }
    std::fs::create_dir_all(output_dir)?;
    let destination = output_dir.join("renderdoc-checkpoint.json");
    let temporary = output_dir.join(format!(".renderdoc-checkpoint.{}.tmp", std::process::id()));
    let file = RuntimeCheckpointFile {
        schema_version: RENDERDOC_RUNTIME_CHECKPOINT_SCHEMA_VERSION,
        status: "valid",
        checkpoint: RuntimeCheckpoint {
            name: RENDERDOC_CHECKPOINT_NAME,
            simulation_tick: result.checkpoint.simulation_tick,
            settle_frames: RENDERDOC_SETTLE_FRAMES,
            capture_frame: RENDERDOC_SETTLE_FRAMES,
            render_frame_index: result.render_frame_index,
            validated_frames: 1,
        },
        render_inventory: result.checkpoint.render_inventory.into(),
        render_resources: current_composite_render_resources(),
        fixture: result.checkpoint.fixture.clone(),
        capture_path: &result.capture_path,
        renderdoc_api_version: RENDERDOC_API_VERSION,
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
        assert_eq!(validate_current_medium_inventory(inventory), Ok(()));
    }

    #[test]
    fn current_composite_binding_contract_is_exact() {
        let resources = current_composite_render_resources();
        assert_eq!(resources.composite_draw_count, 1);
        assert_eq!(
            resources.composite_texture_bindings,
            [
                RuntimeCompositeTextureBinding {
                    target: "scene_target",
                    stage: "fragment",
                    fixed_bind_set_or_space: 2,
                    fixed_bind_number: 1,
                },
                RuntimeCompositeTextureBinding {
                    target: "mask_target",
                    stage: "fragment",
                    fixed_bind_set_or_space: 2,
                    fixed_bind_number: 3,
                },
            ]
        );
        assert_eq!(
            resources.composite_sampler_bindings,
            [
                RuntimeCompositeSamplerBinding {
                    stage: "fragment",
                    fixed_bind_set_or_space: 2,
                    fixed_bind_number: 2,
                },
                RuntimeCompositeSamplerBinding {
                    stage: "fragment",
                    fixed_bind_set_or_space: 2,
                    fixed_bind_number: 4,
                },
            ]
        );
    }
}
