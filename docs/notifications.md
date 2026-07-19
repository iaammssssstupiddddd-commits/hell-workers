# プレイヤー向け結果通知

配置できない理由とセーブ／ロードの終端結果を、ログを開かずゲーム画面で確認するための仕様。
通知の表示基盤は `hw_ui::notifications`、ゲーム固有結果から表示文言への変換は
`bevy_app::interface::ui::notifications` が所有する。

## 通知センター

`UserFacingNotification` Message は次の表示専用フィールドを持つ。

| フィールド | 契約 |
| --- | --- |
| `key` | source、対象、結果分類から作る安定した重複集約キー |
| `severity` | `Info` / `Success` / `Warning` / `Error` |
| `title`, `body` | プレイヤーへ表示可能な文言。raw error、絶対 path、debug dump を含めない |
| `retention` | `ToastOnly` または `Important`。後者だけ履歴へ残る |

`NotificationCenter` は次の有界な runtime state を持つ。

- 同一 key を 2 real-time 秒以内に受け取ると、同じ entry の `repeat_count` を増やし、内容と期限を更新する。
- 表示中トーストは最大 3 件、寿命は 4 real-time 秒。別の pending queue は持たない。
- `Important` 履歴は最大 64 件。超過時は最古を削除する。
- expiry、dedupe、履歴の開閉は `Time<Real>` を使うため、Pause とゲーム速度の影響を受けない。
- revision が変化したときだけ動的な toast/history row を再構築する。

重要通知履歴は画面右上の「通知」ボタンで開く。未読数は履歴上限以内に保ち、履歴を開いた時点で既読化する。
トーストと全子要素は picking-transparent で、world click や camera を遮らない。履歴ボタンと開いた履歴パネルだけが
`UiInputBlocker` である。Modal / Pause の foreground capture 開始時は履歴を閉じ、履歴ボタンを隠す。

## 配置フィードバック

配置プレビューは毎フレーム変化する連続状態なので、通知センターへ Message を送らない。
`PlacementFeedbackState` が現在の `live` feedback と、確定失敗を 2 real-time 秒保持する
`recent_failure`、成功直後の自己干渉表示を抑止する same-anchor blocker を分離して所有する。
表示は `live` を優先する。

`PlacementRejectReason` の14分類が表示文言の正本であり、`PlacementValidation` は最初に拒否された
実タイル座標も保持する。次の経路は preview と commit で同じ validator または area plan を使い、
commit 時にも必ず再検証する。

| 経路 | 共通判定 |
| --- | --- |
| 通常建築 / Tank | `validate_building_placement`。Tank companion は `validate_bucket_storage_placement` |
| BuildingMove | 自己占有だけを許可する `validate_moved_building_placement`。Tank companion は移動用 validator |
| SoulSpa | `building_geometry(BuildingType::SoulSpa)` と共通 validator。単一 Yard が下向き2×2 footprint全体を含むこと |
| Floor / Wall | `AreaPlacementPlan`。範囲構造と各タイルを preview / commit の両方で再構築 |

拒否は `Cannot place`、一部だけ採用できる範囲は `Some tiles will be skipped` として色と見出しを分ける。
Floor / Wall は valid tile が1件以上ならそのタイルだけを従来どおり生成し、invalid tile を飛ばす。
valid tile が0件、または範囲自体が不正な場合だけ全体失敗となる。

実行順は次で固定する。

1. `GameSystemSet::Visual` 冒頭で古い `live` feedback を消す。
2. Visual の建築／SoulSpa ghost と Interface の move／area preview が feedback を生成する。
3. Interface の `PlacementFeedbackSet::Present` がツールチップを表示する。
4. `PlacementFeedbackSet::Commit` が同じ判定を再実行して確定する。

UI上、cursor不在、配置モード終了時は producer が新しい `live` を書かないため、古い理由は次フレームに残らない。
BuildingPlace の確定成功時と Tank の companion 段階遷移時は、クリックした anchor を live feedback blocker に記録する。
カーソルが同じ anchor にある間は、直前に置いた設計図自身との干渉による文字 feedback だけを抑止し、
validator の結果と赤い ghost は変更しない。別の grid へ移動すると blocker を解除し、その位置の拒否理由を通常表示する。
同じ anchor を明示的に再クリックして commit が失敗した場合も blocker を解除し、`recent_failure` を表示する。
配置 mode / build kind の変更と world replacement では blocker を残さない。

## セーブ／ロード結果

