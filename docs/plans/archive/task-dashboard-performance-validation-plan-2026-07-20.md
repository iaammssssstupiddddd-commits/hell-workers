# タスクダッシュボード性能検証フォローアップ 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `task-dashboard-performance-validation-plan-2026-07-20` |
| ステータス | `Completed` |
| 作成日 | `2026-07-20` |
| 最終更新日 | `2026-08-02` |
| 作成者 | `Codex` |
| 関連提案 | `docs/proposals/gameplay-management-improvements-proposal-2026-07-17.md`（Track A3 性能フォローアップ） |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: A3で未整備のdashboard mode別AI work counterと実renderer / allocator計測を、再現可能なperf harnessへ載せる
  - A3 は UI から候補評価や A* を呼ばない責務境界と、latest-only / fixed-width の有界性を実装・自動検証したが、
    dashboard hidden / visible / active-filter を同一 fixture で比較する専用 perf mode と全 counter は未整備である。
  - 旧`summary.csv` schema v10には source selector、connectivity、runtime A* はある一方、candidate snapshot / score、
    Top-K、wheelbarrow arbitration rebuild / bucket build の回数がなかった。
  - 実 renderer の frame-time と allocator の計測は、手操作ではなく既存 `scripts/perf.py` の有効性契約へ載せる必要がある。
- 到達したい状態:
  - 同一 seed / fixture / fixed tick で dashboard mode だけを変え、AI work counter が完全一致することを自動判定できる。
  - Capture / Memoryを混同せず、実frame-time、UI system CPU、allocation / peak memoryを再現可能に採取できる。
    Tracyは任意のzone cross-checkに限定する。
  - A3 の機能仕様や production AI 挙動を変更せず、計測基盤だけを独立して導入・撤去できる。
- 成功指標:
  - hidden / visible / active-filter の初期 fixture checksum と fixed-step audit checksum が一致する。
  - candidate / source / connectivity / arbitration / runtime A* の全 work counter が mode 間で一致する。
  - `scripts/perf.py` が schema、case identity、反復、adapter、ログ健全性を検証し、手作業の数値転記を必要としない。

## 2. スコープ

### 対象（In Scope）

- `task-dashboard` 専用 workload と `hidden` / `visible` / `active-filter` mode。
- mode を case identity、manifest / matrix、`summary.csv`、aggregate 契約へ含める schema 更新。
- 同一 run session 内の3 modeだけを比較する dashboard 専用レポート。既存の汎用 baseline/candidate 比較は
  `dashboard_mode` が異なる case を引き続き拒否する。
- profiling feature 限定の次の累積 counter:
  - candidate snapshot / filter / score attempt
  - Top-K 対象数
  - source selector call / scanned item（既存）
  - `reachable_with_cache_calls`（既存）
  - wheelbarrow arbitration rebuild / request bucket build / candidate scan
  - caller 別 runtime A* / deferred（既存）
- fixed-step audit による mode 間の simulation / AI work 同一性検証。
- Capture、任意Tracy、Memory の分離採取と `docs/performance-profiling.md` の契約同期。

### 非対象（Out of Scope）

- A3 の blocker 分類、filter / sort、priority / cancellation 仕様の再設計。
- UI を開いたときだけ診断 producer を実行する最適化。
- 異なる schema、fixture、workload、GPU / backend の数値比較。
- 性能目標を満たすための最適化実装。回帰が見つかった場合は別の修正計画へ切り出す。
- raw artifact のコミット。

## 3. 現状とギャップ

- `PerfWorkload` は `gather` / `path-door` / `construction` / `ui-gpu` の4種で、dashboard mode を持たない。
- `summary.csv` schema v11はsource selector、connectivity、runtime pathに加え、candidate / Top-K / arbitration / dashboardの
  作業量を同一artifactで比較できる。
- A3 の unit / headless integration test は状態、操作、reset、latest-only map の有界性を保証済みである。
- したがって本計画は機能 correctness を再試験するのではなく、mode 間の作業量同一性と実コストを計測可能にする。

## 4. 実装方針（高レベル）

