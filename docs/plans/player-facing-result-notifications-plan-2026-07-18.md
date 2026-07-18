# Track A2 プレイヤー向け結果通知 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `player-facing-result-notifications-plan-2026-07-18` |
| ステータス | `In Progress` |
| 作成日 | `2026-07-18` |
| 最終更新日 | `2026-07-18` |
| 作成者 | `Codex` |
| 関連提案 | `docs/proposals/gameplay-management-improvements-proposal-2026-07-17.md`（Track A2） |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: 配置不能理由とセーブ/ロードの終端結果をゲーム画面から確実に確認できない。
- 現状の内訳:
  - 配置可否は `PlacementRejectReason` まで計算できる経路がある一方、建築ゴーストや確定操作では
    `bool` / `Option` へ縮退し、プレイヤーには主に赤・緑の色だけが見えている。
  - Floor / Wall は確定後の失敗理由を約 2 秒表示するが、確定前の選択範囲には理由が出ず、
    BuildingMove と SoulSpa は共通の型付き理由へ到達していない。
  - セーブ/ロードの終端結果はログが中心で、画面上では対象と成功・失敗を確実に確認できない。
  - 単純に全結果をトースト化すると、配置プレビューや連続失敗で表示とメモリ使用量が増え続ける。
- 到達したい状態:
  - 一過性のトーストと有界な重要通知履歴を共通の UI 契約で表示する。
  - 全配置モードが確定判定と同じ型付き検証結果をプレビューへ渡し、無効時は確定前に理由を表示する。
  - セーブ/ロードの排他処理が終端結果を型付きで発行し、UI adapter が安全な文言へ変換する。
  - 通知はドメイン処理を観測するだけで、成功・失敗、再試行、world replacement の判断へ影響しない。
- 成功指標:
  - BuildingPlace、Tank の BucketStorage companion、BuildingMove、SoulSpaPlace、FloorPlace、WallPlace の
    無効な候補で、確定前に理由が分かる。
  - Floor / Wall の「一部の有効 tile だけを採用する」既存仕様を維持し、部分成功を全体失敗と表示しない。
  - セーブ/ロード操作の完了後に、操作種別、現在の対象、成功または表示用失敗分類が画面に残る。
  - 同じ通知キーの連続発生は集約され、表示中トースト 3 件、重要履歴 64 件を超えない。

## 2. スコープ

### 対象（In Scope）

- `hw_ui` が所有する、UI 専用の結果通知契約:
  - `UserFacingNotification` Message
  - severity、保持方針、安定した dedupe key
  - 表示中トーストと重要履歴を持つ `NotificationCenter`
  - トースト、履歴ボタン、履歴パネルの入力・描画
- `PlacementRejectReason` を正本とする配置フィードバック:
  - BuildingPlace と通常の建築物
  - Tank 配置に伴う BucketStorage companion
  - BuildingMove と Tank companion の移動
  - SoulSpaPlace
  - FloorPlace / WallPlace のドラッグ範囲
- 現行の単一 `SavePath` に対するセーブ/ロード終端結果:
  - `Succeeded` / `Failed`
  - `Save` / `Load`
  - ファイル名から作る表示安全な対象ラベル
  - 内部エラー文字列と分離した失敗分類
- load 成功、live apply 後の rollback 成功、rollback 失敗を含む world replacement 境界の通知保持。
- reducer、配置 validator、save/load outcome、reset/order を対象とする自動テスト。
- 実装完了時の通知、配置、セーブ/ロード、イベント、crate 境界ドキュメントの同期。

### 非対象（Out of Scope）

- Track A3 のタスク停止理由、フィルタ、キャンセル UI。
- Track B〜D の運営ポリシー、解体、進行システム。
- C2 の `SaveCatalog`、複数スロット、表示名編集、上書き確認、オートセーブ。
- 通知時間・件数の設定画面、通知音、最終アート、ローカライズ基盤。
- キーボードショートカットやゲームパッドでの通知履歴操作。
- 配置ルール自体、Floor / Wall の部分成功仕様、セーブ形式、world replacement transaction の変更。
- ログの廃止。詳細な内部エラーは引き続きログへ残す。
- 通知履歴のセーブデータ化。履歴は現在ロード中の world に属する UI runtime state とし、
  world replacement では stale な対象・文脈を残さないため reset する。

## 3. 現状とギャップ

### 3.1 配置

| 経路 | 現状 | 本計画で埋めるギャップ |
| --- | --- | --- |
| BuildingPlace | `validate_building_placement()` は `PlacementValidation` を返すが、ghost と確定側は主に `can_place` / `None` だけを使う | ghost と確定側が同じ型付き結果を使い、無効理由を live feedback へ渡す |
| Tank companion | typed validator はあるが、確定 API は `bool` を返す | 親と companion のどちらが失敗しても typed reason を維持する |
| BuildingMove | `can_place_moved_building()` が `bool` だけを返す | 自己占有を除外する既存規則を保った `PlacementValidation` へ置き換える |
| SoulSpaPlace | ghost と click が別々の `all(bool)` 判定を持つ | `BuildingType::SoulSpa` の共通 validator / geometry へ統合する |
| Floor / Wall | release 後の全体失敗だけ `PlacementFailureTooltip` に文字列表示する | drag 中も同じ area plan を評価し、全体失敗と部分成功を区別する |

