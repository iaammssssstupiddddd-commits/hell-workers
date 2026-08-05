# P05: 室内 Light Field 保存・再構築 lifecycle計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `single-scene-light-field-05-indoor-light-save-lifecycle-plan-2026-08-03` |
| ステータス | `Blocked by coordination` |
| 作成日 | `2026-08-03` |
| 最終更新日 | `2026-08-04` |
| 作成者 | `Codex` |
| 親計画 | [`../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md`](../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md) |
| 直接依存 | [P03](03-indoor-light-domain-core-plan-2026-08-03.md)、[P04](04-indoor-light-runtime-integration-plan-2026-08-03.md)、[`../../archive/save-rehydration-registry-plan-2026-08-03.md`](../../archive/save-rehydration-registry-plan-2026-08-03.md) |
| 後続 | [P06](06-indoor-light-rendering-plan-2026-08-03.md)、[P07](07-indoor-light-gameplay-room-plan-2026-08-03.md) |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: Light FieldとGPU cacheを保存するとschemaとstale stateが増え、保存しないままreset契約がなければload / rollbackで旧worldの照明が見える。
- 到達したい状態: durableなfixture mountだけを保存し、emitter / occlusion / field / summary / GPU uploadを共通registry経路でfail-darkから再構築する。
- 成功指標: normal load、rollback、recovery-onlyが同一named traceを通り、world replacement開始後に旧field revision / bytesがconsumerから参照できない。

## 2. 着手条件

本計画作成時点では`save/rehydrate` registry関連に別作業の未コミット差分がある。以下のどちらかが成立するまで対象ファイルへ変更を加えない。

1. [`save-rehydration-registry-plan-2026-08-03.md`](../../archive/save-rehydration-registry-plan-2026-08-03.md)が完了しmerge済みである。
2. 同作業ownerと、登録facade・named step・変更commit順を明示的に合意している。

dirty差分をstash、checkout、reset、上書きして着手しない。既存registryを迂回する照明専用rehydrate pathも作らない。

## 3. 永続性分類

| 分類 | data | save方針 |
| --- | --- | --- |
| durable | Building / DoorState / power policy | 既存schemaを正本にする |
| durable（追加） | optional `FixtureMount` | schemaへ追加。旧saveは`FreeStanding` |
| reconstructible | typed `RadialLightEmitter`、presentation shell | BuildingType / fixture definitionからnamed stepで再生成 |
| derived | occlusion grid、indoor mask cache、field bytes / revision、Room summary | 保存しない。reset後にruntimeで再計算 |
| render cache | GPU Image、uploaded revision / epoch | 保存しない。P06 reset consumerがblackにする |

`FixtureMount::WallMounted`は`wall_grid`とcardinal `inward_normal`を保存する。load時に近傍WallやRoomから向きを推測しない。anchor不成立時はfixtureを残しつつ発光0、診断ありとする。

## 4. registry契約

既存phase順を変更せず、rootの狭い登録facadeから次を追加する。

| phase | step ID | 役割 | 依存 |
| --- | --- | --- | --- |
| `DurableNormalize` | `lighting.mount.normalize` | 旧save既定値、cardinal / map bounds検証 | Building / Door durable restore後 |
| `RebuildDerived` | `lighting.emitters.rebuild` | fixture definitionからtyped emitterを再付与 | power-consumer policy、presentation shell後 |
| `WakeDomains` | `lighting.wake` | full dirtyを立て、次のruntime transactionへ渡す | construction / obstacle runtime rebuild後 |

既存named step例は`construction.normalize`、`power-consumer.policy`、`presentation.shells`、`construction.runtime`、`obstacle.runtime`、`domains.wake`である。実装時のregistry APIとstep名を正本として依存edgeを登録し、文字列の暗黙sortへ依存しない。

- rehydrate中にfieldを計算しない。energy allocationがLogicでsettleする前に計算すると未給電状態を一時的に正本化するためである。
- `lighting.wake`はfieldをfull dirtyにするだけで、通常runtime再開後のP04 transactionがSupplied emitterを収集する。
- registry APIがsave module privateなら、root pluginに全domain登録を集約する狭いfacadeを追加する。照明側からsave internalsを公開しない。

