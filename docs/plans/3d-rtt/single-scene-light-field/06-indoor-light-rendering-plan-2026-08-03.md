# P06: 室内 Light Field GPU表示計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `single-scene-light-field-06-indoor-light-rendering-plan-2026-08-03` |
| ステータス | `Accepted — user-approved RenderDoc RD0 timeout exception; S0 / S1 valid, formal artifact remains unregistered` |
| 作成日 | `2026-08-03` |
| 最終更新日 | `2026-08-17` |
| 作成者 | `Codex` |
| 親計画 | [`../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md`](../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md) |
| 直接依存 | [P01](01-single-scene-rtt-plan-2026-08-03.md)、[P02](02-topdown-presentation-plan-2026-08-03.md)、[P04](04-indoor-light-runtime-integration-plan-2026-08-03.md)、[P05](05-indoor-light-save-lifecycle-plan-2026-08-03.md) |
| 受入基盤 | [P02-A](02a-p02-acceptance-infrastructure-plan-2026-08-12.md)の登録済みP02 formal / actual-window artifactをperformance referenceとして再検証してから使う |
| 後続 | [P08](08-legacy-cleanup-release-plan-2026-08-03.md) |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: CPU fieldを実装しても、Terrain / Wall / Door /大型構造物が別material経路のままでは照度表示とload resetが一致しない。
- 到達したい状態: 現production fixtureの100×100を含む、grid dimensionsに対応した共有Light Field textureをrevision単位で1回uploadし、全`Structural3d` materialが同じworld XZ mappingでsamplingする。
- 成功指標: emitter数によらずtexture / upload / binding数が一定、Door / Wall遮光がCPU expected fieldとpixel probeで一致し、load開始時に旧照明が表示されない。

### 2026-08-17 実装結果

- epoch-aware CPU→単一linear RGBA8 `Image` bridge、P06 own load reset hook、black-first、revision／epoch dedup、upload metricsを実装した。
- Terrain LOD1／LOD1-lite／LOD2と全Structural3dを同じLight Field receiverへ移し、finite shared `TopDownStructuralMaterial` poolと`MeshTag` Door root anchorを実装した。
- Rust／Python／RenderDoc／bundle／native selectorを`stage=p06`へ拡張し、GPU sidecar、timeline GPU publication、RenderDoc `gpu_light_field` checkpointをfail-closed schemaへ接続した。
- production headless behavior 7 case×3反復は21 / 21 valid。workspace test、`profiling-renderdoc` Clippy、perf / RenderDoc / native helper self-test、`dev.py verify`はpassした。
- Help impactは`No impact`。既存Outdoor Lampの給電結果を描画へ接続するだけで、配置／給電／Door操作／save-load条件、UI文言、通知、設定、shortcutは不変である。
- commit `19ad5fec1e7c8bfa83ae971f047b174b1e48aa72`でclean subjectを作成後、P06 S0 / S1はvalidだった。formalはRD0の`renderdoccmd capture`がdeadline超過またはprocess groupを残して失敗し、valid formal artifact / baseline登録には至らなかった。
- 2026-08-17にユーザーは上記RD0 failureを例外としてP06を受理した。この判断は無効artifactをpassへ書き換えるものではない。M4のunchecked formal項目と再現用記録は残し、P07 v1 formalへP06 GPU ownerを混入させない。

## 2. スコープ

### 対象（In Scope）

- CPU fieldから`Assets<Image>`へのbridge、texture format / sampler / revision / epoch管理。
- Terrain 3 LOD materialへのLight Field bindingと共通sampling helper。
- TopDown構造物materialの導入、Wall / ProvisionalWall / Door / Floor / Bridge /大型構造物のreceiver移行。
- directional shadow後のlocal light合成、Wall側面 / 上面sampling policy。
- world replacement時のblack clearと、GPU upload / material asset count instrumentation。
- shader / render headless checksとnative acceptance。

