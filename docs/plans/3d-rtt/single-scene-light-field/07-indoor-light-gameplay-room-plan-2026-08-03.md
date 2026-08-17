# P07: 室内 Light Field gameplay・Room統合計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `single-scene-light-field-07-indoor-light-gameplay-room-plan-2026-08-03` |
| ステータス | `Implemented — local gates / consumer-core valid、P05-lineage native formal pending` |
| 作成日 | `2026-08-03` |
| 最終更新日 | `2026-08-17` |
| 作成者 | `Codex` |
| 親計画 | [`../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md`](../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md) |
| 直接依存 | [P03](03-indoor-light-domain-core-plan-2026-08-03.md)、[P04](04-indoor-light-runtime-integration-plan-2026-08-03.md)、[P05](05-indoor-light-save-lifecycle-plan-2026-08-03.md) |
| 証跡分岐 | P05 formal subjectを祖先にしたclean P07 subject。P06のGPU ownerは含めず、P08がP06/P07統合とcross-consumer evidenceを所有する |
| P06判断 | P06は2026-08-17にユーザー承認のRenderDoc RD0 timeout例外で受理済み。これはP07 v1 formalをP06入りsubjectで採取してよい、という意味ではない |
| 後続 | [P08](08-legacy-cleanup-release-plan-2026-08-03.md) |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: 現`lamp_buff_system`は全`PowerConsumer`をLamp候補にし、遮光を無視してLampごとにSoulへ効果を重ね、半径コメントのtileとworld unit計算も一致しない。
- 到達したい状態: 各Soulはslow simulation stepごとにCPU `IndoorLightField`を最大1回sampleし、Room summaryも同じcurrent epoch / field revisionから導出される。
- 成功指標: Lamp数による効果stackがなく、Wall / Door / powerの状態でgameplayとRoom summaryが同じCPU field cellを判定し、Room entity再生成・load後にもstale summaryを参照しない。P06 GPUとの横断一致はP08の`RLV1-P08-CROSS-CONSUMER`で閉じる。

## 2. スコープ

### 対象（In Scope）

- 既存Lamp回復効果のfield samplingへの置換と、tile単位radiusの唯一のsource化。
- epoch-aware field read、Soul回復、`RoomIlluminationState`、Room tile signature / cache / reset。
- Door / movement / power / Room再検出後のPostActor schedule。
- P07 selector、consumer-core、artifact / bundle / RenderDoc / native acceptanceのfail-closed evidence経路。
- debug / perf counter、headless gameplay / Room lifecycle test、正式受入用docs / Help impact判断。

### 非対象（Out of Scope）

- proportional brightnessによる新しい回復curve。
- comfort / health / productivityの新規system、Room照度UI、warning / notification、AI task generation。
- GPU texture readback、renderer pixelをgameplay正本にすること。
- P06 GPU evidenceをP07へ再分類すること、または凍結済み`rtt_light_migration_v1.json`のID / 閾値 / hashを書き換えること。

## 3. 固定契約

### 3.1 field read・balance・radius

初期移行は「current epochのfield cellの`LightCell::luminance`が`0`より大きければ既存の照明中回復rateを1回適用、`0`なら適用しない」のbinary判定とする。

