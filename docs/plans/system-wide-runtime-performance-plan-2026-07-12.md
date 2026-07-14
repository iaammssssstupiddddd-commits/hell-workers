# 全体ランタイム・ホットパス性能改善計画書

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `system-wide-runtime-performance-plan-2026-07-12` |
| ステータス | `In Progress` |
| 作成日 | `2026-07-12` |
| 最終更新日 | `2026-07-13` |
| 作成者 | `Codex` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |
| 関連計画 | `docs/plans/archive/system-wide-performance-followups-plan-2026-07-07.md`, `docs/plans/performance-cpu-2026-04-16.md`, `docs/plans/system-wide-correctness-refactoring-plan-2026-07-12.md`, `docs/plans/runtime-correctness-contracts-plan-2026-07-12.md`, `docs/plans/save-load-hardening-plan-2026-07-12.md`, `docs/plans/structural-maintainability-followups-plan-2026-07-12.md` |

## 1. 目的

- **解決したい課題**: Soul / Familiar / Designation / 建設サイトが増えたときに、意味上は値が変わっていないコンポーネントまで Bevy の変更検知を発火させ、予約再構築・空間同期・UI ViewModel・3D proxy 等の後段処理を毎フレーム連鎖起動している。また、Familiar 委譲、A*、Door、建設進行、UI、shader に全件走査または予算外処理が残っている。
- **到達したい状態**:
  - ゲームロジックに必要な更新と、見た目だけの更新を分離する。
  - 毎フレーム処理を「変更された対象だけ」「明示的にdirtyになった対象だけ」へ限定する。
  - runtimeの実経路探索A*を共通予算内に収め、boolean到達可能性判定はversion付き連結成分cacheへ置換する。
  - Door の通常開閉で到達可能性cacheや既存pathの歩行可否を無効化せず、Locked切替等のtopology変更だけを再検証契機にする。ただし、このversion契約は正しさ計画側で先に一元化する。
  - CPU / GPU のどちらが律速かを再現可能なシナリオで判断し、実測で効果が確認できた変更だけを残す。
- **成功指標**:
  - `cargo check --workspace` と `cargo clippy --workspace --all-targets -- -D warnings` が成功する。
  - タスク未割当の Soul が `task_execution_system` だけを理由に `DamnedSoul` / `AssignedTask` / `Destination` / `Path` / `Inventory` の changed 対象にならない。
  - 予約再構築は、割当・予約対象・予約種別・終了 disposition 等の予約意味が変化したとき、または明示した安全監査周期だけで実行される。
  - 静止中の Soul / Familiar の論理 root `Transform` は visual animation だけを理由に changed にならない。
  - Idle Familiar の通常委譲は仕様どおり最大 0.5 秒周期であり、dirty wake-up を除いて毎フレーム実行されない。
  - runtimeの経路生成core A*呼び出し回数が設定したフレーム予算を超えない。expanded node数は観測し、resumable searchなしでhard capにはしない。
  - Familiar候補判定、auto-gather、command assignmentのboolean到達可能性判定がA*を呼ばず、既存A*とのparity testを満たす。
  - Door の `Open` ↔ `Closed` で topology version が変化せず、`Locked`・Door追加削除・建物/地形変更では必ず変化する。
  - 目標シナリオで変更前後の p50 / p95 / p99 frame time、allocation/frame、対象system時間を記録する。各sub-milestoneは実装前にprimary metricを一つ以上宣言し、3回測定のbaseline分散を超えるwork counterまたは直接時間の改善がある場合だけ採用する。M0/M9はこの採用判定の対象外とする。

## 2. スコープ

### 対象（In Scope）

- 計測用 profile、既存 `--perf-scenario` の測定手順、feature-gated な性能カウンタ。
- `task_execution_system` の mutable access 範囲と予約 dirty 契約。
- Soul / Familiar の論理 root `Transform` と visual animation の分離。
- Familiar task delegation の周期、snapshot、squad/candidate 構築、Build assignment producer の重複。
- Soul pathfinding、task handler、fallbackを含む実経路探索A*予算の一元化と、runtime boolean到達可能性の連結成分cache化。
- Door近傍判定と、正しさ計画が所有する`obstacle_version`契約の利用。Door passability/version mutation自体は本計画で二重実装しない。
- Floor / Wall construction の phase 遷移・completion・curing の変更駆動化。
- vitals / idle decision / state sanity / energy buff 等、移動と同じ 60 Hz を必要としない処理の低頻度化。
- Blueprint progress bar、Entity List、area indicator、RTT composite、3D proxy の更新対象限定。
- 実測で GPU 律速と確認できた場合の shadow shader、Soul mask/shadow scene、RTT 解像度の最適化。
- 実装後の恒久ドキュメントと計画索引の同期。

### 非対象（Out of Scope）

- Familiar の優先順位、タスク成立条件、Soul の疲労・夢・ストレス量などゲームバランスの変更。
- `TaskEndDisposition`、完了 observer、retryable handoff の意味変更。
- A* を JPS、navmesh、flow field 等へ全面置換すること。
- WorldMap のサイズ、地形生成、セーブ形式の全面再設計。
- GPU 律速の実測前に sprite の一括縮小や GLB の見た目を変更すること。
- 2026-07-07 完了計画で既に実装された次の項目の再実装:
  - `WorldMap.obstacle_version` と `Path.validated_obstacle_version` による版一致時の path 検証スキップ。
  - `CachedStockpileGroups` による producer 間の同一フレーム共有。
  - Dream UI の共有 material bucket と global time。
  - Entity List の structure dirty / value dirty 分離そのもの。
  - 3D 非表示時の `run_if`、face material 更新ガード。
  - `TileSiteIndex` の tile → site 逆引き。

### 維持必須の動作契約

- `AssignedTask::None` への終了が、完了・中断・再試行可能な引継ぎを混同しないこと。
- `OnTaskCompleted` は `TaskEndDisposition::Completed` のときだけ発火すること。
- Yard 共有タスクを Idle Familiar が発見でき、割当までの追加遅延は最大 0.5 秒であること。
- Open / Closed Door は通行可能、Locked Door は通行不可であること。Closed Door の追加コストは新規 A* に反映すること。
- UI の structure 変更は即時反映し、Entity List の値表示は現行仕様の 100 ms 以内に反映すること。
- pause 中は既存どおり Spatial / Logic / Actor が停止し、Visual / Interface の挙動を変更しないこと。

## 3. 現状とギャップ

### 3.1 前計画で完了済みの基盤

- SpatialGrid は Added / Changed / Removed の差分同期になっている。
- path は `obstacle_version` が一致する場合に waypoint 再検証を省略できる。
- stockpile group は producer 間でフレーム共有されている。
- Dream world/UI particle は共有 material と global time を利用している。
- terrain は chunk、shared material、LOD hysteresis、境界 early-out を持つ。
- `TileSiteIndex` は reverse index を持つ。

これらは維持し、今回の計画で別方式へ置き換えない。

### 3.2 新たに確認した主要ギャップ

| ID | 優先度 | 現状 | 波及先 | 期待効果 |
| --- | --- | --- | --- | --- |
| H1 | P0 | `task_execution_system` が `AssignedTask::None` を含む全 Soul を mutable context へ変換する | 予約、Visual Mirror、Entity List、system 並列性 | 非常に高い |
| H2 | P0 | Soul / Familiar animation が論理 root `Transform` を毎フレーム変更する | SpatialGrid、3D proxy、Transform propagation | 高い |
| H3 | P0 | Idle Familiar が 0.5 秒 delegation timer を迂回する | snapshot、candidate scoring、A* reachability | 高い |
| H4 | P0 | task helper / fallback の A* が `MAX_PATHFINDS_PER_FRAME` の外で実行される | p95 / p99 frame spike | 高い |
| H5 | P1 | Door ごとに全 Soul と残 path を走査し、Open/Closed でも obstacle version を更新する | path 再検証、reachability cache | 高い |
| H6 | P1 | 建設 site ごとに全 tile、curing 中は全 Soul を毎フレーム走査する | 大規模建設時 CPU / allocation | 高い |
| H7 | P1 | Blueprint progress bar の親には `ProgressBar` が付かず、`Without<ProgressBar>` が永続的に成立する | Blueprint × progress bar の二重走査 | 中～高 |
| H8 | P1 | Entity List の value dirty が 100 ms gate なしで ViewModel 全件再構築へ進む | format、sort、UI sync allocation | 中～高 |
| H9 | P2 | shadow shader は最大 18 shadow sample と最大 12 Soul projector を fragment ごとに評価する | GPU terrain / section pass | GPU 律速時に高い |
| H10 | P2 | Soul ごとに main / mask / shadow 用として同じ GLB scene を 3 系統 spawn する | draw、skin/vertex、mask RTT | GPU 律速時に高い |

### 3.3 旧 `performance-cpu-2026-04-16.md` との関係

旧計画は現行コードと再照合し、本計画で置き換える。実装状況は次のとおりである。

| 旧項目 | 2026-07-12時点 | 本計画での扱い |
| --- | --- | --- |
| P1 Shadow Projector Top-K | 実装済み | 再実装しない |
| P2 Proxy cleanup owner cache | 実装済み | M2はcleanupではなくlogical/visual Transform分離を扱う |
| P3 TaskArea shader global time | 実装済み | 再実装しない |
| P4 producer共有cache | 一部実装済み。producer間共有はあるが`CachedActiveFamiliars` / `CachedActiveYards`は毎フレーム`clear`/`extend`する | M3Aのgeneration/dirty駆動cacheとして残課題だけ扱う |
| P5 construction producerのwaiting cache | `FloorTileWaitingCache` / `WallTileWaitingCache`で実装済み | M5は別基盤の`TileSiteIndex`を使い、phase/completion/curing consumer側を扱う |
| P6 assignment loop一時allocation | 実装済み。fallback用`to_vec()`は意図的に保持 | M3のscratch再利用は別の残存candidate allocationだけ扱う |
| P7 Dream UI global time | 実装済み | 再実装しない |

旧計画は`Superseded`とし、未完P4だけをM3Aへ移管する。

2026-07-07完了計画は `NeedsPath` 等による全Soul二重走査の置換もM4へ記載しているが、現行 `pathfinding_system` には `for prioritize_tasks in [true, false]` と全query走査が残る。version一致時のpath再検証skipは完了済み、対象Soulのqueue/marker化は未完了として本計画M4で扱う。

### 3.4 並行計画との依存・所有権

正しさロードマップは3子計画へ再編済みである。古い「正しさ計画M2/M3/M4/M6」という参照は使わず、次の実在する契約へ依存する。

| 本計画 | 前提/競合 | 正本の所有者 | 実装順 |
| --- | --- | --- | --- |
| M0 | library/test harness、`main.rs`/plugin配線 | `runtime-correctness-contracts` M0がlibrary境界、本計画M0が計測機能 | 正しさM0 → 本計画M0 |
| M1/M3 | task terminal/Relationship/reservation lifecycle | `runtime-correctness-contracts` M3。`save-load-hardening` M5が旧`ReservedForTask`互換を完了 | 両者完了後に本計画をrebase |
| M1/M7 | `RemovedComponents`の完全消費 | `runtime-correctness-contracts` M1 | helper導入後に頻度だけ最適化 |
| M4 | obstacle source、Door passability、version bump | `runtime-correctness-contracts` M4 | 正しさM4完了後。本計画はversionを読むだけ |
| M5 | load frame境界/reset、building footprint/index rehydrate | `runtime-correctness-contracts` M4と`save-load-hardening` M4 | 両者完了後にcounter/`CuringFootprint`再構築をregistryへ追加 |
| M2/M4/M5 | Spatial index共通化 | `structural-maintainability-followups` M2 | 本計画M1～M7後にstructural M2へ移行 |
| M9 | 恒久docs、archive、index | 各計画共同 | 未達を分離して最後に同期 |

`runtime-correctness-contracts` M4は`obstacle_version = is_walkableのtopology世代`、Open/Closedでは不変、Locked境界では更新する契約へ統一済みである。本計画はそのversionを読むだけとし、経路選択costの世代管理が実測で必要になった場合だけ別の`path_cost_version`を追加する。性能計画から`map/doors.rs`等のmutation契約を上書きしない。

推奨実行waveは、(A) `runtime-correctness-contracts` M0、(B) 本計画M0、(C) `runtime-correctness-contracts` M1～M4、(D) `save-load-hardening` M1～M5、(E) 本計画M1～M7、(F) 条件付きM8、(G) structural計画と各M9である。共有ファイルを別branch/別agentで並行編集しない。