### 非対象（Out of Scope）

- emitter / LOS / gameplayの再計算（P03 / P04 / P07）。
- Soul / Familiar / foregroundへのlocal light適用。
- PointLight / SpotLight、shadow map、normal-map lighting、soft shadow。
- Section / projector uniformの物理削除（P08）。

## 3. texture / sampling契約

### 3.1 GPU resource

| 項目 | 契約 |
| --- | --- |
| dimensions | publish済みfieldの`GridDimensions`と同じ。現production fixtureは100×100であり、map dimensionが可変化した場合だけrecreateする |
| format | linear `Rgba8Unorm`（sRGB変換なし）。RGB=local light、A=indoor mask |
| sampler | nearest、clamp-to-edge、mipmapなし |
| ownership | `IndoorLightTexture` resourceがhandle、uploaded revision、uploaded `WorldEpoch`を所有 |
| allocation | liveなImage / handleは常に各1。通常のworld replacementでは同じhandleのbytesだけをblack→new fieldへ更新し、field revisionごとにassetを増やさない。dimension変更時だけold handleをrebind完了後に除去して置換する |
| upload | field revisionまたはepoch変更時だけ。steady-state 0回 |
| reset | P06自身がroot `LoadResetRegistry`へGPU cache reset hookを登録する。成功するworld replacementでblack bytesを同期反映し、uploaded revision / epochをinvalid化する。P05専用messageや「P05 reset通知」を待たない |

100×100×4 bytes = 40,000 bytesを基準容量とする。row pitch / backend stagingはP00 captureで別に記録する。

### 3.1.1 P05 lifecycleとの境界

- `IndoorLightTexture`はroot visual ownerの**非永続GPU cache**であり、`IndoorLightRuntime`、P05のsave schema、rehydrate step、`WorldEpoch`の正本を変更しない。
- upload systemはP05が公開したepoch-aware read APIだけを使う。raw snapshot / `snapshot()`を直接読む経路は作らず、current `WorldEpoch`と一致するfieldが返るまでblackを維持する。
- P06のreset hookは正常load、rollback、recovery-only、duplicate resetでidempotentにblack化する。一方、preflight rejectはP05がreset前に止めるため、P06 cacheをclear / uploadせず、live worldのtextureを保持する。
- reset hook後はold epochのfieldをuploadしない。upload比較はfield revisionだけでなくcurrent epochを必ず含め、rebuild後の同epoch fieldだけを採用する。Visual scheduleではCPU field rebuild完了後にbridgeを実行する。
- `Image`のblack clear、material rebind、old assetの除去は同じcache ownerが行う。loadごとに新しいassetを積み上げたり、receiverごとに別handleを持ったりしない。

### 3.2 world XZからUV

共通WGSL helperを1つだけ持つ。

```text
grid_x = floor((world_x - map_origin_x) / tile_size)
grid_y = floor((map_origin_z - world_z) / tile_size)
uv = (vec2(grid_x, grid_y) + 0.5) / vec2(map_width, map_height)
```

- CPUの`world_to_grid`とY/Z反転規則をgolden vectorで照合する。
- 現行のcentered mapでは`map_origin_x = -map_width * tile_size / 2`、`map_origin_z = map_height * tile_size / 2`をshared half-extentsから導く。Terrain ID用の既存clamp helperを流用せず、Light Field helperだけはbounds外をblackとしてreturnする。
- map外はblackとし、clamp sampleによる端cellの光漏れを防ぐためshader側でbounds判定する。
- UV / dimensions / origin / tile sizeはmaterialごとの複製値ではなく共有uniform contractにする。

### 3.3 surface policy

