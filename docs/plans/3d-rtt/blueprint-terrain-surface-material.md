# TerrainSurfaceMaterial 統合（MS-3-6 Phase 3 ブループリント）

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `blueprint-terrain-surface-material` |
| ステータス | `Implemented（受入確認・微調整継続）` |
| 対象 | **MS-3-6** 再検討後の **Phase 3**（隣接タイプ境界のソフトブレンド＋地形専用マテリアル） |
| 前提 Phase | [地形ビジュアル再検討](terrain-visual-reassessment-2026-04-05.md) の **Phase 1〜2**（観測・metadata・feature tint 等）。本書は **Phase 3 専用** |
| 親計画 | [`ms-3-6-terrain-surface-plan-2026-03-31.md`](ms-3-6-terrain-surface-plan-2026-03-31.md) の **B（隣接ブレンド）** に相当 |
| 参照 | [地形ビジュアル再検討 §4](terrain-visual-reassessment-2026-04-05.md)（`SectionMaterial` 拡張より `TerrainSurfaceMaterial` 新設） |

---

## 0. 実装反映サマリ（2026-04-05）

この文書は実装前のブループリントを元にしているが、2026-04-05 時点で baseline 実装は完了している。現行コードで確定した要点は次のとおり。

- 地形は `MeshMaterial3d<TerrainSurfaceMaterial>` に移行済みで、全タイルが共有 `Handle<TerrainSurfaceMaterial>` 1 本を使う
- startup で `TerrainFeatureMap`（`Rgba8Unorm`）と `TerrainIdMap`（`R8Unorm`）を生成し、`TerrainChangedEvent` 後は `terrain_id_map` の該当ピクセルだけを更新する
- 境界ブレンドは center + cardinal 近傍の重み付き和で実装し、逐次 `mix` は使わない
- 実運用ではブレンド帯を cell edge の狭い範囲に絞り、river が絡むブレンドは `river↔sand` の組み合わせだけを許可している
- 実装上は `terrain_id_map` / `terrain_feature_map` を `textureLoad` で読むため sampler を持たず、binding 番号や uniform の細部は本文の初期案から一部変わっている

したがって、以下の各節は「採用した設計意図の記録」として読む。現行挙動の真実は [`docs/world_layout.md`](../../world_layout.md) と [`docs/architecture.md`](../../architecture.md) を優先する。

## 1. 前提（現状のコードとアセット）

### 1.1 レンダリング経路（現行）

- 地形タイルは `MeshMaterial3d<TerrainSurfaceMaterial>` で描画し、`spawn.rs` は全タイルへ
  `Terrain3dHandles.surface` の同一ハンドルを割り当てる。
- `terrain_id_map_sync_system`（`systems/visual/terrain_material.rs`）が `TerrainChangedEvent` を受信して、
  該当セルの `terrain_id_map` ピクセルだけを更新する。`MeshMaterial3d` の handle 差し替えは行わない。
- 建物側は引き続き `SectionMaterial` を使い、地形専用の複数アルベド・metadata・境界ブレンド責務は
  `TerrainSurfaceMaterial` 側へ分離している。

### 1.2 既に実装済み（本書では「前提として利用」）

- `terrain_metadata.rs`: `TerrainFeatureMap` リソースと `build_terrain_feature_map` システム。
  `PostStartup` の先頭で `GeneratedWorldLayoutResource.layout.masks`（`WorldMasks`）から
  `MAP_WIDTH × MAP_HEIGHT` の `Rgba8Unorm` テクスチャを生成し、`commands.insert_resource` で挿入する。
- `terrain_metadata.rs` には `TerrainIdMap` リソースと `build_terrain_id_map` も追加済みで、
  `GeneratedWorldLayoutResource.layout.terrain_tiles` から `R8Unorm` テクスチャを生成する。
- `init_visual_handles` は `Res<TerrainFeatureMap>` と `Res<TerrainIdMap>` を受け取り、
  地形用 `TerrainSurfaceMaterial` の生成に渡す。
- `PostStartup` チェーンは現行以下。本 Phase で変更箇所あり（§6.1 参照）。

  ```
  build_terrain_feature_map
  → build_terrain_id_map
  → init_visual_handles        ← TerrainFeatureMap / TerrainIdMap を参照
  → spawn_map_timed
  → initial_resource_spawner_timed
  → spawn_entities
  → spawn_familiar_wrapper
  → ... （以下省略）
  .chain()
  ```

### 1.3 `SectionMaterialExt` のバインディング番号

`SectionMaterial`（`ExtendedMaterial<StandardMaterial, SectionMaterialExt>`）は **binding 100〜110** を使用している。
`TerrainSurfaceMaterialExt` は独立した別 Material 型なので、**同じ 100 番台を再利用してよい**（異なるシェーダプログラムのバインドグループ）。

### 1.4 本書で実装済みとなった対象

- **`TerrainIdMap`** リソース：セルごとの `TerrainType` を `R8Unorm` テクスチャとして保持。
- **`TerrainSurfaceMaterial`**（`ExtendedMaterial<StandardMaterial, TerrainSurfaceMaterialExt>`）と専用 WGSL 2 枚。
  全地形タイルを **単一 `Handle<TerrainSurfaceMaterial>`** で共有する。
