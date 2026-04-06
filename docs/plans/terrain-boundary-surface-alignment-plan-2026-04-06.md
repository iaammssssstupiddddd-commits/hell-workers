# 地形境界面と塗り範囲の一致計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `terrain-boundary-surface-alignment-plan-2026-04-06` |
| ステータス | `Draft` |
| 作成日 | `2026-04-06` |
| 最終更新日 | `2026-04-06` |
| 作成者 | `Codex` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題:  
  現行地形描画では、CPU 側の境界曲線 ribbon mesh と、terrain shader 側の直線 edge band ブレンドが完全に独立したロジックで動いており、境界線の形状と実際にテクスチャが切り替わる範囲が一致しない。
  - CPU 側：`extract_boundary_edges → chain_edges_to_polylines → displace_polyline → sample_catmull_rom → build_quad_strip_mesh` で `STRIP_WIDTH = TILE_SIZE * 0.2 (6.4wu)` の Catmull-Rom ribbon を生成し、`StandardMaterial { base_color: kind.color(), unlit: true }` でフラットカラー描画している。
  - Shader 側：`terrain_surface_material.wgsl` の `blend_terrain()` が `blend_band = 0.16` (セル幅の 16% ≈ 5.12wu) の直線 edge band でブレンドしており、ribbon の曲線形状を一切参照しない。
- 到達したい状態:
  ribbon mesh 自体が「テクスチャブレンドを行う塗り面」として機能し、フラットカラー帯ではなく world-space テクスチャを left/right terrain でブレンドして描画する。terrain shader 側の edge band は ribbon で覆われる範囲を無効化（または大幅縮小）し、二重描画による色の濁りをなくす。
- 成功指標:
  1. 境界線と塗り面の輪郭が目視で一致する（ribbon の曲線が塗りの輪郭になる）。
  2. ribbon 内部でのテクスチャブレンドが world-space UV で行われ、アルベドの継ぎ目がない。
  3. 同一セルに複数境界が入るケースでも情報欠落なく描画できる（三叉路・近接境界）。
  4. `TerrainChangedEvent` 後も境界面と塗り範囲の整合が保たれる。

## 2. スコープ

### 対象（In Scope）

- `boundary.rs` の既存形状生成パイプラインに手を加えつつ、left/right terrain 情報と zone bias 情報を追跡する
- `BoundaryEdge` / `BoundaryPolyline` へ `left_terrain` / `right_terrain` と `left_zone_bias` / `right_zone_bias` フィールドを追加
- `build_quad_strip_mesh` に `ATTRIBUTE_UV_0` (u = cross-section, v = arc-length tiling) を追加
- `BoundarySurfaceMaterial` (新 Bevy `MaterialPlugin`) の設計と実装
- `assets/shaders/boundary_surface_material.wgsl` の設計と実装
- `spawn_boundary_meshes` を新 material を使うよう更新
- `terrain_surface_material.wgsl` の `blend_band` を縮小または無効化
- `BoundarySurface` / `BoundarySlice` / `BoundarySliceSpatialIndex` 型の定義（将来の terrain shader 連携用インデックス）
- 初期生成と `TerrainChangedEvent` 更新時の再構築方針
- 関連ドキュメント更新（`docs/world_layout.md`, `docs/architecture.md`）

### 非対象（Out of Scope）

- 既存地形アルベドアセットの差し替え
- 新しい brush / border 専用テクスチャアセットの追加
- 境界表現以外の地形 grade（shore / inland / rock field）の再設計
- 大域的な RtT パイプライン再編
- 最適化のみを目的とした GPU compute / 専用 render graph 化

## 3. 現状コードの精査

### 現行 CPU パイプライン（`boundary.rs`）

```
extract_boundary_edges
  → BoundaryEdge { a, b, kind }  ※ left/right terrain 未保持
chain_edges_to_polylines
  → BoundaryPolyline { points, arc_lengths, is_closed, kind }  ※ 同上
displace_polyline  (ノイズ変位)
sample_catmull_rom (CATMULL_ROM_STEPS=8)
build_quad_strip_mesh
  → Mesh { ATTRIBUTE_POSITION, ATTRIBUTE_NORMAL }  ※ UV なし
spawn_boundary_meshes
  → StandardMaterial { base_color: kind.color(), unlit: true }  ← フラットカラー
```

