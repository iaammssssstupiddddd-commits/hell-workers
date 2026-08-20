# P08: 旧RtT・section・projector撤去とrelease計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `single-scene-light-field-08-legacy-cleanup-release-plan-2026-08-03` |
| ステータス | `Implementation in Progress — final-contract P01 formal fail-closed on 60 Hz pacing / comparable display environment and remaining reference bootstrap pending` |
| 作成日 | `2026-08-03` |
| 最終更新日 | `2026-08-21` |
| 作成者 | `Codex` |
| 親計画 | [`../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md`](../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md) |
| 直接依存 | P01〜[P07](07-indoor-light-gameplay-room-plan-2026-08-03.md)すべて |
| 証跡系譜 | mainlineのP06 GPU owner + P07 CPU consumerを含むfresh P08 serial-integration subject。P07 P05-lineage formalはimmutable referenceでありP08 subjectではない |
| 後続 | なし |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: production経路を停止しても、Soul GLB / shadow proxy、projector、section uniform / shader、hidden structural 2D mirrorが残れば保守対象とbinding / query costが継続する。また凍結contractだけがP08を知り、現行runner / artifact / native経路はP07で停止している。
- 到達したい状態: TopDown + Scene RtT 1枚 + CPU Light Field + GPU Light Field 1枚をproduction契約とし、Soul / Room / GPUが同じcurrent world epoch / field revisionを読むことを同一P08 checkpointで証明してから、旧型・system・asset・featureを物理削除する。
- 成功指標: production runtimeにmask RtT、Soul GLB / shadow、projector、section cut、hidden structural mirrorがなく、凍結`rtt-light-v1`のP08 4 lane / 25 case / exact 16 gateをfresh serial subjectで合格・登録・offline再検証する。

## 2. スコープ

### 対象（In Scope）

- P08 static / behavior / field-core / consumer-core selector、artifact / bundle、RenderDoc、native acceptanceの実行基盤。
- P06 GPU uploadとP07 production CPU consumerを同じP08 runtime / RenderDoc checkpointで照合するcross-consumer evidence。
- P02で停止済みのSoul / Familiar GLB proxy、Soul shadow proxy / material / layer、per-frame projector producerの物理削除。
- 現役`TopDownStructuralMaterial`をsection ownerから独立させた後の`SectionCut` / `SectionMaterial` / Terrain section field / shader branch / prepass branch削除。
- Door / Tank / MudMixerに限定された`LegacyStructural2dMirror`のhidden Sprite writeとshell削除。通常の2D foreground / blueprint / placement ghost Spriteは削除しない。
- frozen projection v1を保持したまま、P08でobsolete runtime inventoryをliteral / derived `0`として出力・検証するstage mapping。
- final native formal、Help impact判断、architecture / rendering / gameplay / save / visual-test docsの同期。

### 非対象（Out of Scope）

- Direct-to-windowへの追加移行。
- section、多層階、Soul shadow、GLB actor backendの再導入。
- 新規照明gameplay、Room UI、hero light、brightness比例balance。
- `scripts/perf_tool/contracts/rtt_light_migration_v1.json`のgate ID / threshold / formal matrix / hash変更、または結果を見たrebaseline。
- P06のinvalid RD0をvalid referenceとして扱うこと、P07のP05-lineage formalをP08 serial proofとして読み替えること。

## 3. 固定契約と着手条件

### 3.1 frozen v1を変更しない

- measurement contract hashは`ba5d6bf7320426b441465df8fae42d6ff80820748ce55e0edf0dbba409dc755a`、fixture hashは`a688d564f8f50c2fdcdbe49dca7625b2cb05d01f8555378215fb8ba89b553eed`をexact pinする。
- P08 required laneは`static` / `behavior` / `field-core` / `consumer-core`の4つ。behaviorは`door-state-v1`と6 load caseの計7 caseで、RenderDoc以外は各3 valid run、RenderDocはmedium / GPU 1 validated frameとする。
- projection schema v1はP08まで列削除・意味変更禁止である。`soul_proxy_3d`、`soul_shadow_proxy_3d`、`familiar_proxy_3d`等はruntime型 / Queryを削除してもprojection列とhistorical readerを保持し、P08 producerはstage契約どおり`0`を出す。
- frozen JSONはread-only source contractとし、M0はproducer / reader / validator / launcherをP08へ接続する。P08実装に合わせてcontract hashを更新しない。

### 3.2 evidence lineageとcanonical reference