- `terrain_material_sync_system` の責務を **`terrain_id_map` ピクセル更新**へ変更（handle 差し替えは廃止）。

### 1.5 クレート境界

- **`hw_world` は変更しない**。メタデータ生成は `bevy_app` が `GeneratedWorldLayoutResource` を読んで行う。

### 1.6 後退不可条件

- 地形タイルの **`SectionCut`（矢視切断）・prepass・lighting（`pbr_input_from_standard_material`）**
  が現行と同等に機能すること。「地形だけ切断に反応しなくなる」変更は不可。

---

## 2. 問題

- 隣接セルで `TerrainType` が変わると色の段差がシャープすぎる。
- 境界ブレンドには **ID マップ上の近傍参照**と**複数アルベドの同時評価**が必要で、
  現行 `SectionMaterial`（1 インスタンス ＝ 1 アルベド）のモデルでは実現できない。

---

## 3. 方針（解決アプローチ）

1. **`TerrainSurfaceMaterial` を新設**。地形タイルだけをこのマテリアルに移す。建物は `SectionMaterial` のまま。
2. **`terrain_id_map`（`R8Unorm`）** を `Handle<Image>` リソースとして保持し、`TerrainChangedEvent` 時に
   該当ピクセルだけ `ResMut<Assets<Image>>` 経由で書き戻す。
3. フラグメントシェーダで **center の `TerrainType`** に応じてアルベドを選び、
   **カーディナル 4 近傍の ID** を参照して境界付近をソフトブレンドする。
   コーナー（斜め）の複雑化は後回し。実装後はブレンド帯を狭く保ち、river を含むブレンドは `river↔sand` に限定する。
4. **既存 PNG（`terrain_blend_mask_soft.png` / `river_normal_like.png` / `shoreline_detail.png`）** を
   境界・河岸表現に活用する。新規アート制作は必須としない。
5. **単一 `Handle<TerrainSurfaceMaterial>`** を全地形タイルで共有。
   見た目の差は ID テクスチャ・メタテクスチャ・UV で表現。
6. **`TerrainFeatureMap` は static bake のまま**。動的更新対象は `terrain_id_map` に限定する。
   障害物撤去後に Dirt へ変わっても、worldgen 由来の shore / inland / rock field / zone 情報は
   再焼き込みしない（仕様として固定）。
7. **Phase 2 の見た目要素は後退させない**。少なくとも
   `terrain_macro_noise` + terrain 種別ごとの `*_macro_overlay`、
   river flow distortion、feature LUT による shore / inland / rock field の差、
   zone bias の palette bias は `TerrainSurfaceMaterial` 側へ移植する。

---

## 4. 期待できる効果

### 4.1 見た目

- Grass / Dirt / Sand / River の境界が **自然にぼかされる**。
- `terrain_blend_mask_soft` で falloff の形状を調整しやすい。

### 4.2 実装・保守

- 建物と地形のシェーダ責務が分離し、`SectionCut` / prepass / lighting は地形でも維持できる。
- ID マップは部分更新で `WorldMap` との整合を維持しやすい。

### 4.3 パフォーマンス

- **メリット**: 同一マテリアル・同一パイプラインに集約するとドローコールがまとまりやすい。
- **デメリット**: フラグメントで複数テクスチャ参照が増える。分岐はカーディナル 4 近傍に限定して単純に保つ。

---

## 5. アセット（実フォルダとコード登録）

### 5.1 ディスク上の実体

次のファイルは `assets/textures/` 直下にすでに存在する。

| ファイル | 想定用途 | 推奨サンプラー |
| --- | --- | --- |
| `river_normal_like.png` | 川面ノイズ・流れ感補助 | Repeat |
| `terrain_blend_mask_soft.png` | 境界 falloff マスク（in-cell fraction に掛ける） | Clamp-to-Edge（タイル内 0..1 での lookup） |
| `shoreline_detail.png` | 河岸・shore sand 細部 | Clamp-to-Edge |

### 5.2 `GameAssets` への追加フィールド（`assets.rs`）

```rust
// crates/bevy_app/src/assets.rs
pub struct GameAssets {
    // ... 既存フィールド ...
    pub river_normal_like: Handle<Image>,        // 追加
    pub terrain_blend_mask_soft: Handle<Image>,  // 追加
    pub shoreline_detail: Handle<Image>,         // 追加
}
```

### 5.3 `asset_catalog.rs` でのロード

既存の `terrain_sampler`（Repeat）と `terrain_lut_sampler`（ClampToEdge + Nearest）ヘルパーを流用する。
`terrain_blend_mask_soft` と `shoreline_detail` は **ClampToEdge + Linear**（境界でピクセルが折り返さないよう）。

