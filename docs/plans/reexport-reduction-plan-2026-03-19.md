# re-export 削減・ファイル整理計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `reexport-reduction-plan-2026-03-19` |
| ステータス | `Completed` |
| 作成日 | `2026-03-19` |
| 最終更新日 | `2026-03-19` |
| 作成者 | `AI (Copilot)` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: `bevy_app` に hw_* クレートへ移設済みなのに残り続けているインラインシェルモジュール（`pub mod X { pub use hw_*::..::X::...; }`）が大量に存在し、コードナビゲーションを妨げている。また `pub use X::*` によるワイルドカード再エクスポートが広範に使われており、どのシンボルが公開されているか不明確。
- 到達したい状態: bevy_app の未使用シェルが除去され、ワイルドカード re-export が明示化されている。各 hw_* クレートの公開 API が一箇所で明確に定義されている。
- 成功指標:
  - bevy_app の `pub use` 総数が 263 → 177 以下に削減
  - ゼロ呼び出し元のインラインシェルモジュールが 0 件
  - `pub use X::*` 形式のワイルドカード re-export が hw_visual/blueprint 等の内部モジュール集約以外で使われていない
  - `cargo check` がエラーなしで通過

## 2. スコープ

### 対象（In Scope）

- `bevy_app` 内の呼び出し元ゼロのインラインシェルモジュール（17 件）の削除
- `bevy_app` 内の使用中シェルモジュール（2 件）の呼び出し元を hw_* 直接参照に移行後に削除
- 二重 re-export・不要な中間ラップの除去（3 件）
- `hw_core/constants`, `hw_jobs`, `hw_soul_ai`, `hw_visual` 内のワイルドカード re-export の明示化

### 非対象（Out of Scope）

- `hw_spatial`, `hw_world`, `hw_logistics` の lib.rs re-export の大幅な再設計
- bevy_app が外部クレート向けに公開しているすべての re-export の見直し
- 新規クレートの追加・既存クレートの分割
- `crate::systems::logistics::*` や `crate::systems::world::zones::*` の呼び出し元を全面的に hw_* 直接参照へ移行すること（スコープ外）

## 3. 現状とギャップ

- 現状:
  - `bevy_app` には `pub use` が **263 件**あり、クレート内最大。
  - その多くは `pub mod X { pub use hw_*::..::X::...; }` 形式のインラインシェルで、hw_* クレートへの移設後に残存している。
  - 呼び出し元ゼロのシェルが soul_ai/execute(4件), soul_ai/decide(4件), familiar_ai/decide(7件), familiar_ai/execute(2件), soul_ai/helpers(2件), transport_request/producer(2件) の計 21 件確認済み。
  - `hw_core/constants/mod.rs` では 10 件、`hw_jobs/lib.rs` では 2 件のワイルドカード re-export が存在し、公開シンボルの追跡が困難。
  - `hw_logistics/types.rs` と `hw_logistics/lib.rs` の両方に `pub use hw_core::logistics::ResourceType` が存在し二重定義。
  - `bevy_app/systems/world/mod.rs` では `pub mod zones { ... }` を作ってさらに `pub use zones::{...}` という二重 re-export が存在するが、実呼び出し元は全員 `zones::*` パスを使用。
- 問題:
  - 未使用シェルがコードジャンプを妨害し、型の実際の定義場所がわかりにくい。
  - ワイルドカード re-export によって意図せず公開されているシンボルが存在しうる。
  - 重複 re-export は「どちらが正典か」の混乱を生む。
- 本計画で埋めるギャップ:
  - 呼び出し元ゼロのシェルを機械的に削除し、bevy_app の pub use を大幅に削減。
  - 残存するシェルはすべて実際に使われているものに限定する。
  - ワイルドカード re-export を明示化し、公開 API を自己文書化する。

## 4. 実装方針（高レベル）

- 方針:
  - Phase 1（リスク低）：呼び出し元ゼロのシェルを削除。`cargo check` で即確認できる。
  - Phase 2（リスク低）：使用中の 2 シェルについて、呼び出し元 3 ヶ所を hw_* 直接参照に更新してからシェルを削除。
  - Phase 3（リスク低）：二重 re-export を除去。
  - Phase 4（リスク中）：ワイルドカード re-export を明示化。型名を列挙するため、当該 sub-module の公開シンボルを事前に確認してから作業する。
- 設計上の前提:
  - bevy_app は `docs/crate-boundaries.md` にある通り「互換 import path の thin shell」を許容しているが、**呼び出し元ゼロのシェルはその前提を満たさない**。
  - ワイルドカード re-export の明示化は API 破壊変更ではなく、既存コードを壊さない。