- Terrain / Floor / Bridge /大型構造物の上面はfragment world positionのcellをsampleする。
- Wall側面は自己遮光cellを直接sampleせず、surface normal方向の隣接cellへ小さくoffsetする。
- Wall上面はcardinal 4近傍からluminance最大の1 cellを選び、そのcellのRGB一式を採る。tieはNorth → East → South → West、map外はblackとし、channel-wise maxで色を合成しない。
- Doorはcurrent leaf transformではなくDoor rootのgrid cellとresting surface normalから同じ規則を使う。Open visual回転で別cellをsampleしない。M3ではroot cellとresting normalをWGSLへ渡すrenderer-supported per-instance `DoorLightAnchor` contractを先に確定する。ECS componentだけを置くことやopened leafのfragment world positionから推測することは不可とし、shared material handleを保ったままspawn / rehydrate / moveへ追従するGPU inputを使う。
- local lightはlinear空間で`directional_styled_rgb + base_color_rgb * local_light_rgb`として既存directional sun / shadow styling後に加算する。local専用ambient floorは0、gainは初期1.0、CPU fieldは1.0でsaturateし、その後は既存Scene tone mappingへ渡す。乗算方式をalternativeとして残さない。
- P03のUNORM16 linear RGBをinteger式`(value * 255 + 32767) / 65535`でround-half-upしてu8へ変換し、alphaはindoor=255 / outdoor=0とする。変換はP03のpure helperを唯一の実装とし、upload system / shaderで別roundingやgamma変換を持たない。
- Soul billboard、Familiar、indicator、speech、selection、OutdoorLamp器具spriteはsampleしない。

## 4. material移行

現`SectionMaterial`はsection discard以外にwall build progress、wall height、surface / UV、directional shadow、prepassを所有する。単純な`StandardMaterial`置換をしない。

`TopDownStructuralMaterial`を`hw_visual`へ追加し、次をP06完了時点で代替する。

- completed / provisional build progress clip。
- wall surface / macro detailとdirectional shadow styling。
- depth prepassのbuild-progress clip。
- shared Light Field texture / sampler / map uniform。
- finite shared material handlesによるTank / MudMixer等のstate表示。

binding layoutは着手時にderive出力とshaderを照合し、次を予約値とする。

| material | uniform | Light Field texture | sampler |
| --- | ---: | ---: | ---: |
| Terrain 3 LOD extension | existing 100 | 133 | 134 |
| TopDown structural extension | existing 100、current assets 101〜110 | 111 | 112 |

binding collision testとBevy 0.19 `AsBindGroup` compileを必須にし、推測だけで確定しない。

P06終了時に`MeshMaterial3d<SectionMaterial>` consumerを0にする。ただしSection型 / projector fields / shader file / `CLIP_DISTANCES`の物理削除はP08で参照0を再確認して行う。

P02の`Structural3d` mappingをP06で暗黙に再定義しない。receiver coverageは次をexhaustiveとし、normal spawn、completion、move、rehydrate、world replacement後のcleanupを同じactive presentation routeで検証する。

| P02 presentation class | P06のreceiver owner | 非receiverとして残すもの |
| --- | --- | --- |
| Terrain | M2のLOD1 / LOD1-lite / LOD2 | なし |
| Wall / Door / Floor / Bridge / Tank / MudMixer / RestArea / SoulSpa | M3の`TopDownStructuralMaterial` | legacy 2D mirrorは非描画のP08削除対象でありreceiverにしない |
| Foreground2d（SandPile / BonePile / WheelbarrowParking / OutdoorLamp器具、Familiar、selection、speech等） | なし | P06でtextureをbindしない |

## 5. マイルストーン

## M0: `stage=p06`の受入経路をfeatureと同じbatchで有効化する

### 変更内容

