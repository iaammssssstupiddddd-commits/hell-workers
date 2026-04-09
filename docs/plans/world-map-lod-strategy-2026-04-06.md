# ワールドマップ LOD 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `world-map-lod-strategy-2026-04-06` |
| ステータス | `Draft` |
| 作成日 | `2026-04-06` |
| 最終更新日 | `2026-04-09` |
| 作成者 | `Codex` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |
| 関連ドキュメント | `docs/world_layout.md`, `docs/architecture.md`, `docs/plans/world-map-render-chunking-plan-2026-04-08.md`, `docs/plans/3d-rtt/ms-3-6-terrain-surface-plan-2026-03-31.md` |

## 1. 目的

- 解決したい課題:
  - 旧LOD案は「地形が 10,000 個の per-tile render entity」「境界が別メッシュ群として残る」という前提で組まれていたが、現実装はすでに **49 個の `TerrainChunk`** と **`boundary_mask` baked** に移行している。
  - そのため、今の主要課題は entity 数削減ではなく、`TerrainSurfaceMaterial` と RtT 経路の **fragment / fill-rate / texture sampling 負荷** をズームに応じて落とせていない点にある。
  - 何が本当のボトルネックかを観測する仕組みがなく、TopDown far overview を先に入れるべきか、shader 簡略化で十分かを判断できない。
- 到達したい状態:
  - LOD の対象を「すでに解決済みの entity 数」ではなく、**現行 chunk renderer 上の描画負荷**へ再定義する。
  - 近景は現行品質を維持し、中景は **同じ chunk 経路のまま shader を軽くする**。
  - 遠景の impostor / overview は、**M2 の実測後に必要な場合だけ**導入する。
  - 旧案の「境界リボンを far でも残す」は採用しない。現行経路では境界は `boundary_mask` として terrain shader 内に統合されているため、far 表示側も **境界情報ごと baked** する設計に寄せる。
- 成功指標:
  - LOD 判定が、表示確認用の `tile_screen_px` と GPU 負荷判定用の `tile_rtt_px`、および hysteresis に基づいて 1 箇所で管理される。
  - LOD1 で TopDown / 矢視の遠景負荷を下げつつ、chunk renderer・`TerrainChangedEvent`・section clip 契約を壊さない。
  - LOD2 は「必要性が確認できた場合のみ」導入し、導入時は TopDown far 限定で stale 表示を起こさない。

## 2. スコープ

### 対象（In Scope）

- 現実装ベースの地形LOD再設計
- `TerrainChunk` / `TerrainSurfaceMaterial` / `TerrainIdMap` / `boundary_mask` 前提での段階計画
- `tile_screen_px` / `tile_rtt_px` 観測基盤と LOD state/hysteresis の導入
- chunk renderer を維持したまま行う LOD1（shader 簡略化）
- 実測後に必要なら行う TopDown far 専用 LOD2（overview/impostor）
- `TerrainChangedEvent` と LOD 表示の同期経路整理
- `docs/world_layout.md` / `docs/architecture.md` / `docs/plans/README.md` との整合

### 非対象（Out of Scope）

- `WorldMap` / pathfinding / AI / logistics の粒度変更
- 地形 chunk renderer の再撤回や per-tile render entity への逆戻り
- `BoundarySurfaceMaterial` を使った別境界描画経路の復活
- Soul / Familiar / 建物 / 影 / RtT 合成全体の LOD 本体
- worldgen アルゴリズムや `WorldMasks` の再設計

## 3. 現状とギャップ

### 3.1 現状

- `crates/bevy_app/src/world/map/spawn.rs`
  - `spawn_map` は 10,000 個の `Tile` 論理 anchor を生成するが、**描画コンポーネントは持たない**。
  - 地形描画は `spawn_terrain_chunks` が担い、`CHUNK_TILES = 16`、**7×7 = 49 個の `TerrainChunk` entity** を生成する。
- `crates/bevy_app/src/plugins/startup/visual_handles.rs`
  - `Terrain3dHandles` は共有 `Handle<TerrainSurfaceMaterial>` 1 本だけを持つ。
  - すべての chunk は同一 material を共有する。
- `crates/bevy_app/src/world/map/boundary.rs`
  - 旧案が想定していた「境界リボンの render entity」は現行 startup 経路にはない。
  - 実際には PostStartup で `boundary_mask` テクスチャを焼き、`TerrainSurfaceMaterial` に後付けする。
- `assets/shaders/terrain_surface_material.wgsl`
  - `terrain_id_map`、`terrain_feature_map`、4 枚の albedo、macro noise、overlay、river flow/normal、shoreline detail、feature LUT、`boundary_mask` を使う。
  - `boundary_mask` では nearest 判定に加え、4 corner を読む**手動 bilinear**分岐を持つ。
- `crates/bevy_app/src/systems/visual/terrain_material.rs`
  - runtime 地形変更は `TerrainChangedEvent` を受けて `TerrainIdMap` の該当ピクセルだけを書き換える。
  - chunk entity / boundary texture の再生成は行わない。