## 4. 実装方針（高レベル）

- **意味のある変更だけを dirty にする**: `Changed<T>` を最適化の根拠にする前に、書き込み側が不要な `DerefMut` を行っていないことを確認する。
- **mutable access を遅延する**: Query から `Mut<T>` を得ても、実際に書く handler だけが `DerefMut` する構造へ寄せる。`bypass_change_detection()` で症状を隠さない。
- **予約は意味の signature で判定する**: `AssignedTask` 内の progress 変化と、予約対象・数量・種別の変化を分離する。
- **論理座標をrootに固定する**: root `Transform`はmovement/teleport/load等の論理処理だけが更新する。FamiliarのSpriteはvisual childへ分離する。Soulは現行GLB proxyがroot scale/rotationを描画へ反映していないため、最適化で新しいpulse/tiltを導入せず、表示consumerのないroot visual writeを削除する。既存表示に実consumerが見つかったoffsetだけを専用visual stateへ移す。
- **到達可能性と経路生成を分ける**: boolean判定はversion付き連結成分cache、waypointが必要な探索だけをbudgeted A*へ通す。
- **予算はcore operationで数える**: logical request数ではなく実core A*呼び出しを数える。expanded node数は観測値として残す。
- **低頻度処理は経過時間を保持する**: 単純な `run_if(on_timer(...))` で render frame の `Time::delta()` を捨てず、固定 step または明示 accumulator を使う。
- **CPU と GPU を分離する**: `Render3dVisible` と既存 `RenderPerfToggles` を使い、GPU変更は M0 の結果を開始条件にする。
- **段階適用する**: 依存waveを守りつつsub-milestoneごとにrevert可能なcommitへ分け、事前宣言したprimary metricで採否を判断する。
- **crate 境界を維持する**: domain resource / component は該当 `hw_*` crate、ゲーム固有の配線と UI 表示は `bevy_app` に置く。
- **load後のEntityを持ち越さない**: 本計画で追加するEntity-bearing Local/Resource/cache/queueは、`save-load-hardening` M4の`WorldEpoch`不一致でclearするか、root load facadeのreset inventoryへ登録してload時にclearし、追加時にreset inventoryも更新する。
- **Bevy 0.19 を一次情報で確認する**: `Mut<T>`、Relationship、FixedUpdate、Assets、shader binding 等は docs.rs または `~/.cargo/registry/src/` の 0.19.0 ソースを確認してから実装する。

## 5. マイルストーン

### M0: 再現可能なベースラインと観測点の確立

- **目的**: 静的推測だけで変更順を決めず、同じ負荷・同じ描画条件で前後比較できる状態にする。
- **変更内容**:
  - 既存 `--spawn-souls` / `--spawn-familiars` / `--perf-scenario` を拡張し、Plugin登録前にCLI/envを一度だけparseした `PerfScenarioConfig { seed, workload, soul_count, familiar_count, render_mode }` を生成する。`--perf-seed`、`--perf-workload gather|path-door|construction|ui-gpu`、`--perf-render cpu|gpu`を追加する。
  - 現行worldgenは`HELL_WORKERS_WORLDGEN_SEED`を独立に読み、perf setupより前にseedが確定する。perf時は`PerfScenarioConfig.seed`をmaster seedの正本とし、worldgen、Soul、Familiar、workload、visualの独立substreamへ決定的に分配する。優先順位はperf CLI > 明示worldgen env > randomとし、非perf起動の挙動は維持する。
  - 初期spawn位置・trait・TaskArea・Designation・Door/建設状態・camera位置を各substreamから決定し、同じ引数なら同じentity/task件数とstate checksumになるようにする。選択workloadの件数・配置・GPU batchingへ影響するvisual variantを変える`thread_rng()`経路はseeded `StdRng`へ移し、CPU計測では無関係なspeech/Dream等のcosmetic randomを無効化できるようにする。
  - scenario driverを専用system setへ置き、`Warmup → Measure → Flush → AppExit`を自動遷移させる。camera移動、Door切替、建設開始等の操作列はvirtual time基準で実行し、手入力を計測条件へ含めない。
  - 次の規模を固定シナリオとして定義する。
    - Small: 50 Soul / 4 Familiar。
    - Medium: 200 Soul / 12 Familiar。
    - Large: 500 Soul / 30 Familiar。
  - warm-up 30秒、計測60秒、固定window size、`HW_PRESENT_MODE=novsync`、固定log filterを標準条件にする。Tracy memory、通常frame time、RenderDocは互いの計測擾乱を避けるため別runで採取する。
  - `--perf-render`から`Render3dVisible`と`RenderPerfToggles`を無入力で固定し、CPU-only寄り/GPU込みを分ける。現行`HW_DISABLE_RTT_SCENE_OBJECTS`が対象外としている`SoulMaskProxy3d`/`SoulShadowProxy3d`も含めてtoggle契約を修正し、OFF時に対象draw/scene entity数が期待値まで下がることを確認する。
  - `[profile.profiling]` を release 継承、debug symbol 有効、thin LTO で追加する。
  - `profiling`はCSV/counter専用、`profiling-tracy`はsystem CPU trace、`profiling-memory`はallocation traceへ分離し、標準frame time runへTracyの接続待ち/メモリ挙動を混在させない。計測siteを持つ各`hw_*` crateにも空defaultの`profiling` featureを定義し、`bevy_app`のfeatureから伝播する。
  - scenario用のdeferred commandを適用した直後、`GameSystemSet::Input`より前に初期fixture checkpointを採る。Soul/Familiar数が構成値に一致するまで開始せず、初回のAI/animation更新後の状態を「初期」と誤認しない。
  - p50/p95/p99とdomain counterはrun固有のignored artifact directoryへCSV出力し、`scripts/perf.py`がbuild、絶対asset root、GPU/backend照合、log健全性、反復集約を一元化する。初期fixture checksumは常に3反復一致を要求する。実時間runのwarm-up終端checksumは、境界frameの可変deltaによる位相差をartifactへ記録する標準`record`とし、完全な状態決定性は将来の固定step auditで別検証する。完了marker後のteardown warningは件数・原文を記録するが、marker前のwarning/errorは失格にする。system CPU timeはTracy capture、allocation/frameはTracy memoryで採取する。GPU pass/drawは固定frameのRenderDoc capture、texture sampleはshaderの静的sample数と利用可能ならvendor profilerで記録する。raw traceはcommitせず、要約値だけを `docs/performance-profiling.md` に残す。
  - 次の軽量カウンタを feature-gated で追加する。
    - task execution の query 件数、実handler件数、idle skip件数。
    - reservation full rebuild 回数と走査した pending/active 件数。
    - delegation cycle、candidate/worker 組合せ、snapshot build 回数。
    - caller別core A*回数、expanded nodes、fallback。`budget defer`はM4導入時に追加する。
    - Door が検査した Soul / waypoint 件数。
    - construction site / tile / evacuation 走査件数。
    - logical root Transform changed 件数、3D proxy Transform 書込件数。
  - hot loop 内の per-candidate Atomic は通常ビルドで無効化し、profiling時もsystem-local集計後に一度だけ加算する。
