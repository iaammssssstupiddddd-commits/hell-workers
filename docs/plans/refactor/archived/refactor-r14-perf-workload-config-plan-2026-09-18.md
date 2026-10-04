# R14: perf設定のworkload別型とvalidation境界の整理 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `refactor-r14-perf-workload-config-plan-2026-09-18` |
| ステータス | Complete |
| 作成日 | 2026-09-18 |
| 最終更新日 | 2026-09-19 |
| 作成者 | Codex |
| 関連提案 | [実装横断レビュー](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md) §5 R14 |
| 関連Issue/PR | N/A |
| 優先度 / 相対規模 | P3 / 中〜大 |
| 調査基準 | `52913b61ca524f1cd429d3d73a0468faf3d0999b` |
| 計画レビュー | 2026-09-18 / [全体レビュー結果](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md#42-計画レビュー2026-09-18)参照。実装受入ではない |
| 計画具体化 | 2026-09-18 / API案・変更単位・fixture/期待値・着手手順を追加。新規名は未実装の設計案 |
| 実装状態 | 11 workloadの検証済みpayload、注入可能なparse入力、workload別Python validator／sidecar dispatchを実装。代表native受入・品質gate・終了整理を完了。 現行仕様は関連docsを正本とする。 |

## 1. 目的

- 解決したい課題: workload固有Option/flagと複数箇所の長い条件分岐を整理し、制約変更の追従漏れを減らす。
- 到達したい状態・成功指標: raw CLI/env入力から検証済みworkload設定への変換が一度だけ行われ、既存の受理/拒否・隔離・artifact独立検証・launcher契約が維持される。

## 2. スコープ

### 対象（In Scope）

- Rust PerfScenarioConfigの共通設定/検証済みpayload
- Python workload別argument validatorとartifact dispatchの整理
- 既存CLI/env compatibility、self-testと代表native smoke

### 非対象（Out of Scope）

- CLI/環境変数/manifest schemaの仕様変更
- runnerを汎用plugin frameworkへ置換
- verifierの期待値をproduction側から生成すること
- 計測結果の高速化比較や歴史的baseline再作成

### 主な変更対象（現行ファイル）

- [crates/bevy_app/src/plugins/startup/perf_scenario/config.rs](../../../../crates/bevy_app/src/plugins/startup/perf_scenario/config.rs)
- [crates/bevy_app/src/plugins/startup/perf_scenario/config/parse.rs](../../../../crates/bevy_app/src/plugins/startup/perf_scenario/config/parse.rs)
- [crates/bevy_app/src/plugins/startup/perf_scenario/config/tests.rs](../../../../crates/bevy_app/src/plugins/startup/perf_scenario/config/tests.rs)
- [crates/bevy_app/src/main.rs](../../../../crates/bevy_app/src/main.rs)（起動時の設定読込・拒否と通常window設定）
- [crates/bevy_app/src/plugins/startup/perf_scenario.rs](../../../../crates/bevy_app/src/plugins/startup/perf_scenario.rs)と[配下のconsumer](../../../../crates/bevy_app/src/plugins/startup/perf_scenario/)（fixture・driver・出力・run condition）
- [crates/bevy_app/src/plugins/startup/startup_systems.rs](../../../../crates/bevy_app/src/plugins/startup/startup_systems.rs)（通常起動と乱数stream）
- [scripts/perf_tool/arguments.py](../../../../scripts/perf_tool/arguments.py)
- [scripts/perf_tool/model.py](../../../../scripts/perf_tool/model.py)
- [scripts/perf_tool/artifacts.py](../../../../scripts/perf_tool/artifacts.py)
- [scripts/perf_tool/execution.py](../../../../scripts/perf_tool/execution.py)
- [scripts/perf_tool/selftest.py](../../../../scripts/perf_tool/selftest.py)

新規moduleやtestは上記ownerの配下に置く。新しいcrateを既定案にせず、定義→生成→使用の順に変更する。

## 3. 現状とギャップ

config.rsの共通resourceにworkload固有Option/flagが並び、Rust parse・Python arguments・artifactsに対応分岐がある。Rustは単発実行、Pythonはmatrix/instrumentationを管理し、両者の制約は完全同一ではない。

`try_from_process`はperf無効なら他のperf引数を解析せずDefaultを返し、有効かつprofiling featureなしなら拒否する。既存config testsは主に個別helperと手動構築したconfigを検査し、CLI/env全体のparse経路を網羅していない。入口の互換テストを追加しなければ、型の整理だけで通常起動を壊す変更を検出できない。

提案時点の静的な根拠と実行再現を区別し、M1で現HEADとの差・実際のcaller・既存testを再確認する。
仕様を直す前に実行経路・前提条件を確認し、再現できない仮説への値調整を繰り返さない。

### 既存テスト・検査の入口

- [crates/bevy_app/src/plugins/startup/perf_scenario/config/tests.rs](../../../../crates/bevy_app/src/plugins/startup/perf_scenario/config/tests.rs)
- [scripts/perf_tool/selftest.py](../../../../scripts/perf_tool/selftest.py)
- [scripts/tests/test_validation_storage.py](../../../../scripts/tests/test_validation_storage.py)
- [scripts/tests/test_cargo_runtime.py](../../../../scripts/tests/test_cargo_runtime.py)

これらは再利用する入口であり、本計画の全条件を既に保証しているという意味ではない。
不足する受入ケースを下記milestoneで追加する。削除・文書整理だけの変更には実装をなぞる新規testを作らない。

## 4. 実装方針と具体設計

- raw入力をparseする段階とvalidated configをconstructする段階を分ける。共通設定＋enum payloadへ寄せ、検証前のworkload固有設定をruntime consumerへ渡さない。
- `try_from_process`はOS入力取得の薄い入口とし、明示args・env lookup/snapshotを受ける検証入口を設ける。並列testからprocess全体の環境変数を書き換えない。perf無効時の早期Default、feature不足の拒否、未指定seedの生成時点を維持する。
- validated payloadとそのdiscriminantは外部から個別変更できない構造にする。既存の公開field・直接構築・test fixtureを棚卸しし、必要なread accessorと検証を通るtest factoryへ段階移行する。独立したnative profileのparser統合は対象外。
- Pythonはworkload別validationとartifact reader dispatchを小さなmodule/functionへ分ける。Rustの単発制約とPythonのmatrix制約を別々に維持する。
- 新旧の受理/拒否caseを比較し、現行で拒否する矛盾env・未対応instrumentationはfail-closedを保持する。未知flagの扱いはRustの必要引数探索とPython CLIで区別して固定し、新しい一律拒否へ変更しない。CLI precedence、errorの主要理由、default、schema/hash契約は変更しない。
- Capture→Memory逐次、exact source/assets、元artifact検証、settings/save隔離、activity/storage guardは既存ownerに残す。共通化で実装と独立verifierの判定を共有しない。

### 具体設計と変更境界

以下の型・関数・新規module名は実装案。既存の公開入口を残して内部を段階移行する。

| 配置・入口 | 具体的な責務 |
| --- | --- |
| `config/parse.rs` → `PerfConfigInput`（案） | argsの順序を保持し、必要なenvだけをlookupから読む。一般のCLI優先とpaired key検査を別helperにする。OS入力の取得は`try_from_process`だけに残す。 |
| `config/validated.rs`（新規案） | `PerfScenarioState::{Disabled, Enabled(ValidatedPerfScenario)}`、privateな共通設定とworkload payloadを保持。public setterは作らず、既存accessor/出力へ同じ値を返す。 |
| `ValidatedWorkload`（案） | 現行11 workloadを網羅。Gatherのpolicy/dialog、IndoorLightのselection/behavior、WallDensityのphase/presentation、DoorDensityのpresentationを各payloadへ置く。固有値のないvariantはunitでよい。 |
| 共通設定 | seed、size/population、render/window/quality、clock/ticks、計測時間、出力先を保持。現在受理して出力している非アクティブな値も勝手に正規化/破棄せず、互換projectionで比較する。 |
| `arguments.py::validate_arguments` | 公開signatureとNamespaceへの正規化結果を保持。共通条件→workload別条件の既存順を維持し、`argument_validation/{common,density,indoor_light,specialized}.py`（新規案）へ分割する。 |
| `artifacts.py::validate_run` | 共通window/log/summary検査を残し、workload固有sidecar読込だけ小関数へ委譲。既存`artifact_readers/`を再利用し、期待値をRust設定から生成しない。 |

入力取得→有効/無効判定→profiling feature判定→既存順でparse/制約検査→validated値構築の順とする。
profiling-renderdocの判定はseed・計測時間・出力先のparse後に維持する。全feature判定を先頭へ寄せて最初のerrorやseed生成回数を変えない。
Disabledは専用perf値をparseせず既存Defaultのobservable値を返す。未指定seedのproviderは必要になった位置で一度だけ呼ぶ。
`allow_log_pattern`と`behavior_cases`はPython側が実際に書き換える値なので、受理/拒否だけでなく正規化後の値・順序も固定する。
behavior_casesはNoneの場合だけ既定の順序で補完し、明示された文字列は受理時も書き換えない。
型構築のtest factoryにも同じ検証入口を通し、productionの不変条件を回避するpublic constructorは追加しない。

### 実装単位と移行順

| 単位 | 対象と成果物 | 次へ進む条件 |
| --- | --- | --- |
| R14-A / M1 | 現行11 workload×入口のcompatibility表を`config/tests.rs`とPython self-testのtable fixtureにする。parse入口へのargs/env/seed注入を追加。 | 有効/無効、paired key、feature、正規化後の値を現行結果と比較できる。 |
| R14-B / M2 | privateなvalidated型とread accessorを追加。main/startup→fixture/run condition→outputを順に移す。 | 各群で同じ設定・schema出力になり、全consumer移行後に旧public fieldを撤去。 |
| R14-C / M3 | Python validationを共通→density→indoor-light→特殊workloadの順に抽出し、最後にsidecar dispatchを整理。 | 各抽出ごとに受理/拒否・正規化結果が一致。旧分岐と新分岐を恒久併存させない。 |
| R14-D / M4 | 下記代表recipeを新subjectで実行し、busy/admission・Capture→Memory・artifact拒否を確認。 | 独立verifyが成功し、全workloadの実機passとは報告しない。 |

代表実行は既存`native_acceptance.py plan-task-dashboard`を第一候補とし、同recipeのheadless audit→actual-window Capture→Memoryで共通入口を通す。
IndoorLight固有のconfig/sidecar dispatchを変更した場合は現行P08 closureを追加する。Wall/Door専用制約はまずtable fixtureで固定し、専用入口/launch argvが変わった場合だけ対応する既存recipeを追加する。

### 依存関係と着手順

他案の必須先行なし。R13とRenderDoc/config/source fingerprintの編集が重なるため直列化する。稼働中/凍結中のnative batchが参照するhelperは書き換えない。

関連計画へのリンクと全体の着手順は[親提案の計画一覧](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md)を参照。
依存は実装上の前提と共有ファイルの編集競合を区別する。全14件の完了を本計画の終了条件にしない。

### Bevy 0.19と変更の所有者

- 既存の正しい0.19実装を基準とし、新しいAPIはrust-analyzer/docsrs MCPまたはlocal registryで確認してから使う。
- Queryはcrate-owned contextへまとめ、rootの配線・leafの実装所有を維持する。lint抑制で構造問題を隠さない。
- コードと文書の編集は主担当が行う。subagentは読み取り専用の調査/レビューに限定する。
- 共通Help snapshot・docs索引・凍結native subjectは並行編集しない。

## 5. マイルストーン

## M1: 受理/拒否matrixとconsumerを固定

- 変更内容:
  - 全workloadについてCLI/env、clock/render、size/population、instrumentation、必須artifactを表にする。
  - Rust config testsとPython selftestの不足組合せを補い、現在の正常/拒否結果を固定する。
  - OS入力取得から検証を分離し、perf無効/feature不足、CLI優先、paired key、重複flag、欠損値を新しい入口のtestで固定する。既存helperのpassを入口全体の保証にしない。
- 変更ファイル: R14-A: config.rs、config/parse.rs、config/tests.rs、scripts/perf_tool/selftest.py。OS入力とpure検証の分離を先に行う。
- 完了条件:
  - [x] Rust単発とPython matrixの責務差が明記され、新旧比較ができる。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M2: Rustの検証済み型を導入

- 変更内容:
  - raw parserを残して共通設定＋workload payloadへ変換する。
  - consumerを一群ずつ移行し、旧Option/accessorを使用がなくなった段階で撤去する。
  - 通常main/startup→fixture/run condition→出力の順に移行し、disabled configが通常window・RtT quality・乱数streamへ与える意味と既存schemaを保つ。
- 変更ファイル: R14-B: 新規config/validated.rs（案）、config.rs、main.rs、startup_systems.rs、perf_scenario配下consumer/output。定義→accessor→consumer→旧field撤去。
- 完了条件:
  - [x] 検証前の矛盾設定がruntimeへ渡らず既存CLI/env互換が成立。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M3: Pythonのworkload validationを整理

- 変更内容:
  - argument validatorとartifact dispatchをworkload単位へ移し、既存readerを再利用する。
  - self-testでschema/hash、source drift、missing artifact等の拒否を確認する。
- 変更ファイル: R14-C: arguments.py、新規argument_validation/（案）、artifacts.py、selftest.py。公開CLI入口・Case/schemaを保持する。
- 完了条件:
  - [x] 独立verifierの拒否能力とexperiment制約が変わらない。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M4: 代表経路とlauncher境界を受入

- 変更内容:
  - headlessの正しさ経路、actual-window Capture、Memoryが必要な代表recipeを選び、その選定理由を記録する。
  - Capture→Memoryを持つrecipeでは順序/二つのbuild契約/原本照合を実行し、別のworkloadはmatrix testで網羅する。
- 変更ファイル: R14-D: 既存native helperとcoordinatorの検証入力。M1〜M3で変えた入口に対応するfixture不足だけ補う。
- 完了条件:
  - [x] 変更した入口の実行証拠があり、未実行workloadをnative passとして報告しない。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M5: 仕様・Help・品質ゲートと終了整理

- 変更内容:
  - 下記仕様へ最終的な所有者・順序・失敗条件を反映する。計画の検討経緯をそのまま恒久仕様へ転記しない。
  - [Help影響Skill](../../../../.cursor/skills/hell-workers-review-help-impact/SKILL.md)で全実装差分のplayer-visible経路を確認する。
  - 開発用runnerの構造整理で通常playerの操作/設定/表示経路が不変か確認してNo impactを判断する。
  - 受入と品質ゲートの結果を記録し、専用出力を整理する。
- 仕様更新対象（該当する契約を変更した文書だけを更新）:
  - [docs/performance-profiling.md](../../../../docs/performance-profiling.md)
  - [docs/DEVELOPMENT.md](../../../../docs/DEVELOPMENT.md)
  - [docs/development-infra/validation-storage-workflow.md](../../../../docs/development-infra/validation-storage-workflow.md)
- 完了条件:
  - [x] §7のケースと必要なnative受入が完了し、未検証範囲を明記した。
  - [x] HelpがUpdate required / No impactのいずれかに確定し、対応する更新または根拠を記録した。
  - [x] `check`・Clippy・`verify`が成功。workspace RA診断は上記MCP制約を記録。
  - [x] primary validation storage checkが成功し、不要出力の整理/継続用途を記録した。
- 検証: §7の完了gate。品質gateのpassだけをHelpの意味レビューやnative受入の代替にしない。

## 6. リスクと対策

| リスク | 対策 |
| --- | --- |
| 型統合でRust/Pythonの異なる責務を潰す | 単発とexperiment matrixの表を分ける。 |
| 共通dispatchでfail-openになる | 欠損/不一致artifactの拒否testを保持する。 |
| 稼働中のverifier依存を変更する | active batchを確認し、未sealのsource/helperを凍結したままにする。 |
| 共有ファイルの並行変更を上書きする | 作業開始/各取り込み前にstatusと対象diffを確認し、変更所有を確認する。 |
| 過去のpassを新subjectへ流用する | 変更後の入力を固定して必要な検査を実施し、未実施は未実施と記録する。 |

## 7. 検証計画

### ケースと判定条件

| ケース | 期待する結果 |
| --- | --- |
| 全workloadの正常/拒否CLI・env | precedence/default/必須組合せの互換維持。 |
| perf無効・profiling featureなし | 無効なら不正なperf専用引数があっても既存の早期Default。有効かつfeature不足なら起動前拒否。通常window/RtT設定は上書きしない。 |
| CLI/env競合・paired key・重複flag・値欠損 | 一般のCLI優先とWall等の両方一致必須を区別。重複値はRustの最初の一致flag、Python各parserの既存結果を別々に固定し、欠損時の拒否を維持。 |
| unknown flag | Rust/Pythonそれぞれの既存の探索/受理/拒否挙動を維持。 |
| 現行で拒否するclock/render/instrumentationの組合せ | 起動前の拒否と主要なerror理由を維持。 |
| save-transaction | runtime root隔離とcanonical binary/回数/規模契約を維持。 |
| source drift/元artifact欠損/異なるhash | 独立verifierが失敗し、部分成功をpassにしない。 |
| Capture→Memory・busy・storage | 逐次実行、admission、元依存保全とseal/finalizeが維持。 |

### 回帰fixtureの作り方（新規test案）

| ID / test案 | 入力・操作 | 必須assert |
| --- | --- | --- |
| R14-T1 `disabled_config_ignores_perf_only_input_test` | perf有効化なし＋不正workload/欠損値を明示inputへ渡す。 | Default相当、window/quality overrideなし、seed provider呼出し0。 |
| R14-T2 `perf_input_precedence_and_paired_keys_test` | CLI/env競合、同flag重複、Wallの片側指定/不一致をtable化。 | Rustの最初の値、一般CLI優先、paired key拒否を区別。Pythonは独自の既存parser結果をassert。 |
| R14-T3 `config_feature_boundary_test` | 同じ入力をdefault/profiling/profiling-renderdocのtest実行へ渡す。不正workload＋RenderDoc要求、seed未指定＋RenderDoc feature不足も分ける。 | profilingなしは入口拒否。profilingありではworkloadエラーがRenderDoc拒否より先。RenderDoc拒否まで到達した場合のseed provider呼出し1。有効featureでは既存正常例を受理。 |
| R14-T4 `workload_config_projection_is_compatible_test` | 11 workloadそれぞれの正常例をparseし、consumerが読む値/文字列へprojectionする。 | 型の形でなく、seed/population/clock/window/phase/出力値が固定期待値と一致。 |
| R14-T5 Python normalization fixture | deconstruction、behavior、field-coreのNamespaceをvalidationへ2回通す。 | allowanceとcase順序が同じ、二重追加なし。拒否ケースを新しい汎用fallbackへ通さない。 |
| R14-T6 artifact refusal fixture | 正常sidecarから必須file/列を1つ除去、hash/sourceを不一致へ変更。 | 各変更が独立に拒否される。dispatch整理による未検証passがない。 |

R14-T1〜T4は既存`config/tests.rs`、T5〜T6は既存`selftest.py`の入口から実行する。未指定seedはfake providerの呼出し数と返した値をassertし、ランダムな実値をgoldenにしない。

### focused command（実装時に実行）

```bash
python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 perf_scenario::config
python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 --no-default-features --features profiling perf_scenario::config
python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 --no-default-features --features profiling-renderdoc perf_scenario::config
python3 scripts/perf.py self-test
python3 -m unittest scripts.tests.test_validation_storage scripts.tests.test_cargo_runtime
```

test filterは実装時に実際の出力件数を確認し、0件実行を成功根拠にしない。
既存filterに入らない新testは正確なmodule/test名をここへ追加する。
既定featureのconfig testだけではprofiling有効経路を証明しない。上記feature別testは逐次実行し、Memory/Tracyのcompileは既存verify、native入口はM4で確認する。

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

runnerの入口変更なのでnative Skillの既存recipeをprimary coordinatorから実行して代表経路を確認する。M1で変更consumerに対応した最小のrecipe集合を確定し、Capture/Memoryがあるものは既定の逐次順を守る。headlessをrenderer証拠にせず、全履歴matrixの再実行はしない。

実機確認を実施する場合は[Native Acceptance Skill](../../../../.cursor/skills/hell-workers-run-native-acceptance/SKILL.md)を読み直す。
primary `dev.py validation plan --spec ... -- <既存helperのplan>`へ登録し、返されたno-prompt kitty launcherだけを使う。
fixtureが対象を覆わなければ先に専用caseと独立verifierを用意する。汎用smokeの成功を個別要件の証明にしない。
Capture/Memoryを含むrecipeは逐次実行し、read-only observer・source/asset/binary一致・timeoutを検証する。
通常の修正feedbackでは同じdev cacheを再利用し、headless・feedback・正式受入・性能結果を区別する。

### 検証データ管理（完了記録）

正本は[検証データ管理](../../../development-infra/validation-storage-workflow.md)。2026-09-19に全batchの独立verifyまたは失敗理由をsealし、不要jobを削除してfinalize／storage checkを完了した。

| 対象 | 結果・整理 |
| --- | --- |
| Task Dashboard | `target/native-acceptance/task-dashboard-20260918T173947Z-7272ff52`（a）は2,015,232→0、`target/native-acceptance/task-dashboard-20260918T181444Z-1ffd845c`（b）は3,145,728→0 allocated bytes。独立verify／seal／finalize済み |
| P08・限定診断 | R13と同じcで受入済み。job・binary capsule・RDC・一時診断はすべて整理し、具体的な削除pathと容量は[親提案](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md)とR13の終了記録へ集約 |
| 計測上の容量 | 各job削除直後のdf空き差は0 bytes（Btrfs）。duのallocated bytesと共有extentの空き増加は同一視しない |
| 最終候補 | `/home/satotakumi/projects/hell-workers-validation/refactor-r01-r14-92a7d87235e6`、clean subject `a0487a35c09ed444dc588902ed198c9532f4c685`、40,952,061,952 allocated bytes |
| 残る具体的用途 | owner `refactor-r01-r14`、consumer `refactor-r01-r14-review`。2026-09-19の実装提示に対する確認・修正で同じcandidateとtargetを再利用する。実装consumerは解除し、review consumerへ引継ぎ済み |
| release_when | 提示したR01〜R14のレビューと必要な修正が承認または明示終了し、利用者がなくなった時。R04/R08/R12の入力確認にはprimary dev cacheを使用する |
| 正本 | code・assets・仕様はprimary。primary HEAD/indexは変更していない。通常Cargo cacheは別の保守寿命で維持 |

## 8. ロールバック方針

Rust型移行とPython module移行、各workloadを別の変更単位にする。外部CLI/schemaは維持し、問題のworkloadだけ旧入口へ戻せる形にする。

戻す前に直近5commit、対象のHEAD差分、他sessionの所有を確認する。
広範囲のcheckout/cleanや他計画の変更破棄を行わず、レビュー可能な限定逆patchを作る。
保存型・asset formatを変える必要が生じた場合は、互換/復旧条件をこの計画へ追記してから変更する。

## 9. AI引継ぎメモ（完了）

11 workloadの検証済みpayload、注入可能なparse入力、workload別Python validator／sidecar dispatchを実装。代表native受入・品質gate・終了整理を完了。

Task Dashboard aはfixtureが管理workspaceを開いておらずvisible／filter counterが0となった。実setup経路を修正し3 mode回帰を加え、bの全21 processで受け入れた。P08の検証器と集約の不具合も修正後のcで確認した。

| 検証 | 最終結果 |
| --- | --- |
| focused回帰 | 11 workloadのseed・人数・clock・window・phase・出力固定projection、featureごとの拒否順序・seed呼出し数を補完。profiling-renderdocのconfig tests25件、通常／profiling workspace tests、Pythonの3 normalization冪等性を含むself-testが成功。 |
| 全体gate | 最終変更後の`dev.py verify`と`dev.py check`が成功。Python 200件、Blender 151件、通常／profiling workspace tests、feature別compile、通常／profiling Clippy警告0 |
| rust-analyzer | workspace診断はMCPのnull応答で取得不可。compile／tests／Clippyで代替確認し、RAの診断取得成功とは報告しない |
| native | Task Dashboard bはhidden／visible／active-filterの固定audit各1、Capture／Memory各3の計21 processが成功。P08 cはCapture／RenderDoc・cross-consumer一致と独立verifyが成功。全workloadのnative性能比較を行ったという意味ではない。 Intel Arc Graphics (MTL)／Vulkan／X11／Mesa 26.1.8 |
| Help | 本計画自体はNo impact。通常の操作・表示の意味・入力条件を維持する。全14件ではMove失敗時の保持・タスク表示・建設工程の3 entryを更新し、exact snapshotをレビュー・検証済み |
| 保存整理 | 全jobをseal／整理／finalize済み。storage check成功、candidate/cacheだけreview-activeとして上記consumerへ引継ぎ |
| 対象外・未検証 | 歴史的baseline再構築・全性能／DPI matrixは対象外。R04/R08/R12のOS許可待ち実入力受入は別の現行計画で管理 |

P08 cのsource fingerprintは`2f3cc42ecb89c75ef0785a96a3e4bfb478733dfed6d72dc30956cad4ca4e8ee3`、harnessは`a017d1a25a78c3189ae515fa094eed09a41bd68176ae03e5a7be324dba3a7b1f`。
CPU/GPU/Soul/Roomのepoch 0／revision 1、Scene 1920×1080、Light Fieldの期待pixelと22件の実bindingが一致した。
詳細な受入結果は[描画仕様](../../../rendering-performance.md)と[性能計測仕様](../../../performance-profiling.md)を正本とする。

### Definition of Done

- [x] milestoneと必要な回帰／native受入を完了
- [x] 仕様・Helpの意味判断と対応を完了
- [x] check・Clippy警告0・verifyに成功し、RA診断取得の制約を記録
- [x] 不要job／capsule／診断スクリプトを整理しstorage checkに成功
- [x] 実装consumerを解除し、最終candidate/cacheを明示review consumerへ引継ぎ
- [x] 具体的な削除path・実測bytes・保持用途を記録
- [x] 計画をarchiveし、両索引を再生成

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-09-18 | Codex | R14の独立計画をテンプレートから作成。依存、milestone、受入、Help、検証データ管理を具体化 |
| 2026-09-18 | Codex | 計画レビュー: 通常起動の早期Default、入力を注入できる検証入口、validated型の変更制限、feature別回帰を補足。 |
| 2026-09-18 | Codex | 全計画ブラッシュアップ: config.rs::try_from_processとarguments.py::validate_argumentsを入口表にし、R14-T1/T2/T5の現行期待値を固定する。 実装は未着手。 |
| 2026-09-19 | Codex | 最終回帰・全体gate・P08 cの独立受入を完了。job整理とreview cacheの引継ぎを記録しarchive。 |
