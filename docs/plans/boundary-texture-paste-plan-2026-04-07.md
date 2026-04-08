# 境界テクスチャ貼り付け計画（terrain_region_map）

## 問題と目標

**問題**: リボンメッシュ（`BoundarySurfaceMaterial`, 幅 48wu）が Z-fighting・自己交差・アルファ二重ブレンドを引き起こす。

**目標**: リボンメッシュを廃止し、CPU でラスタライズした `terrain_region_map` により、テクスチャの切り替わりが有機的な Catmull-Rom 曲線に沿うようにする。ブレンドは**行わない**（境界で厳密に切り替える）。

## 現状（実装開始前の確認済み事実）

- **WGSL**: binding 129/130 (`boundary_mask` / `boundary_mask_sampler`) と `world_to_boundary_uv` は**すでに宣言済み**。ただし現在は float ブレンドウェイトとして使用している。
- **Rust 側**: `TerrainSurfaceMaterialExt` に `boundary_mask` フィールドは**まだ存在しない**（最終フィールドは `terrain_feature_lut` at 127/128）。
- **`BoundaryKind`**: `GrassZoneTone` / `DirtZoneTone` は存在するが `SandZoneTone` は**まだ存在しない**。
- **`extract_boundary_edges`**: `maybe_zone_tone_edge` は Grass/Dirt のみ対応。Sand の shore/inland 境界は**まだ未対応**。
- **`terrain_zone_bias_byte`**: `boundary.rs` に実装済み（`grass_zone_mask` / `dirt_zone_mask` → 0/128/255）。
- **`terrain_sand_variant_byte`**: 同様の関数は**まだ存在しない**（新規追加が必要）。
- **`Image::new` パターン**: `crates/bevy_app/src/world/map/terrain_metadata.rs` の `build_terrain_id_map` が R8Unorm + nearest のテンプレートとして使える。
- **`BitGrid::get(pos: GridPos)`**: `GridPos = (i32, i32)` — 既存コード通り `(gx, gy)` タプルを渡す。
- **`MAP_WIDTH = 100, MAP_HEIGHT = 100`**: `hw_core::constants` より（TERRAIN_REGION_RES=512 で 1 タイル ≈ 5.12px）。

## アーキテクチャ概要

```
BoundaryPolyline (Catmull-Rom 済み)
  └─ rasterize_terrain_regions()
        1. タイルマップで初期化 (512×512, u8)
        2. ポリラインを「壁」としてラスタライズ (sentinel=254)
        3. dilation で sentinel を隣接地形IDで上書き（数パス）
        └→ terrain_region_map: Handle<Image> (R8Unorm, 512×512)

TerrainSurfaceMaterialExt
  └─ boundary_mask (binding 129/130) ← terrain_region_map を設定

WGSL: blend_terrain()
  └─ boundary_mask テクスチャから地形IDを取得して直接 sample_surface_color() へ
     （異カテゴリブレンドは廃止、亜種ブレンドのみ残す）
```

## アルゴリズム: Polyline Barrier + Dilation

ポリゴン構成は不要。境界曲線を「壁」として描き、dilation で埋めるだけ。

### エンコーディング（zone bias + sand variant を含む）

`terrain_id_map` の粗い 4 値エンコーディングと異なり、`terrain_region_map` は  
Grass/Dirt の zone bias および Sand の shore/inland 区別も含む **11 値** エンコーディングを使う。

| TerrainType | variant | u8 値 | raw (÷255) | 粗い ID | 上書きするfeatureフィールド |
|:---|:---|:---:|:---:|:---:|:---|
| Grass  | grass zone (0)  | 0   | 0.000 | 0 | feature.a |
| Grass  | neutral (128)   | 1   | 0.004 | 0 | feature.a |
| Grass  | dirt zone (255) | 2   | 0.008 | 0 | feature.a |
| Dirt   | grass zone (0)  | 85  | 0.333 | 1 | feature.a |
| Dirt   | neutral (128)   | 86  | 0.337 | 1 | feature.a |
| Dirt   | dirt zone (255) | 87  | 0.341 | 1 | feature.a |
| Sand   | regular         | 170 | 0.667 | 2 | feature.r, feature.g |
| Sand   | shore           | 171 | 0.671 | 2 | feature.r, feature.g |
| Sand   | inland          | 172 | 0.675 | 2 | feature.r, feature.g |
| River  | —               | 255 | 1.000 | 3 | — |
| Sentinel | —             | 254 | —     | — | (最終テクスチャに含めない) |