- `PerfDashboardMode` を明示的な case dimension とし、UI state を fixture setup 時に決定する。
- hidden / visible / active-filter は task / Soul / Familiar / seed を変えない。active-filter も同じ row 集合を入力にし、
  `TaskDashboardViewState` だけを変更する。
- counter は既存 hot path の branch に profiling feature 限定の整数加算だけを置き、通常 build の型・Query・分岐を増やさない。
- fixed-step audit で work counter equality を先に受け入れ、実時間 Capture / 任意Tracy / Memory は別 run にする。
- schema を更新するときは Rust writer、Python expected columns、fixture、aggregate、文書を同じ変更単位で更新する。
- mode 間の correctness / cost 比較は、同一 session の matrix から作る
  fixed auditは`dashboard_mode_comparison.json`、実時間costは`dashboard_mode_cost_comparison.json`を正本にする。通常の最適化前後を比べる汎用 `compare` の
  case identity 制約は緩めない。
- Bevy 0.19 の UI visibility / interaction state は既存 production code の設定経路を再利用し、perf 専用 UI 実装を作らない。

## 5. マイルストーン

## M1: mode・counter・schema 契約の固定

### 計測契約

| 区分 | counter / dimension | 増加地点 | reset / snapshot 境界 | mode 間の判定 |
| --- | --- | --- | --- | --- |
| case | `dashboard_mode` | fixture の UI setup | run ごと | `hidden` / `visible` / `active-filter` を必須記録 |
| candidate | `candidate_membership_checks` | policy gate 到達 | realtime measure 開始で reset、fixed checkpoint で累積 snapshot | 完全一致、正値 |
| candidate | `candidate_snapshot_attempts` | policy 通過後の semantic filter / snapshot 試行 | 同上 | 完全一致、正値 |
| candidate | `candidate_score_attempts` / `worker_score_attempts` | base score / worker score 合成 | 同上 | 完全一致、正値 |
| Top-K | `top_k_partition_runs` / `top_k_retained_candidates` / `top_k_fallback_candidates` | worker 候補の partition | 同上 | 完全一致、partition / retained は正値 |
| source | `source_selector_calls` / cache-build scan / candidate scan | Haul source selector | 同上 | 完全一致 |
| connectivity | `reachable_with_cache_calls` | assignment 前の連結性判定 | 同上 | 完全一致、正値 |
| arbitration | rebuild / request bucket build / bucket item scan / retained after Top-K | wheelbarrow arbitration の実 rebuild | 同上 | 完全一致、rebuild は正値 |
| runtime A* | caller 別 core search / deferred と expanded / defer | runtime path budget / path metrics | 同上 | 完全一致 |
| dashboard producer | state rebuild / snapshot row / summary row | production view-model adapter | 同上 | 3 mode 完全一致 |
| dashboard render | rebuild / input row / visible row / despawn root | production TaskList renderer | 同上 | hidden は 0、visible と active-filter は同じ input、active-filter の visible row は少ない |

`candidate_snapshot_attempts` は `candidate_snapshot` が担う semantic filter の試行数であり、別名の
`candidate_filter_attempts` は設けない。同じ branch を二重計上せず、membership / snapshot / score の
境界を明示する。既存 `TransportRequestMetrics` は最新フレームの gauge なので監査には使わず、
profiling 専用の累積 arbitration resource を正本にする。

- 変更内容:
  - dashboard mode と必要 counter の名前、増加地点、reset / snapshot 境界を表にする。
  - case identity と schema version 更新を先にテストへ固定する。
  - `hidden` / `visible` / `active-filter` を1 sessionで走らせ、mode間の checksum / counter equality と
    cost差を出力する専用比較契約を固定する。
- 変更ファイル:
  - `crates/bevy_app/src/plugins/startup/perf_scenario/config.rs`
  - `crates/bevy_app/src/plugins/startup/perf_scenario/output.rs`
  - `scripts/perf_tool/model.py`
  - `scripts/perf_tool/arguments.py`
  - `scripts/tests/`
