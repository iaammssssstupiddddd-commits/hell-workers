use bevy::prelude::*;
use hw_core::{GameSettings, SaveSlotId};
use hw_ui::components::{SaveCatalogDialog, UiInputCapture, UiInputState};
use hw_ui::setup::UiAssets;
use hw_ui::theme::UiTheme;

use crate::assets::GameAssets;
use crate::systems::save::catalog::{
    AutosaveGenerationLimit, SaveContentStatus, SaveFileRevision, SaveStorageRoot,
    format_relative_modified, scan_save_catalog,
};
use crate::systems::save::load::{apply_startup_load, prepare_startup_load};
use crate::systems::save::{SaveCatalogMode, SaveCatalogUi, SaveLoadFailureKind, SaveRecoveryMode};
use crate::world::map::{
    GeneratedWorldLayoutResource, WORLDGEN_SEED_ENV, WorldMap, build_terrain_feature_map,
    build_terrain_id_map, generated_world_layout_for_seed, prepare_generated_world_layout_resource,
    spawn_boundary_meshes,
};

use super::startup_systems::{
    initial_resource_spawner_timed, populate_resource_spatial_grid, spawn_entities,
    spawn_familiar_wrapper, spawn_map_timed, spawn_terrain_chunks_timed,
};
use super::{PerfScenarioConfig, rtt_composite, visual_handles};

#[derive(Resource)]
pub(crate) struct StartupChooser {
    root: Entity,
    was_paused: bool,
    invalid_versions: Vec<(SaveSlotId, SaveFileRevision)>,
}

#[derive(Component, Clone)]
enum StartupChoice {
    NewGame,
    Resume {
        slot: SaveSlotId,
        revision: SaveFileRevision,
        seed: u64,
    },
}

pub(crate) fn gameplay_ready(chooser: Option<Res<StartupChooser>>) -> bool {
    chooser.is_none()
}

pub(super) fn begin_startup(world: &mut World) {
    let perf = world.resource::<PerfScenarioConfig>();
    let explicit_seed = std::env::var(WORLDGEN_SEED_ENV)
        .ok()
        .and_then(|seed| seed.parse::<u64>().ok())
        .is_some();
    if !perf.enabled() && !explicit_seed {
        let slots = resume_slots(world, &[]);
        if !slots.is_empty() {
            let root = spawn_chooser(world, slots, None);
            let mut time = world.resource_mut::<Time<Virtual>>();
            let was_paused = time.is_paused();
            time.pause();
            world.insert_resource(StartupChooser {
                root,
                was_paused,
                invalid_versions: Vec::new(),
            });
            return;
        }
    }
    initialize_new_world(world);
}

fn resume_slots(
    world: &World,
    invalid_versions: &[(SaveSlotId, SaveFileRevision)],
) -> Vec<(String, StartupChoice)> {
    let root = world.resource::<SaveStorageRoot>();
    let generations = AutosaveGenerationLimit(
        world
            .resource::<GameSettings>()
            .normalized_autosave_generations(),
    );
    let (entries, _) = scan_save_catalog(root, None, generations, &[]);
    let now = std::time::SystemTime::now();
    entries
        .into_iter()
        .filter_map(|entry| {
            let SaveContentStatus::CurrentV1 { worldgen_seed } = entry.content else {
                return None;
            };
            let label = if invalid_versions.contains(&(entry.slot, entry.revision.clone())) {
                format!("{}（読み込み失敗・再試行）", entry.player_label())
            } else {
                format!(
                    "{}  ·  {}",
                    entry.player_label(),
                    format_relative_modified(entry.modified, now)
                )
            };
            Some((
                label,
                StartupChoice::Resume {
                    slot: entry.slot,
                    revision: entry.revision,
                    seed: worldgen_seed,
                },
            ))
        })
        .collect()
}

