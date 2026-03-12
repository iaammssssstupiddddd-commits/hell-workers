# フェーズ1: TilemapChunk による基本地形描画の置き換え

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `tilemap-chunk-migration-plan-2026-03-12` |
| ステータス | `Draft` |
| 作成日 | `2026-03-12` |
| 最終更新日 | `2026-03-12` |
| 作成者 | `Claude` |
| 関連提案 | `docs/plans/wfc-terrain-generation-plan-2026-03-12.md` |
| 関連Issue/PR | N/A |

---

## 1. 目的

- **解決したい課題:**
  - 現在、基本地形タイル100×100=10,000個が個別のECSエンティティ（`Tile + Sprite + Transform`）として存在している
  - これは10,000ドローコール相当のCPU負荷をもたらしている（Bevy内部でバッチングされるが、エンティティ管理コストが残る）
  - 地形生成WFC移行（フェーズ2）・Blob-47描画改善（フェーズ3）に向けて、描画基盤をシンプルにしておきたい

- **到達したい状態:**
  - 基本地形タイルをBevy 0.17+標準の `TilemapChunk` コンポーネント1エンティティで描画する
  - ボーダースプライト（`TerrainBorder`）は本計画の対象外として現状維持
  - `WorldMap.tile_entities` への基本タイルエンティティ保存が不要になる

- **成功指標:**
  - 基本地形タイルのエンティティ数が 10,000 → 1 に削減される
  - マップの見た目が移行前後で変わらない
  - `cargo check` が通る

---

## 2. スコープ

### 対象（In Scope）

- `src/world/map/spawn.rs` の `spawn_map` 関数置き換え
- `assets/textures/` へのタイルアトラス画像追加
- `src/assets.rs` の `GameAssets` にアトラスハンドル追加
- `crates/hw_world/src/map/mod.rs` の `WorldMap.tile_entities` フィールド整理

### 非対象（Out of Scope）

- ボーダー描画（`terrain_border.rs`）: 現状の個別スプライト方式を維持
- 地形生成ロジック（`mapgen.rs`）: WFC移行計画に委ねる
- 建設物・壁・障害物などの描画: 別レイヤーのため影響なし

---

## 3. 現状とギャップ

### 現状の `spawn_map` 実装

```rust
// src/world/map/spawn.rs
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
// → 10,000 エンティティ生成
```

**問題:**
- テクスチャが地形種別ごとに別々の `Handle<Image>`（grass.png, dirt.png, sand_terrain.png, river.png）
- `Tile` マーカーコンポーネントを持つが、**現在どのシステムもこれをクエリしていない**（タグとしてのみ存在）
- `WorldMap.tile_entities` に10,000エンティティIDを保存しているが、現状は未使用

### `TilemapChunk` の要件

```rust
// Bevy 0.18 API
pub struct TilemapChunk {
    pub chunk_size: UVec2,          // タイル数単位のチャンクサイズ
    pub tile_display_size: UVec2,   // 1タイルの表示サイズ（ピクセル）
    pub tileset: Handle<Image>,     // 配列テクスチャ（全タイル種別を含む）
    pub alpha_mode: AlphaMode2d,
}

pub struct TilemapChunkTileData(pub Vec<Option<TileData>>);

pub struct TileData {
    pub tileset_index: u16,  // 配列テクスチャのレイヤーインデックス
    pub color: Color,
    pub visible: bool,
}
```

**最重要制約:** `tileset` は **配列テクスチャ（array texture）** でなければならない。
現状の個別PNG4枚をそのまま渡すことはできない。

---

## 4. 実装方針

### タイルセットアトラスの作成

4種の地形テクスチャ（各1024×1024）を縦方向に結合した **4096×1024 PNG** を作成し、
Bevy の `ImageArrayLayout::RowCount { rows: 4 }` でロードして配列テクスチャとして使用する。