```rust
// 追加が必要な新規ヘルパー（既存 terrain_lut_sampler とほぼ同じだが LinearFilter を使う）
fn terrain_clamp_sampler(s: &mut ImageLoaderSettings) {
    s.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        ..default()  // mag/min フィルタはデフォルト Linear
    });
}

// GameAssets 初期化内
river_normal_like: asset_server.load_with_settings(
    "textures/river_normal_like.png", terrain_sampler),       // Repeat
terrain_blend_mask_soft: asset_server.load_with_settings(
    "textures/terrain_blend_mask_soft.png", terrain_clamp_sampler),  // ClampToEdge
shoreline_detail: asset_server.load_with_settings(
    "textures/shoreline_detail.png", terrain_clamp_sampler),  // ClampToEdge
```

---

## 6. 実装ステップ

### 6.1 `TerrainIdMap` リソースと `build_terrain_id_map` システム

**定義場所**: `crates/bevy_app/src/world/map/terrain_metadata.rs`（既存ファイルに追記）

#### `TerrainIdMap` リソース構造

```rust
/// セルごとの TerrainType を R8Unorm テクスチャとして保持するリソース。
/// シェーダが textureLoad でカーディナル近傍の ID を整数座標で参照するために使う。
#[derive(Resource)]
pub struct TerrainIdMap {
    pub image: Handle<Image>,
}
```

#### テクスチャ形式とエンコーディング

- フォーマット: **`TextureFormat::R8Unorm`**
- サンプラー: `ClampToEdge` + `Nearest`（`TerrainFeatureMap` と同じ設定）
- WGSL 型: `texture_2d<f32>`（`R8Unorm` は Bevy が `f32` としてバインドする）

ID エンコーディングは既存 `TERRAIN_KIND_*` 定数（0〜3）と対応させる：

| `TerrainType` | `u8` | R8Unorm として読むと | `round(val * 3.0)` |
| --- | --- | --- | --- |
| `Grass`  | `0`   | `0.000` | 0 → TERRAIN_KIND_GRASS  |
| `Dirt`   | `85`  | `0.333` | 1 → TERRAIN_KIND_DIRT   |
| `Sand`   | `170` | `0.667` | 2 → TERRAIN_KIND_SAND   |
| `River`  | `255` | `1.000` | 3 → TERRAIN_KIND_RIVER  |

#### `build_terrain_id_map` の骨格

```rust
pub fn build_terrain_id_map(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    layout: Res<GeneratedWorldLayoutResource>,
) {
    let w = MAP_WIDTH as usize;
    let h = MAP_HEIGHT as usize;

    // terrain_feature_map と同じ y/x ループで同じ座標基準を共有する
    let mut pixels: Vec<u8> = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            pixels.push(terrain_type_to_id_byte(layout.layout.terrain_tiles[idx]));
        }
    }

    let mut image = Image::new(
        Extent3d { width: w as u32, height: h as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        pixels,
        TextureFormat::R8Unorm,
        default(),
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Nearest,
        min_filter: ImageFilterMode::Nearest,
        ..default()
    });

    commands.insert_resource(TerrainIdMap { image: images.add(image) });
}

fn terrain_type_to_id_byte(terrain: TerrainType) -> u8 {
    match terrain {
        TerrainType::Grass => 0,
        TerrainType::Dirt  => 85,
        TerrainType::Sand  => 170,
        TerrainType::River => 255,
    }
}
```

#### `PostStartup` チェーン変更

`build_terrain_id_map` を `build_terrain_feature_map` の直後・`init_visual_handles` の直前に挿入する。
`init_visual_handles` で `Res<TerrainIdMap>` を参照するため、順序は必ずこの順でなければならない。

```rust
// plugins/startup/mod.rs
.add_systems(
    PostStartup,
    (
        build_terrain_feature_map,
        build_terrain_id_map,       // ← 追加（TerrainFeatureMap 後・init_visual_handles 前）
        init_visual_handles,
        spawn_map_timed,
        // ... 以下変更なし
    ).chain(),
)
```

`terrain_metadata.rs` の `mod.rs` で `pub use build_terrain_id_map, TerrainIdMap;` を追加。

#### 座標変換仕様（シェーダ・Rust 共通）

```
pixel_x = clamp(floor((world_xz.x + half_map_w) / TILE_SIZE), 0, MAP_WIDTH  - 1)
pixel_y = clamp(floor((-world_xz.y + half_map_h) / TILE_SIZE), 0, MAP_HEIGHT - 1)
```

- ワールド原点はマップ中心（既存 `sample_feature()` と同じ基準）
- テクスチャ `v` は top = y=0 → `world_z` を反転
- `terrain_id_map` 更新時も同じ計算式で `x = ev.idx % MAP_WIDTH`、`y = ev.idx / MAP_WIDTH`

---

### 6.2 `TerrainSurfaceMaterial` の型定義（`hw_visual`）

**新規ファイル**: `crates/hw_visual/src/material/terrain_surface_material.rs`

#### ユニフォーム構造体

