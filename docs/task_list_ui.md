# タスクリストUI仕様

最終更新: 2026-09-16

## 概要
右の共通枠で開く管理ページの「仕事」タブです（「使い魔・魂」と切替）。初期状態では閉じています。[地図中心UI](ui-world-first.md)を参照してください。
現在の **Designation（仕事の指示）** をダッシュボードとして表示します。担当状況に加えて、通常の
AI 判定 cycle から得た停止理由、絞り込み・並べ替え、安全な優先度変更・キャンセルを提供します。

## 表示構成

### グループヘッダー
`Sort: Type` のときだけ、隣接する同じ `WorkType` ごとにヘッダーが表示されます。
状態・優先度・担当数で並べ替えた場合は flat list になり、各行のアイコンで種別を識別します。

`[WorkTypeアイコン] [ラベル] ([件数])`

- アイコンは WorkType に対応（斧=Chop、ピッケル=Mine、ハンマー=Build、運搬=Haul系）
- テーマカラーで着色

### タスクアイテム
各アイテムはフォーカス行と、操作可能な場合だけ表示される action bar で構成します。

`[WorkTypeアイコン] [説明] [状態] [priority tier] [ワーカーカウント ×N]`

#### 1. WorkType アイコン (16px)
`WorkType`の正本は`crates/hw_core/src/jobs.rs`です。`WorkType::ALL` と
`stable_index()` が policy editor、filter、normalization の共通安定順を定め、
`hw_ui::panels::task_list::work_type_icon`が全variantをexhaustive matchしてアイコンとテーマ色を決めます：
- **Chop**: 斧アイコン / `chop` 色
- **Mine**: ピッケルアイコン / `mine` 色
- **Build / Move / Refine / ReinforceFloorTile / PourFloorTile / FrameWallTile / CoatWall / GeneratePower / Deconstruct**: ハンマーアイコン / `build` 色
- **Haul / HaulToMixer / WheelbarrowHaul**: 運搬アイコン / `haul` 色
- **GatherWater / HaulWaterToMixer**: 運搬アイコン / `water` 色
- **CollectBone**: 骨アイコン / `gather_default` 色

#### 2. 説明テキスト (12px)
作業種別と対象エンティティに基づいて自動生成されます。
- **建築**: `Construct [BuildingType]` (例: `Construct Wall`)
- **採掘**: `Mine Rock`
- **伐採**: `Chop Tree`
- **運搬**: `Haul [Resource]` (手動), `Haul [Resource] to Mixer` (自動)
- **Soul Spa建設搬入**: `Haul Bone to Soul Spa`
- **水汲み**: `Gather Water`
- **解体**: `Deconstruct [BuildingType]` (例: `Deconstruct Tank`)

優先度は `Normal = 0..=4`、`High = 5..=9`、`Critical = 10..` の共通 tier へ正規化します。
説明色、filter、sort、summary、変更ボタンはすべて `TaskPriorityTier::from_priority` を正本にします。

#### 3. ワーカーカウント (10px)
作業員が割り当てられている場合のみ `×N` を `text_secondary` 色で表示します。

#### 4. 状態

- `Working`: 現在の `TaskWorkers` が 1 件以上。
- `Blocked: <reason>`: applicable な全 producer / evaluator の current cycle が terminal rejection まで完走した場合だけ表示。
- `Evaluating...`: snapshot 不在、input revision 不一致、coverage 不足、割り当て要求 submit 後で worker 未反映など。

停止理由は `No eligible familiar`、`Missing resource or source`、`Unreachable`、
`Waiting for reservation`、`Waiting for dependency`、`Disabled by familiar policy` の 6 分類です。
Deconstruction orderはowner finalizerのtyped blockerを優先して、target変更、owner不整合、安全な回収先不足、
Mixer在庫不整合、移動中、未対応targetを個別表示します。
policy blocker は、idle worker を持つ全 applicable Familiar evaluator が current cycle を完走し、
全 terminal vote が policy-only の場合だけ表示します。UI は候補探索や経路探索を再実行せず、
Familiar delegation / Blueprint auto-build / wheelbarrow arbitration が通常処理中に公開した latest-only snapshot を読みます。
unowned Blueprint の `Build` だけが Familiar delegation と Blueprint auto-build の両 producer を必要とし、
`ManagedBy` 付き Blueprint は auto-build が適用外なので Familiar delegation だけで判定します。
Familiar 側の policy-only 一票で、unowned Blueprint の missing / stale / submitted / complete な
auto-build evidence を上書きしません。
各 blocker record は理由が参照した domain（task / roster / availability / topology）だけを鮮度判定に使います。
ただし producer cycle の evaluator coverage は roster stamp も照合し、作業可能 Soul / Familiar 構成が変わった旧 cycle は
`Evaluating...` に戻します。
Actorの経路探索は`Inventory`のchange detectionを保持したまま処理し、
到達不能タスクの携行品cleanupが必要な場合だけmutable dereferenceします。
通常移動・探索成功・idleの到達不能処理で`Changed<Inventory>`を立てて、
資材の状態が不変なblockerを無効化してはいけません。

