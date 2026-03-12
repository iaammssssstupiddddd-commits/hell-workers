# 建築物・配管・設備配置の空間グリッド根本最適化

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `spatial-grid-architecture-plan-2026-03-12` |
| ステータス | `Draft` |
| 作成日 | `2026-03-12` |
| 最終更新日 | `2026-03-12` |
| 作成者 | `Claude` |
| 関連提案 | N/A |
| 関連Issue/PR | N/A |

---

## 1. 目的

- **解決したい課題:**
  - `WorldMap` に建築物・ドア・ストックパイルの各 `HashMap<(i32,i32), Entity>` が個別に蓄積しており、配管・設備を追加するたびに肥大化する
  - 建築物の格納がECS（`Blueprint.occupied_grids`）と `WorldMap`（`buildings` HashMap）に二重化しており、同期漏れが潜在的なバグ源になっている
  - 配置バリデーションがマウス移動のたびに全占有グリッドを再チェックしており、建築物が増えると線形に遅くなる
  - 障害物 Vec（`obstacles`）がビルド完了・削除のたびに全再構築される

- **到達したい状態:**
  - 各システム（建築物・配管・配線など）が**独立したレイヤー**として空間データを持ち、同一セルに複数種別が共存できる
  - ECSのRelationship（本プロジェクトの標準手法）でエンティティとグリッド位置を接続し、二重保持を排除する
  - 配置バリデーションを差分ベースにして毎フレームの再計算を削減する

- **成功指標:**
  - 配管をフロアと同一セルに配置できる（レイヤーが独立しているため）
  - 新システム追加時に `WorldMap` の既存フィールドを変更せず新フィールドの追加のみで済む
  - 障害物クエリがビルド変更後も全再構築なしに最新状態を返す
  - `cargo check` が通る

---

## 2. 現状の問題構造

### 2-1. WorldMap の肥大化パターン

```rust
// 現状: 種別ごとに HashMap を追加し続ける構造
pub struct WorldMap {
    pub buildings:    HashMap<(i32, i32), Entity>,  // 建築物
    pub doors:        HashMap<(i32, i32), Entity>,  // ドア
    pub door_states:  HashMap<(i32, i32), DoorState>,
    pub stockpiles:   HashMap<(i32, i32), Entity>,  // ストックパイル
    pub bridged_tiles: HashSet<(i32, i32)>,         // 橋

    // 将来: pipes: HashMap<(i32,i32), Entity> を追加?
    // 将来: equipment: HashMap<(i32,i32), Entity> を追加?
    // → 際限なく増える
}
```

**根本原因:** システム種別ごとに HashMap を追加し続けており、将来の配管・配線追加時も同じパターンを繰り返すことになる。
ただし、この「種別ごとに独立したフィールドを持つ」構造自体は、**複数種別が同一セルに共存できる**という正しい性質を持っている。
問題は HashMap のデータ構造であり、フィールドを独立させる設計方針は維持すべきである。

### 2-2. ECS とのデータ二重保持

```
グリッド占有情報が2箇所に存在する:

  Blueprint.occupied_grids: Vec<(i32,i32)>   ← ECS コンポーネント
  WorldMap.buildings: HashMap<(i32,i32),Entity> ← リソース

  → ビルド完了時に両方を更新しなければならない
  → 削除時に片方を忘れると障害物データが腐る
```

### 2-3. バリデーションの毎フレーム全走査

```
マウス移動
  → occupied_grids の全セルをループ
    → has_building() → HashMap lookup × n
    → has_stockpile() → HashMap lookup × n
    → is_walkable() → Vec lookup × n
  → 建築物が密集するほど遅くなる（n はフットプリントのセル数）
  → さらに壁/フロア (最大10×10=100セル) では100回ループ
```

### 2-4. 障害物 Vec の全再構築

```
Building完了/削除
  → obstacles Vec を最初から全セル走査して再計算
  → マップが大きくなるほど O(MAP_WIDTH * MAP_HEIGHT) のコストが毎回発生
```

---

## 3. 根本最適化の方針

### 方針A: HashMap → 独立レイヤー Vec への変換（短期・高効果）

`HashMap<(i32,i32), T>` を種別ごとに独立した `Vec<Option<T>>` に置き換える。

**⚠️ OccupantKind による統合をしない理由:**
単一の `Vec<Option<OccupantEntry>>` に統合すると、1セルに1種別しか保持できなくなる。
配管はフロア建築物と同一セルに共存する必要があるため、**レイヤーは種別ごとに独立して並列に持つ**。

```rust
// 変更後: 種別ごとに独立した Vec（既存のフィールド分離を維持しつつ HashMap を Vec へ）
pub struct WorldMap {
    pub tiles:       Vec<TerrainType>,          // 既存（変更なし）
    pub buildings:   Vec<Option<Entity>>,       // HashMap → Vec（変更）
    pub door_states: Vec<Option<DoorState>>,    // HashMap → Vec（変更）
    pub stockpiles:  Vec<Option<Entity>>,       // HashMap → Vec（変更）
    pub obstacles:   Vec<bool>,                 // 既存（変更なし）

    // 将来: フィールドを追加するだけ。既存フィールドは無変更
    // pub pipes:  Vec<Option<Entity>>,   ← buildings と同一セル共存可
    // pub wires:  Vec<Option<Entity>>,   ← pipes と同一セル共存可
}
```