`PlacementRejectReason` は現在 14 variant ある。表示文言は理由 enum から生成し、validator 内へ
UI 文字列やトースト送信を持ち込まない。既存 `PlacementFailureTooltip` は文字列 1 件の
last-write-wins resource であり、汎用通知センターには流用しない。

### 3.2 セーブ/ロード

- `SaveLoadState` は `Idle` / `SaveRequested` / `LoadRequested` の dispatcher trigger である。
  `Succeeded` / `Failed` を追加すると、終端状態の再実行や次要求の block を招くため変更しない。
- `save_world_system()` / `load_world_system()` は失敗をログして `()` を返す。
  save が 100 ms を超えた成功時は warning のみなので、ログの種類を結果の正本にはできない。
- load は read、format、seed、deserialize、schema、preflight、rehydrate prerequisite、live apply を
  区別できるが、その情報を表示安全な分類として外へ返していない。
- `RequestLoadGame` は対象ファイルがない場合に UI handler 内の warning だけで終了し、
  load owner へ要求が届かない。
- live world の置換時は登録済み Message / UI runtime state が clear される。
  load の結果を reset 前に書くと成功時や rollback 時に消える。

### 3.3 UI と Message

- speech / visual 用 Message はあるが、一般的なトーストと履歴の UI はない。
- `UiMountSlot::Overlay`、theme、既存 UI input gate は再利用できる。
- full-screen mount は `Pickable::IGNORE` / `FocusPolicy::Pass` であり、トーストも world input を
  遮らない構造にできる。履歴ボタンと開いたパネルだけを通常の UI input blocker とする。
- `hw_ui::reset_for_world_replace()` は UI-owned Message と Entity 参照を消去する正式な境界である。

## 4. 実装方針（高レベル）

### 4.1 責務境界

```text
配置 validator ──typed result──> PlacementFeedbackState ──> カーソル付近の理由表示
       │                                  （通知履歴へは送らない）
       └──────────────> 確定処理（同じ pure helper で再検証）

SavePlugin ──SaveLoadOutcome──> bevy_app adapter ──UserFacingNotification──> hw_ui reducer/UI
  成否の正本                 表示文言だけを構築                 bounded toast/history
```

- ドメイン結果と内部失敗分類: `crates/bevy_app/src/systems/save/`。
- 汎用通知 Message、reducer、runtime state、UI renderer: `crates/hw_ui/src/notifications/`。
- `SaveLoadOutcome` から表示用通知への変換: root の `bevy_app` UI adapter。
- 配置 validation の正本: `hw_ui::selection::placement` の pure API。
- 配置の WorldMap / Query adapter と commit: 現在どおり root shell。

### 4.2 汎用通知契約

`UserFacingNotification` は少なくとも次の情報を持つ。

| フィールド | 契約 |
| --- | --- |
| `key` | 表示文言ではなく、source / failure class / target から作る安定した機械用キー |
| `severity` | `Info` / `Success` / `Warning` / `Error` |
| `title`, `body` | UI に表示可能な文字列。raw OS error、絶対 path、debug dump は入れない |
| `retention` | `ToastOnly` または `Important`。後者だけ履歴へ残す |

`NotificationCenter` は純粋な reducer 関数を中心に実装し、初期値を次で固定する。

- entry は stable id、key、severity、title/body、retention、first/last-seen real time、
  `repeat_count` を持つ。重要通知の toast と履歴は同じ entry id を参照し、別々に count を増やさない。
- 同一 key を 2 real-time 秒以内に受けた場合、最新 entry の `repeat_count` を増やして期限を更新する。
  incoming payload で表示内容を更新し、retention は `Important` を優先する。新しい履歴行は追加しない。
- 表示中トーストは最大 3 件とし、別の無制限 pending queue は持たない。超過時は最古を外す。
- トースト寿命は 4 real-time 秒。ゲーム速度や Pause に左右されない。
- `Important` 履歴は最大 64 件。超過時は最古を削除する。
- expired / coalesced / appended / history-open-state-change のときだけ dirty にし、
  変化のないフレームに UI 子 Entity と文字列を再構築しない。

重要通知履歴は top-right のボタンから開く。未読件数は history capacity 以下に保ち、開いた時点で既読化する。
トースト root は `UiMountSlot::Overlay` 配下の `ZIndex(45)` とし、root だけでなく toast row と全子要素へ
`Pickable::IGNORE` / `FocusPolicy::Pass` を適用する。既存 dialog より前面、tooltip / drag ghost より背面にする。
履歴パネルとそのボタンだけは `UiInputBlocker` を持ち、A1 の foreground capture gate に従う。
Modal / Pause capture 開始時は開いていた履歴パネルを閉じ、foreground UI を視覚的にも覆わない。
履歴用 shortcut は追加しない。

