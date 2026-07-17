use super::*;

type SoulProxyRootsQuery<'w, 's> = Query<'w, 's, &'static SoulProxy3d>;
type SoulMaskProxyRootsQuery<'w, 's> = Query<'w, 's, &'static SoulMaskProxy3d>;
type SoulShadowProxyRootsQuery<'w, 's> = Query<'w, 's, &'static SoulShadowProxy3d>;
type ChildListQuery<'w, 's> = Query<'w, 's, &'static Children>;
type TransformReadQuery<'w, 's> = Query<'w, 's, &'static Transform>;
type MeshNameQuery<'w, 's> = Query<'w, 's, &'static GltfMeshName>;
type NameQuery<'w, 's> = Query<'w, 's, &'static Name>;
type MeshMarkerQuery<'w, 's> = Query<'w, 's, (), With<Mesh3d>>;
type AnimationPlayerMarkerQuery<'w, 's> = Query<'w, 's, (), With<AnimationPlayer>>;
#[derive(SystemParam)]
pub struct SoulGltfApplyParams<'w, 's> {
    q_soul_roots: SoulProxyRootsQuery<'w, 's>,
    q_children: ChildListQuery<'w, 's>,
    q_transforms: TransformReadQuery<'w, 's>,
    q_mesh_names: MeshNameQuery<'w, 's>,
    q_names: NameQuery<'w, 's>,
    q_meshes: MeshMarkerQuery<'w, 's>,
    q_animation_players: AnimationPlayerMarkerQuery<'w, 's>,
}

#[derive(SystemParam)]
pub struct SoulMaskGltfApplyParams<'w, 's> {
    q_soul_mask_roots: SoulMaskProxyRootsQuery<'w, 's>,
    q_children: ChildListQuery<'w, 's>,
    q_meshes: MeshMarkerQuery<'w, 's>,
}

#[derive(SystemParam)]
pub struct SoulShadowGltfApplyParams<'w, 's> {
    q_soul_shadow_roots: SoulShadowProxyRootsQuery<'w, 's>,
    q_children: ChildListQuery<'w, 's>,
    q_mesh_names: MeshNameQuery<'w, 's>,
    q_names: NameQuery<'w, 's>,
    q_meshes: MeshMarkerQuery<'w, 's>,
    q_animation_players: AnimationPlayerMarkerQuery<'w, 's>,
}

/// Soul GLB の SceneRoot 子孫へ RenderLayers を付与し、RtT Camera3d に乗せる。
pub fn apply_soul_gltf_render_layers_on_ready(
    scene_ready: On<WorldInstanceReady>,
    mut commands: Commands,
    character_handles: Res<CharacterHandles>,
    mut character_materials: ResMut<Assets<CharacterMaterial>>,
    params: SoulGltfApplyParams,
) {
    let Ok(proxy) = params.q_soul_roots.get(scene_ready.entity) else {
        return;
    };
    if proxy.billboard {
        return;
    }

    let render_layers = RenderLayers::layer(LAYER_3D);
    for child in params.q_children.iter_descendants(scene_ready.entity) {
        let mut entity_commands = commands.entity(child);
        entity_commands.insert(render_layers.clone());

        if params.q_animation_players.get(child).is_ok() {
            entity_commands.insert(SoulAnimationPlayer3d {
                owner: proxy.owner,
                current_body: SoulBodyAnimState::Idle,
                walk_facing_right: None,
                last_owner_pos: None,
                directional_variant_lock_secs: 0.0,
            });
        }

        let is_mesh_entity = params.q_meshes.get(child).is_ok();
        if !is_mesh_entity {
            continue;
        }
        entity_commands.insert(NotShadowCaster);
        let mesh_name = params
            .q_mesh_names
            .get(child)
            .ok()
            .map(|name| name.0.as_str());
        let name = params.q_names.get(child).ok().map(Name::as_str);
        let is_face_mesh =
            matches!(mesh_name, Some("Soul_Face_Mesh")) || matches!(name, Some("Soul_Face_Mesh"));
        if is_face_mesh {
            if let Ok(face_transform) = params.q_transforms.get(child) {
                let mut scaled_face = *face_transform;
                scaled_face.scale *=
                    Vec3::new(SOUL_FACE_SCALE_MULTIPLIER, SOUL_FACE_SCALE_MULTIPLIER, 1.0);
                entity_commands.insert(scaled_face);
            }
            let face_material = character_materials
                .get(&character_handles.soul_face_material)
                .cloned()
                .map(|template| character_materials.add(template))
                .unwrap_or_else(|| character_handles.soul_face_material.clone());
            entity_commands
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .insert(MeshMaterial3d(face_material.clone()))
                .insert(SoulFaceMaterial3d {
                    owner: proxy.owner,
                    material: face_material,
                    last_applied_face: None,
                });
            continue;
        }

        let is_body_mesh = matches!(mesh_name, Some("Soul_Mesh.010"))
            || matches!(name, Some("Soul_Mesh.010"))
            || matches!(name, Some("Soul_Mesh.010.SoulMat"));
        if is_body_mesh {
            entity_commands
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .insert(MeshMaterial3d::<CharacterMaterial>(
                    character_handles.soul_body_material.clone(),
                ));
        }
    }
}

