# コンテキスト付き入力アクション解決 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `input-action-context-resolver-plan-2026-07-17` |
| ステータス | `Completed` |
| 作成日 | `2026-07-17` |
| 最終更新日 | `2026-07-18` |
| 作成者 | `Codex` |
| 関連提案 | `docs/proposals/gameplay-management-improvements-proposal-2026-07-17.md`（Track A1） |
| 関連Issue/PR | `N/A` |

> **計画境界**: 本計画は関連提案の **A1「コンテキスト付き入力アクション」だけ**を扱う。
> A2 の通知、A3 のタスク診断、キーバインド設定 UI、ゲームパッド対応は別計画とする。
> 離散キーボード入力の action 化に加え、Modal/Pause の open request 受理 frame から既存
> pointer/camera 経路へ共通 capture gate を適用するところまでを A1 の受入境界とする。ただし mouse gesture 自体は
> action 化しない。
> 現行コード、`docs/architecture.md`、`docs/state.md`、`docs/tasks.md`、
> `docs/save_load.md`、`docs/debug-features.md`、完了済み text-input 計画を
> 2026-07-17 時点で照合した。

## 0. 設計判断ログ

| ID | 論点 | 決定 | 理由 |
| --- | --- | --- | --- |
| D1 | 実装単位 | A1 だけを独立計画にする | 入力競合は他トラックの機能仕様や永続化に依存せず、先行して安全性を上げられる |
| D2 | 型の所有場所 | `bevy_app` の app-shell に `input_actions` module を置く | 解決時に `PlayMode`、`TaskMode`、UI modal、選択 Entity、debug 状態を横断する。単一 leaf crate のドメイン責務ではない |
| D3 | 対象入力 | project-owned の edge-triggered keyboard shortcut と修飾キー状態 | マウス gesture、`PanCameraPlugin` の held WASD、Bevy `FocusedInput<KeyboardInput>`、独立 `visual_test` binary は別経路のまま維持する。Modal/Pause の共通 gate だけは既存 pointer/camera 経路にも適用する |
| D4 | 出力 | 毎 `PreUpdate` で置換し、その frame の Update 終了まで読む非永続 `ResolvedInputFrame` Resource | Message の残存期間や複数 reader に依存せず、Logic/Visual/Interface の各既存 consumer が同じ frame snapshot を読める |
| D5 | 一意性 | 「1 frame 1 action」ではなく「1 physical chord 1 semantic action」と、異なる chord 間の明示 compatibility を保証する | 同じ chord の多義解決、同一 family の競合に加え、Familiar C + World Z のように別 family でも同じ state を変更する組合せを resolver で排他する |
| D6 | 修飾キー | `Ctrl` / `Alt` / `Shift` / `Super` を左右統合し、binding は完全一致させる | `Ctrl+V` が plain `V`、`Ctrl+Z` が plain `Z` としても発火する現状を止める |
| D7 | UI action | 既存 `UiIntent` に同義 action がある場合は bridge して既存 handler を再利用する | ボタンとキーボードの挙動を揃えつつ、旧「全 UI 操作の巨大 dispatcher」提案を復活させない |
| D8 | F5/F9 | `F5=Save`、`F9=RequestLoadGame` に予約し、Soul mask / extra light の keyboard alias は削除する | 両 debug 操作は DevPanel と環境変数から操作できる。別の隠し chord を増やさず player 契約を一意にする |
| D9 | 数字/B | AreaEdit の exact chord、非 pause かつ `PlayMode::Normal` の Familiar 選択、World の順で解決する | 現在の Familiar alias を通常時だけ維持しつつ二重発火を止める。Area/Placement 中の command 上書きと、pause 中に停止した Logic consumer への入力を防ぐ |
| D10 | Escape / overlay | pending/visible overlay を visual stack の `LoadConfirm -> Settings -> Pause -> OperationDialog` 順で解決し、overlay がない時だけ TextInput → active placement/designation → open menu → Normal の Familiar command とする | overlay open を受理した時点で `InputFocus` を clear し、背景 EditableText と modal close が競合しないようにする。Pause の Escape/Space は同じ TimeControl action と UiIntent owner に統一する |
| D11 | F9 の意味 | `UiIntent::RequestLoadGame` を通し、確認 dialog を開く | `docs/save_load.md` の契約と Pause menu の既存経路に合わせ、直接 load を開始するコード上のずれを直す |
| D12 | 永続化 | binding / context / resolved frame は save と `settings.ron` の対象外 | 再割り当ては別計画。初版は compile-time default binding と deterministic resolver に限定する |
| D13 | Modal/Pause の pointer 遮断 | visible `UiInputCapture` と、overlay open request を受理した frame の pending capture を合成して `UiInputState::world_input_blocked()` を作り、capture root を viewport 全体の blocking UI node にする | `Node.display` は前 frame の presentation state なので、それだけでは open frame を遮断できない。pending gate と Bevy UI picking の両方で world/background UI を閉じる |
| D14 | pause と frame lifetime | `InputContextSnapshot` に `simulation_paused` / `logic_shortcuts_enabled` を含め、`ResolvedInputFrame` を pause 中も毎 frame 置換する | pause 中に skip された Familiar/Area action を unpause 後へ遅延適用しない。Pause overlay 中は Escape/Space、Digit1-4、Save/Load だけを許可する |
| D15 | Dream の D 表示 | `D` binding は追加せず、未実装の tooltip shortcut 表示を削除する | `D` は `PanCameraPlugin` の右パンで使用中。虚偽の操作案内を残さず、再割り当ては別計画へ送る |
| D16 | alias の同時押下 | 異なる chord が同じ `InputAction` へ解決された場合は frame 内で 1 件へ重複排除する | `C` と `Digit1` の同時押下などで同じ consumer 副作用を二重適用しない。異なる action は D22 の compatibility を満たす時だけ維持する |
| D17 | TextInput の pointer 境界 | focus/latch は keyboard action と PanCamera を遮断するが、field 外への明示 click は global capture しない。例外として overlay open helper は `InputFocus` を同期的に clear する | 通常の field 外 click による focus 解除・selection は維持しつつ、overlay 背景の EditableText が raw `FocusedInput` を受け続ける経路だけを閉じる |
| D18 | 複数 action の順序 | context/family priority で整列する前に、binding table の cross-family compatibility で非可換 action を 1 件へ絞る | action vector の並び順は別 system 間の mutation 順を保証しない。B + F3 のような明示 compatible action だけを併存させる |
| D19 | capture 開始中の drag | `world_input_capture_started` を effective capture の false→true で立て、overlay request sync → Resolve → capture/focus/rollback/camera guard を `PickingSystems::Hover` より前の `PreUpdate` に固定する | visible `Node.display` の次 frame 同期を待たず、mouse pan observer と Update の pointer/Logic consumer より前に未確定操作を戻す |
| D20 | AreaEdit の成立条件 | exact AreaEdit chord は現在の `State<PlayMode>` が `TaskDesignation` かつ `TaskMode::AreaSelection(_)` の時だけ生成する | `TaskMode` と pending `NextState<PlayMode>` だけが先に変わった frame でも current state は Normal で、Logic consumer は `run_if(in_state(...))` により実行されない。消える action を生成しない |
| D21 | 同一 family の同時入力 | `SaveLoad`、`TimeControl`、`MenuToggle`、`FamiliarCommand`、`AreaEditCommand`、`CancelOrClose` は各 frame 最大 1 action とし、binding table の明示順位で決める | 現行の `if` / `else if` が持つ排他性を resolver 移行後も維持し、C+M、C+Escape、Space+Digit、Pause Escape+Digit、F5+F9 などの結果を deterministic にする。Pause の Escape/Space は `TimeControl`、Normal+Familiar の Escape toggle は `FamiliarCommand` family に含める。別 family の併存可否は D22 で決める |
| D22 | cross-family conflict | `OverlayTransition`、`SelectionOrMode`、`SimulationControl`、`ViewDebug` の conflict lane と compatibility table を追加し、同じ owner state を変更する異なる family は高い context priority の 1 action だけを残す | Familiar C+Z、AreaEdit Ctrl+V+B、Space+F5、F12+P の結果を consumer 登録順や mutable run condition に依存させない |
| D23 | selection snapshot | resolver は pointer selection より前に frame-start の `SelectedEntity` を読む。selection-dependent / context-mutating action がある frame は world と Entity List の selection mutation を抑止する | world click と Entity List click の schedule 差をなくし、resolver が判定した Familiar と後段 consumer が操作する Familiar を一致させる。通常 click の結果は次 frame の resolver から有効 |
| D24 | gesture 中の Save | `has_in_progress_gesture` 中は `SaveGame` を生成しない。overlay transition と SaveLoad は非 compatible とし、Space+F5 は transition だけにする | AreaEdit は release 前から persisted `TaskArea` を変更し、`Last` save が rollback より先に未確定値を保存できるため |
| D25 | debug snapshot | P/O の可否は resolver 時点の `DebugVisible` だけで決め、consumer 側の mutable `run_if(DebugVisible)` を外す | F12+P で F12 consumer が visibility を変えても、同 frame に解決済みの spawn actionを後段 gate が捨てないようにする |
| D26 | task area と camera の左ドラッグ競合 | Designation / Area / Assign / Zone / Floor / Wall / Dream mode では、左ボタンの press から release frame まで `PanCamera.enabled=false` とする。現在の `TaskMode` に加えて同 frame の Familiar resolver action と通常 selection の TaskArea border hit を先読みし、claim 後は owner が解除されても release まで保持する | Bevy 0.19 の `PanCamera` と task area gesture が同じ左ドラッグを読むため。後段で mode が始まる frame や途中 cancel に隙間を作らず、mode 全期間を無効化して指定間の camera 操作を失うことも避ける |

## 1. 目的

- 解決したい課題:
  - `ButtonInput<KeyCode>` を複数 system が個別に読み、同一入力を別の意味として同じ frame に処理している。
  - `GameSystemSet::Input -> Logic -> Visual -> Interface` の順序はあるが、後段 system も同じ
    `just_pressed` edge を読めるため、順序だけでは消費を表現できない。
  - 各 system に同じ text-input guard が散在し、新しい shortcut 追加時に modal、修飾キー、
    PlayMode、Familiar 選択条件の追従が漏れやすい。
- 到達したい状態:
  - physical chord を `PreUpdate` の project-owned resolver で一度だけ context 解決し、後段は
    `InputAction` だけを読む。
  - text input や modal が active な frame は、許可された action 以外を解決しない。
  - Modal/Pause の open request を受理した frame から、panel 外の world pointer/camera 操作を遮断する。
  - existing consumer のドメイン処理と `UiIntent` handler は維持し、入力解決だけを共通化する。