- slow simulationは10 Hz、stress低減は`0.004/s`、fatigue回復は`0.003/s`を維持し、P07で勝手に増減しない。
- `hw_energy::constants`に`OUTDOOR_LAMP_RADIUS_TILES`（整数tile数）を唯一のsourceとして置き、P04 adapterが`LightRadiusTiles`へ変換する。旧`OUTDOOR_LAMP_EFFECT_RADIUS: f32`とworld distance `5.0`比較は削除する。generic `hw_infra::lighting`へOutdoorLamp balance定数を逆流させない。
- `FieldSnapshot`へpureな境界検査済みgameplay read APIを追加する。out-of-boundsは`None`、in-boundsだがindoor mask外は`Some(0)`（dark）とし、terrain用のclamp helperを再利用しない。Room aggregationには別のstrict room-sample API（または同等の`is_indoor`検査）を与え、mask外と有効なdark cellを必ず区別する。gameplayとRoom aggregationは`LightCell::luminance`を読む。
- runtime consumerは必ず`read_indoor_light_snapshot(runtime, current_world_epoch, current_world_epoch, probe)`を通す。requestedとcurrentへ同じcurrent `WorldEpoch`を渡し、raw `snapshot()` / `snapshot_for_epoch()`、前world snapshot、GPU Imageを読まない。P07 production moduleのraw read呼出しが0であるfocused source-policy testを置く。`None` / epoch mismatch / unavailableはdarkで、旧fieldへのfallbackはない。
- 1 Soul / 1 slow stepにつきsampleとeffectは各最大1回。同じcellを複数Lampが照らしてもstackしない。SuppliedでないLamp、non-Lamp `PowerConsumer`、Wall / Closed Door越しのdark cellはeffect 0。
- colorは初期gameplayへ影響させない。proportional curveや明るさ別bonusは別proposalとbalance test更新を要求する。

### 3.2 同frame schedule

P07はLogicへLamp走査を戻さない。Room entityを`Commands`で再生成する現行経路を含め、同一updateの順序を次で固定する。

```text
Logic:
  energy settlement
  -> detect_rooms_system / Room topology refresh
  -> explicit ApplyDeferred（新しいRoom entityを可視化）

Actor（unpaused）:
  auto-open -> movement -> auto-close

PostActor（pause-open）:
  IndoorLightingRebuildSet
  -> SoulLightRecoverySet（独自の !Time<Virtual>::is_paused() gate）
  -> RoomIlluminationSummarySet（pause-open）
  -> Visual consumer
```

- `SoulLightRecoverySet`は`IndoorLightingRebuildSet`の後に置く。`SlowSimulationClock.steps_this_frame()`が1〜5なら、各Soulへstepごとに最大1 sample / effectを適用する。同一updateのcatch-upはすべてActor後の同じTransformをsampleする。
- `steps_this_frame() == 0`ではrecoveryはearly-returnし、snapshot read、lifecycle probe、Soul query、field sample、effectを一切実行しない。
- `PostActor`はpause中も動くため、recovery system自身にunpaused run conditionを付ける。直前frameのnonzero `steps_this_frame()`をpause中に再適用してはならない。
- Room summaryはpause-openでmanual Doorによるfield rebuildへ追従するが、回復effectはpause中に進めない。
- `detect_rooms_system`の後は明示的`ApplyDeferred`を構成する。Bevyのdirect orderingによる自動flushへ依存する場合は、同じ保証をschedule testで証明してから置換できる。
- 両consumerはcurrent epoch snapshotを1回取得してその処理単位で共有し、field revision不変でもSoulの位置が変わり得るslow stepではsampleする。field自体をrebuildしない。

### 3.3 Room summary・cache・lifecycle

`RoomIlluminationState`はRoom entityへ付く非永続derived componentである。`hw_infra`にはpure sampling / aggregation valueだけを置き、Bevy `Component`、cache、schedule、load reset adapterはroot `bevy_app`が所有する。

| field | 意味 |
| --- | --- |
| `world_epoch` | field revision再利用とworld replacementを区別するepoch |
| `field_revision` | どのcurrent CPU fieldから計算したか |
| `room_topology_revision` | canonical Room partitionのrevision |
| `room_tile_signature` | 当該Roomのsorted tile集合だけから導くEntity非依存signature |
| `sample_count` / `dark_cells` | aggregateした有効tile数とdark tile数 |
| `mean_luminance` / `min_luminance` | `u16` luminanceの整数aggregate |
| `dark_ratio_q16` | floatでなく0〜65535のfixed-point ratio |

