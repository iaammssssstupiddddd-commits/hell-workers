# P00: RtT・室内照明 baseline / gate 固定計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `single-scene-light-field-00-baseline-gates-plan-2026-08-03` |
| ステータス | `In Progress` |
| 作成日 | `2026-08-03` |
| 最終更新日 | `2026-08-05` |
| 作成者 | `Codex` |
| 親計画 | [`../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md`](../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md) |
| 直接依存 | C00-Aはなし。C00-B以降はHVAC計画M0または同等の単一owner correctness commit |
| 後続 | P01〜P08すべて |
| baseline generation | `rtt-light-v1` |
| 関連Issue/PR | `N/A` |

本計画は、RtT / Light Field migrationのproduction変更を始める前に「何を同じ条件とみなし、何を測り、どこから不合格とするか」を機械検証可能にする計画である。P00の成果は単なるスクリーンショットではなく、固定契約、専用fixture、4 evidence family（audit / Capture / Memory / RenderDoc）5 leg、比較用の安定schema、数値gateをまとめたbaseline packageとする。

ここでいう`current`は「Room interior-role等の明示した前提correctness commitを適用済み、P01以降のRtT / Light Field migrationは未適用」の状態を指す。`baseline-index.json`へ前提commit ID / SHAを列挙し、修正前のraw repository stateと混同しない。

## 1. 目的

- 解決したい課題: 実装後の結果を見て性能閾値、fixture、表示分類、光の意味論を都合よく変えられる状態をなくす。
- 到達したい状態: current renderer、production Door / Lamp経路、pause / load境界を再現でき、P01〜P08のcandidateを同じmeasurement contractでfail-closed比較できる。
- 成功指標: 後続計画が未確定の製品判断、`TBD`のhard gate、実行不能な計測command、互換性不明のartifactを持たずに着手できる。

P00はcurrentの不具合を仕様化しない。currentで失敗する経路は「観測された既知不具合」として保存し、target expectationと修正ownerを別に記録する。

## 2. スコープ

### 対象（In Scope）

- 親計画の表示分類、遮光、TopDown-only、Soul / Familiar、radial light契約の凍結。
- current camera / RtT / proxy / material / shader / world 2D pass / DirectionalLight inventory。
- deterministicな`indoor-light` workloadと、production topologyを検査するfixture sidecarの追加。
- formal matrixでwindow logical / physical size、scale factor、RtT品質を強制・記録・検証するperf軸の追加。
- static / behavior fixed-step、Capture、Memory、RenderDocを相互に代用しない4 evidence family / 5 legのcurrent baseline。
- P01のraw perf schema変更後もP08まで比較できる安定`rtt_light_migration` projection。
- source、measurement contract、fixture、binary、tool、adapter、windowを追跡するartifact indexとhash。
- P01〜P08が使うhard gateと診断値の固定。
- production Doorのauto / manual、pause、loadを固定tickで観測するbehavior timeline。

### 非対象（Out of Scope）

- Soul mask、camera、Door、material、Lamp effect等のproduction挙動修正。
- `IndoorLightField`本体、GPU upload、receiver shaderの実装。
- benchmark結果を改善するためのproduction値調整。
- wall-mounted照明のplayer-facing placement / preview / anchor validation / deconstruction UI。migration全体ではP03 / P05が将来用domain / save seamを実装し、P00は契約とtest-only fixtureだけを所有する。
- PointLight / SpotLight、shadow map、section view用の照明互換、多層階。

### 着手条件

1. `crates/bevy_app/src/plugins/startup/perf_scenario/fixture.rs`の既存未コミット変更ownerを確認し、mergeまたは明示的な作業順を決める。P00側から置換しない。
2. save / rehydrate registryの進行中変更を再監査し、P00では触らない。後続P01 / P02 / P05 / P06のowner順を親計画へ反映する。
3. formal baseline採取時はP00 toolingと契約docsを含むclean commitを必須とする。dirty treeはsmokeにしか使わない。
4. Room内のLamp / 大型設備cellをRoom interiorとして維持するcorrectness ownerを、並行HVAC計画M0または単一の先行commitへ固定する。二重実装しない。

## 3. 凍結する契約と現状

### 3.1 製品契約

| 観点 | P00で凍結する契約 |
| --- | --- |
| world composition | TopDown 2.5D、world color targetはScene RtT 1枚。section / multi-floor照明互換は対象外 |
| local light | shadow付きPointLight / SpotLightではなく、CPU map-space radial fieldを使う |
| emitter | `PowerSupplyState::Supplied`のtyped `RadialLightEmitter`だけ。Lamp entity数ではなくeligible emitter数を数える |
| occluder | 完成Wall、Closed / Locked Doorだけ。Open DoorとProvisionalWallは通光 |
| actor | Soul / Familiarはvisual receiver / caster / occluderではない。SoulはP07でgameplay field consumerにはなる |
| Room境界 | field出力はIndoorMask外を0とする。mask外SoulのLamp gameplay効果も0 |
| mask外emitter | emitter自身がmask外でもよく、Open Door等を通るLOSが成立すればmask内cellへ寄与できる |
| occupied interior | Lamp / Tank / MudMixer等の室内設備が占有するfloor cellもIndoorMaskへ含める。`RoomTileLookup`の単純コピーで欠落させない |
| Door / Room | Door Open / ClosedでRoom membershipは変えず、LOSだけを即時更新する |
| wall mount | 今回はfuture seam。player authoringは追加せず、test-only fixtureでwall内側normalとOpen Door越しのradial lightを検証する |
| composition | linear空間で`directional_styled_rgb + base_color_rgb * local_light_rgb`。local ambient 0、gain 1、fieldは1.0でsaturate |
| gameplay | slow simulation 10 Hz、stress低減`0.004/s`、fatigue回復`0.003/s`を維持。照度`> 0`なら1 Soul / slow stepにつき最大1回、Lamp重複でstackしない |
| presentation parity | Tank / MudMixerのplayer-visible stateを維持し、building completion bounceはstage上のactive presentationへ移す。bounce廃止や非描画2D childだけの状態変化は不合格 |
| directional light | formalでactiveなproduction directional sunは1 entity。dev用extra entityを残す場合も既定disabledとし、active / inactiveを別計数する |

標準emitterの論理値は次で固定する。美術的な色温度や別半径を導入する場合は、P00のgateを上書きせず別variant / 別計画として追加する。

| 項目 | 値 |
| --- | --- |
| radius | `5 tile`。現行実装の`5 world unit`（`TILE_SIZE = 32`では`0.15625 tile`）はcurrent bugとして扱う |
| distance | cell center間のEuclidean distance |
| falloff | `max(0, 1 - distance / radius)` |
| boundary | `distance >= radius`は0、emitter cellは1 |
| color / intensity | linear neutral white `(1, 1, 1)`、intensity `1` |
| storage | `UNORM16` linear RGB / luminance（`0..=65535`が`0..=1`）。変換はround-to-nearest、同値時は上側へ丸める |
| overlap | RGBごとに加算後1.0でsaturate。gameplay効果回数は増やさない |

### 3.2 local-light golden vector

- pure pre-texture arithmetic: `base=(0.8, 0.5, 0.25)`、`directional=(0.10, 0.08, 0.04)`、`local=(0.5, 0.25, 1.0)`なら結果は`(0.50, 0.205, 0.29)`。
- occluded / mask外 / stale epochでは`local=(0, 0, 0)`となり、結果はdirectional項だけ。
- field texture texel readbackでは上記localをlinear RGBA8 `(128, 64, 255, 255)`へencodeできることをexact gateとする。
- pure CPU arithmeticは誤差`1e-6`以内、tone mapping前のoffscreen material probeはquantized texelを使った期待値から各channel`2 / 255`以内をhard gateとする。tone mapping後の最終window framebufferはvisual診断だけに使い、linear値と直接比較しない。
- Wall側面はmodelのcardinal normal側、Door側面はrest / closed-frame normal側の隣接cellを1回sampleする。DoorをOpen表示へ回転してもsample側を回転させない。
- Wall上面はcardinal 4近傍からluminance最大の1 cellを選び、そのcellのRGB一式を使う。tieはNorth → East → South → West、map外はblackとし、channel-wise maxで別色を合成しない。
- 通常面のfield sampleは1、Wall上面は4。bindingはreceiver material pipelineごとに1で、emitter数に比例させない。

