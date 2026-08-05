# P08: 旧RtT・section・projector撤去とrelease計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `single-scene-light-field-08-legacy-cleanup-release-plan-2026-08-03` |
| ステータス | `Draft` |
| 作成日 | `2026-08-03` |
| 最終更新日 | `2026-08-04` |
| 作成者 | `Codex` |
| 親計画 | [`../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md`](../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md) |
| 直接依存 | P01〜[P07](07-indoor-light-gameplay-room-plan-2026-08-03.md)すべて |
| 後続 | なし |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: production経路を停止しても、Soul projector、section uniform / shader、legacy 2D mirror、旧perf列やHelpが残れば保守対象とbinding costが継続する。
- 到達したい状態: TopDown + Scene RtT 1枚 + Light Fieldだけをproduction契約とし、旧経路の型・system・asset・feature・docsを参照0確認後に削除する。
- 成功指標: source / runtime inventoryにmask、Soul shadow、projector、elevation、section、legacy duplicateがなく、P00同一matrixの最終artifactが全gateを満たす。

## 2. スコープ

### 対象（In Scope）

- P02で停止済みのSoul shadow proxy / per-frame projector経路の物理削除。
- `SectionCut` / `SectionMaterial` / Terrain section field / shader branch / prepass branchの削除。
- `CLIP_DISTANCES`等の不要renderer feature、system registration、cache / rehydrate / perf列の削除。
- `LegacyStructural2dMirror`が残った場合の全consumer移行とshell削除。
- P00同一matrixのfinal audit / behavior / field-core / consumer-core / Capture / Memory / RenderDocと結果比較。
- Help / architecture / rendering / crate / gameplay / save docsの最終同期。

### 非対象（Out of Scope）

- Direct-to-windowへの追加移行。
- section、多層階、Soul shadowの再導入。
- 新規照明gameplay、Room UI、hero light。

## 3. 削除gate

削除はファイル名単位ではなくconsumer countで進める。

| legacy | P08着手条件 | 完了条件 |
| --- | --- | --- |
| Soul shadow runtime | P02でspawn / observer / sync / rehydrate / perf producer停止済み | type / cache / system / asset / metric列0 |
| projector uniform / WGSL | P06のTopDown materialがbuild / directional shadow / light fieldを代替 | Rust field / shader loop / constants 0 |
| SectionMaterial | P06で`MeshMaterial3d<SectionMaterial>` consumer 0 | type / plugin / sync / shader / prepass 0 |
| Terrain section fields | TopDown-only、dynamic writer 0 | 3 LOD uniform / WGSL discard 0 |
| elevation / SectionCut | P02でinput / producer / Help 0 | resource / state / system / test 0 |
| structural 2D mirror | P02 / P06でstate consumerを3Dへ移行 | mirror entity / hidden sprite / sync 0 |

各削除commitの前後に`rg` inventoryを保存し、consumerが残る場合は削除せず所有計画へ戻す。

## 4. マイルストーン

## M1: Soul shadow / projector残骸を削除する

### 変更内容

1. `SoulShadowProxy3d`、shadow GLB spawn / ready observer / owner cache / sync / cleanup / resetを削除する。
2. `sync_soul_shadow_projectors_system`、nearby projector collection、material per-frame writesを削除する。
3. Section / Terrain material uniformからprojector array / count / radius / opacityを削除する。
4. `soul_shadow_*` WGSL helper / loop、constants、material plugin、assetsを削除する。
5. perf scene root schemaのhistorical列扱いを閉じる。

### 主な変更ファイル

- `crates/bevy_app/src/entities/damned_soul/spawn.rs`
- `crates/bevy_app/src/systems/visual/character_proxy_3d/`
- `crates/bevy_app/src/plugins/visual.rs`
- `crates/hw_visual/src/{visual3d.rs,material/}`
- `assets/shaders/{section_material*.wgsl,terrain_surface_material*.wgsl}`
- perf Rust / Python artifact schema

### 完了条件

- [ ] `rg "SoulShadow|soul_shadow|shadow_projector"`のproduction参照0
- [ ] Soul数に比例するshadow GLB / material writeが0
- [ ] historical perf artifactをsilent reinterpretしない

## M2: section / elevation material残骸を削除する

### 変更内容

1. `SectionCut` resource / sync / plugin registration / testsを削除する。
2. `SectionMaterial`型、plugin、factory、build-progress syncの旧ownerを削除する。
3. Terrain 3 LOD uniform / Rust sync / WGSLからsection plane / direction / discardを削除する。
4. fragment / prepass shaderとimportを削除する。
5. `main.rs`の`WgpuFeatures::CLIP_DISTANCES`を参照0 / adapter互換確認後に削除する。

