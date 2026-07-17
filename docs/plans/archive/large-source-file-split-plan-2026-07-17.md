# 500行以上の実装ファイル分割計画

## メタ情報

- ステータス: Completed（M0〜M8完了）
- 作成日: 2026-07-17
- 最終更新日: 2026-07-17
- 対象: `crates/` 配下のRust実装と `scripts/perf.py`
- 種別: 構造リファクタリング
- 前提: Bevy 0.19 / Rust 2024

## 目的

2026-07-17時点で500行以上あるソースファイルを、公開API、Bevyのsystem登録、
実行順、乱数消費順、save/load契約を変えずに責務単位へ分割する。

単に行数を移すのではなく、次を達成する。

- 変更理由の異なる責務を別モジュールにする。
- 既存の公開パスとplugin wiringを維持する。
- 大きなインラインテストを実装本体から分離する。
- 1ファイル499行以下を必須条件とし、原則450行以下の余白を確保する。
- 各段階を独立して検証・レビュー・revertできる変更単位にする。

## 対象範囲

### 含むもの

- `crates/` 配下の500行以上のRustファイル18件
- 500行以上の運用スクリプト `scripts/perf.py` 1件
- 分割に必要な同一crate内の子モジュール追加
- 分割前後の挙動を固定する回帰テスト
- 参照先が物理ファイルを直接指す場合のドキュメント更新

### 含まないもの

- crate境界の変更、新規crateの追加
- gameplay仕様、save schema、profiling出力形式の変更
- Bevy systemを複数systemへ分解する並列化・スケジューリング変更
- 性能最適化やアルゴリズム変更
- 500行未満のファイルを対象にした便乗リファクタリング
- `scripts/dev.py` など、別作業で変更中の開発環境整備
- vendor、生成物、`target/`、ログ、Markdown文書の行数是正

## 現状調査

### 抽出条件

次の種別をソースとして数える。

- Rust: `.rs`
- Python: `.py`
- JavaScript / TypeScript: `.js`, `.mjs`, `.ts`
- shell: `.sh`
- shader: `.wgsl`

2026-07-17の棚卸しでは19件が500行以上だった。

### 対象一覧

| 行数 | ファイル | 分類 | 主な分割方針 |
|---:|---|---|---|
| 2655 | `crates/bevy_app/src/plugins/startup/perf_scenario.rs` | 実装本体 | fixture、capture、audit、設定、出力 |
| 1952 | `scripts/perf.py` | 実装本体 | CLI shimとPython package |
| 1891 | `crates/bevy_app/src/systems/save/rehydrate.rs` | 実装本体＋大規模テスト | 復元フェーズとテスト群 |
| 1156 | `crates/hw_soul_ai/src/soul_ai/pathfinding/system.rs` | 実装本体 | queue、worker、scheduler、escape |
| 975 | `crates/bevy_app/src/systems/save/schema.rs` | 実装本体＋大規模テスト | schema inventory維持、validationとテストを分離 |
| 864 | `crates/hw_soul_ai/src/soul_ai/execute/task_execution_system.rs` | テスト主体 | 実装は維持し、テストを分類 |
| 802 | `crates/visual_test/src/setup.rs` | 実装本体 | scene、mesh、menu |
| 786 | `crates/hw_world/src/mapgen/wfc_adapter.rs` | 実装本体 | rules、solver、post-process |
| 720 | `crates/bevy_app/src/interface/ui/dev_panel.rs` | 実装本体 | component、spawn、action、status |
| 719 | `crates/hw_world/src/terrain_visual.rs` | テスト主体 | 実装は維持し、テストを分離 |
| 680 | `crates/hw_world/src/river.rs` | 実装本体 | course、sand、テスト |
| 660 | `crates/hw_jobs/src/lifecycle.rs` | テスト主体 | 実装は維持し、テストを分離 |
| 651 | `crates/bevy_app/src/systems/visual/character_proxy_3d.rs` | 実装本体 | sync、cache、GLTF readiness |
| 627 | `crates/hw_world/src/terrain_zones.rs` | テスト主体 | 実装は維持し、テストを分離 |
| 625 | `crates/hw_soul_ai/src/soul_ai/decide/idle_behavior/system.rs` | 実装本体 | wake、target、decision helper |
| 571 | `crates/visual_test/src/types.rs` | 実装本体 | render、behavior、UI、state |
| 542 | `crates/hw_world/src/pathfinding/mod.rs` | テスト主体 | 実装は維持し、テストを分離 |
| 540 | `crates/hw_world/src/mapgen/resources.rs` | テスト主体 | 実装は維持し、テストを分離 |
| 532 | `crates/bevy_app/src/plugins/messages.rs` | テスト主体 | 実装は維持し、テストを分離 |

