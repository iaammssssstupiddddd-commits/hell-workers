# P02: TopDown表示分類・Soul depth billboard移行計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `single-scene-light-field-02-topdown-presentation-plan-2026-08-03` |
| ステータス | `Completed` |
| 作成日 | `2026-08-03` |
| 最終更新日 | `2026-08-13` |
| 作成者 | `Codex` |
| 親計画 | [`../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md`](../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md) |
| 直接依存 | [P00](00-baseline-gates-plan-2026-08-03.md)、[P01](01-single-scene-rtt-plan-2026-08-03.md) |
| 受入基盤 | [P02-A](02a-p02-acceptance-infrastructure-plan-2026-08-12.md) M1〜M4。P02 M6のformal / native開始条件。 |
| 後続 | [P04](04-indoor-light-runtime-integration-plan-2026-08-03.md)、[P06](06-indoor-light-rendering-plan-2026-08-03.md)、[P08](08-legacy-cleanup-release-plan-2026-08-03.md) |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: 同じ`LAYER_2D`をcomposite前後で二重描画し、構造物の2D / 3D visualも併存している。Camera2d Soulは3D Wallとdepthを共有できない。
- 到達したい状態: world Camera2d passはcomposite後の1回だけ、Building / actorのpresentation classはexhaustive、Soulはunlit alpha-mask billboardとしてScene depthへ参加する。
- 成功指標: 2D / 3D二重表示0、Wall裏Soulのdepth成立、Familiarのforeground維持、V入力からsectionへ入れない。

## 2. スコープ

### 対象（In Scope）

- `MainCamera`をcomposite後の唯一のworld Camera2dへ変更し、`WorldForeground2dCamera`を撤去する。
- production Door root / child構造でauto / manual mutationが成立するcorrectness修復。
- `BuildingType -> RenderPresentationClass`の明示mapping。
- structural 2D mirrorを描画対象から外し、必要なstate mirrorだけを期限付きで保持する。
- Soul visible GLBを共有pool型3D billboardへ置換する。
- Soul shadow GLBのspawn / observer / per-frame projector更新を停止する。
- Familiarの3D proxyを撤去し、Familiar / speech / selection / effectのforeground契約を固定する。
- 3D Door visualを`DoorState`へ同期する。
- `CycleElevation` / V binding / Help entryの削除とTopDown camera sync単純化。
- P02 featureのsemantic sourceを、[P02-A](02a-p02-acceptance-infrastructure-plan-2026-08-12.md)がstage=`p02` artifact / native受入へ接続できる形で固定する。

### 非対象（Out of Scope）

- Door manual intentのrequest化、pause scheduling、Light Field更新順（P04）。P02は`Door + Sprite`同一entity依存の解消まで行う。
- SectionMaterial / section shaderの削除（P08）。P02では`SectionCut::default()`を非activeで維持する。
- Soul projector uniform / WGSL /型の物理削除（P08）。production動作はP02で止める。
- Light Field material（P06）。
- 新規GLB / sprite制作。
- P00 frozen contract / projection v1とregistered current / P01 artifactの変更。P02-AはP02 evidenceを実装するが、これらを読み替えない。
- P04 / P06以降のstage固有metric。P02-AはP02だけを所有し、後続stageは同じextension pointをそれぞれ所有する。

## 3. 現状とギャップ

| 現行 | 問題 | target |
| --- | --- | --- |
| MainCamera order 0が`LAYER_2D`を描画 | overlayに覆われるdrawが発生 | MainCamera order 2 / clear none |
| overlay Camera order 1 | Scene compositeに必要 | 維持 |
| WorldForeground2dCamera order 2が`LAYER_2D`を再描画 | 二重pass、camera syncが必要 | entity / sync system削除 |
| production Doorはrootに`Door`、childに`Sprite` | auto / manual queryの`Door + Sprite`同居条件から外れる | Door domain mutationとvisual consumerを分離 |
| `spawn_completed_building`が2D child後にほぼ全種類の3D proxyをspawn | presentation分類が暗黙、二重表示 | exhaustive mappingから片方だけ表示 |
| Bridgeに3D handle / spawnがない | Structural3dへ分類だけすると不可視 | 3D handle / spawn / rehydrateを追加 |
| Building 3D proxyがowner移動へ追従しない | Tank / MudMixer移動後に旧位置へ残る | general owner transform sync |
| Tank / MudMixer状態は2D Spriteだけが表示 | 2D非描画化で状態が消える | finite shared 3D state material |
| Familiarは2D child + 3D proxy | 二重表示 | 2D foreground 1系統 |
| Soul visible / mask / shadow GLB | proxyとmaterialが複数、shadow更新は毎Visual frame | visible billboard 1系統、shadow動作0 |
| `ElevationViewState`と`CycleElevation` | TopDown-only方針と不一致 | dynamic elevation state削除 |

