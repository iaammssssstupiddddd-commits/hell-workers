# 建築物・配管・設備配置の空間グリッド根本最適化 (対案: 1D Array Architecture)

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
  - 空間データ（`buildings`, `obstacles` など）が `HashMap<(i32, i32), Entity>` や動的な `Vec` で管理されており、ハッシュ計算のオーバーヘッドやメモリ断片化によるキャッシュミスがパフォーマンスのボトルネックになっている。
  - 建築物の格納がECS（`Blueprint.occupied_grids`）と `WorldMap`（`buildings`）に二重化している。
  - 配置バリデーションがマウス移動のたびに重い処理（ハッシュルックアップ等）を走らせている。
  - 障害物 `obstacles` が全走査で再計算されており、無駄な負荷がかかっている。
  - 新システム（配管・配線）追加時に共存配置がしにくい。

- **到達したい状態:**
  - `WorldMap` の空間データを `HashMap` から **フラットな1次元配列 (1D Array/Vec)** に完全移行し、真の `O(1)` ルックアップを実現する。
  - バリデーションはキャッシュを持たず、ナノ秒レベルの配列直読みによって毎フレーム計算をゼロコスト化する。
  - 空間インデックスの正本（Single Source of Truth）を `WorldMap` の 1D Array に集約し、ECSエンティティとの不要な二重管理（GridCell Entity等）を避ける。
  - 障害物を変化セルのみ差分更新する。
  - 新システム（配管・配線）は独立した 1D Array として追加し、同一セル共存を実現する。

- **成功指標:**
  - `WorldMap.buildings` 等が `Vec<Option<Entity>>` や `Vec<bool>` に移行されている。
  - マウス移動時のバリデーション負荷が計測不能なレベルまで低下する。
  - 障害物クエリが差分更新により高速化する。
  - `cargo check` が通る。

---

## 2. 現状の問題構造

### 2-1. HashMap による空間ルックアップの限界
現在 `WorldMap.buildings: HashMap<(i32,i32), Entity>` が使われていると推測されるが、マップサイズが固定である場合、HashMapの使用はハッシュ計算とメモリの非連続性によりパフォーマンス上のペナルティが大きい。

### 2-2. バリデーションの重さとキャッシュの罠
マウス移動のたびに `HashMap` を複数回引くため負荷が高く、以前の提案では「キャッシュ機構」を導入しようとしていた。しかしキャッシュは無効化（Dirtyフラグ）の管理が複雑になりバグの温床となる。

### 2-3. ECS GridCell Entity 構想の矛盾
以前の提案（フェーズC）では「GridCell Entityを作り、Relationshipを結ぶ」としていたが、これは `tilemap-chunk-migration-plan` による「基本地形エンティティの削減（10,000→1）」と真っ向から矛盾し、再び大量の目に見えないエンティティをスポーンすることになってしまう。

---

## 3. 最適化の方針 (対案)

### 方針A: 1次元配列(1D Vec)による真の O(1) 空間インデックス化と HashMap の廃止 (最重要)

空間のマス（セル）をEntityにするのではなく、**空間インデックスはECSから切り離してResource(`WorldMap`)の1D Arrayに任せきる**。

```rust
// 改善案 (crates/hw_world/src/map/mod.rs 相当)
pub struct WorldMap {
    pub width: i32,
    pub height: i32,
    // (x, y) へのアクセスは index = (y * width + x) で一発 O(1)
    pub buildings: Vec<Option<Entity>>,
    pub pipes: Vec<Option<Entity>>,
    pub is_walkable: Vec<bool>, // obstaclesの代わりとして高速アクセス可能に
    pub is_buildable_terrain: Vec<bool>, // 地形が建築可能かどうかのフラグ
}

impl WorldMap {
    #[inline]
    pub fn get_index(&self, x: i32, y: i32) -> usize {
        (y * self.width + x) as usize
    }
}
```

**効果:** 
- ハッシュ計算が消滅しルックアップが真の O(1) になる。
- 配列がメモリ上で連続しているためCPUキャッシュヒット率が劇的に向上する。

---

### 方針B: 1D Vec 参照による超高速バリデーション (キャッシュ機構の廃止)

バリデーションキャッシュ用の構造体（`PlacementValidationCache`）を廃止し、**配列の直読みによる力技（だが最速）の毎フレーム計算**を行う。