1. P02 canonical formal / actual-window artifactをhistorical readerで再検証し、P06の`RLV1-P06-PERF`が読むP02 compatible referenceを固定する。P05のepoch / mount contractは変更しない。
2. Rustの`PerfRttLightSelection` / parser / scenario config、Python CLI / policy / artifact reader、RenderDoc extractor / bundle validator、native launcherを`p06`へ同時に拡張する。`p05`を`p06`として読んだり、future fieldをdefault値で補完したりしない。
3. P06専用のruntime / timeline / field-core / Capture / Memory / RenderDoc sidecarを追加する。GPU uploadはGPU renderのCapture / Memory / RenderDocだけでrequired、behavior / field-coreを含む非該当laneはcontractどおりtyped `not_applicable`にする。P05の`stage_before_gpu_owner`値をP06 evidenceとして流用しない。
4. `IndoorLightTexture` probeからimage / handle count、logical / staging bytes、revisionごとのupload、steady allocation、uploaded epoch、black clearを採取する。RenderDocにはreceiver binding、shared image、Point / Spot / shadow / local-light pass増分、mask / duplicate 2D passを同一checkpointへ投影する。pixel probeはtone mapping前のlinear targetを読む。
5. P06のproduction indoor-light fixtureとactual-window observationを追加する。P02 actual-window artifactはpresentation referenceであり、P06 color / GPU receiverの代替証拠にしない。Wall / Door / corner / mount side、Open / Closed / Locked、map edge、LOD境界をphase / sidecar / raw artifactから再検証可能にする。
6. missing GPU sidecar、old-epoch upload、image / binding count、pass count、pixel probe、stage / lane / render mismatch、P05 artifactをP06として渡すケースをそれぞれfail-closedにするnegative/self-testを追加する。P00のfrozen v1 threshold、既存stageのschema、registered artifactをcandidate結果に合わせて変更しない。

### 主な変更ファイル

- `crates/bevy_app/src/plugins/startup/perf_scenario/{config.rs,config/parse.rs,config/tests.rs,behavior_driver.rs,field_core_driver.rs,output.rs,renderdoc_capture.rs}`
- `crates/bevy_app/src/plugins/startup/perf_scenario/`のfixture / Capture / actual-window observer
- `scripts/perf_tool/{arguments.py,policy.py,artifacts.py,fixtures.py,model.py,rtt_light_bundle.py,rtt_light_contract.py,renderdoc_capture.py,renderdoc_extract.py,renderdoc_foundation.py}`
- `scripts/perf_tool/contracts/rtt_light_migration_v1.json`（凍結済みP06 gate / stage rowをread-onlyのvalidator targetとして使う。P06ではschema / threshold / registered artifactを変更しない）
- `.codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py`と対応self-test

### 完了条件

- [x] `p06` selectorがstatic / behavior / field-core / Capture / Memory / RenderDoc / bundle / native commandで明示的に通り、未実装fieldを持つsubjectはmissing evidenceで落ちる
- [ ] P02 referenceとcurrent〜P05 historical artifactが新readerで再検証でき、cross-stage fallbackがない
- [x] P06 GPU source / sidecarを壊すnegative fixtureが対応gateを必ず落とす
- [ ] P06 actual-window observationがP02 profileや`visual_test`を代替証拠として受理しない

## M1: CPU→Image bridgeを実装する

### 変更内容

1. startupでconfigured world grid dimensionsからblack Imageとnearest samplerを1つ作る（現productionは100×100）。最初のpublish済みfieldが異なる`GridDimensions`を持つ場合と将来のdimension変更では、single live handle契約を保ったatomic rebind / old asset removalを実装する。
2. shared texture resourceを`init_visual_handles`より前に初期化し、Terrain / structural materialの全handleがbirth時から同じblack Imageを参照する順序を固定する。
3. P05のepoch-aware read APIでcurrent `WorldEpoch`のfieldだけを読み、revisionとepochを比較してbytesを更新する。
4. root `LoadResetRegistry`へP06 own GPU-cache hookを登録し、successful replacementでblack clear→uploaded revision / epoch invalidation→current epoch fieldのみ再uploadの順を固定する。preflight rejectではhookを実行しない。
5. upload count / bytes / duration / image・handle・material asset count、old-epoch upload、steady allocationをP06 instrumentationへ追加する。
6. `IndoorLightUploadSet`を`IndoorLightingRebuildSet`の後、Door / structural presentation consumerとbehavior observerの前へ置く。同frameのDoor mutation、anchor sync、field rebuild、upload、receiver観測の順をschedule testで固定する。

