# パフォーマンス計測

ランタイム最適化の比較は、`scripts/perf.py` を唯一の入口にする。このファイルは後方互換CLI shimであり、実装は `scripts/perf_tool/` の引数解析・実行・artifact・policy・集約・比較モジュールへ分割されている。各moduleは所有元から必要な名前だけをimportし、`artifact_io.py`がCSV/JSON/hashの共通I/O、`artifact_readers/`がSave transaction・deconstruction・lighting・Wallのschema readerを所有する。`fixtures.py`はfixture生成だけを持ち、assertion corpusを持つ`selftest.py`は`self-test` commandでだけ遅延importされる。runnerは profiling binary を計測外で一度だけbuildし、runごとに隔離したCSV・log・実行環境を保存してから、CSV契約、実GPU、ログ健全性、反復のcheckpointを検証する。正式artifactは`target/perf-runs/`、短縮smokeとnative acceptance jobは`target/native-acceptance/`または`target/perf-runs/`へ置く。`/tmp`は小さいlock以外に使わず、Cargo target、compiler temporary、binary、trace、session artifactを置かない。どちらもcommitしない。

interactive Cargoとは `target/.cargo-activity.lock` を共有し、performance recipe全体は
exclusive leaseを保持する。lease取得に失敗した場合はCargo/game/RenderDoc childを起動せず、
対話Cargo完了後に再試行する。既存のnative acceptance lockは内側で引き続き保持する。

ゲーム側の入口は `crates/bevy_app/src/plugins/startup/perf_scenario.rs` に維持し、設定、fixture、workload driver、capture driver、audit checksum/encoding、出力処理は同名ディレクトリの子モジュールが担当する。CLI option、summary schema、checkpoint順序はこの物理分割に依存しない。

再現性は二段階に分ける。通常の実時間ベンチマークは、ゲーム更新前の**初期 fixture**を必ず一致させ、warm-up/計測終端の状態は実測値として記録する。profilingが有効なautomated fixtureは`Time<Virtual>`をpauseした状態で専用spawn / deferred apply / setup / initial checkpointを完了し、realtime captureはcheckpoint後に明示的にunpauseする。これにより通常の`Logic` / `Actor`が初期actorへ可変delta更新を1frameだけ先行させる経路を持たない。`Time<Virtual>`はwarm-up開始後に実フレームのdeltaで進み、warm-up境界を越える最終frameがrunごとに異なるため、warm-up/measure終端checksumは既定では記録だけを行う。完全に同じsimulation時刻での状態一致は、`scripts/perf.py audit` による固定stepの決定性auditとしてframe-time計測と別に扱う。auditは性能値を出力せず、通常の`summary.csv` baselineとも比較しない。

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
`ba5d6bf7320426b441465df8fae42d6ff80820748ce55e0edf0dbba409dc755a`、fixture hashは
`a688d564f8f50c2fdcdbe49dca7625b2cb05d01f8555378215fb8ba89b553eed`である。layout hashはsmall
`e87a3b1aeb7ee1fbe334d311ad731bef24ce90ec80066af1e35c006ef4273af2`、medium
`e18320b3bcf8089c1ea2743003eadd79a0c938caa44682ed414e9d9d54af8f2d`、large
`3dec65d6c30ee9b88678af28a818a05fa70ededc66f20242ff78dcb6772c56fd`である。session manifestは
選択したsizeごとのchecksum mapを持ち、run metadataは当該caseのchecksumを持つ。
P05 evidence定義を補完したadditive改訂前のhash
`121a365ac3349cd4fa7890ab3069f0392098ced17e0d47f920095a1490c2ba11`は、fixture不変かつP04以前の登録済みstageに限って
compatible predecessorとして保持する。baseline verifierは旧stageのraw inventory / locator / SHA ledgerを構造再検証し、P05以後には旧hashを受理しない。

frame-time / fixed-stepの両runは`data/window.csv` schema v1を必須sidecarとして出力する。windowed runは
開始・終了時のlogical / physical size、scale factor、RtT品質、Scene target寸法、legacy mask target寸法（P01以降は不在を示す0）、resolved window
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
indoor-light static laneではseed済みDoor state自体がfixture topologyなので、fixed auditがsimulation tickを
進めてもDoor auto-open / closeだけをprofiling run conditionで停止する。Doorのproduction automationは通常playと
behavior laneで維持し、P02 behavior artifactがClosed→Open→Open→Locked→Lockedを別途検証する。fixture setupは
static fixtureはDoor domain stateとlegacy child Spriteの期待画像handleを同じsetup transactionでseedする。
その後のproduction presentation syncを通過してからchild image / 3D stateを検証し、固定sceneの不一致を明示的に失格にする。
またP02はlegacy Soul / mask / shadow / Familiar proxyを全render modeで0とし、置換後のSoul billboard / Familiar
foregroundは`p02_presentation.csv`で個体数とexactly-one presentationを検証する。

P03はP02のstatic / behavior / Capture / Memory / RenderDoc evidenceを維持したまま、`field-core` laneを1 case追加する。専用runnerは`p03 / large / cpu / seed 20260803 / headless / Vulkan`を固定し、3 runを実行する。

```bash
PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py field-core \
  --output target/perf-runs/<fresh-session-name>
```

各runはcanonical pure fixtureをtimer外で構築し、32 warmup後の`hw_infra::lighting::rebuild_field`だけを256回測る。`data/indoor_light_cpu.csv`はexact 256 row、`data/indoor_light_field.json`は100×100、50 emitter、radius 5、4 checksum、600 steady updateのno-op count、明示的なowned bufferの論理allocation scopeを持つ。field-core runのdata file setはこの2件だけで、window/ECS fixture/GPU field artifactを混ぜない。P03 gateは3反復のp95 median 2 ms以下、p99 median 4 ms以下、repeat間allocation一致を要求する。正式bundleは既存18 caseとfield-core 1 caseの合計19 caseであり、256×3 measurement rowをcase数として数えない。

P07は`consumer-core` laneを追加する。large production fixtureの500 Soul／16 Room／576 cellを使い、productionの
epoch-aware field readerとRoom state readerを32 warmup + 256 measured call × 3 runで通す。

```bash
PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py consumer-core --stage p07 \
  --output target/perf-runs/<fresh-session-name>
```