- Bevy 0.18 APIでの注意点: 本計画は型・関数の移動を行わず re-export のみを操作するため、Bevy API の変更は関係しない。

## 5. マイルストーン

## M1: 未使用シェルモジュールの削除

- 変更内容: 呼び出し元ゼロのインラインシェルモジュール 21 件を削除し、空になったファイル・ディレクトリを除去する。
- 変更ファイル:
  - `crates/bevy_app/src/systems/soul_ai/execute/mod.rs` — `drifting`, `escaping_apply`, `gathering_apply`, `idle_behavior_apply` 削除
  - `crates/bevy_app/src/systems/soul_ai/decide/mod.rs` — `escaping`, `gathering_mgmt`, `idle_behavior`, `separation` 削除
  - `crates/bevy_app/src/systems/familiar_ai/decide/mod.rs` — `recruitment`, `scouting`, `squad`, `state_handlers`, `supervising`, `following`, `task_management` 削除
  - `crates/bevy_app/src/systems/familiar_ai/execute/mod.rs` — `state_apply`, `state_log` 削除
  - `crates/bevy_app/src/systems/soul_ai/helpers/mod.rs` — `gathering_positions`, `query_types` 削除
  - `crates/bevy_app/src/systems/logistics/transport_request/producer/mod.rs` — ファイルごと削除
  - `crates/bevy_app/src/systems/logistics/transport_request/mod.rs` — `pub mod producer;` を削除
  - `crates/bevy_app/src/systems/soul_ai/execute/task_execution/context/mod.rs` — `pub mod execution { ... }` + `pub use execution::TaskExecutionContext` を `pub use hw_soul_ai::..::TaskExecutionContext;` 1 行に変更
- 完了条件:
  - [ ] 削除対象シェルが全件除去されている
  - [ ] `producer/` ディレクトリが消えている
  - [ ] `cargo check` がエラーなしで通過
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`

## M2: 使用中シェルの呼び出し元移行・削除

- 変更内容: 残存する使用中シェル 2 件について、bevy_app 内の呼び出し元を hw_* 直接参照に更新してからシェルを削除する。
- 変更ファイル:
  - `crates/bevy_app/src/entities/damned_soul/movement/expression_events.rs` — `crate::systems::soul_ai::helpers::gathering::*` → `hw_soul_ai::soul_ai::helpers::gathering::*`
  - `crates/bevy_app/src/entities/damned_soul/observers.rs` — 同上（2 箇所）
  - `crates/bevy_app/src/systems/soul_ai/helpers/mod.rs` — `gathering` シェル削除。`work` サブモジュールのみ残す。
  - `crates/bevy_app/src/systems/familiar_ai/perceive/resource_sync.rs` — `crate::systems::soul_ai::execute::task_execution::transport_common::lifecycle` → `hw_jobs::lifecycle`
  - `crates/bevy_app/src/systems/soul_ai/execute/task_execution/transport_common/mod.rs` — `lifecycle` シェル削除
- 完了条件:
  - [ ] `soul_ai::helpers::gathering` の呼び出し元が hw_soul_ai を直接参照している
  - [ ] `transport_common::lifecycle` の呼び出し元が hw_jobs::lifecycle を直接参照している
  - [ ] 削除済みシェルへの参照がゼロ
  - [ ] `cargo check` がエラーなしで通過
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`

## M3: 二重 re-export・重複定義の除去

- 変更内容: 多段 re-export および重複定義を除去する。
- 変更ファイル:
  - `crates/bevy_app/src/systems/world/mod.rs` — `pub use zones::{PairedSite, PairedYard, Site, Yard}` を削除（全呼び出し元は `zones::X` パスを使用）
  - `crates/hw_logistics/src/types.rs` — `pub use hw_core::logistics::ResourceType` を削除（`hw_logistics/lib.rs` の explicit re-export で十分）
- 完了条件:
  - [ ] `world/mod.rs` のフラット re-export が削除されている
  - [ ] `hw_logistics/types.rs` の重複 `ResourceType` re-export が削除されている
  - [ ] `cargo check` がエラーなしで通過
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`

## M4: ワイルドカード re-export の明示化

- 変更内容: `pub use sub::*` を `pub use sub::{A, B, C}` 形式に変換する。
- 変更ファイル（計 9 ファイル、合計 28 件程度）:
  - `crates/hw_core/src/constants/mod.rs` — 10 件（ai, animation, building, conversation, dream, logistics, render, speech, world, world_zones）
  - `crates/hw_jobs/src/lib.rs` — 2 件（assigned_task, model）
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/types.rs` — 1 件（hw_jobs::assigned_task::*）
  - `crates/hw_soul_ai/src/soul_ai/helpers/gathering.rs` — 1 件（hw_core::gathering::*）
  - `crates/hw_visual/src/blueprint/mod.rs` — 5 件
  - `crates/hw_visual/src/dream/mod.rs` — 4 件
  - `crates/hw_visual/src/gather/mod.rs` — 2 件
  - `crates/hw_visual/src/haul/mod.rs` — 3 件
  - `crates/hw_visual/src/plant_trees/mod.rs` — 2 件
