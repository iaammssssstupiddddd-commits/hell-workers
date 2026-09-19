# R01: 建設タスクの到達結果と中断契約の統一 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `refactor-r01-construction-navigation-plan-2026-09-18` |
| ステータス | Complete |
| 作成日 | 2026-09-18 |
| 最終更新日 | 2026-09-19 |
| 作成者 | Codex |
| 関連提案 | [実装横断レビュー](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md) §5 R01 |
| 関連Issue/PR | N/A |
| 優先度 / 相対規模 | P1 / 中 |
| 調査基準 | `52913b61ca524f1cd429d3d73a0468faf3d0999b` |
| 計画レビュー | 2026-09-18 / [全体レビュー結果](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md#42-計画レビュー2026-09-18)参照。実装受入ではない |
| 計画具体化 | 2026-09-18 / API案・変更単位・fixture/期待値・着手手順を追加。新規名は未実装の設計案 |
| 実装状態 | 実装・必要な回帰・仕様／Help同期・品質gateを完了。専用native受入は不要。設計経緯としてarchiveし、現行仕様は関連docsを正本とする。 |

## 1. 目的

- 解決したい課題: 到達不能な建設タスクが旧Destinationへの近接から作業開始判定へ進める分岐を解消する。
- 到達したい状態・成功指標: 床補強・床流し込み・壁枠組みの材料中心/タイル移動で、Found後だけ到着判定を行い、Deferredではphaseを変えず、Unreachableでは予約とWorkingOnを正しく解除する。

## 2. スコープ

### 対象（In Scope）

- reinforce_floor / pour_floor / frame_wallの移動分岐と共通navigation入口
- 到達不能・旧目的地・予算切れの回帰テストとtask終端契約

### 非対象（Out of Scope）

- 距離閾値、探索budget、path algorithmの調整
- 床/壁の状態機械全体の汎用化、work duration・資材消費の変更

### 主な変更対象（現行ファイル）

- [crates/hw_soul_ai/src/soul_ai/execute/task_execution/reinforce_floor.rs](../../../../crates/hw_soul_ai/src/soul_ai/execute/task_execution/reinforce_floor.rs)
- [crates/hw_soul_ai/src/soul_ai/execute/task_execution/pour_floor.rs](../../../../crates/hw_soul_ai/src/soul_ai/execute/task_execution/pour_floor.rs)
- [crates/hw_soul_ai/src/soul_ai/execute/task_execution/frame_wall.rs](../../../../crates/hw_soul_ai/src/soul_ai/execute/task_execution/frame_wall.rs)
- [crates/hw_soul_ai/src/soul_ai/execute/task_execution/coat_wall.rs](../../../../crates/hw_soul_ai/src/soul_ai/execute/task_execution/coat_wall.rs)
- [crates/hw_soul_ai/src/soul_ai/execute/task_execution/common.rs](../../../../crates/hw_soul_ai/src/soul_ai/execute/task_execution/common.rs)
- [crates/hw_soul_ai/src/soul_ai/execute/task_execution/path_cache.rs](../../../../crates/hw_soul_ai/src/soul_ai/execute/task_execution/path_cache.rs)

新規moduleやtestは上記ownerの配下に置く。新しいcrateを既定案にせず、定義→生成→使用の順に変更する。

## 3. 現状とギャップ

3 handlerはPathSearchResult::Deferredだけを除外し、Unreachableでもis_near_target_or_destを評価する。path_cacheは失敗時に旧Destinationを残す。coat_wallは3結果を網羅している。成立条件はソースで確認済みだが専用の実行再現は未実施。

提案時点の静的な根拠と実行再現を区別し、M1で現HEADとの差・実際のcaller・既存testを再確認する。
仕様を直す前に実行経路・前提条件を確認し、再現できない仮説への値調整を繰り返さない。

### 既存テスト・検査の入口

- [crates/hw_soul_ai/src/soul_ai/execute/task_execution_system/tests/guards.rs](../../../../crates/hw_soul_ai/src/soul_ai/execute/task_execution_system/tests/guards.rs)
- [crates/hw_soul_ai/src/soul_ai/execute/task_execution_system/tests/aborts.rs](../../../../crates/hw_soul_ai/src/soul_ai/execute/task_execution_system/tests/aborts.rs)
- [crates/hw_soul_ai/src/soul_ai/pathfinding/system/tests.rs](../../../../crates/hw_soul_ai/src/soul_ai/pathfinding/system/tests.rs)

これらは再利用する入口であり、本計画の全条件を既に保証しているという意味ではない。
不足する受入ケースを下記milestoneで追加する。削除・文書整理だけの変更には実装をなぞる新規testを作らない。

## 4. 実装方針と具体設計

- 最初に対象セルと旧Destinationの成立条件を再現し、3結果の網羅matchだけで不正なphase進行を止める。再現できなければ前提と経路を見直し、距離値を調整しない。
- 構造整理では既存NavOutcomeを利用するが、navigate_to_posのtarget-only判定へ機械置換しない。成功後の既存target-or-destination判定を保持する専用入口/到着policyを設け、成功した探索の目的地だけを用いる。
- abort_retryable / abort_closedの選択は対象消失と到達不能を区別し、既存terminal APIへ委ねる。新しい完了通知やraw cleanupを追加しない。

### 具体設計と変更境界

新規API案は `task_execution/common.rs::navigate_to_construction_target`。既存の `NavOutcome` とtarget-or-destination到着判定を組み合わせ、既存 `navigate_to_pos` の他callerは変更しない。

| 入力・結果 | helper / callerの責務 |
| --- | --- |
| 材料中心への移動 | siteのTransformを目的地として渡す。材料entityの位置へ置き換えない。 |
| タイルへの移動 | tile.grid_posをWorldMapの既存変換でworld座標へ変換して渡す。 |
| site/tile消失 | callerが既存 `abort_closed` へ渡す。 |
| Deferred | phase・予約・identity・Destination・Pathを保持し、このframeを終了する。 |
| Unreachable | `abort_retryable` へ渡し、旧Destinationを到着判定に使わない。 |
| Found | `is_near_target_or_dest` と現行閾値 `TILE_SIZE * 1.8` で到着判定する。 |

材料到着後のPickingUp、タイル到着後の作業phase、Path消去はcallerに残す。作業時間と資材消費はhelperへ移さない。

### 実装単位と移行順

1. **R01-A / M1:** 既存 `task_execution_test_app` に隔絶mapを追加し、3 task×2移動phaseの再現表を作る。
2. **R01-B / M2:** reinforce_floor / pour_floor / frame_wallの6分岐だけを網羅matchへ直す。ここで回帰修正を独立してレビューする。
3. **R01-C / M3:** 成功したR01-Bを専用helperへ移す。coat_wallは同じ到着契約の分岐だけ採用し、共通化を戻しても限定修正が残るよう分ける。

### 依存関係と着手順

前提計画なし。最初に着手可能。R02/R07/R09とtask実行の共有ファイルを変更する時は、R01の小さい修正を先に確定して順次取り込む。

関連計画へのリンクと全体の着手順は[親提案の計画一覧](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md)を参照。
依存は実装上の前提と共有ファイルの編集競合を区別する。全14件の完了を本計画の終了条件にしない。

### Bevy 0.19と変更の所有者

- 既存の正しい0.19実装を基準とし、新しいAPIはrust-analyzer/docsrs MCPまたはlocal registryで確認してから使う。
- Queryはcrate-owned contextへまとめ、rootの配線・leafの実装所有を維持する。lint抑制で構造問題を隠さない。
- コードと文書の編集は主担当が行う。subagentは読み取り専用の調査/レビューに限定する。
- 共通Help snapshot・docs索引・凍結native subjectは並行編集しない。

## 5. マイルストーン

## M1: 成立条件と回帰例を固定

- 変更内容:
  - 3 task × 材料中心/タイル移動の入力表を作り、旧DestinationがSoul現在地、対象が非隣接かつ到達不能となる小さいWorldを構成する。
  - 正常到達・Deferredも併記し、現行の不正遷移だけが失敗するテストを追加する。
- 変更ファイル: R01-A: task_execution_system/tests/mod.rs の既存Appと guards.rs / aborts.rs 配下の回帰test。
- 完了条件:
  - [x] 対象3経路で再現結果を記録し、探索結果とphase/予約/WorkingOnを同じテストで観測できる。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M2: 網羅matchで限定修正

- 変更内容:
  - Found / Deferred / Unreachableを全移動分岐で明示し、到達不能時はterminal APIを呼ぶ。
  - 旧Destination近傍を到着と見なす処理を探索成功後に限定する。
- 変更ファイル: R01-B: task_execution/{reinforce_floor,pour_floor,frame_wall}.rs の6移動分岐。
- 完了条件:
  - [x] 不正進行を防ぎ、Deferredでphase・予約・完了通知が変化しない。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M3: navigation入口を整理

- 変更内容:
  - 成功後の到着policyを保持した共通入口へ3 handlerを移す。
  - coat_wall等の既存利用先への横展開は同じ契約と確認できた範囲に限定する。
- 変更ファイル: R01-C: task_execution/common.rs と上記3 handler。同じ契約と確認できた coat_wall.rs の分岐のみ追従。
- 完了条件:
  - [x] 挙動修正の回帰テストを保ち、共通化だけを独立して戻せる差分になっている。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M4: 仕様・Help・品質ゲートと終了整理

- 変更内容:
  - 下記仕様へ最終的な所有者・順序・失敗条件を反映する。計画の検討経緯をそのまま恒久仕様へ転記しない。
  - [Help影響Skill](../../../../.cursor/skills/hell-workers-review-help-impact/SKILL.md)で全実装差分のplayer-visible経路を確認する。
  - 到達不能時の作業結果/失敗条件を追い、既存Helpの説明と照合する。挙動修正を単に内部refactorと呼んでNo impactにしない。
  - 受入と品質ゲートの結果を記録し、専用出力を整理する。
- 仕様更新対象（該当する契約を変更した文書だけを更新）:
  - [docs/tasks.md](../../../../docs/tasks.md)
  - [docs/soul_ai.md](../../../../docs/soul_ai.md)
  - [docs/invariants.md](../../../../docs/invariants.md)
- 完了条件:
  - [x] §7のケースと必要なnative受入が完了し、未検証範囲を明記した。
  - [x] HelpがUpdate required / No impactのいずれかに確定し、対応する更新または根拠を記録した。
  - [x] `check`・Clippy・`verify`が成功し、rust-analyzerの取得可能な診断にerrorがない。
  - [x] primary validation storage checkが成功し、不要出力の整理/継続用途を記録した。
- 検証: §7の完了gate。品質gateのpassだけをHelpの意味レビューやnative受入の代替にしない。

## 6. リスクと対策

| リスク | 対策 |
| --- | --- |
| helperへの機械置換で到着範囲が変わる | 探索成功後の既存target-or-destination意味をcharacterization testで固定する。 |
| 中断と完了を混同する | 資材・予約・WorkingOn・terminal通知を一緒に検査する。 |
| 共有ファイルの並行変更を上書きする | 作業開始/各取り込み前にstatusと対象diffを確認し、変更所有を確認する。 |
| 過去のpassを新subjectへ流用する | 変更後の入力を固定して必要な検査を実施し、未実施は未実施と記録する。 |

## 7. 検証計画

### ケースと判定条件

| ケース | 期待する結果 |
| --- | --- |
| 旧目的地が現在地・次tileが隔絶 | 作業phaseに入らず、retryable中断と予約解放。完了通知なし。 |
| 材料中心自体が到達不能 | 資材pickupへ進まない。 |
| 探索budget不足 | task/予約を保持して次frameへ繰越し、abortしない。 |
| 正常な隣接到達・移動途中 | 従来の成功タイミングと移動継続を維持。 |

### 回帰fixtureの作り方（新規test案）

| ID / test案 | 準備・操作 | 必須assert |
| --- | --- | --- |
| R01-T1 `construction_unreachable_keeps_work_unstarted_test` | 既存test mapのx=50全列を閉鎖し、Soul=(25,50)、対象=(75,50)、旧Destination=Soul位置。3 task×材料中心/タイルで1 update。 | task解除、WorkingOnなし、tile状態・site counter・資材量不変、完了通知0、予約解放1回。 |
| R01-T2 `construction_deferred_preserves_assignment_test` | T1で探索budgetを0にする。 | phase・Destination・Path・identity・予約が全て更新前と同じ。abort/完了通知0。 |
| R01-T3 `construction_found_preserves_arrival_policy_test` | 壁を開け、成功した探索の目的地近傍/移動途中/対象近傍を分ける。 | 従来の到着範囲と次phase、Path消去を維持。移動途中は作業しない。 |

fixtureの地形、site/tile relationship、予約を有効にし、別のguardで早期中断していないことを探索結果と併せて確認する。test名は案であり、追加後の実名をfocused commandへ反映する。

### focused command（実装時に実行）

```bash
python3 scripts/dev.py cargo -- test -p hw_soul_ai task_execution
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

この計画の正しさは決定的なECS回帰テストで判定する。新しい表示・入力を変更しない限り専用native/GPU計測は必須にしない。実画面確認を追加する場合はnative Skillとprimary coordinatorを使い、headless成功を実機証拠へ読み替えない。

実機確認を実施する場合は[Native Acceptance Skill](../../../../.cursor/skills/hell-workers-run-native-acceptance/SKILL.md)を読み直す。
primary `dev.py validation plan --spec ... -- <既存helperのplan>`へ登録し、返されたno-prompt kitty launcherだけを使う。
fixtureが対象を覆わなければ先に専用caseと独立verifierを用意する。汎用smokeの成功を個別要件の証明にしない。
Capture/Memoryを含むrecipeは逐次実行し、read-only observer・source/asset/binary一致・timeoutを検証する。
通常の修正feedbackでは同じdev cacheを再利用し、headless・feedback・正式受入・性能結果を区別する。

### 検証データ管理（各バッチの開始前・報告前に更新）

正本は[検証データ管理](../../../development-infra/validation-storage-workflow.md)。実行開始/再開時に必ず読み直す。
通常quality gateは既存primary cacheで行い、専用native/performance出力を作る場合にretain/plan/executeを登録する。

| 項目 | 完了時点 |
| --- | --- |
| batch / owner | 専用native batch不要。主担当Codexがprimaryで実装・通常品質gateを実行 |
| workspace / subject | primaryの作業差分。基準HEADはメタ情報の調査基準を参照 |
| artifact / 開始・終了bytes | 本計画専用のartifact・binary copy・worktreeなし |
| 採用成果と最終結果 | codeと関連仕様はprimary。本書§9に最終検証結果を記録 |
| 削除path / 容量差 | 本計画固有の削除なし、専用出力0 bytes |
| 継続保持 | primary通常Cargo cacheは継続開発用。R13/R14用candidateは各計画・primary保持台帳に用途を記録 |
| 整理状態 | 本計画の受入を完了。未終了の別計画を理由に専用jobを保持しない |

成功は元依存が残る間に独立verifyしてsealする。失敗/中断も理由・未検証範囲をsealし、不要jobを整理してfinalize/checkを通す。
job終了とcandidate/cache解放は分ける。review-activeの無応答・中間passを終了と解釈しない。
保持にはowner・consumer・bytes・次作業・release_whenが必要で、親track未完だけを保持理由にしない。
全jobのcapsule保存、一律日数/容量上限を新設しない。primaryの通常Cargo cacheは別の保守寿命を持つ。

## 8. ロールバック方針

M2の不具合修正とM3の共通化を別commit候補にする。M3だけを戻しても網羅matchと回帰テストを残せる形にする。

戻す前に直近5commit、対象のHEAD差分、他sessionの所有を確認する。
広範囲のcheckout/cleanや他計画の変更破棄を行わず、レビュー可能な限定逆patchを作る。
保存型・asset formatを変える必要が生じた場合は、互換/復旧条件をこの計画へ追記してから変更する。

## 9. AI引継ぎメモ（完了記録）

### 現在地

- 2026-09-19完了。建設navigationの6分岐matrixを追加し、到達不能・予算待ち・到達時の回帰が成功。
- `python3 scripts/dev.py verify`が成功。通常／profilingのworkspace test、通常Clippy警告0、feature別compile、Python、Help、文書・storage gateを確認した。
- Helpは全体差分でUpdate required。Move失敗時の保持、タスク表示、建設工程の3 entryとexact snapshotを更新し、実際のplayer-visible経路と差分をレビュー済み。
- 本計画単体の専用native job／binary copy／worktreeは作成していない。通常primary cacheは継続開発用に保持する。R13/R14用candidateと未終了native確認は各計画が所有し、本計画の完了と分離する。

### 次のAIが最初にやること

現行の関連仕様と実装を参照する。本書の設計案・着手手順を未実装の作業として再実行しない。
新しい変更ではその差分に応じた回帰、Help判断、storage確認を行う。

### 最終確認ログ

| 検査 | 結果 |
| --- | --- |
| focused tests / 新規回帰 | 建設navigationの6分岐matrixを追加し、到達不能・予算待ち・到達時の回帰が成功。 |
| check / rust-analyzer | workspace compile成功。workspace rust-analyzer診断はMCPのnull応答により取得不可で、compileとClippyで代替確認 |
| Clippy / verify | 通常Clippy警告0、verify成功 |
| Help判断 / 仕様同期 | Update requiredとして3 entry・exact snapshot・関連仕様を同期 |
| native / GPU / 性能 | 本計画では不要。性能改善の定量主張なし |
| validation storage | primary `validation check`成功。専用出力なし、他計画の用途を持つcacheを保持 |
| 未解決エラー | なし。上記MCPの取得制約は残る |

### Definition of Done

- [x] 実装と必要な回帰、仕様・Help同期を完了
- [x] workspace compile・Clippy警告0・verify・storage checkが成功
- [x] 本計画固有の不要job／専用環境なし。関連計画の現用途を持つcacheを保持
- [x] primaryのarchiveへ移動し、計画・提案索引を再生成

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-09-18 | Codex | R01の独立計画をテンプレートから作成。依存、milestone、受入、Help、検証データ管理を具体化 |
| 2026-09-18 | Codex | 計画レビュー: 3結果の分岐、成功時の到着policy、限定修正→共通化の順序を確認。追加修正不要。 |
| 2026-09-18 | Codex | 全計画ブラッシュアップ: R01-T1の隔絶mapを既存task_execution_test_appに作り、6分岐が対象guardを通ることと修正前の誤遷移を確認する。 実装は未着手。 |
| 2026-09-19 | Codex | 実装・回帰・Help／仕様同期と品質gateを完了しarchive。 |
