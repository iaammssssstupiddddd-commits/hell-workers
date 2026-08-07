# P03: 室内 Light Field ドメインコア計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `single-scene-light-field-03-indoor-light-domain-core-plan-2026-08-03` |
| ステータス | `Draft` |
| 作成日 | `2026-08-03` |
| 最終更新日 | `2026-08-07` |
| 作成者 | `Codex` |
| 親計画 | [`../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md`](../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md) |
| 直接依存 | [P00](../archived/00-baseline-gates-plan-2026-08-03.md)（registered `rtt-light-v1` current formal baseline） |
| 後続 | [P04](04-indoor-light-runtime-integration-plan-2026-08-03.md)、[P05](05-indoor-light-save-lifecycle-plan-2026-08-03.md)、[P06](06-indoor-light-rendering-plan-2026-08-03.md)、[P07](07-indoor-light-gameplay-room-plan-2026-08-03.md) |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: Wall / Door遮光をrendererやECS queryへ埋め込むと、表示・gameplay・Room集計が別々の照度計算を持つ。
- 到達したい状態: 100×100 map-space grid、radial emitter、遮光LOS、照度蓄積、revision判定が`hw_infra`のpureなドメインAPIとして完結する。
- 成功指標: 同じsnapshotから入力順に依存しない同一fieldが得られ、Wall / Door / corner / mount sideの境界条件をheadless unit testだけで再現できる。

## 2. スコープ

### 対象（In Scope）

- `hw_infra::lighting` moduleと、必要なら`hw_infra` crateの単一bootstrap。
- tile単位の光源半径、固定精度の照度・色、遮光grid、室内mask入力、dirty reason、field revision。
- conservative supercover LOSとWall-mounted origin規則。
- deterministicなradial falloff / saturating accumulation。
- snapshot入出力と純粋な計算API、unit / property test。

### 非対象（Out of Scope）

- Bevy ECS query、system set、Door mutation、energy接続（P04）。
- save / load / rollback登録（P05）。
- GPU Image / material / shader（P06）。
- Soul回復、Room summary ECS component（P07）。
- PointLight / SpotLight、soft shadow、家具やSoulの遮光。

## 3. 所有境界とmodule構成

`hw_infra`はHVAC計画と共有する。着手時にworkspaceを再確認し、crateが既に存在すれば新設せず`lighting` moduleだけを追加する。crate bootstrap、root Plugin登録、Cargo依存追加のownerは同時に1作業だけとする。

```text
crates/hw_infra/src/lighting/
  mod.rs            public facadeと不変条件
  components.rs     emitter / mount / field value types
  occlusion.rs      semantic cell snapshot
  los.rs            supercover traversal
  field.rs          falloff / accumulation / revision
```

`systems.rs`、`world_replace.rs`のようなECS adapterはP04 / P05で追加し、P03のpure coreへBevy entityやrender assetを持ち込まない。

| 所有する型 | 契約 |
| --- | --- |
| `LightRadiusTiles` | tile単位を型で固定し、world unitの`f32`と混在させない |
| `LightLevel` / `LightRgbLinear` | `UNORM16` `u16`（0〜65535を0〜1へ対応）のlinear値、加算はsaturating |
| `RadialLightEmitterSnapshot` | grid cell、radius、intensity / color、mountを値で保持 |
| `FixtureMount` | `FreeStanding`または`WallMounted { wall_grid, inward_normal }` |
| `LightOcclusionGrid` | map dimensionsと遮光cellだけを保持 |
| `IndoorMask` | Room等から渡されるcell mask。LOS判定には使わない |
| `IndoorLightField` | row-major cell値、input / output revision、dimensions |
| `IndoorLightDirty` | entity参照なしのbitflags相当reason集合 |

## 4. 計算契約

### 4.1 emitterとmount

- P03は「有効なemitter snapshot」を受け取るだけで、給電判定はP04 adapterの責務とする。
- `FreeStanding`は器具cell中心をray始点とする。
- `WallMounted`は保存されたcardinal `inward_normal`側の隣接cell中心を始点とする。anchor Wallの外側や斜め法線を推測しない。
- wall cell、始点、radiusがmap外、法線が非cardinal、またはP04がanchorをsemantic completed Wallとして検証できなければ、そのemitterはfail-darkとして寄与0にしdiagnostic reasonを返す。
- 同一cellの複数emitterはsource順に依存せず、固定精度でsaturating加算する。

### 4.2 遮光snapshot

P04が次の意味論を構築し、P03はbool / enum gridとして消費する。