### 現行 Shader ブレンド（`terrain_surface_material.wgsl`）

- `blend_terrain()` 内で `narrow_edge_weight_towards_{low,high}()` が `blend_band = 0.16` の直線帯で隣接 terrain を加重ブレンド
- `terrain_blend_mask_soft` テクスチャをセル内 UV でサンプルして重みをマスク
- zone 境界は `feature.a` による palette bias として `grade_grass` / `grade_dirt` で処理

### ギャップ

| 項目 | CPU 境界 ribbon | Shader edge band |
|---|---|---|
| 形状 | Catmull-Rom 曲線、noise 変位あり | セルエッジに平行な直線 |
| 幅 | 6.4wu (= TILE_SIZE × 0.2) | ≈5.12wu (= TILE_SIZE × 0.16) |
| テクスチャ | フラットカラー | world-space UV でアルベドサンプル |
| 左右 terrain | 未追跡（BoundaryKind のみ） | terrain_id_map セルルックアップ |
| 連携 | なし | なし |

## 4. 実装方針

### アーキテクチャ決定

1. **ribbon mesh が唯一の塗り面**：ribbon mesh を「境界ブレンド帯の表示担体」にする。terrain shader の `blend_terrain()` が担っていた境界ブレンドは ribbon に移管する。異カテゴリ境界だけでなく zone tone 境界も最終的には同一経路へ寄せる。
2. **UV エンコーディング**：`build_quad_strip_mesh` に `ATTRIBUTE_UV_0` を追加。`u = 0.0` が ribbon の `+n2` (left) 辺、`u = 1.0` が `-n2` (right) 辺。`v` はアーク長ベースのタイリング (後述)。cap 頂点は `u=0.5`、弧端は `u=0.0 or 1.0`。
3. **left/right terrain のパイプライン追跡**：`BoundaryEdge` に `left_terrain / right_terrain` を追加し `extract_boundary_edges` で設定する。zone tone 境界用に `left_zone_bias / right_zone_bias` も保持する。`chain_edges_to_polylines` はエッジ向きの逆転を考慮して polarity を引き継ぐ。`spawn_boundary_meshes` で material 生成に使う。
4. **BoundarySurfaceMaterial**：`crates/hw_visual/src/material/boundary_surface_material.rs` に `ExtendedMaterial<StandardMaterial, BoundarySurfaceMaterialExt>` として定義する。extension の binding 番号は `100+` を使い、`StandardMaterial` 側 bind slot と衝突させない。Bevy の `MaterialPlugin::<BoundarySurfaceMaterial>::default()` は `HwVisualPlugin` に追加登録する。
5. **terrain shader の edge band 無効化**：`blend_band` を `0.0` に下げる（定数変更のみ）か、`should_blend_pair` が ribbon で既に処理されるペアは `false` を返すよう分岐させる。zone tone 境界（GrassZoneTone / DirtZoneTone）も ribbon 側で zone bias ブレンドを行う前提で、最終的には shader 側 palette bias の境界表現責務を外す。
6. **BoundarySliceSpatialIndex**：M2 で生成し、将来の「terrain shader が ribbon 幾何を参照する」経路のためにリソースとして保持する。M3 では直接使わないが設計は完成させる。

### 座標系確認

`grid_to_world(x, y)` は：
- `world.x = (x - (MAP_WIDTH-1)/2) * TILE_SIZE`
- `world.y = (y - (MAP_HEIGHT-1)/2) * TILE_SIZE`（グリッド y 増加 → world Y 増加）

`push_vertex_xz` は `[p.x, y_offset, -p.y]`（world Y → -Z）。

水平エッジ（`(x,y)` と `(x,y+1)` の境界、`a→b = +X`）：
- 左法線 = `+Y`、 左側（+Y 側）のセルが `(x, y+1)` = `t1` → `left_terrain = t1`、`right_terrain = t0`

垂直エッジ（`(x,y)` と `(x+1,y)` の境界、`a→b = +Y`）：
- 左法線 = `-X`、左側（-X 側）のセルが `(x, y)` = `t0` → `left_terrain = t0`、`right_terrain = t1`

エッジが `follow_chain` で逆向きに走査される場合、`left_terrain` と `right_terrain` を swap する。

## 5. マイルストーン

---

### M1: left/right terrain / zone bias 追跡と型定義

