# ワールドマップ描画 Chunk 化 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `world-map-render-chunking-plan-2026-04-08` |
| ステータス | `完了` |
| 作成日 | `2026-04-08` |
| 最終更新日 | `2026-04-09` |
| 作成者 | `Codex` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題:
  - `spawn_map`（`crates/bevy_app/src/world/map/spawn.rs`）が 100×100=**10,000 個**の `Tile` entity を spawn している。各 entity は `Mesh3d` + `MeshMaterial3d<TerrainSurfaceMaterial>` + `Transform` + `RenderLayers` を持ち、ECS 登録・GlobalTransform 伝播・Visibility frustum culling・render extraction の固定コストが高い。
  - 全タイルで同一の mesh/material handle を共有しているため Bevy の自動 instancing は効くが、10,000 entity 分の毎フレーム extraction・culling コストは残る。
  - 現行の entity 粒度（1 cell = 1 render entity）がロジック設計にも影響しており、責務の境界が曖昧なまま拡張されるリスクがある。
- 到達したい状態:
  - ワールドの論理タイル情報は `GeneratedWorldLayout.terrain_tiles` / `WorldMap.tiles` の配列データとして保持し続ける（変更なし）。
  - 地形描画を **chunk 単位の Mesh3d/entity** に集約し、render entity 数を 10,000 → ≤100 まで削減する。
  - `tile_entity_at_idx()` に依存する **`direct_collect.rs`（Familiar AI の 1 箇所のみ）** が `Designation` + `TaskWorkers` を取得できる互換性を維持する。`Tile` 論理 anchor entity は描画コンポーネントなしで残す。
  - 将来の `BiomeType` 追加時に、ロジックデータ（配列）と描画データ（texture / chunk mesh）を独立に拡張できる。
- 成功指標:
  - 地形描画 entity 数が 10,000 → **49**（16×16 chunk, 7×7 グリッド）まで減少する。
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` が通る。
  - 描画負荷の主因である render extraction / culling 対象 entity 数が削減される。
  - 起動時 `spawn_map_timed` ログ改善は副次効果として扱い、主指標には置かない。
  - `direct_collect.rs` の `tile_entity_at_idx()` 参照が壊れない。

## 2. スコープ

### 対象（In Scope）

- 地形描画を per-tile entity から **chunk render entity（16×16 tiles/chunk）** へ置き換える
- `Tile` marker entity を「描画なし・論理 anchor のみ」に変換する（`Mesh3d` / `MeshMaterial3d` を持たない）
- `TerrainChangedEvent` 経路の確認（今回の変更で既存の texture pixel 更新が壊れないことの検証）
- `Terrain3dHandles.tile_mesh` フィールドの廃止または chunk mesh へのリネーム
- 将来の biome 情報追加を見据えた tile metadata の保持方針を docs に明記する
- 関連ドキュメント（`docs/world_layout.md`, `docs/architecture.md`）の更新

### 非対象（Out of Scope）

- 新しい biome そのものの設計・実装
- worldgen アルゴリズム自体の改善
- far overview / shader LOD の導入（`world-map-lod-strategy-2026-04-06.md` に委ねる）
- 境界リボンの LOD 設計全体
- パス探索や AI ロジックの最適化
- `tile_entity_at_idx()` / `WorldMap.tile_entities` の全面廃止（別フェーズ）

## 3. 現状の詳細把握

### 3.1 spawn_map の現挙動

```rust
// crates/bevy_app/src/world/map/spawn.rs
pub fn spawn_map(/* ... */) {
    for y in 0..MAP_HEIGHT {           // MAP_HEIGHT = 100
        for x in 0..MAP_WIDTH {        // MAP_WIDTH  = 100
            // …
            let entity = commands.spawn((
                Tile,                  // marker component
                Mesh3d(terrain_handles.tile_mesh.clone()),    // 共有ハンドル (Plane3d 32×32)
                MeshMaterial3d::<TerrainSurfaceMaterial>(terrain_handles.surface.clone()),  // 共有ハンドル
                Transform::from_xyz(pos2d.x, 0.0, -pos2d.y),
                building_3d_render_layers(),   // RenderLayers([LAYER_3D, LAYER_3D_SHADOW_RECEIVER])
            )).id();
            world_map.set_tile_entity_at_idx(idx, entity);
        }
    }
}
```

- `tile_mesh` = `Plane3d::default().mesh().size(TILE_SIZE, TILE_SIZE)` = 32×32 wu のフラットプレーン
- 全タイルで同一ハンドルを参照 → mesh は 1 本、material は 1 本
- `tile_entity_at_idx` に entity を登録：`WorldMap.tile_entities: Vec<Option<Entity>>`

### 3.2 tile_entity_at_idx のランタイム依存箇所（全件）

| ファイル | 用途 | 今回の扱い |
| --- | --- | --- |
| `crates/bevy_app/src/world/map/spawn.rs:71` | `set_tile_entity_at_idx` 書き込み | このまま維持 |
| `crates/hw_familiar_ai/src/.../haul/direct_collect.rs:147` | `tile_entity` に対して `Designation` + `TaskWorkers` を Query | `Tile` entity を論理 anchor として残すことで互換維持 |

`demand.rs` 内の `tile_entities` 変数はフロア/壁の construction tile への参照であり、`WorldMap.tile_entities` とは無関係。`direct_collect.rs` が唯一のランタイム呼び出し元。

### 3.3 TerrainChangedEvent の現在の経路

```
obstacle_cleanup_system (hw_world)
  → ev_terrain_changed.write(TerrainChangedEvent { idx })
  → terrain_id_map_sync_system (bevy_app)
      → images.get_mut(&terrain_id_map.image)
      → data[pixel_idx] = terrain_type_to_id_byte(terrain)   // 単純な 1 バイト書き換え
