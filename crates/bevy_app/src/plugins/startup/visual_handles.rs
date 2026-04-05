//! GameAssets から hw_visual のハンドルリソースを初期化するシステム

use crate::assets::GameAssets;
use bevy::camera::visibility::RenderLayers;
use bevy::ecs::system::SystemParam;
use bevy::mesh::Mesh;
use bevy::prelude::*;
use hw_core::constants::{TILE_SIZE, building_3d_render_layers};
use hw_core::visual::SoulTaskHandles;
use hw_logistics::ResourceItemVisualHandles;
use hw_visual::{
    BuildingAnimHandles, GatheringVisualHandles, HaulItemHandles, MaterialIconHandles,
    PlantTreeHandles, SectionMaterial, SoulMaskMaterial, SoulShadowMaterial, SpeechHandles,
    TerrainSurfaceMaterial, TerrainSurfaceMaterialExt, TerrainSurfaceUniform, WallVisualHandles,
    WorkIconHandles, make_section_material, make_terrain_surface_material, with_alpha_mode,
};
use hw_visual::{CharacterMaterial, soul_face_uv_offset, soul_face_uv_scale};
use hw_world::{DoorVisualHandles, TerrainVisualHandles};

use crate::world::map::{TerrainFeatureMap, TerrainIdMap};

/// 3D レンダリング用メッシュ・マテリアルハンドルリソース
///
/// Phase 2 プレースホルダープリミティブ（Cuboid/Plane3d）を保持する。
/// Phase 3 で GLB に置き換え予定。
#[derive(Resource)]
pub struct Building3dHandles {
    // --- 壁 ---
    pub wall_mesh: Handle<Mesh>,
    pub wall_material: Handle<SectionMaterial>,
    pub wall_provisional_material: Handle<SectionMaterial>,
    pub wall_orientation_aid_mesh: Handle<Mesh>,
    pub wall_orientation_aid_material: Handle<StandardMaterial>,
    // --- 床 ---
    pub floor_mesh: Handle<Mesh>,
    pub floor_material: Handle<StandardMaterial>,
    // --- ドア ---
    pub door_mesh: Handle<Mesh>,
    pub door_material: Handle<StandardMaterial>,
    // --- 設備 (Tank / MudMixer / RestArea / WheelbarrowParking / SandPile / BonePile) ---
    pub equipment_1x1_mesh: Handle<Mesh>,
    pub equipment_2x2_mesh: Handle<Mesh>,
    pub equipment_material: Handle<StandardMaterial>,
    // --- キャラクター ---
    pub soul_scene: Handle<Scene>,
    pub familiar_mesh: Handle<Mesh>,
    pub familiar_material: Handle<StandardMaterial>,
    /// 全3Dエンティティに付与する RenderLayers
    pub render_layers: RenderLayers,
}

/// 地形タイル 3D レンダリング用メッシュ・マテリアルハンドルリソース
///
/// 全タイルで共有する `Plane3d` メッシュ 1 つと、共有 `TerrainSurfaceMaterial` を保持する。
#[derive(Resource)]
pub struct Terrain3dHandles {
    pub tile_mesh: Handle<Mesh>,
    pub surface: Handle<TerrainSurfaceMaterial>,
}

#[derive(Resource)]
pub struct CharacterHandles {
    pub soul_body_material: Handle<CharacterMaterial>,
    pub soul_face_material: Handle<CharacterMaterial>,
    pub soul_mask_material: Handle<SoulMaskMaterial>,
    pub soul_shadow_proxy_material: Handle<SoulShadowMaterial>,
}

#[derive(SystemParam)]
pub struct InitVisualHandlesParams<'w, 's> {
    commands: Commands<'w, 's>,
    game_assets: Res<'w, GameAssets>,
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
    section_materials: ResMut<'w, Assets<SectionMaterial>>,
    terrain_surface_materials: ResMut<'w, Assets<TerrainSurfaceMaterial>>,
    character_materials: ResMut<'w, Assets<CharacterMaterial>>,
    soul_mask_materials: ResMut<'w, Assets<SoulMaskMaterial>>,
    soul_shadow_materials: ResMut<'w, Assets<SoulShadowMaterial>>,
    terrain_feature_map: Res<'w, TerrainFeatureMap>,
    terrain_id_map: Res<'w, TerrainIdMap>,
}