### ツールバー

4 filter と 2 sort control を表示します。filterは選択肢から直接選び、sortは順番に切り替えます。
これらのcontrol/filter/sort variantはプレイヤーHelpの`task-dashboard-filter-sort` entryへ
exhaustive coverageされ、追加・変更時は`docs/help-screen.md`の更新手順とexact approvalを同時に更新します。

- Type: 全種別または単一 `WorkType`
- State: All / Working / Blocked / Pending
- Priority: All / Normal / High / Critical
- Workers: All / Assigned / Unassigned
- Sort: Type / State / Priority / Workers
- Order: Asc / Desc

同値の最終順序は Entity index / generation で固定し、query や HashMap の反復順に依存させません。

### ページとスクロール

filter/sort後の全件を20件ずつ表示します。固定footerの先頭・前・次・末尾ボタンと
表示範囲/総件数で移動し、ページ内は標準 `ScrollArea` / `Scrollbar` でスクロールします。
toolbarとfooterは本文の外側に残ります。group件数はfilter後の全体を数えます。
filter/sort変更は先頭ページ、ページ変更は本文の先頭へ戻ります。通常のデータ更新は
ページと本文のoffsetを保持し、件数減少時は最終ページへclampします。

## ビジュアルフィードバック

エンティティリストと統一されたホバー・選択ハイライトを提供します。

### 背景色
- **デフォルト**: `list_item_default`
- **ホバー**: `list_item_hover`
- **選択中**: `list_item_selected`
- **選択中+ホバー**: `list_item_selected_hover`

### 選択ボーダー
`TaskDashboardActionState.active_task`に対応するアイテムに左 3px の `list_selection_border` 色ボーダーを表示します。

## 更新タイミング

- `PreUpdate` で `detect_task_list_changed_components` → `detect_task_list_removed_components` → `update_task_list_state_system` を順序固定で実行します。
- `LeftPanelMode::TaskList` 中でも、無変更フレームではスナップショット再生成と子 UI の再構築を行いません。
- 再生成トリガーは、`Designation` とその表示内容に影響する関連コンポーネントの `Added` / `Changed` / `Removed`、および管理ページのタブ切替です。
- `TaskListDirty` は `state_dirty` / `list_dirty` / `summary_dirty` の 3 つの責務に分かれます。
- `state_dirty` は snapshot と summary の再計算要求、`list_dirty` は管理ページ本文の再描画要求、`summary_dirty` は画面上部 summary の更新要求です。
- `TaskListState.snapshot` は最新観測済みデータを保持し、未描画の `pending` snapshot は持ちません。
- diagnostics の cycle ID 自体は `TaskEntry` に含めず、表示内容が同じなら周期評価だけで UI を再構築しません。
- `Changed<FamiliarPolicy>` / `RemovedComponents<FamiliarPolicy>` は task diagnostic の roster revision を進めます。
  task list の dirty 検知は policy 本体を再評価せず、更新された diagnostics / revision を通常の dirty source として読みます。
- 管理ページを `TaskList` に切り替えたフレームは `mark_all()` で `state_dirty` / `list_dirty` を両方立て、最新スナップショットで再描画します（タスクデータが変わっていない場合も含む）。
- 画面上部の「要対応」は `AttentionSummary` の新鮮なBlocked対象数と別集計の判定中件数を参照します。確定した施工元は重複集約するため、明細行数とは異なります。
- 常駐するtask行は現在ページの最大20件です。非表示/最小化中は本文を再構築せず、再表示時にdirtyを処理します。
  profilingの`render_visible_rows`はfilter後の総数であり、常駐row数とは区別します。

## 実装アーキテクチャ
- 管理ページが開き、`LeftPanelMode::TaskList` かつ非最小化のとき表示
- `crates/bevy_app/src/interface/ui/panels/task_list/`：責務別に分割
  - `view_model.rs` - ゲーム状態と producer diagnostics を表示用 snapshot へ縮約
  - `presenter.rs` - WorkType → icon / label / description
  - `actions.rs` - capability の positive allow-list、live 再検証、owner 別 action adapter
  - `dirty.rs` - タスクリストと task summary の dirty source
  - `update.rs` - dirty gate、ページclamp、本文offset保持、必要時のみ再描画