## 4. 実装方針

### 4.1 presentation mapping

root adapterに副作用のない`presentation_class(BuildingType)`を置き、全variantを明示matchする。

| class | BuildingType |
| --- | --- |
| `Structural3d` | Wall、Door、Floor、Bridge、Tank、MudMixer、RestArea、SoulSpa |
| `Foreground2d` | SandPile、BonePile、WheelbarrowParking、OutdoorLamp |

- `BuildingCategory`や`blocks_movement()`をmappingに使わない。
- `Structural3d`は3D proxyだけを描画する。
- `Foreground2d`は2D visualだけを描画し、3D equipment cubeを生成しない。
- BridgeはP02でmesh / material handle、spawn、rehydrate、cleanupを実装してから`Structural3d`へ切り替える。mappingだけ先に有効化しない。
- all-building mappingは一般completionだけでなくSoulSpa placement、wall construction phase、floor completion、interface debug、初期`WheelbarrowParking`配置、rehydrate、P00 performance fixtureを同じhelper / route tableへ通す。deconstruction / world replacement後はownerを持つactive presentationが0になることも同じtableで検証する。
- `Building3dVisual { owner }`はAdded visual / `Changed<Transform>` ownerを読むが、root transformをそのまま複製しない。2D XY→3D XZ、種別height、rotation、completion bounce scaleを分けたpresentation-transform resolverで、移動するTank / MudMixerを含め追従する。
- Tank / MudMixerのempty / partial / full / active状態とcompletion bounceはP02がsemantic state / active presentation契約として所有する。P02で必要な有限shared handleは暫定bridgeに留め、durable `TopDownStructuralMaterial` / receiverへの移行はP06 M3が所有する。`state_and_bounce_probes_pass`はmaterial type名に依存させない。
- P00契約どおりcompletion bounceをstage上のactive presentationへ移し、廃止または非描画2D childだけがbounceする状態を残さない。
- Wall / Tank / MudMixer等の既存2D state syncが必要な間は`LegacyStructural2dMirror`相当を非描画で保持できるが、consumer名とP08削除条件をtestに記録する。単なる保険として残さない。
- `WallOrientationAid`はP02で削除 / debug-only化 / `Structural3d`構成要素として維持のいずれかを選ぶ。維持する場合はscene inventoryとP06 receiver ownershipへ明示的に含め、無名の3D childとして残さない。
- load rehydrateも通常spawnと同じmapping helperを使い、別matchを持たない。

| presentation route | P02で固定する契約 | focused evidence |
| --- | --- | --- |
| generic completed blueprint | `BuildingType` mapping helperだけがactive presentationを作る | 全12 variantのnormal spawn |
| wall phase / floor completion / SoulSpa placement | 独自spawnはhelperへ委譲し、route固有の2D / 3D bypassを持たない | completed stateとload後のexactly-one |
| initial `WheelbarrowParking` | root Sprite直spawnをForeground2d helperへ正規化する | new gameとrehydrateの同一分類 |
| interface debug | production mappingを迂回する場合はdebug-only markerを持ち、formal fixtureへ混入しない | debug-on / debug-off inventory |
| rehydrate | normal spawnと同じclassification / transform resolver / cleanupを使う | load-normal fixture |
| P00 performance fixture | stage-aware sidecarのpresentation expectationと同じroute tableを読む | medium / gpu checkpoint |
| deconstruction / world replacement | owner消滅後のactive presentationは0、cache / observerを残さない | teardown / replace focused test |

### 4.2 camera composition

```text
Camera3dRtt order -1 -> Scene Image
Overlay Camera2d order 1 -> Scene composite / window clear
MainCamera order 2, clear none -> LAYER_2D foreground
UI -> final
```

