# パフォーマンス計測

ランタイム最適化の比較は、`scripts/perf.py` を唯一の入口にする。このファイルは後方互換CLI shimであり、実装は `scripts/perf_tool/` の引数解析・実行・artifact・policy・集約・比較モジュールへ分割されている。runnerは profiling binary を計測外で一度だけbuildし、runごとに隔離したCSV・log・実行環境を保存してから、CSV契約、実GPU、ログ健全性、反復のcheckpointを検証する。長期保存する正式artifactの既定配置は`target/perf-runs/`とする。隔離した受入セッションや短縮smokeは`/tmp`へ置いてよいが、どちらもcommitしない。

ゲーム側の入口は `crates/bevy_app/src/plugins/startup/perf_scenario.rs` に維持し、設定、fixture、workload driver、capture driver、audit checksum/encoding、出力処理は同名ディレクトリの子モジュールが担当する。CLI option、summary schema、checkpoint順序はこの物理分割に依存しない。

再現性は二段階に分ける。通常の実時間ベンチマークは、ゲーム更新前の**初期 fixture**を必ず一致させ、warm-up/計測終端の状態は実測値として記録する。`Time<Virtual>`は実フレームのdeltaで進み、warm-up境界を越える最終frameがrunごとに異なるためである。完全に同じsimulation時刻での状態一致は、`scripts/perf.py audit` による固定stepの決定性auditとしてframe-time計測と別に扱う。auditは性能値を出力せず、通常の`summary.csv` baselineとも比較しない。

## 計測モード

| モード | runner option | 用途 | frame timeへのTracy擾乱 |
| --- | --- | --- | --- |
| Capture | `--instrumentation capture` | 標準のframe time・domain counter・Task Dashboard実CPU | なし |
| Tracy | `--instrumentation tracy` | 任意のsystem zone cross-check | あり。CSV baselineとは別run |
| Memory | `--instrumentation memory` | measure区間のRust allocationとprocess peak RSS | allocator計数の擾乱あり。frame timeには使わない |

`frames.csv` の `frame_time_ms` は Bevy 0.19 の `Time<Real>` のフレーム間隔であり、system CPU timeやGPU pass timeではない。`cpu`/`gpu` は描画構成の切替名である。Task Dashboardのsystem CPU timeはCapture buildがmeasure区間だけ記録する`data/task_dashboard_cpu.csv`を正本とし、GPU pass/drawは固定frameのRenderDoc captureで別に採取する。Tracy / Memory sessionのframe quantileはinstrumentation擾乱を含むため、dashboard mode比較結果へ性能値として出さない。

Memory buildは`profiling-memory`限定のglobal allocator wrapperを使う。`data/memory.csv`はmeasure区間のbaseline / peak / final live bytes、alloc / dealloc bytes・calls、realloc callsを記録し、runnerが収支恒等式とaccounting error 0を必須検証する。これはRust global allocatorを通る割当だけが対象で、C / GPU / `mmap`等はGNU timeのprocess最大RSSで補完する。

## 標準手順

最初にrunner自身の検証fixtureを実行する。

```bash
PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py self-test
```

### RtT / 室内照明 migration contract

P00以降の比較正本は`scripts/perf_tool/contracts/rtt_light_migration_v1.json`である。fixtureのsmall /
medium / large exact count、座標生成順、behavior case、stageごとのrequired lane / gate、projection列、
formal matrixを一つのcanonical hashへまとめる。stage別のrequired / forbidden /
`not_applicable`規則、gate resultのexpected metric row、production Door behavior fixtureまで実装済みで、
`lifecycle.status`は`frozen`、`formal_registration_allowed`は`true`である。v1を変更する場合は既存JSONを
書き換えず、v2を追加してreferenceとcandidateを同じversionで再採取する。

```bash
PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py \
  validate-rtt-light-contract \
  --contract rtt-light-v1 --stage current --lane static
```

