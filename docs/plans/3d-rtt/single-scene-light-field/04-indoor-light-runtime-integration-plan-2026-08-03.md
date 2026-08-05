# P04: 室内 Light Field 実行時統合計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `single-scene-light-field-04-indoor-light-runtime-integration-plan-2026-08-03` |
| ステータス | `Draft` |
| 作成日 | `2026-08-03` |
| 最終更新日 | `2026-08-04` |
| 作成者 | `Codex` |
| 親計画 | [`../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md`](../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md) |
| 直接依存 | [P03](03-indoor-light-domain-core-plan-2026-08-03.md)、P02 M1 Door domain mutation、HVAC M0または同等のRoom interior correctness commit |
| 後続 | [P05](05-indoor-light-save-lifecycle-plan-2026-08-03.md)、[P06](06-indoor-light-rendering-plan-2026-08-03.md)、[P07](07-indoor-light-gameplay-room-plan-2026-08-03.md) |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: completed Wall、Door、給電、Room、actor移動は異なるsystem setで更新され、現状のmanual DoorはInterfaceから直接mutationされる。照明更新を曖昧な`Changed` queryへ足すと1frameの不一致やsilent missが起きる。
- 到達したい状態: root adapterがsemantic snapshotを収集し、全topology writerとenergy settlement後、Door最終状態確定後に1回だけfieldを再構築する。
- 成功指標: Wall完成、Door Open / Closed / Locked、Lamp給電変化が定義したvisual frameでfieldへ反映され、入力不変frameではquery scan / rebuildを抑止できる。

## 2. スコープ

### 対象（In Scope）

- `hw_world`のDoor mutation APIと、auto / manual両方が通る単一writer経路。
- completed Wall、ProvisionalWall、DoorState、Room mask、`PowerSupplyState`、emitter / mountからのsemantic snapshot構築。
- lighting専用dirty tracker、input revisions、named system sets、`ApplyDeferred` barrier。
- manual Door requestのpause semanticsとmessage lifecycle。
- runtime instrumentationとheadless schedule / integration tests。

### 非対象（Out of Scope）

- Door sprite / 3D visualそのものの実装（P02。P04はstate producerだけを所有）。
- save / rollback / world replacement（P05）。
- GPU upload（P06）。
- Soul回復 / Room summary（P07）。

## 3. 前提correctness

現行production Doorはrootに`Door`、childに`Sprite`を持つ一方、一部queryが同一entity上の`Door + Sprite`を要求する。この形ではsynthetic testだけ成功してcompleted building Doorが対象外になり得る。

P02の最初のwork packageで次を完了してから、本計画のDoor schedulingへ進む。

1. `hw_world::apply_door_state`を`Door + WorldMap`だけを変更するdomain mutationへ分離する。
2. stageごとのactive presentation consumerはowner rootの`DoorState`を読む。P02最終後は`Door3dVisual`だけを描画consumerとする。
3. `attach_building_shell` / completionから生成したproduction root + active presentation構造でauto / manual integration testを通す。

P04はこのAPIを唯一のDoor state writerとして使い、`Sprite`有無をmutation成立条件に戻さない。

## 4. 実装方針

### 4.1 semantic adapter

| snapshot入力 | 収集条件 | dirty source |
| --- | --- | --- |
| Wall blocker | `BuildingType::Wall`かつcompleted、`Without<ProvisionalWall>` | completion / removal / grid移動 |
| Door blocker | Door rootのClosed / Locked | state transition / spawn / removal |
| emitter | `RadialLightEmitter`かつ`PowerSupplyState::Supplied` | component / power / transform / mount変更 |
| indoor mask | `RoomTileLookup`等のRoom topology revision | Room再検出完了 |

- `Without<Unpowered>`のような消極条件を使わず、`PowerSupplyState::Supplied`を明示する。
- 全`PowerConsumer`をLampと見なさず、`RadialLightEmitter`を持つfixtureだけを収集する。
- P00 fixtureでは未給電negative controlにもtyped componentを付ける。small / medium / largeの`typed_emitter_components`は`2 / 11 / 51`、`PowerSupplyState::Supplied`でsnapshotへ採用する`eligible_supplied_emitters`は`1 / 10 / 50`とする。
- indoor maskはHVAC M0等で確立したinterior roleを読み、Lamp / Tank / MudMixer等が占有するfloor cellを欠落させない。
- snapshotはentity IDをfieldへ保存せず、stable grid keyへ正規化する。
- topology revisionはnavigationの`WorldMap.obstacle_version`から独立させる。
- `WallMounted`は`wall_grid`が同じsemantic snapshot内のcompleted Wallであり、ProvisionalWallでもDoorでもない場合だけ有効とする。`inward_normal`はcardinal、inward隣接cellはmap内でなければならない。不成立時はemitterをsnapshotから発光0として扱い、推測で別Wallへ付け替えない。

### 4.2 named setとbarrier

root pluginで次の順序を明示し、登録順へ依存させない。