- `crates/bevy_app/src/systems/visual/camera_sync.rs`
  - `MainCamera` の `Transform.scale.x` を毎フレーム `Camera3d` の `OrthographicProjection.scale` にコピーしている。
  - ただし、LOD state や `tile_screen_px` / `tile_rtt_px` の集約 Resource はまだ存在しない。

### 3.2 問題

- 旧計画の主問題だった「10,000 render entity の削減」は、`world-map-render-chunking-plan-2026-04-08` でほぼ解消済み。
- 現在の主要コストは **terrain shader 自体**と、RtT で地形が画面を広く覆うときの **fill-rate / texture sampling** に寄っている可能性が高い。
- それにもかかわらず、ズームで変わるのは RtT 解像度ではなく camera scale だけで、terrain shading の重さは常に一定である。
- 現行コード上で「境界LOD」を論じるなら、対象は境界メッシュではなく **`boundary_mask` を用いた shader 内表現**でなければならない。
- `BoundarySurfaceMaterial` は crate に残っているが、現 startup 経路では未使用であり、これを前提にした再設計は現実装とズレる。
- far overview を先に入れても、もしボトルネックが terrain ではなく Soul / 建物 / shadow / RtT 合成側なら、効果が限定的になる。

### 3.3 本計画で埋めるギャップ

- LOD 判定の正本となる `tile_rtt_px` と、表示確認用の `tile_screen_px` を 1 箇所で算出する。
- 49 chunk を維持したまま導入できる **低リスクな LOD1** を先に定義する。
- TopDown far overview は **M2 計測後の条件付き施策**として位置付け直す。
- `TerrainChangedEvent`・`TerrainIdMap`・`boundary_mask` の責務分離を前提に、LOD2 の更新契約を整理する。

## 4. 実装方針（高レベル）

### 4.1 設計原則

- **原則A: まず観測する**
  - 旧案のように「far overview を最初から主役にする」のではなく、まず terrain 側の負荷がどれだけ支配的かを観測する。
- **原則B: 既存 chunk 経路を第一選択にする**
  - 49 entity まで削減済みなので、次の一手は entity 構造変更ではなく shader 側の簡略化を優先する。
- **原則C: far 表示は TopDown 専用の別問題として扱う**
  - 矢視・section correctness を巻き込まず、LOD2 は TopDown far だけに閉じる。
- **原則D: 境界情報は terrain と一体で扱う**
  - 現行経路では境界は `boundary_mask` として terrain shader に織り込まれている。
  - したがって LOD2 を作る場合も「境界だけ別 entity で残す」ではなく、**overview 側へ焼き込む**のを正本にする。

### 4.2 LOD レベル定義

| LOD | 適用範囲 | 内容 | 主目的 |
| --- | --- | --- | --- |
| `LOD0` | 近景 | 現行 `TerrainSurfaceMaterial` をそのまま使用 | 見た目維持 |
| `LOD1` | 中景〜遠景 | 49 chunk は維持し、terrain shader の重い経路を段階的に落とす | fragment / sampling 負荷削減 |
| `LOD2` | TopDown far のみ | 通常 chunk を隠し、overview/impostor を表示 | fill-rate と shading をさらに削減 |

### 4.3 現行シェーダーのコスト分解

LOD1 で削る候補を判断するため、`terrain_surface_material.wgsl` の主要コストを整理する。

| 処理 | 推定コスト | 説明 |
| --- | --- | --- |
| `blend_terrain()` 境界パス | **最重** | 4-corner bilinear + 最大 8 近傍 `textureLoad` = 12+ fetch/pixel（境界付近） |
| `blend_terrain()` fast path | 低 | 4 corners が同一カテゴリ時は 1 `textureSample` のみ |
| `compute_terrain_uv()` domain warp | 高 | `sample_macro_noise` を 2 回呼ぶ（warp 前後）; id=0/1/2 のみ発動 |
| `sample_river_flow_noise` / `sample_river_normal_detail` | 中 | `globals.time` 依存の animated sample; id=3 (River) のみ |
| `grade_sand()` shoreline_detail | 中 | `shoreline_detail` + `terrain_feature_lut` × 2 |
| `sample_macro_overlay` | 低 | albedo 枚数 × 1 sample / terrain type |
| `section_discard` | 無視 | 早期 discard なので最軽量 |

**LOD1 で削る優先順位**（上から順）:
1. `blend_terrain()` の 4-corner bilinear → nearest-only に差し替え（境界ピクセルのみ差が出る）
2. domain warp（`terrain_domain_warp_strength` を LOD1 では常に 0 とみなす）
3. river アニメーション（`scroll_speed=0` 固定 + flow/normal detail をスキップ）
4. `grade_sand()` の `shoreline_detail` サンプリングをスキップ

### 4.4 LOD1 の採用方針

- まずは **同じ `TerrainChunk` + 同じ world-space UV 構造**を維持する。
- 49 chunk しかないため、LOD0/LOD1 の切替は少数 entity の material component 差し替えで十分扱える。
- **material variant 方式を採用する**（WGSL 内の動的 uniform 分岐ではなく、コンパイル済み別シェーダー）。
  - GPU ドライバが「使われない経路も評価する」リスクを排除できる。
  - 49 entity の `MeshMaterial3d` 差し替えコストは trivial。