各runは`indoor_light_consumers.csv`（exact 256 rows）と`indoor_light_consumer_proof.json`
（`consumer-proof-v1`）だけを持つ。validatorは各Soul/stepのsample=1、effect≤1、epoch/revision整合、
mask/stale effect=0、p95/p99、scoped allocation=0をraw artifactから再計算する。P07 behavior 7 caseは
`indoor_light_consumer_lifecycle.json`（`consumer-lifecycle-v1`）を追加し、old-epoch recovery effectと
Room summary readが0であることをtimelineとは独立に検証する。P07 formalはP05 lineageのclean subjectで採取し、
P06 GPU ownerとの統合証跡はP08で扱う。

P08は4 laneを同一serial subjectで有効にし、P02 presentation、runtime field、P06 GPU owner、P07 CPU
consumerを合成する。`static`のGPU Capture / Memoryは`indoor_light_gpu.json`、behavior 7 caseはfield / GPU
lifecycleと`indoor_light_consumer_lifecycle.json`、field-coreとconsumer-coreはそれぞれ既存のexact file setを
維持する。RenderDoc legだけは`indoor_light_cross_consumer.json`（schema v1）を追加し、同一paused checkpointの
CPU publication、GPU uploaded epoch / revision / checksum、production Soul recovery observation、全Room summary stateを
raw factsで照合する。Python validatorはproducerのbooleanを信用せず、Soul sample数、stale effect 0、Room state数と
topology validityを再計算する。P08以外のstageとRenderDoc以外のlegにcross sidecarが存在した場合は失格にする。
GPU owner stageの`pixel_probes_pass`は、owned `Rgba8Unorm` Light Field imageをGPUから直接readbackしてCPU packed
RGBAと一致させる動的probeと、`topdown_structural_material.wgsl`でLight Field加算が
`main_pass_post_lighting_processing`より前にあることを埋め込みsourceから固定する順序checkの積である。probe専用PBR
cameraを毎frame readbackしないため、probe自身がRenderDoc下の600 steady-update証跡を直列stallさせることはない。

一般frame-time `summary.csv`のschema v11にある`energy_lamp_steps`と
`energy_lamp_candidates_scanned`は履歴artifact比較専用の予約列で、P07以降は常に0である。
runtimeの旧Lamp走査counterは削除済みで、consumer workの正本は上記consumer-core artifactとする。

P04は同じ19 caseを`--stage p04`で実行し、P03 pure field artifactを変更せずruntime証跡を追加する。audit / Capture / Memoryとdoor behaviorは`indoor_light_runtime.json`を必須とし、typed emitter `2 / 11 / 51`、eligible supplied `1 / 10 / 50`、unsupplied adoption 0、Room mask cell `36 / 144 / 576`とcanonical checksumを検証する。`load-normal-v1`はP05のlight lifecycle ownerより前なので、world replacement後にruntime-only generator workerを復元せず、typed emitter 2、eligible supplied 0のliveなfail-dark fieldをP04の正規結果とする。RenderDocはP02/P03の置換後presentation inventory（2D camera 2、active 2D pass 1、legacy Soul/Familiar proxy 0）を維持し、同値をruntime checkpointの必須`runtime_field` blockへ記録する。qrenderdoc extractorとoffline bundle validatorはhistorical schema v3と現行schema v4をstage-awareなkey集合・値制約でfail-closedに検証する。behavior timelineは`field_availability=available`とlive input/output revision、dark state、field checksumを持つ。

P04 field-coreはproduction large ECS fixtureを通常のcompletion / energy / Room / lighting scheduleで構築してから600 Updateを実走する。fixture settle開始時に`Time<Virtual>`をpauseして通常のLogic / Actorによるworld mutationを止める一方、pause gate外のPreActor / PostActorは継続し、manual Door mutation境界とproduction lighting collect / rebuildを実測対象に保つ。fixture validationは`DoorPresentationSyncSet`後にdomain state、WorldMap、presentation topologyを照合してReadyを公開する。static / RenderDocの固定fixtureはDoor stateとlegacy child Spriteをsetup transactionで同時にseedし、consumer後のvalidationで維持を検証する。behavior laneはheadlessのrenderer visibilityに依存しないようCapture observer境界で同じproduction Door consumerを再実行し、Open / Closed / Locked遷移を別artifactで検証する。P04 behaviorはfixture Readyだけで開始せず、runtimeのinput/output revisionが3回連続で不変になった点をstep 0にしてsave所要時間によるtimeline差を除く。save latencyはbehavior契約の計測対象外なので、このfixed behavior laneだけ通常の100 ms slow-save warningをinfoへ落とし、save artifactとtimelineのexact validationを成否に使う。field-core driverは次Updateからsteady windowを開始する。headless field-coreはrenderer presentationを計測対象にしないためlegacy child Spriteの画像handle一致を要求せず、同じOpen / Closed / Locked画像契約はX11 static / behavior / RenderDoc各legで必須にする。`indoor_light_cpu.csv` / `indoor_light_field.json`に加え`indoor_light_runtime.json`を出し、steady full scan / rebuild / revision increment / scoped allocation event・byteが0、最大rebuild/updateが1以下であることと、emitter collectが明示的に所有するbufferの論理allocationを別scopeで検証する。

P06はP05までのCPU／lifecycle artifactを継承し、GPU static Capture／Memoryに`indoor_light_gpu.json`を追加する。sidecarは単一image／handle、logical payload、padded staging、changed revision upload、steady no-upload／allocation、uploaded epoch／checksumをexact schemaで保持する。behavior timelineはP06だけGPU availability／epoch／checksumをlive値にし、preflight rejectではinitial／terminal GPU publicationを維持し、replacement reset rowではblack化を`unavailable/null`として記録する。2026-08-17のproduction headless diagnosticでは全7 behavior caseを各3反復し、21 / 21 runがartifact validatorを通過した。これはrenderer／performance formal evidenceの代替ではない。

P06 RenderDoc checkpointは`gpu_light_field` blockを必須にし、field image／handle、receiver全materialのshared handle、revision upload、steady 600 Update、Point／Spot／shadow map／local pass／mask／duplicate 2D pass増分を同じcheckpointへ投影する。projectionの`gpu_upload` groupはGPU Capture／Memory／RenderDocだけでavailableとなり、behavior／field-coreやP05以前の`stage_before_gpu_owner`をP06証跡に流用しない。正式なP06 gate登録はclean committed subjectのS0→S1→RD0→formalとoffline verifierを完了してから行う。

