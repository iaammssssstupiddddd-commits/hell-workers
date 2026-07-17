use super::*;

// ─── 顔アトラス UV ────────────────────────────────────────────────────────────

const ATLAS_COLS: f32 = 3.0;
const ATLAS_ROWS: f32 = 2.0;
const CELL_PX: f32 = 256.0;
const CROP_PX: f32 = 152.0;
const MAG: f32 = 1.4;
const CROP_OX: f32 = 24.0;
const CROP_OY: f32 = 32.0;

pub fn face_uv_scale() -> Vec2 {
    Vec2::new(
        CROP_PX / MAG / CELL_PX / ATLAS_COLS,
        CROP_PX / MAG / CELL_PX / ATLAS_ROWS,
    )
}

pub fn face_uv_offset(col: f32, row: f32) -> Vec2 {
    let adj = (CROP_PX - CROP_PX / MAG) * 0.5;
    Vec2::new(
        (col * CELL_PX + CROP_OX + adj) / (CELL_PX * ATLAS_COLS),
        (row * CELL_PX + CROP_OY + adj) / (CELL_PX * ATLAS_ROWS),
    )
}

// ─── RtT 合成マテリアル ──────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, ShaderType)]
pub struct RttCompositeParams {
    pub pixel_size: Vec2,
    pub mask_radius_px: f32,
    pub mask_feather: f32,
    pub shadow_offset_uv: Vec2,
    pub shadow_width_px: f32,
    pub shadow_strength: f32,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct LocalRttCompositeMaterial {
    #[uniform(0)]
    pub params: RttCompositeParams,
    #[texture(1)]
    #[sampler(2)]
    pub scene_texture: Handle<Image>,
    #[texture(3)]
    #[sampler(4)]
    pub soul_mask_texture: Handle<Image>,
}

impl Material2d for LocalRttCompositeMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/rtt_composite_material.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

// ─── マーカーコンポーネント ───────────────────────────────────────────────────

#[derive(Component)]
pub struct LocalRttComposite;
#[derive(Component)]
pub struct Camera3dSoulMaskTest;
#[derive(Component)]
pub struct Camera3dRtt;
#[derive(Component)]
pub struct TestMainCamera;

// ─── コンポーネント / リソース ────────────────────────────────────────────────

#[derive(Component)]
pub struct TestSoulConfig {
    pub face_mat: Handle<CharacterMaterial>,
    pub body_mat: Handle<CharacterMaterial>,
    pub index: usize,
}

#[derive(Component)]
pub struct SoulMaskConfig {
    pub mask_mat: Handle<SoulMaskMaterial>,
}

#[derive(Component)]
pub struct SoulShadowConfig {
    pub shadow_mat: Handle<SoulShadowMaterial>,
}

#[derive(Component)]
pub struct SoulBlobShadowProxy3d {
    pub owner: Entity,
}

#[derive(Component)]
pub struct SoulAnimHandle {
    pub anim_player_entity: Entity,
    pub clips: Vec<(&'static str, AnimationNodeIndex)>,
    pub current_playing: usize,
}

#[derive(Resource)]
pub struct TestAssets {
    pub soul_scene: Handle<WorldAsset>,
    pub face_atlas: Handle<Image>,
    pub white_pixel: Handle<Image>,
    pub gltf_handle: Handle<Gltf>,
    pub blob_shadow_mesh: Handle<Mesh>,
    pub blob_shadow_material: Handle<StandardMaterial>,
    pub soul_shadow_material: Handle<SoulShadowMaterial>,
    pub soul_mask_material: Handle<SoulMaskMaterial>,
}

// ─── クエリ型エイリアス ───────────────────────────────────────────────────────

pub type AnimPlayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut AnimationPlayer,
        &'static mut AnimationTransitions,
    ),
>;

#[derive(SystemParam)]
pub struct SoulLayoutEntities<'w, 's> {
    pub shadow_proxies: Query<'w, 's, Entity, With<SoulShadowProxy3d>>,
    pub blob_shadow_proxies: Query<'w, 's, Entity, With<SoulBlobShadowProxy3d>>,
    pub mask_proxies: Query<'w, 's, Entity, With<SoulMaskProxy3d>>,
}

pub type Cam3dSyncQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut Transform, &'static mut Projection),
    Or<(With<Camera3dRtt>, With<Camera3dSoulMaskTest>)>,
>;

pub type Cam2dQuery<'w, 's> = Query<
    'w,
    's,
    &'static Transform,
    (
        With<TestMainCamera>,
        Without<Camera3dRtt>,
        Without<Camera3dSoulMaskTest>,
    ),
>;
