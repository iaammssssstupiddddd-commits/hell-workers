# P01: Soul mask撤去・単一Scene RtT移行計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `single-scene-light-field-01-single-scene-rtt-plan-2026-08-03` |
| ステータス | `Draft` |
| 作成日 | `2026-08-03` |
| 最終更新日 | `2026-08-04` |
| 作成者 | `Codex` |
| 親計画 | [`../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md`](../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md) |
| 直接依存 | [P00](00-baseline-gates-plan-2026-08-03.md) |
| 後続 | [P02](02-topdown-presentation-plan-2026-08-03.md)、[P06](06-indoor-light-rendering-plan-2026-08-03.md)、[P08](08-legacy-cleanup-release-plan-2026-08-03.md) |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: Sceneと同解像度のSoul mask target、専用Camera3d、mask proxy、composite拡張、運用toggleが常時costと保守経路を増やしている。
- 到達したい状態: `RttRuntime`はScene handleだけを持ち、overlay compositeはSceneを通常1回sampleする。
- 成功指標: P00 captureに存在したmask pass / attachment / proxyがproduction、test、metricの全経路から消える。

## 2. スコープ

### 対象（In Scope）

- `RttRuntime.soul_mask`とmask target lifecycleの削除。
- `Camera3dSoulMaskRtt`とmask RenderLayerの削除。
- `SoulMaskProxy3d`、`SoulMaskMaterial`、GLB ready / sync / cache / rehydrateの削除。
- composite material / WGSLのScene-only化。
- DevPanel / env toggle / perf scenario / visual_testのmask契約削除。
- resize、DPI、quality変更時のScene target再生成維持。

### 非対象（Out of Scope）

- visible Soul GLBからbillboardへの変更（P02）。
- `SoulShadowProxy3d`の動作停止（P02）とprojector uniform / shaderの物理削除（P08）。
- Camera2d二重passの整理（P02）。
- Light Field texture（P06）。

## 3. 現状とギャップ

| 経路 | 現行symbol | 終了状態 |
| --- | --- | --- |
| runtime | `RttRuntime { scene, soul_mask, ... }` | Scene handle 1つ |
| camera | `Camera3dSoulMaskRtt` order -2 | entity / query 0 |
| proxy | `SoulMaskProxy3d` + owner cache | type / spawn / cleanup 0 |
| material | `SoulMaskMaterial` | plugin / asset / shader 0 |
| composite | mask texture + sampler + radius | Scene texture / samplerのみ |
| toggle | `RenderPerfToggles.soul_mask_enabled` / `HW_DISABLE_SOUL_MASK` | field / env / button 0 |
| metrics | `soul_mask_proxy_3d` | schemaから削除しmigration noteを残す |

## 4. 実装方針

### 4.1 atomic migration order

1. Scene-only `RttRuntime` APIとcomposite assetを先に用意する。
2. startup / resize / camera syncのmask consumerを切り替える。
3. Soul spawn / observer / cache / rehydrateからmask proxyを削除する。
4. plugin / material / layer / toggleを削除する。
5. visual_testとperf schemaを同じcommit列で追従させる。
6. symbol inventory 0とnative captureを確認する。

中間commitでも存在しないhandleをmaterialへbindしない。dummy mask textureによる互換期間は設けない。

### 4.2 Scene target contract

- `RttRuntime`はScene `Handle<Image>`、physical size、target scale factorだけを所有する。
- resize / quality変更はScene imageを一度だけrecreateし、Camera3d targetとcomposite materialを同じsystemでrebindする。
- overlay Camera2dとScene composite spriteは維持する。
- Scene image format、sampler、clear colorはP00 current contractを変えない。

### 4.3 composite contract

- material bindingはScene texture + samplerだけにする。
- fragmentは座標補正後にSceneを通常1回sampleし、色を返す。
- mask blur、center mask、12方向sample、ring色、mask radius uniformを削除する。
- P02のbillboard soft effectを先取りしてcompositeへ新しいactor loopを追加しない。
- `sync_rtt_composite_perf_params_system`と`composite_shadow_offset_uv`はmask / projector専用consumerが0なら一緒に削除する。
- `RttRuntime::pixel_size()`等のmask / offset専用APIもconsumer 0を確認して削除する。

