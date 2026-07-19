# タスクリストUI仕様

最終更新: 2026-07-20

## 概要
画面左側に表示される常駐パネルのモードの1つです（エンティティリストとタブ切替）。
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
作業種別ごとに異なるアイコンとテーマカラーを表示：
- **Chop**: 斧アイコン / `chop` 色
- **Mine**: ピッケルアイコン / `mine` 色
- **Build**: ハンマーアイコン / `build` 色
- **Haul / HaulToMixer / WheelbarrowHaul**: 運搬アイコン / `haul` 色
- **GatherWater / HaulWaterToMixer**: 運搬アイコン / `water` 色
- **CollectSand**: ピッケルアイコン / `gather_default` 色
- **Refine**: ハンマーアイコン / `build` 色

#### 2. 説明テキスト (12px)
作業種別と対象エンティティに基づいて自動生成されます。
- **建築**: `Construct [BuildingType]` (例: `Construct Wall`)
- **採掘**: `Mine Rock`
- **伐採**: `Chop Tree`
- **運搬**: `Haul [Resource]` (手動), `Haul [Resource] to Mixer` (自動)
- **水汲み**: `Gather Water`

優先度は `Normal = 0..=4`、`High = 5..=9`、`Critical = 10..` の共通 tier へ正規化します。
説明色、filter、sort、summary、変更ボタンはすべて `TaskPriorityTier::from_priority` を正本にします。

#### 3. ワーカーカウント (10px)
作業員が割り当てられている場合のみ `×N` を `text_secondary` 色で表示します。

#### 4. 状態

- `Working`: 現在の `TaskWorkers` が 1 件以上。
- `Blocked: <reason>`: applicable な全 producer / evaluator の current cycle が terminal rejection まで完走した場合だけ表示。
- `Evaluating...`: snapshot 不在、input revision 不一致、coverage 不足、割り当て要求 submit 後で worker 未反映など。

停止理由は `No eligible familiar`、`Missing resource or source`、`Unreachable`、
`Waiting for reservation`、`Waiting for dependency` の 5 分類です。UI は候補探索や経路探索を再実行せず、
Familiar delegation / Blueprint auto-build / wheelbarrow arbitration が通常処理中に公開した latest-only snapshot を読みます。
unowned Blueprint の `Build` だけが Familiar delegation と Blueprint auto-build の両 producer を必要とし、
`ManagedBy` 付き Blueprint は auto-build が適用外なので Familiar delegation だけで判定します。
各 blocker record は理由が参照した domain（task / roster / availability / topology）だけを鮮度判定に使います。
ただし producer cycle の evaluator coverage は roster stamp も照合し、作業可能 Soul / Familiar 構成が変わった旧 cycle は
`Evaluating...` に戻します。

### ツールバー

4 filter と 2 sort control を表示します。各ボタンは候補を順番に切り替えます。

- Type: 全種別または単一 `WorkType`
- State: All / Working / Blocked / Pending
- Priority: All / Normal / High / Critical
- Workers: All / Assigned / Unassigned
- Sort: Type / State / Priority / Workers
- Order: Asc / Desc

同値の最終順序は Entity index / generation で固定し、query や HashMap の反復順に依存させません。

## ビジュアルフィードバック

エンティティリストと統一されたホバー・選択ハイライトを提供します。

### 背景色
- **デフォルト**: `list_item_default`
- **ホバー**: `list_item_hover`
- **選択中**: `list_item_selected`
- **選択中+ホバー**: `list_item_selected_hover`

### 選択ボーダー
ピン留めされたエンティティに対応するアイテムに左 3px の `list_selection_border` 色ボーダーを表示します。

## 更新タイミング

- `PreUpdate` で `detect_task_list_changed_components` → `detect_task_list_removed_components` → `update_task_list_state_system` を順序固定で実行します。
- `LeftPanelMode::TaskList` 中でも、無変更フレームではスナップショット再生成と子 UI の再構築を行いません。
- 再生成トリガーは、`Designation` とその表示内容に影響する関連コンポーネントの `Added` / `Changed` / `Removed`、および左パネルのタブ切替です。
- `TaskListDirty` は `state_dirty` / `list_dirty` / `summary_dirty` の 3 つの責務に分かれます。
- `state_dirty` は snapshot と summary の再計算要求、`list_dirty` は左パネル本文の再描画要求、`summary_dirty` は画面上部 summary の更新要求です。
- `TaskListState.snapshot` は最新観測済みデータを保持し、未描画の `pending` snapshot は持ちません。
- diagnostics の cycle ID 自体は `TaskEntry` に含めず、表示内容が同じなら周期評価だけで UI を再構築しません。
- 左パネルを `TaskList` に切り替えたフレームは `mark_all()` で `state_dirty` / `list_dirty` を両方立て、最新スナップショットで再描画します（タスクデータが変わっていない場合も含む）。
- 画面上部の task summary は `TaskListState.summary_total` / `summary_high` を参照し、タスクリストと同じ dirty source を共有します。