- 成功指標:
  - F5/F9、Digit1-4、B、`Ctrl+V`、`Ctrl+Z`、Escape の既知二重発火が 0 件。
  - binding table の各 context/priority で、同一 chord が複数 action へ解決されないことを
    pure test で網羅する。
  - text input または modal 中に World/Familiar/debug shortcut が発火しない。
  - Modal/Pause の open frame を含め、panel 外クリック、placement、selection、camera pan の背後発火が 0 件。
  - pause 中の Familiar/Area shortcut が unpause 後に遅延発火しない。
  - production `bevy_app` の project-owned keyboard edge 判定が resolver へ集約され、
    consumer が `just_pressed(KeyCode::...)` を直接読まない。
  - resolver の処理量が固定 binding 数に比例し、Entity 数や UI node 数に比例する全件処理を追加しない。

## 2. スコープ

### 対象（In Scope）

- `InputAction`、`InputChord`、`InputModifiers`、`InputContextSnapshot`、
  `ResolvedInputFrame` の追加。
- default binding table と context priority の一元化。
- `UiInputState.text_input_focused` / `text_input_consumed_keyboard` を最上位 gate として再利用。
- 現在実装済みの load confirm、Settings、operation dialog を modal context として扱う。
- Pause menu を world-input capture context として扱い、Escape/Space、時間、Save/Load の
  明示 whitelist 以外を抑止する。
- global UI shortcut、Save/Load、Familiar command、TaskArea edit、Entity List Tab、
  elevation、render/debug shortcut の移行。
- AreaSelection の Shift-held 判定を共通 modifier snapshot へ移行。
- 既存 `UiIntent` と既存ドメイン handler の再利用。
- binding/context/ordering の unit test と最小 integration test。
- `UiInputCapture`、`world_input_blocked()` と既存 selection/placement/pan-camera guard の統一。
- overlay open request の pending capture、`InputFocus` clear、visual stack と resolver priority の同期。
- cross-family compatibility と selection mutation suppression による、context-changing action の frame 内排他。
- FloorPlace、BuildingMove、companion placement を含む全 active mode の Escape cleanup 統一。
- 未実装で PanCamera と競合する Dream の `D` tooltip 表示の訂正。
- 入力仕様を説明する恒久ドキュメントの同期。

### 非対象（Out of Scope）

- A2 の toast/alert、配置拒否理由、Save/Load outcome UI。
- A3 のタスク停止理由と管理ダッシュボード。
- ユーザーによるキーバインド変更、binding の `settings.ron` 永続化、競合設定 UI。
- gamepad、アクセシビリティ preset、キー表示 glyph の自動切替。
- マウスクリック/ドラッグ/ホイールの action 化。既存 system には capture guard だけを追加する。
- `PanCameraPlugin` 内部の held key 入力の置換。既存 `pan_camera_world_input_guard_system` の enable 条件だけを共通 capture / pointer claim に合わせる。
- `FocusedInput<KeyboardInput>` を使う Bevy text editing の置換。
- `crates/visual_test` 独立 binary の入力体系変更。
- button、context menu、selection を含む全 UI 操作の単一 action enum/dispatcher 化。
- Familiar command、時間速度、TaskArea 操作そのもののゲーム仕様変更。

## 3. 現状とギャップ

### 3.1 現行 keyboard consumer

| 所有箇所 | 現行入力 | gate | ギャップ |
| --- | --- | --- | --- |
| `plugins/input.rs` | F3〜F9、F12 | text input | F5/F9 が Save/Load と競合する |
| `systems/save/state.rs` | F5/F9 | text input、Idle | debug toggle と同じ edge を読む。F9 は確認 dialog を通らない |
| `interface/ui/interaction/systems.rs` | B/Z、Space、Digit1-4、Escape | text input、Settings の数字だけ個別抑止 | Familiar command、AreaEdit modifier、modal と競合する |
| `systems/command/input.rs` | C/M/H/B、Digit0-4、Delete、Escape | text input、Familiar 選択 | B/Digit/Escape が global/cancel と同時発火する |
| `systems/command/area_selection/shortcuts.rs` | Ctrl/Alt + C/V/Z/Y/1-3 | text input、AreaSelection | global B/Z、elevation V、Familiar Digit と同じ raw edge を後段が再読する |
| `systems/command/area_selection/input/**` | Shift held | TaskMode/PlayMode | modifier 読み取りだけが raw keyboard Resource と結合している |
| `systems/visual/elevation_view.rs` | V | text input | `Ctrl+V` でも V として発火する |
| `interface/ui/list/interaction/navigation.rs` | Tab/Shift+Tab | text input | 個別 guard の追加漏れ余地がある |
| `plugins/interface_debug.rs` | P/O | DebugVisible、text input | debug context が resolver とは別管理 |
| selection / placement / area 系 mouse system | 左右 click、drag | `pointer_over_ui` | Modal/Pause panel 外では false になり、背後の world 操作が発火する |
| Entity List drag / resize / row selection | raw mouse、`Interaction` | 独自 `DragState` / resize state | fullscreen root だけでは進行中 gesture、ghost、同 frame selection mutation が止まらない |
| `pan_camera_world_input_guard_system` | held WASD/QE 等 | `pointer_over_ui`、text input | Modal/Pause panel 外で camera が動く。UI state 更新との PreUpdate 順序も未指定 |

`Logic` set は `Time<Virtual>` の pause 中に実行されない。一方、Input/Visual/Interface は実行されるため、
Familiar/Area action を保持型 Message にすると、consumer が止まっている間の入力が unpause 後に
読まれる危険がある。また `docs/state.md` は全 mode の Escape cancel を契約としているが、現行
keyboard handler は BuildingPlace / ZonePlace / TaskDesignation だけを処理し、FloorPlace と
BuildingMove は右クリックでしか終了できない。

### 3.2 既存の使える基盤

- `UiInputState` は `InputFocusSystems::Dispatch` 後の `PreUpdate` で同期され、
  focus が解除された同 frame も `text_input_consumed_keyboard` latch が保持される。
- `UiInputState.pointer_over_ui` と各 world mouse system の guard はすでに存在するため、
  capture flag を同 Resource に加えれば gesture 本体を移設せず遮断条件を統一できる。
- `GameSystemSet` は `Input -> Spatial -> Logic -> Actor -> Visual -> Interface` の順で chain 済み。
- `UiIntent` は menu、時間、Save/Load、mode 選択の既存 canonical handler を持つ。
- `PlayMode`、`TaskContext(TaskMode)`、`SelectedEntity`、`MenuState`、dialog marker から
  resolver に必要な context を導出できる。
- TaskArea shortcut はすでに modifier を明示しており、exact chord table へ移しやすい。

### 3.3 本計画で埋めるギャップ

```text
PreUpdate
  InputFocusSystems::Dispatch
    -> UiInputState text focus / consumed latch
  UiSystems::Focus
    -> RelativeCursorPosition / Interaction
    -> capture-opening UI request sync
  visible UiInputCapture query + pending capture request
    -> resolve_input_frame_system（frame-start selection を読む）
    -> begin_world_input_capture（InputFocus clear を含む）
    -> UiInputState world_input_captured / world_input_capture_started
    -> rollback_in_progress_gesture_system
    -> pan_camera_world_input_guard_system
  PickingSystems::Hover
    -> Pointer<Drag> observers

Update::GameSystemSet::Input
  handle_mouse_input / selection ingress
    -> action/capture による selection mutation suppression を先に確認
  input_action_to_ui_intent_system / root debug & visual consumers

Update::Logic / Visual / Interface
  existing consumer が ResolvedInputFrame を読む
    -> existing domain mutation / UiIntent handler
  existing pointer consumer
    -> UiInputState::world_input_blocked() で早期 return
```

- 物理入力の収集・modifier 正規化・context 優先順位を resolver に集約する。
- overlay request、visible capture、text focus を同じ PreUpdate snapshot に集約し、`Node.display` だけを
  open frame の正本にしない。
- action の処理本体は既存 owner に残し、巨大な中央 mutation dispatcher を作らない。
- binding table と action consumer ownership をテスト可能なデータとして固定する。

## 4. 実装方針（高レベル）

### 4.1 型と所有権

新規 `crates/bevy_app/src/input_actions/` を root app-shell の入力 adapter とする。

| 型/責務 | 所有候補 | 契約 |
| --- | --- | --- |
| `InputAction` | `input_actions/model.rs` | payload を持たない `Copy` enum。physical key 名を variant 名へ含めない |
| `InputActionFamily` / conflict lane | `input_actions/model.rs` | family 内 cardinality と、family を跨ぐ非可換 action の compatibility を分類する |
| `InputModifiers` | `input_actions/model.rs` | 左右 Ctrl/Alt/Shift/Super を統合した frame snapshot |
| `InputChord` | `input_actions/model.rs` | `KeyCode + InputModifiers`。exact match の単位 |
| `InputContextSnapshot` | `input_actions/context.rs` | text/modal/PlayMode/TaskMode/Familiar/debug の解決入力。保存しない |
| default binding table | `input_actions/bindings.rs` | context 条件、priority、chord、action の唯一の正本 |
| pure resolver | `input_actions/resolver.rs` | 同一 chord について最優先の 1 action だけを返す |
| `ResolvedInputFrame` | `input_actions/mod.rs` | actions と modifiers を毎 frame の PreUpdate で置換。Reflect/Serialize/RegisterType しない |
| `PendingWorldInputCapture` | `input_actions/context.rs` | capture-opening keyboard/UI request の overlay kind と opener をその frame だけ保持。open 前提を満たす request だけを記録し、save/load schemaへ入れない |
| Bevy system wiring | `plugins/input.rs` | PreUpdate resolver/capture と root-owned bridge/consumer の唯一の登録元。Familiar/Area/Tab/P/O 等の domain consumer 登録は owning plugin に残す |
| `UiInputCapture` / capture flags | `hw_ui::components` | modal/pause の visible marker と pending open requestを合成し、cursor 位置に関係なく world pointer/camera を遮断する game 非依存 state。`world_input_capture_started` は false→true の 1 frame だけ立つ。marker は viewport-size の blocking root に付け、背景 UI picking も止める |

`InputAction` 等の game 固有 model は `hw_core` / `hw_ui` へ置かない。`hw_ui` に追加するのは
game action を知らない capture marker/state だけとする。将来、別 app shell でも同じ resolver が
必要になった時点で pure model だけの移設を再評価する。`InputContextSnapshot` は resolver 呼び出し時に
既存正本から組み立てる一時値であり、別 Resource として保持しない。

### 4.2 SystemSet、selection snapshot、frame lifetime

```rust
app.configure_sets(
    PreUpdate,
    (
        InputPreUpdateSet::CaptureRequestSync,
        InputPreUpdateSet::Resolve,
        InputPreUpdateSet::CaptureTransition,
        InputPreUpdateSet::Rollback,
        InputPreUpdateSet::CameraGuard,
    )
        .chain()
        .after(InputFocusSystems::Dispatch)
        .after(UiSystems::Focus)
        .after(text_input_focus_sync_system)
        .after(update_ui_input_state_system)
        .before(PickingSystems::Hover),
);
app.configure_sets(
    Update,
    (
        InputResolutionSet::PointerIngress,
        InputResolutionSet::Consume,
    )
        .chain()
        .in_set(GameSystemSet::Input),
);
```