### 3.3 sourceから確定できるcurrent / target inventory

M1でsymbol inventoryとRenderDocを照合するが、次の値はP00開始時点のsource-derived invariantとして先に固定する。

| 項目 | current | target / owner |
| --- | ---: | ---: |
| world color handle | `2`（Scene + Soul mask） | `1` / P01 |
| explicit RGBA8 payload | `8 × W × H bytes` | `4 × W × H bytes` / P01 |
| FHD High payload | `16,588,800 B` | `8,294,400 B`、`8,294,400 B`削減 / P01 |
| Camera3d RtT | `2`（mask order -2、Scene order -1） | `1` / P01 |
| Camera2d | `3`（Main 0、Overlay 1、Foreground 2） | `2`（Overlay 1、Main 2）/ P02 |
| `LAYER_2D` world pass | `2` | `1` / P02 |
| composite texture / sampler binding | `2 / 2` | `1 / 1` / P01 |
| composite通常sample実行 / pixel | Scene `13` + mask `13` = `26` | Scene `1` / P01 |
| dormant fake-shadow有効時 | 上記 + mask `7` = `33` | 経路0 / P01 |
| Soul GLB root / Soul | visible / mask / shadow各`1` | すべて`0`、billboard `1` / P02 |
| Familiar 3D root / Familiar | `1` | `0` / P02 |
| 現行12 Building各1体 | Sprite `12` + 3D `11` = `23` presentation | Structural3d `8` + Foreground2d `4` = `12` / P02 |
| local Light Field Image | `0` | `1`、100×100 linear `Rgba8Unorm` / P06 |
| CPU IndoorLightField logical cell payload | `0` | UNORM16 RGBA相当（RGB + scalar）`80,000 B` / P03 |
| GPU Light Field Image logical payload | `0` | linear RGBA8 `40,000 B` / P06 |
| 256-byte aligned staging上限 | `0` | `51,200 B` / P06 |
| local Point / Spot / shadow pass | `0 / 0 / 0` | `0 / 0 / 0` / P06〜P08 |

`BuildingType`のvariant数はmeasurement contract作成時のsource hashへ結び付ける。HVAC等がvariantを追加した場合は、親計画の表示表、exhaustive mapping test、fixture expectationを同じ変更で更新する。

### 3.4 currentで観測する既知ギャップ

| 経路 | current観測 | target | 修正owner |
| --- | --- | --- | --- |
| completed Door | rootに`Door`、childに`Sprite`、独立3D visual | domain root + stageごとのexactly one presentation consumer | P02 |
| auto / manual Door | `Door + Sprite`同一entity queryのためproduction rootをmissする | attempted 1 / applied 1、visualとsemantic state一致 | P02、pause request化はP04 |
| perf `path-door` | synthetic rootに`Door + Sprite`を同居 | P00では変更せず、production topology probeを別追加 | P00観測、P02修正 |
| OutdoorLamp power | completion直後は`Unpowered`になり得る | production energy topologyでeligible supplied数を保証 | P00 fixture、P04 runtime |
| Room occupied floor | 建物cellがRoom floor集合から外れる可能性 | InteriorFixture cellをIndoorMaskへ含める | HVAC M0または単一先行correctness commit |
| Lamp radius | `5.0`をworld distanceと比較 | typed `5 tile` | P03 / P07 |
| Lamp overlap | LampごとにSoulへeffectを適用 | fieldを1回sampleしnon-stack | P07 |

current失敗を再現するためにperf側へproduction修正を先取りしない。behavior artifactには`attempted_transitions`と`applied_transitions`を別列で保存する。

## 4. 実装方針

### 4.1 measurement contractとbaseline世代

trackedなcanonical contract `scripts/perf_tool/contracts/rtt_light_migration_v1.json`を追加し、次を1つのhashへ正規化する。

- `contract_id = rtt-light-v1`
- fixture座標生成、semantic entity / Room / Wall / Door / emitter数、固定checkpoint名。stage別presentation期待値は別gateとする。
- seed、size、render、window、DPI、quality、duration、repeat、present mode。
- 安定projection schema、quantile定義、gate式、stageごとのrequired / not-applicable field。
- allowlist warning regex。未登録warning / errorはfailする。

runnerには`--contract rtt-light-v1`、`--stage current|p01|p02|p03|p04|p05|p06|p07|p08`、`--lane static|behavior|field-core|consumer-core`を追加する。`indoor-light`のS1 / formalではcontract / stage / lane省略、未知version、組合せ不一致、resolved hash不一致をすべて拒否し、workload名から暗黙に最新版を選ばない。`field-core`はP03以降、`consumer-core`はP07以降だけrequiredとし、それ以前のstageで実行または数値出力された場合もfailする。

formalの`--allow-log-pattern`はcontract内のexact集合をCLIへ反映するだけとし、追加・省略・別regexを拒否する。candidateごとにwarningを隠す自由入力にはしない。

比較可能性は次の3種類のhashを混同しない。

| hash | baseline / candidate間 | 用途 |
| --- | --- | --- |
| `subject_source_sha256` | 異なってよい。各session内では不変 | 何を測ったかのprovenance |
| `measurement_contract_sha256` | exact一致必須 | matrix、projection、quantile、stage gate式の同一性 |
| `fixture_contract_sha256` | exact一致必須 | 座標、Room / Wall / Door state、emitter / actor等のsemantic inputの同一性 |

stageごとに変わるSprite / 3D presentation期待値はfixture hashへ入れず、measurement contract内の`stage_id`別gateへ置く。resolved production asset hashも各sessionへ保存するが、presentation asset自体を変更するP02等ではsubject差分として扱う。asset一致を要求するstageだけ、stage gateでexact一致を追加する。

raw summary schemaをP01等で変更しても、`data/rtt_light_migration.csv` schema v1はP08まで列削除・意味変更をしない。projectionは`stage_id`、列ごとの型 / 単位、各stageのrequired / forbidden / `not_applicable`条件を持つ。数値列へ文字列を混ぜず、availability列または空値 + reason enumで表し、required stageで欠落が1つでもあればfailする。既存`perf.py compare`に専用projection比較を追加し、raw schema差だけでP00 baselineを読めなくしない。

comparatorは`data/rtt_light_gate_results.csv` schema v1も生成する。primary keyは`(gate_id, stage_id, case_id, metric_id)`で、contractがstageごとのexpected key集合、型、単位、集約法を列挙する。rowは`gate_id / stage_id / case_id / metric_id / status(pass|fail|invalid|not_applicable) / unit / observed / comparator / threshold / reference_artifact / subject_artifact / reason_code`を持つ。複数case / 単位を1つのobserved値へ潰さず、gate全体はexpected rowがすべて`pass`のときだけpassとする。required key欠落、重複、未知key、`invalid` / `not_applicable`はstage完了を拒否する。

contract、fixture意味論、primary matrixのいずれかを変える場合は`rtt-light-v2`を作る。既存v1を上書きせず、比較したいreference commitとcandidate commitの両方をv2で再採取する。candidate結果を見てv1 gateを緩和しない。

### 4.2 `indoor-light` fixture contract

既存workloadでは必要なWall / Door / Lamp / Roomを作れないため、`PerfWorkload::IndoorLight`は条件付きではなく必須とする。

fixtureはlocal tile座標の`N × N` room moduleで作る。各room interiorは6×6、境界線間隔は7 tile、Doorは各roomのnorth boundary中央へ1つ置く。重複を除いた境界cell数は`(N + 1) × (13N + 1)`で、DoorがWallを置換する。

| size | N / Room | completed Floor | completed Wall | Door Open / Closed / Locked | supplied Lamp candidate | unsupplied control | Soul / Familiar |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| small | `1 / 1` | `36` | `27` | `0 / 1 / 0` | `1` | `1` | `50 / 4` |
| medium | `2 / 4` | `144` | `77` | `2 / 1 / 1` | `10` | `1` | `200 / 12` |
| large | `4 / 16` | `576` | `249` | `4 / 8 / 4` | `50` | `1` | `500 / 30` |

2026-08-04時点のruntime実装境界は`current / static / small,medium,large / cpu`である。全規模を
production completion / energy / Room経路へ接続し、medium / largeは全Building spawn matrix、Open /
Closed / Locked Door初期状態、Tank companionまでexact検証する。Doorのauto / manual / pause / load遷移は
static fixtureと混同せず、未実装のbehavior artifactとして残す。