通知 system は root facade で `NotificationSystemSet::{Adapt, Reduce, Present}` の順に構成する。
save/load outcome adapter は `Adapt`、Message ingest / expiry / reducer は `Reduce`、dirty UI renderer は
`Present` へ置く。同じ `Update` で adapter が書いた通知を reducer が読み、表示へ反映できる順序をテストする。
world replacement reset は resource と Message だけでなく、動的な toast/history row Entity を despawn し、
static root の表示と node index を初期状態へ戻す。reset 後の古い文字列 Entity を残さない。

### 4.3 配置フィードバック契約

`PlacementFailureTooltip` を typed な `PlacementFeedbackState` へ置き換える。

- `live`: 現フレームにカーソル下で得た `PlacementFeedback`。
- `recent_failure`: commit 時に失敗した理由を 2 real-time 秒だけ保持する latch。
- `PlacementFeedback` は最低でも status（`Rejected` / `Partial`）、reason、対象 grid、
  area の場合は valid / rejected tile 数を持つ。
- `live` は `GameSystemSet::Visual` 冒頭で毎フレーム clear し、各 producer が再設定する。
  cursor 不在、モード終了、UI 上への移動で古い理由を残さない。
- presenter は `live` を優先し、なければ `recent_failure` を表示する。
- `Rejected` / recent failure は `Cannot place`、`Partial` は `Some tiles will be skipped` のように
  成否を誤認しない別 header と色を使う。partial を赤い全体失敗として表示しない。
- 配置 feedback は連続状態であり、`UserFacingNotification` を毎フレーム発行しない。

各経路の pure validation を次のように揃える。

- `can_place_moved_building()` を `validate_moved_building_placement()` に改め、
  `PlacementValidation` を返す。既存の「移動元の自己占有は許可する」条件を維持する。
- SoulSpa ghost / click の重複 `all(bool)` を、既存 `building_geometry()` と
  `validate_building_placement(BuildingType::SoulSpa, ...)` を使う一つの helper へ寄せる。
- BuildingPlace と Tank companion の commit API は失敗時に reason を返し、ghost と同じ validator を
  再実行する。preview cache を成功判定の正本にはしない。
- Floor / Wall は `AreaPlacementPlan` を作る pure helper を preview と commit で共有する。
  構造的な area reject または valid tile 0 件だけを `Rejected` とし、valid tile が 1 件以上なら
  現行どおりその tile を採用する。無効 tile が混ざる場合は `Partial` と件数・最初の理由を表示する。

system order は次を明示する。

1. Visual: live feedback clear。
2. Visual: BuildingPlace / SoulSpa ghost validation と feedback produce。
3. Interface: BuildingMove / Floor / Wall preview validation と feedback produce。
4. Interface: placement feedback present。
5. Interface: click / release commit。commit は同じ helper で再検証し、失敗時だけ recent latch を更新する。

`GameSystemSet::Visual` → `GameSystemSet::Interface` の既存順序に加え、Interface 内は専用
`PlacementFeedbackSet::{Produce, Present, Commit}` または同等の明示的な `.before()` / `.after()` を使う。
tuple の暗黙順序には依存しない。

### 4.4 セーブ/ロード終端結果

`SaveLoadState` と別に、`SavePlugin` 所有の `SaveLoadOutcome` Message を追加する。

```text
SaveLoadOutcome
├─ operation: Save | Load
├─ target: 表示用の現在対象ラベル
└─ result: Succeeded | Failed(SaveLoadFailureKind)
```

`SaveLoadFailureKind` は raw error text を持たず、最低限を次へ分類する。

| 分類 | 主な発生箇所 | 表示の意味 |
| --- | --- | --- |
| `SaveSerialize` | DynamicWorld encode | セーブデータを作成できない |
| `SaveWrite` | temp file / sync / rename | セーブファイルを書き込めない |
| `LoadNotFound` | read の `NotFound` | 対象セーブがない |
| `LoadRead` | その他 read I/O | 対象セーブを読めない |
| `UnsupportedFormat` | header version | 対応していない形式 |
| `InvalidData` | header/body/deserialize/schema/preflight | セーブ内容が無効または破損 |
| `SeedMismatch` | worldgen seed guard | 現セッションと seed が一致しない |
| `MissingPrerequisite` | registry/assets/rehydrate prerequisite | 現在の実行環境ではロード準備できない |
| `ApplyRecovered` | live apply 失敗、rollback 成功 | ロード失敗。元の world は復旧済み |
| `RecoveryFailed` | live apply と rollback の両方が失敗 | 復旧にも失敗した重大エラー |

save/load の全 terminal outcome は `Important` とする。`Succeeded` は `Success`、`LoadNotFound` と
`ApplyRecovered` は `Warning`、その他の失敗は `Error`（`RecoveryFailed` は重大であることを title にも示す）へ
写像する。dedupe key は `(save_load, operation, target, result-kind)` から作り、同じ対象の成功と失敗、
save と load を誤って一つに集約しない。

詳細文字列は既存どおり severity に応じてログへ残す。UI adapter は分類の exhaustive match で
固定文言へ変換し、絶対 path は表示しない。現行 `SavePath` の `file_name()` が有効ならそれを対象表示に使い、
それ以外は `Current save` のような安全な fallback とする。C2 はこの target field を slot label へ
差し替えられるが、A2 では `SaveSlotId` / `SaveCatalog` を先取りしない。

