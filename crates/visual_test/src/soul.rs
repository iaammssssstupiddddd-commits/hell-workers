use crate::types::*;
use bevy::camera::visibility::RenderLayers;
use bevy::gltf::{Gltf, GltfMeshName};
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::Mesh3d;
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;
use hw_core::constants::{
    LAYER_3D, LAYER_3D_SOUL_MASK, LAYER_3D_SOUL_SHADOW, SOUL_FACE_SCALE_MULTIPLIER, SOUL_GLB_SCALE,
    SOUL_SHADOW_PROXY_PITCH_CORRECTION_DEGREES,
};
use hw_visual::visual3d::{SoulMaskProxy3d, SoulShadowProxy3d};
use hw_visual::{CharacterMaterial, SoulMaskMaterial, SoulShadowMaterial};

// ─── Soul 生成 ────────────────────────────────────────────────────────────────

const BLOB_SHADOW_XY_RADIUS: f32 = 0.28;
const SHADOW_COMPARE_PAIR_GAP: f32 = SOUL_SPACING * 1.75;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TestSoulShadowCaster {
    Glb,
    BlobCandidate,
}

pub struct SoulSpawnArgs<'a> {
    pub soul_scene: &'a Handle<WorldAsset>,
    pub face_atlas: &'a Handle<Image>,
    pub white_pixel: &'a Handle<Image>,
    pub blob_shadow_mesh: &'a Handle<Mesh>,
    pub blob_shadow_material: &'a Handle<StandardMaterial>,
    pub soul_shadow_material: &'a Handle<SoulShadowMaterial>,
    pub soul_mask_material: &'a Handle<SoulMaskMaterial>,
    pub shadow_caster: TestSoulShadowCaster,
    pub x: f32,
    pub z: f32,
    pub index: usize,
    pub initial_expr: FaceExpression,
    pub selected: bool,
}

pub fn spawn_test_soul(
    commands: &mut Commands,
    character_materials: &mut Assets<CharacterMaterial>,
    args: SoulSpawnArgs,
) {
    let face_mat = character_materials.add(CharacterMaterial::face(
        args.face_atlas.clone(),
        LinearRgba::WHITE,
        face_uv_scale(),
        args.initial_expr.uv_offset(),
    ));
    let body_mat = character_materials.add(CharacterMaterial::body(args.white_pixel.clone()));

    let mut entity = commands.spawn((
        WorldAssetRoot(args.soul_scene.clone()),
        Transform::from_xyz(args.x, 0.0, args.z).with_scale(Vec3::splat(SOUL_GLB_SCALE)),
        RenderLayers::layer(LAYER_3D),
        TestSoulConfig {
            face_mat,
            body_mat,
            index: args.index,
        },
    ));
    if args.selected {
        entity.insert(SelectedSoul);
    }
    let soul_entity = entity.id();

    match args.shadow_caster {
        TestSoulShadowCaster::Glb => {
            commands.spawn((
                WorldAssetRoot(args.soul_scene.clone()),
                Transform::from_xyz(args.x, 0.0, args.z)
                    .with_scale(Vec3::splat(SOUL_GLB_SCALE))
                    .with_rotation(Quat::from_rotation_x(
                        (-SOUL_SHADOW_PROXY_PITCH_CORRECTION_DEGREES).to_radians(),
                    )),
                RenderLayers::from_layers(&[LAYER_3D, LAYER_3D_SOUL_SHADOW]),
                SoulShadowProxy3d { owner: soul_entity },
                SoulShadowConfig {
                    shadow_mat: args.soul_shadow_material.clone(),
                },
            ));
        }
        TestSoulShadowCaster::BlobCandidate => {
            commands.spawn((
                Mesh3d(args.blob_shadow_mesh.clone()),
                MeshMaterial3d(args.blob_shadow_material.clone()),
                Transform::from_xyz(args.x, 0.0, args.z)
                    .with_rotation(blob_shadow_proxy_rotation()),
                RenderLayers::from_layers(&[LAYER_3D_SOUL_SHADOW]),
                NotShadowReceiver,
                SoulBlobShadowProxy3d { owner: soul_entity },
            ));
        }
    }

    commands.spawn((
        WorldAssetRoot(args.soul_scene.clone()),
        Transform::from_xyz(args.x, 0.0, args.z).with_scale(Vec3::splat(SOUL_GLB_SCALE)),
        RenderLayers::layer(LAYER_3D_SOUL_MASK),
        SoulMaskProxy3d { owner: soul_entity },
        SoulMaskConfig {
            mask_mat: args.soul_mask_material.clone(),
        },
    ));
}