- `MainCamera`はPanCamera、world cursor、selectionの正本を維持する。
- `sync_world_foreground_2d_camera_system`と`WorldForeground2dCamera`を削除する。
- TopDown `sync_camera3d_system`は2D XY / scaleから3D XZ / orthographic scaleへの単一変換だけを行う。
- MainCameraを無効化するelevation branchを削除する。

### 4.3 Soul billboard

- `ActorBillboard3d` owner shellをSoulごとに1entity生成する。
- geometryは共有quad `Handle<Mesh>`、materialはatlas frameごとの有限poolを共有する。
- 現assetは単一atlasではないため、初期実装は「状態 / frameごとの有限共有material pool」を採用する。runtime atlas生成は行わず、後にasset packagingする場合は別commitとする。
- `movement/animation.rs::select_soul_image`と3D側`desired_body_state` / `desired_face_state`をpureな`SoulBillboardFrame` resolver 1つへ統合する。
- animationはactorごとのmaterial cloneではなく、frame変更時にpool handleを差し替える。
- alphaは`AlphaMode::Mask`相当、depth writeあり、unlit、shadow caster / receiverなしとする。
- positionは既存2D→3D変換`(x, anchor_y, -y)`を1helperへ集約し、quad中心ではなく足元がgroundへ合うanchor offsetを持つ。
- fixed TopDown cameraに正対する回転、足元anchor、scale、face directionをpool / transformで表現する。
- soft glowが必要ならCamera2d foregroundの共有effectへ分離し、depth silhouetteへblendを使わない。
- billboard PoCが不合格ならvisible GLB 1系統へ戻し、P01のmaskを戻さない。
- visible GLBは開発中の一時rollbackに限る。billboardのalpha / depth gateが未成立ならP08 releaseを完了扱いにしない。
- named presentation setで`conversation / timer / domain state resolver -> billboard frame / handle sync -> cache cleanup`の順を固定する。
- bodyだけがWall depthへ隠れ、selection indicator / speech / effectはforegroundで読めることをnative contractにする。
- billboard切替と同じwork packageでSoul shadow proxy spawn / ready observer / cache / rehydrate / per-frame projector system registrationを停止する。P08へ残すのはdead uniform / WGSL / typeの物理削除だけとする。

### 4.4 Door domain / 3D visual

- `hw_world::apply_door_state`をDoor rootとWorldMapだけのdomain mutationにし、Sprite componentを成立条件にしない。
- auto proximity / manual intentの既存consumerをproduction root + child構造のintegration testで検証する。
- generic `Building3dVisual`だけで識別せず`Door3dVisual { owner }`相当を付ける。既存cleanupがこのmarkerを回収できないなら共通presentation tagへ統合する。
- Closed / Lockedはclosed visual、Openは明示したyaw axis・pivot・orientationを持つopen visualを使う。正方形primitiveの中心90度回転だけで状態差を表さず、非対称leafまたはstate別mesh / materialでClosedとOpenを識別可能にする。orientationをwall topologyから得る場合は、topology不在時のdeterministic fallbackも定義する。
- child Spriteと3D visualは`Changed<Door>`とAdded shellを読み、owner不在ならcleanupへ委ねる。Door mutation→`DoorPresentationSyncSet`相当のconsumer→behavior observationの順を固定し、snapshotをmutation直後のpre-visual状態から採らない。P04のmanual request化はこのconsumer / observation境界を変更しない。
- Door visualはstateを書かず、`DoorState`の純粋consumerとする。
- P04後はLight Fieldと同じDoorState revisionへ遷移する。

### 4.5 TopDown-only移行境界

- `InputAction::CycleElevation`、plain V binding、consumer owner、input tests、Help entryを削除する。
- Ctrl+V等のarea-edit chordと`KeyCode::KeyV` labelは別契約なので削除しない。
- `ElevationViewState` / `ElevationDirection`をcamera sync / terrain LODから外し、LOD resolverはTopDownだけを受ける。
- `SectionCut`はP08までdefault inactiveで保持し、dynamic producer `sync_section_cut_normal_system`を削除する。
- Help manifest / provider / exhaustive coverage / exact approval snapshotを同じ変更で更新する。この変更のHelp impact判断は`Update required`である。

## 5. マイルストーン

## M1: production Door経路を修復する

### 変更内容