- **変更ファイル**:
  - `Cargo.toml`
  - `crates/bevy_app/Cargo.toml`
  - `crates/bevy_app/src/main.rs`
  - `crates/bevy_app/src/plugins/visual.rs`
  - `crates/bevy_app/src/plugins/startup/perf_scenario.rs`
  - `crates/bevy_app/src/plugins/startup/mod.rs`
  - `crates/bevy_app/src/systems/settings/mod.rs`
  - `crates/bevy_app/src/plugins/startup/startup_systems.rs`
  - `crates/bevy_app/src/world/map/spawn.rs`
  - `crates/bevy_app/src/entities/damned_soul/spawn.rs`
  - `crates/bevy_app/src/entities/familiar/spawn.rs`
  - `crates/hw_core/Cargo.toml`
  - `crates/hw_jobs/Cargo.toml`
  - `crates/hw_energy/Cargo.toml`
  - `crates/hw_logistics/Cargo.toml`
  - `crates/hw_spatial/Cargo.toml`
  - `crates/hw_ui/Cargo.toml`
  - `crates/hw_world/Cargo.toml`
  - `crates/hw_soul_ai/Cargo.toml`
  - `crates/hw_familiar_ai/Cargo.toml`
  - `crates/hw_visual/Cargo.toml`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/resources.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_delegation.rs`
  - `crates/hw_jobs/src/visual_sync/observers.rs`
  - `crates/hw_soul_ai/src/soul_ai/execute/gathering_apply.rs`
  - 各対象systemのmetrics定義、または新規のfeature-gated diagnostics module
  - `scripts/perf.py`
  - `docs/performance-profiling.md`
  - `docs/gathering.md`
- **期待効果**: 直接の高速化はない。誤った仮説の反復と計測ノイズを減らす。
- **完了条件**:
  - [x] 3規模 × CPU/GPU切り分け条件で、log健全性を満たすp50 / p95 / p99 と主要カウンタを記録した。
  - [x] 同一seed/workloadを3回実行し、ゲーム更新前のinitial fixture（Soul/Familiar/Designation数とstate checksum）、固定camera/配置、操作列が一致することを確認した。
  - [ ] 固定stepの決定性auditを別計測モードとして設計し、必要なworkloadでwarm-up終了時のentity/task/state checksumを一致確認する。実時間frame-time baselineとは混在させない。runnerとactor単位artifactは実装済みだが、現行`gather`はtick 224以降で状態差が出るため、この確認は未達として分離する。
  - [x] CPU toggle OFFでmain/mask/shadowを含む対象3D scene entity数が期待値まで減る。runnerはCPUでSoul main/mask/shadow・Familiar rootが0、GPUでfixture人口と一致する`scene_roots.csv`を検証する。pass/drawの内訳はRenderDoc採取の契約として別runで扱う。
  - [x] CSV、Tracy memory、RenderDocのどの指標をどの手順で採るか文書化した。
  - [x] profiling feature 無効時に計測用処理が hot path に残らない。
  - [x] baseline の起動コマンドと結果記録形式を `docs/performance-profiling.md` に記載した。
- **検証**:
  - `PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py self-test`
  - `python3 scripts/perf.py run --workload gather --sizes small,medium,large --renders cpu,gpu --repeat 3 --seed 20260712 --backend vulkan --adapter Intel --window-backend wayland --output target/perf-runs/<session>`
  - `cargo check -p bevy_app@0.1.0 --no-default-features --features profiling`
  - `cargo check -p bevy_app@0.1.0 --no-default-features --features profiling-tracy`
  - `cargo clippy -p bevy_app@0.1.0 --no-default-features --features profiling -- -D warnings`
  - `cargo check -p bevy_app@0.1.0 --no-default-features --features profiling-memory`
  - `cargo check --workspace`
  - `cargo clippy --workspace`

#### 2026-07-14 M0 smoke（NVIDIA MX250 / Vulkan）

- `scripts/perf.py self-test` と `gather / Small / CPU・GPU / repeat 1 / warm-up 0秒 / measure 1秒` を実行し、2 runともvalidだった。これはrunner、`summary.csv`、adapter/log検証、CPU/GPU切替の経路確認であり、性能比較用のbaselineではない。
- CPUの`scene_roots.csv`はSoul main/mask/shadowとFamiliar rootがすべて0、GPUはそれぞれ50/50/50/4だった。CPU-only条件へ対象外の3D scene rootを混在させない契約を確認した。
- fixed-step auditのrunnerとactor単位artifactは存在するが、`gather`はtick 224以降にstate checksumが分岐する。frame-time baselineの採取を止めない未達項目として保留し、原因追跡はM0の性能計測基盤とは切り離す。

#### 2026-07-13 Gather 暫定値（Intel UHD / Vulkan、比較用には未認定）

- 条件: Fedora 44、Intel Core i7-10850H、Intel UHD Graphics (CML GT2)、Mesa 26.1.4 Vulkan、`WGPU_ADAPTER_NAME=Intel`、`HW_PRESENT_MODE=novsync`、固定1280×720 window。
- 各条件はseed `20260712`、warm-up 30 virtual秒、計測60 virtual秒、`gather` workloadで3回実行した。raw CSV/logは`target/perf-runs/m0-20260713-intel-uhd-seed-20260712/`にのみ保持し、commitしない。
- 下表は3回の中央値。`reach/frame`は`reachable_with_cache_calls / captured frame`の中央値である。

| 規模 | 描画 | p50 ms | p95 ms | p99 ms | reach/frame |
| --- | --- | ---: | ---: | ---: | ---: |
| Small (50/4) | CPU | 16.642 | 17.820 | 19.311 | 0.370 |
| Small (50/4) | GPU | 31.216 | 32.997 | 37.286 | 0.919 |
| Medium (200/12) | CPU | 16.420 | 20.222 | 212.020 | 35.618 |
| Medium (200/12) | GPU | 37.830 | 40.641 | 249.783 | 87.098 |
| Large (500/30) | CPU | 26.517 | 41.269 | 691.625 | 222.330 |
| Large (500/30) | GPU | 50.111 | 56.202 | 707.456 | 377.262 |

- 全18 runでseed、Soul数、Familiar数、Designation数、初期state checksumが各条件の3反復で一致し、asset load errorは0件だった。Small/CPUのp50は10.015–16.662 msと分散が大きかった。
- `source_selector_calls`は全runで0だった。現行Gather操作列はsource selectorを負荷化していないため、当該counterの比較は専用workload追加後に行う。
- 後日のlog再監査で、`PERF_CAPTURE`前に`ParticipatingIn`の不存在target warningと、despawn済みtargetから`GatherHighlightMarker`をremoveするBevy command errorが反復していたことを確認した。したがってこの表は問題の発見用の暫定値として残し、最適化前後の比較baselineには使わない。
- `CommandQueue has un-applied commands`警告は全runで`PERF_CAPTURE` CSV出力の後にのみ発生した。計測区間外のteardown警告としてraw logへ残し、frame timeへは混在させない。
- 次の採取は`PerfScenarioConfig`のstrict parse、1x virtual time固定、run固有output directory、initial/warm-up/measure-end checkpoint、`scripts/perf.py`のadapter/log/checksum検証を通す。RenderDocによるdraw/pass確認、Tracy memoryによるallocation/frameは引き続き未採取である。

#### 2026-07-13 計測器の再検証（Intel UHD / Vulkan、短縮経路）

- `scripts/perf.py self-test`を通し、`gather / Small / CPU / seed 20260712 / warm-up 2秒 / measure 3秒`を3反復した。artifactは`target/perf-runs/m0-runner-repeat-after-initial-checkpoint-20260713-a/`にのみ保持する。
- 3反復とも実adapterは`Intel(R) UHD Graphics (CML GT2)` / Vulkan / Mesa 26.1.4であり、短縮経路の`PERF_CAPTURE`前warning/errorは0件だった。これは計測器の経路検証としてのみ有効であり、30/60秒の正式baselineとは区別する。
- `initial_state_checksum`は全runで`ec0d54db7ffbbd2b`（50 Soul / 4 Familiar / 72 Designation）に一致した。これはcheckpointをゲーム更新前へ移した結果であり、Familiar hover animationを初期fixtureに混ぜない。
- warm-up終端checksumは`0ffee40345c4f99d`、`a2f46c5cbc6f6982`、`e5fb0f4757ccdc35`に分かれ、実warm-up時間も2.000577–2.004146秒だった。可変real deltaで閾値を跨ぐ実時間計測のため、標準policyを`record`へ分離した。`require`なら正しく失格になる。
- `record`再集約では3 valid run、p50中央値 9.812 ms、p95中央値 12.850 ms、p99中央値 13.639 ms（MAD: 0.031 / 0.048 / 0.080 ms）になった。これは手順・契約の短縮検証値であり、30/60秒・全規模×CPU/GPUの正式baselineではない。
- 正式matrixの再開直後のSmall/CPU 30/60秒runでは、`PERF_CAPTURE`前に`ParticipatingIn`の不存在target warningを3件検出したため、そのrunは失格として残し、数値を採用しない。Bevy 0.19の当該warningはtargetのdespawn cleanupではなく、存在しないtargetへのRelationship insert時に出る。`gathering_apply_system`を一括正規化し、同じmessage batchで退役予定のspotをtargetにするRecruitと退役予定absorberへのMergeを破棄し、残るjoinもdeferred command適用時にsource/targetの存続を確認するよう修正した。Dissolve+Recruit、Dissolve+Merge、既に消えたtargetへのRecruitの回帰テストを追加し、修正後に短縮runから正式matrixを再実行する。

#### 2026-07-13 Gather 正式baseline（Intel UHD / Vulkan、schema v2）

- 条件は Fedora 44、Intel Core i7-10850H、Intel(R) UHD Graphics (CML GT2)、Mesa 26.1.4 / Vulkan、1280×720、`HW_PRESENT_MODE=novsync`、seed `20260712`、`gather` workload、warm-up 30 virtual秒、measure 60 virtual秒で固定した。各caseを3反復し、`scripts/perf.py`のadapter/checksum/log検証を通した。
- artifact は `target/perf-runs/m0-post-relationship-fix-long-small-cpu-20260713-a/`、`m0-post-relationship-fix-long-small-gpu-20260713-a/`、`m0-post-relationship-fix-long-medium-large-20260713-a/` にのみ保持する。全18 runがvalidで、capture完了前のwarning/errorは0件だった。
- 下表はrunごとのquantileを3反復で集約した中央値である。値は以後のframe-time比較の基準とし、旧「暫定値」は比較に使わない。

| 規模 | 描画 | p50 ms | p95 ms | p99 ms |
| --- | --- | ---: | ---: | ---: |
| Small (50/4) | CPU | 7.377425 | 8.800670 | 13.545635 |
| Small (50/4) | GPU | 31.235805 | 32.754739 | 33.685718 |
| Medium (200/12) | CPU | 12.602796 | 18.140467 | 222.074834 |
| Medium (200/12) | GPU | 36.885022 | 40.452499 | 203.797758 |
| Large (500/30) | CPU | 25.308291 | 38.291954 | 665.141722 |
| Large (500/30) | GPU | 50.485880 | 56.468044 | 613.658792 |

- `initial_state_checksum`はSmall `ec0d54db7ffbbd2b`、Medium `e0d253e3ac15f363`、Large `feec763dd1218193`であり、各規模・描画条件の3反復で一致した。warm-up終端は実時間frame境界に依存するため、標準policyどおりrecordのみとした。
- post-captureの`CommandQueue has un-applied commands`はSmall CPU/GPUが各39件、Medium/Large matrixが合計1,318件だった。いずれも`PERF_CAPTURE: wrote`後の`Commands::delayed()` teardownであり、runnerが原文・件数をartifactへ保存している。frame-timeを変える強制flushは行わない。
- M0のframe-time baseline条件は満たした。fixed-step determinism audit、CPU toggle時のdraw/scene数、Tracy memory、RenderDoc、Gather以外の専用workloadは未完であり、M0全体を完了扱いにはしない。
- M1-A artifactはtask execution counterを含むschema v3である。M1-Bからreservation sync counterを加えた新規captureはschema v4となる。schema v2/v3 baselineとの比較は共通のframe-timeだけに限り、旧artifactにないcounterを0として扱わない。

### M1: Task execution の Changed 汚染と予約全再構築を解消

- **目的**: 最も波及範囲が広い不要 changed を止める。
- **primary metric**: idle Soulのtask context作成/不要Changed件数とreservation full rebuild/走査件数。
- **変更内容**:
  1. `TaskExecutionContext` 作成前に `AssignedTask::None` を早期除外する。
  2. `WorkingOn` はtask target despawn時にRelationship cleanupで先に消え、`AssignedTask::Some` が一時的に残り得るため、`With<WorkingOn>` をtask executionのfilterには使わない。`Some + Without<WorkingOn>` も従来どおりhandler/cleanupへ到達させる。
  3. `TaskExecutionContext` が全コンポーネントを即座に `&mut T` へ coercion しないよう、`Mut<T>` wrapperまたは用途別contextへ分割する。handlerが実際に書くフィールドだけを changed にする。
  4. `collect_active_reservation_ops` が参照するフィールドだけから、比較専用で `Eq` な `ReservationSignature` を正規化生成する。`sync_reservations_system` の root resource `ReservationSignatureCache` に前回値を保持し、progressだけが変化した場合は `active_dirty` にしない。
     - `AssignedTask`/Soul removal時は該当entryを削除する。
     - first runと安全監査full rebuild時はsignature mapも同じsnapshotから再構築する。
  5. assignment、予約対象/種別のphase遷移、abort、completion、handoff、pending task追加削除、`TaskWorkers`変更を予約dirty条件として列挙し、低頻度のfull rebuildを安全監査として残す。
  6. `AssignedTask` の直接書き換え箇所を監査し、予約意味を変える更新は共通helperまたはdirty通知を通す。
  7. `SharedResourceCache::reset()`が予約snapshotだけでなく`frame_stored_count`/`frame_picked_count`も消す現状を分離する。Familiar Perceive先頭で毎frame実行する`begin_frame()`はtransient差分だけをclearし、予約snapshot置換はreservation dirty/full audit時だけ行う。
  8. `ReservationSignature`は`hw_jobs::lifecycle`に置き、予約operationを生成する唯一の正規化関数と同じmatchから導出する。独立した意味定義を二重に持たない。
  9. signature mapはloadから直接reset可能なroot resourceに置き、`rebuild_transient_caches`で`ReservationSignatureCache`と`ReservationSyncTimer`をdefaultへ戻す。これにより次フレームはfirst-run rebuildとなり、旧worldのEntityを持ち越さない。

#### M1-A: idle task の早期除外と観測（2026-07-13）

- `TaskExecutionSoulQuery`のfilterは変えず、`TaskExecutionContext`を構築する前に`&AssignedTask`としてread-onlyで`None`を判定してcontinueする。これによりidle Soulの`DamnedSoul`、`AssignedTask`、`Destination`、`Path`、`Inventory`へ`&mut`を渡さない。
- `With<WorkingOn>`は追加しない。`AssignedTask::Some + Without<WorkingOn>`は従来どおりexpected-item検証・handler・cleanupへ進む。targetのdespawn後にRelationship cleanupだけが先行する既存の回復経路を変えない。
- profiling feature時だけ`task_execution_souls_queried`、`task_execution_idle_skips`、`task_execution_handler_runs`をcapture期間で集計し、`summary.csv` schema v3と`aggregate.csv`へ出力する。通常buildにはResource・counter更新を含めない。
- `TaskExecutionContext`の用途別分割は後続sub-milestoneとして保留する。reservation signature、cache reset分離、load resetはM1-Bで実施する。M1-Aの測定はLarge/CPUの正式baselineと同一条件で行い、frame-timeと新規work counterを併記する。

#### M1-A 実測（2026-07-13、Large / CPU）

- artifact: `target/perf-runs/m1a-idle-guard-large-cpu-20260713-a/`。M0のLarge/CPU baselineと同じseed、population、Vulkan adapter、window/present mode、30秒warm-up、60秒measureを3反復し、全runがvalidだった。`initial_state_checksum`は全runで`feec763dd1218193`、capture完了前のwarning/errorは0件である。
- `task_execution_souls_queried`の中央値は997,000（MAD 11,500）、`task_execution_idle_skips`は990,287（MAD 12,177）、`task_execution_handler_runs`は7,390（MAD 677）だった。counterはrunごとに比率を出してから集約し、idle skip比率は99.250127%（MAD 0.076553%）、handler到達比率は0.749873%（MAD 0.076553%）である。M0 schema v2にはこのcounterがないため、存在しないbefore値を推測・0埋めしない。

| 指標 | M0 baseline | M1-A | 差分 |
| --- | ---: | ---: | ---: |
| p50 ms | 25.308291 | 24.711394 | -2.359% |
| p95 ms | 38.291954 | 38.270375 | -0.056% |
| p99 ms | 665.141722 | 604.353435 | -9.139% |

- `scripts/perf.py compare --allow-case-subset`でLarge/CPUだけを正式matrixから安全に比較した。p50/p99の低下は観測されたが、p95の差は分散内であり、frame-time単独では過大解釈しない。採用根拠は、idleで5対象コンポーネントをmutable context化しない回帰testと、99%超のqueryが実際に早期除外されたwork counterである。
- post-capture teardown warningは`170;157;169`（合計496）で、既知の`Commands::delayed()`終了経路のみ。計測区間を変えるflushは行わない。

#### M1-B: reservation signature、cache split、観測（2026-07-14）

- `hw_jobs::lifecycle::ReservationSignature`は`collect_active_reservation_ops`と同じ正規化経路からactive operationだけを導出する。`Changed<AssignedTask>`（Addedを含む）のsignatureが同値ならprogress更新ではsnapshotを置換しない。completion、signatureが空になるphase遷移、assignment/Soul removalではdirtyにする。`RemovedComponents<AssignedTask>`はreaderを全件消費し、同一frameのassignment/despawnでも安全側で再構築する。
- signature mapはroot resource `ReservationSignatureCache`として保持し、`load::rebuild_transient_caches`が`SharedResourceCache`、signature cache、timerを同時にdefaultへ戻す。`Local + WorldEpoch`は採用しない。load直後のtimer初回実行が完全再構築を保証する。
- `SharedResourceCache::begin_frame()`は毎frameのpickup/store差分だけをclearし、`replace_reservation_snapshot()`はdirtyまたは定期監査時だけreservation mapを置換する。snapshot置換でframe-local差分を消さない。
- profiling feature時だけ`reservation_sync_full_rebuilds`、`reservation_sync_pending_tasks_scanned`、`reservation_sync_assigned_tasks_scanned`をcapture期間に集計し、schema v4の`summary.csv`と`aggregate.csv`へ出す。通常buildにはこのresourceと更新経路を含めない。schema v3以前にはreservation counterがないため、0埋めやcounter比較を行わない。
- regression testはprogress-only更新のsnapshot非置換、completion/removal、複数removal readerの全消費、profiling counterが実際に実行したsweepだけを数えることを確認する。3反復の30/60秒正式比較はschema v4 artifactで別途採取する。
- smoke artifact: `target/perf-runs/m1b-reservation-metrics-smoke-20260714-a/`。Small/CPU、warm-up 0.25秒、measure 0.5秒の1 runはvalidで、capture前warning/errorとteardown warningは0件だった。`reservation_sync_full_rebuilds=4`、`reservation_sync_pending_tasks_scanned=282`、`reservation_sync_assigned_tasks_scanned=200`を記録した。これはschema/counter経路の検証であり、frame-timeや削減率の正式比較には使わない。

- **変更ファイル**:
  - `crates/hw_soul_ai/src/soul_ai/helpers/query_types.rs`
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution_system.rs`
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/context/`
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/handler/`
  - `crates/hw_soul_ai/src/soul_ai/execute/task_assignment_apply.rs`
  - `crates/hw_soul_ai/src/soul_ai/execute/task_unassign_apply.rs`
  - `crates/hw_jobs/src/lifecycle.rs`
  - `crates/hw_logistics/src/resource_cache.rs`
  - `crates/bevy_app/src/plugins/startup/perf_scenario.rs`
  - `crates/bevy_app/src/systems/familiar_ai/mod.rs`
  - `crates/bevy_app/src/systems/familiar_ai/perceive/resource_sync.rs`
  - `crates/bevy_app/src/systems/save/load.rs`
  - `crates/hw_jobs/src/visual_sync/sync.rs`
  - `crates/hw_logistics/src/visual_sync.rs`
  - `scripts/perf.py`
  - `docs/performance-profiling.md`
  - `docs/tasks.md`
  - `docs/invariants.md`
