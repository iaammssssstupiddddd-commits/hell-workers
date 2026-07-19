# ゲームプレイ管理・フィードバック・進行改善提案

## メタ情報

| 項目 | 値 |
| --- | --- |
| ドキュメントID | `gameplay-management-improvements-proposal-2026-07-17` |
| ステータス | `Draft` |
| 作成日 | `2026-07-17` |
| 最終更新日 | `2026-07-20` |
| 作成者 | `Codex` |
| 関連計画 | `docs/plans/archive/input-action-context-resolver-plan-2026-07-17.md`（A1完了）、`docs/plans/player-facing-result-notifications-plan-2026-07-18.md`（A2実装・自動検証完了、手動受入待ち）、`docs/plans/actionable-task-dashboard-plan-2026-07-19.md`（A3実装・自動検証完了、手動受入待ち） |
| 関連Issue/PR | `N/A` |

## 1. 背景と問題

hell-workers は、建築、Soul の労働、Familiar の指揮、物流、Soul Energy、Dream、
セーブ/ロードという主要ループをすでに備えている。一方、実装を横断して確認すると、
プレイヤーが運営判断を行うための「入力の一貫性」「失敗理由の可視化」
「中長期ポリシー」「進行目標」が個別機能の成長に追いついていない。

現状の主な問題は次のとおり。

- 初期監査では同じキーが複数の操作に割り当てられていた。A1 の M1〜M4 で F5/F9/V、B/Z、
  Space/Digit、Familiar command、context 別 Escape、AreaEdit、Tab、debug shortcut は resolver へ
  移行した。Modal/Pause は open request frame から pointer/camera と背景 UI を capture し、未確定 gesture を rollback する。
- A2で配置不能理由を全対象のtyped previewへ接続し、セーブ/ロード結果を有界なトーストと重要履歴へ
  表示する実装を追加した。現在は重点実機受入を残す。
- タスク一覧は件数と担当数を示せるが、「なぜ止まっているか」を説明しない。
  候補収集側がすでに判定している条件を UI へ安全に還元する仕組みがない。
- Stockpile の `resource_type` は現在内容を表すため、受入方針には使えない。
  Familiar の運用設定はセーブ対象外で、ロード時に既定値へ戻る。
- Soul Energy、解体、セーブスロットなど、運営ゲームとして必要な管理操作が部分的である。
- Dream は資源として存在する一方、継続的な判断や目標へ結び付く用途が少ない。
  Familiar の階級設定も、現状は世界観上の記述が中心である。

これらを別々の UI 追加として実装すると、入力競合、診断ロジックの重複、
セーブ形式の場当たり的な拡張を招く。本提案では、まず信頼できる操作・フィードバック基盤を整え、
その上に運営ポリシーと進行要素を積み上げるロードマップを定義する。

## 2. 目的（Goals）

- 1 回の入力が、現在のコンテキストで許可された 1 つのアクションだけを発火するようにする。
- 配置、セーブ/ロード、タスク停滞など、プレイヤーが次の行動を選ぶために必要な理由を可視化する。
- Stockpile、Familiar、Soul Energy を、個別指示ではなく持続的な運営方針で制御できるようにする。
- 建築物の解体と資源回収、複数セーブスロットを、安全なライフサイクルとして提供する。
- Dream Edict、Contract、Familiar 昇格により、短期作業と中長期目標を接続する。
- 既存のマクロ指揮中心のゲーム性と、タスク状態・物流・セーブの不変条件を維持する。
- 各改善を独立した計画と小さなリリースへ分割できる、依存関係付きの優先順位を示す。

## 3. 非目的（Non-Goals）

- 本提案の全トラックを 1 回の実装・1 PR で導入すること。
- Soul を直接一体ずつ操作するゲームへ変更すること。
- AI の経路探索や候補評価を、UI 表示のたびに再実行すること。
- HVAC/Plumbing、3D RtT、Soul outline、Soul spawn/despawn 最適化の既存提案を置き換えること。
- バランス数値、最終アート、演出品質を本提案の段階で確定すること。
- 診断用状態を、ドメインの正本や永続化必須データとして無条件に追加すること。

## 4. 提案内容（概要）

