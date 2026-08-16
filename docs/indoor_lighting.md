# 室内 Light Field

## 現在の実装範囲

P03は`hw_infra::lighting`に、Bevy ECS・GPU・ゲームワールドqueryへ依存しない室内Light Fieldのpure coreを実装した。P04は`bevy_app::systems::lighting`から通常playへ接続し、completed Wall、Door、typed OutdoorLamp、給電状態、Room maskを同じCPU fieldへ正規化する。P05はdurable mountとsave/load lifecycleを実装した。P06はGPU uploadとshader、P07はgameplay・Room consumerを担当する。

core入力は`GridDimensions`、row-majorの`IndoorMask`、semanticな`LightOcclusionGrid`、正規化済み`RadialLightEmitterSnapshot`である。最大gridはゲームworldと同じ100×100、canonical性能fixtureは50 emitter・radius 5 tileを使う。emitterはstable key順に処理し、duplicate keyは入力全体を拒否する。invalidな個別emitterはstable diagnosticを返してfail-darkにする。

## 遮光とLOS

遮光cellのcanonical byteと意味は次の通り。

| cell | byte | LOS blocker | wall mount anchor |
| --- | ---: | ---: | ---: |
| `Clear` | 0 | no | no |
| `ProvisionalWall` | 1 | no | no |
| `OpenDoor` | 2 | no | no |
| `CompletedWall` | 3 | yes | yes |
| `ClosedDoor` | 4 | yes | no |
| `LockedDoor` | 5 | yes | no |

LOSはinteger supercover traversalで、corner crossingでは対角cellへ進む前に両方のside cellを検査する。floatやepsilonを分岐へ使わない。wall-mounted fixtureは`CompletedWall` anchorと保存済みcardinal inward方向を要求し、別の始点を推測しない。`IndoorMask`は遮光には使わず、mask外targetの出力だけを0にする。

## Field契約

- 距離は`isqrt((dx² + dy²) << 32)`によるfloor Q16、falloffとUNORM16積はround-half-upで固定する。
- linear RGB寄与をstable key順に非負の`u32`へsaturating accumulationし、最後に`u16`へclampする。
- luminanceはRec.709の整数係数`(13933*r + 46871*g + 4732*b + 32768) >> 16`で求める。
- radiance payloadはrow-majorの`[r, g, b, luminance]` little-endian `u16`で、100×100では80,000 byteとなる。
- `pack_rgba8_linear`はRGBをround-half-upで8 bit化し、alphaをmask内255・外0にする。100×100では40,000 byteとなる。
- `input_checksum`、`radiance_checksum`、`mask_checksum`、`field_checksum`はpaddingやnative endianに依存しないcanonical bytesのSHA-256である。
- 初回revisionは1。radianceまたはmaskの公開byteが変化した場合だけchecked incrementし、overflowはerrorとする。radiance差分、mask差分、和集合のcell数を別々に返す。

coreはログや内部dirty stateを保持しない。P04 adapterがworld snapshot、給電判定、stable key、dirty reason、rebuild transaction、telemetryを構築する。

## P04 runtime adapter

`RadialLightEmitter`はruntime-only ECS componentであり、production v1ではcompleted `OutdoorLamp` rootだけに付く。通常loadなどでcomponentが存在しない場合は、lighting transactionの先頭で完成済みrootのgrid位置からfree-standing emitterを再構築してからdirty収集する。mountはP03の`FixtureMount`を再利用し、`PowerSupplyState::Supplied`だけをfield inputへ採用する。`Shed`、`Disconnected`、`InvalidDemand`とnon-Lamp `PowerConsumer`は発光しない。completed WallとClosed / Locked Doorは遮光し、Provisional WallとOpen Doorは遮光しない。map外、重複occlusion、invalid owner/mount、P03 diagnosticは`IndoorLightAvailability::Unavailable`として公開し、前回のlit snapshotをreaderへ返さない。

`RoomTileLookup`はrow-majorのtile membershipを`RoomMaskSignature`として保持する。Room entityを再生成してもtile集合が同じならrevisionを進めないため、Entity IDやRoom validation timerだけでLight Fieldを再構築しない。runtimeのstable emitter keyはrow-major anchor indexとmount tagから作り、Entity/query順に依存しない。