## 5. world replacement / fail-dark

`hw-infra-lighting`のreset hookを既存world replacement inventoryへ登録する。

1. replacement開始時にCPU field / input snapshots / dirty cacheをempty / darkへresetし、derived consumer用のepoch reset hookを発火する。Room summaryはP07、GPU cacheはP06が各owner stageで同hookへ登録する。
2. `WorldEpoch`変更を記録し、旧epochのfield snapshot取得を拒否する。
3. P06のGPU bridgeへblack clear / uploaded revision invalidationを通知する。
4. durable restoreとregistry traceが成功した後だけ`lighting.wake`を立てる。
5. rollback / recovery-onlyも同じhookとtraceを通す。

`RecoveryFailed`は暗いまま保持し、自動unpauseしない。失敗時に旧world bytesへfallbackしない。

candidate / registry preflightがlive reset開始前にrejectされた場合はworld replacementではないため、現行worldとその照明を一切変更しない。fail-darkはreset開始後に旧worldを破棄したtransactionだけへ適用する。

## 6. マイルストーン

## M1: schemaとmigrationを追加する

### 変更内容

1. `FixtureMount`をReflect可能なpersisted componentとして定義し、`save/schema.rs`のDynamicWorld allow-list / type registrationへ追加する。
2. `saving.rs`の既存`build_persisted_world`経路でBuilding rootのmountだけが保存され、field / emitter / cacheが除外されることをschema testで固定する。
3. 旧saveにはcomponent自体が存在しないため、`lighting.mount.normalize`がOutdoorLamp等の対象fixtureへ`FreeStanding`を補う。header / body用の別DTOやoptional fieldを新設しない。
4. invalid wall mountはloadを推測修復せず、rehydrate後のsemantic validationでfail-darkとするmigration testを追加する。

### 主な変更ファイル

- `crates/hw_infra/src/lighting/components.rs`
- `crates/bevy_app/src/systems/save/{schema.rs,saving.rs,load.rs}`
- `crates/bevy_app/src/systems/save/schema/tests.rs`
- `crates/bevy_app/src/systems/save/rehydrate/`のmount normalize step / tests

### 完了条件

- [ ] field / revision / GPU handleがsave payloadにない
- [ ] 旧saveをloadするとFreeStanding emitterへ移行できる
- [ ] invalid mountを推測修復せずdarkにする

## M2: named rehydrate stepを登録する

### 変更内容

1. current registry facadeへ3 stepを登録する。
2. explicit dependency edgeとtrace expectationを追加する。
3. normal / rollback / recovery-onlyで同じordered traceを検証する。

### 主な変更ファイル

- `crates/bevy_app/src/systems/save/rehydrate/registry.rs`
- `crates/bevy_app/src/systems/save/rehydrate/task_runtime.rs`
- `crates/bevy_app/src/systems/save/rehydrate/`のroot registration
- `crates/bevy_app/src/plugins/logic.rs`（登録ownerがrootの場合）

### 完了条件

- [ ] parallelな照明rehydrate loopがない
- [ ] step ID重複 / missing dependencyがfail-closedになる
- [ ] field計算はrehydrate trace内で0回

## M3: reset hookとepochを接続する

### 変更内容

1. CPU lighting reset hookをinventoryへ登録する。
2. Door requestとlighting dirty / revisionをclearする。未導入のRoom summaryをP05から参照しない。
3. P06 / P07のderived consumerが後から登録できるblack-clear / epoch invalidation contractをresource / eventで公開する。

### 主な変更ファイル

- `crates/hw_infra/src/lighting/world_replace.rs`
- `crates/bevy_app/src/systems/save/world_replace/`
- `crates/bevy_app/src/plugins/messages.rs`
- lighting reset inventory tests

### 完了条件

- [ ] replacement開始直後のfield readがdark
- [ ] old epoch snapshotをconsumerが受理しない
- [ ] reset hookがidempotent

## M4: lifecycle failure matrixを閉じる

### 変更内容

次の各caseでphase trace、pause状態、field / GPU expected stateをtable testにする。

