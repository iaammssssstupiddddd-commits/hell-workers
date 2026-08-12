# P01: Soul mask撤去・単一Scene RtT移行計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `single-scene-light-field-01-single-scene-rtt-plan-2026-08-03` |
| ステータス | `Completed` |
| 作成日 | `2026-08-03` |
| 最終更新日 | `2026-08-12` |
| 作成者 | `Codex` |
| 親計画 | [`../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md`](../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md) |
| 直接依存 | [P00](00-baseline-gates-plan-2026-08-03.md) C00-D / C00-E（clean `current` formal 5 leg と登録済み `baseline-index.json`） |
| 後続 | [P02](02-topdown-presentation-plan-2026-08-03.md)、[P06](06-indoor-light-rendering-plan-2026-08-03.md)、[P08](08-legacy-cleanup-release-plan-2026-08-03.md) |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: Sceneと同解像度のSoul mask target、専用Camera3d、mask proxy、composite拡張、運用toggleが常時costと保守経路を増やしている。
- 到達したい状態: `RttRuntime`はviewport / scale設定と**1つだけのworld color Scene handle**を所有し、overlay compositeはSceneを通常1回sampleする。
- 成功指標: Soul mask RtTのproduction symbol / entity / asset / passは0である。一方、P00の安定projectionとstage gateは、削除されたmask target / camera / proxyを明示的な`0`として記録し、P01自身の不在を証明できる。

ここで撤去する`Soul mask RtT`は、Soul輪郭用の`SoulMask*` / `soul_mask_*` active routeだけを指す。P03以降の室内照明用`indoor_mask_cells`やLight Field alphaを削除・改名する作業ではない。

## 2. スコープ

### 対象（In Scope）

- `RttRuntime.soul_mask`とmask target lifecycleの削除。
- `Camera3dSoulMaskRtt`とmask RenderLayerの削除。
- `SoulMaskProxy3d`、`SoulMaskMaterial`、GLB ready / sync / cache / rehydrateの削除。
- composite material / WGSLのScene-only化。Scene texture / samplerのVulkan descriptorは現行どおりfragment set 2のbinding `1 / 2`を維持し、maskの`3 / 4`を撤去する。
- DevPanel / env toggle / perf scenario / visual_testのSoul mask active route削除。
- resize、DPI、quality変更時のScene target再生成とcamera / material rebind維持。
- P01を実行できるstage-aware profiling、RenderDoc、native acceptance launcher、raw artifact reader / writer、gate extractorの整備。
- `rtt_light_migration` projection v1に、P01でのmask target / proxy等の意味上の`0`を明示記録する互換adapter。

### 非対象（Out of Scope）

- visible Soul GLBからbillboardへの変更（P02）。
- `SoulShadowProxy3d`の動作停止（P02）とprojector uniform / shaderの物理削除（P08）。
- Camera2d二重passの整理（P02）。
- Light Field texture、室内照明用alpha、Wall / Door遮光（P03〜P06）。
- frozen `rtt-light-v1` JSON、projection v1、current formal artifactの書換え。契約変更が必要ならP00の手順どおり別generationを作り、referenceとcandidateを再採取する。

## 3. 現状とギャップ