一言要約: **操作の信頼性を先に直し、説明可能な運営ポリシーと進行システムを段階導入する。**

提案を次の 4 トラックに分ける。

| 優先度 | トラック | 内容 | 主な成果 |
| --- | --- | --- | --- |
| P0 | A. 操作とフィードバック | 入力解決、通知、タスク診断 | 誤操作と「何が起きたか分からない」を減らす |
| P1 | B. 運営ポリシー | Stockpile、Familiar、Soul Energy | 繰り返し操作を方針設定へ置き換える |
| P1 | C. 復旧と永続化 | 解体、セーブカタログ、再構築基盤 | 長期プレイと試行錯誤を安全にする |
| P2 | D. 進行と選択 | Dream Edict、Contract、Familiar 昇格 | Dream と運営判断へ中長期の意味を与える |

原則として P0 を先行し、P1/P2 は個別計画へ分割する。P1 内でも、永続化する方針データは
セーブ移行方針を決めてから導入する。

## 5. 詳細設計

### 5.1 仕様

#### Track A: 操作とフィードバック（P0）

##### A1. コンテキスト付き入力アクション

実装状態: `2026-07-18` に M1〜M4、自動回帰、重点実機受入を完了。詳細な設計判断と検証記録は
`docs/plans/archive/input-action-context-resolver-plan-2026-07-17.md` を参照する。

- 物理キーを直接読む各システムの上に、狭い `InputAction` / `InputContext` 解決層を置く。
- 想定コンテキストは少なくとも `World`, `Placement`, `AreaSelection`, `TextInput`,
  `FamiliarCommand`, `Modal` とする。
- 優先順位は `Modal/TextInput`、一時操作モード、通常 World 操作の順とする。
- 1 フレーム内で消費された物理入力は、低優先度コンテキストへ伝播させない。
- 既存の `UiIntent` は UI 内の意味イベントとして維持し、すべての操作を一度に統合しない。
- キーバインド変更 UI は別計画とし、本段階では衝突のない既定割当と入力所有権を確立する。

受入条件:

- F5、F9、数字キー、`Ctrl+V` を含む既知の競合が解消される。
- TextInput または Modal の操作中に、背後の World アクションが発火しない。
- 同一の物理入力が、意図せず複数の意味アクションへ解決されないことを自動テストできる。

##### A2. プレイヤー向け結果通知

実装状態: `2026-07-18` にM1〜M4のコード、回帰テスト、恒久ドキュメント同期を完了。
有界通知センター、全配置経路のtyped live feedback、save/load terminal outcomeとreset後の発行順を実装した。
重点実機受入と計画archiveを残す。詳細な責務境界、world replacement順序、有界性、検証項目は
`docs/plans/player-facing-result-notifications-plan-2026-07-18.md` を参照する。

- 既存のプレゼンテーション用 Message 経路を利用し、短命のトーストと履歴型の重要通知を分ける。
- 配置ゴーストは `PlacementRejectReason` を保持し、カーソル付近または情報領域に
  簡潔な理由を表示する。色は補助表現とし、理由テキストを正本にする。
- セーブ/ロードは `Requested` だけでなく、`Succeeded` / `Failed` と対象スロット、
  表示可能な失敗分類を結果通知へ渡す。
- 同じ原因の通知は短時間に集約し、フレーム単位の大量通知を避ける。
- 通知はゲームロジックの成否を変更せず、観測結果だけを表示する。

受入条件:

- 配置ボタンを押す前に、現在の配置不能理由が分かる。
- セーブ/ロード操作後、画面上で対象と成否を確認できる。
- 同一失敗の連続発生で通知領域が無制限に増えない。

##### A3. アクション可能なタスクダッシュボード

実装状態: `2026-07-20` にコード、自動回帰、恒久文書同期まで完了し、重点実機受入待ち。
task ごとの applicable evaluator / producer coverage、semantic input revision、既存 assignment/arbitration cycle 由来の
latest-only 診断、flat filter/sort dashboard、positive capability allow-list、priority 0/5/10、owner別 cancellation、
world replacement reset を実装した。詳細と手動シナリオは
`docs/plans/actionable-task-dashboard-plan-2026-07-19.md` を参照する。

