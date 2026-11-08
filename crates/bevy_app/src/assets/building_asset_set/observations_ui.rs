//! Read-only, physical-pixel UI observations for ordinary input collection.
//! A rectangle is not a click witness or permission to place a building.
use bevy::prelude::*;
use hw_core::game_state::TaskMode;
use hw_jobs::BuildingType;
use hw_ui::{
    components::{ArchitectBuildingPanel, MenuButton, MenuState},
    intents::UiIntent,
};
use serde_json::{Value, json};

fn equipment(action: UiIntent) -> Option<BuildingType> {
    match action {
        UiIntent::SelectBuild(kind)
            if matches!(
                kind,
                BuildingType::Tank
                    | BuildingType::MudMixer
                    | BuildingType::RestArea
                    | BuildingType::Door
                    | BuildingType::WheelbarrowParking
                    | BuildingType::SandPile
                    | BuildingType::BonePile
                    | BuildingType::OutdoorLamp
            ) =>
        {
            Some(kind)
        }
        UiIntent::SelectTaskMode(TaskMode::SoulSpaPlace(_)) => Some(BuildingType::SoulSpa),
        _ => None,
    }
}

pub(super) fn visible_rect(world: &World, entity: Entity, viewport: Vec2) -> Option<[f32; 4]> {
    let computed = world.get::<ComputedNode>(entity)?;
    let transform = world.get::<UiGlobalTransform>(entity)?;
    if !viewport.is_finite()
        || viewport.min_element() <= 0.0
        || !computed.size().is_finite()
        || computed.size().min_element() <= 0.0
        || !transform.translation.is_finite()
        || transform.matrix2 != bevy::math::Mat2::IDENTITY
        || !world.get::<InheritedVisibility>(entity)?.get()
    {
        return None;
    }
    let half = computed.size() * 0.5;
    let mut min = (transform.translation - half).max(Vec2::ZERO);
    let mut max = (transform.translation + half).min(viewport);
    if let Some(clip) = world.get::<bevy::ui::CalculatedClip>(entity) {
        // Bevy 0.19 ComputedNode::resolve_clip_rect uses -/+ infinity for
        // OverflowAxis::Visible (including the x axis of scroll_y()).
        // Accept only those outward unbounded endpoints, then intersect the
        // finite node/viewport. NaN, inward infinity and inverted clips fail.
        let lower = clip.clip.min.to_array();
        let upper = clip.clip.max.to_array();
        if lower
            .iter()
            .any(|v| !v.is_finite() && *v != f32::NEG_INFINITY)
            || upper.iter().any(|v| !v.is_finite() && *v != f32::INFINITY)
            || !clip.clip.min.cmple(clip.clip.max).all()
        {
            return None;
        }
        min = min.max(clip.clip.min);
        max = max.min(clip.clip.max);
    }
    // Bound malformed hierarchy traversal rather than assuming a hidden parent
    // has already cleared the child's previous computed size.
    let mut ancestor = Some(entity);
    for _ in 0..64 {
        let Some(current) = ancestor else {
            return ((max - min).min_element() > 1.0).then_some([min.x, min.y, max.x, max.y]);
        };
        if world.get::<Node>(current)?.display == Display::None {
            return None;
        }
        ancestor = world.get::<ChildOf>(current).map(ChildOf::parent);
    }
    None
}

pub(super) fn architect_cards(world: &mut World) -> Result<Vec<Value>, String> {
    if world.get_resource::<MenuState>() != Some(&MenuState::Architect) {
        return Ok(Vec::new());
    }
    let mut windows = world.query_filtered::<&Window, With<bevy::window::PrimaryWindow>>();
    let Ok(window) = windows.single(world) else {
        return Ok(Vec::new());
    };
    let viewport = window.physical_size().as_vec2();
    let mut query = world.query_filtered::<(Entity, &MenuButton), With<ArchitectBuildingPanel>>();
    let cards: Vec<_> = query.iter(world).take(33).map(|(e, b)| (e, b.0)).collect();
    if cards.len() > 32 {
        return Err("production Architect card cap exceeded".into());
    }
    let mut rows = Vec::new();
    let mut kinds = std::collections::BTreeSet::new();
    for (entity, action) in cards {
        let Some(kind) = equipment(action) else {
            continue;
        };
        let Some(rect) = visible_rect(world, entity, viewport) else {
            continue;
        };
        let kind = format!("{kind:?}");
        if !kinds.insert(kind.clone()) {
            return Err("duplicate visible production Architect card".into());
        }
        rows.push(json!({"entity": format!("{entity:?}"), "kind": kind,
            "physical_rect": rect, "physical_viewport": viewport.to_array()}));
    }
    rows.sort_by(|a, b| a["kind"].as_str().cmp(&b["kind"].as_str()));
    Ok(rows)
}

