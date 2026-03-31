# MS-3-4 テレイン 3D 化 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `ms-3-4-terrain-3d-plan-2026-03-29` |
| ステータス | `Draft` |
| 作成日 | `2026-03-29` |
| 最終更新日 | `2026-03-30` |
| 親マイルストーン | `docs/plans/3d-rtt/milestone-roadmap.md` **MS-3-4** |
| 前提 MS | **MS-3-3**（`SectionMaterial` 基盤・wall pilot 接続済み） |
| 関連提案 | `docs/proposals/3d-rtt/20260316/section-material-proposal-2026-03-16.md` |
| アセット計画 | `docs/plans/3d-rtt/asset-milestones-2026-03-17.md`（MS-Asset-Terrain：転用確認） |
| 後続 MS | **MS-3-6**（表面表現・境界ブレンド）、**MS-3-7**（Raycast ヒットテスト） |

---

## 1. 目的

### 解決したい課題

- 地形が **Camera2d + `Sprite`** と **`terrain_border` のオーバーレイ Sprite** で描画されており、**Camera3d → RtT** のワールドとレイヤーが分断している。
- 矢視時の **section clip**（`SectionCut`）と地形が同一パイプライン上にないため、Phase 3 の「インゲームは RtT 一本」に到達できない。
- `hw_world::generate_terrain_border_specs` / `terrain_border.rs` に依存した **90° 境界テクスチャ**は、MS-3-6 で廃止予定だが、MS-3-4 時点で **表現の単一化**を始める必要がある。

### 到達したい状態

- **地形メッシュ**が `LAYER_3D` で **Camera3dRtt** にのみ描画され、合成は既存 **RtT composite** 経路のまま成立する。
- **Camera2d 側にインゲーム用地形 Sprite が残らない**（UI・Familiar・オーバーレイは対象外）。
- `TerrainBorder` エンティティおよび `spawn_terrain_borders` への依存を **MS-3-4 完了時点で除去**する（見た目は平坦タイル＋単一テクスチャでよい。有機的な境界は MS-3-6）。

### 成功指標（ロードマップとの整合）

- [ ] `cargo check --workspace` ゼロエラー、`cargo clippy` ワークスペース方針に準拠
- [ ] 地形が **Camera3d → RtT のみ**で見える（TopDown・矢視の両方で退行なし）
- [ ] `terrain_border.rs` のスポーンが登録フローから外れ、`borders` スペック生成に依存しない

---

## 2. スコープ

### 対象（In Scope）

- `crates/bevy_app/src/world/map/spawn.rs`：`spawn_map` の各タイルを **`Mesh3d` + マテリアル**へ置換（`Sprite` 削除）。
- **タイル 1 セル 1 エンティティ**は維持し、`WorldMap::set_tile_entity_at_idx` の契約を壊さない（後述の既存利用者）。
- `RenderLayers::layer(LAYER_3D)`（必要なら shadow receiver 方針は建物と揃えて文書化）。
- `crates/bevy_app/src/world/map/terrain_border.rs` および `startup` の `spawn_terrain_borders_if_enabled` 呼び出しの **削除または恒久的無効化**（環境変数でのスキップだけに残さない）。
- `hw_world::terrain_visual::obstacle_cleanup_system`：**`Sprite` 前提の `Query<&mut Sprite>`** を、地形が 3D になった後の **マテリアル／テクスチャ差し替え**へ更新。
- `docs/architecture.md` の RtT / マップ関連、`docs/world_layout.md`（座標・レイヤー説明が変わる場合）の更新。
- `TerrainChangedEvent` を `hw_core` に置く場合は **`docs/events.md` に 1 行追加**（Producer / Consumer / Timing）。

### 非対象（Out of Scope）

- **MS-3-6**：テクスチャブレンド・ノイズ・境界の有機化、`terrain/*.png` オーバーレイ相当の高品質化。
- **MS-3-7**：`viewport_to_world_2d` の Raycast 置換。MS-3-4 中は既存 2D カメラベースの入力が地形クリックで破綻する場合、**既知の制限**として計画に記録し、MS-3-7 で解消する。
- **地形用 GLB の新規大量制作**：タイルは共有 **平面メッシュ**＋既存 `grass/sand/dirt/river` テクスチャで足りる想定（`asset-milestones` の MS-Asset-Terrain は転用確認が主）。
- **WFC / 手続き的地形生成**（別トラック `wfc-terrain-generation-plan`）。