- **期待効果**: 非常に高い。予約、UI、Visual Mirror、ECS scheduler の不要競合を同時に減らす。
- **完了条件**:
  - [x] `AssignedTask::None` の Soul で5対象コンポーネントのchanged件数が task execution 前後で増えない。
  - [x] `WorkingOn`なしのactive taskがqueryから除外されず、既存handler/cleanup経路へ進める。
  - [ ] active taskで未更新のコンポーネントが changed にならない。
  - [x] progress更新だけでは予約full rebuildが走らない。
  - [x] pickup/storeのframe-local差分が翌frameへ残らず、予約snapshot低頻度化後もlogical countを二重加算しない。
  - [ ] assignment / phase遷移 / abort / completion / handoffで予約内容が正しい。
  - [ ] load後に保存対象外の`AssignedTask`は`None`へ戻り、stale `WorkingOn`の除去、予約再構築、再割当が正しく行われる。
  - [ ] load後に`ReservationSignatureCache`を含むM1追加stateへ旧worldのEntityが0件である。
  - [ ] `OnTaskCompleted` と `TaskEndDisposition` の既存契約を維持した。
- **検証**:
  - task完了、中断、資材不一致、搬送handoff、active taskを持つ状態からのsave/load後cleanupと再割当を個別確認する。
  - M0のtask/changed/reservationカウンタを前後比較する。
  - `cargo check --workspace`
  - `cargo clippy --workspace`

### M2: 論理 root Transform と描画offsetを分離

- **目的**: visual animation が空間・経路・proxy同期をdirtyにしない構造へ変える。
- **primary metric**: 静止entityのlogical root `Changed<Transform>`件数と、それを起点にしたSpatial/proxy同期件数。
- **変更内容**:
  1. gathering / conversation / breakdown / idle 等、root `Transform` へ書く全animation systemを監査し、論理移動と見た目のoffsetを分類する。
  2. SoulはrootにSpriteを持たず、現行main/mask/shadow GLB同期はrootの2D translationだけを読み、scale/rotationを描画へ反映していない。したがって最初は表示consumerのないroot scale/rotation/bob書込を削除し、GLBへ新しいpulse/tiltを追加しない。監査で現行表示を担うconsumerが見つかった場合だけ、その最小offsetを専用visual componentへ移す。
  3. Familiarは`hw_visual::familiar`所有の`FamiliarVisualOffset`とowner linkを持つ2D visual childを追加し、Spriteをrootからchildへ移す。hover/pulse/tilt/flipはchildへ適用し、rootは論理translation/identity rotation/unit scaleと`Visibility::Inherited`を明示保持する。独立3D proxyは、現行root animation由来の見た目が存在する成分だけを同じ座標写像で再現し、新しい高さ/回転表現へ変更しない。
  4. `crates/hw_visual/src/soul/idle.rs`を含むidle/gathering visual systemを監査し、Soulの無効なroot writeを削除、Familiarの有効な見た目だけをvisual child経路へ移す。
  5. SpatialGridはroot translationだけを参照する。
  6. main/mask/shadow/Familiar proxy同期はowner cache経由で統合し、`Changed<Transform>`、存在する場合だけSoul visual state、`Changed<FamiliarVisualOffset>`のownerを更新する。camera billboard/pitch補正は既存式を維持する。
  7. Familiar root Spriteを読むcommand visualizationをowner link経由のchild Spriteへ変更する。range indicatorはhover差引を廃止してlogical rootをground anchorとして使い、aura自体をhoverさせない。
  8. 論理consumerがないことを監査で確認したvisual-only fieldに限り、load直後の旧save由来root rotation/scaleをidentity/unitへ正規化する。3D再表示、camera rotation/elevation変更時だけfull syncを許可し、保存形式にruntime visual offsetを追加しない。
- **変更ファイル**:
  - `crates/bevy_app/src/entities/damned_soul/spawn.rs`
  - `crates/bevy_app/src/entities/damned_soul/movement/animation.rs`
  - `crates/bevy_app/src/entities/familiar/animation.rs`
  - `crates/bevy_app/src/entities/familiar/spawn.rs`
  - `crates/bevy_app/src/entities/familiar/range_indicator.rs`
  - `crates/bevy_app/src/systems/command/visualization.rs`
  - `crates/hw_soul_ai/src/movement.rs`
  - `crates/hw_familiar_ai/src/movement.rs`
  - `crates/hw_spatial/src/soul.rs`
  - `crates/hw_spatial/src/familiar.rs`
  - `crates/bevy_app/src/systems/visual/character_proxy_3d.rs`
  - `crates/bevy_app/src/systems/visual/soul_animation.rs`
  - `crates/bevy_app/src/systems/save/rehydrate.rs`
  - `crates/hw_visual/src/soul/idle.rs`
  - `crates/hw_visual/src/soul/systems.rs`
  - `crates/hw_visual/src/lib.rs`
  - 新規 `crates/hw_visual/src/familiar.rs`
  - `crates/hw_visual/src/visual3d.rs`
  - `docs/rendering-performance.md`
- **期待効果**: 高い。静止entityが多い場面ほどSpatial/Visual処理を削減できる。
- **完了条件**:
  - [ ] 静止Soul/Familiarのroot translation/rotation/scaleがvisual animationで変化しない。
  - [ ] bob/hover/pulse/tilt/表情の見た目を維持した。
  - [ ] SpatialGridには論理位置だけが保存される。
  - [ ] Soulのmain/mask/shadow GLBの見た目が変更前と一致し、最適化を理由に新しいpulse/tiltを導入していない。
  - [ ] Familiarの2D child、3D proxy、command色、ground固定range indicatorがそれぞれ変更前の見た目を維持する。
  - [ ] Spriteを外したFamiliar rootも`Visibility::Inherited`と必要なhierarchy componentを保持し、Bevyのrequired-component/可視性エラーがない。
  - [ ] 3D ON/OFF、load、camera変更後にproxyが1フレーム以内に正しい位置へ同期する。
- **検証**:
  - 静止/移動/集会/睡眠/会話/ストレス中の2Dと3D表示を確認する。
  - M0のroot Changed件数、SpatialGrid更新件数、proxy書込件数を比較する。
  - `cargo check --workspace`
  - `cargo clippy --workspace`

### M3: Familiar 委譲周期・割当経路・候補構築を軽量化

- **目的**: Idle Familiar と大規模Designationで毎フレーム発生する委譲処理を抑える。
- **primary metric**: delegation cycle/s、snapshot build/s、candidate-worker評価数/s、Familiar stateの不要Changed件数。
- **実装単位**: 意味を変えないM3Aと、別途仕様決定が必要なM3Bを分離する。M3Aだけで本マイルストーンの性能完了判定を可能にする。

#### M3A: 周期・snapshot・cache・scratch

- **変更内容**:
  1. `allow_task_delegation || is_idle_command` を廃止し、Yard共有タスクを候補に含めることと実行周期を分離する。
  2. `familiar_task_delegation_system` はmovement/monitoringと `delta_secs` 更新も担当するため、system全体は毎フレーム維持する。`process_task_delegation_and_movement` を毎フレームのmovement/state更新と、0.5秒/dirty gate配下のtask delegationへ分離する。
  3. 通常委譲は0.5秒timer、workerがidleになった、command/TaskArea/ManagedTasksが変化した、候補indexがdirtyになった場合だけ即時wake-upする。
  4. `IncomingDeliverySnapshot` は現状も1 system invocationにつき一度で全Familiar共有されている。この共有構造は維持し、snapshot構築と `TaskManager::delegate_task` だけをdelegation cycle/dirty wake-up時へ限定する。Yard一覧とBuild例外候補も同じcycle snapshotへ統合する。
  5. candidate indexのgenerationを、SpatialGridだけでなくDesignation、TransportRequest、TaskWorkers、Priority、ManagedTasks、TaskArea、commandのAdded/Changed/Removedから明示更新する。60frame全消去等の時間依存invalidationはM4のtopology versionと各domain generationへ置換する。
  6. 旧CPU計画P4の残課題として、`CachedActiveFamiliars` / `CachedActiveYards`を毎フレーム`clear`/`extend`せず、ActiveCommandとTaskAreaのAdded/Changed/Removed、Familiar despawn、Yard lifecycleのgenerationが変わったcycleだけ再構築する。
  7. squad validationとmember `Vec` は `Changed<Commanding>`、recruit/uncommand、entity removal時に更新するcacheへ移す。
  8. candidate scoring用 `Vec` は `Local<Vec<_>>` 等のscratchを再利用し、Top-K処理で同じ候補のclone/sortを繰り返さない。これは旧P6の実装済み`top` allocation削減をやり直すものではない。
  9. Familiar側も毎フレームcontext化で`FamiliarAiState` / `Destination` / `Path`を不要にChangedへしないよう、mutable accessを実際の更新点まで遅延する。
  10. 既存`blueprint_auto_build_system`は意味を変えず、まず0.5秒/候補dirty gate配下へ移す。Build producer統合をM3Aの完了条件にしない。
  11. selector metrics のper-candidate Atomicを通常ビルドから除去する。
  12. squad、candidate、active Familiar/Yard等のEntity-bearing cacheは`WorldEpoch`を保持し、load後のepoch不一致で再構築する。

#### M3B: Build producer統合（仕様決定後のみ）

- 次を `docs/tasks.md` / `docs/familiar_ai.md` で正式契約として決める。
  - TaskArea外のBuildを候補へ含めるか。
  - Idle FamiliarがBuildを割り当てるか。
  - `ManagedBy`の有無とownershipをどう扱うか。
  - 複数FamiliarのTaskAreaが重なる場合のowner/priority。