- `Resolve` は毎 frame の PreUpdate で必ず実行し、pause 中も `ResolvedInputFrame` を空から再構築する。
- capture-opening UI button の `Interaction` と keyboard action は同じ pending capture helper へ入り、
  `CaptureTransition` が `InputFocus` clear と selection/pointer suppression を確定し、`Rollback` が
  owner helper と deferred mutation を Update 前に適用する。
- pending capture は request frame だけ保持し、次 frame は visible marker へ引き継ぐ。open 前提不成立や
  handler no-op の request は pending を立てず、close frame は visible marker が消えるまで capture を維持する。
- `Consume` は root-owned debug/visual action と `UiIntent` bridge を処理する。
- Logic/Visual/Interface consumer は既存 `GameSystemSet` chain により Resolve より後に実行される。
- `ResolvedInputFrame` は frame を越えて action を保持しない。load reset hook や save schema へ追加しない。
- 同時に異なる chord が押された場合も、compatibility table で明示的に compatible な action だけを
  複数保持する。同一 chord、排他的 family、同じ conflict lane の非可換 action は各 1 個だけにする。
- alias chord が同じ semantic action へ解決された場合、`ResolvedInputFrame` はその action を 1 件だけ保持する。
- 排他的 family は `SaveLoad`、`TimeControl`、`MenuToggle`、`FamiliarCommand`、`AreaEditCommand`、
  `CancelOrClose` とし、同 family に複数 chord が来た場合は binding table の `family_priority` が高い
  1 action だけを残す。Debug toggle など composable family も compatibility table を通す。
- 複数 action は context priority、family priority、binding table の宣言順で安定整列するが、
  mutation 順序をこの並びだけに依存させない。非可換 action は consumer 到達前に 1 件へ絞る。
- resolver は world click / Entity List click より前の `SelectedEntity` を frame snapshot とする。
  selection-dependent または context-mutating action がある frame は全 selection ingress を抑止し、
  action がなければ click 結果を次 frame の resolver から有効にする。
- scheduling test で `Resolve -> CaptureTransition -> Rollback -> PointerIngress -> Consume` と、
  `CameraGuard -> PickingSystems::Hover` を明示する。
- `logic_shortcuts_enabled = !Time<Virtual>.is_paused()` とし、pause 中は Familiar/Area action を生成しない。
  frame snapshot の置換により、押して離した action は unpause 後へ残らない。

### 4.3 Context priority

priority は「active context を 1 個だけ選ぶ」のではなく、各 chord を誰が claim するかに使う。

| 優先度 | context | 規則 |
| --- | --- | --- |
| 1 | CaptureOverlay | pending/visible な overlay を `LoadConfirm -> Settings -> Pause -> OperationDialog` の visual stack 順で claim。Modal は Escape だけ、Pause は Escape/Space、Digit1-4、F5/F9 だけを許可する |
| 2 | TextInput | overlay がなく `text_input_blocks_keybinds == true` なら project shortcut を 0 件にする。Bevy text editing は raw input を継続利用する |
| 3 | ActiveMode | active placement/designation の Escape を claimし、owner cleanup と `MenuState::Hidden` を同じ action で行う。B/Z/Tab/Familiar と mode 非互換 action は block する |
| 4 | OpenMenu | PlayMode Normal で Architect/Zones/Orders/Dream が開いている時の Escape を claimし、menu だけを閉じる |
| 5 | AreaEdit | `logic_shortcuts_enabled`、現在の `State<PlayMode> == TaskDesignation`、`TaskMode::AreaSelection(_)` の全条件を満たす時だけ exact Ctrl/Alt chord を claim |
| 6 | FamiliarCommand | `logic_shortcuts_enabled`、`PlayMode::Normal`、Familiar 選択、TaskMode が None または Familiar command 系の時だけ C/M/H/B、Digit0-4、Delete を claim |
| 7 | World | PlayMode Normal の B/Z/Tab と、context-compatible な Space、Digit1-4、V、F5/F9 を claim |
| 8 | Debug | resolver snapshot の DebugVisible 条件付き P/O と、compatibility table で許可された function chord を claim。上位 context が block した場合は発火しない |

複数 overlay が同時に pending/visible になる異常状態でも、Escape と picking root の優先順位は
`LoadConfirm -> Settings -> Pause -> OperationDialog` として visual `ZIndex` と一致させる。
overlay open helper は request 受理時に `InputFocus` を clear し、visible 後の Escape が背景 text fieldへ
先に配信される状態を作らない。

Familiar-compatible な TaskMode は `None`、`DesignateChop`、`DesignateMine`、
`DesignateHaul`、`CancelDesignation`、`SelectBuildTarget` に限定する。
`AreaSelection`、Zone/Floor/Wall/Dream/SoulSpa 系では Familiar command を block し、
active operation の途中状態を別 shortcut で上書きしない。

### 4.4 Default binding contract

| chord | context | action | 備考 |
| --- | --- | --- | --- |
| F5 | World/Paused | `SaveGame` | TextInput/Modal inactive かつ `has_in_progress_gesture == false` の時だけ `UiIntent::SaveGame` へ bridge |
| F9 | World/Paused | `RequestLoadGame` | TextInput/Modal inactive で確認 dialog を開く。直接 `LoadRequested` にしない。save file 不在時は pending capture を開始しない |
| F5/F9 の debug alias | Debug | なし | Soul mask / extra light は DevPanel と環境変数へ一本化 |
| F3/F4/F6/F7/F8/F12 | Debug | 現行 render/debug action | 現行意味を維持 |
| Space | World/Paused | `TogglePause` | Pause の Escape と同じ `TimeControl` action。`UiIntent` へ bridge |
| Digit1/2/3/4 | World/Paused | `TimePaused/Normal/Fast/Super` | 非 pause の Familiar/Area が claim しない時。pause 中は再開操作として許可 |
| B/Z | World | `ToggleArchitect/ToggleZones` | exact unmodified chord。PlayMode Normal かつ pending mode transition なしの時だけ |
| C/M/H/B, Digit1-4 | Familiar | Chop/Mine/Haul/Build | 非 pause、PlayMode Normal、互換 TaskMode、Familiar 選択時はこちらを優先 |
| Digit0/Delete | Familiar | `CancelDesignation` | 現行意味を維持 |
| Escape | Modal/Pause/Mode/Menu/Familiar | cancel/close/resume/toggle command | Pause は Space と同じ `TogglePause` / `TimeControl` / UiIntent owner、Modal/Mode/Menu は `CancelOrClose`、Normal+Familiar は `FamiliarCommand`。各 family 内で 1 actionだけ |
| Ctrl+C/V/Z/Y | AreaEdit | copy/paste/undo/redo | plain C/V/Z/Y へ伝播しない |
| Ctrl+Shift+Z | AreaEdit | redo | Ctrl+Z より exact chord を優先 |
| Ctrl+Digit1-3 | AreaEdit | preset save | Familiar/Time action へ伝播しない |
| Alt+Digit1-3 | AreaEdit | preset load | Familiar/Time action へ伝播しない |
| V | World | `CycleElevation` | exact unmodified chord |
| Tab/Shift+Tab | World | list next/previous | PlayMode Normal のみ。text input 中は Bevy focus 側だけが処理 |
| P/O | DebugVisible | spawn Soul/Familiar | cursor world positionの計算と Message 発行は既存 consumer に残す |
| D | なし | なし | PanCamera の右パンを維持し、Dream tooltip の shortcut 表示だけ削除 |

同一 family に複数 chord が来た時の初版順位も binding table の契約に含める。

| family | 高い順の `family_priority` | 現行互換の根拠 |
| --- | --- | --- |
| SaveLoad | Save > RequestLoad | 同時要求時に destructive な load より save を優先 |
| TimeControl | Super > Fast > Normal > Paused > TogglePause | 現行の Space toggle 後に Digit1→4 の独立 `if` を適用する最終結果。Pause Escape は Space と同じ `TogglePause` alias |
| MenuToggle | Zones > Architect | 現行の独立 `if` が B→Z の順で適用する最終結果 |
| FamiliarCommand | Chop > Mine > Haul > Build > CancelDesignation > ToggleIdlePatrol | 現行 `familiar_command_input_system` の `else if` 順。Normal+Familiar の Escape もこの family に含む |
| AreaEditCommand | Alt preset load > Ctrl preset save > Copy > Paste > Redo > Undo。slot は 1 > 2 > 3 | 現行 shortcut handler の早期 return 順 |
| CancelOrClose | 4.3 の CaptureOverlay（LoadConfirm > Settings > OperationDialog）> ActiveMode > OpenMenu 順 | 同じ Escape を最前面 owner だけが処理。Pause は TimeControl、Normal+Familiar は FamiliarCommand family |

これにより F5+F9 は Save、C+M は Chop、Digit1+Digit2 の World 入力は TimeNormal へ
1 件だけ解決される。順位は偶然の enum 順ではなく binding data と matrix test の双方へ記録する。

family 内 cardinality を適用した後、異なる family は次の conflict lane で再評価する。

| conflict lane | action 例 | compatibility 契約 |
| --- | --- | --- |
| `OverlayTransition` | RequestLoad、pause open/close、modal close | frame 内 1 件。open/close と SaveLoad/Mode/Familiar/Area/selection は併存させない |
| `SelectionOrMode` | B/Z、Tab、Familiar command、AreaEdit、CancelActiveMode | frame 内 1 件。context priority が高い action を残し、world/list selection ingress も抑止する |
| `SimulationControl` | F5、Digit1-4 | gesture がなく overlay transition もない場合だけ、互いに明示 compatible な組を許可する |
| `ViewDebug` | V、F3/F4/F6/F7/F8/F12、P/O | owner state が独立する組だけ明示 compatible。P/O は resolver 時点の DebugVisible で確定する |

default は非 compatible とし、許可する組を binding data に列挙する。初版では `ViewDebug` と
非 overlay lane、`SimulationControl` 内の SaveLoad + 明示 time action、gesture のない
`SelectionOrMode` + 非 transition `SimulationControl` を compatible とする。少なくとも Familiar C+Z は
FamiliarChop、AreaEdit Ctrl+V+B は AreaPaste、Space+F5 は TogglePause だけに解決する。

### 4.5 Cancel / capture-start rollback の owner 契約

Escape と capture 開始は目的を分ける。Escape の `CancelActiveMode` は active mode 自体を終了する。
`world_input_capture_started` は modal/pause の背後に未確定 gesture を残さないため、mode を終了せず
その gesture だけを開始前状態へ戻す。いずれも単なる `TaskContext = None` ではなく owner helper を使う。

