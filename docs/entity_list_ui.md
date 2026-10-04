# エンティティリストUI仕様

最終更新: 2026-09-16

## 概要
通常は非表示です。「管理」または上部の人数から右側の共通枠を開き、使い魔と魂を一覧します。全体の画面契約は [地図中心UI](ui-world-first.md) を参照してください。
表示内容は `EntityListViewModel` を経由して **100ms 間隔**で差分同期されます。

## パネル構成

主要用語の対照は次のとおり。同じ概念にはこの表記を使う。

| UI表記 | 日本語での意味 |
| --- | --- |
| 使い魔・魂 | エンティティ一覧（内部名 Entities / Entity List） |
| 仕事 | タスク一覧（内部名 Tasks） |
| Familiar | 使い魔 |
| Soul | ソウル（作業員） |
| Unassigned Souls | 未所属ソウル（使い魔に所属していない） |
| Unassigned task | 未割当タスク（担当が確定していない） |
| Task Area / Area | 担当範囲 |
| Inspect / Pin | 詳細を固定 |
| Focus | 現地へ（カメラ移動） |
| Clear pin | 選択を表示 |

Soul行は最小32pxの1段グリッド、上下padding各1px・行間margin1px、本文14px、補助12pxを初期値とする。名前と数値の基準位置を揃え、長い名前は名前の列内で折り返す。氏名も共通UIフォントで表示する。

### ヘッダー
- 「戻る」「閉じる」と「使い魔・魂」「仕事」のタブ
- 右側に最小化ボタン（`-` / `+`）
- 最小化時はEntities/Tasks両本文と検索行を非表示化し、パネル高さをヘッダー相当へ縮小
- 外枠の表示は `UiShellState`、本文と検索行は `LeftPanelMode` と `EntityListMinimizeState` を正本に制御する
- 管理非表示・仕事タブ・最小化中はSoul検索を隠して入力focusを解除する。検索文字列はタブ往復で保持する

### 検索バー（`EntityListSearchRow`）
- ヘッダー直下に `検索` ラベル + `EditableText` フィールド
- ライブフィルタ: 入力のたびに `EntityListSearchState.query` を更新し、VM 構築時に部分一致で絞り込み
- 折りたたんだ配下・未所属Soulも検索対象。一致するSoulがあるgroupだけ一時展開し、fold componentを変更せず、検索解除で元の開閉状態へ戻す
- 対象: 使い魔配下 Soul 行・未所属 Soul 行（名前 `SoulIdentity.name`、`str::contains` のそのまま部分一致）
- 最大 64 文字。Enter 確定は不要（検索は常時反映）
- `Escape`: 検索フィールド本体と `EntityListSearchState.query` を同時にクリアし、同フレームのゲーム側 Escape 処理へは伝播しない
- 検索中に `SoulIdentity.name` が変わった場合は structure dirty となり、結果へ即時に入る/消える

### 本文 (`EntityListBody`)
- 本文全体は単一の `EntityListScrollArea`（標準ScrollArea/Scrollbar）。検索とタブは外側に固定
- `FamiliarListContainer`（使い魔セクションの親）
- `未所属の魂` セクション（折りたたみ可能）
- `ホイールでスクロール` ヒント（必要時のみ表示）。本文外の通常レイアウト領域を確保し、最終行へ重ねない

### 使い魔セクション
- 左: 折りたたみボタン（▼/▶）
- 中央: 使い魔選択ボタン
- 見出しと同じ段の右: 使役数上限の増減ボタン（`-` / `+`、各32px）。使い魔名/人数/状態は中央の列内で折り返す
- 右端: 48pxの固定列に現地ボタン。見出しの折返しで操作列を押し出さない
- 表示形式: `{名前} 所属{現在}/{上限}` と日本語状態・`仕事ありN / 休息中M`。仕事はAssignedTaskあり、休息は仕事なしでIdleBehavior::Resting/ Sleeping。検索・折畳み前の全所属から集計する。監督状態を全員の作業中と読み替えない
- 展開時: 配下ソウル行を表示

### ソウル行
- 性別アイコン（`male.png` / `female.png`）
- 名前（ストレス色に連動）
- 疲労アイコン + 疲労%
- ストレスアイコン + ストレス%（高ストレス時は太字）
- Dream値（整数表示、`dream == 0` のとき枯渇色）
- 性別・名前・疲労・ストレス・Dream・作業・現地を同じ1段に揃える。管理幅400pxを使い、長名は名前列だけで折り返す。現地ボタンは右端48px列
- 配下の字下げは行幅に加算する外側marginではなく内側padding。Scrollbarのために本文右側にも余白を確保する
- 値の差分同期が参照する直接childrenの順序は維持し、グリッド配置のみで見た目を分ける
- 名前・数値・作業名は共通UIフォントと16px行高を使用し、アイコンを含む各セルを縦中央へ揃える。文字サイズは名前14px・補助12pxを維持する
- タスクアイコン＋作業名。AssignedTaskを網羅分類し、給水・発電・解体・移設・精製・骨回収を区別する。将来のvariant追加時はmatchの更新が必要