| 経路 | 現行 | P01終了状態 |
| --- | --- | --- |
| runtime | `RttRuntime { scene, soul_mask, ... }` | Scene color handle 1つ（viewport / scale設定は維持） |
| camera | `Camera3dSoulMaskRtt` order -2 | entity / query / sync 0 |
| proxy | `SoulMaskProxy3d` + owner cache | type / spawn / cleanup / rehydrate 0 |
| material | `SoulMaskMaterial` | plugin / asset / shader 0 |
| composite | Scene + mask texture / sampler、mask loop | Scene texture / sampler 1組、通常sample 1回 |
| toggle | `RenderPerfToggles.soul_mask_enabled` / `HW_DISABLE_SOUL_MASK` | field / env / button / label 0 |
| generic profiling output | mask proxyのcomponent queryとraw count | active component query 0。P00互換の意味上のmask countはstage=`p01`で明示`0` |
| window evidence | mask targetがSceneと同じ物理解像度 | `mask_target_present=false`。存在しないtargetの寸法をScene値で偽装しない |
| RenderDoc | schema v2が2 target・2 texture・2 sampler・mask camera 1を固定 | stage-aware schema v3でScene 1、mask target / camera / pass / binding / sample / proxy 0 |
| native formal launcher | `current`を固定 | `--stage p01`で5 formal leg、stage gate、artifact generationを選べる |
| `visual_test` | 独自Soul mask RtT / material / proxy / resize契約 | Scene-only target / camera / compositeに一致 |

### 3.1 着手判定（stop / go）

P01のproduction変更は、次の全条件を満たすまで開始しない。

1. P00 C00-D / C00-Eのclean `current` formal attemptが、audit / behavior / Capture / RenderDoc / Memoryの5 legすべてでvalidとなり、`baseline-index.json`から再検証できる。`RLV1-P01-PERF`はこのcurrent referenceなしには判定不能である。
2. `rtt-light-v1`のcontract / fixture hashとprojection v1をfreeze済みとして扱う。P01はstage-aware producer / readerを追加しても同JSONやcanonical SHAを書き換えない。
3. P00 current raw artifactはhistorical readerで再検証できることを先にtestし、P01 subjectにcurrent-only schemaを混ぜる経路をfail-closedにする。
4. save / rehydrateとnative acceptance launcherには並行変更があり得る。各ownerとworktreeを確認し、同一ファイルを上書きする前に変更順を合意する。

P00 formal baselineは2026-08-11にattempt `9e813f24-0f7b-47f5-8a8d-e3ff34775370`として登録され、native attempt verifierとbaseline registry verifierの両方でvalidである。P01のproduction変更・candidate採取を開始できる。

## 4. 実装方針

### 4.1 atomic migration / merge境界

1. P00 current readerとP01 topology fixtureを先に用意し、stage / schema mismatchをfail-closedにする。
2. Scene-only `RttRuntime`、camera、composite、RenderDoc runtime expectationを同じwork packageで切り替える。
3. Soul spawn / observer / cache / rehydrate、layer、material、toggleを削除する。
4. visual_test、perf output、RenderDoc replay / bundle、native launcherを同じsource treeでP01 topologyへ切り替える。
5. P01 gate row生成・current reference比較・native evidenceを閉じる。

M1〜M3は、公開`SoulMask*`型を削除した時点でvisual_test / perf / rehydrateが同時に追従しなければcompile不能になる。したがってこれらは**1つのcompile可能なP01 removal seriesとして連続commitする**。M2だけを先にmergeしたり、dummy mask texture / 一時toggleを残したりしない。

### 4.2 Scene target contract

- `RttRuntime`はScene `Handle<Image>`、physical viewport、target scale factorを所有する。`Handle<Image>`が1つであることと、Resource全体がhandleだけであることを混同しない。
- resize / quality変更はScene imageを一度だけrecreateし、残る`Camera3dRtt` targetとcomposite materialを同じrebind systemで更新する。
- retired Scene handleはcamera / materialから到達不能であることをfocused testで確認する。
- overlay Camera2dとScene composite spriteは維持する。`Camera3dRtt`はorder `-1`、`LAYER_3D`、transparent clear、Scene handle一致を維持し、mask cameraは0にする。
- `LAYER_3D_SHADOW_RECEIVER`、Wall / TerrainのScene RtT参加、`RttDirectionalLight`、receiver-side projector契約はP01で変えない。Wall遮光を使う室内Light Fieldの土台を削らない。

### 4.3 composite contract