```text
Logic:
  WorldTopologyMutationSet
  -> ApplyDeferred
  -> EnergySettlementSet
  -> ApplyDeferred
  -> RoomTopologyRefreshSet
  -> ApplyDeferred

Pause gate外 / pre-Actor:
  DoorManualMutationSet

Actor（unpaused時）:
  DoorAutoOpenSet
  -> SoulMovementSet
  -> FamiliarMovementSet
  -> DoorAutoCloseSet

Pause gate外 / Actor後:
  IndoorLightingCollectSet
  -> IndoorLightingRebuildSet

Visual:
  Door / structural presentation consumers
```

- `WorldTopologyMutationSet`にはwall / floor / building completion、construction state遷移、deconstruction / world editの全writerを列挙する。
- auto Doorの最終state後にfieldを更新し、同じVisual phaseでDoor visualと照度が一致する。
- Room detectionの既存0.5秒cooldownはindoor mask / summaryだけへ影響する。Wall / Door LOS反映はRoom再検出を待たない。
- `IndoorLightingCollectSet` / `IndoorLightingRebuildSet`はpause gate外へ置き、pause中のmanual Door / world replacement dirtyも次Visual前に処理する。
- P07のSoul gameplay samplingは`IndoorLightingRebuildSet`後かつslow tick時に置く。gameplay pause policyはP07で固定する。

### 4.3 manual Doorとpause

`UiIntent::ToggleDoorLock`はDoor / WorldMapを直接変更せず、ownerだけを持つ`DoorLockToggleRequest`を送る。request型は`hw_world`、message registration / clear-on-world-replaceはrootが所有する。

製品契約は「pause中もlock / unlock可能」とする。したがってmanual mutation setは一般Actor pause gateの内側へ置かず、Interfaceで送信された次Updateの、Visualより前に実行する。

```text
Update N Interface: request enqueue
Update N+1 pre-Visual: validate owner -> apply_door_state -> light dirty
Update N+1 Visual: Door sprite / 3D visualとfield textureが同じstateを表示
```

- owner不存在、construction中、lock不可ならreject reasonを1回だけ通知し、queueへ残さない。
- save / world replace開始時にpending requestをclearする登録をP05と接続する。
- auto Doorとmanual requestが同じupdateに競合した場合はmanual lockを先に適用し、Lockedをauto openが上書きしない既存domain ruleをtestにする。

### 4.4 dirty / snapshot / rebuild

- change observer / queryはdirty reasonだけを記録し、同一frameに複数回fieldを再構築しない。
- collect時にtopology、emitter / power、Room maskのinput revisionを比較し、必要なsnapshotだけを再構築する。
- rebuild後、bytes同一ならfield revisionを維持する。
- `lighting_snapshot_collect_count`、`lighting_field_rebuild_count`、reason別dirty count、changed cell count、durationをP00 instrumentationへ出す。
- steady-stateではfull building / emitter scanも0を目標とする。Bevy change detectionだけで保証できない集合変更は、小さなdomain revision resourceで追跡する。

## 5. マイルストーン

## M1: Door単一writerとrequest境界を導入する

### 変更内容

1. P02の`apply_door_state` APIをauto Doorへ接続する。
2. `DoorLockToggleRequest`とroot message registrationを追加する。
3. UI intent handlerをrequest producerへ変更する。
4. pause時、競合時、invalid owner、world replacement clearのテスト契約を作る。

### 主な変更ファイル

- `crates/hw_world/src/door_systems.rs`
- `crates/hw_spatial/src/door_proximity.rs`
- `crates/bevy_app/src/interface/ui/interaction/{intent_context.rs,intent_handler.rs}`
- `crates/bevy_app/src/plugins/{messages.rs,logic.rs}`

### 完了条件

- [ ] production Door domain rootと、そのstage contractでactiveな全presentation consumerでauto / manualが動く。P02 M3完了後は`Door3dVisual` exactly oneになる
- [ ] `RLV1-P02-DOOR-DOMAIN`を満たす
- [ ] InterfaceからDoor / WorldMapの直接mutationがない
- [ ] pause中のmanual lockが次Updateで反映される
- [ ] requestがworld replace時に残らない

## M2: semantic snapshotとdirty trackerを接続する

### 変更内容

1. Wall / Door / emitter / power / Room mask adapterを実装する。
2. spawn、despawn、completion、state、power、mount別のdirty reasonを発行する。
3. stable keyでP03 snapshotへ変換する。
4. WallMounted anchorをcompleted Wall / provisional / Door / missing / map edgeのtableでsemantic validationする。

### 主な変更ファイル

- `crates/hw_infra/src/lighting/systems.rs`
- `crates/bevy_app/src/systems/lighting/{mod.rs,snapshot.rs,dirty.rs}`
- `crates/bevy_app/src/plugins/logic.rs`
- building completion / removalのdomain revision producer

### 完了条件

