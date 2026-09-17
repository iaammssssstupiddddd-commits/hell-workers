# State管理システム

ゲームの操作モードをBevyのStatesシステムで一元管理します。

## PlayMode

プレイ中の操作モードを表すState。

| モード | 説明 | 遷移条件 |
|--------|------|----------|
| `Normal` | 通常操作（選択・移動） | デフォルト / Escキー |
| `BuildingPlace` | 建物配置中 | 「建てる」から種類選択 |
| `BuildingMove` | Plant建物（Tank/MudMixer）移動先指定中 | Moveボタンクリック |
| `TaskDesignation` | タスク指定中（伐採/採掘/Zone配置など） | 「作業を指示」／「範囲を設定」から選択 |
| `FloorPlace` | 床エリア配置中 | Floor操作開始 |

遷移: Normal ↔ BuildingPlace（建てる/Esc）、Normal ↔ BuildingMove（Moveボタン/Esc/右クリック）、
Normal ↔ TaskDesignation（作業を指示／範囲を設定/Esc）。Zone配置は
`TaskMode::ZonePlacement`、Zone解除は`TaskMode::ZoneRemoval`で表し、専用`PlayMode`は持たない。

## コンテキストリソース

各モードの詳細情報を保持するリソース。

| リソース | 型 | 用途 |
|----------|-----|------|
| `BuildContext` | `Option<BuildingType>` | 配置する建物の種類 |
| `MoveContext` | `Option<Entity>` | 移動対象の建物エンティティ |
| `MovePlacementState` | `Option<PendingMovePlacement>` | BuildingMove中の移動先一次確定（Tank companion再指定待ち） |
| `CompanionPlacementState` | `Option<CompanionPlacement>` | companion配置中の親アンカー・有効半径 |
| `ZoneContext` | `Option<ZoneType>` | 配置するゾーンの種類 |
| `ZonePlacementPreview` | `Option<ZonePreview>` | Stockpile/Yardの直前表示planとepoch。releaseで照合し、capture/取消/loadで破棄 |
| `TaskContext` | `TaskMode` | タスクの詳細（伐採/採掘/運搬など） |
| `StockpilePolicyRangeEditState` | `Option<StockpilePolicyPatch>` | Stockpile 方針の一回限りの矩形編集で、release まで保持する patch |

## TaskMode バリアント一覧

正本: `crates/hw_core/src/game_state.rs`。rootの`crates/bevy_app/src/systems/command/mod.rs`は
`TaskMode` / `TaskModeZoneType`を選択的にre-exportします。

| バリアント | 用途 | `Option<Vec2>` の状態 |
|:--|:--|:--|
| `None` | 通常モード（デフォルト） | — |
| `DesignateChop(Option<Vec2>)` | 伐採指示（矩形ドラッグ） | Some = ドラッグ中 |
| `DesignateMine(Option<Vec2>)` | 採掘指示（矩形ドラッグ） | Some = ドラッグ中 |
| `DesignateHaul(Option<Vec2>)` | 運搬指示（矩形ドラッグ） | Some = ドラッグ中 |
| `DesignateDeconstruct(Option<Vec2>)` | Ordersから完成建物を単体解体指定。release後は`None`へ戻って連続指定を待ち、右クリック/Escapeでmode終了 | Some = press済み・release待ち |
| `CancelDesignation(Option<Vec2>)` | 指示キャンセル（矩形ドラッグ） | Some = ドラッグ中 |
| `SelectBuildTarget` | Familiar建築用の予約variant。現在の`FamiliarBuild`はBlockedで遷移せず、実consumerと完了経路を同時実装した後だけ到達可能にする | — |
| `AreaSelection(Option<Vec2>)` | TaskArea 編集モード | Some = 新規矩形ドラッグ中 |
| `AssignTask(Option<Vec2>)` | 未割当タスクを Familiar に割り当て | Some = ドラッグ中 |
| `ZonePlacement(TaskModeZoneType, Option<Vec2>)` | Stockpile/Yard 配置 | Some = ドラッグ中 |
| `ZoneRemoval(TaskModeZoneType, Option<Vec2>)` | ゾーン解除（現行player導線はStockpileのみ。Yard variantは公開導線なし） | Some = ドラッグ中 |
| `FloorPlace(Option<Vec2>)` | 床エリア配置 | Some = ドラッグ中 |
| `WallPlace(Option<Vec2>)` | 壁ライン配置 | Some = ドラッグ中 |
| `DreamPlanting(Option<Vec2>)` | Dream 植林モード | Some = ドラッグ中 |
| `StockpilePolicyEdit(Option<Vec2>)` | Stockpile 方針の矩形編集 | Some = ドラッグ中 |
| `SoulSpaPlace(Option<Vec2>)` | Soul Spa 配置（2×2） | Some = ドラッグ中 |

