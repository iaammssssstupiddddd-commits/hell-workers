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
    PlantTreeHandles, SoulShadowMaterial, SpeechHandles, TerrainSurfaceLutImageHandle,
    TerrainSurfaceMaterial, TerrainSurfaceMaterialExt, TerrainSurfaceMaterialExtLod1Lite,
    TerrainSurfaceMaterialExtLod2, TerrainSurfaceMaterialLod1Lite, TerrainSurfaceMaterialLod2,
    TerrainSurfaceUniform, TopDownStructuralMaterial, WallVisualHandles, WorkIconHandles,
    make_terrain_surface_material, make_terrain_surface_material_lod1_lite,
    make_terrain_surface_material_lod2, make_topdown_structural_material, with_topdown_alpha_mode,
};
use hw_visual::{CharacterMaterial, soul_face_uv_offset, soul_face_uv_scale};
use hw_world::DoorVisualHandles;

use crate::systems::visual::indoor_light_texture::IndoorLightTexture;
use crate::world::map::{TerrainFeatureMap, TerrainIdMap};

/// 3D レンダリング用メッシュ・マテリアルハンドルリソース
///
/// Phase 2 プレースホルダープリミティブ（Cuboid/Plane3d）を保持する。
/// Phase 3 で GLB に置き換え予定。
#[derive(Resource)]
pub struct Building3dHandles {
    // --- 壁 ---
    pub wall_mesh: Handle<Mesh>,
    pub wall_material: Handle<TopDownStructuralMaterial>,
    pub wall_provisional_material: Handle<TopDownStructuralMaterial>,
    // --- 床 ---
    pub floor_mesh: Handle<Mesh>,
    pub floor_material: Handle<TopDownStructuralMaterial>,
    pub bridge_mesh: Handle<Mesh>,
    pub bridge_material: Handle<TopDownStructuralMaterial>,
    // --- ドア ---
    pub door_mesh: Handle<Mesh>,
    pub door_closed_material: Handle<TopDownStructuralMaterial>,
    pub door_open_material: Handle<TopDownStructuralMaterial>,
    pub door_locked_material: Handle<TopDownStructuralMaterial>,
    // --- 設備 (Tank / MudMixer / RestArea / WheelbarrowParking / SandPile / BonePile) ---
    pub equipment_1x1_mesh: Handle<Mesh>,
    pub equipment_2x2_mesh: Handle<Mesh>,
    pub equipment_material: Handle<TopDownStructuralMaterial>,
    pub tank_partial_material: Handle<TopDownStructuralMaterial>,
    pub tank_full_material: Handle<TopDownStructuralMaterial>,
    pub mixer_idle_material: Handle<TopDownStructuralMaterial>,
    pub mixer_active_material: Handle<TopDownStructuralMaterial>,
    // --- キャラクター ---
    pub soul_scene: Handle<WorldAsset>,
    pub soul_billboards: SoulBillboardHandles,
    /// 全3Dエンティティに付与する RenderLayers
    pub render_layers: RenderLayers,
}

/// 地形タイル 3D レンダリング用マテリアルハンドルリソース
///
/// 全 chunk で共有する LOD1 / LOD1Lite / LOD2 の地形 material を保持する。
/// chunk mesh は `spawn_terrain_chunks` が `Assets<Mesh>` に直接追加する。
#[derive(Resource)]
pub struct Terrain3dHandles {
    pub lod1: Handle<TerrainSurfaceMaterial>,
    pub lod1_lite: Handle<TerrainSurfaceMaterialLod1Lite>,
    pub lod2: Handle<TerrainSurfaceMaterialLod2>,
}

#[derive(Resource)]
pub struct CharacterHandles {
    pub soul_body_material: Handle<CharacterMaterial>,
    pub soul_face_material: Handle<CharacterMaterial>,
    pub soul_shadow_proxy_material: Handle<SoulShadowMaterial>,
}

/// Finite shared material pool for all production Soul billboards.
#[derive(Resource, Clone)]
pub struct SoulBillboardHandles {
    pub mesh: Handle<Mesh>,
    pub normal: Handle<StandardMaterial>,
    pub exhausted: Handle<StandardMaterial>,
    pub happy: Handle<StandardMaterial>,
    pub sleep: Handle<StandardMaterial>,
    pub wine: Handle<StandardMaterial>,
    pub trump: Handle<StandardMaterial>,
    pub stress: Handle<StandardMaterial>,
    pub stress_breakdown: Handle<StandardMaterial>,
}

impl SoulBillboardHandles {
    pub fn material(&self, frame: hw_visual::SoulBillboardFrame) -> Handle<StandardMaterial> {
        match frame {
            hw_visual::SoulBillboardFrame::Normal => self.normal.clone(),
            hw_visual::SoulBillboardFrame::Exhausted => self.exhausted.clone(),
            hw_visual::SoulBillboardFrame::Happy => self.happy.clone(),
            hw_visual::SoulBillboardFrame::Sleep => self.sleep.clone(),
            hw_visual::SoulBillboardFrame::Wine => self.wine.clone(),
            hw_visual::SoulBillboardFrame::Trump => self.trump.clone(),
            hw_visual::SoulBillboardFrame::Stress => self.stress.clone(),
            hw_visual::SoulBillboardFrame::StressBreakdown => self.stress_breakdown.clone(),
        }
    }
}