同一セルに複数種別が共存できる:
```
grids[idx]:
  buildings[idx]  = Some(floor_entity)   // フロアがある
  pipes[idx]      = Some(pipe_entity)    // 同じセルに配管もある  ← OccupantKind方式では不可能
  wires[idx]      = Some(wire_entity)    // さらに配線も共存可
  obstacles[idx]  = false               // 歩行可能（配管・配線は歩行を妨げない）
```

**効果:**
- ハッシュ計算ゼロ、`y*width+x` の整数演算のみでルックアップ
- 新システム追加 = 新フィールド追加のみ（既存フィールド・メソッドへの影響なし）
- `is_walkable()` は `obstacles` Vec のみ参照するため、新レイヤー追加時も変更不要
- `HashMap` のメモリオーバーヘッドが消える

---

### 方針B: ECS Relationship によるグリッド占有の一元化（中期・構造改善）

本プロジェクトはすでに ECS Relationship を主要なエンティティ接続手段として採用している。
グリッド占有もこれで表現することで、`Blueprint.occupied_grids` の Vec を廃止できる。

```
現状:
  Building Entity ──── Blueprint.occupied_grids: Vec<(i32,i32)>
  WorldMap.buildings: HashMap<(i32,i32), Entity>

提案後:
  Building Entity ──[Occupies]──→ GridCell Entity × n
  GridCell Entity が WorldMap.occupants[idx] と対応
```

**具体的には:**
- `GridCell` エンティティを各グリッドに対して1つ生成（または遅延生成）
- 建築物エンティティが `Occupies` Relationship で自身の占有グリッドを参照
- `WorldMap.occupants[idx]` には最上位レイヤーのエンティティを保存

**効果:**
- `Blueprint.occupied_grids` の Vec が不要になり二重保持が解消
- `Query<&Occupies, With<Building>>` で建築物→グリッドの逆引きが O(1)
- 建築物削除時に Relationship を Despawn するだけで WorldMap も自動同期（hook/observer で実現）

---

### 方針C: 差分ベース配置バリデーション（短期・UX改善）

```
現状: マウス移動 → 毎フレーム全セル再バリデーション

提案:
  前フレームの配置位置 == 今フレーム?
    Yes → キャッシュ結果を返す（再計算なし）
    No  → 変化したセルのみ差分チェック

  さらに: 近傍の建築物が変化した?（Changed<Building> で検知）
    No  → キャッシュ有効
    Yes → キャッシュ無効化 → 再計算
```

**実装:**

```rust
// hw_ui 側に追加
#[derive(Resource)]
struct PlacementValidationCache {
    last_anchor: Option<(i32, i32)>,
    last_kind: Option<BuildingKind>,
    result: PlacementResult,
    dirty: bool,  // 近傍 Building 変化時に true にする
}
```

**効果:**
- キャラクターがマウスを静止している間は計算ゼロ
- 建築物配置が密集してもコストが増加しない

---

### 方針D: 障害物の差分更新（中期・パスファインディング改善）

```
現状: Building完了 → obstacles Vec 全走査 O(10,000)

提案: 変化したグリッドのみ更新 O(フットプリント面積)

  fn update_obstacle(world_map: &mut WorldMap, x: i32, y: i32) {
      let idx = world_map.pos_to_idx(x, y);
      world_map.obstacles[idx] = compute_walkability(world_map, x, y);
  }
  // Blueprint.occupied_grids の各セルに対してのみ呼ぶ
```

**効果:**
- ビルド完了/削除コストが O(10,000) → O(フットプリント面積) に削減
- 10×10の壁でも 100 セルの更新で済む（現状は毎回10,000セル走査）

---

## 4. 配管・設備の将来拡張設計

### 配管の性質

配管は建築物と異なり以下の特徴を持つ:
- グリッド1セルを占有（建築物は複数セル可）
- **建築物（フロアなど）と同一セルに共存する** ← レイヤー独立方式でのみ実現可能
- 隣接した配管同士が「接続」して流体ネットワークを形成
- 流体の流れが方向性を持つ（入力端・出力端）

### 配管追加時の WorldMap 変更範囲

方針A（独立レイヤー方式）採用後は:

```rust
// WorldMap への変更はこの1行のみ
pub pipes: Vec<Option<Entity>>,

// 既存の buildings・obstacles・is_walkable() は一切変更不要
// 配管は歩行を妨げないため obstacles には影響しない
```

**配管固有のネットワーク情報**は別途 Resource で管理:
```rust
#[derive(Resource)]
pub struct PipeNetwork {
    // 隣接リストで流体グラフを表現（ECS Relationship でも可）
    connections: HashMap<Entity, smallvec::SmallVec<[Entity; 4]>>,
}
```

