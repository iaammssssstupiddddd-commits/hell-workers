# Familiar Task Management `hw_ai` 抽出提案

## メタ情報

| 項目 | 値 |
| --- | --- |
| ドキュメントID | `familiar-task-management-hw-ai-extraction-proposal-2026-03-11` |
| ステータス | `Draft` |
| 作成日 | `2026-03-11` |
| 最終更新日 | `2026-03-11` |
| 作成者 | `Codex` |
| 関連計画 | `docs/plans/familiar-task-management-hw-ai-extraction-plan-2026-03-11.md` |
| 関連Issue/PR | `N/A` |

## 1. 背景と問題

- 現状:
  `src/systems/familiar_ai/decide/task_management/` には、候補収集、優先度評価、搬送元選定、予約影反映、`AssignedTask` 構築など、使い魔 AI の中核ロジックがまとまって残っている。
- 問題:
  責務は `hw_ai` 向きであるにもかかわらず、root 側 `TaskAssignmentQueries` への依存を通じて `bevy_app` に残留している。結果として root crate が厚くなり、AI コアと app shell の境界が読みにくい。
- なぜ今やるか:
  `soul_ai` 側では apply 層や pure helper の切り出しが進んでおり、次の大きな塊として Familiar の task management を `hw_ai` に寄せると、crate 境界の一貫性が高まる。

## 2. 目的（Goals）

- Familiar のタスク管理ロジックを `hw_ai` の責務に揃える。
- root 側には Query adapter と plugin wiring だけを残す。
- `TaskAssignmentQueries` 依存のどこが真の blocker かを明確化する。

## 3. 非目的（Non-Goals）

- `task_execution` 全体の同時移設。
- `unassign_task` の crate 化。
- `FloorConstructionSite` / `WallConstructionSite` の今回中の移設。
- UI や visual 系の同時整理。

## 4. 提案内容（概要）

- 一言要約:
  `src/systems/familiar_ai/decide/task_management/` を段階的に `hw_ai` へ移し、root 側は thin adapter に縮小する。
- 主要な変更点:
  - `builders/`, `delegation/`, `policy/`, `task_finder/`, `validator/`, `task_assigner.rs` のうち、root-only 型を直接触らない部分を `hw_ai::familiar_ai::decide::task_management` へ移設する。
  - root 側 `FamiliarTaskAssignmentQueries` の alias を廃し、`hw_ai` で受けられる narrow query / adapter に置き換える。
  - construction-site 依存を `TaskAssignmentQueries` から分離するか、construction 系 Query を別 adapter に切り出す。
- 期待される効果:
  - root crate の AI 実装面積を減らせる。
  - Familiar 側の decide フェーズが `hw_ai` に集約され、責務判断が容易になる。
  - 今後の task management 改修で root 全体に波及しづらくなる。

## 5. 詳細設計

### 5.1 仕様

- 振る舞い:
  - 候補収集、スコアリング、source selector、reservation shadow、assignment build の仕様は変えない。
  - system 登録は移設先 crate の Plugin が唯一の所有者になる。
- 例外ケース:
  - `FloorConstructionSite` / `WallConstructionSite` に依存する Query は、そのままでは `hw_ai` に持ち込まない。
  - `TaskArea` や root 側 familiar query を必要とする箇所は、adapter を介して渡す。
- 既存仕様との整合:
  - `docs/cargo_workspace.md` の `hw_ai` 責務と一致する。
  - `docs/familiar_ai.md` と `docs/tasks.md` の挙動仕様は原則不変とする。

### 5.2 変更対象（想定）

- `src/systems/familiar_ai/decide/task_management/mod.rs`
- `src/systems/familiar_ai/decide/task_management/builders/*.rs`
- `src/systems/familiar_ai/decide/task_management/delegation/*.rs`
- `src/systems/familiar_ai/decide/task_management/policy/**/*.rs`
- `src/systems/familiar_ai/decide/task_management/task_assigner.rs`
- `src/systems/familiar_ai/decide/task_management/task_finder/*.rs`
- `src/systems/familiar_ai/decide/task_management/validator/*.rs`
- `src/systems/soul_ai/execute/task_execution/context/*.rs`
- `crates/hw_ai/src/familiar_ai/decide/`
- `docs/cargo_workspace.md`
- `docs/familiar_ai.md`
- `docs/tasks.md`

### 5.3 データ/コンポーネント/API 変更