- exact origin、Door / Lamp / actor座標と列挙順はcanonical JSONへ保存し、乱数で変えない。
- completed Buildingは必ず共有production spawn / shell helperを通す。Doorだけsynthetic entityを作らない。
- medium / largeには`all-building-showcase-v1`を設け、contract時点の全`BuildingType`を少なくとも1体含める。main room gridと重なるWall / Door / OutdoorLampは既存個体を使い、その他はexact座標へ追加する。RenderDoc medium/gpuはこれにより全receiver / foreground presentation pipelineを列挙できることを前提にする。
- completed Floorは各6×6 interiorの全cellへ置く。設備を重ねたcellもHVAC M0等のinterior roleによりRoom / expected maskから欠落させない。
- supplied Lamp candidateはactual `OutdoorLamp + PowerConsumer`とproduction energy topologyから`PowerSupplyState::Supplied`へ到達させる。current〜P03は`supplied_lamp_candidates = 1 / 10 / 50`と`unsupplied_lamp_candidates = 1`を数える。P04以降は未給電negative controlにも`RadialLightEmitter`を付け、`typed_emitter_components = 2 / 11 / 51`と、そのうちsnapshotへ採用された`eligible_supplied_emitters = 1 / 10 / 50`を別列でexact検証する。直接`Supplied`をinsertするpure-field fixtureは別testとし、production統合gateに数えない。
- main Yardはsizeごとに1つ、unsupplied controlはgeneratorのない別Yardへ1基置く。main YardのLamp需要以外を0に固定し、Operational SoulSpa / occupied `SoulSpaTile`から次のgeneration / headroomを作る。generator Soulは表のSoul総数に含め、十分なDreamを持つ固定workerとする。

| size | Operational SoulSpa | generator Soul | generation | active Lamp demand | headroom |
| --- | ---: | ---: | ---: | ---: | ---: |
| small | `1` | `1` | `1.0` | `0.2` | `0.8` |
| medium | `1` | `3` | `3.0` | `2.0` | `1.0` |
| large | `3` | `11`（`4 + 4 + 3`） | `11.0` | `10.0` | `1.0` |

- wall-mounted caseはproduction authoringではなく、test-only semantic fixtureでanchor Wallとcardinal inward normalを明示する。
- static performance laneではDoor stateを固定し、計測中にfixture規模を変えない。
- behavior laneは通常auditと別の`capture_kind = fixed-step-behavior`にする。pause中も進むapp-shellのUpdate-step script clockからeventを注入し、通常auditのunpaused / fixed-delta validatorやframe-time比較には使わない。

behavior case IDとrequired stageはcontract v1で次に固定する。各required caseを3反復し、case間でworldを再利用しない。

| case ID | required stage | expected wake | 目的 |
| --- | --- | ---: | --- |
| `door-state-v1` | current〜P08 | N/A | auto approach、manual lock、pause中mutation、semantic / active presentation同期 |
| `load-normal-v1` | current〜P08 | N/A（current〜P04）/ `1`（P05〜） | contract生成saveの通常loadとworld epoch遷移 |
| `load-preflight-reject-v1` | P05〜P08 | `0` | reset前rejectでlive field / epoch不変 |
| `load-rollback-v1` | P05〜P08 | `1` | primary restore失敗後のrollback success |
| `load-recovery-only-v1` | P05〜P08 | `1` | recovery worldからのwake |
| `load-recovery-failed-v1` | P05〜P08 | `0` | fail-dark維持 |
| `load-duplicate-reset-v1` | P05〜P08 | coalesced `1` | idempotent reset後に1回だけrebuild |

`timeline.json` schema v1は全row共通で`case_id / step_index / script_update / simulation_tick / pause_state / world_epoch / intent / attempted / applied / semantic_state / active_presentation_state / registry_phase / registry_step_id / wake_count / field_availability / field_input_revision / field_output_revision / field_read_count / old_epoch_field_read_count / field_is_dark / field_checksum / gpu_availability / gpu_upload_epoch / gpu_checksum / fixture_checksum / terminal_outcome`を持つ。`world_epoch`とfixture checksumはload caseでcurrentからrequired、field列はP04前、lighting registry / wake / old-field-read列はP05前をtyped `not_applicable`とし、owner stage以降はrequiredにする。GPU列はcapability stage `p06` / `p08`だけrequiredで、current〜p05とP06非依存の`p07`では`not_applicable`にする。各caseのfinal rowはcomplete marker、stage上expectedなwake count、terminal fixture / field checksumを持ち、欠落時はinvalidとする。

- load subjectはcontractから生成するin-memory fixture saveに限定し、save schema version、seed、payload SHA256、expected post-load fixture hashを保存する。ユーザーの`saves/`を読まない。timeline recorderはworld replacementで消えないapp-shell resourceまたは外部orchestratorが所有し、pre / post `WorldEpoch`、fixture再成立、artifact complete markerを検証する。
- P00 current presentation expectationはDoor root Sprite `0`、child Sprite `1`、owner-linked 3D `1`。P02最終targetはroot domain `1`、Sprite `0`、owner-linked `Door3dVisual` `1`。semantic fixture hashは維持し、stage expectationだけを変える。
- frame-time / auditの各runで`data/indoor_light_fixture.csv` schema v1を必須にし、Room、Floor、Wall、Door state、Lamp root、Yard / generator topology、supplied / unsupplied Lamp candidate、expected indoor cell、root / child / presentation owner、layout checksumを検証する。`typed_emitter_components / eligible_supplied_emitters / indoor_mask_cells`はP04前を`not_applicable`とし、P04以降はそれぞれ`2 / 11 / 51`、`1 / 10 / 50`、contract期待mask checksumをrequiredにする。

### 4.3 計測軸

現行runnerはwindow 1280×720、host DPI、Highを暗黙使用するため、次をperf-only CLIからgame processまで通す。

- `--window-width 1920`
- `--window-height 1080`
- `--window-scale-factor 1.0`
- `--rtt-quality high|medium|low`

requested値だけでなく、actual logical / physical window size、actual scale factor、RtT preset、actual Scene / mask target sizeをrun metadataへ保存し、期待と違えばfailする。

primary performance matrixのwindowed legはFHD logical 1920×1080、scale factor 1.0、Highに限定する。headless auditはwindow / RtT軸を`not_applicable`として検証する。DPI 1.0 / 1.5 / 2.0 × High / Medium / Lowは短縮visual compatibility matrixとし、primary性能値へ混ぜない。

| 軸 | formal値 |
| --- | --- |
| seed | `20260803` |
| backend | windowed legは`vulkan`。別backendへfallbackしない |
| adapter | 最初のvalid windowed preflightで`name / driver / driver_info / backend` exact tupleを`environment-lock.json`へfreeze |
| window backend | launcherが選んだX11またはWaylandをgeneration内でexact一致 |
| present mode | `novsync` |
| fixed step | `64 Hz` |
| window / RtT | windowed legは1920×1080 physical、scale 1.0なのでlogicalも1920×1080、High、actual target 1920×1080。headlessはprimary-window fieldを空にし、fallback RtT値は診断のみ |
| repeat | audit / behavior / Capture / Memory / field-core / consumer-coreは各required case `3` valid runs、invalid `0`。RenderDocはfixed frame `1` |

### 4.4 evidence levelと正式artifact matrix

| level / 証跡 | 条件 | 用途 | 性能gateへの使用 |
| --- | --- | --- | --- |
| S0 launcher smoke | 既存task-dashboard、Capture / Memory、1s + 2s、3反復 | lock、sequential build、artifact監視の回帰確認 | 不可 |
| S1 indoor-light smoke | audit small/cpu 129 + 16 ticks、Capture / Memory全size×cpu/gpu 3s + 5s、3反復 | fixture、schema、adapter、log、validator確認 | 不可 |
| Formal audit / static | 全size×cpu/headless、64 Hz、1920 warmup + 128 audit ticks、3反復 | static fixture checksum / state | frame値には不可 |
| Formal behavior | small/cpu/headless、`fixed-step-behavior`、stageごとのrequired case IDを各3反復 | Door / pause / load lifecycle timeline | 不可 |
| Formal Capture | 全size×cpu/gpu、preflight 1、30s warmup + 60s measure、3反復 | wall-frame p50 / p95 / p99、domain counter | 可 |
| Formal Memory | Captureと同じ全size×cpu/gpu、preflight 1、30s + 60s、3反復 | allocator、peak live、RSS | frame quantileは不可 |
| Formal RenderDoc | medium/gpu、Capture binaryと同一SHA、固定checkpoint 1 frame | pass、attachment、binding、draw構造 | 統計値には不可 |
| Formal field-core | P03以降、large/cpu、32 warmup + 256 measured rebuild、3反復 | pure field rebuild elapsed | field CPU gateだけに可 |
| Formal consumer-core | P07以降、large/cpu、32 warmup + 256 measured slow-step consumer call、3反復 | 500 Soul sampling + 16 Room aggregation elapsed | consumer CPU gateだけに可 |