- P08 implementation subjectはmainlineのP06実装`19ad5fec1e7c8bfa83ae971f047b174b1e48aa72`とP07実装`6711dd6350a0df7257827e989ee4f91e38c7edd5`を祖先に含むclean committed serial subjectとする。full P05 correctness `56fa6bd3ee2dc4de0e24c529066836048b2ee5bc`と合わせ、この3 SHAをすべて独立した`--prerequisite-commit`としてnative helperに渡しancestor確認する。
- P07 formal subject`a749a580370947f0f64c1685010e625a903324dd` / attempt`82bd460f-31c6-4fa0-8705-a4d702ff4c1f`はP05-lineageのimmutable CPU referenceとして保持する。P08 mainlineへmergeしていないこと自体は問題ではなく、P08 source ancestryの代用にはしない。
- P06の2026-08-17 user-approved RD0 timeout例外はP06 milestone受理であり、P08の`RLV1-P08-MEMORY`が要求するvalid registered P06 GPU referenceの免除ではない。invalid attemptを登録・改変しない。
- P08 finalizeは同一canonical `baseline_root`に`current` / `p01` / `p02` / `p06` / `p07`のvalid registered projectionがある場合だけ許可する。P01 performanceはcurrent、P02 performanceはP01、P06 performanceはP02をreferenceにするため、P08が直接比較するcurrent / P06 / P07だけを先に置いてもP06 finalizeは成立しない。
- canonical evidence checkoutは、既存P07 registered artifactを所有する`/home/satotakumi/projects/hell-workers-p07-v1-evidence`を使う。同checkoutでsourceを切り替え、`current` subject`0ffb8004d7d5d75cfc49d4808c1bdaa4adf64c92` -> P01 remediation subject -> accepted P02 remediation subject`c3515a40543026a588592682889a08d67cbaeff9` -> P06-lineage clean subject -> existing P07 projection -> P08 serial subjectの順に扱う。
- current / P01 / P02 / P06の各新規採取は、そのstageのclean subjectでfresh S0 -> S1 -> RD0 -> formal -> finalize / register -> offline verifyを閉じてから次へ進む。P06 subjectは`19ad5fec1e7c8bfa83ae971f047b174b1e48aa72`を基点に必要最小のRenderDoc extractor / tooling correctionだけを含み、P07 gameplayを含めない。既存P07はartifactを変更せず同じrootからoffline再検証する。
- bundleは実行中`REPO_ROOT/target/perf-runs/rtt-light/rtt-light-v1/`を物理canonical rootとする。全bootstrap、P08 formal、全offline verifierを必ず上記evidence checkout内から起動し、primary worktreeの`--baseline`等でsibling rootを指さない。artifact directoryをcopy / relocateせず、source checkoutを切り替えても同一physical `target`を保持する。
- 全referenceとP08 subjectでactual adapter / backend / window / present mode / driver stable fieldsを照合し、不一致は比較せずfail-closedにする。各subjectのcleanliness / source fingerprint / binary SHA / adapter manifestもhelperで検証する。chainのいずれかをvalid登録できなければP08 formal開始を停止する。
- 2026-08-21時点で、current subject`0ffb8004d7d5d75cfc49d4808c1bdaa4adf64c92` / attempt`3af17e1b-0b93-4917-875f-2a9d6019b73b`はfresh formal、登録、offline verifyまで合格済みである。P01 historical subject`29a4a719e9fe92b10618f36ce548c4bb5a4c7e80`もfresh S0 / S1 / RD0と全formal legを完走したが、attempt`fa6b00a6-aaca-48ce-b467-55d814b3d12d`は`capture-medium-cpu`の`RLV1-P01-PERF`でcurrent比p95 `+10.071186004173182%`、p99 `+9.2527073866044205%`となり、各`<= 5.0%`を満たさず未登録である。同一subjectのfocused 3-run再測定でもp95 `14.833945 ms` / p99 `17.623100 ms`となり再現したため、単なる一時ノイズとして同じformalを繰り返さない。
- P01の次のsubjectは、凍結contract / fixture /閾値を変更せずmedium CPUの実質的なremediationを含むclean descendantとする。fresh S0 -> S1 -> RD0 -> formal -> register -> offline verifyでP01を閉じるまでP02 / P06 / P08の採取を開始しない。Render3d非表示時のshadow projector、camera / LOD / section、全RtT update chain、GLTF animation chainの単純なstage gateはfocused actual-window診断で改善しなかったため、同じ仮説を反復しない。
- P01 remediation subject`9de7834c52ddc506269f732d33d3bc0087522aa2`は、Scene-only WGSLが宣言も参照もしないlegacy composite uniformと、その値だけを更新する毎frame同期systemを削除した。Intel Arc / Vulkan / X11のfocused medium CPU 3-runはp95 `13.414166 ms` / p99 `15.359286 ms`（MAD `0.024155 / 0.116348 ms`）で、registered current `13.660129 / 15.432964 ms`比`-1.8006% / -0.4774%`となり凍結`<= +5%`を満たした。ただしこれはformal attemptではないため、fresh S0 / S1 / RD0 / formalで再現して登録するまでreferenceとは扱わない。mainlineにも同じ修正を`2dcc1341037fd5445cae0f1155afaf50e8e0c970`として取り込み済みである。
- final frozen contractへ揃えたP01 evidence subject`9ba1d52d96d5a8815b82d63c18f151ec1062b50d`は、retired uniformの毎frame同期を復活させず、binding 0のzero-valued descriptor anchorだけを置いてScene texture / samplerの凍結binding `1 / 2`を維持する。fresh S0`task-dashboard-20260820T211411Z-6bc9c93b`、S1`rtt-light-s1-20260820T213654Z-f165ed5c`、RD0、audit / behavior / Capture / RenderDoc / Memoryはすべてvalidとなった。
- 同subjectのformal attempt`5c64d56d-3795-4e77-807b-f11ae8432733`は、最終offline gateで`capture-small-cpu`のp95 `+142.8378075606974%` / p99 `+136.3594189218451%`、`capture-medium-cpu`のp95 `+5.7030134927715581%`が凍結`<= +5.0%`を超えたためfail-closed未登録である。123 gate row中の不合格はこの3行だけで、invalid artifactは保存し、threshold / current reference / contractを変更しない。
- 同一subjectの診断S1`rtt-light-s1-20260820T232141Z-383d53ad`はaudit / Capture / Memoryをvalid完走したが、Captureのsmall / medium / large CPU p50が各`16.669 / 16.670 / 17.086 ms`となった。GNOME Mutter `GetCurrentState`が現在の唯一のactive displayを`2880x1800@60.001`として返し、small / mediumの全3反復が約16.67 msへ同期したため、一時的な単発ノイズ仮説を否定した。frozen environment lockのadapter / backend / window / present / driverは旧referenceと一致する一方、physical display refreshはschema v2のstable fieldに含まれない。比較可能なdisplay/presentation状態を復元・確認するまで同じformalを反復せず、P01登録とP02以降を停止する。

### 3.3 cross-consumer checkpoint

`RLV1-P08-CROSS-CONSUMER`はP07 consumer-coreの別process結果を流用せず、P08 `renderdoc-medium-gpu`の同一runtime checkpointで次を照合する。

| consumer | 必須のsource fact |
| --- | --- |
| CPU publication | current `WorldEpoch`、`FieldSnapshot.field_revision`、field checksum、availability |
| GPU upload | `IndoorLightTexture.uploaded_epoch / uploaded_revision / uploaded_checksum`、同一shared Image / receiver binding |
| Soul recovery | production `apply_light_recovery_effect_system`が実際に読んだepoch / revision、fixture Soul数、sample / effect / stale facts |
| Room summary | 全current Roomの`RoomIlluminationState.world_epoch / field_revision`、topology validity、stale / missing count |

- profiling時だけ使うboundedな`IndoorLightCrossConsumerObservation`（名称は実装時にこれへ固定）をproduction recovery system内で更新し、外部のpure sampler呼出しだけでSoul観測を偽装しない。
- P08 static fixtureはfield / Roomがreadyになった後、capture freeze前のsetup phaseで実際のslow simulation stepを1回通し、fixture内全Soulのproduction observationを確定する。その後`Time<Virtual>`をpauseしてRenderDoc checkpointまでepoch / revision / checksum / topologyが不変であることを要求する。fixed auditではこのsetup step後のcapture境界をvirtual / fixed elapsedの原点にしてCSVを相対時刻化し、fixture準備時間をaudit tickへ混入させない。観測なし、複数revision混在、pause後のfield変更、Soul / Room件数不足、capture原点より前への時刻逆行はfail-closedにする。
- setup step中のActor更新後、`IndoorLightingRebuildSet`より前にfixture Soul / FamiliarのTransformだけをcanonical cellへ戻す。task / vitals / path / recovery observationを後から書き換えず、production recoveryは復元後のcurrent Transformをsampleする。これにより全sizeのfixture identityとcross-consumer checkpointを同じ位置で固定する。
- observationはderived / nonserialized resourceとし、fixture開始とfield unavailable時にclearする。P08 ownerのidempotent load-reset hookでnormal / rollback / recovery-only / recovery-failed / duplicate reset時に即時clear / epoch-invalid化し、preflight rejectではlive observationを変更しない。reset後にproduction recoveryがcurrent fieldを読むまで旧observationを再公開しない。
- additive schema version 1の`indoor_light_cross_consumer.json`をP08 RenderDoc legだけのrequired sidecarとする。その他stage / legでは禁止する。producerの`revision_epoch_consistency` booleanを信用せず、Python validatorが上表のraw factsから再計算する。
- stale epoch / revision、GPU checksum差、Soul observation欠落、Room state欠落 / topology mismatch、sidecar missing / extra / malformedを個別negative testにする。