- `RoomTopologySignature`は`hw_world`が全Roomのcanonical sorted tile-group集合から導く。既存`RoomMaskSignature`（union mask）を流用せず、semanticなpartitionが変わった時だけrevisionを進める。`RoomTileLookup`がsignature / revisionを併せて所有し、`rebuild_rooms`と`validate_rooms_system`の双方、`replace()`、`Default` / world resetで同じpublish規則を使う。Entityのdespawn / recreateだけでは進めない。
- `RoomIlluminationCache`のkeyは`(world_epoch, field_revision, room_topology_revision, room_tile_signature)`であり、Entity IDを保持しない。同一revision / tile signatureで再生成されたRoomにはcacheからstateを再付与して再aggregate 0を守る。topology外だけでなく、current world epoch・field revision・topology revision以外のkeyもpruneしてboundedにする。
- pure aggregationはnon-emptyのvalid roomだけを扱う。meanは`(sum + sample_count / 2) / sample_count`、`dark_ratio_q16`は`(dark_cells * 65535 + sample_count / 2) / sample_count`、minは最小luminanceとする。sample count 0、out-of-bounds tile、mask外をRoom tileとして含むmalformed topologyはstateを公開せず、metricを増やしてfail-closedにする。partial aggregate / clampはしない。
- `read_indoor_light_snapshot`が`None`なら既存componentをremoveし、cacheをinvalidateする。public readerもcurrent `(world_epoch, field_revision, room_topology_revision, room_tile_signature)`が一致するstateだけを返す。
- P07自身がnamedかつidempotentな`LoadResetRegistry` hookを登録してcache / metricsをclearする。P05の通知やrehydrate calculationを待たない。normal / rollback / recovery-only / recovery-failed / duplicate resetではreset直後のstate・cache・readをabsentにし、次のRoom detect → field rebuild → summaryでcurrent epochだけを公開する。preflight rejectではhookを走らせず、live worldのstate / cache / epochを不変にする。
- Room summaryはLOS入力ではない。照度→summaryの一方向consumerとし、save payloadへ入れない。

### 3.4 P07 evidence topology

凍結v1のP07は`stage_without_gpu_owner`である。一方、mainlineにはP06 GPU ownerが存在するため、serial P06+P07 subjectをP07 v1 formalとして提出してはならない。

1. P07実装・formal evidenceはP05 formal subjectから分岐したclean P07 subjectで採取する。
2. P07 selectorはP02 presentationとruntime CPU fieldを有効にし、P07のP01 / P02 RenderDoc preservation evidenceを出す。P06-only GPU sidecar / probe / gateは出さず、要求もしない。
3. P06とP07の統合、GPUとのcross-consumer assertion、必要なadditive / successor evidenceはP08が所有する。P07のために凍結v1をrebaselineしない。

## 4. マイルストーン

## M0: P07 selector・consumer-core・evidence基盤を先に通す

### 変更内容

1. Rustの`PerfRttLightSelection`、parser、stage / lane validation、behavior / output / RenderDoc checkpointへ`p07`、`static`、`behavior`、`field-core`、`consumer-core`を追加する。`uses_p02_presentation`と`uses_runtime_field`をP07へ正しく拡張し、P06 GPU ownerの条件をP07へ漏らさない。
2. production Soul / Room consumer境界を通るheadless `consumer_core_driver`を追加する。500 Soul / 16 Room / 576 cell、32 warmup、256 measured、専用writer、`PERF_CONSUMER_CORE` marker、通常captureとの排他を固定する。
3. PythonのCLI、artifact、projection、bundle、fixture、contract readerへP07とconsumer-coreを接続する。対象は少なくとも`scripts/perf_tool/{arguments.py,cli.py,artifacts.py,model.py,summary.py,rtt_light_bundle.py,fixtures.py,rtt_light_contract.py,renderdoc_capture.py}`とする。
4. consumer-core legにはP00既存schemaの`indoor_light_consumers.csv`（各runの256 measured rows）をprimary artifactとして必須化し、named / versioned `consumer-proof-v1` sidecarを同legだけに必須化する。後者はSoulごとのsample / effect最大値、Soul / Room双方のworld epoch・field revision整合、masked / stale effect数、256 elapsed値、scoped allocation event / bytesを持つ。behavior legには別のnamed / versioned `consumer-lifecycle-v1` sidecarを必須化し、7 case × repeatごとのold-epoch recovery effect / Room summary read、current epoch / revision、reset後のabsent状態を記録する。CSV / proofはnon-consumer-core legで、lifecycle sidecarはnon-behavior legで禁止し、file-set validatorがschema version、run / case / repeat key、missing / extraをfail-closedにする。既存凍結`timeline.json`を無自覚に変更しない。
5. native acceptance launcherのP07 selector、consumer-core leg、source checkpoint、formal-leg導出、RenderDoc P01 / P02 resource mapを追加する。checkpoint順は`after-field-core` → `after-consumer-core` → `before-registration`とし、consumer-coreもCapture binary SHA一致対象にする。formal process countは固定値でなくfrozen contractのleg / caseから導出し、P07 self-testで固定する。