| owner state | Escape (`CancelActiveMode`) | capture false→true (`rollback_in_progress_gesture_system`) |
| --- | --- | --- |
| `BuildContext` / `CompanionPlacementState` | context/pending/ghost を owner helper で clear 後に Normal | release edge に依存しない click placement なので state/preview を維持し、capture guard で click だけ止める |
| `MoveContext` / `MovePlacementState` / companion move | `clear_move_states` 相当で 3 context を clear 後に Normal。確定前の実体 transform/parent/visibility は変更されていないため復元しない | release edge に依存しない click placement なので state/preview を維持し、閉じた後に再開できる |
| `FloorPlace(None/Some)` / `WallPlace(None/Some)` | start/preview を消し、`TaskMode::None` と Normal | `Some(_) -> None` に戻して preview を消す。`None` は mode を維持する |
| `DesignateChop/Mine/Haul(None/Some)` / `CancelDesignation(None/Some)` / `AssignTask(None/Some)` / `SelectBuildTarget` | start と preview を消し、`TaskMode::None` と Normal | `Some(_)` は同 variant の `None` へ戻し、`None` / `SelectBuildTarget` は mode を維持する |
| `DreamPlanting(None/Some)` | start、preview seed を消し、`TaskMode::None` と Normal | `Some(_) -> None` と `dream_planting_preview_seed` clear。release 済みの `pending_dream_planting` は触らない |
| `AreaSelection(None)` | `TaskMode::None` と Normal | mode/state を維持し、capture guard で新規 drag だけを止める |
| `AreaEditSession.active_drag` | drag 前の `TaskArea` と関連状態を復元後、session を消して Normal | drag 前へ復元して `active_drag=None`、`AreaSelection(None)` に戻す。history/task assignment は追加しない |
| `ZoneContext` / `ZonePlacement(_, None/Some)` | start/preview と `ZoneContext` を clear して Normal | `Some(_)` は同 zone kind の `None` へ戻し、`ZoneContext` と mode は維持する |
| `ZoneRemovalPreviewState` / `ZoneRemoval(_, None/Some)` | `clear_removal_preview` で sprite 色も戻し、`ZoneContext` を clear して Normal | 同 helper で色を戻し、同 zone kind の `None` へ戻して mode は維持する |
| `SoulSpaPlace(None/Some)` | `TaskMode::None` と Normal。未確定 preview がある場合だけ owner helper で消す | click placement なので mode/state を維持し、capture 中の click だけを止める |
| Entity List `DragState` / `EntityListResizeState` | 対象外（現行 Escape 契約は追加しない） | capture 開始時に active/pending drag、ghost、resize state を同じ reset helper で clear し、release で squad request や panel resizeを確定しない |

AreaEdit drag は held 中に `TaskArea`、`Destination`、`ActiveCommand` をすでに変更するため、
`AreaEditDrag` を必要な rollback snapshot（少なくとも元 `TaskArea` と、変更対象なら元 `Destination` /
`ActiveCommand`）まで拡張する。rollback は Commands の deferred 適用順も含めて、capture が始まった
frame の後段 gesture system が再度変更しないよう `rollback -> pointer consumers` を固定する。
`InputContextSnapshot::has_in_progress_gesture` は AreaEdit active drag と上表の rollback-required gesture を
表し、true の間は `SaveGame` を生成しない。Pause/Modal open は先に rollback してから、Pause menu の
Save button または後続 frame の F5 を許可する。

OpenMenu の Escape は `MenuState::Hidden` だけを設定し、Familiar Idle/Patrol を変更しない。
ActiveMode と menu が異常に同時 active の場合は、mode owner cleanup と `MenuState::Hidden` を
`CancelActiveMode` の 1 consumer 内で完了する。

### 4.6 Action consumer ownership

| action 群 | consumer | 方針 |
| --- | --- | --- |
| menu toggle/time/save/load/modal close | `input_action_to_ui_intent_system` | `TogglePause`、`CancelLoadConfirm`、`CloseSettings`、`CloseDialog` を含む既存 `UiIntent` へ変換し、`handle_ui_intent` が処理。OpenMenu の Escape は次行の root adapter |
| Familiar command | `familiar_command_input_system` | raw key 分岐だけを action match に置換。active command/TaskContext の挙動は維持 |
| TaskArea history/preset | `task_area_edit_history_shortcuts_system` | raw key/modifier 分岐を action match に置換 |
| Area apply Shift | area-selection input handler | `ResolvedInputFrame.modifiers.shift` を読む |
| elevation | `elevation_view_input_system` | `CycleElevation` だけを処理 |
| Tab cycle | `entity_list_tab_focus_system` | next/previous action だけを処理 |
| render/debug toggles | `plugins/input.rs` の小さな consumer | Resource mutationは現行 owner に残す。F12 は `DebugVisible`、`GizmoConfigStore`、`GameSettings.debug_gizmos_enabled`、Settings checkbox の同期を維持 |
| P/O debug spawn | `debug_spawn_system` | action に応じて既存 spawn Message を発行。登録側の mutable `run_if(DebugVisible)` は外し、resolver snapshot の判定を再 gate しない |
| mode/menu Escape | root の cancel adapter + mode/menu 固有 helper | OpenMenu/全 active mode の priority 解決後、4.5 の owner 契約を一度だけ実行する。Modal/Pause は UiIntent bridge、Normal+Familiar は familiar consumer が所有し、旧 `ui_keyboard_shortcuts_system` の raw Escape path は削除する |
| overlay capture transition | root の capture request adapter + `begin_world_input_capture` helper | keyboard/UI button の全 open path を同じ helper へ集約し、pending capture、InputFocus clear、selection suppression を mutation owner より前に確定する |
| capture-start rollback | root の capture transition adapter + gesture 固有 helper | action ではなく `world_input_capture_started` を 1 回だけ処理し、4.5 の未確定 gesture だけを戻す |
| pointer/camera capture | 既存 selection/placement/area/Entity List system と `pan_camera_world_input_guard_system` | action 化せず `world_input_blocked()` と frame-local selection suppression / primary-pointer claim を共通 guard として読む |

1 action variant の mutation owner は 1 system に限定する。binding table の一意性に加え、
テスト用の owner classification が全 `InputAction` variant を一度だけ分類することを検証する。

### 4.7 既存 `UiIntent` との境界

- button と同じ意味を持つ keyboard action は `UiIntent` へ bridge する。
- Entity payload、文字列、mouse position を `InputAction` に持たせない。
- `UiIntent` 自体を `InputAction` の別名にせず、既存 UI button/message API を維持する。
- Familiar/AreaEdit の root-domain mutation を `handle_ui_intent` の巨大 match へ移さない。
- 旧 `docs/proposals/archive/05-unified-interaction-layer.md` の button/context-menu/selection 統合は
  本計画では採用しない。

### 4.8 Bevy 0.19 API と ordering の注意

- raw keyboard は現行どおり `Res<ButtonInput<KeyCode>>` の `just_pressed` / `pressed` を使う。
- text editing は `FocusedInput<KeyboardInput>` と `InputFocusSystems::Dispatch` の既存経路を維持する。
- resolver は `InputFocusSystems::Dispatch` / `text_input_focus_sync_system` と
  `UiSystems::Focus` / `update_ui_input_state_system` の後、`PickingSystems::Hover` の前の PreUpdate で動かし、
  observer の consumed latch、focus、更新済み `Interaction` を同じ frame に読む。
- overlay open request を受理した helper は `InputFocus` を同期的に clear する。次 frame の focus sync や
  Escape 二度押しに解除を委ねない。
- `State<PlayMode>` は現在値、`NextState<PlayMode>` は次状態なので、context 解決に NextState を推測利用しない。
- overlay 判定は既存 helper と同じ `Node.display` の可視性に pending open request を合成する。
  `Node.display` は stable visible state であり、open frame の唯一の正本にはしない。
- RequestLoad は save file 存在、OperationDialog は対象 selection など、既存 handler の open 前提を
  capture request adapter でも同じ pure helper から確認し、no-op request で 1 frame capture しない。
- `UiInputCapture` も `Node.display != Display::None` のものだけを active とし、hidden dialog を capture 扱いしない。
- Bevy 0.19 の `UiSystems::Focus` が `RelativeCursorPosition` と `Interaction` を更新するため、
  `update_ui_input_state_system.after(UiSystems::Focus)` を明示する。その後に pending/visible capture と
  text-focus latch を同期し、`pan_camera_world_input_guard_system` は両同期より後かつ
  `PickingSystems::Hover` より前へ固定する。
- capture 前から window drag 中の `Pointer<Drag>` observer も `PanCamera.enabled == false` を読むよう、
  integration test は enabled flag だけでなく camera `Transform` 不変を確認する。
- `MessageWriter<UiIntent>` は Input set で書き、既存 Interface chain の `handle_ui_intent` が同 frame に読む ordering test を置く。
- resolver の selection snapshot は pointer click 前の値とする。world click / Entity List click と
  selection-dependent action が同 frame の時は action を優先して click mutation を抑止する。
- overlay root の `ZIndex` は `LoadConfirm > Settings > Pause > OperationDialog` とし、resolver の Escape
  priority と一致させる。
- `ButtonInput` 自体を mutate/clear して擬似的に「消費」しない。Bevy/外部 plugin の入力状態を壊さない。

## 5. マイルストーン

## M1: Resolver 基盤と F5/F9 vertical slice

- 変更内容:
  - `input_actions` module と 4.1 の型を追加する。
  - default binding table、pure context resolver、binding uniqueness/owner completeness test を追加する。
  - 4.2 のうち `InputPreUpdateSet::Resolve` と Update の `PointerIngress -> Consume` を
    `InputPlugin` で登録し、capture 用 PreUpdate set は実処理を追加する M3b で導入する。domain consumer の登録は
    owning plugin に残す。
  - Save/Load と elevation を最初の consumer として移行する。
  - `has_in_progress_gesture` 中は F5 を生成しない。
  - Soul mask / extra light の F5/F9 keyboard reader を削除し、DevPanel / 環境変数経路は維持する。
  - F9 keyboard 経路を `UiIntent::RequestLoadGame` へ bridge し、load confirm を通す。
  - `save_load_keybind_system` を登録から外す。削除は参照が 0 になった時点で同 milestone 内に行う。
- 変更ファイル:
  - `crates/bevy_app/src/lib.rs`
  - `crates/bevy_app/src/input_actions/{mod.rs,model.rs,bindings.rs,context.rs,resolver.rs,tests.rs}`（新規）
  - `crates/bevy_app/src/plugins/input.rs`
  - `crates/bevy_app/src/plugins/visual.rs`（elevation consumer の登録順変更）
  - `crates/bevy_app/src/systems/save/{mod.rs,state.rs}`
  - `crates/bevy_app/src/systems/visual/elevation_view.rs`
  - `crates/bevy_app/src/interface/ui/interaction/`（UiIntent bridge の最小配線）
  - `docs/architecture.md`
  - `docs/save_load.md`
  - `docs/debug-features.md`
  - `docs/proposals/gameplay-management-improvements-proposal-2026-07-17.md`
