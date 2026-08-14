# P03: 室内 Light Field ドメインコア計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `single-scene-light-field-03-indoor-light-domain-core-plan-2026-08-03` |
| ステータス | `Implementation complete / formal evidence pending` |
| 作成日 | `2026-08-03` |
| 最終更新日 | `2026-08-14` |
| 作成者 | `Codex` |
| 親計画 | [`../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md`](../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md) |
| 直接依存 | [P00](00-baseline-gates-plan-2026-08-03.md)。P03 stage の evidence 有効化は完了済みの [P02-A](02a-p02-acceptance-infrastructure-plan-2026-08-12.md) extension point を利用する |
| 後続 | [P04](04-indoor-light-runtime-integration-plan-2026-08-03.md)、[P05](05-indoor-light-save-lifecycle-plan-2026-08-03.md)、[P06](06-indoor-light-rendering-plan-2026-08-03.md)、[P07](07-indoor-light-gameplay-room-plan-2026-08-03.md) |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: Wall / Door遮光をrendererやECS queryへ埋め込むと、表示・gameplay・Room集計が別々の照度計算を持つ。
- 到達したい状態: 100×100 map-space grid、最大50 radial emitter、遮光LOS、照度蓄積、mask、revisionが`hw_infra`のpureなドメインAPIとして完結する。
- 成功指標: 同じ正規化snapshotから入力順・CPU・OSに依存しない同一fieldが得られ、Wall / Door / corner / mount sideの境界条件をheadless unit testだけで再現できる。
- P03固有の完了物: P00で凍結済みの`p03 / field-core`を、P02-Aで完成したacceptance extension pointへ接続し、pure coreだけを測る正式bundleを生成・再検証できる。

## 2. スコープ

### 対象（In Scope）

- `hw_infra::lighting` moduleと`hw_infra` crateの初期bootstrap。
- tile単位の光源半径、固定精度の照度・色、semantic occlusion grid、室内mask入力、field revision。
- conservative integer supercover LOSとWall-mounted origin規則。
- deterministicなradial falloff / stable-order saturating accumulation。
- radiance・mask・checksum・revisionを同時に返すimmutable snapshot、pure CPU/GPU pack境界、unit / property test。
- 100×100 / supplied emitter 50 / radius 5のcanonical pure fixtureと`field-core` driver。
- P03 selector、artifact、bundle、RenderDoc/native helper、validatorのextension。

### 非対象（Out of Scope）

- Bevy ECS component、query、system set、Door mutation、energy接続、dirty reason、rebuild scheduling（P04）。
- save / load / rollback登録（P05）。
- GPU Image / material / shader（P06）。
- Soul回復、Room summary ECS component（P07）。
- PointLight / SpotLight、soft shadow、家具やSoulの遮光。

## 3. 所有境界とmodule構成

P03は`hw_infra`の最初のbootstrap ownerである。workspace memberは`crates/*` wildcardのため、root`Cargo.toml`のmember追加は不要と確認する。P03 M1のdirect dependencyは`hw_core`、`serde`、`sha2`だけとし、`bevy_app`、Bevy ECS/GPU、`hw_world`、`hw_jobs`、`hw_energy`を追加しない。

HVACはP03後に同じcrateを拡張できる。crate全体の将来依存とP03 coreの境界を混同しない。P03が作る`lighting`のpure core source（値型、occlusion、LOS、field、packing、fixture）は恒久的にECS/GPU非依存とする。P04/P05は必要なら同じ`lighting`配下へadapter moduleを追加できるが、pure coreからBevy/world依存を逆流させない。

```text
crates/hw_infra/src/lighting/
  mod.rs            public facadeと値型
  occlusion.rs      semantic cell snapshot
  los.rs            integer supercover traversal
  field.rs          falloff / accumulation / revision / checksum
  packing.rs        pure RGBA8 pack helper
  fixture.rs        canonical pure field-core fixture
```

ECS adapter、`IndoorLightDirty`、`systems.rs`、`world_replace.rs`はP04 / P05で追加し、P03のpure coreへBevy entityやrender assetを持ち込まない。