### 3.4 削除gate

削除はファイル名だけでなく、production consumer・test/tool consumer・frozen historyを分けて判定する。

| legacy | P08着手条件 | 完了条件 |
| --- | --- | --- |
| Soul / Familiar GLB proxy | P02 billboard / foreground pathがnativeで成立 | production / `visual_test`の型・spawn・observer・cache member・asset load 0。現役billboardは維持 |
| Soul shadow runtime / layer | P02でspawn / sync停止済み | runtime entity / material / shader / render layer / light-layer membership 0、projection列はP08値0で維持 |
| projector producer / uniform | P06 Light Field + directional shadowが代替 | producer / constants / uniform fields / WGSL loop 0。`shadow_style.wgsl`の現役directional helperは維持 |
| SectionMaterial / SectionCut | active structural materialを独立ownerへ移行済み | alias / resource / sync / shader / prepass / clip feature 0 |
| Terrain section fields | TopDown-only、dynamic writer 0 | LOD1 / LOD1-lite / LOD2 Rust uniformとWGSL discard 0 |
| structural 2D mirror | Door / Tank / MudMixerの3D state consumerが既存 | marker / hidden child Sprite / old state writer 0、foreground Spriteは維持 |

各cleanup commit前後にscope付き`rg` inventoryを保存する。`docs/plans/archive`、凍結artifact、historical readerの文字列はruntime参照0判定に含めず、allowlist理由を記録する。

## 4. マイルストーン

## M0: P08 serial integration / tooling / reference bootstrap

### 変更内容

1. Rust `PerfRttLightSelection`へP08の4 laneを追加し、`stage=p08`がP02 presentation、P04 / P05 runtime field、P06 GPU owner、P07 CPU consumerを同時に有効化するnamed predicateを置く。stage stringの散在比較だけで合成しない。
2. static output、behavior timeline / lifecycle sidecar、field-core、consumer-core、RenderDoc checkpointをP08へ配線する。`indoor_light_gpu.json`はGPU Capture / Memory caseだけで必須とし、CPU / audit / field-core / consumer-coreでは禁止する。behaviorはfield / GPU / consumer lifecycle、field-coreはruntime field、consumer-coreはP07 CSV / proofをexact file setとして要求し、cross JSONはRenderDoc legだけで許可する。
3. `scripts/perf_tool`のCLI / artifact / projection / bundle / summary / fixture / RenderDoc mapをP08へ拡張する。missing / extra artifact、lane leak、stage leak、row count / order、schema / identity / checkpoint mismatchをfail-closedにする。
4. P08 RenderDocへ`indoor_light_cross_consumer.json`と対応するRust checkpoint / Python extractor / validator / gate producerを追加する。P01 Scene topology、P02 actual presentation、P06 GPU image / pixel probe、P07 CPU consumer、P08 cross factsを同じvalidated frameへ結ぶ。
   - GPU pixel proofはowned `Rgba8Unorm` Light Field imageのdirect readbackとCPU packed RGBA一致を動的に検証し、receiver WGSLのLight Field加算が`main_pass_post_lighting_processing`より前であることを埋め込みsource順序checkで固定する。probe専用PBR cameraの継続readbackは600 steady-updateを自己stallさせるため使用しない。
   - P08 static fixtureはcapture開始前にproduction slow stepをexactly 1回だけ通す。fixed auditのdeterminism elapsedはcapture境界からの相対値とし、setup stepを実行してもcheckpoint 0=`0 ns`、tick 1=`fixed timestep`を維持する。
   - fixture actor位置の復元systemは`GameSystemSet::PostActor`かつ`IndoorLightingRebuildSet`前に置き、recovery / Room / GPUが同じcanonical post-Actor位置とfieldを観測する。
5. native launcherへP08 plan / S0 / S1 / RD0 / formal / verifyを追加する。formal leg / behavior case / preflightからprocess countを導出し、P08 self-testで現在の期待値（25 unique case、contract由来の86 game process）を固定する。
6. source checkpoint順を`... -> after-field-core -> after-consumer-core -> before-registration`とし、field-core / consumer-core binaryもCapture binary SHA一致対象にする。P08 cross sidecarとRenderDoc capture / replayのsource fingerprintを同一にする。
7. §3.2のcanonical physical rootでcurrent -> P01 -> P02 -> valid P06 -> existing P07を順に登録・offline verifyする。reference bootstrapはcleanup commitとは分離し、凍結contractや既存P07 artifactを変更しない。

### 主な変更ファイル

- `crates/bevy_app/src/plugins/startup/perf_scenario/{config.rs,config/tests.rs,output.rs,behavior_driver.rs,renderdoc_capture.rs}`
- `crates/bevy_app/src/plugins/startup/perf_scenario/{field_core_driver.rs,consumer_core_driver.rs,indoor_light_fixture.rs}`
- `crates/bevy_app/src/plugins/startup/{perf_scenario.rs,perf_scenario/capture_driver.rs,mod.rs}`
- `crates/bevy_app/src/systems/{energy/lamp_buff.rs,lighting/room_summary.rs,visual/indoor_light_texture.rs}`
- `scripts/perf_tool/{arguments.py,cli.py,artifacts.py,model.py,summary.py,fixtures.py,policy.py}`
- `scripts/perf_tool/{rtt_light_contract.py,rtt_light_bundle.py,renderdoc_capture.py,renderdoc_extract.py,renderdoc_foundation.py}`
- `.codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py`
- frozen `scripts/perf_tool/contracts/rtt_light_migration_v1.json`はread-only test oracle

### 完了条件

- [x] Rust / Python / nativeのP08全4 lane selector positive testと、unknown lane / stage mismatch negative testが合格
- [x] P08 fixed auditがproduction slow stepを1回通してfixture readyへ到達し、capture-relative virtual / fixed elapsedでdeterminism validatorに合格
- [x] setup step後のfixture actorをlighting前にcanonical cellへ復元し、small / medium / largeで初期位置checkとfixed auditに合格
- [ ] P08はGPU sidecarとconsumer sidecarを同時に生成し、p06 / p07や他legへのcross sidecar leakが0
- [ ] cross-consumer raw factsからvalidatorが一致を再計算し、stale / missing / malformed fixtureがすべてrejectされる
- [ ] P08 RenderDoc checkpointがpausedで、production Soul observation / Room state / CPU field / GPU uploadの同epoch / revision / checksumを証明
- [ ] cross observationはnonserializedで、fixture開始 / unavailable / replacement resetで消え、preflight rejectだけは不変。旧epoch observationをsidecarへ再利用できない
- [ ] schedule assertionがPostActorの`IndoorLightingRebuildSet -> SoulLightRecoverySet -> RoomIlluminationSummarySet`と、後続Visualの`IndoorLightingRebuildSet -> IndoorLightUploadSet -> DoorPresentationSyncSet`を固定する
- [ ] 同一physical canonical rootでcurrent / p01 / p02 / p06 / p07 referenceがvalid registeredかつoffline verifier合格。1つでも欠ければM4 formalへ進まない
- [ ] fresh P01の`RLV1-P01-PERF` medium CPU p95 / p99を凍結current比`<= 5.0%`で合格させて登録する。2026-08-21のhistorical P01 attemptはfail-closed未登録であり、referenceに使わない
- [ ] `python3 scripts/perf.py ... --stage p08`とnative `plan-rtt-light --stage p08`が全required legを列挙する