このうち、実装本体の責務分割が必要なのはPythonを含む12件である。
残り7件は実装本体が500行未満であり、インラインテストの外出しで解消できる。

## 既存計画との関係

- `structural-maintainability-followups` は大規模ファイル分割を対象外として完了済み。
  本計画はその未着手領域を独立して扱い、完了済みのcrate抽出をやり直さない。
- `system-wide-correctness-refactoring` の不変条件を優先し、構造変更で意味論を変えない。
- WFCの既存計画で延期されていたadapter分割を、本計画のworld milestoneへ統合する。
- 開発環境hardeningやdocs index整備など、現在の別作業は本計画へ取り込まない。

## 設計方針

### 1. 既存ファイルをfacadeとして残す

既存の `foo.rs` は削除せず、同名ディレクトリの子モジュールを束ねるfacadeにする。

```text
foo.rs
foo/responsibility_a.rs
foo/responsibility_b.rs
foo/tests/mod.rs
```

これにより、原則として次を維持できる。

- `crate::...::foo` という既存module path
- 外部crateから見えるre-export
- pluginとappからのsystem登録箇所
- ドキュメントに記載された論理的な入口

`foo.rs` から `foo/mod.rs` への変換は、path衝突など明確な理由がない限り行わない。

### 2. systemではなく内部責務を分割する

Bevy system関数を複数systemへ分けると、deferred commandの適用点、query競合、
実行順、change detection、乱数消費順が変わり得る。本計画では既存system関数を
登録単位として維持し、pure helper、状態遷移、入出力変換を子モジュールへ移す。

### 3. 可視性を最小化する

- 子モジュール間共有は原則 `pub(super)` とする。
- crate外の公開型・関数・macro pathは維持する。
- facadeで不要な一括 `pub use` を追加しない。
- 分割のためだけにcomponentやresourceを公開しない。

### 4. テストは責務別に配置する

大きな `#[cfg(test)] mod tests` は `tests/mod.rs` と責務別ファイルへ移す。
private itemの検証が必要な場合も、production APIを広げず親module配下の可視性で解決する。

### 5. 行数基準

- 必須: 全対象ソースファイルを499行以下にする。
- 推奨: 新規子モジュールは450行以下にする。
- 例外: 宣言的なschema inventoryなど、一体性が安全性に直結するものは、
  無理に散らさずvalidationやtestsを外へ出して基準を満たす。
- コメントや空行の削除だけで達成した扱いにはしない。

## 維持すべき契約

### 共通

- system、observer、event/message登録元は一意のままにする。
- `.before`、`.after`、`.in_set`、`.run_if`、`.chain` の関係を変えない。
- `SoulAiCorePlugin`、`MessagesPlugin`、`SavePlugin`、startup、visual、interfaceの
  plugin wiringを変えない。
- 同一frame内のCommand適用タイミングを変えない。
- 分割だけのcommitではデータ構造やアルゴリズムを変更しない。

### task execution / AI

- `AssignedTask::Some` と `Without<WorkingOn>` の組み合わせは正当な一時状態である。
- 正常完了とretry可能な中断を混同しない。
- reservation cleanupとtask handoffの順序を維持する。
- `PathSearchResult::Deferred` を到達不能扱いせず、探索状態を保持する。
- path requestのFIFO、round-robin、budget、`WorldEpoch` resetを維持する。
- idle decisionではtask overrideを空間検索より先に評価する。
- 同一frameのpending rest reservation、cadence、profiling時のRNG消費順を維持する。

### world / WFC

- weighted-pattern選択のstale entry対策を維持する。
- retry回数、subseed生成、乱数消費順を維持する。
- terrain、zone、river等のpost-process順を維持する。
- `RemovedComponents` は毎回すべてdrainする。

### save / load