dispatcher は要求を処理する直前に `SaveLoadState::Idle` へ戻し、save / load 関数から得た
terminal outcome を処理完了後に 1 回だけ書く。`RequestLoadGame` の file-exists 判定は結果の正本にせず、
対象がある場合だけ従来の確認 dialog を開く。対象がない場合は確認 dialog を開かず `LoadRequested` を設定し、
load owner の read が `LoadNotFound` を発行する。判定後にファイルが消える race も同じ owner 経路で分類する。

load の順序は必ず次とする。

1. request を `Idle` へ戻す。
2. read / prepare / preflight / prerequisite / transaction を実行する。
3. 成功または rollback の最終結果を確定する。
4. world replacement に伴う全 reset が終わった後で `SaveLoadOutcome` を書く。
5. 次の `Update` で root adapter が読み、`UserFacingNotification` を書く。

`SavePlugin` は単独テスト可能性を維持するため、自身で `SaveLoadOutcome` を登録し、save feature 用の
world-replace reset hook で clear する。`HwUiPlugin` は notification Message / center / dynamic row を
`hw_ui::reset_for_world_replace()` に含め、root UI facade が既存の `hw-ui` hook へ接続する。
成功 load では旧 world の通知履歴を消した後、load 成功が新しい最初の重要通知になる。
transaction 前の load 失敗は world を置換しないため、既存履歴を保ったまま失敗を追記する。
rollback 経路では reset が複数回走っても、最後の reset 後に発行した outcome が残ることをテストする。

失敗経路のテストは production 分岐を `cfg(test)` で置換しない。save は内部の
`execute_save_with(encode, write)`、load/transaction は既存の post-write / finalizer 注入点と同等の小さな
関数境界へ分け、同じ error-to-outcome mapping を production と test が通るようにする。filesystem test は
一意な `/tmp` 配下だけを使い、repository の `savegame.ron` や既存セーブを変更しない。

### 4.5 Bevy 0.19 API での注意点

- 通知 expiry、dedupe window、配置 recent latch は `Time<Real>` を使う。
  UI の実時間アニメーションは `Time<Virtual>` に依存させず、Pause 中も期限を進める。
- Message の reader/writer は Bevy 0.19 の `MessageReader` / `MessageWriter` を使い、旧 `EventReader` API を使わない。
- world replacement 中の Message clear は owner の `Messages<T>::clear()` を通し、reader cursor や二重 buffer を
  推測で直接操作しない。
- UI overlay は Bevy 0.19 の `Pickable`、`FocusPolicy`、`Interaction` の既存実装を踏襲する。
- 実装前に該当 API のシグネチャをローカルの Bevy 0.19 source または docs.rs で再確認する。

## 5. マイルストーン

## M1: 有界な通知契約と UI センター

- 変更内容:
  - `hw_ui::notifications` に Message、severity、retention、entry、reducer、runtime state を追加する。
  - 2 秒 dedupe、4 秒 expiry、toast 3 件、history 64 件、repeat count の純粋テストを先に追加する。
  - `HwUiPlugin` が Message / resource を所有し、`reset_for_world_replace()` で Message、queue、history、
    unread、開閉状態を reset する。
  - Overlay に非 pickable な toast stack、top-right に履歴ボタン、有界な履歴パネルを追加する。
  - Message ingest、real-time expiry、dirty renderer、履歴 open/close を明示順序で登録する。
- 変更ファイル:
  - `crates/hw_ui/src/notifications/mod.rs`（新規）
  - `crates/hw_ui/src/notifications/model.rs`（新規候補）
  - `crates/hw_ui/src/notifications/reducer.rs`（新規候補）
  - `crates/hw_ui/src/notifications/ui.rs`（新規候補）
  - `crates/hw_ui/src/lib.rs`
  - `crates/hw_ui/src/intents.rs`
  - `crates/hw_ui/src/components.rs`
  - `crates/hw_ui/src/setup/root.rs`
  - `crates/hw_core/src/ui_nodes.rs`
  - `crates/bevy_app/src/interface/ui/plugins/`（root facade の system 登録）
  - `docs/notifications.md`（新規。M1 の公開契約を記録）
  - `docs/README.md`
  - `docs/events.md`
  - `docs/cargo_workspace.md`
  - `crates/bevy_app/src/interface/ui/README.md`
- 完了条件:
  - [x] 同一 key の 2 秒以内の通知が 1 entry と repeat count へ集約される。
  - [x] トーストと重要履歴が上限を超えない。
  - [x] `ToastOnly` は履歴へ入らず、`Important` は入る。
  - [x] Pause 中も `Time<Real>` でトーストが expire する。
  - [x] 非表示または変化なしのフレームに UI 子 Entity を再生成しない。
  - [x] toast root は world input を capture せず、履歴 panel だけが hover / click を block する。
  - [x] Modal / Pause capture 開始時に履歴 panel が閉じ、toast だけが入力透過で残る。
  - [x] world replacement reset 後に通知 Message、履歴、unread、UI 開閉状態、動的 row Entity が残らない。
  - [x] M1 時点の公開契約と owner / reset が durable docs に記録されている。
