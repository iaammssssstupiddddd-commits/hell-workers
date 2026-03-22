# Observer / Message Hot Path Optimization Plan

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `observer-message-optimization-plan-2026-03-23` |
| ステータス | `Completed` |
| 作成日 | `2026-03-23` |
| 最終更新日 | `2026-03-23` |
| 作成者 | `Codex` |
| 更新者 | `Copilot` |
| 関連提案 | `docs/proposals/architecture-improvements-2026.md`（提案4） |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題:
  `Observer` / `Event` / `Message` の使い分け基準はあるが、ホットパス上の通知系が一部混在しており、視覚演出・ログ用途の副作用まで `commands.trigger()` にぶら下がっている。
- 到達したい状態:
  ゲーム状態の即時整合性に必要な通知だけを `Observer` に残し、大量発生しうる視覚・ログ系通知は `Message` ベースのバッチ消費へ寄せる。
- 成功指標:
  タスク割り当て・集会参加などの高頻度経路で「即時必須の副作用」と「次フェーズで十分な副作用」が明文化され、主要ホットパスで不要な `Observer` 依存が削減されている。

## 2. スコープ

### 対象（In Scope）

- `OnTaskAssigned` / `OnTaskCompleted` / `OnGatheringParticipated` / `OnGatheringLeft` 周辺の発火・消費経路の棚卸し
- visual / speech / log 用の副作用を `Observer` から `MessageReader` 系システムへ移す設計
- `events.md` / `architecture.md` の運用ルール更新
- 必要に応じた軽量な計測ポイント追加

### 非対象（Out of Scope）

- パスファインディング、Spatial Grid、Flow Field など他提案の最適化
- AI 判断ロジックそのものの変更
- すべての `Observer` を `Message` へ統一する全面リライト
- UI/visual 演出内容のデザイン変更

## 3. 現状とギャップ（コード調査済み）

コードベース調査（2026-03-22）で以下の事実が確認済み。

### 確認済み事実 1: `OnGatheringLeft` は完全なデッドコード

`OnGatheringLeft` は `plain Event`（EntityEvent でも Message でもない）として定義されており、
**消費者 Observer が一切存在しない**。発火元だけが 6 ファイル・8 箇所ある。

発火元（全削除対象）:
| ファイル | 件数 |
|---|---|
| `hw_soul_ai/.../task_assignment_apply.rs` | 1 |
| `hw_soul_ai/.../gathering_apply.rs` | 2 |
| `hw_soul_ai/.../idle_behavior_apply.rs` | 2 |
| `hw_soul_ai/.../escaping_apply.rs` | 1 |
| `hw_soul_ai/.../decide/drifting.rs` | 1 |
| `hw_familiar_ai/.../squad_logic.rs` | 1 |

### 確認済み事実 2: `bevy_app::damned_soul::observers` に純ログ Observer が2本ある

`crates/bevy_app/src/entities/damned_soul/observers.rs` の以下2関数は
`info!()` を呼ぶだけでゲーム状態を変えない。ホットパスで毎回 push される Observer として不要。

- `on_task_assigned` → `info!("OBSERVER: Soul {:?} assigned ...")`
- `on_task_completed` → `info!("OBSERVER: Soul {:?} completed ...")`

登録元: `crates/bevy_app/src/entities/damned_soul/mod.rs` の `add_observer` 2行。

### 確認済み事実 3: `OnTaskAssigned` / `OnTaskCompleted` の speech Observer は遅延可

`hw_visual/src/speech/observers.rs` の `on_task_assigned` / `on_task_completed` は
speech bubble の spawn のみ。1 フレーム遅れても視覚的に問題なく、
`OnTaskAssigned` と `OnTaskCompleted` は既に `#[derive(Message)]` を持つため、
`messages.rs` に `add_message` 登録するだけで `MessageReader` ベースに変換できる。

### 確認済み事実 4: `OnTaskCompleted` には即時必須の gameplay Observer がある

`hw_soul_ai/.../update/vitals.rs::on_task_completed_motivation_bonus` は
モチベーションボーナスを即時付与する Observer であり、これは **Observer のまま維持する**。
同じイベントを Message としても同時配信できるため、Observer と MessageReader は共存可能。

### イベント分類表（調査完了）