- `RttCompositeMaterial`のmask texture / sampler、mask radius / feather、mask blur / center mask / 12方向sample / ring色を削除する。
- uniform `0`を維持する限り、Scene texture / samplerはfragment set 2のbinding `1 / 2`を維持する。mask binding `3 / 4`は存在しない。`AsBindGroup`、WGSL、RenderDoc stage table、reflection testを同じ番号へ固定する。
- fragmentは座標補正後に`scene_texture`を通常1回だけsampleして色を返す。focused static testは`textureSample(scene_texture, ...)`が1か所、mask identifier / sampleが0であることを確認する。RenderDocは同一composite drawのScene texture 1、sampler 1を確認する。
- P02のbillboard / shadowの効果を先取りしない。`SoulShadowProxy3d`がactiveな間はshadow関連uniform / offsetをP01で物理削除せず、P02 / P08のowner境界を守る。

### 4.4 evidence / schema compatibility contract

P00のprojection v1はP01以降もmaskの**不在を測るため**にfield名を維持する。productionの型・query・asset名と、migration artifactのhistorical field名を混同しない。

| artifact | P00 current reference | P01 subject | 互換規則 |
| --- | --- | --- | --- |
| `summary.csv` | v11 | v11のまま | Soul mask専用ではないため不要なbumpをしない |
| `scene_roots.csv` | 現行header | legacy `soul_mask_proxy_3d`を明示`0` | headerを黙って変えない。component queryではなくstage topologyからzero evidenceを出す |
| `render_inventory.csv` | schema v1 | schema v1、mask target / proxyは`0` | target / camera / actor数はCSVとprojectionで一致させる |
| `window.csv` | current schema v2 historical reader | p01 schema v3、`mask_target_present=false`とnullable / 空のmask寸法 | Scene寸法をmask寸法として書かない。current v2はreferenceとしてのみ受理 |
| RenderDoc checkpoint / extraction | current schema v2、2 resource shape | p01 schema v3、Scene 1 resource / texture / sampler、mask resource absent | current v2をhistorical readerで再検証し、p01にv2、currentにp01-only v3 shapeを混ぜれば失格 |
| `rtt_light_migration.csv` | schema v1 | schema v1のまま | `mask_target_count` / `soul_mask_proxy_3d`等をP01では意味上の`0`として出し、contract SHAを変えない |

RenderDoc schema v3は`stage_id`を持ち、P01ではmask target labelをnullable、composite binding collectionを可変長の1要素とする。code-side stage tableはP01 medium / gpuのexact inventory `1 / 0 / 1 / 3 / 2 / 200 / 0 / 200 / 12`（Scene target、mask target、3D RtT camera、2D camera、`LAYER_2D` pass、Soul、mask Soul、shadow Soul、Familiar）を唯一の正本にする。frozen `rtt_light_migration_v1.json`へresource shapeを追記しない。

native helper、extractor、bundle validatorのhashはP00 current referenceとP01 subjectで異なり得る。その差はprovenanceとして記録するが、cross-stage比較の等値条件にはしない。candidate側の新readerがP00 current raw attemptをlegacy modeで再検証し、同一contract / fixture / matrix / host / window / adapter条件を比較する。

## 5. マイルストーン

## M1: Scene-only runtime / compositeを成立させる

### 変更内容

1. `RttRuntime`から`soul_mask`、`soul_mask_render_target()`、同時recreateを削除する。
2. `RttCompositeMaterial`からmask binding / radius / featherとmask shader branchを削除し、Scene binding `1 / 2`を維持する。
3. resize / DPI / quality rebind queryをScene Camera 1台へ縮小する。
4. shaderをScene 1-sampleへ変更する。
5. startup inventory testで残るScene cameraのorder / layer / clear / target、mask camera 0、cameraとmaterialのrebind一致を固定する。

### 主な変更ファイル

