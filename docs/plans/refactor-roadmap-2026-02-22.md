# 全体リファクタ計画（2026-02-22）

更新日: 2026-02-22  
対象: `src/` 全体（特に `soul_ai` / `familiar_ai` / `logistics` / `interface/ui`）

## 1. 目的

- 依存の密結合を解消し、変更影響範囲を局所化する。
- 割り当て・予約・運搬の整合性リスクを先に下げる。
- 500 Soul / 30 Familiar でも劣化しにくい構造へ段階移行する。
- `cargo check` を常時グリーンに保つ。

## 2. 現状スナップショット

- `src/` は 434 ファイル、約 55,320 行。
- 規模上位:
- `systems/soul_ai`: 12,641 行
- `interface/ui`: 9,511 行
- `systems/familiar_ai`: 8,485 行
- `systems/visual`: 5,997 行
- `systems/logistics`: 5,632 行
- `cargo check` は通過。

## 3. 優先課題

| 優先度 | 課題 | 主要ファイル | 影響 |
|---|---|---|---|
| P0 | `ReservationShadow` の適用経路が不統一 | `src/systems/familiar_ai/decide/task_management/builders/*.rs` | 同一フレーム重複予約の温床 |
| P1 | `TaskAssignmentQueries` の肥大化と逆依存 | `src/systems/soul_ai/execute/task_execution/context.rs`, `src/systems/familiar_ai/decide/task_delegation.rs` | 変更容易性と安全性の低下 |
| P1 | タスク割当適用処理の多責務化 | `src/systems/soul_ai/execute/task_execution/mod.rs` | 新タスク追加時の漏れリスク |
| P2 | pathfinding システムの多責務化 | `src/entities/damned_soul/movement/pathfinding.rs` | 回帰調査コスト増大 |
| P2 | floor/wall producer の重複ロジック | `src/systems/logistics/transport_request/producer/floor_construction.rs`, `src/systems/logistics/transport_request/producer/wall_construction.rs` | 同種不具合の横展開 |
| P3 | UI/Visual 大型ファイルの保守性 | `src/interface/ui/interaction/status_display.rs`, `src/systems/visual/dream/ui_particle.rs` | 開発速度低下 |

## 4. 実行方針

- 1フェーズ1PRを原則にする。
- 仕様変更と構造変更を同じPRに混ぜない。
- 各フェーズ完了時に `cargo check` を実行する。
- 予約・割当・運搬は「挙動互換優先」で、先に共通化してから最適化する。

## 5. フェーズ計画

### Phase 0: ベースライン固定（0.5日）

- 現行の確認項目を固定:
- タスク割当
- 予約再構築
- floor/wall の資材搬入
- pathfinding 到達不能時の挙動
- 完了条件:
- `cargo check` 通過
- 確認観点をこの文書に明示済み

### Phase 1: 割当リクエスト発行経路の統一（1日）

- `builders/haul.rs` と `builders/water.rs` の直接 `assignment_writer.write(...)` を廃止。
- 全経路を `submit_assignment(...)` 経由へ統一し、`ReservationShadow` を必ず反映。
- 完了条件:
- 発行APIが単一化される
- `cargo check` 通過

### Phase 2: TaskAssignmentQueries 分割（2日）

- `TaskAssignmentQueries` を用途別 `SystemParam` に分割。
- `familiar_ai` から `soul_ai::...::context::TaskAssignmentQueries` への直接依存を縮小。
- 完了条件:
- クエリ境界が用途単位で明確
- `cargo check` 通過

### Phase 3: タスク割当適用の責務分離（2日）

- `apply_task_assignment_requests_system` を以下に分解:
- ワーカー検証
- idle/休憩状態の正規化
- 予約反映
- `DeliveringTo` 付与
- イベント発火
- 完了条件:
- `AssignedTask` 新規追加時の差分点が明確
- `cargo check` 通過

### Phase 4: pathfinding 系の分割（2日）

- `pathfinding_system` から次を抽出:
- 既存パス再利用判定
- 再探索実行
- 休憩所フォールバック
- 失敗時クリーンアップ
- 完了条件:
- 機能別関数へ分離
- 既存挙動を維持
- `cargo check` 通過

### Phase 5: floor/wall producer 共通化（2-3日）

- `delivery_sync` の共通部を抽出し、差分を設定値化。
- upsert/cleanup の重複ロジックを削減。
- 完了条件:
- 重複実装の削減
- 搬入と状態遷移が互換
- `cargo check` 通過

### Phase 6: UI/Visual 大型ファイル分割（1-2日）

- `status_display.rs` を表示責務ごとに分割。
- `dream/ui_particle.rs` を update/merge/trail/icon に分割。
- 完了条件:
- 変更影響が局所化
- `cargo check` 通過

### Phase 7: テスト/文書整備（1日）

- 予約・割当の回帰テストを最小追加。
- `docs/plans` 索引と実体の整合を整理。
- 完了条件:
- 追加テストが通る
- ドキュメント参照が一貫

## 6. 検証コマンド

```bash
cargo check
```

必要時:

```bash
cargo run -- --spawn-souls 500 --spawn-familiars 30 --perf-scenario
```

## 7. リスクと緩和策

- 予約系リファクタで競合が顕在化するリスク:
- Phase 1を最優先で実施し、発行経路を先に単純化する。
- クエリ分割時の借用競合リスク:
- `SystemParam` 分割を小さい単位で導入し、段階的に移行する。
- パフォーマンス劣化リスク:
- 各フェーズで既存メトリクスを確認し、悪化時は当該フェーズのみロールバック可能な粒度で進める。

## 8. 完了判定

- Phase 1〜7 の完了条件をすべて満たす。
- `cargo check` を最終通過している。
- 主要フロー（割当/運搬/移動/UI）が既存仕様を満たしている。