`current`では`static`と`behavior`がrequiredであり、behavior caseは`door-state-v1`と
`load-normal-v1`である。`field-core`はP03より前、`consumer-core`はP07より前では失格になる。validatorは
canonical JSONをpinし、意図しない変更を検出する。canonical measurement contract hashは
`121a365ac3349cd4fa7890ab3069f0392098ced17e0d47f920095a1490c2ba11`、fixture hashは
`a688d564f8f50c2fdcdbe49dca7625b2cb05d01f8555378215fb8ba89b553eed`である。layout hashはsmall
`e87a3b1aeb7ee1fbe334d311ad731bef24ce90ec80066af1e35c006ef4273af2`、medium
`e18320b3bcf8089c1ea2743003eadd79a0c938caa44682ed414e9d9d54af8f2d`、large
`3dec65d6c30ee9b88678af28a818a05fa70ededc66f20242ff78dcb6772c56fd`である。session manifestは
選択したsizeごとのchecksum mapを持ち、run metadataは当該caseのchecksumを持つ。

frame-time / fixed-stepの両runは`data/window.csv` schema v1を必須sidecarとして出力する。windowed runは
開始・終了時のlogical / physical size、scale factor、RtT品質、Scene / mask target寸法、resolved window
backend、actual adapter/backend、requested / effective present modeを記録し、途中で1項目でも変化したrunを
失格にする。`--window-width` / `--window-height`はBevy 0.19
`WindowResolution::new`へ渡すphysical pixelである。headless runではprimary-window fieldを空にし、
`window_present=false`を要求する。fallback RtT寸法は経路診断として検証するが、actual window / surface /
present能力の証拠には数えない。

session manifest schema v2はmanifestのcase集合と`preflight-NNN` / `run-NNN` directory集合をexact比較し、
欠損、未知directory、invalid preflight、欠損`validation.json`を集約前に拒否する。比較もschema v2かつ
`status=valid`のsessionだけを受理し、NaN / infinityや0以下のreference値を拒否する。各frame summaryの
p50 / p95 / p99 / maxは`frames.csv`から同じround-half-up式で再計算する。fixed-stepは
`determinism.csv`だけでなくactor record sidecarのschema、checkpoint、stable order、population countを
`determinism_records.csv`から検証し、各checkpointのrecord payloadからRustと同じFNV-1aで
`state_checksum`を再計算する。

`indoor-light`はcurrent stageで`static`と`behavior`を実行できる。staticはsmall / medium / large、
behaviorは上記2 caseのfixed-step artifactであり、field / consumer coreはstage成立前に捏造しない。
3規模ともFloor / Wall / Door、給電Lamp、無給電control Lamp、SoulSpa、Yard / PowerGrid、Roomを
production spawn / completion / energy / Room再構築経路で作り、契約したSoul / Familiarを固定する。
Door状態はsmallがClosed 1、mediumがOpen 2 / Closed 1 / Locked 1、largeがOpen 4 / Closed 8 /
Locked 4である。

medium / largeの`all-building-showcase-v1`はRust runtimeとoffline validatorの両方でexact化済みである。
`BuildingType::ALL`順の12 root、各anchor / footprint、追加7棟のcompletion route、Tank用BucketStorage
companion、post-process component、current presentationを固定する。既存のFloor / Wall / Door /
SoulSpa / OutdoorLampを再利用し、追加7棟をexact rootとして数える。Bridgeは現行2×5 completion geometryを
観測するfixture専用probeであり、蛇行するgenerated riverに対してplayer authoring validationが成立するとは
主張しない。Tank companionは論理配置1件だが、現行production topologyどおり1 tileずつ2つの
`BucketStorage` entityを作り、空Bucket 5個を3 / 2へ決定的に格納する。Doorの動的遷移はbehavior artifactで
別に閉じる。

runtime sidecar期待値はmediumがlayout 722行 / presentation 12行 / indoor semantic actor 258件、largeが
2306行 / 12行 / 936件である。通常actorのうちSoul / Familiarはsize契約、Designationは各checkpointの
`determinism.csv`宣言値と照合し、indoor semantic actorとは別に増減を記録する。
Python self-testは3規模を生成・再読込し、Bridge footprint改変、size別actor欠落、presentation topology差を
fail-closedに扱う。fixed auditは各checkpointでDoor state / child image / WorldMap、Room reverse lookup、
電力網、SoulSpa worker、showcase componentを再検証する。realtime Capture / Memoryもwarmup終端とmeasure終端で
同じsemantic validatorを通し、初期sidecarだけが正しいstale artifactを成功扱いしない。