## M1: legacy character / Soul shadow / projector producerを削除する

### 変更内容

1. [完了 2026-08-20] productionと`visual_test`の`SoulProxy3d` / `FamiliarProxy3d` / `SoulShadowProxy3d` / animation player / face materialとspawn / ready observer / sync / cleanupを削除した。凍結inventoryはliteral 0へ移し、P02の`ActorBillboard3d`とFamiliar foreground Spriteを維持した。
2. [完了 2026-08-20] `SoulProxyOwnerCache`を`ActorBillboardOwnerCache`へrename / narrowし、現役`actor_billboard` lookup / resetだけを残してlegacy soul / shadow / familiar mapを削除した。
3. [完了 2026-08-20] `soul_animation.rs`をbillboard用`SoulAnimVisualState` resolverだけへ狭め、GLB `AnimationGraph` / player / face-material部分を削除した。`visual_test`置換後にshared `CharacterMaterial`とshaderも削除した。
4. [完了 2026-08-20] 未登録だった`sync_soul_shadow_projectors_system`とprojector collection / per-frame material writeを削除した。全shared materialのprojector countが常時0であることを確認できたため、uniform / WGSL fieldも描画結果を変えない独立cleanupとして同時に削除した。
5. [完了 2026-08-20] `CharacterHandles`、`Building3dHandles.soul_scene`、`GameAssets.soul_gltf / soul_scene / soul_face_atlas`とproduction loadを削除した。
6. [完了 2026-08-20] camera / directional lightからshadow-only layer membershipを削除し、`LAYER_3D_SOUL_SHADOW`、shadow-only constants、GLB専用scale、`SoulShadowMaterial`とshader / prepassを削除した。
7. [完了 2026-08-20] `crates/visual_test`のlegacy Soul GLB / shadow mode、ResetElevation input、専用surfaceを削除し、building / terrain TopDown visual testだけを維持した。active actor billboard acceptanceはP02 actual-window / P08 native fixtureを正本とし、`docs/visual_test.md`を同期した。
8. [方針確定 2026-08-20] `assets/models/characters/soul.glb`、face atlas等はGit管理外で、workstation / Blender移行計画がlocal comparison referenceとして所有するため削除しない。production asset catalog / code / `visual_test` loadは0とし、製品release gateへ含めない。将来の削除・canonical化はasset provenanceを扱う同計画で行う。

### 主な変更ファイル

- `crates/hw_visual/src/{visual3d.rs,lib.rs,material/mod.rs,material/character_material.rs,material/soul_shadow_material.rs}`
- `crates/bevy_app/src/systems/visual/{character_proxy_3d.rs,character_proxy_3d/,soul_shadow_projector.rs,soul_animation.rs,actor_billboard.rs,mod.rs}`
- `crates/bevy_app/src/plugins/{visual.rs,startup/asset_catalog.rs,startup/visual_handles.rs,startup/startup_systems.rs}`
- `crates/bevy_app/src/{assets.rs,systems/save/rehydrate.rs,systems/save/rehydrate/presentation.rs}`とfocused save tests
- `crates/hw_core/src/constants/render.rs`
- `crates/visual_test/src/`、`docs/visual_test.md`
- `assets/shaders/{character_material.wgsl,soul_shadow_material.wgsl,soul_shadow_prepass.wgsl}`とreference 0のcharacter assets
- perf Rust inventory query / P08 literal-zero producer。projection v1 / historical readerは削除しない

### 完了条件

- [x] production / workspace test codeのlegacy GLB / shadow type、spawn、observer、system、asset load、render layer、`SOUL_GLB_SCALE / SOUL_FACE_SCALE_MULTIPLIER`参照0
- [x] 1 Soulにつきactive billboard exactly 1、Familiar 3D proxy 0、shadow caster / shadow GLB 0
- [x] actor billboard owner cache / load cleanup / animation-state invalidationが維持される
- [x] `soul_shadow_proxy_3d`等のfrozen projection列は存在し、P08値0、過去stage readerは旧値を同じ意味で読める
- [x] visual_test building / terrain modesとP02 / P08 actual-window actor probesが合格

## M2: active materialをre-homeしてsection / projector fieldを削除する

### 変更内容

1. [完了 2026-08-20] `topdown_structural_material.rs`へ独立`TopDownStructuralMaterialExt` / uniform / factory / fragment / prepass shaderを作り、`SectionMaterialExt` aliasを解消した。
2. [完了 2026-08-20] 独立materialはP06のbuild progress / wall height、provisional wall alpha / prepass、directional shadow style、Door root light anchor、shared Light Field bindingを保持する。`AsBindGroup` / WGSLの予約layoutはuniform `100`、Light Field texture / sampler `111 / 112`を維持し、cut / projector fieldは新uniformへ持ち込まない。
3. [code完了 2026-08-20 / fresh P08 S0+S1 valid] `Building3dHandles`全handle、spawn、Door / Tank / Mixer material swap、wall completion、P02 actual-window exact-handle probe、save rehydrate fixtureを独立型へ移した。material / Image handle identityのRust契約を維持し、committed subject `3735cab2` のactual-window / Vulkan S0+S1でruntime pipeline成立を再確認した。visual semanticsの最終判定はfresh RD0 / formalで閉じる。
4. [完了 2026-08-20] structural consumer 0確認後に`SectionMaterial` alias / factory、`SectionCut` resource / sync / plugin registration / tests、`section_material*.wgsl`を削除した。存在しない`systems/visual/section_cut.rs`は新設していない。
5. [完了 2026-08-20] Terrain LOD1 / LOD1-lite / LOD2のRust uniformと全fragment / prepass WGSLからcut position / normal / thickness / active、projector arrays / metadata / discard / loopを削除した。Light Field、terrain blend、directional shadow、alpha / depth契約は維持した。
6. [完了 2026-08-20] `shadow_style.wgsl`は全削除せず、Soul projector helper / args / mixだけを除去し、現役directional shadow helperを保持した。
7. [完了 2026-08-20] `MAX_SOUL_SHADOW_PROJECTORS`、projector radius / feather / strength / forward extentをreference 0確認後に削除した。全active shader / pipelineでclip参照0を確認し、`WgpuFeatures::CLIP_DISTANCES`要求も削除した。committed subject `3735cab2` のfresh S0+S1はIntel Arc / Vulkan / X11でvalidation errorなくvalid完了した。

### 主な変更ファイル

