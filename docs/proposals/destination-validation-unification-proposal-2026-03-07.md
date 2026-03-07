# 搬入先バリデーションの一元化

## メタ情報

| 項目 | 値 |
| --- | --- |
| ドキュメントID | `destination-validation-unification-proposal-2026-03-07` |
| ステータス | `Draft` |
| 作成日 | `2026-03-07` |
| 最終更新日 | `2026-03-07` |
| 作成者 | `AI (Claude)` |
| 関連計画 | `docs/plans/destination-validation-unification-plan-2026-03-07.md` |
| 関連Issue/PR | `N/A` |

## 1. 背景と問題

- 現状: 搬入先（FloorSite, WallSite, ProvisionalWall 等）の受入可能量チェックが **3箇所に独立実装** されている:
  1. **割り当て時** — `policy/haul/demand.rs` の `compute_remaining_*` 系関数
  2. **手運搬Dropping時** — `haul/dropping.rs` の `floor_site_can_accept` / `wall_site_can_accept` / `provisional_wall_can_accept`
  3. **猫車Unloading時** — `haul_with_wheelbarrow/phases/unloading.rs` の `floor_site_remaining` / `wall_site_remaining` / `provisional_wall_remaining`
- 問題:
  - 同一ロジック（タイル状態チェック + 定数参照 + 近傍資材カウント）が3回書かれており、合計 ~250行の重複
  - 定数やタイル状態の判定条件を変更すると **4箇所** を同期する必要がある
  - `count_nearby_ground_resources` も `dropping.rs` と `unloading.rs` で独立に定義（微差あり: `exclude_item` パラメータの有無）
  - `demand.rs` は `IncomingDeliveries` + `ReservationShadow` を差し引く一方、実行時ガードは `nearby_ground_resources` を差し引くという **異なる引き算ロジック** が暗黙に併存
- なぜ今やるか: 現在の変更セット（overdelivery 防止ガード追加）で重複が急増した直後であり、パターンが新鮮なうちに統合すべき。新しい搬入先（WallConstruction 等）が追加されるたびに重複が線形に増加する。

## 2. 目的（Goals）

- 各搬入先の「残需要」「受入可否」を **単一モジュール** で定義し、割り当て時と実行時の両方から呼び出す
- `water.rs` で既に確立されたパターン（`logistics/` 配下に搬入先別の pure function を置く）を他の搬入先にも適用
- 搬入先追加時の変更箇所を 1箇所に集約

## 3. 非目的（Non-Goals）

- trait ベースの `DestinationAcceptability` プロトコルの導入（過剰抽象化。搬入先ごとの差異が大きく、trait で統一するメリットが薄い）
- `demand.rs` の `IncomingDeliveries` / `ReservationShadow` ロジック自体の変更（本提案は「タイル残需要の計算」の共通化に限定）
- 割り当て時と実行時の引き算ロジックの完全統一（それぞれ参照できるデータが異なるため、呼び出し側の責務として残す）

## 4. 提案内容（概要）

- 一言要約: `logistics/` 配下に搬入先別の需要計算モジュールを追加し、既存3箇所の重複を解消する
- 主要な変更点:
  1. `src/systems/logistics/floor_construction.rs` を新設 — `floor_site_tile_demand(tiles, site_entity, resource_type) -> usize`
  2. `src/systems/logistics/wall_construction.rs` を新設 — `wall_site_tile_demand(tiles, site_entity, resource_type) -> usize`
  3. `src/systems/logistics/provisional_wall.rs` を新設 — `provisional_wall_demand(building, provisional_opt) -> usize`
  4. `src/systems/logistics/ground_resources.rs` を新設 — `count_nearby_ground_resources(...)` の共通実装
  5. `demand.rs`, `dropping.rs`, `unloading.rs` をこれらの共通関数の呼び出しに置き換え
- 期待される効果:
  - 重複 ~200行の削減
  - 新しい搬入先を追加する際の変更箇所が「logistics モジュール + 1箇所の呼び出し追加」に限定
  - 割り当て時と実行時で同一のタイル残需要計算が保証される

## 5. 詳細設計

### 5.1 仕様

**共通関数のシグネチャ（案）**:

```rust
// src/systems/logistics/floor_construction.rs
/// FloorConstructionSite の特定リソースに対するタイル残需要（incoming 控除なし）
pub fn floor_site_tile_demand(
    floor_tiles: impl Iterator<Item = &FloorTileBlueprint>,
    site_entity: Entity,
    resource_type: ResourceType,
) -> usize;

// src/systems/logistics/wall_construction.rs
pub fn wall_site_tile_demand(
    wall_tiles: impl Iterator<Item = &WallTileBlueprint>,
    site_entity: Entity,
    resource_type: ResourceType,
) -> usize;

// src/systems/logistics/provisional_wall.rs
/// 仮設壁 1 棟の StasisMud 残需要（0 or 1）
pub fn provisional_wall_mud_demand(
    building: &Building,
    provisional_opt: Option<&ProvisionalWall>,
) -> usize;
```

- 振る舞い: 各関数は **タイル状態と搬入済みカウントのみ** から「あと何個必要か」を返す。`IncomingDeliveries` / `ReservationShadow` / `nearby_ground_resources` の控除は **呼び出し側の責務** として残す。
- 例外ケース: `site_entity` に一致するタイルが 0 件の場合は 0 を返す。
- 既存仕様との整合: 計算結果は現行の `demand.rs` 内部関数と同一。呼び出し側が控除する方法が異なるだけ（割り当て時は `IncomingDeliveries + Shadow`、実行時は `nearby_ground_resources`）。

### 5.2 変更対象（想定）

**新規作成:**
- `src/systems/logistics/floor_construction.rs`
- `src/systems/logistics/wall_construction.rs`
- `src/systems/logistics/provisional_wall.rs`
- `src/systems/logistics/ground_resources.rs`（`count_nearby_ground_resources` 共通化）

**変更:**
- `src/systems/logistics/mod.rs` — 新モジュールの `pub mod` 追加
- `src/systems/familiar_ai/decide/task_management/policy/haul/demand.rs` — `compute_remaining_with_incoming` / `compute_remaining_wall_with_incoming` を共通関数呼び出しに置換（~40行削減）
- `src/systems/soul_ai/execute/task_execution/haul/dropping.rs` — `floor_site_can_accept` / `wall_site_can_accept` / `provisional_wall_can_accept` / `count_nearby_ground_resources` を共通関数呼び出しに置換（~100行削減）
- `src/systems/soul_ai/execute/task_execution/haul_with_wheelbarrow/phases/unloading.rs` — `floor_site_remaining` / `wall_site_remaining` / `provisional_wall_remaining` / `count_nearby_ground_resources` を共通関数呼び出しに置換（~80行削減）

### 5.3 データ/コンポーネント/API 変更

- 追加: 上記の `pub fn` 4〜5個（pure function、コンポーネント/リソース変更なし）
- 変更: なし
- 削除: 各ファイルのローカル重複関数

## 6. 代替案と比較

| 案 | 採否 | 理由 |
| --- | --- | --- |
| A: `logistics/` に pure function を置く（本提案） | 採用 | `water.rs` と同じパターン。ECS 非依存で テスト容易 |
| B: trait `DestinationAcceptability` を定義 | 不採用 | 搬入先ごとの引数が異なりすぎる（Stockpile は capacity ベース、Site はタイル走査ベース）。trait 化の恩恵が薄い |
| C: `demand.rs` に集約（`pub fn` 化のみ） | 不採用 | `demand.rs` は `TaskAssignmentQueries` に依存しており、実行時（`TaskExecutionContext`）から呼べない。logistics 層に置くことで両方から参照可能 |

## 7. 影響範囲

- ゲーム挙動: 変更なし（計算結果は同一）
- パフォーマンス: 変更なし（関数呼び出しのインダイレクション追加のみ。インライン化される）
- UI/UX: 変更なし
- セーブ互換: 影響なし
- 既存ドキュメント更新: `docs/logistics.md` §8「システム追加時の実装ルール」に共通モジュール参照の記述を追加