## 実装アーキテクチャ
- `LeftPanelMode::TaskList` 時に表示
- `crates/bevy_app/src/interface/ui/panels/task_list/`：責務別に分割
  - `view_model.rs` - ゲーム状態と producer diagnostics を表示用 snapshot へ縮約
  - `presenter.rs` - WorkType → icon / label / description
  - `actions.rs` - capability の positive allow-list、live 再検証、owner 別 action adapter
  - `dirty.rs` - タスクリストと task summary の dirty source
  - `update.rs` - dirty gate 付きオーケストレーション、必要時のみ再描画
- `crates/bevy_app/src/interface/ui/plugins/info_panel.rs` が `PreUpdate` の dirty 検知と state 更新、`Update` の左パネル表示更新を束ねます。
- `crates/bevy_app/src/interface/ui/interaction/status_display/mode_panel.rs` が cached summary を読み、task summary 表示だけを差分更新します。
- `Designation` コンポーネントを持つエンティティをクエリし、関連コンポーネント（Blueprint, TransportRequest等）を参照して説明文を生成
- `task_list_visual_feedback_system` が `Interaction` と `InfoPanelPinState` を監視し、`ui/list::apply_row_highlight` でホバー・選択ハイライトを適用
- `hw_ui::panels::task_list` が表示型、filter / sort、render、pure UI interaction を所有する

## インタラクション
- **ホバー**: 背景色がハイライト
- **クリック**: カメラをそのタスク（対象エンティティ）の位置へ移動し、InfoPanel にピン留め
- **選択状態**: ピン留めされたエンティティに対応するアイテムに選択ボーダーと背景色が表示
- **優先度**: 許可された手動 Chop / Mine と ManualTransportRequest だけを `0 / 5 / 10` で上下する
- **キャンセル**: 1 回目で行内確認、同じ対象・種別の 2 回目で intent を発行する。Floor / Wall は site 全体を対象にする

フォーカス行の `Button` と action bar の各 `Button` は sibling であり、nested Button にしません。
Pause / Modal capture 中も action intent reader は drain して拒否結果を返し、解除後に遅延適用しません。
選択変更、タブ変更、filter/sort 変更、capture 開始、world replacement は保留中の確認を消去します。

### 操作 capability

| 対象 | Priority | Cancel |
| --- | --- | --- |
| 保存済み `PlayerIssuedDesignation` 付き、かつ `AutoGatherDesignation` のない Chop / Mine | 可 | Designation owner cleanup |
| `ManualTransportRequest` + fixed source | 可 | `hw_logistics` typed close API |
| Blueprint | 不可 | Blueprint 専用 cancellation lifecycle |
| Floor / Wall tile または対応 material request | 不可 | parent site 全体 |
| Move、自動 gather、自動 request、GeneratePower、provenance 不明 | 不可 | 不可 |

表示時の capability はヒントに過ぎません。適用時に Entity generation、`WorkType`、owner marker、必要 component を
再確認し、stale / unsupported / pause / capture は simulation state を変更せず `TaskActionOutcome` にします。
手動エリア指定が既存 auto-gather 対象を覆う場合は `AutoGatherDesignation` を外し、選択中 Familiar または
unowned manual task へ所有権を明示的に移してから `PlayerIssuedDesignation` を付与します。
操作結果だけを A2 通知の `ToastOnly` へ変換し、blocker の周期更新は通知履歴へ送りません。

## 検証と実機受入

状態判定、並び順、capability、owner cleanup、save/load、reset、AI work counter は決定的に再現できるため、
自動テストを受入の正本にします。手動操作で正しそうに見えることを、未実装の回帰テストの代わりにはしません。

- `cargo test -p hw_ui task_list`: 状態ラベル / semantic color token、全 filter / sort、camera / pin、capture 後の持越し防止
- `cargo test -p bevy_app@0.1.0 task_dashboard`: status adapter、action capability、live revalidation、Pause / capture 時の drain
- `python3 scripts/dev.py verify`: workspace 全体の unit / integration / clippy / docs gate

現時点で実機へ残すのは次の 2 種類だけです。

1. 実 renderer 上で状態・priority の色、文字、action bar が読みやすく崩れていないこと。
2. 実 pointer で row と action button の hit-test が分離され、Pause / Modal が背後入力を遮断すること。

同一 fixture の dashboard hidden / visible / active-filter capture による UI frame-time と実メモリ量は、
再現可能な perf harness の完成後にだけ実施します。現時点では正式な実機確認項目に含めません。

未整備の統合テストと実機手順の詳細は
`docs/plans/actionable-task-dashboard-plan-2026-07-19.md` §7 を正本とします。

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
- `crates/bevy_app/src/interface/ui/plugins/info_panel.rs` - task list の dirty 検知 / state 更新 / 左パネル system 登録
- `crates/bevy_app/src/interface/ui/interaction/status_display/mode_panel.rs` - task summary の cached 描画