各規模は`indoor_light_fixture.csv` 1行と、small 187 / 5、medium 722 / 12、large 2306 / 12行の
`indoor_light_layout.csv` / `indoor_light_presentation.csv`を必須出力する。indoor semantic actorはsmall 78、
medium 258、large 936件を全checkpointでexact検証する。generic actorはSoul / Familiar / Designationを
checkpoint宣言値へ照合し、fixture外のproduction designationをindoor契約へ誤って固定しない。
Room entityはproduction検出で再生成され得るため、Entity IDをhashせず、floor cell集合、wall / Door境界、
bounds、reverse lookupから毎checkpoint一意に再同定する。

短縮headless smokeは次のshapeで実行できる。dev profile binaryと129 + 16 tickは経路確認専用であり、
formal baselineやrenderer / GPU性能証跡には使わない。

```bash
cargo build -p bevy_app@0.1.0 --no-default-features --features profiling
PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py audit \
  --workload indoor-light --contract rtt-light-v1 \
  --stage current --lane static --sizes small,medium,large --renders cpu \
  --seed 20260803 --repeat 1 --preflight-runs 0 \
  --backend auto --window-backend headless --present-mode novsync \
  --fixed-hz 64 --warmup-ticks 129 --audit-ticks 16 \
  --skip-build --binary target/debug/bevy_app \
  --allow-log-pattern 'driver that only supports software rendering' \
  --output /tmp/rtt-light-current-static
```

ローカルVulkan loader由来の追加ERRORを診断用regexで許可したrunはformal evidenceに昇格させない。
P00 current formal baselineは`2026-08-07`に登録済みである。subjectは
`43e353eeb90f6c930366b8e9e275088e55275c16`、attemptは
`target/perf-runs/rtt-light/rtt-light-v1/current-43e353eeb90f6c93/attempts/0e0b8d96-d757-4505-9cd8-c5f093387f8c`
で、same-source S0 / S1、actual-window Capture / Memory、medium/gpu RenderDoc replay、offline bundle
validationを満たす。実測matrixとprovenanceは[レンダリングパフォーマンス](rendering-performance.md)を正本とする。

登録済みattemptの再検証は次で行う。verifierはsource checkpoint、全required case、binary provenance、actual
environment、RenderDoc topology、CSV再生成、artifact directory digest、baseline locatorをfail-closedに照合する。

```bash
PYTHONDONTWRITEBYTECODE=1 python3 \
  .codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py \
  verify-rtt-light --repo "$PWD" \
  --attempt target/perf-runs/rtt-light/rtt-light-v1/current-43e353eeb90f6c93/attempts/0e0b8d96-d757-4505-9cd8-c5f093387f8c

PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py \
  verify-rtt-light-baseline \
  --baseline target/perf-runs/rtt-light/rtt-light-v1
```

invalid / interrupted attemptは診断artifactを残すが、比較referenceへ登録しない。`plan-rtt-light`が`blocked`を
返した場合も実行へ進めない。

RenderDoc runtime checkpoint / replay extractionはschema v2である。checkpointはScene / Soul mask targetの
labelに加え、compositeのVulkan descriptor contract（fragment set 2: Scene texture / sampler = binding 1 / 2、
mask texture / sampler = binding 3 / 4）を固定する。extractorは`vkCmdNextSubpass`の`EndPass | BeginPass`を
旧subpassのclose後に次subpassへ開く。`CommandBufferBoundary`のvirtual markerはrender-pass state遷移として
扱わない。globalなsampler件数ではなく、同一draw上の2 texture + 2 samplerがこのexact `(set, binding)`を
満たすことを要求する。runtime labelを解決できない場合にもcomposite descriptor位置からresource対応を照合し、
checkpointが欠けたfailureでは`renderdoc-failure-*`診断を保持する。isolated QRenderDoc profileとanalytics
opt-outを使うため、replayは対話ダイアログに依存しない。schema v1のcheckpoint / extractionはformal baselineへ
登録できない。

### 許可ダイアログなし実機受入

実機確認、実機テスト、actual window、renderer / GPU / backend、native performanceの受入では、repository Skill `hell-workers-run-native-acceptance`を毎回使う。Skillを利用できないagentは`.cursor/skills/hell-workers-run-native-acceptance/SKILL.md`を直接読む。Task Dashboardの標準recipeは次のplan commandを入口にし、返された`launcher_command`を先頭の`kitty`を変えずに直接実行する。