- 契約決定まではproducerを削除しない。決定後、Familiar `TaskManager`へ契約を一元化し、同じBlueprint/worker requestのapply前dedupeを追加してから `blueprint_auto_build_system` を削除する。
- この統合はassignment semantics変更を含むため、M3A後の別コミット/正しさレビュー対象とし、未決定なら根拠付きskipを許可する。
- **変更ファイル**:
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_delegation.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/resources.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/squad.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/mod.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/task_finder/`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/delegation/assignment_loop.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/policy/haul/selector_metrics.rs`
  - `crates/hw_logistics/src/transport_request/producer/active_unit_cache.rs`
  - `crates/hw_soul_ai/src/soul_ai/decide/work/auto_build.rs`
  - `crates/hw_soul_ai/src/soul_ai/mod.rs`
  - `docs/familiar_ai.md`
  - `docs/tasks.md`
- **期待効果**: 高い。Idle Familiar数、worker数、Designation数の積で増える処理を周期/dirty単位へ抑える。
- **完了条件**:
  - [ ] dirty wake-upなしのdelegate cycleは1秒あたり2回以下である。
  - [ ] Familiarのmovement/monitoringはrender frame cadenceを維持し、delegation gateの影響を受けない。
  - [ ] Idle FamiliarがYard共有タスクを最大0.5秒以内に発見する。
  - [ ] cycle内で各snapshot/indexを1回だけ構築する。
  - [ ] `CachedActiveFamiliars` / `CachedActiveYards`がsteady-stateで再構築されない。
  - [ ] Familiar側の未変更state/path/destinationがdelegation systemだけを理由にChangedにならない。
  - [ ] load後にM3のsquad/candidate/active-unit cacheへ旧worldのEntityが0件である。
  - [ ] M3Bを実施した場合だけ、Build request producerが1系統で同じBlueprint/workerへの重複requestがなく、TaskArea/Idle/ManagedBy/owner契約が恒久ドキュメントとテストで固定されている。未実施時は理由と既存producerのgate化を記録する。
  - [ ] 複数FamiliarのTaskArea重複時もTaskSlots/WorkingOn契約を維持する。
- **検証**:
  - Idle、GatherResources、Yard共有、複数TaskArea、Build資材完了の各シナリオを確認する。
  - M0のdelegation/snapshot/candidate/A*カウンタを比較する。
  - `cargo check --workspace`
  - `cargo clippy --workspace`

### M4: Runtime経路探索予算・到達可能性cache・Door近傍処理

- **目的**: pathfindingのフレームスパイクを制御し、Door通常開閉による不要なcache無効化をなくす。
- **開始条件**: `runtime-correctness-contracts` M4のDoor/passability/topology version APIとテストがgreenであること。
- **primary metric**: runtime core A*/frame、boolean reachability A*/frame、最大defer frame、Door検査候補数。
- **変更内容**:

#### M4A: boolean到達可能性を連結成分cacheへ置換

1. `hw_world::pathfinding::connectivity`に、mapと同じdense sizeのcomponent ID配列と`obstacle_version`を持つ`WalkabilityConnectivityCache`を追加する。version不一致時に一度だけ全walkable cellをflood-fillし、同version中はO(1)で判定する。
2. flood-fillはA*と同じ斜め移動/corner-cutting規則を共有helperから使う。walkable targetは同一component、blocked targetは有効な隣接goalのいずれかがstart componentと一致するかで判定し、現行`allow_target_walkable`契約を保つ。
3. Familiar assignment、auto-gather、command assignmentのboolean用途をcacheへ置換する。command assignmentは一回限りのUI操作なので`Deferred`を導入せず、選択集合を同frameでatomicに確定する。
4. mapgen/testはunbudgeted A*を継続利用する。代表map、blocked endpoint、斜めcorner、Door Open/Closed/Lockedでcache結果と既存A*結果のparity testを作る。

#### M4B: waypoint生成A*の共通予算と公平性

5. 起動時map検証/unit testが使う`pub(crate)` unbudgeted core APIと、runtime crateへ公開するbudgeted facadeを分離する。runtime facadeは`PathSearchResult<T> { Found(T), Unreachable, Deferred }`を返し、budget不足を到達不能と区別する。
6. `PathBudgetResetSet`を`PreUpdate`に置き、Logic/Actorを含む全runtime A* callerより前に一度だけresetする。`plugins/logic.rs`のsystem ordering testで、command assignmentを含む全callerがreset後であることを固定する。
7. request classを少なくとも`ActiveTask` / `EscapeOrBlocked` / `IdleOrRest`へ分け、予約枠を持たせる。同class内もpersistent FIFOまたはround-robin cursorで公平にし、caller/candidate indexを保持してDeferred位置から再開する。class/caller別最大defer frameの上限をworkloadごとに定義する。
8. hard limitはcore A* invocation数とする。direct失敗後のadjacent探索もcore境界で実呼び出しごとに課金し、expanded nodesは観測値に留める。blocked target/footprintの複数goalはmulti-goal A*一回へ寄せる。
9. `pathfinding_system`のlocal counterを共通budgetへ置換する。task execution context/handler、path cache、bucket routing、fallback、escape等を全列挙し、`Deferred`を最上位callerまで維持する。task handlerはDeferred時にphase、予約、Destination、Path、dispositionを変更せず、次frameへ再投入する。
10. Destination変更、path消費、cooldown終了、topology version変更時に`NeedsPath`相当のmarker/queueへ登録し、`for prioritize_tasks in [true, false]`による全Soul二重走査を置換する。版一致時のpath再検証skipは既存実装として維持する。
11. `hw_world`外からunbudgeted route APIをimportできないことをcompile境界で確認し、runtime caller別core A*合計とbudget消費が一致するtestを追加する。
12. persistent FIFO/round-robin queueとcaller再開cursorは`WorldEpoch`不一致時に全clearし、load前requestを新worldへ適用しない。

#### M4C: Door近傍処理