```rust
#[derive(Clone, Copy, Debug, ShaderType, Reflect)]
pub struct TerrainSurfaceUniform {
    // SectionCut（SectionMaterialUniform と同一レイアウト・同一意味）
    pub cut_position:  Vec4,
    pub cut_normal:    Vec4,
    pub thickness:     f32,
    pub cut_active:    f32,
    // Terrain meta
    pub map_world_width:      f32,   // MAP_WIDTH  * TILE_SIZE
    pub map_world_height:     f32,   // MAP_HEIGHT * TILE_SIZE
    pub uv_scale:             f32,   // 1.0 / TILE_SIZE（アルベド repeat UV）
    pub blend_strength:       f32,   // 境界ブレンド強度係数（デフォルト 1.0）
    pub macro_noise_scale:    f32,   // 共通低周波ノイズの world-space scale
    pub overlay_scale:        f32,   // macro overlay の world-space scale
    pub tile_size:            f32,   // `hw_core::constants::TILE_SIZE` と同一（シェーダの world→cell で使用）
}
```

**Rust `TerrainSurfaceUniform` と WGSL `TerrainSurfaceUniforms` は同一フィールド順・同一意味とする**（§6.7）。`ShaderType` / `encase` のアライメントで末尾パディングが入る場合は、実装時に両者のメモリレイアウトを一致させる。

`TerrainSurfaceUniform` は共有 material 全体で共通の値だけを持つ。
**terrain 種別ごとの挙動差（river scroll、grass/dirt/sand の warp / brightness / UV distort 強度）は
uniform に載せず、shader 側の helper 定数関数で現行値を再現**する。

```wgsl
fn terrain_uv_scroll_speed(id: u32) -> f32 {
    if id == 3u { return 0.03; } // river
    return 0.0;
}

fn terrain_domain_warp_strength(id: u32) -> f32 {
    switch id {
        case 0u: { return 16.0; } // grass
        case 1u: { return 12.0; } // dirt
        case 2u: { return 10.0; } // sand
        default: { return 0.0; }  // river
    }
}

fn terrain_brightness_strength(id: u32) -> f32 {
    switch id {
        case 0u: { return 0.08; } // grass
        case 1u: { return 0.10; } // dirt
        case 2u: { return 0.08; } // sand
        default: { return 0.0; }  // river
    }
}

fn terrain_uv_distort_strength(id: u32) -> f32 {
    if id == 0u { return 0.03; } // grass only
    return 0.0;
}
```

#### マテリアル拡張

```rust
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct TerrainSurfaceMaterialExt {
    #[uniform(100)]
    pub uniforms: TerrainSurfaceUniform,

    // --- ID マップ（R8Unorm, nearest, clamp） ---
    #[texture(101)]
    #[sampler(102)]
    pub terrain_id_map: Option<Handle<Image>>,

    // --- フィーチャーマップ（Rgba8Unorm, nearest, clamp） ---
    #[texture(103)]
    #[sampler(104)]
    pub terrain_feature_map: Option<Handle<Image>>,

    // --- 4 アルベド（repeat） ---
    #[texture(105)]
    #[sampler(106)]
    pub grass_albedo: Option<Handle<Image>>,

    #[texture(107)]
    #[sampler(108)]
    pub dirt_albedo: Option<Handle<Image>>,

    #[texture(109)]
    #[sampler(110)]
    pub sand_albedo: Option<Handle<Image>>,

    #[texture(111)]
    #[sampler(112)]
    pub river_albedo: Option<Handle<Image>>,

    // --- 共通ノイズ類（repeat） ---
    #[texture(113)]
    #[sampler(114)]
    pub terrain_macro_noise: Option<Handle<Image>>,

    // --- terrain 種別ごとの macro overlay（repeat） ---
    #[texture(115)]
    #[sampler(116)]
    pub grass_macro_overlay: Option<Handle<Image>>,

    #[texture(117)]
    #[sampler(118)]
    pub dirt_macro_overlay: Option<Handle<Image>>,

    #[texture(119)]
    #[sampler(120)]
    pub sand_macro_overlay: Option<Handle<Image>>,

    // --- 境界ブレンドマスク（clamp） ---
    #[texture(121)]
    #[sampler(122)]
    pub terrain_blend_mask_soft: Option<Handle<Image>>,

    // --- 川ノイズ（repeat） ---
    #[texture(123)]
    #[sampler(124)]
    pub river_flow_noise: Option<Handle<Image>>,

    // --- 川面/河岸ディテール ---
    #[texture(125)]
    #[sampler(126)]
    pub river_normal_like: Option<Handle<Image>>,

    #[texture(127)]
    #[sampler(128)]
    pub shoreline_detail: Option<Handle<Image>>,

    // --- フィーチャー LUT（clamp, nearest） ---
    #[texture(129)]
    #[sampler(130)]
    pub terrain_feature_lut: Option<Handle<Image>>,
}

impl MaterialExtension for TerrainSurfaceMaterialExt {
    fn fragment_shader() -> ShaderRef {
        "shaders/terrain_surface_material.wgsl".into()
    }
    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/terrain_surface_material_prepass.wgsl".into()  // 専用 prepass（後述）
    }
}

pub type TerrainSurfaceMaterial = ExtendedMaterial<StandardMaterial, TerrainSurfaceMaterialExt>;
```