- dynamic component inventoryを単一のsource of truthとして維持する。
- preflight validationをworld mutationより先に完了する。
- rehydrateのフェーズ順、二重実行時のidempotence、参照解決順を維持する。
- save format、schema、互換性ルールは変更しない。

### profiling

- summary schema version 10を維持する。
- checksumの対象、encoding、順序を維持する。
- audit checkpointと固定fixtureの意味を維持する。
- `scripts/perf.py` のCLI option、exit code、artifact layoutを維持する。

## 期待される性能影響

runtime性能の向上は本計画の受入条件にしない。意図する挙動はno-opである。
分割後も追加system、追加query、追加allocationを導入しない。

見込める効果は、変更時の認知負荷低減、レビュー範囲の縮小、incremental compilationの
局所化である。実測なしにruntime改善を主張しない。

## 実装マイルストーン

### M0: ベースラインと回帰契約の固定

#### 作業

1. 500行以上の対象一覧を再取得し、本計画の表との差分を確認する。
2. `cargo check --workspace`、workspace test、Clippyの基準状態を記録する。
3. 既存のfocused test commandとtest名を各対象ごとに記録する。
4. 次の不足する回帰テストを、移動前のコードに対して最小限追加する。
   - WFC: 同一seedのrun-to-run一致だけでなく、固定fixtureのterrain/mask checksum
   - character proxy: transform同期、cache invalidation、GLTF ready遷移
   - profiling: schema version、checksum、固定checkpoint
   - save/load: preflight失敗時にworldが未変更であること、rehydrateのidempotence
5. `scripts/perf.py --help`、代表的な引数解析、policy判定、終了コードをfixture化する。

#### 完了条件

- 分割前のgreen baseline、または既知の無関係な失敗が記録されている。
- 高リスク領域で「移動後に同じ結果」を比較できる。
- この段階ではproduction codeの責務分割を行わない。

### M1: テスト主体7ファイルの分割

production codeを動かさず、インラインテストだけを外出しする。

| facade | 追加先 |
|---|---|
| `hw_soul_ai/.../task_execution_system.rs` | `task_execution_system/tests/{mod,fixtures,guards,aborts,completion}.rs` |
| `hw_world/src/terrain_visual.rs` | `terrain_visual/tests/{mod,spawn,updates,removal}.rs` |
| `hw_jobs/src/lifecycle.rs` | `lifecycle/tests/{mod,fixtures,assignment,completion,cleanup}.rs` |
| `hw_world/src/terrain_zones.rs` | `terrain_zones/tests/{mod,classification,updates,removal}.rs` |
| `hw_world/src/pathfinding/mod.rs` | `pathfinding/tests/{mod,costs,routes,edge_cases}.rs` |
| `hw_world/src/mapgen/resources.rs` | `mapgen/resources/tests/{mod,fixtures,validation,determinism}.rs` |
| `bevy_app/src/plugins/messages.rs` | `plugins/messages/tests/{mod,registration,delivery,cleanup}.rs` |

#### 実施規則

- test名とassertionを変更しない。
- 共通fixtureだけを `fixtures.rs` に置き、test間の暗黙依存を作らない。
- production itemの可視性は広げない。
- 各ファイルごとに移動と検証を完結させる。

#### 完了条件

- 7つのfacadeが499行以下である。
- 対応crateのtest結果が分割前と一致する。
- production diffがmodule宣言以外にない。

### M2: world純粋ロジックの分割

#### `hw_world/src/mapgen/wfc_adapter.rs`

```text
wfc_adapter.rs                 # 公開入口とorchestration
wfc_adapter/rules.rs           # tile/pattern mappingとweight
wfc_adapter/constraints.rs     # 隣接制約と候補絞り込み
wfc_adapter/solver.rs          # collapse/retry/subseed
wfc_adapter/post_process.rs    # zone/terrain後処理
wfc_adapter/visual_cross.rs    # visual test用変換
wfc_adapter/tests/mod.rs
```

最初に固定seedのgolden checksumを追加し、その後は機械的移動を優先する。
solverとpost-processを同時に書き換えない。

#### `hw_world/src/river.rs`

```text
river.rs
river/course.rs                # 流路候補と選択
river/sand.rs                  # 河岸・砂地処理
river/tests/mod.rs
```

river生成の呼び出し順とRNG引数の受け渡しをfacadeに残す。