---

## 3. 現状とギャップ

### 現状（コードの事実）

| 箇所 | 内容 |
| --- | --- |
| `world/map/spawn.rs` | 各タイルに `Tile` + `Sprite { image, custom_size: TILE_SIZE }` + `Transform::from_xyz(pos.x, pos.y, Z_MAP)` |
| `world/map/terrain_border.rs` | `TerrainBorder` + `Sprite` で `grass_edge` 等を重ねる |
| `hw_world/src/terrain.rs` | `TerrainType::z_layer()` が `Z_MAP` / `Z_MAP_SAND` / `Z_MAP_DIRT` / `Z_MAP_GRASS` を返す（2D Z オーダー用） |
| `hw_world/src/terrain_visual.rs` | `TerrainVisualHandles { dirt: Handle<Image> }`。障害物削除時に `Sprite.image = handles.dirt.clone()` で差し替え |
| `plugins/startup/visual_handles.rs` | `TerrainVisualHandles` を `game_assets.dirt.clone()` で初期化（`init_visual_handles` 内） |
| `hw_familiar_ai/.../direct_collect.rs` | `tile_entity_at_idx` でタイル Entity を取得し Designation 等を参照する（ロジック側のみ・描画非依存） |

### ギャップ

**座標系の不一致**
- `grid_to_world(x, y) -> Vec2` は 2D 座標（Y 上向き）を返す。3D では `Transform::from_xyz(pos2d.x, 0.0, -pos2d.y)` に写像する（建物 Floor の `spawn_building_3d_visual` と同じパターン）。

**表現コンポーネントの置換**
- `Tile` Entity はゲームロジックの錨なので削除不可。`Sprite` を除去し `Mesh3d` + `MeshMaterial3d<SectionMaterial>` + `RenderLayers` を追加するだけでよい。

**SectionCut 同期は `section_cut.rs` 側の追加なし**
- `sync_section_cut_to_materials_system`（`hw_visual/src/material/section_material.rs`）は `cut.is_changed()` のとき `Assets<SectionMaterial>` を **`iter_mut()` で全件更新**する。地形用 `SectionMaterial` を `Assets` に足せばカットは伝播する。一方、地形タイル数ぶんマテリアルアセットが増えると **`SectionCut` 変更時の更新コストが増える**（§8 リスク表参照）。`section_cut.rs` に地形専用クエリを足す必要はない。

**クレート境界の制約**
- `hw_world` は `hw_visual` に依存しない（`Cargo.toml` 確認済み）。`obstacle_cleanup_system` が `Sprite.image` を差し替えている箇所は、3D 化後に `MeshMaterial3d<SectionMaterial>` の差し替えが必要となるが、`SectionMaterial` は `hw_visual` 型のため **`hw_world` 内で直接参照できない**。
  - → `obstacle_cleanup_system` を **hw_world（WorldMap 状態更新）** と **bevy_app（マテリアル差し替え）** に分離する必要がある（§5 M4 参照）。

**`TerrainVisualHandles` の不足**
- 現状は `Handle<Image>` 1 本（dirt のみ）。3D では `Handle<SectionMaterial>` が地形タイプ数（grass / dirt / sand / river）分必要。`bevy_app` に `Terrain3dHandles` リソースを新設する。

---

## 4. 実装方針（高レベル）

### 4.1 メッシュ戦略

- **全タイル共通 1 枚の `Plane3d` メッシュ**（Handle を 1 つ使い回し）：
  ```rust
  // visual_handles.rs 内 init_visual_handles — floor_mesh と同じ生成式を流用
  let terrain_tile_mesh = meshes.add(Plane3d::default().mesh().size(TILE_SIZE, TILE_SIZE));
  ```
  - `TILE_SIZE` は **`hw_core::constants::TILE_SIZE`**（`floor_mesh` 等と同一 import 元）。
  - `Plane3d::default()` は法線 Y 上向きの水平板。Camera3d 俯瞰視点でタイルとして機能する。
  - 全タイルが同一 `Handle<Mesh>` を参照し、差異はマテリアルハンドル（地形タイプ別）のみ。
