# B1 Stockpileポリシー実機受入計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `stockpile-policy-manual-acceptance-plan-2026-07-23` |
| ステータス | `Blocked` |
| 作成日 | `2026-07-23` |
| 最終更新日 | `2026-07-23` |
| 作成者 | `Codex` |
| 関連提案 | `docs/proposals/gameplay-management-improvements-proposal-2026-07-17.md` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: B1 M1〜M5完了後の実機受入結果と残件を、自動テストとは別に追跡する。
- 到達したい状態: 実機でしか確認できない操作と表示を固定手順で確認し、不合格を再現可能な修正対象として残す。
- 成功指標:
  - 管理対象Stockpileの選択、単一編集、矩形編集、通知、Draining、in-flight、save/loadを実機で確認できる。
  - production UIで作れないfixtureは自動テスト専用であることを明記し、未実施と合格を混同しない。
  - 不合格項目には再現手順、期待結果、実結果、次の検証を残す。

実装設計と自動回帰の完了記録は
`docs/plans/archive/stockpile-policy-plan-2026-07-20.md` を正本とし、本書から履歴を書き換えない。

## 2. スコープ

### 対象（In Scope）

- Bevy renderer上のStockpile inspection/editorのレイアウトとpointer操作。
- 単一セル編集、矩形範囲編集、Escape取消、world入力capture。
- `Any` / `Only(ResourceType)`、priority、target、export、Drainingの表示と実挙動。
- F5/F9を通る方針値のround-tripとworld replacement後のUI reset。
- `StockpilePolicyChangeOutcome` のToastOnly表示と対象件数。
- 実AIによるDraining搬出と、target低下時のcommitted搬送完了。
- Tank、Mixer、`BucketStorage` に通常Stockpile editorが出ないこと。

### 非対象（Out of Scope）

- B2 Familiar運用ポリシー、B3 Soul Energy制御。
- 一般的なsave/loadや通知基盤の再設計。
- production配置判定が拒否する重複Yardを、実機受入のためだけに作れるよう変更すること。
- debug buildの描画FPSをrelease性能基準として扱うこと。

## 3. 現状とギャップ

### 3.1 自動確認済み

| 確認 | 結果 | 証跡 |
| --- | --- | --- |
| workspace品質ゲート | 合格 | `2026-07-22`: `python3 scripts/dev.py verify` |
| B1関連crateのlib test | 合格、431件 | `cargo test -p hw_logistics -p hw_familiar_ai -p hw_soul_ai -p hw_ui -p bevy_app@0.1.0 --lib --locked` |
| 内訳 | 合格 | `bevy_app 237 / hw_familiar_ai 42 / hw_logistics 58 / hw_soul_ai 47 / hw_ui 47` |
| 重複Yardの対象dedup | 合格、自動のみ | production UIはYard重複配置を拒否するためfixture testを正本とする |
| manual destination / normal / wheelbarrow / grant shadow | 合格、自動のみ | 上記cross-crate test |
| committed / unreserved、save migration / round-trip | 合格、自動のみ | 上記cross-crate test |
| UI開閉steady-state比較 | 合格、自動のみ | 固定60 Hz、40 tickの同一fixture比較 |

### 3.2 実機確認環境

- `2026-07-22`〜`2026-07-23`、1366x768、X11、Vulkan、NVIDIA GeForce MX250。
- `HELL_WORKERS_WORLDGEN_SEED=20260722` の固定world。
- repositoryのassetsを使用し、ユーザーの通常セーブを上書きしない隔離作業ディレクトリと一時saveを使用。
- debug buildのため表示FPSは性能受入値に使わず、操作経路と状態遷移だけを判定した。

### 3.3 実機結果

