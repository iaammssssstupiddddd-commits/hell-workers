# R12: ProgressBarの親所有APIと床・壁のライフサイクル共有 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `refactor-r12-progress-bar-lifecycle-plan-2026-09-18` |
| ステータス | Complete |
| 作成日 | 2026-09-18 |
| 最終更新日 | 2026-09-19 |
| 作成者 | Codex |
| 関連提案 | [実装横断レビュー](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md) §5 R12 |
| 関連Issue/PR | N/A |
| 優先度 / 相対規模 | P3 / 中 |
| 調査基準 | `52913b61ca524f1cd429d3d73a0468faf3d0999b` |
| 計画レビュー | 2026-09-18 / [全体レビュー結果](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md#42-計画レビュー2026-09-18)参照。実装受入ではない |
| 計画具体化 | 2026-09-18 / API案・変更単位・fixture/期待値・着手手順を追加。新規名は未実装の設計案 |
| 実装状態 | 本体・回帰・仕様／Help同期、品質gate、必要なnative受入と終了整理を完了。現行仕様は関連docsを正本とする。 |

## 1. 目的

- 解決したい課題: 親引数を受けながら接続しないProgressBar APIと、callerに分散した親子付与・生成破棄を明確にする。
- 到達したい状態・成功指標: 親接続済みbar pairを返す共通APIで4 callerが動作し、床/壁の生成破棄を共有してもfill・Z・完成/取消/load後の消滅が不変。

## 2. スコープ

### 対象（In Scope）

- hw_visual ProgressBar APIとSoul/Blueprint/Floor/Wall caller
- 床/壁のbar lifecycle共有とECS/native回帰

### 非対象（Out of Scope）

- Soul/Blueprintの状態機械統合
- barデザイン変更、アニメーション速度変更、GPU性能最適化

### 主な変更対象（現行ファイル）

- [crates/bevy_app/src/interface/ui/native_acceptance.rs](../../../../crates/bevy_app/src/interface/ui/native_acceptance.rs)（progress-bar専用fixtureとworld Sprite観測）
- [.codex/skills/hell-workers-run-native-acceptance/scripts/ui_usability_acceptance.py](../../../../.codex/skills/hell-workers-run-native-acceptance/scripts/ui_usability_acceptance.py)（専用case・manifest・独立verifier）
- [scripts/tests/test_ui_usability_acceptance.py](../../../../scripts/tests/test_ui_usability_acceptance.py)
- [crates/hw_visual/src/progress_bar.rs](../../../../crates/hw_visual/src/progress_bar.rs)
- [crates/hw_visual/src/soul/systems.rs](../../../../crates/hw_visual/src/soul/systems.rs)
- [crates/hw_visual/src/blueprint/progress_bar.rs](../../../../crates/hw_visual/src/blueprint/progress_bar.rs)
- [crates/hw_visual/src/floor_construction.rs](../../../../crates/hw_visual/src/floor_construction.rs)
- [crates/hw_visual/src/wall_construction.rs](../../../../crates/hw_visual/src/wall_construction.rs)

新規moduleやtestは上記ownerの配下に置く。新しいcrateを既定案にせず、定義→生成→使用の順に変更する。

## 3. 現状とギャップ

spawn helperの_parent/_parent_transformは未使用でlocal座標の2entityを返す。ChildOfはcallerが付与する。床/壁には可視対象の親収集、生成、不要bar破棄の重複がある。

提案時点の静的な根拠と実行再現を区別し、M1で現HEADとの差・実際のcaller・既存testを再確認する。
仕様を直す前に実行経路・前提条件を確認し、再現できない仮説への値調整を繰り返さない。

### 既存テスト・検査の入口

- [crates/hw_visual/src/blueprint/progress_bar.rs](../../../../crates/hw_visual/src/blueprint/progress_bar.rs)

これらは再利用する入口であり、本計画の全条件を既に保証しているという意味ではない。
不足する受入ケースを下記milestoneで追加する。削除・文書整理だけの変更には実装をなぞる新規testを作らない。

## 4. 実装方針と具体設計

- 背景/fillのpairと親を一つの生成操作で返す。Bevy 0.19の既存ChildOf/linked despawn patternを確認し、Relationship targetは手動編集しない。
- 親Transformなど不要入力をなくし、callerの二重ChildOf付与を撤去する。local位置・Z・比率更新と、Blueprintの既存の同値Transform非書込み契約を維持する。他callerの書込み条件はM1で基準を確認し、全callerに同じ現行契約があるとは扱わない。
- 床/壁はvisible/ratio/color/configの導出を残し、共通lifecycleだけを利用する。Soul/Blueprint固有task progressを汎用化しない。

### 具体設計と変更境界

API案は `spawn_progress_bar(commands, owner, config) -> ProgressBarPair { background, fill }`。
helper内で両entityへChildOf(owner)を付ける。**背景とfillは同じ親の兄弟**とし、fillを背景の子へ変えてZ/Transform合成を変更しない。位置同期APIから未使用のparent Transformを除く。

Soulは既存SoulUiLinks、BlueprintはBlueprintProgressBarsを保持する。床/壁だけに `reconcile_site_progress_bars<M>`（案）を置き、callerが導出した(owner, config)と既存(bar, owner)を受けて生成/破棄・marker付与を共通化する。ratio/color更新と可視条件の導出は各callerに残す。

### 実装単位と移行順

1. **R12-A / M1:** 4 callerの親・local座標・端値・終了時消滅をECSで固定。
2. **R12-B / M2:** pair生成APIを先に移行し、4 callerの重複ChildOf付与と不要Transform引数を撤去。
3. **R12-C / M3:** 床/壁lifecycleだけを共有。既存NativeUiへ `progress-bars` case（案）を追加し、production同期元を持つfixtureを作る。

native fixtureでは通常の「Familiar無効化＋大量Chop」準備を専用caseで通さない。実際のSoul/Blueprint/床/壁の同期元を初期化し、以後はsimulationと実入力で進める。
read-only observerはbar種別・owner・親・比率・GlobalTransformとcamera/RenderLayersに対応した投影矩形を記録する。world SpriteにUIのComputedNode矩形を流用しない。

### 依存関係と着手順

必須先行なし。R11のvisual README整理と同じ所有説明を触る場合は後続が引き継ぐ。R13とhw_visual/lib.rsの登録やnative凍結subjectを同時変更しない。
R04/R08とnative fixture/helper/testの3ファイルを共有する。case追加は順次取り込み、既存caseを維持する。

関連計画へのリンクと全体の着手順は[親提案の計画一覧](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md)を参照。
依存は実装上の前提と共有ファイルの編集競合を区別する。全14件の完了を本計画の終了条件にしない。

### Bevy 0.19と変更の所有者

- 既存の正しい0.19実装を基準とし、新しいAPIはrust-analyzer/docsrs MCPまたはlocal registryで確認してから使う。
- Queryはcrate-owned contextへまとめ、rootの配線・leafの実装所有を維持する。lint抑制で構造問題を隠さない。
- コードと文書の編集は主担当が行う。subagentは読み取り専用の調査/レビューに限定する。
- 共通Help snapshot・docs索引・凍結native subjectは並行編集しない。

## 5. マイルストーン

## M1: 4 callerの所有契約を固定

- 変更内容:
  - 全spawn/update callerを参照検索し、親/背景/fill数、local Z、ratio、消滅条件を表にする。
  - 既存Blueprint lifecycle testを基準に床/壁/Soulの不足ケースを追加する。
  - 既存NativeUi入口にprogress-bar caseを追加する設計を固定する。汎用fixtureのFamiliar作業禁止・大量Chop指定はこのcaseで実行せず、4callerのproduction同期元を満たす状態を一度だけ準備する。新しいperf workloadや独立pluginは追加しない。
- 変更ファイル: R12-A: hw_visual/src/progress_bar.rs とSoul/Blueprint/Floor/Wall各callerのECS test。
- 完了条件:
  - [x] 0/中間/完了、owner消失、同値更新の期待値が固定されている。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M2: 親接続済みAPIへ移行

- 変更内容:
  - bar pairの生成時に親を付与し、不要Transform引数を撤去する。
  - 4 callerを移行して二重生成/二重親付与を防ぐ。
- 変更ファイル: R12-B: progress_bar.rs、Soul progress UI、blueprint/progress_bar.rs、floor_construction.rs、wall_construction.rs。
- 完了条件:
  - [x] 非原点の親に追従し、owner despawnでbarが残らない。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M3: 床/壁のlifecycle共有

- 変更内容:
  - 可視性とratio導出はcallerに残し、生成・破棄の共通処理へ移す。
  - 同frameの取消/phase終了、world replacementを検証する。
  - native helperのcheckpoint・入力列・環境・manifest・verifierへcaseを通す。UIのComputedNode矩形を流用せず、world Spriteの親/GlobalTransformとcamera投影位置を観測する。
- 変更ファイル: R12-C: 床/壁の共通lifecycleと既存interface/ui/native_acceptance.rs・UI helper・Python verifier testの3入口。
- 完了条件:
  - [x] bar数・親・fill・消滅契約が不変で、重複lifecycleがなくなる。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M4: 仕様・Help・品質ゲートと終了整理

- 変更内容:
  - 下記仕様へ最終的な所有者・順序・失敗条件を反映する。計画の検討経緯をそのまま恒久仕様へ転記しない。
  - [Help影響Skill](../../../../.cursor/skills/hell-workers-review-help-impact/SKILL.md)で全実装差分のplayer-visible経路を確認する。
  - progress意味・表示時点・操作が不変か実画面と経路を確認し、内部所有整理ならNo impact。表示結果が変わった場合は再判定する。
  - 受入と品質ゲートの結果を記録し、専用出力を整理する。
- 仕様更新対象（該当する契約を変更した文書だけを更新）:
  - [docs/building.md](../../../building.md)
  - [docs/gather_haul_visual.md](../../../gather_haul_visual.md)
  - [docs/cargo_workspace.md](../../../cargo_workspace.md)
  - [docs/architecture.md](../../../architecture.md)
- 完了条件:
  - [x] §7のケースと必要なnative受入が完了し、未検証範囲を明記した。
  - [x] HelpがUpdate required / No impactのいずれかに確定し、対応する更新または根拠を記録した。
  - [x] `check`・Clippy・`verify`が成功。workspace RA診断は上記MCP制約を記録。
  - [x] primary validation storage checkが成功し、不要出力の整理/継続用途を記録した。
- 検証: §7の完了gate。品質gateのpassだけをHelpの意味レビューやnative受入の代替にしない。

## 6. リスクと対策

| リスク | 対策 |
| --- | --- |
| ChildOfの重複付与や孤児entity | spawn ownerを一つにしbar数/linked despawnをテスト。 |
| 共通化で床壁の可視条件が揃ってしまう | visible/ratio/color/config導出をcallerに残す。 |
| 共有ファイルの並行変更を上書きする | 作業開始/各取り込み前にstatusと対象diffを確認し、変更所有を確認する。 |
| 過去のpassを新subjectへ流用する | 変更後の入力を固定して必要な検査を実施し、未実施は未実施と記録する。 |

## 7. 検証計画

### ケースと判定条件

| ケース | 期待する結果 |
| --- | --- |
| 親が原点外・移動/resize | local位置とZ、fillが正しく追従。 |
| 0/中間/完了・同値frame | 比率表示と各callerの更新条件を維持。Blueprintの同値Transform非書込みは既存testで確認し、他callerはM1で確認した基準と比較する。 |
| 取消・owner despawn・load | 背景/fillの孤児・重複・残像なし。 |
| Soul/Blueprint/床養生/壁Framing→Coating | 各状態で表示/非表示が従来どおり。 |

### 回帰fixtureの作り方（新規test案）

| ID / test案 | 準備・操作 | 必須assert |
| --- | --- | --- |
| R12-T1 `progress_pair_is_attached_to_owner_test` | 原点外のownerで4 callerのbarを生成し、親を移動。 | 背景/fill各1、同一ChildOf、既存local offset/Z、親移動後の正しいGlobalTransform。 |
| R12-T2 `progress_ratio_keeps_left_edge_test` | ratio=0 / 0.5 / 1をECSで反映。 | 幅と左端位置が現行式どおり。Blueprintの同値Transform非書込みも維持。 |
| R12-T3 `progress_lifecycle_leaves_no_orphans_test` | task/phase終了、取消、owner despawn、world replacementを適用。 | 必要なbarだけ残り、背景/fillの孤児・重複なし。適用flush後に検査。 |
| R12-T4 native `progress-bars` | production fixtureで中間進捗を保持し、unpause/取消/loadを実入力。4 callerと適用可能な終了経路を観測。 | 親追従と中間表示、終了時消滅が画像とobserverで対応。完了frameに満杯画像があることは要求しない。 |

厳密な0/1・同frame完了はT2/T3で判定する。nativeで全状態×全viewportの直積を作らず、4 callerの可視性と適用可能な取消/loadを覆う。

### focused command（実装時に実行）

```bash
python3 -m unittest scripts.tests.test_ui_usability_acceptance
python3 scripts/dev.py cargo -- test -p hw_visual progress_bar
python3 scripts/dev.py cargo -- test -p hw_visual construction
python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 rehydrate
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

比率0/1と同frameのphase終了・取消はECS testで保証する。nativeは4callerの安定した中間表示、親追従、通常simulationまたは実UI入力による終了・適用可能な取消/load後の消滅を確認する。完了時の満杯bar撮影は要求せず、caller契約どおり非表示になることを成功とする。

既存native_acceptance.rsとui_usability_acceptance.pyへ専用caseを追加し、`--smoke`相当の1画面とnonce/PID/input ACKを再利用する。初期準備後のobserverはtask/phase/比率を書き換えず、bar種別、owner/ChildOf、背景/fill数、比率、GlobalTransform、camera投影した画面位置、消滅を記録する。test_ui_usability_acceptance.pyでcaller欠落・孤児/重複・画面外・偽の終了証拠を拒否する。Wall/Door storyboardだけを証拠にせず、GPU capture・性能/Memory matrixは要求しない。

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

親接続API移行と床/壁lifecycle共通化を別単位にする。lifecycleだけ戻す場合も親所有契約は維持する。

戻す前に直近5commit、対象のHEAD差分、他sessionの所有を確認する。
広範囲のcheckout/cleanや他計画の変更破棄を行わず、レビュー可能な限定逆patchを作る。
保存型・asset formatを変える必要が生じた場合は、互換/復旧条件をこの計画へ追記してから変更する。

## 9. AI引継ぎメモ（完了記録）

### 最終状態

親接続APIと床／壁bar lifecycleを共通化。4 callerの回帰、中間表示、完了、Save/Load再構成と取消を確認し、Load時のsite Visibility欠落も修正した。

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
| 2026-09-18 | Codex | R12の独立計画をテンプレートから作成。依存、milestone、受入、Help、検証データ管理を具体化 |
| 2026-09-18 | Codex | 計画レビュー: 境界値ECSとnative中間/終了表示を分離し、既存NativeUiへの専用caseとworld Sprite観測を具体化。 |
| 2026-09-18 | Codex | 全計画ブラッシュアップ: 4 callerで原点外の親とratio=0/0.5/1を持つR12-T1/T2を作り、背景とfillが兄弟である現行座標契約を固定する。 実装は未着手。 |
| 2026-09-19 | Codex | suite-i全18 checkpoint・独立verify・全画像・無警告を確認。pass seal／整理／finalizeし、本計画を完了・archive |
