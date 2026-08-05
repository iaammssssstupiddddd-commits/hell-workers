# 単一 Scene RtT・室内 Light Field 移行プログラム計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `single-scene-rtt-indoor-light-field-migration-plan-2026-08-03` |
| ステータス | `Draft` |
| 作成日 | `2026-08-03` |
| 最終更新日 | `2026-08-04` |
| 作成者 | `Codex` |
| 採用判断 | TopDown 2.5D、Scene RtT 1枚、map-space radial Light Field |
| 関連提案 | `N/A` |
| 置換した計画 | [`milestone-roadmap.md`](milestone-roadmap.md)の未完項目 / [`lighting-visual-plan-2026-04-04.md`](lighting-visual-plan-2026-04-04.md) / [Soul outline proposal](../../proposals/soul-outline-mask-ring-proposal-2026-04-16.md) |
| 並行計画 | [`../hvac-plumbing-plan-2026-07-13.md`](../hvac-plumbing-plan-2026-07-13.md) / [`../archive/save-rehydration-registry-plan-2026-08-03.md`](../archive/save-rehydration-registry-plan-2026-08-03.md) |
| 関連Issue/PR | `N/A` |

本書は移行全体の製品契約、依存順、統合gateだけを所有する。実装手順、変更ファイル、focused test、rollbackは下記9計画を正本とし、本書へ重複させない。

## 1. 目的

### 解決したい課題

- 現行RtTは画面解像度の`scene`と`soul_mask`を常時保持し、2台のCamera3dと多サンプルcompositeを必要とする。
- 旧ロードマップのfull RtT、Soul mask、矢視、section、多層階は、現在のTopDown 2.5D方針と一致しない。
- 室内環境設備として必要なのは多数の放射状光源であり、Lampごとのshadow mapではなくWall / Door遮光を一定costで表現する必要がある。
- 現行Lamp gameplay効果は遮光を参照せず、表示とgameplayが別の正本になっている。
- Camera2d actorは3D Wallとdepthを共有できないため、Wall裏へ隠すactorだけ同一Sceneへ移す必要がある。

### 到達したい状態

1. 画面解像度依存のworld color targetはScene RtT 1枚だけである。
2. Terrain、Floor、Wall、Door、Bridge、大型構造物だけを構造3D sceneへ置く。
3. Soulは共有unlit billboardとして同じdepth sceneへ置くが、室内光のcaster / visual receiverにはしない。
4. Familiarと構造depth不要の小物・effectはcomposite後のforegroundへ置く。
5. `hw_infra`のCPU `IndoorLightField`を表示、Room summary、Lamp gameplay効果の唯一の照度正本にする。
6. 完成WallとClosed / Locked Doorは遮光し、Open DoorとProvisionalWallは通光する。
7. load / rollbackは旧worldの照度を表示せずfail-darkから再構築する。
8. V矢視とsection固有runtimeを段階的に撤去し、TopDown-onlyをHelpまで同期する。

## 2. 固定する製品契約

### 2.1 表示分類

P00でこの表をbaseline artifactと一緒に凍結する。分類変更は子計画内だけで行わず、本表とP02を同時更新する。

| 対象 | `RenderPresentationClass` | depth | 室内光 | shadow |
| --- | --- | --- | --- | --- |
| Terrain / Floor / Bridge | `Structural3d` | あり | receiver | directional sunのみ |
| Wall / Door | `Structural3d` | あり | receiver + occluder | directional sunのみ |
| Tank / MudMixer / RestArea / SoulSpa | `Structural3d` | あり | receiver | directional sunのみ |
| SandPile / BonePile / WheelbarrowParking / OutdoorLamp器具 | `Foreground2d` | なし | 非receiver | なし |
| Soul | `DepthBillboard3d` | Wallと共有 | 非receiver | casterにしない |
| Familiar | `Foreground2d` | 常時前景 | 非receiver | なし |
| 選択・範囲・speech等 | `Foreground2d` | 常時前景 | 非receiver | なし |

- BuildingCategory、`blocks_movement()`、footprintだけから表示分類を推論しない。
- OutdoorLampの器具spriteがforegroundでも、論理`RadialLightEmitter`はworld grid上に存在する。
- Soul billboardの開発中rollbackはvisible GLB 1系統だけとし、Soul mask RtTは戻さない。ただしこれは一時fallbackであり、billboard未成立のままP08 releaseを完了しない。

