//! マップスポーン

use crate::plugins::startup::Terrain3dHandles;
use bevy::prelude::*;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH, TILE_SIZE, building_3d_render_layers};
use hw_visual::TerrainSurfaceMaterial;
use hw_world::{GeneratedWorldLayout, generate_world_layout, grid_to_world};

use super::{Tile, WorldMapWrite};

const WORLDGEN_SEED_ENV: &str = "HELL_WORKERS_WORLDGEN_SEED";

/// 地形描画 chunk のサイズ（タイル数/辺）
const CHUNK_TILES: i32 = 16;

/// 地形描画 chunk entity に付与するマーカーコンポーネント。
///
/// 1 つの chunk が `CHUNK_TILES × CHUNK_TILES` タイル分の平面 mesh を担う。
/// 100×100 マップでは 7×7 = 49 chunk entity が生成される。
#[derive(Component)]
pub struct TerrainChunk {
    pub cx: i32,
    pub cy: i32,
}

#[derive(Resource, Clone)]
pub struct GeneratedWorldLayoutResource {
    pub master_seed: u64,
    pub layout: GeneratedWorldLayout,
}

pub fn resolve_worldgen_seed() -> u64 {
    match std::env::var(WORLDGEN_SEED_ENV) {
        Ok(raw) => match raw.parse::<u64>() {
            Ok(seed) => seed,
            Err(err) => {
                warn!(
                    "BEVY_STARTUP: invalid {}='{}' ({err}); falling back to random seed",
                    WORLDGEN_SEED_ENV, raw
                );
                rand::random::<u64>()
            }
        },
        Err(_) => rand::random::<u64>(),
    }
}

pub fn prepare_generated_world_layout_resource() -> GeneratedWorldLayoutResource {
    let master_seed = resolve_worldgen_seed();
    let layout = generate_world_layout(master_seed);
    GeneratedWorldLayoutResource {
        master_seed,
        layout,
    }
}

/// `WorldMap.tile_entities` に登録される論理 anchor entity を生成する。
///
/// 各 `Tile` entity は描画コンポーネント（`Mesh3d` / `MeshMaterial3d`）を持たない。
/// 地形描画は `spawn_terrain_chunks` が担う `TerrainChunk` entity が行う。
///
/// `Tile` entity は Familiar AI（`direct_collect.rs`）が `Designation` / `TaskWorkers` を
/// 参照する際の lookup anchor として存続する。`Transform` は `DesignationSpatialGrid` と
/// UI/選択系 Query が依存するため必須。
pub fn spawn_map(
    mut commands: Commands,
    mut world_map: WorldMapWrite,
    generated_layout: Res<GeneratedWorldLayoutResource>,
) {
    let terrain_tiles = &generated_layout.layout.terrain_tiles;

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = world_map
                .pos_to_idx(x, y)
                .expect("x/y within MAP_WIDTH x MAP_HEIGHT");
            let terrain = terrain_tiles[idx];
            world_map.set_terrain_at_idx(idx, terrain);

            let pos2d = grid_to_world(x, y);
            let entity = commands
                .spawn((
                    Tile,
                    Transform::from_xyz(pos2d.x, 0.0, -pos2d.y),
                ))
                .id();

            world_map.set_tile_entity_at_idx(idx, entity);
        }
    }

    info!(
        "BEVY_STARTUP: Map tile anchors spawned ({}x{} tiles, worldgen seed={}, attempt={}, fallback={})",
        MAP_WIDTH,
        MAP_HEIGHT,
        generated_layout.master_seed,
        generated_layout.layout.generation_attempt,
        generated_layout.layout.used_fallback
    );
}

/// 地形描画 chunk entity を生成する。
///
/// 1 chunk = `CHUNK_TILES × CHUNK_TILES` タイルの `Plane3d` mesh として spawn する。
/// 100×100 マップでは 7×7 = 49 entity が生成される（辺端は 4 tile 幅の端数 chunk）。
///
/// shader は world-space UV で `terrain_id_map` / `terrain_feature_map` を参照するため、
/// chunk 境界での継ぎ目は発生しない。
pub fn spawn_terrain_chunks(
    mut commands: Commands,
    terrain_handles: Res<Terrain3dHandles>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let chunks_x = (MAP_WIDTH + CHUNK_TILES - 1) / CHUNK_TILES;
    let chunks_y = (MAP_HEIGHT + CHUNK_TILES - 1) / CHUNK_TILES;

    for cy in 0..chunks_y {
        for cx in 0..chunks_x {
            // 端数 chunk の実タイル幅（辺端は MAP_WIDTH % CHUNK_TILES = 4 tile 幅）
            let w = ((cx + 1) * CHUNK_TILES).min(MAP_WIDTH) - cx * CHUNK_TILES;
            let h = ((cy + 1) * CHUNK_TILES).min(MAP_HEIGHT) - cy * CHUNK_TILES;

            // chunk 内の最初と最後のタイル中心 world 座標から chunk 中心を算出する
            let origin = grid_to_world(cx * CHUNK_TILES, cy * CHUNK_TILES);
            let end = grid_to_world(cx * CHUNK_TILES + w - 1, cy * CHUNK_TILES + h - 1);
            let center = (origin + end) * 0.5;

            let chunk_mesh =
                meshes.add(Plane3d::default().mesh().size(w as f32 * TILE_SIZE, h as f32 * TILE_SIZE));

            commands.spawn((
                TerrainChunk { cx, cy },
                Mesh3d(chunk_mesh),
                MeshMaterial3d::<TerrainSurfaceMaterial>(terrain_handles.surface.clone()),
                Transform::from_xyz(center.x, 0.0, -center.y),
                building_3d_render_layers(),
            ));
        }
    }

    info!(
        "BEVY_STARTUP: Terrain chunks spawned ({}x{} chunks = {} entities, chunk_size={}x{})",
        chunks_x,
        chunks_y,
        chunks_x * chunks_y,
        CHUNK_TILES,
        CHUNK_TILES
    );
}