- `crates/hw_visual/src/material/{section_material.rs,topdown_structural_material.rs,terrain_surface_material.rs,mod.rs}`
- `crates/hw_visual/src/{lib.rs,visual3d.rs}`、`crates/hw_visual/README.md`
- `crates/bevy_app/src/plugins/startup/{visual_handles.rs,startup_systems.rs}`
- `crates/bevy_app/src/systems/{jobs/building_completion/spawn.rs,visual/building3d_cleanup.rs}`
- `crates/bevy_app/src/plugins/startup/perf_scenario/p02_actual_window.rs`
- `crates/bevy_app/src/systems/save/rehydrate/tests/`
- `crates/bevy_app/src/{main.rs,plugins/visual.rs}`
- `assets/shaders/{section_material*.wgsl,topdown_structural_material*.wgsl,terrain_surface_material*.wgsl,shadow_style.wgsl}`

### 完了条件

- [x] `TopDownStructuralMaterial`はSection aliasでなく独立type / plugin / shaderで、全structural handle / queryが新型を使う
- [x] `rg "SectionCut|SectionMaterial|section_cut|MAX_SOUL_SHADOW_PROJECTORS|soul_shadow_projector"`のproduction Rust / active shader参照0
- [ ] provisional wall build progress / alpha / prepass、Door Open / Closed / Locked root-cell light、Tank / Mixer state、directional shadowが維持される
- [ ] Terrain LOD1 / LOD1-lite / LOD2 shader compile、shared Light Field binding、native pixel / RenderDoc probeが合格
- [x] structural Rust `AsBindGroup` / fragment / prepassのbindingは`100` / `111` / `112`で一致し、collision / layout drift testが合格
- [x] `CLIP_DISTANCES`削除後もVulkan adapter / actual windowが起動し、shader validation error 0

## M3: structural mirror / config / runtime inventoryを掃除する

### 変更内容

1. [実装完了 2026-08-20 / native再確認待ち] `LegacyStructural2dMirror`を付けるDoor / Tank / MudMixer hidden child Sprite spawnとmarkerを削除した。Wall / Floor等の別purpose Sprite、blueprint、placement ghost、foreground actorは対象外とした。
2. [実装完了 2026-08-20 / native再確認待ち] Doorの`sync_door_presentation_system`からchild Sprite branchだけを外し、3D material / transform / `DoorPresentationState`更新を保持した。`IndoorLightUploadSet -> DoorPresentationSyncSet`のP07同frame edgeは維持する。
3. [実装完了 2026-08-20 / native再確認待ち] `hw_visual::tank` / `mud_mixer`の旧Sprite state writerとplugin registrationを削除し、root `sync_structural_presentation_state_system`による3D stateを唯一のruntime consumerにした。
4. [実装完了 2026-08-20 / actual-window再確認待ち] P08 perf fixtureはstructural child Sprite `0`を期待し、p02〜p07のhistorical artifact expectationを変更せずP08 stage mappingだけを追加した。actual-window probeは既存のDoor semantic root / production 3D syncを継続利用する。
5. obsolete DevPanel / env / visual-test flag / message / reset entry / queryをinventoryし、runtime ownerがないものだけ削除する。active P05 light reset、P06 GPU reset、P07 Room cache resetは残す。
6. frozen projection列 / contract / historical readerは保持する。runtime Queryを消した列はP08 producerでliteral / derived `0`とし、`artifacts.py` / `rtt_light_contract.py` / RenderDoc validatorがP08の0を要求する。
7. save lifecycleはderived visual / Room state / cross observationをserializeしない。preflight rejectはlive state不変、normal / rollback / recovery-only / recovery-failed / duplicate resetはold epoch field / GPU / Soul / Room / cross observation read 0とdark-first / idempotenceを維持する。
8. P07 Room cache key`(world_epoch, field_revision, topology_revision, room_tile_signature)`とbounded prune契約を回帰固定する。Room validationによるinvalid despawn、同tileのRoom entity再生成、topology revision / tile signature変更、field unavailableの各経路で旧component / cacheを残さずcurrent stateだけを再付与する。
9. P07 CPU lifecycleのPostActor順`IndoorLightingRebuildSet -> SoulLightRecoverySet -> RoomIlluminationSummarySet`と、P06 / P07 visual順`IndoorLightingRebuildSet -> IndoorLightUploadSet -> DoorPresentationSyncSet`をnamed-set schedule testで固定する。cleanupでregistration / run condition / cross observation timingを変えない。

### 主な変更ファイル

- `crates/hw_visual/src/{visual3d.rs,tank.rs,mud_mixer.rs,lib.rs}`
- `crates/bevy_app/src/systems/jobs/building_completion/spawn.rs`
- `crates/bevy_app/src/systems/visual/building3d_cleanup.rs`
- `crates/bevy_app/src/plugins/{visual.rs,logic.rs}`
- `crates/bevy_app/src/plugins/startup/perf_scenario/{indoor_light_fixture.rs,p02_actual_window.rs}`
- `crates/bevy_app/src/plugins/startup/perf_scenario.rs`
- `scripts/perf_tool/{artifacts.py,rtt_light_contract.py,rtt_light_bundle.py}`
- save / reset / behavior focused tests、DevPanel / config inventoryでremaining ownerがある実ファイル

### 完了条件

- [ ] Door / Tank / MudMixer hidden Sprite / marker / old writer 0、各building rootにactive presentation exactly 1
- [ ] Door semantic change -> field rebuild / upload -> 3D presentationの同visual-frame schedule testが合格
- [x] rebuild -> recovery -> Room summaryとrebuild -> upload -> Doorの4 ordering edgeがschedule assertionで合格
- [ ] P08 actual-windowはstructural child Sprite 0、foreground child Sprite 1、owner-linked 3D exactly 1を証明
- [ ] active load-reset hook、Room summary / cache fail-dark、GPU black resetを誤って削除していない
- [ ] Room invalid despawn / same-tile entity recreation / topology change / field unavailableで旧state / cache 0、current keyだけが公開される
- [ ] dead runtime config / message / cache member / query 0。frozen projection列とhistorical readerは維持

## M4: final性能・製品・docs gateを閉じる

### formal entry condition

- fresh clean committed P08 serial subjectである。
- P05 `56fa6bd3ee2dc4de0e24c529066836048b2ee5bc`、P06 `19ad5fec1e7c8bfa83ae971f047b174b1e48aa72`、mainline P07 `6711dd6350a0df7257827e989ee4f91e38c7edd5`の3 SHAを`--prerequisite-commit`でancestor確認する。P07 evidence subject`a749a580...`はflagへ渡さない。
- S0とS1が同一subject commit / source fingerprint / actual adapter / window backendを証明する。
- usable `renderdoccmd` / `qrenderdoc` / `librenderdoc`があり、P08専用fresh RD0がvalidである。P06のinvalid RD0やP07の旧RD0は流用しない。
- 同じphysical canonical baseline rootのcurrent / p01 / p02 / p06 / p07 registered referenceがoffline verify済みで、adapter / backend / window / present / driver stable fieldsが比較可能である。

### formal matrix