`frames.csv`はBevy `Time<Real>`由来のwall-frame間隔であり、CPU system時間またはGPU時間とは呼ばない。field rebuild時間は専用`data/indoor_light_cpu.csv`、GPU構造はRenderDocで採る。GPU percentileが必要になった場合はtimestamp query instrumentationを別計画で追加し、RenderDoc 1 frameから推定しない。

`indoor_light_cpu.csv` schema v1はpure field rebuild専用phaseで生成する。largeの同一immutable snapshotに対し32回のunrecorded warmup後、256回のfull rebuildを直列実行し、monotonic clockのelapsed nsを1 call / 1 rowで記録する。rowは`sample_index / grid_cells / supplied_emitters / radius_tiles / input_checksum / output_checksum / elapsed_ns`を持つ。rebuildしないUpdateはrowにせず、row数が256でなければinvalidとする。run内quantileは現行frame summaryと同じ`sorted[round((n - 1) × ratio)]`、session値は3 runの各quantile中央値とし、ECS schedule / upload時間を混ぜない。P04 runtime costは別counterで観測する。

`indoor_light_consumers.csv` schema v1はP07のconsumer-core専用phaseで生成する。P00 large fixture相当のproduction componentを持つpre-spawned world（500 Soul / 16 Room / 576 interior cell）を使い、32回のunrecorded warmup後、256回のmeasured slow-step consumer callを直列実行する。各call前に固定seedでSoul cellを更新し、2枚の既知field payload / revisionを交互に切り替えて全Soul sampleと全Room summary再計算を成立させる。timerはproduction Soul effect set入口からRoom summary set出口までのECS query / component writeを含み、field / input差し替え、clock注入、field rebuild、ECS spawn / despawn、GPU uploadは含めない。rowは`sample_index / soul_count / room_count / room_cell_count / field_revision / soul_samples / room_cell_samples / effect_applications / summary_updates / scoped_allocation_events / scoped_allocation_bytes / output_checksum / elapsed_ns`を持ち、row数256、Soul sample 500、Room cell sample 576、summary update 16でなければinvalidとする。quantile式と3 run集約はfield-coreと同じにする。

lighting固有allocation `0`はglobal allocatorの推測差分では判定しない。collect / rebuild / upload ownerへprofiling-only scoped allocation counterを追加し、scope内allocation event / bytesをmigration projectionへ出す。scope外process allocationとRSSはMemory artifactのcompatible-reference比較へ分離する。

RenderDoc checkpointはfixture / asset ready後にVirtual timeを固定し、同一stateを4 render frame settleさせた4枚目とする。`checkpoint name / simulation tick / render frame index / RenderDoc version / tool hash / adapter / backend`をmanifestへ記録する。raw `.rdc`、event / pass / attachment / binding抽出JSON、SHA256が1つでも欠ければfailする。RenderDocを利用できない場合はP00をblockedとし、auditで代用しない。

runtime checkpoint / replay extraction schema v2は、Scene / Soul mask target labelだけでなく、compositeのfragment
descriptor `(set, binding)`をScene texture / sampler=`(2,1) / (2,2)`、mask texture / sampler=`(2,3) / (2,4)`として
固定する。抽出は`vkCmdNextSubpass`相当の`EndPass | BeginPass`を旧subpass close後に開き、同一drawに2 textureと
2 samplerがこのexact identityで存在することを要求する。global sampler数や別drawへの重複で合格させない。

### 4.5 artifact layoutとprovenance

smokeはUUID付き`/tmp`へ置く。formalは次の構造で`target/perf-runs/`へ保存し、P08完了まで自動削除しない。

```text
target/perf-runs/rtt-light/rtt-light-v1/
  baseline-index.json
  SHA256SUMS
  <stage>-<commit>/
    environment-lock.json
    attempts/
      <attempt-uuid>/
        job.json
        orchestrator.log
        audit/
        behavior/
        capture/
        memory/
        renderdoc/
        field-core/
        consumer-core/
```

失敗と再実行は必ず新しいattempt UUIDへ分離し、valid attemptだけを`baseline-index.json`へ登録する。固定`audit/`等へ再実行して既存artifactを上書きしない。`baseline-index.json`はP00 currentの5 leg、P03以降のfield-core、P07以降のconsumer-coreについてpath、case ID、status、size、SHA256、subject commit、contract / fixture hash、binary hash、schema version、gate generationを列挙する。正式manifestには少なくとも次を保存する。

- clean commit、subject source hash、measurement / fixture contract hash。
- 実際に解決したfixture asset hash、Capture / Memory / RenderDoc binary hash。
- perf runner、native helper自身、Skill、RenderDoc toolのversion / hash。
- host fingerprint、exact adapter tuple、backend、window backend、present mode。
- windowed legのrequested / actual window logical / physical size、scale factor、RtT品質 / target size。headless auditは各fieldを明示`not_applicable`とし、pass / capacity証拠には使わない。
- seed、leg別のexact duration / tick、preflight、repeat。Capture / Memoryは30.0 / 60.0、preflight 1、repeat 3とし、31 / 61等を同じformal generationへ混ぜない。
- prerequisite correctness commit列と、valid preflightから生成した`environment-lock.json`のSHA256。

native acceptance helperにはgenericまたは`rtt-light`専用recipe / cross-session validatorを追加する。現行`task-dashboard`専用recipeをそのまま流用したことにしない。audit → behavior → stage-required field-core / consumer-core → Capture build / run → RenderDoc → Memory build / runをrepository lock下で逐次実行し、Capture / audit / behavior / core laneのbinary SHA一致、Memory別binary、全legのsource不変を検証する。

新recipeも既存Skillの安全契約を継承する。8 GiB available RAM、15 GiB workspace、1 GiB `/tmp`を開始下限とし、available RAMが12 GiB以上ならCargo job 2、それ未満なら1、`CARGO_INCREMENTAL=0`、一意job root、repository-wide lock、全process逐次を必須とする。recipe内では`--skip-build` / 任意`--binary`、自動cleanup、routine `cargo clean`を禁止する。

P08までに`cargo clean`が必要になった場合は、immutable payloadをrepository内のgitignored `.artifacts/perf-runs/rtt-light/`へ退避する。元`baseline-index.json`を変更せず、sorted relative path + byte size + file SHA256からcanonical directory digestを再計算する。移動先には旧 / 新path、旧 / 新digest、実行時刻を持つ`relocation.json`と新しい`SHA256SUMS`を生成し、両方のdigest一致後にだけP08の参照ledgerへ移動先を追記する。invalid / interrupted sessionは別UUIDのまま残し、comparatorのreferenceへ登録しない。

### 4.6 hard gate

判定値は各run内quantileを計算し、3 runの中央値をsession値とする。p50とsingle-run maxは診断値、p95 / p99と下記exact countをhard gateとする。

#### artifact validity

gate IDは`rtt-light-v1` contractのstable keyであり、v1内でrename / reuseしない。後続計画、artifact ledger、comparatorは表示名ではなく次のIDを参照する。

共通validity 6件のbundle IDは`RLV1-BUNDLE-VALID`とし、下表の6 IDをexact順で展開する。contractは未定義のglob / prefix参照を受理しない。

