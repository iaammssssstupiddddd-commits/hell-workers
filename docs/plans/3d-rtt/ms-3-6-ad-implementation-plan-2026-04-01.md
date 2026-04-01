# MS-3-6 A/D 実装計画（現行アセット限定）

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `ms-3-6-ad-implementation-plan-2026-04-01` |
| ステータス | `Implemented`（恒久仕様は `docs/world_layout.md` / `docs/architecture.md` を参照） |
| 親計画 | [`ms-3-6-terrain-surface-plan-2026-03-31.md`](ms-3-6-terrain-surface-plan-2026-03-31.md) **§2・§3・§4** |
| 親マイルストーン | `milestone-roadmap.md` **MS-3-6** |
| スコープ | **方針 A（シェーダ）**と **方針 D のうち現行 PNG のみで可能な範囲**。**新規テクスチャ差し替え・B/C は含まない** |

---

## 1. 目的と制約

### 目的

- [`asset_catalog.rs`](../../../crates/bevy_app/src/plugins/startup/asset_catalog.rs) が参照する **現行 4 枚**だけで、地形のタイリング・シーム・川の静止感を **コードとシェーダ**で改善する。
- 親計画の **A（ワールド UV・Stochastic 回転・低周波歪み・弱い明度変調）** と **D（マテリアル／サンプラ側でできること）** を実装する。

### 硬い制約

| 項目 | 内容 |
| --- | --- |
| **テクスチャ** | `textures/grass.png`・`dirt.png`・`sand_terrain.png`・`river.png` の **4 枚のみ**。**新規 PNG・差し替えは本計画に含めない**（後続 MS-Asset-Terrain）。 |
| **対象メッシュ** | 地形タイル（`Plane3d` 共有メッシュ）のみ **A の UV 論理**を適用。**建物・壁など他 `SectionMaterial` は従来どおりメッシュ UV**（後述 §2）。 |

---

## 2. 最重要：地形と非地形の分岐

`SectionMaterial` は **建物・壁・地形で共有**される。フラグメントで **常に** `world_position.xz` からアルベド UV を計算すると、**直立メッシュの側面アルベドが破綻**する。

**採用方針**: `SectionMaterialUniform` に `albedo_uv_mode: f32` を追加し、地形用ヘルパで `1.0` を設定。建物側は `0.0`（デフォルト）のまま。地形専用 `MaterialExtension` に分割するよりコストが低く、既存の `sync_section_cut_to_materials_system` との整合も取りやすい。

---

## 3. 方針 D — 現行アセットのみでできること

| # | 内容 | 対象ファイル | 優先度 |
| --- | --- | --- | --- |
| D1 | `StandardMaterial` 基底の `perceptual_roughness: 1.0` / `reflectance: 0.0` は既に設定済み。追加調整は不要（確認のみ）。 | `section_material.rs` | 確認のみ |
| D2 | **`AddressMode::Repeat` 設定**。Bevy デフォルトは `ClampToEdge` のため、ワールド空間 UV で 0〜1 を超えると端で引き伸びる。**A1 と同時に実施する必須作業**。 | `asset_catalog.rs` | **必須（A1 と同時）** |
| D3 | 四辺シーム・ミップモアレの目視チェック（Parent plan §3.0）。問題は後続 MS-Asset-Terrain にチケット化。ファイル差し替えは行わない。 | 検証メモ | 任意 |

### D2 実装詳細 — `AddressMode::Repeat`

**変更ファイル**: `crates/bevy_app/src/plugins/startup/asset_catalog.rs`

`asset_server.load("textures/grass.png")` を `load_with_settings` に変更。**4 枚すべて**に適用：

```rust
use bevy::render::texture::{ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
// bevy::render::render_resource::AddressMode は ImageSamplerDescriptor を通じてアクセス

let terrain_sampler = |s: &mut ImageLoaderSettings| {
    s.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: bevy::render::render_resource::AddressMode::Repeat,
        address_mode_v: bevy::render::render_resource::AddressMode::Repeat,
        ..default()
    });
};

GameAssets {
    grass: asset_server.load_with_settings("textures/grass.png", terrain_sampler),
    dirt:  asset_server.load_with_settings("textures/dirt.png",  terrain_sampler),
    sand:  asset_server.load_with_settings("textures/sand_terrain.png", terrain_sampler),
    river: asset_server.load_with_settings("textures/river.png", terrain_sampler),
    // 他フィールドは変更なし
    ..
}
```