- `crates/bevy_app/src/interface/ui/plugins/info_panel.rs` が `PreUpdate` の dirty 検知と state 更新、`Update` の管理ページ表示更新を束ねます。
- `crates/bevy_app/src/interface/ui/interaction/status_display/mode_panel.rs` が cached summary を読み、task summary 表示だけを差分更新します。
- `Designation` コンポーネントを持つエンティティをクエリし、関連コンポーネント（Blueprint, TransportRequest等）を参照して説明文を生成
- `task_list_visual_feedback_system` が `Interaction` と `TaskDashboardActionState.active_task` を監視し、`ui/list::apply_row_highlight` でホバー・選択ハイライトを適用
- `hw_ui::panels::task_list` が表示型、filter / sort、render、pure UI interaction を所有する

## インタラクション
- **ホバー**: 背景色がハイライト
- **クリック**: 管理ページを維持して当該Taskを選択し、行内操作欄を表示。カメラとpinは明示ボタンから変更する
- **選択状態**: `TaskDashboardActionState.active_task`に対応するアイテムに選択ボーダーと背景色が表示
- **優先度**: 許可された手動 Chop / Mine、ManualTransportRequest、DeconstructionOrderだけを `0 / 5 / 10` で上下する
- **キャンセル**: 原則は1回目で行内確認、同じ対象・種別の2回目でintentを発行する。Soul Spa搬入は情報パネルと共通の確認パネルを開き、独立した確定ボタンを使う。Floor / Wall はsite全体を対象にする

フォーカス行の `Button` と action bar の各 `Button` は sibling であり、nested Button にしません。
Pause / Modal capture 中も action intent reader は drain して拒否結果を返し、解除後に遅延適用しません。
選択変更、タブ変更、filter/sort 変更、capture 開始、共通枠の遷移・閉鎖、world replacement は保留中の確認を消去します。

### 操作 capability

| 対象 | Priority | Cancel |
| --- | --- | --- |
| 保存済み `PlayerIssuedDesignation` 付き、かつ `AutoGatherDesignation` のない Chop / Mine | 可 | Designation owner cleanup |
| `ManualTransportRequest` + fixed source | 可 | `hw_logistics` typed close API |
| Blueprint | 不可 | Blueprint 専用 cancellation lifecycle |
| Floor / Wall tile または対応 material request | 不可 | parent site 全体 |
| `DeliverToSoulSpa` + `TargetSoulSpaSite` + Constructing site | 不可 | `SoulSpaConstructionCancelRequest`をsite ownerへ発行 |
| player-issued `DeconstructionOrder` + canonical pending + claim未取得 | 可 | `DeconstructionCancelRequest`をowner finalizerへ発行 |
| Move、自動 gather、自動 request、GeneratePower、provenance 不明 | 不可 | 不可 |

表示時の capability はヒントに過ぎません。適用時に Entity generation、`WorkType`、owner marker、必要 component を
再確認し、stale / unsupported / pause / capture は simulation state を変更せず `TaskActionOutcome` にします。
Deconstruct cancelのUI adapterはorder/pending/worker componentを直接変更しない。canonical
`DeconstructionCancelOutcome`だけが成功・claim中・staleを確定し、dashboardの二段階確認はlive capabilityが
消えた時点で破棄するため、claim取得後に古い確認操作を再表示しません。
Soul Spa搬入行もrequestの`kind=DeliverToSoulSpa`、Bone、anchor、`TargetSoulSpaSite`が同じConstructing siteを
指す場合だけcancel可能とし、malformed requestから別siteを取消しません。通常時はownerの
`SoulSpaConstructionCancelOutcome`だけを通知し、Pause / Modal capture中は`TaskActionOutcome`でその場で拒否して
解除後へrequestを持ち越しません。
手動エリア指定が既存 auto-gather 対象を覆う場合は `AutoGatherDesignation` を外し、選択中 Familiar または
unowned manual task へ所有権を明示的に移してから `PlayerIssuedDesignation` を付与します。
操作結果だけを A2 通知の `ToastOnly` へ変換し、blocker の周期更新は通知履歴へ送りません。

## 検証と実機受入

状態判定、並び順、capability、owner cleanup、save/load、reset、latest-only map の構造上の有界性は決定的に再現できるため、
自動テストを受入の正本にします。手動操作で正しそうに見えることを、未実装の回帰テストの代わりにはしません。