| topology | occlusion |
| --- | --- |
| completed Wall | block |
| ProvisionalWall | pass |
| Door Open | pass |
| Door Closed / Locked | block |
| Soul / Familiar / 家具 | pass |
| map外 | block |

`WorldMap.obstacle_version`やnavigation obstacleを照明revisionへ流用しない。光の意味論に無関係な変化でfieldを再構築しないためである。

### 4.3 LOS

- rayはsource cell中心からtarget cell中心へ引き、conservative supercoverで横切る全cellを調べる。
- source cellは判定から除外する。target cellが遮光cellならtarget自体は暗くする。
- grid cornerを厳密に通る場合は、角に接する直交2cellを両方検査し、片方でも遮光なら通さない。
- 浮動小数のepsilonで分岐させず、integer grid traversalまたは同等の再現可能な規則を使う。
- LOSはRoom membershipを参照しない。Door開閉の即時性をRoom検出cooldownへ依存させない。

### 4.4 falloff・field・revision

- 初期falloffはP00契約どおりcell center間Euclidean distanceの`max(0, 1 - distance / radius)`、radius `5 tile`、`distance >= radius`は0、emitter cellは1に固定する。UNORM16変換はround-to-nearest / ties-upとし、shaderで再計算しない。
- indoor mask外のRGB / scalarは0とするが、LOS snapshotとmask revisionは独立して追跡する。
- emitter snapshotはstable keyでsortしてから計算するか、順序不変な固定精度蓄積を用い、ECS iteration順を出力へ漏らさない。
- input dirtyでも再計算結果のbytesが同じなら`field_revision`を増やさない。
- `rebuild_count`、`changed_cell_count`、`last_reason`を計測できる値を返す。logging自体はadapter側の責務とする。

### 4.5 色・scalar・GPU量子化

- field cellはUNORM16 linear RGBと、そこから導出したUNORM16 scalar luminanceを保持する。sRGB値をfieldへ保存しない。
- scalarはRec.709係数を16-bit整数化した`(13933 * r + 46871 * g + 4732 * b + 32768) >> 16`でround-half-upし、0〜65535へclampする。3係数の合計は65536とする。
- GPU u8は`(value * 255 + 32767) / 65535`でround-half-upする。P06はこのpure helperだけを使い、shader / upload側に別変換を持たない。
- `Rgba8Unorm`はlinear unormとしてsampleし、sRGB decode / encodeを挟まない。fixture色がsRGB指定ならsnapshot生成前に共通のsRGB→linear helperで一度だけ変換する。
- indoor alphaはmask内65535 / 外0、GPUでは255 / 0とする。
- P07のgameplay thresholdはUNORM16 scalarへ適用し、threshold値そのものをtyped constantとして保存する。
- RGB→scalar、u16→u8、sRGB→linearの境界値をgolden vectorにし、表示・Room・gameplayが同じcell payloadから派生することをtestする。

## 5. マイルストーン

## M1: crate / public contractを確定する

### 変更内容

1. HVAC側の`hw_infra` bootstrap状態を確認し、単一ownerでcrate / plugin registrationを行う。
2. lighting moduleの公開型、単位、固定精度、dimension上限、invalid input規則を文書化する。
3. `hw_infra`から`hw_world` / `hw_energy`のECS componentを直接queryしない依存方向をCargo testで固定する。

### 主な変更ファイル

- `Cargo.toml`
- `crates/hw_infra/Cargo.toml`
- `crates/hw_infra/src/{lib.rs,lighting/mod.rs,lighting/components.rs}`
- `docs/crate-map.md`

### 完了条件

- [ ] `hw_infra` bootstrap ownerが親計画へ記録されている
- [ ] tile / light valueの単位が型で区別されている
- [ ] public型に`Entity`、`Handle<Image>`、UI型がない

## M2: occlusion snapshotとLOSを実装する

### 変更内容

1. fixed-size row-major occlusion gridを追加する。
2. conservative supercover traversalをpure functionで実装する。
3. corner、start cell、target blocker、map edge、zero radius、wall mount内外をtable testにする。

### 主な変更ファイル

- `crates/hw_infra/src/lighting/{occlusion.rs,los.rs}`

### 完了条件

- [ ] Closed / Locked相当のblockとOpen相当のpassをsnapshot testで表現できる
- [ ] exact cornerの両隣cellを検査する
- [ ] invalid wall mountがpanicせずdarkになる

## M3: deterministic field計算を実装する

### 変更内容

1. radius / falloff / color accumulationを実装する。
2. source列挙順を反転・shuffleしたproperty testを追加する。
3. indoor mask適用とfield byte comparisonを追加する。
4. luminance / GPU量子化のgolden vectorを追加する。
5. P00が予約した`field-core` laneへpure core providerを接続し、100×100 / supplied emitter snapshot 50、32 warmup + 256 measured rebuildを`indoor_light_cpu.csv` schema v1へ出す。ECS collect / uploadはtimerへ含めない。