```bash
PYTHONDONTWRITEBYTECODE=1 python3 \
  .codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py \
  plan-task-dashboard --repo "$PWD" --adapter Intel \
  --backend vulkan --window-backend x11
```

helperはUUID付きの一意なartifact rootとatomicな`job.json`を使い、fixed audit、実window Capture、native Memoryをrepository-wide lockの内側で逐次実行する。各sessionは`perf.py`自身に対応featureのbuildを行わせ、`--skip-build`と任意`--binary`を使わない。これにより、`target/profiling/bevy_app`が直前のMemory flavorであるのにCaptureとして記録する取り違えを防ぐ。CaptureとMemoryの間ではbinary hashが変わり、fixed auditとCaptureでは一致することをfail-closedで検証する。

resource preflightは`MemAvailable` 8 GiB、workspace空き15 GiB、`/tmp`空き1 GiBを開始下限とする。12 GiB以上ではCargo 2 job、未満では1 jobとし、`CARGO_INCREMENTAL=0`を固定する。`/tmp`はtmpfsなのでbinaryやfull Tracy traceを置かず、小さいmanifest / CSV / logだけを置く。通常のMemory受入はnative allocator + GNU timeを使い、Cargo/game/Capture/Memoryは並列化しない。artifactやCargo cacheの自動削除、別target directory、routineな`cargo clean`、`nice` / `ionice` / CPU affinityは行わない。

既承認launcherが利用可能な間は、displayやGUIの追加許可をユーザーへ求めない。helperが返す`status_command`だけを15〜30秒間隔でpollし、通常は大きなbuild/game logを会話へ読み込まない。headlessはfixed correctnessまたはCPU-only route smokeに限定し、実renderer / adapter / presentの証拠にはしない。

### window backendの使い分け

`--window-backend headless`はWinit、primary window、surface、swapchain、presentを作らない。display socketを使わずに固定step監査とCPU-onlyの経路smokeを実行できるが、実renderer frame-time、GPU adapter、presentの証拠にはしない。`headless`は`--renders cpu`だけを許可し、software adapter警告を明示的にallowlistしたsmokeの値も性能比較には使わない。

sandbox内から実windowへ直接接続できない場合は、既に許可されたterminal launcherを入口にしてrunner全体をsandbox外で起動する。追加の対話的許可を要求せず、artifactを通常どおり監視・検証する。

```bash
kitty --directory "$PWD" --detach \
  python3 scripts/perf.py run \
  --workload task-dashboard --sizes small --renders cpu \
  --dashboard-modes hidden,visible,active-filter --repeat 3 \
  --backend vulkan --adapter Intel --window-backend x11 \
  --present-mode novsync --output /tmp/task-dashboard-x11
```

launcher経路でも`--adapter`と`--backend`を省略せず、manifestの実adapter/backend一致をrunnerに検証させる。headless成功を理由に実window runを省略しない。

次に、比較するGPU、backend、window backendを明示し、同じseedを3回採取する。以下はIntel/Vulkanで全規模・全描画条件を採る例である。

```bash
python3 scripts/perf.py run \
  --workload gather \
  --sizes small,medium,large \
  --renders cpu,gpu \
  --repeat 3 \
  --seed 20260712 \
  --backend vulkan \
  --adapter Intel \
  --window-backend wayland \
  --present-mode novsync \
  --warmup-checksum-policy record \
  --output target/perf-runs/gather-intel-vulkan-seed-20260712
```

- runnerは `BEVY_ASSET_ROOT` をワークスペース根へ固定し、profiling binaryを直接起動する。
- perf起動ではユーザーの`settings/settings.ron`にあるpause/倍速を無視し、`Time<Virtual>`をunpause・1xへ固定する。終了時にもsettingsを書き戻さない。
- `WGPU_ADAPTER_NAME` は一致しなくてもBevyが別adapterへfallbackできる。`--adapter`と`--backend`を指定したrunでは、log上の実 `AdapterInfo` が一致しなければ失格になる。
- `--souls <n> --familiars <n>` を組にするとsize presetを上書きできる。custom populationもcase IDとsummaryへ記録される。
- `--preflight-runs 1` は本測定前に同じcaseを一回だけ温める。preflight artifactは残すが、aggregateには入れない。

