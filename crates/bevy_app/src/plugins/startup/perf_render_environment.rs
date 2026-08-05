//! Profiling-only evidence for the concrete window/surface configuration.

use super::PerfScenarioConfig;
use bevy::prelude::*;
use bevy::render::renderer::{RenderAdapter, RenderAdapterInfo, RenderInstance};
use bevy::render::view::window::{ExtractedWindows, create_surfaces};
use bevy::render::{Render, RenderApp};
use bevy::window::PresentMode;
use std::env;
use std::sync::{Arc, Mutex};
use wgpu::rwh::RawDisplayHandle;
use wgpu::{PresentMode as WgpuPresentMode, SurfaceCapabilities, SurfaceTargetUnsafe};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PerfRenderEnvironment {
    pub(super) window_backend: &'static str,
    pub(super) adapter_name: String,
    pub(super) adapter_backend: &'static str,
    pub(super) requested_present_mode: &'static str,
    pub(super) effective_present_mode: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum PerfRenderEnvironmentState {
    Disabled,
    Pending,
    Ready(PerfRenderEnvironment),
    Failed(String),
}

#[derive(Resource, Clone)]
pub(super) struct PerfRenderEnvironmentEvidence(Arc<Mutex<PerfRenderEnvironmentState>>);

impl PerfRenderEnvironmentEvidence {
    pub(super) fn snapshot(&self) -> PerfRenderEnvironmentState {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn replace_pending(&self, state: PerfRenderEnvironmentState) {
        let mut current = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if matches!(*current, PerfRenderEnvironmentState::Pending) {
            *current = state;
        }
    }
}

pub(super) fn install(app: &mut App) {
    let enabled = app
        .world()
        .get_resource::<PerfScenarioConfig>()
        .is_some_and(PerfScenarioConfig::enabled);
    let headless =
        env::var("HW_WINDOW_BACKEND").is_ok_and(|value| value.eq_ignore_ascii_case("headless"));
    let evidence = PerfRenderEnvironmentEvidence(Arc::new(Mutex::new(if enabled && !headless {
        PerfRenderEnvironmentState::Pending
    } else {
        PerfRenderEnvironmentState::Disabled
    })));
    app.insert_resource(evidence.clone());
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        evidence.replace_pending(PerfRenderEnvironmentState::Failed(
            "RenderApp is unavailable for a windowed profiling run".to_string(),
        ));
        return;
    };
    render_app.insert_resource(evidence).add_systems(
        Render,
        observe_perf_render_environment.before(create_surfaces),
    );
}

fn observe_perf_render_environment(
    windows: Res<ExtractedWindows>,
    render_instance: Res<RenderInstance>,
    render_adapter: Res<RenderAdapter>,
    adapter_info: Res<RenderAdapterInfo>,
    evidence: Res<PerfRenderEnvironmentEvidence>,
) {
    if !matches!(evidence.snapshot(), PerfRenderEnvironmentState::Pending) {
        return;
    }
    let Some(primary) = windows.primary else {
        return;
    };
    let Some(window) = windows.windows.get(&primary) else {
        return;
    };
    let window_backend = match window.handle.get_display_handle() {
        RawDisplayHandle::Xlib(_) | RawDisplayHandle::Xcb(_) => "x11",
        RawDisplayHandle::Wayland(_) => "wayland",
        other => {
            evidence.replace_pending(PerfRenderEnvironmentState::Failed(format!(
                "unsupported profiling display handle: {other:?}"
            )));
            return;
        }
    };
    let target = SurfaceTargetUnsafe::RawHandle {
        raw_display_handle: Some(window.handle.get_display_handle()),
        raw_window_handle: window.handle.get_window_handle(),
    };
    // SAFETY: ExtractedWindow owns an Arc-backed raw-handle wrapper for the live
    // window. The temporary surface is dropped in this system immediately after
    // capability inspection and before Bevy's create_surfaces system runs.
    let surface = match unsafe { render_instance.create_surface_unsafe(target) } {
        Ok(surface) => surface,
        Err(error) => {
            evidence.replace_pending(PerfRenderEnvironmentState::Failed(format!(
                "failed to create profiling capability surface: {error}"
            )));
            return;
        }
    };
    let capabilities = surface.get_capabilities(&render_adapter);
    let effective = match resolve_present_mode(window.present_mode, &capabilities) {
        Ok(mode) => mode,
        Err(reason) => {
            evidence.replace_pending(PerfRenderEnvironmentState::Failed(reason));
            return;
        }
    };
    drop(surface);
    evidence.replace_pending(PerfRenderEnvironmentState::Ready(PerfRenderEnvironment {
        window_backend,
        adapter_name: adapter_info.name.clone(),
        adapter_backend: adapter_info.backend.to_str(),
        requested_present_mode: bevy_present_mode_name(window.present_mode),
        effective_present_mode: wgpu_present_mode_name(effective),
    }));
}

fn resolve_present_mode(
    requested: PresentMode,
    capabilities: &SurfaceCapabilities,
) -> Result<WgpuPresentMode, String> {
    let requested = match requested {
        PresentMode::Fifo => WgpuPresentMode::Fifo,
        PresentMode::FifoRelaxed => WgpuPresentMode::FifoRelaxed,
        PresentMode::Mailbox => WgpuPresentMode::Mailbox,
        PresentMode::Immediate => WgpuPresentMode::Immediate,
        PresentMode::AutoVsync => WgpuPresentMode::AutoVsync,
        PresentMode::AutoNoVsync => WgpuPresentMode::AutoNoVsync,
    };
    let fallbacks: &[WgpuPresentMode] = match requested {
        WgpuPresentMode::AutoVsync => &[WgpuPresentMode::FifoRelaxed, WgpuPresentMode::Fifo],
        WgpuPresentMode::AutoNoVsync => &[
            WgpuPresentMode::Immediate,
            WgpuPresentMode::Mailbox,
            WgpuPresentMode::Fifo,
        ],
        WgpuPresentMode::Mailbox => &[
            WgpuPresentMode::Mailbox,
            WgpuPresentMode::Immediate,
            WgpuPresentMode::Fifo,
        ],
        mode => &[mode, WgpuPresentMode::Fifo],
    };
    fallbacks
        .iter()
        .copied()
        .find(|mode| capabilities.present_modes.contains(mode))
        .ok_or_else(|| {
            format!(
                "no concrete present-mode fallback for {requested:?}; capabilities={:?}",
                capabilities.present_modes
            )
        })
}

fn bevy_present_mode_name(mode: PresentMode) -> &'static str {
    match mode {
        PresentMode::Fifo => "fifo",
        PresentMode::FifoRelaxed => "fifo_relaxed",
        PresentMode::Mailbox => "mailbox",
        PresentMode::Immediate => "immediate",
        PresentMode::AutoVsync => "auto_vsync",
        PresentMode::AutoNoVsync => "auto_no_vsync",
    }
}