```
tileset_terrain.png (4096 × 1024)
┌─────────────┐ ← layer 0: River   (index 0)
│  river.png  │
├─────────────┤ ← layer 1: Sand    (index 1)
│  sand.png   │
├─────────────┤ ← layer 2: Dirt    (index 2)
│  dirt.png   │
├─────────────┤ ← layer 3: Grass   (index 3)
│  grass.png  │
└─────────────┘
```

レイヤーインデックスの対応:

| TerrainType | tileset_index |
|-------------|--------------|
| River       | 0            |
| Sand        | 1            |
| Dirt        | 2            |
| Grass       | 3            |

`TerrainType` に `tileset_index() -> u16` メソッドを追加してこの対応を管理する。

### 配列テクスチャのロード

```rust
// src/assets.rs または専用のローダー
use bevy::render::texture::{ImageArrayLayout, ImageLoaderSettings};

let tileset = asset_server.load_with_settings(
    "textures/tileset_terrain.png",
    |settings: &mut ImageLoaderSettings| {
        settings.array = Some(ImageArrayLayout::RowCount { rows: 4 });
    },
);
```

### 新しい `spawn_map` 実装

```rust
pub fn spawn_map(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut world_map: WorldMapWrite,
) {
    let terrain_tiles = generate_base_terrain_tiles(MAP_WIDTH, MAP_HEIGHT, super::SAND_WIDTH);

    // テクスチャのみ WorldMap に保存（既存の terrain 設定は維持）
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = world_map.pos_to_idx(x, y).unwrap();
            world_map.set_terrain_at_idx(idx, terrain_tiles[idx]);
        }
    }

    // TilemapChunkTileData を構築
    let tile_data: Vec<Option<TileData>> = terrain_tiles
        .iter()
        .map(|&terrain| Some(TileData::from_tileset_index(terrain.tileset_index())))
        .collect();

    // 1エンティティでマップ全体を描画
    commands.spawn((
        TilemapChunk {
            chunk_size: UVec2::new(MAP_WIDTH as u32, MAP_HEIGHT as u32),
            tile_display_size: UVec2::splat(TILE_SIZE as u32),
            tileset: game_assets.tileset_terrain.clone(),
            alpha_mode: AlphaMode2d::Opaque,
        },
        TilemapChunkTileData(tile_data),
        Transform::from_xyz(/* チャンク左下のワールド座標 */, Z_MAP),
    ));
}
```

### `TilemapChunk` の Transform（座標系の注意）

現状: 個別スプライトは `grid_to_world(x, y)` でタイル中心に配置。

`TilemapChunk` の場合、`Transform` はチャンク**全体**の基準点を指定する。
Bevy の `TilemapChunk` は原点がチャンクの**左下**（または中心）になる設計のため、
既存の `grid_to_world` 関数との整合を確認する必要がある。

```rust
// 確認必須: チャンク原点 = マップ左下のワールド座標
let chunk_origin = grid_to_world(0, 0) - Vec2::splat(TILE_SIZE / 2.0);
Transform::from_xyz(chunk_origin.x, chunk_origin.y, Z_MAP)
```

実際の原点挙動は M2 で `calculate_tile_transform()` を用いて検証する。

### `WorldMap.tile_entities` の扱い

現状: `Vec<Option<Entity>>` に10,000エンティティIDを保存。
調査結果: **現在どのシステムもこのフィールドを通じてタイルエンティティを参照していない。**

移行後の方針:
- `tile_entities` フィールドから基本タイルエンティティを保存しなくなる
- フィールド自体は後方互換のため残しつつ、エントリを `None` のままにする
- 将来 `TilemapChunkTileData` への直接アクセスが必要になった場合はチャンクエンティティを別途保存する

### ボーダースプライト（変更なし）

`TerrainBorder` エンティティ群は引き続き個別スプライトとして機能する。
Z層（0.01〜0.03）はベースチャンク（0.0）より上なので、描画順は維持される。

### Bevy 0.18 API での注意点

- `TilemapChunk` は `bevy::sprite_render` モジュールに存在（`use bevy::sprite_render::...`）
- `ImageArrayLayout` は `bevy::render::texture` にある
- アトラステクスチャのロードは `load_with_settings` を使用（非同期のためスタートアップシステムが完了するまでに読み込み完了しているか確認が必要）

