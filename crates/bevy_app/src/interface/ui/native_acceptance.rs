//! Profiling-only fixtures and passive observations for OS-driven UI acceptance.
//! After fixture preparation this module never writes UI interaction or game state.
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use bevy::ecs::message::MessageCursor;
use bevy::prelude::*;
use bevy::ui_widgets::{ScrollArea, ScrollbarThumb};
use hw_core::{GameSettings, WorldEpoch};
use hw_ui::components::*;
use hw_ui::notifications::*;
use hw_ui::panels::task_list::{
    TaskActionButton, TaskDashboardControl, TaskDashboardViewState, TaskListScroll,
};
use serde_json::{Value, json};

use crate::systems::save::{SaveLoadOutcome, SaveStorageRoot};
use crate::systems::settings::SettingsStorageRoot;

pub struct NativeUiAcceptancePlugin {
    root: PathBuf,
    nonce: String,
    size: Vec2,
    scale: f32,
}

impl NativeUiAcceptancePlugin {
    pub fn window_resolution(&self) -> bevy::window::WindowResolution {
        bevy::window::WindowResolution::new(self.size.x as u32, self.size.y as u32)
            .with_scale_factor_override(1.0)
    }

    pub fn try_from_process() -> Result<Option<Self>, String> {
        let Some(root) = std::env::var_os("HW_NATIVE_UI_ROOT") else {
            return Ok(None);
        };
        if std::env::var("HW_WINDOW_BACKEND").as_deref() != Ok("x11") {
            return Err("UI input acceptance requires an actual X11 window".into());
        }
        let root = PathBuf::from(root);
        if !root.is_absolute() || !root.is_dir() || root.join("observed.json").exists() {
            return Err("HW_NATIVE_UI_ROOT requires a fresh absolute directory".into());
        }
        let nonce = std::env::var("HW_NATIVE_UI_NONCE").map_err(|error| error.to_string())?;
        if nonce.is_empty()
            || !nonce
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err("invalid UI acceptance nonce".into());
        }
        let number = |name: &str| -> Result<f32, String> {
            std::env::var(name)
                .map_err(|error| error.to_string())?
                .parse()
                .map_err(|error: std::num::ParseFloatError| error.to_string())
        };
        let size = Vec2::new(
            number("HW_NATIVE_UI_WIDTH")?,
            number("HW_NATIVE_UI_HEIGHT")?,
        );
        let scale = number("HW_NATIVE_UI_SCALE")?;
        if !size.is_finite()
            || size.x < 1280.0
            || size.x > 3840.0
            || size.y < 720.0
            || size.y > 2160.0
            || !scale.is_finite()
            || !(0.85..=1.25).contains(&scale)
        {
            return Err("unsupported UI acceptance viewport".into());
        }
        Ok(Some(Self {
            root,
            nonce,
            size,
            scale,
        }))
    }
}

impl Plugin for NativeUiAcceptancePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SaveStorageRoot::new(self.root.join("saves")))
            .insert_resource(SettingsStorageRoot::new(self.root.join("settings")))
            .insert_resource(UiObserver {
                root: self.root.clone(),
                nonce: self.nonce.clone(),
                scale: self.scale,
                frame: 0,
                prepared: false,
                started: Instant::now(),
                save_cursor: MessageCursor::default(),
                outcomes: Vec::new(),
            })
            .add_systems(
                PostUpdate,
                observe_ui
                    .after(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate)
                    .after(bevy::ui::UiSystems::PostLayout)
                    .after(bevy::ui::UiSystems::Stack),
            );
    }
}

#[derive(Resource)]
struct UiObserver {
    root: PathBuf,
    nonce: String,
    scale: f32,
    frame: u64,
    prepared: bool,
    started: Instant,
    save_cursor: MessageCursor<SaveLoadOutcome>,
    outcomes: Vec<String>,
}