| leg | case | repeat / 条件 |
| --- | --- | --- |
| audit | small / medium / large CPU | 各3 valid |
| behavior | Door 1 + load 6 | 各3 valid、計7 case |
| Capture | 3 size × CPU / GPU | 各3 valid |
| RenderDoc | medium GPU | 1 validated frame |
| Memory | 3 size × CPU / GPU | 各3 valid |
| field-core | large CPU | 3 valid、32 warmup + 256 measure + 600 steady |
| consumer-core | large CPU | 3 valid、32 warmup + 256 measure |

unique formal case IDは25。native helperはpreflightを含むgame process数をcontractから導出し、現在のP08期待値86をself-testする。固定値を手書きしたlauncher分岐だけに依存しない。

### required gates

- validity: `RLV1-BUNDLE-VALID`
- preservation: `RLV1-P01-RTT`、`RLV1-P02-DOOR-DOMAIN`、`RLV1-P02-PRESENT`
- field / lifecycle: `RLV1-P03-FIELD`、`RLV1-P04-EMITTER`、`RLV1-P04-STEADY`、`RLV1-P05-LIFECYCLE`
- rendering: `RLV1-P06-UPLOAD`、`RLV1-P06-RENDER`、`RLV1-P06-COLOR`
- gameplay / integration: `RLV1-P07-CPU-CONSUMERS`、`RLV1-P07-CONSUMER-CPU`、`RLV1-P08-CROSS-CONSUMER`
- final budget: `RLV1-P08-FRAME`、`RLV1-P08-MEMORY`

### exact budget / reference

- P03 field-core p95 `<= 2 ms`、p99 `<= 4 ms`。
- P07 consumer-core p95 `<= 1 ms`、p99 `<= 2 ms`、1 Soul / slow step sample `= 1`、effect `<= 1`、masked / stale effect `= 0`、scoped allocation event / byte `= 0`。
- P05 replacement load casesは`old_epoch_field_reads = 0`に加え、P08で`old_epoch_gpu_uploads = 0`。preflight rejectはlive field / epoch / consumer state不変。
- P08 Capture p95 / p99は各caseのcurrent reference比`<= +5%`、large GPUはp95 `<= 16.667 ms`、p99 `<= 25 ms`。
- P08 Memoryはallocator accounting error `= 0`、max RSSはcurrent比`<= +5%`、large peak-live deltaはCPUをp07、GPUをp06 referenceとして各`<= 4 MiB`。
- candidate結果を見て閾値・reference・contract hashを緩和しない。変更が必要なら新baseline generation / proposalとしてP08を停止する。

### 製品・docs・登録

1. `hell-workers-run-native-acceptance` SkillでS0 -> S1 -> RD0 -> formal -> finalize / register -> attempt / full baseline offline verifyを順に実行する。valid registered attempt以外を完了根拠にしない。
2. Door / Wall / Tank / Mixer / all `BuildingType`、Soul billboard / Familiar foreground、quality / DPI、save / rollbackをactual-windowでspot-checkし、renderer / validation error 0を確認する。
3. `hell-workers-review-help-impact` Skillを実装後の実経路に対して実行し、`Update required`または`No impact`を根拠付きで確定する。P02で削除済みのplain V説明をP08で再度変更したと仮定しない。
4. durable contractを恒久docsへ同期し、P00〜P08の歴史証跡を保持したままplan familyをarchive / deleteするか親計画の完了時に判断する。凍結artifactや完了計画を単なる`rg 0`のために改変しない。

### 更新対象docs

- `docs/architecture.md`
- `README.md`
- `docs/README.md`
- `docs/DEVELOPMENT.md`
- `docs/cargo_workspace.md`
- `docs/crate-boundaries.md`
- `docs/building.md`
- `docs/world_layout.md`
- `docs/indoor_lighting.md`
- `docs/soul_energy.md`
- `docs/room_detection.md`
- `docs/save_load.md`
- `docs/rendering-performance.md`
- `docs/performance-profiling.md`
- `docs/visual_test.md`
- `docs/assets_workflow.md`（asset削除時）
- `docs/map_generation.md`
- `docs/blender-setup.md`（`soul.glb`削除時）
- `docs/art-style-criteria.md`（現行契約なら更新、歴史的判断なら明示allowlist）
- `docs/help-screen.md`（Help impactがUpdate requiredの場合）
- `crates/hw_visual/README.md`

### 完了条件

- [ ] exact 16 gateがすべてpassし、25 case / required repeats / RenderDoc 1 frameがvalid
- [ ] P08 attemptがcanonical indexへ登録され、attempt verifierと全baseline verifierがpass
- [ ] current -> p01 -> p02 -> p06 reference chainとp07 locator / SHA ledger / source lineageが同じcanonical rootで解決する
- [ ] native actual-window / Vulkan / RenderDoc / allocator evidenceがfresh P08 subjectと一致
- [ ] Help impact decision、durable docs、generated docs indexが一致
- [ ] legacy runtime inventory 0とfrozen historical schema保持を同時に証明

## 5. 最終受入matrix

| fixture | 必須観測 |
| --- | --- |
| 直線Wall + Door | Open通光、Closed / Locked遮光、field uploadと3D visualが同frame |
| L字corner | exact corner光漏れなし、Door root anchor不変 |
| wall-mounted Lamp | inward側のみ発光、invalid mount / unsuppliedはdark |
| many Lamp | 50 emitterでもtexture / binding数一定、Soul effect non-stack |
| Soul / Familiar | Soul billboard exactly 1、Familiar foreground、GLB / shadow caster 0 |
| Room | current topologyの全Room stateがCPU fieldと同epoch / revision、stale / missing 0 |
| all BuildingType | exhaustive class、duplicate / invisible 0、move / state表示維持 |
| load 6 case | preflight不変、replacementはdark-first、old epoch field / GPU / Soul / Room read 0 |
| quality / DPI | map-space light範囲不変、camera composition正常 |
| P08 cross checkpoint | CPU / GPU / Soul / Roomのepoch / revision / checksum整合 |

## 6. 検証計画

- 各削除対象のbefore / after scope付き`rg` inventoryとallowlist review。
- focused Rust unit / schedule / save / shader / material handle test。
- P08 Rust / Python / native selector、artifact、bundle、RenderDoc positive / negative self-test。
- `python3 scripts/dev.py check`
- `python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`
- `python3 scripts/dev.py cargo -- test --workspace`
- `python3 scripts/dev.py verify`
- `python3 scripts/dev.py docs --write`後にgenerated `docs/README.md` / `docs/plans/README.md`をreview。
- `python3 scripts/dev.py docs --check`
- `python3 scripts/check_help_impact.py`
- native acceptance SkillによるS0 / S1 / RD0 / formal / offline verification。
- `git diff --check`と最終clean commit / source fingerprint確認。

## 7. リスクと対策