- `crates/bevy_app/src/plugins/startup/{mod.rs,rtt_setup.rs,rtt_composite.rs,startup_systems.rs}`
- `crates/bevy_app/src/systems/visual/{camera_sync.rs,terrain_lod.rs}`
- `assets/shaders/rtt_composite_material.wgsl`

### 完了条件

- [x] active world color handleはScene 1つだけ
- [x] resize / DPI / quality変更後、Scene Cameraとcompositeが同じ新handleを参照し、旧handleを参照しない
- [x] shaderにmask binding / identifier / loopがなく、Scene sampleは1回
- [x] Wall / Terrain receiverとdirectional Scene routeがP01前と同じ

## M2: mask camera / proxy / materialとvisual_testを同時に撤去する

### 変更内容

1. `Camera3dSoulMaskRtt`のspawn、export、query arm、sync、visibility toggleを削除する。
2. Soul spawnからmask SceneRootを削除する。
3. mask GLB ready observer、sync、owner cache register / cleanup、save presentation clear / rehydrateからmask shellを削除する。
4. `SoulMaskMaterial`のMaterialPlugin、handle、Rust module、WGSLを削除する。
5. `LAYER_3D_SOUL_MASK`を削除する。
6. `terrain_lod`のmask-camera exclusionだけを除去する。`SceneObjectQuery`と`apply_render3d_visibility_system`自体はBuilding / visible Soul / shadow / Familiar / main RtT / compositeを維持するため削除せず、`With<SoulMaskProxy3d>` armとmask camera loopだけを除く。
7. `visual_test`をScene-only化する。独自`VisualTestRttRuntime.soul_mask`、`Camera3dSoulMaskTest`、local composite binding `3 / 4`、`SoulMaskConfig` / material / ready / sync、resize assertionを同じcommitで除去し、Scene targetだけのresize / rebind assertionを追加する。
8. visible Soul GLBとSoul shadow pathはP01前と同じ各1系統を維持する。mask proxyだけを0にする。

### 主な変更ファイル

- `crates/bevy_app/src/entities/damned_soul/spawn.rs`
- `crates/bevy_app/src/systems/visual/{character_proxy_3d.rs,character_proxy_3d/{cache.rs,gltf_ready.rs,sync.rs},camera_sync.rs,terrain_lod.rs}`
- `crates/bevy_app/src/systems/save/{rehydrate.rs,rehydrate/{presentation.rs,tests/presentation.rs}}`
- `crates/bevy_app/src/plugins/{visual.rs,startup/{mod.rs,visual_handles.rs,startup_systems.rs}}`
- `crates/hw_visual/src/{lib.rs,visual3d.rs,material/{mod.rs,soul_mask_material.rs}}`
- `crates/hw_core/src/constants/render.rs`
- `crates/visual_test/src/{main.rs,input.rs,soul.rs,systems.rs,setup/scene.rs,types/render.rs}`
- `assets/shaders/soul_mask_material.wgsl`

### 完了条件

- [x] production / runtime inventoryの`SoulMask`、`soul_mask`、`LAYER_3D_SOUL_MASK`参照は0
- [x] P00 contract、projection、historical reader、fixture / negative testに限定したlegacy metric名は許可され、P01値はすべて0
- [x] Soul spawn / load / despawnでmask entityを生成しない
- [x] visual_testがScene-onlyで起動し、resize後もtarget / material relationが正しい
- [x] visible Soul GLB / shadow各1、mask proxy 0、Wall receiver route維持

## M3: P01 stage tooling / metric / native contractを閉じる

### 変更内容