- 現行 `TaskEntry` を、表示用の `TaskStatusSummary` と操作意図へ拡張する。
- タスク候補収集時に判定済みの情報から、次のような有界な停止理由を導出する。
  - 担当可能な Familiar がいない
  - 必要資源または搬送元がない
  - 到達不能
  - 一時的に延期中
  - 依存タスク待ち
- 現在 `None` を返している候補除外分岐から、内部用の `CandidateRejectReason` を返す。
  通常の候補評価中にタスク別の固定長カウンタへ集約し、文字列生成や追加 Query は UI adapter 側だけで行う。
- 1 体でも実行可能な Familiar がいれば blocker を表示しない。全候補が除外された場合は、
  理由数と安定した優先順位から代表理由を選ぶ。対象 Familiar が 0 体で候補評価自体が走らない場合は、
  roster の派生状態から「担当可能な Familiar がいない」とする。
- 要求、資源、予約、TaskArea、Familiar roster など候補入力の dirty 化で要約を失効させ、
  タスク削除または実行可能化時に古い理由を残さない。
- `Deferred` と `Unreachable` を混同しない。UI のための追加 A* は実行しない。
- 種別、状態、優先度、担当数でのフィルタ/ソートを追加し、選択行から
  フォーカス、優先度変更、許可されたキャンセルを行えるようにする。
- キャンセルは既存のタスク終了・予約解除経路を通し、UI から直接コンポーネントを剥がさない。

受入条件:

- 停滞タスクについて、少なくとも 1 つの安定した理由または「判定待ち」を表示できる。
- ダッシュボード表示の有無で、候補評価や経路探索回数が増えない。
- 優先度変更とキャンセル後も、予約・担当・要求の不変条件が保たれる。

#### Track B: 運営ポリシー（P1）

##### B1. Stockpile ポリシー

- 現在の在庫内容を表す `Stockpile.resource_type` と、受入方針を分離する。
- 新しい `StockpilePolicy` は、少なくとも次を持つ。
  - 受入資源（単一または許可集合）
  - 搬入優先度
  - 目標量
  - 搬出許可
- 空になったときに現在内容は消えても、ポリシーは保持する。
- 初版の所有単位は各 Stockpile セル Entity とする。Yard ごとの `StockpileGroup` は派生値であり、
  重複 Yard では同じセルが複数グループに属するため、方針の正本を group 側へ置かない。
  範囲 UI は選択セルへ同じ変更 Intent を一括適用する。
- 現在内容と新しい受入方針が不一致になった場合、既存在庫を削除・移動せず、
  新規搬入だけを止めて搬出を許可する draining 状態として扱う。空になった後に新方針を通常適用する。
- 物流候補評価では、容量、予約量、現在内容、ポリシーの全条件を満たす搬送先だけを使う。
- UI は複数 Stockpile への一括適用を許可するが、変更は Message/Intent 経由で行う。

##### B2. Familiar 運用ポリシーと永続化

- 現在の `FamiliarOperation`（疲労閾値、最大管理 Soul 数）をセーブ対象へ含める。
- WorkType ごとの許可/優先度、活動範囲などを追加する場合は、実行時 AI 状態と分離した
  `FamiliarPolicy` として設計する。
- 全 WorkType を禁止した状態は明示的な待機方針として許可し、警告と A3 の停止理由を表示する。
  実行中タスクの安全な完了や Soul 自身の休息など、作業種別外の生命維持挙動は妨げない。
- ロード時に既定値で上書きせず、保存値がない旧セーブだけを既定値へ移行する。
- 方針不一致で実行中タスクを即時破棄せず、既存の安全な中断点または次回判断時に反映する。

##### B3. Soul Energy 制御

- 第 1 段階では、既存 `SoulSpaSite.active_slots` を UI から設定できるようにする。
- 第 2 段階で、消費設備に優先度を付け、供給不足時の負荷遮断を決定的にする。
- 同順位では安定した tie-break を使い、供給境界付近では hysteresis または最小保持時間により
  毎 tick の給電/遮断反転を防ぐ。