1. Door state mutationをrootの`Door + WorldMap`だけへ分離する。
2. auto open / closeとmanual lockのqueryから同一entity上の`Sprite`要件を除く。
3. child Spriteと`Door3dVisual`をowner state consumerへ変更し、Closed / Open / Lockedを識別可能なvisual、orientation / pivot、owner-safe cleanupを実装する。
4. Door mutation→`DoorPresentationSyncSet`相当のconsumer→behavior observationのorderを明示する。manual direct mutationがP04でrequestへ移る前後で、同じ観測境界を維持する。
5. `attach_building_shell` / completionから生成したDoorを使うintegration test、despawn / world replacement test、P02-A M2のproduction-shell behavior sourceを追加する。

### 主な変更ファイル

- `crates/hw_world/src/door_systems.rs`
- `crates/hw_spatial/src/door_proximity.rs`
- `crates/bevy_app/src/interface/ui/interaction/{intent_context.rs,intent_handler.rs}`
- `crates/bevy_app/src/systems/jobs/building_completion/spawn.rs`
- `crates/bevy_app/src/systems/visual/building3d_cleanup.rs`
- `crates/bevy_app/src/plugins/logic.rs`
- `crates/bevy_app/src/plugins/startup/perf_scenario/{workload_driver.rs,indoor_light_fixture.rs,behavior_driver.rs}`
- P02-A M2のDoor evidence / focused test owner

### 完了条件

- [x] root Door / child Spriteでauto open / closeが成立する
- [x] manual mutationがSprite有無へ依存しない
- [x] synthetic `Door + Sprite`だけのtestで完了判定していない
- [x] active child Spriteまたは3D visualが同じDoorStateを表示し、Open / Closedがactual-windowで識別できる
- [x] auto / manual / pausedのbehavior snapshotはDoor presentation consumer後に採られる
- [x] owner消滅後にDoor visual / cache / observerが残らない
- [x] P02-A M2が要求する`RLV1-P02-DOOR-DOMAIN`のsource-to-evidenceを満たす。P02全体未完了時のpartial artifactをformal合格と呼ばない

### 検証

- production root / child Doorのauto、manual、paused、despawn / world replacement focused tests
- `python3 scripts/dev.py check`
- `python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`
- `python3 scripts/dev.py cargo -- test --workspace`

## M2: camera passとTopDown-only契約を閉じる

### 変更内容

1. MainCameraをorder 2 / clear noneへ変更する。
2. WorldForeground2dCamera spawn / marker / sync / queryを削除する。
3. startup inventory testでCamera3dRtT=1、overlay order 1=1、Main order 2=1、WorldForeground=0を固定する。
4. elevation input / state / branch / dynamic SectionCut producerを削除する。
5. Terrain LODをTopDown resolverへ単純化する。
6. Help manifest / provider / coverage / exact approvalからplain V elevationを削除し、Ctrl+Vを維持する。
7. P02-A M2が同じmedium / gpu checkpointへ束縛するcamera / pass evidenceを提供する。

### 主な変更ファイル

- `crates/bevy_app/src/plugins/startup/startup_systems.rs`
- `crates/bevy_app/src/systems/visual/{camera_sync.rs,elevation_view.rs,section_cut.rs,terrain_lod.rs}`
- `crates/bevy_app/src/input_actions/{model.rs,mod.rs,bindings.rs,tests.rs}`
- `crates/bevy_app/src/plugins/visual.rs`
- `crates/bevy_app/src/interface/ui/help_content/{providers/camera_selection.rs,coverage.rs,coverage_approval.snap}`
- root Help manifest / approval gate
- `docs/{help-screen.md,visual_test.md,rendering-performance.md,architecture.md,world_layout.md}`

### 完了条件

- [x] world `LAYER_2D` cameraがMainCamera 1台だけ
- [x] Render3d非表示でもoverlay clear + foregroundが正常
- [x] UI camera、pan / zoom / cursor conversionが回帰しない
- [x] plain V action / binding / Helpが0、Ctrl+Vは維持
- [x] `SectionCut`はdefault inactive以外のwriterを持たない
- [x] P02-A M2がcamera / pass evidenceをP02 presentation sidecarへ記録できる

### 検証

- camera order / clear / cursor conversion / Render3d hidden focused tests
- Help manifest / provider / exhaustive coverage / exact approval snapshot test
- `python3 scripts/dev.py check`
- `python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`
- `python3 scripts/dev.py cargo -- test --workspace`