### 2.2 光と遮光

| 入力 | 契約 |
| --- | --- |
| emitter | `PowerSupplyState::Supplied`の`RadialLightEmitter`だけを収集する |
| Wall | `BuildingType::Wall`かつ`Without<ProvisionalWall>`だけが遮光する |
| Door | Openは通光、Closed / Lockedは遮光する |
| Soul / Familiar / 家具 | caster / occluder / visual receiverにしない |
| 合成 | directional shadow適用後にlocal lightを加算する |
| gameplay | GPU textureではなく同じCPU fieldを読む |
| steady-state | input revision不変時はfield再計算0回、Image upload 0回 |

local lightの合成はlinear空間で`directional_styled_rgb + base_color_rgb * local_light_rgb`とし、local専用ambient floorは追加しない。CPU fieldを1.0でsaturateした後のtone mappingは既存Scene pipelineを使う。gain、clamp、色変換をP06だけで別調整せず、P00のgolden fixtureでこの式を固定する。

標準室内照明にshadow付きPointLight / SpotLightを使わない。例外的なhero lightは別計画、固定budget、実測を必須とする。

### 2.3 Viewと非対象

- productionの正本はTopDown 2.5Dとする。
- V矢視の入力とHelpをP02で削除し、到達不能になったsection固有material / shaderをP08で撤去する。
- section互換、多層階、3D volume light、天井高、光源高、soft shadow、specular、家具遮光は対象外とする。
- Direct-to-window、新しい室内照明overlay、Room照度UI、快適度・健康バランス、新規アート制作は対象外とする。

## 3. 分割計画と依存順

```text
P00 baseline / contract
 ├─> P01 one Scene RtT ─> P02 TopDown presentation ────────────────┐
 └─> P03 Light Field core ─> P04 runtime integration ─> P05 save ─┼─> P06 GPU rendering ─┐
                                                    └──────────────┴─> P07 gameplay / Room ┤
 P01 + P02 + P03 + P04 + P05 + P06 + P07 ────────────────────────────────> P08 release
```

| ID | 実装計画 | 直接依存 | 独立した完了成果 |
| --- | --- | --- | --- |
| P00 | [`00-baseline-gates-plan-2026-08-03.md`](single-scene-light-field/00-baseline-gates-plan-2026-08-03.md) | C00-Aはなし。C00-B以降はHVAC M0または同等correctness commit | current capture、安定比較schema、数値gate、表示分類、system-order contract |
| P01 | [`01-single-scene-rtt-plan-2026-08-03.md`](single-scene-light-field/01-single-scene-rtt-plan-2026-08-03.md) | P00 | Soul mask target / camera / proxy / metricを除去したScene RtT 1枚 |
| P02 | [`02-topdown-presentation-plan-2026-08-03.md`](single-scene-light-field/02-topdown-presentation-plan-2026-08-03.md) | P00, P01 | Door実経路修復、world 2D pass 1回、表示分類、Soul billboard / Familiar前景、Soul shadow動作停止、V入力停止 |
| P03 | [`03-indoor-light-domain-core-plan-2026-08-03.md`](single-scene-light-field/03-indoor-light-domain-core-plan-2026-08-03.md) | P00 | `hw_infra`のdeterministicな論理fieldとpure LOS |
| P04 | [`04-indoor-light-runtime-integration-plan-2026-08-03.md`](single-scene-light-field/04-indoor-light-runtime-integration-plan-2026-08-03.md) | P02 M1, P03、HVAC M0または同等correctness commit | topology / energy / Room / Doorを結ぶ更新transactionとsteady-state dirty管理 |
| P05 | [`05-indoor-light-save-lifecycle-plan-2026-08-03.md`](single-scene-light-field/05-indoor-light-save-lifecycle-plan-2026-08-03.md) | P03, P04、save registry計画 | durable mount、named rehydrate step、load / rollback fail-dark |
| P06 | [`06-indoor-light-rendering-plan-2026-08-03.md`](single-scene-light-field/06-indoor-light-rendering-plan-2026-08-03.md) | P01, P02, P04, P05 | 100×100 Light Field textureと全`Structural3d` receiver |
| P07 | [`07-indoor-light-gameplay-room-plan-2026-08-03.md`](single-scene-light-field/07-indoor-light-gameplay-room-plan-2026-08-03.md) | P03, P04, P05 | Soul / Roomが同じfield revisionを読むgameplay統合 |
| P08 | [`08-legacy-cleanup-release-plan-2026-08-03.md`](single-scene-light-field/08-legacy-cleanup-release-plan-2026-08-03.md) | P01〜P07 | projector / section残骸撤去、性能比較、Help、最終gate |