| イベント | 型 | 即時必須 Consumer | 遅延可 Consumer | 方針 |
|---|---|---|---|---|
| `OnTaskAssigned` | `EntityEvent+Message` | なし | `speech::on_task_assigned`（speech bubble）、`damned_soul::on_task_assigned`（`info!()` のみ） | log observer 削除。speech → `MessageReader<OnTaskAssigned>` へ移行（要 `add_message` 登録） |
| `OnTaskCompleted` | `EntityEvent+Message` | `vitals::on_task_completed_motivation_bonus`（motivation+=） | `speech::on_task_completed`（speech bubble）、`damned_soul::on_task_completed`（`info!()` のみ） | log observer 削除。speech → `MessageReader<OnTaskCompleted>` へ移行。motivation Observer は維持 |
| `OnSoulRecruited` | `EntityEvent+Message` | `vitals::on_soul_recruited_effect`（motivation/stress）, `damned_soul::on_soul_recruited`（idle/path/drifting 正規化） | `speech::on_soul_recruited`（speech bubble） | `damned_soul::on_soul_recruited` は現時点で維持。`task_assignment_apply.rs` に加えて `hw_familiar_ai::squad_logic_system` も trigger 元のため、削除は別途 trigger 元統合後に再評価 |
| `OnGatheringLeft` | `plain Event` | **なし（consumer ゼロ）** | **なし** | **全発火元と event 型を削除** |
| `OnExhausted` | `EntityEvent+Message` | `damned_soul::on_exhausted`（unassign + cleanup） | `speech::on_exhausted`（speech bubble） | cleanup Observer は維持。speech は低頻度のため優先度外 |
| `OnStressBreakdown` | `EntityEvent+Message` | `damned_soul::on_stress_breakdown`（unassign + cleanup） | `speech::on_stress_breakdown`（speech bubble） | cleanup Observer は維持。speech は低頻度のため優先度外 |
| `OnGatheringParticipated` | `EntityEvent+Message` | `expression_events.rs` が既に `MessageReader` で消費済み | — | 既に適切。変更不要 |
| `OnGatheringJoined` | `EntityEvent` | なし（speech bubble のみ） | `speech::on_gathering_joined` | 低頻度。今計画のスコープ外 |
| `OnTaskAbandoned` | `EntityEvent` | なし（speech bubble のみ） | `speech::on_task_abandoned` | R-E3 制約あり。今計画のスコープ外 |
| `OnEncouraged` | `EntityEvent` | `vitals::on_encouraged_effect`（motivation/stress） | `speech::on_encouraged`（speech bubble+delay） | 低頻度。Observer 維持で問題なし |
| `OnReleasedFromService` | `EntityEvent` | なし（speech bubble のみ） | `speech::on_released_from_service` | 低頻度。今計画のスコープ外 |

## 4. 実装方針（高レベル）

- 方針:
  gameplay 状態の即時整合性に直結する通知は `Observer` を維持し、speech / expression / debug log のように 1 フレーム遅れても成立する処理は `Message` に分離する。
- 設計上の前提:
  既存の `Request` 系は `Message` が主経路であり、この方針は維持する。`Observer` はライフサイクル通知と root adapter に限定していく。
- Bevy 0.18 APIでの注意点:
  `MessageWriter<T>` は同型 writer 間で並列性を制約するが、`MessageReader<T>` は reader 同士で並列実行可能。`Observer` は push 型で即時反応するため、ホットパスでは多段副作用を避ける。

## 5. マイルストーン

## M1: ~~イベント経路の棚卸しと分類表作成~~ → 完了（分類表はセクション3に掲載）

コード調査と分類は完了済み。M1 で予定していた分類表はセクション3のイベント分類表として確定した。
`events.md` のルール更新は M2/M3 の変更後にまとめて実施する。

- 完了条件:
  - [x] `OnTaskAssigned` / `OnTaskCompleted` / `OnSoulRecruited` / `OnEncouraged` / `OnGatheringParticipated` / `OnGatheringLeft` / `BuildingCompletedEvent` の分類が文書化されている
  - [x] `OnGatheringLeft` の現在の消費経路有無が確認されている（consumer ゼロ = デッドコード）
  - [x] 1フレーム遅延可能な副作用一覧が作成されている（セクション3参照）

---

## M2: task hot path の純ログ Observer 削除 と speech Observer の Message 化

**前提**: `OnTaskAssigned` と `OnTaskCompleted` は既に `#[derive(Message)]` を持つが、
`messages.rs` には未登録のため Observer 経路のみで配信されている。
`add_message` 登録を追加することで Observer と MessageReader を**同時に共存**させられる。

### 変更手順（この順序で実施すること）

**Step 1: 純ログ Observer の削除**

ファイル: `crates/bevy_app/src/entities/damned_soul/observers.rs`
- `on_task_assigned` 関数を削除（`info!()` のみで gameplay への影響ゼロ）
- `on_task_completed` 関数を削除（同上）