`Option<Vec2>` は `None` = 待機、`Some(pos)` = pointer gesture進行中を示す。area系ではドラッグ開始位置、
Deconstructではpress位置として使い、release時に単一requestへ確定する。

## TaskDesignation の補足（TaskArea 編集）

`PlayMode::TaskDesignation` で `TaskContext = TaskMode::AreaSelection(...)` のとき、TaskArea 専用の連続編集モードとして動作します。

### AreaSelection の状態
- `TaskMode::AreaSelection(None)`: 待機（新規ドラッグ開始 / 既存エリア直接編集）
- `TaskMode::AreaSelection(Some(start_pos))`: 新規矩形ドラッグ中

### 遷移ルール
- 使い魔を左クリック→詳細上部「作業範囲を指定／変更」、または「作業を指示」→「使い魔の作業範囲を指定 / 変更」で `TaskMode::AreaSelection(None)` に遷移。管理一覧から詳細へ進んでも利用できる
- 詳細と右クリックは `SelectAreaTaskFor(Entity)` で対象を固定し、実行時に生存する使い魔か再検証する。固定詳細と別対象選択が併存しても表示対象を編集する。capture／pending overlay／RecoveryFailedでは開始しない
- 右上の300px編集パネルは対象と現寸法・入力案内・終了を常時表示し、履歴／コピー／3保存枠は「詳細操作を開く」で展開する。対象・world epoch・非表示への切替で閉じ、同対象の寸法更新では展開を維持する
- 無効操作もtooltipで理由を読める。実行payloadは既存revision/epoch/snapshot照合を通し、見出し・余白を含めてworld pointerを遮断する。「編集を終了」は適用済み範囲を保持し未確定dragだけrollbackする
- 適用後はデフォルトで `TaskMode::AreaSelection(None)` を維持（連続編集）
- `Shift + 左ボタンリリース` で適用と同時に `PlayMode::Normal` へ復帰
- `Esc` で `PlayMode::Normal` へ復帰

### 入力補足
- `Tab` / `Shift + Tab` は `PlayMode::Normal` かつ管理ページの使い魔・魂一覧を展開中の巡回だけに使い、Areaモードを含む active mode 中は処理しない
- `Ctrl + Z / Y`（および `Ctrl + Shift + Z`）で TaskArea の Undo/Redo を行う
- Area shortcut と適用時の Shift 判定は frame-local resolver の exact chord / modifier snapshot を使い、raw keyboard を再読しない

## MenuState と Architect サブメニュー

`MenuState` Resource が建築・作業・範囲・Dreamのサブメニューを管理する。管理／表示／詳細は `UiShellState`、システムメニューは `SystemMenuState` が所有する。

サブメニューは `fit_submenus_to_viewport` で画面内に収め、表示中のModeText（下部tips）の実測上端から4px上に下端を配置する。残りの高さを最大高に使い、UI倍率や案内の折り返しが変わってもtipsを覆わない。

| バリアント | サブメニュー |
|:---|:---|
| `Hidden` | 全サブメニュー非表示 |
| `Architect` | 絵・名前・用途付きの全12種類カタログ |
| `Zones` | ゾーンメニュー |
| `Orders` | 命令メニュー |
| `Dream` | Dreamメニュー |
| `Settings` | 設定モーダル（中央オーバーレイ。Architect 等サブメニューは非表示） |

### 建築カタログ

「建てる」は全12種類を最大2段で表示する。画像・名前・用途から直接選べ、種類選択まで2操作以内に到達する。カテゴリは任意の絞り込みで、再選択すると「すべて」へ戻る。種類を選ぶとカタログを閉じ、地図上の配置と短い案内へ切り替える。必要資材は配置プレビューのdomain値を使い、総在庫から調達可能とは推定しない。

### ArchitectCategoryState

`ArchitectCategoryState(Option<BuildingCategory>)` Resource が現在展開中のカテゴリを保持する。