## サイズとリサイズ
- 初期幅: 使い魔・魂は `400px`、仕事タブは比較用の `480px`。タブ切替と上部HUDからの直接遷移で幅を同期する
- 初期高さ: `380px`。「戻る・タブ・最小化・閉じる」を1行に統合し、本文の表示量と14px/12pxの文字サイズを維持する
- 最小高さ: `220px`
- 高さスナップ: `20px` 刻み
- 上下端から `10px` 以内でドラッグすると縦リサイズ開始
- 上端/下端どちらもドラッグ可能
- カーソルは `NsResize` に切り替え
- 最小化中・管理非表示中はリサイズ無効
- UI倍率とウィンドウ寸法に合わせて展開高さ・上端をclampする。capture中やcursor不在でも寸法を補正し、最小化高さは維持する
- 下端はモード案内の実際の上端も考慮し、折り返した案内がTasksのページ操作欄や一覧末尾を覆わないようにする。案内の描画寸法をUI座標へ変換して使用する
- リサイズ入力はUI倍率で座標を変換し、表示とドラッグ量の単位を合わせる

## 同期方式（実装）

`EntityListDirty` を **structure dirty / value dirty** に分離し、行の増減・並び替え（重い再構築）と
バイタル値の更新（軽量なテキスト差し替え）を別 system・別 run_if で処理する。

折りたたみマーカー`SectionFolded` / `UnassignedFolded`の追加・変更と削除の両方を構造変更として検知する。
展開はマーカー削除で行うため、削除通知も毎回消費し、pause中でも行を再生成する。

Soul名は単語が列幅を超える場合に文字単位で折り返す。長い英字名でも隣の疲労値と重ならず、行は内容に応じて高さを確保する。

2026-09-19のR04／R08実機受入では、1920×1080／UI scale 1、Intel Arc MTL／Vulkan／X11 under Waylandでportal入力の13 checkpointを確認した。解体・発電・給水の同じSoulについて一覧／詳細／Tooltipの意味が一致し、改名・検索／解除・折りたたみ／再展開と、通常simulationによる疲労25→26%の同一row更新が成立した。全13画像の可読性、長い名前の折返し、独立verifierと無警告ログも確認済み。この結果はIME composition、他viewport／DPI入力、全UI操作や性能改善率の受入を含まない。

- `build_entity_list_view_model_system`（run_if: `needs_structure_sync() || needs_value_sync_only()`）
  - `current/previous` スナップショット構築。構造・値どちらの変化でも VM を作り直す
- `sync_entity_list_from_view_model_system`（run_if: **`needs_structure_sync()` のみ**）
  - 使い魔セクションを差分同期（追加/削除/折りたたみ/ヘッダーテキスト）
  - 未所属ソウル行をキー管理で差分更新（`EntityListNodeIndex.unassigned_rows`）
  - 表示順は `replace_children` でビュー順へ再整列
  - 行の生成時に値も設定するため、構造変化フレームでは値行 system は走らせない
- `sync_entity_list_value_rows_system`（run_if: **`needs_value_sync_only()` のみ**）
  - 生成時の`SoulRowNodes`がgender/name/fatigue/stress/dream/task icon/task labelの7 Entityを保持し、`hw_ui::list::values`がその参照で更新する。childの並び順には依存しない。生成と更新は`list/style.rs`を共有する
  - 既存行の `Text` / `TextColor` / `TextFont` / `ImageNode` を in-place 更新
  - **代入前に現値と比較し、変化した項目だけ書き込む**（`get_mut` の DerefMut を避けることで、値が変わらない vitals 更新で `Changed` が立って UI が再レイアウトされるのを防ぐ）
  - 二つの run_if は排他（structure 変化フレームは全再構築側が値も含めて処理する）
- 検索入力の同期は `EditableText` の pending edit 適用後（`EditableTextSystems` 後）の `PostUpdate` で行う
  - 検索値が変わった場合のみ `EntityListDirty::mark_structure()` を立てる
  - `last_applied` は同じ検索文字列で毎フレーム structure dirty を立て続けないための適用済み値

## インタラクション