// ─── GLB マテリアル差し替え + アニメーション設定 ──────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn on_soul_scene_ready(
    scene_ready: On<WorldInstanceReady>,
    q_configs: Query<&TestSoulConfig>,
    q_children: Query<&Children>,
    q_mesh_names: Query<&GltfMeshName>,
    q_names: Query<&Name>,
    q_meshes: Query<(), With<Mesh3d>>,
    q_transforms: Query<&Transform>,
    q_anim_players: Query<(), With<AnimationPlayer>>,
    assets: Option<Res<TestAssets>>,
    gltfs: Res<Assets<Gltf>>,
    mut anim_graphs: ResMut<Assets<AnimationGraph>>,
    mut commands: Commands,
) {
    let Ok(config) = q_configs.get(scene_ready.entity) else {
        return;
    };
    let render_layers = RenderLayers::layer(LAYER_3D);
    let mut anim_player_entity: Option<Entity> = None;

    for child in q_children.iter_descendants(scene_ready.entity) {
        let mut ec = commands.entity(child);
        ec.insert(render_layers.clone());
        if q_anim_players.get(child).is_ok() {
            anim_player_entity = Some(child);
        }
        if q_meshes.get(child).is_err() {
            continue;
        }
        let mesh_name = q_mesh_names.get(child).ok().map(|n| n.0.as_str());
        let name = q_names.get(child).ok().map(Name::as_str);

        if matches!(mesh_name, Some("Soul_Face_Mesh")) || matches!(name, Some("Soul_Face_Mesh")) {
            if let Ok(face_tf) = q_transforms.get(child) {
                let mut scaled = *face_tf;
                scaled.scale *=
                    Vec3::new(SOUL_FACE_SCALE_MULTIPLIER, SOUL_FACE_SCALE_MULTIPLIER, 1.0);
                ec.insert(scaled);
            }
            ec.remove::<MeshMaterial3d<StandardMaterial>>()
                .insert((MeshMaterial3d(config.face_mat.clone()), NotShadowCaster));
            continue;
        }

        let is_body = matches!(mesh_name, Some("Soul_Mesh.010"))
            || matches!(name, Some("Soul_Mesh.010") | Some("Soul_Mesh.010.SoulMat"));
        if is_body {
            ec.remove::<MeshMaterial3d<StandardMaterial>>().insert((
                MeshMaterial3d::<CharacterMaterial>(config.body_mat.clone()),
                NotShadowCaster,
            ));
        }
    }

    // アニメーション設定
    let Some(player_entity) = anim_player_entity else {
        return;
    };
    let Some(ref assets) = assets else { return };
    let Some(gltf) = gltfs.get(&assets.gltf_handle) else {
        return;
    };

    let mut graph = AnimationGraph::new();
    let clips: Vec<(&'static str, AnimationNodeIndex)> = ANIM_CLIP_NAMES
        .iter()
        .filter_map(|name| {
            gltf.named_animations
                .get(*name)
                .cloned()
                .map(|clip| (*name, graph.add_clip(clip, 1.0, graph.root)))
        })
        .collect();
    if clips.is_empty() {
        return;
    }

    let graph_handle = anim_graphs.add(graph);
    commands.entity(player_entity).insert((
        AnimationGraphHandle(graph_handle),
        AnimationTransitions::new(),
    ));
    commands.entity(scene_ready.entity).insert(SoulAnimHandle {
        anim_player_entity: player_entity,
        clips,
        current_playing: usize::MAX,
    });
}

// ─── シャドウ・マスクプロキシ Observer ──────────────────────────────────────

pub fn on_shadow_scene_ready(
    scene_ready: On<WorldInstanceReady>,
    q_configs: Query<&SoulShadowConfig>,
    q_children: Query<&Children>,
    q_meshes: Query<(), With<Mesh3d>>,
    mut commands: Commands,
) {
    let Ok(config) = q_configs.get(scene_ready.entity) else {
        return;
    };
    let render_layers = RenderLayers::from_layers(&[LAYER_3D, LAYER_3D_SOUL_SHADOW]);
    for child in q_children.iter_descendants(scene_ready.entity) {
        let mut ec = commands.entity(child);
        ec.insert((render_layers.clone(), NotShadowReceiver));
        ec.remove::<NotShadowCaster>();
        if q_meshes.get(child).is_ok() {
            ec.remove::<MeshMaterial3d<StandardMaterial>>()
                .insert(MeshMaterial3d(config.shadow_mat.clone()));
        }
    }
}

// ─── マスクプロキシ Observer ─────────────────────────────────────────────────

pub fn on_mask_scene_ready(
    scene_ready: On<WorldInstanceReady>,
    q_configs: Query<&SoulMaskConfig>,
    q_children: Query<&Children>,
    q_meshes: Query<(), With<Mesh3d>>,
    mut commands: Commands,
) {
    let Ok(config) = q_configs.get(scene_ready.entity) else {
        return;
    };
    let mask_mat = config.mask_mat.clone();
    let render_layers = RenderLayers::layer(LAYER_3D_SOUL_MASK);
    for child in q_children.iter_descendants(scene_ready.entity) {
        let mut ec = commands.entity(child);
        ec.insert(render_layers.clone());
        if q_meshes.get(child).is_ok() {
            ec.remove::<MeshMaterial3d<StandardMaterial>>()
                .insert(MeshMaterial3d(mask_mat.clone()));
        }
    }
}

// ─── プロキシ同期 ─────────────────────────────────────────────────────────────

pub fn sync_mask_proxies(
    q_souls: Query<(Entity, &Transform), With<TestSoulConfig>>,
    mut q_proxies: Query<(&SoulMaskProxy3d, &mut Transform), Without<TestSoulConfig>>,
) {
    for (proxy, mut proxy_tf) in q_proxies.iter_mut() {
        if let Ok((_, soul_tf)) = q_souls.get(proxy.owner) {
            proxy_tf.translation = soul_tf.translation;
            proxy_tf.scale = soul_tf.scale;
            proxy_tf.rotation = Quat::IDENTITY;
        }
    }
}

pub fn sync_shadow_proxies(
    q_souls: Query<(Entity, &Transform), With<TestSoulConfig>>,
    mut q_proxies: Query<(&SoulShadowProxy3d, &mut Transform), Without<TestSoulConfig>>,
) {
    let pitch = Quat::from_rotation_x((-SOUL_SHADOW_PROXY_PITCH_CORRECTION_DEGREES).to_radians());
    for (proxy, mut proxy_tf) in q_proxies.iter_mut() {
        if let Ok((_, soul_tf)) = q_souls.get(proxy.owner) {
            proxy_tf.translation = soul_tf.translation;
            proxy_tf.scale = soul_tf.scale;
            proxy_tf.rotation = pitch;
        }
    }
}

pub fn sync_blob_shadow_proxies(
    q_souls: Query<(Entity, &Transform), With<TestSoulConfig>>,
    mut q_proxies: Query<(&SoulBlobShadowProxy3d, &mut Transform), Without<TestSoulConfig>>,
) {
    let rotation = blob_shadow_proxy_rotation();
    for (proxy, mut proxy_tf) in q_proxies.iter_mut() {
        if let Ok((_, soul_tf)) = q_souls.get(proxy.owner) {
            proxy_tf.translation = soul_tf.translation;
            proxy_tf.scale = soul_tf.scale / SOUL_GLB_SCALE;
            proxy_tf.rotation = rotation;
        }
    }
}

#[derive(Default)]
pub struct SoulRebuildEntities {
    pub souls: Vec<Entity>,
    pub shadows: Vec<Entity>,
    pub blob_shadows: Vec<Entity>,
    pub masks: Vec<Entity>,
}

pub fn rebuild_soul_test_layout(
    commands: &mut Commands,
    character_materials: &mut Assets<CharacterMaterial>,
    assets: &TestAssets,
    state: &mut TestState,
    entities: SoulRebuildEntities,
    layout: SoulLayout,
) {
    for entity in entities.souls {
        commands.entity(entity).despawn();
    }
    for entity in entities.shadows {
        commands.entity(entity).despawn();
    }
    for entity in entities.blob_shadows {
        commands.entity(entity).despawn();
    }
    for entity in entities.masks {
        commands.entity(entity).despawn();
    }

    state.soul_layout = layout;
    state.soul_count = 0;
    state.next_index = 0;

    match layout {
        SoulLayout::Default => {
            for i in 0..3 {
                spawn_test_soul(
                    commands,
                    character_materials,
                    SoulSpawnArgs {
                        soul_scene: &assets.soul_scene,
                        face_atlas: &assets.face_atlas,
                        white_pixel: &assets.white_pixel,
                        blob_shadow_mesh: &assets.blob_shadow_mesh,
                        blob_shadow_material: &assets.blob_shadow_material,
                        soul_shadow_material: &assets.soul_shadow_material,
                        soul_mask_material: &assets.soul_mask_material,
                        shadow_caster: TestSoulShadowCaster::Glb,
                        x: (i as f32 - 1.0) * SOUL_SPACING,
                        z: 0.0,
                        index: state.next_index,
                        initial_expr: initial_expr_for_layout(state.face_mode, i),
                        selected: i == 0,
                    },
                );
                state.next_index += 1;
                state.soul_count += 1;
            }
        }
        SoulLayout::ShadowCompare => {
            for (i, (x, shadow_caster)) in [
                (-SHADOW_COMPARE_PAIR_GAP * 0.5, TestSoulShadowCaster::Glb),
                (
                    SHADOW_COMPARE_PAIR_GAP * 0.5,
                    TestSoulShadowCaster::BlobCandidate,
                ),
            ]
            .into_iter()
            .enumerate()
            {
                spawn_test_soul(
                    commands,
                    character_materials,
                    SoulSpawnArgs {
                        soul_scene: &assets.soul_scene,
                        face_atlas: &assets.face_atlas,
                        white_pixel: &assets.white_pixel,
                        blob_shadow_mesh: &assets.blob_shadow_mesh,
                        blob_shadow_material: &assets.blob_shadow_material,
                        soul_shadow_material: &assets.soul_shadow_material,
                        soul_mask_material: &assets.soul_mask_material,
                        shadow_caster,
                        x,
                        z: 0.0,
                        index: state.next_index,
                        initial_expr: initial_expr_for_layout(state.face_mode, i),
                        selected: i == 0,
                    },
                );
                state.next_index += 1;
                state.soul_count += 1;
            }
        }
    }
}

fn initial_expr_for_layout(face_mode: FaceMode, index: usize) -> FaceExpression {
    match face_mode {
        FaceMode::Single(expr) => expr,
        FaceMode::AllDifferent => FaceExpression::ALL[index % FaceExpression::ALL.len()],
    }
}

fn blob_shadow_proxy_rotation() -> Quat {
    Quat::from_rotation_x((-70.0f32).to_radians()) * Quat::from_rotation_z((-70.0f32).to_radians())
}

pub fn blob_shadow_outline() -> Vec<Vec2> {
    (0..48)
        .map(|i| {
            let angle = (i as f32 / 48.0) * std::f32::consts::TAU;
            let wobble = 1.0
                + 0.18
                    * ((angle * 3.0 + 1.3).sin() * 0.50
                        + (angle * 5.0 + 2.7).sin() * 0.30
                        + (angle * 7.0 + 0.8).sin() * 0.20);
            let radius = BLOB_SHADOW_XY_RADIUS * wobble;
            Vec2::new(angle.cos() * radius, angle.sin() * radius)
        })
        .collect()
}