- 発電、配線接続、消費、遮断理由を共通の検査レンズで表示する。
- Battery は需要制御と HVAC/Plumbing の消費設備が安定した後の拡張とする。
- 同一グリッドの全消費設備を一律 on/off する現行挙動は、優先度導入後に置き換える。

受入条件:

- Stockpile と Familiar の方針がセーブ/ロード後も一致する。
- 方針変更後に不正な予約や二重搬送が発生しない。
- Soul Spa の稼働枠と消費設備の遮断順を UI から説明できる。

#### Track C: 復旧と永続化（P1）

##### C1. 一般建築物の解体と資源回収

- 明示的な `AssignedTask::Deconstruct { ... }` と対応 `WorkType` / `TaskMode` を追加する。
- 対象指定、作業予約、解体実行、関係解除、資源返却、再描画を一つの状態遷移として扱う。
- 解体時は建築種別に応じて、少なくとも次をクリーンアップする。
  - タスク要求と担当
  - 物流要求、予約、Stockpile 関係
  - WorldMap、経路障害物、Room/Power 接続
  - 親子 Entity とビジュアル
- 回収量は専用の回収テーブルを正本とする。床、橋、Soul Spa など構築方式が異なる対象へ
  `required_materials()` を一律に逆適用しない。
- 解体不能な特殊対象は理由を A2 の通知経路で表示する。

##### C2. セーブカタログ、手動スロット、オートセーブ

- 単一の `SavePath` を、スロット ID とメタデータを持つ `SaveCatalog` へ拡張する。
- `SaveCatalog` は保存ディレクトリと各 slot の header を走査して作る runtime index とし、
  world save 内には保存しない。header を読めないファイルも、ファイル情報と失敗分類を持つ
  破損 slot として一覧へ残す。ユーザー表示名を持たせる場合は header または同時に原子的更新する
  sidecar を正本とする。
- 第 1 段階は手動スロット、上書き確認、破損/非互換表示、最終更新時刻を提供する。
- 第 2 段階でオートセーブの保存先、世代数、間隔を追加する。
- 現行の同期的な排他保存の所要時間を計測し、許容時間を超える場合は
  バックグラウンド化より先にスナップショット境界を設計する。
- ロード失敗時は現在 World を破壊せず、失敗結果を A2 へ通知する。
- 現行 v1 `SaveHeader.format_version` と schema allow-list を前提に、container format と
  world schema evolution の責務境界、および v1 から次形式への移行方針を、
  新しい永続コンポーネント導入前に確定する。

##### C3. 再構築レジストリの分割

- ロード後の大きな rehydrate root を、ドメイン別の再構築ステップへ分割する。
- 実行順序をレジストリまたは明示的な system set として管理し、依存関係をテストする。
- セーブされる正本と、ロード後に再生成する index/cache/visual を文書化する。
- C3 は既存 HVAC/Plumbing 計画の M0〜M2 の前提にしない。C3 を採用する場合は、
  M3 の Conduit 保存と FluidGrid/lookup 再構築へ入る前に実装し、
  `docs/plans/hvac-plumbing-plan-2026-07-13.md` の M3 手順と変更対象を同時に改訂する。

受入条件:

- 解体後に孤立 Entity、予約、要求、経路障害物が残らない。
- 手動スロットの保存/読込/上書き/破損が、それぞれ明示的な結果になる。
- 旧セーブに既定値を補い、新セーブの方針値を失わずロードできる。

#### Track D: 進行と選択（P2）

##### D1. Dream Edict

- Dream を消費して、期間限定または範囲限定の運営方針を発令する。
- 初期候補は、効果と代償が明確な少数に限定する。
  - `Overtime`: 一時的に作業効率を上げるが、疲労増加を強める。
  - `Mandatory Repose`: 休息を優先し、短期生産を下げて疲労を回復する。
- 発令時に影響範囲と対象集合を固定または明示的に再評価し、別 World/別対象への漏れを防ぐ。
- 実行中タスクを強制破棄せず、既存のタスク終了規約に従って効果を適用する。
- active Edict は種別、期限または残り時間、効果範囲を永続化する。範囲は transient な派生 Entity ID
  ではなく durable な所有者または footprint で表し、ロード時に再検証する。
  セーブ/ロードで効果時間や代償をリセットできないようにする。