## M3: Building presentation mappingを導入する

### 変更内容

1. class enumと全12 `BuildingType`のexhaustive mapping helperを追加する。
2. completion、rehydrate、SoulSpa placement、wall phase、floor completion、debug spawn、初期`WheelbarrowParking`配置、P00 performance fixtureを§4.1のroute tableに従って同じhelperへ接続する。
3. Bridge用3D handles / spawn / rehydrate / cleanupを追加してからStructural3dへ切り替える。
4. Structural3dの描画2D child、Foreground2dの3D proxyを生成しない。
5. XY→XZ、type height、rotation、bounce scaleを分離したgeneral 3D presentation-transform resolverを追加し、Tank / MudMixer moveとloadをtestする。
6. Tank / MudMixer stateと必要なcompletion bounceをactive presentationへ移す。P06 M3へ渡すstate / bounce contractとtemporary material bridgeを明記する。
7. Wall connection / blueprint consumerを分離し、`WallOrientationAid`の最終扱いと必要なlegacy mirrorだけをmarker付きで期限設定する。

### 主な変更ファイル

- `crates/bevy_app/src/systems/jobs/building_completion/{spawn.rs,mod.rs}`
- `crates/bevy_app/src/systems/save/rehydrate/`のpresentation shell adapter
- `crates/bevy_app/src/systems/visual/building3d_cleanup.rs`
- `crates/bevy_app/src/interface/selection/soul_spa_place/spawn.rs`
- `crates/bevy_app/src/systems/logistics/initial_spawn/facilities.rs`
- `crates/bevy_app/src/plugins/startup/perf_scenario/indoor_light_fixture.rs`
- wall construction / floor completion / interface debugの独自spawn owner
- `crates/hw_visual/src/{visual3d.rs,layer/,wall_connection.rs,tank.rs,mud_mixer.rs}`

### 完了条件

- [x] §4.1の全routeで全BuildingTypeのactive presentationがexactly one、owner消滅後は0
- [x] Bridgeが不可視にならない
- [x] Tank / MudMixer move後に座標、height、rotation、bounce scaleを正しく保って3D proxyが追従する
- [x] Tank / MudMixerの状態表示が維持される
- [x] legacy mirrorがある場合、全consumerとP08削除gateが列挙されている
- [x] P00 fixture / P02-A sidecarの期待分類がproduction mappingと同じroute tableを使う

### 検証

- 全BuildingTypeのnormal spawn / load / initial spawn / teardown mapping tests
- Bridge、moving Tank / MudMixer、state / bounce、WallOrientationAid focused tests
- `python3 scripts/dev.py check`
- `python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`
- `python3 scripts/dev.py cargo -- test --workspace`

## M4: Soul billboard・Familiar前景・Soul shadow停止を閉じる

### 変更内容

1. shared quadと状態 / frameごとの有限共有material poolを追加する。
2. pure `SoulBillboardFrame` resolverへ既存2系統のanimation state判定を統合する。
3. Soul spawn / rehydrateを足元anchor付き`ActorBillboard3d`へ切り替える。
4. named presentation setでstate resolver→frame sync→cleanupの順を固定する。
5. visible GLB ready observerとper-Soul face material cloneをproductionから外す。
6. FamiliarProxy3dのspawn / sync / cache / rehydrate / reset / perf列を削除し、2D child 1つだけにする。
7. SoulShadowProxy3dのspawn / observer / cache / rehydrateとprojector sync登録を停止する。
8. `SceneObjectQuery` / Render3d visibility pathをbillboardを含むP02 scene objectへ更新し、visible / hiddenで同じowner lifecycleを保つ。
9. alpha / depth / Wall前後 / selection / speech / effect / animationのproduction actual-window fixtureをP02-A M3へ渡す。独立`visual_test`は補助検査に限定する。

### 主な変更ファイル

- `crates/hw_visual/src/material/`のbillboard material
- `crates/hw_visual/src/visual3d.rs`
- `crates/bevy_app/src/entities/{damned_soul,familiar}/spawn.rs`
- `crates/bevy_app/src/systems/visual/{character_proxy_3d,soul_animation.rs}`
- `crates/bevy_app/src/plugins/visual.rs`
- Soul movement / animation resolver owner
- `crates/bevy_app/src/systems/save/rehydrate/`のactor shell
- `crates/visual_test/src/{soul.rs,systems.rs,types/}`
- `assets/shaders/actor_billboard_material.wgsl`
- P02-A M2のpresentation sidecar / focused test owner