| 値 | 意味 |
|:---|:---|
| `None` | 全種類を表示（すべて） |
| `Some(Structure)` | Structure の建物列を表示 |
| `Some(Architecture)` | Architecture の建物列を表示 |
| `Some(Plant)` | Plant の建物列を表示 |
| `Some(Temporary)` | Temporary の建物列を表示 |

書き込み元: `UiIntent::SelectArchitectCategory`を単一consumer
`handle_ui_intent`（`crates/bevy_app/src/interface/ui/interaction/intent_handler.rs`）が適用。
Architect以外のMenuStateへ遷移すると `menu_visibility_system` がカテゴリを初期化する。

## IndoorLightRuntime（非永続）

`IndoorLightRuntime`は`Initializing → Available | Unavailable`を持つroot-owned Resourceである。dirty inputが正常にcollect/rebuildされた場合だけ`Available`としてCPU snapshotを公開する。map外、重複occlusion、invalid emitter/mount、P03 diagnosticでは`Unavailable`となり、内部に前回snapshotが残っていてもreaderへ返さない。

dirtyはtopology、typed emitter/power、Room maskの3系統をcoalesceする。`RoomMaskSignature`はtile membershipが変わった時だけrevisionを進める。入力checksumが変わった時だけinput revision、公開field bytesが変わった時だけoutput revisionが進み、入力不変Updateはcollect/rebuildしない。このResourceはsave/Reflect対象外である。

world replacementではrootの`lighting-runtime` reset hookがsnapshot、pending input、revision、published epochを消去して`Unavailable`へ遷移する。rehydrate後に公開するsnapshotは`WorldEpoch`でtagし、epoch-aware readは現在epochと一致する場合だけ返す。`RecoveryFailed`ではlighting transaction自体を停止し、successful normal / rollback / recovery-onlyの`lighting.wake`だけが次Updateの再構築をarmする。durableな正本はOutdoorLamp rootの`LightingFixtureMount`であり、このResourceやruntime-only `RadialLightEmitter`ではない。

## HelpPanelState と HelpPauseGuard

Helpは背景メニューを保持するため`MenuState` variantにせず、`hw_ui::help::HelpPanelState`で
`open`と`active_topic`を管理する。表示済みtopicと読書位置を保持し、loadでリセットする。検索は見出し・本文のAND一致で絞り込む。

rootの`HelpPauseGuard`は「Helpが実行中の時間をpauseしたか」だけを保持する。通常時から開いた場合は
`paused_by_help=true`としてpauseし、close時にunpauseする。すでにPause中なら`false`のまま時間を変更せず、
close後もPauseを維持する。Help open/closeは`PlayMode`、`TaskMode`、`MenuState`、
`ArchitectCategoryState`を変更しない。

## SaveRecoveryMode

`SaveRecoveryMode`はBevy `State`ではなく、save transaction coordinatorが所有するtrust Resourceである。

| 値 | 意味 |
|:--|:--|
| `Healthy` | 通常のsave/load transactionを許可する |
| `RecoveryFailed` | live apply後のrollback自体が失敗し、worldを信頼できない。virtual timeをpauseし、saveと通常loadを拒否する |

`RecoveryFailed`中も通常のF9/normal catalog loadは拒否する。foreground recovery catalog が現在の
dialog sessionを所有する`RecoveryCatalog` originを作った場合だけ、incomingをfull preflightしてrollback
snapshotなしで置換するrecovery-only経路へ送る。通常F9やraw UI payloadからこのoriginを作れない。
この間はsave、autosave、通常loadとworld-mutating UI intentを拒否し、既に開いているforeground surfaceを
閉じる安全なintentとrecovery catalog操作だけを通す。再失敗時はpaused fail-closedを維持し、成功時だけ
`Healthy`へ戻すが自動unpauseしない。

## 共通仕様

### Escキーによるキャンセル

- 最前面 overlay がある場合は `Save / Load / Recovery catalog（確認を含む） → Help → Settings → Pause → OperationDialog`
  の優先順で、その overlay だけを閉じる。catalog確認のEscapeは親catalogへ戻り、通常catalogの次のEscapeだけが
  catalogを閉じる。Recovery catalogはRecoveryFailed中に閉じて通常world操作へ戻る経路を作らない。HelpのEscapeは
  Helpが所有したpauseだけを解除し、システムメニューのEscapeはメニューを閉じて同メニューが所有するpauseだけを解除する。
  いずれも背景mode stateは変更しない。