> **注意**: `let terrain_sampler = |s| {...}` は `FnMut` が Move になり 2 回目以降に使えない。**`fn terrain_sampler(s: &mut ImageLoaderSettings) { ... }` と `fn` で定義するのが最もシンプル**（`fn` は `Copy` なので 4 回渡せる）。Bevy 0.18 の `load_with_settings` シグネチャを `~/.cargo/registry/src/` で確認してから実装すること。

**本計画に含めない（後続）**

- 4 枚の **リペイント・シームレス再出力**
- **`river_frame2.png`** によるフレーム切替（親計画 §3.4 方式 B）

---

## 4. 方針 A — シェーダ実装手順

実装順は **親計画 §4・§9** と整合させる。

### ステップ A0 — Rust 側 uniform 拡張

**変更ファイル**: `crates/hw_visual/src/material/section_material.rs`

#### 4-1. `SectionMaterialUniform` に 4 フィールド追加

現在のサイズ: 2×`Vec4`(32 bytes) + 4×`f32`(16 bytes) = **48 bytes**（16-byte aligned）。
追加後: 48 + 16 = **64 bytes**（16-byte aligned を維持）。

```rust
#[derive(Clone, Copy, Debug, ShaderType, Reflect)]
pub struct SectionMaterialUniform {
    // 既存フィールド（変更なし）
    pub cut_position:   Vec4,
    pub cut_normal:     Vec4,
    pub thickness:      f32,
    pub cut_active:     f32,
    pub build_progress: f32,
    pub wall_height:    f32,
    // 新規追加（4 f32 = 16 bytes、アライメント維持）
    pub albedo_uv_mode:    f32, // 0.0=メッシュ UV（建物など）, 1.0=ワールド XZ（地形）
    pub uv_scale:          f32, // UV スケール = 1.0 / TILE_SIZE。スクロールにも共用
    pub uv_scroll_speed:   f32, // river: ~0.003、その他: 0.0（停止）
    pub _pad:              f32, // 将来拡張用パディング
}
```

`Default` 実装で新フィールドを `0.0` に設定する（建物は `albedo_uv_mode = 0.0` のまま動作）。

#### 4-2. `make_terrain_section_material` ヘルパを追加

```rust
use hw_core::constants::TILE_SIZE;

/// 地形タイル専用 SectionMaterial。ワールド UV モードを有効化する。
pub fn make_terrain_section_material(
    texture: Handle<Image>,
    uv_scroll_speed: f32,           // river: 0.03、grass/dirt/sand: 0.0
    uv_distort_strength: f32,       // 草のみ TERRAIN_GRASS_UV_DISTORT_STRENGTH、他は 0.0
    brightness_variation_strength: f32, // 草のみ TERRAIN_GRASS_BRIGHTNESS_VARIATION_STRENGTH、他は 0.0
) -> SectionMaterial { ... }
```

> **実装メモ**: `SectionMaterialUniform` は `Default` を impl しておらず、`..Default::default()` は使えない。全フィールドを明示的に初期化する（`_pad_section_tail_0/1/2` も `0.0`）。

#### 4-3. `visual_handles.rs` の `Terrain3dHandles` 生成を差し替え

**変更ファイル**: `crates/bevy_app/src/plugins/startup/visual_handles.rs`

```rust
// Before:
let terrain_grass = section_materials.add(make_section_material_textured(game_assets.grass.clone()));
// ...

// After（実際の引数: texture, scroll_speed, uv_distort_strength, brightness_variation_strength）:
let terrain_grass = section_materials.add(make_terrain_section_material(game_assets.grass.clone(), 0.0, TERRAIN_GRASS_UV_DISTORT_STRENGTH, TERRAIN_GRASS_BRIGHTNESS_VARIATION_STRENGTH));
let terrain_dirt  = section_materials.add(make_terrain_section_material(game_assets.dirt.clone(),  0.0, 0.0, 0.0));
let terrain_sand  = section_materials.add(make_terrain_section_material(game_assets.sand.clone(),  0.0, 0.0, 0.0));
let terrain_river = section_materials.add(make_terrain_section_material(game_assets.river.clone(), 0.03, 0.0, 0.0));
```

`Building3dHandles` 側（`make_section_material` 経由）は **一切変更しない**。

#### 4-4. `sync_section_cut_to_materials_system` — 変更不要