#### LOD1 shader (`terrain_surface_material_lod1.wgsl`) の変更点

```wgsl
// blend_terrain → blend_terrain_lod1 に差し替え
// 4-corner bilinear を廃止し nearest-only に落とす（境界ピクセルのギザギザは far では不可視）
fn blend_terrain_lod1(world_xz: vec2<f32>, cell: vec2<i32>, feature_in: vec4<f32>) -> vec3<f32> {
    let uv        = world_to_boundary_uv(world_xz);
    let raw_center = textureSample(boundary_mask, boundary_mask_sampler, uv).r;
    let region_id  = region_to_coarse_id(raw_center);
    let region_raw = region_to_raw_byte(raw_center);
    let eff_f      = feature_with_zone_tone(feature_in, region_raw, region_id);
    return sample_surface_color(region_id, world_xz, eff_f, region_raw);
}

// compute_terrain_uv の domain warp を無効化
fn compute_terrain_uv_lod1(id: u32, world_xz: vec2<f32>) -> vec2<f32> {
    // domain_warp_strength は呼ばない → macro_noise 2 fetch を省略
    let base_uv = world_xz * tsm.uv_scale;
    // scroll_speed / distort も省略（river は静止画扱い）
    return base_uv;
}

// grade_sand_lod1: shoreline_detail サンプリングをスキップ
fn grade_sand_lod1(base_rgb: vec3<f32>, brightness: f32, feature: vec4<f32>) -> vec3<f32> {
    let shore_lut  = sample_feature_lut(1.0);
    let inland_lut = sample_feature_lut(2.0);
    var graded_rgb = base_rgb * brightness;
    graded_rgb = apply_feature_grade(graded_rgb, shore_lut, feature.r, 1.05);
    graded_rgb = apply_feature_grade(graded_rgb, inland_lut, feature.g, 0.90);
    return graded_rgb;
}
```

#### `Terrain3dHandles` の拡張

```rust
// crates/bevy_app/src/plugins/startup/visual_handles.rs
#[derive(Resource)]
pub struct Terrain3dHandles {
    pub lod0: Handle<TerrainSurfaceMaterial>,      // 現行 surface → rename
    pub lod1: Handle<TerrainSurfaceMaterialLod1>,  // LOD1 軽量 variant（新規）
}
```

`lod1` は **別 material 型**として実装する。binding layout と uniform/texture 構造は LOD0 と同じまま、
`fragment_shader()` だけを `terrain_surface_material_lod1.wgsl` に変えた
`TerrainSurfaceMaterialLod1 = ExtendedMaterial<StandardMaterial, TerrainSurfaceMaterialExtLod1>` を新設する。
これにより shader は完全に別パイプラインでコンパイルされ、動的分岐を持ち込まない。

#### chunk への material component 切り替え

```rust
// crates/bevy_app/src/systems/visual/terrain_lod.rs
pub fn terrain_lod_switch_system(
    mut commands: Commands,
    lod: Res<TerrainLodState>,
    handles: Res<Terrain3dHandles>,
    q_lod0: Query<Entity, (With<TerrainChunk>, With<MeshMaterial3d<TerrainSurfaceMaterial>>)>,
    q_lod1: Query<Entity, (With<TerrainChunk>, With<MeshMaterial3d<TerrainSurfaceMaterialLod1>>)>,
) {
    if !lod.is_changed() { return; }

    match lod.level {
        LodLevel::Lod1 => {
            for entity in &q_lod0 {
                commands.entity(entity)
                    .remove::<MeshMaterial3d<TerrainSurfaceMaterial>>()
                    .insert(MeshMaterial3d::<TerrainSurfaceMaterialLod1>(handles.lod1.clone()));
            }
        }
        LodLevel::Lod0 | LodLevel::Lod2 => {
            for entity in &q_lod1 {
                commands.entity(entity)
                    .remove::<MeshMaterial3d<TerrainSurfaceMaterialLod1>>()
                    .insert(MeshMaterial3d::<TerrainSurfaceMaterial>(handles.lod0.clone()));
            }
        }
    }
}
```

LOD2 時は通常 chunk を `Visibility::Hidden` にするため、復帰先の既定 material は LOD0 に固定して構わない。

### 4.5 LOD2 の採用条件

- M2 完了後、以下が確認できた場合のみ着手する:
  - far で terrain が依然として主要ボトルネック
  - LOD1 だけでは改善幅が不足
  - TopDown far の見た目要件が overview でも満たせる
- LOD2 では:
  - TopDown far のみ通常 `TerrainChunk` を `Visibility::Hidden`
  - map 全体 footprint を持つ 1 枚の overview mesh（`Plane3d`, 100x100 tiles 相当）に切り替える
  - overview texture は `RgbaU8` / `MAP_WIDTH × MAP_HEIGHT` px で terrain 種別 + 境界情報を焼き込む