- **チャンク結合**: MS-3-4 スコープ外。必要なら別タスク。

### 4.2 マテリアル

- **`SectionMaterial`（初回から直接）を採用する**。二段階（StandardMaterial 先行）は行わない。
  理由：`sync_section_cut_to_materials_system` は `Assets<SectionMaterial>` 全走査のため、使うだけで section clip が自動有効。工数対効果が最良。
- テクスチャ付き SectionMaterial の生成パターン：
  ```rust
  // visual_handles.rs 内に地形タイプごとに作成
  fn make_terrain_section_material(
      texture: Handle<Image>,
      section_materials: &mut Assets<SectionMaterial>,
  ) -> Handle<SectionMaterial> {
      section_materials.add(SectionMaterial {
          base: StandardMaterial {
              base_color_texture: Some(texture),
              perceptual_roughness: 1.0,
              reflectance: 0.0,
              opaque_render_method: OpaqueRendererMethod::Forward,
              ..default()
          },
          extension: SectionMaterialExt::default(),
      })
  }
  ```
  (`make_section_material` は `base_color` のみでテクスチャ非対応のため新関数が必要)

### 4.3 `Terrain3dHandles` リソース（新設・bevy_app）

`TerrainVisualHandles`（hw_world）は `Handle<Image>` のまま残す（hw_world はhw_visual に依存できない）。別途 `Terrain3dHandles` を `bevy_app/src/plugins/startup/visual_handles.rs` に追加する：

```rust
#[derive(Resource)]
pub struct Terrain3dHandles {
    pub tile_mesh: Handle<Mesh>,
    pub grass:     Handle<SectionMaterial>,
    pub dirt:      Handle<SectionMaterial>,
    pub sand:      Handle<SectionMaterial>,
    pub river:     Handle<SectionMaterial>,
}
```

`init_visual_handles` の末尾で `commands.insert_resource(Terrain3dHandles { ... })` する。`spawn_map` はこのリソースを `Res<Terrain3dHandles>` として受け取る。

### 4.4 深度・レイヤー

- 3D での Y は全タイル `0.0`（地面）。`TerrainType::z_layer()` は 3D では不要。
- `RenderLayers` は `RenderLayers::from_layers(&[LAYER_3D, LAYER_3D_SHADOW_RECEIVER])` — 建物と同じ。影を受けるため `LAYER_3D_SHADOW_RECEIVER` を含める。
- `TerrainType::z_layer()` は `terrain_border.rs` 削除後に呼び出し元がゼロとなるため **メソッドを削除**（clippy dead_code ゼロ維持）。

### 4.5 `TerrainBorder` の廃止

- `spawn_terrain_borders_if_enabled` を `PostStartup` チェーンから削除。
- `terrain_border.rs` ファイルを削除。`world/map/mod.rs` から `mod terrain_border` 宣言・`use` を除去。
- `generate_terrain_border_specs` / `TerrainBorderKind` 等の `hw_world` 側関数は、他に利用者がなければ同時に削除。

### 4.6 `obstacle_cleanup_system` の分割（クレート境界対応）

`hw_world::obstacle_cleanup_system` をリファクタリングし 2 つの責務に分離する：

1. **hw_world 側（WorldMap 状態更新のみ）**：`Sprite` の差し替えコードを除去し、WorldMap の状態更新（`remove_grid_obstacle` / `set_terrain_at_idx`）のみ残す。
2. **bevy_app 側（新規システム `terrain_material_sync_system`）**：イベント（または同等の通知）で `WorldMap` 上の `terrain_at_idx` に合わせ `MeshMaterial3d<SectionMaterial>` を **`Terrain3dHandles` の該当ハンドル**に差し替える（障害物撤去後の Dirt だけでなく、将来の地形変化にも同じ経路で対応）。