/// Soul shadow proxy 用 SceneRoot 子孫へ RenderLayers と shadow proxy 設定を付与する。
pub fn apply_soul_shadow_gltf_render_layers_on_ready(
    scene_ready: On<WorldInstanceReady>,
    mut commands: Commands,
    character_handles: Res<CharacterHandles>,
    params: SoulShadowGltfApplyParams,
) {
    let Ok(proxy) = params.q_soul_shadow_roots.get(scene_ready.entity) else {
        return;
    };

    let render_layers = RenderLayers::from_layers(&[LAYER_3D, LAYER_3D_SOUL_SHADOW]);
    for child in params.q_children.iter_descendants(scene_ready.entity) {
        let mut entity_commands = commands.entity(child);
        entity_commands.insert((render_layers.clone(), NotShadowReceiver));
        entity_commands.remove::<NotShadowCaster>();

        if params.q_animation_players.get(child).is_ok() {
            entity_commands.insert(SoulAnimationPlayer3d {
                owner: proxy.owner,
                current_body: SoulBodyAnimState::Idle,
                walk_facing_right: None,
                last_owner_pos: None,
                directional_variant_lock_secs: 0.0,
            });
        }

        if params.q_meshes.get(child).is_err() {
            continue;
        }

        let mesh_name = params
            .q_mesh_names
            .get(child)
            .ok()
            .map(|name| name.0.as_str());
        let name = params.q_names.get(child).ok().map(Name::as_str);
        let is_face_mesh =
            matches!(mesh_name, Some("Soul_Face_Mesh")) || matches!(name, Some("Soul_Face_Mesh"));
        if is_face_mesh {
            entity_commands
                .remove::<MeshMaterial3d<CharacterMaterial>>()
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .remove::<MeshMaterial3d<SoulShadowMaterial>>()
                .insert(MeshMaterial3d(
                    character_handles.soul_shadow_proxy_material.clone(),
                ))
                .insert(NotShadowCaster);
            continue;
        }

        let is_body_mesh = matches!(mesh_name, Some("Soul_Mesh.010"))
            || matches!(name, Some("Soul_Mesh.010"))
            || matches!(name, Some("Soul_Mesh.010.SoulMat"));
        if is_body_mesh {
            entity_commands
                .remove::<MeshMaterial3d<CharacterMaterial>>()
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .remove::<MeshMaterial3d<SoulShadowMaterial>>()
                .insert(MeshMaterial3d(
                    character_handles.soul_shadow_proxy_material.clone(),
                ))
                .insert(NotShadowCaster);
        }
    }
}

/// Soul mask 用 SceneRoot 子孫へ RenderLayers と単色 mask material を付与する。
pub fn apply_soul_mask_gltf_render_layers_on_ready(
    scene_ready: On<WorldInstanceReady>,
    mut commands: Commands,
    character_handles: Res<CharacterHandles>,
    params: SoulMaskGltfApplyParams,
) {
    let Ok(_proxy) = params.q_soul_mask_roots.get(scene_ready.entity) else {
        return;
    };

    let render_layers = RenderLayers::layer(LAYER_3D_SOUL_MASK);
    for child in params.q_children.iter_descendants(scene_ready.entity) {
        let mut entity_commands = commands.entity(child);
        entity_commands.insert(render_layers.clone());

        if params.q_meshes.get(child).is_err() {
            continue;
        }

        entity_commands
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .remove::<MeshMaterial3d<CharacterMaterial>>()
            .insert(MeshMaterial3d::<SoulMaskMaterial>(
                character_handles.soul_mask_material.clone(),
            ));
    }
}