### 主な変更ファイル

- `crates/bevy_app/src/systems/visual/indoor_light_texture.rs`
- `crates/bevy_app/src/plugins/visual.rs`
- `crates/bevy_app/src/plugins/startup/{mod.rs,startup_systems.rs,visual_handles.rs}`
- `crates/hw_visual/src/material/indoor_light.rs`
- perf metric / artifact schema

### 完了条件

- [x] live Image / handleは常に各1で、normal load / rollback / recoveryでasset countが増えない
- [x] startup順序がTerrain / structural receiver全てのhandle identityを同一black Imageへ固定する
- [ ] unchanged 600 frameのuploadが0
- [x] epoch mismatch時はblack以外をuploadせず、successful replacementではblack-first、preflight rejectではlive texture不変
- [ ] canonical 100×100 fixtureでは40,000-byte payloadとrow orientationが一致し、non-100 `GridDimensions`でもrebind / bounds / old asset removalが一致する

## M2: Terrain 3 LODをreceiver化する

### 変更内容

1. Terrain extension 3種類へ同じtexture / sampler bindingを追加する。
2. 共通`indoor_light_field.wgsl` helperを全fragment shaderから呼ぶ。
3. LOD切替前後でUV / brightnessが一致するpixel probeを追加する。
4. map edge / non-indoor / Door / L字Wall fixtureをcaptureする。

### 主な変更ファイル

- `crates/hw_visual/src/material/terrain_surface_material.rs`
- `assets/shaders/{terrain_surface_material.wgsl,terrain_surface_material_lod1_lite.wgsl,terrain_surface_material_lod2.wgsl}`
- `assets/shaders/indoor_light_field.wgsl`
- visual / material tests

### 完了条件

- [ ] LOD1 / LOD1-lite / LOD2のsamplingが同一cellを指す
- [ ] indoor mask外とmap外がblack
- [ ] directional shadowとの合成順が固定画像と一致する

## M3: 全Structural3dをTopDown materialへ移す

### 変更内容

1. Doorを移す前にBevy 0.19で実現可能な`DoorLightAnchor` GPU transportを確定する。root grid cellとresting normalをopened leaf transformから独立してWGSLへ渡し、per-door material cloneを作らない。spawn / rehydrate / move / cleanupの全routeで同期し、ECS componentだけでshader入力を省略しない。
2. `TopDownStructuralMaterial`、prepass、shared handle registryを追加する。
3. Wall / ProvisionalWallのbuild progress、alpha / prepass behavior、surface表現を移植する。
4. Door / Floor / Bridge / Tank / MudMixer / RestArea / SoulSpaをreceiverへ移す。
5. Tank / MudMixerの有限state material、move / transform sync、load shellをP02契約と結合する。
6. P02 preservation probeをmaterial type名へ依存させずTopDown materialへ移行する。Bridgeのexact mesh / material residency / render layer、Tank / MudMixer state / bounce、all-building exactly-oneを同じproduction fixture / actual-window routeで維持する。
7. `SectionMaterial` consumer countを0へする。

### 主な変更ファイル

- `crates/hw_visual/src/material/{mod.rs,topdown_structural_material.rs}`
- `crates/hw_visual/src/lib.rs`
- `assets/shaders/{topdown_structural_material.wgsl,topdown_structural_material_prepass.wgsl}`
- `crates/bevy_app/src/systems/visual/`
- `crates/bevy_app/src/plugins/startup/visual_handles.rs`とbuilding completion / spawn / rehydrate adapters
- `crates/bevy_app/src/plugins/startup/perf_scenario/p02_actual_window.rs`と`crates/bevy_app/src/systems/save/rehydrate/tests/`のhandle fixture