fn prepare_fixture(world: &mut World, scale: f32) {
    use hw_core::familiar::FamiliarPolicy;
    use hw_jobs::{Designation, PlayerIssuedDesignation, Priority, TaskSlots, Tree};
    world.resource_mut::<GameSettings>().ui_scale = scale;
    if std::env::var("HW_NATIVE_UI_LAYOUT_ONLY").as_deref() == Ok("1") {
        if std::env::var("HW_NATIVE_UI_LAYOUT_MENU").as_deref() == Ok("1") {
            *world.resource_mut::<MenuState>() = MenuState::Zones;
        }
        for mut node in world
            .query_filtered::<&mut Node, With<crate::interface::ui::dev_panel::DevPanelBody>>()
            .iter_mut(world)
        {
            node.display = Display::None;
        }
        for mut text in world.query_filtered::<&mut Text, With<crate::interface::ui::dev_panel::DevPanelMinimizeButtonLabel>>().iter_mut(world) {
            text.0 = "+".into();
        }
        let familiars: Vec<_> = world
            .query_filtered::<Entity, With<crate::entities::familiar::Familiar>>()
            .iter(world)
            .collect();
        let souls: Vec<_> = world
            .query_filtered::<Entity, With<crate::entities::damned_soul::DamnedSoul>>()
            .iter(world)
            .collect();
        // Exercise the production area renderer with unselected, established areas.
        for (index, familiar) in familiars.iter().enumerate() {
            let center = Vec2::new(-180.0 + index as f32 * 220.0, 40.0);
            world
                .entity_mut(*familiar)
                .insert(hw_core::area::TaskArea::from_points(
                    center - Vec2::new(100.0, 80.0),
                    center + Vec2::new(100.0, 80.0),
                ));
        }
        use hw_ui::shell::{UiShellState, WorkspaceAction};
        let scene =
            std::env::var("HW_NATIVE_UI_LAYOUT_SCENE").unwrap_or_else(|_| "management".into());
        match scene.as_str() {
            "management" => world.resource_mut::<UiShellState>().apply(
                WorkspaceAction::OpenEntities,
                None,
                None,
            ),
            "selection" => {
                world.resource_mut::<hw_ui::selection::SelectedEntity>().0 =
                    familiars.first().copied()
            }
            "area" | "area-details" => {
                world.resource_mut::<hw_ui::selection::SelectedEntity>().0 =
                    familiars.first().copied();
                world.resource_mut::<crate::app_contexts::TaskContext>().0 =
                    hw_core::game_state::TaskMode::AreaSelection(None);
                world
                    .resource_mut::<NextState<hw_core::game_state::PlayMode>>()
                    .set(hw_core::game_state::PlayMode::TaskDesignation);
            }
            "pinned" => {
                world
                    .resource_mut::<crate::interface::ui::InfoPanelPinState>()
                    .entity = familiars.first().copied();
                world.resource_mut::<hw_ui::selection::SelectedEntity>().0 =
                    familiars.get(1).copied();
            }
            "build" => *world.resource_mut::<MenuState>() = MenuState::Architect,
            "display" => world.resource_mut::<UiShellState>().apply(
                WorkspaceAction::ToggleDisplay,
                None,
                None,
            ),
            "normal" => {}
            _ => panic!("unsupported native UI layout scene"),
        }
        for (index, soul) in souls.into_iter().enumerate() {
            world
                .entity_mut(soul)
                .remove::<hw_core::relationships::CommandedBy>();
            if let Some(familiar) = familiars.get(index / 2) {
                world
                    .entity_mut(soul)
                    .insert(hw_core::relationships::CommandedBy(*familiar));
            }
        }
        world.resource_mut::<Time<Virtual>>().pause();
    }
    for mut policy in world.query::<&mut FamiliarPolicy>().iter_mut(world) {
        policy.set_all_allowed(false);
    }
    // This is isolated fixture setup, before the first external input checkpoint.
    let existing: Vec<_> = world
        .query_filtered::<Entity, With<Designation>>()
        .iter(world)
        .collect();
    for entity in existing {
        world.entity_mut(entity).remove::<Designation>();
    }
    for index in 0..200 {
        let pos = hw_world::WorldMap::grid_to_world(70 + index % 20, 70 + index / 20);
        world.spawn((
            Tree,
            Designation {
                work_type: hw_core::jobs::WorkType::Chop,
            },
            PlayerIssuedDesignation,
            TaskSlots::new(1),
            Priority(0),
            Transform::from_translation(pos.extend(0.0)),
            Name::new(format!("UI task {index:03}")),
        ));
    }
    for index in 0..64 {
        world.write_message(UserFacingNotification::new(
            format!("ui-history-{index:02}"),
            NotificationSeverity::Info,
            format!("通知 {index:02} — 長い日本語の表示確認"),
            "作業対象・結果・次に確認する場所が画面内で読めることを確認します。",
            NotificationRetention::Important,
        ));
    }
}