**目的**: パイプライン全体に left/right terrain / zone bias 情報を流し込み、将来の spatial index と shader の基盤を整える。

**変更内容**:

#### `BoundaryEdge`（`boundary.rs`）

```rust
pub struct BoundaryEdge {
    pub a: Vec2,
    pub b: Vec2,
    pub kind: BoundaryKind,
    /// a→b の +法線（左）側のセルの TerrainType
    pub left_terrain: TerrainType,
    /// a→b の -法線（右）側のセルの TerrainType
    pub right_terrain: TerrainType,
    /// zone tone 境界で使う左側 zone bias（草=0, 中立=128, 土=255）
    pub left_zone_bias: u8,
    /// zone tone 境界で使う右側 zone bias
    pub right_zone_bias: u8,
}
```

`extract_boundary_edges` 内で設定ルール:
- 水平エッジ: `left_terrain = t1 (y+1)`, `right_terrain = t0 (y)`
- 垂直エッジ: `left_terrain = t0 (x)`, `right_terrain = t1 (x+1)`

#### `BoundaryPolyline`（`boundary.rs`）

```rust
pub struct BoundaryPolyline {
    pub points: Vec<Vec2>,
    pub arc_lengths: Vec<f32>,
    pub is_closed: bool,
    pub kind: BoundaryKind,
    /// polyline の +n2（左）側の TerrainType（chain 内のエッジ極性を統一して引き継ぐ）
    pub left_terrain: TerrainType,
    /// polyline の -n2（右）側の TerrainType
    pub right_terrain: TerrainType,
    /// polyline 左右の zone bias
    pub left_zone_bias: u8,
    pub right_zone_bias: u8,
}
```

`chain_edges_to_polylines` 変更方針:
- `follow_chain` は各エッジを順方向・逆方向どちらで追加するかを判定しているため、エッジを正方向で追加する場合は `left = edge.left_terrain`、逆方向（b→a）の場合は `left = edge.right_terrain` と swap してポリラインに記録する。
- 閉ループの場合も同様にフラグを持たせる。
- 同一 `BoundaryKind` でも `left/right terrain` または `left/right zone_bias` の組が途中で変わる場合は、その変化点で `BoundarySurface` を分割する。単一 material instance が表現できる metadata は 1 組だけとする。

#### 新規型（`boundary.rs` 末尾または `boundary_surface.rs` に分離）

```rust
/// ribbon ひとつ分の描画単位。BoundarySliceSpatialIndex が参照する。
pub struct BoundarySurface {
    pub id: usize,
    pub kind: BoundaryKind,
    /// terrain_type_to_id_byte(left_terrain) の結果（shader uniform 用）
    pub left_terrain_id: u8,
    pub right_terrain_id: u8,
    pub left_zone_bias: u8,
    pub right_zone_bias: u8,
    /// sample_catmull_rom 後のサンプル点列（セル clipping の入力）
    pub sampled_center: Vec<Vec2>,
    pub is_closed: bool,
}

/// セル内の boundary 被覆範囲。
/// polygon_world は cell AABB でクリップされた ribbon polygon の頂点（world 座標）。
pub struct BoundarySlice {
    pub surface_id: usize,
    pub kind: BoundaryKind,
    /// clipped polygon（軸平行クリップ、最大 8 頂点）の world 空間頂点
    pub polygon_world: smallvec::SmallVec<[Vec2; 8]>,
}

/// Cell (grid_x, grid_y) → BoundarySlice のマルチマップ
#[derive(Resource)]
pub struct BoundarySliceSpatialIndex {
    pub cells: std::collections::HashMap<(i32, i32), smallvec::SmallVec<[BoundarySlice; 2]>>,
}
```

**変更ファイル**:
- `crates/bevy_app/src/world/map/boundary.rs`
- `docs/world_layout.md`（責務境界を追記）

**完了条件**:
- [ ] `BoundaryEdge` / `BoundaryPolyline` に `left_terrain` / `right_terrain` が追加されている
- [ ] zone tone 境界で使う `left_zone_bias` / `right_zone_bias` が追加されている
- [ ] `extract_boundary_edges` が水平・垂直エッジとも正しく left/right を設定している
- [ ] `chain_edges_to_polylines` がエッジ極性の逆転を考慮して left/right を引き継いでいる
- [ ] left/right terrain または zone bias の組が途中で変わるケースの分割方針が固定されている
- [ ] `BoundarySurface` / `BoundarySlice` / `BoundarySliceSpatialIndex` が定義されコンパイルが通る
- [ ] `cargo check` が通る