#### prepass シェーダ

`section_material_prepass.wgsl` は `SectionMaterialUniforms`（binding 100）を参照しており
`TerrainSurfaceUniform` とフィールド名が異なる。**新規 `terrain_surface_material_prepass.wgsl` を作成**する。

内容はほぼ `section_material_prepass.wgsl` のコピーだが、uniform 型名・struct 定義を
`TerrainSurfaceUniform` に合わせる（§6.2 の全フィールド。`cut_*` の意味は `SectionMaterial` と同じ）。

---

### 6.3 `SectionCut` の同期拡張

現行 `sync_section_cut_to_materials_system` は `ResMut<Assets<SectionMaterial>>` を走査しており、
`TerrainSurfaceMaterial` は対象外。

**対応**: `TerrainSurfaceMaterial` を対象にする専用システムを追加する（既存システムは変更しない）。

```rust
// hw_visual/src/material/terrain_surface_material.rs に追加
pub fn sync_section_cut_to_terrain_surface_system(
    cut: Res<SectionCut>,
    mut materials: ResMut<Assets<TerrainSurfaceMaterial>>,
) {
    if !cut.is_changed() {
        return;
    }
    let cut_pos   = cut.position.extend(0.0);
    let cut_nor   = cut.normal.normalize_or_zero().extend(0.0);
    let thickness = cut.thickness.max(0.0);
    let active    = if cut.active { 1.0 } else { 0.0 };

    for (_, mat) in materials.iter_mut() {
        let u = &mut mat.extension.uniforms;
        u.cut_position = cut_pos;
        u.cut_normal   = cut_nor;
        u.thickness    = thickness;
        u.cut_active   = active;
    }
}
```

このシステムを `sync_section_cut_to_materials_system` と同じスケジュール位置（`Visual` セット内）に登録する。

---

### 6.4 `TerrainSurfaceMaterial` 初期化（`visual_handles.rs`）

#### `Terrain3dHandles` 型変更

```rust
// 変更前
pub struct Terrain3dHandles {
    pub tile_mesh: Handle<Mesh>,
    pub grass:     Handle<SectionMaterial>,
    pub dirt:      Handle<SectionMaterial>,
    pub sand:      Handle<SectionMaterial>,
    pub river:     Handle<SectionMaterial>,
}

// 変更後
pub struct Terrain3dHandles {
    pub tile_mesh: Handle<Mesh>,
    pub surface:   Handle<TerrainSurfaceMaterial>,  // 全地形タイルで共有する単一ハンドル
}
```

#### `init_visual_handles` の変更

`InitVisualHandlesParams` に `Res<TerrainIdMap>` を追加し、4 インスタンスを 1 つに集約する。

```rust
// params への追加
terrain_id_map: Res<'w, TerrainIdMap>,
terrain_surface_materials: ResMut<'w, Assets<TerrainSurfaceMaterial>>,

// --- 地形 3D ハンドル（変更後） ---
let terrain_tile_mesh = meshes.add(Plane3d::default().mesh().size(TILE_SIZE, TILE_SIZE));

let terrain_surface = terrain_surface_materials.add(TerrainSurfaceMaterial {
    base: StandardMaterial {
        perceptual_roughness: 1.0,
        reflectance: 0.0,
        opaque_render_method: OpaqueRendererMethod::Forward,
        ..default()
    },
    extension: TerrainSurfaceMaterialExt {
        uniforms: TerrainSurfaceUniform {
            cut_position:         Vec4::ZERO,
            cut_normal:           Vec3::NEG_Z.extend(0.0),
            thickness:            TILE_SIZE * 5.0,
            cut_active:           0.0,
            map_world_width:      MAP_WIDTH  as f32 * TILE_SIZE,
            map_world_height:     MAP_HEIGHT as f32 * TILE_SIZE,
            uv_scale:             1.0 / TILE_SIZE,
            blend_strength:       1.0,
            macro_noise_scale:    0.00045,
            overlay_scale:        0.0012,
            tile_size:            TILE_SIZE,
        },
        terrain_id_map:           Some(params.terrain_id_map.image.clone()),
        terrain_feature_map:      Some(feature_map_handle),
        grass_albedo:             Some(game_assets.grass.clone()),
        dirt_albedo:              Some(game_assets.dirt.clone()),
        sand_albedo:              Some(game_assets.sand.clone()),
        river_albedo:             Some(game_assets.river.clone()),
        terrain_macro_noise:      Some(game_assets.terrain_macro_noise.clone()),
        grass_macro_overlay:      Some(game_assets.grass_macro_overlay.clone()),
        dirt_macro_overlay:       Some(game_assets.dirt_macro_overlay.clone()),
        sand_macro_overlay:       Some(game_assets.sand_macro_overlay.clone()),
        terrain_blend_mask_soft:  Some(game_assets.terrain_blend_mask_soft.clone()),
        river_flow_noise:         Some(game_assets.river_flow_noise.clone()),
        river_normal_like:        Some(game_assets.river_normal_like.clone()),
        shoreline_detail:         Some(game_assets.shoreline_detail.clone()),
        terrain_feature_lut:      Some(game_assets.terrain_feature_lut.clone()),
    },
});

commands.insert_resource(Terrain3dHandles {
    tile_mesh: terrain_tile_mesh,
    surface:   terrain_surface,
});
```

