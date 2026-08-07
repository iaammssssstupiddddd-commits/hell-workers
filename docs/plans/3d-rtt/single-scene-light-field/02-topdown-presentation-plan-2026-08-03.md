# P02: TopDown表示分類・Soul depth billboard移行計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `single-scene-light-field-02-topdown-presentation-plan-2026-08-03` |
| ステータス | `Draft` |
| 作成日 | `2026-08-03` |
| 最終更新日 | `2026-08-07` |
| 作成者 | `Codex` |
| 親計画 | [`../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md`](../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md) |
| 直接依存 | [P00](../archived/00-baseline-gates-plan-2026-08-03.md)（registered `rtt-light-v1` current formal baseline）、[P01](01-single-scene-rtt-plan-2026-08-03.md) |
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

### 非対象（Out of Scope）

- Door manual intentのrequest化、pause scheduling、Light Field更新順（P04）。P02は`Door + Sprite`同一entity依存の解消まで行う。
- SectionMaterial / section shaderの削除（P08）。P02では`SectionCut::default()`を非activeで維持する。
- Soul projector uniform / WGSL /型の物理削除（P08）。production動作はP02で止める。
- Light Field material（P06）。
- 新規GLB / sprite制作。

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
- all-building mappingは一般completionだけでなくSoulSpa placement、wall construction phase、floor completion、interface debug等の独自spawn経路も同じhelperへ通す。
- `Building3dVisual { owner }`はAdded visual / `Changed<Transform>` ownerを読み、移動するTank / MudMixerを含めtransformへ追従する。
- Tank / MudMixerのempty / partial / full / active状態は有限個の共有3D material handleへ移す。状態表現を落とす場合は暗黙縮退ではなくP00製品判断として親計画へ記録する。
- P00契約どおりcompletion bounceをstage上のactive presentationへ移し、廃止または非描画2D childだけがbounceする状態を残さない。
- Wall / Tank / MudMixer等の既存2D state syncが必要な間は`LegacyStructural2dMirror`相当を非描画で保持できるが、consumer名とP08削除条件をtestに記録する。単なる保険として残さない。
- load rehydrateも通常spawnと同じmapping helperを使い、別matchを持たない。

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
- generic `Building3dVisual`だけで識別せず`Door3dVisual { owner }`相当を付ける。
- Closed / Lockedはclosed transform、Openは中心pivotで90度回転したopen transformを使う。
- child Spriteと3D visualは`Changed<Door>`とAdded shellを読み、owner不在ならcleanupへ委ねる。
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
3. child Spriteをowner state consumerへ変更し、3D visualも同じownerを読む。
4. `attach_building_shell` / completionから生成したDoorを使うintegration testを追加する。

### 主な変更ファイル

- `crates/hw_world/src/door_systems.rs`
- `crates/hw_spatial/src/door_proximity.rs`
- `crates/bevy_app/src/interface/ui/interaction/{intent_context.rs,intent_handler.rs}`
- `crates/bevy_app/src/systems/jobs/building_completion/spawn.rs`
- `crates/bevy_app/src/systems/visual/building3d_cleanup.rs`
- `crates/bevy_app/src/plugins/logic.rs`

### 完了条件

- [ ] root Door / child Spriteでauto open / closeが成立する
- [ ] manual mutationがSprite有無へ依存しない
- [ ] synthetic `Door + Sprite`だけのtestで完了判定していない
- [ ] child Spriteと3D visualが同じDoorStateを表示する
- [ ] `RLV1-P02-DOOR-DOMAIN`を満たす

## M2: camera passとTopDown-only契約を閉じる

### 変更内容

1. MainCameraをorder 2 / clear noneへ変更する。
2. WorldForeground2dCamera spawn / marker / sync / queryを削除する。
3. startup inventory testでCamera3dRtT=1、overlay order 1=1、Main order 2=1、WorldForeground=0を固定する。
4. elevation input / state / branch / dynamic SectionCut producerを削除する。
5. Terrain LODをTopDown resolverへ単純化する。
6. Help manifest / provider / coverage / exact approvalからplain V elevationを削除し、Ctrl+Vを維持する。

### 主な変更ファイル