```rust
// bevy_app/src/systems/visual/terrain_material.rs（新規）
pub fn terrain_material_sync_system(
    world_map: WorldMapRead,
    handles: Res<Terrain3dHandles>,
    mut q_tiles: Query<(Entity, &mut MeshMaterial3d<SectionMaterial>), With<Tile>>,
    // ... 変化検知のメカニズム（後述）
) { ... }
```

変化検知の方法（どちらか選択）：
- **Option A**：`obstacle_cleanup_system` が変化したタイルのインデックスを `Events<TerrainChangedEvent>` で発行し、bevy_app システムが受信してマテリアル差し替え。
- **Option B**：`WorldMap` に `dirty_tile_indices: Vec<usize>` を持たせ、bevy_app システムがフレームごとにドレイン。

現時点では **Option A（イベント）を推奨**。Event 型 **`TerrainChangedEvent { idx: usize }` は `hw_core` に定義**し（`hw_core/src/events.rs` 等）、`docs/events.md` に追記する。`obstacle_cleanup_system` が発行、`terrain_material_sync_system` が消費する。**`hw_world` に `App::add_event` を登録する Plugin は無い**ため、イベントの登録は **bevy_app 側**（`obstacle_cleanup_system` と同じ `Plugin`、例：`plugins/logic.rs` の `LogicPlugin`）で `app.add_event::<TerrainChangedEvent>()` を行う（§5 M4-a 参照）。

### 4.7 Bevy 0.18 での注意

- `Plane3d::default().mesh().size(TILE_SIZE, TILE_SIZE)` は `visual_handles.rs` の `floor_mesh` で実績あり。
- `Mesh3d` / `MeshMaterial3d` / `RenderLayers` の付け方は `building_completion/spawn.rs` の `spawn_building_3d_visual` に従う。
- `SectionMaterial` の prepass / shadow の挙動は wall pilot と同じく **ライトの `render_layers` とカメラの交差**を維持する。

---

## 5. 実装ステップ（推奨順序）

### M1: `Terrain3dHandles` リソース新設

**ファイル**: `crates/bevy_app/src/plugins/startup/visual_handles.rs`

1. `Terrain3dHandles` 構造体を追加（§4.3 参照）。
2. `init_visual_handles` 内で `terrain_tile_mesh` ハンドルを作成（`floor_mesh` の直後）：
   ```rust
   let terrain_tile_mesh = meshes.add(Plane3d::default().mesh().size(TILE_SIZE, TILE_SIZE));
   ```
3. 地形タイプ別 `SectionMaterial` を作成（§4.2 のヘルパー関数を使用）：
   ```rust
   let terrain_grass  = make_terrain_section_material(game_assets.grass.clone(),  &mut section_materials);
   let terrain_dirt   = make_terrain_section_material(game_assets.dirt.clone(),   &mut section_materials);
   let terrain_sand   = make_terrain_section_material(game_assets.sand.clone(),   &mut section_materials);
   let terrain_river  = make_terrain_section_material(game_assets.river.clone(),  &mut section_materials);
   ```
4. `commands.insert_resource(Terrain3dHandles { tile_mesh: terrain_tile_mesh, grass: terrain_grass, dirt: terrain_dirt, sand: terrain_sand, river: terrain_river })` する。
5. `cargo check` で確認。

### M2: `spawn_map` を 3D スポーンへ置換

**ファイル**: `crates/bevy_app/src/world/map/spawn.rs`

- `Sprite` を削除し、以下のコンポーネントセットに置換する：
  ```rust
  // Before
  Sprite { image: texture, custom_size: Some(Vec2::splat(TILE_SIZE)), ..default() },
  Transform::from_xyz(pos.x, pos.y, Z_MAP),

  // After（pos2d = grid_to_world(x, y)）
  Mesh3d(terrain_handles.tile_mesh.clone()),
  MeshMaterial3d(terrain_material(terrain, &terrain_handles)),
  Transform::from_xyz(pos2d.x, 0.0, -pos2d.y),
  RenderLayers::from_layers(&[LAYER_3D, LAYER_3D_SHADOW_RECEIVER]),
  ```