各規模は`indoor_light_fixture.csv` 1行と、small 187 / 5、medium 722 / 12、large 2306 / 12行の
`indoor_light_layout.csv` / `indoor_light_presentation.csv`を必須出力する。indoor semantic actorはsmall 78、
medium 258、large 936件を全checkpointでexact検証する。generic actorはSoul / Familiar / Designationを
checkpoint宣言値へ照合し、fixture外のproduction designationをindoor契約へ誤って固定しない。
Room entityはproduction検出で再生成され得るため、Entity IDをhashせず、floor cell集合、wall / Door境界、
bounds、reverse lookupから毎checkpoint一意に再同定する。

短縮headless smokeは次のshapeで実行できる。129 + 16 tickは経路確認専用であり、
formal baselineやrenderer / GPU性能証跡には使わない。

```bash
PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py audit \
  --workload indoor-light --contract rtt-light-v1 \
  --stage current --lane static --sizes small,medium,large --renders cpu \
  --seed 20260803 --repeat 1 --preflight-runs 0 \
  --backend auto --window-backend headless --present-mode novsync \
  --fixed-hz 64 --warmup-ticks 129 --audit-ticks 16 \
  --allow-log-pattern 'driver that only supports software rendering' \
  --output target/perf-runs/rtt-light-current-static
```

ローカルVulkan loader由来の追加ERRORを診断用regexで許可したrunはformal evidenceに昇格させない。
P00 currentのcanonical formal baselineは2026-08-11に登録済みである。subject commitは
`10763a4da6bfbe0b480971fb85c474e6ff7a5f86`、attempt IDは
`9e813f24-0f7b-47f5-8a8d-e3ff34775370`、source fingerprintは
`db34c8fc901a4c3ecdd80158f391bf986842323641c6a593daef67a6d89d7bce`である。audit / behavior /
Capture / RenderDoc / Memoryの5 leg、18 case、各realtime / fixed-step case 3反復、RenderDoc 1 frameを登録し、
raw artifact 884件のdirectory SHA256は
`a9f4927186fe7c8c5f009583fd645cd7963b6ad36398e9d614fbcbbff1f8a6aa`である。登録済みattemptはnative helperの
`verify-rtt-light --repo … --attempt …`、baseline rootは`python3 scripts/perf.py
verify-rtt-light-baseline --baseline …`で再検証する。canonical registryの`baseline-index.json`と
`SHA256SUMS`を正本とし、個別artifactの絶対pathはhost内の診断locatorとして扱う。
履歴readerは`BEVY_ASSET_ROOT`を現在のcheckoutではなく記録済みmanifestの`repo_root`と照合する。windowed matrixの
`environment_lock`絶対pathも非意味論的なhost locatorとして、非空文字列とmanifest / matrix.json間の一致だけを確認し、
lock payloadと各raw runは別途再検証する。

P01 Scene-onlyのcanonical formal candidateは2026-08-12に登録済みである。subject commitは
`29a4a719e9fe92b10618f36ce548c4bb5a4c7e80`、attempt IDは
`8bc82f04-10ac-4903-89b6-89011dacdada`、source fingerprintは
`27d3d59b39a83be5d61df09c3f07c70e26cab2f16bbfb1136acca8d7412c5fbc`である。P00と同じ5 leg・18 caseを登録し、
gate ledgerは123 / 123 row pass、raw artifact 884件のdirectory SHA256は
`68e470e51cf30f7659bb87eb1893235758d49f2c9d7a1e8f881f0e5f2a9f7502`である。`baseline-index.json`の
`stages.p01`とattempt manifestを正本とし、`verify-rtt-light --attempt …`で再検証する。

P02 TopDown presentation subjectは `--stage p02` を Rust / Python / native launcherの明示selectorで受理する。current / P01の既存schemaを変更せず、P02以降のCapture / Memoryは `p02_presentation.csv`、RenderDocはruntime checkpointの同名blockを必須にする。Door behavior validatorはP02以降でClosed→Open→Open→Locked→Lockedを要求し、current / P01のhistorical Closed-only timelineを維持する。P04ではmanual lockだけInterface requestから次UpdateのPreActorへhandoffされるが、script rowは同じsemantic stepを記録する。bundleは `RLV1-P02-DOOR-DOMAIN`、`RLV1-P02-PRESENT`、`RLV1-P02-PERF` のexact rowを生成し、P02 frame p95/p99は登録済みP01 projectionをreferenceにする。

P02 TopDown presentationのcanonical formal candidateは2026-08-14に登録済みである。subject commitは
`c3515a40543026a588592682889a08d67cbaeff9`、attempt IDは
`9ff336ef-1312-4248-b0bf-bb454111decc`、source fingerprintは
`6e6e37c5cbc898b6f39bf85b2362e854f3ad10bbe176c1e41abe4fd007cd5f02`である。Intel Arc (MTL) / Mesa 26.1.5 /
Vulkan / X11でAudit / Behavior / Capture / RenderDoc / Memoryの5 legを登録し、gate ledgerは128 / 128 row pass、raw artifact
932件のdirectory SHA256は`a6b76b64ca8df601d051abeb15589ffd464fd1bd41dfe61c9f25fa734f6a0e72`である。P01比のframe hard gateは
p95 / p99とも全6 caseで5%以下となり、正の最大値はmedium / cpuのp95 +2.52%、p99 +1.96%だった。
`baseline-index.json`の`stages.p02`とattempt manifestを正本とし、`verify-rtt-light --attempt …`とbaseline history readerで再検証する。
subject `6ea0bf99` / attempt `54d85a63-e237-4501-a0d0-33c1d0a29f3b`はfrozen v1 formalの履歴として保持する。

最初のformal candidateはP02 presentation syncが静止中も全Building / Soulを走査していたためframe gateを満たさず、
登録しなかった。owner transform・presentation state・actor visual stateの変更時だけ同期する経路へ修正してから、同じ
frozen contractとP01 referenceでcanonical candidateを採り直した。candidate結果を理由に閾値は変更していない。

