# ワールドマップ LOD1 パフォーマンス改善計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `world-map-lod1-performance-plan-2026-04-09` |
| ステータス | `Archived` |
| 作成日 | `2026-04-09` |
| 最終更新日 | `2026-07-13` |
| 作成者 | `Codex` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

> `boundary_proximity_mask`、`Lod1Lite`、3段階ヒステリシス、feature LUT の uniform fast-path は実装済み。未解決の LOD 切替ポップは現行の `terrain-lod-switch-flicker-plan-2026-04-17.md` へ移管した。

## 1. 目的

- 解決したい課題:
  ワールドマップの近景表示で使う `LOD1` が、draw call 数ではなく fragment shader コストによって 3D RtT パイプラインの支配項になっている。`rendering-performance.md` によると LOD1 は **~15 tex sample/px × ~1,760,000 px ≈ 26M sample/frame** で支配的。
- 到達したい状態:
  近景の見た目を大きく崩さず、`LOD1` の平均 1px あたり処理量を減らし、LOD 切替閾値を不必要に下げなくても通常プレイ時の負荷を抑えられる状態にする。
- 成功指標:
  - `LOD1` 使用時の GPU ボトルネックが現状より明確に低下する（フレームタイムまたは dev 表示で確認）
  - 近景での曲線境界、砂地の見分け、ゾーントーン差が受け入れ可能な範囲で維持される
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` が通る

## 2. スコープ

### 対象（In Scope）

- `assets/shaders/terrain_surface_material.wgsl` の `LOD1` 経路の軽量化（主に `blend_terrain` 関数）
- `boundary.rs` の `rasterize_terrain_regions` / `spawn_boundary_meshes` への境界近傍マスクベイク追加
- `LOD1` と `LOD2` の間に入る `LOD1-lite` material / shader の追加
- `TerrainLodState` / `LodLevel` / `Terrain3dHandles` の 3 段階 LOD 対応
- `sample_feature_lut` の固定インデックス呼び出しを uniform 値に置換
- LOD1 負荷観測のための DevPanel 表示追加・比較手順ドキュメント化
- 影響仕様のドキュメント更新

### 非対象（Out of Scope）

- chunk 数や `CHUNK_TILES` の再設計
- 地形ロジック（`WorldMap.tiles`、worldgen、obstacle cleanup 契約）の変更
- 建築物・Soul・UI パイプラインの最適化
- WASM 専用最適化
- 地形表現を全面的に unlit 化する大規模アート方針変更

## 3. 現状とギャップ

### 現状の LOD1 フラグメントシェーダー構造（コード実態）

`fragment()` 関数（`terrain_surface_material.wgsl`）の処理ステップ：

1. `section_discard` — 高速
2. `pbr_input_from_standard_material` — StandardMaterial テクスチャ数本
3. `world_to_cell` / `sample_feature(cell)` / `cell_terrain_id(cell)` — 2 textureLoad
4. **`blend_terrain(world_xz, cell, feature)`** ← 主要コスト源
   - a. `textureSample(boundary_mask, …, uv)` Nearest 1回（`raw_center` 取得）
   - b. `if region_id != cell_terrain_id(cell)` → 8近傍 `textureLoad` ×2（terrain_id_map + feature_map）= 最大 16 textureLoad
   - c. **4コーナー UV 算出 + `textureSample(boundary_mask, …)` ×4**（bilinear 疑似実装）
   - d. 4コーナー全同一 → **fast path**: `sample_surface_color` ×1（~5-8 sample）
   - e. 不一致 → **slow path**: 8近傍 feature 探索 + `sample_surface_color` ×4（~20-32 sample）
5. `roughness_delta_for_id` — `sample_feature_lut(3.0)` または ×2 呼び出し
6. `apply_pbr_lighting` / `main_pass_post_lighting_processing`

**fast path は既に存在するが、fast path 判定前に必ず c の 4 bilinear サンプルが走る。**
内部タイル（境界帯外）でも c のコストを毎回払っている。

### `sample_surface_color` のコスト内訳

`compute_terrain_uv`: `terrain_domain_warp_strength` > 0 → `sample_macro_noise` 1回
`sample_albedo`: albedo テクスチャ 1回
`sample_macro_overlay` + `sample_macro_noise`: brightness 計算で 2回
Grass: `apply_palette_bias` のみ — 追加 sample なし
Dirt: `sample_feature_lut(3.0)` — 1回
Sand: `sample_feature_lut(1.0)` + `sample_feature_lut(2.0)` + `shoreline_detail` — 3回

合計 ~5-8 sample/call。slow path では ×4 なので ~20-32 sample。

### `sample_feature_lut` の重複問題

`sample_feature_lut(idx: f32)` は固定インデックス（1.0 / 2.0 / 3.0）しか使用しない。
1x256 ピクセルの静的 LUT テクスチャをフラグメントごとに 1-3 回参照している。
これは startup 時に uniform 値として焼ける。

### LOD 状態の現状

`terrain_lod.rs` の `LodLevel`: `Lod0`（未使用、将来予約）/ `Lod1`（現行 near）/ `Lod2`（far）
切替閾値: `LOD1_TO_LOD2_ENTER_PX = 14.0` / `LOD2_TO_LOD1_EXIT_PX = 16.0`
`Terrain3dHandles`: `lod1: Handle<TerrainSurfaceMaterial>` + `lod2: Handle<TerrainSurfaceMaterialLod2>`

LOD2 との差異（`terrain_surface_material_lod2.wgsl` の実態）:
- bilinear →  nearest-only に変更（`blend_terrain_lod2`）
- `sample_surface_color_lod2`: macro noise / domain warp / UV distort / river scroll を除去
- `grade_sand_lod2`: `shoreline_detail` サンプル除去
- PBR 経路は LOD1 と同一

### 本計画で埋めるギャップ

1. **M2**: `boundary_mask`（R8Unorm, 1024×1024, binding 129）の派生として `boundary_proximity_mask`（binding 131）を新設し、内部タイル画素で c の 4 bilinear サンプルを完全スキップする
2. **M3**: LOD1-lite = LOD2 の surface color 関数 + LOD1 の bilinear 境界ブレンド + M2 の early-out 。閾値を 3 段階化し、フル品質を近距離のみに限定する
3. **M4**: `sample_feature_lut(1.0/2.0/3.0)` を `TerrainSurfaceUniforms` の定数フィールドに置換し、フラグメントあたり 1-3 テクスチャサンプルを削減する

## 4. 実装方針

### 基本方針

1. M2 → M3 → M4 の順に実施。効果確認後にのみ次へ進む
2. 各マイルストーンで `cargo check` を通してから次へ進む
3. material 切替は `MeshMaterial3d(handle.clone())` の差し替えのみ。`get_mut` で共有 material を mutate しない
4. `ExtendedMaterial<StandardMaterial, ...>` を維持し、prepass / section cut の整合を保つ

### 設計上の制約（コードから確認済み）

- `boundary_mask` は `R8Unorm` / 1024×1024。追加テクスチャは新 binding（131/132）で独立して追加する
- `TerrainSurfaceMaterialExt` と `TerrainSurfaceMaterialExtLod2` はバインドグループレイアウトを共有している。新テクスチャは **両方**の拡張構造体に追加が必要
- `LodLevel::Lod0` は将来予約済み。新レベル `Lod1Lite` は `Lod1` と `Lod2` の間に挿入する
- `terrain_feature_map` / `boundary_mask` は static bake。`terrain_id_map` は runtime 更新対象。この責務差を崩さない

### バインディング番号管理

```
現行 LOD1 shader のバインディング:
 100: TerrainSurfaceUniforms (uniform)
 101: terrain_id_map
 102: terrain_feature_map
 103-110: grass/dirt/sand/river albedo + sampler ×4
 111-118: macro_noise, grass/dirt/sand overlay + sampler ×4
 119-120: terrain_blend_mask_soft + sampler
 121-126: river_flow_noise, river_normal_like, shoreline_detail + sampler ×3
 127-128: terrain_feature_lut + sampler
 129-130: boundary_mask + sampler