fn wgpu_present_mode_name(mode: WgpuPresentMode) -> &'static str {
    match mode {
        WgpuPresentMode::Fifo => "fifo",
        WgpuPresentMode::FifoRelaxed => "fifo_relaxed",
        WgpuPresentMode::Immediate => "immediate",
        WgpuPresentMode::Mailbox => "mailbox",
        WgpuPresentMode::AutoVsync | WgpuPresentMode::AutoNoVsync => "unresolved_auto",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capabilities(present_modes: Vec<WgpuPresentMode>) -> SurfaceCapabilities {
        SurfaceCapabilities {
            formats: vec![],
            present_modes,
            alpha_modes: vec![],
            usages: wgpu::TextureUsages::RENDER_ATTACHMENT,
        }
    }

    #[test]
    fn present_mode_resolution_matches_bevy_0_19_fallback_order() {
        let caps = capabilities(vec![WgpuPresentMode::Mailbox, WgpuPresentMode::Fifo]);
        assert_eq!(
            resolve_present_mode(PresentMode::AutoNoVsync, &caps),
            Ok(WgpuPresentMode::Mailbox)
        );
        assert_eq!(
            resolve_present_mode(PresentMode::Immediate, &caps),
            Ok(WgpuPresentMode::Fifo)
        );
        assert_eq!(
            resolve_present_mode(PresentMode::AutoVsync, &caps),
            Ok(WgpuPresentMode::Fifo)
        );
    }
}