---

### M2: UV 追加・BoundarySurface 生成・Spatial Index 構築

**目的**: ribbon mesh に UV を付与し、`BoundarySurface` と `BoundarySliceSpatialIndex` を生成するスタートアップ経路を整備する。

**変更内容**:

#### `build_quad_strip_mesh` の UV 追加

各クワッドセグメントの頂点 UV:

| 頂点 | UV |
|---|---|
| `li` (left of pi) | `(0.0, v_i)` |
| `ri` (right of pi) | `(1.0, v_i)` |
| `lj` (left of pj) | `(0.0, v_j)` |
| `rj` (right of pj) | `(1.0, v_j)` |

`v_i = arc_length[i] * uv_scale`（`uv_scale = 1.0 / TILE_SIZE` でアルベドタイリングに合わせる）。  
ラウンドキャップ頂点: center = `(0.5, v_cap)`、弧左端 = `(0.0, v_cap)`、弧右端 = `(1.0, v_cap)`（theta=π で右端、theta=0 で左端）。  
arc_length は `build_quad_strip_mesh` 呼び出し元（`spawn_boundary_meshes`）で生成済みの sampled 点列から `parameterize_arc_length` で計算して渡す（関数シグネチャ変更）:

```rust
pub fn build_quad_strip_mesh(
    points: &[Vec2],
    arc_lengths: &[f32],  // ← 追加
    width: f32,
    y_offset: f32,
    is_closed: bool,
) -> Mesh
```

#### `spawn_boundary_meshes` の更新

```rust
// sampled 後に arc_lengths を再計算して mesh に渡す
let sampled_arcs = parameterize_arc_length(&sampled);
let mesh = build_quad_strip_mesh(&sampled, &sampled_arcs, STRIP_WIDTH, Y_MAP_BOUNDARY_BASE, polyline.is_closed);
```

また、`BoundarySurface` を組み立てて `Vec<BoundarySurface>` に収集し、後続の `build_boundary_slice_spatial_index` へ渡す。

#### `build_boundary_slice_spatial_index` 関数（`boundary.rs` に追加）

```rust
/// BoundarySurface 群からセル→スライスのインデックスを生成する。
///
/// 各 surface の sampled_center 点列から ribbon セグメント quad を算出し、
/// Sutherland-Hodgman 法（軸平行版）でセル AABB にクリップする。
pub fn build_boundary_slice_spatial_index(
    surfaces: &[BoundarySurface],
    half_width: f32,
) -> BoundarySliceSpatialIndex
```

**クリップアルゴリズム（軸平行版 Sutherland-Hodgman）**:
1. 各セグメント `(pi, pj)` のクワッド 4 頂点 `[li, lj, rj, ri]` を初期 polygon とする。
2. クワッドの AABB が重なるセルを列挙する（AABB テスト後に正確クリップ）。
3. セル矩形 `[cx*TILE_SIZE, (cx+1)*TILE_SIZE] × [cy*TILE_SIZE, (cy+1)*TILE_SIZE]` に対し、軸 4 本それぞれで Sutherland-Hodgman クリップを適用する。
4. 結果が 3 頂点以上なら `BoundarySlice` を生成し index に挿入する。

ラウンドキャップは cap 扇形の凸 hull polygon（center + 弧 `ROUND_CAP_SEGMENTS+1` 頂点）として同様にクリップする。AABB 近似には落とさない。

**関連するスタートアップ配線** (`startup/mod.rs`):

```rust
// PostStartup chain に追加（spawn_boundary_meshes の後）
build_boundary_slice_spatial_index_system,
```

`build_boundary_slice_spatial_index_system` は Resource `BoundarySliceSpatialIndex` を `Commands::insert_resource` で挿入する。

**変更ファイル**:
- `crates/bevy_app/src/world/map/boundary.rs`
- `crates/bevy_app/src/plugins/startup/mod.rs`