### 主な変更ファイル

- `crates/bevy_app/src/plugins/startup/perf_scenario/{config.rs,output.rs,behavior_driver.rs,renderdoc_capture.rs,consumer_core_driver.rs}`
- `crates/bevy_app/src/plugins/startup/{mod.rs,perf_scenario.rs}`
- `scripts/perf_tool/`のP07 selector / artifact / bundle / RenderDoc modules
- `.codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py`とそのself-test

### 完了条件

- [x] P07 selector、全4 lane、consumer-core command、native planがP07をrejectしない
- [x] missing / extra / malformed artifact、wrong row count / order、fixture mismatch、metric failure、stage leakをfail-closedで拒否する
- [x] P07はP01 / P02 RenderDoc evidenceを要求し、P06 GPU-only evidenceを要求・生成しない
- [x] consumer-core / behavior sidecarのleg境界、checkpoint順、Capture binary SHA、source fingerprint mismatchをfail-closedで拒否する
- [x] frozen v1 contractのID / threshold / hashは不変

## M1: balance・sample境界・単位を固定する

### 変更内容

1. P00 artifactと現`lamp_buff_system`からcurrent rate、tick、radius mismatch、stack挙動を記録する。
2. `LightCell::luminance`のbounds / mask-aware gameplay read、strict room-sample read、binary decision、fixed vectorを追加する。
3. `hw_energy::constants::OUTDOOR_LAMP_RADIUS_TILES`を唯一のtile sourceにし、P04の`LightRadiusTiles`へ明示変換する。旧float radius APIと文書を削除・更新する。
4. binary threshold、non-stack、out-of-bounds fail-darkを恒久Soul Energy docsへ記載する。

### 主な変更ファイル

- `crates/hw_infra/src/lighting/{field.rs,mod.rs}`
- `crates/hw_energy/src/constants.rs`
- `crates/bevy_app/src/systems/lighting/components.rs`とP04 emitter adapter
- `docs/soul-energy.md`

### 完了条件

- [x] rate / tick / threshold / roundingに`TBD`がない
- [x] tileとworld unitを混用するradius APIがない
- [x] bounds / mask / OOB goldenがclampなしでpassする
- [x] 複数emitterでもpure decisionは1回

## M2: Soul recoveryをP04 field consumerへ置換する

### 変更内容

1. root energy ownerの`lamp_buff_system`を`SoulLightRecoverySet`のfield consumerへ置換し、Logic chainから旧Lamp queryを外す。
2. Lamp query、per-Lamp stacking、raw distance計算、`OUTDOOR_LAMP_EFFECT_RADIUS`、旧`energy_lamp_*` metrics / perf outputを削除またはP07 consumer metricsへ置換する。
3. recoveryはPostActorで`IndoorLightingRebuildSet`後に実行し、独自unpaused gate、`steps_this_frame() == 0` early-return、1〜5 step catch-up loopを持つ。
4. Soul Transformは`WorldMap::world_to_grid`からcheckedな`LightGridPos`へ変換し、OOBはdarkとする。world-distance / unclamped座標変換を再導入しない。
5. epoch-aware snapshot readを一回に集約し、invalid / stale / unavailable fieldでeffect 0・sample数の不正増分なしを保証する。P07 production sourceのraw snapshot readが0であるpolicy testを追加する。