**配管バリデーション（方針Cの延長）:**
- `pipes[idx]` が `None` かどうかの1回チェックで配置可否が決まる
- 接続バリデーション（隣接セルの `pipes[idx]` 参照）も O(1) × 4方向
- `buildings[idx]` に値があっても配管は配置可能（共存が前提）

---

## 5. 段階的移行ロードマップ

### フェーズA: HashMap → 独立レイヤー Vec への変換（最優先・単独実施可能）

**変更方針:** アクセサメソッド（`has_building()`, `set_building()` 等）のシグネチャは変えず、
内部実装のみ `HashMap` → `Vec` に置き換える。呼び出し側のコードは無変更。

**変更ファイル:**
- `crates/hw_world/src/map/mod.rs` — `WorldMap` 構造体のフィールド変更（HashMap → Vec）
- `crates/hw_world/src/map/access.rs` — アクセサラッパーの内部実装更新

**推定難易度:** 低〜中（アクセサが整備されているため、呼び出し側の変更は原則不要）

---

### フェーズB: 障害物差分更新

**変更ファイル:**
- `crates/hw_world/src/map/mod.rs` — `rebuild_obstacles` を `update_obstacle_at` に置き換え
- `src/systems/jobs/building_completion/world_update.rs`

**推定難易度:** 低（局所的な変更）

---

### フェーズC: 配置バリデーションキャッシュ

**変更ファイル:**
- `crates/hw_ui/src/selection/placement.rs`
- `crates/hw_ui/src/selection/mod.rs`（リソース追加）

**推定難易度:** 低

---

### フェーズD: ECS Relationship による占有一元化

**変更ファイル:**
- `crates/hw_jobs/src/model.rs` — `Blueprint.occupied_grids` の廃止
- `crates/hw_world/src/map/mod.rs` — `buildings` HashMap をRelationshipベースに
- 関連する全システム

**推定難易度:** 高（全建築物システムへの影響大）
**前提:** フェーズA完了後に実施

---

## 6. 依存関係まとめ

```
フェーズA: HashMap → Vec 統合
    ↓ (独立実施可)
フェーズB: 障害物差分更新
フェーズC: バリデーションキャッシュ
    ↓ (A完了後に推奨)
フェーズD: ECS Relationship 統一
    ↓ (D完了後に自然に実現)
配管・設備の追加
```

フェーズA〜C は独立して実施可能。最も ROI が高いのはフェーズA（HashMapをVecに変えるだけで配置バリデーション・パスファインディング両方が高速化）。

---

## 7. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| HashMap → Vec 移行で `building_entries()` などのイテレータが変わる | コンパイルエラー | `Vec` のイテレータは `enumerate().filter_map(...)` で同等に書ける。呼び出し箇所を `grep` で事前確認する |
| Vec の初期化サイズが MAP_WIDTH/MAP_HEIGHT 定数に依存 | サイズ変更時に再確認が必要 | `WorldMap::default()` の初期化コードに `MAP_WIDTH * MAP_HEIGHT` を明示し、サイズ定数を一元管理する（現状も同様） |
| Relationship ベース移行でシステム実行順が変わる | 同期タイミングのバグ | フェーズD は既存の HashMap/Vec を残しつつ段階移行する |

---

## 8. AI引継ぎメモ（最重要）

### 現在地
- 進捗: `0%`（調査・提案のみ、実装未着手）

### 次のAIが最初にやること

1. `building_entries()` や `stockpile_entries()` など Vec への変換で影響が出るイテレータメソッドの呼び出し箇所を `grep -r "building_entries\|stockpile_entries" src/ crates/` で確認する
2. フェーズA: `WorldMap` の `buildings`, `door_states`, `stockpiles` フィールドを `Vec<Option<T>>` に変更し、アクセサメソッドの内部実装のみ書き換える（シグネチャは変えない）
3. `cargo check` でコンパイルが通ることを確認してからフェーズB・C へ

### 参照必須ファイル

- `crates/hw_world/src/map/mod.rs` — WorldMap 構造体
- `crates/hw_world/src/map/access.rs` — アクセサラッパー
- `crates/hw_jobs/src/model.rs` — Blueprint/Building コンポーネント
- `crates/hw_ui/src/selection/placement.rs` — バリデーションロジック
- `src/systems/jobs/building_completion/world_update.rs` — WorldMap 同期

### Definition of Done

- [ ] 配管がフロア建築物と同一セルに共存して配置できる
- [ ] 新システム追加時に既存フィールドへの変更なく新フィールド追加のみで済む
- [ ] `obstacles` の再構築が差分ベースになる
- [ ] `cargo check` が成功

---

## 9. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-12` | `Claude` | 初版作成 |
| `2026-03-12` | `Claude` | OccupantKind統合方式を廃止し独立レイヤーVec方式に修正（配管の建築物との共存要件を反映） |