pub(super) fn save_catalog_context(world: &mut World) -> Result<Value, String> {
    use crate::systems::save::catalog::SaveStorageRoot;
    use crate::systems::save::{SaveCatalog, SaveCatalogMode, SaveCatalogUi};
    use hw_core::SaveSlotId;
    let Some(ui) = world.get_resource::<SaveCatalogUi>() else {
        return Ok(json!({"available": false}));
    };
    let mode = match ui.mode {
        SaveCatalogMode::Closed => "Closed",
        SaveCatalogMode::SaveCatalog => "SaveCatalog",
        SaveCatalogMode::LoadCatalog => "LoadCatalog",
        SaveCatalogMode::LoadConfirm {
            slot: SaveSlotId::Manual1,
            recovery: false,
        } => "LoadConfirmManual1",
        _ => "Unsupported",
    };
    let session = ui.session;
    let storage_root = world
        .get_resource::<SaveStorageRoot>()
        .and_then(|root| root.0.to_str())
        .map(str::to_owned);
    let manual1 = world.get_resource::<SaveCatalog>().and_then(|catalog| catalog.entry(SaveSlotId::Manual1)).map(|entry| {
        json!({"exists": entry.revision.exists, "length": entry.revision.length,
            "can_save": entry.capabilities.can_manual_save, "can_load": entry.capabilities.can_load})
    });
    let mut rows = Vec::new();
    if mode != "Closed" {
        let mut windows =
            world.query_filtered::<(Entity, &Window), With<bevy::window::PrimaryWindow>>();
        let Ok((primary, window)) = windows.single(world) else {
            return Err("production save catalog primary window missing or ambiguous".into());
        };
        let viewport = window.physical_size().as_vec2();
        let mut query = world.query::<(Entity, &MenuButton)>();
        let buttons: Vec<_> = query.iter(world).take(513).map(|(e, b)| (e, b.0)).collect();
        if buttons.len() > 512 {
            return Err("production save catalog button cap exceeded".into());
        }
        for (entity, action) in buttons {
            let (action, button_session) = match action {
                UiIntent::SelectSaveCatalogSlot {
                    slot: SaveSlotId::Manual1,
                    session,
                } => ("Save", session),
                UiIntent::SelectLoadCatalogSlot {
                    slot: SaveSlotId::Manual1,
                    session,
                } => ("Load", session),
                UiIntent::ConfirmLoadCatalogSlot {
                    slot: SaveSlotId::Manual1,
                    session,
                } => ("ConfirmLoad", session),
                _ => continue,
            };
            let expected_action = match mode {
                "SaveCatalog" => "Save",
                "LoadCatalog" => "Load",
                "LoadConfirmManual1" => "ConfirmLoad",
                _ => continue,
            };
            if action != expected_action
                || button_session != session
                || !enabled_hierarchy(world, entity)
            {
                continue;
            }
            let Some(rect) = visible_rect(world, entity, viewport) else {
                continue;
            };
            let Some(camera_entity) = world
                .get::<ComputedUiTargetCamera>(entity)
                .and_then(ComputedUiTargetCamera::get)
            else {
                continue;
            };
            let Some(camera) = world.get::<Camera>(camera_entity) else {
                continue;
            };
            let target = world.get::<bevy::camera::RenderTarget>(camera_entity);
            let primary_target = match target {
                Some(bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Primary)) => true,
                Some(bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Entity(
                    target,
                ))) => *target == primary,
                _ => false,
            };
            if !camera.is_active
                || camera.viewport.is_some()
                || !primary_target
                || camera.physical_viewport_size() != Some(viewport.as_uvec2())
            {
                continue;
            }
            rows.push(json!({"action": action, "session": button_session, "slot": "Manual1",
                "entity": format!("{entity:?}"), "camera": format!("{camera_entity:?}"),
                "window": format!("{primary:?}"), "physical_rect": rect, "physical_viewport": viewport.to_array()}));
        }
    }
    if rows.len() > 3
        || rows.iter().enumerate().any(|(i, row)| {
            rows[..i]
                .iter()
                .any(|other| other["action"] == row["action"])
        })
    {
        return Err("duplicate production save catalog control".into());
    }
    Ok(json!({"available": true, "mode": mode, "session": session,
        "storage_root": storage_root, "manual1": manual1, "controls": rows}))
}