### 主な変更ファイル

- `crates/bevy_app/src/systems/energy/{lamp_buff.rs,mod.rs}`とenergy perf / output owner
- `crates/bevy_app/src/plugins/{logic.rs,lighting.rs}`
- `crates/bevy_app/src/systems/lighting/runtime.rs`のconsumer API tests

### 完了条件

- [x] Soul 1体あたりfield sample / effectは各slow stepで最大1回
- [x] emitter数に応じたN×M queryがない
- [x] Wall / Door / power / movementとeffectが同じfield expected値になる
- [x] step 0 early-return、1〜5 catch-up step、pause後、checked OOB、stale epoch / unavailableでeffect 0を含むschedule testがpassする

## M3: Room summary、topology identity、load resetを追加する

### 変更内容

1. `hw_infra::lighting::room_summary`へpure `u16` aggregation value / functionだけを追加する。`RoomIlluminationState(Component)`、`RoomIlluminationCache(Resource)`、metrics、reader、schedule、P07 reset hookは`bevy_app/src/systems/lighting/room_summary.rs`が所有する。
2. `hw_world`へper-Room `RoomTileSignature`とcanonical `RoomTopologySignature` / revisionを追加し、`RoomTileLookup`の`replace()` / `Default` / resetと`rebuild_rooms` / `validate_rooms_system`の双方でpublishする。global union `RoomMaskSignature`の意味論は変更しない。
3. Room detect後のdeferred flush、`IndoorLightingRebuildSet`後のpause-open summary、current epoch / field / topologyだけを残すbounded cache reattach / prune、current epoch-only readerを実装する。
4. P07 own named reset hookを登録し、normal / rollback / recovery-only / recovery-failed / duplicate resetでcacheとold componentをclearする。preflight rejectはlive stateを保つ。rehydrate時にsummaryを計算しない。

### 主な変更ファイル

- `crates/hw_infra/src/lighting/{mod.rs,room_summary.rs}`
- `crates/hw_world/src/{room_detection.rs,room_detection/ecs.rs,room_systems.rs,lib.rs}`
- `crates/bevy_app/src/systems/lighting/{mod.rs,room_summary.rs,runtime.rs}`
- `crates/bevy_app/src/plugins/lighting.rs`とload-reset registration owner

### 完了条件

- [x] unchanged `(world_epoch, field_revision, room topology revision, room tile signature)`ではaggregate 0、entity再生成後はcacheからnew entityへ再付与される
- [x] semantic partition変化、`validate_rooms_system`だけのinvalid Room despawn、topology / epoch / field pruneが正しくcache / componentを更新し、cache sizeはboundedである
- [x] fixed vectorのmean / min / dark ratio rounding、zero sample、invalid tileが契約どおりになる
- [x] snapshot unavailable / stale epochでold stateが公開されず、summaryはsave payloadにない
- [x] preflight reject不変、replacement系loadのold summary absent、duplicate reset idempotenceが実transaction経路のlocal behavior 21 / 21でpassする

## M4: soak・formal evidence・文書を閉じる

### 変更内容

1. P00 `consumer-core` laneで500 Soul / 16 Room / 576 cellを32 warmup + 256 measured call × 3 runで測る。p95 / p99は256 elapsed値から算出し、allocationはscoped event / bytes合計0を確認する。
2. Door連続開閉、power churn、Room再検出、Soul移動、1〜5 catch-up、pause、loadを組み合わせたsoakとdeterministic auditを実行する。
3. `door-state-v1`と`load-normal-v1`、`load-preflight-reject-v1`、`load-rollback-v1`、`load-recovery-only-v1`、`load-recovery-failed-v1`、`load-duplicate-reset-v1`の全7 behavior caseを各3反復する。behavior専用`consumer-lifecycle-v1` sidecarでold-epoch recovery effect / Room summary readが0であることを検証する。
4. full P05 correctness SHA `56fa6bd3ee2dc4de0e24c529066836048b2ee5bc`を`--prerequisite-commit`としてancestor確認したfresh clean P07 subjectで、actual adapter / window backendとusable `renderdoccmd` / `qrenderdoc` / `librenderdoc`を確認してからS0 → 同一subject / source fingerprintのS1 → formalを実行する。registered valid attemptだけをformalとし、Audit、behavior、Capture、Memory、field-core、consumer-core、P07 RenderDocを独立legとしてbundle化・登録・offline再検証する。
5. `docs/indoor_lighting.md`、`docs/soul-energy.md`、`docs/room_detection.md`、`docs/architecture.md`、`docs/performance-profiling.md`、必要なら`docs/save_load.md`を更新する。実装後のplayer pathでHelp impactを判定し、回復条件の説明が変わるならHelp manifest / provider / exhaustive coverage / approval snapshotも更新する。