1. `RenderPerfToggles.soul_mask_enabled`、`HW_DISABLE_SOUL_MASK`、DevPanelのMask button / label / action、test presetを削除する。
2. `PerfChecksumQueries`からdeleted component queryを外す。ただし`PerfSceneRootCounts`、`render_inventory.csv`、P00 projectionにはcontract-backed zero evidenceを残し、P01のmask proxy値を黙って欠落 / 読み替えにしない。
3. `PerfRttLightSelection` / config、`policy.py`、`fixtures.py`、`artifacts.py`のstage allowlist、behavior timeline validatorを`p01`対応にする。current reader / fixtureはcurrent referenceとして維持し、stage + schema許可表にない組合せを拒否する。
4. RenderDoc runtime capture、`renderdoc_extract.py`、`rtt_light_bundle.py`をstage-awareにする。GPU-ready条件、checkpoint、replay resource topology、pass / attachment / binding / sample抽出をP01の1 Scene targetへ切り替え、current v2とP01 v3のpositive / negative fixtureを両方self-testする。
5. bundleのP01 gate extractorを実装する。`RLV1-P01-RTT`のtarget / camera / pass / binding / sample / proxy / explicit color bytesと、`RLV1-P01-PERF`のcurrent referenceに対するp95 / p99 / RSS / large peak-live deltaをgate CSVへ出力し、unknown metric・reference locator不在・schema混在を失格にする。
6. native acceptanceの`native_acceptance.py`を`--stage p01`化する。formal leg、output generation、gate selection、current reference読み込みをstageから選び、currentとの後方互換とself-testを追加する。正本Skillとadapter mirrorの`rtt-light`手順もP01を選択できるよう同期する。
7. `window.csv`はmask target absentを明示し、P01 raw recordでScene targetの寸法をcopyしない。summary schema v11は変更しない。
8. `docs/world_layout.md`、`docs/performance-profiling.md`、`docs/rendering-performance.md`、`docs/visual_test.md`を実装後のScene-only contractへ更新する。historical proposalは削除対象にせず、現行仕様との混同だけを防ぐ。

### 主な変更ファイル

- `crates/bevy_app/src/{lib.rs,interface/ui/dev_panel/,plugins/interface.rs}`
- `crates/bevy_app/src/plugins/startup/perf_scenario/{config.rs,output.rs,audit_checksum.rs,renderdoc_capture.rs}`
- `crates/bevy_app/src/plugins/startup/perf_scenario.rs`
- `scripts/perf_tool/{model.py,artifacts.py,fixtures.py,policy.py,rtt_light_bundle.py,renderdoc_extract.py}`と各self-test / negative fixture
- `.codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py`
- `.cursor/skills/hell-workers-run-native-acceptance/SKILL.md`、各adapter mirror
- `docs/{world_layout.md,performance-profiling.md,rendering-performance.md,visual_test.md}`

### 完了条件

- [x] env / UI / testから存在しないmask機能を選べない
- [x] current raw evidenceはhistorical readerで再検証でき、P01 subjectはP01 schemaだけを受理する
- [x] `rtt_light_migration.csv` v1とfrozen contract SHAは不変で、P01のmask-related migration fieldsは明示`0`
- [x] P01 RenderDocはScene label / texture / sampler各1、mask target / camera / pass / binding / sample / proxy各0をfail-closedに検証する
- [x] native launcherが`--stage p01`のformal 5 legを組み立て、P01 gate CSVとcurrent reference比較を生成する

## M4: formal / native受入を閉じる

### 変更内容

1. native acceptance Skillのstage-aware `rtt-light` recipeから、P00と同じcontract / fixture / host / window / adapter matrixで`stage=p01`を採取する。
2. auditはsmall / medium / large × cpu、behaviorはsmall / cpuの`door-state-v1` / `load-normal-v1`、CaptureとMemoryは全size × cpu / gpuを各3反復する。RenderDocはmedium / gpuの固定1 frameを採取する。`field-core` / `consumer-core`はP01では実行・出力ともに拒否する。
3. `RLV1-BUNDLE-VALID`、`RLV1-P01-RTT`、`RLV1-P01-PERF`の全expected rowをcurrent referenceに対して検証する。
4. High / Medium / Low、DPI 1.0 / 1.5 / 2.0、pan / zoom / resizeで、透明clear上のblack frameなし、Scene Camera / composite handle一致、visible Soul GLB + shadow各1、mask 0を確認する。
5. Help impact reviewを実際のDevPanel / player-visible経路から実施し、必要なHelp更新または理由付きNo impact判断を同じ変更batchで閉じる。