### マウス
- 行ホバー: ハイライト
- 行クリック: 選択を更新して同じ枠の詳細へ移動。カメラ位置と既存pinを変えず、戻ると一覧を復元する
- 各行の「現地へ」: 現在位置を再取得してカメラ移動。選択/pinを変えず、対象消滅時は移動しない
- 選択中行: 左ボーダー + 選択色
- 一覧本文でホイール/スクロールバー: 使い魔・配下・未所属をまとめて縦スクロール
- ソウル行長押し（実時間0.2秒）でドラッグ開始
- 配属drag中は本文の上下28 UI px内で端scroll（240 UI px/秒）。capture・cursor離脱・releaseで停止
- 使い魔行へドロップで配属リクエスト送信
- 使い魔行の `-` / `+` クリック: 使役数上限を変更（`1..=8`）

### キーボード
- 管理ページの使い魔・魂一覧を展開中のみ巡回可能。`Tab`: 次の候補を選択（cameraは移動しない）
- `Shift + Tab`: 前の候補を選択
- 候補はViewModelのFamiliar→配下Soul→次のFamiliar→未所属の表示順。検索/折りたたみ後の対象全体を双方向循環し、移動先の行へscrollする
- active `PlayMode` 中は World action への漏れを防ぐため Tab 巡回を行わない。TaskArea 編集中も Tab は処理しない
- テキスト入力フォーカス中（検索バー・リネーム等）は `Tab` 巡回を含むゲーム keybind を抑止（`UiInputState::text_input_blocks_keybinds`）
- `Tab` / `Shift+Tab` の方向は resolver の `ListNext` / `ListPrevious` で確定し、consumer は raw keyboard を再読しない

## 補助表示
- 一覧本文全体がオーバーフローした時のみスクロールヒント表示
- `IgnoreScroll` によりヘッダー要素はスクロール対象から除外

## 入力ガード
- 管理を閉じるとpending drag・ghostを破棄し、当該検索focusを解除する。
- `UiInputBlocker` + `UiInputState.pointer_over_ui` でリスト上のワールド入力を抑止する。hover は UI 自身の行クリックを止めない
- Modal/Pause の `world_input_captured` 中は行クリック、長押し drag、resize、minimize、section toggle、task tab、rename を開始・継続しない
- capture 開始 frame は Entity List の ghost、pending/active drop、resize edge を reset し、capture 中の release で squad request や panel resize を確定しない
- スクロール領域上のホイール入力はリスト優先
- `EditableText` フォーカス中は `text_input_focused` / `text_input_consumed_keyboard` によりゲーム側 shortcut（WASD パン、resolver action、Tab 巡回、Ctrl+C/V/Z/Y 等）を抑止
- Escape でフォーカス解除した同フレームは `text_input_consumed_keyboard` latch によりゲーム Escape 処理へ伝播しない
- `ResolvedInputFrame.pointer_selection_suppressed()` が true の frame は、world click と同様に Entity List
  の行選択・長押し drag を開始しない。Familiar command consumer は resolver が frame 開始時に保存した
  Familiar Entity を使うため、同時 click で command 対象が入れ替わらない。

## Familiar 設定の domain 同期

- 使役数上限の `-` / `+` は target Entity と `AdjustMaxControlledSoul { delta: i8 }` を持つ
  `UiIntent::ApplyFamiliarSettingsFor` を発行します。Entity List は
  `FamiliarOperation` やヘッダー text を直接変更しません。
- root adapter は foreground / pause / modal と live target を再検証し、
  `FamiliarSettingsChangeRequest` へ変換します。domain consumer は次の非 pause Logic 冒頭で
  operation と必要な roster 解放を一括 commit します。
- 表示は commit 済み durable state から通常の100ms value syncで更新します。同値 patch は
  component を書き戻さず、stale / missing / pause-modal は typed outcome になります。
- Operation dialog も同じ request / outcome 経路を使うため、Entity List と別の writer や
  楽観的表示状態を持ちません。

## Familiar Operation dialog

- Familiar のコンテキストメニューにある `Open Operation` は、受理された opener とクリック対象が同じ
  live Familiar であることを確認してから対象を latch します。開いた後の `SelectedEntity` や
  InfoPanel の pin 変更では対象を切り替えません。
- 中央ダイアログは幅 `640px`（viewport の最大92%）、高さは viewport の88%を上限とし、本文だけを
  標準 scrollbar 付きで縦スクロールします。
- 疲労閾値、最大使役 Soul 数、`Enable all` / `Disable all`、`WorkType::ALL` の安定順に並ぶ
  16行の許可設定と `Low / Normal / High` を編集します。全禁止は有効な待機方針であり、警告文を表示します。
- Pause またはより優先度の高い modal が前面にある間、pointer 操作は発生元で抑止します。
  synthetic intent も domain request を保留せず `Rejected(PausedOrModal)` で終端します。
- `Close` / `X` / `Esc` は target と scroll を同時に消去します。target が stale、または durable
  operation / policy が欠落した場合も自動で閉じ、scroll を先頭へ戻します。