### 主な変更ファイル

- `crates/hw_visual/src/material/{section_material.rs,terrain_surface_material.rs,mod.rs}`
- `crates/hw_visual/src/lib.rs`
- `crates/bevy_app/src/systems/visual/{section_cut.rs,mod.rs}`
- `crates/bevy_app/src/{main.rs,plugins/visual.rs}`
- `assets/shaders/section_material*.wgsl`
- `assets/shaders/terrain_surface_material*.wgsl`

### 完了条件

- [ ] `rg "SectionCut|SectionMaterial|section_cut|CLIP_DISTANCES"`のproduction参照0
- [ ] provisional wall build progress / prepassがP06 materialで維持される
- [ ] Terrain 3 LOD shader compileとnative captureが合格

## M3: legacy mirror / schema /設定を掃除する

### 変更内容

1. P02で期限付き保持した`LegacyStructural2dMirror` consumerを3D stateへ移し、entity / syncを削除する。
2. 旧mask / proxy / elevation / shadowのDevPanel、env var、visual_test flag、perf column、artifact validatorを最終inventoryする。
3. root message / reset / cache inventoryからdead entryを削除する。
4. `cargo machete`等のrepository既定手順がある場合だけ不要dependencyを確認する。
5. P02のvisible GLB一時fallbackがactiveならreleaseを停止する。P08の完成形はbillboard 1系統であり、fallback GLBを第2の正式backendとして残さない。

### 主な変更ファイル

- `crates/bevy_app/src/systems/visual/`
- `crates/hw_visual/src/`
- `crates/bevy_app/src/interface/ui/dev_panel.rs`
- `crates/bevy_app/src/interface/ui/dev_panel/`
- `crates/visual_test/src/`
- `scripts/perf_tool/`
- workspace Cargo / plugin inventories

### 完了条件

- [ ] 1 building / actorにつき親計画どおりexactly one presentation
- [ ] hidden structural Sprite / Familiar 3D proxy / Soul GLB proxyが0
- [ ] billboard fallback flag / branchが0で、alpha / depth acceptanceが合格
- [ ] dead config / message / reset / perf columnが0

## M4: final性能・製品・docs gateを閉じる

### 変更内容

1. P00と同じcommit cleanliness / adapter manifestでaudit、behavior、field-core、consumer-core、Capture、Memoryを各required case 3反復し、RenderDocは固定checkpointの1 frameを採取する。
2. current / P01 / finalのDoor timeline、attachment、pass、scene roots、field rebuild / upload、frame p50 / p95 / p99、RSSを比較表にする。
3. save / rollback、Door、Wall、billboard、Building全分類、quality / DPIのnative acceptanceを実行する。
4. Help impact reviewを実行し、plain V削除、照明挙動、操作上必要な説明とexact approval snapshotを同期する。
5. 恒久docsへ設計契約を移し、親 / 子計画を完了後にarchiveまたは削除する。

`stage=p08`でrequiredなgate IDは次のexact集合とする。

- validity: `RLV1-BUNDLE-VALID`
- preservation: `RLV1-P01-RTT`、`RLV1-P02-DOOR-DOMAIN`、`RLV1-P02-PRESENT`
- field / lifecycle: `RLV1-P03-FIELD`、`RLV1-P04-EMITTER`、`RLV1-P04-STEADY`、`RLV1-P05-LIFECYCLE`
- rendering: `RLV1-P06-UPLOAD`、`RLV1-P06-RENDER`、`RLV1-P06-COLOR`
- gameplay / integration: `RLV1-P07-CPU-CONSUMERS`、`RLV1-P07-CONSUMER-CPU`、`RLV1-P08-CROSS-CONSUMER`
- final budget: `RLV1-P08-FRAME`、`RLV1-P08-MEMORY`

P01 / P02 / P06の導入時performance gateは各owner stageで閉じ、finalでは`RLV1-P08-FRAME` / `RLV1-P08-MEMORY`へ置き換える。構造 / 表示のpreservation gateは置き換えない。

### 更新対象docs

- `docs/architecture.md`
- `docs/rendering-performance.md`
- `docs/performance-profiling.md`
- `docs/crate-map.md`
- `docs/buildings.md`
- `docs/rooms.md`
- `docs/soul-energy.md`
- `docs/save-system.md`または現行save正本
- `docs/help-screen.md`
- `crates/hw_infra/README.md`
- `crates/hw_visual/README.md`