### 完了条件

- [x] 全`Structural3d`が同じfield revisionをsampleする
- [x] build progress / alpha / prepass / directional shadowが回帰しない
- [x] Open / Closed / Locked Doorが同じroot cellをsampleし、edge / corner / side normal fixtureでanchor transportを検証する
- [ ] per-building material cloneがなく、Tank / MudMixer / Door stateを含むfinite shared handle poolのcardinalityがspawn / load / move後も増えない
- [x] `MeshMaterial3d<SectionMaterial>` query / spawnが0

## M4: reset・性能・native受入を閉じる

### 変更内容

1. P00 `stage=p06`の`door-state-v1`と6 load lifecycle caseを各3反復する。preflight rejectはGPU handle / bytes / checksum不変、reset / upload 0を検証し、normal / rollback / recovery-only / duplicate reset / recovery-failedのreplacement branchだけでblack-firstとold-epoch upload 0を検証する。
2. P00 small / medium / largeでGPU render Captureのwall-frame p95 / p99、Memory RSS / peak live bytes、RenderDocのpass / bindingをP02 compatible referenceと比較する。CPU field-coreはP03/P04/P05 gateを維持し、GPU evidenceの代替にしない。
3. Door Open / Closed / Locked、Wall内外、corner、wall-mounted inward / outwardをlinear pixel probe + P06 production actual-window imageで検証する。P00 §3.2のquantized texelとtone mapping前許容差を使い、最終window framebufferをlinear値の代替比較にしない。
4. quality Low / Medium / High、DPI 1.0 / 1.5 / 2.0の短縮P06 actual-window compatibility matrixでmap-space照明が変形しないことを確認し、FHD / High / DPI 1.0 primary performance matrixへ混ぜない。P02 actual-window profileはP06 color evidenceの代替にしない。
5. audit / behavior / Capture / Memory / field-coreを各required case 3反復し、RenderDocを固定1 frame採取して共通validityを閉じる。formalはclean committed subjectでS0→S1→RD0→formalの順に、`hell-workers-run-native-acceptance` Skillの返すno-prompt launcherだけから実行し、attempt / baseline verifierを独立再実行する。

### 完了条件

- [ ] stale-light frameが0
- [ ] Lamp数を増やしてもGPU texture / material binding数が一定
- [ ] `RLV1-BUNDLE-VALID`、`RLV1-P01-RTT`、`RLV1-P02-DOOR-DOMAIN`、`RLV1-P02-PRESENT`、`RLV1-P03-FIELD`、`RLV1-P04-EMITTER`、`RLV1-P04-STEADY`、`RLV1-P05-LIFECYCLE`、`RLV1-P06-UPLOAD`、`RLV1-P06-RENDER`、`RLV1-P06-COLOR`、`RLV1-P06-PERF`のexact集合を満たす
- [ ] P06 native S0 / S1 / formal artifactがfail-closed validatorと独立attempt / baseline verifierを通る

## 6. 検証計画