| 領域 | P03の責務 | 後続の責務 |
| --- | --- | --- |
| crate | `hw_infra`の初期bootstrap、pure lighting core | HVACが既存crateを拡張。ただしP03 coreのpure境界は維持 |
| input | 正規化済み値型の検証、invalid emitterのfail-dark | P04がECS/world snapshotとstable keyを作る |
| field | LOS、合成、mask、payload、checksum、公開revision | P04がdirty / input revision / rebuild transactionを管理 |
| lifecycle | immutable field snapshotを返すだけ | P05がepoch / world replacementを管理 |
| GPU | `pack_rgba8_linear` helperのみ | P06が`Image`、upload、material、shaderを所有 |
| consumer | fieldの型とrevision契約を公開 | P07がgameplay / Room / AI consumerを実装 |
| evidence | `p03 / field-core`とP03 bundle integration | P04以降が各自のstage / metricを追加 |

P03の公開型は、少なくとも`GridDimensions`、`LightGridPos`、`LightRadiusTiles`、`LightRgbLinear`、`RadialLightEmitterSnapshot`、`FixtureMount`、`LightOcclusionGrid`、`IndoorMask`、`FieldSnapshot`、`RebuildOutcome`、stable diagnosticを持つ。`Entity`、`Handle<Image>`、UI型、`IndoorLightDirty`はP03 public APIへ持ち込まない。

## 4. 計算契約

### 4.1 gridとemitter入力

- `GridDimensions`は幅・高さをそれぞれ`1..=MAP_WIDTH` / `1..=MAP_HEIGHT`、checked areaを10,000以下に制限する。100×100はbenchmark canonical sizeであり、unit testは小さいgridを用いてよい。row-major indexは`index = y * width + x`とする。
- `IndoorMask`と`LightOcclusionGrid`はgrid areaと完全に一致する長さを要求する。mask byteはrow-majorの`0x00`（外）/`0x01`（内）だけ、occlusion cellはstable one-byte discriminantだけでserializeする。長さ不一致・checked overflowはallocation前に`Err`とし、panic・暗黙truncate・補完を行わない。
- `LightGridPos`へ変換済みの値だけをcoreへ渡す。`hw_core::GridPos`からのchecked conversionはP04 adapter境界で行い、ゲーム世界の生座標をP03の範囲検証へ混ぜない。
- P03は「正規化済みemitter snapshot」を受け取るだけで、給電判定はP04 adapterの責務とする。
- `RadialLightEmitterSnapshot`は`EmitterStableKey`（`u64`）、free-standing source cellまたはwall anchorとinward cell、radius（tile）、linear RGB `u16`、intensity `u16`を持つ。coreはEntity IDを受け取らず、emitter IDをfield payloadに保存しない。
- emitterは`EmitterStableKey`昇順で評価する。同一keyは入力全体を`Err`とする。
- `FreeStanding`は器具cell中心をray始点とする。`WallMounted`は保存されたcardinal`inward_normal`側の隣接cell中心を始点とし、anchor Wallの外側や斜め法線を推測しない。
- radiusが0、source/inward cellがmap外、free-standing sourceがblocker上、wall anchorがcompleted Wallでない、inward cellがcardinal neighborでない・map外・blocker上のいずれもemitter-local invalid inputとする。該当sourceはfail-dark（寄与0）にし、stable key順のdiagnosticを返す。別の壁・原点・隣接cellを推測して選ばない。
- coreはloggingをしない。P04 adapterが必要に応じてlogging / telemetryを担当する。

### 4.2 遮光snapshot

P04が次の意味論を構築し、P03はboolではなくsemantic enum gridとして消費する。

| cell | canonical byte | LOSを遮る | wall-mounted anchorとして有効 |
| --- | ---: | ---: | ---: |
| `Clear` | `0` | no | no |
| `ProvisionalWall` | `1` | no | no |
| `OpenDoor` | `2` | no | no |
| `CompletedWall` | `3` | yes | yes |
| `ClosedDoor` | `4` | yes | no |
| `LockedDoor` | `5` | yes | no |

Soul / Familiar / 家具は`Clear`へ投影する。map外はLOS上のblocker扱いだが、anchorにはできない。P03は任意のblockerをWallと見なしてmountを許可してはならない。

`WorldMap.obstacle_version`やnavigation obstacleを照明revisionへ流用しない。光の意味論に無関係な変化でfieldを再構築しないためである。

### 4.3 LOS