短縮した経路確認には、例えば次を使う。

```bash
python3 scripts/perf.py run \
  --workload gather --sizes small --renders cpu --repeat 3 \
  --warmup-secs 3 --measure-secs 5 \
  --backend vulkan --adapter Intel --window-backend wayland \
  --output target/perf-runs/smoke-gather-intel
```

CPU/GPU切替、artifact、CSV契約だけを短時間で確認するときは、既にprofiling binaryをbuild済みである場合に限り次を使う。これは起動経路の確認であり、性能比較用のbaselineではない。

```bash
PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py run --skip-build \
  --workload gather --sizes small --renders cpu,gpu --repeat 1 \
  --warmup-secs 0 --measure-secs 1 \
  --backend vulkan --window-backend wayland \
  --output target/perf-runs/m0-smoke
```

CPU条件では`data/scene_roots.csv`のSoul main/mask/shadowとFamiliar rootがすべて0、GPU条件ではSoul数・Familiar数と一致しなければrunnerが失格にする。これはCPU-only条件へ対象外の3D sceneを混ぜないための契約である。

## 標準 workload

すべての workload は初期 checkpoint より前に決定的に生成される。手操作や既存 save を前提にしてはならない。

| workload | 固定負荷 | 主な counter |
| --- | --- | --- |
| `gather` | Gather designation と Familiar 指揮 | task / reservation / delegation |
| `path-door` | corridor、Door 開閉、両方向の Soul traffic | core A*、defer frame、Door近傍候補 |
| `construction` | Curing 中の Floor site（Small/Medium/Large = 16/64/128 tile） | construction site/tile、evacuation候補 |
| `ui-gpu` | Blueprint（Small/Medium/Large = 64/160/320） | UI/visual の描画条件 |
| `task-dashboard` | 同一task / Soul / Familiar集合とdashboard 3 mode | AI work、dashboard producer / render、Task Dashboard CPU / memory |

`construction` は Curing footprint の安全監査を含む。完成済みの別 workload の数値を construction の比較値として流用しない。

固定step auditはsimulation状態の診断専用である。frame-timeを採取せず、`summary.csv`も生成しない。

```bash
PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py audit \
  --sizes small --renders cpu --repeat 3 \
  --fixed-hz 64 --warmup-ticks 1920 --audit-ticks 128 \
  --backend vulkan --window-backend wayland \
  --output target/perf-runs/gather-fixed-audit
```

audit artifactの`data/determinism.csv`はcheckpointごとの状態checksum、`data/determinism_records.csv`は差分調査用のactor単位recordである。失敗時はそのaudit sessionを失格にするが、実時間baselineの`summary.csv`を置き換えたり、frame-time比較に混ぜたりしない。

fixed-step determinism artifactはschema v4である。Familiar actor recordにはoperationと、
`WorkType::ALL`の安定順に並べた全effective rule（allowed / priority）を含む。raw override vectorの
格納形が違ってもeffective policyが同じなら同じbytesを生成する。Gatherに加え、Haul assignmentは
phaseとitem / stockpileのTransformをEntity ID非依存で符号化する。
`determinism.csv`にはpolicyを除外した`structural_checksum`、policyを含む`state_checksum`と、
delegation / candidate policy gate / snapshot / score / worker score / source selector / connectivity counterを
checkpointごとの累積値として記録する。schema v4ではTop-K、wheelbarrow arbitration、caller別runtime A*、dashboard producer / render counterも同じcheckpointへ含める。Rust writerとPython runnerはどちらもdeterminism schema v4を要求するため、旧determinism schema v1〜v3 artifactは現行runnerではinvalidになる。これは後述の`summary.csv` schema v11とは独立したversionである。

Familiar policyのcontrolled auditは`gather`固定step専用で、次のexact matrixを使う。

```bash
PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py audit \
  --workload gather --sizes small --renders cpu \
  --familiar-policies default,disabled \
  --operation-dialog-modes hidden,open \
  --repeat 2 --preflight-runs 1 --seed 20260731 \
  --fixed-hz 64 --warmup-ticks 129 --audit-ticks 16 \
  --backend vulkan --window-backend x11 \
  --output target/perf-runs/familiar-policy-controlled-audit
```

