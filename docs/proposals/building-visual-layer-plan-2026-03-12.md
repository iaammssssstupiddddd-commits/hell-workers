# 建築物ビジュアルレイヤー方式への移行計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `building-visual-layer-plan-2026-03-12` |
| ステータス | `Draft` |
| 作成日 | `2026-03-12` |
| 最終更新日 | `2026-03-12` |
| 作成者 | `Claude` |
| 関連提案 | `docs/plans/spatial-grid-architecture-plan-2026-03-12.md`（配管・配線の空間データ）<br>`docs/plans/tilemap-chunk-migration-plan-2026-03-12.md`（フロア・壁の静的描画） |
| 関連Issue/PR | N/A |

---

## 1. 目的

- **解決したい課題:**
  - 現状の建築物は1エンティティ = 1スプライト構造のため、照明・家具・配線など複数の視覚要素を同一セルに重ねる設計ができない
  - 照明・配線などの新システムを追加するたびに、既存の建築物スプライト管理に手を加える必要が生じる
  - 配線の接続描画（隣接セルに応じたスプライト選択）が、既存の壁接続ロジックと同一パターンで実装できるが、現構造では追加場所が曖昧

- **到達したい状態:**
  - 建築物セルに対して**Z軸スロットで分割された独立ビジュアルレイヤー**を重ねられる
  - 照明・家具・配線それぞれが独立したエンティティとして存在し、互いに干渉しない
  - 空間データ（`WorldMap.pipes` 等）の変化を Observer で検知し、ビジュアルレイヤーが自動スポーン・更新される

- **成功指標:**
  - フロアの上に家具・配線・照明を独立して配置・削除できる
  - 配線スプライトが隣接セルの配線を参照して自動更新される（壁接続と同一パターン）
  - 新視覚要素の追加時に既存レイヤーのコードを変更しなくて済む
  - `cargo check` が通る

---

## 2. 現状の問題構造

### 2-1. 単一スプライトの限界

```rust
// 現状: 完成建築物は1エンティティ = 1スプライト
commands.spawn((
    Building { kind, is_provisional },
    Sprite { image: texture, .. },  // 1枚のみ
    Transform::from_xyz(x, y, Z_AURA),
));
// → 同じエンティティに Sprite を2つ持てない
// → 照明をフロアの上に重ねたい場合、別エンティティを手動管理する必要がある
```

### 2-2. 新システム追加時の設計上の曖昧さ

配線・照明を追加しようとすると現状では:
- 「建築物エンティティの子として追加するか」
- 「完全に独立したエンティティとして並べるか」
- 「WorldMap の別フィールドで管理するか」

の判断基準がなく、追加のたびに設計が揺れる。

### 2-3. Z 層が未整理

```rust
// 現状のビルディング関連 Z 定数（render.rs より）
Z_MAP      = 0.0    // 地形
Z_ROOM_OVERLAY = 0.08
Z_ITEM     = 0.1    // アイテム
Z_AURA     = 0.2    // Blueprint がここに配置される
// ← 建築物完成後の Z が明示定数化されていない（0.0 がデフォルト）
// ← 配線・家具・照明用の Z スロットが存在しない
```

---

## 3. 提案: 視覚レイヤースロット方式

### 3-1. Z スロット定義

建築物セル内の視覚要素を Z 軸で明確に分割する。

```rust
// crates/hw_core/src/constants/render.rs に追加
pub const Z_BUILDING_BASE:      f32 = 0.08;  // フロア・地面面
pub const Z_BUILDING_PIPE:      f32 = 0.09;  // 配管オーバーレイ（床面直上）
pub const Z_BUILDING_WIRE:      f32 = 0.10;  // 配線オーバーレイ
pub const Z_BUILDING_STRUCT:    f32 = 0.12;  // 壁・構造体
pub const Z_BUILDING_FURNITURE: f32 = 0.15;  // 家具・設備
pub const Z_BUILDING_LIGHT:     f32 = 0.18;  // 照明エフェクト（αブレンド）
pub const Z_BUILDING_DECAL:     f32 = 0.19;  // デカール・汚れ・サイン
// Z_ITEM = 0.1 はアイテム（拾えるオブジェクト）のままとし建築と共存
// Z_ITEM_OBSTACLE = 0.5 は木・岩などのままとする
```

### 3-2. ECS 表現（Relationship 方式）

プロジェクト標準の ECS Relationship でレイヤーエンティティを建築物エンティティに接続する。

```rust
// 新規コンポーネント（crates/hw_visual/src/layer/ に追加）
#[derive(Component)]
pub struct VisualLayer {
    pub kind: VisualLayerKind,
    pub cell: (i32, i32),
}

pub enum VisualLayerKind {
    Floor,
    Pipe,
    Wire,
    Furniture(FurnitureKind),
    Light { radius: f32, color: Color },
    Decal(DecalKind),
}

// Building --[HasVisualLayer]--> VisualLayer Entity
// VisualLayer は Building と独立して存在し、単独で Despawn 可能
```

### 3-3. 各レイヤーの描画特性

