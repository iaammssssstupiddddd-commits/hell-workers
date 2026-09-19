# R04: 一覧・詳細・Tooltipのタスク表示分類統一 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `refactor-r04-task-presentation-plan-2026-09-18` |
| ステータス | Complete |
| 作成日 | 2026-09-18 |
| 最終更新日 | 2026-09-19 |
| 作成者 | Codex |
| 関連提案 | [実装横断レビュー](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md) §5 R04 |
| 関連Issue/PR | N/A |
| 優先度 / 相対規模 | P1 / 小〜中 |
| 調査基準 | `52913b61ca524f1cd429d3d73a0468faf3d0999b` |
| 計画レビュー | 2026-09-18 / [全体レビュー結果](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md#42-計画レビュー2026-09-18)参照。実装受入ではない |
| 計画具体化 | 2026-09-18 / API案・変更単位・fixture/期待値・着手手順を追加。新規名は未実装の設計案 |
| 実装状態 | 本体・回帰・仕様／Help同期、品質gate、必要なnative受入と終了整理を完了。現行仕様は関連docsを正本とする。 |

## 1. 目的

- 解決したい課題: 解体taskが詳細/TooltipでBucketTransportへ落ちる表示分類の不整合を解消する。
- 到達したい状態・成功指標: 全AssignedTask variantを網羅した共通分類を一覧・詳細・Tooltipが利用し、解体/発電/水運搬のラベルと意味が一致する。

## 2. スコープ

### 対象（In Scope）

- rootのtask presentation adapterと各ViewModel builder
- variant網羅テスト、詳細/Tooltipと一覧の実画面確認
- 変更文言のHelp反映

### 非対象（Out of Scope）

- 列レイアウトやアイコンデザインの変更
- UIへゲームQueryを移すこと、全localizationの導入
- R08のnode構造変更

### 主な変更対象（現行ファイル）

- [crates/bevy_app/src/interface/ui/native_acceptance.rs](../../../../crates/bevy_app/src/interface/ui/native_acceptance.rs)（専用fixture/observer）
- [.codex/skills/hell-workers-run-native-acceptance/scripts/ui_usability_acceptance.py](../../../../.codex/skills/hell-workers-run-native-acceptance/scripts/ui_usability_acceptance.py)（専用case・manifest・独立verifier）
- [scripts/tests/test_ui_usability_acceptance.py](../../../../scripts/tests/test_ui_usability_acceptance.py)
- [crates/bevy_app/src/interface/ui/presentation/mod.rs](../../../../crates/bevy_app/src/interface/ui/presentation/mod.rs)
- [crates/bevy_app/src/interface/ui/presentation/builders.rs](../../../../crates/bevy_app/src/interface/ui/presentation/builders.rs)
- [crates/bevy_app/src/interface/ui/list/view_model.rs](../../../../crates/bevy_app/src/interface/ui/list/view_model.rs)
- [crates/bevy_app/src/interface/ui/list/sync.rs](../../../../crates/bevy_app/src/interface/ui/list/sync.rs)
- [crates/hw_jobs/src/tasks/mod.rs](../../../../crates/hw_jobs/src/tasks/mod.rs)

新規moduleやtestは上記ownerの配下に置く。新しいcrateを既定案にせず、定義→生成→使用の順に変更する。

## 3. 現状とギャップ

presentation::format_task_strはDeconstructを列挙せず包括fallbackへ落とす。一覧側にはDeconstruct分類とテストがある。buildersから同じ誤分類が詳細/Tooltip modelへ渡る。

提案時点の静的な根拠と実行再現を区別し、M1で現HEADとの差・実際のcaller・既存testを再確認する。
仕様を直す前に実行経路・前提条件を確認し、再現できない仮説への値調整を繰り返さない。

### 既存テスト・検査の入口

- [crates/bevy_app/src/interface/ui/list/view_model.rs](../../../../crates/bevy_app/src/interface/ui/list/view_model.rs)
- [crates/bevy_app/src/interface/ui/presentation/mod.rs](../../../../crates/bevy_app/src/interface/ui/presentation/mod.rs)

これらは再利用する入口であり、本計画の全条件を既に保証しているという意味ではない。
不足する受入ケースを下記milestoneで追加する。削除・文書整理だけの変更には実装をなぞる新規testを作らない。

## 4. 実装方針と具体設計

- まずDeconstructの限定修正を行い、全variantの表示表を追加する。AssignedTaskを判定するmatchでは包括fallbackを使わない。
- 分類ownerはroot presentation。work_type()で共有できる分類とHaulToBlueprint等の表示上の違いを分離し、phase labelも明示的に組み立てる。
- hw_uiはdisplay model/widgetを所有し、ゲームQueryやAssignedTask依存を新たに持ち込まない。既存TaskVisualとの変換をroot adapterに集約する。
- 文言と意味の変更としてHelp SkillのUpdate required経路を実施する。stable IDを維持し、exact snapshotの全差分をレビューする。

### 具体設計と変更境界

rootの新規 `interface/ui/presentation/task.rs` に次のAPIを置く案とする。

- `TaskKindPresentation { list_visual: TaskVisual, detail_name: &'static str }` と `task_kind_presentation(&AssignedTask)`: allocationを伴わない表示分類。
- `format_task_phase(&AssignedTask) -> Option<String>`: None以外のpayload.phaseを現行のDebug表現で整形する。
- 既存 `format_task_str` は詳細名とphaseの組立だけにする。一覧の `task_visual` は分類APIだけを呼び、phase文字列を作らない。

| task | 一覧と詳細で残す違い |
| --- | --- |
| Gather | 一覧はChop / Mine / GatherDefault、詳細名はGather。 |
| HaulToMixer | 一覧の搬入分類と詳細名HaulToMixerを区別。 |
| 床/壁の4 task | 一覧はBuild、詳細名はReinforceFloor / PourFloor / FrameWall / CoatWall。 |
| BucketTransport | River/Tankとも一覧Water、詳細名BucketTransportを維持。work_typeの違いを新しい表示差にしない。 |
| None / Deconstruct | NoneはIdle・phaseなし。Deconstructは解体の分類と詳細名を明示し、BucketTransportへ落とさない。 |

### 実装単位と移行順

1. **R04-A / M1:** Deconstructの限定修正と全17 variantの固定期待表を追加。
2. **R04-B / M2:** 分類/phaseを新moduleへ移し、list ViewModelとpresentation builderの入口を移行。hw_uiには完成済みmodelだけ渡す。
3. **R04-C / M3:** 既存NativeUi入口へ `task-presentation` case（案）を追加し、3 Soulの実入力確認とHelp更新を行う。

### 依存関係と着手順

先行条件なし。R08より先にtask分類APIを確定すると重複修正を減らせる。R08が先に動く場合もpresentationとnode同期の境界を分ける。UI提案U14の一覧修正は維持する。
R08とnative fixture/helper/testの3ファイルも共有するため、case追加は順次取り込む。

関連計画へのリンクと全体の着手順は[親提案の計画一覧](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md)を参照。
R12も同じnative fixture/helper/testを拡張するため、受入caseの編集と凍結期間を重ねない。
依存は実装上の前提と共有ファイルの編集競合を区別する。全14件の完了を本計画の終了条件にしない。

### Bevy 0.19と変更の所有者

- 既存の正しい0.19実装を基準とし、新しいAPIはrust-analyzer/docsrs MCPまたはlocal registryで確認してから使う。
- Queryはcrate-owned contextへまとめ、rootの配線・leafの実装所有を維持する。lint抑制で構造問題を隠さない。
- コードと文書の編集は主担当が行う。subagentは読み取り専用の調査/レビューに限定する。
- 共通Help snapshot・docs索引・凍結native subjectは並行編集しない。

## 5. マイルストーン

## M1: 表示表と限定修正

- 変更内容:
  - 全AssignedTaskの表示名/一覧分類/phaseの期待値表を用意し、Deconstructの誤分類を修正する。
  - 水運搬や発電と同じ分類へ落ちない回帰テストを追加する。
- 変更ファイル: R04-A: presentation/mod.rs の限定修正と同module配下test、list/view_model.rs の既存分類を期待表の根拠にする。
- 完了条件:
  - [x] 解体の詳細/Tooltip modelが正しい名称を返す。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M2: root adapterに集約

- 変更内容:
  - 表示分類とTaskVisual変換を共有し、一覧/詳細/Tooltipの独立matchを整理する。
  - phaseや搬入先の違いを保持し、包括fallbackをなくす。
- 変更ファイル: R04-B: 新規 presentation/task.rs（案）、presentation/{mod.rs,builders.rs}、list/view_model.rs。
- 完了条件:
  - [x] 新variant追加時に未対応表示がコンパイルまたは網羅テストで検出される。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M3: 実画面とHelpを確認

- 変更内容:
  - 解体/発電/水運搬を持つSoulの一覧・詳細・Tooltipを同じsubjectで表示し、文字切れも確認する。
  - native_acceptance.rsとUI helperへ専用caseを追加する。独立verifierの期待値はproduction formatterから生成せず、誤分類・非表示文字・別対象を拒否するPython fixture testを追加する。
  - Help manifest/provider/coverageを更新し、生成snapshotをレビューする。
- 変更ファイル: R04-C: interface/ui/native_acceptance.rs、ui_usability_acceptance.py、scripts/tests/test_ui_usability_acceptance.py とHelp manifest/provider/snapshot。
- 完了条件:
  - [x] 表示三経路の一致、Help exact test、専用native caseの証拠が揃う。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M4: 仕様・Help・品質ゲートと終了整理

- 変更内容:
  - 下記仕様へ最終的な所有者・順序・失敗条件を反映する。計画の検討経緯をそのまま恒久仕様へ転記しない。
  - [Help影響Skill](../../../../.cursor/skills/hell-workers-review-help-impact/SKILL.md)で全実装差分のplayer-visible経路を確認する。
  - ラベル変更なのでUpdate requiredを予定する。manifest/provider/coverage/exact snapshotを同じbatchで更新する。
  - 受入と品質ゲートの結果を記録し、専用出力を整理する。
- 仕様更新対象（該当する契約を変更した文書だけを更新）:
  - [docs/info_panel_ui.md](../../../info_panel_ui.md)
  - [docs/entity_list_ui.md](../../../entity_list_ui.md)
  - [docs/task_list_ui.md](../../../task_list_ui.md)
  - [docs/help-screen.md](../../../help-screen.md)
- 完了条件:
  - [x] §7のケースと必要なnative受入が完了し、未検証範囲を明記した。
  - [x] HelpがUpdate required / No impactのいずれかに確定し、対応する更新または根拠を記録した。
  - [x] `check`・Clippy・`verify`が成功。workspace RA診断は上記MCP制約を記録。
  - [x] primary validation storage checkが成功し、不要出力の整理/継続用途を記録した。
- 検証: §7の完了gate。品質gateのpassだけをHelpの意味レビューやnative受入の代替にしない。

## 6. リスクと対策

| リスク | 対策 |
| --- | --- |
| work_typeだけに潰し搬送先やphaseを失う | 分類と説明文を分離しvariant表を固定。 |
| 一覧だけ直し別経路を残す | 共通modelと三経路のactual-window確認で閉じる。 |
| 共有ファイルの並行変更を上書きする | 作業開始/各取り込み前にstatusと対象diffを確認し、変更所有を確認する。 |
| 過去のpassを新subjectへ流用する | 変更後の入力を固定して必要な検査を実施し、未実施は未実施と記録する。 |

## 7. 検証計画

### ケースと判定条件

| ケース | 期待する結果 |
| --- | --- |
| 全task variantのmodel | 表示上必要な違いを保持し分類漏れなし。 |
| 解体・発電・BucketTransport | 一覧/詳細/Tooltipで意味が一致。 |
| Idle・task切替・選択対象消失 | 旧task文字列が残らず安全に更新。 |
| 1920×1080/UI scale 1の専用case | 新文言が読め、同じSoulの一覧/詳細/Tooltipが対応。layout差が出た場合だけ既存6 viewport/scaleへ拡大する。 |

### 回帰fixtureの作り方（新規test案）

| ID / test案 | 準備・操作 | 必須assert |
| --- | --- | --- |
| R04-T1 `task_kind_presentation_is_exhaustive_test` | 全17 variant＋Gather3分類＋Bucket2 sourceのpayloadを作る。 | 独立した固定表とlist_visual/detail_nameが一致。包括fallbackなし。 |
| R04-T2 `task_phase_and_detail_are_preserved_test` | None、GeneratePower(Generating)、DeconstructのGoingToTarget / Dismantling(progress=0.25) / AwaitingCommitを整形。 | Idleのphaseなし、既存Debug表現、解体名を維持。builderから詳細とTooltipへ同じ説明が渡る。 |
| R04-T3 native `task-presentation` | 解体/発電/給水のSoulをfixtureへ置き、一覧選択・詳細表示・hoverを実入力で行う。 | 同一Soulのidentityと可視文字が対応。解体SoulにBucketTransportが出る誤分類、別Soulの証拠、不可視文字だけの証拠はverifierが拒否。給水Soulの詳細名BucketTransportは受理。 |

T3は既存smokeの1920×1080/UI scale 1から始める。verifierの期待語彙はproduction formatterから生成せず固定し、失敗fixtureもPython testへ追加する。

### focused command（実装時に実行）

```bash
python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 interface::ui
python3 scripts/dev.py cargo -- test -p hw_ui
python3 -m unittest scripts.tests.test_ui_usability_acceptance
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

native_acceptance.rsのprofiling fixture/observerとui_usability_acceptance.pyのcase・manifest・独立verifierを拡張する。解体/発電/水運搬のSoulを一度だけ準備し、実UI入力と読み取り専用観測で同じ対象の一覧・詳細・Tooltipを記録する。既存の汎用smokeだけではtask分類を証明しない。

新caseは既存の`--smoke`相当の1920×1080/UI scale 1で受入する。layout差を発見した場合だけ既存6組合せへ拡大し、新しいviewport選択機構や性能/Memory比較は導入しない。verifierは可視文字と対象identityを検証し、画像も目視する。

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

Deconstructの修正と分類共通化を分ける。共通化だけを戻しても正しい名称と回帰テストを維持する。Helpは実際に残る表示に合わせる。

戻す前に直近5commit、対象のHEAD差分、他sessionの所有を確認する。
広範囲のcheckout/cleanや他計画の変更破棄を行わず、レビュー可能な限定逆patchを作る。
保存型・asset formatを変える必要が生じた場合は、互換/復旧条件をこの計画へ追記してから変更する。

## 9. AI引継ぎメモ（完了記録）

### 最終状態

タスク表示分類を共有しDeconstructの誤分類を修正。全variantの分類／phase回帰と、解体・発電・給水の一覧／詳細／Tooltipの実入力を確認した。

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
| 2026-09-18 | Codex | R04の独立計画をテンプレートから作成。依存、milestone、受入、Help、検証データ管理を具体化 |
| 2026-09-18 | Codex | 計画レビュー: 専用native caseの実装/検証入口と独立期待値を具体化し、既存smokeを受入範囲の基本にした。 |
| 2026-09-18 | Codex | 全計画ブラッシュアップ: 全17 variantの一覧分類/詳細名/phase表を作り、Deconstructの旧fallbackだけが不一致になるR04-T1/T2を先に追加する。 実装は未着手。 |
| 2026-09-19 | Codex | suite-i全18 checkpoint・独立verify・全画像・無警告を確認。pass seal／整理／finalizeし、本計画を完了・archive |
