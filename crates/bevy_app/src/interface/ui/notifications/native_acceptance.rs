//! Opt-in actual-window acceptance for Track A2 placement and notifications.
//!
//! The driver is dormant unless `HW_NATIVE_NOTIFICATION_ACCEPTANCE_ARTIFACT`
//! is set. It observes the production placement tooltip and notification
//! adapter/reducer/presenter paths without synthetic desktop input.

use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::render::renderer::RenderAdapterInfo;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::render::view::window::ExtractedWindows;
use bevy::render::{Render, RenderApp};
use bevy::ui::FocusPolicy;
use bevy::window::PrimaryWindow;
use hw_ui::components::{HoverTooltip, TooltipTemplate, UiInputBlocker};
use hw_ui::notifications::{
    NotificationCenter, NotificationHistoryPanel, NotificationHistoryRow, NotificationToastRoot,
    NotificationToastRow, NotificationToastSurface,
};
use hw_ui::selection::{
    AreaPlacementPlan, PlacementFeedbackState, PlacementFeedbackStatus, PlacementRejectReason,
    PlacementTileRejection, PlacementValidation,
};

use crate::systems::save::{
    LoadRequestOrigin, SaveLoadFailureKind, SaveLoadOperation, SaveLoadOutcome,
    SaveLoadOutcomeSource, SaveLoadResult, SaveRequestOrigin, SaveStorageRoot,
};
use crate::systems::settings::SettingsStorageRoot;

const ARTIFACT_ENV: &str = "HW_NATIVE_NOTIFICATION_ACCEPTANCE_ARTIFACT";
const RUNTIME_ROOT_ENV: &str = "HW_NATIVE_NOTIFICATION_ACCEPTANCE_RUNTIME_ROOT";
const RUN_ID_ENV: &str = "HW_NATIVE_NOTIFICATION_ACCEPTANCE_RUN_ID";
const RESULT_FILE: &str = "driver-result.json";
const SCREENSHOT_FILE: &str = "a2-notifications.png";
const READY_FRAMES: u32 = 30;
const DRIVER_TIMEOUT: Duration = Duration::from_secs(90);
const PRESENT_SETTLE: Duration = Duration::from_millis(150);
const TOAST_EXPIRY_TIMEOUT: Duration = Duration::from_secs(7);
const MIN_SCREENSHOT_WIDTH: u32 = 640;
const MIN_SCREENSHOT_HEIGHT: u32 = 360;
const MAX_SCREENSHOT_BYTES: u64 = 16 * 1024 * 1024;

/// Adds a bounded A2 sequence to the regular actual-window application.
/// This plugin is never enabled by default.
pub struct NativeNotificationAcceptancePlugin {
    artifact_dir: PathBuf,
    runtime_root: PathBuf,
    run_id: String,
}