P02のpixel / animation補完は専用`p02-presentation-actual-window-v9`（schema 7）profileで行う。入口は
`.codex/skills/hell-workers-run-native-acceptance/scripts/p02_presentation_acceptance.py plan`であり、
High / Medium / Low × DPI 1.0 / 1.5 / 2.0 × Render3d visible / hiddenの18 caseをproduction
`indoor-light/p02/static` fixtureから逐次採取する。各caseはDoor Open / Closed / Locked、Soul前 / 後、Bridge、
Wall bounce active / rest、Foreground animation A / Bの10 ready phaseを持つ。Rust側はproduction ownerとphase nonce /
generation / ROIをsidecarへ出し、Python側はReadyなgenerationをACKしてからprocess tree所有のX11 clientだけを採取する。
Doorの3状態、SoulのWall前後とalpha-masked差分、Bridgeのvisible / hidden差分、Wall bounce、Foregroundの局所animationを
PNG pixelsから再計算する。Bridge phaseは画像差分だけに依存せず、RtT cameraとの`RenderLayers`交差、期待するBridge
mesh / material handle、両assetのregistry在籍もsource側でfail-closedに確認する。

manifest・observation・raw performance validation・binary / source / harness / runtime asset provenance・PNG hash / ROIを
相互照合し、unknown file、symlink、phase欠落、sidecar / PNG / CSVの改竄をfail-closedで拒否する。headless、`visual_test`、
root desktop screenshot、case欠落は代替証拠として受理しない。旧v1〜v8 profileのartifactはschema / profile mismatchであり、
v9のP02完了証跡としては受理しない。

v9 actual-window artifactはsubject
`c3515a40543026a588592682889a08d67cbaeff9`、source fingerprint
`6e6e37c5cbc898b6f39bf85b2362e854f3ad10bbe176c1e41abe4fd007cd5f02`、harness fingerprint
`cd4fc4a06669a82884ebced366941e556d746824247f1a91e2725beccb15cb5e`、runtime asset fingerprint
`97c17421b1438331f7a9cb59ab66cd703aa7da10fef99f567a54d16310c49a83`、profiling binary SHA256
`55496636014ca5e98eea93ce5eb80d8ab9f25dc72fa2d7c6e1822a6433c61b8f`で封印する。Intel Arc (MTL) /
Mesa 26.1.5 / Vulkan / X11、1280×720 client windowで18 / 18 caseが完了し、`job.json=status: valid`、
`manifest.json=status: pass`、独立`verify`がpassしたことを記録する。絶対pathではなく、これらsealed metadataと
各case artifactを正本とする。

2026-08-11 の diagnostic RD0 では Intel Arc / Vulkan / X11 の実ゲームから 699,959,528 byte の RDC
（SHA256 `aaf0f73c02baebf018ad69f0c229ee0570b52c26bb9bc7df5c183c00a71243b8`）を採取し、
requested App API 1.6.0 に対して returned 1.7.0、schema v3 checkpoint、orphan 0 を確認した。同一RDCの
local replay 2回は normalized topology digest
`554785d1b484efac994568d2311945e7c5ab8affdb2998eec5e18796673862cf` で一致した。これはactual-window
経路の成立証拠だが、`target/native-acceptance/renderdoc-foundation/` のdiagnostic artifactであり、formal registryへ
移動・昇格しない。

同日のstage-start gate修正後のformal RD0では、開始時`MemAvailable` 13.10 GiBで8 GiB gateを通過し、実行中に
約7.1 GiBまで低下してもcapture / 二重replayを継続した。700,992,434 byteのRDCと2つの抽出JSONを生成したため、
8 GiBを開始後のkill条件にしない経路は実機で成立している。日本語text処理時の完全一致診断
`ICU4X data error: No segmentation model for language: ja`は、近似regexではなく既知の非致命行として共通log分類器で
数える。qrenderdocのGTK theme診断も完全一致shapeだけを許可し、近似行やその他のERRORは引き続きformalを失格にする。

formal再試行ではgeneration-scoped environment lockを再利用する。Capture preflightは自分が所有しない既封印の
RenderDoc / Memory capsule hashを消去せず、各leg ownerだけが該当hashを確定する。RD0とformalは同じRenderDoc binary /
sealed capsuleを要求するが、別々のRDCに含まれるvolatile event IDやresource IDのためreplay digestの一致は要求しない。
各RDC自身の二重replay integrityとnormalized topology gateを独立に検証する。offline registrationではcanonicalな
RDC relative locator、SHA256、byte sizeを正本とし、capture時scratchのabsolute pathは絶対pathであったことだけを確認する。
capture-time validatorは実際のscratch pathとのexact一致を維持する。

RenderDoc は `profiling-renderdoc` 専用 Cargo feature と専用 Cargo profile の binary capsule を使う。`wgpu-hal 29.0.4` は `debug_assertions=false` の build で RenderDoc bridge を無効化するため、専用 profile は `profiling` を継承しつつ debug assertions を有効にする。また、build peakを抑えて後続stageの8 GiB開始ゲートへ到達しやすくするためLTOを無効化し、codegen unitを16に固定して、mutable outputを`target/profiling-renderdoc/bevy_app`へ分離する。Capture (`profiling`) / Memory (`profiling-memory`) と SHA を共有せず、environment-lock schema v2 が leg ごとの hash を保持する。mutable output は build 直後に read-only capsule へ封印し、RD0 と formal は `profile=profiling-renderdoc`、同一 capsule ID / SHA / build fingerprint だけを受理する。runtime checkpointはP08 cross factsを追加したschema v4を現行producerとし、既存登録済みreferenceのschema v3はhistorical readerでのみ保持する。P08はv4以外を拒否する。実測fixed simulation tick、連続4 GPU-ready frame（`ready_frame_ordinal == capture_frame == 4`）、pre/post GPU-ready signature、requested / returned App API（major 1かつ1.6以上）、wgpuのdevice-selected / null-window capture strategy、raw `.rdc` のSHA256 / byte sizeをartifactだけから再検証する。composite の Vulkan descriptor contract はstage-awareで、P00 currentはScene `(1, 2)` + mask `(3, 4)`、P01/P02はScene texture / sampler `(1, 2)`だけを許可する。P02は同じScene-only topologyでも独立stage entryとpresentation checkpointを必須にする。formal 前の actual-game RD0 は同一 RDC を2回 local replayし、volatile resource IDを除いた normalized topology digest が一致した場合だけ formal を許可する。RD0 と failure diagnostic は `target/native-acceptance/renderdoc-foundation/<uuid>/` に置き、canonical registry へ昇格しない。schema v1/v2 checkpoint は登録できない。
static preflight の `qrenderdoc --version` / `--help` は Qt の display 自動接続を行わない `QT_QPA_PLATFORM=offscreen` で実行する。これは tool metadata probe だけの契約であり、actual-window RD0 / formal capture と local replay の実行環境を offscreen へ置き換えない。

### 許可ダイアログなし実機受入