`TERRAIN_GRASS_UV_DISTORT_STRENGTH` 等のタイル種別ごとの定数は、シェーダ内の helper 関数へ移し、
**現行 `SectionMaterial` と同じ値を維持する**。共有 material 化に伴って見た目を単純化しない。

---

### 6.5 `spawn.rs` の変更

`terrain_material()` ヘルパー関数は削除し、スポーン時に全タイルへ同一ハンドルを付与する。

```rust
// 変更前
let material = terrain_material(terrain, &terrain_handles);
// 変更後
let material = terrain_handles.surface.clone();
```

`MeshMaterial3d::<SectionMaterial>` を `MeshMaterial3d::<TerrainSurfaceMaterial>` に変更する。

---

### 6.6 `terrain_material.rs` の変更（責務の置き換え）

ファイル名は `terrain_material.rs` のままでよいが、システム名を変更する。

#### 変更後のシステム

```rust
/// テレイン変更後に terrain_id_map の対応ピクセルを更新するシステム。
/// TerrainChangedEvent を受信し、Handle<TerrainSurfaceMaterial> の差し替えは行わない。
pub fn terrain_id_map_sync_system(
    world_map: WorldMapRead,
    terrain_id_map: Res<TerrainIdMap>,
    mut images: ResMut<Assets<Image>>,
    mut events: MessageReader<TerrainChangedEvent>,
) {
    for ev in events.read() {
        let Some(terrain) = world_map.terrain_at_idx(ev.idx) else { continue; };
        let Some(image) = images.get_mut(&terrain_id_map.image) else { continue; };

        let x = ev.idx % MAP_WIDTH as usize;
        let y = ev.idx / MAP_WIDTH as usize;
        let pixel_idx = y * MAP_WIDTH as usize + x;  // R8Unorm: 1 byte/pixel

        // `TextureFormat::R8Unorm` かつ非圧縮の生バッファであること（`asset` 側で圧縮 Image にしない）
        image.data.as_mut().expect("terrain_id_map must be uncompressed R8Unorm")[pixel_idx]
            = terrain_type_to_id_byte(terrain);
    }
}

// 再利用のため terrain_type_to_id_byte を pub(crate) で terrain_metadata.rs に定義し
// ここから import する
```

#### システム登録の変更

`visual.rs` または登録箇所で `terrain_material_sync_system` → `terrain_id_map_sync_system` に差し替える。
`Terrain3dHandles` の参照も削除（`terrain_id_map_sync_system` は `TerrainIdMap` を使うため）。

---

### 6.7 シェーダ `assets/shaders/terrain_surface_material.wgsl`

#### バインディング宣言（先頭部）

```wgsl
#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::globals,
}

// §6.2 の `TerrainSurfaceUniform` と同一レイアウト（フィールド名は WGSL 慣例で snake_case）
struct TerrainSurfaceUniforms {
    cut_position:      vec4<f32>,
    cut_normal:        vec4<f32>,
    thickness:         f32,
    cut_active:        f32,
    map_world_width:   f32,
    map_world_height:  f32,
    uv_scale:          f32,
    blend_strength:    f32,
    macro_noise_scale: f32,
    overlay_scale:     f32,
    tile_size:         f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> tsm: TerrainSurfaceUniforms;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var terrain_id_map:           texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var terrain_id_sampler:       sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var terrain_feature_map:      texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var terrain_feature_sampler:  sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(105) var grass_albedo:             texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(106) var grass_sampler:            sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(107) var dirt_albedo:              texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(108) var dirt_sampler:             sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(109) var sand_albedo:              texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(110) var sand_sampler:             sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(111) var river_albedo:             texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(112) var river_sampler:            sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(113) var terrain_macro_noise:      texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(114) var macro_noise_sampler:      sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(115) var grass_macro_overlay:      texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(116) var grass_overlay_sampler:    sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(117) var dirt_macro_overlay:       texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(118) var dirt_overlay_sampler:     sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(119) var sand_macro_overlay:       texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(120) var sand_overlay_sampler:     sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(121) var blend_mask_soft:          texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(122) var blend_mask_sampler:       sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(123) var river_flow_noise:         texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(124) var river_flow_sampler:       sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(125) var river_normal_like:        texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(126) var river_normal_sampler:     sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(127) var shoreline_detail:         texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(128) var shoreline_detail_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(129) var terrain_feature_lut:      texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(130) var feature_lut_sampler:      sampler;
```

実装時は **`TerrainSurfaceUniform` の `ShaderType` レイアウト**と WGSL の `var<uniform>` を一致させる（`f32` 列のパディングが入る場合は **encase / naga の生成結果**に合わせて `TerrainSurfaceUniform` 側に `_padding` 等を足す）。

#### 座標変換ヘルパー

