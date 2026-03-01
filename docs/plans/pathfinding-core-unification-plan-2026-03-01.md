# Pathfinding 探索核共通化 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `pathfinding-core-unification-plan-2026-03-01` |
| ステータス | `Completed` |
| 作成日 | `2026-03-01` |
| 最終更新日 | `2026-03-01` |
| 作成者 | `Codex` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: `find_path` と `find_path_to_boundary` に A* の中核処理が重複しており、修正時の片側漏れが発生しやすい。
- 到達したい状態: 探索ループとノード更新処理を共通核へ集約し、関数ごとの差分はポリシー関数で扱う。
- 成功指標:
  - 探索ロジック重複の削減。
  - `find_path` / `find_path_to_adjacent` / `find_path_to_boundary` の挙動互換維持。
  - `cargo check` が成功。

## 2. スコープ

### 対象（In Scope）

- `src/world/pathfinding.rs` の探索本体共通化。
- ゴール判定・通行可否判定・コスト付与の差分抽出。
- 呼び出し側互換を維持した関数シグネチャ調整。

### 非対象（Out of Scope）

- ヒューリスティック定数変更。
- `WorldMap` のデータ構造変更。
- path smoothing 追加などの仕様拡張。

## 3. 現状とギャップ

- 現状:
  - 標準探索と boundary 探索が別実装で類似ループを持つ。
- 問題:
  - バグ修正・最適化時に2箇所修正が必要。
  - 条件分岐の差分把握コストが高い。
- 本計画で埋めるギャップ:
  - 共通 A* エンジン + 戦略差分注入で保守単位を統一する。

## 4. 実装方針（高レベル）

- 方針:
  - `PathSearchPolicy`（仮称）で以下を差し替え可能にする。
    - 隣接ノード通過可否
    - 角抜け判定
    - 追加コスト
    - 終了条件とパス後処理
  - 既存 API 名は維持し、内部で共通核を呼ぶ。
- 設計上の前提:
  - `PathfindingContext` の再利用モデルは維持する。
  - `allow_goal_obstacle` の互換挙動を壊さない。
- Bevy 0.18 APIでの注意点:
  - Bevy API依存は低いが、`Resource` として使う `WorldMap` 参照契約を変更しない。

## 5. マイルストーン

## M1: 探索共通核の抽出

- 変更内容:
  - A* ループ・open set 更新・`came_from` 再構築を共通関数化。
  - 既存 `find_path` を新共通核に接続。
- 変更ファイル:
  - `src/world/pathfinding.rs`
  - `docs/architecture.md`（必要時）
- 完了条件:
  - [x] `find_path` の挙動が維持される
  - [x] 共有コードが1箇所に集約される
- 検証:
  - `cargo check`

## M2: boundary/adjacent の移行

- 変更内容:
  - `find_path_to_boundary` / `find_path_to_adjacent` を共通核ベースへ移行。
  - ターゲット境界停止の後処理をポリシー化。
- 変更ファイル:
  - `src/world/pathfinding.rs`
  - `src/entities/damned_soul/movement/pathfinding.rs`（必要時）
- 完了条件:
  - [x] 3関数が共通核で動作
  - [x] 既存テスト（`test_path_to_boundary_1x1_open`）が通る
- 検証:
  - `cargo check`

## M3: 呼び出し側の整理と文書化

- 変更内容:
  - 呼び出し元コメント・補助関数を最新設計へ更新。
  - 挙動上の注意点（goal obstacle 等）を docs へ明記。
- 変更ファイル:
  - `src/world/pathfinding.rs`
  - `docs/architecture.md`（必要時）
  - `docs/world_layout.md`（必要時）
- 完了条件:
  - [x] 差分責務がコード上で明確
  - [x] `cargo check` 成功
- 検証:
  - `cargo check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 境界停止判定の後退 | Soul が障害物へ侵入/停止位置不正 | `find_path_to_boundary` の既存テストと追加シナリオで検証 |
| 共通化で条件が複雑化 | 可読性低下 | ポリシー関数を小分けにし、1責務1関数を徹底 |
| door cost 適用漏れ | ルート選択が変化 | 旧実装との比較ログを一時的に出して差分確認 |

## 7. 検証計画

- 必須:
  - `cargo check`
- 手動確認シナリオ:
  - 通常目的地、障害物目的地、2x2 占有建物境界への到達。
  - RestArea 近傍での fallback 経路計算。
- パフォーマンス確認（必要時）:
  - `cargo run -- --spawn-souls 500 --spawn-familiars 30 --perf-scenario`

## 8. ロールバック方針

- どの単位で戻せるか:
  - `src/world/pathfinding.rs` のコミット単位で戻せる。
- 戻す時の手順:
  - 共通化コミットを revert。
  - 既存テストと `cargo check` を再実行。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`
- 完了済みマイルストーン: `M1` / `M2` / `M3`
- 未着手/進行中: なし

### 次のAIが最初にやること

1. 仕様変更時は `find_path_with_policy` と既存呼び出し3関数の双方を確認する。
2. 角抜け判定と penalty 付与のポリシー差分を先に明文化してから変更する。
3. 変更後に `cargo check` を実行する。

### ブロッカー/注意点

- `find_path_to_adjacent` の逆引きロジックは `allow_goal_obstacle` 依存があるため維持が必須。

### 参照必須ファイル

- `src/world/pathfinding.rs`
- `src/entities/damned_soul/movement/pathfinding.rs`
- `docs/DEVELOPMENT.md`

### 最終確認ログ

- 最終 `cargo check`: `2026-03-01` / `pass` (`cargo check --target-dir /tmp/hell-workers-target`)
- 未解決エラー: なし（計画作成時点）

### Definition of Done

- [x] 目的に対応するマイルストーンが全て完了
- [x] 影響ドキュメントが更新済み
- [x] `cargo check` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-01` | `Codex` | 初版作成 |
| `2026-03-01` | `Codex` | 実装完了に合わせてステータス・進捗・DoDを更新 |