- 検証:
  - `cargo test -p hw_ui notifications`
  - `python3 scripts/dev.py docs --write`
  - `python3 scripts/dev.py docs --check`
  - `cargo check --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- ロールバック境界:
  - producer 未接続の UI-only milestone として単独で戻せる。

## M2: 全配置モードの型付き live feedback

- 変更内容:
  - `PlacementFailureTooltip` を live / recent を分けた `PlacementFeedbackState` へ移行する。
  - 14 variant の formatter と validator matrix を拡充する。
  - BuildingPlace、Tank companion、BuildingMove、SoulSpa の preview / commit が typed result を共有する。
  - Floor / Wall の area plan を pure helper 化し、preview / commit で同じ valid tile 集合と最初の理由を使う。
  - Visual / Interface 内の produce、present、commit 順を system set で固定する。
  - 文字理由をカーソル付近に表示し、色だけを正本にしない。
- 変更ファイル:
  - `crates/hw_ui/src/selection/placement.rs`
  - `crates/hw_ui/src/selection/placement/validation.rs`
  - `crates/hw_ui/src/selection/placement/tests.rs`
  - `crates/hw_ui/src/components.rs`
  - `crates/hw_ui/src/interaction/tooltip/system.rs`
  - `crates/bevy_app/src/systems/visual/placement_ghost.rs`
  - `crates/bevy_app/src/interface/selection/building_place/`
  - `crates/bevy_app/src/interface/selection/building_move/`
  - `crates/bevy_app/src/interface/selection/soul_spa_place/`
  - `crates/bevy_app/src/interface/selection/floor_place/`
  - `crates/bevy_app/src/interface/ui/plugins/core.rs`
  - `crates/bevy_app/src/interface/ui/plugins/tooltip.rs`
  - `docs/notifications.md`
  - `docs/building.md`
  - `docs/architecture.md`
  - `docs/invariants.md`
  - `crates/bevy_app/src/interface/README.md`
- 完了条件:
  - [x] 全 14 `PlacementRejectReason` が表示文言を持ち、panic / empty text にならない。
  - [x] BuildingPlace、BucketStorage companion、BuildingMove（Tank companion を含む）、SoulSpa、Floor、Wall の
    全経路で preview と commit が同じ typed validator / area plan を使う。
  - [x] UI 上、cursor 不在、mode 終了の次フレームに live reason が消える。
  - [x] commit は cached preview を信用せず再検証する。
  - [x] Floor / Wall は valid tile 1 件以上なら従来どおり部分成功し、invalid tile を生成しない。
  - [x] 全体失敗は既存どおり recent reason を短時間表示する。
  - [x] live feedback から毎フレーム notification Message が増えない。
  - [x] 配置対象、typed validation、部分成功、system order が durable docs に記録されている。
- 検証:
  - `cargo test -p hw_ui placement`
  - `cargo test -p bevy_app placement`
  - `python3 scripts/dev.py docs --write`
  - `python3 scripts/dev.py docs --check`
  - `cargo check --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- ロールバック境界:
  - M1 の汎用通知を残したまま、配置 adapter と typed feedback だけを戻せる。

## M3: セーブ/ロード終端 outcome と通知 adapter

- 変更内容:
  - `SaveLoadOutcome`、operation、result、表示安全な failure kind / target label を追加する。
  - dispatcher が state を先に `Idle` へ戻し、save / load の返り値を outcome として 1 回発行する。
  - load preparation / transaction の内部エラーを exhaustive に display-safe classification へ写像する。
  - missing-file 時は確認 dialog を省略して load owner へ要求し、terminal outcome の正本を一本化する。
  - encode / write / post-write / recovery を安全に失敗させられる内部 test seam を設ける。
  - root adapter が outcome を重要通知へ変換し、成功・警告・重大失敗の severity と dedupe key を付ける。
  - owner ごとの Message / runtime reset と、成功 / recovery 後の発行順を固定する。
- 変更ファイル:
  - `crates/bevy_app/src/systems/save/state.rs`
  - `crates/bevy_app/src/systems/save/mod.rs`
  - `crates/bevy_app/src/systems/save/saving.rs`
  - `crates/bevy_app/src/systems/save/load.rs`
  - `crates/bevy_app/src/systems/save/format.rs`
  - `crates/bevy_app/src/systems/save/transaction.rs`
  - `crates/bevy_app/src/interface/ui/interaction/handlers/save_game.rs`
  - `crates/bevy_app/src/interface/ui/notifications.rs`（新規候補）
  - `crates/bevy_app/src/interface/ui/plugins/`（adapter 登録）
  - `docs/notifications.md`
  - `docs/save_load.md`
  - `docs/events.md`
  - `docs/architecture.md`
  - `docs/invariants.md`
