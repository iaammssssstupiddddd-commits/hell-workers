# Think フェーズのイテレーション最適化

## メタ情報

| 項目 | 値 |
| --- | --- |
| ドキュメントID | `think-phase-iteration-optimization-proposal-2026-03-07` |
| ステータス | `Draft` |
| 作成日 | `2026-03-07` |
| 最終更新日 | `2026-03-07` |
| 作成者 | `AI (Claude)` |
| 関連計画 | `TBD` |
| 関連Issue/PR | `N/A` |

## 1. 背景と問題

タスク割り当て（Think/Decide フェーズ）で、2つの O(n) イテレーションがボトルネック候補として特定された:

### 問題A: タイル親サイトの O(n) スキャン

`demand.rs` の `compute_remaining_with_incoming` / `compute_remaining_wall_with_incoming` が **全 FloorTileBlueprint / WallTileBlueprint をイテレート** して `parent_site == anchor_entity` でフィルタしている。

```rust
// demand.rs:192-196 (現状)
for tile in queries.storage.floor_tiles.iter()
    .filter(|tile| tile.parent_site == anchor_entity)
{ ... }
```

- 20 サイト × 1000 タイル = 毎委譲サイクル（0.5秒）最大 20,000 回のイテレーション
- `dropping.rs` と `unloading.rs` でも同一パターンが使われている（実行時は Soul 単位なので影響は小さいが、存在する）

### 問題B: IncomingDeliveries カウントの O(m) ルックアップ

`count_matching_incoming_deliveries` が搬入先ごとに全配送中アイテムをイテレートし、各アイテムに対して ECS `get()` を呼ぶ。

```rust
// demand.rs:246-258 (現状)
incoming.iter()
    .filter(|&&item| {
        queries.reservation.resources.get(item)  // ← ECS query per item
            .is_ok_and(|resource_item| predicate(resource_item.0))
    })
    .count()
```

- Blueprint / FloorSite / WallSite / Stockpile / ProvisionalWall の各搬入先で呼ばれる
- 100+ の配送中アイテム × 20+ 搬入先 = 数千回の ECS ルックアップ/サイクル

## 2. 目的（Goals）

- A: タイル→サイトの逆引きインデックスを導入し、`parent_site` フィルタの O(n) スキャンを O(1) ルックアップに置換
- B: `IncomingDeliveries` のリソース種別カウントをキャッシュし、`count_matching_incoming_deliveries` の ECS ルックアップを削減

## 3. 非目的（Non-Goals）

- 空間グリッドの構造変更（既に効率的に動作している）
- SharedResourceCache の再構築間隔の変更（0.2秒は適切）
- 実行時（Soul ごと）のタイルイテレーション最適化（Soul 単位なので影響小、提案 001 の共通化で十分）

## 4. 提案内容（概要）

- 一言要約: 2つのデータ構造（タイルインデックス + 配送カウントキャッシュ）を追加して Think フェーズの計算量を削減
- 期待される効果:
  - A: 委譲サイクルのタイルイテレーション 20,000 → 数百（サイトあたりのタイル数のみ）
  - B: ECS ルックアップ 数千/サイクル → 数十/サイクル

## 5. 詳細設計

### 5.1 改善A: タイル→サイト逆引きインデックス

**方針**: `Resource` として `HashMap<Entity, Vec<Entity>>` を持ち、FloorTileBlueprint / WallTileBlueprint の追加・削除時に Change Detection で同期する。

```rust
// src/systems/logistics/tile_index.rs (新規)
#[derive(Resource, Default)]
pub struct TileSiteIndex {
    pub floor: HashMap<Entity, Vec<Entity>>,  // site_entity -> [tile_entity]
    pub wall: HashMap<Entity, Vec<Entity>>,   // site_entity -> [tile_entity]
}
```

**同期システム**:
- `Added<FloorTileBlueprint>` で `floor[parent_site].push(entity)`
- `RemovedComponents<FloorTileBlueprint>` で除去
- WallTileBlueprint も同様
- `GameSystemSet::Spatial` に配置（他の空間グリッドと同じフェーズ）