実機確認、実機テスト、actual window、renderer / GPU / backend、native performanceの受入では、repository Skill `hell-workers-run-native-acceptance`を毎回使う。Skillを利用できないagentは`.cursor/skills/hell-workers-run-native-acceptance/SKILL.md`を直接読む。Task Dashboardの標準recipeは次のplan commandを入口にし、返された`launcher_command`を先頭の`kitty`を変えずに直接実行する。

```bash
PYTHONDONTWRITEBYTECODE=1 python3 \
  .codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py \
  plan-task-dashboard --repo "$PWD" --adapter Intel \
  --backend vulkan --window-backend x11
```

helperはUUID付きの一意なartifact rootとatomicな`job.json`を使い、fixed audit、実window Capture、native Memoryをrepository-wide lockの内側で逐次実行する。各sessionは`perf.py`自身に対応featureのbuildを行わせ、`--skip-build`と任意`--binary`を使わない。これにより、`target/profiling/bevy_app`が直前のMemory flavorであるのにCaptureとして記録する取り違えを防ぐ。CaptureとMemoryの間ではbinary hashが変わり、fixed auditとCaptureでは一致することをfail-closedで検証する。

配置不能理由、save/load結果通知、dedupe、Pause中expiry、toast/history入力境界のTrack A2受入は、
性能recipeへ混ぜず専用actual-window profileを使う。

```bash
PYTHONDONTWRITEBYTECODE=1 python3 \
  .codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py \
  plan-notifications --repo "$PWD" --adapter Intel \
  --backend vulkan --window-backend x11 --present-mode novsync
```

このprofileも返されたdirect `kitty` launcherと`status_command`だけを使い、save/settingsをjob固有runtimeへ
隔離する。結果は`verify-notifications`で再検証し、headless結果をUI/renderer証跡へ代用しない。

壁M0のBlender / Bevy色校正は`wall-color-calibration-v1`専用profileで行う。先に同じclean
source fingerprintからsealed OCIO configを使ったBlender reference PNG / metadataを生成し、次の
`plan`へimmutable pathとして渡す。

```bash
PYTHONDONTWRITEBYTECODE=1 python3 \
  .codex/skills/hell-workers-run-native-acceptance/scripts/wall_color_acceptance.py \
  plan --repo "$PWD" --adapter Intel \
  --reference "$HELL_WORKERS_ASSET_ROOT/staging/reports/<reference>.png" \
  --reference-metadata "$HELL_WORKERS_ASSET_ROOT/staging/reports/<reference>.json"
```

返されたdirect `kitty` launcherだけを実行する。profileは`wall-density-v1`のSmall / completed / GPUを
10秒warm-up＋10秒measureで1回実行し、通常wall calibrationとは排他的な
`--wall-color-actual-window`をRust側の`HW_WALL_COLOR_ACTUAL_WINDOW=1`と二重鍵にする。専用final
Camera2dの5 patchを単一X11 clientから320×96で無拡大cropし、4 base patchのCIEDE2000とemissive
luminance liftをofflineで再計算する。最高order cameraへ未指定UIが移るBevy 0.19の既定動作を避けるため、
既存MainCameraを`IsDefaultUiCamera`として明示し、専用cameraへのUI混入がないこともstatusで検証する。
PNG、metadata、OCIO proof、contract、source / harness / binary /
asset fingerprints、performance sidecarのいずれかが変われば`verify`は失敗する。この色artifactはcurrent
wall visual、12-run Capture、RenderDoc draw-group evidenceの代用にしない。

壁M5の同一binary内performance比較は、承認済みcandidateを配置したclean validation worktreeから
`wall_production_performance_acceptance.py plan --repo "$VALIDATION_WORKTREE" --adapter Intel`で計画する。
返されたdirect launcherは`fallback-control`と`production`を順番に実行し、completed / provisionalの
N=96 / 4N=384を各3 run、30秒warm-up＋60秒measureで採る。各runは既存
`wall_density_fixture.json` / `wall_density_layout.csv`に加えて`wall_density_presentation.json`を持ち、
指定modeへの全owner収束、candidate identity、resident poolとtriangle上限が開始／終了で同一でなければ無効になる。
profileはphase別p95 / p99比較CSVを固有名で保存し、production中央値がfallback-controlより5%を超えると
job全体をinvalidにする。

subject `991392b8`の正式job
`target/native-acceptance/wall-production-performance-20260902T173804Z-3f6bc903`は24 runと4比較を完走し、
独立verifyも`status=pass`となった。全8比較行の最大回帰はprovisional N p95の`+2.031%`である。
manifest SHA-256は`1d0ba117fcc755f303d29c170a7389777dcedc9b678f51f91dd4fb70badf3de1`、binary SHA-256は
`200234f59cdd8d74df629b95d954c39403396873233abbb4d77836618dbe1443`である。

resource preflightはnative recipe開始時に`MemAvailable` 10 GiBと実際のCargo target filesystem空き15 GiBを要求する。16 GiB以上ではCargo 2 job、未満では1 jobとし、`CARGO_INCREMENTAL=0`を固定する。各build / game / capture / replay stageの開始直前には`MemAvailable` 8 GiBを要求し、admission snapshotをartifactへ記録する。この8 GiBはstage開始ゲートであり、開始後に`MemAvailable`が一時的に8 GiBを下回ったことだけを理由に実行中processを停止しない。swapの使用量はmanifestへ診断情報として記録するが、RAMの下限を満たす場合の開始条件にはしない。Linuxで`MemAvailable`を読めない場合はstage開始を拒否する。helperは親環境の`CARGO_TARGET_DIR`を無視してworkspace `target/`へ、`TMPDIR` / `TMP` / `TEMP`を`target/.native-acceptance-tmp`へ固定し、`CARGO_HOME` / `RUSTUP_HOME`は安全な永続overrideだけを保持してtmpfs指定をaccount既定cacheへ戻す。job rootも`target/native-acceptance/`へ生成する。formal RenderDocのraw capture / replay workは`target/native-acceptance/renderdoc-foundation/<uuid>/`に置き、成功時だけscratchを削除する。失敗時はpartial raw、log、checkpoint、failure reasonを同じUUIDに保持する。capture/replay childは各600秒、RD0 outerは1,920秒、formal RenderDoc outerは1,320秒を上限とし、owned process groupをTERM→KILL→reapしてorphanを拒否する。capture前とRDC copy前には`max(15 GiB, 2 × RDC bytes + 1 GiB)`の同一filesystem空きを要求する。`/tmp`またはmemory-backed filesystemのjob / artifactを、default path・明示pathともに解決済みsymlink/mountまで検査して拒否し、残存`/tmp/hell-workers-*-target`はサイズを出して停止するが自動削除しない。`scripts/perf.py`と`scripts/dev.py`も同じくworkspace target・disk temporary・toolchain cache・最大2 Cargo jobsへ正規化し、Cargo compilationは`MemAvailable` 8 GiB未満では開始しない。Tracy capture / csvexportとRenderDocの子processもこのtemporary環境を継承する。通常のMemory受入はnative allocator + GNU timeを使い、Cargo/game/Capture/Memoryは並列化しない。artifactやCargo cacheの自動削除、別target directory、routineな`cargo clean`、`nice` / `ionice` / CPU affinityは行わない。