### 完了条件

- [ ] `RLV1-BUNDLE-VALID`、`RLV1-P01-RTT`、`RLV1-P02-DOOR-DOMAIN`、`RLV1-P03-FIELD`、`RLV1-P04-EMITTER`、`RLV1-P04-STEADY`、`RLV1-P05-LIFECYCLE`、`RLV1-P07-CPU-CONSUMERS`、`RLV1-P07-CONSUMER-CPU`のexact集合を満たす（formal bundle待ち）
- [x] local behavior / consumer-coreでeffect / summary / field revision / world epoch mismatch、masked / stale effect、old-epoch Room readが0
- [ ] P07 native S0 / S1 / formal artifactがfail-closed validatorと独立attempt / baseline verifierを通る
- [x] Help impactを`Update required`と確定し、Help provider / approval snapshot / docs indexを同期する

## 5. 検証計画

- pure luminance boundary / threshold / unit / non-stack、mean / min / Q16 ratio / zero sample golden。
- one Soul / many Lamp、many Soul / one Lamp、Door / Wall / power / movement / 1〜5 catch-up / pause table test。
- Room entity recreation、same mask but changed partition、same tile signature reattach、invalid Room prune、stale / unavailable epoch test。
- transaction実経路のpreflight reject不変、normal / rollback / recovery-only / recovery-failedのold state absent、duplicate reset idempotence test。
- P07 selector / consumer-core / artifact / proof sidecar / bundle / RenderDoc resource map / native launcherのpositive・negative self-test。
- `python3 scripts/dev.py check`
- `python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`
- `python3 scripts/dev.py cargo -- test --workspace`
- `PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py self-test`
- `python3 scripts/dev.py verify`
- native renderer / formalは`hell-workers-run-native-acceptance` SkillのP07 stage recipeのみを使い、P05-lineage subject、S0 / S1 / formal、offline artifact revalidationを完了する。
- 実装batchの完了前に`hell-workers-review-help-impact` SkillでSoul Energyの実player pathを辿り、`Update required`または`No impact`を確定する。
- docs変更後は`python3 scripts/dev.py docs --write`、`python3 scripts/dev.py docs --check`、`git diff --check`を実行する。

## 6. リスクと対策

| リスク | 対策 |
| --- | --- |
| current bugを仕様として維持する | current stack / unit mismatchは観測値、targetはbinary non-stackと明記する |
| raw snapshotやrevision単独でload後の旧stateを読む | canonical epoch-aware readだけを通し、state / cache keyに`WorldEpoch`を含める |
| Room entityをcacheしてstale参照になる | Entityをcacheせず、canonical tile signatureとcurrent epoch / revisionをkeyにする |
| pause中に古いslow stepを再適用する | recovery独自のunpaused gateと1〜5 step testを持つ |
| P06入りsubjectをP07 v1 formalと誤認する | P05-lineage isolationを必須にし、統合はP08へ残す |
| consumer-coreが集計値だけで違反を隠す | per-Soul / per-Room proof sidecarとfail-closed validatorを持つ |

## 7. ロールバック方針