- 完了条件:
  - [x] F5 で Save だけが要求され、Soul mask は変わらない
  - [x] save file がある F9 で load confirm だけが開き、確認前に `LoadRequested` にならない
  - [x] save file がない F9 は既存 warning/no-op 契約を維持し、direct load を要求しない
  - [x] F5/F9 以外の操作と DevPanel から Soul mask / extra light を引き続き変更できる
  - [x] Ctrl+V では elevation が変わらないための exact chord test が先に通る
  - [x] `Resolve -> PointerIngress -> Consume` の M1 ordering test が通る
  - [x] AreaEdit active drag 中の F5 は SaveGame を生成しない
  - [x] 同じ action の alias を同 frame に押しても action/consumer は 1 回だけ処理する
  - [x] 異なる family は明示 compatible な組だけを安定順で返し、同じ排他的 family と非 compatible lane は priority の 1 action だけを返す
  - [x] M1 終了時点で unused type/system や `#[allow(dead_code)]` がない
- 検証:
  - `cargo test -p bevy_app@0.1.0 input_actions`
  - `cargo test -p bevy_app@0.1.0 systems::save`
  - `cargo check --workspace --locked`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`

## M2: Global、Modal、Familiar、Escape の一意解決

- 変更内容:
  - B/Z、Space、Digit1-4 を resolver から既存 `UiIntent` へ bridge する。
  - `ui_keyboard_shortcuts_system` の raw keyboard path を resolver/bridge/root cancel adapter へ置換し、参照が 0 になれば削除する。
  - load confirm、Settings、operation dialog の可視性から modal context を構築する。
  - overlay open request を受理する keyboard/UI handler は `InputFocus` を同期 clear し、背景 text fieldを
    focused のまま残さない。
  - M2 で resolver へ移行済みの shortcut は、modal 中に Escape 以外を抑止する。残存 raw reader を含む
    全 project shortcut の抑止は M3a 完了条件とする。
  - Pause overlay 中は Escape/Space、Digit1-4、F5/F9 だけを許可し、Escape は resume に解決する。
  - Familiar 選択時の C/M/H/B、Digit0-4、Delete、Escape を action 化する。
  - cross-family compatibility を導入し、Familiar C+Z と Space+F5 のような非可換 action を
    consumer 到達前に 1 件へ絞る。AreaEdit action の組合せは M3a で追加する。
  - `ResolvedInputFrame` に frame-local な pointer-selection suppression を持たせ、selection-dependent / context-mutating
    action がある frame は world click と Entity List row click/drag が `SelectedEntity` を変更する前に止める。
    Tab action consumer 自体はこの pointer ingress gate の対象にしない。
  - 非 pause、PlayMode Normal、互換 TaskMode で selected Familiar と World が同じ B/Digit を
    claim した場合、Familiar action だけを返す。Area/Placement 中は Familiar command を生成しない。
  - pause 中は Familiar/Area action を生成せず、`ResolvedInputFrame` を毎 frame 置換する。
  - active placement/designation の Escape は mode cancel だけを行い、Familiar Idle/Patrol を同時変更しない。
  - ActiveMode 中の B/Z/Tab は World へ fallthrough させない。keyboard/UI button の mode toggle が
    active owner を終了する場合は、Escape と同じ owner cleanup helper を経由する。
  - BuildingPlace / FloorPlace / WallPlace / BuildingMove / SoulSpaPlace / companion placement を含む
    全 active mode の Escape を、
    4.5 の各 mode 固有 state cleanup helper へ接続する。
  - OpenMenu の Escape を ActiveMode と Familiar の間で解決し、menu だけを閉じる。
  - Normal かつ Familiar 選択時の Escape は現行 Idle/Patrol toggle を維持する。
- 変更ファイル:
  - `crates/bevy_app/src/input_actions/{bindings.rs,cancel.rs,context.rs,mod.rs,model.rs,resolver.rs,tests.rs}`
  - `crates/bevy_app/src/interface/ui/interaction/{systems.rs,intent_context.rs,intent_handler.rs,handlers/{general.rs,save_game.rs,settings.rs,mode_selection.rs,mode_toggle.rs}}`
  - `crates/bevy_app/src/interface/selection/input.rs`
  - `crates/bevy_app/src/interface/ui/list/{interaction.rs,interaction/navigation.rs,drag_drop.rs}`
  - `crates/bevy_app/src/interface/ui/panels/context_menu.rs`
  - `crates/bevy_app/src/interface/selection/{building_place/,floor_place/,building_move/,soul_spa_place/}`
  - `crates/bevy_app/src/app_contexts.rs`（既存 state の参照のみ。型変更が不要なら編集しない）
  - `crates/bevy_app/src/systems/command/input.rs`
  - `crates/bevy_app/src/systems/command/area_selection/{input.rs,input/**}`
  - `crates/bevy_app/src/systems/command/zone_placement/{placement.rs,removal.rs,removal_preview.rs}`
  - `crates/hw_ui/src/area_edit/state.rs`（AreaEdit rollback snapshot が必要な場合）
  - `crates/bevy_app/src/plugins/{input.rs,logic.rs}`（ordering/登録変更が必要な場合のみ）
- 完了条件:
  - [x] Familiar 未選択の Digit1-4 は時間速度だけを変更する
  - [x] Familiar 選択中の Digit1-4 は Familiar command だけを変更する
  - [x] Familiar 選択中の B は Build command だけで、Architect menu を開かない
  - [x] Area/Placement 中は C/M/H/B/Digit が Familiar command として TaskMode を上書きしない
  - [x] pause 中の Digit2-4 は時間速度だけを変更し、選択 Familiar の command は変わらない
  - [x] pause 中の Escape は resume だけを行い、背景の active mode/Familiar state を変更しない
  - [x] Space/Escape の `TogglePause` と Digit1-4 が同 frame の場合は TimeControl priority の 1 action だけを処理する
  - [x] pause 中に押して離した Familiar/Area shortcut が unpause 後に発火しない
  - [x] plain Z は Zones、Ctrl+Z は plain Z として解決されない
  - [x] text input focus/latch 中は resolved action が空になる
  - [x] TextInput focus 中に LoadConfirm/Settings/OperationDialog/Pause を開くと InputFocus が clear される
  - [x] 各 modal 中は Escape が visual stack 最上位 overlay だけを閉じ、M2 移行済みの Space/B/Z/Digit/Familiar action は発火しない
  - [x] Familiar C+Z は FamiliarChop だけ、Space+F5 は TogglePause だけを生成する
  - [x] Familiar action と同 frame の world/Entity List click は click mutation を抑止し、resolver snapshot と consumer target が一致する
  - [x] ActiveMode 中の B/Z/Tab は World action を生成せず、UI button で mode を切り替える場合も stale owner state を残さない
  - [x] placement cancel と Familiar command toggle が同じ Escape で併発しない
  - [x] Normal + Familiar で C/M/B/Digit と Escape を同時押下しても現行 `else if` 順の Familiar action 1 件だけを処理する
  - [x] OpenMenu + Familiar の Escape は menu だけを閉じ、Idle/Patrol を変更しない
  - [x] ActiveMode + OpenMenu の Escape は owner cleanup と menu close を 1 action で行う
  - [x] 全 PlayMode の Escape で AreaEdit drag、Dream seed、Zone removal preview、pending companion、move state を残さず Normal へ戻る
- 検証:
  - `cargo test -p bevy_app@0.1.0 input_actions`
  - text input focus/latch と selection suppression の新規 schedule test
  - `cargo check --workspace --locked`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`

## M3: AreaEdit、残存 shortcut、Modal/Pause capture gate

- 変更内容:
  - **M3a: 残存 keyboard reader の完全移行**
  - Ctrl+C/V/Z/Y、Ctrl+Shift+Z、Ctrl+Digit1-3、Alt+Digit1-3 を action 化する。
  - AreaEdit action は現在の `State<PlayMode> == TaskDesignation` と
    `TaskMode::AreaSelection(_)` の両方を要求し、同 frame の `NextState` だけでは生成しない。
  - TaskArea shortcut の modifier/slot 判定を binding table へ移し、`hotkey_slot_index` の
    raw keyboard 依存を削除する。
  - AreaSelection の Shift-held 挙動は `ResolvedInputFrame.modifiers.shift` を読む。
  - Tab/Shift+Tab と P/O debug spawn を action 化する。
  - F3/F4/F6/F7/F8/F12 を action 化し、project-owned edge shortcut の raw reader を resolver に集約する。
  - F12 consumer の DebugVisible / GizmoConfigStore / GameSettings / Settings checkbox 同期は変更しない。
  - P/O consumer の `run_if(DebugVisible)` を外し、resolver snapshot で生成済みの action を後段で再 gate しない。
  - **M3b: Overlay capture、picking、owner rollback**
  - `UiInputCapture` を load confirm、Settings、operation dialog、Pause menu へ付与し、
    pending open request と visible marker を合成した `UiInputState::world_input_blocked()`、false→true の
    `world_input_capture_started` latch を追加する。
  - M1 の `Resolve -> PointerIngress -> Consume` を、capture 実処理を含む
    `Resolve -> CaptureTransition -> Rollback -> PointerIngress -> Consume` へ拡張する。
  - keyboard と capture-opening UI button の全経路を `begin_world_input_capture` へ集約し、request 受理時に
    `InputFocus` clear と frame-local selection/pointer suppression を確定する。
  - 各 capture 対象を viewport 全体の transparent blocking root + 前景 panel の構造へ変更し、
    Bevy UI picking でも背景 button を遮断する。Pause panel には mouse 操作用の Resume button を追加する。
  - selection、hover、assignment、building/floor/move/soul-spa/zone/area placement、context menu、
    debug spawn、Entity List row selection/drag/resize の既存 guard を `world_input_blocked()` と
    selection suppression へ統一する。
  - `ui_interaction_system` は pending/active overlay の foreground ancestry を確認し、open request を発生させた
    button と最前面 overlay 内 button 以外の背景 `MenuButton` / `UiIntent` を同 frame から無視する。
  - Entity List capture 開始時は `DragState` と ghost を reset し、`EntityListResizeState.active=false` にする。
  - `pan_camera_world_input_guard_system` も capture flag を使い、capture/text-focus sync 後かつ
    `PickingSystems::Hover` より前に実行する。
  - capture 開始 frame に `rollback_in_progress_gesture_system` を一度だけ実行し、4.5 の owner helper で
    AreaEdit、designation、Dream、Floor/Wall、Zone placement/removal、Entity List の未確定 drag/previewを開始前へ戻す。
    既に release 済みの確定操作や `pending_dream_planting` は戻さない。
  - BuildingPlace / BuildingMove / companion / SoulSpa の click placement は state を維持し、capture 中の click だけを止める。
  - `InputFocusSystems::Dispatch` / `UiSystems::Focus` → text/pointer/request/visible sync → Resolve → capture/rollback/camera guard →
    `PickingSystems::Hover` → Update pointer consumer の明示順を作る。
  - project-owned production shortcut consumer から `ButtonInput<KeyCode>` / `KeyCode` 分岐を除く。
  - `PanCameraPlugin`、Bevy text editing、test resource 初期化、`visual_test` は明示 whitelist とする。
- 変更ファイル:
  - `crates/bevy_app/src/input_actions/{bindings.rs,cancel.rs,capture.rs,context.rs,resolver.rs,tests.rs}`
  - `crates/bevy_app/src/systems/command/area_selection/{shortcuts.rs,geometry.rs,input.rs,input/**}`
  - `crates/bevy_app/src/interface/ui/list/interaction/navigation.rs`
  - `crates/bevy_app/src/interface/ui/list/{interaction.rs,drag_drop.rs}`
  - `crates/bevy_app/src/plugins/{input.rs,interface.rs,interface_debug.rs}`
  - `crates/bevy_app/src/interface/ui/{interaction/**,plugins/foundation.rs}`
  - `crates/hw_ui/src/{components.rs,interaction/text_field.rs,list/resize.rs,setup/dialogs.rs,setup/settings_panel.rs,setup/pause_menu.rs}`
  - `crates/bevy_app/src/interface/selection/**`
  - `crates/bevy_app/src/systems/command/{assign_task.rs,zone_placement/**,area_selection/**}`
  - `crates/bevy_app/src/interface/ui/panels/context_menu.rs`
- 完了条件:
  - [x] Ctrl+V は Area paste だけで elevation を変更しない
  - [x] Ctrl+Z/Y と Ctrl+Shift+Z は Area history だけを変更する
  - [x] Ctrl/Alt+Digit1-3 は preset だけを変更し、Familiar/Time action を発火しない
  - [x] AreaEdit Ctrl+V+B は AreaPaste だけを生成する
  - [x] plain V は elevation だけを変更する
  - [x] Tab/Shift+Tab の方向を維持し、active mode 中は生成しない
  - [x] DebugVisible=false の P/O は action を生成しない
  - [x] DebugVisible=true の F12+P は resolver snapshot の契約どおり処理され、P action が後段 `run_if` で消えない
  - [x] `Resolve -> CaptureTransition -> Rollback -> PointerIngress -> Consume` と
    `CameraGuard -> PickingSystems::Hover` の M3b ordering test が通る
  - [x] Modal/Pause 中は M3a で移行した debug/Tab/AreaEdit を含む全 project shortcut が抑止される
  - [x] Modal/Pause 中は panel 外の click/drag でも selection、placement、assignment、context menu が変化しない
  - [x] Modal/Pause 中は前景 panel 以外の UI button が反応せず、Pause の Resume/Save/Load/Settings は操作できる
  - [x] Modal/Pause 中は PanCamera が無効になり、capture 開始 frame の mouse drag でも camera Transform が変化せず、閉じた次 frame に既存設定どおり復帰する
  - [x] UI hover/capture は `UiSystems::Focus` と同 frame の値を使い、keyboard/UI button の open request 受理 frameから漏れがない
  - [x] TextInput focus 中に各 overlay を開くと focus が同期的に clear され、背景 EditableText が文字/Escapeを受けない
  - [x] drag 中に Modal/Pause を開いて capture 中に mouse release しても、再開後に `TaskMode::*Some`、
    `AreaEditSession.active_drag`、Dream seed、Zone removal preview が残らず、AreaEdit は元状態へ戻る
  - [x] capture 開始 rollback は AreaEdit history、task assignment、Dream pending request、zone mutationを新規確定しない
  - [x] Entity List drag/resize 中の capture 開始で ghost、pending/active drag、resize active が残らず、squad request と panel resize を確定しない
  - [x] production shortcut の raw keyboard audit が resolver と whitelist 以外 0 件
- 検証:
  - `cargo test -p bevy_app@0.1.0 input_actions`
  - `cargo test -p hw_ui --lib`
  - AreaEdit / Entity List / overlay capture の明示名付き focused test
  - `rg -n "ButtonInput<KeyCode>|just_pressed\(KeyCode" crates/bevy_app/src --glob '*.rs'`
  - `cargo check --workspace --locked`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`

## M4: 恒久ドキュメント同期と全体回帰

- 変更内容:
  - keyboard shortcut の唯一の解決元、context priority、consumer ownership を恒久 docs に記載する。
  - F5/F9 の player 予約と debug alias 廃止、F9 confirmation、Familiar/World/pause priority を操作表へ反映する。
  - Dream の未実装 `D` shortcut 表示を削除し、PanCamera の D binding は維持する。
  - direct keyboard reader の追加時は binding table と resolver test を更新するルールを記載する。
  - 関連提案の A1 状態と、本計画の完了/アーカイブ判断を同期する。
- 変更ファイル:
  - `docs/architecture.md`
  - `docs/state.md`
  - `docs/tasks.md`
  - `docs/save_load.md`
  - `docs/debug-features.md`
  - `docs/entity_list_ui.md`
  - `docs/settings.md`
  - `docs/building.md`
  - `docs/dream.md`
  - `docs/cargo_workspace.md`
  - `crates/bevy_app/src/interface/README.md`
  - `crates/bevy_app/src/interface/ui/README.md`
  - `crates/bevy_app/src/systems/command/README.md`
  - `crates/hw_ui/src/setup/bottom_bar.rs`
  - `docs/proposals/gameplay-management-improvements-proposal-2026-07-17.md`
- 完了条件:
  - [x] 恒久 docs と default binding table が一致する
  - [x] `python3 scripts/dev.py docs --check` が成功する
  - [x] 自動回帰で代替できない重点実機受入（task-area camera、capture 中 release、overlay open 時の背景入力遮断）を完了する
  - [x] full repository quality gate が成功する
- 検証:
  - `python3 scripts/dev.py docs --check`
  - `python3 scripts/dev.py verify`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| resolver 自体が全 UI mutation を抱える | 新しい巨大 dispatcher になる | resolver は chord -> action だけを担当し、mutation は既存 owner/UiIntent handler に残す |
| 同じ action を複数 consumer が処理する | physical chord は一意でも副作用が二重になる | action owner table を固定し、全 variant がちょうど一 owner に分類される test を置く |
| overlay 背景の text focus が残る | 背景 EditableText が文字/Escape を受け、modal を閉じられない | open request 受理時に `InputFocus` を同期 clear し、overlay + text focus の schedule test を置く |
| visible marker だけで capture を判定する | Interface で開いた overlay の open frame に world/camera 入力が漏れる | pending open request と `Node.display` を合成し、request 受理時点から effective capture を立てる |
| modal の存在だけを見て hidden dialog を active 扱いする | game shortcut が恒常的に無効になる | pending request がなく、existing visibility helper も hidden と判定する時は capture しない |
| Modal/Pause capture が cursor hover と同じ意味に混ざる | panel を閉じても world input が無効のままになる | `pointer_over_ui` と `world_input_captured` は別 field で保持し、`world_input_blocked()` だけが合成する。hidden marker の test を置く |
| capture flag だけで背景 UI button が生き残る | keyboard は止まるが mouse で同じ UiIntent が発火する | pending frame は foreground ancestry gate、visible 後は full-viewport blocking root で背景 button を止める。Pause には Resume button を用意する |
| `Pointer<Drag>` observer より後に PanCamera guard が走る | capture 開始 frame に camera Transform が 1 回動く | guard を sync/Resolve/capture 後かつ `PickingSystems::Hover` 前へ固定し、Transform 不変を integration test する |
| capture 中の release edge を guard が捨てる | drag/preview state が残り、再開後に意図しない確定や ghost が起きる | false→true latch で owner 別 rollback を 1 回実行し、drag→capture→release→resume の integration test を置く |
| AreaEdit drag rollback が表示だけを消す | held 中に変更済みの TaskArea/Destination/ActiveCommand が残る | `AreaEditDrag` に開始前 snapshot を保持し、history/task assignment を作らず全関連 state を復元する |
| `NextState<PlayMode>` だけで AreaEdit context を判定する | current state Normal の frame に action を生成するが Logic consumer が停止し、入力が消える | current `State<PlayMode> == TaskDesignation` と `TaskMode::AreaSelection(_)` を共に要求する test を置く |
| 同じ family の複数 chord が複数 mutation を起こす | C+M や Digit1+2 の結果が consumer 順に依存する | `InputActionFamily` と明示 `family_priority` で最大 1 件にし、同時押下 matrix を固定する |
| 異なる family の非可換 action が併存する | C+Z や Ctrl+V+B の最終 state が consumer set 順に依存する | conflict lane と default-deny compatibility table で consumer 到達前に 1 action へ絞る |
| resolver 後に selection が変わる | Familiar 判定に使った Entity と consumer が変更する Entity がずれる | selection-dependent/context-mutating action の frame は world/list selection ingress を抑止する |
| active gesture 中に F5 が通る | release 前の persisted `TaskArea` 等を `Last` save が記録する | `has_in_progress_gesture` 中は SaveGame を生成せず、capture rollback 後の frame で保存する |
| Entity List の raw drag/resize が capture 外に残る | 背景 ghost、squad request、panel resize が進行・確定する | world capture guard と capture-start reset helper を row selection/drag/resize に適用する |
| F9 の確認導入で既存 code test が変わる | keyboard load のタイミングが変わる | 恒久 docs を正本とし、request -> confirm -> Last apply の test へ更新する |
| Familiar 選択中に数字速度 shortcut が使えない | UX 上の驚き | Normal の Familiar context だけを優先すると明記し、Area/Placement/pause 中は Familiar alias を無効化する。通常時は UI button でも速度変更可能とする |
| B が Familiar Build を優先する | Architect hotkey が選択状態依存になる | context indicator/既存 mode UI で状態を示し、本計画で binding を曖昧にしない |
| F5/F9 debug keyboard alias の削除を見落とす | docs と実装の操作表がずれる | DevPanel 代替を確認し、`debug-features.md` と `architecture.md` を同 milestone で更新する |
| ButtonInput を clear して外部入力を消費する実装になる | PanCamera/TextInput 等を破壊する | raw Resource は read-only。消費は resolver 出力の一意性だけで表現する |
| pause 中に resolved frame が更新されない | stale action が再実行される | resolver は pause に gate されない PreUpdate set で毎 frame 置換する |
| generic Escape が owner 固有 state を一部だけ消す | ghost、pending move、companion state が残る | 単純な `NextState<PlayMode>` 書換えにせず、実在する owner cleanup を helper 化して同じ経路を呼ぶ |
| binding table が設定システムへ早期拡張される | A1 の範囲が肥大する | compile-time default だけを実装し、serialization/UI は後続計画へ送る |
| source grep を品質契約と誤解する | test helper や外部入力まで禁止する | resolver、test init、PanCamera/TextInput、`visual_test` の whitelist を docs と review に明記する |

## 7. 検証計画

### 必須自動検証

- `cargo fmt --all -- --check`
- `cargo test -p bevy_app@0.1.0 input_actions`
- `cargo test -p hw_ui --lib`
- `cargo check --workspace --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- `python3 scripts/dev.py docs --check`
- `git diff --check`
- rust-analyzer MCP の workspace diagnostics が error/warning 0 件。MCP unavailable の場合は
  `cargo check --workspace --locked` と変更ファイルの local diagnostics を記録する

### Resolver matrix test

| context | chord | 期待 action | 同時に出てはいけない action |
| --- | --- | --- | --- |
| World | F5 | SaveGame | Soul mask toggle side effect |
| World | F9 | RequestLoadGame | extra light toggle side effect / direct LoadRequested |
| World | Digit1 | TimePaused | FamiliarChop |
| Familiar | Digit1 | FamiliarChop | TimePaused |
| Familiar | B | FamiliarBuild | ToggleArchitect |
| Paused + Familiar | Digit2 | TimeNormal | FamiliarMine |
| Paused + AreaEdit | Ctrl+Digit1 | なし | SavePreset1 / FamiliarChop / TimePaused |
| Paused + Placement | Escape | TogglePause（resume） | CancelActiveMode / ToggleFamiliarIdlePatrol |
| World | Space + Digit2（同 frame） | TimeNormal 1 件 | TogglePause |
| Paused + Placement | Escape + Digit1（同 frame） | TimePaused 1 件 | TogglePause / CancelActiveMode |
| AreaEdit | Ctrl+V | AreaPaste | CycleElevation |
| AreaEdit | Ctrl+Z | AreaUndo | ToggleZones |
| AreaEdit | Ctrl+Shift+Z | AreaRedo | AreaUndo / ToggleZones |
| AreaEdit | Ctrl+Digit1 | SavePreset1 | FamiliarChop / TimePaused |
| TextInput | 任意 project chord | なし | 全 project action |
| LoadConfirm | Escape | CancelLoadConfirm | mode cancel / Familiar toggle |
| Settings | Escape | CloseSettings | mode cancel / Familiar toggle |
| OperationDialog | Escape | CloseOperationDialog | mode cancel / Familiar toggle |
| Placement + Familiar | Escape | CancelActiveMode | ToggleFamiliarIdlePatrol |
| OpenMenu + Familiar | Escape | CloseMenu | ToggleFamiliarIdlePatrol |
| Placement + OpenMenu | Escape | CancelActiveMode（menu も Hidden） | CloseMenu の二重 action |
| Normal + Familiar | Escape | ToggleFamiliarIdlePatrol | CancelActiveMode |
| DebugVisible=false | P/O | なし | DebugSpawn* |
| World/Paused（TextInput/Modal inactive。F5 は gesture なし） | F5/F9 | player action | Soul mask / extra light debug action |
| Familiar | C + Digit1（同 frame） | FamiliarChop 1 件 | FamiliarChop の重複 |
| Familiar | C + M（同 frame） | FamiliarChop 1 件 | FamiliarMine |
| Familiar | C + Z（同 frame） | FamiliarChop 1 件 | ToggleZones |
| Normal + Familiar | C + Escape（同 frame） | FamiliarChop 1 件 | ToggleFamiliarIdlePatrol |
| AreaEdit | Ctrl+V + B（同 frame） | AreaPaste 1 件 | ToggleArchitect |
| ActiveMode | B/Z/Tab | なし | ToggleArchitect / ToggleZones / ListNext / ListPrevious |
| World | Digit1 + Digit2（同 frame） | TimeNormal 1 件 | TimePaused |
| World | F5 + F9（同 frame） | SaveGame 1 件 | RequestLoadGame |
| World | F5 + Digit2（同 frame） | SaveGame + TimeNormal | なし（明示 compatible） |
| World | B + F3（同 frame） | ToggleArchitect + ToggleRender3d | なし（明示 compatible） |
| World | Space + F5（同 frame） | TogglePause 1 件 | SaveGame |
| AreaEdit active drag | F5 | なし | SaveGame |
| DebugVisible=true | F12 + P（同 frame） | ToggleDebug + DebugSpawnSoul | consumer 側 run condition による DebugSpawnSoul 消失 |
| Normal + `TaskMode::AreaSelection` + pending `NextState::TaskDesignation` | Ctrl+V | なし | AreaPaste |
| TaskDesignation + `TaskMode::AreaSelection` | Ctrl+V | AreaPaste | CycleElevation |
| Familiar + world/list click | C（同 frame） | FamiliarChop（frame-start selection） | click による `SelectedEntity` 変更 |

### UI capture integration test

- pending open request または visible `LoadConfirmDialog` / `SettingsPanel` / `OperationDialog` / `PauseMenu` により
  `UiInputState.world_input_captured == true` になる。
- pending request がなく同じ Entity が `Display::None` の時は capture しない。
- save file 不在の RequestLoad や対象 selection 不在の OperationDialog request は pending capture を開始しない。
- `world_input_blocked()` は `pointer_over_ui || world_input_captured` と一致する。
- `update_ui_input_state_system` は `UiSystems::Focus` が更新した同 frame の
  `RelativeCursorPosition` / `Interaction` を読み、hover/capture に 1 frame lag がない。
- capture 中の left/right click で `SelectedEntity`、各 placement context、`TaskContext` が変化しない。
- 前景 panel 外側の座標で背景の menu/time button が hover/pressed にならず、前景 panel の
  button だけが UiIntent を発行する。
- overlay root の visual/picking stack と Escape priority が `LoadConfirm > Settings > Pause > OperationDialog`
  で一致する。
- TextInput focused 中に各 overlay の open request を受理すると `InputFocus` が clear され、背景 field へ
  文字/Escape が届かない。
- capture sync / text-focus sync → `pan_camera_world_input_guard_system` → `PickingSystems::Hover` の順で、
  capture または text focus 中は `PanCamera.enabled == false` かつ既存 window drag の camera Transform が変化せず、
  capture 解除後は pointer/text-focus 条件どおり `PanCamera.enabled` が戻り、設定側が所有する
  `PanCamera.mouse_pan_settings.enabled` を上書きしない。
- task area mode 待機中、同 frame の Familiar area action、Normal からの TaskArea border press の各開始経路で
  実 `PanCameraPlugin` の primary `Drag` を送っても camera Transform が変化しない。claim 後に owner を解除しても
  release までは不変で、release 後の通常 world drag は再び Transform を更新する。
- AreaEdit / designation / Dream / Floor / Wall / Zone placement/removal の drag 中に capture を開始し、
  capture 中に release してから再開しても `TaskMode::*Some`、`active_drag`、Dream seed、Zone preview が残らない。
- AreaEdit は開始前の `TaskArea`、`Destination`、`ActiveCommand` へ戻り、history、task assignment、
  Dream pending request、zone mutation が追加されない。
- BuildingPlace / BuildingMove / companion の click placement は release edge に依存しないため、capture 中の
  click だけを無視し、owner state/preview を維持して close 後に同じ mode を再開できる。
- `SoulSpaPlace` も click placement として capture 中の click だけを無視し、mode/state を維持する。
- Entity List row selection/drag/resize は capture または selection suppression 中に mutation せず、capture 開始時に
  drag ghost/pending/active state と resize active state を clear する。

### 手動確認シナリオ

1. 通常 World で F5 を押し、save file が更新されても DevPanel Mask 表示が変わらない。
2. DevPanel の Mask / Light2 button から各 debug 表示を変更でき、F5/F9 では変化しないことを確認する。
3. F9 を押し、確認 dialog で Cancel/Confirm の両経路を確認する。
4. Familiar を選択して Digit1-4 と B を押し、時間速度/menu が変わらないことを確認する。
5. pause 中に同じ Familiar shortcut を押して離してから再開し、遅延 command が発火しないことを確認する。
6. Familiar 選択を外して Digit1-4 と B を押し、時間速度/menu が現行どおり変わることを確認する。
7. AreaSelection で copy/paste/undo/redo/preset を実行し、elevation/menu/time が変わらないことを確認する。新規指定と既存 TaskArea border からの直接編集を左ドラッグしても camera が動かず、release 後は通常の mouse pan が復帰することも確認する。
8. 検索/rename text field で B/Z/V/Space/Digit/Tab/Ctrl+V/Escape を入力し、ゲーム操作へ漏れないことを確認する。focus 中に各 overlay を UI から開き、focus が解除されて背景 field に文字/Escape が届かないことも確認する。
9. load confirm、Settings、operation dialog、Pause menu を keyboard/UI button から開き、open request を受理した同 frame を含めて panel 外の shortcut/click/drag/WASD と背景 UI button が world/UI state を変えないことを確認する。
10. Pause panel の Resume/Save/Load/Settings が操作でき、pause 中の Escape は mode を壊さず resume だけを行うことを確認する。
11. 非 pause の BuildingPlace、Floor/WallPlace、BuildingMove、ZonePlace、TaskDesignation、SoulSpaPlace、companion placement で Escape を押し、owner state を残さず Normal へ戻ることを確認する。各 ActiveMode の B/Z/Tab で World action が発火しないことも確認する。
12. Architect/Zones/Orders/Dream menu を Familiar 選択中に開いて Escape を押し、menu だけが閉じて Familiar command が変わらないことを確認する。
13. AreaEdit、designation、Dream、Floor/Wall、Zone removal の drag 中に Pause/dialog を開き、capture 中に mouse を離して再開しても未確定 state/preview が残らず、AreaEdit が開始前へ戻ることを確認する。
14. BuildingPlace / BuildingMove / companion / SoulSpa placement 中に Pause/dialog を開き、capture 中の click が無視され、閉じると同じ mode/state から再開できることを確認する。
15. Entity List の row click/drag/resize 中に overlay を開き、selection、drag ghost、squad request、panel size が背景で変化・確定しないことを確認する。
16. AreaEdit drag 中の F5 が save を要求せず、rollback 後の F5 または Pause menu Save が開始前/確定後の整合した状態だけを保存することを確認する。
17. Familiar C+Z、AreaEdit Ctrl+V+B、Space+F5、F12+P を同時入力し、matrix の action だけが処理されることを確認する。
18. DebugVisible の on/off で P/O を確認し、Modal で抑止される一方、通常時の PanCamera held key と mouse pan が回帰していないことを確認する。capture 前から mouse drag 中の open frame でも camera Transform が動かないことを確認する。
19. Dream tooltip に D shortcut が表示されず、D は従来どおり camera 右パンだけを行うことを確認する。

### パフォーマンス確認

- resolver は default binding table の固定長走査だけとし、Entity 数に応じた処理を行わない。
- modal/Familiar context 構築 Query は `single`/選択 Entity lookup に限定し、全 Entity 集計を行わない。
- save file existence は F9 edge を検出した frame だけ確認し、毎 frame の filesystem I/O を追加しない。
- profiling workload の work counter 追加は不要。変更前後で通常操作時の frame trace に新しい目立つ system cost がないことだけを確認する。

## 8. ロールバック方針

- M1〜M4 を別 commit 単位にし、M3 は M3a（keyboard 完全移行）と M3b（capture/picking/rollback）を
  さらに別 commit にして、各単位終了時に compile/test green を保つ。
- M2/M3a の consumer 移行は 1 consumer ごとに「action 読み取りへ変更 -> raw key 分岐削除」を同じ差分で行い、
  二重経路を残さない。
- M1 を戻す場合は Save/Load の旧 binding、F5/F9 debug alias、`save_load_keybind_system` の登録を
  同じ commit で復元する。debug alias だけを先に戻して競合状態を再導入しない。
- M3 の capture gate を戻す場合は `UiInputCapture` marker、`UiInputState` field、全 consumer の
  guard 置換を同じ commit で戻し、marker だけが残る半端な状態にしない。
- milestone を戻す時は M4 → M3b → M3a → M2 → M1 の逆順とし、後段だけを残した中間状態を作らない。
- 本計画は save schema / settings file を変更しないため、データ migration の rollback は不要。
- 問題切り分け用に raw keyboard fallback flag を恒久実装へ残さない。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `計画 100% / 実装 100%`
- 完了済みマイルストーン: M1、M2、M3a、M3b、M4
- 未着手/進行中: なし。自動回帰と重点実機受入を完了し、本計画はアーカイブ対象になった
- 前提: A1 だけを対象とする。A2/A3 や keybinding settings を同時実装しない。

### 次のAIが最初にやること

1. 本計画は完了済み。入力契約を変更する場合は恒久 docs と既存 matrix/capture/PanCamera 回帰を正本にする。
2. A2 以降は総合提案の採否をユーザーと確定してから、トラック単位の新規計画を作る。

### ブロッカー/注意点

- `UiInputState` の focus/consumed latch は完了済み text-input 契約であり、別の focus model に置き換えない。
- `UiInputState.pointer_over_ui` と `world_input_captured` を同じ field に畳まず、hover と modal capture の
  診断可能性を残す。
- effective capture は pending open request と visible marker を合成する。`world_input_capture_started` は
  current/previous effective capture から導出する 1 frame latch とし、capture 中ずっと rollback を繰り返さない。
- overlay open helper は `InputFocus` clear と pending capture を同時に行い、presentation の
  `Node.display` 更新や次 frame の focus sync を待たない。
- raw `ButtonInput<KeyCode>` を mutate/clear しない。
- `UiIntent` は `Copy` を維持し、payload 付き入力状態を詰め込まない。
- F9 は恒久 docs 上 confirmation 必須。旧 direct `LoadRequested` をそのまま action handler に移さない。
- `has_in_progress_gesture` 中は F5 を生成しない。特に AreaEdit active drag の persisted `TaskArea` を
  rollback/commit 前に `Last` save へ渡さない。
- pause 中は Logic set が停止する。Familiar/Area action を Message や永続 queue に積まず、
  `ResolvedInputFrame` を空から置換して stale action を残さない。
- Escape は `PlayMode` だけを Normal にせず、Build/Move/Floor/Zone/Task/companion の owner state を
  実在する cleanup helper で消す。BuildingMove は未確定実体 transform を復元するモデルではなく、
  `clear_move_states` 相当で context を clear する。
- capture 開始は mode cancel ではない。未確定 gesture だけを owner helper で開始前へ戻し、AreaEdit は
  `TaskArea` だけでなく `Destination` / `ActiveCommand` も snapshot どおり復元する。
- AreaEdit action の context は current `State<PlayMode>` と `TaskMode` の両方で判定し、`NextState` を
  current state の代用にしない。
- resolver は world/list selection 前の snapshot を使う。selection-dependent/context-mutating action がある
  frame は全 selection ingress を抑止し、consumer が別 Entity を操作しないようにする。
- family が異なっても default compatible としない。Familiar C+Z、AreaEdit Ctrl+V+B、Space+F5 は
  conflict lane で 1 action へ絞る。
- `GameSystemSet::Interface` は Visual より後段。UiIntent bridge をどこに置いても同 frame consumption test を必ず行う。
- `NextState<PlayMode>` を current context として読まず、`State<PlayMode>` を使う。
- 新しい Bevy API が必要になった場合は 0.19 の local crate source または docsrs 一次情報を確認する。

### 参照必須ファイル

- `docs/proposals/gameplay-management-improvements-proposal-2026-07-17.md`
- `docs/architecture.md`
- `docs/state.md`
- `docs/tasks.md`
- `docs/save_load.md`
- `docs/debug-features.md`
- `docs/entity_list_ui.md`
- `docs/plans/archive/text-input-ui-plan-2026-07-05.md`
- `crates/bevy_app/src/plugins/input.rs`
- `crates/bevy_app/src/plugins/game.rs`
- `crates/bevy_app/src/interface/ui/interaction/systems.rs`
- `crates/bevy_app/src/interface/ui/interaction/{mode.rs,intent_context.rs,handlers/mode_toggle.rs}`
- `crates/bevy_app/src/interface/ui/list/{interaction.rs,drag_drop.rs}`
- `crates/bevy_app/src/systems/command/input.rs`
- `crates/bevy_app/src/systems/command/area_selection/shortcuts.rs`
- `crates/bevy_app/src/systems/save/state.rs`
- `crates/hw_ui/src/components.rs`
- `crates/hw_ui/src/setup/{dialogs.rs,settings_panel.rs,pause_menu.rs,bottom_bar.rs}`
- `crates/hw_ui/src/interaction/text_field.rs`
- `crates/hw_ui/src/list/resize.rs`
- `~/.cargo/registry/src/.../bevy_camera_controller-0.19.0/src/pan_camera.rs`
- `~/.cargo/registry/src/.../bevy_picking-0.19.0/src/lib.rs`

### 最終確認ログ

- 最終 `cargo fmt --all -- --check`: `2026-07-18 / pass（task-area camera 修正後）`
- 最終 `cargo check --workspace --locked`: `2026-07-18 / pass（task-area camera 修正後）`
- 最終 `python3 scripts/dev.py docs --check`: `2026-07-18 / pass`
- 最終 `git diff --check`: `2026-07-18 / pass`
- 最終 `cargo clippy --workspace --all-targets --locked -- -D warnings`: `2026-07-18 / pass（task-area camera 修正後）`
- 最終 `cargo test --workspace --locked`: `2026-07-18 / pass（bevy_app 160件・hw_ui 8件を含む）`
- 最終 rust-analyzer diagnostics: `2026-07-18 / workspace error 0、warning 0`
- 最終 `python3 scripts/dev.py verify`: `2026-07-18 / pass（task-area camera 修正後）`
- 未解決エラー: なし。manual scenario で判明した task-area 左ドラッグと PanCamera の競合を修正し、
  実 `PanCameraPlugin` event を含む自動回帰を追加した。`2026-07-18` に自動回帰で代替できない
  task-area camera、capture→release、背景 EditableText 非配送の重点実機受入が完了し、M4 を完了した。

### Definition of Done

- [x] M1〜M4 が完了している
- [x] 同一 physical chord が複数 semantic action へ解決されない
- [x] 排他的 `InputActionFamily` が frame 内最大 1 件で、同時 chord の priority test が成功する
- [x] cross-family compatibility が default-deny で、非可換 action が consumer 到達前に 1 件へ絞られる
- [x] text/modal/mode/Familiar/World/debug の priority test が成功する
- [x] overlay open request が同 frame に pending capture と InputFocus clear を成立させる
- [x] pause 中の skipped action が unpause 後へ遅延発火しない
- [x] Modal/Pause が open frame から world pointer/camera input を panel 外でも遮断し、PanCamera Transform が変化しない
- [x] capture 開始時の未確定 gesture が owner state ごとに rollback され、release edge 消失後も残らない
- [x] active gesture 中の SaveGame が抑止され、未確定 persisted state を保存しない
- [x] world/Entity List selection ingress と drag/resize が action/capture snapshot を破壊しない
- [x] 全 active PlayMode の Escape が owner state を残さず完了する
- [x] project-owned production keyboard shortcut が resolver 経由になっている
- [x] PanCamera/TextInput/mouse/visual_test の非対象経路が回帰していない
- [x] 影響ドキュメントが更新済み
- [x] rust-analyzer workspace diagnostics が error/warning 0 件
- [x] `python3 scripts/dev.py verify` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-07-17` | `Codex` | 初版作成。A1 を M1〜M4 の独立実装単位として具体化 |
| `2026-07-17` | `Codex` | pause 中の frame lifetime、Modal/Pause capture、全 mode Escape cleanup、F5/F9/D の既定割当を現行コード照合後に確定 |
| `2026-07-17` | `Codex` | 最終レビューを反映。`UiSystems::Focus` 後の capture 順序、drag rollback、OpenMenu Escape、AreaEdit current-state gate、排他的 action family を受入契約へ追加 |
| `2026-07-17` | `Codex` | 再レビュー指摘を反映。pending overlay capture、InputFocus clear、Picking 前 PanCamera guard、cross-family conflict、frame-start selection、gesture 中 Save 抑止、全 owner/Entity List rollback を追加し、M1/M2/M3 の実装・受入境界を修正 |
| `2026-07-17` | `Codex` | M1完了。frame-local resolver、exact chord、binding data priority/compatibility、F5/F9 UiIntent bridge、V consumerを実装し、debug aliasとdirect F9 loadを削除。恒久docsと回帰testを同期 |
| `2026-07-17` | `Codex` | M2完了。B/Z/Space/Digit、Modal/Pause、Familiar、context別Escapeをresolverへ移行し、frame-start selection抑止、TaskMode/pending遷移を含む共通owner cleanup、overlay open時のInputFocus clearを実装。同一PlayModeへの冗長なpendingはblockerから除外 |
| `2026-07-17` | `Codex` | M3a完了。AreaEdit exact chord/Shift snapshot、Tab、P/O、F3/F4/F6/F7/F8/F12をresolverへ移行し、production raw keyboard readerをresolverとwhitelistへ限定 |
| `2026-07-17` | `Codex` | M3b実装。pending/visible overlay capture、viewport blocking root、foreground UI gate、capture開始時gesture rollback、Entity List reset、PanCamera/Picking orderingを導入。自動検証を完了し、実drag/releaseはM4手動受入へ明記 |
| `2026-07-17` | `Codex` | M4自動回帰と恒久docs同期を完了。全capture overlay受理と全gesture variant/AreaEdit history非破壊testを追加し、verify・docs・rust-analyzerを通過。実pointer/keyboardのmanual scenarioは未完了として維持 |
| `2026-07-18` | `Codex` | M4手動確認で判明した task-area 左ドラッグと PanCamera の競合を修正。current mode、同 frame resolver action、TaskArea border press を先読みする release-sticky claim と実 PanCamera Transform 回帰を追加し、verify・docs・rust-analyzerを再通過。実機再確認は未完了として維持 |
| `2026-07-18` | `Codex` | 自動回帰に加え、ユーザー実機確認で task-area camera、capture 中 release、overlay open 時の text focus/背景入力遮断を重点受入。M4 と Definition of Done を完了し、計画をアーカイブ |
