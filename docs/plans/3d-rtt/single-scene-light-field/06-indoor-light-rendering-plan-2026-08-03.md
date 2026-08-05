# P06: 室内 Light Field GPU表示計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `single-scene-light-field-06-indoor-light-rendering-plan-2026-08-03` |
| ステータス | `Draft` |
| 作成日 | `2026-08-03` |
| 最終更新日 | `2026-08-04` |
| 作成者 | `Codex` |
| 親計画 | [`../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md`](../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md) |
| 直接依存 | [P01](01-single-scene-rtt-plan-2026-08-03.md)、[P02](02-topdown-presentation-plan-2026-08-03.md)、[P04](04-indoor-light-runtime-integration-plan-2026-08-03.md)、[P05](05-indoor-light-save-lifecycle-plan-2026-08-03.md) |
| 後続 | [P08](08-legacy-cleanup-release-plan-2026-08-03.md) |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: CPU fieldを実装しても、Terrain / Wall / Door /大型構造物が別material経路のままでは照度表示とload resetが一致しない。
- 到達したい状態: 100×100の共有Light Field textureをrevision単位で1回uploadし、全`Structural3d` materialが同じworld XZ mappingでsamplingする。
- 成功指標: emitter数によらずtexture / upload / binding数が一定、Door / Wall遮光がCPU expected fieldとpixel probeで一致し、load開始時に旧照明が表示されない。

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
| dimensions | world gridと同じ100×100。map dimensionが可変化した場合のみrecreate |
| format | linear `Rgba8Unorm`（sRGB変換なし）。RGB=local light、A=indoor mask |
| sampler | nearest、clamp-to-edge、mipmapなし |
| ownership | `IndoorLightTexture` resourceがhandle、uploaded revision、uploaded `WorldEpoch`を所有 |
| allocation | worldごとに1 Image。field revisionごとにassetを増やさずbytesだけ更新 |
| upload | field revisionまたはepoch変更時だけ。steady-state 0回 |
| reset | P05 reset通知時にblack bytesを同期反映し、旧revisionをinvalid化 |

100×100×4 bytes = 40,000 bytesを基準容量とする。row pitch / backend stagingはP00 captureで別に記録する。

### 3.2 world XZからUV

共通WGSL helperを1つだけ持つ。

```text
grid_x = floor((world_x - map_origin_x) / tile_size)
grid_y = floor((map_origin_z - world_z) / tile_size)
uv = (vec2(grid_x, grid_y) + 0.5) / vec2(map_width, map_height)
```

- CPUの`world_to_grid`とY/Z反転規則をgolden vectorで照合する。
- map外はblackとし、clamp sampleによる端cellの光漏れを防ぐためshader側でbounds判定する。
- UV / dimensions / origin / tile sizeはmaterialごとの複製値ではなく共有uniform contractにする。

### 3.3 surface policy

- Terrain / Floor / Bridge /大型構造物の上面はfragment world positionのcellをsampleする。
- Wall側面は自己遮光cellを直接sampleせず、surface normal方向の隣接cellへ小さくoffsetする。
- Wall上面はcardinal 4近傍からluminance最大の1 cellを選び、そのcellのRGB一式を採る。tieはNorth → East → South → West、map外はblackとし、channel-wise maxで色を合成しない。
- Doorはcurrent transformではなくDoor rootのgrid cellとsurface normalから同じ規則を使う。Open visual回転で別cellをsampleしない。
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

## 5. マイルストーン

## M1: CPU→Image bridgeを実装する

### 変更内容

1. startupでblack 100×100 Imageとnearest samplerを1つ作る。
2. P04 field snapshotのrevision / epochを比較してbytesを更新する。
3. P05 reset通知を最優先で処理し、同frameのstale uploadを拒否する。
4. upload count / bytes / duration / asset countをinstrumentationへ追加する。

### 主な変更ファイル

- `crates/bevy_app/src/systems/visual/indoor_light_texture.rs`
- `crates/bevy_app/src/plugins/visual.rs`
- `crates/hw_visual/src/material/indoor_light.rs`
- perf metric / artifact schema

### 完了条件

- [ ] Imageはworldごとに1つ
- [ ] unchanged 600 frameのuploadが0
- [ ] epoch mismatch時はblack以外をuploadしない
- [ ] 40,000-byte payloadとrow orientationがtestで一致する

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

- [ ] LOD0 / 1-lite / 2のsamplingが同一cellを指す
- [ ] indoor mask外とmap外がblack
- [ ] directional shadowとの合成順が固定画像と一致する

## M3: 全Structural3dをTopDown materialへ移す

### 変更内容

1. `TopDownStructuralMaterial`、prepass、shared handle registryを追加する。
2. Wall / ProvisionalWallのbuild progressとsurface表現を移植する。
3. Door / Floor / Bridge / Tank / MudMixer / RestArea / SoulSpaをreceiverへ移す。
4. Tank / MudMixerの有限state material、move / transform sync、load shellをP02契約と結合する。
5. `SectionMaterial` consumer countを0へする。