root scheduleは`Input → Spatial → Logic → PreActor → Actor → PostActor → Visual → Interface`である。Interfaceで生じた`DoorLockToggleRequest`は次Updateのpause gate外`PreActor`で一度だけ検証・適用され、Actorのauto Door/movement後、pause gate外`PostActor`でdirty収集・snapshot収集・最大1回のCPU rebuildを行う。`DoorPresentationSyncSet`はVisual内でrebuild後に走る。入力不変Updateではfull snapshot scan、rebuild、output revision増分を行わない。

`RadialLightEmitter`とfield snapshotはsave対象ではない。P05以後は`LightingFixtureMount(FixtureMount)`だけをcompleted `OutdoorLamp` rootへ保存し、通常spawn / Transform同期 / rehydrateが同じmountを読む。legacy saveでwrapperが欠ける場合だけTransform gridから`FreeStanding`を補完する。wrapperのowner、origin、Transform、`WorldMap` ownership、wall anchorが不正なcandidateはlive reset前にrejectし、live fieldと`WorldEpoch`を変えない。

## P05 save/load lifecycle

`IndoorLightingPlugin`はfreeze前に`lighting.mount.normalize`、`lighting.emitters.rebuild`、`lighting.wake`をproduction rehydrate registryへ登録する。normal load、rollback、recovery-onlyはいずれも同じresolved planを通り、wakeはfull dirtyを一度だけarmする。field rebuild自体は次の通常UpdateにあるP04 transactionが所有する。

world replacementの`lighting-runtime` reset hookはsnapshot、pending input、checksum、revision、published epoch、dirty/allocation stateを消去し、`IndoorLightAvailability::Unavailable`へfail-dark化する。公開snapshotは`WorldEpoch`でtagされ、epoch-aware readはtag不一致、reset中、unavailableで`None`を返す。rollbackはresetを2回通り得るがepoch advanceは1回、terminal wakeは1回である。`RecoveryFailed`中はemitter sync、dirty collect、snapshot collect、field rebuildを停止し、最後のlit fieldを再公開しない。

energyの`TaskWorkers`と`PowerSupplyState`はruntime-derivedで保存しない。したがってload後のenergy full rebuildはworkerのないSoul Spa出力を0へ再計算し、durable fixtureとruntime emitterを復元してもeligible supplied emitterが0ならavailableなdark fieldを公開する。P05のterminal checksumはdurable fixtureのsemantic rebindを証明し、旧field bytesの一致を要求しない。

GPU textureとuploadはP06、照度gameplay readはP07の責務である。

## P03 field-core evidence

P03の専用headless計測は次を使う。

```bash
PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py field-core \
  --output target/perf-runs/<fresh-session-name>
```

各3 runは32 warmup後のpure `rebuild_field`を256回計測し、`data/indoor_light_cpu.csv`と`data/indoor_light_field.json`だけを出力する。fixture構築、CSV/JSON serialization、ECS collection、GPU uploadはtimer外である。JSONには100×100/50/radius 5、4 checksum、600回のsteady no-op契約、pure rebuildが明示的に所有するbufferの論理allocation scopeを記録する。

正式な`p03` native bundleはrepositoryの`hell-workers-run-native-acceptance` skillから、cleanでcommit済みのsubjectに対して採取する。単独のfield-core sessionはparserと計測経路の検証には使えるが、actual-window Capture/Memory/RenderDocを含む`RLV1-BUNDLE-VALID`の代替にはしない。

## P04 runtime evidence

`--stage p04`はstatic / behavior / field-coreを受理する。staticとbehaviorは`indoor_light_runtime.json`へtyped / eligible emitter数、unsupplied adoption 0、canonical Room mask、input/output revision、availability、field checksumを出す。Room mask checksumはEntity IDを使わず、4近傍で連結した室内をanchorのrow-major順、各室内cellをrow-major順に直列化するため、P00の複数Room fixture契約と一致しつつRoom再生成に依存しない。behavior timelineもplaceholderではなく同じlive field値を記録する。

P04 field-coreはP03の`indoor_light_cpu.csv`と`indoor_light_field.json`をそのまま継承し、別の`indoor_light_runtime.json`でproduction ECS fixtureを実走した600 Updateを証明する。large fixtureはtyped 51 / eligible 50 / mask 576（16室×36 cell）であり、steady windowのfull scan、rebuild、revision increment、scoped allocation event/byteは0、1 Updateあたりのrebuildは最大1である。emitter collect allocationはadapterが明示的に所有するbufferの論理scopeとして別記録する。