### 完了条件

- [x] Wall前 / Wall裏でSoul body depthが正しい
- [x] alpha edgeがWallを貫通せず、足元anchorが地面に合う
- [x] selection / speech / effectはforegroundで読める
- [x] pool cardinalityとspawn / load / despawn後のper-actor material asset増加なしをfocused production testで検証する
- [x] Soul billboard / Familiar Spriteがspawn / load / despawnで各1系統
- [x] Soul / Familiarの3D GLB proxyとSoul projector material writeがproductionで0
- [x] Soul shadow spawn / ready observer / cache / rehydrate / per-frame projector registrationがproductionで0
- [x] Render3d visible / hiddenでbillboardを含むscene objectが一貫してtoggleされ、foreground consumerは残る
- [x] animation / expressionが1つのresolverと明示schedule順を使う

### 検証

- billboard resolver / shared-pool / spawn / rehydrate / despawn / scene-toggle focused tests
- P02-A M3 production actual-window scenarioのWall前後、alpha、foreground、animation observation
- `python3 scripts/dev.py check`
- `python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`
- `python3 scripts/dev.py cargo -- test --workspace`

## M5: P02 evidence / stage toolingを接続する

### 変更内容

1. [P02-A](02a-p02-acceptance-infrastructure-plan-2026-08-12.md) M1〜M4を、P02 M1〜M4のproduction sourceと同じcommit seriesで完了する。
2. `stage=p02` selector、historical current / P01 reader、Door / presentation sidecar、RenderDoc expectation、bundle gate extractor、native recipeを接続する。
3. P00 frozen contract / projection v1を変更せず、P02 Door six metric、presentation seven metric、P01 compatible performance referenceへのsource-to-gate対応とnegative testを閉じる。
4. all-building、Door、Soul、Familiar、camera compositionのproduction fixtureをP02-A actual-window scenarioへ渡す。`visual_test`は補助検査でありformal / nativeの代替にしない。

### 主な変更ファイル

- P02-Aで列挙する`perf_scenario` Rust / Python artifact、fixture、RenderDoc、bundle self-test
- `.codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py`とSkill / adapter mirror
- 実装後の`docs/{performance-profiling.md,rendering-performance.md,visual_test.md}`

### 完了条件

- [x] P02-A M1〜M4が完了し、current / P01 historyを読み替えずP02 raw evidenceを生成できる
- [x] P02の各Door / presentation metricを壊すnegative fixtureが対応gateを落とす
- [x] P02 Scene-only checkpointがP01 RTT preservationとP02 presentation evidenceを同時に満たす
- [x] P02 full formalのcommand、input、artifact validator、failure triageが揃う。actual candidate採取はM6だけが行う

### 検証

- P02-A M1〜M4のselector / reader / fixture / behavior / RenderDoc / bundle / native self-test
- `python3 scripts/dev.py check`
- `python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`
- `python3 scripts/dev.py cargo -- test --workspace`
- `PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py self-test`

## M6: presentation formal / native受入を閉じる

### 変更内容

1. P02-Aがreadyにしたno-prompt recipeで、P00 `stage=p02`のaudit / behavior / Capture / Memoryを各required case 3反復し、medium / gpu RenderDocを固定1 frame採取する。
2. `RLV1-BUNDLE-VALID`、`RLV1-P01-RTT`、`RLV1-P02-DOOR-DOMAIN`、`RLV1-P02-PRESENT`、`RLV1-P02-PERF`だけをexact required gate集合として検証する。
3. High / Medium / Low、DPI 1.0 / 1.5 / 2.0、Render3d visible / hiddenでP02-A actual-window scenarioを実行し、black frame、double draw、Door stateの非識別、Wall前後 / alpha、foreground、animation、invisible Bridgeを判定する。
4. P02-PERFはP01 compatible referenceに対するp95 / p99をhard gateにする。RSSは診断値として記録し、P02の新規閾値にしない。閾値追加はv2 contractと双方rebaselineを別提案で行う。
5. M2のHelp変更を含む実装batchに対し、`hell-workers-review-help-impact` Skillで実際のplayer-visible経路から`Update required`を完了する。

