# フェーズ1: TilemapChunk による基本地形描画の置き換え

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `tilemap-chunk-migration-plan-2026-03-12` |
| ステータス | `Draft` |
| 作成日 | `2026-03-12` |
| 最終更新日 | `2026-03-12` |
| 作成者 | `Claude` |
| 関連提案 | `docs/proposals/wfc-terrain-generation-plan-2026-03-12.md` |
| 関連Issue/PR | N/A |

---

## 1. 目的

- **解決したい課題:**
  - 現在、基本地形 100×100 = 10,000 マスが個別の描画 entity（`Sprite + Transform`）として生成されている
  - ベース地形は静的表示が主であり、描画のためだけに 10,000 entity を持つ必要は薄い
  - WFC 地形生成や Blob タイル化に向けて、地形描画の責務を `TilemapChunk` に寄せたい

- **到達したい状態:**
  - ベース地形の描画は `TilemapChunk` に一本化する
  - `world_map.tiles` はランタイム地形データの正本として維持する
  - 木・岩・建物・資源など、個別状態が必要なものは従来どおり個別 entity のままにする
  - 地形由来の採取元ロジック（砂浜・川）のエリア化は **本フェーズでは扱わない**

- **成功指標:**
  - 基本地形の**描画 entity 数**が `10,000 -> 1` になる
  - マップの見た目が移行前後で変わらない
  - `world_map.tiles` を利用する既存ロジックが維持される
  - `cargo check` が通る

---

## 2. スコープ

### 対象（In Scope）

- `src/world/map/spawn.rs` のベース地形描画を `TilemapChunk` に置き換える
- `assets/textures/` への地形 tileset 画像追加
- `src/assets.rs` と `src/plugins/startup/asset_catalog.rs` のアセット読み込み更新
- `world_map.tiles` 変更時に `TilemapChunkTileData` を更新する経路の追加
- 既存の terrain task 依存を壊さないための最小限の logical anchor 維持

### 非対象（Out of Scope）

- 砂浜・川のエリア化 / source area index 化
- ボーダー描画の全面見直し
- 地形生成ロジックそのものの変更（WFC 化など）
- 建物・壁・障害物・資源アイテムの描画方式変更
- 物流 / Familiar task finder の構造改革

---

## 3. 現状とギャップ

### 現状の `spawn_map` 実装

```rust
for y in 0..MAP_HEIGHT {
    for x in 0..MAP_WIDTH {
        let entity = commands.spawn((
            Tile,
            Sprite { image: texture, custom_size: Some(Vec2::splat(TILE_SIZE)), ..default() },
            Transform::from_xyz(pos.x, pos.y, Z_MAP),
        )).id();
        world_map.set_tile_entity_at_idx(idx, entity);
    }
}
```

### 現状の整理

- `world_map.tiles` は地形種別の正本であり、歩行判定・境界生成・配置判定の基礎になっている
- `world_map.tile_entities` は「地形の正本」ではなく、terrain task の参照先として使われている
- `tile_entities` は未使用ではない
  - 砂浜 / 川タイルを採取元として選ぶ処理
  - Mixer 用の sand source 選定
  - 岩除去後の地形見た目更新

### ギャップ

- ベース地形の描画は `TilemapChunk` で集約できる
- ただし、terrain task が地形 tile entity を参照しているため、描画 entity を単純に全廃すると既存ロジックが壊れる
- したがって本フェーズでは、
  - **描画は `TilemapChunk` に移す**
  - **既存ロジックが必要とする terrain anchor は最小限残す**
  という分離が必要

---

## 4. 実装方針

### 4.1 ベース地形描画を `TilemapChunk` へ移行

- Grass / Dirt / Sand / River のベース地形は 1 個の `TilemapChunk` で描画する
- 各マスの地形種別は `world_map.tiles` に保存し続ける
- `TilemapChunkTileData` は `world_map.tiles` から構築する

> **重要: `TilemapChunk` は Bevy 0.18 組み込み機能** (`bevy::sprite_render` モジュール)。
> 外部クレートの追加・独自実装は不要。`"2d"` feature → `bevy_sprite_render` に既に含まれる。
>
> `TilemapChunkPlugin` を App に手動登録すること（`DefaultPlugins` には含まれない）:
> ```rust
> app.add_plugins(TilemapChunkPlugin);
> ```