| ID | 確認事項 | 状態 | 2026-07-23時点の結果 |
| --- | --- | --- | --- |
| B1-R01 | 起動、assets読込、renderer表示 | 合格 | 固定seedで起動し、欠落assetなしでworldを表示 |
| B1-R02 | 管理対象セルの選択とeditor境界 | 合格 | 同位置にWoodが5個あるセルでもResourceItemではなくStockpileを選択。1366x768内に全操作を表示 |
| B1-R03 | 単一編集 | 合格 | `Any -> Only(Wood)`、`Normal -> High`、`Target 10 -> 9`、`Export On -> Off` をpointer操作で反映 |
| B1-R04 | F5/F9方針round-trip | 合格 | save本文とF9後の再選択の両方で `Only(Wood) / High / 9 / Off` を確認 |
| B1-R05 | world replacement後の情報パネルreset | **不合格** | F9成功後も旧 `Only(Rock) / Draining` パネルが残留。別entity選択後は更新される |
| B1-R06 | 矩形編集のmode lifecycle | 合格 | Codexによる開始・Escape取消・空矩形完了に加え、ユーザー実機で複数管理セルへの適用を確認 |
| B1-R07 | 矩形操作中のworld入力capture | 合格 | Codexによるcamera非移動に加え、ユーザー実機でpause/overlay中断と再試行を確認 |
| B1-R08 | ToastOnly通知 | 合格 | ユーザー実機でタイトル、対象件数、履歴へ残らないToastOnly表示を確認 |
| B1-R09 | Draining即時遷移と在庫保持 | 合格 | Wood 5個を削除せず `Draining` と `Export Off (Draining override)` を表示することを確認 |
| B1-R10 | 実AIによるDraining完了 | 合格 | ユーザー実機で別セルへの搬出、空状態への遷移、新方針資源の受入を確認 |
| B1-R11 | target低下時のin-flight契約 | 合格 | ユーザー実機でcommitted搬送の安全な完了と、その後の新規搬入停止を確認 |
| B1-R12 | 特殊設備に通常editorが出ない | 合格 | ユーザー実機でTank、Mixer、`BucketStorage` が通常Stockpile editorを出さないことを確認 |
| B1-R13 | 重複Yard内のdedupと件数 | 自動確認のみ | production UIで重複Yardを作れないため、実機項目から除外 |

### 3.4 B1-R05の切り分け

再現手順:

1. 管理対象Stockpileを選び、`Only(Wood) / High / 9 / Off` でF5保存する。
2. live値だけを `Only(Rock)` に変え、`Draining` 表示にする。
3. F9の確認dialogからLoadを実行する。
4. load成功後、別entityを選ばず情報パネルを観察する。

期待結果:

- `SelectedEntity` とinspection表示がresetされ、旧worldの情報パネルが即座に閉じる。
- Stockpileを再選択すると、保存済み `Only(Wood) / High / 9 / Off` を表示する。

実結果:

- loadとrehydrateは成功し、再選択後の方針値も保存内容どおりだった。
- 再選択前は旧 `Only(Rock) / Draining` パネルが残り、別entityを選ぶと初めて更新された。
- load成功から約20秒後に、despawn済みEntityを参照したcommandのwarningも1件観測した。B1-R05との因果は未確定として別に扱う。

コード経路と再現に一致する最有力原因:

- `hw_ui::reset_for_world_replace` は `InfoPanelState` と `EntityInspectionViewModel` を両方defaultへ戻すが、
  静的な `InfoPanelRoot` の `Node.display` を直接 `None` にしていない。
- 次frameの `info_panel_system` は `panel_state.last == next_model == None` で早期returnできるため、
  reset前の表示nodeと文字列が残る。
- 既存reset testはresource内のEntity参照を検証しているが、実際のroot nodeが非表示になることまでは固定していない。

## 4. 検証方針（高レベル）

- 決定的なevaluator、dedup、priority、reservation、migrationは自動回帰を正本にする。
- renderer、pointer、短命toast、実AIの時間経過、実save fileだけを実機受入へ残す。
- 実機試験はユーザーの通常saveから隔離し、各scenarioの開始状態と期待結果を先に固定する。
- B1-R05を修正する際は、resource値だけでなく `InfoPanelRoot` の `Display::None` まで確認する回帰を先に追加する。
- Bevy 0.19のUI change detectionとsystem orderingは、ローカルsourceまたは既存projectコードで確認してから変更する。

## 5. マイルストーン

## M1: 自動ベースラインと実機環境の固定

