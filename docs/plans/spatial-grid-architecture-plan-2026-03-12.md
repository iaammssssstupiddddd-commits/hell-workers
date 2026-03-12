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
  - 建築物の格納がECS（`Blueprint.occupied_grids`）と `WorldMap`（`buildings` HashMap）に二重化しており、同期漏れが潜在的なバグ源になっている
  - 配置バリデーションがマウス移動のたびに全占有グリッドを再チェックしており、将来建築物が増えると問題になる
  - 障害物 Vec（`obstacles`）がビルド完了・削除のたびに全走査で再計算されている
  - 配管・配線など新システム追加時に、同一セルへの共存配置が現状の構造では設計しにくい

- **到達したい状態:**
  - ECSのRelationship（本プロジェクトの標準手法）でエンティティとグリッド位置を接続し、二重保持を排除する
  - 配置バリデーションを差分ベースにして毎フレームの再計算を削減する
  - 障害物を変化セルのみ差分更新する
  - 新システム（配管・配線）は既存フィールドと独立したフィールドとして追加し、同一セル共存を実現する

- **成功指標:**
  - `Blueprint.occupied_grids` と `WorldMap.buildings` の二重保持が解消される
  - 障害物クエリがビルド変更後も全走査なしに最新状態を返す
  - 配管をフロアと同一セルに配置できる
  - `cargo check` が通る

---

## 2. 現状の問題構造

### 2-1. ECS とのデータ二重保持

```
グリッド占有情報が2箇所に存在する:

  Blueprint.occupied_grids: Vec<(i32,i32)>     ← ECS コンポーネント
  WorldMap.buildings: HashMap<(i32,i32),Entity> ← リソース

  → ビルド完了時に両方を更新しなければならない
  → 削除時に片方を忘れると障害物データが腐る
```

### 2-2. バリデーションの毎フレーム全走査

```
マウス移動
  → occupied_grids の全セルをループ
    → has_building() × n
    → has_stockpile() × n
    → is_walkable() × n
  → 壁/フロア (最大10×10=100セル) では100回ループ
  → 建築物が密集しても、静止中も、毎フレーム再計算
```

### 2-3. 障害物 Vec の全走査再計算

```
Building完了/削除
  → obstacles Vec を最初から全セル走査して再計算
  → O(MAP_WIDTH * MAP_HEIGHT) = O(10,000) が毎回発生
  → 実際には変化するのはフットプリントの数セルのみ
```

### 2-4. 新システム追加時の共存問題

現状の `buildings: HashMap<(i32,i32), Entity>` は1セルに1エンティティしか保持できない。
配管をフロアと同一セルに置く設計にするには、独立したフィールドとして並列に持つ必要がある。

```rust
// 現状: 配管を追加しようとすると buildings と競合する
world_map.buildings.get(&(x, y))  // → Some(floor_entity) で埋まっている
// 配管を別フィールドで持てば競合しない
world_map.pipes.get(&(x, y))      // → 独立して参照可能
```

---

## 3. 最適化の方針

### 方針A: ECS Relationship によるグリッド占有の一元化（中期・構造改善）

本プロジェクトはすでに ECS Relationship を主要なエンティティ接続手段として採用している。
グリッド占有もこれで表現することで、`Blueprint.occupied_grids` の Vec を廃止できる。

```
現状:
  Building Entity ──── Blueprint.occupied_grids: Vec<(i32,i32)>
  WorldMap.buildings: HashMap<(i32,i32), Entity>  ← 二重保持

提案後:
  Building Entity ──[Occupies]──→ GridCell Entity × n
  WorldMap はグリッド→エンティティの索引としてのみ機能
```

**効果:**
- `Blueprint.occupied_grids` の Vec が不要になり二重保持が解消
- 建築物削除時に Relationship を Despawn するだけで WorldMap も自動同期（Observer で実現）
- ECS の変更検知（`Added<Occupies>`, `RemovedComponents<Occupies>`）でシステムが反応できる

---

### 方針B: 差分ベース配置バリデーション（短期・UX改善）

```
現状: マウス移動 → 毎フレーム全セル再バリデーション

提案:
  前フレームの配置位置 == 今フレーム? → キャッシュ結果を返す（再計算なし）
  近傍の建築物が変化した?（Changed<Building> で検知） → キャッシュ無効化 → 再計算
```

**実装:**

```rust
#[derive(Resource)]
struct PlacementValidationCache {
    last_anchor: Option<(i32, i32)>,
    last_kind: Option<BuildingKind>,
    result: PlacementResult,
    dirty: bool,
}
```

**効果:**
- マウス静止中は計算ゼロ
- 建築物が密集してもコストが増加しない

---

### 方針C: 障害物の差分更新（短期・確実な改善）

```
現状: Building完了 → obstacles Vec 全走査 O(10,000)

提案: 変化したグリッドのみ更新 O(フットプリント面積)
```

**実装:**