同システムは `cut_position` / `cut_normal` / `thickness` / `cut_active` の **4 フィールドを名前指定で書き込む**だけなので、新フィールド（`albedo_uv_mode` 等）を上書きしない。変更不要。

---

### ステップ A1 — ワールド空間アルベド UV（最優先）

**変更ファイル**: `assets/shaders/section_material.wgsl`（+ `section_material_prepass.wgsl`）

**必ず D2（AddressMode::Repeat）と同時に実装すること。** UV が 1.0 を超えるため、ClampToEdge のままでは端が引き伸びる。

#### WGSL uniform struct を両シェーダで更新

```wgsl
// section_material.wgsl / section_material_prepass.wgsl 共通
struct SectionMaterialUniforms {
    cut_position:      vec4<f32>,
    cut_normal:        vec4<f32>,
    thickness:         f32,
    cut_active:        f32,
    build_progress:    f32,
    wall_height:       f32,
    // 新規追加
    albedo_uv_mode:    f32,
    uv_scale:          f32,
    uv_scroll_speed:   f32,
    _pad:              f32,
}
```

> **両ファイルを必ず同時に更新すること**。struct 定義が異なると GPU バインディングが壊れる。

#### `section_material.wgsl` の fragment — UV 分岐

```wgsl
// globals.time 参照のためインポートを追加（既存 imports の末尾に追記）
#import bevy_pbr::mesh_view_bindings::globals

// fragment 関数内、pbr_input_from_standard_material() の前に挿入:
if section_material.albedo_uv_mode > 0.5 {
    // ワールド XZ ベース UV（タイル境界でリセットされない）
    let world_uv = compute_terrain_uv(in.world_position.xz);
    // PBR 入力の UV を上書き（bevy_pbr は in.uv をテクスチャサンプルに使う）
    // → pbr_input_from_standard_material を呼ぶ前に in.uv を差し替えるか、
    //   または pbr_input.material.base_color をテクスチャサンプルで上書きする。
    // 実装方法は §4 末尾の「UV 上書き方法」を参照。
}
```

> **UV 上書き方法**: `pbr_input_from_standard_material(in, is_front)` は `in.uv` を使ってテクスチャをサンプルする。`VertexOutput` の `uv` フィールドを差し替えるには `var in_mut = in; in_mut.uv = world_uv;` として `pbr_input_from_standard_material(in_mut, is_front)` を呼べばよい（WGSL の `var` はコピー）。

---

### ステップ A2 — Stochastic UV 回転（90°）

**変更ファイル**: `assets/shaders/section_material.wgsl`

`compute_terrain_uv` ヘルパを同ファイルに定義する。

```wgsl
// タイルサイズ（TILE_SIZE = 32.0）の逆数は uv_scale から計算可能
// uv_scale = 1/TILE_SIZE なので tile_world_size = 1.0 / uv_scale

fn hash2(p: vec2<f32>) -> f32 {
    let q = vec2<f32>(dot(p, vec2<f32>(127.1, 311.7)),
                      dot(p, vec2<f32>(269.5, 183.3)));
    return fract(sin(dot(q, vec2<f32>(1.0, 1.0))) * 43758.5453);
}

fn compute_terrain_uv(world_xz: vec2<f32>) -> vec2<f32> {
    let tile_world_size = 1.0 / section_material.uv_scale; // = TILE_SIZE
    let tile_coord = floor(world_xz / tile_world_size);     // タイルグリッド（整数）

    // Stochastic: タイルごとに 0°/90°/180°/270° をランダム選択
    let h = hash2(tile_coord);
    let rot = u32(floor(h * 4.0)); // 0..3

    let tile_center = (tile_coord + vec2<f32>(0.5)) * tile_world_size;
    let local = world_xz - tile_center; // タイル中心からのローカル座標

    var rotated: vec2<f32>;
    switch rot {
        case 1u:  { rotated = vec2<f32>(-local.y,  local.x); }
        case 2u:  { rotated = vec2<f32>(-local.x, -local.y); }
        case 3u:  { rotated = vec2<f32>( local.y, -local.x); }
        default:  { rotated = local; }                         // 0°
    }

    // UV 座標に変換（回転後のワールド座標からスケール）
    let base_uv = (tile_center + rotated) * section_material.uv_scale;

    // UV スクロール（river のみ非ゼロ、grass/dirt/sand は 0.0 で停止）
    return base_uv + vec2<f32>(0.0, section_material.uv_scroll_speed * globals.time);
}
```