##### D2. Contract とマイルストーン

- GameTime、イベント、人口/物流統計から評価可能な期限付きまたは継続目標を導入する。
- 失敗をゲームオーバーにせず、報酬減少、次候補の変化、演出で扱う。
- 報酬は Dream、称号/印章、建築・Edict・Familiar 昇格の解禁を中心とする。
- 初期チュートリアルを Contract の特殊系列として表現できるようにする。

##### D3. Familiar 昇格

- 世界観上の `Imp`、`Servitor`、`Greater`、`Overseer` を段階的な Familiar rank として定義する。
- 昇格条件は Contract、管理実績、Dream/印章など、観測可能な進行値に結び付ける。
- 効果は管理 Soul 上限、活動範囲、作業専門化など、既存のマクロ指揮を強化する方向に限定する。
- ランクと選択した特性は永続化し、B2 の方針データと責務を分ける。

受入条件:

- Dream 支出が、短期的利益と明示的な代償を持つ。
- Contract の進捗が同じイベントを二重計上せず、セーブ/ロード後も保持される。
- Familiar rank は管理能力を拡張するが、Soul の直接操作を必須にしない。

#### トラック間の必須依存

```text
C3 を採用する場合 ───── B2 / D1 / D2 / D3 の world schema 追加
                   └── HVAC M3 の保存・再構築変更

D2 Contract ─────────── D3 Familiar 昇格
                         （Contract を昇格条件に使う場合）

B3 Soul Energy 制御 + HVAC 消費設備 ─── Battery
```

A1、A2、A3 は相互の技術的前提ではなく、D1 も D2 から独立して導入できる。

#### 推奨実装順

1. A1 で既知の入力競合を除去する。
2. A2 の共通結果通知を作り、A3 の診断 UI へ展開する。
3. A2/A3 を再利用して B1/B2、C1/C2、D1 の操作結果と停止理由を表示する。
4. 新しい world 永続データへ進む前に移行方針を確定し、C3 を採用する場合は先に導入する。
5. B3 と HVAC 消費設備の運用を確認した後に Battery を設計する。
6. D2 の Contract を昇格条件として採用した場合は、D3 を後続させる。

実装順は必須依存を満たす範囲で変更できる。ただし、競合入力を増やす変更では A1 を、
永続データを増やす変更ではセーブ移行方針を、それぞれ先送りしない。

#### 共通設計原則

- ドメイン状態を正本とし、UI 用要約は導出データとして扱う。
- UI 操作は Intent/Message を発行し、ドメインコンポーネントを直接書き換えない。
- `Deferred`、`Failed`、`Unreachable`、`Cancelled` を同義にしない。
- UI のためにホットパスの探索・全 Entity 走査を増やさない。
- 永続化対象、再構築対象、実行時一時状態を型または文書で区別する。
- 新しい `AssignedTask` は struct variant とし、クエリは `TaskQueries` に集約する。

### 5.2 変更対象（想定）

実装計画で正確な境界を再調査する。主な対象候補は次のとおり。

- 入力・UI: `crates/bevy_app/src/plugins/input.rs`、
  `crates/bevy_app/src/interface/`、`crates/hw_ui/src/`
- タスク・AI: `crates/bevy_app/src/systems/soul_ai/execute/task_execution/`、
  Familiar の候補収集/タスク管理、`crates/hw_core/src/`
- 物流: `crates/hw_logistics/src/zone.rs` と搬送候補・予約処理
- エネルギー・建築: Soul Energy、建築配置、解体、WorldMap/Room/Power の各システム
- セーブ: `crates/bevy_app/src/systems/save/` と各ドメインの serialize/rehydrate 境界
- 恒久ドキュメント: `docs/settings.md`、`docs/state.md`、`docs/tasks.md`、
  `docs/task_list_ui.md`、`docs/info_panel_ui.md`、`docs/logistics.md`、
  `docs/familiar_ai.md`、`docs/soul_energy.md`、`docs/building.md`、
  `docs/room_detection.md`、`docs/save_load.md`、`docs/dream.md`、
  `docs/world_lore.md`、`docs/events.md`、`docs/invariants.md`、`docs/architecture.md`