controlled fixtureは通常`gather`負荷を置き換え、同じFamiliar rosterにmanual Haul 1件とChop 1件を与える。
Haulはsource selector、Chopはconnectivity cacheを正規経路で通す。runnerは4 caseが揃った場合だけ
`familiar_policy_comparison.json`を生成し、次をfail-closedで検証する。

- policyごとにdialog hidden / openの全checkpoint checksumとAI work counterが完全一致する。
- fixture初期`structural_checksum`はdefault / disabledで一致し、policyを含む`state_checksum`は異なる。
- defaultはcandidate gate以後のsnapshot / score / worker / source / connectivity counterがすべて正になる。
- disabledは全candidateをpolicy gateでrejectし、後段counterがすべて0になる。

`--familiar-policies`と`--operation-dialog-modes`のcontrolled値は、通常`run`や`gather`以外では拒否する。
dashboard表示条件はこのmatrixへ混ぜず、Task Dashboard性能計画が所有する。

Task Dashboardの正式matrixは、固定step監査、Capture、Memoryを別sessionで採る。

```bash
PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py audit --skip-build \
  --workload task-dashboard --sizes small --renders cpu \
  --dashboard-modes hidden,visible,active-filter --repeat 1 --seed 20260802 \
  --fixed-hz 64 --warmup-ticks 129 --audit-ticks 16 \
  --backend vulkan --window-backend headless \
  --allow-log-pattern 'driver that only supports software rendering' \
  --binary target/profiling/bevy_app --output /tmp/task-dashboard-fixed

python3 scripts/perf.py run --skip-build --instrumentation capture \
  --workload task-dashboard --sizes small --renders cpu \
  --dashboard-modes hidden,visible,active-filter --repeat 3 --seed 20260802 \
  --backend vulkan --adapter Intel --window-backend x11 --present-mode novsync \
  --binary target/profiling/bevy_app --output /tmp/task-dashboard-capture

python3 scripts/perf.py run --skip-build --instrumentation memory \
  --workload task-dashboard --sizes small --renders cpu \
  --dashboard-modes hidden,visible,active-filter --repeat 3 --seed 20260802 \
  --backend vulkan --adapter Intel --window-backend x11 --present-mode novsync \
  --binary target/profiling/bevy_app --output /tmp/task-dashboard-memory

python3 scripts/perf.py compare-dashboard-modes \
  --session /tmp/task-dashboard-capture --min-runs 3
python3 scripts/perf.py compare-dashboard-modes \
  --session /tmp/task-dashboard-memory --min-runs 3
```

Capture / Tracy / Memoryは標準baselineとして混ぜない。特に同じ`target/profiling/bevy_app`をfeature別buildが上書きするため、各sessionの直前に対応する`profiling` / `profiling-tracy` / `profiling-memory` featureでbuildし、manifestのbinary hashを証跡にする。

```bash
python3 scripts/perf.py run --instrumentation tracy --sizes medium --renders cpu \
  --backend vulkan --adapter Intel --output target/perf-runs/tracy-medium

python3 scripts/perf.py run --instrumentation memory --sizes medium --renders cpu \
  --backend vulkan --adapter Intel --output target/perf-runs/memory-medium
```

Tracy runだけがTracy 0.13.1のcapture / csvexport executableを要求する。runnerはゲームがmeasure artifactを書き終えた境界でcaptureへdisconnectを要求してtraceを保存する。validated runでは固定秒の`--tracy-capture-secs`を拒否し、warm-up前にtraceだけ終了した結果を有効化しない。Memory runはTracy executableもlocal profiling socketも使用しない。

## 反復・有効性の契約

各runは次をすべて満たしたときだけ有効である。

1. processが成功終了し、`PERF_CAPTURE: wrote`、空でない`frames.csv`、schema version一致の`summary.csv`がある。
2. `seed`、workload、size、render、初期entity/task checksumが要求caseと一致する。
3. 指定した場合、logの実adapter/backendが要求値と一致する。
4. capture完了前に、allowlist外の`WARN`、`ERROR`、Bevy command errorがない。
5. 同じcaseの全反復で、ゲーム更新前に採った`initial_state_checksum`（Soul/Familiar/Designation数と位置を含む）が一致する。これは常に必須である。