M2 で追加:
 131: boundary_proximity_mask  (texture_2d<f32>, R8Unorm)
 132: boundary_proximity_sampler (sampler, Nearest)
```

## 5. マイルストーン

## M1: LOD1 観測基盤整備

### 変更内容

- `TerrainLodMetrics`（`terrain_lod.rs`）に `lod_label: &'static str` を追加し、DevPanel に LOD 文字列と `tile_rtt_px` を表示する
- 比較観測手順を `docs/rendering-performance.md` の §7 に追記する

### 変更ファイル

- `crates/bevy_app/src/interface/ui/dev_panel.rs` — LOD 状態行の追加
- `crates/bevy_app/src/systems/visual/terrain_lod.rs` — `TerrainLodMetrics` への表示補助フィールド追加（任意）
- `docs/rendering-performance.md` — §7 に比較手順・観測フォーマット追記

### 完了条件

- [ ] DevPanel に現在の `LodLevel`（文字列）と `tile_rtt_px`（整数 px）が表示される
- [ ] `docs/rendering-performance.md` §7 に、同一 seed / 同一 camera zoom での LOD 比較手順が記載されている

### 検証

- `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
- 手動: `cargo run` でデバッグ表示確認

---

## M2: `boundary_proximity_mask` による early-out 導入

### 概要

内部タイル（全 8 近傍が同一 terrain_region_byte）のフラグメントで、`blend_terrain` 内の **4 bilinear サンプル + 8近傍 feature 探索** を完全スキップする。

### CPU 側: `boundary.rs` の変更

`rasterize_terrain_regions` が返す 1024×1024 `buf: Vec<u8>` を受け取り、同解像度の proximity mask を生成する関数を追加する：

```rust
/// buf の各ピクセルについて 3×3 近傍内に異なる値が存在するか確認し、
/// 境界近傍 → 255、内部 → 0 の R8Unorm バッファを返す。
/// さらに N=5px の dilation で safe margin を確保する。
fn bake_boundary_proximity_mask(buf: &[u8], res: usize) -> Vec<u8> { … }
```

`spawn_boundary_meshes` の `rasterize_terrain_regions` 呼び出し直後にこの関数を呼び出し、256×256 にダウンサンプルして `Image`（`TextureFormat::R8Unorm`、`ImageSamplerDescriptor` Nearest）を生成、ハンドルを `spawn_boundary_meshes` 内で `mat.extension.boundary_proximity_mask` に後付け設定する。

> **解像度の根拠**: 1024px / 100tiles ≈ 10px/tile。bilinear footprint は ±0.5px。dilation 5px で余裕を持った境界帯を確保できる。256×256 にダウンサンプルしても 2.56px/tile で判別は十分。

### Rust 側: `TerrainSurfaceMaterialExt` / `TerrainSurfaceMaterialExtLod2`

```rust
// crates/hw_visual/src/material/terrain_surface_material.rs
pub struct TerrainSurfaceMaterialExt {
    // … 既存フィールド …
    #[texture(131)]
    #[sampler(132)]
    pub boundary_proximity_mask: Option<Handle<Image>>,
}
```

同様に `TerrainSurfaceMaterialExtLod2` にも追加（バインドグループレイアウト共有のため必須）。

### Shader 側: `terrain_surface_material.wgsl`

`blend_terrain` の先頭に early-out を挿入する：

```wgsl
@group(#{MATERIAL_BIND_GROUP}) @binding(131) var boundary_proximity_mask: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(132) var boundary_proximity_sampler: sampler;

fn blend_terrain(world_xz: vec2<f32>, cell: vec2<i32>, feature_in: vec4<f32>) -> vec3<f32> {
    let prox_uv = world_to_boundary_uv(world_xz);

    // Early-out: 境界近傍マスクが 0（内部タイル）なら bilinear ブレンド不要
    let is_boundary = textureSample(boundary_proximity_mask, boundary_proximity_sampler, prox_uv).r > 0.5;
    if !is_boundary {
        let raw_center = textureSample(boundary_mask, boundary_mask_sampler, prox_uv).r;
        let region_id  = region_to_coarse_id(raw_center);
        let region_raw = region_to_raw_byte(raw_center);
        let eff_f = feature_with_zone_tone(feature_in, region_raw, region_id);
        return sample_surface_color(region_id, world_xz, eff_f, region_raw);
    }

    // 既存の blend_terrain 本体（bilinear path）をそのまま続ける …
```

`terrain_surface_material_lod2.wgsl` にも binding 131/132 を追加するが、LOD2 の `blend_terrain_lod2` では early-out の使用は任意（LOD2 はすでに nearest-only で安価なため、追加してもコスト削減が小さい）。

### 変更ファイル

- `crates/bevy_app/src/world/map/boundary.rs` — `bake_boundary_proximity_mask` 関数追加、`spawn_boundary_meshes` 呼び出し箇所（line ~994）に追記
- `crates/bevy_app/src/plugins/startup/visual_handles.rs` — startup material 初期化で `boundary_proximity_mask: None` を追加
- `crates/hw_visual/src/material/terrain_surface_material.rs` — `TerrainSurfaceMaterialExt` / `Ext Lod2` に binding 131/132 追加
- `assets/shaders/terrain_surface_material.wgsl` — binding 131/132 宣言 + `blend_terrain` 先頭 early-out
- `assets/shaders/terrain_surface_material_lod2.wgsl` — binding 131/132 宣言のみ（unused として追加し、バインドグループレイアウト一致を維持）
- `docs/world_layout.md` — `boundary_proximity_mask` の role 追記
- `docs/architecture.md` — startup bake テクスチャ一覧更新

### 完了条件

- [ ] 内部タイル（全 8 近傍が同一 terrain_region_byte）のフラグメントで 4 bilinear サンプルが走らない
- [ ] 曲線境界・ゾーントーン遷移が近景で破綻しない（草↔土、土↔砂、river、zone tone 各境界を確認）
- [ ] `spawn_boundary_meshes` の startup 完了時間が許容範囲内（目視で顕著な遅延なし）

### 検証

- `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
- 手動: 近景で grass / dirt / sand / river 境界を拡大確認

---

## M3: `LOD1-lite` の追加

### 概要

LOD1（フル品質）と LOD2（軽量）の中間に `LOD1-lite` を追加する。
- **維持**: bilinear 境界ブレンド（曲線境界の見た目）+ M2 early-out
- **削除**: macro noise/overlay の brightness 変調、domain warp、UV distort、river animation、shoreline_detail サンプル

推定コスト: ~5-7 sample/px（LOD1 ~15 → LOD1-lite ~6 → LOD2 ~4）

### Shader: `terrain_surface_material_lod1_lite.wgsl`

LOD1 shader をベースに以下を削除：

```
compute_terrain_uv:
  - domain_warp_strength チェックと sample_macro_noise 呼び出し → sample_xz = world_xz 固定
  - distort_strength チェックと wobble 計算 → base_uv そのまま
  - scroll_speed チェックと river_flow_noise / river_normal_detail → uv 補正なし

sample_surface_color_lod1_lite:
  - brightness_strength チェックを削除 → brightness = 1.0 固定
    （sample_macro_noise / sample_macro_overlay を呼ばない）

grade_sand_lod1_lite:
  - shoreline_detail テクスチャサンプル（binding 125/126）を削除
  - apply_shoreline_tone 呼び出しを削除
```

bindings 111-126（macro noise / overlay / river / shoreline）は宣言するがアクセスしない（バインドグループレイアウト一致のため）。

### Rust 側: 型・ハンドルの追加

```rust
// crates/hw_visual/src/material/terrain_surface_material.rs に追加
pub struct TerrainSurfaceMaterialExtLod1Lite { … }  // ExtLod2 と同構造、shader パスのみ変更
pub type TerrainSurfaceMaterialLod1Lite =
    ExtendedMaterial<StandardMaterial, TerrainSurfaceMaterialExtLod1Lite>;
```

```rust
// crates/bevy_app/src/plugins/startup/visual_handles.rs
pub struct Terrain3dHandles {
    pub lod1:      Handle<TerrainSurfaceMaterial>,
    pub lod1_lite: Handle<TerrainSurfaceMaterialLod1Lite>,  // 追加
    pub lod2:      Handle<TerrainSurfaceMaterialLod2>,
}
```

### `terrain_lod.rs` の 3 段階 LOD

```rust
pub enum LodLevel {
    Lod0,      // 将来予約
    Lod1,      // 近景フル品質
    Lod1Lite,  // 中距離（追加）
    Lod2,      // 遠景
}

// 新閾値（入方向 / 戻り方向）
pub const LOD1_TO_LOD1LITE_ENTER_PX: f32 = 22.0;
pub const LOD1LITE_TO_LOD1_EXIT_PX:  f32 = 25.0;
pub const LOD1LITE_TO_LOD2_ENTER_PX: f32 = 14.0;  // 既存値を流用
pub const LOD2_TO_LOD1LITE_EXIT_PX:  f32 = 16.0;  // 既存値を流用
```

`resolve_lod_level` と `terrain_lod_switch_system` を 3 段階に更新する。
`ChunkLod1LiteQuery` 型エイリアスを追加する。

### Plugin / system 配線

`TerrainSurfaceMaterialLod1Lite` を追加するだけでは不十分で、`HwVisualPlugin` 側の登録も必要。

- `MaterialPlugin::<material::TerrainSurfaceMaterialLod1Lite>::default()` を追加
- `sync_section_cut_to_terrain_surface_lod1_lite_system` を追加し、既存の `TerrainSurfaceMaterial` / `TerrainSurfaceMaterialLod2` と同じ契約で `SectionCut` を反映する
- 必要なら `hw_visual::lib.rs` の `pub use` に `TerrainSurfaceMaterialExtLod1Lite` / `TerrainSurfaceMaterialLod1Lite` / `make_terrain_surface_material_lod1_lite` を追加

### 閾値の根拠

`tile_rtt_px = 22px` は LOD0 仮定（32px）より小さく、通常プレイの「やや引いた」カメラ位置に相当する。macro noise の輝度変調は tile が 22px 以下になると目立たなくなる。

### 変更ファイル

- `crates/hw_visual/src/material/terrain_surface_material.rs` — `TerrainSurfaceMaterialExtLod1Lite` / `TerrainSurfaceMaterialLod1Lite` 追加
- `crates/hw_visual/src/lib.rs` — `MaterialPlugin` 登録、`SectionCut` sync system 追加、公開 API 更新
- `crates/bevy_app/src/plugins/startup/visual_handles.rs` — `Terrain3dHandles.lod1_lite` + startup 初期化
- `crates/bevy_app/src/systems/visual/terrain_lod.rs` — `LodLevel::Lod1Lite` / 閾値定数 / `resolve_lod_level` / `terrain_lod_switch_system` 更新
- `assets/shaders/terrain_surface_material_lod1_lite.wgsl` — 新規作成
- `docs/rendering-performance.md` — §7 テクスチャサンプル概算表更新
- `docs/world_layout.md` — LOD 段数と閾値更新
- `docs/art-style-criteria.md` — LOD1-lite での visual 品質目標追記

### 完了条件

- [ ] 中距離帯（tile_rtt_px ≈ 22px）で `LOD1-lite` が選択される
- [ ] zoom in/out で LOD1 ↔ LOD1-lite ↔ LOD2 の遷移にちらつきがない
- [ ] macro noise なしでも grass/dirt/sand の面識別が受け入れ可能

### 検証

- `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
- 手動: zoom in/out でフル遷移確認

---

## M4: `sample_feature_lut` の uniform 定数化

### 概要

`sample_feature_lut(idx: f32)` は固定インデックス（1.0 / 2.0 / 3.0）のみ使用する。
これらをテクスチャ参照から uniform 値に移すことで、slow bilinear path で最大 3 tex sample × 4 corners = **12 sample** を削減できる。

### CPU 側

`TerrainSurfaceUniform`（`crates/hw_visual/src/material/terrain_surface_material.rs`）に LUT 定数を追加：

```rust
pub struct TerrainSurfaceUniform {
    // … 既存フィールド …
    pub lut_shore:  Vec4,   // sample_feature_lut(1.0) の結果
    pub lut_inland: Vec4,   // sample_feature_lut(2.0) の結果
    pub lut_rock:   Vec4,   // sample_feature_lut(3.0) の結果
    pub feature_lut_constants_ready: f32, // 0.0=未同期, 1.0=同期済み
}
```

`AssetServer::load_with_settings(...)` の直後に `Assets<Image>` の `image.data` が必ず読める前提は置かない。
そのため `make_terrain_surface_material` / `make_terrain_surface_material_lod1_lite` / `make_terrain_surface_material_lod2` では `lut_*` を neutral 値、`feature_lut_constants_ready = 0.0` で初期化しておく。

別途 one-shot の同期 system を追加し、`Assets<Image>` に `game_assets.terrain_feature_lut` が実際に載ったタイミングで 1/2/3 番エントリを読み出して全 terrain material の uniform を更新する。

```rust
#[derive(Resource, Default)]
pub struct TerrainFeatureLutUniformSyncState {
    pub done: bool,
}

pub fn sync_terrain_feature_lut_uniforms_system(
    game_assets: Res<GameAssets>,
    images: Res<Assets<Image>>,
    mut mats_lod1: ResMut<Assets<TerrainSurfaceMaterial>>,
    mut mats_lod1_lite: ResMut<Assets<TerrainSurfaceMaterialLod1Lite>>,
    mut mats_lod2: ResMut<Assets<TerrainSurfaceMaterialLod2>>,
    mut state: ResMut<TerrainFeatureLutUniformSyncState>,
) { … }
```

この system は `state.done == true` になったら以後 no-op にする。

### Shader 側

```wgsl
// TerrainSurfaceUniforms に追加（binding 100）
struct TerrainSurfaceUniforms {
    // … 既存フィールド …
    lut_shore:  vec4<f32>,   // idx=1 の feature_lut 値
    lut_inland: vec4<f32>,   // idx=2 の feature_lut 値
    lut_rock:   vec4<f32>,   // idx=3 の feature_lut 値
    feature_lut_constants_ready: f32,
}

// ready フラグが立つまでは既存の textureSample(terrain_feature_lut, …) 経路を使う
// ready 後は tsm.lut_* を使う
// grade_sand: textureSample(terrain_feature_lut, …) を tsm.lut_shore / lut_inland に置換
// grade_dirt: textureSample(terrain_feature_lut, …) を tsm.lut_rock に置換
// roughness_delta_for_id: 同様に tsm.lut_* に置換
```

LOD1 と LOD1-lite の両 shader に適用する（LOD2 は既に `sample_feature_lut` を使用しているため同様に対応）。

### 変更ファイル

- `crates/hw_visual/src/material/terrain_surface_material.rs` — `TerrainSurfaceUniform` に lut_* 追加
- `crates/hw_visual/src/lib.rs` — LUT uniform 同期 system と sync state resource の登録
- `crates/bevy_app/src/plugins/startup/visual_handles.rs` — material 初期値を neutral + `feature_lut_constants_ready = 0.0` に設定
- `assets/shaders/terrain_surface_material.wgsl` — `TerrainSurfaceUniforms` 拡張 + `grade_*` / `roughness_delta_for_id` 置換
- `assets/shaders/terrain_surface_material_lod1_lite.wgsl` — 同上
- `assets/shaders/terrain_surface_material_lod2.wgsl` — 同上（LOD2 にも適用）
- `docs/architecture.md` — startup bake 情報更新

### 完了条件

- [x] 通常経路は uniform fast-path を使い、asset ready 前だけ `sample_feature_lut` fallback を残す
- [x] Dirt / Sand / rock の grading 契約を維持している

### 検証

- `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
- 手動: 岩場 dirt、shore sand、inland sand を近景で比較

---

## M5: 最終調整とドキュメント同期

### 変更内容

- 採用した LOD 構成、閾値、shader 責務、startup bake 契約を docs に反映する
- `rendering-performance.md` §7 のテクスチャサンプル概算表を更新する（LOD1-lite 行追加）
- 未採用案（地形 unlit 化、chunk 再設計）を非採用理由付きで記録する

### 変更ファイル

- `docs/world_layout.md`
- `docs/architecture.md`
- `docs/rendering-performance.md`
- `docs/art-style-criteria.md`
- `docs/plans/README.md`

### 完了条件

- [x] 実装に対応する docs が同期されている
- [x] LOD1 改善の判断根拠と閾値が追える

### 検証

- `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
- docs diff の目視確認

---

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `bake_boundary_proximity_mask` の dilation が不足し、bilinear footprint が内部判定領域に入る | 近景の境界付近に段差・alias | dilation を保守的に 5-7px（≈ 0.5-0.7 tile）に設定し、草↔土 / 砂浜 / zone tone ごとに近景拡大確認 |
| LOD1-lite の閾値（22px）が不適切で遷移ちらつき | UX 悪化 | hysteresis band ≥ 3px を維持し、zoom in/out で往復テスト |
| `TerrainSurfaceMaterialExtLod1Lite` と `Ext`/`ExtLod2` のバインドグループレイアウトが食い違い、パニックまたは描画なし | 起動時クラッシュ | binding 番号を `terrain_surface_material.rs` の既存定義と照合し、LOD2 と同一の番号体系を守る |
| lut_* の uniform 値が asset 未ロードの時点で参照される | 起動直後の表示バグ | `feature_lut_constants_ready` フラグを導入し、同期完了前は既存 texture lookup を使う |
| feature/LUT uniform 化で runtime 更新契約が壊れる | 地形更新後に grading が不整合 | `terrain_feature_lut` は完全 static asset なので runtime 更新はない。問題なし |
| M2/M3/M4 を同時に実装して問題切り分けが困難になる | デバッグコスト増大 | マイルストーン単位でコミットし、cargo check + 目視を挟む |

## 7. 検証計画

- 必須（各マイルストーン後）:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
- 手動確認シナリオ:
  - M2 後: 近景で grass / dirt / sand / river 境界が破綻していないか、早期 return された内部タイルの色が正しいか確認
  - M3 後: zoom in/out で LOD1 → LOD1-lite → LOD2 がヒステリシスどおり単発切替し、各レベルで tile_rtt_px が意図値か確認。切替瞬間の視覚ポップは現行 `../terrain-lod-switch-flicker-plan-2026-04-17.md` へ移管
  - M4 後: 岩場 dirt / shore sand / inland sand の色・roughness が変わっていないか確認
  - 全体: 矢視方向（TopDown / North / East / South / West）で tile_rtt_px 切替が安定するか確認
- パフォーマンス確認（任意）:
  - dev panel の LOD 表示 + tile_rtt_px で固定カメラ条件を再現
  - GPU profiler または OS/driver 側のフレームタイム観測で比較する

## 8. ロールバック方針

| マイルストーン | ロールバック方法 |
| --- | --- |
| M2 early-out | shader の early-out ブロックを削除、binding 131/132 を除去、`bake_boundary_proximity_mask` を `spawn_boundary_meshes` から切り離す |
| M3 LOD1-lite | `LodLevel::Lod1Lite` アームを `resolve_lod_level` から除去し `Lod1` へフォールバック、`Terrain3dHandles.lod1_lite` フィールドを削除 |
| M4 lut 定数化 | `TerrainSurfaceUniforms` から lut_* フィールドを削除し、shader の `tsm.lut_*` 参照を `sample_feature_lut(n)` に戻す |

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`（M1〜M5 実装済み、2026-07-13 棚卸し確認）
- 完了済みマイルストーン:
  - M1〜M5
- 後続:
  - LOD 切替瞬間の視覚ポップは `../terrain-lod-switch-flicker-plan-2026-04-17.md`

> 以下の「コードベース調査結果」と当初の実装手順は、着手前スナップショットとして残す。現行コードの指示として使用しない。

### コードベース調査結果サマリー

以下は `2026-04-09` にコードを実読して確認済みの事実：

| 事実 | ソース |
| --- | --- |
| `boundary_mask` は `TextureFormat::R8Unorm`、`1024×1024`、binding 129 | `boundary.rs:1062`、`terrain_surface_material.wgsl:129` |
| `blend_terrain` に fast path は**既存**（4corner 全同一チェック後）。ただし fast path 判定前に必ず 4 bilinear sample が走る | `terrain_surface_material.wgsl` `blend_terrain` 関数内 |
| `sample_feature_lut` は固定インデックス 1.0/2.0/3.0 のみ使用 | `grade_sand`, `grade_dirt`, `roughness_delta_for_id` |
| `LodLevel::Lod0` は将来予約済みで未使用。`resolve_lod_level` 内で `Lod1` にフォールバック | `terrain_lod.rs` |
| `Terrain3dHandles` は `lod1` / `lod2` の 2 ハンドルのみ | `visual_handles.rs:59-65` |
| `TerrainSurfaceMaterialExt` と `ExtLod2` のバインドグループレイアウトは同一であることが要求されている | `terrain_surface_material.rs:139-239` のコメント |
| `spawn_boundary_meshes`（line ~994）が terrain_region_map をベイクして material に設定する | `boundary.rs:994` |

### 次のAIが最初にやること

1. 本書から実装を再開しない。
2. LOD 切替ポップを扱う場合は `../terrain-lod-switch-flicker-plan-2026-04-17.md` の M1 観測から開始する。

### ブロッカー/注意点

- `TerrainSurfaceMaterialExt` と `ExtLod2` のバインドグループレイアウトが一致しなくなるとパニック（Bevy の ExtendedMaterial 制約）。**必ず両方に binding 追加すること**
- `LOD1-lite` 追加時は `crates/hw_visual/src/lib.rs` の `MaterialPlugin` 登録と `SectionCut` 同期 system も同時に追加すること
- `terrain_feature_map` は static bake、`terrain_id_map` は runtime 更新という責務差を崩さないこと
- `terrain_feature_lut` は startup 直後に `image.data` が必ず読めるとは限らない。M4 は one-shot 同期 system + fallback 経路前提で進めること
- LOD1-lite shader で binding 111-126 の未使用テクスチャは**宣言だけして呼ばない**（GPU に不要な bind は発生しない）
- `docs/plans/README.md` に載っている `world-map-lod-strategy-2026-04-06.md` は現ワークツリー上で実ファイルが見当たらない。重複確認不要

### 参照必須ファイル

- `assets/shaders/terrain_surface_material.wgsl` — `blend_terrain`, `sample_surface_color`, `grade_*`, `roughness_delta_for_id`
- `assets/shaders/terrain_surface_material_lod2.wgsl` — LOD2 の削除済み処理の参考
- `crates/bevy_app/src/world/map/boundary.rs` — `rasterize_terrain_regions`（line ~290）, `spawn_boundary_meshes`（line ~994）
- `crates/hw_visual/src/material/terrain_surface_material.rs` — `TerrainSurfaceMaterialExt` の binding 定義
- `crates/bevy_app/src/plugins/startup/visual_handles.rs` — `Terrain3dHandles`、startup material 初期化
- `crates/bevy_app/src/systems/visual/terrain_lod.rs` — `LodLevel`, `TerrainLodMetrics`, `resolve_lod_level`, `terrain_lod_switch_system`
- `docs/rendering-performance.md` — §7 フラグメントシェーダーコスト表

### 最終確認ログ

- 最終 `cargo check`: `2026-04-09` / `not run (plan only)`
- 未解決エラー: 未確認

### Definition of Done

- [x] 目的に対応するマイルストーンが全て完了
- [x] 影響ドキュメントが更新済み
- 当時の最終 `cargo check` ログは本書に記録されていない

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-09` | `Codex` | 初版作成 |
| `2026-07-13` | `Codex` | 実装済み状態へ訂正し、feature LUT fallback と LOD pop の後続先を明記してアーカイブ |
| `2026-04-10` | `Codex` | startup 順序・plugin 配線・LUT 同期タイミング・検証手順の不整合を修正 |
| `2026-04-09` | `Copilot` | コードベース実読（shader/boundary.rs/terrain_lod.rs 等）に基づき全セクションを具体化。バインディング番号・関数名・コード行番号・実装手順を明記 |