fn visible_rect(world: &World, entity: Entity, viewport: Vec2) -> Option<[f32; 4]> {
    if world
        .get::<InheritedVisibility>(entity)
        .is_some_and(|visibility| !visibility.get())
    {
        return None;
    }
    let computed = world.get::<ComputedNode>(entity)?;
    let transform = world.get::<UiGlobalTransform>(entity)?;
    if computed.size().min_element() <= 0.0 {
        return None;
    }
    let half = computed.size() * 0.5;
    let mut min = (transform.translation - half).max(Vec2::ZERO);
    let mut max = (transform.translation + half).min(viewport);
    // Use the renderer's clip: popovers can override a scrolling ancestor's
    // clip, so reconstructing clipping from ancestor overflow is incorrect.
    if let Some(clip) = world.get::<bevy::ui::CalculatedClip>(entity) {
        min = min.max(clip.clip.min);
        max = max.min(clip.clip.max);
    }
    let mut ancestor = Some(entity);
    while let Some(current) = ancestor {
        if world
            .get::<Node>(current)
            .is_some_and(|node| node.display == Display::None)
        {
            return None;
        }
        ancestor = world.get::<ChildOf>(current).map(ChildOf::parent);
    }
    ((max - min).min_element() > 1.0).then_some([min.x, min.y, max.x, max.y])
}

fn node_key(world: &World, entity: Entity) -> Option<String> {
    if world.get::<Text>(entity).is_some()
        && let Some(parent) = world.get::<ChildOf>(entity).map(ChildOf::parent)
        && (world
            .get::<hw_ui::area_edit::panel::AreaEditDetailsToggle>(parent)
            .is_some()
            || world
                .get::<hw_ui::area_edit::panel::AreaEditControlButton>(parent)
                .is_some()
            || world.get::<MenuButton>(parent).is_some_and(|button| {
                matches!(
                    button.0,
                    hw_ui::UiIntent::FinishAreaEdit | hw_ui::UiIntent::SelectAreaTaskFor(_)
                )
            }))
    {
        return node_key(world, parent).map(|key| format!("label:{key}"));
    }
    if world
        .get::<hw_ui::area_edit::panel::AreaEditDetailsToggle>(entity)
        .is_some()
    {
        return Some("area-details-toggle".into());
    }
    if let Some(control) = world.get::<hw_ui::area_edit::panel::AreaEditControlButton>(entity) {
        return Some(format!("area-control:{:?}", control.0));
    }
    if matches!(world.get::<UiSlot>(entity), Some(UiSlot::Header)) {
        return Some("text:inspection-header".into());
    }
    if world
        .get::<crate::interface::ui::dev_panel::DevPanelMinimizeButton>(entity)
        .is_some()
    {
        return Some("dev-minimize".into());
    }
    if let Some(button) = world.get::<MenuButton>(entity) {
        let mut ancestor = Some(entity);
        while let Some(current) = ancestor {
            if world.get::<PauseMenu>(current).is_some() {
                return Some(format!("pause-menu:{:?}", button.0));
            }
            ancestor = world.get::<ChildOf>(current).map(ChildOf::parent);
        }
        return Some(format!("menu:{:?}", button.0));
    }
    if let Some(tab) = world.get::<LeftPanelTabButton>(entity) {
        return Some(format!("tab:{:?}", tab.0));
    }
    if world.get::<EntityListMinimizeButton>(entity).is_some() {
        return Some("minimize".into());
    }
    if world.get::<NotificationHistoryButton>(entity).is_some() {
        return Some("notifications".into());
    }
    if world.get::<NotificationHistoryClose>(entity).is_some() {
        return Some("notifications-close".into());
    }
    if let Some(control) = world.get::<TaskDashboardControl>(entity) {
        return Some(format!("task-control:{control:?}"));
    }
    if let Some(action) = world.get::<TaskActionButton>(entity) {
        return Some(format!(
            "task-action:{}:{:?}",
            action.target.to_bits(),
            action.kind
        ));
    }
    if let Some(item) = world.get::<TaskListItem>(entity) {
        return Some(format!("task:{}", item.0.to_bits()));
    }
    if let Some(item) = world.get::<FamiliarListItem>(entity) {
        return Some(format!("familiar:{}", item.0.to_bits()));
    }
    if let Some(field) = world.get::<SettingsSliderMarker>(entity) {
        return Some(format!("slider:{:?}", field.0));
    }
    if let Some(field) = world.get::<hw_ui::widgets::text_field::TextFieldRole>(entity) {
        return Some(format!("text-field:{field:?}"));
    }
    if world.get::<ScrollArea>(entity).is_some() {
        if world.get::<TaskListScroll>(entity).is_some() {
            return Some("scroll:tasks".into());
        }
        if world.get::<NotificationHistoryScroll>(entity).is_some() {
            return Some("scroll:history".into());
        }
        return Some(format!(
            "scroll:{}",
            world.get::<Name>(entity).map_or("unnamed", Name::as_str)
        ));
    }
    if world.get::<ScrollbarThumb>(entity).is_some() {
        return Some(format!("thumb:{}", entity.to_bits()));
    }
    None
}