### 完了条件

- [x] black frame / double draw / invisible Bridge / Door state非識別がない
- [x] camera inventory、scene root count、P02 presentation sidecarがtargetに一致する
- [x] `stage=p02`のexact gate ID集合を満たす
- [x] actual-window artifactとformal artifactがfail-closed検証を通る
- [x] Help impact review、影響docs、native evidence registryが同じ完了batchで閉じる

### 検証

- `hell-workers-run-native-acceptance` Skillの`rtt-light` formal recipe（P02 stage）
- P02-A actual-window presentation scenario
- `python3 scripts/dev.py verify`
- `git diff --check`

## 6. リスクと対策

| リスク | 対策 |
| --- | --- |
| MainCamera order変更でwindowがclearされる | overlayをclear owner、MainCameraをclear noneとしてcaptureする |
| structural 2D child削除でWall connection等が壊れる | consumer inventory後にmirrorまたは3D stateへ移し、暗黙削除しない |
| BridgeをStructural3dにして不可視になる | handle / spawn / rehydrateを先に用意しmapping testとnative captureを通す |
| 移動構造物のproxyが旧位置に残る | owner Transformのgeneral syncとmove / load testを追加する |
| billboard blend sortでWallを貫通する | alpha mask + depth writeを必須にする |
| visible GLBとbillboardが同時spawnする | presentation shellを1 helper / 1 owner cacheへ統合する |
| shadow cleanupをP08へ先送りしてcostが残る | spawn / per-frame updateはP02で停止し、dead layout削除だけP08へ残す |
| V削除でCtrl+Vまで消す | chord単位testを維持し、plain Vだけを除去する |
| Open Doorが正方形yawでClosedと見分けられない | leaf / state visual、orientation / pivot、actual-window observationを同じM1で固定する |
| Door visualがwriter後に同期されずbehaviorがstale stateを読む | consumer→observation boundaryを明示し、P02-A behavior snapshotをその後だけに置く |
| current / P01 artifactをP02 schemaで読み替える | P02-Aのstage×schema許可表とhistorical reader / cross-stage negativeでfail-closedにする |
| initial spawn / fixtureがmapping helperを迂回する | §4.1 route tableの全pathをP02-A sidecarとfocused testへ束縛する |
| P02 / P06でTank / MudMixer material ownerが重なる | P02はsemantic state / bounce、P06はdurable receiver materialを所有し、P02 gateはmaterial名へ依存させない |

## 7. 検証計画

- presentation mapping exhaustive tests
- camera order / clear / owner count tests
- billboard spawn / rehydrate / cleanup / pool tests
- production shell Door state mutation / visual sync tests
- Bridge spawn / moving structural transform / Tank / MudMixer state tests
- Familiar exactly-one-presentation / Soul shadow runtime-zero tests
- named billboard resolver / sync order tests
- input binding / Help exact coverage tests
- P02-A selector / schema reader / fixture / behavior / RenderDoc / bundle / native positive・negative self-test
- production actual-window Wall前後 / alpha / selection / animation / Door acceptance
- M1〜M5の各独立commitで`python3 scripts/dev.py check`、Rust変更時は`python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`と`python3 scripts/dev.py cargo -- test --workspace`
- M2ではHelp manifest / provider / exhaustive coverage / exact approval snapshotを再生成し、M6の実装batchでHelp impact reviewを完了する
- M6で`python3 scripts/dev.py verify`、`hell-workers-run-native-acceptance` SkillのP02 formal recipe、artifact fail-closed検証、`git diff --check`

## 8. ロールバック方針

- M1 Door correctnessは照明と独立した不具合修正として保持し、presentation rollbackで`Door + Sprite`同居queryへ戻さない。
- M2 camera変更は独立commitとし、black frame時に限定revertできるようにする。
- billboard不合格時は開発中だけvisible GLB 1系統へ戻せる。Soul mask、per-actor material clone、2D foreground Soulは戻さず、P08 releaseはblockedとする。
- mapping不合格のBuildingTypeだけを暗黙fallbackせず、親表とmapping testを同時変更する。
- V / section再導入は本計画のrollback対象外とする。

## 9. AI引継ぎメモ

### 現在地