| gate ID | gate | 合格条件 |
| --- | --- | --- |
| `RLV1-VALID-SOURCE` | formal source | clean commit、採取中source fingerprint不変 |
| `RLV1-VALID-RUNS` | valid run | audit / behavior / Capture / Memory、P03以降のfield-core、P07以降のconsumer-coreはrequired caseごとに`3 / 3`、invalid `0`。RenderDocはvalidated fixed frame `1 / 1` |
| `RLV1-VALID-AUDIT` | audit | 3 runのcheckpoint / final signature完全一致 |
| `RLV1-VALID-LOG` | log | marker前のallowlist外WARN / ERROR `0` |
| `RLV1-VALID-ENV` | environment | contract、fixture、host、duration一致。windowed legはexact adapter、backend、window、presentも一致 |
| `RLV1-VALID-SIDECAR` | sidecar | fixture、determinism records、migration projection、instrumentation別必須fileが全てschema検証済み |

#### stage gate

| gate ID | owner / kind | hard gate |
| --- | --- | --- |
| `RLV1-P01-RTT` | P01以降のpreservation | Scene target `1`、mask target / camera / pass / binding / sample / proxy `0`。FHD explicit color `8,294,400 B` |
| `RLV1-P01-PERF` | P01だけ | 全Capture caseのp95 / p99とmax RSS中央値がP00 current比`+5%`以内、large peak live bytesが`+4 MiB`以内 |
| `RLV1-P02-DOOR-DOMAIN` | P02 M1以降 | production root Doorでauto / manualのattempted / appliedが各`1`、mutation成立条件にSpriteを要求せず、stage上の全active presentation consumerがsemantic stateと一致 |
| `RLV1-P02-PRESENT` | P02以降のpreservation | `LAYER_2D` camera / pass `1 / 1`、duplicate presentation `0`、全Building exactly one、Soul billboard `1 / Soul`、Familiar 3D `0`。Tank / MudMixer state probeとcompletion bounceがactive presentationで成立 |
| `RLV1-P02-PERF` | P02だけ | 全Capture caseのp95 / p99がP01 compatible reference比`+5%`以内 |
| `RLV1-P03-FIELD` | P03以降 | 100×100、CPU logical cell payload `80,000 B`、radius 5、large supplied emitter snapshot 50でfield rebuildのrun-p95中央値`<= 2.0 ms`、run-p99中央値`<= 4.0 ms`。max `8.0 ms`は診断線 |
| `RLV1-P04-EMITTER` | P04 | typed emitter component `2 / 11 / 51`、eligible supplied emitter `1 / 10 / 50`、unsupplied typed negativeがsnapshot採用`0` |
| `RLV1-P04-STEADY` | P04 | input不変600 Updateでfull snapshot scan、field rebuild、output revision増分、scoped lighting allocation event / bytesがすべて`0`。同一frame複数dirtyでもrebuild最大`1` |
| `RLV1-P05-LIFECYCLE` | P05 | `load-preflight-reject-v1`はlive field / epoch不変。他5 lifecycle caseはreplacement開始からwake / terminal failureまで旧epoch field read `0`、reset直後dark、期待wake count、terminal fixture / field checksum一致。GPU bridge導入後は同区間の旧uploadも`0` |
| `RLV1-P06-UPLOAD` | P06 | field Image / handle `1`、logical `40,000 B`、staging `<= 51,200 B`。changed revision 1回につきupload最大`1`、steady upload / lighting固有allocation `0` |
| `RLV1-P06-RENDER` | P06 | local Point / Spot / shadow map / local-light render pass増分 `0`。各receiver pipelineのfield binding `1`、全て同一Image。mask / duplicate 2D pass `0` |
| `RLV1-P06-COLOR` | P06 | §3.2 golden、Wall / Door / corner / mount sideのCPU fieldとpixel probeが許容差内 |
| `RLV1-P06-PERF` | P06だけ | 全Capture caseのp95 / p99とmax RSS中央値がP02 compatible reference比`+5%`以内、large peak live bytesが`+4 MiB`以内 |
| `RLV1-P07-CPU-CONSUMERS` | P07以降 | 1 Soul / slow stepのfield sample `1`、Lamp effect適用最大`1`、Room summaryとgameplayが同じCPU field revision、mask外 / stale epochのeffect `0` |
| `RLV1-P07-CONSUMER-CPU` | P07以降 | consumer-coreの500 Soul / 16 Room / 576 cellでrun-p95中央値`<= 1.0 ms`、run-p99中央値`<= 2.0 ms`、scoped allocation event / bytes `0` |
| `RLV1-P08-CROSS-CONSUMER` | P08 integration | gameplay、Room summary、GPU uploadが同じCPU field revision / world epoch由来 |
| `RLV1-P08-FRAME` | P08 | 全compatible caseのp95 / p99がP00 current比`+5%`以内。large/gpuは加えてp95 `<= 16.667 ms`、p99 `<= 25.0 ms` |
| `RLV1-P08-MEMORY` | P08 | allocator accounting error `0`、各case max RSS中央値がP00比`+5%`以内、large peak live bytesがcompatible reference比`+4 MiB`以内 |

#### stage applicability

`stage_id`は単純な累積commit番号ではなく、そのplan boundaryのacceptance capabilityを表す。`p03`はP01 / P02と独立する。`p04` / `p05` / `p07`はP02 M1のtransitive dependencyによりP01 RtTとDoor domain gateだけを要求し、P02 full presentationは要求しない。P06とP07はP05後に並行可能なので、`p06`はP07 gateを、`p07`はP06 / GPU gateを要求しない。両者を統合したcross-consumer gateは`p08`で初めてrequiredにする。次表以外のgate IDを暗黙継承しない。

| stage ID | required lanes / behavior cases | exact required gate IDs |
| --- | --- | --- |
| `current` | static、`door-state-v1`、`load-normal-v1`、Capture、Memory、RenderDoc | `RLV1-BUNDLE-VALID` |
| `p01` | currentと同じ | `RLV1-BUNDLE-VALID`、`RLV1-P01-RTT`、`RLV1-P01-PERF` |
| `p02` | currentと同じ | `RLV1-BUNDLE-VALID`、`RLV1-P01-RTT`、`RLV1-P02-DOOR-DOMAIN`、`RLV1-P02-PRESENT`、`RLV1-P02-PERF` |
| `p03` | current + field-core | `RLV1-BUNDLE-VALID`、`RLV1-P03-FIELD` |
| `p04` | current + field-core | `RLV1-BUNDLE-VALID`、`RLV1-P01-RTT`、`RLV1-P02-DOOR-DOMAIN`、`RLV1-P03-FIELD`、`RLV1-P04-EMITTER`、`RLV1-P04-STEADY` |
| `p05` | static、behavior全7 case、Capture、Memory、RenderDoc、field-core | `RLV1-BUNDLE-VALID`、`RLV1-P01-RTT`、`RLV1-P02-DOOR-DOMAIN`、`RLV1-P03-FIELD`、`RLV1-P04-EMITTER`、`RLV1-P04-STEADY`、`RLV1-P05-LIFECYCLE` |
| `p06` | p05と同じ | `RLV1-BUNDLE-VALID`、`RLV1-P01-RTT`、`RLV1-P02-DOOR-DOMAIN`、`RLV1-P02-PRESENT`、`RLV1-P03-FIELD`、`RLV1-P04-EMITTER`、`RLV1-P04-STEADY`、`RLV1-P05-LIFECYCLE`、`RLV1-P06-UPLOAD`、`RLV1-P06-RENDER`、`RLV1-P06-COLOR`、`RLV1-P06-PERF` |
| `p07` | p05 + consumer-core | `RLV1-BUNDLE-VALID`、`RLV1-P01-RTT`、`RLV1-P02-DOOR-DOMAIN`、`RLV1-P03-FIELD`、`RLV1-P04-EMITTER`、`RLV1-P04-STEADY`、`RLV1-P05-LIFECYCLE`、`RLV1-P07-CPU-CONSUMERS`、`RLV1-P07-CONSUMER-CPU` |
| `p08` | static、behavior全7 case、Capture、Memory、RenderDoc、field-core、consumer-core | `RLV1-BUNDLE-VALID`、`RLV1-P01-RTT`、`RLV1-P02-DOOR-DOMAIN`、`RLV1-P02-PRESENT`、`RLV1-P03-FIELD`、`RLV1-P04-EMITTER`、`RLV1-P04-STEADY`、`RLV1-P05-LIFECYCLE`、`RLV1-P06-UPLOAD`、`RLV1-P06-RENDER`、`RLV1-P06-COLOR`、`RLV1-P07-CPU-CONSUMERS`、`RLV1-P07-CONSUMER-CPU`、`RLV1-P08-CROSS-CONSUMER`、`RLV1-P08-FRAME`、`RLV1-P08-MEMORY` |

