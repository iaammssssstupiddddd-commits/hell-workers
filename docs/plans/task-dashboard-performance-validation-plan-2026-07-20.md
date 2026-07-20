# タスクダッシュボード性能検証フォローアップ 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `task-dashboard-performance-validation-plan-2026-07-20` |
| ステータス | `Draft` |
| 作成日 | `2026-07-20` |
| 最終更新日 | `2026-07-20` |
| 作成者 | `Codex` |
| 関連提案 | `docs/proposals/gameplay-management-improvements-proposal-2026-07-17.md`（Track A3 性能フォローアップ） |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: A3で未整備のdashboard mode別AI work counterと実renderer / allocator計測を、再現可能なperf harnessへ載せる
  - A3 は UI から候補評価や A* を呼ばない責務境界と、latest-only / fixed-width の有界性を実装・自動検証したが、
    dashboard hidden / visible / active-filter を同一 fixture で比較する専用 perf mode と全 counter は未整備である。
  - `summary.csv` schema v10 には source selector、connectivity、runtime A* はある一方、candidate snapshot / score、
    Top-K、wheelbarrow arbitration rebuild / bucket build の回数がない。
  - 実 renderer の frame-time と allocator の計測は、手操作ではなく既存 `scripts/perf.py` の有効性契約へ載せる必要がある。
- 到達したい状態:
  - 同一 seed / fixture / fixed tick で dashboard mode だけを変え、AI work counter が完全一致することを自動判定できる。
  - Capture / Tracy / Memory を混同せず、実 frame-time、UI system CPU、allocation / peak memory を再現可能に採取できる。
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
- Capture、Tracy、Memory の分離採取と `docs/performance-profiling.md` の契約同期。

### 非対象（Out of Scope）

- A3 の blocker 分類、filter / sort、priority / cancellation 仕様の再設計。
- UI を開いたときだけ診断 producer を実行する最適化。
- 異なる schema、fixture、workload、GPU / backend の数値比較。
- 性能目標を満たすための最適化実装。回帰が見つかった場合は別の修正計画へ切り出す。
- raw artifact のコミット。

## 3. 現状とギャップ

- `PerfWorkload` は `gather` / `path-door` / `construction` / `ui-gpu` の4種で、dashboard mode を持たない。
- `summary.csv` schema v10 は source selector、connectivity、runtime path の counter を持つが、A3 が追加した
  candidate / arbitration の全作業量を比較できない。
- A3 の unit / headless integration test は状態、操作、reset、latest-only map の有界性を保証済みである。
- したがって本計画は機能 correctness を再試験するのではなく、mode 間の作業量同一性と実コストを計測可能にする。

## 4. 実装方針（高レベル）

- `PerfDashboardMode` を明示的な case dimension とし、UI state を fixture setup 時に決定する。
- hidden / visible / active-filter は task / Soul / Familiar / seed を変えない。active-filter も同じ row 集合を入力にし、
  `TaskDashboardViewState` だけを変更する。
- counter は既存 hot path の branch に profiling feature 限定の整数加算だけを置き、通常 build の型・Query・分岐を増やさない。
- fixed-step audit で work counter equality を先に受け入れ、実時間 Capture / Tracy / Memory は別 run にする。
- schema を更新するときは Rust writer、Python expected columns、fixture、aggregate、文書を同じ変更単位で更新する。
- mode 間の correctness / cost 比較は、同一 session の matrix から作る
  `dashboard_mode_comparison.json` を正本にする。通常の最適化前後を比べる汎用 `compare` の
  case identity 制約は緩めない。
- Bevy 0.19 の UI visibility / interaction state は既存 production code の設定経路を再利用し、perf 専用 UI 実装を作らない。

## 5. マイルストーン

## M1: mode・counter・schema 契約の固定

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
  - [ ] 3 mode が case identity と artifact に必ず記録される。
  - [ ] Rust / Python の schema column 集合が一致する。
  - [ ] counter の同義重複や runtime A* との誤った代理関係がない。
  - [ ] 汎用 baseline/candidate 比較は dashboard mode 不一致を拒否し、専用比較だけが3 modeを横断する。
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
  - [ ] fixture の task / Soul / Familiar 数と初期 checksum が3 modeで一致する。
  - [ ] candidate / source / connectivity / arbitration / runtime A* counter が3 modeで一致する。
  - [ ] dashboard mode が AI system parameter、producer gate、timerへ入らない。
- 検証:
  - `cargo test -p bevy_app@0.1.0 perf_scenario`
  - `cargo test -p hw_familiar_ai task_management`
  - `cargo test -p hw_logistics wheelbarrow_arbitration`
  - `cargo check -p bevy_app@0.1.0 --lib --no-default-features --features profiling`

## M3: 実時間・system CPU・memoryの分離採取

- 変更内容:
  - Capture で frame-time、Tracy で UI system CPU、Memory で allocation / peak memory を採取する。
  - 同一 session 内の mode 間比較コマンドと有効性判定を文書化する。
- 変更ファイル:
  - `scripts/perf_tool/`
  - `docs/performance-profiling.md`
- 完了条件:
  - [ ] 各 mode 3 valid run の同一 matrix 比較が成立する。
  - [ ] Capture / Tracy / Memory の値を同じ baseline として混在させない。
  - [ ] 失格 run を黙って除外せず、理由を artifact に残す。
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
| UI frame-timeとAI作業量を混同する | 原因を誤診する | work counter、Capture、Tracy、Memoryを別の判定軸にする |
| 過去artifactと比較する | schema欠落を0と誤認する | 同一schema・fixture・matrix以外は履歴参考値に限定する |

## 7. 検証計画

- 必須:
  - 3 mode の初期 fixture / fixed-step checksum 一致
  - 全 AI work counter の完全一致
  - Rust / Python schema self-test
  - profiling feature check、workspace clippy / test
- 実機:
  - Capture 3反復で frame-time
  - Tracy別runでUI system CPU
  - Memory別runでallocation / peak memory
- 計画完了時:
  - `python3 scripts/dev.py docs --write`
  - `python3 scripts/dev.py verify`
  - `git diff --check`

## 8. ロールバック方針

- workload / mode、schema、counterを同じ変更単位で戻す。
- A3 production機能と恒久UI仕様は本計画のrollback対象にしない。
- schemaを戻す場合はRust writerとPython validatorを同時に戻し、互換しないartifactを混在させない。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手: `M1`〜`M3`

### 次のAIが最初にやること

1. `PerfScenarioConfig`、`output.rs`、`scripts/perf_tool/model.py` の現行schema v10を照合する。
2. modeを新workload名へ埋め込まず、比較可能な明示dimensionとして設計する。
3. counter追加前に3 modeのfixture identity testを作る。

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

- 最終 `cargo check --workspace`: 未実施
- 最終 `cargo clippy --workspace --all-targets -- -D warnings`: 未実施
- 最終 `cargo test --workspace`: 未実施
- 未解決エラー: なし（未着手）

### Definition of Done

- [ ] M1〜M3が完了
- [ ] 3 modeのfixed-step checksumとAI work counterが一致
- [ ] Capture / Tracy / Memoryの有効なartifactと比較結果がある
- [ ] `docs/performance-profiling.md`が新schemaと正式手順に同期
- [ ] `python3 scripts/dev.py verify`が成功
- [ ] 完了後に本計画をarchive

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-07-20` | `Codex` | A3クローズ時にT11/R03を独立移管。dashboard mode、AI work counter、実renderer/allocator計測の境界を定義 |