- `crates/bevy_app/src/plugins/startup/startup_systems.rs`
- `crates/bevy_app/src/systems/visual/{camera_sync.rs,elevation_view.rs,section_cut.rs,terrain_lod.rs}`
- `crates/bevy_app/src/input_actions/{model.rs,mod.rs,bindings.rs,tests.rs}`
- `crates/bevy_app/src/plugins/visual.rs`
- `crates/bevy_app/src/interface/ui/help_content/{providers/camera_selection.rs,coverage.rs,coverage_approval.snap}`
- root Help manifest / approval gate
- `docs/{help-screen.md,visual_test.md,rendering-performance.md}`

### 完了条件

- [ ] world `LAYER_2D` cameraがMainCamera 1台だけ
- [ ] Render3d非表示でもoverlay clear + foregroundが正常
- [ ] UI camera、pan / zoom / cursor conversionが回帰しない
- [ ] plain V action / binding / Helpが0、Ctrl+Vは維持
- [ ] `SectionCut`はdefault inactive以外のwriterを持たない

## M3: Building presentation mappingを導入する

### 変更内容

1. class enumと全12 `BuildingType`のexhaustive mapping helperを追加する。
2. completion、rehydrate、SoulSpa placement、wall phase、floor completion、debug spawnを同じhelperへ接続する。
3. Bridge用3D handles / spawn / rehydrate / cleanupを追加してからStructural3dへ切り替える。
4. Structural3dの描画2D child、Foreground2dの3D proxyを生成しない。
5. general 3D proxy transform syncを追加し、Tank / MudMixer moveとloadをtestする。
6. Tank / MudMixer stateと必要なcompletion bounceを3D consumerへ移す。
7. Wall connection / blueprint consumerを分離し、必要なlegacy mirrorだけmarker付きで期限設定する。

### 主な変更ファイル

- `crates/bevy_app/src/systems/jobs/building_completion/{spawn.rs,mod.rs}`
- `crates/bevy_app/src/systems/save/rehydrate/`のpresentation shell adapter
- `crates/bevy_app/src/systems/visual/building3d_cleanup.rs`
- `crates/bevy_app/src/interface/selection/soul_spa_place/spawn.rs`
- wall construction / floor completion / interface debugの独自spawn owner
- `crates/hw_visual/src/{visual3d.rs,layer/,wall_connection.rs,tank.rs,mud_mixer.rs}`

### 完了条件

- [ ] 全BuildingTypeの通常spawn / loadでexactly one presentation
- [ ] Bridgeが不可視にならない
- [ ] Tank / MudMixer move後に3D proxyが追従する
- [ ] Tank / MudMixerの状態表示が維持される
- [ ] legacy mirrorがある場合、全consumerとP08削除gateが列挙されている

## M4: Soul billboard・Familiar前景・Soul shadow停止を閉じる

### 変更内容

1. shared quadと状態 / frameごとの有限共有material poolを追加する。
2. pure `SoulBillboardFrame` resolverへ既存2系統のanimation state判定を統合する。
3. Soul spawn / rehydrateを足元anchor付き`ActorBillboard3d`へ切り替える。
4. named presentation setでstate resolver→frame sync→cleanupの順を固定する。
5. visible GLB ready observerとper-Soul face material cloneをproductionから外す。
6. FamiliarProxy3dのspawn / sync / cache / rehydrate / reset / perf列を削除し、2D child 1つだけにする。
7. SoulShadowProxy3dのspawn / observer / cache / rehydrateとprojector sync登録を停止する。
8. alpha / depth / Wall前後 / selection / speech / effect / animationのnative fixtureを追加する。

### 主な変更ファイル

- `crates/hw_visual/src/material/`のbillboard material
- `crates/hw_visual/src/visual3d.rs`
- `crates/bevy_app/src/entities/{damned_soul,familiar}/spawn.rs`
- `crates/bevy_app/src/systems/visual/{character_proxy_3d,soul_animation.rs}`
- Soul movement / animation resolver owner
- `crates/bevy_app/src/systems/save/rehydrate/`のactor shell
- `crates/visual_test/src/{soul.rs,systems.rs,types/}`
- `assets/shaders/actor_billboard_material.wgsl`
- perf scene root Rust / Python schema