> **シームに関する注意**: Stochastic 90° 回転はタイル境界で UV が不連続になる。テクスチャが完全にシームレス（＋回転対称に近い）であれば目立たない。現行テクスチャでシームが目立つ場合は、後続 MS-Asset-Terrain でテクスチャを差し替えるまでの暫定として A2 を無効化（`rot = 0` 固定）できるよう実装する。

> **`globals.time` について**: `bevy_pbr::mesh_view_bindings::globals` は Bevy の PBR view bind group から `Globals` 構造体を取り込む。`globals.time` は `f32`（秒単位の経過時間）。同プロジェクトでは `character_material.wgsl` が `bevy_pbr::mesh_view_bindings` を既に import しており、同様のパスで利用可能なことを確認済み。ただし実装時は `cargo check` でインポートエラーがないことを確認すること。

---

### ステップ A3 — 低周波 UV 歪み・任意の明度変調

親計画 §4.1 より優先度低。A1+A2 で目視改善が確認できてから実施する。

```wgsl
// compute_terrain_uv の rotated 計算後に追加（任意）
// 低周波ノイズで UV をわずかにオフセット（タイル内の単調さを崩す）
let distort = sin(world_xz * 0.05 + vec2<f32>(0.3, 1.7)) * 0.008;
// base_uv 計算前の rotated に加算: rotated = rotated + distort;
// → 強すぎる場合は係数 0.008 を下げる。高周波にしない（Rough Vector 美術方針に反する）
```

明度変調（任意）: `pbr_input.material.base_color.rgb *= 1.0 + sin(world_xz.x * 0.03 + world_xz.y * 0.02) * 0.04;`（±4%。A1+A2 確認後に検討）

---

### ステップ A4 — 川（`river` のみ）UV スクロール

`uv_scroll_speed` フィールドと `globals.time` の組み合わせで実装済み（`compute_terrain_uv` 内）。

`make_terrain_section_material(texture, 0.003)` で river のみ `uv_scroll_speed = 0.003` を設定するだけ。**既存の WGSL コードに追加分岐は不要**。

> 速度は要調整（画面スクロール方向の好みや見た目次第）。初期値 `0.003` は 1 秒でテクスチャ全体の 0.3% 分スクロール（緩やか）。

---

### ステップ A5 — `SectionCut` との順序・品質

既存の `section_discard` 呼び出し順（fragment 先頭）は変更しない。UV 計算と `section_discard` の順序:

```wgsl
@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    // 1. section discard（変更なし）
    section_discard(in.world_position.xyz);

    // 2. 地形モードなら UV 差し替え
    var in_mut = in;
    if section_material.albedo_uv_mode > 0.5 {
        in_mut.uv = compute_terrain_uv(in.world_position.xz);
    }

    // 3. PBR 処理（差し替えた UV を使用）
    var pbr_input = pbr_input_from_standard_material(in_mut, is_front);
    // ...以降は変更なし
}
```

矢視（SectionCut アクティブ時）で切断面のテクスチャ引き伸びが気になる場合は、Triplanar マッピングを別スパイクとして切り出す。

---

### ステップ A6 — `section_material_prepass.wgsl`

prepass は深度・法線パスのみ（アルベドテクスチャをサンプルしない）。地形の terrain タイルは不透明なので alpha discard も不要。

**必要な変更**: `SectionMaterialUniforms` struct 定義の同期（新フィールド追加）のみ。UV 計算ロジックの追加は不要。

---

## 5. 変更ファイル一覧

| ファイル | 変更内容 |
| --- | --- |
| `crates/hw_visual/src/material/section_material.rs` | `SectionMaterialUniform` に 4 フィールド追加、`make_terrain_section_material` 追加 |
| `assets/shaders/section_material.wgsl` | `SectionMaterialUniforms` struct 更新、`globals` import、`hash2` + `compute_terrain_uv` 追加、fragment に UV 分岐 |
| `assets/shaders/section_material_prepass.wgsl` | `SectionMaterialUniforms` struct 更新（main shader と同期） |
| `crates/bevy_app/src/plugins/startup/visual_handles.rs` | `Terrain3dHandles` 生成を `make_terrain_section_material` に差し替え |
| `crates/bevy_app/src/plugins/startup/asset_catalog.rs` | 地形 4 枚のロードを `load_with_settings` + `AddressMode::Repeat` に変更 |

---

## 6. 実装順の依存関係