**完了条件**:
- [ ] ribbon mesh に `ATTRIBUTE_UV_0` が付与されており、u = [0,1] が cross-section になっている
- [ ] cap 頂点の UV が center=0.5、弧端が 0.0 / 1.0 で設定されている
- [ ] `BoundarySliceSpatialIndex` Resource が起動時に生成される
- [ ] 三叉路セルが複数の `BoundarySlice` を持つことを `info!` ログで確認できる
- [ ] open chain endpoint の cap がセル index に含まれる
- [ ] `cargo check` が通る

---

### M3: BoundarySurfaceMaterial と塗り経路の置換

**目的**: ribbon mesh に world-space テクスチャブレンドを行う新 material を割り当て、terrain shader の edge band を無効化する。

**変更内容**:

#### 新ファイル `crates/hw_visual/src/material/boundary_surface_material.rs`

```rust
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};

#[derive(Clone, Copy, Debug, ShaderType, Reflect)]
pub struct BoundarySurfaceUniform {
    /// terrain_type_to_id_byte(left_terrain) / 255.0 → shader で u32 に変換
    pub left_terrain_id: f32,
    pub right_terrain_id: f32,
    pub left_zone_bias: f32,
    pub right_zone_bias: f32,
    pub uv_scale: f32,       // 1.0 / TILE_SIZE
    pub blend_softness: f32, // smoothstep の遷移幅 (default 0.15)
    pub map_world_width: f32,
    pub map_world_height: f32,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct BoundarySurfaceMaterialExt {
    #[uniform(100)]
    pub uniforms: BoundarySurfaceUniform,
    // TerrainSurfaceMaterial と同じアルベドを共有する（Handle のクローン）
    #[texture(101)] #[sampler(102)]  pub grass_albedo:  Option<Handle<Image>>,
    #[texture(103)] #[sampler(104)]  pub dirt_albedo:   Option<Handle<Image>>,
    #[texture(105)] #[sampler(106)]  pub sand_albedo:   Option<Handle<Image>>,
    #[texture(107)] #[sampler(108)]  pub river_albedo:  Option<Handle<Image>>,
    #[texture(109)] #[sampler(110)] pub terrain_macro_noise: Option<Handle<Image>>,
}

impl MaterialExtension for BoundarySurfaceMaterialExt {
    fn fragment_shader() -> ShaderRef {
        "shaders/boundary_surface_material.wgsl".into()
    }
}

pub type BoundarySurfaceMaterial =
    ExtendedMaterial<StandardMaterial, BoundarySurfaceMaterialExt>;
```

`StandardMaterial` ベースの設定：
```rust
StandardMaterial {
    base_color: Color::WHITE,
    alpha_mode: AlphaMode::Blend,  // エッジフェード用
    unlit: true,
    double_sided: true,
    cull_mode: None,
    ..default()
}
```

#### 新ファイル `assets/shaders/boundary_surface_material.wgsl`

主要な shader ロジック（擬似コード）:

```wgsl
@fragment fn fragment(in: VertexOutput) -> FragmentOutput {
    let u = in.uv.x;   // 0=left edge, 1=right edge
    let world_xz = in.world_position.xz;

    // left/right のテクスチャをそれぞれ world-space UV でサンプル
    let left_id  = u32(round(bsm.left_terrain_id));
    let right_id = u32(round(bsm.right_terrain_id));
    let left_color  = sample_terrain_albedo(left_id,  world_xz, bsm.uv_scale);
    let right_color = sample_terrain_albedo(right_id, world_xz, bsm.uv_scale);
    let left_graded  = apply_zone_bias_if_needed(left_color, left_id, bsm.left_zone_bias);
    let right_graded = apply_zone_bias_if_needed(right_color, right_id, bsm.right_zone_bias);

    // u=0.5 で等分ブレンド、softness でぼかす
    let s = bsm.blend_softness;
    let blend_t = smoothstep(0.5 - s, 0.5 + s, u);
    let blended = mix(left_graded, right_graded, blend_t);

    // リボン端（u≈0, u≈1）でアルファを 0 へフェード
    let edge_fade = smoothstep(0.0, 0.08, u) * (1.0 - smoothstep(0.92, 1.0, u));

    out.color = vec4(blended, edge_fade);
}
```

zone tone 境界（GrassZoneTone / DirtZoneTone）は left_id == right_id であっても、`left_zone_bias` / `right_zone_bias` を使って左右で異なる palette state を適用し、ribbon 側で境界表現を完結させる。