#### 完了条件

- WFCの固定checksum、determinism、retry testが通る。
- riverの既存fixture結果が一致する。
- map generation phase順が変わっていない。

### M3: visual test・開発UI・character proxyの分割

#### `visual_test/src/setup.rs`

```text
setup.rs
setup/scene.rs
setup/mesh.rs
setup/menu/mod.rs
setup/menu/widgets.rs
setup/menu/header.rs
setup/menu/camera.rs
setup/menu/soul.rs
setup/menu/build.rs
```

#### `visual_test/src/types.rs`

```text
types.rs
types/render.rs
types/behavior.rs
types/ui.rs
types/state.rs
```

#### `bevy_app/src/interface/ui/dev_panel.rs`

```text
dev_panel.rs
dev_panel/components.rs
dev_panel/spawn.rs
dev_panel/actions.rs
dev_panel/status.rs
dev_panel/button_visuals.rs
```

#### `bevy_app/src/systems/visual/character_proxy_3d.rs`

```text
character_proxy_3d.rs
character_proxy_3d/sync.rs
character_proxy_3d/cache.rs
character_proxy_3d/gltf_ready.rs
character_proxy_3d/tests/mod.rs
```

#### 完了条件

- plugin/system登録とordering labelが分割前と同一である。
- visual testが起動し、menu操作と代表sceneが表示できる。
- dev panelのaction routingとbutton stateが一致する。
- character proxyの同期・cache回帰テストが通る。

### M4: profiling実装の分割

#### `bevy_app/src/plugins/startup/perf_scenario.rs`

```text
perf_scenario.rs
perf_scenario/config.rs
perf_scenario/config/parse.rs
perf_scenario/random.rs
perf_scenario/fixtures/mod.rs
perf_scenario/fixtures/gather.rs
perf_scenario/fixtures/path_door.rs
perf_scenario/fixtures/construction.rs
perf_scenario/fixtures/ui_gpu.rs
perf_scenario/fixtures/layout.rs
perf_scenario/capture/mod.rs
perf_scenario/capture/state.rs
perf_scenario/capture/driver.rs
perf_scenario/capture/checkpoint.rs
perf_scenario/audit/mod.rs
perf_scenario/audit/model.rs
perf_scenario/audit/checksum.rs
perf_scenario/audit/encoding.rs
perf_scenario/output.rs
perf_scenario/tests/mod.rs
```

facadeにはplugin wiring、scenario選択、最上位のphase遷移だけを残す。
fixture生成とcapture state machineは別commitにする。

#### `scripts/perf.py`

```text
scripts/perf.py                # 後方互換CLI shim
scripts/perf_tool/__init__.py
scripts/perf_tool/model.py
scripts/perf_tool/artifacts.py
scripts/perf_tool/execution.py
scripts/perf_tool/policy.py
scripts/perf_tool/summary.py
scripts/perf_tool/compare.py
scripts/perf_tool/fixtures.py
scripts/perf_tool/cli.py
scripts/tests/test_perf_tool.py
```

stdlib-onlyという現状を維持し、既存の直接実行パスを変えない。

#### 完了条件

- `scripts/perf.py` のhelp、引数、exit codeが一致する。
- 同じ保存artifactに対するsummary/checksum/policy判定が一致する。
- schema version 10と固定audit checkpointが一致する。
- 代表scenarioの短時間smoke runが成功する。

### M5: Soul AIの分割

#### `hw_soul_ai/src/soul_ai/decide/idle_behavior/system.rs`

```text
system.rs                      # 単一のBevy decision system
system/wake.rs
system/targets.rs
system/decision_helpers.rs
system/tests/mod.rs
```

task override判定とpending reservationの作成位置は最上位systemに明示的に残す。

#### `hw_soul_ai/src/soul_ai/pathfinding/system.rs`

```text
system.rs                      # 登録されるsystemとbudget orchestration
system/work_queue.rs
system/worker.rs
system/scheduler.rs
system/escape.rs
system/tests/mod.rs
```

queueの所有権と1frame当たりbudgetはfacade側から追跡できる形にする。

#### 完了条件

- decision cadenceと同一seed時の結果が一致する。
- task overrideが空間検索より先に評価される。
- FIFO、round-robin、Deferred再開、WorldEpoch resetのtestが通る。
- system登録数とschedule orderingが変わっていない。