| リスク | 対策 |
| --- | --- |
| contractだけP08対応でrunnerが証跡を作れない | cleanup前のM0で4 lane / artifact / native / RenderDocをpositive / negative testまで閉じる |
| reference chain不在でP08 / P06 performance gateを計算できない | same canonical rootへvalid current -> P01 -> P02 -> P06 chainとP07を登録・offline verifyするまでformalを停止 |
| `SectionMaterial`削除で現役TopDown receiverも消える | 独立Ext / uniform / shaderへ全consumerを先に移し、alias 0を確認してから削除 |
| owner cache削除でbillboard cleanupが壊れる | actor billboard lookup / resetをrenameして保持し、legacy mapだけ削除 |
| `shadow_style.wgsl`全削除でdirectional shadowが壊れる | projector helperだけを削除し、directional helper / goldenを固定 |
| frozen perf列削除で旧artifactを誤読する | projection v1 / historical readerを保持し、P08 runtime値だけ0にする |
| visual_testがlegacy GLB / shadow assetを保持する | Soul legacy modeを削除し、active actor acceptanceをP02 / P08 actual-windowへ一本化 |
| cross proofを別processの値で合成する | same P08 RenderDoc checkpointのraw factsとproduction Soul observationだけを許可 |
| cleanupでP05〜P07 reset / scheduleを消す | reset / schedule focused testsと全7 behavior caseをM3 / M4 gateにする |
| final値を見てgateを緩める | frozen hash / thresholdをpinし、新世代承認なしの変更を禁止 |

## 8. ロールバック方針

- M0、legacy character、material re-home、section field deletion、mirror deletion、docs / evidenceを独立commit列にする。
- cleanup後にconsumerが見つかった場合は、該当cleanup commitだけを戻してownerを特定する。mask RtT、Soul shadow runtime、GLB backendを完成形へ再導入しない。
- material regression時は独立`TopDownStructuralMaterial`移行commitまで戻し、`SectionCut`を機能として復活させない。
- P08 formal不合格時はCPU publication / GPU upload / Soul / Room / renderer / reference bootstrapをartifact境界で分離し、frozen gateを書き換えない。
- section / V / Soul shadow / GLB actor backendの製品再導入はrollbackではなく新proposalとnative evidenceを要求する。

## 9. AI引継ぎメモ

### 現在地

- 進捗: `M0 implementation in progress / M1・M2 code cleanup complete / M3 hidden mirror implemented / fresh P08 S0+S1 valid / current reference registered / final-contract P01 S0・S1・RD0・formal legs valid but performance gate fail-closed / comparable display environment・以降のreference bootstrap・P08 formal pending`
- 完了済み: P00〜P07、P08 contract / gate設計、P08 4 lane selector、artifact file-set、RenderDoc schema v4 / cross sidecar、native 25 case / 86 process self-test、stopped projector producer / uniform / WGSL cleanup、P08 production slow-step fixture priming、fixed-audit capture-relative clock、post-Actor actor位置復元、同一commitのfresh S0 / S1、Door / Tank / MudMixer hidden structural mirrorと旧2D writer、productionとvisual_testのSoul GLB / character material / shadow proxy backend、Help no-impact review。
- 未完了: comparable display環境でのfresh P01 performance再採取 / 登録、P02 / P06 reference登録、actual P08 RenderDoc cross checkpoint、M2のRenderDoc pixel / visual semantics確認、M4 formal。local legacy character assetはworkstation / Blender移行計画のcomparison referenceとして保持方針を確定済み。
- 現ブロッカー: final-contract P01 subject`9ba1d52d`はRD0と全formal legをvalid完走したが、現在の60.001 Hz display状態ではsmall / medium CPUが約16.67 msへ同期し、凍結current比gateを超える。contractが固定するadapter / backend / window / present / driverだけではこの差を検出できないため、比較可能なphysical display / presentation状態の復元を確認するまでP01を再実行・登録せず、P02以降へ進めない。

### 次のAIが最初にやること

1. physical display / compositor / presentation状態がregistered current採取時と比較可能であることをread-onlyで確認する。現在の60.001 Hzだけの状態では再実行しない。比較可能な状態を復元できた場合だけP01 subject`9ba1d52d96d5a8815b82d63c18f151ec1062b50d`でfresh S0 -> S1 -> RD0 -> formalを採取し、`RLV1-P01-PERF`を合格させて登録 / offline verifyする。
2. P01登録後だけP02をfresh採取 / 登録し、P06の既存valid legまたは必要なfresh evidence-only subjectをP02 referenceでfinalize / registerする。existing P07を同じphysical rootからoffline再検証する。
3. committed P08 subjectのfresh RD0でstructural / Terrain / cross-consumer pixel semanticsを閉じ、canonical referenceが揃った後だけM4 formalを採取する。

### ブロッカー/注意点

- `TopDownStructuralMaterial`は独立`TopDownStructuralMaterialExt`へ移行済みである。binding `100 / 111 / 112`とbuild-progress / prepass / directional shadow / Light Field契約を維持する。
- `ActorBillboardOwnerCache.actor_billboard`は現役である。resource全体を削除しない。
- `shadow_style.wgsl`と2D Spriteには現役consumerがある。文字列一致だけでファイル全体を削除しない。
- P08のobsolete perf列は値0であり、列0ではない。frozen projection / historical readerを維持する。
- frozen environment lock schema v2はphysical display refreshをstable fieldとして記録しない。現在の60.001 Hz pacingと旧currentの高速pacingを、adapter / backend / window / present / driver一致だけで比較可能と判断しない。
- 実機検証には`hell-workers-run-native-acceptance` Skillを必ず使う。
- 機能・runtime data変更後、commit / completion前に`hell-workers-review-help-impact` Skillを必ず使う。

### 最終確認ログ

- Rust gates: `2026-08-20` / `pass (M2 structural / terrain shader contract tests under profiling-renderdoc、Clippy workspace all-targets、python3 scripts/dev.py verify)`
- headless P08 audit: `2026-08-20` / `pass (target/perf-runs/p08-production-glb-removal-smoke-4、small CPU、Valid 1 / Invalid 0、determinism signature 4ea0f6ef43859c30)`
- native acceptance: `2026-08-20` / `partial (cleanup committed subject 3735cab2、source fingerprint 22bfca85...でfresh S0 / P08 S1がvalid。Intel Arc / Vulkan / X11でS1のaudit 3、Capture 18、Memory 18、field-core 3、consumer-core 3 runはinvalid 0。S0/S1のCapture SHA 3aa544f5...、Memory SHA eb09c1f9...は一致。S1 job rtt-light-s1-20260820T082815Z-20f021f6。RD0 / formalは未採取)`
- reference bootstrap: `2026-08-21` / `partial fail-closed (Intel Arc / Vulkan / X11、Mesa 26.1.6 / kernel 7.1.8。同一physical rootでcurrent 0ffb8004 / attempt 3af17e1bをfresh formal・登録・offline verify。P01 29a4a719 / attempt fa6b00a6はS0・S1・RD0・全formal leg validだが、medium CPU p95 15.035866 ms / p99 16.860931 ms、current 13.660129 / 15.432964比 +10.071% / +9.253%でRLV1-P01-PERF不合格。focused 3-runでも再現し、未finalize・未登録のままP02以降を停止)`
- P01 remediation probe: `2026-08-21` / `pass as focused diagnostic only (subject 9de7834c、Intel Arc / Vulkan / X11、medium CPU 3/3 valid。p95 13.414166 ms / MAD 0.024155、p99 15.359286 ms / MAD 0.116348でregistered current比 -1.801% / -0.477%。Scene-only shaderで未使用だったcomposite uniform / sync system除去。fresh S0 / S1 / RD0 / formalは未採取)`
- final-contract P01 evidence: `2026-08-21` / `partial fail-closed (subject 9ba1d52d、S0 task-dashboard-20260820T211411Z-6bc9c93b、S1 rtt-light-s1-20260820T213654Z-f165ed5c、RD0と全formal leg valid。attempt 5c64d56d-3795-4e77-807b-f11ae8432733は123 gate row中small CPU p95/p99とmedium CPU p95の3行だけ不合格で未登録。診断S1 rtt-light-s1-20260820T232141Z-383d53adもvalidだがsmall/medium CPU全反復が60.001 Hz相当の約16.67 msへ同期したため、同条件のformal反復を停止)`
- Help impact: `2026-08-20` / `No impact (停止済みproduction GLB backend、非表示structural mirror、到達不能section-cutと未使用GPU feature要求の削除。可視billboard / 3D presentation、通常gameplayのinput / state semantics / save / label / workflowは不変)`
- docs gate: `2026-08-20` / `pass (docs --write / --check、check_docs、diff --check)`