impl NativeNotificationAcceptancePlugin {
    /// Returns an opt-in plugin when the artifact environment variable is set.
    pub fn try_from_process() -> Result<Option<Self>, String> {
        let Some(raw_artifact) = env::var_os(ARTIFACT_ENV) else {
            return Ok(None);
        };
        if raw_artifact.is_empty() {
            return Err(format!("{ARTIFACT_ENV} must not be empty"));
        }
        let artifact_dir = PathBuf::from(raw_artifact);
        if !artifact_dir.is_absolute() || !artifact_dir.is_dir() {
            return Err(format!(
                "{ARTIFACT_ENV} must name an existing absolute directory"
            ));
        }
        let runtime_root =
            PathBuf::from(env::var_os(RUNTIME_ROOT_ENV).ok_or_else(|| {
                format!("{RUNTIME_ROOT_ENV} must name a fresh runtime directory")
            })?);
        if !runtime_root.is_absolute() || !runtime_root.is_dir() {
            return Err(format!(
                "{RUNTIME_ROOT_ENV} must name an existing absolute directory"
            ));
        }
        let canonical_artifact = artifact_dir.canonicalize().map_err(|error| {
            format!(
                "could not canonicalize A2 artifact {}: {error}",
                artifact_dir.display()
            )
        })?;
        let canonical_runtime = runtime_root.canonicalize().map_err(|error| {
            format!(
                "could not canonicalize A2 runtime {}: {error}",
                runtime_root.display()
            )
        })?;
        if canonical_runtime.starts_with(&canonical_artifact) {
            return Err(format!(
                "{RUNTIME_ROOT_ENV} must remain outside {ARTIFACT_ENV}"
            ));
        }
        if env::var("HW_WINDOW_BACKEND").is_ok_and(|value| value.eq_ignore_ascii_case("headless")) {
            return Err(format!(
                "{ARTIFACT_ENV} cannot be combined with HW_WINDOW_BACKEND=headless"
            ));
        }
        let run_id = env::var(RUN_ID_ENV)
            .map_err(|_| format!("{RUN_ID_ENV} must be set to a fresh run identifier"))?;
        validate_run_id(&run_id)?;
        for owned_path in [
            artifact_dir.join(RESULT_FILE),
            artifact_dir.join(SCREENSHOT_FILE),
            runtime_root.join("settings/settings.ron"),
        ] {
            if owned_path.exists() {
                return Err(format!(
                    "A2 acceptance contains stale output: {}",
                    owned_path.display()
                ));
            }
        }
        for runtime_dir in [runtime_root.join("saves"), runtime_root.join("settings")] {
            fs::create_dir_all(&runtime_dir).map_err(|error| {
                format!(
                    "could not initialize A2 runtime root {}: {error}",
                    runtime_dir.display()
                )
            })?;
        }
        Ok(Some(Self {
            artifact_dir,
            runtime_root,
            run_id,
        }))
    }
}