#### `spawn_boundary_meshes` の更新

```rust
// 変更前: materials.add(StandardMaterial { ... })
// 変更後: boundary_surface_materials.add(BoundarySurfaceMaterial { ... })

pub fn spawn_boundary_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut boundary_materials: ResMut<Assets<BoundarySurfaceMaterial>>,
    layout: Res<GeneratedWorldLayoutResource>,
    game_assets: Res<GameAssets>,
) { ... }
```

material インスタンスは `BoundaryKind` ごとではなく、少なくとも `(kind, left_terrain_id, right_terrain_id, left_zone_bias, right_zone_bias)` ごとに生成・共有する。

#### MaterialPlugin 登録

`crates/hw_visual/src/lib.rs` の `HwVisualPlugin` に追加：

```rust
app.add_plugins(MaterialPlugin::<BoundarySurfaceMaterial>::default());
```

#### `terrain_surface_material.wgsl` の blend_band 縮小

```wgsl
// 変更前
let blend_band = 0.16;
// 変更後（ribbon が塗りを担当するため edge band を実質 0 に）
let blend_band = 0.0;
```

> zone tone 境界も ribbon 側で処理する前提なので、最終的には異カテゴリペアと同様に terrain shader 側の境界表現責務を外す。移行途中だけ一時的に既存経路を残してよいが、DoD には含めない。

**変更ファイル**:
- `crates/hw_visual/src/material/boundary_surface_material.rs`（新規）
- `crates/hw_visual/src/material/mod.rs`（pub use 追加）
- `crates/hw_visual/src/lib.rs`（re-export）
- `assets/shaders/boundary_surface_material.wgsl`（新規）
- `crates/bevy_app/src/world/map/boundary.rs`（`spawn_boundary_meshes` 更新）
- `crates/hw_visual/src/lib.rs`（`MaterialPlugin::<BoundarySurfaceMaterial>` 追加）
- `assets/shaders/terrain_surface_material.wgsl`（blend_band 縮小）
- `docs/world_layout.md`

**完了条件**:
- [ ] 境界線の ribbon が world-space アルベドをブレンドした色で描画される（フラットカラーではない）
- [ ] ribbon の曲線形状が塗りの輪郭になっている（terrain shader の直線帯が ribbon で隠れている）
- [ ] ribbon 端部でアルファフェードが機能し、terrain 面との継ぎ目が自然に見える
- [ ] `AlphaMode::Blend` の描画順が他の透過オブジェクトと競合しない（RenderLayers の確認）
- [ ] `cargo check` が通る

---

### M4: ランタイム更新と docs 同期

**目的**: `TerrainChangedEvent` 後に境界面データを再構築し、ドキュメントを最終構成へ同期する。

**変更内容**:

#### `terrain_id_map_sync_system` と境界再構築の順序確立

現状 `TerrainChangedEvent` は `terrain_id_map_sync_system` が `terrain_id_map` を更新するだけで、ribbon mesh は更新されない。M4 ではイベント受信後に境界 mesh / BoundarySliceSpatialIndex を再構築する経路を追加する。

再構築時の terrain ソースは **`GeneratedWorldLayoutResource` ではなく最新の `WorldMap`** を使う。`GeneratedWorldLayoutResource` は startup snapshot のため runtime 地形変化を反映できない。zone tone 判定に必要な `WorldMasks` が runtime 不変である間は `layout.layout.masks` を併用してよいが、terrain type は必ず `WorldMap` から読む。

戦略（初期は全再生成）:
1. `TerrainChangedEvent` 受信後、`boundary_rebuild_request` Resource（フラグ or dirty rect）をセット
2. 次フレームの Visual フェーズ冒頭で `rebuild_boundary_meshes_system` が全再生成
3. `BoundarySliceSpatialIndex` も再生成して Resource を上書き

`rebuild_boundary_meshes_system` のシグネチャ案:

```rust
pub fn rebuild_boundary_meshes_system(
    mut commands: Commands,
    query: Query<Entity, With<BoundaryMeshMarker>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut boundary_materials: ResMut<Assets<BoundarySurfaceMaterial>>,
    world_map: WorldMapRead,
    layout: Res<GeneratedWorldLayoutResource>, // masks のみ使用
    game_assets: Res<GameAssets>,
    rebuild_req: Res<BoundaryRebuildRequest>,
) { ... }
```