| レイヤー | Z スロット | AlphaMode | 接続ロジック | 更新トリガー |
|---------|-----------|-----------|------------|-------------|
| Floor | `Z_BUILDING_BASE` | Opaque | なし | 建築完了時 |
| Pipe | `Z_BUILDING_PIPE` | Blend | 4方向隣接（壁接続と同方式） | `WorldMap.pipes` 変化時 |
| Wire | `Z_BUILDING_WIRE` | Blend | 4方向隣接（壁接続と同方式） | `WorldMap.wires` 変化時 |
| Struct (壁) | `Z_BUILDING_STRUCT` | Opaque | 4方向隣接（既存 `wall_connection.rs`） | 隣接壁変化時 |
| Furniture | `Z_BUILDING_FURNITURE` | Blend | なし | 家具配置時 |
| Light | `Z_BUILDING_LIGHT` | Blend | なし（半透明の光円） | 照明設置時 |
| Decal | `Z_BUILDING_DECAL` | Blend | なし | ゲームイベント時 |

### 3-4. 配線スプライトの接続描画

既存の `wall_connection.rs` と同一アーキテクチャで実装する。

```
配線スプライト選択ロジック:
  上下左右の wires セルを参照（WorldMap.wires[idx]）
    → isolated / horizontal / vertical
    → corner_ne / corner_nw / corner_se / corner_sw
    → t_junction × 4方向
    → cross
  → 合計 10 パターン（壁の 7 パターンより多い）

壁接続との違い:
  - 壁は不透明スプライト → Wire は半透明 (AlphaMode2d::Blend)
  - 壁はセル全体を占有 → Wire はセル内の細い線として描画
```

### 3-5. 照明エフェクト

照明は半透明の円形（または正方形）スプライトとして `Z_BUILDING_LIGHT` に配置する。将来的に「暗い環境」を実装する場合、全体に暗いオーバーレイを敷いた上で照明レイヤーを加算合成することで光源表現が可能になる。

```rust
// 照明 VisualLayer エンティティのスポーン例
commands.spawn((
    VisualLayer { kind: VisualLayerKind::Light { radius: 2.0, color: Color::srgba(1.0, 0.9, 0.3, 0.25) }, cell: (x, y) },
    Sprite {
        image: assets.light_glow,
        color: Color::srgba(1.0, 0.9, 0.3, 0.25),
        custom_size: Some(Vec2::splat(TILE_SIZE * 3.0)),  // 3タイル分の光の広がり
        ..default()
    },
    Transform::from_xyz(pos.x, pos.y, Z_BUILDING_LIGHT),
));
```

---

## 4. 空間データとの接続

`spatial-grid-architecture-plan` で提案した `WorldMap.pipes` / `WorldMap.wires` フィールドが追加された場合、Observer でビジュアルレイヤーの自動スポーン・削除を実現する。

```rust
// Observer の概念図（Bevy Observer / hook）
fn on_pipe_placed(
    trigger: Trigger<PipePlaced>,
    mut commands: Commands,
    world_map: WorldMapRead,
    assets: Res<GameAssets>,
) {
    let cell = trigger.event().cell;
    let pos = grid_to_world(cell.0, cell.1);

    // ビジュアルレイヤーをスポーン
    commands.spawn((
        VisualLayer { kind: VisualLayerKind::Pipe, cell },
        Sprite { image: assets.pipe_isolated, .. },
        Transform::from_xyz(pos.x, pos.y, Z_BUILDING_PIPE),
    ));

    // 隣接セルの Pipe VisualLayer の接続スプライトを更新
    update_adjacent_pipe_visuals(cell, &world_map, &mut commands);
}
```

---

## 5. TilemapChunk との関係

`tilemap-chunk-migration-plan` で予定している TilemapChunk 移行との役割分担:

| レイヤー | 描画方式 | 理由 |
|---------|---------|------|
| 地形（草・土・砂・川） | TilemapChunk | 静的・密・全セル存在 |
| フロア・壁ベース | TilemapChunk（別チャンク） | 静的、接続ロジックは CPU で事前計算してインデックス更新 |
| 配管・配線 | 個別 VisualLayer Entity | 動的追加削除・接続スプライト必要 |
| 家具 | 個別 VisualLayer Entity | セル単位で独立配置・削除 |
| 照明 | 個別 VisualLayer Entity | αブレンド・動的変化・可変サイズ |

壁の接続スプライト更新（`wall_connection.rs`）は、将来 TilemapChunk 採用後も CPU 側で接続パターンを計算して `TilemapChunkTileData` のインデックスを更新する方式に移行できる。

---

## 6. 段階的移行ロードマップ

### フェーズA: Z スロット定数の整備（前提・低コスト）

- **変更内容:** `render.rs` に `Z_BUILDING_*` 定数群を追加。既存の建築物スポーン箇所の Z 値を新定数に置き換える
- **変更ファイル:**
  - `crates/hw_core/src/constants/render.rs`
  - `src/systems/jobs/building_completion/spawn.rs`
  - `src/interface/selection/building_place/placement.rs`
- **完了条件:**
  - [ ] `Z_BUILDING_BASE`〜`Z_BUILDING_DECAL` が定数として定義されている
  - [ ] 既存建築物の Z 値が新定数を参照している