### M6: save schemaの分割

#### 方針

dynamic component inventoryを複数ファイルへ散らさない。`schema.rs` にすべての
`for_each_*` inventory macroを残し、validation型・処理とテストを外出しする。

```text
schema.rs                      # inventoryの単一source of truth
schema/validation.rs
schema/tests/mod.rs
schema/tests/inventory.rs
schema/tests/validation.rs
schema/tests/compatibility.rs
```

必要なら宣言順を保ったまま、serialization helperだけを追加子モジュールへ移す。

#### 完了条件

- inventoryの重複・欠落がない。
- save schemaの列挙順と互換性testが一致する。
- unknown/missing/duplicate componentのvalidation結果が一致する。
- `schema.rs` 自体が499行以下になる。

### M7: rehydrateの分割

最も副作用と順序依存が強いため、他の分割パターン確立後に実施する。

```text
rehydrate.rs                   # preflightと復元順のorchestration
rehydrate/prerequisites.rs
rehydrate/presentation.rs
rehydrate/construction.rs
rehydrate/obstacles.rs
rehydrate/tests/mod.rs
rehydrate/tests/fixtures.rs
rehydrate/tests/prerequisites.rs
rehydrate/tests/presentation.rs
rehydrate/tests/construction.rs
rehydrate/tests/obstacles.rs
rehydrate/tests/idempotence.rs
```

#### 実施順

1. 既存テストを責務別ファイルへ移す。
2. pureな判定・変換helperを移す。
3. 各復元フェーズを、現在の呼び出し順を保って移す。
4. 最後にfacadeの重複importと一時的なwrapperを整理する。

#### 完了条件

- preflight失敗時にworld mutationがない。
- 復元phaseの順序が分割前と一致する。
- 複数回実行時のidempotenceが維持される。
- save/load round-tripと既存fixtureが通る。

### M8: 全体受入・ドキュメント整理

#### 作業

1. 対象一覧を再生成し、500行以上のソースが0件であることを確認する。
2. workspace全体のformat、check、test、Clippyを実行する。
3. plugin wiring、system数、登録順に意図しない差分がないか確認する。
4. 物理pathを記載する文書だけを更新する。
5. 実装完了後、本計画をarchiveへ移すか削除し、plans indexを更新する。

#### 完了条件

- 本計画の抽出条件で500行以上のソースファイルが0件である。
- 全新規ファイルが499行以下、原則450行以下である。
- 公開API、save schema、profiling schema、CLIに意図しない差分がない。
- `cargo check --workspace` とClippyがwarning 0で成功する。
- testと必要な手動smoke testが成功する。
- 恒久的な責務境界だけが関連docsへ反映されている。

## 検証方法

### 行数

```bash
find crates scripts -type f \( -name '*.rs' -o -name '*.py' -o -name '*.js' -o -name '*.mjs' -o -name '*.ts' -o -name '*.sh' -o -name '*.wgsl' \) -print0 \
  | xargs -0 wc -l \
  | sort -nr \
  | awk '$1 >= 500 && $2 != "total"'
```

500行以上の抽出結果が空であることを確認する。

### Rust

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo check -p bevy_app@0.1.0 --lib --features profiling
cargo check -p visual_test
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

各milestoneでは、workspace検証の前に変更crateのfocused testを実行する。

### Python

```bash
python3 scripts/perf.py --help
python3 -m unittest discover -s scripts/tests -p 'test_perf_tool.py'
```

テストmodule名は実装時の既存 `scripts/tests` 構成に合わせるが、CLIをsubprocessで
呼び、後方互換のentrypointも検証対象に含める。

### 差分品質

```bash
git diff --check
git diff --stat
```

各commitで「移動」と「挙動変更」を混在させない。本計画中に不具合を発見した場合は、
分割commitとは別の修正として扱う。

## 変更予定ファイル

### 既存ファイル

- 対象一覧の19ファイル
- module宣言に必要な直近の親module
- 物理pathを直接記載している関連docs
- `docs/plans/README.md`

### 新規ファイル

- 各milestoneで示した子モジュール
- 回帰テスト用の責務別test module
- `scripts/perf_tool/` 配下のPython module

親moduleやpluginファイルは、Rust module解決または登録維持に必要な最小差分に留める。