- world replacement では `OperationDialogState`、target、scroll を default 化し、static root を
  同期的に `Display::None` へ戻します。旧 world の Entity や表示を次フレームへ持ち越しません。

## 主な関連ファイル（最終境界反映）

### root shell（adapter）
- `crates/bevy_app/src/interface/ui/list/mod.rs` - イベント受付、interaction/system 登録
- `crates/bevy_app/src/interface/ui/list/view_model.rs` - ゲームエンティティ → ビューモデル変換（検索フィルタ含む）
- `crates/bevy_app/src/interface/ui/list/change_detection.rs` - 変更検知トリガ（DamnedSoul/Familiar Changed 監視、検索中の `SoulIdentity` 変更は structure dirty）
- `crates/bevy_app/src/interface/ui/list/sync.rs` - `sync_entity_list_from_view_model_system` / `sync_entity_list_value_rows_system`（hw_ui sync helpers の thin shell）
- `crates/bevy_app/src/interface/ui/plugins/entity_list.rs` - 検索 sync を `EditableTextSystems` 後に登録
- `crates/bevy_app/src/interface/ui/list/drag_drop.rs` - ドラッグ&ドロップシステム（`DragState` 型は hw_ui）
- `crates/bevy_app/src/interface/ui/list/interaction.rs`, `interaction/navigation.rs` - 行クリック・Tab 巡回・target 付き `UiIntent` 発行（`FamiliarOperation` 直接更新は行わない）
- `crates/bevy_app/src/interface/ui/interaction/intent_handler.rs` - `UiIntent` dispatcher
- `crates/bevy_app/src/interface/ui/interaction/intent_context.rs` - `UiIntent` 処理が共有する `SystemParam` / query 集約
- `crates/bevy_app/src/interface/ui/interaction/handlers/familiar_settings.rs` - dialog/list button 共通の foreground / target 再検証と `FamiliarSettingsChangeRequest` 変換

### `hw_ui` 側（移設済み）
- `crates/hw_ui/src/list/models.rs` - ビューモデル型・`EntityListNodeIndex`・`FamiliarSectionNodes`
- `crates/hw_ui/src/list/values.rs`, `style.rs` - typed node経由の差分値更新と共通表示規則
- `crates/hw_ui/src/list/spawn.rs` - `spawn_familiar_section`, `spawn_soul_list_item_entity` 等（`dyn UiAssets` 経由）
- `crates/hw_ui/src/list/sync.rs` - `sync_familiar_sections`, `sync_unassigned_souls`（`dyn UiAssets` 経由）
- `crates/hw_ui/src/list/section_toggle.rs` - `entity_list_section_toggle_system`（折りたたみ純UI操作）
- `crates/hw_ui/src/list/dirty.rs` - `EntityListDirty` リソース定義
- `crates/hw_ui/src/list/drag_state.rs` - `DragState` 型
- `crates/hw_ui/src/list/minimize.rs` - `EntityListMinimizeState` + 最小化トグルシステム
- `crates/hw_ui/src/list/resize.rs` - `EntityListResizeState` + リサイズシステム
- `crates/hw_ui/src/list/selection_focus.rs` - `focus_camera_on_entity`, `select_entity_and_focus_camera`
- `crates/hw_ui/src/list/tree_ops.rs` - `clear_children`
- `crates/hw_ui/src/list/visual.rs` - `apply_row_highlight`, `entity_list_visual_feedback_system`
- `crates/hw_ui/src/list/search.rs` - `EntityListSearchState`、検索 sync system
- `crates/hw_ui/src/components.rs` - `OperationDialogState` と dialog marker
- `crates/hw_ui/src/setup/dialogs.rs` - Operation dialog の静的レイアウト、scroll editor、16行の操作部品
- `crates/hw_ui/src/intents.rs` - target 付き open intent と settings patch intent
- `crates/hw_ui/src/widgets/text_field.rs` - 検索バー用 `spawn_text_field`
- `crates/hw_ui/src/interaction/text_field.rs` - フォーカス枠・Enter/Escape・検索ライブ sync（Escape 検索クリア含む）
- `crates/hw_ui/src/list/mod.rs` - `hw_ui` 対外エクスポート
- `crates/hw_ui/src/setup/mod.rs` - `UiAssets` トレイト（`icon_arrow_right`, `icon_idle`, `font_soul_name` 含む）

### 境界横断
- `crates/hw_ui/src/components.rs`, `crates/hw_ui/src/theme.rs` は `hw_ui` API の再エクスポートシェルとして残す
- `crates/hw_ui/src/setup/entity_list.rs`（初期構築・検索バー行 spawn）は root shell 経由で呼び出される