- runtime 更新:
  - `TerrainChangedEvent` は引き続き `TerrainIdMap` の truth update として使う
  - LOD2 側も同イベントを購読するが、再ベイク単位は **dirty cell 単体ではなく dirty rect + 1 cell halo** とする
  - halo を含める理由は、現行 shader が `boundary_mask` の 4-corner 補間と 8 近傍 feature 参照で最終色を決めるため、隣接セル側の見た目も変わり得るからである
  - `boundary_mask` と `TerrainFeatureMap` は worldgen snapshot のままなので、通常の obstacle cleanup では再生成不要

### 4.6 LOD の駆動変数

- `ortho.scale` の生値を直接閾値にするのではなく、以下 2 つの指標を分けて扱う。
  - `tile_rtt_px`: `Camera3dRtt` が実際に描いている RtT 上での 1 タイル見かけサイズ。LOD 閾値の正本。
  - `tile_screen_px`: composite 後のスクリーン表示上での 1 タイル見かけサイズ。デバッグ表示と見た目確認用。
- 算出方法は hand-tuned な式ではなく、Bevy 0.18 の `Camera::world_to_viewport(...)` を `Camera3dRtt` に対して使って:
  - ある地表点 `P`（map center 固定で十分）
  - `P + Vec3::new(TILE_SIZE, 0.0, 0.0)`
  - `P + Vec3::new(0.0, 0.0, TILE_SIZE)`
  - を投影し、**画面内で有効な 2 辺の差分長のうち大きい方**を 1 タイル見かけサイズとして採用する
- `East/West` 矢視では world X 辺が視線方向へ潰れるため、`P -> P + X` だけで判定しない。
- `tile_screen_px` は別カメラで再投影せず、`tile_rtt_px` から **composite 表示倍率**で導出する。
  - `screen_scale_x = logical_composite_size.x / runtime.viewport.width as f32`
  - `screen_scale_y = logical_composite_size.y / runtime.viewport.height as f32`
  - `tile_screen_px = tile_rtt_px * max(screen_scale_x, screen_scale_y)`
  - `logical_composite_size` は `rtt_composite.rs` の `logical_composite_size(window)` と同じ定義を使う
- これにより `tile_screen_px` は「補助観測値」として一意に定義され、LOD 判定自体は常に `tile_rtt_px` のみを参照する。

#### `TerrainLodMetrics` / `TerrainLodState` の定義

```rust
// crates/bevy_app/src/systems/visual/terrain_lod.rs

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LodLevel { #[default] Lod0, Lod1, Lod2 }

#[derive(Resource, Default)]
pub struct TerrainLodMetrics {
    pub tile_rtt_px:   f32,   // LOD 判定正本
    pub tile_screen_px: f32,  // デバッグ表示用
}

#[derive(Resource, Default)]
pub struct TerrainLodState {
    pub level: LodLevel,
    pub applied_level: LodLevel, // material / visibility 側が最後に適用済みの level
}

// hysteresis 閾値（初期値: 観測後に調整）
pub const LOD0_TO_LOD1_ENTER_PX: f32 = 10.0; // tile_rtt_px < これで LOD1 へ
pub const LOD1_TO_LOD0_EXIT_PX:  f32 = 14.0; // tile_rtt_px > これで LOD0 へ復帰
pub const LOD1_TO_LOD2_ENTER_PX: f32 = 4.0;  // TopDown far のみ
pub const LOD2_TO_LOD1_EXIT_PX:  f32 = 6.0;
```

#### `tile_rtt_px` 算出システム

```rust
pub fn update_terrain_lod_metrics_system(
    q_cam3d: Query<(&Camera, &GlobalTransform), With<Camera3dRtt>>,
    runtime: Res<RttRuntime>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    mut metrics: ResMut<TerrainLodMetrics>,
    mut state: ResMut<TerrainLodState>,
    elevation: Res<ElevationViewState>,
) {
    let Ok((cam, gtf)) = q_cam3d.single() else { return };

    // map center を基準点に（カメラ位置依存にしない）
    let p  = Vec3::new(0.0, 0.0, 0.0);
    let px = p + Vec3::new(TILE_SIZE, 0.0, 0.0);
    let pz = p + Vec3::new(0.0, 0.0, TILE_SIZE);

    let to_vp = |world: Vec3| -> Option<Vec2> {
        cam.world_to_viewport(gtf, world).ok()
    };

    if let (Some(v0), Some(vx), Some(vz)) = (to_vp(p), to_vp(px), to_vp(pz)) {
        let dx = (vx - v0).length();
        let dz = (vz - v0).length();
        metrics.tile_rtt_px = dx.max(dz);
    }

    if let Ok(window) = q_window.single() {
        let logical = composite_logical_size(window);
        let screen_scale_x = logical.x / runtime.viewport.width as f32;
        let screen_scale_y = logical.y / runtime.viewport.height as f32;
        metrics.tile_screen_px = metrics.tile_rtt_px * screen_scale_x.max(screen_scale_y);
    }

    // LOD 状態遷移（hysteresis）
    let new_level = resolve_lod_level(state.level, metrics.tile_rtt_px, &elevation);
    if new_level != state.level {
        state.level = new_level;
    }
}

fn resolve_lod_level(
    current: LodLevel,
    tile_rtt_px: f32,
    elevation: &ElevationViewState,
) -> LodLevel {
    match current {
        LodLevel::Lod0 => {
            if tile_rtt_px < LOD0_TO_LOD1_ENTER_PX { LodLevel::Lod1 } else { LodLevel::Lod0 }
        }
        LodLevel::Lod1 => {
            if tile_rtt_px > LOD1_TO_LOD0_EXIT_PX {
                LodLevel::Lod0
            } else if elevation.direction.is_top_down()
                && tile_rtt_px < LOD1_TO_LOD2_ENTER_PX
            {
                LodLevel::Lod2
            } else {
                LodLevel::Lod1
            }
        }
        LodLevel::Lod2 => {
            if tile_rtt_px > LOD2_TO_LOD1_EXIT_PX { LodLevel::Lod1 } else { LodLevel::Lod2 }
        }
    }
}
```