- 変更内容: B1関連cross-crate test、workspace品質ゲート、固定seed・隔離saveの実機環境を確認する。
- 変更ファイル:
  - `docs/plans/stockpile-policy-manual-acceptance-plan-2026-07-23.md`
- 完了条件:
  - [x] B1関連431 testが成功する。
  - [x] 前回のworkspace `verify` 成功を確認する。
  - [x] 通常saveを変更しない実機環境で起動できる。
- 検証:
  - `cargo test -p hw_logistics -p hw_familiar_ai -p hw_soul_ai -p hw_ui -p bevy_app@0.1.0 --lib --locked`
  - `python3 scripts/dev.py verify`

## M2: UI・save/load・Drainingの実機確認

- 変更内容: B1-R01〜B1-R13を実行し、合格・部分合格・不合格・自動のみを分離する。
- 変更ファイル:
  - `docs/plans/stockpile-policy-manual-acceptance-plan-2026-07-23.md`
- 完了条件:
  - [x] 起動、選択、単一編集、F5/F9 round-tripを確認する。
  - [x] range開始、Escape取消、空矩形完了を確認する。
  - [x] Drainingの即時表示と既存在庫保持を確認する。
  - [x] 複数管理セルのrange適用とToastOnly件数を確認する。
  - [x] 実AIのDraining完了とin-flight契約を確認する。
  - [x] 特殊設備で通常editorが出ないことを確認する。
  - [x] B1-R05の不合格を再現手順付きで修正対象へ固定する。
- 検証:
  - 下記「7. 検証計画」の実機scenarioを使用する。

## M3: 不合格修正と受入完了

- 変更内容: B1-R05を修正し、world replacementに関係する実機項目を再確認する。
- 変更ファイル:
  - `crates/hw_ui/src/lib.rs` または情報パネル更新責務を持つ同crate内ファイル
  - `crates/bevy_app/src/interface/ui/plugins/info_panel.rs`
  - 対応するunit / integration test
  - 本書と関連恒久文書
- 完了条件:
  - [ ] F9成功直後に旧情報パネルが消える。
  - [ ] 再選択後に保存済み方針値を表示する。
  - [ ] load後に観測したdespawn済みEntity warningの所有者を切り分ける。
  - [ ] reset修正の影響範囲に応じて、選択/editorとsave/loadの実機回帰を確認する。
  - [ ] `python3 scripts/dev.py verify` が成功する。