ファイル: `crates/bevy_app/src/entities/damned_soul/mod.rs`
- `.add_observer(observers::on_task_assigned)` を削除
- `.add_observer(observers::on_task_completed)` を削除

> **注意**: `apply_task_assignment_requests_system` 内の `debug!(...)` がすでに同等情報を出力している。

**Step 2: `messages.rs` への Message 登録追加**

ファイル: `crates/bevy_app/src/plugins/messages.rs`
- `use` に `OnTaskAssigned, OnTaskCompleted` を追加
- `.add_message::<OnTaskAssigned>()` を追加
- `.add_message::<OnTaskCompleted>()` を追加

> これにより `commands.trigger(OnTaskAssigned {...})` がEntityEvent として Observer に push されつつ、
> Message チャンネルにも同時エンキューされる。

**Step 3: speech Observer を MessageReader システムに変換**

ファイル: `crates/hw_visual/src/speech/observers.rs`
- `on_task_assigned` のシグネチャを `On<OnTaskAssigned>` から `MessageReader<OnTaskAssigned>` ベースのシステムに変換
  - 関数名を `speech_on_task_assigned_system` に変更（Bevy システム命名規則に従う）
  - `on.entity` → `event.entity` に変更（MessageReader の場合 entity 取得方法が異なる）
- `on_task_completed` も同様に変換（`speech_on_task_completed_system`）

ファイル: `crates/hw_visual/src/speech/mod.rs`
- `app.add_observer(on_task_assigned)` を削除し、既存の `Update` / `GameSystemSet::Visual` 配置に `speech_on_task_assigned_system` を追加
- `app.add_observer(on_task_completed)` を削除し、同じく `Update` / `GameSystemSet::Visual` 配置に `speech_on_task_completed_system` を追加
- 新しい専用 `SystemSet` はこの作業では導入しない。まずは既存の visual scheduling に載せて ordering を最小変更に保つ

**Step 4: `on_soul_recruited` の依存監査（削除前提にしない）**

`crates/bevy_app/src/entities/damned_soul/observers.rs::on_soul_recruited` は以下を実施:
- `idle.total_idle_time = 0.0`
- Drifting 時に `idle.behavior = Wandering` リセット
- `path.waypoints.clear()` + `path.current_index = 0`
- `DriftingState` remove

一方 `apply_task_assignment_requests_system` の `normalize_worker_idle_state` + `apply_assignment_state` が、
**task assignment 経路に限って** これらの大半を実施済み。
しかし現時点で `OnSoulRecruited` には少なくとも 2 つの trigger 元がある。

- `hw_soul_ai::execute::task_assignment_apply`（task assignment 時）
- `hw_familiar_ai::execute::squad_logic_system`（分隊追加時）

後者は `CommandedBy` と `ParticipatingIn` の操作しか行わず、idle/path/drifting の正規化を observer 側に依存している。
そのため **現時点では `damned_soul::on_soul_recruited` を削除しない**。
本計画で行うのは「trigger 元ごとの責務を棚卸しし、将来統合できるかを文書化する」までとする。

- 変更ファイル:
  - `crates/bevy_app/src/plugins/messages.rs`
  - `crates/hw_visual/src/speech/observers.rs`
  - `crates/hw_visual/src/speech/mod.rs`
  - `crates/bevy_app/src/entities/damned_soul/observers.rs`
  - `crates/bevy_app/src/entities/damned_soul/mod.rs`

- 完了条件:
  - [ ] `on_task_assigned` / `on_task_completed` の純ログ Observer が `damned_soul/observers.rs` から削除されている
  - [ ] `OnTaskAssigned` / `OnTaskCompleted` が `messages.rs` に `add_message` 登録されている
  - [ ] `speech/observers.rs` の task assigned/completed ハンドラが `MessageReader` ベースで動作している
  - [ ] `hw_soul_ai::vitals::on_task_completed_motivation_bonus` Observer は変更なしで動作継続している
  - [ ] `on_soul_recruited` の trigger 元ごとの責務が文書化され、現時点で observer 維持が必要か判断されている

- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
  - タスク割り当て直後に Soul の speech bubble（💪）と Familiar の発言が表示されること
  - タスク完了直後に Soul の speech bubble（😊）が表示されること
  - `on_task_completed_motivation_bonus` による motivation 加算が従来通り動作すること

---

## M3: `OnGatheringLeft` の完全削除

**前提**: `OnGatheringLeft` は consumer ゼロのデッドコード。6 ファイル・8 箇所の trigger を削除し、
event 型定義も削除する。`OnGatheringLeft` は `plain Event`（`#[derive(Event, Debug, Reflect)]`）のみで
`Message` / `EntityEvent` ではないため、Message チャンネルには存在しない。