---

## 5. マイルストーン

### M1: タイルセット配列テクスチャの作成

- **変更内容:** 4種の地形テクスチャを縦に結合した `tileset_terrain.png` を作成
- **変更ファイル:**
  - `assets/textures/tileset_terrain.png`（新規作成）
  - `scripts/convert_to_png.py` を活用（既存スクリプト使用）
- **作成コマンド:**
  ```bash
  python scripts/convert_to_png.py "..." "assets/textures/tileset_terrain.png"
  # または ImageMagick で4画像を縦結合:
  # convert river.png sand_terrain.png dirt.png grass.png -append tileset_terrain.png
  ```
- **完了条件:**
  - [ ] `tileset_terrain.png` が 4096×1024 の PNG として存在する
  - [ ] 上から River/Sand/Dirt/Grass の順に並んでいる
  - [ ] PNG シグネチャが `89 50 4e 47 0d 0a 1a 0a`
- **検証:** `head -c 8 assets/textures/tileset_terrain.png | od -An -t x1`

---

### M2: `TerrainType` にインデックスメソッド追加

- **変更内容:** `tileset_index() -> u16` を `TerrainType` に追加
- **変更ファイル:**
  - `crates/hw_world/src/terrain.rs`
- **実装:**
  ```rust
  impl TerrainType {
      pub fn tileset_index(self) -> u16 {
          match self {
              TerrainType::River => 0,
              TerrainType::Sand  => 1,
              TerrainType::Dirt  => 2,
              TerrainType::Grass => 3,
          }
      }
  }
  ```
- **完了条件:**
  - [ ] `tileset_index()` メソッドが全4種に対して正しい値を返す
- **検証:** `cargo check`

---

### M3: `GameAssets` にアトラスハンドル追加 + ローディング実装

- **変更内容:** `GameAssets` に `tileset_terrain: Handle<Image>` フィールドを追加し、
  `load_with_settings` で配列テクスチャとしてロード
- **変更ファイル:**
  - `src/assets.rs`
  - `src/plugins/startup/asset_catalog.rs`（ロード処理）
- **完了条件:**
  - [ ] `game_assets.tileset_terrain` が有効な `Handle<Image>` を持つ
  - [ ] `ImageArrayLayout::RowCount { rows: 4 }` でロードされている
- **検証:** `cargo check`

---

### M4: `spawn_map` の `TilemapChunk` 移行

- **変更内容:** `spawn_map` を `TilemapChunk` ベースに書き換え、
  個別スプライトエンティティのスポーンを削除
- **変更ファイル:**
  - `src/world/map/spawn.rs`
- **完了条件:**
  - [ ] 個別 `Tile` エンティティのスポーンが削除されている
  - [ ] `TilemapChunk` + `TilemapChunkTileData` で1エンティティとして描画される
  - [ ] `world_map.set_terrain_at_idx()` は引き続き呼ばれ地形データが保存される
  - [ ] `world_map.tile_entities` への書き込みが削除されている
- **検証:** `cargo check` + ゲーム起動して地形が表示されることを目視確認

---

### M5: Transform の整合確認・調整

- **変更内容:** チャンクのワールド座標原点を `grid_to_world` との整合に合わせて調整
- **変更ファイル:**
  - `src/world/map/spawn.rs`（Transform の計算部分）
- **完了条件:**
  - [ ] 地形タイルの表示位置が移行前後で一致している
  - [ ] ボーダースプライトが地形タイルと正しくアライメントされている
  - [ ] キャラクター・アイテムの表示位置が地形に対してズレていない
- **検証:** ゲーム起動して各種エンティティの配置を目視確認

---

### M6: `Tile` コンポーネント削除（任意）

- **変更内容:** `src/world/map/mod.rs` の `Tile` 構造体とそのインポートを削除
- **変更ファイル:**
  - `src/world/map/mod.rs`
  - `src/world/map/spawn.rs`