### 4.2 terrain tile entity は「描画」ではなく「最小限のロジック用アンカー」に限定

本フェーズではエリア化を行わないため、既存の terrain task を壊さない互換層が必要。

- ベース地形の描画 sprite entity は廃止する
- 代わりに、既存ロジックが参照している terrain source 用 tile だけ logical anchor entity を残す
  - 想定対象: `Sand`, `River`
- Grass / Dirt は原則として logical anchor を持たない
- `world_map.tile_entities` は **sparse** に使う
  - Sand / River など task 参照が必要なタイルのみ `Some(entity)`
  - それ以外は `None`

これにより、`world_map.tile_entity_at_idx(idx)` を使う既存コードの大半を温存しつつ、描画 entity 数は削減できる。

### 4.3 地形更新は `world_map.tiles` を正本にして描画へ反映

今後の地形更新は次の順に統一する。

1. `world_map.set_terrain_at_idx(idx, terrain)` で正本を更新
2. chunk entity の `TilemapChunkTileData` を直接変更する

`TilemapChunkTileData` の変更は Bevy の `Changed<TilemapChunkTileData>` で自動検出・再描画される。
dirty tile を別途記録する必要はない。

```rust
// TilemapChunkTileData を Query で取得し直接変更する例
chunk_tile_data.0[idx] = Some(TileData::from_tileset_index(new_terrain.tileset_index()));
```

本フェーズでは 100×100 の単一 chunk を前提にしてよいが、更新 API は chunk 分割へ拡張できる形にしておく。

### 4.4 タイルセット画像は縦スタックの配列テクスチャとして用意

4 種の地形テクスチャ（各 1024×1024）を**縦方向に結合した `1024×4096 PNG`** とし、
`ImageArrayLayout::RowCount { rows: 4 }` で array texture として読み込む。

```text
tileset_terrain.png (1024 x 4096)
┌─────────────┐ ← layer 0: River
│  river.png  │
├─────────────┤ ← layer 1: Sand
│  sand.png   │
├─────────────┤ ← layer 2: Dirt
│  dirt.png   │
├─────────────┤ ← layer 3: Grass
│  grass.png  │
└─────────────┘
```

### 4.5 Bevy 0.18 API に合わせたローダーを使う

```rust
use bevy::image::{ImageArrayLayout, ImageLoaderSettings};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::sprite_render::{TilemapChunk, TilemapChunkTileData, TileData};

let tileset = asset_server.load_with_settings(
    "textures/tileset_terrain.png",
    |settings: &mut ImageLoaderSettings| {
        settings.array_layout = Some(ImageArrayLayout::RowCount { rows: 4 });
        settings.asset_usage = RenderAssetUsages::default();
    },
);
```

`ImageArrayLayout::RowCount { rows: 4 }` / `ImageLoaderSettings::array_layout` は Bevy 0.18.0 で確認済み。

### 4.6 `TilemapChunk` の Transform は map center を原点に置く

本プロジェクトの `grid_to_world(0, 0)` はマップ左下ではなく、**マップ全体を原点中心に配置する座標系**である。

そのため、単一 chunk の `Transform` は map center に置く。

```rust
Transform::from_xyz(0.0, 0.0, Z_MAP)
```

必要なら `TilemapChunk::calculate_tile_transform(UVec2::ZERO)` と
`grid_to_world(0, 0)` を比較して検証する。

### 4.7 `TerrainType` の index 変換は既存 API と重複させない

`TerrainType::priority()` が現在 `u8` を返し（River=0, Sand=1, Dirt=2, Grass=3）、tileset index と一致している。

**決定: `priority()` を `tileset_index() -> u16` に改名する。**

- `TileData::from_tileset_index(u16)` が `u16` を受け取るため、戻り値型を `u16` にしておくと `as` キャストが不要になる
- 呼び出し側は `terrain.tileset_index()` に統一し、`priority()` の残存参照をすべて置き換える
- 改名後に元の `priority()` を削除する（dead code 禁止）