- 完了条件:
  - [x] 3 mode が case identity と artifact に必ず記録される。
  - [x] Rust / Python の schema column 集合が一致する。
  - [x] counter の同義重複や runtime A* との誤った代理関係がない。
  - [x] 汎用 baseline/candidate 比較は dashboard mode 不一致を拒否し、専用比較だけが3 modeを横断する。
- 検証:
  - `python3 scripts/perf.py self-test`
  - `python3 -m unittest discover -s scripts/tests -p 'test_*.py'`

## M2: deterministic fixture と work counter equality

- 変更内容:
  - `task-dashboard` fixture と3 modeを実装する。
  - candidate / Top-K / arbitration の不足 counter を profiling feature に追加する。
  - 同一 fixed tick の mode 間で checksum と全 AI work counter の完全一致を検証する。
- 変更ファイル:
  - `crates/bevy_app/src/plugins/startup/perf_scenario/fixture.rs`
  - `crates/bevy_app/src/plugins/startup/perf_scenario/capture_driver.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/`
  - `crates/hw_logistics/src/transport_request/arbitration/`
- 完了条件:
  - [x] fixture の task / Soul / Familiar 数と初期 checksum が3 modeで一致する。
  - [x] candidate / source / connectivity / arbitration / runtime A* counter が3 modeで一致する。
  - [x] dashboard mode が AI system parameter、producer gate、timerへ入らない。
- 検証:
  - `cargo test -p bevy_app@0.1.0 perf_scenario`
  - `cargo test -p hw_familiar_ai task_management`
  - `cargo test -p hw_logistics wheelbarrow_arbitration`
  - `cargo check -p bevy_app@0.1.0 --lib --no-default-features --features profiling`

## M3: 実時間・system CPU・memoryの分離採取

- 変更内容:
  - Captureでframe-timeとmeasure同期済みUI system CPU、Memoryでallocator / peak memoryを採取する。
    Tracyはsocketを利用できる環境での任意cross-checkとし、完了条件へ含めない。
  - 同一 session 内の mode 間比較コマンドと有効性判定を文書化する。
- 変更ファイル:
  - `scripts/perf_tool/`
  - `docs/performance-profiling.md`
- 完了条件:
  - [x] 各 mode 3 valid run の同一 matrix 比較が成立する。
  - [x] Capture / Tracy / Memory の値を同じ baseline として混在させない。
  - [x] 失格 run を黙って除外せず、理由を artifact に残す。
- 検証:
  - `python3 scripts/perf.py run --workload task-dashboard --dashboard-modes hidden,visible,active-filter ...`
  - `python3 scripts/perf.py compare-dashboard-modes --session <run-dir>`
  - `python3 scripts/dev.py verify`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| mode ごとに fixture が変わる | UI以外の差を性能差と誤認する | initial checksum と fixed-step checksum を必須一致にする |
| counter 自体が hot path を歪める | 通常buildの性能を悪化させる | profiling feature限定の整数counterにする |
| schemaだけ片側で更新する | runnerが誤集約する | Rust writer / Python validator / fixtureを同一変更にする |
| UI frame-timeとAI作業量を混同する | 原因を誤診する | work counter、Capture、任意Tracy、Memoryを別の判定軸にする |
| 過去artifactと比較する | schema欠落を0と誤認する | 同一schema・fixture・matrix以外は履歴参考値に限定する |

## 7. 検証計画

- 必須:
  - 3 mode の初期 fixture / fixed-step checksum 一致
  - 全 AI work counter の完全一致
  - Rust / Python schema self-test
  - profiling feature check、workspace clippy / test
- 実機:
  - Capture 3反復でframe-timeとnative UI system CPU
  - Memory 3反復でmeasure区間allocation / live peakとprocess peak RSS
  - Tracyは任意cross-checkであり、正式値へ混ぜない
- 計画完了時:
  - `python3 scripts/dev.py docs --write`
  - `python3 scripts/dev.py verify`
  - `git diff --check`

## 8. ロールバック方針

- workload / mode、schema、counterを同じ変更単位で戻す。
- A3 production機能と恒久UI仕様は本計画のrollback対象にしない。
- schemaを戻す場合はRust writerとPython validatorを同時に戻し、互換しないartifactを混在させない。