## 8. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 呼び出し側の控除ロジックの不一致が残る | 割り当て時と実行時で「受入可能」の判定結果が微妙に異なる可能性 | 共通関数のドキュメントに「控除は呼び出し側の責務」と明記。将来的に実行時ガードも `IncomingDeliveries` ベースに統一検討 |
| `ground_resources.rs` のクエリパラメータ差異 | `dropping.rs` は `exclude_item` を渡すが `unloading.rs` は渡さない | 共通関数に `Option<Entity>` として `exclude_item` を受け取るシグネチャにする |

## 9. 検証計画

- `cargo check`
- 手動確認シナリオ:
  - FloorConstruction に Bone / StasisMud を搬入 → 必要数ぴったりで停止すること
  - WallConstruction に Wood / StasisMud を搬入 → 同上
  - ProvisionalWall に StasisMud を搬入 → 1個で停止すること
  - 猫車で上記各搬入先に搬入 → 過剰搬入が発生しないこと
  - Blueprint に資材搬入 → 既存動作と変わらないこと
- 計測/ログ確認: 搬入ログ (`TASK_EXEC:`, `WB_HAUL:`) が従来通り出力されること

## 10. ロールアウト/ロールバック

- 導入手順: 共通モジュール作成 → 呼び出し置換を1ファイルずつ実施 → 各段階で `cargo check`
- 段階導入の有無: あり（`dropping.rs` → `unloading.rs` → `demand.rs` の順に段階置換可能）
- 問題発生時の戻し方: 共通モジュールを削除し、ローカル関数を復元（git revert で対応可能）

## 11. 未解決事項（Open Questions）

- [ ] `count_nearby_ground_resources` を `logistics/ground_resources.rs` に置く場合、`TaskExecutionContext` のクエリ（`resource_items`）への参照をどう渡すか。クエリ結果のイテレータを受け取る形にするか、`&Query<...>` を直接渡すか。
- [ ] `demand.rs` 側はタイルイテレータを `queries.storage.floor_tiles.iter()` で取得するが、将来的にインデックス化（提案 002）した場合にシグネチャを変える必要があるか。→ `impl Iterator<Item = &FloorTileBlueprint>` なら透過的に対応可能。

## 12. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 直近で完了したこと: 提案書の作成
- 現在のブランチ/前提: `master`、overdelivery 防止ガードが `dropping.rs` / `unloading.rs` に追加済み（未コミットの変更セット内）

### 次のAIが最初にやること

1. `src/systems/logistics/water.rs` を参考に、`floor_construction.rs` / `wall_construction.rs` / `provisional_wall.rs` の共通関数を作成
2. `dropping.rs` のローカル関数を共通関数呼び出しに置換し、`cargo check`
3. `unloading.rs` のローカル関数を共通関数呼び出しに置換し、`cargo check`
4. `demand.rs` の内部関数を共通関数呼び出しに置換し、`cargo check`

### ブロッカー/注意点

- 現在 `demand.rs` / `dropping.rs` / `unloading.rs` に未コミットの変更がある。これらの変更がコミットされた後に着手するのが安全。
- `dropping.rs` の `count_nearby_ground_resources` は `exclude_item: Entity` を受け取るが、`unloading.rs` 版は受け取らない。共通化時に `Option<Entity>` で統一する。

### 参照必須ファイル

- `src/systems/logistics/water.rs` — 参考パターン
- `src/systems/familiar_ai/decide/task_management/policy/haul/demand.rs` — 割り当て時の需要計算
- `src/systems/soul_ai/execute/task_execution/haul/dropping.rs` — 手運搬の実行時ガード
- `src/systems/soul_ai/execute/task_execution/haul_with_wheelbarrow/phases/unloading.rs` — 猫車の実行時ガード

### 完了条件（Definition of Done）

- [ ] `logistics/` 配下に共通関数モジュールが存在する
- [ ] `dropping.rs` / `unloading.rs` / `demand.rs` からローカル重複関数が削除されている
- [ ] `cargo check` がエラーなしで通過する
- [ ] 手動テストで過剰搬入が発生しないことを確認

## 13. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-07` | `AI (Claude)` | 初版作成 |
| `2026-03-07` | `Codex` | 詳細版の実装計画 `docs/plans/destination-validation-unification-plan-2026-03-07.md` を追加し、関連計画を更新 |