impl Plugin for NativeNotificationAcceptancePlugin {
    fn build(&self, app: &mut App) {
        let render_evidence = NativeRenderEvidence::pending();
        app.insert_resource(SaveStorageRoot::new(self.runtime_root.join("saves")))
            .insert_resource(SettingsStorageRoot::new(self.runtime_root.join("settings")))
            .insert_resource(NativeNotificationAcceptance::new(
                self.artifact_dir.clone(),
                self.runtime_root.clone(),
                self.run_id.clone(),
                render_evidence.clone(),
            ));
        install_render_evidence(app, render_evidence);
        app.add_systems(PostUpdate, drive_native_notification_acceptance);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RenderEnvironment {
    adapter_name: String,
    adapter_backend: String,
    display_handle: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum RenderEvidenceState {
    Pending,
    Ready(RenderEnvironment),
    Failed(String),
}

#[derive(Resource, Clone)]
struct NativeRenderEvidence(Arc<Mutex<RenderEvidenceState>>);

impl NativeRenderEvidence {
    fn pending() -> Self {
        Self(Arc::new(Mutex::new(RenderEvidenceState::Pending)))
    }

    fn snapshot(&self) -> RenderEvidenceState {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn replace_pending(&self, next: RenderEvidenceState) {
        let mut state = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if matches!(*state, RenderEvidenceState::Pending) {
            *state = next;
        }
    }
}

fn install_render_evidence(app: &mut App, evidence: NativeRenderEvidence) {
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        evidence.replace_pending(RenderEvidenceState::Failed(
            "RenderApp is unavailable for A2 native acceptance".to_owned(),
        ));
        return;
    };
    render_app
        .insert_resource(evidence)
        .add_systems(Render, observe_render_environment);
}

fn observe_render_environment(
    windows: Res<ExtractedWindows>,
    adapter_info: Res<RenderAdapterInfo>,
    evidence: Res<NativeRenderEvidence>,
) {
    if !matches!(evidence.snapshot(), RenderEvidenceState::Pending) {
        return;
    }
    let Some(primary) = windows.primary else {
        return;
    };
    let Some(window) = windows.windows.get(&primary) else {
        return;
    };
    evidence.replace_pending(RenderEvidenceState::Ready(RenderEnvironment {
        adapter_name: adapter_info.name.clone(),
        adapter_backend: adapter_info.backend.to_str().to_owned(),
        display_handle: format!("{:?}", window.handle.get_display_handle()),
    }));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AcceptanceStage {
    WaitForUi,
    AwaitPlacementTooltip,
    AwaitNotifications,
    AwaitHistory,
    AwaitToastExpiry,
    AwaitFinalPresentation,
    AwaitScreenshot,
    Finished,
}

#[derive(Resource)]
struct NativeNotificationAcceptance {
    artifact_dir: PathBuf,
    runtime_root: PathBuf,
    run_id: String,
    stage: AcceptanceStage,
    started_at: Instant,
    stage_started_at: Instant,
    ready_frames: u32,
    history_reopen_attempts: u8,
    checks: [bool; 5],
    render_evidence: NativeRenderEvidence,
}

impl NativeNotificationAcceptance {
    fn new(
        artifact_dir: PathBuf,
        runtime_root: PathBuf,
        run_id: String,
        render_evidence: NativeRenderEvidence,
    ) -> Self {
        Self {
            artifact_dir,
            runtime_root,
            run_id,
            stage: AcceptanceStage::WaitForUi,
            started_at: Instant::now(),
            stage_started_at: Instant::now(),
            ready_frames: 0,
            history_reopen_attempts: 0,
            checks: [false; 5],
            render_evidence,
        }
    }

    fn result_path(&self) -> PathBuf {
        self.artifact_dir.join(RESULT_FILE)
    }

    fn screenshot_path(&self) -> PathBuf {
        self.artifact_dir.join(SCREENSHOT_FILE)
    }

    fn enter(&mut self, stage: AcceptanceStage) {
        self.stage = stage;
        self.stage_started_at = Instant::now();
    }
}

fn drive_native_notification_acceptance(world: &mut World) {
    let Some(mut driver) = world.remove_resource::<NativeNotificationAcceptance>() else {
        return;
    };
    if driver.stage == AcceptanceStage::Finished {
        return;
    }
    if driver.started_at.elapsed() > DRIVER_TIMEOUT {
        fail_driver(world, &mut driver, "A2 native acceptance timed out");
        return;
    }
    if let Err(reason) = drive_stage(world, &mut driver) {
        fail_driver(world, &mut driver, &reason);
        return;
    }
    if driver.stage != AcceptanceStage::Finished {
        world.insert_resource(driver);
    }
}

fn drive_stage(world: &mut World, driver: &mut NativeNotificationAcceptance) -> Result<(), String> {
    match driver.stage {
        AcceptanceStage::WaitForUi => wait_for_ui(world, driver),
        AcceptanceStage::AwaitPlacementTooltip => await_placement_tooltip(world, driver),
        AcceptanceStage::AwaitNotifications => await_notifications(world, driver),
        AcceptanceStage::AwaitHistory => await_history(world, driver),
        AcceptanceStage::AwaitToastExpiry => await_toast_expiry(world, driver),
        AcceptanceStage::AwaitFinalPresentation => await_final_presentation(world, driver),
        AcceptanceStage::AwaitScreenshot => await_screenshot(world, driver),
        AcceptanceStage::Finished => Ok(()),
    }
}

fn wait_for_ui(world: &mut World, driver: &mut NativeNotificationAcceptance) -> Result<(), String> {
    let ready = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .iter(world)
        .count()
        == 1
        && world
            .query_filtered::<Entity, With<NotificationToastRoot>>()
            .iter(world)
            .count()
            == 1
        && world
            .query_filtered::<Entity, With<NotificationHistoryPanel>>()
            .iter(world)
            .count()
            == 1
        && world
            .query_filtered::<Entity, With<HoverTooltip>>()
            .iter(world)
            .count()
            == 1;
    if !ready {
        driver.ready_frames = 0;
        return Ok(());
    }
    driver.ready_frames += 1;
    if driver.ready_frames < READY_FRAMES {
        return Ok(());
    }

    validate_placement_contract()?;
    let now = world.resource::<Time<Real>>().elapsed();
    world
        .resource_mut::<PlacementFeedbackState>()
        .show_recent_rejection(PlacementRejectReason::OccupiedByBuilding, (7, 9), now);
    driver.enter(AcceptanceStage::AwaitPlacementTooltip);
    Ok(())
}

fn validate_placement_contract() -> Result<(), String> {
    if PlacementRejectReason::ALL
        .into_iter()
        .any(|reason| reason.message(1, 2).trim().is_empty())
    {
        return Err("one or more placement rejection reasons have no display text".to_owned());
    }
    let partial = AreaPlacementPlan {
        valid_tiles: vec![(0, 0), (1, 0)],
        total_tile_count: 3,
        first_reject: Some(PlacementTileRejection {
            grid: (2, 0),
            reason: PlacementRejectReason::OutOfBounds,
        }),
    }
    .feedback()
    .ok_or_else(|| "partial area placement did not produce feedback".to_owned())?;
    if partial.status != PlacementFeedbackStatus::Partial
        || partial.valid_tile_count != 2
        || partial.rejected_tile_count != 1
        || !partial.body().contains("2 valid, 1 skipped")
    {
        return Err("partial placement feedback lost its valid/skipped contract".to_owned());
    }

    let target = (4, 5);
    let occupied = PlacementValidation::rejected(PlacementRejectReason::OccupiedByBuilding);
    let mut blocker = PlacementFeedbackState::default();
    blocker.block_live_feedback_at(target);
    blocker.set_live_building_validation(&occupied, target);
    if blocker.live.is_some() {
        return Err("same-anchor placement blocker exposed self-interference".to_owned());
    }
    blocker.set_live_building_validation(&occupied, (5, 5));
    if blocker.live.is_none() {
        return Err("placement blocker did not release after the cursor moved".to_owned());
    }
    Ok(())
}

fn await_placement_tooltip(
    world: &mut World,
    driver: &mut NativeNotificationAcceptance,
) -> Result<(), String> {
    if driver.stage_started_at.elapsed() < PRESENT_SETTLE {
        return Ok(());
    }
    let now = world.resource::<Time<Real>>().elapsed();
    let feedback = world
        .resource::<PlacementFeedbackState>()
        .visible(now)
        .ok_or_else(|| "typed placement feedback was not visible".to_owned())?;
    if feedback.header() != "Cannot place"
        || !feedback.body().contains("Tile (7,9)")
        || !feedback.body().contains("occupied by a building")
    {
        return Err("placement feedback did not preserve its typed reason".to_owned());
    }

    let tooltip_entity = single_entity::<HoverTooltip>(world, "hover tooltip")?;
    let tooltip = world
        .get::<HoverTooltip>(tooltip_entity)
        .ok_or_else(|| "hover tooltip component disappeared".to_owned())?;
    let node = world
        .get::<Node>(tooltip_entity)
        .ok_or_else(|| "hover tooltip node is missing".to_owned())?;
    let text = descendant_text(world, tooltip_entity).join("\n");
    if tooltip.template_type != TooltipTemplate::PlacementRejected
        || node.display == Display::None
        || !text.contains("Cannot place")
        || !text.contains("Tile (7,9)")
    {
        return Err("production placement tooltip did not render the rejection reason".to_owned());
    }
    driver.checks[0] = true;
    write_notification_fixture(world);
    driver.enter(AcceptanceStage::AwaitNotifications);
    Ok(())
}

fn write_notification_fixture(world: &mut World) {
    world.write_message(SaveLoadOutcome {
        operation: SaveLoadOperation::Save,
        target: "Manual 1".to_owned(),
        result: SaveLoadResult::Succeeded,
        source: SaveLoadOutcomeSource::Manual(SaveRequestOrigin::ManualCatalog {
            dialog_session: 1,
        }),
    });
    let missing = SaveLoadOutcome {
        operation: SaveLoadOperation::Load,
        target: "Manual 2".to_owned(),
        result: SaveLoadResult::Failed(SaveLoadFailureKind::LoadNotFound),
        source: SaveLoadOutcomeSource::Load(LoadRequestOrigin::NormalCatalog { dialog_session: 2 }),
    };
    world.write_message(missing.clone());
    world.write_message(missing);
    world.write_message(SaveLoadOutcome {
        operation: SaveLoadOperation::Load,
        target: "Manual 3".to_owned(),
        result: SaveLoadResult::Failed(SaveLoadFailureKind::SeedMismatch),
        source: SaveLoadOutcomeSource::Load(LoadRequestOrigin::NormalCatalog { dialog_session: 3 }),
    });
}

fn await_notifications(
    world: &mut World,
    driver: &mut NativeNotificationAcceptance,
) -> Result<(), String> {
    if driver.stage_started_at.elapsed() < PRESENT_SETTLE {
        return Ok(());
    }
    let center = world.resource::<NotificationCenter>();
    if center.toast_count() != 3 || center.history_count() != 3 {
        return Err(format!(
            "notification reducer produced {} toasts and {} history entries; expected 3 and 3",
            center.toast_count(),
            center.history_count()
        ));
    }
    let entries = center.history_entries().collect::<Vec<_>>();
    let repeated = entries
        .iter()
        .find(|entry| entry.title == "Save not found")
        .ok_or_else(|| "load-not-found notification is missing".to_owned())?;
    if repeated.repeat_count != 2 || !repeated.body.contains("Manual 2") {
        return Err("identical load failures did not coalesce with a repeat count".to_owned());
    }
    if !entries
        .iter()
        .any(|entry| entry.title == "Game saved" && entry.body.contains("Manual 1"))
        || !entries
            .iter()
            .any(|entry| entry.title == "World seed mismatch" && entry.body.contains("Manual 3"))
    {
        return Err(
            "save/load target and result text are missing from notification history".to_owned(),
        );
    }
    if entries.iter().any(|entry| {
        entry.title.contains('/')
            || entry.title.contains('\\')
            || entry.body.contains("/home/")
            || entry.body.contains("\\Users\\")
    }) {
        return Err("notification text exposed a filesystem path".to_owned());
    }
    let toast_root = single_entity::<NotificationToastRoot>(world, "notification toast root")?;
    if world
        .get::<Node>(toast_root)
        .is_none_or(|node| node.display != Display::Flex)
        || count_entities::<NotificationToastRow>(world) != 3
    {
        return Err("notification presenter did not render three visible toast rows".to_owned());
    }
    validate_toast_pick_through(world)?;
    driver.checks[1] = true;
    driver.checks[2] = true;
    driver.checks[3] = true;
    world.resource_mut::<NotificationCenter>().toggle_history();
    driver.enter(AcceptanceStage::AwaitHistory);
    Ok(())
}

fn validate_toast_pick_through(world: &mut World) -> Result<(), String> {
    let mut surfaces = world.query_filtered::<
        (Option<&Pickable>, Option<&FocusPolicy>),
        With<NotificationToastSurface>,
    >();
    let mut count = 0;
    for (pickable, focus) in surfaces.iter(world) {
        count += 1;
        if pickable != Some(&Pickable::IGNORE) || focus != Some(&FocusPolicy::Pass) {
            return Err("a toast surface can capture pointer or focus input".to_owned());
        }
    }
    if count == 0 {
        return Err("notification presenter produced no toast surfaces".to_owned());
    }
    Ok(())
}

fn await_history(
    world: &mut World,
    driver: &mut NativeNotificationAcceptance,
) -> Result<(), String> {
    if driver.stage_started_at.elapsed() < PRESENT_SETTLE {
        return Ok(());
    }
    let panel = single_entity::<NotificationHistoryPanel>(world, "notification history panel")?;
    if world
        .get::<Node>(panel)
        .is_none_or(|node| node.display != Display::Flex)
        || world.get::<UiInputBlocker>(panel).is_none()
        || world.get::<FocusPolicy>(panel) != Some(&FocusPolicy::Block)
        || count_entities::<NotificationHistoryRow>(world) != 3
    {
        return Err("history panel did not render as the blocking foreground surface".to_owned());
    }
    driver.checks[4] = true;
    world.resource_mut::<Time<Virtual>>().pause();
    driver.enter(AcceptanceStage::AwaitToastExpiry);
    Ok(())
}

fn await_toast_expiry(
    world: &mut World,
    driver: &mut NativeNotificationAcceptance,
) -> Result<(), String> {
    if !world.resource::<Time<Virtual>>().is_paused() {
        return Err("virtual time resumed during the pause-expiry check".to_owned());
    }
    if world.resource::<NotificationCenter>().toast_count() != 0 {
        if driver.stage_started_at.elapsed() > TOAST_EXPIRY_TIMEOUT {
            return Err(
                "notification toasts did not expire while virtual time was paused".to_owned(),
            );
        }
        return Ok(());
    }
    let toast_root = single_entity::<NotificationToastRoot>(world, "notification toast root")?;
    if world
        .get::<Node>(toast_root)
        .is_none_or(|node| node.display != Display::None)
    {
        return Err("expired toast stack remained visible".to_owned());
    }
    // Toast expiry above must be proven while paused, but the placement
    // tooltip's intentional reveal animation uses Time<Virtual>. Resume only
    // for the final presentation setup, then pause again before capture.
    world.resource_mut::<Time<Virtual>>().unpause();
    let now = world.resource::<Time<Real>>().elapsed();
    world
        .resource_mut::<PlacementFeedbackState>()
        .show_recent_rejection(PlacementRejectReason::NotInYard, (11, 12), now);
    world.write_message(SaveLoadOutcome {
        operation: SaveLoadOperation::Load,
        target: "Manual 1".to_owned(),
        result: SaveLoadResult::Succeeded,
        source: SaveLoadOutcomeSource::Load(LoadRequestOrigin::NormalCatalog { dialog_session: 4 }),
    });
    driver.enter(AcceptanceStage::AwaitFinalPresentation);
    Ok(())
}

fn await_final_presentation(
    world: &mut World,
    driver: &mut NativeNotificationAcceptance,
) -> Result<(), String> {
    if driver.stage_started_at.elapsed() < PRESENT_SETTLE {
        return Ok(());
    }
    let tooltip = single_entity::<HoverTooltip>(world, "hover tooltip")?;
    let history = single_entity::<NotificationHistoryPanel>(world, "notification history panel")?;
    let toast = single_entity::<NotificationToastRoot>(world, "notification toast root")?;
    let tooltip_visible = world
        .get::<Node>(tooltip)
        .is_some_and(|node| node.display != Display::None);
    let history_visible = world
        .get::<Node>(history)
        .is_some_and(|node| node.display == Display::Flex);
    let toast_visible = world
        .get::<Node>(toast)
        .is_some_and(|node| node.display == Display::Flex);
    if !history_visible {
        let history_open = world.resource::<NotificationCenter>().history_open();
        if !history_open && driver.history_reopen_attempts < 3 {
            world.resource_mut::<NotificationCenter>().toggle_history();
            driver.history_reopen_attempts += 1;
            driver.stage_started_at = Instant::now();
            return Ok(());
        }
        if driver.stage_started_at.elapsed() < Duration::from_secs(1) {
            return Ok(());
        }
    }
    if !tooltip_visible || !history_visible || !toast_visible {
        return Err(format!(
            "final A2 UI scene visibility is tooltip={tooltip_visible}, toast={toast_visible}, history={history_visible} after {} history reopen attempts",
            driver.history_reopen_attempts,
        ));
    }
    world.resource_mut::<Time<Virtual>>().pause();
    spawn_pass_banner(world);
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(driver.screenshot_path()));
    driver.enter(AcceptanceStage::AwaitScreenshot);
    Ok(())
}

fn spawn_pass_banner(world: &mut World) {
    world.spawn((
        Text::new("Native A2 Acceptance PASS"),
        TextFont {
            font_size: FontSize::Px(24.0),
            ..default()
        },
        TextColor(Color::WHITE),
        BackgroundColor(Color::srgba(0.0, 0.35, 0.12, 0.94)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(16.0),
            top: Val::Px(16.0),
            padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BorderColor::all(Color::srgb(0.25, 1.0, 0.45)),
        ZIndex(10_000),
        Name::new("Native A2 Acceptance PASS"),
    ));
}

fn await_screenshot(
    world: &mut World,
    driver: &mut NativeNotificationAcceptance,
) -> Result<(), String> {
    if !driver.screenshot_path().is_file() {
        return Ok(());
    }
    let screenshot = validate_png(&driver.screenshot_path())?;
    let renderer = match driver.render_evidence.snapshot() {
        RenderEvidenceState::Ready(renderer) => renderer,
        RenderEvidenceState::Pending => return Ok(()),
        RenderEvidenceState::Failed(reason) => return Err(reason),
    };
    if driver.checks.into_iter().any(|passed| !passed) {
        return Err("A2 reached screenshot before every acceptance check passed".to_owned());
    }
    write_success_result(driver, &renderer, screenshot)?;
    driver.stage = AcceptanceStage::Finished;
    info!("NATIVE_NOTIFICATION_ACCEPTANCE: PASS");
    world.write_message(AppExit::Success);
    Ok(())
}

fn single_entity<T: Component>(world: &mut World, label: &str) -> Result<Entity, String> {
    let entities = world
        .query_filtered::<Entity, With<T>>()
        .iter(world)
        .collect::<Vec<_>>();
    if entities.len() != 1 {
        return Err(format!("expected one {label}, observed {}", entities.len()));
    }
    Ok(entities[0])
}

fn count_entities<T: Component>(world: &mut World) -> usize {
    world
        .query_filtered::<Entity, With<T>>()
        .iter(world)
        .count()
}

fn descendant_text(world: &World, root: Entity) -> Vec<String> {
    let mut result = Vec::new();
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if let Some(text) = world.get::<Text>(entity) {
            result.push(text.0.clone());
        }
        if let Some(children) = world.get::<Children>(entity) {
            stack.extend(children.iter());
        }
    }
    result
}

#[derive(Clone, Copy, Debug)]
struct PngEvidence {
    width: u32,
    height: u32,
    bytes: u64,
}

fn validate_png(path: &Path) -> Result<PngEvidence, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("could not read screenshot {}: {error}", path.display()))?;
    let (width, height) = validate_png_structure(&bytes)?;
    let byte_count = bytes.len() as u64;
    if width < MIN_SCREENSHOT_WIDTH || height < MIN_SCREENSHOT_HEIGHT {
        return Err(format!("A2 screenshot is too small: {width}x{height}"));
    }
    if byte_count == 0 || byte_count > MAX_SCREENSHOT_BYTES {
        return Err(format!("A2 screenshot size is invalid: {byte_count} bytes"));
    }
    Ok(PngEvidence {
        width,
        height,
        bytes: byte_count,
    })
}

fn validate_png_structure(bytes: &[u8]) -> Result<(u32, u32), String> {
    const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < SIGNATURE.len() || &bytes[..SIGNATURE.len()] != SIGNATURE {
        return Err("A2 screenshot is not a PNG".to_owned());
    }
    let mut offset = SIGNATURE.len();
    let mut dimensions = None;
    let mut saw_idat = false;
    while offset < bytes.len() {
        let header_end = offset
            .checked_add(8)
            .ok_or_else(|| "A2 PNG chunk offset overflowed".to_owned())?;
        if header_end > bytes.len() {
            return Err("A2 PNG has a truncated chunk header".to_owned());
        }
        let data_len = u32::from_be_bytes(
            bytes[offset..offset + 4]
                .try_into()
                .expect("four-byte PNG chunk length"),
        ) as usize;
        let chunk_type = &bytes[offset + 4..offset + 8];
        let data_end = header_end
            .checked_add(data_len)
            .ok_or_else(|| "A2 PNG chunk length overflowed".to_owned())?;
        let chunk_end = data_end
            .checked_add(4)
            .ok_or_else(|| "A2 PNG CRC offset overflowed".to_owned())?;
        if chunk_end > bytes.len() {
            return Err("A2 PNG has a truncated chunk".to_owned());
        }
        let expected_crc = u32::from_be_bytes(
            bytes[data_end..chunk_end]
                .try_into()
                .expect("four-byte PNG chunk CRC"),
        );
        if png_crc32(&bytes[offset + 4..data_end]) != expected_crc {
            return Err("A2 PNG has a corrupt chunk CRC".to_owned());
        }
        match chunk_type {
            b"IHDR" => {
                if offset != SIGNATURE.len() || data_len != 13 || dimensions.is_some() {
                    return Err("A2 PNG has an invalid IHDR".to_owned());
                }
                let width =
                    u32::from_be_bytes(bytes[header_end..header_end + 4].try_into().unwrap());
                let height =
                    u32::from_be_bytes(bytes[header_end + 4..header_end + 8].try_into().unwrap());
                if width == 0 || height == 0 {
                    return Err("A2 PNG has zero dimensions".to_owned());
                }
                dimensions = Some((width, height));
            }
            b"IDAT" => saw_idat = true,
            b"IEND" => {
                if data_len != 0 || dimensions.is_none() || !saw_idat || chunk_end != bytes.len() {
                    return Err("A2 PNG has an invalid terminal IEND".to_owned());
                }
                return Ok(dimensions.expect("dimensions checked above"));
            }
            _ if dimensions.is_none() => {
                return Err("A2 PNG does not start with IHDR".to_owned());
            }
            _ => {}
        }
        offset = chunk_end;
    }
    Err("A2 PNG is missing its terminal IEND".to_owned())
}

fn png_crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn write_success_result(
    driver: &NativeNotificationAcceptance,
    renderer: &RenderEnvironment,
    screenshot: PngEvidence,
) -> Result<(), String> {
    let body = format!(
        concat!(
            "{{\n",
            "  \"status\": \"PASS\",\n",
            "  \"profile\": \"player-facing-result-notifications\",\n",
            "  \"run_id\": \"{}\",\n",
            "  \"checks\": {{\"A1\": \"PASS\", \"A2\": \"PASS\", \"A3\": \"PASS\", \"A4\": \"PASS\", \"A5\": \"PASS\"}},\n",
            "  \"runtime\": {{\"save_root\": \"{}\", \"settings_root\": \"{}\"}},\n",
            "  \"screenshot\": {{\"path\": \"{}\", \"width\": {}, \"height\": {}, \"bytes\": {}}},\n",
            "  \"renderer\": {{\"adapter_name\": \"{}\", \"backend\": \"{}\", \"display_handle\": \"{}\"}}\n",
            "}}\n"
        ),
        json_escape(&driver.run_id),
        json_escape(&driver.runtime_root.join("saves").display().to_string()),
        json_escape(&driver.runtime_root.join("settings").display().to_string()),
        json_escape(&driver.screenshot_path().display().to_string()),
        screenshot.width,
        screenshot.height,
        screenshot.bytes,
        json_escape(&renderer.adapter_name),
        json_escape(&renderer.adapter_backend),
        json_escape(&renderer.display_handle),
    );
    write_new_atomic(&driver.result_path(), body.as_bytes())
        .map_err(|error| format!("could not publish A2 acceptance result: {error}"))
}

fn fail_driver(world: &mut World, driver: &mut NativeNotificationAcceptance, reason: &str) {
    if driver.stage == AcceptanceStage::Finished {
        return;
    }
    let body = format!(
        "{{\n  \"status\": \"FAIL\",\n  \"profile\": \"player-facing-result-notifications\",\n  \"run_id\": \"{}\",\n  \"reason\": \"{} (stage {:?})\"\n}}\n",
        json_escape(&driver.run_id),
        json_escape(reason),
        driver.stage,
    );
    if let Err(error) = write_new_atomic(&driver.result_path(), body.as_bytes()) {
        error!("NATIVE_NOTIFICATION_ACCEPTANCE: {reason}; result write failed: {error}");
    } else {
        error!("NATIVE_NOTIFICATION_ACCEPTANCE: {reason}");
    }
    driver.stage = AcceptanceStage::Finished;
    world.write_message(AppExit::error());
}

fn validate_run_id(run_id: &str) -> Result<(), String> {
    if run_id.is_empty()
        || run_id.len() > 64
        || !run_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(format!(
            "{RUN_ID_ENV} must contain 1..=64 ASCII letters, digits, '-' or '_'"
        ));
    }
    Ok(())
}

fn write_new_atomic(path: &Path, contents: &[u8]) -> io::Result<()> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid output filename"))?;
    let temporary_path = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));
    let write_result = (|| {
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)?;
        output.write_all(contents)?;
        output.sync_all()?;
        fs::hard_link(&temporary_path, path)?;
        Ok(())
    })();
    let _ = fs::remove_file(temporary_path);
    write_result
}

fn json_escape(value: &str) -> String {
    value
        .chars()
        .flat_map(char::escape_default)
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_contract_covers_reasons_partial_summary_and_same_anchor_blocker() {
        validate_placement_contract().unwrap();
    }

    #[test]
    fn run_id_rejects_paths_and_multiline_values() {
        assert!(validate_run_id("a2-native_20260809").is_ok());
        assert!(validate_run_id("").is_err());
        assert!(validate_run_id("../other-run").is_err());
        assert!(validate_run_id("first\nsecond").is_err());
    }
}