### Definition of Done

- [ ] M0〜M4が完了
- [ ] valid current / P01 / P02 / P06 reference chain、existing P07、fresh P08 registered formalが同じcanonical rootにある
- [ ] exact 16 gate / 25 formal case / required repeatsが合格
- [ ] productionの旧mask / GLB proxy / shadow / projector / section / hidden mirror参照0
- [ ] frozen projection v1 / historical readerが保持され、P08 obsolete inventory値0
- [ ] Help / native / docs / workspace full gateが完了
- [ ] 親計画を完了し、durable docsへの移管とplan familyのarchive / delete判断が完了

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-08-21` | `Codex` | final frozen contractのP01 subject`9ba1d52d`でfresh S0 / S1 / RD0と全formal legをvalid完走。offline gateはsmall CPU p95 / p99とmedium CPU p95の3行だけ不合格となり未登録。追加S1でsmall / medium全反復の約16.67 ms pacingと現在の唯一のactive display `60.001 Hz`を確認し、単発ノイズ仮説を棄却。比較可能なdisplay状態を復元するまでformal反復とP02以降を停止 |
| `2026-08-21` | `Codex` | P01のScene-only WGSLが参照しないlegacy composite uniformと同期systemをremediation subject`9de7834c`で削除し、mainlineへ`2dcc1341`として同一修正を取り込み。Intel Arc / Vulkan / X11のfocused medium CPU 3-runはp95 `13.414166 ms` / p99 `15.359286 ms`でregistered current以内へ改善。formal evidenceではないため、fresh same-subject S0 / S1 / RD0 / formalを次のhard gateとして維持 |
| `2026-08-21` | `Codex` | canonical evidence rootでfresh current subject`0ffb8004` / attempt`3af17e1b`をformal登録・offline verify。P01 subject`29a4a719`はfresh S0 / S1 / RD0と全formal legを完走したが、attempt`fa6b00a6`がmedium CPUの凍結current比gateをp95 `+10.071%` / p99 `+9.253%`で不合格となりfail-closed未登録。focused actual-window 3-runで再現し、改善しなかった4系統の単純stage-gate仮説を打ち切ってP02以降を停止 |
| `2026-08-20` | `Codex` | production PostActorのfield rebuild -> Soul recovery -> Room summaryと、Visualのfield rebuild -> GPU upload -> Door presentationを実行順で検証するfocused schedule assertionを追加。Git管理外のlegacy character assetはworkstation / Blender移行計画のlocal comparison referenceとして保持し、production / visual-test consumer 0と分離 |
| `2026-08-20` | `Codex` | cleanup committed subject `3735cab2`でfresh actual-window S0とP08 S1をvalid取得。Intel Arc / Vulkan / X11、同一source fingerprint / Capture・Memory binary SHAでaudit / Capture / Memory / field-core / consumer-coreの57 processを完走し、`CLIP_DISTANCES`削除後のpipeline起動を確認。これはRD0 / formal / registered evidenceではない |
| `2026-08-20` | `Codex` | `visual_test`をbuilding / terrain TopDown試験へ限定し、Soul GLB / animation / face / shadow modeと旧矢視UIを削除。remaining consumerがなくなったshared CharacterMaterial / SoulShadowMaterial / shadow-only layer・constants・shaderを削除 |
| `2026-08-20` | `Codex` | production Soul GLB asset catalog、CharacterHandles、proxy observer / sync、animation player / face materialを削除。owner cacheをActor billboard専用へnarrowし、obsolete perf inventoryをliteral 0へ移行。visual_testのlegacy GLB / shadow surfaceは次単位へ分離 |
| `2026-08-20` | `Codex` | actor復元subject `addc5724`でfresh S0 / P08 S1をvalid取得。続いてDoor / Tank / MudMixer hidden structural Sprite、marker、旧2D state writerを削除し、3D state consumerへ一本化。P08 fixtureだけchild Sprite 0へ更新し、p02〜p07 historical projectionを保持 |
| `2026-08-20` | `Codex` | P08 static fixtureをproduction slow stepでprimeし、fixed auditのvirtual / fixed elapsedをcapture境界相対へ修正。headless small 1-runとfresh S0がvalidとなり、S1 fixed auditも旧paused待ちを解消。S1 Captureでmedium / large Soul移動を検出したため、PostActorかつlighting前にfixture actor位置だけをcanonical cellへ復元し、headless medium / largeをValid 2 / Invalid 0で再検証。stopped Soul projector producer / uniform / WGSL cleanupも独立commitで完了 |
| `2026-08-19` | `Codex` | M0を`6675f751`でcommit。existing P07 attemptをprimary canonical rootへ原子的登録し、current / p01 / p02 / p04 / p05 / p07の6 stage・6,200 fileをoffline verify。P06 fresh formal retryはRD0で同じ600秒deadlineを再現したため、probe専用PBR cameraを廃止し、owned Light Field direct GPU pixel readback + receiver WGSL pre-post-processing順序checkへ修正開始 |
| `2026-08-18` | `Codex` | M0実装開始。Rust / Python / nativeへP08 4 laneを追加し、GPU + CPU consumer合成、RenderDoc checkpoint schema v4、production Soul observation、Room / CPU / GPU cross validator、P08 cross sidecar、25 case / 86 process self-testを実装。native reference bootstrapとactual RenderDocは未実施 |
| `2026-08-18` | `Codex` | P08を現行mainline / frozen contract / evidence topologyへ再レビュー。M0 tooling・reference bootstrap、same-checkpoint cross-consumer proof、TopDown material re-home、frozen projection維持、legacy character / mirrorの実consumer、fresh 25-case formalを実装順へ固定 |
| `2026-08-04` | `Codex` | P00の全formal legへ最終計測を同期し、統計3反復と固定frame RenderDoc captureを分離 |
| `2026-08-03` | `Codex` | 旧runtime / shader削除とfinal acceptanceを独立計画化 |