### 5.3 データ/コンポーネント/API 変更

以下は設計候補であり、採用後の各計画で既存型との重複を再確認する。

追加候補:

- `InputAction`, `InputContext`, 入力消費結果
- `UserFacingNotification`, `OperationOutcome`
- `TaskStatusSummary`, `TaskBlockerReason`
- `StockpilePolicy`
- `FamiliarPolicy` または永続化対応した `FamiliarOperation`
- 消費設備優先度、Power inspection 用導出状態
- `AssignedTask::Deconstruct { ... }` と解体回収テーブル
- `SaveSlotId`, `SaveSlotMetadata`, `SaveCatalog`
- `DreamEdict`, `Contract`, `ContractProgress`, `FamiliarRank`

変更候補:

- `TaskEntry` に状態要約と操作対象 ID を追加する。
- 配置結果を `bool` 表示だけでなく `PlacementRejectReason` まで UI へ渡す。
- `FamiliarOperation` と新しい方針/進行値をセーブ境界へ含める。
- Power grid の一律給電判定を優先度付き割当へ段階的に変更する。

削除候補:

- 競合した物理キーを各システムが独立して直接解釈する経路。
- ロード後に保存済み Familiar 設定を無条件で既定値へ戻す経路。
- 優先度導入後の、全消費設備を同時に on/off する一律判定。

## 6. 代替案と比較

| 案 | 採否 | 理由 |
| --- | --- | --- |
| 各機能を独立に追加する | 不採用 | 入力、通知、永続化、診断が重複し、機能間で挙動がずれる |
| 先に Contract/昇格だけを追加する | 不採用 | 目標は増えるが、失敗理由と運営手段が不足したままになる |
| すべての入力/UI を単一巨大フレームワークへ移行する | 不採用 | 変更範囲と回帰リスクが大きく、既存 `UiIntent` の価値も失う |
| P0 基盤後にトラック別導入する | 採用 | 早期に誤操作を減らし、各 P1/P2 機能を独立して検証できる |
| 診断のため毎回候補評価/A*を再実行する | 不採用 | UI 開閉がシミュレーション負荷と結果へ影響する |
| 実行時に判定済みの理由を有界に要約する | 採用 | 低コストで説明可能性を上げ、正本を増やし過ぎない |

## 7. 影響範囲

- ゲーム挙動: 入力の優先順位、物流/AI 方針、給電、解体、進行目標が変わる。
- パフォーマンス: UI 用再探索を禁止する一方、診断要約と通知の保持コストが増える。
  変更検知、イベント駆動、有界履歴を基本とする。
- UI/UX: 通知、タスクダッシュボード、ポリシー編集、セーブ選択、進行画面が増える。
- セーブ互換: B2、C2、D1、D2、D3 は形式または永続 schema の変更を伴う。
  形式バージョンと旧データの既定値補完が必要。
- AI/物流: 方針フィルタが候補集合を狭めるため、候補なし理由と fallback を明示する必要がある。
- 既存ドキュメント更新: 各トラック完了時に 5.2 の恒久ドキュメントと
  `docs/invariants.md`、必要に応じて `docs/cargo_workspace.md` を同期する。

## 8. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 入力解決層が全操作の巨大な中央集権になる | 機能追加が難しくなる | 物理入力から意味アクションへの解決だけを担当し、処理本体は既存ドメインへ残す |
| タスク理由が実際の AI 判断とずれる | 誤解を招く | 判定済みの理由コードを同じ候補評価経路から採取し、UI 独自推測を避ける |
| 方針変更で予約や実行中タスクが壊れる | 二重搬送・幽霊担当 | 安全な反映点を定義し、既存終了 disposition と cleanup を通す |
| 永続化項目の追加が旧セーブを壊す | 長期データ喪失 | 形式バージョン、既定値移行、非破壊ロード失敗、fixture テストを先行する |
| 解体 cleanup の漏れ | 到達不能・残存要求・描画不整合 | 建築種別ごとの cleanup matrix と固定 tick シナリオテストを作る |
| Power 優先度が毎 tick 全走査になる | 規模拡大時の低速化 | grid dirty 時だけ再配分し、inspection は導出結果を読む |
| Contract が単純なチェックリストになる | 世界観と運営判断が弱い | 期限、代償、複数解法、失敗後の変化を持つ少数の手作り Contract から始める |
| P2 が P0/P1 を待ち続ける | 進行要素が届かない | トラックごとに採否し、A1/A2 以外の不要な依存を作らない |