- gameplay consumerだけをfeature-offしてもP03〜P06 field / visualは保持できる。
- 旧Lamp effectへ一時的に戻す場合も、全`PowerConsumer`をLamp扱いするqueryとper-Lamp stackはcorrectness fixとして戻さない。
- `RoomIlluminationState`とcacheはderivedなのでregistrationを外して安全に削除でき、save migrationは不要。
- P07 evidence branchが不成立でも凍結v1を改変しない。P06/P07統合の証跡方針はP08の明示的な判断として扱う。

## 8. AI引継ぎメモ

### 現在地

- 進捗: `90%`（M0〜M3実装、M4 local behavior / consumer-core / quality / docs完了。P05-lineage native formal未完了）
- 完了済み: P07 selector / consumer-core / artifact / native基盤、epoch-aware Soul recovery、Room summary / topology / reset、Help / docs、local behavior 21 / 21とconsumer-core 3 / 3
- 未完了: P05-lineage clean subjectでの全7 behavior、S0 / S1 / RenderDoc / formal bundle登録

### 次のAIが最初にやること

1. 現P07実装差分をfull P05 correctness SHAを祖先にしたclean P07 evidence subjectへ移し、同一source fingerprintを固定する。
2. Skill launcherでS0 → S1 → 全7 behavior / field-core / consumer-core / RenderDocを含むformalを順に実行する。
3. exact gate集合をoffline再検証してvalid attemptだけを登録し、P08統合へ引き渡す。

### ブロッカー/注意点

- P06はユーザー承認で受理済みだが、P07 frozen v1 formalはP06 GPU ownerを含むsubjectでは採取しない。
- Room summaryを保存しない。P05のextension pointを使うが、P05 reset通知を想定しない。
- 新しいcomfort / health効果へscopeを広げない。

### 最終確認ログ

- plan review: `2026-08-17` / P04/P05 lifecycle、Room owner / schedule、P07 tooling、frozen v1 evidence topologyをread-only照合
- local quality: `2026-08-17` / pass (`dev.py check`, workspace Clippy 0 warning、workspace test、`dev.py verify`)
- behavior: `2026-08-17` / final source binary `0a04b05a…`、全7 case × 3 = 21 / 21 valid。old-epoch recovery effect / Room read 0、recovery-failedだけfield revision null / Room state unavailable
- consumer-core: `2026-08-17` / 同一binary、valid 3/3、500 Soul / 16 Room / 576 cell、p95 median 0.022560 ms、p99 median 0.025054 ms、sample/effect max 1、stale/masked 0、allocation 0
- tooling: `2026-08-17` / pass (`perf.py self-test`, native acceptance self-test、P07 S1 plan ready)
- Help / docs: `2026-08-17` / `Update required`、provider / approval snapshot更新、`docs --write / --check`、link / plans index check、`diff --check` pass

### Definition of Done

- [ ] M0〜M4が完了（M0〜M3完了、M4 formal待ち）
- [x] Soul effectがbinary non-stackのepoch-aware field consumerになっている
- [x] Room summaryがworld epoch / field revision / canonical topologyを追跡し、Entity再生成とloadでstale stateを残さない
- [ ] P05-lineage `stage=p07`がexact gate ID集合、全7 behavior case、consumer-core / RenderDoc evidenceを満たす
- [x] Help impact reviewとgameplay / Room / architecture / performance docs更新が完了

## 9. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-08-17` | `Codex` | M0〜M3とM4 local経路を実装。binary non-stack回復、epoch/topology-aware Room summary、P07 tooling / consumer-core、Help / docsを追加し、full verifyとlocal 3-runを通過。P05-lineage native formalは未完了として保持。 |
| `2026-08-17` | `Codex` | P07を実装可能な計画へ改訂。P05-lineage formal、PostActor recovery / pause gate、epoch-aware Room cache / reset、canonical Room topology、P07 consumer-core / native / RenderDoc evidence、全7 behavior caseを固定した。 |
| `2026-08-04` | `Codex` | P00の10 Hz / rate / threshold、consumer-core数値gate、P05 reset hook ownershipへ同期。 |
| `2026-08-03` | `Codex` | GPU表示から独立したCPU field consumerとしてgameplay / Room計画を具体化。 |