- **検証:** `cargo check`

---

### フェーズB: `VisualLayer` コンポーネントと基本 Relationship の実装

- **変更内容:** `VisualLayer` コンポーネントと `VisualLayerKind` を新規クレート or `hw_visual` に追加。完成建築物のスポーン時に `VisualLayer` を持つ子エンティティを生成するよう変更
- **変更ファイル:**
  - `crates/hw_visual/src/layer/mod.rs`（新規）
  - `src/systems/jobs/building_completion/spawn.rs`
- **完了条件:**
  - [ ] `VisualLayer { kind: Floor, cell }` エンティティが建築完了時にスポーンされる
  - [ ] 既存の建築物の見た目が変わらない
- **検証:** `cargo check` + 目視確認

---

### フェーズC: 配線ビジュアルレイヤーの実装

**前提:** `spatial-grid-architecture-plan` のフェーズC（ECS Relationship 統一）または `WorldMap.wires` フィールドの追加が完了していること

- **変更内容:** 配線配置時に `VisualLayerKind::Wire` エンティティをスポーンし、隣接配線を参照してスプライトを選択するシステムを追加
- **変更ファイル:**
  - `crates/hw_visual/src/layer/wire_connection.rs`（新規、`wall_connection.rs` を参考）
  - `src/systems/visual/wire_layer.rs`（新規）
- **完了条件:**
  - [ ] 配線が配置されると対応する `VisualLayer` がスポーンされる
  - [ ] 隣接する配線に応じてスプライトが切り替わる
  - [ ] 配線を削除すると `VisualLayer` も削除される
- **検証:** `cargo check` + 目視確認

---

### フェーズD: 照明ビジュアルレイヤーの実装

- **変更内容:** 照明設備配置時に `VisualLayerKind::Light` エンティティをスポーン
- **変更ファイル:**
  - `src/systems/visual/light_layer.rs`（新規）
  - `assets/textures/light_glow.png`（新規アセット）
- **完了条件:**
  - [ ] 照明設置時に半透明の光エフェクトスプライトがスポーンされる
  - [ ] 照明を削除するとエフェクトも消える
- **検証:** `cargo check` + 目視確認

---

## 7. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| VisualLayer エンティティが建築物削除時に孤立する | 不可視の幽霊エンティティが残る | Relationship の OnDespawn hook で連鎖削除する |
| Z スロットが既存の `Z_ITEM (0.1)` と重なる | アイテムと配線が同 Z で深度競合する | `Z_BUILDING_WIRE = 0.10` を `Z_ITEM` と同値にしないよう調整（0.11 にする等） |
| 照明スプライトのサイズが複数タイルにまたがる | 隣接セルの要素に視覚的に重なる | 照明の `custom_size` を `TILE_SIZE * 2.5` 以下に制限し、中心セルに基準を置く |
| フェーズC は `WorldMap.wires` 追加が前提 | 空間データが未実装の場合フェーズCが着手できない | フェーズA・Bのみ先行し、フェーズC・Dは空間データ計画と同時進行 |

---

## 8. AI引継ぎメモ（最重要）

### 現在地
- 進捗: `0%`（調査・提案のみ、実装未着手）

### 次のAIが最初にやること

1. `crates/hw_core/src/constants/render.rs` を読み、既存 Z 定数との競合がないか確認してからフェーズA の定数を追加する
2. `src/systems/jobs/building_completion/spawn.rs` を読み、現在の建築物スポーン時の Z 値を `Z_BUILDING_BASE` / `Z_BUILDING_STRUCT` に置き換える
3. `cargo check` でエラーがないことを確認してからフェーズB へ

### ブロッカー/注意点

- **フェーズC・D は空間データ計画との依存関係を確認すること:** `WorldMap.pipes` / `WorldMap.wires` が未実装の場合、Observer のトリガーイベントが存在しない
- **`wall_connection.rs` は変更しない:** 配線接続ロジック（`wire_connection.rs`）は壁接続を参考に新規作成する。既存の壁接続コードに手を加えない
- **TilemapChunk 移行との調整:** フロア・壁を TilemapChunk 化する場合、`Z_BUILDING_BASE` と `Z_BUILDING_STRUCT` はチャンクの `Transform.translation.z` として設定する

### 参照必須ファイル

- `crates/hw_core/src/constants/render.rs` — Z 定数定義
- `src/systems/jobs/building_completion/spawn.rs` — 建築物スポーン
- `crates/hw_visual/src/wall_connection.rs` — 配線接続ロジックの参考実装
- `src/systems/visual/floor_construction.rs` — フロアビジュアル更新の参考
- `crates/hw_jobs/src/model.rs` — Blueprint/Building コンポーネント

### Definition of Done

- [ ] `Z_BUILDING_*` 定数が `render.rs` に定義されている
- [ ] `VisualLayer` コンポーネントが `hw_visual` に存在する
- [ ] フロア・壁が `VisualLayer` エンティティとしてスポーンされる
- [ ] 削除時に孤立エンティティが残らない
- [ ] `cargo check` が成功

---

## 9. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-12` | `Claude` | 初版作成 |