- 追加:
  - `hw_ai` 側の `task_management` モジュール。
  - 必要に応じて narrow query / adapter trait。
- 変更:
  - `TaskAssignmentQueries` の責務分割。
  - root 側 `pub use` / alias の位置づけ。
- 削除:
  - root 側にしか意味のない `task_management` 実装本体。

## 6. 代替案と比較

| 案 | 採否 | 理由 |
| --- | --- | --- |
| `task_management` を `hw_ai` に段階移設する | 採用 | 責務が最も素直で、サイズも大きく、分割効果が高い。 |
| 現状のまま root に置く | 不採用 | crate 境界方針と逆行し、AI 実装が root に滞留する。 |
| `task_execution` 全体を先に移す | 不採用 | blocker が強く、現段階では移設コストが高い。 |

## 7. 影響範囲

- ゲーム挙動:
  原則変更しない。割り当て結果と予約挙動を維持する。
- パフォーマンス:
  実行時性能よりもコンパイル境界と保守性への影響が主。
- UI/UX:
  直接影響なし。
- セーブ互換:
  なし。
- 既存ドキュメント更新:
  `docs/cargo_workspace.md`, `docs/familiar_ai.md`, `docs/tasks.md`, 必要なら `src/systems/familiar_ai/README.md`。

## 8. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `TaskAssignmentQueries` が construction 系 root-only 型を抱えたまま | `hw_ai` へ移せない | Query を construction 系と非 construction 系に分割する |
| root 側 adapter と crate 側 system が二重登録される | schedule 初期化時の不整合 | Plugin 所有者を 1 つに固定する |
| `TaskArea` や familiar query の渡し方が肥大化する | 境界が逆に複雑化する | narrow query / trait で必要最小限の read access に絞る |
| 予約反映の経路が変わる | タスク割り当て挙動の回帰 | `ReservationShadow` と assignment build を先にテスト観点で固定する |

## 9. 検証計画

- `cargo check --workspace`
- 手動確認シナリオ:
  - Familiar が blueprint / haul / water 系タスクを従来通り選定できる
  - `TaskArea` 内外の候補フィルタが変わらない
  - 予約競合時に同一資源へ二重割り当てしない
- 計測/ログ確認:
  - task delegation 周辺の debug log と assignment 件数を比較する

## 10. ロールアウト/ロールバック

- 導入手順:
  1. `TaskAssignmentQueries` の construction 依存を切り分ける。
  2. `task_management` の pure / AI-core 部分を `hw_ai` へ移す。
  3. root 側を re-export と thin adapter に縮小する。
- 段階導入の有無:
  あり。builders / finder / policy 単位での段階移設を前提とする。
- 問題発生時の戻し方:
  移設単位ごとに commit を分け、root 側 module に差し戻す。

## 11. 未解決事項（Open Questions）

- [ ] `TaskAssignmentQueries` の最小分割単位をどう定義するか
- [ ] `FloorConstructionSite` / `WallConstructionSite` を先に `hw_jobs` 側へ寄せるべきか
- [ ] `TaskArea` 依存は adapter で十分か、それとも shared model 化が必要か

## 12. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 直近で完了したこと:
  - crate 化候補の棚卸しと優先順位付け
- 現在のブランチ/前提:
  - root 側に `task_management` 実装が残っている

### 次のAIが最初にやること

1. `TaskAssignmentQueries` と `context/access.rs` の construction 依存を再確認する。
2. `task_management` 内で root-only 型を直接触る箇所を一覧化する。
3. 段階移設の実装計画を `docs/plans/` に起こす。

### ブロッカー/注意点

- `TaskAssignmentQueries` が `FloorConstructionSite` / `WallConstructionSite` を直接 Query している。
- `task_execution` 側の問題を同時に解こうとするとスコープが崩れる。
- 移設先 Plugin を唯一の登録元にすること。

### 参照必須ファイル

- `docs/cargo_workspace.md`
- `docs/familiar_ai.md`
- `docs/tasks.md`
- `src/systems/familiar_ai/decide/task_management/mod.rs`
- `src/systems/soul_ai/execute/task_execution/context/access.rs`
- `src/systems/soul_ai/execute/task_execution/context/queries.rs`

### 完了条件（Definition of Done）

- [ ] 提案内容がレビュー可能な粒度で記述されている
- [ ] リスク・影響範囲・検証計画が埋まっている
- [ ] 実装へ進む場合の `docs/plans/...` が明記されている

## 13. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-11` | `Codex` | 初版作成 |