## 5. マイルストーン

## M1: Scene-only runtime / compositeを成立させる

### 変更内容

1. `RttRuntime`から`soul_mask`、`soul_mask_render_target()`、同時recreateを削除する。
2. `RttCompositeMaterial`からmask binding / radiusを削除する。
3. resize / DPI / quality rebind queryをScene Camera 1台へ縮小する。
4. shaderをScene 1-sampleへ変更する。

### 主な変更ファイル

- `crates/bevy_app/src/plugins/startup/{rtt_setup.rs,rtt_composite.rs,startup_systems.rs}`
- `assets/shaders/rtt_composite_material.wgsl`

### 完了条件

- [ ] `RttRuntime`のcolor handleが1つ
- [ ] resize時にScene Cameraとcompositeが同じ新handleを参照する
- [ ] shaderにmask binding / loopがない

### focused test

- Scene target recreateがsize / scaleを保持するtest
- resize rebind後のhandle一致test
- composite material reflection / binding test

## M2: mask camera / proxy / materialを撤去する

### 変更内容

1. `Camera3dSoulMaskRtt`のspawn、query exclusion、sync、visibility toggleを削除する。
2. Soul spawnからmask SceneRootを削除する。
3. mask GLB ready observer、sync、owner cache register / cleanupを削除する。
4. save presentation cleanup / rehydrateからmask shellを削除する。
5. `SoulMaskMaterial`のMaterialPlugin、handle、Rust module、WGSLを削除する。
6. `LAYER_3D_SOUL_MASK`を削除する。
7. terrain LODのmask-camera exclusion、`SceneObjectQuery`、`apply_render3d_visibility_system`のmask perf branch、`hw_visual::reset_for_world_replace`のmask cacheを削除する。

### 主な変更ファイル

- `crates/bevy_app/src/entities/damned_soul/spawn.rs`
- `crates/bevy_app/src/systems/visual/character_proxy_3d/{cache.rs,gltf_ready.rs,sync.rs,tests/}`
- `crates/bevy_app/src/systems/save/rehydrate/{presentation.rs,tests/presentation.rs}`および現行registry adapter
- `crates/bevy_app/src/plugins/{visual.rs,startup/visual_handles.rs,startup/startup_systems.rs}`
- `crates/hw_visual/src/{lib.rs,visual3d.rs,material/mod.rs,material/soul_mask_material.rs}`
- `crates/hw_core/src/constants/render.rs`
- `assets/shaders/soul_mask_material.wgsl`

### 完了条件

- [ ] `rg -n "SoulMask|soul_mask|LAYER_3D_SOUL_MASK" crates assets`のproduction参照が0
- [ ] Soul spawn / load / despawnでmask entityを生成しない
- [ ] visible Soul GLBとSoul shadow pathはP01前と同じ

## M3: toggle / test / metric契約を整理する

### 変更内容

1. `RenderPerfToggles.soul_mask_enabled`、`HW_DISABLE_SOUL_MASK`、test presetを削除する。
2. DevPanelのMask button、label、action、presentationを削除する。
3. visual_testのmask camera / material / proxy / resize pathを削除する。
4. `PerfSceneRootCounts` / `PerfChecksumQueries` / Rust outputとPython `SCENE_ROOT_COLUMNS` / reader / expected countsから`soul_mask_proxy_3d`を削除する。
5. artifact schemaを明示的にbumpし、旧schema（現行v11等）はhistoricalとして受理または明示rejectする。列を無言で読み替えない。
6. P00 artifactとのcolumn差を`docs/performance-profiling.md`またはP00 logへ記録する。

### 主な変更ファイル