13. Door open/close判定はDoorごとの全Soul/waypoint走査から、Soul用`SpatialGrid`の近傍候補またはmovement側Door requestへ置換する。close時は接触cellの直接確認を残す。
14. 本計画は正しさM4が提供する`obstacle_version`をcache/path invalidationに利用するだけとし、`sync_door_passability`、`set_door_state`、`is_walkable`のmutation契約を変更しない。`WorldMap::is_changed()`や60frame TTLでcacheを全消去しない。
- **変更ファイル**:
  - `crates/hw_world/src/pathfinding/`
  - 新規 `crates/hw_world/src/pathfinding/connectivity.rs`
  - `crates/hw_world/src/door_systems.rs`
  - `crates/hw_soul_ai/src/soul_ai/pathfinding/system.rs`
  - `crates/hw_soul_ai/src/soul_ai/pathfinding/reuse.rs`
  - `crates/hw_soul_ai/src/soul_ai/pathfinding/fallback.rs`
  - `crates/hw_soul_ai/src/soul_ai/decide/escaping.rs`
  - `crates/hw_soul_ai/src/soul_ai/perceive/escaping.rs`
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution_system.rs`
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/context/execution.rs`
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/path_cache.rs`
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/bucket_transport/routing.rs`
  - navigationを行う`crates/hw_soul_ai/src/soul_ai/execute/task_execution/`配下のcaller
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/delegation/assignment_loop.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/auto_gather_for_blueprint/helpers.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/auto_gather_for_blueprint/actions.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/blueprint_auto_gather.rs`
  - `crates/bevy_app/src/systems/command/assign_task.rs`
  - `crates/bevy_app/src/plugins/logic.rs`
  - `crates/bevy_app/src/world/mod.rs`
  - `crates/hw_spatial/src/soul.rs`
  - `docs/soul_ai.md`
  - `docs/familiar_ai.md`
  - `docs/invariants.md`
- **期待効果**: 高い。平均よりp95/p99 spikeと大規模Door/AI場面に効く。
- **完了条件**:
  - [ ] runtime caller別core A*の合計が設定budgetを超えず、mapgen/testはunbudgeted coreを利用できる。
  - [ ] budget deferを到達不能/タスク失敗として扱わない。
  - [ ] Familiar/auto-gather/command assignmentのboolean判定がruntime A*を0回しか呼ばず、A* parity testを満たす。
  - [ ] 同class内を含む最大defer frameがworkloadごとの定義上限以内で、Deferredがtask abort/phase変更へ流れない。
  - [ ] load後にbudget queue/restart cursorへ旧worldのEntity/requestが0件である。
  - [ ] Open/Closedでtopology versionと既存path validityが変わらない。
  - [ ] Locked、建物追加、地形変更後は正しさ計画のversion契約に従って既存pathとconnectivity cacheを再検証する。
  - [ ] Doorが調べるSoul/waypoint数が全Soul数に比例しない。
- **検証**:
  - Door開閉、Locked切替、Door削除、建物でpath遮断、複数Soul同時再探索を確認する。
  - A*回数、expanded nodes、defer待ち時間、path再利用率、Door検査数を比較する。
  - `cargo check --workspace`
  - `cargo clippy --workspace`

### M5: Construction phase・completion・curingを変更駆動化

- **目的**: 建設siteごとの毎フレーム全tile走査と、curing中の毎フレーム全Soul走査をなくす。
- **開始条件**: `runtime-correctness-contracts` M4と`save-load-hardening` M4が完了し、load reset/rehydrate登録順が固定されていること。
- **primary metric**: site/tile/evacuation走査数/frameとconstruction completion system時間。
- **変更内容**:
  1. Floor / Wall siteの既存 `tiles_reinforced` / `tiles_poured` / `tiles_framed` / `tiles_coated` をphase遷移の第一判定に使用する。
  2. counterはtile taskの成果適用時に増えるため、read-onlyな `Changed<FloorConstructionSite>` / `Changed<WallConstructionSite>` queryで閾値へ到達したsiteだけを判定し、選ばれたsiteだけをapply側でmutable取得する。判定のための`DerefMut`でsiteを再dirtyにしない。`TaskEndDisposition::Completed` のframe境界へ依存する新messageは追加しない。
  3. tile列挙が必要なphase遷移時だけ `TileSiteIndex` を使い、site × 全tile queryを廃止する。Wallは既存順序どおり `wall_framed_tile_spawn_system` 後に判定し、`tiles_framed == tiles_total` へ到達した一度だけindexed tileの `spawned_wall` を検証する。
     - counter閾値へ到達した遷移候補はrelease buildでも`tiles_total > 0`、indexed tile数=`tiles_total`、対象工程以上のstate rankが全tileで成立することを確認する。不一致時はerrorを記録して遷移せず、安全監査/rehydrate helperで修復する。
  4. Floor completionを次のsystemへ分ける。
     - `begin_floor_curing_system`: `tiles_poured == tiles_total` へ到達したsiteをCuringへ移し、indexed tileからephemeralな `CuringFootprint { grids }` と安全監査timerを構築する。
     - `tick_floor_curing_system`: Curing siteだけのtimerを進め、完了時だけindexed tileを列挙する。
  5. Soul evacuationはcuring開始時にSoul用`SpatialGrid`候補へ実行し、その後は0.5秒周期の安全監査だけを行う。WorldMapでblockedなtileへ通常pathが入らないことを前提に、全Soul毎フレーム走査を廃止する。
  6. `CuringFootprint`はruntime cacheとして保存しない。loadのexclusive apply後、Spatial/Logic再開前に次の同期手順をpost-load registryへ登録する。
     1. deserialize済みの全Floor/Wall tileから`TileSiteIndex`を同期的に再構築する。
     2. tile stateの工程rankからsite counterを再計算する。例としてFloorの`WaitingMud`以降はreinforced済み、Wallの`WaitingMud`以降はframed済みと数え、現在phaseより前の成果を0へ戻さない。
     3. `phase == Curing`のsiteだけにindexed tileから`CuringFootprint`を再生成する。
     保存済み`WorldMap`を正本とし、この手順ではobstacle/occupancyを再reserveしない。
  7. 通常phase遷移時もcounterとindexed tile stateの工程rank一致をdebug assertし、cancel/rebuildを含むcounter/index更新を同じhelperで行う。
- **変更ファイル**:
  - `crates/hw_jobs/src/construction.rs`
  - `crates/bevy_app/src/systems/jobs/floor_construction/completion.rs`
  - `crates/bevy_app/src/systems/jobs/floor_construction/mod.rs`
  - `crates/bevy_app/src/systems/jobs/wall_construction/completion.rs`
  - `crates/bevy_app/src/systems/jobs/wall_construction/phase_transition.rs`
  - `crates/hw_logistics/src/tile_index.rs`
  - `crates/hw_spatial/src/soul.rs`
  - `crates/bevy_app/src/systems/save/rehydrate.rs`
  - `crates/bevy_app/src/systems/save/load.rs`
  - `docs/tasks.md`
  - `docs/invariants.md`
  - `docs/save_load.md`
- **期待効果**: 建設中tile数とSoul数が多い場面で高い。通常場面では中程度。
- **完了条件**:
  - [ ] counter未達siteでtile queryを実行しない。
  - [ ] siteがphase遷移するframe以外は、他siteのtileを列挙しない。
  - [ ] release buildでもindex件数/state rank不一致のsiteをphase遷移させない。
  - [ ] curing中のevacuationが開始時 + 0.5秒安全監査だけで実行される。
  - [ ] save/load途中の建設siteがSpatial/Logic再開前に正しいindex/phase/counter/`CuringFootprint`へ復元され、保存済みWorldMapへfootprintを二重適用しない。
  - [ ] cancel/rebuild後にcounter、TileSiteIndex、WorldMap occupancyが一致する。
- **検証**:
  - 大面積Floor/Wall、各phase途中save/load、curing中侵入、cancel/rebuildを自動testと手動scenarioで確認する。
  - site/tile/Soul走査数と対象system時間を比較する。
  - `cargo check --workspace`
  - `cargo clippy --workspace`

### M6: Vitals・idle decision・energyを明示周期へ分離

- **目的**: movement以外の全Soul simulationと安全監査を、時間積分と既存phase順を維持したまま低頻度/dirty駆動へ移す。
- **primary metric**: slow simulation対象Soul更新数/s、idle decision数/s、energy output/grid全件再計算数/sと各system時間。
- **実装単位**: M6A、M6B、M6Cを別コミット/計測単位とし、事前宣言したprimary metricで個別に採否を決める。

#### M6A: Vitals 10Hz accumulator

- `SoulAiSystemSet::Update`内に`SlowSimulationClock`を置き、`Time<Virtual>`を0.1秒stepへ蓄積する。現段階では`FixedUpdate`へ移さず、既存Familiar→Soul順、Logic pause条件を維持する。clock systemを全slow consumerより前へ明示orderし、同frameの`steps_this_frame`は全consumerがread-onlyで共有する。
- 1 render frameあたり最大5 stepを処理し、超過分は捨てずにaccumulatorへ保持する。pause中は加算せず、unpause時にwall timeをcatch-upしない。
- fatigue / dream / stress / familiar influence / rest area効果 / Dream移送 / `RestAreaCooldown`を同じstep契約へ統合する。複数systemが各自5stepをまとめて処理して順序を変えないよう、per-stepの純粋helperを一つのslow Soul driverから`fatigue → influence/rest → dream/stress → threshold/message`の順で呼ぶ。movementは毎フレームのままにする。
- Commands適用前に最大5step進んでも、同じSoulのrest退出・`OnStressBreakdown`等のone-shotを1 render frameで重複発行しない。状態を即時更新するかframe-local setでdedupeする。
- 変更前実装を120 FPSで60 virtual秒進めたsnapshotをreferenceとし、30/60/120 FPSおよび1x/2x/3xで比較する。normalized値（motivation/fatigue/stress/laziness）は絶対差0.001以内、dream/DreamPoolは0.05以内、idle/cooldown timerとthreshold遷移時刻は0.1 virtual秒以内を合格条件にする。

#### M6B: Idle decisionとsanity audit

- `idle_behavior_decision_system`はtask/移動不可判定を空間検索より先に行う。
- 現行system自身が`Time::delta_secs()`で`total_idle_time`/`idle_timer`を進めるため、timer advanceをdecision evaluationから分離する。周期tickだけが`dt = 0.1`を積算し、task終了・目的地到達・rest state変更による`NeedsIdleDecision`の即時wake-upは`dt = 0`で再評価だけを行う。
- `clock advance → slow integration → one-shot event/message emission → Decide`のorderingを固定し、backlog中にtask/restが変化しても旧状態へ余分なstepを適用しない。
- `ensure_rest_area_component_system` 等はAdded/Changed/Removed主体にし、full invariant auditは1秒周期とする。

#### M6C: Energy更新

- `lamp_buff_system`は10HzでSoul用`SpatialGrid`からLamp近傍Soulだけを処理する。
- `lamp_buff_system`は`SlowSimulationClock.steps_this_frame`をread-onlyで共有し、独立timerでrender deltaを失わない。slow vitalsのstep反映後、energy grid/`Unpowered`確定後の定義済み順序でbuffを適用する。
- energy grid再計算はgenerator/consumer Added/Changed/Removedと`GeneratesFor`/`ConsumesFrom` relationship変更でdirtyにする。
- 現行`power_output`は同値writeを既に避けるため、この挙動を維持したうえで計算自体をdirty gateする。dirty sourceはSoulSpaTileの`Changed<TaskWorkers>`、`Changed<SoulSpaSite>`/phase、`Changed<Children>`/tile追加削除、`PowerGenerator.output_per_soul`変更、load後初回再構築を含む。
- `relationship/Children反映 → ApplyDeferred → output計算 → grid dirty化 → grid再計算 → Unpowered反映`のorderingを`plugins/logic.rs`で固定する。grid lifecycleとpower outputを同じdirty契約へ統合し、steady-stateで全SoulSpa/Children/generator/consumerを走査しない。

- **変更ファイル**:
  - `crates/hw_soul_ai/src/soul_ai/mod.rs`
  - `crates/hw_soul_ai/src/soul_ai/update/vitals_update.rs`
  - `crates/hw_soul_ai/src/soul_ai/update/dream_update.rs`
  - `crates/hw_soul_ai/src/soul_ai/update/vitals_influence.rs`
  - `crates/hw_soul_ai/src/soul_ai/update/rest_area_update.rs`
  - `crates/hw_soul_ai/src/soul_ai/update/state_sanity.rs`
  - `crates/hw_soul_ai/src/soul_ai/decide/idle_behavior/system.rs`
  - `crates/bevy_app/src/systems/energy/lamp_buff.rs`
  - `crates/bevy_app/src/systems/energy/grid_recalc.rs`
  - `crates/bevy_app/src/systems/energy/grid_lifecycle.rs`
  - `crates/bevy_app/src/systems/energy/power_output.rs`
  - `crates/bevy_app/src/plugins/logic.rs`
  - `docs/soul_ai.md`
  - `docs/soul_energy.md`
  - `docs/architecture.md`
- **期待効果**: Soul/Lamp/energy entity数が多い場面で中～高。
- **完了条件**:
  - [ ] M6Aの数値/遷移時刻誤差が定義値以内である。
  - [ ] pause/unpauseでaccumulatorが不正にcatch-upせず、長いframeでも経過時間を捨てない。
  - [ ] 最大5stepのframeでもone-shot退出/breakdown通知を同じSoulへ重複発行しない。
  - [ ] idle task終了/目的地到達は次のrender frameまたは次の10Hz stepでdecisionされる。
  - [ ] sanity full auditは1秒あたり1回以下で、Added/Changed/Removedは同frameに反映する。
  - [ ] steady-state energy output計算とgrid再計算回数が0で、関係/出力変更時は同frameまたは次のLogic frameに再計算する。
- **検証**:
  - 30/60/120 FPS、1x/2x/3x、pause/unpause、長い1frame/backlog、sleep/rest/stress/familiar influence、Lamp追加削除、SoulSpa worker/phase/child、generator/consumer関係変更を自動testする。
  - vitals数値、decision latency、audit/grid rebuild回数、対象system時間を比較する。
  - `cargo check --workspace`
  - `cargo clippy --workspace`

### M7: UI・Blueprint・RTT同期の対象限定

- **目的**: 表示内容が変わらないフレームのentity生成、format、asset mutation、Transform書込を止める。
- **primary metric**: ViewModel build、UI/indicator spawn-despawn、Blueprint root Transform write、asset mutation、inspection model buildの各回数/frame。
- **実装単位**: M7A～M7Dを別コミット/計測単位とする。

#### M7A: Blueprint progress bar

- Blueprint親へprogress bar所有markerまたはlinkを付け、`Without<ProgressBar>` + 全bar `.any()` を廃止する。
- progress barは子のlocal Transform継承を利用し、定数位置の毎フレーム再同期を削除する。cleanupはRemovedComponentsまたはowner linkで行い、全BlueprintのHashSetを毎フレーム構築しない。
- visual color/scale/fillは`Changed<BlueprintVisualState>`または前回値差分があるownerだけ更新する。Building中のpulseだけactive marker付きvisual childで毎フレーム動かし、logical rootのscaleを書かない。rootを残す場合も同値比較後だけwriteする。

#### M7B: Entity List 100ms cadence

- structure dirtyを即時、value dirtyを`Time<Real>`基準のlatched 100 ms cadenceで処理し、pause/game speedの影響を受けないようにする。
- dirty entity IDを保持し、ViewModelをentity indexで更新してvisible rowの全件format/sortを避ける。
- RemovedComponents readerの完全消費は`runtime-correctness-contracts` M1の完了を前提とし、修正を重複させない。

#### M7C: Area/TaskArea indicator・RTT・inspection model

- area edit handleはSprite 9個をselection/area edit開始時に生成して保持し、area変更時は位置だけ更新、終了時にcleanupする。毎フレームdespawn/spawnしない。
- TaskArea indicatorはFamiliarごとの全探索をowner indexへ置換し、Rectangle meshを共有する。material/Transform/VisibilityはAdded/Changed<TaskArea/Familiar>/Removedとselection等の表示条件変更時だけ更新する。
- RTT composite material/quadはcamera、projection、window、performance toggle変更時、または前回値との差分がある場合だけ更新する。
- Info Panel / TooltipはEntity keyed cache、またはselected/pinned/hoverを分けた複数slotのinspection snapshotを利用する。selection/hover変更は即時、component変更 + `Time<Real>`の10Hz fallbackでmodelを更新し、異なるselected/hover entityを同一snapshotで上書きしない。
- Entity Listのdirty ID、TaskArea owner index、inspection snapshot等のEntity-bearing stateは`WorldEpoch`不一致でclear/rebuildし、load前Entityを保持しない。

#### M7D: Dream material/particle（計測条件付き）

- WGSLで未使用の `velocity_dir` を一次確認し、未使用ならRust/WGSLのuniform layoutを同時に更新して512 bucketから64 bucketへ削減する。
- particle mergeがM7A～M7C後もUI上位コストとして残る場合だけ、`Local<Vec<_>>`再利用、entity pool、screen-space cellを導入する。
- layout変更後はゲームまたは`visual_test`を起動し、shader pipeline validation errorがないことと変更前後の見た目を確認する。particle merge最適化はprimary metricが上位でなければ根拠付きskipを許可する。
- **変更ファイル**:
  - `crates/hw_visual/src/blueprint/progress_bar.rs`
  - `crates/hw_visual/src/blueprint/mod.rs`
  - `crates/bevy_app/src/interface/ui/list/change_detection.rs`
  - `crates/bevy_app/src/interface/ui/list/view_model.rs`
  - `crates/bevy_app/src/interface/ui/list/sync.rs`
  - `crates/bevy_app/src/interface/ui/plugins/entity_list.rs`
  - `crates/hw_ui/src/list/dirty.rs`
  - `crates/hw_ui/src/list/models.rs`
  - `crates/bevy_app/src/systems/command/indicators.rs`
  - `crates/bevy_app/src/plugins/startup/rtt_composite.rs`
  - `crates/bevy_app/src/interface/ui/presentation/`
  - `crates/bevy_app/src/interface/ui/plugins/info_panel.rs`
  - `crates/bevy_app/src/interface/ui/plugins/tooltip.rs`
  - `crates/bevy_app/src/interface/ui/interaction/tooltip/`
  - `crates/hw_ui/src/panels/info_panel/`
  - `crates/hw_ui/src/panels/tooltip_builder/`
  - `crates/hw_ui/src/interaction/tooltip/`
  - `crates/hw_ui/src/models/inspection.rs`
  - `crates/hw_visual/src/dream/dream_bubble_material.rs`
  - `crates/hw_visual/src/dream/mod.rs`
  - `crates/hw_visual/src/dream/ui_handles.rs`
  - `crates/hw_visual/src/dream/ui_particle/merge.rs`
  - `crates/hw_visual/src/dream/ui_particle/update.rs`
  - `crates/hw_visual/src/dream/ui_particle/update/update_standard.rs`
  - `crates/hw_visual/src/dream/ui_particle/update/update_trail.rs`
  - `crates/hw_visual/src/dream/ui_particle/trail.rs`
  - `assets/shaders/dream_bubble_ui.wgsl`
  - `docs/entity_list_ui.md`
  - `docs/dream-visual.md`
  - `docs/rendering-performance.md`
- **期待効果**: 中～高。UI展開時、Blueprint多数、3D表示時に効く。
- **完了条件**:
  - [ ] Blueprint数に関係なく、既存bar確認が親1件あたりO(1)である。
  - [ ] 未変更Blueprintのroot Transform/Sprite/bar fillを書かず、pulseはactive visual childだけを更新する。
  - [ ] area edit中にhandleのspawn/despawnが毎フレーム発生しない。
  - [ ] 未変更TaskAreaのmesh/material/Transform/Visibilityを書かない。
  - [ ] Entity Listのvalue ViewModel buildはdirty中でも最大10Hzである。
  - [ ] entity追加削除、検索、sort、drag/dropは即時反映する。
  - [ ] 静止cameraでRTT composite asset/quadが毎フレームchangedにならない。
  - [ ] Dream material layout変更前後で見た目が一致する。
  - [ ] shader pipeline validation errorがない。
  - [ ] load後にM7のdirty ID/owner index/inspection cacheへ旧worldのEntityが0件である。
  - [ ] M7A～M7Dを個別計測し、各sub-milestoneを実施または根拠付きskipとして記録した。
- **検証**:
  - Entity List展開/折りたたみ/検索/drag、Blueprint大量配置、area resize、3D toggleを確認する。
  - ViewModel build、allocation、spawn/despawn、AssetEvent、Transform書込数を比較する。
  - `cargo check --workspace`
  - `cargo clippy --workspace`

### M8: GPU律速時のみ shadow・mask・assetを最適化

- **開始条件**:
  - M0でmain/mask/shadow proxyまで完全に切れることを確認済みのRender3d/terrain/mask toggleによりframe timeが明確に改善し、GPU passが律速または上位コストと確認できた場合だけ着手する。
- **primary metric**: 固定DirectionalLight本数でのGPU pass時間、draw/scene entity数、shader静的sample数。
- **変更内容**:
  1. 現行`shadow_style.wgsl`はshadow有効なDirectionalLightごとにouter 9 + inner 9 sampleを行う。各kernelのbudgetをHigh 9 / Medium 4 / Low 1（合計18/8/2 sample per light）として品質/LOD別に段階化し、計測時のshadow有効DirectionalLight本数を固定する。
  2. Soul projector最大12件もlightごとに評価されるため、遠景LODでは無効化または上限を減らす。
  3. low-resolution shadow/projector mask passの方が安いか比較し、shader内の多点sampleより改善する場合だけ採用する。
  4. Soul mask/shadow用の同一GLB sceneを簡易capsule/impostorへ置換する試作を行う。
  5. mask RTTをscene RTTとは独立に1/2、1/4解像度で比較する。現行`RttRuntime`の共有viewportをscene/mask別へ分離し、compositeへmask側pixel sizeを渡し、window/quality変更時に両targetを再生成する。sceneごと縮小する既存Low presetをmask単独比較として扱わない。
  6. 明示ロードされる1024px spriteを使用サイズとズーム上限で棚卸しし、VRAM/起動時間が問題の場合だけ128～256px化、atlas、KTX2/Basisを別PRで行う。KTX2/Basisを採用する場合は `bevy/ktx2` と選択した `bevy/zstd_rust` または `bevy/basis-universal` feature、変換script、元asset保持方針を同じPRへ含める。
- **変更ファイル**:
  - `assets/shaders/shadow_style.wgsl`
  - `assets/shaders/rtt_composite_material.wgsl`
  - `assets/shaders/terrain_surface_material.wgsl`
  - `assets/shaders/terrain_surface_material_lod1_lite.wgsl`
  - `assets/shaders/terrain_surface_material_lod2.wgsl`
  - `assets/shaders/section_material.wgsl`
  - `crates/hw_visual/src/material/terrain_surface_material.rs`
  - `crates/hw_visual/src/material/section_material.rs`
  - `crates/bevy_app/src/systems/visual/terrain_lod.rs`
  - `Cargo.toml`
  - `crates/hw_core/src/constants/render.rs`
  - `crates/bevy_app/src/entities/damned_soul/spawn.rs`
  - `crates/bevy_app/src/plugins/startup/rtt_setup.rs`
  - `crates/bevy_app/src/plugins/startup/rtt_composite.rs`
  - `crates/bevy_app/src/plugins/startup/asset_catalog.rs`
  - `crates/bevy_app/src/plugins/startup/visual_handles.rs`
  - `crates/visual_test/src/types.rs`
  - `crates/visual_test/src/setup.rs`
  - `crates/visual_test/src/systems.rs`
  - 対象 `assets/textures/`
  - `docs/rendering-performance.md`
- **期待効果**: GPU律速時に高い。CPU律速時は着手しない。
- **完了条件**:
  - [ ] 固定camera/DirectionalLight本数でGPU pass時間、shader静的sample数、draw/scene entity数を比較した。
  - [ ] LOD境界、Soul輪郭、shadowの見た目に許容できない劣化がない。
  - [ ] CPU frame timeを悪化させていない。
- **検証**:
  - terrain/mask/shadow toggle、50/100/200 Soul、各LODで比較する。
  - visual test sceneとゲーム本体の両方でスクリーンショット/動画比較を行う。
  - `cargo check --workspace`
  - `cargo clippy --workspace`

### M9: 恒久ドキュメント同期と総合回帰確認

- **変更内容**:
  - 実装した契約を `docs/invariants.md`、`docs/tasks.md`、`docs/familiar_ai.md`、`docs/soul_ai.md`、`docs/soul_energy.md`、`docs/entity_list_ui.md`、`docs/rendering-performance.md`、`docs/performance-profiling.md`、`docs/save_load.md`、`docs/architecture.md`、`docs/building.md`、`docs/DEVELOPMENT.md` へ反映する。M8のvisual test手順を変更した場合は`docs/visual_test.md`も同期する。
  - `performance-cpu-2026-04-16.md`は本レビューで`Superseded`としたため、M3Aへ移したP4残課題の最終結果だけを照合し、必要なら本計画と同時にarchiveする。
  - 本計画の結果欄、最終計測値、未実施の条件付き項目を更新する。
  - 正しさ/save-load/structural計画と共有する契約・未達項目を相互照合し、同じ機能を複数計画で完了扱いにしない。
  - 完了後は本計画を `docs/plans/archive/`へ移し、`python scripts/update_docs_index.py`を実行する。archiveはgitignore対象なので、commitを依頼された場合は`git status --short`でrename/addを確認し、必要な計画だけ`git add -f docs/plans/archive/<file>`でstageする。
- **変更ファイル**:
  - 上記恒久ドキュメント
  - `docs/plans/performance-cpu-2026-04-16.md`
  - 本計画書
  - `docs/plans/README.md`
- **期待効果**: 後続実装で不要changedやversion更新を再導入するリスクを減らす。
- **完了条件**:
  - [ ] 実装と恒久ドキュメントの契約が一致する。
  - [ ] M0と同条件で最終計測を実施した。
  - [ ] 未達マイルストーンを「完了」と記載していない。
  - [ ] 計画索引を再生成した。
  - [ ] archive移動をcommitする場合、gitignore下の移動先がstage済みで対象外plan/docを含まない。
- **検証**:
  - `python scripts/update_docs_index.py`
  - 変更したRustファイルのrustfmt確認。既存workspace全体のformat baselineがgreenになった後は `cargo fmt --all -- --check`。
  - `cargo check --workspace`
  - `cargo clippy --workspace`
  - `cargo test --workspace`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `WorkingOn` targetのdespawnでrelationshipだけ先に消える | `AssignedTask::Some` のcleanupが実行されない | `With<WorkingOn>` filterは使わず、`AssignedTask::None`の値による早期除外だけを使う。`Some + Without<WorkingOn>`回帰テストを追加する。 |
| 予約signatureの項目漏れ | 二重予約または予約不足 | `collect_active_reservation_ops` を唯一の正規化元にし、assignment/phase/abort/completion/handoff/loadの表形式テストを作る。 |
| visual分離でselection、z-order、3D位置がずれる | 見た目・操作回帰 | root collider/selection座標を維持する。Soul GLBへ新offsetを足さず、Familiarだけ現行表示をvisual childへ移す。main/mask/shadow/command色/auraを固定sceneで比較する。 |
| delegation周期修正でタスク割当が遅れる | idle時間増加 | 0.5秒上限を仕様として維持し、worker-idle/command/candidate dirtyの即時wake-upを用意する。 |
| A* budget不足を失敗扱いする | task abort、handoff消失 | `Deferred` を `Unreachable` と別状態にし、次フレームへ再投入する。 |
| 同classの先頭callerがA*予算を独占する | 後続Soulが永久Deferredになる | persistent FIFO/round-robinと再開cursorを持ち、class/caller別最大defer frameをtestする。 |
| `obstacle_version`契約が性能変更で再び拡張される | cache staleまたは開閉ごとの全invalidate | runtime M4を唯一のmutation ownerとし、cost世代が必要なら`path_cost_version`へ分離する。性能計画からDoor APIを上書きしない。 |
| construction counterがsave/load後にdriftする | phase遷移停止または早期完了 | load中にindex→工程rank counter→footprintの順で同期再構築し、保存済みWorldMapへ再reserveしない。 |
| 低頻度化で時間積分がFPS依存、またはone-shotが重複する | vitals/balance・通知変化 | Update内0.1秒accumulator、per-step順序、frame内dedupeを固定し、30/60/120 FPS・速度倍率・長frameを自動testする。 |
| Entity List gateでstructure反映まで遅れる | 操作対象が見えない | structure dirtyは即時、value dirtyのみ100ms gateとする。 |
| feature-gated計測自体がhot pathを歪める | 誤判定 | 各crateまでfeatureを伝播し、release通常buildとprofiling buildを両方測る。hot loopではlocal集計後に一度だけresourceへ反映する。 |
| perf seedがworldgenより後に適用される/未seed randomが残る | 実行ごとにmap/workloadが変わる | Plugin登録前configをmaster seedとしsubstreamへ分配し、ゲーム更新前のinitial fixture checksumを同seed3回で必須一致にする。warm-up状態の完全一致はfixed-step auditで別途検証する。 |
| shader sample削減・mask簡略化で品質が落ちる | visual回帰 | M8を条件付きにし、品質tierとスクリーンショット比較、即時rollback可能な独立PRにする。 |

## 7. 検証計画

### 必須静的検証

- 変更したRustファイルをrustfmtし、差分を確認する。
- `cargo fmt --all -- --check` は既存workspace baselineがgreenになった後に必須化する。既存失敗が残る間は、対象外の広範なformat変更を混ぜない。
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`（`runtime-correctness-contracts` M0完了後の共通gate）
- `cargo test --workspace`
- rust-analyzer diagnostics 0件。