```rust
// マウス移動時のバリデーションループ
let mut can_build = true;
for offset in blueprint.footprint() {
    let index = world_map.get_index(anchor_x + offset.x, anchor_y + offset.y);
    if world_map.buildings[index].is_some() || !world_map.is_buildable_terrain[index] {
        can_build = false;
        break;
    }
}
```

**効果:**
- キャッシュ無効化バグの懸念がなくなり、コードが極めてシンプルになる。
- 1D Arrayのルックアップはナノ秒単位のため、毎フレーム計算しても全く負荷にならない。

---

### 方針C: 障害物の差分更新（短期・確実な改善）

これは以前の提案を維持。
現状: Building完了 → obstacles Vec 全走査 O(10,000)
提案: 変化したグリッド（配列の該当インデックス）のみフラグを更新（O(フットプリント面積)）。

**実装:**
```rust
for &grid in &bp.occupied_grids {
    let idx = world_map.get_index(grid.0, grid.1);
    world_map.is_walkable[idx] = false; // add_obstacle() の代替
}
```

---

### 方針D: 新システムの独立配列として追加（将来・設計ガイドライン）

配管・配線などは、同一の1D Arrayサイズで別のフィールドとして定義する。

```rust
pub pipes: Vec<Option<Entity>>,   // buildings と独立 → 同一セル共存可
pub wires: Vec<Option<Entity>>,   // pipes と独立   → 同一セル共存可
```

---

## 4. 段階的移行ロードマップ

### フェーズA: 障害物の差分更新と is_walkable 配列化（最優先・低リスク）
- **変更方針:** `obstacles` や `rebuild_obstacles()` を廃止し、`is_walkable: Vec<bool>` に移行。建築完了/削除時は該当インデックスの bool だけ反転させる。
- **変更ファイル:**
  - `crates/hw_world/src/map/mod.rs`
  - `crates/bevy_app/src/systems/jobs/building_completion/world_update.rs`

### フェーズB: WorldMap.buildings の HashMap から 1D Vec への移行
- **変更内容:** `buildings: HashMap<(i32,i32), Entity>` を `buildings: Vec<Option<Entity>>` に変更。
- **変更ファイル:**
  - `crates/hw_world/src/map/mod.rs`
  - 各種参照箇所

### フェーズC: バリデーションの直接参照化（キャッシュ不要化）
- **変更内容:** `PlacementValidationCache` のような仕組みを取り入れず、毎フレーム `WorldMap` の Vec を引く形にリファクタ。
- **変更ファイル:**
  - `crates/hw_ui/src/selection/placement.rs` 等

---

## 5. 依存関係まとめ
```
フェーズA: is_walkable の導入と差分更新  ← 今すぐ可能
     ↓
フェーズB: HashMap 廃止 (1D Array化)     ← A の後推奨
     ↓
フェーズC: 超高速バリデーション         ← B 完了後に自然と達成される
     ↓
方針D: 配管・配線の追加                 ← B 以降ならいつでも可能
```

---

## 6. リスクと対策
| リスク | 影響 | 対策 |
| --- | --- | --- |
| 大量の `HashMap` 依存コードがある場合のリファクタ量 | 移行時のコンパイルエラー多数 | `Map` のアクセサメソッド（`get_building(x,y)`等）を先に作り、内部実装だけをVecに差し替えることで影響を最小化 |
| マップサイズ可変への対応 | Vecサイズ再確保の手間 | 本プロジェクトは固定サイズ前提のため問題なし |

---

## 7. AI引継ぎメモ（最重要）

### 次のAIが最初にやること
1. `crates/hw_world/src/map/mod.rs` において、`WorldMap` の `buildings` や `obstacles` の現在の定義を確認する。
2. それらを `Vec<Option<Entity>>` や `Vec<bool>` で置き換える PR / 実装に着手する。
3. `crates/bevy_app/src/systems/jobs/building_completion/world_update.rs` などの `obstacles` 全再構築箇所を差分更新に修正する。

### Definition of Done
- [ ] `WorldMap` から `HashMap` による空間管理が排除され、1D Array に移行している。
- [ ] 毎フレームの配置バリデーションがキャッシュなしで軽量に動作している。
- [ ] 障害物の全再計算処理が消滅し、差分更新になっている。
- [ ] `cargo check` が成功する。