- 完了条件:
  - [x] `SaveLoadState` は trigger 3 variant のままで、成功・失敗を状態として保持しない。
  - [x] save success / serialize failure / write failure がそれぞれ 1 terminal outcome を発行する。
  - [x] load の missing、read、format、invalid data、seed、prerequisite、apply recovered、recovery failed が分類される。
  - [x] UI 文言に raw error と絶対 path が含まれず、ログには診断詳細が残る。
  - [x] load 成功後に旧履歴は消え、load 成功通知が次 Update で読める。
  - [x] rollback 成功 / 失敗の reset 後にも対応する failure outcome が 1 件読める。
  - [x] 同一 target / failure の連続結果が通知センターで集約される。
  - [x] terminal outcome、failure class、reset 後の発行順が durable docs に記録されている。
- 検証:
  - `cargo test -p bevy_app systems::save`
  - `cargo test -p bevy_app save_game`
  - `python3 scripts/dev.py docs --write`
  - `python3 scripts/dev.py docs --check`
  - `cargo check --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- ロールバック境界:
  - save/load の既存ログを残したまま outcome / adapter を外せる。セーブ形式と transaction は変えない。

## M4: 文書同期と統合受入

- 変更内容:
  - 通知の表示、上限、dedupe、reset、producer/consumer を durable docs に記録する。
  - 配置モード別の typed validation と部分成功契約を更新する。
  - セーブ/ロードの terminal outcome と world replacement 後の発行順を更新する。
  - Message inventory、crate owner、system order、invariant を同期する。
  - A2 完了後に関連提案の実装状態を更新し、本計画を archive する。
- 変更ファイル:
  - `docs/notifications.md`
  - `docs/README.md`
  - `docs/building.md`
  - `docs/save_load.md`
  - `docs/events.md`
  - `docs/architecture.md`
  - `docs/cargo_workspace.md`
  - `docs/invariants.md`
  - `crates/bevy_app/src/interface/README.md`
  - `crates/bevy_app/src/interface/ui/README.md`
  - `docs/proposals/gameplay-management-improvements-proposal-2026-07-17.md`
  - `docs/plans/README.md`
- 完了条件:
  - [x] A2 の公開契約、owner、reset、上限、順序が code と docs で一致する。
  - [x] 提案の 3 受入条件を自動テストと手動確認へ対応付けられる。
  - [ ] docs index が最新で、完了計画が archive されている。
  - [x] full quality gate が成功する。
- 検証:
  - `python3 scripts/dev.py docs --write`
  - `python3 scripts/dev.py docs --check`
  - `python3 scripts/dev.py verify`
  - `git diff --check`
- ロールバック境界:
  - code rollback と同じ milestone の文書だけを同時に戻す。

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 配置 preview と commit の検証が再び分岐する | 表示では可能だが確定失敗、または逆が起きる | World adapter だけを分け、pure validator / area plan を共有する。commit は再検証する |
| Floor / Wall を一括 bool 化する | 一部 valid tile を採用する既存仕様が壊れる | valid tile 集合と first reject を持つ `AreaPlacementPlan` を使い、0 件だけ全体失敗にする |
| live placement をトーストへ送る | 毎フレーム allocation / dedupe と表示 churn が起きる | live typed resource と result notification Message を完全に分ける |
| notification queue / history が増え続ける | 長時間プレイでメモリと UI が増える | toast 3、history 64、pending queue なし、2 秒 dedupe を reducer invariant としてテストする |
| Pause 中に expiry が止まる | 古いトーストが残り続ける | Bevy 0.19 の `Time<Real>` を使用する |
| 非 pickable toast が UI input を遮る | A1 で直した world input ownership が退行する | display root を `Pickable::IGNORE` / `FocusPolicy::Pass` にし、interaction test と手動確認を行う |
| save/load outcome を `SaveLoadState` に入れる | 再実行または次要求 block | terminal result は別 Message とし、state は処理前に `Idle` へ戻す |
| load outcome を reset 前に書く | 成功または rollback 時に通知が消える | transaction の全 reset と最終結果確定後に 1 回だけ発行する |
| raw I/O / deserialize error を UI へ出す | path や内部型が漏れ、文言も不安定になる | display-safe enum を exhaustive match し、raw detail はログだけに残す |
| C2 の slot model を先取りする | A2 が肥大化し、将来の正本と競合する | A2 は current `SavePath` の安全な label のみ。target field を将来差し替え可能にする |
| UI renderer を毎フレーム rebuild する | Entity churn と allocation が増える | reducer の dirty revision と node cache を使い、変化時だけ差分反映する |
| reset を複数 owner に重複登録する | 二重登録 assert や状態の取り残し | Save outcome は `SavePlugin` の専用 hook、notification は `hw_ui` の既存 owner reset function に集約し、root facade から各 1 回だけ接続する |

## 7. 検証計画

### 7.1 自動テスト

- 通知 reducer:
  - same key / window 内、window 境界外、different key。
  - repeat count と expiry 延長。
  - toast 3 件、history 64 件、古い entry の eviction。
  - `ToastOnly` / `Important`、unread、open 時の既読化。
  - `Time<Real>` 相当の elapsed input による Pause 非依存 expiry。
  - `Adapt` が書いた Message を同じ Update の `Reduce` が読み、`Present` が dirty state を反映する。
  - Modal / Pause capture 開始時の history close と、全 toast descendant の pick-through。
- 配置:
  - 14 reason の formatter が空でない。
  - building、bridge、door、site、yard、occupancy、companion radius の validator matrix。
  - moved building の self occupancy と他 entity / stockpile / out-of-bounds。
  - SoulSpa preview と commit の同一 result。
  - Floor / Wall の全 valid、部分 valid、0 valid、area too large、not straight line。
  - cursor 不在 / mode 解除で live state が次フレームに残らない。
- セーブ/ロード:
  - success と一意な temp path を使った write failure。実セーブファイルは触らない。
  - injected encoder failure から `SaveSerialize`、injected writer failure から `SaveWrite` が発行される。
  - missing file、read failure、unsupported format、malformed body、seed mismatch、schema / prerequisite failure。
  - injected post-write failure の `ApplyRecovered` / `RecoveryFailed`。
  - state が失敗後も `Idle`、各 request が outcome 1 件だけを発行。
  - successful replacement と recovery reset 後に outcome が生存し、adapter が次 Update で 1 通知へ変換。
  - UI 向け target / body に絶対 path、raw error が含まれない。
- reset / plugin isolation:
  - `SavePlugin` 単独で outcome Message を登録・発行できる。
  - `RequestLoadGame` は missing 時に dialog を開かず load を要求し、存在時だけ確認を開く。
  - `HwUiPlugin` reset が notification Message / center / UI runtime state / dynamic row Entity を消す。
  - root adapter がない構成でも save/load の成否が変わらない。

### 7.2 必須コマンド

- マイルストーン中:
  - `cargo check --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 計画完了時:
  - `python3 scripts/dev.py docs --write`
  - `python3 scripts/dev.py docs --check`
  - `python3 scripts/dev.py verify`
  - `git diff --check`

