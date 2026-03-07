# 運搬過剰搬入修正 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `transport-overdelivery-fix-plan-2026-03-07` |
| ステータス | `Draft` |
| 作成日 | `2026-03-07` |
| 最終更新日 | `2026-03-07` |
| 作成者 | `Codex` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: 設計図搬入と補充系 request で、必要量を超える資材が搬送・消費・地面残留する。
- 到達したい状態: request の需要、割り当て時判定、搬入実行時判定が同じ数量契約で揃い、stale task や遅延到着でも過剰搬入しない。
- 成功指標:
  - `DeliverToBlueprint` / `DeliverToFloorConstruction` / `DeliverToWallConstruction` / `DeliverToProvisionalWall` / `DepositToStockpile` で必要量超過が発生しない。
  - 通常運搬と猫車運搬の両方で「配達済み + 流入中 + 当フレーム予約」を上限内に保つ。
  - `cargo check` が成功する。

## 2. スコープ

### 対象（In Scope）

- Blueprint / construction / stockpile request の需要計算と `TransportDemand` 更新。
- Familiar 割り当て時の残需要再計算。
- Soul 実行時の搬入直前ガードと surplus 処理。
- 影響する仕様ドキュメントの更新。

### 非対象（Out of Scope）

- 運搬候補スコアリングや経路探索アルゴリズムの変更。
- UI 演出や資材表示の見た目変更。
- request 種別の追加削除。

## 3. 現状とギャップ

- 現状:
  - `DeliverToBlueprint` と `DepositToStockpile` は producer 側で `TransportDemand.inflight` を常に `0` に戻す経路がある。
  - `Blueprint` 通常運搬、床骨搬入、補充系の一部では、割り当て直前に残需要を再確認していない。
  - Blueprint 搬入と site/drop 搬入は、到着時点で需要充足済みでも無条件で消費またはドロップする。
- 問題:
  - stale request が再利用されると、既に十分な搬入先へ追加搬送される。
  - 猫車 request は `TransportDemand.remaining()` に依存するため、`inflight` 不整合で過剰 lease を許しやすい。
  - 補充系は drop 後に `delivery_sync` が拾うまで地面に余剰資材が残る。
- 本計画で埋めるギャップ:
  - request 数量契約を「必要量 - delivered - incoming/inflight」で統一する。
  - assignment と execution の両方で最終需要確認を入れる。
  - surplus 発生時の扱いを「消費しない / drop しない / 予約を解放する」に揃える。

## 4. 実装方針（高レベル）

- 方針:
  - producer では既存 worker/lease/incoming を `TransportDemand` に正しく反映し、`Pending` へ巻き戻すだけの upsert をやめる。
  - assignment では request kind ごとの共通残需要 API を使い、0 の場合は未割り当てで返す。
  - execution では destination ごとの「受け入れ可能量」を再判定し、不要になった荷物は消費せず予約解除して安全に戻す。
- 設計上の前提:
  - `DeliveringTo` / `IncomingDeliveries` は destination 予約の単一ソースとして維持する。
  - `TransportDemand.inflight` は producer 間で意味を揃え、worker 数または実積載数のどちらを採るかを request 種別ごとに明文化する。
  - request が stale でも panic や item 消失を起こさない。
- Bevy 0.18 APIでの注意点:
  - Relationship 依存の `IncomingDeliveries` / `TaskWorkers` は deferred 適用タイミングがあるため、同フレームの増分は既存の `ReservationShadow` / cache を併用して扱う。

## 5. マイルストーン

## M1: 需要 bookkeeping の統一

- 変更内容:
  - `DeliverToBlueprint` / `DepositToStockpile` / `ConsolidateStockpile` の producer を、construction 系 helper と同じく `inflight` 付き upsert に揃える。
  - `TransportRequestState` の上書き条件を見直し、作業中 request が producer によって `Pending` に戻されないよう整理する。
  - request kind ごとの `desired_slots` / `inflight` の意味をコメントまたは helper 名で明確化する。
- 変更ファイル:
  - `src/systems/logistics/transport_request/producer/blueprint.rs`
  - `src/systems/logistics/transport_request/producer/task_area.rs`
  - `src/systems/logistics/transport_request/producer/consolidation.rs`
  - `src/systems/logistics/transport_request/producer/mod.rs`
  - `src/systems/logistics/transport_request/state_machine.rs`
- 完了条件:
  - [ ] 既存 request upsert で `TransportDemand.inflight` が worker/lease 状態に応じて保持される
  - [ ] 作業中 request が `Pending` 扱いへ巻き戻らない
  - [ ] 猫車候補抽出が stale `remaining()` を使い続けない
- 検証:
  - `cargo check`

## M2: 割り当て時の残需要再検証を共通化

- 変更内容:
  - Blueprint 通常運搬、床骨搬入、補充運搬、必要なら仮設壁搬入に残需要チェックを追加する。
  - `policy/haul/demand.rs` に destination 種別ごとの共通 API を追加し、assign 側の重複分岐を減らす。
  - stale request は assignment しない方針に統一する。
- 変更ファイル:
  - `src/systems/familiar_ai/decide/task_management/policy/haul/demand.rs`
  - `src/systems/familiar_ai/decide/task_management/policy/haul/blueprint.rs`
  - `src/systems/familiar_ai/decide/task_management/policy/haul/floor.rs`
  - `src/systems/familiar_ai/decide/task_management/policy/haul/stockpile.rs`
  - `src/systems/familiar_ai/decide/task_management/policy/haul/provisional_wall.rs`
  - `src/systems/familiar_ai/decide/task_management/policy/haul/wall.rs`
