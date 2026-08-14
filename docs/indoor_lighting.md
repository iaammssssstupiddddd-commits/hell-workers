# 室内 Light Field

## 現在の実装範囲

P03は`hw_infra::lighting`に、Bevy ECS・GPU・ゲームワールドqueryへ依存しない室内Light Fieldのpure coreを実装する。通常プレイへの接続はまだ行わず、P04がECS snapshotとdirty/rebuild scheduling、P05がsave/load lifecycle、P06がGPU uploadとshader、P07がgameplay・Room consumerを担当する。

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

## P03 field-core evidence

P03の専用headless計測は次を使う。

```bash
PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py field-core \
  --output target/perf-runs/<fresh-session-name>
```

各3 runは32 warmup後のpure `rebuild_field`を256回計測し、`data/indoor_light_cpu.csv`と`data/indoor_light_field.json`だけを出力する。fixture構築、CSV/JSON serialization、ECS collection、GPU uploadはtimer外である。JSONには100×100/50/radius 5、4 checksum、600回のsteady no-op契約、pure rebuildが明示的に所有するbufferの論理allocation scopeを記録する。

正式な`p03` native bundleはrepositoryの`hell-workers-run-native-acceptance` skillから、cleanでcommit済みのsubjectに対して採取する。単独のfield-core sessionはparserと計測経路の検証には使えるが、actual-window Capture/Memory/RenderDocを含む`RLV1-BUNDLE-VALID`の代替にはしない。