- `terrain_texture` 関数を `terrain_material` に書き換え（`Terrain3dHandles` を受け取る）。
- `GameAssets` の参照が不要になれば `Res<GameAssets>` パラメータを除去し `Res<Terrain3dHandles>` に替える。
- `cargo check`、起動して RtT 上に地形が出ることを目視確認。Section clip が地形にも効いているか（矢視 on/off）を確認。

### M3: TerrainBorder オーバーレイ撤去

**ファイル**:
- `crates/bevy_app/src/plugins/startup/mod.rs` — `spawn_terrain_borders_if_enabled` を `.chain()` から削除。
- `crates/bevy_app/src/plugins/startup/startup_systems.rs` — `spawn_terrain_borders_if_enabled` 関数・`spawn_terrain_borders` import・`skip_terrain_borders` 関数を削除。
- `crates/bevy_app/src/world/map/terrain_border.rs` — ファイル削除。
- `crates/bevy_app/src/world/map/mod.rs` — `mod terrain_border;` / `pub use terrain_border::…` 行を削除。
- `hw_world` の `generate_terrain_border_specs` / `TerrainBorderKind` が他に利用者がなければ削除。
- `cargo check`、起動確認。

### M4: `obstacle_cleanup_system` の分割と `TerrainChangedEvent` 導入

#### M4-a: イベント定義と `App` 登録（`hw_core` + bevy_app）

1. **`hw_core`** に `TerrainChangedEvent` を追加する（例：`hw_core/src/events.rs`）。

```rust
#[derive(Event)]
pub struct TerrainChangedEvent {
    pub idx: usize,
}
```

2. **`bevy_app`**：`obstacle_cleanup_system` を登録している **`LogicPlugin`（`crates/bevy_app/src/plugins/logic.rs`）** の `build` で `app.add_event::<TerrainChangedEvent>();` を呼ぶ。`EventReader` / `EventWriter` を使う前に必須。

3. **`docs/events.md`**：Producer（`obstacle_cleanup_system`）・Consumer（`terrain_material_sync_system`）・Timing（Update 等）を 1 行で追記。

#### M4-b: hw_world 側 `obstacle_cleanup_system` の変更

- `Sprite` 関連の Query・**`Res<TerrainVisualHandles>`** を**すべて除去**。
- 地形が更新された各タイル（障害物撤去後の Dirt 等）について `TerrainChangedEvent { idx }` を **`EventWriter<TerrainChangedEvent>`** で発行。
- **同一 PR で** `crates/bevy_app/src/plugins/startup/visual_handles.rs` の **`TerrainVisualHandles` の `insert_resource` を削除**し、`plugins/logic.rs` の `obstacle_cleanup_system` から **`TerrainVisualHandles` 依存が残らない**ようにする（登録漏れでパニックしないよう完了条件で確認）。

#### M4-c: bevy_app 側 `terrain_material_sync_system` の新設

**ファイル**: `crates/bevy_app/src/systems/visual/terrain_material.rs`（新規）

```rust
pub fn terrain_material_sync_system(
    world_map: WorldMapRead,
    terrain_handles: Res<Terrain3dHandles>,
    mut events: EventReader<TerrainChangedEvent>,
    mut q_tiles: Query<&mut MeshMaterial3d<SectionMaterial>, With<Tile>>,
) {
    for ev in events.read() {
        let Some(tile_entity) = world_map.tile_entity_at_idx(ev.idx) else { continue };
        let terrain = world_map.terrain_at_idx(ev.idx);
        let Ok(mut mat_handle) = q_tiles.get_mut(tile_entity) else { continue };
        *mat_handle = MeshMaterial3d(terrain_material(terrain, &terrain_handles));
    }
}

fn terrain_material(terrain: TerrainType, handles: &Terrain3dHandles) -> Handle<SectionMaterial> {
    match terrain {
        TerrainType::Grass => handles.grass.clone(),
        TerrainType::Dirt  => handles.dirt.clone(),
        TerrainType::Sand  => handles.sand.clone(),
        TerrainType::River => handles.river.clone(),
    }
}
```

`VisualPlugin` の `Visual` セット内に `terrain_material_sync_system` を追加登録する（`EventReader` は M4-a の `add_event` 後に有効）。