native formalのproduction subject fingerprintと起動・監視harness fingerprintは別に封印する。Python native helper、build coordination、Cargo runtime guardだけを変更した場合、同じclean validation worktreeとworkspace `target/`を再利用し、既存Rust binaryやvalid S0/S1を不要にfull rebuildしない。asset fingerprintはmtimeではなく内容をhashする。Cargo profile、feature、toolchain、Rust/Cargo source、または測定結果validatorの変更は証拠の意味を変えるため、該当capsuleを再build / 再検証する。

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
  --present-mode novsync --output target/native-acceptance/task-dashboard-x11
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

legacy `data/scene_roots.csv` はP00 / P01 historyのGLB root契約を保持する。P02ではGPU条件でも旧Soul main / mask / shadowとFamiliar 3D rootは0で、Soulのactive scene参加は`p02_presentation.csv`の`soul_count / soul_billboard_count`で1:1を検証する。CPU条件ではbillboardを含む3D scene rootを生成しない。

## 標準 workload

すべての workload は初期 checkpoint より前に決定的に生成される。手操作や既存 save を前提にしてはならない。

| workload | 固定負荷 | 主な counter |
| --- | --- | --- |
| `gather` | Gather designation と Familiar 指揮 | task / reservation / delegation |
| `path-door` | corridor、Door 開閉、両方向の Soul traffic | core A*、defer frame、Door近傍候補 |
| `construction` | Curing 中の Floor site（Small/Medium/Large = 16/64/128 tile） | construction site/tile、evacuation候補 |
| `ui-gpu` | Blueprint（Small/Medium/Large = 64/160/320） | UI/visual の描画条件 |
| `task-dashboard` | 同一task / Soul / Familiar集合とdashboard 3 mode | AI work、dashboard producer / render、Task Dashboard CPU / memory |
| `dream-ui-burst` | production Dream UI update / merge / trail経路で128 active particleを決定的に維持 | particle更新、merge比較、Node write、spawn/despawn、Dream lane CPU、RNG / trajectory / lifetime checksum |
| `deconstruction` | Medium固定の完成済み建築100棟（`BuildingType::ALL`の12種類を安定順で反復）からBonePileを1件だけcommit | deconstruction commit elapsed、target/order cleanup、回収Bone、steady-state scan |
| `save-transaction` | small / medium / large各fixtureでManual slot 1へ1回だけ同期save | serialize、temp file sync、commit + directory sync、total、body bytes、allocator peak-live growth、process max RSS |

`construction` は Curing footprint の安全監査を含む。完成済みの別 workload の数値を construction の比較値として流用しない。

`dream-ui-burst`はsmall / cpu / default population / hidden dashboard固定で、actual windowを必須とする。
profiling限定のmaster-seed RNGをproductionのparticle update / noise / trail経路へ渡し、通常buildの
`thread_rng()`経路は変更しない。fixed auditはmeasure境界でparticle / trail fixtureとRNGを再構築し、
stable ordinal順でparticle更新とmerge候補を処理する。Capture / Memoryはこの監査専用sortを使わず、
productionと同じquery反復経路を計測する。短縮経路smokeは次で実行できるが、1反復・1秒/2秒をformal baselineや
production candidateの採否へ使ってはならない。

```bash
PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py run \
  --workload dream-ui-burst --sizes small --renders cpu \
  --repeat 1 --preflight-runs 0 --warmup-secs 1 --measure-secs 2 \
  --adapter Intel --backend vulkan --window-backend x11 \
  --present-mode novsync --instrumentation capture \
  --output target/native-acceptance/dream-ui-burst-smoke
```

`deconstruction` は fixed-step audit 専用のCPU/headless workloadである。fixtureは100棟を生成し、
全12種類が少なくとも1棟ずつ存在すること、実際の `DeconstructionCommitRequest` が1件だけ
`Committed` になること、対象を除いた完成棟数が99になること、固定salvage表どおりBoneが5個増えることを
sidecarへ記録する。targetだけを`WorldMap`へ登録し、残り99棟は安定したECS人口として配置するため、
deconstruction対象のowner snapshotやroom/path topologyへfixture外のノイズを混ぜない。選択workerだけを直接
`AwaitingCommit`へ置き、残りのSoulは既存の`CommandedBy`待機経路へ固定する。owner Familiarは`Idle`かつ
`max_controlled_soul = 0`のため、warm-up境界で解体と無関係なidle/rest/pathfindingがfixed-step checksumへ混入しない。
この固定はplayer-facingな割当や解体経路を置き換えず、owner transactionだけを監査するfixtureの人口制御である。
`data/deconstruction_fixture.csv` は次の不変条件を満たさないrunを失格にする。

- `initial_completed_buildings=100`, `final_completed_buildings=99`, `building_type_count=12`
- `commit_requests=1`, `committed=1`, `recovery_items=5`, `commit_validation_passes=1`,
  `successful_cleanup_transactions=1`, `recovery_items_spawned=5`
- `post_commit_updates>0`, `steady_state_validation_delta=0`,
  `successful_transaction_elapsed_ns>0`

`commit_validation_passes` と `successful_cleanup_transactions` は、fixtureが値を埋める定数ではない。
profiling buildのfinalizerがcurrent-worldかつ非cancel/非duplicateなcommitを実際に検証した時だけ前者を、
cleanupを実際にapplyした時だけ後者と `recovery_items_spawned` を更新する。
`successful_transaction_elapsed_ns` はrequestの発行待ちを含めず、同じfinalizer呼出し内のvalidation + applyを
`Instant`で計測した経過時間である。`steady_state_validation_delta=0`はcommit後の更新で新たなvalidation passが
走らなかったことを示す（frame-timeやOS CPU時間の代替値ではない）。