```rust
pub fn tileset_index(self) -> u16 {
    match self {
        TerrainType::River => 0,
        TerrainType::Sand  => 1,
        TerrainType::Dirt  => 2,
        TerrainType::Grass => 3,
    }
}
```

### 4.8 `TilemapChunk` spawn 時の制約と構築例

`TilemapChunk` は `immutable` コンポーネントであり、insert 時に `on_insert` フックが起動する。
フック内で `TilemapChunkTileData` の長さが `chunk_size.x * chunk_size.y` と一致しているか検証される。

**制約:**
- `TilemapChunk` と `TilemapChunkTileData` は同一 `commands.spawn(...)` 呼び出しで同時に挿入する
- `TilemapChunkTileData` の要素数は必ず `MAP_WIDTH * MAP_HEIGHT` と一致させる（不一致は警告のみで描画されない）

**spawn 例:**
```rust
use bevy::sprite_render::{AlphaMode2d, TilemapChunk, TilemapChunkTileData, TileData};

let tile_data = TilemapChunkTileData(
    terrain_tiles.iter()
        .map(|&terrain| Some(TileData::from_tileset_index(terrain.tileset_index())))
        .collect(),
);

commands.spawn((
    TilemapChunk {
        chunk_size: UVec2::new(MAP_WIDTH as u32, MAP_HEIGHT as u32),
        tile_display_size: UVec2::splat(TILE_SIZE as u32),  // TILE_SIZE: f32 = 32.0
        tileset: game_assets.tileset_terrain.clone(),
        alpha_mode: AlphaMode2d::Opaque,
    },
    tile_data,
    Transform::from_xyz(0.0, 0.0, Z_MAP),
));
```

**地形更新時の TilemapChunkTileData 書き換え例:**
```rust
// chunk_entity を Resource や Query で保持しておく
if let Ok(mut tile_data) = q_chunk_tile_data.get_mut(chunk_entity) {
    tile_data.0[idx] = Some(TileData::from_tileset_index(new_terrain.tileset_index()));
    // Changed<TilemapChunkTileData> が自動検出し再描画される
}
```

---

## 5. マイルストーン

### M1: tileset 画像の作成

- **変更内容:** `tileset_terrain.png` を新規追加
- **変更ファイル:**
  - `assets/textures/tileset_terrain.png`
- **注意点:**
  - `scripts/convert_to_png.py` は画像結合ツールではないため、M1 には使わない
  - 画像結合は ImageMagick か専用スクリプトで行う
- **完了条件:**
  - [ ] `tileset_terrain.png` が `1024x4096` の PNG として存在する
  - [ ] 上から River / Sand / Dirt / Grass の順に並んでいる
  - [ ] PNG シグネチャが正しい

### M2: `GameAssets` に tileset ハンドル追加 + Plugin 登録

- **変更内容:** `GameAssets` に `tileset_terrain: Handle<Image>` を追加し、array texture としてロードする。`TilemapChunkPlugin` を App に登録する。
- **変更ファイル:**
  - `src/assets.rs`
  - `src/plugins/startup/asset_catalog.rs`
  - App プラグイン登録箇所（`main.rs` または plugin 一覧）
- **完了条件:**
  - [ ] `load_with_settings` で `array_layout = Some(ImageArrayLayout::RowCount { rows: 4 })` が設定されている
  - [ ] `TilemapChunkPlugin` が App に `add_plugins` されている
  - [ ] `cargo check` が通る

### M3: `spawn_map` を描画 + logical anchor の二層に分離（obstacle.rs 更新を含む）

- **変更内容:**
  - ベース地形描画を `TilemapChunk` に置き換える（§4.8 の spawn 例を参照）
  - Sand / River の logical anchor entity を生成する（anchor entity には `Tile` + `Transform` + タスク状態コンポーネント `Designation`, `WorkingOn` 相当が必要。`q_task_state.get(tile_entity)` が通ること）
  - `world_map.tile_entities` は sparse に更新する（Sand/River のみ `Some(entity)`）
  - `src/systems/obstacle.rs` の岩除去処理を sprite 差し替えから `TilemapChunkTileData` 更新に変更する（**M3 と同時実施。M3 単独で完了にしないこと**）
  - `TerrainType::priority()` を `tileset_index() -> u16` に改名し、既存の `priority()` 呼び出しを全て置き換える
  - `GameAssets` の `grass`, `dirt`, `sand`, `river` テクスチャハンドルを削除する（`tileset_terrain` に統合。dead code 禁止ルール）