| P00 behavior case ID | expected field | expected wake | pause |
| --- | --- | --- | --- |
| `load-preflight-reject-v1` | live worldのfield / epoch不変 | 0 | 不変 |
| `load-normal-v1` | reset中dark、再開後rebuild | 1 | 元契約に従う |
| `load-rollback-v1` | reset中dark、rollback worldからrebuild | 1 | 元契約に従う |
| `load-recovery-only-v1` | reset中dark、recovery worldからrebuild | 1 | 元契約に従う |
| `load-recovery-failed-v1` | darkを維持 | 0 | 自動解除しない |
| `load-duplicate-reset-v1` | 2回目reset後もdark、restore後に1回だけrebuild | coalesced `1` | 変化なし |

### 完了条件

- [ ] 全caseが同じregistry / reset inventoryを通る
- [ ] load前worldのrevision / bytesが1frameも再利用されない
- [ ] stale Door request / lighting snapshot参照が残らない。Room summaryのstale参照0はP07が同hookへ登録して閉じる
- [ ] P00 behavior schemaのepoch / read / dark / wake / checksum列が揃い、`RLV1-P05-LIFECYCLE`を満たす

## 7. 検証計画

- old/current save fixture round-trip tests。
- registry dependency / trace / duplicate registration tests。
- normal / rollback / recovery-only / failure lifecycle tests。
- P00 behavior 6 lifecycle case × 3 runのoffline schema / gate検証。
- P00 `stage=p05`のaudit / behavior / Capture / Memory / field-core各required run + RenderDoc固定1 frameの共通validity検証。
- double reset / partial restore / invalid mount tests。
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `python3 scripts/dev.py verify`
- P06接続後のnative load / rollback fail-dark acceptance。
- Help impact review。

## 8. リスクと対策

| リスク | 対策 |
| --- | --- |
| 並行中registry差分を上書きする | 完了またはowner合意を着手条件にする |
| rehydrate中の未settled powerでfieldを作る | WakeDomainsではdirtyだけ立て、runtime Logic後に計算する |
| old GPU textureだけ残る | CPU resetと同時にepoch invalidation契約を発行する |
| wall mountをload時推測して向きが変わる | mount normalをdurableにしinvalidはfail-dark |
| rollbackだけ別pathになる | normal / rollback / recovery-onlyのtraceを同一tableで固定する |

## 9. ロールバック方針

- `FixtureMount`を一度writerへ追加した後は、機能rollbackでもreader / writer compatibility shimを残す。
- emitter / fieldはreconstructible / derivedなので、registry step registrationを外せば暗い状態で安全に停止できる。
- reset hookは旧field表示を防ぐcorrectness境界のため、P06だけを戻す場合も保持する。

## 10. AI引継ぎメモ

### 現在地

- 進捗: `0%`
- 完了済み: persistence分類とregistry step設計
- 未着手: M1〜M4

### 次のAIが最初にやること

1. `git status --short`を確認する。
2. save registry計画の完了 / owner合意を確認する。
3. current registry phase APIとworld replacement inventory testを読み、本文のstep依存を実装APIへ写す。

### ブロッカー/注意点

- 本計画作成時点のsave / rehydrate dirty差分は別作業に属する。
- schemaへCPU field / Room summary / GPU bytesを追加しない。
- `lighting.wake`内でenergy settlementを迂回しない。

### 最終確認ログ

- Rust gates: `2026-08-04` / `not run (plan-only update)`
- native acceptance: `2026-08-04` / `not run (plan-only update)`
- docs gate: `2026-08-04` / `pass (docs --write / --check, check_docs, diff --check)`

### Definition of Done

- [ ] 着手条件を満たしている
- [ ] M1〜M4が完了
- [ ] normal / rollback / recovery-onlyが同じtraceを通る
- [ ] `stage=p05`のexact gate ID集合を満たす
- [ ] fail-darkとepoch invalidationがheadless / nativeで確認済み
- [ ] Help impact reviewと恒久save docs更新が完了

## 11. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-08-04` | `Codex` | P00 behavior case / lifecycle gateへ同期し、Room summary reset登録をP07 ownerへ修正 |
| `2026-08-03` | `Codex` | runtime統合から保存・rollback lifecycleを分離し、現行registryへの接続点を具体化 |
