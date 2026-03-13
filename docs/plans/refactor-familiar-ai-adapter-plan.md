# Familiar AI のオーケストレーター分離・Plugin 移行計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `familiar-ai-adapter-plan-2026-03-13` |
| ステータス | `Draft` |
| 作成日 | `2026-03-13` |
| 最終更新日 | `2026-03-13` |
| 作成者 | `Gemini Agent` |
| 関連提案 | `docs/proposals/crate-boundaries-refactor-plan.md` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: Familiar AI の意思決定に関するロジックの一部が依然として `bevy_app` 側のアダプターシステムとして残留しており、アーキテクチャの境界原則（`docs/crate-boundaries.md`）の「意思決定フェーズの分離」が不完全な状態にある。
- 到達したい状態: 純粋なドメインロジックが `hw_familiar_ai` 側に完全に抽出され、`hw_familiar_ai` の Plugin でシステム登録が行われる状態になる。`bevy_app` には Root 固有のリソースを注入する真のアダプターのみが残る。
- 成功指標: 対象システムの純粋関数化と Plugin 登録の移譲が完了し、`cargo check --workspace` が通過、かつゲーム内で使い魔が正しくタスクを委任できること。

## 2. スコープ

### 対象（In Scope）

- `crates/bevy_app/src/systems/familiar_ai/decide/state_decision.rs` の純粋化と登録移譲
- `crates/bevy_app/src/systems/familiar_ai/decide/familiar_processor.rs` の純粋化と登録移譲
- `crates/bevy_app/src/systems/familiar_ai/decide/task_delegation.rs` の純粋化と登録移譲

### 非対象（Out of Scope）

- Soul AI の経路探索や Jobs の建設完了判定の移動（別計画で実施）

## 3. 現状とギャップ

- 現状: `state_decision` などの処理のコアはすでに `hw_familiar_ai` へ移動済みだが、それを包む `System` 自体は `bevy_app` にあり、`bevy_app` の Plugin で登録されている。
- 問題: Leaf クレートが自律的に自身のドメインシステムを登録するという原則（§3.3）から外れている。
- 本計画で埋めるギャップ: `Cargo.toml` の依存だけで完結する System を `hw_familiar_ai` 側に移し、自クレートの Plugin で登録するように修正する。

## 4. 実装方針（高レベル）

- 方針: 「自クレート内で完結する」システムか否かを見極め、移動可能なものを `hw_familiar_ai` へ移す。
- 設計上の前提: `WorldMapRead` や `PathfindingContext`、`GameAssets` など Root 固有の型を引数に取る場合は、引き続き `bevy_app` 側に残すが、その内部ロジックは可能な限り `hw_familiar_ai` の純粋関数として抽出する。
- Bevy 0.18 APIでの注意点: System の登録順序（`before`/`after`/`chain`）が壊れないよう、移動先でも既存の `GameSystemSet::Logic` 等の制約を維持する。

## 5. マイルストーン

### M1: state_decision_system の移譲

- 変更内容: `state_decision_system` を `hw_familiar_ai` 側へ移動し、`FamiliarAiCorePlugin` 等で `add_systems` する。
- 変更ファイル:
  - `crates/bevy_app/src/systems/familiar_ai/decide/state_decision.rs`
  - `crates/hw_familiar_ai/src/...`
- 完了条件:
  - [ ] システムの登録責務が `hw_familiar_ai` に移っている
- 検証:
  - `cargo check --workspace`

### M2: familiar_processor と task_delegation の純粋化と移譲

- 変更内容: `familiar_processor` および `task_delegation` 内の `WorldMapRead` 依存等を確認し、アダプター層とドメイン層の境界を引き直す。
- 変更ファイル:
  - `crates/bevy_app/src/systems/familiar_ai/decide/familiar_processor.rs`
  - `crates/bevy_app/src/systems/familiar_ai/decide/task_delegation.rs`
  - `crates/hw_familiar_ai/src/...`
- 完了条件:
  - [ ] 純粋ロジックが `hw_familiar_ai` に移動され、適切なアダプター経由で実行されている
- 検証:
  - `cargo check --workspace`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 登録順序の喪失による 1 フレーム遅れの発生 | 高 | 移動前の `bevy_app` 側 Plugin に記述されていた `.before()` などの順序制約を、移動先の Plugin でも正確に再現する。 |
| Root 固有型を誤って Leaf に持ち込んでしまう | 高 | `cargo check` がコンパイルエラーとして弾くため、それに従って厳格に境界を引く。 |

## 7. 検証計画

- 必須:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`
- 手動確認シナリオ:
  - ゲーム内で使い魔が「木を伐る」「物を運ぶ」などのタスクを正しく検知し、魂に委任できるか確認する。
  - 使い魔の Idle -> Scouting 等の状態遷移がスムーズに行われるか確認する。

## 8. ロールバック方針

- どの単位で戻せるか: Gitコミット単位（M1, M2の完了ごと）。
- 戻す時の手順: 該当する移動コミットを `git revert` する。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: M1

### 次のAIが最初にやること

1. `crates/bevy_app/src/systems/familiar_ai/decide/state_decision.rs` の中身を確認する。
2. これが `bevy_app` に依存していないか（`Cargo.toml` の範囲内で完結するか）を調査する。
3. 完結しているなら `hw_familiar_ai` へ移動し、Plugin に登録処理を追加する。

### ブロッカー/注意点

- 「型・ドメインモデルの移動（`types-migration-plan`）」が完了した後に着手することを推奨。

### 参照必須ファイル

- `docs/crate-boundaries.md`

### 最終確認ログ

- 最終 `cargo check`: N/A
- 未解決エラー: なし

### Definition of Done

- [ ] 目的に対応するマイルストーンが全て完了
- [ ] 影響ドキュメントが更新済み
- [ ] `cargo check --workspace` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-13` | Gemini Agent | 初版ドラフト作成 |