`cargo check`、岩撤去後にタイルが dirt テクスチャに変わることを目視確認。

### M5: `TerrainType::z_layer()` の削除と Z 定数の整理

- `hw_world/src/terrain.rs` から `z_layer()` メソッドを削除（`terrain_border` 削除後、呼び出し元は消える）。
- **`hw_core::constants::render` の `Z_MAP_SAND` / `Z_MAP_DIRT` / `Z_MAP_GRASS` は、`crates/visual_test/src/building.rs` 等が 2D 地形表示に**直接参照している**可能性がある。**`rg Z_MAP_` でワークスペース全体を確認し**、参照が残る限り **`render.rs` から定数を削除しない**。削除する場合は **visual_test 側を 3D 写像または別定数に合わせてから**行う。
- `#[allow(dead_code)]` でごまかさず、参照ゼロを確認してから削除する。

### M6: ドキュメント更新

- `docs/architecture.md`：地形が RtT パイプライン（Camera3d → RtT）で描画されること、`Terrain3dHandles` の役割を追記。
- `docs/world_layout.md`：2D Z 定数（Z_MAP 系）から 3D への移行内容を記録。terrain Y=0 固定の旨を明記。
- `docs/events.md`：M4-a で未記載なら `TerrainChangedEvent` を追記。
- `milestone-roadmap.md` の MS-3-4 チェックボックスを更新（`Completed`）。

---

## 6. 変更ファイル（想定）

| ファイル | 変更内容 |
| --- | --- |
| `crates/bevy_app/src/plugins/startup/visual_handles.rs` | `Terrain3dHandles` 追加、`make_terrain_section_material` ヘルパー、`init_visual_handles` 拡張 |
| `crates/bevy_app/src/world/map/spawn.rs` | `Sprite` → `Mesh3d` + `MeshMaterial3d<SectionMaterial>` + `RenderLayers`、Transform 写像 |
| `crates/bevy_app/src/world/map/terrain_border.rs` | **ファイル削除** |
| `crates/bevy_app/src/world/map/mod.rs` | `mod terrain_border` / `use` 行削除 |
| `crates/bevy_app/src/plugins/startup/mod.rs` | `.chain()` から `spawn_terrain_borders_if_enabled` を削除 |
| `crates/bevy_app/src/plugins/startup/startup_systems.rs` | `spawn_terrain_borders_if_enabled` 関数・import 削除 |
| `crates/bevy_app/src/systems/visual/terrain_material.rs` | **新規**：`terrain_material_sync_system`（`TerrainChangedEvent` を受信してマテリアル差し替え） |
| `crates/bevy_app/src/systems/visual/mod.rs` | `terrain_material_sync_system` を `Visual` セットに登録 |
| `crates/hw_world/src/terrain_visual.rs` | `obstacle_cleanup_system`：`Sprite` / `TerrainVisualHandles` 除去、`EventWriter<TerrainChangedEvent>`（`hw_core`）で発行 |
| `crates/hw_world/src/terrain.rs` | `z_layer()` メソッド削除 |
| `crates/hw_core/src/events.rs`（または既存イベント集約箇所） | `TerrainChangedEvent` 定義 |
| `crates/hw_core/src/lib.rs` | `TerrainChangedEvent` の re-export（プロジェクト慣習に合わせる） |
| `crates/bevy_app/src/plugins/logic.rs` | `app.add_event::<TerrainChangedEvent>()` |
| `crates/hw_core/src/constants/render.rs` | `Z_MAP_*` は **`rg` で全参照が無くなった後**のみ削除（`visual_test` 等に注意） |
| `crates/visual_test/`（該当ファイル） | `Z_MAP_*` を削除する場合は 3D/別定数へ追随 |
| `docs/architecture.md` / `docs/world_layout.md` / `docs/events.md` | 仕様・イベントカタログ追従 |

**注**: `generate_terrain_border_specs` / `TerrainBorderKind` 等 `hw_world` の border API は、他利用者がなければ同時に削除する。利用者がいれば dead code チェック後に削除。

---