### 完了条件

- [ ] Wall前 / Wall裏でSoul body depthが正しい
- [ ] alpha edgeがWallを貫通せず、足元anchorが地面に合う
- [ ] selection / speech / effectはforegroundで読める
- [ ] per-actor material asset増加がない
- [ ] Soul billboard / Familiar Spriteがspawn / load / despawnで各1系統
- [ ] Soul / Familiarの3D GLB proxyとSoul projector material writeがproductionで0
- [ ] animation / expressionが1つのresolverと明示schedule順を使う

## M5: presentation native / performance受入を閉じる

### 変更内容

1. all-building、Door、Soul、Familiar、camera compositionのvisual_testを固定する。
2. High / Medium / Low、DPI 1.0 / 1.5 / 2.0、Render3d visible / hiddenをnative確認する。
3. P00 `stage=p02`のaudit / behavior / Capture / Memoryを各required case 3反復し、RenderDoc固定1 frameでcamera、scene roots、material assets、frame / RSSを比較する。

### 完了条件

- [ ] black frame / double draw / invisible Bridgeがない
- [ ] camera inventoryとscene root countがtargetに一致する
- [ ] `stage=p02`のexact gate ID集合を満たす
- [ ] native artifactがfail-closed検証を通る

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

## 7. 検証計画

- presentation mapping exhaustive tests
- camera order / clear / owner count tests
- billboard spawn / rehydrate / cleanup / pool tests
- production shell Door state mutation / visual sync tests
- Bridge spawn / moving structural transform / Tank / MudMixer state tests
- Familiar exactly-one-presentation / Soul shadow runtime-zero tests
- named billboard resolver / sync order tests
- input binding / Help exact coverage tests
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- native Wall前後 / alpha / selection / animation / Door acceptance
- Help impact review

## 8. ロールバック方針

- M1 Door correctnessは照明と独立した不具合修正として保持し、presentation rollbackで`Door + Sprite`同居queryへ戻さない。
- M2 camera変更は独立commitとし、black frame時に限定revertできるようにする。
- billboard不合格時は開発中だけvisible GLB 1系統へ戻せる。Soul mask、per-actor material clone、2D foreground Soulは戻さず、P08 releaseはblockedとする。
- mapping不合格のBuildingTypeだけを暗黙fallbackせず、親表とmapping testを同時変更する。
- V / section再導入は本計画のrollback対象外とする。

## 9. AI引継ぎメモ

### 現在地

- 進捗: `0%`
- 完了済み: 計画作成
- 未着手: M1〜M5

### 次のAIが最初にやること

1. P01完了とmask inventory 0を確認する。
2. production Door root / child query mismatchが未修正であることを確認する。
3. M1のcompletion helper由来Door integration testから着手する。

### ブロッカー/注意点

- save presentation shellは別作業のregistry変更と重なる可能性がある。
- WallはP02時点でSectionMaterialを使い続ける。
- P04がmanual requestのpause / frame契約を所有し、P02はdomain mutationの成立までを所有する。

### 最終確認ログ

- Rust gates: `2026-08-04` / `not run (plan-only update)`
- native acceptance: `2026-08-04` / `not run (plan-only update)`
- docs gate: `2026-08-04` / `pass (docs --write / --check, check_docs, diff --check)`

### Definition of Done

- [ ] M1〜M5が完了
- [ ] production Door silent pathが解消済み
- [ ] 全Building / Soul / Familiarがexactly one presentation
- [ ] Soul shadow spawn / per-frame projector更新が0
- [ ] `stage=p02`のexact gate ID集合が合格
- [ ] presentation / camera / billboard native gate合格
- [ ] V elevation Help削除済み
- [ ] Help impact review完了
- [ ] 影響docs更新済み

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-08-07` | `Codex` | archive済みP00のregistered current formal baseline locatorへ同期。P02は未着手のDraftを維持 |
| `2026-08-04` | `Codex` | P00のstable presentation / performance gateと共通validity bundle参照へ同期 |
| `2026-08-03` | `Codex` | 統合計画M2をcamera、Building分類、billboard、TopDown-onlyへ具体化 |