このsidecarはframe-timeの速度値ではなく、fixed-stepで一度だけ通すowner transactionの経路証跡である。
したがって`summary.csv`やframe quantileは出力せず、他workloadのbaselineと混ぜない。

```bash
PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py audit \
  --workload deconstruction --sizes medium --renders cpu \
  --repeat 20 --preflight-runs 0 --seed 20260805 \
  --backend vulkan --window-backend headless --present-mode novsync \
  --fixed-hz 64 --warmup-ticks 1920 --audit-ticks 128 \
  --output target/perf-runs/deconstruction-medium-fixed-20260805
```

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

`save-transaction`はfixed-step auditに載せない。Capture timingとMemory/RSSを別session・別binaryで、
small / medium / largeそれぞれ3 preflight + 20 measured runの固定matrixとして逐次実行する。各runは
warmup 1秒、measurement 2秒の中で決定的fixtureからManual slot 1へ一度だけ保存する。preflightは
artifactに残すがaggregateには含めない。Capture largeのmeasured 20本だけからnearest-rank p95とmaxを
再計算し、totalが100 ms / 250 ms以下であることを要求する。Memory legはallocator peak-live growthと
GNU timeのprocess max RSSを記録するが、timing閾値には混ぜない。

正式実行は`hell-workers-run-native-acceptance` Skillの`plan-save-catalog`が返すno-prompt launcherだけを
使う。helperがactual-window save catalog V1〜V5の後にCapture、Memoryを順番に起動し、raw CSV、fixture、
source / harness / binary fingerprint、runtime cleanupを再検証する。`save_transaction.csv`はbody/pathを含めず、
`target/.save-transaction-runtime/<run-id>/`以下のsave/settings rootはartifact外に置く。raw processはrunごとの
exact rootを終了前に削除し、helperは最終bundle検証でも全派生rootの不存在を要求する。helperは保存済みの
`validation.json`だけを通行証にせず、window/log/environment/matrix、CSV schema、Memory収支、artifact集合を
再読し、unknown file・symlink・serialized save bodyをfail-closedで拒否する。`perf.py audit --workload save`や
generic fixed auditはこのworkloadのtiming/RSS証拠として使わない。

actual-window V1〜V5のscreenshotはX11限定である。monitorはroot desktopを撮らず、起動Cargo process treeと
`_NET_WM_PID`が一致する唯一のclient windowを直接撮影し、`x11-client-window` scope、window ID、PIDを
driver resultとackで照合する。ready/ackはatomicにpublishし、X11照会とcaptureは各5秒のbounded callとする。
driverはack受理時点でもSave catalogのforeground captureが継続していることを要求する。これにより別windowや
desktop overlay、またはcatalogを閉じた後に残るmarkerの画像をSave catalog UI証跡へ混入させない。

### Dense Tile anchor削減の検証結果（2026-08-27）

Dense anchor削減は、同一seed/fixtureのprofiling限定A/B
`target/perf-runs/tile-anchor-dense-{capture,memory}-20260826` と
`target/perf-runs/tile-anchor-omitted-{capture,memory}-20260826`で削減上限を先に固定し、
production実装後にSave Catalog recipe
`target/native-acceptance/save-catalog-20260826T235536Z-372acde8`で実経路を閉じた。
後者はIntel Arc / Vulkan / X11、V1〜V5、Capture/Memory各60 measured・invalid 0、各case 3 preflightでvalidである。

| large指標 | Dense baseline | production sparse | 改善 |
| --- | ---: | ---: | ---: |
| save body | 4,058,025 bytes | 1,036,182 bytes | 74.47%減 |
| Capture serialize p95 | 50,721,781 ns | 15,201,681 ns | 70.03%減 |
| Capture total p95 | 73,332,099 ns | 35,711,658 ns | 51.30%減 |
| Memory peak-live growth p95 | 5,998,168 bytes | 1,432,071 bytes | 76.12%減 |
| Memory process max RSS | 1,374,840 KiB | 1,378,428 KiB | 0.26%増（非gate） |

production closureはseed 20260827、Dense baselineはseed 20260826のため、両artifactを
`perf.py compare`の同一fixture比較として扱わない。採否の同一seed A/Bは上記omitted artifactが担い、
production closureは同じ3規模・population・runner schemaで削減量が再現し、実際のsave/load UIと
current canonical `Tile` 0経路が成立することを確認する。RSSは従来どおり改善主張に使わない。

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
PYTHONDONTWRITEBYTECODE=1 python3 \
  .codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py \
  plan-task-dashboard --repo "$PWD" --adapter Intel \
  --backend vulkan --window-backend x11
```

返されたdirect `kitty` launcherを実行する。個別の`--skip-build` / `--binary` / `/tmp` outputを組み合わせてmatrixを手作業で組まない。helperが安全なworkspace target、disk temporary、逐次build、job root、Capture/Memoryのbinary hash契約をまとめて所有する。

Capture / Tracy / Memory / RenderDoc は標準 baseline として混ぜない。各 session の直前に対応する `profiling` / `profiling-tracy` / `profiling-memory` / `profiling-renderdoc` feature で build し、manifest の binary hash を証跡にする。

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

scenario driverは `Warmup → Measure → Flush → AppExit` を自動遷移する。realtime phaseの終了判定と`summary.csv`へ記録する経過秒は同じf64 deltaを積算し、f32の先行丸めによって要求時間未満のrunを確定しない。各checkpointのinitial、warm-up終端、measure終端のentity数・Designation数・state checksum、実際のvirtual/real秒数、p50/p95/p99/maxは`summary.csv`に入る。`gather`、`path-door`、`construction`、`ui-gpu` はすべて専用 fixture を持つため、異なる workload の結果を相互の速度比較に使わない。

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
      data/transport_request_changes.csv # construction / task-dashboard frame-time
      data/spatial_query_metrics.csv # path-door / gather frame-time
      data/dream_ui_metrics.csv      # dream-ui-burst Capture / Memory / fixed audit
      data/memory.csv              # Memory build
      data/deconstruction_fixture.csv # deconstruction fixed-step owner-transaction contract
      profile-artifact.json
      resource-usage.txt           # Memory build
```

fixed-step auditでは`frames.csv`と`summary.csv`の代わりに、`data/determinism.csv`と`data/determinism_records.csv`を出力する。