### 3.1 着手とmergeの規則

- P00完了前にP01以降のproduction変更へ着手しない。
- P00 C00-BとP04は、室内設備が占有するfloor cellをRoom interiorとして維持するHVAC M0がmerge済み、または同じcorrectness変更を単一ownerで先行するまで開始しない。
- P03とP01はP00後に並行可能だが、同じファイルを触る作業は同時に実行しない。
- P04はsave / rehydrateを編集せずruntime transactionだけを所有する。
- P05は`save-rehydration-registry-plan`の実装がmerge済み、またはその所有者と変更順が合意済みになるまで`save/rehydrate*`を編集しない。
- P06とP07はP04 / P05のfield revision / epoch contractを変更せずconsumerとして実装する。
- 各子計画はfocused test、workspace check、Clippy、Help impact判断、必要なnative受入まで閉じてから次の依存計画を開始する。
- 子計画の途中状態を長期間productionへ残さない。1計画内のwork packageは連続commitとする。

### 3.2 推奨merge列

| batch | work package | merge可能条件 |
| --- | --- | --- |
| B00 | P00契約・fixture・4 evidence family / 5 current leg・stable gate ID・数値gate | Room interior-role owner確定後、clean commitのformal baselineが揃う |
| B01 | P01 M1 Scene-only runtime / composite | P00完了 |
| B02 | P01 M2〜M4 mask camera / proxy / tooling / schema撤去 | B01 green |
| B03 | P02 M1 production Door経路修復 | P01完了。照明とは独立したcorrectness commit |
| B04 | P02 M2 Camera2d一本化・V / Help削除 | B03 green |
| B05 | P02 M3 Building全分類・Bridge・移動 / 状態同期 | B04 green |
| B06 | P02 M4〜M5 Soul billboard・Familiar 3D撤去・Soul shadow動作停止 | B05 green |
| B07 | P03 pure Light Field core | P00後にB01〜B06と並行可。共有Cargo ownerは調整 |
| B08 | P04 runtime snapshot / Door request / schedule / dirty | B03とB07完了 |
| B09 | P05 schema / registry / reset / epoch | B08完了かつsave registry owner条件成立 |
| B10 | P06 Image bridge→Terrain→structural material→native | B02、B06、B08、B09完了 |
| B11 | P07 Soul effect→Room summary→soak | B08、B09完了。P06とはconsumer単位で並行可 |
| B12 | P08 projector→section→mirror cleanup→final artifact | B10、B11完了 |

各batchは少なくともfocused testと`cargo check --workspace`がgreenな独立commitにする。B04のHelp変更、B06 / B10 / B12のvisual変更は各batch内でHelp impact review / native acceptanceまで閉じる。

## 4. 境界契約

### 4.1 crate ownership

| 所有先 | 所有するもの | 所有しないもの |
| --- | --- | --- |
| `hw_world` | world/grid変換、Wall / Door / Room topology、DoorState rule | 照度、GPU Image、Lamp効果 |
| `hw_energy` | demand、allocation、`PowerSupplyState` | 発光半径、色、LOS |
| `hw_infra` | emitter / mount、occlusion snapshot、CPU field、dirty / revision、Room summary | Bevy render asset、UI、camera |
| `hw_visual` | billboard / structural material、shader binding contract | gameplay照度、Door rule |
| `bevy_app` | cross-domain ordering、save adapter、`Assets<Image>` bridge、presentation mapping | pure LOS / accumulation |

`hw_infra` bootstrapはHVAC計画と共有する。先着計画だけがcrate / Pluginを作り、後着計画は既存crateへmoduleを追加する。

### 4.2 update transaction