`SavePlugin` は `SaveLoadState` を要求用の3状態のまま保ち、各要求に対して
`SaveLoadOutcome { operation, target, result }` を1件だけ発行する。`target` は `SavePath` の
安全なファイル名だけを使い、取得できなければ `Current save` とする。

| `SaveLoadFailureKind` | 意味 | UI severity |
| --- | --- | --- |
| `SaveSerialize` | セーブデータを作成できない | Error |
| `SaveWrite` | temp file、sync、renameを含む書込失敗 | Error |
| `LoadNotFound` | 対象が存在しない | Warning |
| `LoadRead` | その他の読込I/O失敗 | Error |
| `UnsupportedFormat` | 非対応のheader version | Error |
| `InvalidData` | header/body/deserialize/schema/preflightが無効 | Error |
| `SeedMismatch` | 現セッションと保存worldのseedが異なる | Error |
| `MissingPrerequisite` | registry、asset、rehydrate前提が不足 | Error |
| `ApplyRecovered` | live apply失敗後、旧worldのrollbackに成功 | Warning |
| `RecoveryFailed` | live applyとrollbackの両方に失敗 | Error |

成功は `Success`、全terminal outcomeは `Important` として履歴へ残す。詳細なOS／RON／transaction errorは
ログにだけ残し、root adapter は分類の exhaustive match から固定文言を作る。dedupe key は
operation、対象、result kindを含むため、SaveとLoad、成功と失敗を誤って集約しない。

dispatcherは要求を処理する直前に `SaveLoadState::Idle` へ戻す。load成功またはrollbackでは
world replacementのresetが通知Messageと旧履歴を消し、その全処理が終わった後にdispatcherがoutcomeを発行する。
したがって次の `Update` でload結果が新world最初の重要通知になる。transaction開始前のload失敗は
worldを置換しないため、現在の履歴へ追記される。

## タスク操作結果

task dashboard の priority/cancel は root が `TaskActionOutcome` を 1 intent につき 1 件発行する。
結果は priority tier 変更、cancel request、malformed manual request の安全な close、stale、unsupported、pause、capture を
exhaustive に分類し、raw component/debug 情報を表示文言へ渡さない。

adapter は Entity、action kind、result kind を含む key で `ToastOnly` の `UserFacingNotification` へ変換する。
そのため同じ結果の短時間連打だけを集約し、別 Entity、成功/拒否、priority up/down を誤って dedupe しない。
`ToastOnly` は重要履歴へ残らず、1 操作が生成する visible notification は最大 1 件である。

`Working / Blocked / PendingEvaluation` と blocker reason はライブ dashboard state であり、cycle ごとに
notification Message を発行しない。

## Messageとsystem順

```text
SaveLoadOutcome / TaskActionOutcome
  → NotificationSystemSet::Adapt（rootで安全な表示文言へ変換）
  → UserFacingNotification
  → NotificationSystemSet::Reduce（ingest / dedupe / expiry）
  → NotificationSystemSet::Present（revision差分だけ描画）
```

`Adapt → Reduce → Present` は同じ `Update` 内でchainされる。`SaveLoadOutcome` は `Last` で発行されるため
次の `Update`、task action outcome は Interface 内の apply 後に同じ `Update` の adapter に読まれる。

## world replacement reset

- `SavePlugin` の専用hookが古い `SaveLoadOutcome` bufferを消す。
- `hw_ui::reset_for_world_replace()` が `UserFacingNotification`、center、unread、履歴開閉、描画revisionを初期化する。
- `MessagesPlugin` が旧 world の `TaskActionOutcome` buffer を clear し、task confirmation / `UiIntent` も UI owner hook が消す。
- 動的toast/history rowはdespawnし、static root、panel、未読labelを非表示／初期表示へ戻す。
- 配置の `live` / `recent_failure` も同じUI owner hookで消す。

通知履歴はセーブ対象ではなく、現在のworldに属するruntime UI stateである。

## 検証

```bash
cargo test -p hw_ui notifications
cargo test -p hw_ui placement
cargo test -p bevy_app@0.1.0 notifications
cargo test -p bevy_app@0.1.0 placement
cargo test -p bevy_app@0.1.0 systems::save
cargo test -p bevy_app@0.1.0 save_game
```

手動では、各配置モードの無効候補とFloor / Wallの部分採用、F5の成功通知、存在しない対象を含む
F9結果、正常load後に旧履歴が消えてload成功だけが残ることを確認する。