P00 currentがP08の絶対frame gateを既に満たさない場合も、P00採取自体は完了できる。ただし既存release blockerとしてledgerへ赤で固定し、P08結果を見て絶対値を緩めない。

P06の増分costはP02完了時のcompatible referenceとも比較し、P01 / P02の削減がlocal-light costを隠していないことを確認する。P08はP00 currentとの全体比較と絶対budgetの両方を満たす。

`RLV1-P07-CONSUMER-CPU`のp99 `2.0 ms`は10 Hz slow-step budget `100 ms`の2%を上限として先に予約した値であり、candidate結果を見て変更しない。field rebuildは`RLV1-P03-FIELD`で別計測するため二重計上しない。

## 5. マイルストーンとwork package

### M1 / C00-A: 製品契約・依存・静的inventoryを凍結する

#### 変更内容

1. §3の製品契約を親計画と照合し、表示分類をmeasurement contract時点の全`BuildingType`でexhaustiveにする。
2. current camera / target / pass / material / proxy / shader / directional light inventoryをsymbolとstartup testの両方で記録する。
3. Door auto / manual、Lamp power、Room occupied cell、save / pauseのcurrent経路と修正ownerを記録する。
4. HVAC M0とのInteriorFixture owner、save registry owner、`fixture.rs` ownerを親計画の着手条件へ反映する。
5. P01のScene handle / mask camera removalをcompile可能なatomic境界に直し、後続計画のstale path / enum / ownership記述を再監査する。

#### 主な変更ファイル

- 本計画、親計画、影響するP01〜P08計画
- `docs/rendering-performance.md`
- `docs/performance-profiling.md`

#### 完了条件

- [x] 製品契約とdownstream ownerに未決定がない
- [x] current inventoryのsource countとstartup test expectationが一致する。capture照合はM4へ明示handoffされている
- [x] Room interior、Door、Lamp powerの前提がproduction経路で確認されている
- [x] 後続計画の中間commitがcompile不能になる依存順を残していない

### M2 / C00-B: fixtureと安定計測schemaを実装する

#### 変更内容

1. `PerfWorkload::IndoorLight`、small / medium / large fixture、`static / behavior / field-core / consumer-core`の明示laneを追加する。future core laneはowner stage前に値を捏造せず`not_applicable`にする。
2. production helper由来Door / Lamp / energy topologyを使い、fixture sidecarとlayout checksumを出す。
3. audit checksum / encodingへFloor、Wall、Door state、Lamp、supply、Room / expected indoor cells、timelineを安定key順で追加する。P03 / P04後にtyped emitter / actual mask列をrequired化する。
4. `indoor_light_cpu.csv`、`indoor_light_consumers.csv`、schema v1の`rtt_light_migration.csv` / `rtt_light_gate_results.csv`を追加し、currentではfuture fieldを`not_applicable`として出す。
5. Capture wall-frame p95 / p99 comparatorと、Memory RSS / allocation comparatorを別実装にする。Memory buildのframe quantileをgateへ使わない。

#### 主な変更ファイル

- `crates/bevy_app/src/plugins/startup/perf_scenario/{config.rs,fixture.rs,workload_driver.rs,capture_driver.rs,audit_checksum.rs,audit_encoding.rs,output.rs}`
- `crates/bevy_app/src/plugins/startup/perf_scenario/config/tests.rs`
- `crates/bevy_app/src/plugins/startup/perf_scenario.rs`
- `scripts/perf_tool/{arguments.py,model.py,cli.py,execution.py,artifacts.py,summary.py,compare.py,policy.py,fixtures.py}`
- `scripts/perf_tool/contracts/rtt_light_migration_v1.json`

#### 完了条件

- [x] fixture exact count / layout / expected indoor cell checksumが全runで一致し、P04以降はactual mask checksumも一致する
- [x] production Door topologyを使い、currentのattempted / applied差を隠さない
- [x] supplied Lamp candidate `1 / 10 / 50`とunsupplied negative control `1`を検証でき、P04以降はtyped component `2 / 11 / 51`、eligible supplied `1 / 10 / 50`が一致する
- [x] raw perf schemaをbumpしてもmigration projection v1の意味と列が維持される
- [x] `python3 scripts/perf.py self-test`がinvalid artifactも含めfail-closedで成功する

#### 2026-08-04 実装済みslice

- `small`のFloor 36 / Wall 27 / Closed Door 1 / Lamp 2 / SoulSpa 1を共有production helperで生成し、
  energy topology、Room 36 cell / boundary lookup 24、Soul 50 / Familiar 4を初期fixtureでexact検証する。
- fixture summary 1行、semantic layout ledger 187行、current presentation 5行をsidecar化し、欠損、改変、
  列順、hash不一致をPython validatorで拒否する。
- 各固定step checkpointで通常actor 58 + indoor semantic actor 78 = 136 recordを出し、7 checkpoint
  952 recordのkind / count / stable order / checksumを検証する。production Room再生成後もEntity IDではなく
  floor集合とreverse lookupで一意に追跡する。
- `current / static / small`以外、selector欠落、population / seed / mode不一致をRust / Python双方で拒否する。
- dev binaryによる129 + 16 tick headless smokeでmanifest validを確認した。ローカルVulkan loader ERRORは
  診断用regexでのみ許可したため、このrunはformal evidenceではない。
- `all-building-showcase-v1`をbooleanからexact matrixへ昇格し、medium / largeの12 `BuildingType` root、
  anchor / footprint、追加7棟のcompletion route、Tank companion、post-process component、current
  presentationをcanonical JSONへ固定した。smallのsemantic layout hashと187 / 5行は維持する。
- medium / largeのpure sidecarはそれぞれlayout 722 / 2306行、presentation 12行、indoor semantic actor
  258 / 936件を期待し、size別session / run checksum、Bridge footprint改変、actor欠落をPython self-testで
  fail-closed検証する。Rust runtimeも同じmatrixへ接続し、size guardを解除した。
- Bridge showcaseはcurrent 2×5 completion topologyを観測するfixture-seeded probeであり、蛇行川に対する
  player authoring validationの成立を表さない。この境界をcontractとdocsへ明記した。
- completed Buildingを契約anchor / production draw positionで一意追跡し、初期worldの設備をfixture数へ
  混入させない。Tank companionは論理配置1件 / `BucketStorage` entity 2件、Bucket 5件の3 / 2分配を検証する。
- realtime Capture / Memoryはwarmup終端とmeasure終端でもindoor semantic validatorを再実行し、Door、grid、
  Room、showcaseが途中でdriftしたstale sidecar artifactを拒否する。
- Rust側でも3規模のsummary / layout / presentation行数、Bridge anchor、Tank companion ledgerをunit testし、
  Python期待値生成だけに依存しないsidecar境界を持つ。

### M3 / C00-C: window軸・native launcher・RenderDoc validatorを実装する

#### 変更内容

1. window width / height / scale factor / RtT品質のperf-only引数をRust configまで通す。
2. requested値とactual logical / physical / RtT targetをmanifest / run metadata / validatorへ追加する。
3. native acceptance helperへ`rtt-light` recipe、source / helper / contract fingerprint、cross-leg validatorを追加する。
4. fixed simulation checkpointのRenderDoc captureと抽出manifest / offline validatorを追加する。
5. S0 task-dashboard recipeを再実行して、generic化が既存recipeを壊していないことを確認する。
6. `.artifacts/perf-runs/`をgitignoreし、artifact relocation digest / ledger validatorを追加する。

#### 主な変更ファイル

- `crates/bevy_app/src/main.rs`
- `crates/bevy_app/src/plugins/startup/perf_scenario/{config.rs,config/tests.rs,output.rs}`
- quality初期化ownerと必要なfocused test
- `.codex/skills/hell-workers-run-native-acceptance/{SKILL.md,scripts/native_acceptance.py}`
- `scripts/perf_tool/`のmanifest / RenderDoc helper / validator
- `.gitignore`
- `docs/performance-profiling.md`

#### 完了条件