```text
Logic:
  WorldTopologyMutationSet
  -> ApplyDeferred
  -> Energy settlement
  -> ApplyDeferred
  -> RoomTopologyRefreshSet
  -> ApplyDeferred

Pause gate外 / pre-Visual:
  manual Door request（前frameのInterfaceから受信）

Actor（unpaused時）:
  auto-open -> movement -> auto-close

Pause gate外 / Actor後:
  IndoorLightingUpdateSet

Visual:
  IndoorLightUploadSet（revision / WorldEpoch変更時だけ）
  -> Door / structure presentation sync

Interface:
  UiIntent::ToggleDoorLock -> DoorLockToggleRequest（次updateで適用）
```

- auto Doorは同じvisual frame、manual Doorはpause中も操作の次visual frameまでにfieldと3D visualへ同時反映する。
- InterfaceはDoor / WorldMapを直接変更しない。
- Roomはindoor mask / summaryだけに使い、LOSの正本にしない。
- `WorldMap.obstacle_version`をlight dirtyへ流用しない。

### 4.3 save / rollback

- durable: Building、DoorState、Power policy、optional `FixtureMount`。
- reconstructible: `RadialLightEmitter`、presentation shell。
- derived: occlusion grid、field、Room summary、GPU Image、revision / upload cache。
- 旧saveに`FixtureMount`がなければ`FreeStanding`へ移行する。wall mountはanchor Wallとcardinal interior normalを保存し、推測復元しない。
- world replacement開始時にderived stateをdark / emptyへresetし、既存`WorldEpoch`更新後も旧Imageを有効扱いしない。
- paused load / failed rollbackでも旧worldのlight textureを次frameへ持ち越さない。

## 5. プログラム完了gate

| 観点 | 合格条件 |
| --- | --- |
| RtT | Scene以外のworld color target、mask Camera、mask proxy、mask bindingが0 |
| composite | 通常Scene sample 1回。mask拡張loopなし |
| presentation | 2D world pass 1回。表示分類が全`BuildingType`を網羅し、2D / 3D二重表示なし |
| Soul | Wall depthへ参加するunlit billboard。local light / shadowへ不参加 |
| lighting | Wall / Door / corner / mount sideのpure testが合格 |
| lifecycle | unchanged frameのrecompute / upload 0。load / rollbackはfail-dark。normal / rollback / recovery-onlyが同じnamed traceを通る |
| consistency | gameplay、Room summary、GPU uploadが同じfield revision由来 |
| performance | P00で固定した同一adapter matrixでp95 / p99、pass、RSS hard gateを満たす。p50は診断値として併記する |
| product | V / sectionのHelp、input、runtime、shader残骸がない |
| repository | `python3 scripts/dev.py verify`とnative acceptance artifact検証が成功 |

性能閾値はP00で実装前に本書とP00へ追記し、P08の結果を見て緩和しない。

## 6. 横断リスク

| リスク | 対策 |
| --- | --- |
| 並行中のsave registry実装を上書きする | P05着手時にworktreeとownerを確認し、既存registryへnamed stepを追加する |
| `hw_infra`をHVACと二重作成する | P00でbootstrap ownerを記録し、Cargo / Plugin登録元を1つにする |
| material移行前に`SectionMaterial`を削除する | P06でTopDown material parityとconsumer移行、P08で参照0確認後に削除する |
| UI Doorとauto Doorが別revisionになる | UIはrequest化し、Actorの単一writer経路へ集約する |
| load中に旧GPU bytesが見える | load resetでblack handle / epoch invalidationを同期適用する |
| billboardが透明sortでWallを貫通する | alpha mask + depth writeをnative fixtureで検証する |
| 数値調整で経路不成立を隠す | topology、給電、dirty、revision、bindingの成立を先にtestする |

## 7. ロールバック方針

- 子計画単位で独立commit列にし、親計画全体を一括revertしない。
- P01失敗時はmask関連変更だけを戻し、P03の論理fieldへ影響させない。
- P02 billboardが不合格なら開発中はvisible GLB 1系統へ戻せるが、mask RtTは戻さない。billboard成立までP08 releaseはblockedとする。
- P06 visualが不合格でもP03 / P04 / P05 logical lifecycleは保持し、local visualだけをfeature-offできる境界を残す。
- P07 gameplayが不合格なら旧Lamp effectを一時維持するが、全`PowerConsumer`をLamp扱いする誤りは別correctness fixとして残す。
- `FixtureMount`はdurable schemaなので、P05以後を戻す場合も互換reader / writerまたはmigration shimを残す。
- section / Vを再導入する場合はrollbackではなく別proposalとnative acceptanceを必要とする。