## 8.1 完了artifact（raw、commit対象外）

| 判定軸 | artifact | 結果 |
| --- | --- | --- |
| fixed-step correctness | `/tmp/hw-task-dashboard-audit-20260802-headless-v5` | 3 mode各1 run、3 valid / 0 invalid、`dashboard_mode_comparison.json` PASS |
| 実renderer Capture | `/tmp/hw-task-dashboard-capture-20260802-x11-native-v2` | Intel Vulkan / X11、各mode 3 run、9 valid / 0 invalid、cost comparison PASS |
| native Memory | `/tmp/hw-task-dashboard-memory-20260802-x11-native-v2` | Intel Vulkan / X11、各mode 3 run、9 valid / 0 invalid、cost comparison PASS、全run accounting error 0・収支一致 |

fixed auditのpost-warmupでは、AI側counterが3 modeで完全一致した。membership `1320`、snapshot `1320`、
score `1264`、worker score `195`、Top-K partition / retained `3 / 72`、connectivity `3`、
wheelbarrow rebuild / bucket / scanned / retained `129 / 129 / 129 / 129`、runtime core A* `53`である。
hiddenのdashboard renderは0、visibleは入力 / 表示`661 / 661`、active-filterは`661 / 320`だった。

Capture中央値はhidden / visible / active-filterのp50が`18.729823 / 19.043225 / 19.117661 ms`、
Task Dashboard CPUが`266.5 / 6259.5 / 4096.2 ns/invocation`だった。Memory中央値はallocated bytes/frameが
`3,531,955 / 4,971,800 / 3,738,212`、process peak RSSが`1,334,428 / 1,316,132 / 1,358,060 KiB`だった。
Memory sessionのframe quantileはallocator計数の擾乱を含むため、比較結果と受入値に使わない。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`
- 完了済みマイルストーン: `M1`〜`M3`
- 未着手: なし

### 次のAIが最初にやること

1. 新しいdashboard最適化を行う場合は、同じschema / fixture / matrixで新baselineを採る。
2. Memoryのframe-timeを性能回帰値へ流用しない。
3. raw artifactをcommitしない。

### ブロッカー/注意点

- A3は機能として完了済み。本計画を理由にA3のpriority / cancellation / blocker仕様を変更しない。
- 実時間baselineとfixed-step auditを混ぜない。
- counterがない値を0として比較しない。
- raw artifactは`target/perf-runs/`外へ書かず、commitしない。

### 参照必須ファイル

- `docs/performance-profiling.md`
- `docs/task_list_ui.md`
- `docs/plans/archive/actionable-task-dashboard-plan-2026-07-19.md`
- `crates/bevy_app/src/plugins/startup/perf_scenario/`
- `scripts/perf_tool/`
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/`
- `crates/hw_logistics/src/transport_request/arbitration/`

### 最終確認ログ

- 最終 `cargo check --workspace`: `python3 scripts/dev.py verify`で実施
- 最終 `cargo clippy --workspace --all-targets -- -D warnings`: `python3 scripts/dev.py verify`で実施
- 最終 `cargo test --workspace`: `python3 scripts/dev.py verify`で実施
- 未解決エラー: なし

### Definition of Done

- [x] M1〜M3が完了
- [x] 3 modeのfixed-step checksumとAI work counterが一致
- [x] Capture / Memoryの有効なartifactと比較結果があり、Tracyを任意cross-checkへ限定
- [x] `docs/performance-profiling.md`が新schemaと正式手順に同期
- [x] `python3 scripts/dev.py verify`が成功
- [x] 完了後に本計画をarchive

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-07-20` | `Codex` | A3クローズ時にT11/R03を独立移管。dashboard mode、AI work counter、実renderer/allocator計測の境界を定義 |
| `2026-08-02` | `Codex` | B2クローズ依頼を受けてA3残件を巻き取り、dashboard mode別harnessとartifact採取へ着手 |
| `2026-08-02` | `Codex` | schema v11 / determinism v4、3 mode fixed audit、Intel Vulkan/X11 Capture・native Memory各9-run比較を完了。Tracy socket依存をMemoryから除外し、計画をarchive |