- 進捗: `100%`
- 完了済み: M1〜M6とP02-A M1〜M4。subject `6ea0bf99391b1660607537304a3764f380a10eac`でproduction actual-window 18 / 18、S0 / S1、formal Audit / Behavior / Capture / RenderDoc / MemoryをIntel Arc (MTL) / Mesa 26.1.5 / Vulkan / X11上でvalid確認した。formal attempt `54d85a63-e237-4501-a0d0-33c1d0a29f3b`は128 / 128 gate row passで登録し、独立verifierもpassした
- 未完了: なし。後続P04 / P06は本計画のpresentation契約と登録済みP02 stageを入力にする。

### 次のAIが最初にやること

1. P04ではP02 M1のDoor domain / presentation consumer境界を維持したままmanual request化とLight Field transactionを実装する。
2. P06では登録済みP02 stageを性能referenceとしてstructural receiverを導入する。
3. P08で期限付きlegacy mirrorと到達不能section / projector型を撤去する。

### ブロッカー/注意点

- save presentation shellは別作業のregistry変更と重なる可能性がある。
- WallはP02時点でSectionMaterialを使い続ける。
- P04がmanual requestのpause / frame契約を所有し、P02はdomain mutationの成立までを所有する。
- P02 full formalはP02-A M1〜M4がreadyになるまで採取しない。P04のentryはP02 M1 Door correctnessのままとし、P02-A full formalを新規blockerにしない。
- P00 frozen contract / projection v1とregistered current / P01 artifactをP02実装の都合で更新しない。

### 最終確認ログ

- Rust gates: `2026-08-13` / `python3 scripts/dev.py check pass、Clippy 0 warnings。完了docs反映後にverifyを再実行`
- native acceptance: `2026-08-13` / `subject 6ea0bf99、source fingerprint 0f43c3cf…、actual-window 18/18 valid、S0 / S1 valid、formal attempt 54d85a63-e237-4501-a0d0-33c1d0a29f3b valid、128/128 gate row pass、独立verify pass`
- docs gate: `2026-08-13` / `完了docs反映後にdocs --write / checkを再実行`

### Definition of Done

- [x] M1〜M6とP02-A M1〜M4が完了
- [x] production Door silent pathが解消済み
- [x] 全Building / Soul / Familiarがexactly one presentation
- [x] Soul shadow spawn / per-frame projector更新が0
- [x] `stage=p02`のexact gate ID集合が合格
- [x] presentation / camera / billboard native gate合格
- [x] V elevation Help削除済み
- [x] Help impact review完了
- [x] 影響docs更新済み

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-08-13` | `Codex` | subject `6ea0bf99`のactual-window 18 / 18、S0 / S1、formal 5 leg・128 / 128 gate rowと独立verifierをvalid確認し、M1〜M6を完了 |
| `2026-08-12` | `Codex` | 修正後S1をIntel Arc / Vulkan / X11で再実行し、Audit 3/3、Capture 18/18、Memory 18/18 valid・source unchangedを確認 |
| `2026-08-12` | `Codex` | S1再試行のCaptureでmedium / large Open Doorのpresentation settle待ち不足とP02 GPU legacy proxy期待値の旧契約を検出。fixtureを1 frame待機、P02 legacy proxyを0へ修正 |
| `2026-08-12` | `Codex` | S1初回auditでDoor 0のClosed→Openを検出。static laneだけDoor automationを停止し、fixed auditでP02 sidecarを出さないproducer/validator契約へ修正、focused audit 3/3 validを確認 |
| `2026-08-12` | `Codex` | P02 Door / load behaviorを各3回validで確認し、旧2D mirrorをDoor / Tank / MudMixerだけへ限定するproduction/fixture契約を同期 |
| `2026-08-12` | `Codex` | M1〜M5を実装し、M6のclean-subject native / formal採取だけを未完了として現在地へ反映 |
| `2026-08-12` | `Codex` | P02-A受入基盤計画を分離し、Door visual / schedule / lifecycle、全presentation route、P02 stage tooling、actual-window formalをM1〜M6へ具体化 |
| `2026-08-04` | `Codex` | P00のstable presentation / performance gateと共通validity bundle参照へ同期 |
| `2026-08-03` | `Codex` | 統合計画M2をcamera、Building分類、billboard、TopDown-onlyへ具体化 |
