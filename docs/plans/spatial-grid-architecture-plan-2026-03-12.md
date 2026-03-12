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
  - 「何がどのグリッドに存在するか」を**単一の空間レイヤー**で管理し、配管・設備追加時に `WorldMap` を改変不要にする
  - ECSのRelationship（本プロジェクトの標準手法）でエンティティとグリッド位置を接続し、二重保持を排除する
  - 配置バリデーションを差分ベースにして毎フレームの再計算を削減する

- **成功指標:**
  - `WorldMap` に新規建築物種別を追加する際、構造体の変更が不要になる
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

**根本原因:** 「グリッドに何かが存在する」という概念が、種別ごとに別々のデータ構造として表現されている。

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

### 方針A: HashMap → 平坦 Vec への統一（短期・高効果）

`HashMap<(i32,i32), T>` を `Vec<Option<T>>` に置き換える。

**理由:**
- 100×100 = 10,000 セルは密なグリッド → HashMap のハッシュ計算コストが無駄
- `Vec` はキャッシュコヒーレントで L1/L2 ヒット率が高い
- インデックスは既存の `pos_to_idx(x, y)` が使える（既実装）

```rust
// 変更後
pub struct WorldMap {
    // 建築物・ドア・ストックパイルを1本の Vec に統合
    pub occupants: Vec<Option<OccupantEntry>>,  // index = y*width + x
    pub door_states: Vec<Option<DoorState>>,    // 同上
    pub obstacles:   Vec<bool>,                 // 既存（変更なし）
}

pub struct OccupantEntry {
    pub entity: Entity,
    pub kind: OccupantKind,
}

pub enum OccupantKind {
    Building,
    Door,
    Stockpile,
    Bridge,
    // 将来: Pipe, Equipment など追加のみ（WorldMap 構造体変更不要）
}
```

**効果:**
- グリッドに「何があるか」を1回のインデックスアクセスで取得
- 将来の配管・設備は `OccupantKind` に追加するだけ
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
- 隣接した配管同士が「接続」して流体ネットワークを形成
- 流体の流れが方向性を持つ（入力端・出力端）

### 設備の性質

設備（ポンプ、バルブ、タンクなど）:
- 配管ネットワーク上のノード
- 建築物と配管の中間的な存在（固定グリッド + 接続ポート）

### 拡張設計（方針A/B の前提）

```rust
// OccupantKind への追加のみで WorldMap 構造体変更不要
pub enum OccupantKind {
    Building,
    Door,
    Stockpile,
    Bridge,
    Pipe { direction: PipeDirection },      // 新規
    Equipment { kind: EquipmentKind },      // 新規
}

// 配管ネットワークは別途 Resource で管理
#[derive(Resource)]
pub struct PipeNetwork {
    // 隣接リストで流体グラフを表現
    connections: HashMap<Entity, Vec<Entity>>,
    // 方向B 採用時は GridCell Relationship で自動構築
}
```

**配管バリデーション（方針Cの延長）:**
- 配管は常に1セルなのでフットプリント計算が不要
- `OccupantKind` チェック1回で配置可否が決まる
- 接続バリデーション（隣接する配管/設備の確認）も `occupants Vec` から O(1) × 4方向

---

## 5. 段階的移行ロードマップ

### フェーズA: HashMap → Vec 統合（最優先・単独実施可能）

**変更ファイル:**
- `crates/hw_world/src/map/mod.rs` — `WorldMap` 構造体の変更
- `crates/hw_world/src/map/access.rs` — アクセサメソッドの更新
- `src/systems/jobs/building_completion/world_update.rs` — 更新箇所の修正
- `crates/hw_ui/src/selection/placement.rs` — バリデーション呼び出し修正

**推定難易度:** 中（データ構造の変更だが、インターフェース互換を保てる）

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
| `occupants Vec` への移行で既存の HashMap 参照が多数ある | コンパイルエラーが大量発生 | `has_building()` などのアクセサメソッドを維持して内部実装だけ変える |
| Relationship ベース移行でシステム実行順が変わる | 同期タイミングのバグ | フェーズD は既存の HashMap/Vec を残しつつ段階移行する |
| 配管追加時に `OccupantKind` マッチが exhaustive でなくなる | コンパイルエラー | Rust のパターンマッチが網羅性を強制するため、追加漏れはコンパイル時に検出できる |

---

## 8. AI引継ぎメモ（最重要）

### 現在地
- 進捗: `0%`（調査・提案のみ、実装未着手）

### 次のAIが最初にやること

1. `crates/hw_world/src/map/mod.rs` の `WorldMap` 構造体を読み、各 HashMap の利用箇所を `grep -r "world_map.buildings\|world_map.doors\|world_map.stockpiles" src/ crates/` で全列挙
2. フェーズA の `OccupantEntry` 型と `occupants: Vec<Option<OccupantEntry>>` を定義し、`has_building()` 等のアクセサの内部実装だけを置き換える
3. `cargo check` でコンパイルが通ることを確認してからフェーズB へ

### 参照必須ファイル

- `crates/hw_world/src/map/mod.rs` — WorldMap 構造体
- `crates/hw_world/src/map/access.rs` — アクセサラッパー
- `crates/hw_jobs/src/model.rs` — Blueprint/Building コンポーネント
- `crates/hw_ui/src/selection/placement.rs` — バリデーションロジック
- `src/systems/jobs/building_completion/world_update.rs` — WorldMap 同期

### Definition of Done

- [ ] `WorldMap` に新種別を追加する際に構造体変更が不要になる
- [ ] `obstacles` の再構築が差分ベースになる
- [ ] `cargo check` が成功

---

## 9. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-12` | `Claude` | 初版作成 |