- source/targetがmap外ならfalse、targetがblockerならfalseとする。source自身はtraversalから除外する。axis方向は単純走査する。
- 非axis方向はinteger supercover traversalとし、各stepで`(2 * ix + 1) * abs_dy`と`(2 * iy + 1) * abs_dx`を比較する。左辺が小さければxを一歩、右辺が小さければyを一歩進めて検査する。
- 二値が等しいcorner crossingでは、対角へ進む前にx側・y側の二つのside cellをともに検査する。いずれかがblockerまたはmap外ならfalseとし、その後で対角cellを検査する。
- float、epsilon、platform固有丸めを分岐に入れない。7×7 gridの独立reference traversalと全single-blocker placementで照合し、axis / diagonal / corner / target blocker / OOB / L字doorwayをtable testにする。
- LOSはRoom membershipを参照しない。Door開閉の即時性をRoom検出cooldownへ依存させない。

### 4.4 falloff・field・revision

- source/targetのcenter差分を整数`dx`、`dy`とする。距離は`distance_q16 = isqrt((dx² + dy²) << 32)`（floor Q16、checked`u128` intermediate）、`radius_q16 = radius_tiles << 16`とする。`distance_q16 >= radius_q16`なら寄与0、emitter cellはfull strengthに固定する。
- falloffは`round_half_up((radius_q16 - distance_q16) * 65535 / radius_q16)`で固定する。各channelは`mul_unorm16(a, b)`を明示的に定義し、`color * intensity`、続いて`* falloff`の順に毎回round-half-upする。
- emitter寄与をstable key順に`u32`で非負のsaturating accumulationし、最終RGBを`u16`へclampする。ECS iteration順を出力へ漏らさない。
- `IndoorMask`はLOS blockerではない。mask外sourceはmask内targetに寄与してよいが、mask外targetのRGB / scalarは常に0とする。
- canonical radiance payloadは各cellを`[r, g, b, luminance]`のlittle-endian`u16`でrow-majorに連結する。1 cellは8 byte、100×100は厳密に80,000 byteであり、mask byteはradiance payloadに含めない。
- checksumはすべてSHA-256とする。`radiance_checksum`はradiance payload、`mask_checksum`はrow-major mask byte、`field_checksum`はASCII `P03-field-v1` prefix + dimensionsのlittle-endian bytes + radiance payload + mask byteで計算する。native endianやstruct paddingをhash対象にしない。
- この`mask_checksum`はfield payload用のraw byte digestであり、P00 audit sidecarの`indoor_mask_checksum`（canonical JSONのfixture semantic digest）を置換しない。両者は用途とencodingが異なる。
- `input_checksum`はASCII `P03-input-v1` prefix + dimensionsのlittle-endian bytes + row-major mask byte + row-major occlusion discriminant + stable-key順emitter recordのcanonical bytesに対するSHA-256とする。emitter recordは`stable_key: u64 LE`、`mount_kind: u8`（free=0 / wall=1）、`origin_x/origin_y: i32 LE`、`anchor_x/anchor_y: i32 LE`（freeなら0）、`inward_x/inward_y: i8`（freeなら0）、`radius_tiles: u16 LE`、`r/g/b/intensity: u16 LE`の順に固定する。P03のfield-core artifactはこの`input_checksum`とoutput側checksumを必ず出す。
- coreは`RebuildOutcome { snapshot: FieldSnapshot, input_checksum, diagnostics }`を返す。`FieldSnapshot`はradiance payload、row-major mask、radiance/mask/field checksum、`field_revision`、`changed_radiance_cells`、`changed_mask_cells`、`changed_cell_count`を一体で持つ。`changed_cell_count`はradianceまたはmaskの少なくとも一方が変わったcell数とする。
- pure rebuildは`previous: Option<&FieldSnapshot>`を明示入力にする。`None`はall-zero radiance / all-zero mask baselineとし、最初のsnapshotの`field_revision`は1とする。public bytesが変わればprevious revisionをchecked incrementし、overflowはwrapせず`Err`とする。bytesが同じならprevious revisionを維持する。P04がinput revision / rebuild count / last reasonを持ち、P03はdirty reason、epoch、world identityを保持しない。

### 4.5 色・scalar・GPU量子化

