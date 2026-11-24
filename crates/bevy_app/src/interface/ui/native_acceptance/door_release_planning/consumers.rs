//! Read-only actual normal Door preview consumers and placement state.
use super::*;
use crate::assets::door_asset_set::{
    DoorAssetAuthority, DoorAssetReadiness, DoorAssetReadinessState, ProductionDoorAssetPool,
};
use crate::systems::visual::placement_ghost::PlacementGhost;
use bevy::sprite::Anchor;
use hw_ui::components::BuildingCatalogPreview;
use hw_visual::blueprint::{BlueprintPulseOverlayChild, BlueprintVisual};
use hw_visual::visual3d::resolve_door_presentation_axis;
use hw_visual::wall_connection::{WallConnectionMask, WallTopologyIndex};

fn sprite(world: &World, entity: Entity, expected: &Handle<Image>) -> Result<Value, String> {
    let sprite = world
        .get::<Sprite>(entity)
        .ok_or("Door consumer Sprite missing")?;
    let pose = world
        .get::<Transform>(entity)
        .ok_or("Door consumer Transform missing")?;
    let global = world
        .get::<GlobalTransform>(entity)
        .ok_or("Door consumer GlobalTransform missing")?;
    let anchor = world
        .get::<Anchor>(entity)
        .ok_or("Door consumer Anchor missing")?;
    let visible = world
        .get::<ViewVisibility>(entity)
        .is_some_and(|visibility| visibility.get());
    let image = world.resource::<Assets<Image>>().get(&sprite.image);
    Ok(
        json!({"entity":entity.to_bits(),"image":format!("{:?}",sprite.image.id()),
        "expected_image":format!("{:?}",expected.id()),"matches":sprite.image==*expected,
        "image_size":image.map(|image|image.size().to_array()),"size":sprite.custom_size.map(|size|size.to_array()),
        "anchor":anchor.0.to_array(),"color":sprite.color.to_srgba().to_f32_array(),
        "visible":visible,"visibility":world.get::<Visibility>(entity).map(|v|format!("{v:?}")),
        "inherited_visible":world.get::<InheritedVisibility>(entity).is_some_and(|visibility| visibility.get()),
        "layers":world.get::<bevy::camera::visibility::RenderLayers>(entity).cloned().unwrap_or_default().iter().collect::<Vec<_>>(),
        "parent":world.get::<ChildOf>(entity).map(|parent|parent.parent().to_bits()),
        "position":pose.translation.to_array(),"rotation":pose.rotation.to_array(),
        "scale":pose.scale.to_array(),"global_matrix":global.to_matrix().to_cols_array(),
        "flip_x":sprite.flip_x,"flip_y":sprite.flip_y,"rect":sprite.rect.map(|rect|[rect.min.x,rect.min.y,rect.max.x,rect.max.y])}),
    )
}