`summary.csv` schema v11には、frame-timeに加えcapture期間全体の task execution / reservation / delegation counter、candidate snapshot / score、Top-K、wheelbarrow arbitration、caller別 runtime A* と defer counter、dashboard producer / render、Door候補数、construction の site/tile/evacuation counterを入れる。さらにslow simulationのstep / 更新Soul / idle decision / sanity auditと、energyのoutput / grid / lamp候補counterを入れる。task executionの`souls_queried`は全Soul数ではなく、active identityまたはfail-closed edgeから実際にrandom-access取得した数であり、`idle_skips`はstale identityを持つidle edgeだけを表す。`aggregate.csv`には各counterの中央値/MADと、run内で割り算してから集約したidle skip比率・handler到達比率を併記する。これらはframeあたりの値ではないため、比較時は同じmeasure秒数でのみ用いる。別々のcounterを独立に中央値化した値どうしを引き算して比率を作ってはならない。

`transport_request_changes.csv` schema v2は`construction` / `task-dashboard`のframe-time runだけが出力する。13種の`TransportRequestKind`を固定順で1行ずつ持ち、measure区間の`observer_runs`、`changed_components`、`added_components`、`changed_existing_components`に加え、`producer_observations`と`producer_spawns / producer_missing_repairs / producer_semantic_updates / producer_disable_updates / producer_no_op_writes / producer_steady_observations`を記録する。logistics crate所有producerはTransportの`ApplyDeferred`後、root所有のSoul Spa producerは既存energy pipeline内の専用`ApplyDeferred`後に、それぞれkindを分担するcollectorで採取する。`observer_runs`は通常collectorの1回/frameを正本とし、producer systemへ共有`ResMut`競合やcross-set schedule edgeを追加しない。

`spatial_query_metrics.csv` schema v1は`path-door` / `gather`のframe-time runだけが出力する。`path-door`はSoul indexの`door-open` / `door-close`を固定順で記録し、半径48 px（`small-le-64`）を計測する。`gather`は`gather-recruitment` 1行で半径240 px（`medium-le-320`）を記録する。各行はquery数、invalid query、coordinate probe、occupied bucket、bucket member検査、exact hit、position fallbackを分離する。query coreは`SpatialQueryStats`を値で返し、各systemがcaller専用resourceへ集約するため、Spatial indexにAtomicや共有`ResMut`を追加しない。offline validatorはworkload別exact header / 行順 / radius contract / canonical非負整数 / `occupied <= probes` / `hits, fallbacks <= members`をfail closedで検証する。これは収録済みcallerのruntime query mixを示すsidecarであり、今後の`spatial-core` CPU laneやResource / wide recruitmentの代理にはしない。

`dream_ui_metrics.csv` schema v2は`dream-ui-burst`だけが出力する。target / maximum active particleは
128、`node_writes == active_particle_updates`、sample overflow 0、checksum 3種のlowercase 16桁を
fail closedで検証する。Dream lane p95はupdate / merge / trailのsystem-local elapsedを同じVisual frameへ
合算した値であり、全frame timeのp95ではない。65,536 sample分のbufferはmeasure前に確保し、resetは
capacityを維持するため、p95採取自身のmeasure区間allocationを発生させない。
Capture / fixed auditでは`scoped_allocator_available=false`かつallocation値0、Memory buildでは
thread-local system scopeのcalls / bytesを正数として要求し、`memory.csv` / process RSSで補完する。
Captureのchecksumはrecord-only、fixed auditの同一seed反復だけが
RNG sequence / trajectory / lifetimeのexact一致を要求する。
Dream UIのalgorithm candidateを開始できるのは、128 particleの30秒warm-up / 60秒measure × 3 valid runで、
CaptureのDream lane p95中央値が100,000 ns/frame以上、またはMemoryのscoped allocation中央値が
32 KiB/frame以上の場合だけである。開始後もspatial hashは`merge_pair_comparisons / active_particle_updates >= 16`、
`UiTransform`は`node_writes / active_particle_updates >= 0.9`、poolはspawn + despawnが8件/frame以上の
該当候補だけを評価する。分母0は0とし、Memory legのframe timingをこの判定に使わない。

ここで`changed_existing_components`はcomponentの意味差分ではなく、Bevy change tickとして観測された既存`TransportRequest`置換である。`producer_no_op_writes`はproducer-owned componentのいずれかにchange tickが立った一方、前frame snapshotと値が同じだった観測、`producer_steady_observations`は対象componentにchange tickがなくsnapshotも同一だった観測である。削除はsnapshotのcomponent存在差で検出する。manual requestは除外し、`TransportRequestState`やlease等のruntime owner fieldはproducer分類へ含めない。

warm-up終端では累積counterだけをresetし、snapshot cacheは保持するためmeasure開始時の既存requestをspawnへ誤分類しない。同値再挿入を止めるM1 candidateは`producer_no_op_writes`と`changed_existing_components`を主指標にする一方、これらからgameplay変更回数を推定しない。全requestのsnapshot検査自体も`profiling` build限定で、通常buildへ観測負荷を持ち込まない。offline validatorはexact header / 13行順序 / canonical非負整数 / `changed = added + changed-existing` / `producer_observations = 6 outcomeの合計` / 全行共通かつ非0のobserver回数をfail closedで検証する。schema v1 artifactは現行validatorでは受理しない。fixed-step auditはこのsidecarを出力せず、determinism schema v4を維持する。

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

Resource visual cleanupは、LogicでResourceItemのdespawnが予約された同じframeにVisual queryが旧snapshotを読む場合がある。Sprite復元と`ResourceVisual`除去は存続Entityにだけ意味があるため、後者も`try_remove`でfail closedにし、deferred despawn後の通常`remove` warningを正式runへ持ち込まない。`chain_ignore_deferred`を使うfocused testがこのsame-frame順序を固定する。

## 起動経路のデバッグ

raw Cargoやprofiling binaryの直接起動は、親shellのtarget / temporary設定を取り込んで
resource contractを壊しやすいため、通常の調査でもrunnerを使う。

```bash
PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py run \
  --workload gather --sizes medium --renders cpu --repeat 1 \
  --warmup-secs 0 --measure-secs 1 --seed 20260712 \
  --backend vulkan --adapter Intel --window-backend wayland \
  --present-mode novsync --output target/native-acceptance/manual-debug
```

この短縮runもworkspace target、disk temporary、adapter検証、artifact manifestを持つ。性能比較や
正式baselineには使用しない。