- field cellはUNORM16 linear RGBと、そこから導出したUNORM16 scalar luminanceを保持する。sRGB値をfieldへ保存しない。将来sRGB入力が必要ならsnapshot生成前のadapter境界で、固定test済みの共通sRGB→linear helperを一度だけ使う。
- scalarはRec.709係数を16-bit整数化した`(13933 * r + 46871 * g + 4732 * b + 32768) >> 16`でround-half-upし、0〜65535へclampする。3係数の合計は65536とする。
- `pack_rgba8_linear`はGPU resourceを作らないpure helperとする。RGBは`(value * 255 + 32767) / 65535`でround-half-upし、alphaはmask内255 / 外0とする。100×100のpacked outputは40,000 byteである。
- P06はこのpack契約を使い、shader / upload側に別のCPU色変換を持たない。`Rgba8Unorm`をlinearとしてsampleするかどうかを含むrender asset / shaderの選択はP06の責務とする。
- P07のgameplay thresholdはUNORM16 scalarへ適用し、threshold値そのものをtyped constantとして保存する。
- RGB→scalar、u16→u8、sRGB→linearの境界値、payload endianness、mask-only change時のalpha更新をgolden vectorにし、表示・Room・gameplayが同じcell payloadから派生することをtestする。

## 5. マイルストーン

## M1: crate bootstrapとpublic contractを確定する

### 変更内容

1. P03を`hw_infra`の最初のbootstrap ownerとしてparent / HVAC計画へ同期する。workspace wildcardを確認し、root member変更を不要とする。
2. `hw_infra`に`hw_core`、`serde`、`sha2`だけをdirect dependencyとして追加し、`lighting` public facadeとpure coreの値型を作る。
3. `GridDimensions`、`LightGridPos`、`IndoorMask`、semantic occlusion、emitter snapshot、diagnostic、`FieldSnapshot`を定義し、checked validationをtestで固定する。
4. durable wall mount用の値型はserde可能にするが、P05のEntity / lifecycle所有権を取り込まない。
5. `docs/cargo_workspace.md`、`docs/crate-boundaries.md`、`docs/architecture.md`、新規`docs/indoor_lighting.md`にcrateの目的、dependency direction、P04/P06との境界を追記する。存在しない`docs/crate-map.md`は更新対象にしない。

### 主な変更ファイル

- `crates/hw_infra/{Cargo.toml,README.md,src/lib.rs,src/lighting/**}`
- `docs/{cargo_workspace.md,crate-boundaries.md,architecture.md,indoor_lighting.md}`
- [HVAC/Plumbing計画](../../hvac-plumbing-plan-2026-07-13.md)

### 完了条件

- [x] P03が作るpure core sourceにBevy ECS/GPU type/importがない。後続adapterはcore APIを一方向に利用する
- [x] invalid dimension / mask / occlusion inputがallocation前に`Err`になりpanicしない
- [x] HVAC計画が「P03がcrateを新設、HVACは既存crateを拡張」と明記する

## M2: semantic occlusion、mount validation、整数LOSを実装する

### 変更内容

1. semantic cellのblocker / valid wall-anchor規則をAPIとtest tableにする。
2. 4.3のexact integer supercover traversalをpure functionで実装する。cornerは二つのside cellを先に検査する。
3. free-standingとwall-mounted emitterのvalidationを実装し、invalid emitterがstable diagnosticとzero contributionになることを固定する。
4. 7×7のindependent referenceと全single-blocker placementでLOSを照合する。

### 主な変更ファイル

- `crates/hw_infra/src/lighting/{occlusion.rs,los.rs}`
- 対応するunit test

### 完了条件

- [x] axis、diagonal、corner、target blocker、source/target OOB、L字doorway、Door stateの表形式testがある
- [x] `CompletedWall`だけがwall-mounted anchorとして通り、`ClosedDoor` / `LockedDoor`はLOS blockerでもanchorにならない
- [x] traversalにfloat・epsilon・iteration-order依存がない

## M3: deterministic field、revision、pack helperを実装する

### 変更内容

1. Q16 distance/falloff、`mul_unorm16`、stable-key ordering、saturating RGB accumulation、luminanceを実装する。
2. radiance / mask / checksum / revision / diff countを一つのimmutable snapshotとして生成する。
3. canonical 100×100 / 50 emitter / radius 5 fixtureをpure value builderとして定義する。ECS queryやprivateな`IndoorLightLayout`をtiming pathに使わない。
4. P06 handoff用`pack_rgba8_linear`を実装し、payload size・endianness・alpha規則をfixed vectorで固定する。

### 主な変更ファイル

- `crates/hw_infra/src/lighting/{field.rs,packing.rs,fixture.rs}`
- 対応するunit test

### 完了条件