### 変更手順

**Step 1: trigger 呼び出しの削除（8箇所）**

| ファイル | 削除する行 |
|---|---|
| `hw_soul_ai/.../task_assignment_apply.rs` | `commands.trigger(OnGatheringLeft { entity: worker_entity })` |
| `hw_soul_ai/.../gathering_apply.rs` | `commands.trigger(OnGatheringLeft { entity: *soul_entity })` × 2 |
| `hw_soul_ai/.../idle_behavior_apply.rs` | `commands.trigger(OnGatheringLeft { entity: ... })` × 2 |
| `hw_soul_ai/.../escaping_apply.rs` | `commands.trigger(OnGatheringLeft { entity })` |
| `hw_soul_ai/.../decide/drifting.rs` | `commands.trigger(OnGatheringLeft { entity })` |
| `hw_familiar_ai/.../squad_logic.rs` | `commands.trigger(OnGatheringLeft { entity: ... })` |

**Step 2: `use` 文の削除（6ファイル）**

上記6ファイルそれぞれの `use hw_core::events::{..., OnGatheringLeft, ...}` から `OnGatheringLeft` を除去。

**Step 3: event 型定義の削除**

ファイル: `crates/hw_core/src/events.rs`
- `OnGatheringLeft` struct（`#[derive(Event, Debug, Reflect)]` ブロック）を削除

- 変更ファイル:
  - `crates/hw_core/src/events.rs`
  - `crates/hw_soul_ai/src/soul_ai/execute/task_assignment_apply.rs`
  - `crates/hw_soul_ai/src/soul_ai/execute/gathering_apply.rs`
  - `crates/hw_soul_ai/src/soul_ai/execute/escaping_apply.rs`
  - `crates/hw_soul_ai/src/soul_ai/execute/idle_behavior_apply.rs`
  - `crates/hw_soul_ai/src/soul_ai/decide/drifting.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/execute/squad_logic.rs`
  - `docs/events.md`
  - `docs/architecture.md`

- 完了条件:
  - [ ] `OnGatheringLeft` のコード上の参照がゼロになっている（`grep -r OnGatheringLeft crates/` が空）
  - [ ] `events.md` から `OnGatheringLeft` 行が削除されている
  - [ ] 集会参加・離脱・統合・逃走開始の各シナリオで `ParticipatingIn` relationship が正しく外れる

- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
  - 集会参加 → タスク割り当て受諾で `ParticipatingIn` が除去されること
  - 集会 Dissolve/Merge で既存の `GatheringSpot` entity が正常に despawn されること
  - 逃走開始で Soul の逃走パスが設定されること

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 即時副作用を `Message` 化しすぎて 1 フレーム遅延による仕様差分が出る | タスク解除やリクルート直後の状態整合が壊れる | gameplay 変更と visual/log 変更を分離し、即時整合が必要なものは `Observer` に残す |
| イベント分割で docs とコードが再びずれる | 運用判断が属人化する | `events.md` に producer / consumer / timing を同時更新し、M1 で分類表を先に固定する |
| `OnGatheringLeft` のような半端な状態を触って回帰を出す | 集会参加者管理や夢演出が壊れる | relationship の source/target と consumer を先に追跡し、未使用確認後に段階的に削る |
| `Message` 追加で reader/writer の依存が増え ordering が複雑化する | 期待フレームで反映されない | `SoulAiSystemSet` / `GameSystemSet::Visual` のどこで消費するかを最初に固定する |

## 7. 検証計画

- 必須:
  - `cargo check --workspace`
- 手動確認シナリオ:
  - Familiar が複数 Soul に連続で task を配る状況で speech / 表情 / motivation の発火を確認
  - 集会参加 → 離脱 → 休憩所移動 → 再勧誘の遷移で relationship と visual が壊れないことを確認
  - 建設完了、疲労限界、ストレス崩壊など lifecycle 系 Observer が遅延なく動くことを確認
- パフォーマンス確認（必要時）:
  - タスク割り当てが多い状況で `apply_task_assignment_requests_system` と speech/expression 系のフレーム時間を比較
  - `trace` / profiler が使える環境では `commands.trigger()` 多発区間と `MessageReader` 消費区間の比率を測る

## 8. ロールバック方針

- どの単位で戻せるか:
  milestone ごとに戻せるよう、M2 は event type 追加と consumer 切替を小分けに行う。