pub fn init_visual_handles(mut params: InitVisualHandlesParams) {
    let game_assets = params.game_assets.as_ref();
    let commands = &mut params.commands;
    let meshes = &mut params.meshes;
    let materials = &mut params.materials;
    let section_materials = &mut params.section_materials;
    let terrain_surface_materials = &mut params.terrain_surface_materials;
    let character_materials = &mut params.character_materials;
    let soul_mask_materials = &mut params.soul_mask_materials;
    let soul_shadow_materials = &mut params.soul_shadow_materials;
    let feature_map_handle = params.terrain_feature_map.image.clone();
    let terrain_id_map_handle = params.terrain_id_map.image.clone();
    commands.insert_resource(WallVisualHandles {
        stone_isolated: game_assets.wall_isolated.clone(),
        stone_horizontal_left: game_assets.wall_horizontal_left.clone(),
        stone_horizontal_right: game_assets.wall_horizontal_right.clone(),
        stone_horizontal_both: game_assets.wall_horizontal_both.clone(),
        stone_vertical_top: game_assets.wall_vertical_top.clone(),
        stone_vertical_bottom: game_assets.wall_vertical_bottom.clone(),
        stone_vertical_both: game_assets.wall_vertical_both.clone(),
        stone_corner_tl: game_assets.wall_corner_top_left.clone(),
        stone_corner_tr: game_assets.wall_corner_top_right.clone(),
        stone_corner_bl: game_assets.wall_corner_bottom_left.clone(),
        stone_corner_br: game_assets.wall_corner_bottom_right.clone(),
        stone_t_up: game_assets.wall_t_up.clone(),
        stone_t_down: game_assets.wall_t_down.clone(),
        stone_t_left: game_assets.wall_t_left.clone(),
        stone_t_right: game_assets.wall_t_right.clone(),
        stone_cross: game_assets.wall_cross.clone(),
        door_closed: game_assets.door_closed.clone(),
        door_open: game_assets.door_open.clone(),
        mud_isolated: game_assets.mud_wall_isolated.clone(),
        mud_horizontal: game_assets.mud_wall_horizontal.clone(),
        mud_vertical: game_assets.mud_wall_vertical.clone(),
        mud_corner_tl: game_assets.mud_wall_corner_top_left.clone(),
        mud_corner_tr: game_assets.mud_wall_corner_top_right.clone(),
        mud_corner_bl: game_assets.mud_wall_corner_bottom_left.clone(),
        mud_corner_br: game_assets.mud_wall_corner_bottom_right.clone(),
        mud_t_up: game_assets.mud_wall_t_up.clone(),
        mud_t_down: game_assets.mud_wall_t_down.clone(),
        mud_t_left: game_assets.mud_wall_t_left.clone(),
        mud_t_right: game_assets.mud_wall_t_right.clone(),
        mud_cross: game_assets.mud_wall_cross.clone(),
        mud_end_top: game_assets.mud_wall_end_top.clone(),
        mud_end_bottom: game_assets.mud_wall_end_bottom.clone(),
        mud_end_left: game_assets.mud_wall_end_left.clone(),
        mud_end_right: game_assets.mud_wall_end_right.clone(),
        mud_floor: game_assets.mud_floor.clone(),
    });

    commands.insert_resource(BuildingAnimHandles {
        mud_mixer_idle: game_assets.mud_mixer.clone(),
        mud_mixer_anim_1: game_assets.mud_mixer_anim_1.clone(),
        mud_mixer_anim_2: game_assets.mud_mixer_anim_2.clone(),
        mud_mixer_anim_3: game_assets.mud_mixer_anim_3.clone(),
        mud_mixer_anim_4: game_assets.mud_mixer_anim_4.clone(),
        tank_empty: game_assets.tank_empty.clone(),
        tank_partial: game_assets.tank_partial.clone(),
        tank_full: game_assets.tank_full.clone(),
    });

    commands.insert_resource(WorkIconHandles {
        hammer: game_assets.icon_hammer.clone(),
        pick: game_assets.icon_pick.clone(),
        axe: game_assets.icon_axe.clone(),
        haul: game_assets.icon_haul.clone(),
        wheelbarrow_small: game_assets.icon_wheelbarrow_small.clone(),
    });

    commands.insert_resource(MaterialIconHandles {
        wood_small: game_assets.icon_wood_small.clone(),
        rock_small: game_assets.icon_rock_small.clone(),
        sand_small: game_assets.icon_sand_small.clone(),
        bone_small: game_assets.icon_bone_small.clone(),
        stasis_mud_small: game_assets.icon_stasis_mud_small.clone(),
        water_small: game_assets.icon_water_small.clone(),
        font_ui: game_assets.font_ui.clone(),
    });

    commands.insert_resource(HaulItemHandles {
        wheelbarrow_empty: game_assets.wheelbarrow_empty.clone(),
        wheelbarrow_loaded: game_assets.wheelbarrow_loaded.clone(),
        wheelbarrow_parking: game_assets.wheelbarrow_parking.clone(),
        bucket_empty: game_assets.bucket_empty.clone(),
        bucket_water: game_assets.bucket_water.clone(),
        sand_pile: game_assets.sand_pile.clone(),
        stasis_mud: game_assets.stasis_mud.clone(),
    });

    commands.insert_resource(SpeechHandles {
        bubble_9slice: game_assets.bubble_9slice.clone(),
        glow_circle: game_assets.glow_circle.clone(),
        font_familiar: game_assets.font_familiar.clone(),
        font_soul_name: game_assets.font_soul_name.clone(),
        font_soul_emoji: game_assets.font_soul_emoji.clone(),
    });

    commands.insert_resource(PlantTreeHandles {
        magic_circle: game_assets.plant_tree_magic_circle.clone(),
        life_spark: game_assets.plant_tree_life_spark.clone(),
    });

    commands.insert_resource(GatheringVisualHandles {
        aura_circle: game_assets.aura_circle.clone(),
        card_table: game_assets.gathering_card_table.clone(),
        campfire: game_assets.gathering_campfire.clone(),
        barrel: game_assets.gathering_barrel.clone(),
    });

    commands.insert_resource(SoulTaskHandles {
        wood: game_assets.wood.clone(),
        tree_animes: game_assets.tree_animes.clone(),
        rock: game_assets.rock.clone(),
        icon_bone_small: game_assets.icon_bone_small.clone(),
        icon_sand_small: game_assets.icon_sand_small.clone(),
        icon_stasis_mud_small: game_assets.icon_stasis_mud_small.clone(),
        bucket_water: game_assets.bucket_water.clone(),
        bucket_empty: game_assets.bucket_empty.clone(),
    });

    commands.insert_resource(TerrainVisualHandles {
        dirt: game_assets.dirt.clone(),
    });

    commands.insert_resource(DoorVisualHandles {
        door_open: game_assets.door_open.clone(),
        door_closed: game_assets.door_closed.clone(),
    });

    commands.insert_resource(ResourceItemVisualHandles {
        icon_bone_small: game_assets.icon_bone_small.clone(),
        icon_wood_small: game_assets.icon_wood_small.clone(),
        icon_stasis_mud_small: game_assets.icon_stasis_mud_small.clone(),
    });

    // --- 3D レンダリング用ハンドル（Phase 2 プレースホルダー）---
    let wall_mesh = meshes.add(Cuboid::new(TILE_SIZE, TILE_SIZE, TILE_SIZE));
    let wall_orientation_aid_mesh = meshes.add(Cuboid::new(
        TILE_SIZE * 0.96,
        TILE_SIZE * 0.12,
        TILE_SIZE * 0.96,
    ));
    let floor_mesh = meshes.add(Plane3d::default().mesh().size(TILE_SIZE, TILE_SIZE));
    let door_mesh = meshes.add(Cuboid::new(TILE_SIZE, TILE_SIZE * 0.5, TILE_SIZE));
    let equipment_1x1_mesh = meshes.add(Cuboid::new(TILE_SIZE, TILE_SIZE * 0.6, TILE_SIZE));
    let equipment_2x2_mesh = meshes.add(Cuboid::new(
        TILE_SIZE * 2.0,
        TILE_SIZE * 0.8,
        TILE_SIZE * 2.0,
    ));
    let familiar_mesh = meshes.add(Rectangle::new(TILE_SIZE * 0.9, TILE_SIZE * 0.9));

    let wall_material = section_materials.add(make_section_material(LinearRgba::new(
        0.56, 0.44, 0.30, 1.0,
    )));
    let wall_provisional_material = section_materials.add(with_alpha_mode(
        make_section_material(LinearRgba::new(0.95, 0.72, 0.45, 0.9)),
        AlphaMode::Blend,
    ));
    let wall_orientation_aid_material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.95, 0.2),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let floor_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.4, 0.3, 0.2),
        perceptual_roughness: 1.0,
        reflectance: 0.0,
        ..default()
    });
    let door_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.6, 0.45, 0.2),
        perceptual_roughness: 1.0,
        reflectance: 0.0,
        ..default()
    });
    let equipment_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.5, 0.6),
        perceptual_roughness: 1.0,
        reflectance: 0.0,
        ..default()
    });
    let familiar_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        base_color_texture: Some(game_assets.familiar.clone()),
        cull_mode: None,
        ..default()
    });

    commands.insert_resource(Building3dHandles {
        wall_mesh,
        wall_material,
        wall_provisional_material,
        wall_orientation_aid_mesh,
        wall_orientation_aid_material,
        floor_mesh,
        floor_material,
        door_mesh,
        door_material,
        equipment_1x1_mesh,
        equipment_2x2_mesh,
        equipment_material,
        soul_scene: game_assets.soul_scene.clone(),
        familiar_mesh,
        familiar_material,
        render_layers: building_3d_render_layers(),
    });

    // --- 地形 3D ハンドル ---
    let terrain_tile_mesh = meshes.add(Plane3d::default().mesh().size(TILE_SIZE, TILE_SIZE));
    let terrain_surface =
        terrain_surface_materials.add(make_terrain_surface_material(TerrainSurfaceMaterialExt {
            uniforms: TerrainSurfaceUniform {
                cut_position: Vec4::ZERO,
                cut_normal: Vec3::NEG_Z.extend(0.0),
                thickness: TILE_SIZE * 5.0,
                cut_active: 0.0,
                map_world_width: hw_core::constants::MAP_WIDTH as f32 * TILE_SIZE,
                map_world_height: hw_core::constants::MAP_HEIGHT as f32 * TILE_SIZE,
                uv_scale: 1.0 / TILE_SIZE,
                blend_strength: 1.0,
                macro_noise_scale: 0.00045,
                overlay_scale: 0.0012,
            },
            terrain_id_map: Some(terrain_id_map_handle),
            terrain_feature_map: Some(feature_map_handle),
            grass_albedo: Some(game_assets.grass.clone()),
            dirt_albedo: Some(game_assets.dirt.clone()),
            sand_albedo: Some(game_assets.sand.clone()),
            river_albedo: Some(game_assets.river.clone()),
            terrain_macro_noise: Some(game_assets.terrain_macro_noise.clone()),
            grass_macro_overlay: Some(game_assets.grass_macro_overlay.clone()),
            dirt_macro_overlay: Some(game_assets.dirt_macro_overlay.clone()),
            sand_macro_overlay: Some(game_assets.sand_macro_overlay.clone()),
            terrain_blend_mask_soft: Some(game_assets.terrain_blend_mask_soft.clone()),
            river_flow_noise: Some(game_assets.river_flow_noise.clone()),
            river_normal_like: Some(game_assets.river_normal_like.clone()),
            shoreline_detail: Some(game_assets.shoreline_detail.clone()),
            terrain_feature_lut: Some(game_assets.terrain_feature_lut.clone()),
        }));
    commands.insert_resource(Terrain3dHandles {
        tile_mesh: terrain_tile_mesh,
        surface: terrain_surface,
    });

    commands.insert_resource(CharacterHandles {
        soul_body_material: character_materials
            .add(CharacterMaterial::body(game_assets.white_pixel.clone())),
        soul_face_material: character_materials.add(CharacterMaterial::face(
            game_assets.soul_face_atlas.clone(),
            LinearRgba::WHITE,
            soul_face_uv_scale(),
            soul_face_uv_offset(0.0, 0.0),
        )),
        soul_mask_material: soul_mask_materials.add(SoulMaskMaterial::solid_white()),
        soul_shadow_proxy_material: soul_shadow_materials.add(SoulShadowMaterial::default()),
    });
}
