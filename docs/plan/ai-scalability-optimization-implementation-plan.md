# 実装計画: AIスケーラビリティ最適化

作成日: 2026-02-08  
対応提案: `docs/proposals/ai-scalability-optimization.md`

## 1. 目的

30 Familiar / 500 Soul スケールでのフレームタイム悪化要因を段階的に削減し、挙動を維持したまま AI 処理の負荷を下げる。

## 2. スコープ

### 対象（In Scope）
- タスク委任・逃走判定のA*頻度制御
- Familiar影響クエリの統合
- auto_build の全Soul走査削減
- DesignationSpatialGrid 更新方式の最適化
- reservation 同期と Build 候補収集の効率化

### 非対象（Out of Scope）
- ゲーム仕様の追加変更
- UI/アート面の変更
- 大規模なパスファインディングアルゴリズム置換

## 3. 実装フェーズ

## Phase 0: 計測基準の固定

### 実装内容
- 30 Familiar / 500 Soul 条件で比較可能な計測手順を固定する。
- 最適化前の基準値（FPS、スパイク頻度）を取得する。

### 完了条件
- 最適化前ベースライン値が記録されている。

## Phase 1: 高効果・低リスク変更

### 実装内容
1. `#1` タスク委任のタイマーゲート（0.5秒）
2. `#4` 逃走A*再評価のタイマーゲート（0.5秒）
3. `#5` `DesignationSpatialGrid` の増分更新化（全再構築廃止）

### 変更対象（予定）
- `src/systems/familiar_ai/mod.rs`
- `src/systems/familiar_ai/familiar_processor.rs`
- `src/systems/soul_ai/idle/escaping.rs`
- `src/systems/soul_ai/mod.rs`
- `src/systems/spatial/designation.rs`
- `src/plugins/spatial.rs`

### 完了条件
- `cargo check` が通る。
- タスク委任・逃走・Designation更新の挙動が維持される。
- ベースラインよりFPS/スパイクが改善する。

## Phase 2: 構造改善

### 実装内容
1. `#2` Familiar影響クエリの統合（1Soulあたり1回の空間クエリ）
2. `#3` auto_build を `Commanding` ベースの部下走査へ置換

### 変更対象（予定）
- `src/systems/soul_ai/vitals/influence.rs`
- `src/systems/soul_ai/vitals/update.rs`
- `src/systems/soul_ai/mod.rs`
- `src/systems/soul_ai/work/auto_build.rs`
- `src/relationships.rs`

### 完了条件
- `cargo check` が通る。
- stress/motivation/supervision と auto_build の挙動回帰がない。

## Phase 3: 追加最適化

### 実装内容
1. `#6` `sync_reservations_system` にタイマーゲート導入（0.2〜0.5秒）
2. `#7` Build候補収集の全Designation走査を廃止し、空間グリッド経由へ統一

### 変更対象（予定）
- `src/systems/familiar_ai/resource_cache.rs`
- `src/systems/familiar_ai/mod.rs`
- `src/systems/familiar_ai/task_management/task_finder/filter.rs`

### 完了条件
- `cargo check` が通る。
- 予約競合や割当漏れの悪化がない。

## Phase 4: 任意の深掘り（必要時のみ）

### 実装内容
- `#1-C` 連結成分マップを導入し、A*前に到達不能候補をO(1)で除外する。

### 変更対象（予定）
- `src/world/map.rs`
- `src/world/pathfinding.rs`
- `src/systems/familiar_ai/task_management/task_finder/filter.rs`

### 完了条件
- Phase 1〜3で性能が不足する場合にのみ実施する。
- `cargo check` が通る。

## 4. 検証計画

### 静的確認
- 各Phase完了ごとに `cargo check`

### 手動確認
- 30 Familiar + 500 Soul 条件でのFPS比較（前後差）
- タスク割り当ての応答性（タイマー導入後）
- 逃走行動の自然さ（反応遅延が許容範囲か）
- 建築タスク割当と予約整合性

## 5. リスクと対策

- リスク: タイマー導入で応答性が低下する  
  対策: 0.5秒を初期値にし、必要なら0.3秒まで調整する。

- リスク: クエリ統合時のロジック差異による回帰  
  対策: 旧条件分岐をテスト観点として明文化し、手動確認する。

- リスク: reservation同期の間引きで競合が増える  
  対策: まず短い間隔（0.2秒）で導入し、問題なければ拡張する。

## 6. 実装順（確定）

1. Phase 0（計測基準固定）
2. Phase 1（`#1` `#4` `#5`）
3. Phase 2（`#2` `#3`）
4. Phase 3（`#6` `#7`）
5. Phase 4（`#1-C` 必要時）

## 7. 承認ポイント

- この計画で着手する場合、まず Phase 1 の実装を開始する。
- 各Phase完了時に `cargo check` 結果と計測差分を共有する。