`--warmup-checksum-policy record` が既定であり、実時間ベンチマークの標準条件である。warm-up終端checksumの差と実際のvirtual/real秒数をartifactへ残し、負荷の位相ずれを確認できる。`require` は、同じwarm-up状態が成立することを診断したい場合だけ使う。現在の可変delta実行では、同じseedでも境界を越えるframeが異なるため、`require`で失格になることは期待される挙動である。

計測完了後のwarning/errorは有効性を失わせないが、`validation.json`の`teardown_warning_lines`、`aggregate.csv`の`post_capture_teardown_warning_counts`、`report.md`へ必ず記録される。現在確認されている`CommandQueue has un-applied commands`は、speech/conversationの`Commands::delayed()`が次の`PreUpdate`より前に`AppExit`で破棄されるteardown由来であり、強制flushして計測状態を変えてはならない。完了マーカー前の同種warningは従来どおり失格である。

scenario driverは `Warmup → Measure → Flush → AppExit` を自動遷移する。各checkpointのinitial、warm-up終端、measure終端のentity数・Designation数・state checksum、実際のvirtual/real秒数、p50/p95/p99/maxは`summary.csv`に入る。`gather`、`path-door`、`construction`、`ui-gpu` はすべて専用 fixture を持つため、異なる workload の結果を相互の速度比較に使わない。

## Artifact形式と集約

```text
target/perf-runs/<session>/
  manifest.json             # git/binary hash、host、要求環境、実adapter、session status
  matrix.json               # seed、規模、描画、反復、時間、checksum policy
  aggregate.csv             # valid runだけのrunごとquantileの中央値/MAD
  report.md                 # valid/invalidと失格理由
  cases/<workload-size-render-seed>/
    run-001/
      command.txt
      requested-environment.json
      run.log
      validation.json
      run-metadata.json
      data/frames.csv
      data/summary.csv
      data/scene_roots.csv
      data/task_dashboard_cpu.csv # task-dashboard Capture / Tracy
      data/memory.csv              # Memory build
      profile-artifact.json
      resource-usage.txt           # Memory build
```

fixed-step auditでは`frames.csv`と`summary.csv`の代わりに、`data/determinism.csv`と`data/determinism_records.csv`を出力する。

`summary.csv` schema v11には、frame-timeに加えcapture期間全体の task execution / reservation / delegation counter、candidate snapshot / score、Top-K、wheelbarrow arbitration、caller別 runtime A* と defer counter、dashboard producer / render、Door候補数、construction の site/tile/evacuation counterを入れる。さらにslow simulationのstep / 更新Soul / idle decision / sanity auditと、energyのoutput / grid / lamp候補counterを入れる。`aggregate.csv`には各counterの中央値/MADと、run内で割り算してから集約したidle skip比率・handler到達比率を併記する。これらはframeあたりの値ではないため、比較時は同じmeasure秒数でのみ用いる。別々のcounterを独立に中央値化した値どうしを引き算して比率を作ってはならない。

`runtime_path_total_core_searches` は caller別 `*_core_searches` の和であり、capture中に budgeted facade がclaimした実core A*数である。`*_deferred` は枠不足で拒否されたcore A* request数であり、requestの待機frame数ではない。frameごとのhard limitは `RuntimePathSearchBudget` のclaim境界とunit testで保証し、capture合計だけから1フレームの上限を推定してはならない。

`reachable_with_cache_calls` は schema v11でも互換のため名前を維持しているが、M4A以後は Familiar 委譲が version付き連結成分 cache に問い合わせた回数であり、core A* 呼び出し回数ではない。Boolean 到達判定が A* を呼ばないことは cache/A* parity test と topology version 回帰 test で保証する。既存schema v4以前の`reachable_with_cache_calls`や新しいcaller counterを、互いの代理指標にしてはならない。

`aggregate.csv`はframe sampleをrun間で混ぜず、各runのp50/p95/p99/maxを先に出し、その値の中央値とMADをcaseごとに出す。initial fixture checksum、warm-up checksum群、post-capture teardown warning件数も併記する。invalid runを黙って除外せず、session全体をinvalidにする。`summary.csv` schema v2の既存artifactにはtask execution counterがなく、schema v3以前のartifactにはreservation sync counterがない。frame-time比較は可能だが、存在しないcounterを0としてM1以降と比較してはならない。