**利用側の変更**:
```rust
// demand.rs (改善後)
let tiles = tile_index.floor.get(&anchor_entity).unwrap_or(&EMPTY);
for &tile_entity in tiles {
    let tile = queries.storage.floor_tiles.get(tile_entity)?;
    needed += needed_per_tile(tile);
}
```

### 5.2 改善B: IncomingDeliveries リソース種別キャッシュ

**方針**: 委譲サイクルの開始時に1回だけ全搬入先の `IncomingDeliveries` を走査し、`(destination, ResourceType) -> count` のマップを構築する。サイクル内ではこのマップを O(1) 参照する。

```rust
// src/systems/familiar_ai/decide/task_management/mod.rs (または demand.rs 内)
pub struct IncomingDeliverySnapshot {
    /// (destination_entity, resource_type) -> count
    by_dest_resource: HashMap<(Entity, ResourceType), u32>,
    /// destination_entity -> total count (型を問わず)
    by_dest_total: HashMap<Entity, u32>,
}
```

**構築タイミング**: `familiar_task_delegation_system` の冒頭（0.5秒間隔のタイマーゲート通過後、候補収集前）で1回構築。

**走査対象**: `Query<(Entity, &IncomingDeliveries)>` を1回イテレートし、各 delivery のリソース型を `resources` クエリで取得してカウント。全搬入先 × 全アイテムの走査は1回で済む。

**利用側の変更**:
```rust
// demand.rs (改善後)
fn count_exact_incoming_deliveries(
    target: Entity,
    resource_type: ResourceType,
    snapshot: &IncomingDeliverySnapshot,
) -> u32 {
    snapshot.by_dest_resource.get(&(target, resource_type)).copied().unwrap_or(0)
}
```

### 5.2 変更対象（想定）

**新規作成:**
- `src/systems/logistics/tile_index.rs` — `TileSiteIndex` Resource + 同期システム

**変更:**
- `src/systems/logistics/mod.rs` — `pub mod tile_index` 追加、プラグイン登録
- `src/systems/familiar_ai/decide/task_management/policy/haul/demand.rs` — タイルインデックス参照 + スナップショット参照に切替
- `src/systems/familiar_ai/decide/task_management/task_assigner.rs` — スナップショット構築の呼び出し追加
- `src/systems/familiar_ai/decide/task_management/mod.rs` — `IncomingDeliverySnapshot` 型定義

### 5.3 データ/コンポーネント/API 変更

- 追加: `TileSiteIndex`（Resource）、`IncomingDeliverySnapshot`（フレームローカル構造体）
- 変更: `demand.rs` の関数シグネチャ（追加引数）
- 削除: `demand.rs` の `count_matching_incoming_deliveries`（スナップショットで置換）

## 6. 代替案と比較

| 案 | 採否 | 理由 |
| --- | --- | --- |
| A1: `TileSiteIndex` Resource（本提案） | 採用 | Change Detection で差分更新。全グリッドと同じパターン |
| A2: `FloorTileBlueprint` に `Children` Relationship を使う | 不採用 | Bevy 0.18 の `Children` は 1 親のみで、既に他の用途で使用されている可能性がある。明示的インデックスの方が安全 |
| A3: `parent_site` フィールドに Relationship を使う | 不採用 | `FloorTileBlueprint` は `Component` であり Relationship 化すると構造変更が大きい |
| B1: 委譲サイクル冒頭のスナップショット（本提案） | 採用 | 構築コスト O(全配送数) は1回で済む。参照は O(1) |
| B2: `TransportDemand` にカウントを持たせる | 不採用 | `IncomingDeliveries` は Relationship の自動更新であり、変更検知でカウントを同期するのは複雑 |

## 7. 影響範囲

- ゲーム挙動: 変更なし（計算結果は同一）
- パフォーマンス: Think フェーズの CPU 負荷を推定 10-20% 削減（サイト数・タイル数・配送中アイテム数に比例して効果増大）
- UI/UX: 変更なし
- セーブ互換: 影響なし
- 既存ドキュメント更新: `docs/architecture.md` の空間グリッド一覧に `TileSiteIndex` を追記