**変更ファイル**:
- `crates/bevy_app/src/world/map/boundary.rs`（`BoundaryMeshMarker` Component, `BoundaryRebuildRequest` Resource）
- `crates/bevy_app/src/systems/visual/terrain_material.rs`（rebuild trigger 追加）
- `crates/bevy_app/src/plugins/visual.rs`（system 登録）
- `docs/world_layout.md`, `docs/map_generation.md`, `docs/architecture.md`, `docs/README.md`

**完了条件**:
- [ ] `TerrainChangedEvent` 後に ribbon mesh が再生成される
- [ ] 再生成後も境界面と塗り範囲の整合が保たれる
- [ ] rebuild コストを `info!` ログで確認できる
- [ ] 更新経路が docs に明記されている
- [ ] `cargo check` が通る

---

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `follow_chain` でのエッジ極性反転の追跡が漏れる | left/right swap ミスでテクスチャが逆になる | M1 でテストシナリオ（草←→土の長い境界）を 1 本用意し、left/right が正しい側に出るか目視確認する |
| `build_quad_strip_mesh` の UV がキャップ部分で不正 | cap 境界で u が突変してテクスチャが裂ける | キャップ弧の theta=0 → u=0 / theta=π → u=1 の対応を明示し、テスト境界で端部拡大確認する |
| セル内 polygon clipping 実装が複雑化する | 実装工数増大、バグ混入 | セグメント quad も cap 扇形も exact polygon として軸平行クリップする。近似で粗くしない。実装が重ければ M2 を遅らせても近似へ落とさない |
| `BoundarySurfaceMaterial` の binding slot が `StandardMaterial` 拡張と衝突する | コンパイルエラー / GPU エラー | `ExtendedMaterial<StandardMaterial, ...>` の拡張 binding は `100+` を使う。既存 `TerrainSurfaceMaterialExt` と同様の方針に合わせる |
| `AlphaMode::Blend` の描画順が不安定 | ribbon が他の透過オブジェクトの前後で flickering | ribbon entity の `Y_MAP_BOUNDARY_BASE = 0.01` は terrain 面より高く保つ。`bevy_pbr` の depth sort は camera からの距離ベース。複数 ribbon が重なるエリアは `BoundaryKind` の priority 順に Y_OFFSET を微調整する（最大差 0.005wu 程度）|
| zone tone 境界（GrassZoneTone / DirtZoneTone）の palette bias が ribbon 側未実装のまま残る | shader 側 blend を落とすと zone tone 境界が消える | M1 で zone bias を left/right metadata として追跡し、M3 で ribbon material に組み込む。zone tone だけ旧 shader に残さない |
| 既存 `StandardMaterial` blend と `BoundarySurfaceMaterial` が重なる | 色が二重になる | M3 で `blend_band = 0.0` に変更するのは `should_blend_pair` が true（異カテゴリ）のペアのみ |
| runtime 全再生成が重い | 地形変更時のフレーム落ち | M4 初版は正しさ優先で全再生成。`cargo run -- --perf-log-fps` でフレーム時間を測り、必要なら dirty bounds に切り出す |

## 7. 検証計画

- 必須:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
- 手動確認シナリオ:
  - grass↔dirt の長い曲線境界で、ribbon の曲線輪郭と塗り輪郭が一致する
  - sand↔river の細い蛇行境界で、塗り幅が ribbon 幾何と一致する
  - 草アルベドと土アルベドが ribbon 中央で world-space UV で自然にブレンドされている
  - grass zone / neutral / dirt zone の境界で、ribbon 自体が zone bias 差を表現し、旧 terrain shader の境界表現に依存していない
  - 三叉路で複数境界面が同一セルを共有しても塗り欠けしない
  - マップ端へ抜ける未閉ループ境界で、終端 round cap を含む塗り面が欠けない
  - `TerrainChangedEvent` を伴う地形変更後も ribbon 塗りと terrain 整合が保たれる
- パフォーマンス確認（必要時）:
  - `cargo run -- --spawn-souls 500 --spawn-familiars 30 --perf-scenario --perf-log-fps` での境界全再生成時のフレーム時間
  - `BoundarySliceSpatialIndex` のメモリ量（`info!` ログで cell 数 × slice 数を出力）

## 8. ロールバック方針