## 9. 検証計画

共通の自動検証:

- `cargo fmt --all -- --check`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- 変更 crate の単体テストと、固定 tick の横断シナリオテスト
- CI の profiling self-test と代表 workload の回帰確認

トラック別の主要シナリオ:

1. 入力: TextInput、配置、Familiar 指令、通常 World の各コンテキストで同じキーを押し、
   許可された 1 アクションだけが発火する。
2. 配置/通知: 全 `PlacementRejectReason`、保存成功、保存失敗、読込失敗を画面から識別する。
3. タスク: 資源不足、担当不在、延期、到達不能を作り、追加 A* なしで理由が更新される。
4. 物流: 空/満杯/予約中/異種資源を含む Stockpile 方針変更で、不正搬送が発生しない。
5. Familiar: 方針変更中の実行タスクを安全に終え、保存/読込後も値が維持される。
6. Power: 発電不足時に優先度順で遮断し、grid dirty でない tick に再配分しない。
7. 解体: 各建築カテゴリを解体し、Entity、要求、予約、WorldMap、Room/Power、描画の残存を確認する。
8. セーブ: 旧形式、新形式、破損、存在しないスロット、上書き、オートセーブ世代を確認する。
9. 進行: Contract/Edict の境界時刻、重複イベント、失敗、保存/読込、昇格を確認する。

成功指標:

- 既知の入力二重発火が 0 件。
- 配置不能と保存/読込結果の 100% が、ログを開かず識別可能。
- タスクダッシュボード表示による候補評価・経路探索回数の増加が 0。
- 方針、active Edict、Contract、rank のセーブ往復で値の欠落が 0。
- 解体シナリオ後の孤立要求、予約、障害物、子 Entity が 0。

## 10. ロールアウト/ロールバック

導入手順:

1. Track A を `A1`、`A2`、`A3` の別計画・別変更として実装する。
2. セーブ形式バージョンと rehydrate 分割を設計し、永続化変更の共通前提を作る。
3. Track B/C を機能単位で実装し、各段階で恒久ドキュメントと fixture を更新する。
4. Track D は少数の Edict/Contract/rank で vertical slice を作り、遊びの判断密度を評価する。
5. 各トラックの計測とプレイ確認後に、次の段階を有効化する。

段階導入:

- 新 UI は既存挙動を読み取る read-only 段階から始め、操作は後続変更で有効化できる。
- 新しい方針/進行データは旧セーブに既定値を補う。
- Power 優先度、オートセーブ、Contract は機能フラグまたは設定で個別に無効化できる境界を保つ。

ロールバック:

- 各トラックを独立した変更にし、未採用トラックを他トラックから参照しない。
- 新形式で保存したデータを旧実装が安全に無視できない場合、形式バージョン不一致として
  読込を拒否し、既存 World を保持する。
- 一時的な診断/計測コードは恒久実装と分離し、原因確認後に撤去する。

## 11. 未解決事項（Open Questions）

- [x] A1 の入力既定割当と将来のキーバインド設定との責務境界を確定した。
  `docs/plans/archive/input-action-context-resolver-plan-2026-07-17.md` の D8〜D26 を設計記録とする。
- [ ] タスク停止理由を保存せず導出する場合の、表示更新頻度と履歴保持範囲を決める。
- [ ] `StockpilePolicy` の初版を単一資源指定にするか、資源カテゴリ/許可集合まで含めるか決める。
- [ ] Familiar の活動範囲を空間領域、距離、TaskArea のどれで表現するか決める。
- [ ] 解体回収率を固定値、建築別、難易度/Edict 影響のどこまで初版へ含めるか決める。
- [ ] 現行 v1 header 上の schema evolution をどう扱うか、container format と world schema の
  バージョンを分離するか、既存 `saves/world.scn.ron` をどう移行するか決める。