- [x] canonical fieldは80,000 byte、packed RGBA8は40,000 byte
- [x] emitter inputのpermutation後もstable keyが同じならfield bytes / checksumが同じ
- [x] fixed vectorがQ16 distance、falloff、RGB、luminance、saturationをbyte単位で拘束する
- [x] input/radiance/mask/field SHA-256がcanonical bytesから再現でき、host endian / struct layoutに依存しない
- [x] first snapshot、radiance-only、mask-only、両方、bytes不変、revision overflowの各caseで`field_revision`と`changed_*`規則を検証する

## M4: P03 formal evidenceとacceptance extensionを閉じる

### 変更内容

1. P02-Aのextension pointを使い、`p03`と`field-core` laneをRust selector、Python CLI、native helper、RenderDoc helper、bundle/revalidatorに追加する。既存`current` / `p01` / `p02` schemaと意味を変更しない。
2. `field_core_driver`はcanonical pure fixtureをtimer外で構築し、core invocationだけを測る。ECS collection、GPU upload、CSV書き込み、artifact serializationをtimerに入れない。
3. P00 contractの32 warmup、256 measured row、600 steady frame、formal 3 runをそのまま実装する。measured CSVはwarmupを含めず、row index、duration、input/radiance/mask/field checksum、grid cells、payload bytes、emitter count、radius、allocation scopeを出力する。
4. P03 stageのstatic / behavior / capture / memory / RenderDocはP02のscene evidenceを維持する。P03はP06のGPU field resource/image/upload証跡を要求・偽装しない。
5. frozen`rtt_light_migration_v1.json`の意味・hash・thresholdを変更しない。既存P03 entryを有効化するだけとし、schema/threshold変更が必要ならP00とv2/rebaselineの判断へ戻す。
6. pre-P03 field-core reject、lane/stage mismatch、256 row欠落・重複・順序違い、checksum mismatch、不正duration、mixed schema、historical stage再検証のpositive/negative tool testを追加する。

### 主な変更ファイル

- `crates/bevy_app/src/plugins/startup/perf_scenario/{config.rs,config/tests.rs,field_core_driver.rs,mod.rs,output.rs}`
- `crates/bevy_app/Cargo.toml`
- `scripts/perf_tool/{arguments.py,policy.py,artifacts.py,rtt_light_bundle.py,rtt_light_contract.py,fixtures.py,renderdoc_capture.py}`
- `.codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py`
- `docs/performance-profiling.md`とP03の実装/evidence docs

### 完了条件

- [ ] `p03 / field-core` formal bundleが`RLV1-P03-FIELD`と`RLV1-BUNDLE-VALID`を通る
- [x] 19 formal caseは既存18 caseと新規field-core 1 caseであり、256×3 raw measurement rowと取り違えない
- [x] `field_rebuild_allocation`がfield-core artifactに存在し、範囲・scopeがvalidatorで検証される
- [x] `p03`が許可されてもP06のGPU evidence要件は導入されない
- [ ] actual native runはrepositoryの`hell-workers-run-native-acceptance` skillのno-prompt launcherを使い、artifact存在とbundle validationをfail-closedで確認する

## 6. 検証計画

- pure unit: grid dimension、mask/occlusion length、OOB、duplicate stable key、invalid mount。
- LOS: 7×7 exhaustive single-blocker reference、axis / diagonal / corner / doorway table。
- fixed vector: Q16 arithmetic、falloff boundary、round-half-up、saturation、luminance、payload / pack / checksum。
- determinism: stable-key permutation、radiance-only / mask-only / both / no-byte-change revision。
- evidence: P03 selector、field-core CSV/artifact parser、bundle/revalidation、historical stage compatibility。

```bash
python3 scripts/dev.py check
python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings
python3 scripts/dev.py cargo -- test --workspace
PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py validate-rtt-light-contract --contract rtt-light-v1 --stage p03 --lane field-core
PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py self-test
python3 scripts/dev.py verify
git diff --check
```

P03のactual formal/native recipeは、M4で`p03` selectorをnative helperに実装した後、repositoryのnative acceptance skill経由で実行する。任意の`cargo run`や手作業artifactは正式証跡に代えない。

## 7. リスクと対策