- **前提:** どのシステムも `With<Tile>` でクエリしていないことを再確認してから実施
- **完了条件:**
  - [ ] `Tile` コンポーネントが削除されている
  - [ ] `grep -r "With<Tile>" src/` で0件
- **検証:** `cargo check`

---

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `ImageArrayLayout` のロードが非同期完了前に描画される | マップが真っ黒になる | スタートアップシステムでアセットの準備完了を待つか、テクスチャをデフォルト値で初期化する |
| `TilemapChunk` の Transform 原点が想定と異なる | 全タイルが数十ピクセルズレる | M5 で `calculate_tile_transform()` を使って期待座標と比較検証 |
| `tile_entities` フィールドが実は他のシステムから参照されている | ランタイムパニック | M4 着手前に `grep -r "tile_entities" src/` で全参照を確認する |
| `TilemapChunk` が `AlphaMode2d::Opaque` でボーダーを隠す | ボーダースプライトが見えなくなる | Z層差（0.0 vs 0.01-0.03）で解決するはずだが、ボーダーが半透明の場合は `AlphaMode2d::Blend` に変更 |
| テクスチャの縦結合順序ミス | 地形の見た目が入れ替わる | M1 完了後に `tileset_terrain.png` をビューアで確認 |

---

## 7. 検証計画

- **必須:** `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
- **手動確認シナリオ:**
  - ゲーム起動直後にマップが表示される（真っ黒にならない）
  - 川・砂・土・草が移行前と同じ位置に表示される
  - ボーダーオーバーレイが地形境界に正しく表示される
  - キャラクターや建設物が地形上の正しい位置に表示される
  - キャラクターが歩行可能/不可能な地形を正しく判定している（WorldMap のデータが正しい）

---

## 8. ロールバック方針

- 本計画の変更は `spawn_map` の実装入れ替えが中心であり、`WorldMap` のデータ構造は変わらない
- git revert 1コミットで旧 `spawn_map` に戻せる
- `tileset_terrain.png` は新規追加のみで既存アセットを変更しないため、ロールバック後も副作用なし

---

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: M1〜M6 すべて未着手

### 次のAIが最初にやること

1. `grep -r "tile_entities" src/ crates/` で `WorldMap.tile_entities` の全参照を確認（M4のリスク検証）
2. `grep -r "With<Tile>" src/` で `Tile` コンポーネントの全クエリ参照を確認（M6の前提確認）
3. ImageMagick または Python/Pillow で `tileset_terrain.png` を作成（M1）

### ブロッカー/注意点

- **M1の画像結合順序は厳守:** River=0, Sand=1, Dirt=2, Grass=3 の順（上から）
- **ボーダースプライトは変更しない:** `terrain_border.rs` は本計画でタッチしない
- **`TilemapChunk` は `bevy::sprite_render` にあり、プレリュードに含まれない可能性がある:** `use bevy::sprite_render::{TileData, TilemapChunk, TilemapChunkTileData};` を明示的に追加する

### 参照必須ファイル

- `src/world/map/spawn.rs` — 変更対象メイン
- `src/world/map/mod.rs` — `Tile` コンポーネント定義
- `src/world/map/terrain_border.rs` — 変更しないが座標系確認のため参照
- `crates/hw_world/src/terrain.rs` — `tileset_index()` 追加対象
- `src/assets.rs` — `GameAssets` 定義
- `src/plugins/startup/asset_catalog.rs` — アセットロード処理
- `crates/hw_core/src/constants/render.rs` — Z_MAP 定数
- `crates/hw_core/src/constants/world.rs` — MAP_WIDTH, MAP_HEIGHT, TILE_SIZE

### 最終確認ログ

- 最終 `cargo check`: 未実施（計画のみ）
- 未解決エラー: なし（実装前）

### Definition of Done

- [ ] 基本地形タイルのエンティティ数が 10,000 → 1 に削減
- [ ] マップの見た目が移行前後で一致
- [ ] `cargo check` が成功

---

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-12` | `Claude` | 初版作成 |