### 完了条件

- [x] Scene以外の画面解像度依存world color targetがない
- [x] `RLV1-BUNDLE-VALID`、`RLV1-P01-RTT`、`RLV1-P01-PERF`が合格
- [x] RenderDocでScene texture / sampler 1、mask pass / attachment / binding / sample 0
- [x] black frame、stale handle、resizeずれ、Wall / Terrain receiver脱落がない
- [x] formal artifactとcurrent referenceがbaseline index / hashから再検証できる

## 6. リスクと対策

| リスク | 対策 |
| --- | --- |
| P00 formal currentがないままP01を始める | P00 C00-D / C00-Eと登録済みbaseline indexをhard entry gateにする |
| type参照を消してP01 gateの証拠まで消す | active symbolとprojection legacy fieldを分離し、stage=`p01`では明示0を必須にする |
| frozen contractをP01 resource shapeで上書きする | RenderDoc schema / code-side stage tableをversion化し、`rtt-light-v1` JSON / SHA / projection v1は変更しない |
| P01 candidateがcurrent-only raw artifactを読み替える | stage + schema許可表、historical current reader、cross-stage mismatchのnegative testを持つ |
| RenderDocが2 target前提のまま | runtime / replay / extractor / bundleを1 Scene target stage tableへ同時更新する |
| `SceneObjectQuery`等をまとめて削除してmain routeを壊す | mask arm / mask camera loopだけを外し、remaining consumerのinventory testを保つ |
| visual_testを後回しにして型削除でcompile不能 | M2と同じatomic removal seriesでScene-only化する |
| Scene targetが消えたmask寸法を偽装する | `window.csv`にtarget present stateとnullable / empty mask dimensionsを明示する |
| P01 tooling変更でP00 referenceが読めない | 新readerでP00 current v2を再検証し、tool hash差はprovenanceに留める |
| Soul maskと室内Light Fieldのmaskを混同する | `SoulMask*`だけを対象にし、P03 / P06の`indoor_mask*`契約を検索除外・docsで区別する |

## 7. 検証計画