### 主な変更ファイル

- `crates/hw_visual/src/material/{mod.rs,topdown_structural_material.rs}`
- `assets/shaders/{topdown_structural_material.wgsl,topdown_structural_material_prepass.wgsl}`
- `crates/bevy_app/src/systems/visual/`
- building spawn / completion / rehydrate adapters

### 完了条件

- [ ] 全`Structural3d`が同じfield revisionをsampleする
- [ ] build progress / prepass / directional shadowが回帰しない
- [ ] per-building material cloneがない
- [ ] `MeshMaterial3d<SectionMaterial>` query / spawnが0

## M4: reset・性能・native受入を閉じる

### 変更内容

1. P00 `stage=p06`の`door-state-v1`とbehavior 6 load lifecycle caseを各3反復し、GPU epoch / upload / checksum列とblack-firstを検証する。
2. P00 small / medium / largeでCPU upload、Captureのgpu render mode wall-frame p95 / p99、RenderDocのpass / binding、RSSを比較する。
3. Door Open / Closed / Locked、Wall内外、corner、wall-mounted inward / outwardをpixel probe + native imageで検証する。
4. quality Low / Medium / High、DPI 1.0 / 1.5 / 2.0でmap-space照明が変形しないことを確認する。
5. audit / Capture / Memory / field-coreを各required case 3反復し、RenderDocを固定1 frame採取して共通validityを閉じる。

### 完了条件

- [ ] stale-light frameが0
- [ ] Lamp数を増やしてもGPU texture / material binding数が一定
- [ ] `stage=p06`のexact gate ID集合を満たす
- [ ] native artifactがfail-closed validatorを通る

## 6. 検証計画

- Image byte layout / revision / epoch / steady-state tests。
- WGSL import / binding / shader compile tests。
- CPU expected field対pixel probeのgolden tests。
- Terrain LOD seam / map orientation tests。
- structural material shared-handle / build-progress / prepass tests。
- world replacement black-first tests。
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `python3 scripts/dev.py verify`
- native renderer / GPU / DPI acceptance（`hell-workers-run-native-acceptance` Skill必須）。
- Help impact review。

## 7. リスクと対策

| リスク | 対策 |
| --- | --- |
| binding番号衝突 | derive / shaderを同時確認しcompile test。Terrainは133/134を予約 |
| Wallが自己遮光で常時暗い | side normal offsetとtop max-adjacentを固定fixture化 |
| SectionMaterial置換でconstruction clipが消える | parity material完成後にconsumerを一括移行する |
| Imageをrevisionごとに生成してassetが増える | handle固定、bytes更新、asset count gateを持つ |
| load直後に旧textureを1frame表示する | epoch invalidation / black clearをfield uploadより先に処理する |

## 8. ロールバック方針

- GPU consumerはfeature registrationを外してblack / ambient-onlyへ戻せる。P03〜P05 logical stateは保持する。
- Terrain receiverとstructural receiverを別commitにし、shader問題の範囲を限定する。
- `TopDownStructuralMaterial`移行後にSectionMaterialをP08まで残すため、M3内の限定rollbackを可能にする。ただし二重描画は許可しない。

## 9. AI引継ぎメモ

### 現在地

- 進捗: `0%`
- 完了済み: texture / material移行契約
- 未着手: M1〜M4

### 次のAIが最初にやること

1. P01 / P02 / P04 / P05の完了を確認する。
2. Bevy 0.19 `Image` / `AsBindGroup` APIをdocsrs-mcpまたはlocal registryで確認する。
3. current Terrain / Section binding layoutを再度inventoryし、M1 black Image testから着手する。

### ブロッカー/注意点

- `frames.csv`はGPU pass timeではない。wall-frame quantileはCapture、pass / binding構造はRenderDocというP00の区分を使う。
- Soul billboardやforegroundへ照明textureをbindしない。
- section fieldの削除はP08まで行わない。

### 最終確認ログ

- Rust gates: `2026-08-04` / `not run (plan-only update)`
- native acceptance: `2026-08-04` / `not run (plan-only update)`
- docs gate: `2026-08-04` / `pass (docs --write / --check, check_docs, diff --check)`

### Definition of Done

- [ ] M1〜M4が完了
- [ ] 全Structural3d receiverが同一field textureを使う
- [ ] steady-state upload 0、stale-light frame 0
- [ ] `stage=p06`のexact gate ID集合とnative acceptanceが合格
- [ ] Help impact reviewとrenderer恒久docs更新が完了

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-08-04` | `Codex` | P00のevidence区分へ同期し、Wall上面sampleのtie / OOB / RGB選択規則を確定 |
| `2026-08-03` | `Codex` | CPU fieldからGPU texture / Terrain / structural receiverへの移行を独立計画化 |