```rust
// 既存の add_obstacle() / remove_obstacle() を呼ぶだけで済む（すでに実装済み）
// 問題は「全再構築」を呼んでいる箇所をこれらに置き換えること

// building_completion/world_update.rs で:
for &grid in &bp.occupied_grids {
    world_map.add_obstacle(grid.0, grid.1);  // 全再構築の代わり
}
```

**効果:**
- ビルド完了/削除コストが O(10,000) → O(フットプリント面積) に削減

---

### 方針D: 新システムの独立フィールドとして追加（将来・設計ガイドライン）

配管・配線など新システムを追加する際のガイドライン:

```rust
// 既存フィールドとは独立したフィールドを追加する（同一セル共存のため）
// データ構造は既存パターン（HashMap）に合わせる
pub pipes: HashMap<(i32, i32), Entity>,   // buildings と独立 → 同一セル共存可
pub wires: HashMap<(i32, i32), Entity>,   // pipes と独立   → 同一セル共存可
```

歩行可否への影響:
- 配管・配線は歩行を妨げない → `obstacles` は変更不要
- 歩行を妨げる設備（大型機器など）→ `add_obstacle()` を呼ぶだけ

配管ネットワーク固有の情報は別途 Resource で管理:

```rust
#[derive(Resource)]
pub struct PipeNetwork {
    connections: HashMap<Entity, Vec<Entity>>,
}
```

---

## 4. 段階的移行ロードマップ

### フェーズA: 障害物の差分更新（最優先・低リスク）

**変更方針:** `rebuild_obstacles()` 相当の全走査呼び出しを、
既存の `add_obstacle()` / `remove_obstacle()` の個別呼び出しに置き換える。

**変更ファイル:**
- `src/systems/jobs/building_completion/world_update.rs`

**推定難易度:** 低

---

### フェーズB: 配置バリデーションキャッシュ

**変更ファイル:**
- `crates/hw_ui/src/selection/placement.rs`
- `crates/hw_ui/src/selection/mod.rs`（キャッシュ Resource 追加）

**推定難易度:** 低

---

### フェーズC: ECS Relationship による占有一元化

**変更ファイル:**
- `crates/hw_jobs/src/model.rs` — `Blueprint.occupied_grids` の廃止
- `crates/hw_world/src/map/mod.rs` — WorldMap 同期を Observer に移行
- 関連する全システム

**推定難易度:** 高（全建築物システムへの影響大）

---

## 5. 依存関係まとめ

```
フェーズA: 障害物差分更新      ← 独立実施可・今すぐ効果あり
フェーズB: バリデーションキャッシュ ← 独立実施可・今すぐ効果あり
     ↓
フェーズC: ECS Relationship 統一 ← A/B 完了後に推奨
     ↓
方針D: 配管・配線の追加         ← C 完了後が理想、C 前でも追加自体は可能
```

---

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 障害物の全走査再構築を呼んでいる箇所が複数ある | 差分更新への移行漏れ | `grep -r "rebuild_obstacles\|obstacles.*iter\|obstacles.*fill" src/ crates/` で全箇所確認 |
| Relationship ベース移行でシステム実行順が変わる | 同期タイミングのバグ | フェーズC は既存の HashMap を残しつつ段階移行する |
| 配管追加時に is_walkable() の変更が必要になる | 配管が障害物扱いになる | 配管は `add_obstacle()` を呼ばない設計を守る |

---

## 7. AI引継ぎメモ（最重要）

### 現在地
- 進捗: `0%`（調査・提案のみ、実装未着手）

### 次のAIが最初にやること

1. `grep -r "rebuild_obstacles\|obstacles\.fill\|obstacles\.iter_mut" src/ crates/` で障害物全走査箇所を確認
2. フェーズA: `building_completion/world_update.rs` の全走査を `add_obstacle()` 個別呼び出しに置き換え
3. `cargo check` 確認後、フェーズB へ

### 参照必須ファイル

- `crates/hw_world/src/map/mod.rs` — WorldMap 構造体・`add_obstacle()`/`remove_obstacle()` 実装
- `crates/hw_jobs/src/model.rs` — Blueprint/Building コンポーネント
- `crates/hw_ui/src/selection/placement.rs` — バリデーションロジック
- `src/systems/jobs/building_completion/world_update.rs` — WorldMap 同期

### Definition of Done

- [ ] `Blueprint.occupied_grids` と `WorldMap.buildings` の二重保持が解消（フェーズC）
- [ ] `obstacles` の更新が差分ベースになる（フェーズA）
- [ ] マウス静止中の配置バリデーション計算がゼロになる（フェーズB）
- [ ] `cargo check` が成功

---

## 8. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-12` | `Claude` | 初版作成 |
| `2026-03-12` | `Claude` | OccupantKind統合方式を廃止し独立レイヤーVec方式に修正 |
| `2026-03-12` | `Claude` | HashMap→Vec変換を削除。根拠のない変換を排除し、実効性のある3方針に絞り込み |