- [ ] ProvisionalWall追加だけでは遮光しない
- [ ] completed transition / removalが1回dirtyになる
- [ ] typed emitter component `2 / 11 / 51`のうちeligible supplied `1 / 10 / 50`だけがsnapshotに入る
- [ ] Supplied以外のemitterがsnapshotに入らない
- [ ] non-Lamp `PowerConsumer`が発光しない
- [ ] completed WallだけがWallMounted anchorとして有効
- [ ] provisional / Door / missing / OOB anchorはdiagnostic付きfail-dark

## M3: schedule transactionを固定する

### 変更内容

1. topology / energy / Room / Door / lightingのnamed setをrootでconfigureする。
2. deferred command境界へ明示的な`ApplyDeferred`を置く。
3. shuffled plugin registrationでも同じorder edgeを持つschedule testを追加する。
4. auto Door開閉とWall完成をframe-by-frame integration testにする。

### 主な変更ファイル

- `crates/bevy_app/src/plugins/{game.rs,logic.rs,visual.rs}`
- `crates/bevy_app/src/entities/damned_soul/mod.rs`
- `crates/bevy_app/src/systems/familiar_ai/mod.rs`
- `crates/bevy_app/src/systems/lighting/mod.rs`
- relevant schedule tests

### 完了条件

- [ ] Wall完成は最初のcompleted visual frameから遮光する
- [ ] auto Doorのstate / field / visualが同一Visual phaseで一致する
- [ ] Room cooldownに関係なくDoor LOSが更新される

## M4: instrumentationとsteady-stateを閉じる

### 変更内容

1. reason別dirty / collect / rebuild / changed-cell metricを追加する。
2. P00 small / medium / large fixtureでburstとsteady-stateを測る。
3. unchanged 600 updateのrebuild 0、不要なfull scan 0をtestする。
4. P00 `stage=p04`の全required legを採取し、`door-state-v1` / `load-normal-v1`のfield revision列をrequired検証する。

### 完了条件

- [ ] steady-state recomputeが0
- [ ] 1frameの複数dirtyが1 rebuildへcoalesceされる
- [ ] `RLV1-P02-DOOR-DOMAIN`、`RLV1-P03-FIELD`、`RLV1-P04-EMITTER`、`RLV1-P04-STEADY`を満たす

## 6. 検証計画

- production shell由来Doorのauto / manual / pause integration tests。
- Wall provisional→completed→removed timeline tests。
- Door Open→Closed→Locked→Open field revision tests。
- power Supplied→Curtailed / Unpowered→Supplied tests。
- shuffled source entity order / plugin registration order tests。
- 600-frame steady-state counter test。
- P00 audit / behavior / Capture / Memory / field-core各3 run + RenderDoc固定1 frameのoffline gate検証。
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `python3 scripts/dev.py verify`
- native Door / Wall same-frame acceptance。
- Help impact review。

## 7. リスクと対策

| リスク | 対策 |
| --- | --- |
| synthetic Doorだけ通りproductionが失敗する | completion helper由来domain root + active presentation fixtureを必須にする |
| manual requestがpause gate内で停滞する | pause外のnamed mutation setへ置き、timeline testを持つ |
| deferred spawnをsnapshotが見落とす | topology / energy / Room間に明示barrierを置く |
| `Changed`だけではdespawnを拾えない | domain revision / removal observerをdirty sourceにする |
| Room cooldownでDoor照明が遅れる | LOS topologyとindoor mask revisionを分離する |

## 8. ロールバック方針

- M1 Door correctnessは照明と独立した不具合修正として残し、Light Field rollbackで元の`Door + Sprite` queryへ戻さない。
- snapshot adapterはfeature registrationを外して無効化でき、P03 pure coreへ影響させない。
- request schedulingを戻す場合もpending messageをclearし、二重writerを残さない。

## 9. AI引継ぎメモ

### 現在地

- 進捗: `0%`
- 完了済み: runtime transaction設計
- 未着手: M1〜M4

### 次のAIが最初にやること

1. P02 Door correctness work packageとP03 APIの完了を確認する。
2. current `GameSystemSet`のpause conditionと全Door / topology writerを`rg`で再監査する。
3. M1 production-shell Door integration testから着手する。

### ブロッカー/注意点

- P05のsave registry dirty worktreeをP04で編集しない。
- Root ordering owner以外のcrateからcross-domain `.before()` chainを増殖させない。
- Room entityは再生成され得るため、dirty stateへRoom Entityを保持しない。

### 最終確認ログ

- Rust gates: `2026-08-04` / `not run (plan-only update)`
- native acceptance: `2026-08-04` / `not run (plan-only update)`
- docs gate: `2026-08-04` / `pass (docs --write / --check, check_docs, diff --check)`

### Definition of Done

- [ ] M1〜M4が完了
- [ ] production Door silent pathが解消されている
- [ ] Wall / Door / power / Room maskの更新順がtestで固定されている
- [ ] `stage=p04`のexact gate ID集合を満たす
- [ ] Help impact reviewと必要なnative acceptanceが完了

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-08-04` | `Codex` | Room interior依存、stage別Door consumer、typed / eligible emitter exact gateをP00へ同期 |
| `2026-08-03` | `Codex` | 統合計画からDoor・energy・topologyのruntime transactionを独立化 |