本計画では自動testを任意にしない。少なくともM1 task/reservation lifecycle、M2 root/proxy初期化、M3 delegation cadence/dirty、M4 connectivity parity/budget defer、M5 load rehydrate、M6数値積分/one-shot、M7 ownership/latchの回帰testを各sub-milestoneの成果物に含める。

### 手動確認シナリオ

1. **Task lifecycle**: assignment、通常完了、資材不一致、abort、retryable handoff、対象despawn、save/load。
2. **Familiar**: Idle/Yard共有、GatherResources、TaskArea重複、Build、複数Familiar/worker。
3. **Path/Door**: Open/Closed/Locked、Door追加削除、建物で既存path遮断、予算超過時のdefer。
4. **Construction**: 大面積Floor/Wall、各phase、curing、Soul evacuation、cancel、途中save/load。
5. **Vitals**: 30/60/120 FPS、1x/2x/3x、pause/unpause、長frame、sleep/rest/stress/familiar influence。
6. **Visual/UI**: 静止/移動/bob/hover、2D/3D切替、Entity List検索/drag、Blueprint progress、area edit、Dream particle。
7. **GPU**: terrain LOD、section、shadow、mask RTT、50/100/200 Soul。

### パフォーマンス確認

- 標準コマンド例:

  ```bash
  HW_PRESENT_MODE=novsync cargo run --profile profiling -p bevy_app@0.1.0 --no-default-features --features profiling -- --spawn-souls 200 --spawn-familiars 12 --perf-scenario --perf-seed 20260712 --perf-workload gather --perf-render cpu
  ```