## リスクと対策

| リスク | 兆候 | 対策 |
|---|---|---|
| module移動で可視性を広げる | `pub` が大量に増える | `pub(super)` を基本とし、facade経由にする |
| Bevy実行順が変わる | same-frame testやvisualが変化 | systemは分割せず、登録式を変更しない |
| RNG消費順が変わる | 同一seedのchecksumが変化 | RNGをfacadeから同じ順で渡し、golden testで固定 |
| schema inventoryが欠落する | round-tripやvalidation失敗 | inventory macroを1ファイルに残す |
| rehydrate順が変わる | entity参照やvisual復元が不安定 | orchestrationをfacadeに残し、phaseごとに移す |
| test移動でproduction APIが拡大する | test用 `pub` が増える | 親子moduleのprivate accessを使う |
| 行数だけを満たす過分割 | 1関数だけのmoduleが乱立 | 変更理由と依存が同じ責務はまとめる |
| 並行作業の差分を巻き込む | 無関係なファイルがstagedされる | milestoneごとにpath限定でdiffとstageを確認する |
| docs linkが陳腐化する | 旧物理pathへのリンク切れ | facade維持を優先し、必要なdocsのみ更新する |

## Commit分割方針

- 1 commitは原則1facade、または同じcrateの密接なtest-only群とする。
- M0の回帰テスト追加はproduction移動より先にcommitする。
- file moveとalgorithm修正を同じcommitにしない。
- save/schema/rehydrateはそれぞれ独立commitにする。
- Python分割とRust profiling分割は別commitにする。
- 各commit messageは `refactor: split ...` または `test: cover ...` を基本とする。

## Rollback方針

milestone単位で独立して戻せるようにする。失敗時は直前の分割commitだけを対象にし、
他セッションのworktree変更を破棄しない。revert前には履歴と対象diffを確認する。

golden testが変化した場合、期待値を更新して通すのではなく、まず次を確認する。

1. 呼び出し順
2. iteration順
3. RNG消費順
4. deferred command適用点
5. save/rehydrateのmutation開始点

構造変更では一致しない理由が説明できるまで期待値を変更しない。

## 実装結果

- 対象19ファイルを責務別facade・子モジュール・test moduleへ分割し、抽出条件に合う500行以上のソースを19件から0件へ削減した。
- 最大は `idle_behavior/system.rs` の499行。新規子モジュールもすべて499行以下である。
- 既存のplugin/system登録、WFCの乱数・後処理順、pathfinding queue/budget、save schema inventory、rehydrate順、profiling schema、`scripts/perf.py` entrypointを維持した。
- `scripts/dev.py verify` によりPython 14テスト、perf self-test、docs/repository契約、workspace check、profiling feature check、warning 0のClippy、workspace test、doctestが成功した。

## AI引継ぎメモ

### 現在地

- M0〜M8の分割・検証・恒久docs同期を完了し、対象19件はすべて499行以下になった。
- 最大ファイルはidle decision facadeの499行で、500行以上の抽出結果は空。
- focused test、profiling feature test、workspace品質ゲートはすべて成功済み。

### 次に行うこと

なし。本書は完了計画としてarchiveへ移す。

### 実装時の注意

- 作業開始時に対象ファイルの最新行数とworktree差分を再確認する。
- 同時進行中の変更を本計画のcommitへ含めない。
- Bevy APIを変更する必要が生じた場合は0.19の一次情報を確認する。
- 新しいsystemや公開APIが必要になった場合は、分割ではなく設計変更なので作業を止めて再計画する。
- milestone完了ごとに本書のステータスと更新履歴を更新する。

## 更新履歴

| 日付 | 内容 |
|---|---|
| 2026-07-17 | M8完了。500行以上0件、docs同期、`scripts/dev.py verify` 全品質ゲート成功を確認しarchiveへ移動 |
| 2026-07-17 | M2〜M7完了。world、visual/UI、profiling、Soul AI、save schema、rehydrateを責務別子モジュールへ分割し、500行以上0件を確認 |
| 2026-07-17 | 実装開始。M0 baselineを確認し、M1のtest-only分割へ着手 |
| 2026-07-17 | 初版。500行以上のソース19件を棚卸しし、M0〜M8の9段階の分割計画を作成 |