- M1 は純粋なフィールド追加（`cargo check` さえ通れば既存動作に影響なし）。
- M2 は UV 追加＋Resource 追加のみ。`build_quad_strip_mesh` の引数変更は呼び出し元 1 箇所のみ。
- M3 でロールバックが必要な場合:
  1. `spawn_boundary_meshes` を `StandardMaterial` 版に戻す（1 コミット単位）。
  2. `terrain_surface_material.wgsl` の `blend_band` を `0.16` に戻す。
  3. `MaterialPlugin::<BoundarySurfaceMaterial>` の登録を外す。
- M4 のランタイム更新を戻す場合: `rebuild_boundary_meshes_system` の登録を外すだけ。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `12%`
- 完了済みマイルストーン:
  - 方式選定
  - 低解像度 mask 案の却下
  - terrain 側サブセル範囲管理の採用
  - コードベース精査（boundary.rs 全読み + shader 全読み + startup wiring 確認）
- 未着手:
  - M1: left/right terrain フィールド追加
  - M2: UV 追加・Spatial Index 構築
  - M3: BoundarySurfaceMaterial 実装
  - M4: ランタイム更新・docs 同期

### 次のAIが最初にやること

1. **M1 着手**: `boundary.rs` を開き、`BoundaryEdge` に `left_terrain/right_terrain` を追加し `extract_boundary_edges` を更新する
2. `chain_edges_to_polylines` → `follow_chain` の逆走査フラグを確認し、polarity swap ロジックを入れる
3. `BoundarySurface` / `BoundarySlice` / `BoundarySliceSpatialIndex` を `boundary.rs` 末尾（または分離ファイル）に追加する
4. `cargo check` で M1 が clean になることを確認してから M2 へ進む

### ブロッカー/注意点

- ユーザーは「元の境界線より粗くなる方式」を却下している。固定解像度 mask / subcell grid へ戻さないこと。
- サブセル範囲は連続 polygon / clipped quad として扱い、離散分割へ落とさないこと。
- 複数境界が同一セルを共有するケースを前提に設計すること。
- `build_quad_strip_mesh` の関数シグネチャ変更（`arc_lengths` 引数追加）は呼び出し元が `spawn_boundary_meshes` のみ。変更漏れは `cargo check` で検出できる。
- Bevy 0.18 の `MaterialPlugin` は `app.add_plugins(MaterialPlugin::<T>::default())` で登録する。既存の `Material2dPlugin` / `TerrainSurfaceMaterialPlugin` の登録場所（`startup/mod.rs` の `StartupPlugin::build`）を参考にする。
- `BoundarySurfaceMaterialExt` は `ExtendedMaterial<StandardMaterial, _>` のため、Bevy が自動挿入する binding 0 (`StandardMaterial`) との重複を避けるため、独自 binding は 0 から始めず Bevy の `MaterialExtension` トレイト上の custom binding (0+) を使う。具体的には `#[uniform(100)]` などにずらすか、`MaterialExtension::fragment_shader` の実装を参考に確認する。

### 参照必須ファイル

- `crates/bevy_app/src/world/map/boundary.rs`（現行パイプライン全体）
- `assets/shaders/terrain_surface_material.wgsl`（blend_band 箇所: L403, L408）
- `crates/hw_visual/src/material/terrain_surface_material.rs`（ExtendedMaterial パターン）
- `crates/bevy_app/src/plugins/startup/mod.rs`（MaterialPlugin 登録と PostStartup 配線）
- `crates/bevy_app/src/plugins/startup/visual_handles.rs`（GameAssets ハンドル取得パターン）
- `crates/hw_world/src/coords.rs`（`grid_to_world` 座標系確認）
- `docs/world_layout.md`
- `docs/map_generation.md`

### 最終確認ログ

- 最終 `cargo check`: `2026-04-06` / `not run (plan brushup only)`
- 未解決エラー: `N/A`

### Definition of Done

- [ ] 目的に対応するマイルストーンが全て完了
- [ ] `cargo check` が成功
- [ ] 境界線と塗り面の輪郭が目視で一致する
- [ ] 影響ドキュメントが更新済み

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-06` | `Codex` | 初版作成 |
| `2026-04-06` | `Copilot` | コードベース精査に基づく具体化（型定義・UV設計・shader設計・座標系確認・リスク詳細化） |