- M0 selector / historical reader / GPU sidecar / stage-lane-render mismatch / RenderDoc resource-map / bundle / native helperのpositive・negative self-test。
- Image byte layout / dynamic dimensions / revision / epoch / startup handle identity / steady-state tests。
- P05 transaction実経路でのpreflight-reject不変、normal / rollback / recovery replacement black-first、old-epoch GPU upload 0、duplicate reset idempotence tests。
- WGSL import / binding / shader compile testsと、P06 RenderDoc checkpointでfield image / handle / receiver binding / pass増分を直接検証するtests。P01 composite bindingだけから推測しない。
- CPU expected field対tone-mapping前pixel probeのgolden tests、Terrain LOD seam / centered-map orientation / OOB black tests。
- structural material shared-handle / build-progress / alpha / prepass / DoorLightAnchor / Open-Closed-Locked root-cell / P02 preservation probe tests。
- `python3 scripts/dev.py cargo -- check --workspace`
- `python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`
- `python3 scripts/dev.py cargo -- test --workspace`
- `PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py self-test`
- `python3 scripts/dev.py verify`
- native renderer / GPU / DPI acceptanceは`hell-workers-run-native-acceptance` SkillのP06 stage recipeだけを使い、S0 / S1 / RD0 / formal / offline artifact revalidationを完了する。
- 実装batchの完了前に`hell-workers-review-help-impact` Skillで、Lamp配置→給電→屋内 / 屋外・Door状態→load reset後の実表示を辿り、`Update required`または`No impact`を実経路から確定する。更新が必要ならroot Help manifest / provider / coverage / approval snapshotを同じbatchで更新し、不要なら理由付きの判断を残す。
- docs変更後は`python3 scripts/dev.py docs --write`、`python3 scripts/dev.py docs --check`、`git diff --check`を実行する。native Skill / adapter mirrorを変更した場合は正本とmirrorを同期し、`python3 scripts/check_agent_rules.py`とnative helper self-testも実行する。

## M5: 恒久docs・Help判断・release handoffを閉じる

### 変更内容

1. `docs/indoor_lighting.md`へGPU cache ownership、epoch-aware bridge、black-first / preflight-reject差、receiver / non-receiver、sampling / Door anchor契約を追記する。
2. `docs/architecture.md`、`docs/rendering-performance.md`、`docs/performance-profiling.md`へP06のsystem order、material ownership、P06 stage evidence、P02 reference比較、actual-window / RenderDocの区分を反映する。crate ownershipやdependencyが変わる場合だけ`docs/{crate-boundaries.md,cargo_workspace.md}`も更新する。
3. Help impact Skillの結論を実player pathから出す。Update requiredなら既存Lamp / Soul Energy Help topicを優先してmanifest / provider / exhaustive coverage / approval snapshotを更新し、No impactなら到達したpathと表示上変わらない理由をdecisionとして残す。
4. parent planとplans indexへP06の実際の進捗・formal resultだけを同期し、P08へ渡す`SectionMaterial`物理削除待ちの境界を再確認する。

### 完了条件

- [x] renderer / lighting / performance docsがresource owner、schedule、silent fail-dark / preflight例外、evidence readerを実装と同じ責務で記述する
- [x] Help impactが実経路から`No impact`へ確定し、reasoned decisionが同じchange batchにある
- [x] parent / plans index / docs contractsが同期し、`docs --write / --check`と`git diff --check`が通る

## 7. リスクと対策

| リスク | 対策 |
| --- | --- |
| binding番号衝突 | derive / shaderを同時確認しcompile test。Terrainは133/134を予約 |
| Wallが自己遮光で常時暗い | side normal offsetとtop max-adjacentを固定fixture化 |
| SectionMaterial置換でconstruction clipが消える | parity material完成後にconsumerを一括移行する |
| Imageをrevisionごとに生成してassetが増える | handle固定、bytes更新、asset count gateを持つ |
| load直後に旧textureを1frame表示する | P06 own reset hookでepoch invalidation / black clearをfield uploadより先に処理し、old epoch uploadをartifactで拒否する |
| preflight rejectまでblack化してlive worldを暗くする | P05 reset前rejectをP06 hookが観測しないこと、handle / bytes / checksum不変をtransaction testで固定する |
| Open Doorのleaf transformから別cellをsampleする | `DoorLightAnchor`をWGSLへ明示transportし、root-cell不変のOpen / Closed / Locked goldenを持つ |
| P06をP05 selector / P02 artifactで偽装する | M0でstage-specific source / sidecar / resource-mapを追加し、cross-stage negativeとindependent validatorを必須にする |
| P02 preservation probeが`StandardMaterial`型依存で壊れる | mesh / residency / layer / semantic probeをmaterial type名から分離し、P02 formal referenceを先に再検証する |