- これにより TopDown / 矢視の違いを同じ尺度で扱える。
- `East/West` を含む全方向で、view 面内の実効タイルサイズを使える。
- RtT 品質変更（`QualitySettings.rtt`）を `tile_rtt_px` に反映できる。
- `tile_screen_px` も composite 表示倍率から一意に導出でき、デバッグ表示に使える。
- metric 更新と level 切替トリガーを分離できるため、metric 書き換えだけで material 差し替えが毎フレーム走るのを避けられる。
- `logical_composite_size(window)` は現状 private 関数なので、M1 では shared helper として `pub(crate)` 抽出する。

### 4.7 モード制約

- `ElevationDirection::TopDown`
  - `LOD0 / LOD1 / LOD2` を許可
- `ElevationDirection::{North, South, East, West}`
  - `LOD0 / LOD1` まで
  - `LOD2` は禁止（`resolve_lod_level` で `is_top_down()` チェック済み）
- `SectionCut` 連動部分は correctness 優先
  - LOD1 は `section_discard` を LOD0 と同一実装で保持する
  - LOD2 は section 系で使用しない

## 5. マイルストーン

## M1: 観測基盤と LOD 契約の固定

- 変更内容:
  - `crates/bevy_app/src/systems/visual/terrain_lod.rs` を新規作成し、以下を実装する:
    - `LodLevel` enum (`Lod0 / Lod1 / Lod2`)
    - `TerrainLodMetrics` Resource (`tile_rtt_px`, `tile_screen_px`)
    - `TerrainLodState` Resource (`level`, `applied_level`)
    - `update_terrain_lod_metrics_system`: `Camera3dRtt` の `world_to_viewport` で `tile_rtt_px` を算出し、`RttRuntime.viewport` と `composite_logical_size(window)` から `tile_screen_px` を導出し、`resolve_lod_level` で `state.level` を更新
    - `resolve_lod_level`: hysteresis 閾値テーブル (`LOD0_TO_LOD1_ENTER_PX = 10.0` など) を参照し、矢視中は LOD2 を禁止
  - `crates/bevy_app/src/plugins/startup/rtt_composite.rs` の `logical_composite_size(window)` を `pub(crate) fn composite_logical_size(window)` へ抽出し、LOD 観測と composite で共有する
  - `crates/bevy_app/src/plugins/visual.rs` でシステムを `Visual` セットの適切な位置に登録する
  - perf シナリオ下で zoom range を走査し、初期閾値 (`10.0` / `14.0` / `4.0` / `6.0`) が妥当かを確認して調整する
- 変更ファイル:
  - `crates/bevy_app/src/systems/visual/terrain_lod.rs` **(新規)**
  - `crates/bevy_app/src/systems/visual/mod.rs`
  - `crates/bevy_app/src/plugins/visual.rs`
  - `crates/bevy_app/src/plugins/startup/rtt_composite.rs`
  - `docs/world_layout.md`
  - `docs/architecture.md`