- **変更ファイル:**
  - `src/world/map/spawn.rs`
  - `src/world/map/mod.rs` または `crates/hw_world/src/map/mod.rs`（chunk entity 保持 / 補助 API）
  - `src/systems/obstacle.rs`
  - `crates/hw_world/src/terrain.rs`（`priority()` → `tileset_index()` 改名）
  - `src/assets.rs`（個別テクスチャハンドル削除）
  - `src/plugins/startup/asset_catalog.rs`（個別テクスチャロード削除）
- **完了条件:**
  - [ ] 基本地形描画 sprite のスポーンが削除されている
  - [ ] 1 個の `TilemapChunk` が `TilemapChunkTileData` と同時に生成される
  - [ ] `TilemapChunkTileData` の長さが `MAP_WIDTH * MAP_HEIGHT` と一致している
  - [ ] Sand / River anchor entity がタスク状態クエリを通過できるコンポーネントを持つ
  - [ ] 岩除去後に `TilemapChunkTileData` が Dirt に更新される（obstacle.rs 更新済み）
  - [ ] `TerrainType::tileset_index()` が存在し、`priority()` が削除されている
  - [ ] `GameAssets` から `grass`, `dirt`, `sand`, `river` ハンドルが削除されている
  - [ ] `cargo check` が通る

### M4: 座標・表示整合の確認

- **変更内容:** 既存座標系と `TilemapChunk` 表示を照合する
- **注記:** `calculate_tile_transform` はチャンク中心を Transform の原点として計算する（左下原点ではない）。Transform `(0, 0, Z_MAP)` を設定すると chunk 全体がマップ中心に配置される。`grid_to_world(0, 0)` と比較して整合を確認すること。
- **完了条件:**
  - [ ] ベース地形が移行前後で同じ位置に表示される
  - [ ] ボーダーオーバーレイがズレない
  - [ ] Soul / Familiar / 建物の表示位置が地形に対してズレない

### M5: `tile_entities` 命名の整理（任意）

- **変更内容:**
  - `tile_entities` の意味が「全地形描画 entity」から「terrain task anchor の sparse index」に変わるため、必要なら命名を見直す
- **候補:**
  - `tile_entities` を維持しつつコメントで意味を明記
  - 将来的に `terrain_task_entities` などへ改名
- **備考:** 本フェーズでは互換優先のため rename は必須ではない

---

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `tile_entities` を未使用と誤認して全廃する | 砂浜 / 川の task 選定が壊れる | 本フェーズでは sparse anchor として維持する |
| `TilemapChunk` を外部クレート or 独自実装しようとする | 不要な複雑化・Bevy API との競合 | `bevy::sprite_render::TilemapChunk` を使う。外部クレート追加・独自実装禁止 |
| `TilemapChunkPlugin` を登録し忘れる | `TilemapChunkMeshCache` が存在せず `on_insert` フックが警告を出して描画されない | M2 の完了条件に登録を含める |
| `TilemapChunk` と `TilemapChunkTileData` を別々に spawn する | `on_insert` フック起動時に `TilemapChunkTileData` が見つからず描画されない | 同一 `commands.spawn(...)` で同時挿入する |
| `TilemapChunkTileData` の長さが chunk サイズと不一致 | 警告のみで描画されない（サイレント失敗） | 長さを `MAP_WIDTH * MAP_HEIGHT` に合わせて検証する |
| M3 完了後に obstacle.rs が未修正のまま | 岩除去後の視覚更新がサイレントにスキップされる | M3 に obstacle.rs 更新を含め、分離しない |
| `TilemapChunk` の座標原点を左下と誤認する | 地形全体がズレる | chunk `Transform` を `(0,0,Z_MAP)` とし、`calculate_tile_transform()` と `grid_to_world()` を比較する |
| array texture のロード設定を誤る | テクスチャ不正 or コンパイルエラー | `array_layout = Some(ImageArrayLayout::RowCount { rows: 4 })` を使用（Bevy 0.18.0 確認済み） |
| 1 chunk 更新で全 tile data を再 pack する | 頻繁な地形更新時の CPU 負荷 | フェーズ1では許容。将来必要なら複数 chunk 化を検討する |
| ボーダーが起動時生成のまま | 地形変化後に境界が古いままになる | 本フェーズでは既存仕様として据え置く。必要なら別提案で再生成対応する |