## 8. ロールバック方針

- GPU consumerはfeature registrationを外してblack / ambient-onlyへ戻せる。P03〜P05 logical stateは保持する。
- Terrain receiverとstructural receiverを別commitにし、shader問題の範囲を限定する。
- `TopDownStructuralMaterial`移行後にSectionMaterialをP08まで残すため、M3内の限定rollbackを可能にする。ただし二重描画は許可しない。

## 9. AI引継ぎメモ

### 現在地

- 進捗: `ユーザー承認で受理。S0 / S1 valid、RD0 failureのためformal artifactは未登録`
- 完了済み: M0〜M3、M5、production headless behavior 21 / 21、workspace test、full verify、clean subjectでのS0 / S1
- 記録上の未完了: M4のvalid RenderDoc / formal artifactと独立再検証。これは受理判断を覆さず、後続の再現・調査用に残す。

### 次のAIが最初にやること

1. RD0 timeoutを再調査する場合は、受理済みP06を再オープンせず、clean subjectとnative Skillのartifactを使う別の再現調査として扱う。
2. P08ではP06のGPU receiverとP07 CPU consumerを統合し、cross-consumer evidenceを新規に採取する。P07 v1 formalへP06 GPU evidenceを流用しない。

### ブロッカー/注意点

- `frames.csv`はGPU pass timeではない。wall-frame quantileはCapture、pass / binding構造はRenderDocというP00の区分を使う。
- Soul billboardやforegroundへ照明textureをbindしない。
- P05はP06専用messageを公開しない。epoch-aware readとP06 own reset hookを使い、raw snapshotをconsumerへ渡さない。
- native launcherはdirty / uncommitted subjectを受理しない。P06ではその後clean subjectを作成してS0 / S1を実行したが、RD0 failureはartifact validatorの正当なfailureとして保持する。
- section fieldの削除はP08まで行わない。

### 最終確認ログ

- Rust gates: `2026-08-17` / pass (`workspace test`, `profiling-renderdoc clippy -D warnings`, `dev.py verify`)
- native acceptance: `2026-08-17` / S0 / S1 valid。formal RD0は`renderdoccmd capture` deadline / process-group failureでinvalid。ユーザーが例外としてP06を受理
- docs gate: `2026-08-17` / pass (`docs --write / --check`, `check_docs`, Help no-impact decision, `diff --check`)

### Definition of Done

- [ ] M0〜M5が完了
- [x] 全Structural3d receiverが同一field textureを使う
- [ ] steady-state upload 0、stale-light frame 0
- [ ] `stage=p06`のexact gate ID集合、P06 native S0 / S1 / formal、独立artifact / baseline verifierが合格
- [x] Help impact判断とrenderer恒久docs更新が完了

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-08-17` | `Codex` | commit `19ad5fec`でP06 GPU bridge / Terrain・Structural receiver / Door root anchor / reset lifecycle / selector・artifact・RenderDoc toolingを実装。headless 21 / 21とfull verify、S0 / S1はvalid。formal RD0は`renderdoccmd capture` deadline / process-group failureでinvalidだったが、ユーザーが例外としてP06を受理した。 |
| `2026-08-16` | `Codex` | P05完了後のP06 review。P06 own reset hook / epoch-aware bridge、P06 selector・sidecar・RenderDoc・native evidence、Door root-anchor GPU transport、dynamic dimensions、P02 preservation、Help / docs lifecycleを実装前条件として明確化。 |
| `2026-08-04` | `Codex` | P00のevidence区分へ同期し、Wall上面sampleのtie / OOB / RGB選択規則を確定 |
| `2026-08-03` | `Codex` | CPU fieldからGPU texture / Terrain / structural receiverへの移行を独立計画化 |