fn spawn_chooser(
    world: &mut World,
    slots: Vec<(String, StartupChoice)>,
    error: Option<&str>,
) -> Entity {
    world.resource_mut::<SaveCatalogUi>().mode = SaveCatalogMode::LoadCatalog;
    let font = world.resource::<GameAssets>().font_ui().clone();
    let theme = world.resource::<UiTheme>();
    let background = theme.colors.dialog_bg;
    let button_color = theme.colors.button_default;
    let font_color = theme.colors.text_primary_semantic;
    world
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(background),
            GlobalZIndex(30_000),
            UiInputCapture,
            SaveCatalogDialog,
            Name::new("Startup save chooser"),
        ))
        .with_children(|root| {
            root.spawn(Node {
                flex_direction: FlexDirection::Column,
                width: Val::Px(420.0),
                max_width: Val::Percent(94.0),
                max_height: Val::Percent(94.0),
                row_gap: Val::Px(5.0),
                ..default()
            })
            .with_children(|panel| {
                let text_font = TextFont {
                    font: font.clone().into(),
                    font_size: FontSize::Px(17.0),
                    ..default()
                };
                panel.spawn((
                    Text::new("保存から再開"),
                    text_font.clone(),
                    TextColor(font_color),
                ));
                if let Some(error) = error {
                    panel.spawn((Text::new(error), text_font.clone(), TextColor(font_color)));
                }
                for (label, choice) in
                    std::iter::once(("新規ゲーム".to_owned(), StartupChoice::NewGame)).chain(slots)
                {
                    panel
                        .spawn((
                            Button,
                            choice,
                            Node {
                                min_height: Val::Px(32.0),
                                padding: UiRect::all(Val::Px(5.0)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                            BackgroundColor(button_color),
                        ))
                        .with_children(|button| {
                            button.spawn((
                                Text::new(label),
                                text_font.clone(),
                                TextColor(font_color),
                            ));
                        });
                }
            });
        })
        .id()
}

pub(super) fn accept_startup_choice(world: &mut World) {
    if !world.contains_resource::<StartupChooser>() {
        return;
    }
    let choice = world
        .query::<(Entity, &StartupChoice)>()
        .iter(world)
        .find(|(entity, _)| world.resource::<UiInputState>().button_activated(*entity))
        .map(|(_, choice)| choice.clone());
    let Some(choice) = choice else { return };
    let mut chooser = world
        .remove_resource::<StartupChooser>()
        .expect("startup chooser is active");
    world.despawn(chooser.root);
    *world.resource_mut::<SaveCatalogUi>() = SaveCatalogUi::default();

    if let StartupChoice::Resume {
        slot,
        revision,
        seed,
    } = choice
    {
        let path = world.resource::<SaveStorageRoot>().resolve(slot);
        let result = prepare_startup_load(world, &path, &revision, seed).and_then(|prepared| {
            initialize_map(world, generated_world_layout_for_seed(seed));
            apply_startup_load(world, prepared)
        });
        if let Err(cause) = result {
            if *world.resource::<SaveRecoveryMode>() == SaveRecoveryMode::RecoveryFailed {
                initialize_terrain_presentation(world);
                finish_startup(world);
                return;
            }
            world.remove_resource::<GeneratedWorldLayoutResource>();
            world.insert_resource(WorldMap::default());
            world.insert_resource(crate::world::regrowth::RegrowthManager::default());
            if cause == SaveLoadFailureKind::InvalidData {
                chooser
                    .invalid_versions
                    .retain(|(previous, _)| *previous != slot);
                chooser.invalid_versions.push((slot, revision));
            }
            let error = match cause {
                SaveLoadFailureKind::LoadSelectionChanged => {
                    "保存内容が更新されました。スロットを選び直してください。"
                }
                SaveLoadFailureKind::InvalidData => "保存データを読み込めませんでした。",
                _ => "ワールドを再開できませんでした。スロットを選び直してください。",
            };
            let slots = resume_slots(world, &chooser.invalid_versions);
            chooser.root = spawn_chooser(world, slots, Some(error));
            world.insert_resource(chooser);
            return;
        }
        initialize_terrain_presentation(world);
    } else {
        initialize_new_world(world);
    }
    if !chooser.was_paused {
        world.resource_mut::<Time<Virtual>>().unpause();
    }
    finish_startup(world);
}

fn initialize_new_world(world: &mut World) {
    let layout = prepare_generated_world_layout_resource(world.resource::<PerfScenarioConfig>());
    initialize_map(world, layout);
    initialize_terrain_presentation(world);
    world
        .run_system_cached(initial_resource_spawner_timed)
        .expect("spawn initial resources");
    world
        .run_system_cached(spawn_entities)
        .expect("spawn initial Souls");
    world
        .run_system_cached(spawn_familiar_wrapper)
        .expect("spawn initial Familiar");
}

fn initialize_map(world: &mut World, layout: GeneratedWorldLayoutResource) {
    info!(
        "BEVY_STARTUP: Prepared worldgen layout (seed={}, attempt={}, fallback={})",
        layout.master_seed, layout.layout.generation_attempt, layout.layout.used_fallback,
    );
    world.insert_resource(layout);
    world
        .run_system_cached(spawn_map_timed)
        .expect("initialize terrain tiles");
}

fn initialize_terrain_presentation(world: &mut World) {
    world
        .run_system_cached(build_terrain_feature_map)
        .expect("build terrain feature map");
    world
        .run_system_cached(build_terrain_id_map)
        .expect("build terrain ID map");
    world
        .run_system_cached(visual_handles::bind_terrain_maps)
        .expect("bind terrain maps");
    world
        .run_system_cached(spawn_terrain_chunks_timed)
        .expect("spawn terrain chunks");
    world
        .run_system_cached(spawn_boundary_meshes)
        .expect("initialize terrain boundaries");
}

pub(super) fn finish_startup(world: &mut World) {
    world
        .run_system_cached(crate::interface::ui::setup_ui)
        .expect("initialize gameplay UI");
    world
        .run_system_cached(crate::interface::ui::dev_panel::spawn_dev_panel_system)
        .expect("initialize developer panel");
    world
        .run_system_cached(populate_resource_spatial_grid)
        .expect("populate resource spatial grid");
    world
        .run_system_cached(rtt_composite::spawn_rtt_composite_sprite)
        .expect("initialize terrain composite");
}