## 8. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `TileSiteIndex` と実際のタイルの不整合 | タイル需要の計算結果が不正確になる | `RemovedComponents` で確実にクリーンアップ。デバッグビルドで整合性アサーションを追加 |
| スナップショットが古い（0.5秒前の状態） | 割り当て時に搬入済みアイテムを二重カウント | 現行の `count_matching_incoming_deliveries` も同じデータを見ているため、鮮度は同一。`ReservationShadow` がフレーム内差分を補完 |
| `TileSiteIndex` の同期システムが他の空間グリッドと実行順で競合 | `Added` 検知が1フレーム遅れる | `Spatial` セットに配置すれば `Logic` フェーズ前に完了。既存グリッドと同じ保証 |

## 9. 検証計画

- `cargo check`
- 手動確認シナリオ:
  - FloorConstructionSite を作成 → タイルインデックスが正しく構築されること（デバッグログ）
  - サイト削除 → インデックスからエントリが除去されること
  - 多数のサイト（10+）を同時建設 → フレームレートが改善方向であること
- 計測/ログ確認:
  - 改善前後で `familiar_task_delegation_system` の実行時間を `Instant::now()` で計測
  - `TileSiteIndex` のエントリ数をデバッグログに出力

## 10. ロールアウト/ロールバック

- 導入手順:
  1. `TileSiteIndex` Resource + 同期システムを追加（この時点では使わない）
  2. `demand.rs` をインデックス参照に切替
  3. `IncomingDeliverySnapshot` を追加し、`count_matching_incoming_deliveries` を置換
- 段階導入の有無: あり（A と B は独立に導入可能）
- 問題発生時の戻し方: 各段階で git revert 可能

## 11. 未解決事項（Open Questions）

- [ ] `TileSiteIndex` を `Resource` にするか、`SystemParam` にするか。`Resource` の方が他システムからもアクセスしやすい。
- [ ] `IncomingDeliverySnapshot` のライフタイムをどうするか。`Local<IncomingDeliverySnapshot>` にしてシステム引数として受け取るか、`task_assigner` 内のローカル変数として構築→参照渡しするか。
- [ ] 提案 001（搬入先バリデーション一元化）との実施順序。001 を先に実施してから 002 のインデックスを適用する方が、共通関数のシグネチャを `impl Iterator` で設計しやすい。

## 12. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 直近で完了したこと: 提案書の作成
- 現在のブランチ/前提: `master`

### 次のAIが最初にやること

1. 提案 001 が完了していることを確認
2. `src/systems/logistics/tile_index.rs` を作成し、`Added` / `RemovedComponents` の同期システムを実装
3. `demand.rs` のタイルイテレーションをインデックス参照に置換し、`cargo check`
4. `IncomingDeliverySnapshot` を実装し、`demand.rs` の `count_matching_incoming_deliveries` を置換

### ブロッカー/注意点

- 提案 001（搬入先バリデーション一元化）を先に実施することを推奨。001 が完了していれば、タイルイテレーションの変更箇所が `logistics/floor_construction.rs` と `logistics/wall_construction.rs` の2箇所に限定される。
- `FloorTileBlueprint` / `WallTileBlueprint` のコンポーネント追加・削除が発生するシステムを確認すること（`floor_construction_completion_system` でタイル despawn 等）。

### 参照必須ファイル

- `src/systems/familiar_ai/decide/task_management/policy/haul/demand.rs` — 現行のタイルイテレーション + IncomingDeliveries カウント
- `src/systems/familiar_ai/decide/task_management/task_assigner.rs` — スナップショット構築の挿入先
- `src/systems/spatial/` — 既存の空間グリッド同期パターンの参考
- `src/systems/jobs/floor_construction.rs` — `FloorTileBlueprint` のライフサイクル
- `src/systems/jobs/wall_construction.rs` — `WallTileBlueprint` のライフサイクル

### 完了条件（Definition of Done）

- [ ] `TileSiteIndex` が `Spatial` セットで同期されている
- [ ] `demand.rs` がタイルインデックスを使用している
- [ ] `IncomingDeliverySnapshot` が委譲サイクル冒頭で構築され、`demand.rs` から参照されている
- [ ] `cargo check` がエラーなしで通過する
- [ ] 多数サイト同時建設時のフレームレートが改善方向であること（目視確認）

## 13. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-07` | `AI (Claude)` | 初版作成 |
