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

fn visible_rect(world: &World, entity: Entity, viewport: Vec2) -> Option<[f32; 4]> {
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
        if !clip.clip.min.is_finite() || !clip.clip.max.is_finite() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use hw_jobs::BuildingCategory;

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
}