- `crates/bevy_app/src/lib.rs`
- `crates/bevy_app/src/interface/ui/dev_panel/`
- `crates/bevy_app/src/plugins/{interface.rs,startup/perf_scenario.rs,startup/perf_scenario/}`
- `crates/visual_test/src/`
- `scripts/perf_tool/{model.py,artifacts.py,fixtures.py}`
- `docs/{architecture.md,debug-features.md,visual_test.md,performance-profiling.md,rendering-performance.md}`

### 完了条件

- [ ] env / UI / testから存在しないmask機能を選べない
- [ ] perf artifact validatorが新schemaを受理し、旧baselineとの差を明示する
- [ ] visual_testがScene-onlyで起動する

## M4: native / performance受入を閉じる

### 変更内容

1. P00 `stage=p01`のaudit / behavior / Capture / Memoryを各required case 3反復し、RenderDocを固定1 frame採取する。
2. DPI / quality / resize / pan / zoomで座標とalphaを確認する。
3. captureでmask pass / attachment消滅とScene sample数を確認する。
4. P00の`RLV1-P01-RTT` / `RLV1-P01-PERF`と`RLV1-BUNDLE-VALID`を比較する。

### 完了条件

- [ ] Scene以外の画面解像度依存world color targetがない
- [ ] `RLV1-P01-RTT` / `RLV1-P01-PERF`と`RLV1-BUNDLE-VALID`を満たす
- [ ] black frame、stale handle、resizeずれがない

## 6. リスクと対策

| リスク | 対策 |
| --- | --- |
| proxy typeだけ削除してrehydrateがstale entityを残す | spawn / clear / rehydrate / owner cacheを一つのinventoryで削除する |
| resize後にcompositeが旧handleを読む | Camera targetとmaterial rebindを同じsystem / testで固定する |
| perf schema削除でP00比較不能 | column migration noteと互換性reject理由を残す |
| mask削除とvisible Soul変更が混ざる | P01ではvisible GLBとSoul shadowを維持し、P02で同時に切り替える |

## 7. 検証計画

- focused RtT runtime / rebind tests
- character proxy lifecycle / rehydrate tests
- `python3 scripts/perf.py self-test`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- native audit / behavior / Capture / Memory / RenderDoc acceptance
- `git diff --check`

## 8. ロールバック方針

- M1〜M3を同じP01 commit列としてrevertし、dummy maskや半端なtoggleを残さない。
- P01後のSoul輪郭品質が不足してもmask RtTを即時復活させず、P02 billboardのalpha silhouetteで評価する。
- performance未達時はattachment、composite、proxyのどの削除が原因かP00 capture単位で切り分ける。

## 9. AI引継ぎメモ

### 現在地

- 進捗: `0%`
- 完了済み: 計画作成
- 未着手: M1〜M4

### 次のAIが最初にやること

1. P00の`RLV1-P01-RTT` / `RLV1-P01-PERF`、`RLV1-BUNDLE-VALID`、artifact pathを確認する。
2. `rg`でmask inventoryを更新し、本計画の変更ファイルとの差を確認する。
3. M1のruntime / composite focused testから着手する。

### ブロッカー/注意点

- save / rehydrateには並行変更があり得る。現行registry / shell ownershipを読んでからM2を編集する。
- `Camera3dSoulMaskRtt`はcamera sync / terrain LOD query exclusionにも現れる。
- visual_testとperf toolingをproduction削除より後回しにしない。

### 最終確認ログ

- Rust gates: `2026-08-04` / `not run (plan-only update)`
- native acceptance: `2026-08-04` / `not run (plan-only update)`
- docs gate: `2026-08-04` / `pass (docs --write / --check, check_docs, diff --check)`

### Definition of Done

- [ ] M1〜M4が完了
- [ ] mask inventoryが0
- [ ] `RLV1-P01-RTT` / `RLV1-P01-PERF`と`RLV1-BUNDLE-VALID`が合格
- [ ] Help impact review完了
- [ ] 影響docs更新済み

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-08-04` | `Codex` | P00のstable RtT / performance gateと共通validity bundle参照へ同期 |
| `2026-08-03` | `Codex` | 統合計画M1をruntime / proxy / tooling / native gateへ具体化 |