- 戻す時の手順:
  1. 新設した visual/log 用 `Message` consumer を無効化する
  2. 既存 `Observer` consumer を再接続する
  3. `events.md` / `architecture.md` を旧方針に戻す

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `30%`（M1 完了、M2/M3 未着手）
- 完了済みマイルストーン:
  - M1: イベント分類表の作成・確定（セクション3に掲載済み）
- 未着手/進行中:
  - M2: speech Observer の Message 化 + 純ログ Observer 削除
  - M3: `OnGatheringLeft` 完全削除

### 次のAIが最初にやること（優先度順）

#### 優先度1: M3（低リスクの安全な削除）

`OnGatheringLeft` consumer ゼロを確認済み。以下コマンドで再確認してから削除を開始:
```bash
rg -n "On<OnGatheringLeft|fn.*gathering_left|add_observer.*gathering_left" crates
# → 空であれば consumer ゼロ確定
```
削除対象: セクション5 M3「変更手順」の Step 1〜3 を順番に実施。

#### 優先度2: M2 Step 1（純ログ Observer 削除）

`crates/bevy_app/src/entities/damned_soul/observers.rs` の `on_task_assigned` / `on_task_completed` を削除。
削除後は `damned_soul/mod.rs` の `add_observer` 2行も削除。**これも gameplay への影響ゼロ。**

#### 優先度3: M2 Step 2〜3（speech Observer の Message 化）

`messages.rs` への登録追加後、`speech/observers.rs` のシステム変換を実施。
変換後は **speech システムが `GameSystemSet::Visual` 内に配置されるよう** ordering を確認すること。
既存 `apply_conversation_expression_event_system` が `MessageReader<OnExhausted>` を使う実装例として参照可能。

### ブロッカー/注意点

- M2 Step 3 で speech システムのスケジュール配置を間違えると speech bubble が 1 フレーム空振りする可能性がある。
  `hw_visual/src/speech/mod.rs` で現在どの `SystemSet` に配置されているかを先に確認すること。
- `on_soul_recruited` (`damned_soul/observers.rs`) の削除は「M2 Step 4 監査」を先に完了してから判断。
  削除可能と判断した場合は M2 の完了条件チェックリストに追加すること。
- `OnTaskAssigned` / `OnTaskCompleted` を `add_message` 登録した後も、既存の gameplay Observer
  （`vitals::on_task_completed_motivation_bonus` 等）は `commands.trigger()` 経由で引き続き動作する。
  **MessageReader 側は加えて動くだけで既存 Observer を壊さない。**

### 参照必須ファイル（コード調査済みの実装詳細あり）

- `crates/hw_core/src/events.rs` — イベント型定義と `#[derive]`
- `crates/bevy_app/src/plugins/messages.rs` — 登録済み Message 一覧
- `crates/bevy_app/src/entities/damned_soul/observers.rs` — 純ログ Observer（削除対象）
- `crates/bevy_app/src/entities/damned_soul/mod.rs` — `add_observer` 登録（削除対象2行）
- `crates/hw_visual/src/speech/observers.rs` — speech Observer（Message 化対象）
- `crates/hw_visual/src/speech/mod.rs` — Observer 登録と SystemSet 配置
- `crates/hw_soul_ai/src/soul_ai/update/vitals.rs` — 維持する gameplay Observer
- `crates/bevy_app/src/entities/damned_soul/movement/expression_events.rs` — `MessageReader` 実装例
- `crates/hw_soul_ai/src/soul_ai/execute/task_assignment_apply.rs` — OnGatheringLeft trigger（削除対象）

### 最終確認ログ

- 最終 `cargo check`: `2026-03-23` / `not run (plan update only)`
- 未解決エラー:
  - `N/A`

### Definition of Done

- [ ] M2: 純ログ Observer 2本の削除完了
- [ ] M2: `OnTaskAssigned` / `OnTaskCompleted` の speech Observer → MessageReader 変換完了
- [ ] M3: `OnGatheringLeft` の全参照削除完了（`grep -r OnGatheringLeft crates/` が空）
- [ ] 影響ドキュメント（`events.md`、`architecture.md`）が更新済み
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-23` | `Codex` | 初版作成 |
| `2026-03-23` | `Copilot` | コードベース調査に基づき具体化: M1 完了・分類表確定、M2/M3 を実装手順レベルにブラッシュアップ、AI引継ぎメモを調査結果で更新 |
| `2026-03-23` | `Codex` | レビュー指摘を反映: `OnSoulRecruited` の現行依存を明記、speech system の登録先を既存 `GameSystemSet::Visual` に修正、`OnGatheringLeft` 件数表記と日付を整合化 |