fn snapshot(world: &mut World, observer: &UiObserver) -> Value {
    let window = world
        .query::<&Window>()
        .single(world)
        .expect("native UI window");
    let viewport = Vec2::new(
        window.physical_width() as f32,
        window.physical_height() as f32,
    );
    let scale_factor = window.scale_factor();
    let base_scale_factor = window.resolution.base_scale_factor();
    let cursor = window
        .physical_cursor_position()
        .map(|position| [position.x, position.y]);
    let mut controls = serde_json::Map::new();
    let mut query = world.query_filtered::<Entity, With<Node>>();
    for entity in query.iter(world) {
        if let Some(key) = node_key(world, entity)
            && let Some(rect) = visible_rect(world, entity, viewport)
        {
            let computed = world.get::<ComputedNode>(entity).unwrap();
            let transform = world.get::<UiGlobalTransform>(entity).unwrap();
            let min = transform.translation - computed.size() * 0.5;
            let max = transform.translation + computed.size() * 0.5;
            let raw = [min.x, min.y, max.x, max.y];
            let fully_visible = raw.iter().zip(rect).all(|(a, b)| (a - b).abs() <= 1.0);
            controls.insert(key, json!({"rect": rect, "unclipped_rect": raw, "fully_visible": fully_visible, "entity": entity.to_bits(),
                "text": world.get::<Text>(entity).map(|text| &text.0),
                "interaction": world.get::<Interaction>(entity).map(|value| format!("{value:?}")),
                "scroll": world.get::<ScrollPosition>(entity).map(|scroll| [scroll.0.x, scroll.0.y])}));
        }
    }
    let mut visible_history = Vec::new();
    let mut history = world.query::<(Entity, &NotificationHistoryKey)>();
    for (entity, key) in history.iter(world) {
        if visible_rect(world, entity, viewport).is_some() {
            visible_history.push(key.0);
        }
    }
    visible_history.sort_unstable();
    let tooltip_entity = world
        .query_filtered::<Entity, With<HoverTooltip>>()
        .iter(world)
        .next();
    let tooltip_alpha = tooltip_entity
        .and_then(|entity| world.get::<HoverTooltip>(entity))
        .map_or(0.0, |tooltip| tooltip.fade_alpha);
    let presentation = |entity: Entity| {
        json!({
            "rect": visible_rect(world, entity, viewport),
            "stack_index": world.get::<bevy::ui::ComputedStackIndex>(entity).map(|index| index.0),
        })
    };
    let tooltip_presentation = tooltip_entity.map(presentation);
    let mode_presentation = world
        .resource::<UiNodeRegistry>()
        .get_slot(UiSlot::ModeText)
        .map(presentation);
    let tooltip_text = world
        .query_filtered::<(Entity, &Text, &TextColor), Or<(With<TooltipHeader>, With<TooltipBody>)>>()
        .iter(world)
        .map(|(entity, text, color)| json!({
            "text": text.0, "alpha": color.0.alpha(),
            "rect": visible_rect(world, entity, viewport),
        }))
        .collect::<Vec<_>>();
    let task_rows = world.query::<&TaskListItem>().iter(world).count();
    let list_text = world
        .query::<(Entity, &Text)>()
        .iter(world)
        .filter(|(entity, _)| {
            world
                .get::<ChildOf>(*entity)
                .is_some_and(|parent| world.get::<FamiliarListItem>(parent.parent()).is_some())
        })
        .map(|(entity, text)| {
            json!({"text": text.0,
            "rect": visible_rect(world, entity, viewport)})
        })
        .collect::<Vec<_>>();
    let zones_rect = world
        .query_filtered::<Entity, With<ZonesSubMenu>>()
        .iter(world)
        .find_map(|entity| visible_rect(world, entity, viewport));
    let context_menus = world
        .query_filtered::<Entity, With<ContextMenu>>()
        .iter(world)
        .count();
    let camera = world
        .query_filtered::<(&Transform, &Projection), With<hw_ui::camera::MainCamera>>()
        .iter(world)
        .map(|(transform, projection)| format!("{transform:?}:{projection:?}"))
        .collect::<Vec<_>>();
    let center = world.resource::<NotificationCenter>();
    let scene = std::env::var("HW_NATIVE_UI_LAYOUT_SCENE").unwrap_or_else(|_| "management".into());
    let panel_rect = |marker: Entity| visible_rect(world, marker, viewport);
    let inspector_rect = world
        .resource::<UiNodeRegistry>()
        .get_slot(UiSlot::InfoPanelRoot)
        .and_then(panel_rect);
    let management_rect = world
        .iter_entities()
        .find(|entity| entity.contains::<EntityListPanel>())
        .and_then(|entity| panel_rect(entity.id()));
    let catalog_rect = world
        .iter_entities()
        .find(|entity| entity.contains::<ArchitectSubMenu>())
        .and_then(|entity| panel_rect(entity.id()));
    let display_rect = world
        .iter_entities()
        .find(|entity| entity.contains::<hw_ui::world_view::WorldViewPanel>())
        .and_then(|entity| panel_rect(entity.id()));
    let ui_rects: Vec<_> = world
        .iter_entities()
        .filter(|entity| {
            entity.contains::<Node>()
                && (entity
                    .get::<BackgroundColor>()
                    .is_some_and(|color| color.0.alpha() > 0.0)
                    || entity
                        .get::<bevy::ui::BackgroundGradient>()
                        .is_some_and(|gradient| {
                            gradient.0.iter().any(|gradient| match gradient {
                                bevy::ui::Gradient::Linear(value) => {
                                    value.stops.iter().any(|stop| stop.color.alpha() > 0.0)
                                }
                                bevy::ui::Gradient::Radial(value) => {
                                    value.stops.iter().any(|stop| stop.color.alpha() > 0.0)
                                }
                                bevy::ui::Gradient::Conic(value) => {
                                    value.stops.iter().any(|stop| stop.color.alpha() > 0.0)
                                }
                            })
                        }))
        })
        .filter_map(|entity| panel_rect(entity.id()))
        .collect();
    let persistent_areas: Vec<_> = world.iter_entities().filter_map(|entity| {
        let kind = if entity.contains::<hw_visual::task_area_visual::TaskAreaVisual>() {
            "work"
        } else if entity.contains::<crate::entities::familiar::FamiliarRangeIndicator>() {
            "command"
        } else if entity.contains::<hw_visual::site_yard_visual::SiteYardBoundaryVisual>() {
            "site-yard"
        } else {
            return None;
        };
        Some(json!({
            "kind": kind,
            "visible": entity.get::<InheritedVisibility>().is_some_and(|visibility| visibility.get()),
        }))
    }).collect();
    let layout = json!({
        "area_edit_rect": world.iter_entities().find(|entity| entity.contains::<hw_ui::area_edit::panel::AreaEditPanel>()).and_then(|entity| panel_rect(entity.id())),
        "layout_scene": scene, "inspector_rect": inspector_rect, "management_rect": management_rect,
        "catalog_rect": catalog_rect, "display_rect": display_rect, "ui_rects": ui_rects,
        "workspace_page": format!("{:?}", world.resource::<hw_ui::shell::UiShellState>().page),
        "pinned": world.resource::<crate::interface::ui::InfoPanelPinState>().entity.map(Entity::to_bits),
        "persistent_areas": persistent_areas,
    });
    let mut value = json!({
        "nonce": observer.nonce, "frame": observer.frame, "pid": std::process::id(),
        "list_text": list_text,
        "zones_rect": zones_rect,
        "menu_state": format!("{:?}", world.resource::<MenuState>()),
        "elapsed": observer.started.elapsed().as_secs_f64(), "ready": observer.prepared && observer.frame > 90,
        "world_epoch": world.resource::<WorldEpoch>().get(),
        "viewport": [viewport.x, viewport.y], "scale_factor": scale_factor,
        "base_scale_factor": base_scale_factor, "dpi_mode": "viewport-override",
        "cursor": cursor,
        "mouse_left_pressed": world.resource::<ButtonInput<MouseButton>>().pressed(MouseButton::Left),
        "world_input_captured": world.resource::<UiInputState>().world_input_captured,
        "pointer_over_ui": world.resource::<UiInputState>().pointer_over_ui,
        "ui_scale": world.resource::<GameSettings>().ui_scale,
        "paused": world.resource::<Time<Virtual>>().is_paused(),
        "left_panel": format!("{:?}", world.resource::<LeftPanelMode>()),
        "minimized": world.resource::<hw_ui::list::EntityListMinimizeState>().minimized,
        "page": world.resource::<TaskDashboardViewState>().page_index, "task_rows": task_rows,
        "task_total": world.resource::<crate::interface::ui::panels::task_list::TaskListState>().snapshot.len(),
        "history_open": center.history_open(), "history_count": center.history_count(),
        "visible_history": visible_history, "tooltip_alpha": tooltip_alpha,
        "tooltip_presentation": tooltip_presentation, "tooltip_text": tooltip_text,
        "mode_presentation": mode_presentation,
        "catalog": format!("{:?}", world.resource::<crate::systems::save::SaveCatalogUi>().mode),
        "save_session": world.resource::<crate::systems::save::SaveCatalogUi>().session,
        "save_outcomes": observer.outcomes,
        "context_menus": context_menus,
        "camera": camera,
        "selected": world.resource::<hw_ui::selection::SelectedEntity>().0.map(Entity::to_bits),
        "controls": controls,
    });
    value
        .as_object_mut()
        .expect("snapshot object")
        .extend(layout.as_object().expect("layout object").clone());
    value
}