### 7.3 手動確認シナリオ

1. 通常建築、Bridge、Door、Tank companion を有効 / 無効 tile へ動かし、赤色だけでなく理由が確定前に見える。
2. BuildingMove で元 footprint と重なる移動は可能、他建築 / stockpile / map 外は理由付きで拒否される。
3. SoulSpa を Yard 内外、occupied tile へ動かし、ghost と click の結果が一致する。
4. Floor / Wall をドラッグし、全無効では理由、部分有効では valid / skipped の要約が出て valid tile だけ生成される。
5. `F5` 保存後、対象ラベルと成功がトーストと重要履歴に出る。
6. 保存先をテスト用の書込不能条件にし、ゲームが継続したまま安全な失敗分類が出る。
7. セーブなしの `F9` は不要な確認 dialog を開かず失敗を表示する。破損データ、seed 不一致でも
   現在 world が維持されて分類済みの失敗が表示される。
8. 正常 load 後に旧履歴が消え、load 成功が新しい履歴の先頭になる。
9. 同じ失敗を短時間に繰り返し、行が増えず repeat count が増える。異なる失敗は別 entry になる。
10. Pause 中も toast が expire し、toast 上の click / drag / scroll が world input を不必要に block しない。
11. 履歴パネル上では world hover / click / camera input が block され、閉じると通常入力へ戻る。

### 7.4 パフォーマンス確認

- 5 分間の通常配置で通知 Message 数がフレーム数に比例して増えない。
- 通知の変化がない 300 フレームで toast/history child Entity 数が一定である。
- history 64 件到達後も Entity 数と resource 内 entry 数が増えない。
- `python3 scripts/perf.py run` が必要な大規模退行は想定しないが、UI Entity churn が観測された場合だけ
 既存 profiling fixture で同一 schema / fixture 比較を行う。

## 8. ロールバック方針

- M1、M2、M3 は独立して戻せる。M2 と M3 は M1 の API だけに依存し、相互依存させない。
- 各 milestone で公開 enum / Message を追加する変更、producer 接続、docs 同期を同じ commit 単位にする。
- M2 を戻す場合は `PlacementFeedbackState` と全 consumer を同時に戻し、途中で旧
  `PlacementFailureTooltip` と新 state を二重に残さない。
- M3 を戻す場合も save format / persisted schema は変えていないため、既存セーブとの移行作業は不要。
- load transaction の復旧機構は変更対象にせず、outcome 生成の adapter 層だけを外せる形にする。
- 実際の rollback 前には repository の Git Revert Policy に従い、履歴と対象 diff を確認する。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `95%`（M1〜M3、M4の実装・自動検証・恒久docs同期完了。重点実機受入とarchive待ち）
- 完了済みマイルストーン: `M1`、`M2`、`M3`
- 進行中: `M4`（重点実機受入）
- 未着手: なし

### 次のAIが最初にやること

1. §7.3の重点実機項目を確認する。特に全配置経路のpreview理由、Floor / Wall部分採用、F5/F9通知、load後の履歴resetを優先する。
2. 問題がなければ本計画を`docs/plans/archive/`へ移し、`python3 scripts/dev.py docs --write`と`verify`を再実行する。
3. 問題があれば該当milestoneのcode / test / durable docsを同じ変更で直す。

### ブロッカー/注意点