- 検証:
  - `python3 scripts/dev.py verify`
  - `git diff --check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 自動回帰の成功を実機表示の成功とみなす | stale表示やpointer競合を見逃す | 実機結果を別IDで追跡する |
| 通常saveを試験で上書きする | ユーザーデータを失う | 隔離作業ディレクトリと一時saveを使う |
| 重複Yardを実機で無理に作る | production配置契約を壊す | dedupはfixture testを正本にする |
| Drainingを即時表示だけで合格にする | 実搬出や新資源受入の欠陥を見逃す | 2セルfixtureで空になるまで観測する |
| load後のstale UIをpolicy round-trip失敗と誤認する | save schemaを誤修正する | save本文と再選択後の値を別に確認する |

## 7. 検証計画

- 必須:
  - `cargo test -p hw_logistics -p hw_familiar_ai -p hw_soul_ai -p hw_ui -p bevy_app@0.1.0 --lib --locked`
  - `python3 scripts/dev.py verify`
- 計画完了時:
  - `python3 scripts/dev.py docs --write`
  - `python3 scripts/dev.py docs --check`
  - `git diff --check`
- 手動確認シナリオ:
  1. **選択/editor**: `Zones -> Stockpile` で管理セルを配置し、itemが同位置にあってもStockpile editorを開く。
     `State / Stored / Incoming / Acceptance / Target / Priority / Export / Apply Policy to Area` が画面内に収まることを確認する。
  2. **複数セルrange**: 2個以上の管理セルを作り、1セルの方針を矩形適用する。toastの `Changed / already matched /
     skipped / duplicate` と実セル数を照合する。Escape取消、drag中のpause/overlay、camera非移動も確認する。
  3. **Draining**: Wood入りdonorと空きのあるreceiverを同じownerに用意し、donorを `Only(Rock)` にする。
     Woodを削除せず新規搬入を止め、consolidationで搬出し、空になった後にRockを受けることを確認する。
  4. **in-flight**: `Incoming > 0` を確認してpauseし、targetを `Stored` 以下へ下げて再開する。
     committed搬送は安全に完了し、その後の新規搬入が止まることを確認する。
  5. **save/load**: 非default方針でF5保存し、live値を変えてF9する。確認dialog、load結果通知、旧panelの即時消去、
     再選択後の保存値を順に確認する。
  6. **特殊設備**: Tank、Mixer、`BucketStorage` を選択し、通常Stockpile policy editorが表示されないことを確認する。
- パフォーマンス確認:
  - producer / arbitration workは固定60 Hz・40 tickの同一fixture自動比較を正本にする。
  - 実機ではeditor開閉・矩形dragの入力応答だけを観察し、debug FPSをrelease基準にしない。

## 8. ロールバック方針

- 本書は受入記録のため、B1実装を巻き戻さない。
- B1-R05の修正は情報パネルのworld replacement resetと回帰testを1変更単位にする。
- 修正でload、選択、pin表示へ回帰が出た場合は、その修正単位だけを戻し、本書の不合格状態を維持する。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `自動確認 100% / 実機確認実施 100% / 13項目中 合格11・不合格1・自動のみ1 / B1-R05修正待ち`
- 完了済みマイルストーン: `M1`、`M2`
- 未着手/進行中: `M3未着手（B1-R05でBlocked）`

### 次のAIが最初にやること

1. B1-R05を再現するintegration testを追加し、`InfoPanelRoot` の実displayまで失敗を固定する。
2. UI reset修正後、F9直後・別entity選択前の画面を再確認する。
3. B1-R06〜B1-R12は合格済みとして維持し、reset修正が経路へ影響する場合だけ該当項目を再確認する。

### ブロッカー/注意点

- B1のpolicy保存値はround-tripしている。B1-R05は旧panelの表示reset不備であり、schema移行問題として扱わない。
- 重複Yardはproduction UIで作れないため、手動fixtureを作る変更を要求しない。
- 一時probe、screenshot、隔離saveは受入記録を反映した後に削除する。

### 参照必須ファイル

- `docs/plans/archive/stockpile-policy-plan-2026-07-20.md`
- `docs/logistics.md`
- `docs/info_panel_ui.md`
- `docs/save_load.md`
- `crates/hw_ui/src/lib.rs`
- `crates/hw_ui/src/panels/info_panel/update.rs`
- `crates/bevy_app/src/interface/ui/plugins/info_panel.rs`
- `crates/bevy_app/src/systems/save/reset.rs`

### 最終確認ログ

- 最終 `cargo check --workspace`: `2026-07-23 / pass`（`scripts/dev.py verify` 内）
- 最終 `cargo clippy --workspace --all-targets -- -D warnings`: `2026-07-23 / pass`（0 warnings）
- 最終 `cargo test --workspace`: `2026-07-23 / pass`（`scripts/dev.py verify` 内）
- B1 focused lib tests: `2026-07-22 / pass / 431 tests`
- 未解決エラー: `B1-R05`。F9成功後に旧情報パネルが残留する。

### Definition of Done

- [ ] B1-R05が修正され、実displayを含む回帰testがある
- [x] B1-R06〜B1-R12の残件が合格、または自動専用の根拠が記録済み
- [x] 影響ドキュメントが更新済み
- [x] `cargo check --workspace` が成功
- [x] `cargo clippy --workspace --all-targets -- -D warnings` が成功
- [x] `cargo test --workspace` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-07-23` | `Codex` | 自動431 testと隔離実機確認を整理。方針round-trip、Draining表示、range lifecycleを確認し、F9後の旧情報パネル残留をB1-R05として記録 |
| `2026-07-23` | `User / Codex` | ユーザー実機でrange、capture、Toast、Draining完了、in-flight、特殊設備を確認。実機確認の実施を完了し、未修正のB1-R05だけをblockerとして継続。全workspace品質ゲートも再実行して合格 |