- RtT runtime / rebind / camera inventory / WGSL single-sample focused tests。
- character proxy lifecycle / rehydrate / cache cleanup / visual_test resize tests。
- stage-aware perf selector、raw schema reader、behavior timeline、window absent-target、RenderDoc runtime / replay / bundle / gate extractionのpositive / negative self-test。
- `cargo check --workspace`、default testに加えprofiling / profiling-memory feature構成のcheck / focused test。
- `cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`python3 scripts/perf.py self-test`、`python3 scripts/dev.py verify`。
- native audit / behavior / Capture / Memory / RenderDoc acceptance（`hell-workers-run-native-acceptance` Skill必須）。
- `git diff --check`、Help impact review、影響docs再読。

## 8. ロールバック方針

- P01 removal seriesを同じcommit列でrevertし、dummy mask、half-enabled toggle、current / p01 schemaの曖昧なreaderを残さない。
- P01後のSoul輪郭品質が不足してもmask RtTを即時復活させず、P02 billboardのalpha silhouetteで評価する。
- formal性能未達時はattachment、composite、proxyのどの差分かをP00 current / P01 RenderDoc topologyとgate row単位で切り分ける。gate、fixture、contract v1を結果に合わせて緩めない。

## 9. AI引継ぎメモ

### 現在地

- 進捗: `100%`（M1〜M4 complete）
- 完了済み: Scene-only runtime / composite、mask camera / proxy / material / toggle削除、save / visual_test追従、window schema v3、current / P01 RenderDoc topology、P01 RTT / performance gate抽出、stage-aware native launcher、P00 canonical再検証、P01 formal / native受入とbaseline登録
- 進行中: なし
- 未着手: なし

### 次のAIが最初にやること

1. 後続の[P02](02-topdown-presentation-plan-2026-08-03.md)を、登録済みP01 attemptをentry evidenceとして開始する。
2. P02ではP01のScene-only topologyとfrozen `rtt-light-v1` contractを変更せず、Door実経路とTopDown presentationを実装する。

### ブロッカー/注意点

- P00 formalの正本はattempt `9e813f24-0f7b-47f5-8a8d-e3ff34775370`である。diagnostic RD0、失敗attempt、dirty treeやheadless smokeをreferenceに昇格させない。
- P01 formalの正本はsubject `29a4a719e9fe92b10618f36ce548c4bb5a4c7e80`、attempt `8bc82f04-10ac-4903-89b6-89011dacdada`である。source fingerprintは`27d3d59b39a83be5d61df09c3f07c70e26cab2f16bbfb1136acca8d7412c5fbc`である。
- raw perf output、RenderDoc capture / bundle、native launcherはP01 topologyへ追従済み。削除済みのactive mask symbolを復活させない。
- P02までvisible Soul GLB / shadowは保持する。P03以降の室内Light Field maskとは別物である。

### 最終確認ログ

- plan review: `2026-08-05` / P00 contract、source inventory、formal tooling、visual_test、native launcherとの整合を再確認
- Rust gates: `2026-08-05` / `not run (plan-only update)`
- P00 prerequisite: `2026-08-11` / current formal attempt登録、native / registry verifier pass
- M1〜M3 gates: `2026-08-11` / workspace check、Clippy、focused Rust tests、perf / RenderDoc / native self-test pass。P00 canonical attemptをnew readerで再検証 pass
- Help impact: `2026-08-11` / No impact（内部Soul mask RtTとdeveloper-only toggleの削除。player操作・Help workflow・成立条件・結果・labelは不変）
- native acceptance: `2026-08-12` / Intel Arc・Vulkan・X11。S0 / S1 pass、formal 5 leg・18 case valid。attempt `8bc82f04-10ac-4903-89b6-89011dacdada`を登録し、`verify-rtt-light` pass
- P01 gates: `2026-08-12` / 123 / 123 row pass（うち`RLV1-P01-RTT` 9 row、`RLV1-P01-PERF` 20 row）。RenderDocは14 pass、163 draw、Scene binding `(1, 2)`各1、mask topology 0
- docs gate: `2026-08-12` / `pass (docs --write / --check, check_docs, diff --check)`

### Definition of Done

- [x] P00 current formal baselineが登録済み
- [x] M1〜M4が完了
- [x] active mask inventoryが0、migration zero evidenceがP01 stageで存在
- [x] `RLV1-BUNDLE-VALID` / `RLV1-P01-RTT` / `RLV1-P01-PERF`が合格
- [x] current / P01 schema readersとgate extractionのself-testが合格
- [x] Help impact review完了
- [x] 影響docs更新済み

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-08-12` | `Codex` | P01 formal attemptを登録・再検証し、Scene-only topology、性能gate、Help impact、native acceptanceを閉じてM1〜M4を完了 |
| `2026-08-11` | `Codex` | P00 current canonical attemptの登録・再検証完了を受けてblockedを解除し、P01を着手可能へ更新 |
| `2026-08-05` | `Codex` | P00 formal baseline未登録をentry blocker化し、frozen projectionのzero evidence、stage-aware RenderDoc / artifact / native launcher、visual_testのatomic移行、P01 gate extractionを具体化 |
| `2026-08-04` | `Codex` | P00のstable RtT / performance gateと共通validity bundle参照へ同期 |
| `2026-08-03` | `Codex` | 統合計画M1をruntime / proxy / tooling / native gateへ具体化 |