- 実装ブロッカーは現在なし。自動検証ではUIの実見た目とpointer操作感を代替できないため、archive前に重点実機受入を残す。
- 配置対象は BuildingPlace、BucketStorage companion、BuildingMove（Tank companion を含む）、SoulSpa、
  Floor、Wall の全経路である。
- `PlacementRejectReason` は 14 variant。数を推測せず enum と test を同時に更新する。
- Floor / Wall は invalid tile を skip する部分成功が既存仕様であり、全体 `bool` へ縮退させない。
- `SaveLoadState` に terminal state を増やさない。terminal outcome は別 Message。
- load outcome は `replace_persisted_world()` が成功 / recovery を完了した後に書く。
- live placement feedback と result notification queue を接続しない。
- `SavePath` の絶対 path と内部 error string を UI へ渡さない。
- Bevy API は 0.19 source で確認し、`Time<Real>` / Message API の旧版例を使わない。
- 他セッションの変更を破棄しない。rollback 時は AGENTS.md の Git Revert Policy を守る。

### 参照必須ファイル

- `docs/proposals/gameplay-management-improvements-proposal-2026-07-17.md`
- `docs/building.md`
- `docs/save_load.md`
- `docs/events.md`
- `docs/architecture.md`
- `docs/invariants.md`
- `crates/hw_ui/src/selection/placement.rs`
- `crates/hw_ui/src/selection/placement/validation.rs`
- `crates/hw_ui/src/components.rs`
- `crates/hw_ui/src/interaction/tooltip/system.rs`
- `crates/hw_ui/src/lib.rs`
- `crates/bevy_app/src/systems/visual/placement_ghost.rs`
- `crates/bevy_app/src/interface/selection/floor_place/`
- `crates/bevy_app/src/systems/save/mod.rs`
- `crates/bevy_app/src/systems/save/saving.rs`
- `crates/bevy_app/src/systems/save/load.rs`
- `crates/bevy_app/src/systems/save/transaction.rs`
- `crates/bevy_app/src/interface/ui/interaction/handlers/save_game.rs`

### 最終確認ログ

- 最終 `cargo check --workspace`: `2026-07-18` / pass（`python3 scripts/dev.py verify` 内）
- 最終 `cargo clippy --workspace --all-targets -- -D warnings`: `2026-07-18` / pass（同上）
- 最終 `cargo test --workspace`: `2026-07-18` / pass（同上）
- 最終 `python3 scripts/dev.py verify`: `2026-07-18` / pass
- 未解決エラー: なし

### Definition of Done

- [ ] M1〜M4 がすべて完了
- [ ] 提案 A2 の 3 受入条件を自動テストと手動確認で満たす
- [x] 影響ドキュメントが更新済み
- [x] `python3 scripts/dev.py docs --check` が成功
- [x] `cargo check --workspace` が成功
- [x] `cargo clippy --workspace --all-targets -- -D warnings` が成功
- [x] `cargo test --workspace` が成功
- [x] `python3 scripts/dev.py verify` が成功
- [ ] 完了した本計画が archive され、索引が最新

## 10. 受入条件トレーサビリティ

| 提案の受入条件 | 設計 / 実装 | 自動検証 | 手動確認 |
| --- | --- | --- | --- |
| 配置確定前に現在の不能理由が分かる | M2 typed live feedback、共通 validator / area plan | mode 別 preview / commit 一致、stale clear | 1〜4 |
| セーブ/ロード後に対象と成否を画面で確認できる | M1 toast/history、M3 terminal outcome / adapter | outcome classification、reset 後の生存、safe text | 5〜8 |
| 同一失敗で通知領域が無制限に増えない | 2 秒 dedupe、toast 3、history 64、pending queue なし | reducer capacity / repeat count | 9 |

## 11. 計画レビュー基準

実装開始前に本計画を再確認し、少なくとも次を満たさない場合は計画を修正する。

- 提案 A2 の 3 受入条件が milestone、test、手動確認へ追跡できる。
- 現行の型名、path、system order、reset owner と矛盾しない。
- load success / pre-transaction failure / recovered / recovery-failed の全経路で outcome の発行位置が一意である。
- toast、history、dedupe、UI rebuild の全てに上限または停止条件がある。
- Floor / Wall の部分成功と moved building の self occupancy を維持する。
- milestone を独立して実装・rollback できる。
- real save file を変更せずに主要失敗を注入・検証できる。
- C2 の slot catalog、A3 の task diagnostics、設定 / localization を先取りしていない。
- durable docs、AI handoff、完了後の archive 手順が明示されている。

## 12. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-07-18` | `Codex` | 初版作成 |
| `2026-07-18` | `Codex` | 自己レビューを実施。missing-load の owner 経路、adapter/reducer 順序、dynamic UI row reset、Modal/Pause capture、配置経路、失敗注入 seam、milestone ごとの文書同期、索引用要約を明確化 |
| `2026-07-18` | `Codex` | M1〜M3実装、M4の恒久docs同期とfull quality gateを完了。実rollback成功／失敗、outcome dedupe、全配置typed validationを回帰化。重点実機受入とarchiveを残す |