- overlay がない active owner（non-Normal `PlayMode`、non-`None` `TaskMode`、pending non-Normal 遷移）では
  共通 `ActiveModeCleanupParams` を通り、`Normal` を予約すると同時に
  `BuildContext` / `MoveContext` / `MovePlacementState` / `CompanionPlacementState` / `ZoneContext` /
  `TaskContext` と `MenuState` を初期化する。
- AreaEdit drag は元の `TaskArea` / `Destination` / `ActiveCommand` を復元する。Dream preview seed、Zone
  removal preview、Stockpile 方針の保留 patch、building move の未確定 state も同じ cleanup で破棄する。
- `PlayMode::Normal` でメニューだけが開いている場合はメニューだけを閉じる。次に右の共通枠を `WorkspaceBack` で戻る／閉じる。どちらもない状態でシステムメニューを開く。Idle / PatrolはIまたは対象のcontext menuで操作する。
- keyboard shortcut と UI の mode 切替は同じ active-owner predicate と cleanup helper を使い、current
  state がまだ Normal の pending 遷移 frame も含めて旧 owner state を残さない。

### Modal/Pause capture開始時のrollback

- overlay open は mode cancel ではない。Helpを含め、`PlayMode` と BuildingPlace / BuildingMove /
  companion / SoulSpa の placement state は維持し、capture 中の新しい world click だけを止める。
- capture の false→true frame では、AreaEdit の active drag を開始前の `TaskArea` / `Destination` /
  `ActiveCommand` へ戻し、Designation / Area / Assign / Zone / Floor / Wall / Dream のドラッグ中 variant を
  同じ mode の待機状態へ戻す。単体Deconstructのpress済みgestureもorderを発行せず待機状態へ戻す。
  Dream preview seed と Zone removal preview も破棄するが、SoulSpa の
  placement state と release 済みの history、assignment、`pending_dream_planting` は変更しない。
- `StockpilePolicyEdit(Some(start))` も capture 開始時は `StockpilePolicyEdit(None)` へ戻すが、
  `StockpilePolicyRangeEditState.patch` は保持する。overlay を閉じた後、同じ patch で矩形選択をやり直せる。
- Entity List の drag ghost / pending drop / active drop / resize edge も同じ latch で reset する。

### エリア指定中のカメラ入力

- Designation / Deconstruct / Area / Assign / Zone / Floor / Wall / Dream / Stockpile 方針編集の各 task mode は、左ボタンの press から
  release frame まで一次ポインタを指定 gesture に予約する。待機中の `TaskMode::* (None)`、同 frame に
  mode を始める Familiar action、通常 selection からの TaskArea border press を開始条件に含める。
  一度得た claim は Escape/capture 等で owner state が先に変わっても release まで維持するため、drag の
  開始時にも途中にも `PanCamera` は動かない。
- 左ボタンの release edge が消えた次 frame には `PanCamera` を再び有効化する。したがって mode 待機中は
  camera 操作を続けられ、設定の `camera_mouse_pan_enabled` も上書きしない。

### run_if条件

モード限定システムは `.run_if(in_state(PlayMode::BuildingPlace))` のようにステートでゲートする。`OnEnter` / `OnExit` でモード遷移時の初期化・クリーンアップを実装する。

## 関連ファイル

- `crates/hw_core/src/game_state.rs` - `PlayMode`、`TaskMode`、`TaskModeZoneType`の正本
- `crates/bevy_app/src/app_contexts.rs` - `BuildContext`、`MoveContext`、`ZoneContext`等のroot app context
- `crates/bevy_app/src/plugins/game.rs` - PlayMode State登録と game system set の実行条件
- `crates/bevy_app/src/input_actions/{bindings.rs,cancel.rs,capture.rs}` - context 別 Escape 解決、共通 owner cleanup、Modal/Pause capture rollback
- `crates/bevy_app/src/interface/ui/{help_content/,help_controller.rs}` - Help catalog、accepted open、可逆pause
- `crates/bevy_app/src/interface/ui/interaction/handlers/` - ボタンによる状態遷移と共通 cleanup 呼び出し
- `crates/bevy_app/src/systems/command/zone_placement/` - zone_placement（ZoneContext使用）

### Ordersの担当と結果