- 完了条件:
  - [ ] `TerrainLodMetrics` と `TerrainLodState` が分離されており、metric 更新だけで material 切替が毎フレーム走らない
  - [ ] `tile_rtt_px` と `tile_screen_px` が毎フレーム更新される
  - [ ] `East/West` 矢視でも `tile_rtt_px` が view 面内の実効サイズを返す（X/Z 2 軸の大きい方）
  - [ ] LOD0 / LOD1 / LOD2 の enter / exit 閾値が定数として 1 箇所に書かれている
  - [ ] `resolve_lod_level` が矢視中は `LodLevel::Lod2` を返さないことをコードで保証する
  - [ ] `cargo check / clippy` がクリーン
  - [ ] TopDown / 矢視で zoom in / out し、`TerrainLodState.level` の遷移を `dbg!` または既存 UI 近傍のデバッグ表示で確認する
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace`
  - `cargo run -p bevy_app -- --spawn-souls 500 --spawn-familiars 30 --perf-scenario`
    で既存 FPS UI を見ながら zoom range を記録し、閾値の妥当性を確認

## M2: LOD1 導入（chunk 維持の shader 簡略化）

- 変更内容:
  1. **LOD1 shader 作成**: `assets/shaders/terrain_surface_material_lod1.wgsl` を新規作成する
     - `blend_terrain_lod1`: 4-corner bilinear を廃止し nearest-only（境界付近のみ差が出るが far では不可視）
     - `compute_terrain_uv_lod1`: domain warp / scroll / distort を除去
     - `grade_sand_lod1`: `shoreline_detail` サンプリングをスキップ
     - `section_discard` は LOD0 と同一実装を維持（section correctness 保持）
  2. **LOD1 material type 追加**: `crates/hw_visual/src/material/terrain_surface_material.rs` に
     `TerrainSurfaceMaterialExtLod1` newtype を追加し、`fragment_shader()` だけ `lod1.wgsl` を返すようにする
  3. **`Terrain3dHandles` 拡張**: `surface: Handle<...>` を `lod0: Handle<...>` に rename し、
     `lod1: Handle<TerrainSurfaceMaterialLod1>` を追加する
  4. **切り替えシステム追加**: `terrain_lod.rs` に `terrain_lod_switch_system` を実装し、
     `TerrainLodState.level != TerrainLodState.applied_level` の時のみ 49 chunk の `MeshMaterial3d` component を
     `remove::<MeshMaterial3d<TerrainSurfaceMaterial>>() / insert::<MeshMaterial3d<TerrainSurfaceMaterialLod1>>()`
     で差し替え、適用後に `applied_level = level` へ同期する
  5. **`TerrainChangedEvent` 動作確認**: `terrain_id_map_sync_system` は `TerrainIdMap` を直接更新するため、
     handle が lod0 / lod1 どちらでも `terrain_id_map` texture を共有しているので追加対応不要であることを確認する
- 変更ファイル:
  - `assets/shaders/terrain_surface_material_lod1.wgsl` **(新規)**
  - `crates/hw_visual/src/material/terrain_surface_material.rs`
  - `crates/hw_visual/src/lib.rs`（新型を pub re-export）
  - `crates/bevy_app/src/plugins/startup/visual_handles.rs`
  - `crates/bevy_app/src/world/map/spawn.rs`
  - `crates/bevy_app/src/world/map/boundary.rs`
  - `crates/bevy_app/src/systems/visual/terrain_lod.rs`
  - `crates/bevy_app/src/plugins/visual.rs`
  - `docs/world_layout.md`
- 完了条件:
  - [ ] `lod1.wgsl` が `terrain_id_map` / `boundary_mask` を LOD0 と同じ binding で参照する
  - [ ] `Terrain3dHandles.surface` の rename 影響が [spawn.rs](/home/satotakumi/projects/hell-workers/crates/bevy_app/src/world/map/spawn.rs#L134) と [boundary.rs](/home/satotakumi/projects/hell-workers/crates/bevy_app/src/world/map/boundary.rs#L1070) を含めて解消されている
  - [ ] 49 chunk を維持したまま LOD1 切替が機能する（material component 差し替えで完結）
  - [ ] LOD1 時に `blend_terrain` の 4-corner fetch と domain warp fetch がコードレベルで除去されている
  - [ ] obstacle cleanup 後も `TerrainChangedEvent` → `TerrainIdMap` 更新が LOD0 / LOD1 両方で反映される
  - [ ] TopDown / 矢視で近景復帰時（LOD1→LOD0 遷移）に大きなポップが出ない
  - [ ] perf シナリオで far 時の FPS が M1 比で改善（または同等でコスト削減を GPU profiler で確認）
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace`
  - `cargo run -p bevy_app -- --spawn-souls 500 --spawn-familiars 30 --perf-scenario`
  - TopDown / 矢視で zoom 往復し、FPS と見た目の差分を記録する
  - obstacle cleanup（岩除去）後に far / near を往復して見た目の更新を確認

## M3: 条件付き LOD2（TopDown far overview）

- 着手条件（M2 計測後に評価）:
  - far で terrain が依然として主要ボトルネックであること
  - LOD1 だけでは改善幅が目標値に届かないこと
  - TopDown far の見た目要件が overview でも満たせること
- 変更内容:
  1. **overview texture 準備**: `MAP_WIDTH × MAP_HEIGHT` px の `RgbaU8` texture を CPU 側で焼き、
     境界情報（`boundary_mask` baked value）と terrain 種別色を合成する
  2. **overview entity 追加**: `PostStartup` で map 全体 footprint の `Plane3d`（100×100 tiles）を 1 枚 spawn し、
     overview texture を持つ軽量 material を付与する（`UnlitMaterial` 相当）
  3. **chunk 可視制御**: LOD2 遷移時に 49 `TerrainChunk` entity を `Visibility::Hidden` にし、
     LOD2 解除時に `Visibility::Inherited` に戻す
  4. **dirty 更新**: `TerrainChangedEvent` を overview 側でも購読し、`dirty_cell + 1 cell halo` の
     矩形範囲だけ CPU 側で overview texture を再書き込みして GPU にアップロードする
  5. **矢視ガード**: `terrain_lod_switch_system` で `elevation.direction.is_top_down()` が false の場合は
     LOD2 chunk 切り替えをスキップする