fn observe_ui(world: &mut World) {
    world.resource_scope(|world, mut observer: Mut<UiObserver>| {
        observer.frame += 1;
        if observer.started.elapsed() > Duration::from_secs(600) {
            world.write_message(AppExit::error());
            return;
        }
        if observer.frame == 45 {
            prepare_fixture(world, observer.scale);
            observer.prepared = true;
        }
        // Complete this scene's one-time fixture after the mode/model has settled.
        if observer.frame == 48
            && std::env::var("HW_NATIVE_UI_LAYOUT_ONLY").as_deref() == Ok("1")
            && std::env::var("HW_NATIVE_UI_LAYOUT_SCENE").as_deref() == Ok("area-details")
        {
            for mut panel in world
                .query::<&mut hw_ui::area_edit::panel::AreaEditPanel>()
                .iter_mut(world)
            {
                panel.details_expanded = true;
            }
        }
        let outcomes: Vec<_> = observer
            .save_cursor
            .read(world.resource::<Messages<SaveLoadOutcome>>())
            .map(|outcome| format!("{:?}:{:?}", outcome.operation, outcome.result))
            .collect();
        observer.outcomes.extend(outcomes);
        if observer.outcomes.len() > 32 {
            observer.outcomes.drain(..16);
        }
        if observer.frame < 46 || !observer.frame.is_multiple_of(3) {
            return;
        }
        let value = snapshot(world, &observer);
        let temporary = observer.root.join("observed.tmp");
        let result = fs::write(&temporary, value.to_string())
            .and_then(|()| fs::rename(&temporary, observer.root.join("observed.json")));
        if let Err(error) = result {
            error!("UI acceptance observation failed: {error}");
            world.write_message(AppExit::error());
        }
    });
}