- `python3 scripts/dev.py cargo -- test -p hw_ui task_list`: 状態ラベル / semantic color token、全 filter / sort、camera / pin、capture 後の持越し防止
- `python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 task_dashboard`: status adapter、action capability、live revalidation、Pause / capture 時の drain
- `python3 scripts/dev.py verify`: workspace 全体の unit / integration / clippy / docs gate

リリース前の実機 smoke check には次の 2 種類だけを残します。

1. 実 renderer 上で状態・priority の色、文字、action bar が読みやすく崩れていないこと。
2. 実 pointer で row と action button の hit-test が分離され、Pause / Modal が背後入力を遮断すること。

同一fixtureのdashboard hidden / visible / active-filterは、`task-dashboard` perf workloadで比較します。
fixed-step auditはsimulation checksumとAI work counterの一致、Captureは実renderer frame-timeと
`task_dashboard_cpu.csv`のUI system実CPU、Memoryはmeasure区間のallocator量とprocess最大RSSを所有します。
dashboard mode比較はhiddenのrender workが0、visible / active-filterの入力行数が同等、active-filterの表示行数が
減ることもfail-closedで検証します。Memory buildのframe-timeはallocator計数の擾乱を含むため性能値にしません。
採取・有効性契約は`docs/performance-profiling.md`を正本とします。

A3 の完了判断と受入履歴は
`docs/plans/archive/actionable-task-dashboard-plan-2026-07-19.md` §7 を参照します。
定量性能フォローアップの完了記録は
`docs/plans/archive/task-dashboard-performance-validation-plan-2026-07-20.md`を参照します。

## 関連ファイル（最終境界反映）

### `hw_ui` 側（実装本体）
- `crates/hw_ui/src/panels/task_list/types.rs` - `TaskEntry`, status/reason、filter/sort、action capability/state
- `crates/hw_ui/src/panels/task_list/render.rs` - `rebuild_task_list_ui`
- `crates/hw_ui/src/panels/task_list/interaction.rs` - focus、filter/sort、ハイライト、confirmation reset
- `crates/hw_ui/src/panels/task_list/work_type_icon.rs` - WorkType → アイコン/カラー/ラベル変換
- `crates/hw_ui/src/panels/menu.rs` - `menu_visibility_system`

### root shell（adapter）
- `crates/bevy_app/src/interface/ui/panels/task_list/mod.rs` - hw_ui re-export + ゲーム固有モジュール統合
- `crates/bevy_app/src/interface/ui/panels/task_list/view_model.rs` - スナップショット生成と summary 集計（ゲームエンティティクエリ）
- `crates/bevy_app/src/interface/ui/panels/task_list/dirty.rs` - dirty 検知システム（Designation 等の Changed 監視）
- `crates/bevy_app/src/interface/ui/panels/task_list/update.rs` - dirty gate 付きオーケストレーション（`Res<GameAssets>` 依存のため root 残留）
- `crates/bevy_app/src/interface/ui/panels/task_list/actions.rs` - live capability resolver、typed action outcome、owner別適用
- `crates/bevy_app/src/interface/ui/plugins/info_panel.rs` - task list の dirty 検知 / state 更新 / 管理ページ system 登録
- `crates/bevy_app/src/interface/ui/interaction/status_display/mode_panel.rs` - task summary の cached 描画

### 選択と詳細固定の分離

Task行の選択は `SelectedEntity` と `TaskDashboardActionState.active_task` を更新する。カメラとpinを自動変更しない。選択行には「現地へ」「詳細を固定」と利用可能な優先度/取消操作を表示する。pinなしで操作でき、pin済みの別対象の閲覧も維持できる。page/filter後に表示対象から外れたactive_taskと未確定取消を解除するが、生存中のpinは維持する。capture開始で未確定取消を解除し、world置換でactive_taskも初期化する。

### 条件の直接選択

Type/State/Priority/Workersは対応する選択肢一覧を開き、単一条件を直接選ぶ。選択肢は標準スクロール領域に収める。「全解除」は4条件とpageを初期化し、sort key/directionを保持する。選択肢を開閉するだけではfilter値とpageを変えない。Taskが0件でもtoolbarと全解除を残す。関連blockerからの操作は別途実装中で、現時点のラベルを操作可能とは扱わない。

### 関連対象への導線

選択行の「担当の詳細」は現在のManagedBy、「運搬の関連先」はTransportRequest.anchorに対応する情報パネルを固定する。anchorはrequest種別により用途が違うため、常に搬入先とは表記しない。inspect可能な型のlive Entityだけを表示し、実行直前にもtask種別・関連づけ・対象存在を照合する。集計blockerから特定のFamiliarを推測しない。カメラは移動しない。担当の情報にはpolicyが停止している作業種別とOperationへの案内を表示する。