#[derive(SystemParam)]
pub struct InitVisualHandlesParams<'w, 's> {
    commands: Commands<'w, 's>,
    game_assets: Res<'w, GameAssets>,
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
    structural_materials: ResMut<'w, Assets<TopDownStructuralMaterial>>,
    terrain_surface_materials: ResMut<'w, Assets<TerrainSurfaceMaterial>>,
    terrain_surface_materials_lod1_lite: ResMut<'w, Assets<TerrainSurfaceMaterialLod1Lite>>,
    terrain_surface_materials_lod2: ResMut<'w, Assets<TerrainSurfaceMaterialLod2>>,
    character_materials: ResMut<'w, Assets<CharacterMaterial>>,
    terrain_feature_map: Res<'w, TerrainFeatureMap>,
    terrain_id_map: Res<'w, TerrainIdMap>,
    indoor_light_texture: Res<'w, IndoorLightTexture>,
}

pub fn init_visual_handles(mut params: InitVisualHandlesParams) {
    let game_assets = params.game_assets.as_ref();
    let commands = &mut params.commands;
    let meshes = &mut params.meshes;
    let materials = &mut params.materials;
    let structural_materials = &mut params.structural_materials;
    let character_materials = &mut params.character_materials;
    let feature_map_handle = params.terrain_feature_map.image.clone();
    let terrain_id_map_handle = params.terrain_id_map.image.clone();
    commands.insert_resource(TerrainSurfaceLutImageHandle(
        game_assets.terrain_feature_lut.clone(),
    ));
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

    commands.insert_resource(DoorVisualHandles {
        door_open: game_assets.door_open.clone(),
        door_closed: game_assets.door_closed.clone(),
    });

    commands.insert_resource(ResourceItemVisualHandles {
        icon_bone_small: game_assets.icon_bone_small.clone(),
        icon_wood_small: game_assets.icon_wood_small.clone(),
        icon_rock_small: game_assets.icon_rock_small.clone(),
        icon_sand_small: game_assets.icon_sand_small.clone(),
        icon_stasis_mud_small: game_assets.icon_stasis_mud_small.clone(),
    });

    // --- 3D レンダリング用ハンドル（Phase 2 プレースホルダー）---
    let wall_mesh = meshes.add(Cuboid::new(TILE_SIZE, TILE_SIZE, TILE_SIZE));
    let floor_mesh = meshes.add(Plane3d::default().mesh().size(TILE_SIZE, TILE_SIZE));
    let bridge_mesh = meshes.add(Cuboid::new(
        TILE_SIZE * 2.0,
        TILE_SIZE * 0.18,
        TILE_SIZE * 5.0,
    ));
    // A narrow leaf keeps Open and Closed visibly distinct from the fixed
    // TopDown view. The presentation consumer applies the hinge transform.
    let door_mesh = meshes.add(Cuboid::new(
        TILE_SIZE * 0.82,
        TILE_SIZE * 0.5,
        TILE_SIZE * 0.18,
    ));
    let equipment_1x1_mesh = meshes.add(Cuboid::new(TILE_SIZE, TILE_SIZE * 0.6, TILE_SIZE));
    let equipment_2x2_mesh = meshes.add(Cuboid::new(
        TILE_SIZE * 2.0,
        TILE_SIZE * 0.8,
        TILE_SIZE * 2.0,
    ));
    let soul_billboard_mesh = meshes.add(Rectangle::new(TILE_SIZE * 0.9, TILE_SIZE * 1.1));

    let indoor_light_field = params.indoor_light_texture.handle().clone();
    let wall_provisional_material = structural_materials.add(with_topdown_alpha_mode(
        make_topdown_structural_material(
            LinearRgba::new(0.95, 0.72, 0.45, 0.9),
            indoor_light_field.clone(),
        ),
        AlphaMode::Blend,
    ));
    let mut structural_material = |color: LinearRgba| {
        structural_materials.add(make_topdown_structural_material(
            color,
            indoor_light_field.clone(),
        ))
    };
    let wall_material = structural_material(LinearRgba::new(0.56, 0.44, 0.30, 1.0));
    let floor_material = structural_material(LinearRgba::new(0.4, 0.3, 0.2, 1.0));
    let bridge_material = structural_material(LinearRgba::new(0.38, 0.24, 0.12, 1.0));
    let door_closed_material = structural_material(LinearRgba::new(0.6, 0.45, 0.2, 1.0));
    let door_open_material = structural_material(LinearRgba::new(0.32, 0.62, 0.28, 1.0));
    let door_locked_material = structural_material(LinearRgba::new(0.68, 0.20, 0.16, 1.0));
    let equipment_material = structural_material(LinearRgba::new(0.3, 0.5, 0.6, 1.0));
    let tank_partial_material = structural_material(LinearRgba::new(0.24, 0.48, 0.72, 1.0));
    let tank_full_material = structural_material(LinearRgba::new(0.16, 0.68, 0.88, 1.0));
    let mixer_idle_material = structural_material(LinearRgba::new(0.42, 0.32, 0.24, 1.0));
    let mixer_active_material = structural_material(LinearRgba::new(0.75, 0.38, 0.12, 1.0));
    let mut billboard_material = |image: Handle<Image>| {
        materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(image),
            unlit: true,
            alpha_mode: AlphaMode::Mask(0.5),
            cull_mode: None,
            ..default()
        })
    };
    let soul_billboards = SoulBillboardHandles {
        mesh: soul_billboard_mesh,
        normal: billboard_material(game_assets.soul.clone()),
        exhausted: billboard_material(game_assets.soul_exhausted.clone()),
        happy: billboard_material(game_assets.soul_lough.clone()),
        sleep: billboard_material(game_assets.soul_sleep.clone()),
        wine: billboard_material(game_assets.soul_wine.clone()),
        trump: billboard_material(game_assets.soul_trump.clone()),
        stress: billboard_material(game_assets.soul_stress.clone()),
        stress_breakdown: billboard_material(game_assets.soul_stress_breakdown.clone()),
    };

    commands.insert_resource(Building3dHandles {
        wall_mesh,
        wall_material,
        wall_provisional_material,
        floor_mesh,
        floor_material,
        bridge_mesh,
        bridge_material,
        door_mesh,
        door_closed_material,
        door_open_material,
        door_locked_material,
        equipment_1x1_mesh,
        equipment_2x2_mesh,
        equipment_material,
        tank_partial_material,
        tank_full_material,
        mixer_idle_material,
        mixer_active_material,
        soul_scene: game_assets.soul_scene.clone(),
        soul_billboards: soul_billboards.clone(),
        render_layers: building_3d_render_layers(),
    });
    commands.insert_resource(soul_billboards);

    // --- 地形 3D ハンドル ---
    let terrain_ext = TerrainSurfaceMaterialExt {
        uniforms: TerrainSurfaceUniform::default(),
        terrain_id_map: Some(terrain_id_map_handle.clone()),
        terrain_feature_map: Some(feature_map_handle.clone()),
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
        boundary_mask: None, // spawn_boundary_meshes (PostStartup) で後から設定される
        boundary_proximity_mask: None,
        indoor_light_field: Some(indoor_light_field.clone()),
    };
    let terrain_surface = params
        .terrain_surface_materials
        .add(make_terrain_surface_material(terrain_ext));

    let terrain_surface_lod1_lite =
        params
            .terrain_surface_materials_lod1_lite
            .add(make_terrain_surface_material_lod1_lite(
                TerrainSurfaceMaterialExtLod1Lite {
                    uniforms: TerrainSurfaceUniform::default(),
                    terrain_id_map: Some(terrain_id_map_handle.clone()),
                    terrain_feature_map: Some(feature_map_handle.clone()),
                    grass_albedo: Some(game_assets.grass.clone()),
                    dirt_albedo: Some(game_assets.dirt.clone()),
                    sand_albedo: Some(game_assets.sand.clone()),
                    river_albedo: Some(game_assets.river.clone()),
                    terrain_feature_lut: Some(game_assets.terrain_feature_lut.clone()),
                    boundary_mask: None,
                    boundary_proximity_mask: None,
                    indoor_light_field: Some(indoor_light_field.clone()),
                    ..Default::default()
                },
            ));

    // LOD2 マテリアル: 実際に使うテクスチャのみ設定し、未使用スロットは None のまま。
    let terrain_surface_lod2 =
        params
            .terrain_surface_materials_lod2
            .add(make_terrain_surface_material_lod2(
                TerrainSurfaceMaterialExtLod2 {
                    uniforms: TerrainSurfaceUniform::default(),
                    terrain_id_map: Some(terrain_id_map_handle),
                    terrain_feature_map: Some(feature_map_handle),
                    grass_albedo: Some(game_assets.grass.clone()),
                    dirt_albedo: Some(game_assets.dirt.clone()),
                    sand_albedo: Some(game_assets.sand.clone()),
                    river_albedo: Some(game_assets.river.clone()),
                    terrain_feature_lut: Some(game_assets.terrain_feature_lut.clone()),
                    boundary_mask: None, // spawn_boundary_meshes (PostStartup) で後から設定される
                    boundary_proximity_mask: None,
                    indoor_light_field: Some(indoor_light_field),
                    ..Default::default()
                },
            ));

    commands.insert_resource(Terrain3dHandles {
        lod1: terrain_surface,
        lod1_lite: terrain_surface_lod1_lite,
        lod2: terrain_surface_lod2,
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
        // P02 stops the production shadow pipeline. Keep a default handle only
        // so the P08 physical-deletion batch can remove legacy module types.
        soul_shadow_proxy_material: Handle::default(),
    });
}