```

**chunk mesh の再生成は不要**。shader は `terrain_id_map` / `terrain_feature_map` をワールド座標で参照するため、texture 1 ピクセル更新だけで chunk entity をそのまま使い続けられる。TerrainChangedEvent の経路は M2 実装後も変更不要。

### 3.4 chunk サイズの決定

| chunk サイズ | chunk 数（100×100 マップ） | 備考 |
| --- | --- | --- |
| 8×8 | ⌈100/8⌉² = 13² = **169** | AABB 粒度が細かい（カメラ culling に有利） |
| 16×16 | ⌈100/16⌉² = 7² = **49** | 約 200 倍削減、辺端に 4 tile の端数 chunk が生じる |
| 32×32 | ⌈100/32⌉² = 4² = **16** | AABB が粗い（マップ全体が 1 カメラで見えるシーンでは差なし） |

**選択: 16×16**（49 entities）。AABB は 16×16×32wu = 512×512wu で十分な frustum culling 効果を持ちつつ、端数処理が単純（端が 4 tile 幅）。

```
100 = 6 × 16 + 4  →  6 フルチャンク + 1 端数チャンク（4 tile 幅） per axis
→ 7 × 7 = 49 chunks
```

## 4. 実装方針（具体）

### 4.1 Tile entity の扱い

`Tile` entity は描画コンポーネントを持たない**論理 anchor**として存続させる。

```rust
// 変更前
commands.spawn((
    Tile,
    Mesh3d(terrain_handles.tile_mesh.clone()),
    MeshMaterial3d::<TerrainSurfaceMaterial>(terrain_handles.surface.clone()),
    Transform::from_xyz(pos2d.x, 0.0, -pos2d.y),
    building_3d_render_layers(),
))

// 変更後
commands.spawn((
    Tile,
    Transform::from_xyz(pos2d.x, 0.0, -pos2d.y),
    // Mesh3d / MeshMaterial3d を削除 → render extraction 対象から外れる
))
```

`Transform` は**省略しない**。`direct_collect.rs` 自体は `Designation` / `TaskWorkers` しか参照しないが、`Designation` を持つ entity は `DesignationSpatialGrid` 更新と UI / 選択系 Query が `&Transform` を前提にしているため、anchor tile から `Transform` を外すと別経路が壊れる。

確認済みの依存:

- `crates/hw_spatial/src/designation.rs`
  - `Query<(Entity, &Transform), (With<Designation>, Or<(Added<Designation>, Changed<Transform>)>)>`
- `crates/bevy_app/src/systems/command/area_selection/queries.rs`
  - `DesignationTargetQuery` が `&Transform` を要求
- `crates/bevy_app/src/interface/ui/panels/task_list/view_model.rs`
  - `DesignationQuery` が `&Transform` を要求

### 4.2 Chunk mesh の生成

```rust
// crates/bevy_app/src/world/map/spawn.rs 追加（または別ファイルに切り出し）

const CHUNK_TILES: i32 = 16;

#[derive(Component)]
pub struct TerrainChunk {
    pub cx: i32,  // chunk grid X (0..CHUNKS_X)
    pub cy: i32,  // chunk grid Y (0..CHUNKS_Y)
}