pub(super) fn snapshot(world: &mut World) -> Result<Value, String> {
    let Some(target) = world.get_resource::<preparation::Target>().cloned() else {
        return Ok(Value::Null);
    };
    if current_case() != Some(target.case) {
        return Err("Door context case changed".into());
    }
    let resolved = world.resource::<ProductionDoorAssetPool>().resolved.clone();
    let Some(resolved) = resolved else {
        return Ok(json!({"scope":SCOPE,"ready":false,"failure":null}));
    };
    if resolved.identity.authority != DoorAssetAuthority::ReleaseApproved
        || resolved.identity.asset_set_generation != 7
        || resolved.identity.manifest_sha256
            != "4e3f9236db067772b9a60b37ea98437cb29e481f9ec9cbb4d6dcf6517374a449"
    {
        return Err("Door context requires exact normal released Door g7".into());
    }
    let eligible = matches!(&world.resource::<DoorAssetReadiness>().state,
        DoorAssetReadinessState::Eligible(identity) if *identity==resolved.identity);
    if !eligible {
        return Ok(json!({"scope":SCOPE,"ready":false,"failure":null}));
    }
    let cameras = world
        .query_filtered::<(Entity, &Camera, &GlobalTransform), With<hw_ui::camera::MainCamera>>()
        .iter(world)
        .map(|(id, camera, global)| (id, camera.clone(), *global))
        .collect::<Vec<_>>();
    let [(camera_id, camera, global)] = cameras.as_slice() else {
        return Err("Door requires one main camera".into());
    };
    let windows = world
        .query_filtered::<(Entity, &Window), With<bevy::window::PrimaryWindow>>()
        .iter(world)
        .map(|(id, window)| (id, window.clone()))
        .collect::<Vec<_>>();
    let [(window_id, window)] = windows.as_slice() else {
        return Err("Door requires one primary window".into());
    };
    if !camera.is_active
        || camera.viewport.is_some()
        || camera.physical_viewport_size() != Some(window.physical_size())
        || world
            .get::<bevy::camera::visibility::RenderLayers>(*camera_id)
            .cloned()
            .unwrap_or_default()
            != bevy::camera::visibility::RenderLayers::layer(0)
        || !matches!(
            world.get::<bevy::camera::RenderTarget>(*camera_id),
            Some(bevy::camera::RenderTarget::Window(
                bevy::window::WindowRef::Primary
            ))
        )
    {
        return Err("Door camera is not the complete primary client".into());
    }
    let point = WorldMap::grid_to_world(target.anchor.0, target.anchor.1);
    let projected = camera
        .world_to_viewport(global, point.extend(0.))
        .map_err(|error| error.to_string())?;
    let cursor = window
        .cursor_position()
        .and_then(|point| camera.viewport_to_world_2d(global, point).ok())
        .map(WorldMap::world_to_grid);
    let map = world.resource::<WorldMap>();
    let supports=target.supports.iter().map(|(grid,owner)| {
        let kind=world.get::<Building>(*owner).map(|building|format!("{:?}",building.kind));
        json!({"grid":grid,"owner":owner.to_bits(),"actual_owner":map.building_entity(*grid).map(Entity::to_bits),
            "kind":kind,"provisional":world.get::<Building>(*owner).map(|building|building.is_provisional)})
    }).collect::<Vec<_>>();
    let anchor_owner = map.building_entity(target.anchor).map(Entity::to_bits);
    let walkable = map.is_walkable(target.anchor.0, target.anchor.1);
    let topology = world.resource::<WallTopologyIndex>();
    let (x, y) = target.anchor;
    let axis = resolve_door_presentation_axis(WallConnectionMask::from_neighbors(
        topology.connection_mask((x, y + 1)).is_some(),
        topology.connection_mask((x, y - 1)).is_some(),
        topology.connection_mask((x - 1, y)).is_some(),
        topology.connection_mask((x + 1, y)).is_some(),
    ));
    let image = if format!("{axis:?}") == "NorthSouth" {
        &resolved.preview_ns
    } else {
        &resolved.preview_ew
    };
    let ghosts = world
        .query_filtered::<Entity, With<PlacementGhost>>()
        .iter(world)
        .collect::<Vec<_>>();
    let ghosts = ghosts
        .into_iter()
        .map(|id| sprite(world, id, image))
        .collect::<Result<Vec<_>, _>>()?;
    let ids = world
        .query::<(Entity, &Blueprint)>()
        .iter(world)
        .filter(|(_, bp)| bp.kind == BuildingType::Door)
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    let mut blueprints = Vec::new();
    for id in ids {
        let bp = world
            .get::<Blueprint>(id)
            .ok_or("Door Blueprint disappeared")?;
        let preview = sprite(world, id, image)?;
        let pulse = world
            .get::<BlueprintVisual>(id)
            .and_then(|visual| visual.pulse_overlay)
            .map(|overlay| overlay.entity);
        let pulse = if let Some(pulse) = pulse {
            if world.get::<BlueprintPulseOverlayChild>(pulse).is_none() {
                return Err("Door pulse owner marker missing".into());
            }
            Some(sprite(world, pulse, image)?)
        } else {
            None
        };
        blueprints.push(json!({"entity":id.to_bits(),"debug_entity":format!("{id:?}"),
            "footprint":bp.occupied_grids,"progress":bp.progress,"materials_complete":bp.materials_complete(),
            "sprite":preview,"pulse":pulse}));
    }
    blueprints.sort_by_key(|row| row["entity"].as_u64());
    let ids = world
        .query::<(Entity, &BuildingCatalogPreview)>()
        .iter(world)
        .filter(|(_, marker)| marker.0 == BuildingType::Door)
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    let mut catalog = Vec::new();
    for id in ids {
        let Some(node) = world.get::<ImageNode>(id) else {
            return Err("Door catalog marker has no image".into());
        };
        let rect = visible_rect(world, id, window.physical_size().as_vec2());
        let raw = world
            .get::<ComputedNode>(id)
            .zip(world.get::<UiGlobalTransform>(id))
            .map(|(node, transform)| {
                let lo = transform.translation - node.size() * 0.5;
                let hi = transform.translation + node.size() * 0.5;
                [lo.x, lo.y, hi.x, hi.y]
            });
        catalog.push(json!({"entity":id.to_bits(),"image":format!("{:?}",node.image.id()),
            "expected_image":format!("{:?}",resolved.preview_ew.id()),"matches":node.image==resolved.preview_ew,
            "image_size":world.resource::<Assets<Image>>().get(&node.image).map(|image|image.size().to_array()),
            "rect":rect,"unclipped_rect":raw,
            "primary_camera":primary_ui_camera(world,id,Some(window_id.to_bits()),window.physical_size().as_vec2())}));
    }
    let inventory = preparation::inventory(world)?;
    Ok(json!({"scope":SCOPE,"ready":true,"failure":null,
        "fixture":preparation::initial_snapshot(&target),"supports":supports,
        "identity":{"asset_set_generation":resolved.identity.asset_set_generation,"authority":resolved.identity.authority,
            "manifest_sha256":resolved.identity.manifest_sha256},
        "anchor":target.anchor,"point":projected.to_array(),"cursor_grid":cursor,"actual_axis":format!("{axis:?}"),
        "anchor_owner":anchor_owner,"walkable":walkable,"catalog":catalog,"ghosts":ghosts,"blueprints":blueprints,
        "building_inventory":inventory,"primary_window":window_id.to_bits(),"world_epoch":world.resource::<WorldEpoch>().get(),
        "play_mode":format!("{:?}",world.resource::<State<hw_core::game_state::PlayMode>>().get()),
        "build_kind":world.resource::<crate::app_contexts::BuildContext>().kind.map(|kind|format!("{kind:?}")),
        "real_seconds":world.resource::<Time<Real>>().elapsed_secs_f64(),"input":input::snapshot(world),
        "promotion_authority":false,"performance_evidence":false,"natural_construction_evidence":false}))
}