fn enabled_hierarchy(world: &World, entity: Entity) -> bool {
    let mut ancestor = Some(entity);
    for _ in 0..64 {
        let Some(current) = ancestor else {
            return true;
        };
        if world
            .get::<bevy::ui::InteractionDisabled>(current)
            .is_some()
        {
            return false;
        }
        ancestor = world.get::<ChildOf>(current).map(ChildOf::parent);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_jobs::BuildingCategory;

    fn save_app() -> (App, Entity, Entity) {
        use bevy::app::{HierarchyPropagatePlugin, PropagateSet};
        use bevy::camera::{ComputedCameraValues, RenderTargetInfo};
        let mut app = App::new();
        app.init_resource::<UiScale>();
        app.add_plugins(HierarchyPropagatePlugin::<ComputedUiTargetCamera>::new(
            PostUpdate,
        ));
        app.configure_sets(
            PostUpdate,
            PropagateSet::<ComputedUiTargetCamera>::default(),
        );
        app.add_plugins(HierarchyPropagatePlugin::<
            bevy::ui::ComputedUiRenderTargetInfo,
        >::new(PostUpdate));
        app.configure_sets(
            PostUpdate,
            PropagateSet::<bevy::ui::ComputedUiRenderTargetInfo>::default(),
        );
        app.add_systems(Update, bevy::ui::update::propagate_ui_target_cameras);
        let world = app.world_mut();
        world.insert_resource(crate::systems::save::SaveCatalogUi {
            mode: crate::systems::save::SaveCatalogMode::SaveCatalog,
            session: 7,
            selected: None,
        });
        world.init_resource::<crate::systems::save::SaveStorageRoot>();
        world.spawn((
            Window {
                resolution: (1280, 720).into(),
                ..default()
            },
            bevy::window::PrimaryWindow,
        ));
        let camera = world
            .spawn((
                Camera2d,
                Camera {
                    computed: ComputedCameraValues {
                        target_info: Some(RenderTargetInfo {
                            physical_size: UVec2::new(1280, 720),
                            scale_factor: 1.0,
                        }),
                        ..default()
                    },
                    ..default()
                },
            ))
            .id();
        let button = card(world, BuildingType::Tank);
        world.entity_mut(button).insert((
            UiTargetCamera(camera),
            MenuButton(UiIntent::SelectSaveCatalogSlot {
                slot: hw_core::SaveSlotId::Manual1,
                session: 7,
            }),
        ));
        app.update();
        (app, camera, button)
    }

    #[test]
    fn save_controls_bind_actual_camera_session_and_do_not_mutate_ui() {
        let (mut app, camera, button) = save_app();
        let before = app
            .world()
            .resource::<crate::systems::save::SaveCatalogUi>()
            .clone();
        let row = save_catalog_context(app.world_mut()).unwrap();
        assert_eq!(row["storage_root"], "saves");
        assert_eq!(row["controls"][0]["action"], "Save");
        assert_eq!(row["controls"][0]["camera"], format!("{camera:?}"));
        assert_eq!(row["controls"][0]["entity"], format!("{button:?}"));
        assert_eq!(
            *app.world()
                .resource::<crate::systems::save::SaveCatalogUi>(),
            before
        );
        app.world_mut()
            .resource_mut::<crate::systems::save::SaveCatalogUi>()
            .session = 8;
        assert_eq!(
            save_catalog_context(app.world_mut()).unwrap()["controls"],
            json!([])
        );
    }

    #[test]
    fn save_controls_reject_disabled_ancestors_foreign_or_inactive_cameras() {
        let (mut app, camera, button) = save_app();
        let parent = app
            .world_mut()
            .spawn((Node::default(), bevy::ui::InteractionDisabled))
            .id();
        app.world_mut().entity_mut(button).insert(ChildOf(parent));
        assert_eq!(
            save_catalog_context(app.world_mut()).unwrap()["controls"],
            json!([])
        );
        app.world_mut()
            .entity_mut(parent)
            .remove::<bevy::ui::InteractionDisabled>();
        app.world_mut().get_mut::<Camera>(camera).unwrap().is_active = false;
        assert_eq!(
            save_catalog_context(app.world_mut()).unwrap()["controls"],
            json!([])
        );
        app.world_mut().get_mut::<Camera>(camera).unwrap().is_active = true;
        let other_window = app.world_mut().spawn(Window::default()).id();
        app.world_mut()
            .entity_mut(camera)
            .insert(bevy::camera::RenderTarget::Window(
                bevy::window::WindowRef::Entity(other_window),
            ));
        assert_eq!(
            save_catalog_context(app.world_mut()).unwrap()["controls"],
            json!([])
        );
    }

    #[test]
    fn save_controls_fail_closed_for_missing_resources_camera_and_excess_buttons() {
        assert_eq!(
            save_catalog_context(&mut World::new()).unwrap(),
            json!({"available": false})
        );
        let (mut app, _, button) = save_app();
        app.world_mut()
            .entity_mut(button)
            .remove::<ComputedUiTargetCamera>();
        assert_eq!(
            save_catalog_context(app.world_mut()).unwrap()["controls"],
            json!([])
        );
        for _ in 0..512 {
            app.world_mut()
                .spawn(MenuButton(UiIntent::SelectBuild(BuildingType::Bridge)));
        }
        assert!(
            save_catalog_context(app.world_mut())
                .unwrap_err()
                .contains("cap")
        );
    }

    fn card(world: &mut World, kind: BuildingType) -> Entity {
        world
            .spawn((
                Node::default(),
                MenuButton(UiIntent::SelectBuild(kind)),
                ArchitectBuildingPanel(BuildingCategory::Plant),
                InheritedVisibility::VISIBLE,
                ComputedNode {
                    size: Vec2::new(140.0, 56.0),
                    ..default()
                },
                UiGlobalTransform::from_translation(Vec2::new(100.0, 100.0)),
            ))
            .id()
    }

    fn world() -> World {
        let mut world = World::new();
        world.insert_resource(MenuState::Architect);
        world.spawn((
            Window {
                resolution: (1280, 720).into(),
                ..default()
            },
            bevy::window::PrimaryWindow,
        ));
        world
    }

    #[test]
    fn only_non_bridge_equipment_actions_are_observed() {
        assert_eq!(equipment(UiIntent::SelectBuild(BuildingType::Bridge)), None);
        assert_eq!(equipment(UiIntent::SelectFloorPlace), None);
        assert_eq!(
            equipment(UiIntent::SelectTaskMode(TaskMode::SoulSpaPlace(None))),
            Some(BuildingType::SoulSpa)
        );
    }

    #[test]
    fn physical_rect_is_read_only_and_hidden_parents_are_rejected() {
        let mut world = world();
        let entity = card(&mut world, BuildingType::Tank);
        let rows = architect_cards(&mut world).unwrap();
        assert_eq!(rows[0]["physical_rect"], json!([30.0, 72.0, 170.0, 128.0]));
        assert_eq!(
            world.get::<MenuButton>(entity).unwrap().0,
            UiIntent::SelectBuild(BuildingType::Tank)
        );
        let parent = world
            .spawn(Node {
                display: Display::None,
                ..default()
            })
            .id();
        world.entity_mut(entity).insert(ChildOf(parent));
        assert!(architect_cards(&mut world).unwrap().is_empty());
    }

    #[test]
    fn renderer_clip_and_missing_layout_visibility_fail_closed() {
        let mut world = world();
        let entity = card(&mut world, BuildingType::Tank);
        world.entity_mut(entity).insert(bevy::ui::CalculatedClip {
            clip: Rect::from_corners(Vec2::new(50.0, 80.0), Vec2::new(150.0, 120.0)),
        });
        assert_eq!(
            architect_cards(&mut world).unwrap()[0]["physical_rect"],
            json!([50.0, 80.0, 150.0, 120.0])
        );
        world.entity_mut(entity).remove::<InheritedVisibility>();
        assert!(architect_cards(&mut world).unwrap().is_empty());
    }

    #[test]
    fn duplicate_cards_and_excess_nodes_are_rejected() {
        let mut world = world();
        card(&mut world, BuildingType::Tank);
        card(&mut world, BuildingType::Tank);
        assert!(architect_cards(&mut world).is_err());
        for _ in 0..31 {
            card(&mut world, BuildingType::Bridge);
        }
        assert!(architect_cards(&mut world).unwrap_err().contains("cap"));
    }

    #[test]
    fn hidden_menu_never_exposes_stale_card_rectangles() {
        let mut world = world();
        card(&mut world, BuildingType::Tank);
        world.insert_resource(MenuState::Hidden);
        assert!(architect_cards(&mut world).unwrap().is_empty());
    }

    #[test]
    fn transformed_or_nonfinite_cards_are_not_click_rectangles() {
        let mut world = world();
        let entity = card(&mut world, BuildingType::Tank);
        for transform in [
            UiGlobalTransform::from_scale(Vec2::new(2.0, 1.0)),
            UiGlobalTransform::from_rotation(Rot2::radians(0.5)),
            UiGlobalTransform::from_scale(Vec2::new(f32::NAN, 1.0)),
            UiGlobalTransform::from_translation(Vec2::new(f32::INFINITY, 100.0)),
        ] {
            world.entity_mut(entity).insert(transform);
            assert!(architect_cards(&mut world).unwrap().is_empty());
        }
    }

    #[test]
    fn bevy_scroll_y_unbounded_x_clip_becomes_a_finite_physical_rect() {
        let mut world = world();
        let entity = card(&mut world, BuildingType::Tank);
        let computed = ComputedNode {
            size: Vec2::new(100.0, 50.0),
            inverse_scale_factor: 1.0,
            ..default()
        };
        let mut clip = computed.resolve_clip_rect(Overflow::scroll_y(), default());
        clip.min += Vec2::splat(100.0);
        clip.max += Vec2::splat(100.0);
        assert_eq!(clip.min.x, f32::NEG_INFINITY);
        assert_eq!(clip.max.x, f32::INFINITY);
        world
            .entity_mut(entity)
            .insert(bevy::ui::CalculatedClip { clip });
        assert_eq!(
            architect_cards(&mut world).unwrap()[0]["physical_rect"],
            json!([30.0, 75.0, 170.0, 125.0])
        );
    }

    #[test]
    fn nan_inward_infinity_and_inverted_clips_remain_rejected() {
        let mut world = world();
        let entity = card(&mut world, BuildingType::Tank);
        for (min, max) in [
            (Vec2::new(f32::NAN, 75.0), Vec2::new(170.0, 125.0)),
            (
                Vec2::new(f32::INFINITY, 75.0),
                Vec2::new(f32::INFINITY, 125.0),
            ),
            (
                Vec2::new(f32::NEG_INFINITY, 75.0),
                Vec2::new(f32::NEG_INFINITY, 125.0),
            ),
            (Vec2::new(30.0, 140.0), Vec2::new(170.0, 120.0)),
        ] {
            world.entity_mut(entity).insert(bevy::ui::CalculatedClip {
                clip: Rect { min, max },
            });
            assert!(architect_cards(&mut world).unwrap().is_empty());
        }
    }
}
