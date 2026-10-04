# R08: Soul一覧のtyped node参照とwidget値同期の移管 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `refactor-r08-soul-row-node-ownership-plan-2026-09-18` |
| ステータス | Complete |
| 作成日 | 2026-09-18 |
| 最終更新日 | 2026-09-19 |
| 作成者 | Codex |
| 関連提案 | [実装横断レビュー](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md) §5 R08 |
| 関連Issue/PR | N/A |
| 優先度 / 相対規模 | P2 / 中 |
| 調査基準 | `52913b61ca524f1cd429d3d73a0468faf3d0999b` |
| 計画レビュー | 2026-09-18 / [全体レビュー結果](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md#42-計画レビュー2026-09-18)参照。実装受入ではない |
| 計画具体化 | 2026-09-18 / API案・変更単位・fixture/期待値・着手手順を追加。新規名は未実装の設計案 |
| 実装状態 | 本体・回帰・仕様／Help同期、品質gate、必要なnative受入と終了整理を完了。現行仕様は関連docsを正本とする。 |

## 1. 目的

- 解決したい課題: hw_uiが生成するChildrenの位置をrootが添字で更新し、表示規則も複製されている状態を解消する。
- 到達したい状態・成功指標: 構造変更を伴わない値更新ではSoulRowNodes経由で同じrow Entityを更新する。hw_uiがspawn/sync/styleを所有し、rootはゲームQueryからのViewModel生成・asset注入・dirty橋渡しを担う。

## 2. スコープ

### 対象（In Scope）

- hw_ui listのnode参照と共有style、root value syncの移管
- spawn/値更新、構造変更、検索/折りたたみ/loadの回帰

### 非対象（Out of Scope）

- 承認済みの列幅・文字サイズ・UI配置変更
- ViewModelのゲームQueryをhw_uiへ移すこと
- 更新周期・全リストvirtualizationの変更

### 主な変更対象（現行ファイル）

- [crates/bevy_app/src/interface/ui/native_acceptance.rs](../../../../crates/bevy_app/src/interface/ui/native_acceptance.rs)（row更新fixture/observer）
- [.codex/skills/hell-workers-run-native-acceptance/scripts/ui_usability_acceptance.py](../../../../.codex/skills/hell-workers-run-native-acceptance/scripts/ui_usability_acceptance.py)（専用case・manifest・独立verifier）
- [scripts/tests/test_ui_usability_acceptance.py](../../../../scripts/tests/test_ui_usability_acceptance.py)
- [crates/hw_ui/src/list/spawn.rs](../../../../crates/hw_ui/src/list/spawn.rs)
- [crates/hw_ui/src/list/models.rs](../../../../crates/hw_ui/src/list/models.rs)
- [crates/hw_ui/src/list/sync.rs](../../../../crates/hw_ui/src/list/sync.rs)
- [crates/bevy_app/src/interface/ui/list/sync.rs](../../../../crates/bevy_app/src/interface/ui/list/sync.rs)
- [crates/bevy_app/src/interface/ui/list/view_model.rs](../../../../crates/bevy_app/src/interface/ui/list/view_model.rs)

新規moduleやtestは上記ownerの配下に置く。新しいcrateを既定案にせず、定義→生成→使用の順に変更する。

## 3. 現状とギャップ

root list/syncはChildren[0,1,3,5,6,7,8]へ依存する。spawn側とroot側にgender/task/stress/dreamの表示対応が重複し、layout変更が型検査で検出されない。

提案時点の静的な根拠と実行再現を区別し、M1で現HEADとの差・実際のcaller・既存testを再確認する。
仕様を直す前に実行経路・前提条件を確認し、再現できない仮説への値調整を繰り返さない。

### 既存テスト・検査の入口

- [crates/bevy_app/src/interface/ui/list/view_model.rs](../../../../crates/bevy_app/src/interface/ui/list/view_model.rs)
- [crates/hw_ui/src/lib.rs](../../../../crates/hw_ui/src/lib.rs)
- [crates/hw_ui/src/shell.rs](../../../../crates/hw_ui/src/shell.rs)

これらは再利用する入口であり、本計画の全条件を既に保証しているという意味ではない。
不足する受入ケースを下記milestoneで追加する。削除・文書整理だけの変更には実装をなぞる新規testを作らない。

## 4. 実装方針と具体設計

- InfoPanelNodes/FamiliarSectionNodesの既存patternに従い、spawn時にSoulRowNodesを確定する。entityを含む参照は行の消滅/WorldEpoch更新で残留させない。
- style/icon導出と値同期はhw_uiへ置きUiAssetsを受け取る。rootに残すのはゲームQueryからのViewModelとasset注入・dirty橋渡し。
- 100msの値更新とstructure更新を区別し、値更新だけでrowを再spawnしない。既存差分比較で同値Text/Color/Fontを書き直さない。

### 具体設計と変更境界

`hw_ui::list::models` にComponent `SoulRowNodes { gender_icon, name_text, fatigue_text, stress_text, dream_text, task_icon, task_label }` を追加する案。全fieldはEntityとし、spawn時に捕捉してrowへ付ける。既存 `EntityListNodeIndex` のSoul→row対応を使い、別のglobal node cacheを作らない。

新規leaf入口 `sync_entity_list_values`（案）はViewModel参照、node index、借用 `&dyn UiAssets`、theme、leaf所有Query contextを受ける。header更新・Soul lookup・差分適用をhw_uiへ移す。rootはGameAssets adapterを借用で渡し、ゲームQuery→ViewModel生成とdirty橋渡しを保持する。

| 更新 | 維持する境界 |
| --- | --- |
| 初回spawn | typed nodeを確定し、値更新と同じstyle/icon導出を使用。 |
| 100ms値同期 | row Entityを保持し、変わったText/Color/Font/Imageだけ更新。Childrenの添字を参照しない。 |
| 検索/所属など構造同期 | 既存再spawn/削除を維持。検索中の改名で行が消えることを許す。 |
| despawn / WorldEpoch | row Componentと既存indexを同時に無効化。古いnodeを次Worldへ持ち越さない。 |

### 実装単位と移行順

1. **R08-A / M1:** 異なるicon handleを返すtest UiAssetsと、初回spawn/値同期の比較testを作る。
2. **R08-B / M2:** typed node Componentと共有style導出を導入。rootの旧同期を一時的に新参照へ接続して差分を小さくする。
3. **R08-C / M3:** 同期本体をleafへ移し、rootをViewModel/assets/dirty adapterに絞る。専用native caseを同時に追加する。

### 依存関係と着手順

R04の表示分類を先に確定することを推奨。R04未着手でもnode参照の定義は独立可能だが、list/view_model.rs・sync.rs・spawn.rsを同時編集しない。
R04とnative fixture/helper/testの3ファイルも共有するため、既存caseを保持して順次取り込む。

関連計画へのリンクと全体の着手順は[親提案の計画一覧](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md)を参照。
R12も同じnative fixture/helper/testを拡張するため、受入caseの編集と凍結期間を重ねない。
依存は実装上の前提と共有ファイルの編集競合を区別する。全14件の完了を本計画の終了条件にしない。

### Bevy 0.19と変更の所有者

- 既存の正しい0.19実装を基準とし、新しいAPIはrust-analyzer/docsrs MCPまたはlocal registryで確認してから使う。
- Queryはcrate-owned contextへまとめ、rootの配線・leafの実装所有を維持する。lint抑制で構造問題を隠さない。
- コードと文書の編集は主担当が行う。subagentは読み取り専用の調査/レビューに限定する。
- 共通Help snapshot・docs索引・凍結native subjectは並行編集しない。

## 5. マイルストーン

## M1: 構造と値更新契約の固定

- 変更内容:
  - 現行node順、row保持条件、値/structure dirtyと更新周期を確認する。
  - 初回spawnと値だけ更新した結果、同値更新、load後resetのECSテストを用意する。
- 変更ファイル: R08-A: hw_ui/src/list 配下test、setupのUiAssets test adapter。root list/sync.rs を期待値の根拠にする。
- 完了条件:
  - [x] 見た目とrow identityの基準があり、値更新で再spawnしないことを検査できる。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M2: typed nodeとstyle共有

- 変更内容:
  - SoulRowNodesをspawnで生成し、gender/task/stress/dreamの表示規則をhw_ui内で共有する。
  - root callerをtyped参照へ段階移行する。
- 変更ファイル: R08-B: hw_ui/src/list/{models,spawn}.rs と共有style module（必要時新規）、root list/sync.rs のtyped参照。
- 完了条件:
  - [x] Soul行のChildren添字参照がなく、レイアウトが不変。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M3: 値同期をwidgetへ移す

- 変更内容:
  - UiAssets注入付きsync APIへ移しrootを薄くする。
  - 検索・所属変更・折りたたみ・World replacementのnode参照破棄を検証する。
  - 既存UI native入口へrow更新caseを追加し、Soul/row/typed nodeのidentityと可視値を観測する。欠落node・旧値・別対象を独立verifierのfixture testで拒否する。
- 変更ファイル: R08-C: hw_ui listの値同期入口、root list/{sync,view_model}.rs、既存NativeUi/helper/Python verifierの3入口。
- 完了条件:
  - [x] spawn/syncの値が一致し、epochを跨ぐstale参照がない。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M4: 仕様・Help・品質ゲートと終了整理

- 変更内容:
  - 下記仕様へ最終的な所有者・順序・失敗条件を反映する。計画の検討経緯をそのまま恒久仕様へ転記しない。
  - [Help影響Skill](../../../../.cursor/skills/hell-workers-review-help-impact/SKILL.md)で全実装差分のplayer-visible経路を確認する。
  - 表示/操作/文言が不変なら実経路根拠付きNo impact。R04の文言変更を同じbatchへ取り込む場合はそのHelp更新も含める。
  - 受入と品質ゲートの結果を記録し、専用出力を整理する。
- 仕様更新対象（該当する契約を変更した文書だけを更新）:
  - [docs/entity_list_ui.md](../../../entity_list_ui.md)
  - [docs/crate-boundaries.md](../../../crate-boundaries.md)
  - [docs/cargo_workspace.md](../../../cargo_workspace.md)
  - [docs/ui-world-first.md](../../../ui-world-first.md)
- 完了条件:
  - [x] §7のケースと必要なnative受入が完了し、未検証範囲を明記した。
  - [x] HelpがUpdate required / No impactのいずれかに確定し、対応する更新または根拠を記録した。
  - [x] `check`・Clippy・`verify`が成功。workspace RA診断は上記MCP制約を記録。
  - [x] primary validation storage checkが成功し、不要出力の整理/継続用途を記録した。
- 検証: §7の完了gate。品質gateのpassだけをHelpの意味レビューやnative受入の代替にしない。

## 6. リスクと対策

| リスク | 対策 |
| --- | --- |
| typed参照がload後に残る | 行消滅/epoch resetを明示し既存world_replace testを拡張。 |
| 移管で更新頻度やlayoutが変わる | 100ms値更新・structure更新・実画面基準を保持。 |
| 共有ファイルの並行変更を上書きする | 作業開始/各取り込み前にstatusと対象diffを確認し、変更所有を確認する。 |
| 過去のpassを新subjectへ流用する | 変更後の入力を固定して必要な検査を実施し、未実施は未実施と記録する。 |

## 7. 検証計画

### ケースと判定条件

| ケース | 期待する結果 |
| --- | --- |
| structure変更を伴わない改名・疲労/stress/dream/task変更 | 同row Entityの対応nodeだけ更新。 |
| 検索中の改名 | 既存のstructure更新を維持し、検索結果に応じた行の追加/削除を許す。 |
| 同値更新 | 不要なText/Color/Font/Image書込みなし。 |
| 所属変更・検索・折りたたみ復帰 | 行の追加削除とselectionが正しい。 |
| load/world replacement | 旧Soul/旧nodeの参照を保持しない。 |

### 回帰fixtureの作り方（新規test案）

| ID / test案 | 準備・操作 | 必須assert |
| --- | --- | --- |
| R08-T1 `soul_row_spawn_and_sync_match_test` | 男/女、stress3段階、dream有/無、task切替。用途別に異なるHandleを返すUiAssetsを使う。 | 初回spawnと値同期後のText/Color/Font/Imageが一致。iconの取り違えも検出。 |
| R08-T2 `soul_row_equal_values_do_not_write_test` | 初回反映後にchange tickを進め、同じVMを再適用。 | row Entity不変、対象componentのChangedなし。 |
| R08-T3 `soul_row_nodes_follow_structure_lifetime_test` | 検索中改名、所属変更、折りたたみ、行削除、WorldEpoch変更。 | 表示対象とselectionを維持し、削除row/node参照がindexに残らない。 |
| R08-T4 native `soul-row-values` | 通常simulationで可視値を変化させ、検索/clear・折りたたみ・選択/戻るを実入力。 | 対象Soul・可視値・行状態が対応。observerによる値の直接書換えを証拠にしない。 |

既存TestAssetsは複数画像に同一handleを返すため、T1ではそのまま流用しない。nativeは既存smokeから始め、表示差があったときだけviewport行列を拡大する。

### focused command（実装時に実行）

```bash
python3 -m unittest scripts.tests.test_ui_usability_acceptance
python3 scripts/dev.py cargo -- test -p hw_ui
python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 interface::ui::list
```

test filterは実装時に実際の出力件数を確認し、0件実行を成功根拠にしない。
既存filterに入らない新testは正確なmodule/test名をここへ追加する。

### 完了gate

```bash
python3 scripts/dev.py docs --write
python3 scripts/dev.py docs --check
python3 scripts/check_help_impact.py
python3 scripts/dev.py check
python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings
python3 scripts/dev.py verify
python3 scripts/dev.py validation check
git diff --check
```

`verify`がworkspace testを含むため、成功後に理由なく同じ全testを繰り返さない。
上のコマンドは実装完了時の要件であり、計画作成時の実行済み記録ではない。
Help No impactのローカルoverrideはSkillに従い具体的理由を設定し、将来の差分を事前承認しない。
Help更新時はmanifest/provider/coverageと生成したexact snapshotを同じbatchに含め、全差分を読む。

### 実画面・性能確認

native_acceptance.rsとui_usability_acceptance.pyへ専用row更新case・manifest・独立verifierを追加し、test_ui_usability_acceptance.pyで拒否条件を検査する。同値書込み抑制とrow保持の厳密な判定はECS test、nativeは値変化の表示、検索/折りたたみ復帰、選択と戻るの操作を確認する。初期準備後のobserverは読み取り専用とし、値変化は通常simulationまたは実UI入力で起こす。

既存`--smoke`相当の1920×1080/UI scale 1を必須とする。現在のhelperは1画面または6組合せなので、2画面専用selectorを新設しない。layout差が出た場合だけ既存6組合せへ拡大する。no-input静止画像を動的更新の証拠にせず、GPU/性能比較は要求しない。

実機確認を実施する場合は[Native Acceptance Skill](../../../../.cursor/skills/hell-workers-run-native-acceptance/SKILL.md)を読み直す。
primary `dev.py validation plan --spec ... -- <既存helperのplan>`へ登録し、返されたno-prompt kitty launcherだけを使う。
fixtureが対象を覆わなければ先に専用caseと独立verifierを用意する。汎用smokeの成功を個別要件の証明にしない。
Capture/Memoryを含むrecipeは逐次実行し、read-only observer・source/asset/binary一致・timeoutを検証する。
通常の修正feedbackでは同じdev cacheを再利用し、headless・feedback・正式受入・性能結果を区別する。

### 検証データ管理（完了結果）

2026-09-19、primary coordinatorの`refactor-suite-portal-20260919-i`で最終受入を完了した。
ownerは`refactor-r01-r14`、consumerは`refactor-r04-r08-r12-input`。
元source／assets／harness／binaryがある間に独立verifyし、全18画像を確認してpass sealした。
job `target/native-acceptance/ui-usability-feedback-20260919T121448Z-3cc51761`は、process／file利用がないことを確認して削除した（69,300,224→0 allocated bytes、df空き差0）。finalize／storage check成功。

この入力受入に実行中batch・保持jobは残らない。採用コードと恒久仕様はprimaryへ反映済み。
P08のcandidate／cacheは別consumer `refactor-r01-r14-review`でレビューを継続する。
全batchの結果・削除path／実測bytes・保持用途は[親提案§12](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md#12-ai引継ぎメモ)へ集約した。
旧失敗結果を新subjectへ流用せず、終了済みrawの再保存義務も設けない。

## 8. ロールバック方針

typed参照導入とsync移管を分ける。syncを戻す場合もtyped参照を使い、位置添字へ戻さない。

戻す前に直近5commit、対象のHEAD差分、他sessionの所有を確認する。
広範囲のcheckout/cleanや他計画の変更破棄を行わず、レビュー可能な限定逆patchを作る。
保存型・asset formatを変える必要が生じた場合は、互換/復旧条件をこの計画へ追記してから変更する。

## 9. AI引継ぎメモ（完了記録）

### 最終状態

SoulRowNodesとwidget所有の値／style同期を導入。13分類、再順序、同値更新、despawn回帰に加え、改名・検索・折りたたみ／再展開、同じrowの疲労25→26%更新を実入力で確認した。

2026-09-19 12:16 UTC、最終suite-iの全18 checkpointと独立verifierが成功した。
Intel Arc Graphics (MTL)／Vulkan／Mesa 26.1.8、X11 under Wayland、1920×1080／UI scale 1。
rows 13枚とbars 5枚を全て確認し、対象／表示一致、長い名前の折返し、barの表示・消滅を確認した。
両game logにWARN／ERRORはなく、Load後のB0004も解消した。source／harness／binary hashは親提案に記録した。

許可操作は主担当がAT-SPIで対象を照合して扱えることを先行suiteで確認済み。
今回のsuite-iでは操作スクリプトの実行前に許可応答と入力が進んだため、主担当が許可buttonを押した証拠とは扱わない。
両gameのdevices=3と同一portal sessionを独立検証し、sessionは終了時に閉じた。
ユーザーへ反復する表示確認・OS許可操作を要求する手順は採用しない。

### 最終確認ログ

| 検査 | 結果 |
| --- | --- |
| focused tests／回帰 | 上記実装回帰が成功。入力関連45件、追加construction 6件と関連RtT 8件も成功 |
| check／rust-analyzer | 最終sourceのcheck成功。最新construction_shells.rs個別診断error／warning 0。一括診断はMCPのnull応答のためworkspace compile／Clippyで確認 |
| Clippy／verify | 通常・profilingのworkspace all-targets Clippy警告0、verify成功（通常／profiling Rust test、feature別compile、Python等を含む） |
| Help／仕様 | 全体Update required。Move失敗時の保持・タスク表示・建設工程の3 entryと生成snapshotを更新し全差分レビュー済み。追加の表示復元と検証helperには新しい操作・意味・文言なし |
| native | suite-i 13 row＋5 bar、独立verify、全画像、WARN／ERROR 0。性能改善率・IME composition・6 viewport入力・全UIシナリオの受入ではない |
| 保存管理 | pass seal、job削除、finalize／storage check成功。採用コードと仕様はprimary、レビュー用P08 candidate／cacheのみ別consumerで保持 |
| 未解決エラー | 本計画の必須受入に未解決エラーなし |

### 継続時の扱い

本計画は完了履歴としてarchiveする。新たな要求・回帰にはprimaryの現行仕様を使い、影響範囲だけ再検証する。
最終候補とcacheは[検証データ管理](../../../development-infra/validation-storage-workflow.md)のレビュー寿命を維持する。
本計画を新しい作業の正本として再開したり、終了jobの原本を再保存したりしない。

### Definition of Done

- [x] M1〜M4の完了条件と必要ケースが成立
- [x] 仕様／Help同期と意味レビューを完了
- [x] check・Clippy警告0・verifyと必要なnative受入が成功
- [x] 結果をsealし、用途のないjobを削除／finalizeしてstorage check成功
- [x] 入力受入consumerの実行／保持対象を終了し、candidateは別review consumerへ引継ぎ
- [x] 削除path・実測bytesと採用成果／恒久結果を正本へ集約
- [x] 計画をarchiveし、docs --writeで両索引を更新

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-09-19 | Codex | ユーザーの対応可能回答を受けportalで再開。許可応答timeoutで入力せず終了。jobをseal／整理／finalizeし、ダイアログ表示・操作状況の確認待ちとして記録 |
| 2026-09-19 | Codex | 実装・回帰・Help／仕様同期と全体gateが成功。残る実入力受入をOS許可対応待ちとして引継ぎ、終了jobの整理を完了 |
| 2026-09-18 | Codex | R08の独立計画をテンプレートから作成。依存、milestone、受入、Help、検証データ管理を具体化 |
| 2026-09-18 | Codex | 計画レビュー: row保持を非構造更新へ限定し、dirty橋渡し・専用native case・実行可能なsmoke範囲を明確化。 |
| 2026-09-18 | Codex | 全計画ブラッシュアップ: SoulRowNodesの7参照を現行spawnのentityへ対応付け、異なるicon handleで初回spawnと値同期を比較するR08-T1を作る。 実装は未着手。 |
| 2026-09-19 | Codex | suite-i全18 checkpoint・独立verify・全画像・無警告を確認。pass seal／整理／finalizeし、本計画を完了・archive |