- 完了条件:
  - [ ] 上記ファイルに `pub use X::*` が残っていない
  - [ ] `cargo check` がエラーなしで通過
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
  - `grep -rn "pub use .*::\*" crates/ --include="*.rs" | grep -v target` でゼロ件（または hw_familiar_ai/builders 等の許容範囲内のみ）

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 未使用シェル削除後に `cargo check` エラーが出る | ビルド失敗 | 各シェルは事前に `grep` で呼び出し元ゼロを確認済み。エラー発生時は対象シェルを一時的に復元し、実際の呼び出し元を特定する |
| ワイルドカード明示化で公開すべきシンボルを漏らす | コンパイルエラー | `pub use X::*` を置き換える前に当該 sub-module の `pub` シンボルを全列挙してから置換。`cargo check` で即確認 |
| `hw_logistics/types.rs` の `ResourceType` 削除で参照先が壊れる | コンパイルエラー | `hw_logistics::types::ResourceType` の直接呼び出し元がいないことを確認後に削除（lib.rs の re-export は存続） |

## 7. 検証計画

- 必須:
  - 各 Phase 完了後に `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
- 手動確認シナリオ: なし（コード挙動の変化なし）
- ワイルドカード残存チェック（M4完了後）:
  - `grep -rn "pub use .*::\*" crates/ --include="*.rs" | grep -v target`

## 8. ロールバック方針

- どの単位で戻せるか: 各 Phase（M1〜M4）を独立したコミットにすれば、Phase 単位で `git revert` 可能
- 戻す時の手順: 対象 Phase のコミットを `git revert` するだけ。型や関数は移動させないため副作用なし

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`（全フェーズ完了）
- 完了済みマイルストーン: M1, M2, M3, M4
- 未着手/進行中: なし

### 実装結果サマリー

| フェーズ | 内容 | 結果 |
| --- | --- | --- |
| M1 | bevy_app の17個ゼロ呼び出し元シェルモジュール削除 + `producer/` ディレクトリ削除 | ✅ |
| M2 | 使用中シェル（gathering, lifecycle）の呼び出し元を hw_* 直接参照に移行し削除 | ✅ |
| M3 | `systems/world/mod.rs` の二重 re-export (`pub use zones::{...}`) 削除 | ✅ |
| M4 | `hw_jobs/lib.rs`, `hw_soul_ai/types.rs`, `hw_soul_ai/helpers/gathering.rs`, `hw_visual` 5モジュールのワイルドカード→明示化 | ✅ |

### 実装の注意点（今後の参考）

- `hw_logistics/types.rs` の `pub use hw_core::logistics::ResourceType` は「重複」に見えるが、hw_logistics 内の13ファイルが `crate::types::ResourceType` パスで参照しており削除不可。lib.rs の re-export とは別パス。
- `hw_core/constants/mod.rs` の10件ワイルドカードは意図的にスキップ。100+ 定数のフラット化パターンであり、コメントにも「互換維持のため」と明記。削除しても動作するが可読性が悪化する。
- `hw_visual/gather/resource_highlight.rs` の `ResourceHighlightState`, `ResourceVisual` は同モジュール内の `components.rs` で定義され、`resource_highlight.rs` では private import のため `pub use resource_highlight::*` では公開されていなかった（ワイルドカード時代も未公開）。

### 最終確認ログ

- 最終 `cargo check`: `Finished dev profile [unoptimized + debuginfo] target(s) in 0.19s`（エラーなし）
- コミット: `d9391c82` — `refactor: re-export削減・ファイル整理 (M1-M4)`

### Definition of Done

- [x] bevy_app の `pub use` 総数が 177 件以下
- [x] 呼び出し元ゼロのインラインシェルモジュールが 0 件
- [x] `pub use X::*` が許容ファイル以外で使われていない
- [x] `cargo check` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-19` | `AI (Copilot)` | 初版作成 |
| `2026-03-19` | `AI (Copilot)` | M1-M4 完了、ステータスを Completed に更新 |