- [ ] Dream Edict の効果対象を World 全体、Room、Familiar 管轄のどれから始めるか決める。
- [ ] Contract を手書きデータ、RON asset、コード定義のどれで管理するか決める。

## 12. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `提案初版 100% / A1 実装 100% / A2 コード・自動検証・docs 100%（実機受入待ち）/ A3 コード・自動検証・docs 100%（実機受入待ち）/ B〜D 未採否`
- 直近で完了したこと: A3 の latest-only task diagnostics、filter/sort dashboard、安全な priority/cancel、
  owner cancellation、save/reset回帰、恒久ドキュメント同期。
- 現在のブランチ/前提: `master`。A1 はアーカイブ済み、A2/A3 は重点実機受入後に計画をarchiveする。

### 次のAIが最初にやること

1. A2/A3 の重点実機項目を確認し、問題なければ各計画をarchiveする。
2. B〜D はユーザーと採否を確定してから、サブトラック単位の別計画を作る。

### ブロッカー/注意点

- 本書はロードマップ提案であり、そのまま全項目を一括実装しない。
- HVAC/Plumbing は `docs/proposals/hvac-plumbing-proposal.md` と対応計画を正本とする。
- タスク停止理由を得るため、UI から候補評価や A* を再実行しない。
- `Stockpile.resource_type` は現在内容であり、受入ポリシーとして再利用しない。
- `FamiliarOperation` は現状ロード時に既定値へ戻るため、方針追加前に永続化境界を修正する。
- 新しい Bevy API は 0.19 の一次情報またはローカル crate source で確認する。

### 参照必須ファイル

- `docs/invariants.md`
- `docs/tasks.md`
- `docs/logistics.md`
- `docs/familiar_ai.md`
- `docs/soul_energy.md`
- `docs/save_load.md`
- `docs/dream.md`
- `docs/world_lore.md`
- `docs/proposals/hvac-plumbing-proposal.md`
- `crates/bevy_app/src/plugins/input.rs`
- `crates/hw_ui/src/selection/placement.rs`
- `crates/hw_ui/src/panels/task_list/types.rs`
- `crates/hw_logistics/src/zone.rs`
- `crates/hw_core/src/familiar.rs`
- `crates/bevy_app/src/systems/save/`

### 完了条件（Definition of Done）

- [x] 提案内容がレビュー可能な粒度で記述されている
- [x] リスク・影響範囲・検証計画が埋まっている
- [ ] Track A〜D の採否と初回スコープが決定されている
- [x] 実装へ進むサブトラックの `docs/plans/...` が作成されている
- [x] A1 完了時に関連する恒久ドキュメントと自動回帰が更新されている
- [x] 実装完了時に関連する恒久ドキュメントと不変条件が更新されている

## 13. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-07-17` | `Codex` | 初版作成。操作、運営ポリシー、永続化、進行の 4 トラックを整理 |
| `2026-07-17` | `Codex` | A1 M2 完了を反映。数字/B、Modal/Pause、Familiar、Escape の resolver 移行と次の M3 境界を同期 |
| `2026-07-17` | `Codex` | A1 M3b 実装を反映。残存 shortcut の resolver 移行、Modal/Pause capture、gesture rollback、foreground UI ownership を同期。手動受入は関連計画を参照 |
| `2026-07-18` | `Codex` | A1 M4 と重点実機受入の完了を反映。task-area camera 競合修正、自動回帰、恒久docs同期を完了し、関連計画をアーカイブ |
| `2026-07-18` | `Codex` | A2 の計画作成と自己レビュー完了を反映。配置 feedback、通知上限、save/load terminal outcome、world replacement 後の発行順を関連計画へ固定 |
| `2026-07-18` | `Codex` | A2 のコード・自動回帰・恒久docs同期を反映。有界通知、全配置typed feedback、save/load outcomeとrollback後の生存を実装。重点実機受入は関連計画へ残す |
| `2026-07-20` | `Codex` | A3 のコード・自動回帰・恒久docs同期を反映。latest-only停止理由、filter/sort、priority/cancel、owner cleanup、reset境界を実装。重点実機受入は関連計画へ残す |