### 主な変更ファイル

- `crates/hw_infra/src/lighting/field.rs`
- `crates/hw_infra/benches/indoor_light_field.rs`（workspace方針上benchを持つ場合）
- `crates/bevy_app/src/plugins/startup/perf_scenario/field_core_driver.rs`およびP00で追加したlane registration

### 完了条件

- [ ] source順を変えてもfield bytes / revisionが一致する
- [ ] indoor mask外が常に0
- [ ] 100×100のRGB + scalar cell payloadがP00契約どおりlogical `80,000 B`
- [ ] scalar / GPU byte変換がexact expected vectorと一致する
- [ ] `field-core`が1 run 256 row、3 valid runsを生成し、`RLV1-P03-FIELD`を満たす

## M4: dirty / revision APIを閉じる

### 変更内容

1. topology、emitter、power、indoor mask、world replacementを別dirty reasonにする。
2. input revision更新、再計算、output同一時のrevision維持をtestする。
3. consumerがfieldとrevisionをatomicに取得できるsnapshot APIを公開する。

### 完了条件

- [ ] unchanged入力のrebuild要求がない
- [ ] 出力同一の再計算でfield revisionが増えない
- [ ] dirty stateがdespawn済みEntity参照を保持しない

## 6. 検証計画

- pure unit tests: axis / diagonal / exact corner / source-cell / target-cell / OOB / wall mount。
- property tests: emitter順、同一snapshot再計算、saturating境界。
- fixed expected field vectors: 1 emitter、2 overlap、Wall列、L字角、Door gap。
- P00 `field-core` laneによる100×100 / 50 emitter、32 warmup + 256 sample × 3 run artifact検証。
- P00 `stage=p03`のaudit / behavior / Capture / Memory / field-coreを各required case 3反復し、RenderDoc固定1 frameと`RLV1-BUNDLE-VALID`を検証。
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `python3 scripts/dev.py verify`
- `git diff --check`

## 7. リスクと対策

| リスク | 対策 |
| --- | --- |
| `f32`加算でiteration順差が出る | fixed-point / stable accumulationを公開契約にする |
| RoomをLOSへ使いDoor反映が遅れる | Roomはmaskだけ、occlusion gridを独立入力にする |
| wall-mounted originを自動推測して反対室を照らす | cardinal inward normalをdurable値として要求しinvalidはdark |
| HVACとcrate bootstrapが競合する | 着手時にownerを1つにし、同じCargo / plugin変更を並行しない |

## 8. ロールバック方針

- P03はconsumerを接続する前のpure moduleとしてmergeし、P04以降と別commit列にする。
- APIが不合格なら`hw_infra::lighting` moduleだけを戻せる。HVACが共有するcrate bootstrapは戻さない。
- fixed-point形式を変更する場合はfield expected vectorとsave非永続契約を同じ変更で更新する。

## 9. AI引継ぎメモ

### 現在地

- 進捗: `0%`
- 完了済み: 設計分割
- 未着手: M1〜M4

### 次のAIが最初にやること

1. `git status --short`とCargo workspaceを確認し、HVAC側の`hw_infra` ownerを確定する。
2. P00のfield rebuild budgetとfixtureを読む。
3. M1のpure型とexpected field vectorから着手する。

### ブロッカー/注意点

- P03でECS scheduleやsave registryを実装しない。
- `WorldMap.obstacle_version`を便利なdirty sourceとして採用しない。
- radius `5`は5 world unitではなく5 tileとして型を通す。

### 最終確認ログ

- Rust gates: `2026-08-04` / `not run (plan-only update)`
- docs gate: `2026-08-04` / `pass (docs --write / --check, check_docs, diff --check)`

### Definition of Done

- [ ] M1〜M4が完了
- [ ] pure LOS / field testが全境界条件を覆う
- [ ] `RLV1-P03-FIELD`と`RLV1-BUNDLE-VALID`を満たす
- [ ] API / unit / revision contractが恒久docsへ反映済み
- [ ] Help impact reviewが完了

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-08-07` | `Codex` | archive済みP00のregistered current formal baseline locatorへ同期。P03は未着手のDraftを維持 |
| `2026-08-04` | `Codex` | 固定精度・falloff・CPU payloadをP00へ統一し、field-core artifactと`RLV1-P03-FIELD` ownerを確定 |
| `2026-08-03` | `Codex` | 統合計画の論理計算をECS / save / renderから分離して具体化 |