伐採・採掘・運搬・Area開始時は選択済み使い魔、なければ既存の担当範囲なし優先順で担当を選ぶ。使い魔不在なら既存モードを変更せず理由を重要通知する。Deconstructはこの自動選択条件へ加えない。伐採・採掘・運搬のmode表示へ担当名を加え、release時にdesignation ownerが実際に返す対象・適用・受入先なし・担当なし件数を履歴へ通知する。Areaは配属処理が返した件数を通知する。

### モード案内と画面内配置

Normal + TaskMode::Noneでは案内を隠す。操作中のモード表示を下部のボタン列の上へ分け、最大幅94%で折り返す。配置・移設・範囲指定ごとに次の入力と終了方法を併記する。床・壁のdrag中は既存preview ownerが作成したAreaPlacementPlanから採用・除外数と必要資材を表示する（除外セルは費用へ含めない）。床はBone/Mud、壁はWood/Mudのdomain定数を使い、debug即時壁では資材不要と区別する。preview不能時・world reset時に要約を破棄する。
Architect / Zones / Orders / DreamのsubmenuはUI倍率とwindow寸法から位置・最大高さを補正し、長い本文は標準ScrollAreaで移動する。表示matrixの実機受入は [未検証事項と再検証条件](ui-world-first.md#未検証事項と再検証条件)で追跡する。

## 時間停止とシステムメニュー

時間のSpace/1〜4操作はメニューを開かない。`SystemMenuState`は独立した非永続Resourceで、
`ToggleSystemMenu`を右上の「メニュー」／未処理Escapeから受ける。実行中から開く場合だけ相対速度を記録し、
閉じるとその速度へ戻す。元からpausedなら閉じてもpaused。Help/Settingsを重ねてもメニューのpause所有権を移さない。
loadのUI resetとRecoveryFailedで所有権を破棄する。内部の`PauseMenu` / `InputOverlay::Pause`名はシステムメニューのcaptureを指す。

停止中は閲覧・選択・カメラ、TaskArea指定/移動/resize/履歴、新規Chop/Mine、既存Stockpile単体policy、完成Door lockを許可する。
`UiIntent::allowed_while_paused`をボタン受理・表示とroot consumerで共有し、keyboard/domain側でも再検証する。
建築/移設、Haul、Zone作成/解除、Dream、Soul配属、作業取消、範囲policy、slots/power priority等は拒否する。
無効ボタンは減光しTooltipに「停止中は利用不可」を示す。Familiar settingsとSoul Spa取消の既存Paused outcomeは維持する。
停止へ切り替える際の未許可modeは共通cleanupで終了し、再開時に保留操作を再送しない。

Familiar command・Area入力/履歴だけをInput後/Spatial前へ分離し、各ownerの許可判定後にその場で適用する。
AI・Actor・通常Spatial全体のpause gateは解除しない。選択用のSoul/Familiar/資源/障害物/Stockpile/Designation索引は
PreUpdateで差分更新し、停止直前の変更とworld置換後の選択を最新化する。TaskArea更新はDestination/ActiveCommandと
既存未配属designationのManagedBy/Priorityまでで、actor座標は動かさない。新規採取は既存Designation/TaskSlots/ManagedBy/workerを上書きしない。
Door requestは単一のPostUpdate ownerでdoor/mapを更新してからLastのsaveに渡す。表示更新より先に適用し、Light Fieldは次Updateで再構築する。

## 任意の操作ガイド

Helpの「操作ガイドを開始」はHelpとシステムメニューを閉じ、それぞれが所有するpauseだけを解除する。
root `work_guide::WorkGuide`は使い魔・確定TaskArea・範囲内の本人のPlayerIssuedDesignation・所属SoulのGather phaseを読む。
既存範囲/指定も利用でき、作業開始はCollecting、完了はDoneから確認する。採掘の成功despawnはDoneを先に読んで完了と判定し、
単なる対象消滅を成功扱いしない。使い魔/範囲/指定の消滅は必要な段階へ戻す。閉じる/スキップ/loadで終了し、保存schemaは増やさない。

採取完了後は担当範囲内の休息所Blueprintを明示選択し、その1件の木材搬入・施工・完成を8段階の案内で追う。
完成ownerの `publish_building_completed` がdomain Eventと別配送する `BuildingCompletedVisualMessage.blueprint_entity` で消滅前の施工元を照合する。ガイドreaderは生存確認より先に正式完成を確認し、取消・同座標への再建・別工事の完成を成功にしない。
この入門例は木材だけで作る休息所を使用し、床の養生工程ガイドは含まない。採取した資源個体と搬入資源個体の同一性は主張しない。