- 変更ファイル:
  - `crates/bevy_app/src/world/map/overview.rs` **(新規)**
  - `crates/bevy_app/src/plugins/startup/mod.rs`
  - `crates/bevy_app/src/plugins/startup/visual_handles.rs`
  - `crates/bevy_app/src/systems/visual/terrain_lod.rs`
  - `docs/world_layout.md`
  - `docs/architecture.md`
- 完了条件:
  - [ ] TopDown far で 49 chunk が `Visibility::Hidden` になり、overview mesh だけが描画される
  - [ ] overview が境界情報込みで地形種別の可読性を保つ
  - [ ] dirty 更新範囲が `dirty cell + 1 cell halo` として実装されており、obstacle cleanup 後に seam が出ない
  - [ ] `TerrainChangedEvent` 後に stale 表示が出ない
  - [ ] 矢視（North/East/South/West）ではチャンクが隠れず、overview も表示されない
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace`
  - 岩除去後に near / far を往復して stale が出ないことを確認
  - `cargo run -p bevy_app -- --spawn-souls 500 --spawn-familiars 30 --perf-scenario`
    で M2 単体と比較し、既存 FPS UI と profiler で差を記録する

## M4: 地形LODの到達点判定

- 変更内容:
  - M2 / M3 の計測結果を基に「world-map LOD で十分か」「RtT 全体の別計画が必要か」を判断する
  - terrain 以外が支配的なら、問題を別計画へ切り出す
- 変更ファイル:
  - `docs/plans/...`
  - `docs/architecture.md`
- 完了条件:
  - [ ] world-map LOD の完了条件が明文化されている
  - [ ] terrain 以外のボトルネックを world-map LOD へ混ぜない整理ができている
- 検証:
  - 計測ログ比較
  - `cargo clippy --workspace`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| terrain 以外がボトルネックなのに terrain LOD へ寄り過ぎる | 高 | M1/M2 で観測を先に入れ、terrain 支配を確認してから M3 へ進む |
| WGSL の動的分岐で期待ほど軽くならない | 高 | LOD0 / LOD1 を material variant として分離し、重い経路をコンパイル時に外しやすくする |
| `boundary_mask` の簡略化で far の輪郭が崩れる | 中 | nearest 固定ではなく、まず簡略 blend 版を作って比較する |
| `East/West` 矢視で LOD 指標が視線方向へ潰れ、誤判定する | 高 | `P -> P + X` 固定ではなく、world X/Z の 2 軸投影差分の大きい方を採用する |
| LOD2 が runtime 地形変更に追従せず stale になる | 高 | `TerrainChangedEvent` を単一の truth 更新イベントとして扱い、dirty cell 単体ではなく halo 付き dirty rect 更新を必須契約にする |
| RtT 品質変更で同じ `tile_screen_px` でも GPU コストが変わり、閾値がずれる | 中 | LOD 閾値の正本を `tile_rtt_px` に置き、`tile_screen_px` は見た目確認用に分離する |
| TopDown / 矢視 / section で切替条件が複雑化する | 中 | `TerrainLodState` に view mode 制約を集約し、個別システムに判定を散らさない |
| 旧 docs / 旧思考が残り、境界メッシュ前提で再実装してしまう | 中 | `world_layout.md` と本計画に「境界は baked path が正本」と明記する |
| metric 更新だけで material 切替が毎フレーム走る | 高 | `TerrainLodMetrics` と `TerrainLodState` を分離し、切替条件は `level != applied_level` を使う |

## 7. 検証計画

- 必須:
  - `cargo check --workspace`
  - `cargo clippy --workspace`
- 手動確認シナリオ:
  - TopDown で zoom in / out を往復し、LOD0 / LOD1 / LOD2 の切替と復帰を確認する
  - 矢視へ切り替え、LOD2 に入らないことを確認する
  - `East/West` 矢視でも LOD 判定が暴れず、TopDown と同じ hysteresis で安定することを確認する
  - obstacle cleanup 後に terrain の見た目が近景・遠景とも更新されることを確認する
  - LOD2 導入時は、変更セルの隣接境界をまたぐ場所でも stale / seam が出ないことを確認する
- パフォーマンス確認:
  - `cargo run -p bevy_app -- --spawn-souls 500 --spawn-familiars 30 --perf-scenario`
  - FPS 表示と、可能なら renderdoc / profiler の GPU 側観測で terrain pass の差を見る
  - `QualitySettings.rtt` の High / Medium / Low で `tile_rtt_px` と改善幅の相関を確認する
  - M2 と M3 は必ず別々に比較する

## 8. ロールバック方針

- どの単位で戻せるか:
  - M1 は Resource / debug 観測追加なので個別 revert しやすい
  - M2 は LOD0 variant を残すため、切替システムを外せば現行品質へ戻せる
  - M3 は TopDown far 専用 entity / resource 単位で無効化できるようにする
- 戻す時の手順:
  - `TerrainLodState` を固定で `LOD0` にする
  - overview entity / runtime resource の登録を外す
  - docs では M3 を未採用として明記する

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `20%`（計画の具体化完了）
- 完了済みマイルストーン:
  - 現実装ベースの前提整理
  - シェーダーコスト分解（`blend_terrain` 4-corner bilinear が最重、domain warp が次点）
  - `TerrainLodMetrics` / `TerrainLodState` / `LodLevel` / `resolve_lod_level` の型・関数シグネチャ確定
  - material variant 方針（uniform 分岐ではなく別 wgsl ファイル + newtype）の採用決定
  - `Terrain3dHandles.lod0` / `lod1` への拡張インターフェース確定
- 未着手/進行中:
  - M1 以降の実装は未着手

### 次のAIが最初にやること

1. `terrain_lod.rs` を新規作成し、4.6 節の `TerrainLodMetrics` / `TerrainLodState` / `LodLevel` / 閾値定数 / `update_terrain_lod_metrics_system` / `resolve_lod_level` をそのまま実装する
   - `Camera3dRtt` は `crates/bevy_app/src/plugins/startup/rtt_setup.rs` で定義されているので確認すること
   - `ElevationViewState` は `crates/bevy_app/src/systems/visual/elevation_view.rs` から import する
2. `crates/bevy_app/src/plugins/visual.rs` に `TerrainLodMetrics` / `TerrainLodState` を `init_resource` し、`update_terrain_lod_metrics_system` を登録する（`sync_camera3d_system` の後）。同時に `rtt_composite.rs` から `composite_logical_size(window)` を共有 helper 化する
3. zoom 往復で `tile_rtt_px` が数値を出すことを確認してから、LOD1 shader の作成（M2）に進む

### ブロッカー/注意点

- `BoundarySurfaceMaterial` は存在するが現 startup 経路では未使用。これを前提に計画を戻さないこと。
- `TerrainChangedEvent` は現在 `TerrainIdMap` の部分更新だけを担う。boundary / feature は runtime 更新対象ではない。
- `tile_screen_px` 単独では GPU コストの正本にならない。RtT 実解像度を反映した `tile_rtt_px` を判定に使うこと。
- `TerrainLodMetrics` と `TerrainLodState` を分けないと、metric 書き換えだけで `state.is_changed()` が毎フレーム立つ。切替判定は `level != applied_level` を使うこと。
- LOD2 の dirty 更新は cell 単体では不足する。最低でも 1 cell halo を含めること。
- LOD2 の必要性は未確定。M2 計測前に前倒ししないこと。
- `Terrain3dHandles.surface` は `lod0` に rename するため、参照箇所を grep で全洗いすること:
  `grep -r "\.surface" crates/bevy_app/src --include="*.rs"`
- LOD1 material は `TerrainSurfaceMaterial` とは **別の型** として固定する。したがって切り替えは handle 差し替えではなく、
  `Commands::entity().remove::<...>().insert(...)` で `MeshMaterial3d` component 自体を差し替える前提で進めること。
- `tile_screen_px` は `tile_rtt_px` から composite 表示倍率で導出する補助指標であり、LOD 閾値判定には使わないこと。
- `logical_composite_size(window)` は現状 private 関数なので、そのまま別モジュールから呼べない。shared helper へ抽出してから使うこと。

### 参照必須ファイル

- `docs/world_layout.md`
- `docs/architecture.md`
- `docs/plans/world-map-render-chunking-plan-2026-04-08.md`
- `crates/bevy_app/src/world/map/spawn.rs`
- `crates/bevy_app/src/world/map/boundary.rs`
- `crates/bevy_app/src/systems/visual/camera_sync.rs`
- `crates/bevy_app/src/systems/visual/elevation_view.rs`
- `crates/bevy_app/src/plugins/startup/rtt_setup.rs`
- `crates/hw_visual/src/material/terrain_surface_material.rs`
- `assets/shaders/terrain_surface_material.wgsl`

### 最終確認ログ

- 最終 `cargo check`: `未実行` / `docs-only`（計画ブラッシュアップのみ）
- 未解決エラー:
  - なし（コード未変更）

### Definition of Done

- [ ] M1 が完了し、LOD state と閾値契約が固定されている
- [ ] M2 が完了し、chunk renderer 上で terrain shader の負荷が落ちている（既存 FPS UI または profiler で確認）
- [ ] M3 は必要性が確認できた場合のみ完了している
- [ ] 影響ドキュメントが更新済み（`docs/world_layout.md`, `docs/architecture.md`）
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace` が成功
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace` が成功（0 warnings）

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-06` | `Codex` | 初版作成 |
| `2026-04-09` | `Codex` | chunk renderer・`boundary_mask` baked・`TerrainChangedEvent` 現契約を前提に全面再計画 |
| `2026-04-09` | `Codex` | レビュー反映: `tile_rtt_px` 正本化、矢視 2 軸測定、LOD2 halo 更新、`clippy` 検証を追記 |
| `2026-04-08` | `Copilot` | シェーダーコスト分解・具体的型定義・コードスニペット・マイルストーン詳細化でブラッシュアップ |