pub fn spawn_terrain_chunks(
    mut commands: Commands,
    terrain_handles: Res<Terrain3dHandles>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let chunks_x = MAP_WIDTH.div_ceil(CHUNK_TILES);   // 7
    let chunks_y = MAP_HEIGHT.div_ceil(CHUNK_TILES);  // 7

    for cy in 0..chunks_y {
        for cx in 0..chunks_x {
            // 端数チャンクの実タイル幅
            let w = ((cx + 1) * CHUNK_TILES).min(MAP_WIDTH) - cx * CHUNK_TILES;
            let h = ((cy + 1) * CHUNK_TILES).min(MAP_HEIGHT) - cy * CHUNK_TILES;

            // ワールド座標：chunk の中心を求める
            // grid_to_world(x, y) は各タイルの中心 world 座標を返す
            let origin = grid_to_world(cx * CHUNK_TILES, cy * CHUNK_TILES);
            let end    = grid_to_world(cx * CHUNK_TILES + w - 1, cy * CHUNK_TILES + h - 1);
            let center = (origin + end) * 0.5;

            let chunk_mesh = meshes.add(
                Plane3d::default().mesh().size(w as f32 * TILE_SIZE, h as f32 * TILE_SIZE),
            );

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
        chunks_x, chunks_y, chunks_x * chunks_y, CHUNK_TILES, CHUNK_TILES
    );
}
```

**注意**:
- `Plane3d::default().mesh().size(w, h)` は origin（0,0）中心のプレーンを生成する。Transform でチャンク中心に移動することで正しく配置される。
- shader の `terrain_id_map` / `terrain_feature_map` はワールド座標で参照されるため、チャンク境界での継ぎ目は発生しない。
- `building_3d_render_layers()` = `RenderLayers::from_layers(&[LAYER_3D, LAYER_3D_SHADOW_RECEIVER])` は Tile と同一設定を維持。
- フルチャンク（16×16）はメッシュハンドルを共有できる（同サイズなら `meshes.add()` の結果を再利用）。端数チャンクは個別に追加。

### 4.3 Terrain3dHandles の更新

```rust
// visual_handles.rs
#[derive(Resource)]
pub struct Terrain3dHandles {
    // tile_mesh は廃止（chunk に置き換え）
    pub surface: Handle<TerrainSurfaceMaterial>,
}
```

`tile_mesh` フィールドを削除し、chunk mesh は `spawn_terrain_chunks` が `Assets<Mesh>` に直接追加するパターンへ変更。`visual_handles.rs` の `init_visual_handles` 内で `Plane3d` mesh を登録している行を削除する。

### 4.4 startup chain の変更

```
// 変更前（PostStartup chain の一部）
build_terrain_feature_map,
build_terrain_id_map,
init_visual_handles,      // tile_mesh を含む Terrain3dHandles を挿入
spawn_map_timed,          // 10,000 Tile (render) entity を spawn
spawn_boundary_meshes,

// 変更後
build_terrain_feature_map,
build_terrain_id_map,
init_visual_handles,      // Terrain3dHandles（surface のみ）を挿入
spawn_map_timed,          // 10,000 Tile (anchor only) entity を spawn → tile_entities 設定
spawn_terrain_chunks,     // 49 TerrainChunk render entity を spawn  ← 新規追加
spawn_boundary_meshes,
```

`spawn_map_timed` と `spawn_terrain_chunks` は `.chain()` 内で順序を保つ必要がある（`surface` handle が `init_visual_handles` 後に存在することを保証）。

### 4.5 TerrainChangedEvent の扱い（変更なし）

`terrain_id_map_sync_system` は `TerrainChangedEvent` を受けて `terrain_id_map` の 1 ピクセルを書き換えるだけであり、chunk entity への変更は不要。M3 での改修対象から外す。

### 4.6 biome-ready metadata 格納方針

- biome タイプは `WorldMap.tiles: Vec<TerrainType>` と並列に `Vec<BiomeType>` を `WorldMap` に追加する形が最もシンプル。
- 描画用には `TerrainFeatureMap` と同様の独立した `BiomeIdMap` texture（`R8Unorm`）を startup で生成し、`TerrainSurfaceMaterial` の uniform に追加するだけで chunk entity は変更不要。
- この方針は今回実装しないが、M3 で docs に明記する。

### 4.7 効果範囲の明確化

今回の chunk 化で主に減るのは**描画系の固定コスト**であり、`Tile` anchor entity を 10,000 個残す以上、次は主対象にしない。

- 主対象:
  - render extraction 対象数
  - frustum culling 対象数
  - `Mesh3d` / `MeshMaterial3d` / render world へのコピー負荷
- 副次効果:
  - startup 時の render component 生成量減少
- 今回は大きく減らない:
  - `Tile` entity 自体の spawn 数
  - `WorldMap.tile_entities` の管理コスト
  - `direct_collect.rs` 互換維持のために残る anchor entity 数

将来、startup hitch や ECS entity 数そのものをさらに削る場合は、`tile_entity_at_idx()` 依存を外して anchor tile を廃止する後続フェーズを別計画で扱う。

## 5. マイルストーン

### M1: データ契約と依存棚卸しの固定（docs のみ）

- 変更内容:
  - `docs/world_layout.md` に以下を明記する:
    - 論理タイル情報の truth source = `WorldMap.tiles: Vec<TerrainType>`
    - `tile_entities: Vec<Option<Entity>>` は論理 anchor lookup 層（描画 entity ではない）
    - `tile_entity_at_idx()` のランタイム利用箇所: `direct_collect.rs:147` の 1 箇所のみ
    - biome 追加時の格納先 = `WorldMap` 側の並列配列 + startup-baked texture
  - `docs/architecture.md` に chunk render entity (`TerrainChunk`) の概念を追記する。
  - `docs/events.md` に「`TerrainChangedEvent` は texture pixel 更新のみ、chunk mesh 再生成は不要」と明記する。
- 変更ファイル:
  - `docs/world_layout.md`
  - `docs/architecture.md`
  - `docs/events.md`
- 完了条件:
  - [ ] 論理タイル情報の truth source が docs で明文化されている
  - [ ] `tile_entity_at_idx()` のランタイム依存 1 箇所（`direct_collect.rs`）が文書化されている
  - [ ] `TerrainChangedEvent` が chunk dirty を必要としない理由が docs に説明されている
  - [ ] biome 追加先の第一候補が tile 配列 + texture として固定されている
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`（docs 変更なのでエラーは出ないはずだが必須）

### M2: Chunk renderer 導入

- 変更内容:
  1. `spawn_map` から `Mesh3d` / `MeshMaterial3d` / `building_3d_render_layers()` の spawn を削除し、`Tile` を論理 anchor のみに変更する。`Transform` は **必ず残す**。
  2. `spawn_terrain_chunks` 関数を追加し、16×16 tile ごとに `TerrainChunk` + `Mesh3d` + `MeshMaterial3d` + `Transform` + `building_3d_render_layers()` を持つ chunk entity を spawn する。
  3. `Terrain3dHandles` から `tile_mesh` フィールドを削除する。`init_visual_handles` の対応行も削除する。
  4. `startup/mod.rs` の PostStartup chain に `spawn_terrain_chunks` を追加する。
  5. startup ログで chunk 数・旧 tile 数の比較を info! 出力する。
- 変更ファイル:
  - `crates/bevy_app/src/world/map/spawn.rs`（`spawn_map` 修正 + `spawn_terrain_chunks` + `TerrainChunk` コンポーネント追加）
  - `crates/bevy_app/src/plugins/startup/visual_handles.rs`（`tile_mesh` フィールド削除）
  - `crates/bevy_app/src/plugins/startup/startup_systems.rs`（timed wrapper に `spawn_terrain_chunks` 追加または別 wrapper 作成）
  - `crates/bevy_app/src/plugins/startup/mod.rs`（PostStartup chain に `spawn_terrain_chunks` 追加）
- 完了条件:
  - [ ] 起動時の地形描画 entity 数が 49（`TerrainChunk`）に削減されている
  - [ ] `Tile` entity は `Mesh3d` / `MeshMaterial3d` を持たず、`WorldMap.tile_entities` に登録されている
  - [ ] `direct_collect.rs` の `tile_entity_at_idx()` 呼び出しが壊れていない（`Designation` / `TaskWorkers` を Query できる）
  - [ ] 地形の見た目が変化しない（shader はワールド座標参照なのでチャンク境界に継ぎ目がない）
  - [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` が通る
  - [ ] `cargo clippy --workspace` 警告が 0 件
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
  - ゲーム起動で地形が全域崩れず表示される
  - startup ログ `Terrain chunks spawned (7x7 chunks = 49 entities ...)` が出ること
  - inspector または専用 debug 出力で `TerrainChunk` entity 数が 49 であることを確認

### M3: ドキュメント最終整合と biome-ready 明記

- 変更内容:
  - M2 の実装を踏まえ、`docs/world_layout.md` に chunk renderer の最終形（chunk サイズ・entity 構成・mesh 生成方針）を記述する。
  - biome 追加手順（`WorldMap` に `Vec<BiomeType>` を追加し `BiomeIdMap` texture を生成する経路）を docs に追記する。
  - chunk renderer 導入後も `terrain_id_map` / `terrain_feature_map` / `boundary_mask` が正常に動作することを確認し、`docs/events.md` を確定版に更新する。
  - M2 完了後も残った TODO や将来検討事項を `docs/architecture.md` に整理する。
- 変更ファイル:
  - `docs/world_layout.md`
  - `docs/architecture.md`
  - `docs/events.md`
- 完了条件:
  - [ ] chunk renderer の構成が docs で完全に追える
  - [ ] biome 追加時の格納先・texture 拡張経路が docs に記述されている
  - [ ] `TerrainChangedEvent` の consumer 契約（texture 更新のみ・chunk dirty 不要）が docs に確定している
- 検証:
  - 岩除去など `TerrainChangedEvent` 発火シナリオで見た目更新が維持されることを確認
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| chunk mesh の center 計算が 1 タイル分ずれる | 地形が全体的にシフト、境界ずれ | `grid_to_world(first) + grid_to_world(last)) * 0.5` で中心を導出し、単体タイルと比較検証する |
| `Plane3d` の UV が chunk サイズに対して正規化される | shader が UV を world 座標以外で参照していた場合にブレーク | shader（`terrain_surface_material.wgsl`）が world-space uv を使用していることをコード確認（実施済み: `terrain_id_map` / `terrain_feature_map` ともに world-space）|
| `Tile` entity から `Transform` まで除いてしまう | `DesignationSpatialGrid`、UI、選択系 Query が壊れる | `Tile` anchor には `Transform` を必須で残す。レビューで確認済み依存を M2 着手前に再確認する |
| `Tile` entity から `Mesh3d` を除いた後に他の系が `Mesh3d` を Query している | `With<Tile, Mesh3d>` のようなクエリで panic または空結果 | `rg "With<Tile>" --include="*.rs"` / `rg "Query.*Tile"` で確認し、`TerrainChunk` で代替する |
| `Terrain3dHandles.tile_mesh` 削除で依存コードがコンパイルエラー | ビルド失敗 | `rg tile_mesh` で全参照を先に洗い出し、M2 着手前に確認する |
| 端数チャンク（4 tile 幅）の Transform が微妙にずれる | 端部が欠ける、または重なる | フルチャンクと端数チャンクの中心計算を同じロジックで統一する |
| startup の `.chain()` に追加した `spawn_terrain_chunks` が `Terrain3dHandles` を必要とする際、chain 順が誤っている | `Res<Terrain3dHandles>` not found | `init_visual_handles` の直後に `spawn_terrain_chunks` が来るよう chain 内の順序を明示する |
| 成果指標を startup 全体の高速化に寄せすぎる | 実装後に期待値と結果がズレる | 本計画の主目的を render entity 削減に固定し、entity 数そのものの削減は後続フェーズへ分離する |

## 7. 検証計画

- 必須:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace`（0 警告）
- 手動確認シナリオ:
  - 通常起動で地形が全域表示される（TopDown / 矢視両方）
  - 地形位置ずれがない（タイル境界が既存建物・魂の配置と合う）
  - 岩除去（`TerrainChangedEvent` 発火）後に地形テクスチャが更新される
  - `Familiar` が岩/資源を正常に Collect タスクとして認識する（`direct_collect.rs` 経路）
- パフォーマンス確認:
  - startup ログの `spawn_map_timed` と `spawn_terrain_chunks` 所要時間を記録
  - entity inspector で `TerrainChunk` が 49 個、`Tile` が 10,000 個（anchor のみ）であることを確認
  - `cargo run -- --spawn-souls 500 --spawn-familiars 30 --perf-scenario --perf-log-fps` で FPS 傾向を比較

## 8. ロールバック方針

- M1: docs 変更のみなので git revert 1 コミットで即座に戻せる
- M2: `spawn_map` の変更を `git revert` または差分パッチで戻す。`spawn_terrain_chunks` を削除し、旧 per-tile render 行を復元する。`Terrain3dHandles.tile_mesh` を再追加する。
- M3: docs 変更のみ。

## 9. AI 引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`（コードベース調査済み、計画ブラッシュアップ済み）
- 完了済みマイルストーン: なし
- 未着手/進行中: M1, M2, M3 すべて未着手

### 次の AI が最初にやること

1. **`rg tile_mesh`** でビルドエラーを事前に洗い出す（`visual_handles.rs` 以外に `tile_mesh` を参照している箇所がないか確認）
2. **`rg "With<Tile\b"` / `rg "Query.*Tile\b"`** で `Tile` component を mesh 前提で Query している箇所がないか確認する
3. M1 の docs 更新から着手し、`cargo check` で通ることを確認する
4. M2 で `spawn_map` の `Mesh3d` / `MeshMaterial3d` 行を削除し、`spawn_terrain_chunks` を実装する
5. `grid_to_world` の返値型・座標系（Y 軸向き）を確認した上で chunk center 計算を実装する（`Transform::from_xyz(cx, 0.0, -cy)` の符号に注意）

### ブロッカー / 注意点

- **shader は world-space 参照**: `terrain_surface_material.wgsl` は `terrain_id_map` / `terrain_feature_map` をワールド座標で参照するため、chunk 境界に継ぎ目は出ない。UV の mapping は問題にならない。
- **`TerrainChangedEvent` 経路は変更不要**: texture pixel 更新だけで chunk entity の再生成は不要。M3 で確認するだけでよい。
- **LOD 計画との関係**: 本計画は `world-map-lod-strategy-2026-04-06.md` の前提整理である。chunk 化により LOD の「mesh 切替」が容易になるが、LOD 本体は別計画に委ねる。
- **`boundary.rs` は CPU bake が重い**: `spawn_boundary_meshes` の所要時間は chunk 化の効果測定に含めないこと。`spawn_map_timed` 単独で計測する。
- **`Transform` は残す**: `direct_collect.rs` だけを見ると不要に見えるが、`DesignationSpatialGrid` と UI/選択系が `Designation + Transform` を前提にしている。anchor tile から `Transform` を外さない。
- **効果の主戦場は render 側**: `Tile` anchor 10,000 個は残すため、ECS entity 数そのものの削減や startup hitch の大幅改善は別フェーズ課題として扱う。
- **`int i32::div_ceil` は Rust 1.73 stable stable**（本プロジェクトは Rust 2024 edition なので使用可）。

### 参照必須ファイル

- `crates/bevy_app/src/world/map/spawn.rs`（現在の `spawn_map` 実装）
- `crates/bevy_app/src/plugins/startup/visual_handles.rs`（`Terrain3dHandles` 定義）
- `crates/bevy_app/src/plugins/startup/mod.rs`（PostStartup chain 順序）
- `crates/hw_familiar_ai/src/.../haul/direct_collect.rs:147`（`tile_entity_at_idx` 唯一のランタイム呼び出し）
- `crates/hw_core/src/constants/world.rs`（`MAP_WIDTH=100`, `MAP_HEIGHT=100`, `TILE_SIZE=32.0`）
- `crates/hw_core/src/constants/render.rs`（`building_3d_render_layers()`）
- `docs/world_layout.md`, `docs/architecture.md`, `docs/events.md`
- `docs/plans/world-map-lod-strategy-2026-04-06.md`（競合しないよう確認）

### 最終確認ログ

- 最終 `cargo check`: `2026-04-08` / `not run (plan only)`
- 未解決エラー: なし

### Definition of Done

- [ ] M1: truth source とイベント契約が docs で明文化されている
- [ ] M2: 描画 entity が 49 chunk に削減され、`direct_collect.rs` が壊れていない
- [ ] M3: biome 追加経路と TerrainChangedEvent 最終形が docs に記述されている
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` 成功
- [ ] `cargo clippy --workspace` 0 警告

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-08` | `Codex` | 初版作成 |
| `2026-04-08` | `Codex` | レビュー反映: 描画 entity と論理 tile anchor を分離し、描画コスト改善優先の方針へ修正 |
| `2026-04-08` | `Copilot` | コードベース調査に基づき全面ブラッシュアップ（具体的なコード例・依存箇所全件・chunk サイズ決定・TerrainChangedEvent 経路整理） |
| `2026-04-09` | `Codex` | レビュー追記: `Tile` anchor の `Transform` 必須化、効果範囲を render 最適化中心へ明確化 |