- [x] windowed legで1920×1080 / scale 1.0 / Highを強制し、actual mismatchがfailする。headless auditはwindow field不在を正しく検証する
- [x] exact adapter / seed / duration / present / host mismatchがfailする
- [x] audit → behavior → stage-required field-core / consumer-core → Capture → RenderDoc → Memoryが同一job / lockで逐次実行される
- [x] raw `.rdc`または抽出manifest欠落時に成功扱いしない
- [x] resource下限、bounded Cargo jobs、`CARGO_INCREMENTAL=0`、repository lock、build順、自動cleanup禁止が既存Skill contractと一致する
- [ ] 実機S0と同一sourceのS1がvalidになり、formal launcherがその証跡なしには開始しない

#### 2026-08-05 実装済みslice

- canonical contract、stage projection、gate expected row、Door / load behavior artifactを凍結し、current stageで
  `static`と`behavior`をformal legとして受理する。`field-core` / `consumer-core`は成立前stageで拒否する。
- `window.csv`はresolved window backend、actual adapter/backend、requested / effective present modeを開始・終了で
  exact検証する。formalのenvironment lockはCapture binary hashまで束縛する。
- native helperはS1（51 process）とformal（64 process）を別recipeとして持つ。formalはclean subject、ancestor
  correctness commit、同一sourceのS0 / S1、frozen contract、RenderDoc tool probeをfail-closedで要求し、behavior /
  RenderDoc後に8秒settleする。
- RenderDocはVulkan、GPU readyの連続4 frame、simulation tick 0、label付きScene / Soul mask targetを固定し、raw
  `.rdc`、runtime checkpoint、qrenderdoc replay抽出、pass boundary、attachment、reflection binding、label解決した
  target topologyをoffline validatorまで結ぶ。schema v2ではVulkan `(set, binding)`も記録し、subpass transitionを
  close→openで扱い、同じ1 composite drawにScene / mask texture各1とsampler各1が存在することをexact検証する。
- formal registrationはattempt manifest / SHA256SUMS / baseline-index locatorのledgerで行い、未登録・改変・
  到達不能なartifactをreferenceにしない。

### M4 / C00-D: formal current baselineを採取する

#### 実行順

1. prerequisite correctness commit列とP00 toolingを含むclean commit、contract hashをfreezeする。
2. formal auditを全size / cpu / headlessで実行する。
3. formal behaviorのcurrent-required `door-state-v1` / `load-normal-v1`をsmall / cpu / headlessで各3反復する。
4. formal Captureを全size × cpu/gpuで実行する。
5. Capture binaryのmedium/gpu固定checkpointをRenderDoc captureする。
6. formal Memoryを別buildで全size × cpu/gpu実行する。
7. cross-leg validator、`SHA256SUMS`、`baseline-index.json`を生成する。
8. behavior timelineでauto / manual / pause / loadのcurrent観測をtarget expectationと並記する。field-coreはP03以降のstage artifactとして追加する。

#### formal audit command shape

M2 / M3完了後にのみ次のshapeを使う。`indoor-light`登録前にこのcommandを実行しない。

```bash
P00_AUDIT_OUTPUT=target/perf-runs/rtt-light/rtt-light-v1/current-0123456789abcdef/attempts/00000000-0000-4000-8000-000000000000/audit
PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py audit \
  --workload indoor-light \
  --contract rtt-light-v1 \
  --stage current \
  --lane static \
  --sizes small,medium,large \
  --renders cpu \
  --seed 20260803 \
  --repeat 3 \
  --preflight-runs 0 \
  --backend vulkan \
  --window-backend headless \
  --instrumentation capture \
  --fixed-hz 64 \
  --warmup-ticks 1920 \
  --audit-ticks 128 \
  --allow-log-pattern 'driver that only supports software rendering' \
  --output "$P00_AUDIT_OUTPUT"
```

上はM2 / M3後のCLI契約例である。formalではnative helperがclean commitと一意UUIDから未使用outputを生成し、既存pathを拒否する。Capture / Memoryはnative Skillのno-prompt launcherから、§4.4のexact matrixで別build / 別outputへ実行する。直接binary起動や`audit --renders gpu`を代替証拠にしない。

#### 完了条件

- [ ] audit / behavior / Capture / Memory / RenderDocの5 formal legが§4.6のvalidity gateを満たす
- [ ] source inventoryとRenderDocのpass / attachment / bindingが一致する
- [ ] current known defectがtimelineへ記録され、target expectationと混ざっていない
- [ ] baseline indexから全command、manifest、raw artifactを再検証できる

### M5 / C00-E: gate ledgerとhandoffを確定する

#### 変更内容

1. P00 current実測値、stage reference、hard target、診断値、artifact path / SHAを`docs/rendering-performance.md`へ転記する。
2. primary matrixのp50 / p95 / p99、RSS、pass / attachment、inventory、stabilityを表にする。
3. P01〜P08のacceptance表へ同じgate IDを参照させ、数値を重複記述しない。
4. current絶対gate未達、known bug、non-blocking診断値を明示する。
5. docs index、Help impact、workspace gateを閉じる。

#### 完了条件

- [ ] hard targetに`TBD`がない
- [ ] 全current値がartifact path / hashへ追跡できる
- [ ] stageごとのreference lineageとrequired fieldが一意である
- [ ] P01以降の着手者が追加の製品判断を必要としない

## 6. commit / stop-go規則

| checkpoint | merge可能条件 | stop条件 |
| --- | --- | --- |
| C00-A contract / inventory | docs整合、owner決定 | Room / section / mount等の製品境界が未決定 |
| C00-B fixture / projection | focused tests、self-test、workspace check | `fixture.rs` owner競合、production helperを使えない |
| C00-C runner / native | S0 / S1、fail-closed validator | actual matrixを強制できない、RenderDoc unavailable |
| C00-D artifact | clean commit、全formal leg valid | source変化、adapter / host不一致、invalid run |
| C00-E ledger | hard target確定、downstream参照同期 | `TBD`、hash不明、互換性のないsession混在 |

- C00-B / C00-Cはprofiling専用変更であり、通常startup pathの挙動不変testを持つ。
- C00-Dのartifactはcode commitへ混ぜない。ledger更新だけをdocs commitにする。
- P00完了前にP01以降のproduction変更へ進まない。

## 7. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| fixtureがsynthetic Doorを使う | production silent failureを検出できない | production helperを必須にし、attempted / appliedを別記録 |
| Lamp rootだけ数えて全て未給電 | dark fixtureを有効baselineと誤認 | production energy topologyとeligible supplied countを検証 |
| Room内設備cellがmask外 | Lamp自身や大型receiverが常時dark | HVAC M0のInteriorFixture correctnessを着手条件にする |
| raw schema削除でP08比較不能 | current / finalの比較が破綻 | migration projection v1をP08まで凍結 |
| FHD / DPIを指定したつもりになる | 実際は1280×720 / host DPI | perf CLI、actual metadata、validatorを同時追加 |
| source hashを前後一致させる | candidate実装を比較できない | sourceはsession provenance、contract / fixture / environmentを互換条件にする |
| Memory frame値を性能値へ使う | instrumentation擾乱を改善と誤認 | frameはCapture、allocation / RSSはMemoryへ限定 |
| RenderDoc 1 frameをGPU percentile扱い | 統計的根拠がない | pass構造だけに使い、GPU percentileは別instrumentation扱い |
| mask削減がfield costを隠す | P06増分を評価できない | P02 compatible referenceとP00 currentの二重比較 |
| current bugを仕様へ固定 | Door / radius / stackの不具合が残る | current observation、target、ownerを別列で保持 |

## 8. 検証計画

### C00-A docs only

- `python3 scripts/dev.py docs --write`
- `python3 scripts/dev.py docs --check`
- `python3 scripts/check_docs.py`
- `git diff --check`

### C00-B / C00-C tooling