---

## 7. 検証計画

- **必須:** `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
- **手動確認シナリオ:**
  - ゲーム起動直後にマップが正しく表示される
  - 川・砂・土・草が移行前と同じ位置に表示される
  - ボーダーオーバーレイが地形境界に正しく重なる
  - 砂浜 / 川タイル由来の task が従来どおり発行・割当される
  - 岩撤去後に該当タイルが Dirt 表示へ更新される

---

## 8. ロールバック方針

- `spawn_map` と terrain visual sync を中心に変更するため、ロールバックは比較的容易
- `world_map.tiles` 自体の責務は変えない
- 新規アセットは追加のみで既存画像を置き換えない

---

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: M1〜M6 すべて未着手

### 次のAIが最初にやること

1. `world_map.tile_entity_at_idx()` の参照箇所を再確認し、Sand / River 以外に anchor が必要なケースがないか最終確認する
2. `tileset_terrain.png` を `1024x4096` の縦スタックで作成する
3. `asset_catalog.rs` を Bevy 0.18 の `array_layout` で更新し、`TilemapChunkPlugin` を App に登録する

### TilemapChunk の実装に関する重要事項（独自実装禁止）

- **`TilemapChunk` は Bevy 0.18 組み込み** → `bevy::sprite_render::TilemapChunk`
- **外部クレート追加・独自実装は禁止**。`"2d"` feature → `bevy_sprite_render` に既に含まれる
- 使用する型: `TilemapChunk`, `TilemapChunkTileData`, `TileData`, `TilemapChunkPlugin`（全て `bevy::sprite_render` から）
- `TilemapChunk` は `immutable` コンポーネント → `TilemapChunkTileData` と同時 spawn 必須
- `TilemapChunkPlugin` を App に登録しないと描画されない（`TilemapChunkMeshCache` 未初期化）
- `TilemapChunkTileData` は `Vec<Option<TileData>>` で長さ = `chunk_size.x * chunk_size.y` が必須
- 更新は `TilemapChunkTileData.0[idx]` を直接変更するだけ（`Changed<>` で自動反映）

### ブロッカー/注意点

- 地形ロジックの area 化は **このフェーズではやらない**
- `tile_entities` は未使用ではない（Sand/River anchor として維持）
- `scripts/convert_to_png.py` は画像結合に使えない
- `TilemapChunk` の chunk `Transform` は map center 前提で考える
- obstacle.rs の更新は M3 と同時実施（分離禁止）
- `TilemapChunkTileData` 更新は off-screen でも走るが、フェーズ1では許容する

### 参照必須ファイル

- `src/world/map/spawn.rs`
- `src/world/map/mod.rs`
- `src/systems/obstacle.rs`
- `src/world/map/terrain_border.rs`
- `crates/hw_world/src/map/mod.rs`
- `crates/hw_world/src/terrain.rs`
- `src/assets.rs`
- `src/plugins/startup/asset_catalog.rs`

### Definition of Done

- [ ] 基本地形の描画 entity 数が `10,000 -> 1` になる
- [ ] `world_map.tiles` が引き続き地形正本として機能する
- [ ] Sand / River terrain task が壊れない
- [ ] 地形更新時に `TilemapChunk` 表示が追従する
- [ ] `cargo check` が成功する

---

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-12` | `Claude` | 初版作成 |
| `2026-03-12` | `Codex` | エリア化をスコープ外へ移し、描画とロジックの分離方針・sparse anchor 方針・runtime visual sync 方針を反映 |
| `2026-03-12` | `Claude` | レビュー反映: TilemapChunk=Bevy組み込み明示・独自実装禁止追記、ImageArrayLayout API確認、TilemapChunkPlugin登録、spawn制約と構築例追加、§4.3のdirty記録削除、§4.7をtileset_index()改名に決定、M3にobstacle.rs更新を統合、M4削除、リスク表拡充 |