- 完了条件:
  - [ ] destination が既に満たされている request は worker に割り当てられない
  - [ ] 通常運搬と猫車運搬で同じ残需要基準を使う
  - [ ] stale request 由来の新規 `AssignedTask` が発行されない
- 検証:
  - `cargo check`

## M3: 搬入実行時の最終ガードと仕様同期

- 変更内容:
  - Blueprint 搬入時に必要量を再確認し、不要分は消費せずキャンセルまたは drop へ分岐する。
  - floor/wall/provisional/stockpile の dropping・unloading で受け入れ可能量を確認し、超過分を置かない。
  - 仕様文書を「厳密管理」の実装に合わせて更新し、過剰搬入防止の責務位置を明記する。
- 変更ファイル:
  - `src/systems/soul_ai/execute/task_execution/haul_to_blueprint.rs`
  - `src/systems/soul_ai/execute/task_execution/haul/dropping.rs`
  - `src/systems/soul_ai/execute/task_execution/haul_with_wheelbarrow/phases/unloading.rs`
  - `src/systems/soul_ai/execute/task_execution/transport_common/cancel.rs`
  - `docs/building.md`
  - `docs/logistics.md`
  - `docs/tasks.md`（必要時）
- 完了条件:
  - [ ] 到着後に需要充足済みだった場合でも余剰資材を消費しない
  - [ ] 補充系で余剰 drop が発生しない
  - [ ] 文書が「需要計算」「assignment」「execution」の責務分担と一致する
- 検証:
  - `cargo check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `inflight` の定義を誤る | request が不足/過剰発行する | request kind ごとに「worker数基準」か「積載数基準」かを先に固定し、helper 化する |
| 到着時キャンセルで予約解放漏れが出る | 資材が永久予約される | `cancel` / `reservation` helper を流用し、成功/失敗/超過の全経路を列挙する |
| 補充系の余剰防止で既存の floor/wall sync が止まる | 建設進行が停止する | delivery_sync の消費条件と drop 条件を分離し、小さな手動シナリオで確認する |
| state 管理変更で猫車 lease が張り付く | 猫車が再利用されない | `TransportRequestState` と `WheelbarrowLease` の解除条件を同時に確認する |

## 7. 検証計画

- 必須:
  - `cargo check`
- 手動確認シナリオ:
  - Wall / Door / RestArea など通常 Blueprint へ複数 Soul で同時搬入しても必要数を超えないこと。
  - Floor / Wall construction site に複数運搬を重ねても site 周辺に余剰資材が残らないこと。
  - Stockpile 補充で満杯直前に複数運搬を重ねても capacity 超過の drop が起きないこと。
  - 仮設壁の泥搬入が 1 回で止まり、重複搬入しないこと。
- パフォーマンス確認（必要時）:
  - `cargo run -- --spawn-souls 500 --spawn-familiars 30 --perf-scenario`

## 8. ロールバック方針

- どの単位で戻せるか:
  - `producer` / `policy/haul` / `task_execution` の3段階で個別に戻せる。
- 戻す時の手順:
  - まず execution 側ガードのみ revert して挙動差分を切り分ける。
  - 次に assignment 側需要チェックを revert して task 枯渇の有無を確認する。
  - 最後に producer/state 修正を revert し、`cargo check` で整合を確認する。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: `M1` / `M2` / `M3`

### 次のAIが最初にやること

1. `DeliverToBlueprint` / `DepositToStockpile` producer の `TransportDemand` / `TransportRequestState` 更新契約を整理する。
2. assign 側で残需要チェックが抜けている通常運搬経路を埋める。
3. execution 側で surplus を消費しないガードを追加して `cargo check` を実行する。

### ブロッカー/注意点

- `Bridge` の柔軟資材問題はこの計画の一部だが、これ単独ではなく共通契約崩れの一症状として扱うこと。
- `IncomingDeliveries` は deferred 反映なので、同フレーム内増分を `ReservationShadow` だけで見ている箇所と整合を取る必要がある。
- stockpile 補充は manual request と auto request が共存するため、固定 source request を壊さないこと。

### 参照必須ファイル

- `src/systems/logistics/transport_request/producer/blueprint.rs`
- `src/systems/logistics/transport_request/producer/task_area.rs`
- `src/systems/logistics/transport_request/producer/mod.rs`
- `src/systems/familiar_ai/decide/task_management/policy/haul/demand.rs`
- `src/systems/familiar_ai/decide/task_management/policy/haul/blueprint.rs`
- `src/systems/familiar_ai/decide/task_management/policy/haul/floor.rs`
- `src/systems/familiar_ai/decide/task_management/policy/haul/stockpile.rs`
- `src/systems/soul_ai/execute/task_execution/haul_to_blueprint.rs`
- `src/systems/soul_ai/execute/task_execution/haul/dropping.rs`
- `src/systems/soul_ai/execute/task_execution/haul_with_wheelbarrow/phases/unloading.rs`
- `docs/building.md`
- `docs/logistics.md`

### 最終確認ログ

- 最終 `cargo check`: `2026-03-07` / `not run`
- 未解決エラー: 未確認（計画書作成のみ）

### Definition of Done

- [ ] 目的に対応するマイルストーンが全て完了
- [ ] 影響ドキュメントが更新済み
- [ ] `cargo check` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-07` | `Codex` | 初版作成 |