`sample_feature` の既存実装と同じ規則（`-world_xz.y` で v 反転）を踏む。

```wgsl
/// world_xz → terrain_id_map 上のセル整数座標（textureLoad 用）
/// `tile_size` は `TILE_SIZE`（ワールド単位 1 セルの幅・高さ）と一致させる。
fn world_to_cell(world_xz: vec2<f32>) -> vec2<i32> {
    let half_w = tsm.map_world_width  * 0.5;
    let half_h = tsm.map_world_height * 0.5;
    let tw = tsm.tile_size;
    let map_w  = i32(tsm.map_world_width  / tw);
    let map_h  = i32(tsm.map_world_height / tw);
    let ix = clamp(i32(floor((world_xz.x + half_w) / tw)), 0, map_w - 1);
    let iy = clamp(i32(floor((-world_xz.y + half_h) / tw)), 0, map_h - 1);
    return vec2(ix, iy);
}

/// セル座標 → terrain_id（0=Grass, 1=Dirt, 2=Sand, 3=River）
fn cell_terrain_id(cell: vec2<i32>) -> u32 {
    let raw = textureLoad(terrain_id_map, cell, 0).r;
    return u32(round(raw * 3.0));
}

/// terrain_id に応じてアルベドをサンプル
fn sample_albedo(id: u32, uv: vec2<f32>) -> vec3<f32> {
    switch id {
        case 1u: { return textureSample(dirt_albedo,  dirt_sampler,  uv).rgb; }
        case 2u: { return textureSample(sand_albedo,  sand_sampler,  uv).rgb; }
        case 3u: { return textureSample(river_albedo, river_sampler, uv).rgb; }
        default: { return textureSample(grass_albedo, grass_sampler, uv).rgb; }
    }
}

fn sample_macro_overlay(id: u32, world_xz: vec2<f32>) -> f32 {
    let uv = world_xz * tsm.overlay_scale;
    switch id {
        case 1u: { return textureSample(dirt_macro_overlay, dirt_overlay_sampler, uv).r * 2.0 - 1.0; }
        case 2u: { return textureSample(sand_macro_overlay, sand_overlay_sampler, uv).r * 2.0 - 1.0; }
        default: { return textureSample(grass_macro_overlay, grass_overlay_sampler, uv).r * 2.0 - 1.0; }
    }
}
```

#### カーディナルブレンドのロジック（フラグメント内）

```wgsl
fn blend_terrain(world_xz: vec2<f32>, base_uv: vec2<f32>) -> vec3<f32> {
    let cell_c = world_to_cell(world_xz);
    let id_c   = cell_terrain_id(cell_c);

    // セル内フラクション（0..1, left/top 起点）
    let half_w  = tsm.map_world_width  * 0.5;
    let half_h  = tsm.map_world_height * 0.5;
    let tw = tsm.tile_size;
    let cell_local = vec2(
        fract((world_xz.x + half_w) / tw),
        fract((-world_xz.y + half_h) / tw),
    );

    let albedo_c = sample_albedo(id_c, base_uv);

    // 境界ブレンドマスク（テクスチャ or 自前関数）
    // blend_mask_soft は 0..1 の falloff 形状。セル内 UV でサンプル。
    let mask = textureSample(blend_mask_soft, blend_mask_sampler, cell_local).r;

    // カーディナル 4 方向の重み（セルエッジに近いほど大きい）
    let w_n = max(0.0, 0.5 - cell_local.y);  // 上エッジ付近
    let w_s = max(0.0, cell_local.y - 0.5);  // 下エッジ付近
    let w_e = max(0.0, cell_local.x - 0.5);  // 右エッジ付近
    let w_w = max(0.0, 0.5 - cell_local.x);  // 左エッジ付近

    // 近傍 ID を取得し、異タイプのみブレンド。
    // `neighbors` は `world_to_cell` / `textureLoad` のセル座標系（`iy` 増加 = -world_z 側）に合わせる。
    // 上下が逆に見える場合は N/S のオフセットを入れ替えて検証する。
    var accum = albedo_c;
    var accum_weight = 1.0;
    let neighbors = array<vec2<i32>, 4>(
        cell_c + vec2( 0, -1),  // N（テクスチャ row 上 = y-1）
        cell_c + vec2( 0,  1),  // S
        cell_c + vec2( 1,  0),  // E
        cell_c + vec2(-1,  0),  // W
    );
    let weights = array<f32, 4>(w_n, w_s, w_e, w_w);

    for (var i = 0u; i < 4u; i++) {
        let id_n = cell_terrain_id(neighbors[i]);
        if id_n != id_c {
            let w = weights[i] * 2.0 * mask * tsm.blend_strength;
            let wn = clamp(w, 0.0, 1.0);
            accum += sample_albedo(id_n, base_uv) * wn;
            accum_weight += wn;
        }
    }
    return accum / accum_weight;
}
```

境界ブレンドは **逐次 `mix` で上書きしない**。中心 + 近傍の寄与を重み付き和で集約し、
`accum_weight` で正規化して順序依存を避ける。