```
D2（AddressMode::Repeat）
    ↓ 必ず同時
A0（uniform 拡張 + make_terrain_section_material）
    ↓
A1（ワールド UV）← A0 + D2 が完了してから動作確認できる
    ↓
A2（Stochastic 回転）← A1 の動作確認後
    ↓
A3（低周波歪み）← A1+A2 の見た目確認後（任意）
    |
A4（river スクロール）← A0 の uv_scroll_speed フィールドが前提
```

---

## 7. 検証

| 項目 | 方法 |
| --- | --- |
| 同種タイル連続 | トップダウンで Grass 等の **タイル境界が目立たない**こと |
| Stochastic | **同種タイルの繰り返しパターンが揃って見えない**こと |
| 建物・壁 | **見た目の退行なし**（メッシュ UV のまま）。特に壁の側面テクスチャを確認 |
| 矢視 | **地形の SectionCut** が破綻しない（discard 順序が正しいこと） |
| 川 | **静止感が減る**（ゆっくりスクロールが見える） |
| ビルド | `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` エラーなし |
| Clippy | `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace` 警告ゼロ |

---

## 8. 完了条件（本サブ計画）

- [x] §2 の **地形／非地形分岐**（`albedo_uv_mode`）が実装され、建物に誤適用されていない
- [x] D2: 地形 4 テクスチャが `AddressMode::Repeat` でロードされている
- [x] A1: 地形タイルがワールド XZ UV を使い、同種タイル間のシームが消えている
- [ ] A2: Stochastic 90° 回転 — **現行実装では未採用**（連続ワールド UV。タイル境界は連続のまま）
- [x] A3: 低周波 UV 歪み・明度変調 — **草タイルのみ**（`uv_distort_strength` は UV 空間の振幅、`brightness_variation_strength` は `sin` 乗算。土・砂・川は `0`）
- [x] A4: `river` のみ UV スクロール（U 方向・速度は `visual_handles` で設定。画面上は左→右の見え）
- [x] §7 のビルド検証に相当する確認（`cargo check` / `clippy`）
- [x] `docs/world_layout.md` の地形レンダリング節・`docs/architecture.md` を更新

### 実装メモ（コードとの差分）

- **`make_terrain_section_material`** は引数 `(texture, uv_scroll_speed, uv_distort_strength, brightness_variation_strength)`。
- **uniform のパディング**に `[f32; 3]` を使わない（encase: uniform 配列はストライド 16 の制約によりパニック）。**`f32` を個別フィールド**で並べる。
- A3 の UV 歪みは **ワールド座標に足してから `uv_scale` するのではなく**、**UV 空間で `base_uv` に加算**しないと効果がほぼ見えない。

---

## 9. 参照

| 文書 / コード | 内容 |
| --- | --- |
| [`ms-3-6-terrain-surface-plan-2026-03-31.md`](ms-3-6-terrain-surface-plan-2026-03-31.md) | 全体方針・B/C・性能 |
| `crates/bevy_app/src/plugins/startup/asset_catalog.rs` | 現行 4 テクスチャパス |
| `crates/hw_visual/src/material/section_material.rs` | 現行 uniform / ヘルパ定義 |
| `assets/shaders/section_material.wgsl` | 現行フラグメントシェーダ |
| `assets/shaders/character_material.wgsl` | `bevy_pbr::mesh_view_bindings` import の実装例 |
| `docs/world_lore.md` §6 | アート指針（Rough Vector Sketch・高周波ノイズ禁止） |

---

## 10. 更新履歴

| 日付 | 内容 |
| --- | --- |
| 2026-04-01 | 初版（現行アセットのみの A/D 実装手順） |
| 2026-04-01 | D2 を「任意」→「必須（A1 と同時）」に訂正。A4 を `uv_scroll_speed: f32` 方式に変更 |
| 2026-04-01 | コードベース調査に基づきブラッシュアップ。具体的な struct 差分・WGSL コード・依存グラフ・D2 の `load_with_settings` 実装詳細を追記。`sync_section_cut_to_materials_system` 変更不要の根拠を明記 |
| 2026-04-01 | 実装完了に合わせて §8・実装メモを更新。A2 未採用・草のみ A3・川 U スクロール・encase 配列制約・`docs/world_layout.md` / `architecture.md` 追記を反映 |
| 2026-04-01 | §4-2 / §4-3 コード例を実際のシグネチャ（4 引数）・速度値（`0.03`）に合わせ修正 |