| リスク | 対策 |
| --- | --- |
| `f32`加算でiteration順差が出る | fixed-point / stable accumulationを公開契約にする |
| RoomをLOSへ使いDoor反映が遅れる | Roomはmaskだけ、occlusion gridを独立入力にする |
| wall-mounted originを自動推測して反対室を照らす | cardinal inward normalをdurable値として要求しinvalidはdark |
| bool occlusionではDoorとcompleted Wallを区別できない | semantic cellとmount-validity tableをP03 APIに含める |
| mask-only changeがGPU alphaに届かない | radianceとmaskをatomic snapshotにし、いずれのbyte changeでもpublic revisionを進める |
| benchmarkがECS/GPU/CSVを測ってしまう | pure fixtureをtimer外で作り、core invocationのみをtiming scopeにする |
| HVACとcrate bootstrapが競合する | P03をbootstrap ownerと明記し、HVACは既存crateを拡張する計画へ同期する |
| frozen P00 contractを再定義してしまう | P03はcontract entryをactivationするだけとし、変更はv2/rebaselineの明示判断に戻す |
| P03がP04/P05/P06の責務を吸収する | ownership tableとmilestone acceptanceでboundaryをfail-closedにする |

## 8. ロールバック方針

- P03 coreはconsumerを接続する前のpure moduleとしてmergeし、P04以降と別commit列にする。
- APIが不合格なら`hw_infra::lighting` moduleだけを戻せる。HVACが共有するcrate bootstrapは戻さない。
- fixed-point形式またはserialized payloadを変更する場合はfield expected vector、P00 contract、consumer handoffへの影響を同じレビューで明示する。

## 9. Help impactとAI引継ぎメモ

### Help impact

P03実装後、player-visibleなHelp導線・操作・設定・通知・runtime dataに影響があるかを、実際の変更経路で`hell-workers-review-help-impact` skillにより判定する。pure coreであることを理由に実装前からNo impactと決め打ちしない。

### 現在地

- 進捗: `90%`
- 完了済み: M1〜M3、M4のselector/driver/artifact/bundle/native helper実装、unit/tool/full verification、Help no-impact review
- 残作業: cleanかつcommit済みsubjectに対するP03 formal native bundleの採取・登録

### 次のAIが最初にやること

1. 実装差分をreviewし、commitする場合はHelp no-impact trailerを付けてcleanなsubjectを作る。
2. repositoryのnative acceptance skillで`p03` formal jobを実行する。
3. `RLV1-P03-FIELD`と`RLV1-BUNDLE-VALID`、19 case、binary/source checkpointをartifactから再検証する。

### ブロッカー/注意点

- P03でECS scheduleやsave registryを実装しない。
- `WorldMap.obstacle_version`を便利なdirty sourceとして採用しない。
- radius `5`は5 world unitではなく5 tileとして型を通す。
- Bevy 0.19 APIが必要なのはM4の既存perf scenario接続だけである。新しいAPIはlocal sourceまたはdocs.rsの一次情報で確認する。
- temporary probe / loggingをcoreに残さない。diagnosticは戻り値にし、P04 adapterが観測を担当する。

### 最終確認ログ

- Rust gates: `2026-08-14` / `pass (hw_infra unit 16、workspace check、all-target Clippy -D warnings、verify)`
- tooling gates: `2026-08-14` / `pass (perf self-test、P03 contract validator、native helper self-test、profiling/renderdoc compile)`
- docs/Help gates: `2026-08-14` / `pass (docs --write/check、Help No impact、verify、diff --check)`
- formal native: `2026-08-14` / `pending (clean committed subject required; current implementation worktree is intentionally uncommitted)`

### Definition of Done

- [ ] M1〜M4が完了
- [x] pure LOS / field testが全境界条件を覆う
- [ ] `RLV1-P03-FIELD`と`RLV1-BUNDLE-VALID`を満たす
- [x] API / unit / revision contractが恒久docsへ反映済み
- [x] Help impact reviewが完了

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-08-14` | `Codex` | pure lighting core、canonical fixture、P03 field-core計測/artifact/bundle/native extension、恒久docsを実装。全quality gateとHelp No impact reviewを完了し、formal native evidenceだけをclean subject待ちとして残した |
| `2026-08-13` | `Codex` | `hw_infra` bootstrapのP03所有、semantic occlusion / exact LOS / payload-revision契約、P03 formal evidenceの実装境界を具体化。HVAC・親計画との責務競合を解消 |
| `2026-08-04` | `Codex` | 固定精度・falloff・CPU payloadをP00へ統一し、field-core artifactと`RLV1-P03-FIELD` ownerを確定 |
| `2026-08-03` | `Codex` | 統合計画の論理計算をECS / save / renderから分離して具体化 |