**粗い ID の復元**: `u32(round(raw * 3.0))`  
- Grass: 0/1/2 → すべて 0  
- Dirt: 85/86/87 → すべて 1  
- Sand: 170/171/172 → すべて 2（round(0.667~0.675 × 3) = round(2.0~2.024) = 2）  
- River: 255 → 3

### なぜこのエンコーディングで各ゾーン境界が自動的に動くか

初期化時に各タイルを variant 込みのバイトで塗ることで、  
対応する境界ポリライン（GrassZoneTone/DirtZoneTone/**SandZoneTone**）が「壁」として入ると、  
dilation が各タイルの variant バイトを自然に伝播する。  
ポリライン側に left/right variant 情報を追加する必要はない。

**SandZoneTone について**:  
現在 `BoundaryKind` に Sand の variant 境界は存在しない。  
このため **M1 で `SandZoneTone` を追加する**（shore ↔ inland/regular の境界をポリラインとして生成）。

### 座標変換（CPU側）

```
世界座標 (world_x, world_y) → ピクセル座標 (px, py)
  px = (world_x + half_w) / world_w * RES
  py = (-world_y + half_h) / world_h * RES   ← Y 反転（Vec2.y = world_Z）
```

`world_to_boundary_uv` (WGSL) と同じ変換。

### 処理手順

```
Step 1. タイルIDで初期化（zone bias 込み）
  for each tile (tx, ty):
    id_byte = terrain_region_byte(terrain_tiles[ty * MAP_WIDTH + tx], masks, (tx, ty))
    // terrain_region_byte: TerrainType + WorldMasks → 0/1/2/85/86/87/170/255
    // 対応するピクセル矩形を id_byte で塗りつぶす
    px_start = tx * RES / MAP_WIDTH
    px_end   = (tx + 1) * RES / MAP_WIDTH
    py_start = ty * RES / MAP_HEIGHT
    py_end   = (ty + 1) * RES / MAP_HEIGHT

Step 2. ポリラインを sentinel=254 で上書き
  for each BoundaryPolyline (Catmull-Rom 済みの sampled 点列):
    for each consecutive pair (p[i], p[i+1]):
      Bresenham line の各ピクセル (px, py) に sentinel=254 を設定
      // 2px 幅: 水平線なら上下、垂直線なら左右の隣接ピクセルも sentinel に設定

Step 2.5. 非 junction 開チェーン端点に sentinel blob を描画（ギャップ閉鎖）
  for each open chain endpoint e where original_grid_corner_key NOT in junctions:
    (px, py) = world_to_pixel(e.displaced_world_pos)
    fill circle of radius 3px around (px, py) with sentinel=254
  // 根拠: 隣接する2チェーンの端点が独立したノイズで変位するため、
  //       最大ギャップ = 2 × NOISE_AMPLITUDE = 2 × 12wu ≈ 4px。
  //       radius=3px なら重複幅 = 2×3-4 = 2px > 0 が保証される。
  // 注意: junction 点（degree≥3）はすべてのチェーンで変位ゼロが保証されているため不要。

Step 3. Dilation で sentinel を消去
  max_passes = 10
  for pass in 0..max_passes:
    changed = false
    for each pixel (px, py) where value == SENTINEL:
      // 4-neighbor で最初に見つかった非 sentinel の値を採用
      if any 4-neighbor != SENTINEL:
        value = that neighbor's value
        changed = true
    if !changed: break

// assert: sentinel ピクセルが残っていないこと（デバッグ用）
```

### 設計上の根拠

- **BFS flood fill を使わない理由**: タイルグリッドで初期化済みなので、sentinel（壁）以外のピクセルは既にタイルIDが入っている。dilation は「壁を隣の地形で埋める」だけでよく、flood fill は不要。
- **sentinel=254 の理由**: River の 255 と区別するため。最終テクスチャに含めないので WGSL の decode には影響しない。
- **解像度 512×512 の理由**: 1タイル ≈ 5.12px。Catmull-Rom 点列の間隔（≈0.64px）に対して十分な密度。

---

## マイルストーン

### M1: CPU ラスタライズ（bevy_app/boundary.rs）

**ファイル**: `crates/bevy_app/src/world/map/boundary.rs`

#### 追加する import

既存の `use hw_core::constants::{...}` に追加:
```rust
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use crate::plugins::startup::visual_handles::{Terrain3dHandles, TerrainSurfaceMaterial};
```
削除する import:
```rust
// 削除:
use hw_visual::{BoundarySurfaceMaterialExt, BoundarySurfaceUniform, make_boundary_surface_material};
```

#### 追加する Resource

```rust
/// CPU ベイクした terrain_region_map テクスチャハンドルを保持する。
#[derive(Resource)]
pub struct BoundaryRegionMap {
    pub image: Handle<Image>,
}
```

#### 追加する定数と関数

```rust
const TERRAIN_REGION_RES: usize = 512;
const TERRAIN_REGION_SENTINEL: u8 = 254;

/// Sand タイルの shore/inland variant バイトを返す。
/// shore=171, inland=172, regular=170（`terrain_zone_bias_byte` の Sand 版）。
#[inline]
fn terrain_sand_variant_byte(masks: &WorldMasks, pos: (i32, i32)) -> u8 {
    let is_final = masks.final_sand_mask.get(pos);
    let is_inland = masks.inland_sand_mask.get(pos);
    if is_final && !is_inland { 171 }  // shore
    else if is_inland          { 172 }  // inland
    else                       { 170 }  // regular
}

/// TerrainType + WorldMasks + タイル座標 → terrain_region_map 用バイト（11 値エンコーディング）。
fn terrain_region_byte(t: TerrainType, masks: &WorldMasks, pos: (i32, i32)) -> u8 {
    match t {
        TerrainType::Grass => match terrain_zone_bias_byte(masks, pos) {
            0   => 0,    // grass zone
            255 => 2,    // dirt zone
            _   => 1,    // neutral
        },
        TerrainType::Dirt => match terrain_zone_bias_byte(masks, pos) {
            0   => 85,
            255 => 87,
            _   => 86,
        },
        TerrainType::Sand => terrain_sand_variant_byte(masks, pos),
        TerrainType::River => 255,
    }
}

/// Bresenham ライン（整数ピクセル）を sentinel で 2px 幅で塗りつぶす。
///
/// p0/p1 はピクセル座標（浮動小数点 → 整数切り捨て）。
/// 水平線: 上下各 1px を sentinel にする。垂直線: 左右各 1px を sentinel にする。
fn rasterize_segment_barrier(buf: &mut [u8], res: usize, p0: (f32, f32), p1: (f32, f32)) {
    // Bresenham で各ピクセルを sentinel=254 に設定（2px 幅）
    // ...
}

/// タイルマップ + ポリライン点列群から terrain_region_map バッファ（RES×RES, u8）を生成。
///
/// 1. タイル ID (variant 込み) で初期化
/// 2. 全ポリライン点列を sentinel=254 で「壁」として描画
/// 3. 非 junction 開端点に radius=3px の sentinel blob を描画（ギャップ閉鎖）
/// 4. Dilation で sentinel を隣接地形バイトで上書き
fn rasterize_terrain_regions(
    terrain_tiles: &[TerrainType],
    masks: &WorldMasks,
    sampled_polylines: &[Vec<Vec2>],
    endpoint_blobs: &[Vec2],
) -> Vec<u8> {
    // ... 詳細は Step 1〜3 参照
}
```

#### BoundaryKind に SandZoneTone を追加

```rust
pub enum BoundaryKind {
    GrassDirt,
    GrassSand,
    GrassRiver,
    DirtSand,
    DirtRiver,
    SandRiver,
    GrassZoneTone,
    DirtZoneTone,
    SandZoneTone,  // ← 新規追加: shore ↔ inland/regular の境界
}
```

`extract_boundary_edges` に Sand の shore/inland 境界検出を追加  
（水平エッジループと垂直エッジループの両方に、`maybe_zone_tone_edge` の else if ブロックの後に追加）:

> **注意**: `zone_tone_boundary_kind` / `maybe_zone_tone_edge` は変更しない。  
> `maybe_zone_tone_edge` は `both_grass || both_dirt` でガードしており Sand は弾かれるため、  
> Sand 境界は以下の独立した `else if` ブロックで直接検出する。

```rust
// Sand 同士かつ shore/inland variant が異なる場合
} else if t0 == TerrainType::Sand
    && t1 == TerrainType::Sand
    && terrain_sand_variant_byte(masks, (gx, gy)) != terrain_sand_variant_byte(masks, (gx, gy + 1))
{
    let center = grid_to_world(gx, gy);
    edges.push(BoundaryEdge {
        a: Vec2::new(center.x - half, center.y + half),
        b: Vec2::new(center.x + half, center.y + half),
        kind: BoundaryKind::SandZoneTone,
        left_terrain: t1,
        right_terrain: t0,
    });
}
// （垂直ループ側も同様: gy→gy, gx→gx+1, a/b の座標は垂直境界用に合わせる）
```

#### spawn_boundary_meshes の変更

**パラメータ変更**:

| 変更種別 | パラメータ |
|:---:|:---|
| 削除 | `mut meshes: ResMut<Assets<Mesh>>` |
| 削除 | `mut boundary_materials: ResMut<Assets<hw_visual::BoundarySurfaceMaterial>>` |
| 削除 | `game_assets: Res<GameAssets>` |
| 削除 | `feature_map: Res<TerrainFeatureMap>` |
| 追加 | `mut images: ResMut<Assets<Image>>` |
| 追加 | `terrain_handles: Res<Terrain3dHandles>` |
| 追加 | `mut terrain_surface_materials: ResMut<Assets<TerrainSurfaceMaterial>>` |

**ループ内で削除するコード**:
- `kind_material_cache` HashMap の定義
- `let add_start_cap` / `let add_end_cap` ブロック
- `build_quad_strip_mesh(...)` 呼び出し
- `kind_material_cache.entry(...).or_insert_with(|| { ... })` ブロック
- `commands.spawn((Mesh3d(mesh_handle), MeshMaterial3d(...), ...))` ブロック

**ループ内で追加するコード** (蓄積):
```rust
let mut sampled_polylines: Vec<Vec<Vec2>> = Vec::new();
let mut endpoint_blobs: Vec<Vec2>         = Vec::new();

for polyline in polylines {
    // ... 既存の displaced/chamfered/sampled 計算はそのまま ...
    sampled_polylines.push(sampled.clone());

    // 非 junction 開端点を endpoint_blobs に追加
    if !polyline.is_closed {
        if !polyline.points.is_empty()
            && !junctions.contains(&world_to_corner_key(polyline.points[0]))
        {
            endpoint_blobs.push(sampled[0]);
        }
        if polyline.points.len() > 1
            && !junctions.contains(&world_to_corner_key(*polyline.points.last().unwrap()))
        {
            endpoint_blobs.push(*sampled.last().unwrap());
        }
    }
}
```

**ループ後に追加するコード** (ラスタライズ → テクスチャ作成 → マテリアル設定):
```rust
let buf = rasterize_terrain_regions(
    terrain_tiles,
    &layout.layout.masks,
    &sampled_polylines,
    &endpoint_blobs,
);

// terrain_metadata.rs の build_terrain_id_map と同一パターン
let mut image = Image::new(
    Extent3d {
        width:                 TERRAIN_REGION_RES as u32,
        height:                TERRAIN_REGION_RES as u32,
        depth_or_array_layers: 1,
    },
    TextureDimension::D2,
    buf,
    TextureFormat::R8Unorm,
    default(),
);
image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
    address_mode_u: ImageAddressMode::ClampToEdge,
    address_mode_v: ImageAddressMode::ClampToEdge,
    mag_filter:     ImageFilterMode::Nearest,
    min_filter:     ImageFilterMode::Nearest,
    ..default()
});

let handle = images.add(image);

// TerrainSurfaceMaterial は ExtendedMaterial<StandardMaterial, TerrainSurfaceMaterialExt>。
// .extension.boundary_mask でアクセスする（.base ではない）。
if let Some(mat) = terrain_surface_materials.get_mut(&terrain_handles.surface) {
    mat.extension.boundary_mask = Some(handle.clone());
}

commands.insert_resource(BoundaryRegionMap { image: handle });
```

---

### M2: Rust マテリアルバインディング追加（hw_visual）

**ファイル**: `crates/hw_visual/src/material/terrain_surface_material.rs`

`TerrainSurfaceMaterialExt` の末尾（`terrain_feature_lut` の直後）に追加:

```rust
// 既存（変更なし）
#[texture(127)]
#[sampler(128)]
pub terrain_feature_lut: Option<Handle<Image>>,

// ↓ 追加
#[texture(129)]
#[sampler(130)]
pub boundary_mask: Option<Handle<Image>>,
```

`Default` impl（`default()` マクロを使っている場合は derive で OK、手書きの場合は追加）:
```rust
boundary_mask: None,
```

---

**ファイル**: `crates/bevy_app/src/plugins/startup/visual_handles.rs`

`TerrainSurfaceMaterialExt { ... }` 初期化ブロックに追加（`terrain_feature_lut: Some(...)` の後）:

```rust
terrain_feature_lut: Some(game_assets.terrain_feature_lut.clone()),
boundary_mask: None,  // ← spawn_boundary_meshes (PostStartup) で後から設定される
```

---

### M3: シェーダー修正（terrain_surface_material.wgsl）

**ファイル**: `assets/shaders/terrain_surface_material.wgsl`

#### 変更 1: region → feature フィールド復元関数群を追加

terrain_region_map の raw_byte から各 feature フィールドを高解像度で復元する。

```wgsl
/// zone_bias を復元（Grass/Dirt 用）。feature.a の高解像度版。
fn region_to_zone_bias(raw_byte: u32) -> f32 {
    switch raw_byte {
        case 0u:  { return 0.0; }    // Grass + grass zone
        case 1u:  { return 0.502; }  // Grass + neutral (128/255)
        case 2u:  { return 1.0; }    // Grass + dirt zone
        case 85u: { return 0.0; }    // Dirt + grass zone
        case 86u: { return 0.502; }  // Dirt + neutral
        case 87u: { return 1.0; }    // Dirt + dirt zone
        default:  { return 0.502; }  // Sand/River: neutral
    }
}

/// shore sand フラグを復元（Sand 用）。feature.r の高解像度版。
fn region_to_shore_sand(raw_byte: u32) -> f32 {
    if raw_byte == 171u { return 1.0; }  // shore
    return 0.0;
}

/// inland sand フラグを復元（Sand 用）。feature.g の高解像度版。
fn region_to_inland_sand(raw_byte: u32) -> f32 {
    if raw_byte == 172u { return 1.0; }  // inland
    return 0.0;
}
```

#### 変更 2: blend_terrain の書き換え

**現状**: `boundary_w = textureSample(...).r` を float ブレンドウェイトとして使い、`should_blend_pair` で隣接 ID をブレンドしている。

**変更後**: `boundary_mask` から地形 ID と variant を取得して `sample_surface_color` へ直接渡す。異カテゴリブレンドは廃止。亜種ブレンド（`id_c == id_n && raw_c != raw_n`）はそのまま残す。

```wgsl
fn blend_terrain(world_xz: vec2<f32>, cell: vec2<i32>, feature: vec4<f32>) -> vec3<f32> {
    // terrain_region_map から有機的な地形 ID と variant を取得
    let raw_region = textureSample(
        boundary_mask, boundary_mask_sampler, world_to_boundary_uv(world_xz)
    ).r;
    let id_region      = u32(round(raw_region * 3.0));
    let raw_byte_region = u32(round(raw_region * 255.0));

    // feature の各フィールドを高解像度 region 値で上書き
    var feature_r = feature;
    feature_r.a = region_to_zone_bias(raw_byte_region);    // Grass/Dirt zone bias
    feature_r.r = region_to_shore_sand(raw_byte_region);   // Sand: shore
    feature_r.g = region_to_inland_sand(raw_byte_region);  // Sand: inland

    // 亜種ブレンド用（terrain_id_map から粗い ID と亜種バイト）
    let id_c = cell_terrain_id(cell);
    let raw_c = cell_terrain_raw_byte(cell);
    let cell_local = world_to_cell_local(world_xz);
    let mask = textureSample(terrain_blend_mask_soft, blend_mask_sampler, cell_local).r;

    let w_n = narrow_edge_weight_towards_low(cell_local.y);
    let w_s = narrow_edge_weight_towards_high(cell_local.y);
    let w_e = narrow_edge_weight_towards_high(cell_local.x);
    let w_w = narrow_edge_weight_towards_low(cell_local.x);

    // メインサンプル: id_region と raw_byte_region（region map の variant）を使う。
    // raw_byte_region を渡すことで、variant_luma_mul が境界をまたいでも正しい色調になる。
    var accum = sample_surface_color(id_region, world_xz, feature_r, raw_byte_region);
    var accum_weight = 1.0;

    let neighbor_cells = array<vec2<i32>, 4>(
        clamp_cell(cell + vec2<i32>(0, -1)),
        clamp_cell(cell + vec2<i32>(0, 1)),
        clamp_cell(cell + vec2<i32>(1, 0)),
        clamp_cell(cell + vec2<i32>(-1, 0)),
    );
    let weights = array<f32, 4>(w_n, w_s, w_e, w_w);

    // 亜種ブレンドループ: terrain_id_map は現在 4 値（Grass=0, Dirt=85, Sand=170, River=255）のみ。
    // 同種タイル間で raw_c != raw_n は発火しないため、このループは現状 no-op。
    // 将来 terrain_id_map に亜種バイトを持たせた際に有効になる予定（削除しない）。
    for (var i = 0u; i < 4u; i++) {
        let id_n  = cell_terrain_id(neighbor_cells[i]);
        let raw_n = cell_terrain_raw_byte(neighbor_cells[i]);
        if id_c == id_n && (id_c == 0u || id_c == 1u) && raw_c != raw_n {
            let wn = clamp(weights[i] * mask * tsm.blend_strength * 0.42, 0.0, 1.0);
            if wn > 0.0 {
                accum += sample_surface_color(id_n, world_xz, feature_r, raw_n) * wn;
                accum_weight += wn;
            }
        }
    }

    return accum / accum_weight;
}
```

> **注意: `raw_c` と `raw_byte_region` の使い分け**
> - メインサンプルと異カテゴリブレンド撤廃後: `raw_byte_region`（region map 由来, 11 値エンコーディング）を使う。
> - 亜種ブレンドループの隣接セルサンプル: `raw_n`（id_map 由来, 各セルの粗い亜種バイト）を使う（現状維持）。

#### 変更 3: should_blend_pair 関数を削除

```wgsl
// 削除: fn should_blend_pair(id_a: u32, id_b: u32) -> bool { ... }
```

---

### M4: リボンメッシュ廃止とクリーンアップ

**ファイル**: `crates/bevy_app/src/world/map/boundary.rs`

- `spawn_boundary_meshes` からリボンメッシュスポーンコードを削除（M1 で実施済み）
- `BoundaryMeshMarker` コンポーネントの削除（スポーンされなくなるので不要）
- 使われなくなる関数を削除:
  - `build_quad_strip_mesh`
  - `miter_pair`
- `RenderLayers` / `Mesh3d` / `MeshMaterial3d` など ribbon 専用 import を削除（他で使われていない場合）

**ファイル**: `crates/hw_visual/src/material/boundary_surface_material.rs`
- ファイルごと削除

**ファイル**: `crates/hw_visual/src/material/mod.rs`
- `pub mod boundary_surface_material;` 行を削除
- `pub use boundary_surface_material::{...}` 行を削除

**ファイル**: `crates/hw_visual/src/lib.rs`
- `MaterialPlugin::<material::BoundarySurfaceMaterial>::default()` の登録を削除
- `BoundarySurfaceMaterial` / `BoundarySurfaceMaterialExt` / `BoundarySurfaceUniform` / `make_boundary_surface_material` の re-export を削除（ll.37, 44 付近）

---

## 変更ファイル一覧

| ファイル | 変更種別 | 主な内容 |
|:---|:---:|:---|
| `crates/bevy_app/src/world/map/boundary.rs` | 変更 | `SandZoneTone` 追加, `terrain_sand_variant_byte` 新規, `rasterize_terrain_regions()` 新規, `BoundaryRegionMap` Resource, メッシュスポーン削除, `build_quad_strip_mesh`/`miter_pair` 削除 |
| `crates/bevy_app/src/plugins/startup/visual_handles.rs` | 変更 | `boundary_mask: None` を初期化ブロックに追加 |
| `crates/hw_visual/src/material/terrain_surface_material.rs` | 変更 | `#[texture(129)] #[sampler(130)] pub boundary_mask: Option<Handle<Image>>` と `Default` impl 追加 |
| `crates/hw_visual/src/material/boundary_surface_material.rs` | 削除 | リボンメッシュマテリアル廃止 |
| `crates/hw_visual/src/material/mod.rs` | 変更 | `boundary_surface_material` mod・re-export 削除 |
| `crates/hw_visual/src/lib.rs` | 変更 | `MaterialPlugin::<BoundarySurfaceMaterial>` 登録と re-export 削除 |
| `assets/shaders/terrain_surface_material.wgsl` | 変更 | `region_to_*` 関数追加, `blend_terrain` 書き換え, `should_blend_pair` 削除 |

---

## 注意事項・トラップ

### 座標系（最重要）

- `BoundaryPolyline::points` の `Vec2.y` = **ワールド Z 軸**
- CPU ラスタライズ時の Y 反転:
  ```rust
  py = (-point.y + half_h) / world_h * RES as f32
  ```
  これは `world_to_boundary_uv` の `-world_xz.y` と対称。

### terrain_id_map は残す

- binding 101 `terrain_id_map` は `cell_terrain_raw_byte()` で亜種情報として引き続き使用
- 廃止しない

### zone_bias の高解像度化は terrain_region_map 1 枚で完結する

- `terrain_feature_map.a`（100×100）は **使い続ける**（`sample_feature(cell)` で取得）
- `blend_terrain` の `feature_r.a` を `region_to_zone_bias(raw_byte_region)` で上書きするだけ
- 追加テクスチャ不要、追加バインディング不要

### terrain_region_byte が WorldMasks を必要とする

- `rasterize_terrain_regions` の引数に `&layout.layout.masks: &WorldMasks` を追加
- `spawn_boundary_meshes` はすでに `layout: Res<GeneratedWorldLayoutResource>` を受け取っており、`&layout.layout.masks` で渡せる

### boundary_mask binding はシェーダーに既にある

- WGSL には binding 129/130 が宣言済み、`world_to_boundary_uv` も実装済み
- Rust 側（`TerrainSurfaceMaterialExt`）に `#[texture(129)] #[sampler(130)]` を追加するだけ

### 非 junction 開チェーン端点のギャップ閉鎖（Step 2.5 の背景）

- `displace_polyline` は `junctions` に含まれる点のみ変位をゼロに抑制する。
- open chain の端点は junction でない限り通常通り変位する。
- 異種チェーン（例: GrassZoneTone と CoarseType=Grass/Dirt）の端点が同じグリッド角に収束する場合、
  それぞれ独立したノイズで変位するため最大ギャップ = 2 × 12wu ≈ 4px が生じる。
- sentinel blob（radius=3px）は step 2 の壁ラスタライズ後に追加するため、
  dilation がギャップを埋める前にその領域を sentinel で封鎖できる。
- `chamfer_polyline_points` は open endpoint を chamfer しないが変位は抑制しない。
  blob の対象は変位**後**のワールド座標（= sampled 点列の先頭/末尾）を使う。

### テクスチャサンプラー

- 補間によるアーティファクト（中間 ID が出る）を防ぐため **Nearest** フィルタリングを設定する
- パターンは `terrain_metadata.rs` の `build_terrain_id_map` と完全に同じ:
  ```rust
  image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
      address_mode_u: ImageAddressMode::ClampToEdge,
      address_mode_v: ImageAddressMode::ClampToEdge,
      mag_filter:     ImageFilterMode::Nearest,
      min_filter:     ImageFilterMode::Nearest,
      ..default()
  });
  ```
- 使用する import（`terrain_metadata.rs` と同じ）:
  ```rust
  use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
  use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
  ```

### TerrainSurfaceMaterial のバインド型

- `TerrainSurfaceMaterial = ExtendedMaterial<StandardMaterial, TerrainSurfaceMaterialExt>`
- `terrain_surface_materials.get_mut(&handle)` で返るのは `&mut ExtendedMaterial`
- `boundary_mask` は `.extension.boundary_mask` でアクセス（`.base` ではない）

### spawn_boundary_meshes の実行タイミング

- PostStartup 実行
- `Terrain3dHandles.surface` は Startup 時に既に作成済み（`visual_handles.rs`）
- PostStartup から `terrain_surface_materials.get_mut(&terrain_handles.surface)` で取得可能
- `visual_handles.rs` で `boundary_mask: None` に初期化しておき、`spawn_boundary_meshes` で `Some(handle)` に上書きする

---

## 検証コマンド

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace
```

**目視確認**:
1. 地形境界線がタイルグリッドではなく有機的な Catmull-Rom 曲線に沿っている
2. 境界での Z-fighting が解消（半透明リボンが消えている）
3. 境界でのテクスチャ切り替わりが sharp（ブレンドなし）
4. 川・砂・草・土の各境界が正しく描画されている
5. **草 grass zone（緑が濃い）と grass neutral zone との境界が曲線に沿っている**
6. **土 dirt zone（茶色が濃い）と dirt neutral zone との境界が曲線に沿っている**