### 完了条件

- [ ] 上記`stage=p08` exact gate ID集合がすべて合格
- [ ] native artifact validatorがfail-closedで合格
- [ ] Help manifest / provider / coverage / exact approvalが一致
- [ ] current specsから旧full RtT / section / Soul shadow記述がない

## 5. 最終受入matrix

| fixture | 必須観測 |
| --- | --- |
| 直線Wall + Door | Open通光、Closed / Locked遮光、visualと同frame |
| L字corner | exact corner光漏れなし |
| wall-mounted Lamp | inward側のみ発光、invalid mountはdark |
| many Lamp | 50 emitterでもtexture / binding数一定、effect non-stack |
| Soul / Familiar | SoulはWall depth、Familiarはforeground、shadow caster 0 |
| all BuildingType | exhaustive class、duplicate / invisible 0、move / state表示維持 |
| load / rollback failure | resetからdark、旧epoch再利用0 |
| quality / DPI | map-space light範囲不変、camera composition正常 |

## 6. 検証計画

- 各削除対象のbefore / after `rg` inventory。
- workspace format / check / clippy / test / docs / policy full gate。
- P00 deterministic audit / behavior / field-core / consumer-core。
- native Capture / Memory build、artifact validation。
- RenderDoc pass / attachment / binding確認。
- visual_test golden / pixel probe。
- Help exhaustive coverage / exact approval。
- `git diff --check`。

## 7. リスクと対策

| リスク | 対策 |
| --- | --- |
| SectionMaterialが隠れたbuild consumerを持つ | P06 consumer 0 gateとM2前の`rg`を必須にする |
| perf schema削除で旧artifactを誤読する | schema version / historical readerを明示してsilent変更しない |
| cleanupと機能修正が混ざる | M1〜M3をconsumer別commitにし、挙動変更は所有計画へ戻す |
| final値を見てgateを緩める | P00の数値を固定し、変更には新baseline世代と根拠を要求する |
| Helpだけ古い操作を残す | mandatory Help impact Skillでmanifestからsnapshotまで確認する |

## 8. ロールバック方針

- cleanup commitはSoul projector、section、mirror、schema / docsの単位で分ける。
- 削除前materialへ戻す必要が出た場合は該当consumerが残る最後のcommitだけを戻し、mask RtTやSoul shadow runtimeは再導入しない。
- final性能不合格時はP06 visual consumerをambient-onlyへ切れる境界を使って原因を分離し、P00 gateを無断で変更しない。
- section / V / Soul shadowの製品再導入はrollbackではなく新proposalとnative evidenceを要求する。

## 9. AI引継ぎメモ

### 現在地

- 進捗: `0%`
- 完了済み: 削除gate / final acceptance設計
- 未着手: M1〜M4

### 次のAIが最初にやること

1. P01〜P07のDefinition of Doneと未完compatibility markerを一覧化する。
2. P00 baseline manifestと同一adapter / cleanlinessを確保する。
3. M1のSoul shadow runtime / uniform参照inventoryから着手する。

### ブロッカー/注意点

- P02でSoul shadow動作経路は停止済みであること。P08まで動かし続けない。
- P06でSectionMaterial consumer 0になるまでM2へ進まない。
- 実機検証には`hell-workers-run-native-acceptance` Skillを必ず使う。
- 機能・runtime docs変更後は`hell-workers-review-help-impact` Skillを必ず実行する。

### 最終確認ログ

- Rust gates: `2026-08-04` / `not run (plan-only update)`
- native acceptance: `2026-08-04` / `not run (plan-only update)`
- Help impact: `2026-08-04` / `not run (plan-only update)`
- docs gate: `2026-08-04` / `pass (docs --write / --check, check_docs, diff --check)`

### Definition of Done

- [ ] M1〜M4が完了
- [ ] 親計画の全横断gateが合格
- [ ] 旧RtT / shadow / section / mirror参照0
- [ ] audit / behavior / field-core / consumer-core / Capture / Memory / RenderDocのfinal artifactsが保存済み
- [ ] Help / native / docs / workspace full gateが完了
- [ ] 親と全子計画をarchiveまたは削除し、恒久docsへ移管済み

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-08-04` | `Codex` | P00の全formal legへ最終計測を同期し、統計3反復と固定frame RenderDoc captureを分離 |
| `2026-08-03` | `Codex` | 旧runtime / shader削除とfinal acceptanceを独立計画化 |