## 8. AI引継ぎメモ

### 現在地

- 進捗: `10%`（P00の実装はC00-D formal baseline採取待ち、P01〜P08は未着手）
- 完了済み: 計画分割、設計契約、Room interior-role correctness、P00 current startup inventory、frozen
  `rtt-light-v1` contract、3規模static / behavior fixture、stable projection / gate row、window / RtT
  environment evidence、S1 / formal native recipe、RenderDoc capture / replay validator、runtime / offline ledger validator
- 未完了: same-source S0 / S1、clean commit上のformal 5 leg、registered baseline index / current gate ledger、P01〜P08

### 次のAIが最初にやること

1. `git status --short`を確認し、直近commit後のfixture / save ownerとP00未コミット差分を分離する。
2. P00 formal要件を満たすclean commitとancestor correctness commitを用意し、S0 / S1を同一source fingerprintで採取する。
3. `plan-rtt-light --level formal`がreadyになった後だけ、native acceptance Skillに従ってcurrent baselineを採取する。
4. P00完了後にのみP01またはP03へ進む。

P00の数値gateは実装前契約として確定済みである。candidate結果を見て同じbaseline generationの閾値を緩和しない。

### ブロッカー/注意点

- save / rehydrate registryは別commitとして進行している。履歴と現worktreeを再確認し、P00から破棄・上書きしない。
- `hw_infra`はまだ存在しない可能性がある。着手時のCargoを正本とする。
- 現行manual Door lockはInterfaceで直接DoorStateを変更する。
- 現行Lamp gameplay queryは任意の`PowerConsumer`を発光扱いし、半径`5.0`をworld unitとして比較している。
- 現行Wallは`SectionMaterial`、Terrainは3種の`TerrainSurfaceMaterial`、他構造物は`StandardMaterial`である。

### 最終確認ログ

- 最終 `cargo check --workspace`: `2026-08-04` / `pass`
- 最終 `cargo clippy --workspace --all-targets -- -D warnings`: `2026-08-04` / `pass (0 warning)`
- 最終 `cargo test --workspace`: `2026-08-04` / `pass`
- 最終 `python3 scripts/dev.py verify`: `2026-08-04` / `pass`
- 最終 docs gate: `2026-08-04` / `pass (docs --write / --check, check_docs, diff --check)`

### Definition of Done

- [ ] P00〜P08が全て完了
- [ ] 全子計画のDefinition of Doneが合格
- [ ] 横断gateと性能gateが合格
- [ ] Help impact reviewとnative acceptanceが完了
- [ ] 恒久docsへ契約を移し、本計画と子計画をarchiveまたは削除

## 9. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-08-05` | `Codex` | P00 contract / behavior / projection / native / RenderDoc実装完了と、formal baseline未採取の環境条件を現在地へ同期 |
| `2026-08-04` | `Codex` | P00の3規模static production fixture、全Building runtime audit、Door初期状態、Tank companion、realtime終端再検証を現在地へ反映 |
| `2026-08-04` | `Codex` | P00 medium/large全Building showcaseのexact contract、size別artifact validator、Bridge completion-only境界を現在地へ反映 |
| `2026-08-04` | `Codex` | P00のRoom / startup inventory、contract / runner / window軸の実装開始と残るnative / RenderDoc blockerを現在地へ反映 |
| `2026-08-04` | `Codex` | P00のstable gate ID、core lane、p50診断 / p95・p99 hard gate区分を統合gateへ同期 |
| `2026-08-04` | `Codex` | P00のC00-A〜C00-D実行順と、実装前に固定済みの数値gateをAI引継ぎへ同期 |
| `2026-08-04` | `Codex` | P00のstable baseline契約とRoom interior-roleのHVAC M0依存を依存表・merge条件へ反映 |
| `2026-08-03` | `Codex` | 統合版を作成 |
| `2026-08-03` | `Codex` | 統合版を9つの独立実装計画へ分割し、本書を依存関係と統合gateの正本へ縮約 |