- `scripts/perf.py run`で30秒warm-up後に60秒採取し、最低3回実行して中央値を採用する。実時間baselineはinitial fixtureを必須一致、warm-up終端は`record`する。
- 記録項目:
  - frame time p50 / p95 / p99 / max。
  - system別 CPU time。
  - allocation/frame。
  - M0で定義したdomain counter。
  - GPU pass time、draw/scene entity数、shader静的sample数（M8のみ）。
- 各sub-milestoneを単独で比較し、実装前にprimary metricと期待方向を記録する。
- 3回のbaseline分散を超えてprimary work counterまたは対象system/GPU pass時間が改善した場合に採用する。frame timeがノイズ内でも決定的work counterが意図どおり減れば採用でき、counterが不変でもGPU/system直接時間が明確に改善すれば採用できる。どちらも改善しない場合は仮説を一度で打ち切ってrollbackする。M0/M9はこの基準の対象外とする。

## 8. ロールバック方針

- §3.4の依存wave順に適用し、次のsub-milestone単位でrevert可能なcommitへ分ける。相互依存するM0～M9を「順不同の独立PR」とは扱わない。
  - M1: idle早期除外 / context mutable遅延 / reservation signature。
  - M2: Soul無効root write削除 / Familiar visual child / proxy・command・aura追従。
  - M3: cadence/cache/scratch（M3A） / 条件付きBuild統合（M3B）。
  - M4: connectivity / budget・公平性 / multi-goal・NeedsPath / Door近傍化。
  - M5: phase/index / curing・evacuation / load rehydrate。
  - M6: vitals accumulator / idle・sanity / energy。
  - M7: Blueprint / Entity List / area・RTT・inspection / Dream。
  - M8: shader sample / projector / mask scene / RTT解像度 / asset変換。
- rollback時はそのマイルストーンで追加したcomponent/resource/system registrationと恒久ドキュメントを同じ単位で戻す。
- M0 instrumentationとbaseline形式は後続比較が完了するまで維持し、個別最適化のrollbackに巻き込まない。
- 正しさ/save-load計画と共有するtask context、reservation sync、WorldMap、load、UI change detectionは、§3.4の前提完了後に編集する。
- gitで破棄する前に `git log --oneline -5` と対象ファイルの `git diff HEAD -- <file>` を確認し、並行作業の差分が含まれないことを確認する。
- 計測用probe、debug material、temporary environment defaultは原因確認後に撤去し、恒久実装へ混在させない。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: 性能 M0の計測基盤・Gather正式baseline、M1-A（idle早期除外とcounter）、M1-Bのreservation signature/cache split/counterが完了。M0全体とM1のcontext mutable遅延以降は継続中。
- 完了済みsub-milestone: M1-A、M1-Bのreservation同期部分。3規模×CPU/GPUのGather frame-time baselineも有効化済みだが、M0のfixed-step audit・draw/scene count・Tracy memory・RenderDoc・専用workloadは未完である。
- 未着手/進行中: §3.4 wave A の `runtime-correctness-contracts` M0 は完了。性能 M0 は strict parse、固定size/render条件、profiling feature分離、Warmup/Measure/Flush CSV capture、ゲーム更新前initial fixture checkpoint、run固有artifact、adapter/log/checksum検証、M1-A task execution counter、M1-B reservation counterを実装済み。旧18 runは計測中errorのため比較対象外。M1のcontext分割はreservation同期と独立して、正しさ/save-load前提を確認してから開始する。
- M1-AはLarge/CPUの正式baselineと同条件で3 valid runを採取し、idle skip比率99.250127%を確認した。M1-B以後のcaptureはschema v4で、schema v2/v3 artifactとはframe-timeだけを比較し、欠落counterは新規postcondition観測として扱う。

### 次のAIが最初にやること

1. `README.md`、`docs/DEVELOPMENT.md`、`docs/README.md`、本計画、正しさロードマップと3子計画、`docs/plans/archive/system-wide-performance-followups-plan-2026-07-07.md`を読む。
2. userの未コミット差分を `git status --short` と `git diff` で確認し、対象外ファイルを編集しない。
3. `runtime-correctness-contracts` M0完了を確認し、未完なら性能M0より先に同計画を実施する。
4. runtime M4のDoor topology version契約と回帰テストがgreenであることを確認し、本計画からmutation APIを変更しない。
5. M0のraw CSVを保持したまま、fixed-step determinism audit、CPU toggle時のdraw/scene count、RenderDoc、Tracy memory、Gather以外の専用workloadを補完する。実時間frame-time baselineとは混在させない。
6. schema v4でM1-B reservation同期の3反復30/60秒比較を採取し、`reservation_sync_full_rebuilds`と2種の走査件数をframe-timeと併記する。M1のcontext mutable遅延は正しさ/save-loadの前提wave確認後に別sub-milestoneとして実施し、M1-A/Bのtask/reservation契約を変えない。

### ブロッカー/注意点

- M1で `With<WorkingOn>` をtask execution filterへ追加しない。target despawn後の`Some + Without<WorkingOn>`をcleanup可能なまま保つ。
- 正しさ/save-load子計画と共有するファイルを並行編集しない。§3.4の前提を完了し、本計画をrebaseする。
- `TaskEndDisposition` と `OnTaskCompleted`、retryable handoffの意味を変更しない。
- M4のbudget deferをpath失敗やtask abortへ流さない。
- M6の低頻度化でrender frameのdeltaを捨てず、dirty wake-upで時間を二重加算しない。
- M8はM0でGPU律速を確認するまで開始しない。
- `docs/plans` は作業文書であり、完了時は恒久ドキュメントへ契約を移してarchiveする。

### 参照必須ファイル

- `docs/DEVELOPMENT.md`
- `docs/tasks.md`
- `docs/invariants.md`
- `docs/familiar_ai.md`
- `docs/soul_ai.md`
- `docs/entity_list_ui.md`
- `docs/rendering-performance.md`
- `docs/plans/system-wide-correctness-refactoring-plan-2026-07-12.md`
- `docs/plans/runtime-correctness-contracts-plan-2026-07-12.md`
- `docs/plans/save-load-hardening-plan-2026-07-12.md`
- `docs/plans/structural-maintainability-followups-plan-2026-07-12.md`
- `docs/plans/archive/system-wide-performance-followups-plan-2026-07-07.md`
- `crates/hw_soul_ai/src/soul_ai/execute/task_execution_system.rs`
- `crates/bevy_app/src/systems/familiar_ai/perceive/resource_sync.rs`
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_delegation.rs`
- `crates/hw_soul_ai/src/soul_ai/pathfinding/system.rs`
- `crates/hw_soul_ai/src/soul_ai/execute/task_execution/path_cache.rs`
- `crates/hw_world/src/door_systems.rs`
- `crates/hw_world/src/map/doors.rs`
- `crates/bevy_app/src/world/map/spawn.rs`
- `crates/bevy_app/src/systems/save/load.rs`
- `crates/hw_jobs/src/construction.rs`
- `crates/hw_visual/src/blueprint/progress_bar.rs`
- `assets/shaders/shadow_style.wgsl`

### 最終確認ログ

- 計画作成時 `cargo check --workspace`: `2026-07-12 / pass`
- 計画作成時 `cargo clippy --workspace`: `2026-07-12 / pass (warnings 0)`
- 自己レビュー後 `python scripts/update_docs_index.py`: `2026-07-12 / pass`
- 自己レビュー後 `cargo check --workspace`: `2026-07-12 / pass`
- 自己レビュー後 `cargo clippy --workspace`: `2026-07-12 / pass (warnings 0)`
- 自己レビュー後 `cargo test --workspace`: `2026-07-12 / pass (76 passed, 0 failed)`
- 性能 M0 実装後 `cargo check --workspace`: `2026-07-13 / pass`
- 性能 M0 実装後 `cargo check -p bevy_app@0.1.0 --no-default-features --features profiling`: `2026-07-13 / pass`
- 性能 M0 実装後 `cargo check -p bevy_app@0.1.0 --no-default-features --features profiling-memory`: `2026-07-13 / pass`
- 性能 M0 実装後 `cargo clippy --workspace -- -D warnings`: `2026-07-13 / pass (warnings 0)`
- 性能 M0 実装後 `cargo test -p bevy_app@0.1.0 --lib perf_scenario`: `2026-07-13 / pass (1 passed)`
- asset catalog修正後 `cargo check --workspace`: `2026-07-13 / pass`
- asset catalog修正後 `cargo clippy --workspace -- -D warnings`: `2026-07-13 / pass (warnings 0)`
- runtime baseline: `2026-07-13 / Gather / Small・Medium・Large × CPU・GPU × 3 run / CSV capture pass`
- M1-A `PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py self-test`: `2026-07-13 / pass`
- M1-A `cargo test -p hw_soul_ai` / `cargo test -p hw_soul_ai --features profiling`: `2026-07-13 / pass (5 passed each)`
- M1-A `cargo check -p bevy_app@0.1.0 --no-default-features --features profiling|profiling-tracy|profiling-memory`: `2026-07-13 / pass`
- M1-A `cargo check --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace`: `2026-07-13 / pass`
- M1-A runtime candidate: `2026-07-13 / Gather Large・CPU × 3 run / CSV capture pass / p50 -2.359% / idle skip 99.250127%`
- M1-B `cargo test -p bevy_app@0.1.0 --no-default-features --features profiling --lib systems::familiar_ai::perceive::resource_sync::tests`: `2026-07-14 / pass (5 passed)`
- M1-B `cargo check -p bevy_app@0.1.0 --no-default-features --features profiling` / `cargo check --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace` / `PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py self-test`: `2026-07-14 / pass`
- M1-B runtime smoke: `2026-07-14 / Gather Small・CPU × 1 run / schema v4 CSV capture pass / reservation rebuild=4, pending scan=282, assigned scan=200`
- 未解決設計gate: なし（Door cost/topology version契約はruntime M4へ統一済み）。

### Definition of Done

- [ ] M0～M7の必須部分を完了し、各primary metricとframe timeを前後比較した。M3B/M7Dの条件付き部分は実施または根拠付きskipを記録した。
- [ ] M8は開始条件を評価し、実施または根拠付きでskipした。
- [ ] M9の恒久ドキュメント・計画整理を完了した。
- [ ] task、reservation、path、Door、construction、UIの動作契約を維持した。
- [ ] probe/debug用の一時実装を撤去した。
- [ ] 影響する恒久ドキュメントを更新した。
- [ ] rust-analyzer diagnosticsが0件である。
- [ ] 変更したRustファイルがrustfmt済みである。workspace format baselineがgreenの場合は `cargo fmt --all -- --check` も成功した。
- [ ] `cargo check --workspace` が成功した。
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` が成功した。
- [ ] `cargo test --workspace` が成功した。
- [ ] 本計画をarchiveし、docs indexを更新した。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-07-12` | `Codex` | 現行コードの全体性能レビューと2026-07-07完了計画の照合に基づき初版作成。 |
| `2026-07-12` | `Codex` | 自己レビューを反映。正しさ/save-load/structural計画との実在依存へ更新し、worldgen seed、connectivity cache、visual互換、construction load順、slow simulation、UI/GPU検証、必須test/rollback境界を修正。 |
| `2026-07-12` | `Codex` | runtime M4のDoor topology version契約、明示package ID、共通all-target Clippy gateへ整合。 |
| `2026-07-13` | `Codex` | M0を開始。固定seed/規模/描画条件、profiling feature、CSV captureと計測手順を実装し、専用workload・domain counter・実測baselineは未完として継続管理。 |
| `2026-07-13` | `Codex` | M0の実装を更新。worldgen/Soul/Familiar/cosmeticのseed stream、Familiar delegation counter、profiling/profiling-memoryの検証を追加。`gather`以外の専用操作列と実測baselineは未完のまま分離した。 |
| `2026-07-13` | `Codex` | 存在しないasset参照を除去・修正し、Intel Vulkan上でGather baseline 18 runを採取。中央値、再現性、未完の観測項目をM0へ記録。 |
| `2026-07-13` | `Codex` | 計測runner/CSV契約をschema v3へ拡張し、部分case比較とtask execution counterを追加。M1-Aのidle早期除外を実装・実測し、恒久task contractを更新した。 |
| `2026-07-14` | `Codex` | M1-Bのreservation signature/cache splitを実装。load reset可能なroot signature cache、frame deltaとsnapshotの分離、schema v4 reservation sync counter、回帰testを追加した。正式3反復比較は未採取。 |