新規sessionの`manifest.json`はschema v2である。manifestに列挙したcase、`repeat`、
`preflight_runs`と実ディレクトリ集合をaggregation前にexact比較し、case/run欠落、未知run、
重複case、`validation.json`欠落、invalid preflightをsession全体の失格にする。preflight値は
aggregateへ含めないが、その失敗を無視して本測定だけをvalidにはしない。`frames.csv`もexact header、
0始まりの連続index、finiteかつ非負のframe timeを要求する。schema v1以前の既存manifestは再集約互換を
維持するが、v2のformal bundleとしては扱わない。

schemaが異なる過去artifactの共通frame-timeは、対応する単一変更の**履歴上の参考値**にだけ使える。現行実装全体の改善率を示す場合は、schema v11・同一workload/fixture・同一計測matrixで採ったbaselineとcandidateを比較し、異なるschemaや別workloadの結果を合算してはならない。

既存artifactの再集約と、互換なsession同士の比較には次を使う。

```bash
python3 scripts/perf.py summarize target/perf-runs/gather-intel-vulkan-seed-20260712

python3 scripts/perf.py compare \
  --baseline target/perf-runs/baseline \
  --candidate target/perf-runs/candidate \
  --metric p50 \
  --max-regression-pct 5
```

`summarize --warmup-checksum-policy record|require`は、既存artifactのCSV/log検証結果を保ったまま、以前に適用したwarm-up policyだけを再評価する。たとえば調査時の`require`失格を、標準の`record`へ戻して再集約できる。

`compare`はmatrixと実adapterが異なるsession、または各caseに3 valid runがないsessionを比較しない。異なるマシンの値は参考値として扱う。

正式matrixの一部caseだけを再測定する場合は、明示的に`--allow-case-subset`を付ける。この場合もworkload、seed、反復数、warm-up/measure秒数、checksum policy、custom population、計測mode、要求環境、実adapterは一致し、candidateのsize/renderがbaselineの部分集合でなければ失格にする。

```bash
python3 scripts/perf.py compare \
  --baseline target/perf-runs/full-baseline \
  --candidate target/perf-runs/large-cpu-candidate \
  --allow-case-subset \
  --metric p50
```

## 新しい workload への展開

別の最適化対象でも、同じrunnerとartifact契約を使う。新しいworkloadは、手操作や既存saveへ依存させず、次の順に追加・採取する。

1. `PerfWorkload`とscenario setupに名前・決定的な操作列・必要entity数を追加する。初期fixture checkpointより前に配置を完了し、master seedから専用substreamを分ける。
2. `--workload <name> --sizes small,medium,large --renders cpu,gpu`の短縮runを3反復し、initial fixture、実adapter/backend、marker前logが全て有効であることを確認する。失格artifactは削除せず残すが、比較値にはしない。
3. 標準の30秒warm-up / 60秒measure matrixを3反復する。frame-timeはCaptureだけ、対象system CPUは専用sidecarまたはTracy、allocation / RSSはMemory、draw/passはRenderDocへ分ける。
4. 最適化前後は同じseed、population、window/backend、adapter、present mode、runner versionを使い、`compare`でcaseごとに比較する。workloadの意味やfixtureが変わった場合は新しいbaselineとして扱う。

marker前のwarning/errorは、allowlistへ追加して通すのではなく、発火したsystem・deferred command順・target/sourceの存続条件を特定してから修正する。特にBevy Relationship警告は「存在しないtargetへのinsert」を示すため、targetのdespawn処理だけでなく、同じmessage/deferred command batch内の後続insertも監査する。

## 直接実行のデバッグ

runnerを使わない調査時も、asset rootと出力先は必ず固定する。

```bash
cargo build --profile profiling -p bevy_app@0.1.0 --no-default-features --features profiling

BEVY_ASSET_ROOT="$PWD" \
WGPU_BACKEND=vulkan WGPU_ADAPTER_NAME=Intel \
HW_WINDOW_BACKEND=wayland HW_PRESENT_MODE=novsync \
target/profiling/bevy_app \
  --perf-scenario --perf-seed 20260712 --perf-size medium \
  --perf-workload gather --perf-render cpu \
  --perf-output-dir "$PWD/target/perf-runs/manual-debug/data"
```

直接実行はartifact manifest、adapter検証、反復集約を作らないため、最終比較には使用しない。