## 7. 検証

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace 2>&1 | grep "^warning:" | grep -v generated
```

### 手動チェックリスト

- [ ] **TopDown モード**：地形（草・土・砂・川）が RtT 上に表示される。Camera2d 側に地形 Sprite なし
- [ ] **矢視モード**：`SectionCut` on/off で地形も含め破綻しない（clip が地形を貫通する、等）
- [ ] **地形テクスチャ確認**：草は草、砂は砂、川は川のテクスチャが当たっている
- [ ] **ウィンドウリサイズ・F4 品質切替**：`RttRuntime` 連鎖で地形テクスチャが追従する（既存同期のまま）
- [ ] **岩（障害物）撤去後**：該当タイルが dirt テクスチャに戻る（`terrain_material_sync_system` 経由）
- [ ] **Familiar / Soul の採取 Designation**：`tile_entity_at_idx` ベースのフローが壊れていない
- [ ] **`cargo clippy` ゼロ警告**（`z_layer` 削除・dead code 除去を含む）

---

## 8. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| タイル数 × メッシュでドローコール増（MAP_WIDTH × MAP_HEIGHT 枚の Mesh3d） | GPU 負荷増 | 全タイル共有 `Handle<Mesh>` 1 つでインスタンシングしやすい。将来チャンク化で追加最適化 |
| 地形タイル数ぶん `SectionMaterial` アセットが増え、`SectionCut` 変更時に `sync_section_cut_to_materials_system` が **全 `SectionMaterial` を `iter_mut()`** する | `SectionCut` 更新の CPU コスト増 | 当面は許容。悪化時は MS-3-6 以降でマテリアル共有・チャンク化・同期対象の絞り込みを検討 |
| MS-3-7 前はクリックが 2D カメラ基準のまま | 地形クリックのズレ | 既知制限として `docs/world_layout.md` に文書化。MS-3-7 で解消 |
| `TerrainChangedEvent` の `add_event` 登録漏れ | `EventWriter` / `EventReader` 使用時のパニック | **`bevy_app` の `LogicPlugin`（`plugins/logic.rs`）**で `app.add_event::<TerrainChangedEvent>()` を登録する（`hw_world` に World Plugin が無い） |
| `init_visual_handles` で `Assets<SectionMaterial>` を参照するが、`MaterialPlugin::<SectionMaterial>` 登録前に呼ばれる場合 | パニック | `MaterialPlugin::<SectionMaterial>` は `HwVisualPlugin` で登録済み。既存 wall material と同じタイミング |
| `generate_terrain_border_specs` など `hw_world` border API を他クレートが使っている場合 | 削除時コンパイルエラー | 削除前に `cargo check --workspace` で利用者を確認してから対処 |
| `TerrainVisualHandles` を削除する際、`visual_handles` の `insert_resource` と `logic.rs` の `obstacle_cleanup` の **`Res` パラメータ**を同じ PR で除去し忘れる | パニック（Resource 未登録／不要 `Res`） | M4-b の完了条件で **両方**をチェック |
| `Z_MAP_SAND` 等を `render.rs` から削除し **`visual_test` が未更新** | コンパイルエラー | M5 のとおり `rg` で参照を確認してから削除 |

---

## 9. 完了の定義（この計画書）

- ロードマップ **MS-3-4** の完了条件（`milestone-roadmap.md`）をすべて満たす。
- 本計画の **§1 成功指標**および **§7 手動チェックリスト**をすべて満たす。
- `cargo clippy --workspace` ゼロ警告（`z_layer` 削除・dead code 除去含む）。
- ステータスを `Completed` に更新し、親ロードマップの MS-3-4 チェックを更新する。

---

## 10. 更新履歴

| 日付 | 内容 |
| --- | --- |
| 2026-03-29 | 初版（Draft） |
| 2026-03-30 | コードベース調査によるブラッシュアップに加え、レビュー反映：`TerrainChangedEvent` は `hw_core` 定義 + `logic.rs` の `add_event`；`Z_MAP_*` は `visual_test` 等の参照確認後にのみ削除；`SectionCut` 全件走査コストをリスク化；`TILE_SIZE` 注記；`docs/events.md` 追記 |