- focused Rust config / fixture / checksum / output tests
- Python contract / artifact / invalid fixture / compare / native helper tests
- `python3 scripts/perf.py self-test`
- `cargo test -p bevy_app@0.1.0 --no-default-features --features profiling perf_scenario`
- `python3 scripts/dev.py check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- Help impact review。profiling-only変更でも実際のplayer-visible pathから`No impact` / `Update required`を判断する
- `PYTHONDONTWRITEBYTECODE=1 python3 .codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py self-test`
- `python3 scripts/check_agent_rules.py`
- `python3 /home/satotakumi/.codex/skills/.system/skill-creator/scripts/quick_validate.py .codex/skills/hell-workers-run-native-acceptance`

### C00-D / C00-E native and final

- `hell-workers-run-native-acceptance` SkillによるS0、S1、Formalの逐次実行
- audit / behavior / Capture / Memory / RenderDocのoffline artifact verification
- p95 / p99比較を別々に実行し、両方の合格を要求
- `python3 scripts/dev.py verify`
- docs gateと`git diff --check`

## 9. ロールバック・AI引継ぎ

### ロールバック方針

- C00-A〜C00-Eを独立commitにし、fixture / runner / native helper / ledgerを一括revertしない。
- fixtureに非決定性があればartifactをreference登録せず、C00-Bだけを修正する。
- contract v1を採取後に編集せず、変更はv2追加と両側再baselineで行う。
- Door / Room interior-role等のproduction correctness修正はP00 tooling rollbackへ巻き込まず、owner計画の独立commitにする。
- 採取済み数値を再計測なしに書き換えない。

### 現在地

- 進捗: `75%`（C00-A / C00-B完了、C00-C実装完了。C00-Dのformal環境待ち）
- 完了済み: Room interior role / `RoomBoundaryLookup` correctness、current RtT startup inventory test、frozen
  `rtt-light-v1` contract、3規模fixture、Door / load behavior、stable projection / gate expected row、manifest / raw
  artifact / ledger validator、window / adapter / present evidence、S1 / formal native recipe、RenderDoc capture /
  replay extractor、Runtime target label / topology verification、Skill手順、self-test群。
- 未完了: actual S0、同一sourceのS1、clean subjectでのformal audit / behavior / Capture / RenderDoc / Memory、
  registered `baseline-index.json`、current測定値を持つC00-E gate ledger。P01〜P08は未着手。

### 次のAIが最初にやること

1. formal対象のclean commitを作り、Room / HVAC M0相当のcorrectness ancestor commitを一つ以上指定する。
2. 8 GiB以上のavailable memoryとRenderDoc一式を用意して、同一commit / source fingerprintでS0、S1を順に採取する。
3. `plan-rtt-light --level formal`が`ready`になることを確認して返却された`kitty` launcherのみを実行し、5 legと
   registered baselineをoffline revalidateする。

### ブロッカー/注意点

- formalはclean commit必須であり、現在のdirty worktreeをそのままbaselineにしない。最低一つのancestor
  correctness commitと同一sourceのS0 / S1も必要である。
- 2026-08-05のformal planは`MemAvailable 7.0 GiB < 8 GiB`、dirty worktree、`renderdoccmd`不在でblockedだった。
  S0 / S1 job rootもまだない。これらはheadless smokeで代用しない。
- contract v1はfrozenである。変更が必要ならv2を追加してreference / candidateを同条件で再採取する。
- RenderDoc実装はraw `.rdc`とreplay topologyをfail-closedで検証するが、実toolによるcapture未採取のため
  actual GPU pass値はまだ存在しない。
- Room interior correctnessと3規模production spawn / energy settleは実装済み。今後もsynthetic Buildingや
  直接`PowerSupplyState::Supplied` insertへ戻さない。
- 現在のhostでloader / ICDのunexpected logが出る場合はformal contractへ追加せず、発生源を是正する。

### 最終確認ログ

- quality gates: `2026-08-04` / `python3 scripts/dev.py verify` pass（Python 47、perf self-test、
  workspace test、profiling check、Clippy 0 warning、Help、docs、diff hygieneを含む）
- focused Rust: `2026-08-04` / `cargo test -p bevy_app@0.1.0 --features profiling indoor_light_fixture --lib`
  3件pass（small / medium / large layoutとRust sidecar shape）
- diagnostic smoke: `2026-08-04` / headless frame-time Low 640×360、fixed audit 129 + 16 ticksが各1 run valid。actual-window evidenceには不使用
- indoor-light smoke: `2026-08-04` / `current / static / small / cpu`、129 + 16 ticks、1 run、
  3 sidecar（1 / 187 / 5行）と952 audit recordを検証してmanifest valid。local loader診断allowlist使用のためformal evidenceには不使用
- indoor-light medium smoke: `2026-08-04` / `current / static / medium / cpu`、129 + 16 ticks、
  sidecar（1 / 722 / 12行）と7 checkpointのsemantic auditを検証してmanifest valid。local software
  adapter警告の診断allowlist使用のためformal evidenceには不使用
- indoor-light large smoke: `2026-08-04` / `current / static / large / cpu`、129 + 16 ticks、
  sidecar（1 / 2306 / 12行）と7 checkpointのsemantic auditを検証してmanifest valid。local software
  adapter警告の診断allowlist使用のためformal evidenceには不使用
- indoor-light realtime smoke: `2026-08-04` / `current / static / medium / cpu`、warmup / measure各0.1秒、
  warmup終端 / measure終端のsemantic再検証とsidecar出力を通してmanifest valid。software adapter警告と
  既知のRoomBorderLine B0004を診断用にallowしたため性能値・formal evidenceには不使用
- native acceptance: `2026-08-04` / actual S0 / S1は未実行（当時のheadless smokeのみ。2026-08-05時点では
  `rtt-light` recipe / RenderDoc実装済み）
- docs gate: `2026-08-04` / `scripts/dev.py docs --check` pass
- tooling self-test: `2026-08-05` / `python3 scripts/perf.py self-test`、native helper self-test、RenderDoc
  capture / extractor self-test、AI rule contract、docs index pass。RenderDoc schema v2はsubpass close→open、
  Vulkan `(set, binding)`、同一drawの2 texture / 2 samplerをsynthetic fixtureでもfail-closed確認する
- profiling compile: `2026-08-05` / P00実装後の再実行は既存の
  `interface/ui/panels/task_list/actions.rs`における`commands`未束縛2件で停止。P00 RenderDoc moduleの新規diagnosticは
  出ていないため、この並行production変更の解消後に再実行する
- Help impact: `2026-08-05` / P00 scopeはprofiling-only capture metadata、offline validator、開発docsだけであり、
  通常の入力 / UI label / workflow / gameplayは不変としてNo impact。worktree全体には並行するHelp catalog更新が含まれるため、
  その更新の承認をP00判断で代用しない
- formal plan: `2026-08-05` / blocked（7.0 GiB < 8 GiB、dirty tree、`renderdoccmd`不在、S0 / S1未採取）

### Definition of Done

- [ ] M1〜M5が完了
- [ ] contract v1、fixture、stable projection、native validatorがgreen
- [ ] 5 formal legとbaseline indexがfail-closed検証済み
- [ ] hard targetに`TBD`がなく、downstreamが同じgate IDを参照
- [ ] current known defectsとtarget expectationが分離されている
- [ ] Help impact review、workspace、native、docs gateが完了
- [ ] 影響docsが更新済み

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-08-05` | `Codex` | RenderDoc runtime / extraction schema v2へ更新し、Vulkan subpass境界、`(set, binding)`、同一composite drawのScene / mask texture・samplerをfail-closedに固定。formal環境再評価のblock条件も記録 |
| `2026-08-05` | `Codex` | contract freeze、behavior / projection / gate row、window environment、S1 / formal native recipe、RenderDoc replay validatorを反映。formal baselineは環境条件未達のため未採取として明記 |
| `2026-08-04` | `Codex` | medium/large production runtime、Door静的state同期、全Building component audit、Tankの論理1配置/ECS 2 storage、realtime終端再検証を反映 |
| `2026-08-04` | `Codex` | medium/large全Building showcaseのexact座標・footprint・completion/component/presentation契約、size別ledger/audit validator、Bridge authoring境界を反映 |
| `2026-08-04` | `Codex` | production helper由来small/current/static fixture、3種のexact sidecar、checkpoint semantic audit、fail-closed validator、headless smoke結果とmedium/large・Door blockerを反映 |
| `2026-08-04` | `Codex` | C00-A Room / startup inventory、pinned draft contract、fail-closed artifact検証、window / RtT axisの実装進捗とformal未成立条件を反映 |
| `2026-08-04` | `Codex` | 現行監査を反映し、branch-aware stage / stable gate ID、fixture、behavior lifecycle、core lane、artifact matrix、数値gate、依存境界を全面具体化 |
| `2026-08-03` | `Codex` | 統合計画M0を独立計画へ分割し、fixture・artifact・数値gateを具体化 |