#### Phase 2 見た目要素の移植方針

- **macro brightness / domain warp**:
  `terrain_macro_noise` と terrain 種別ごとの `*_macro_overlay` を合成し、
  現行 `section_material.wgsl` と同じ係数で brightness / warp を決める。
- **river flow**:
  `river_flow_noise` に加えて `river_normal_like` を流れ方向の微細変調へ使い、
  少なくとも現行の「左→右のうねり」は維持する。
- **shoreline detail**:
  `shoreline_detail` は shore sand のみに掛け、`TerrainFeatureMap.r` を gate にして
  inland sand へ漏らさない。
- **feature grading**:
  `terrain_feature_lut` と `TerrainFeatureMap` を用い、
  shore / inland / rock field / zone bias のロジックは現行 shader から移植する。

#### `section_discard` の移植

`SectionMaterial` と同じロジックを `terrain_surface_material.wgsl` 先頭にコピーする（`tsm.` prefix で参照）。

---

### 6.8 `hw_visual/src/lib.rs` への登録

```rust
// 追加
use bevy::pbr::MaterialPlugin;
// 既存の MaterialPlugin 登録ブロックに追記
MaterialPlugin::<material::TerrainSurfaceMaterial>::default(),
```

`material/mod.rs` に `pub mod terrain_surface_material;` と必要な `pub use` を追加。

---

## 7. 変更ファイル一覧

| 種別 | パス | 変更内容 |
| --- | --- | --- |
| アセット定義 | `crates/bevy_app/src/assets.rs` | `river_normal_like` / `terrain_blend_mask_soft` / `shoreline_detail` フィールド追加 |
| アセットカタログ | `crates/bevy_app/src/plugins/startup/asset_catalog.rs` | 上記 3 ファイルのロード追加、`terrain_clamp_sampler` ヘルパー追加 |
| ハンドル初期化 | `crates/bevy_app/src/plugins/startup/visual_handles.rs` | `Terrain3dHandles` 型変更（4→1 ハンドル）、`init_visual_handles` 変更 |
| スタートアップ | `crates/bevy_app/src/plugins/startup/mod.rs` | `build_terrain_id_map` を PostStartup チェーンに挿入、`pub use` 追加 |
| メタデータ | `crates/bevy_app/src/world/map/terrain_metadata.rs` | `TerrainIdMap` / `build_terrain_id_map` / `terrain_type_to_id_byte` 追加 |
| マップ mod | `crates/bevy_app/src/world/map/mod.rs` | `pub use build_terrain_id_map, TerrainIdMap;` 追加 |
| スポーン | `crates/bevy_app/src/world/map/spawn.rs` | `terrain_material()` 削除、スポーン時に `handles.surface.clone()` |
| 地形更新 | `crates/bevy_app/src/systems/visual/terrain_material.rs` | `terrain_material_sync_system` → `terrain_id_map_sync_system` に完全置き換え |
| マテリアル（新規） | `crates/hw_visual/src/material/terrain_surface_material.rs` | `TerrainSurfaceUniform` / `TerrainSurfaceMaterialExt` / `TerrainSurfaceMaterial` / `sync_section_cut_to_terrain_surface_system` |
| マテリアル mod | `crates/hw_visual/src/material/mod.rs` | `pub mod terrain_surface_material;` + `pub use` |
| hw_visual lib | `crates/hw_visual/src/lib.rs` | `MaterialPlugin::<TerrainSurfaceMaterial>` 登録、SectionCut sync システム追加登録 |
| シェーダ（新規） | `assets/shaders/terrain_surface_material.wgsl` | フラグメントシェーダ本体 |
| シェーダ（新規） | `assets/shaders/terrain_surface_material_prepass.wgsl` | `section_material_prepass.wgsl` ベースに uniform 型名を `TerrainSurfaceUniform` へ変更 |

---

## 8. 検証方法

1. `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`
2. `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace`（警告ゼロ）
3. 同一 `HELL_WORKERS_WORLDGEN_SEED` で before / after スクリーンショット比較。
4. 異タイプ境界が **過度ににじみすぎていない**こと、かつシャープすぎないこと。
5. **`SectionCut` が地形・建物ともに正常**に動作すること（建物は `SectionMaterial`、地形は `TerrainSurfaceMaterial`）。
6. `TerrainChangedEvent` 後、**境界ブレンドが更新後の `TerrainType` を反映**していること
   （障害物撤去後 → Dirt の境界が Dirt として正しくブレンドされる）。
7. **`TerrainFeatureMap` 由来の metadata は static bake のまま**であること（仕様どおり）。
8. `terrain_id_map` 用 `Image` が **非圧縮**であり、`terrain_id_map_sync_system` でピクセル書き換えが可能であること。

---

## 9. 関連ドキュメント

- [地形ビジュアル再検討（2026-04-05）](terrain-visual-reassessment-2026-04-05.md)
- [MS-3-6 テレイン表面表現改善](ms-3-6-terrain-surface-plan-2026-03-31.md)
- [docs/crate-boundaries.md](../../crate-boundaries.md)
