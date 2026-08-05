# P07: 室内 Light Field gameplay・Room統合計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `single-scene-light-field-07-indoor-light-gameplay-room-plan-2026-08-03` |
| ステータス | `Draft` |
| 作成日 | `2026-08-03` |
| 最終更新日 | `2026-08-04` |
| 作成者 | `Codex` |
| 親計画 | [`../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md`](../single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md) |
| 直接依存 | [P03](03-indoor-light-domain-core-plan-2026-08-03.md)、[P04](04-indoor-light-runtime-integration-plan-2026-08-03.md)、[P05](05-indoor-light-save-lifecycle-plan-2026-08-03.md) |
| 並行可能 | [P06](06-indoor-light-rendering-plan-2026-08-03.md)とconsumer単位で並行可能 |
| 後続 | [P08](08-legacy-cleanup-release-plan-2026-08-03.md) |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: 現`lamp_buff_system`は全`PowerConsumer`をLamp候補にし、遮光を無視してLampごとにSoulへ効果を重ね、半径コメントのtileとworld unit計算も一致しない。
- 到達したい状態: 各Soulはslow simulation stepごとにCPU `IndoorLightField`を1回sampleし、Room summaryも同じfield revisionから導出される。
- 成功指標: Lamp数による効果stackがなく、Wall / Door / powerの状態でgameplayとRoom summaryが同じCPU field cellを判定し、Room entity再生成後もstale summaryを参照しない。GPU表示との横断一致はP06統合後の`RLV1-P08-CROSS-CONSUMER`で閉じる。

## 2. スコープ

### 対象（In Scope）

- 既存Lamp回復効果のfield samplingへの置換。
- radiusをtile型へ統一し、既存回復rateを維持する初期balance契約。
- `RoomIlluminationState`のderived summaryと更新revision。
- Soul移動 / Door / field更新後のsampling schedule。
- debug / perf counters、headless gameplay / Room tests。

### 非対象（Out of Scope）

- proportional brightnessによる新しい回復curve。
- comfort / health / productivityの新規system。
- Room照度UI、warning / notification、AI task generation。
- GPU texture readback、renderer pixelをgameplay正本にすること。

## 3. gameplay契約

### 3.1 初期balance

初期移行は「UNORM16 field scalarが`0`より大きければ既存の照明中回復rateを1回適用、`0`なら適用しない」のbinary判定とする。

- slow simulationは10 Hz、stress低減は`0.004/s`、fatigue回復は`0.003/s`を維持し、P07で勝手に増減しない。
- emitter標準半径は`LightRadiusTiles(5)`とし、world distance `5.0`として比較しない。
- 1 Soul / 1 slow stepにつき効果は最大1回。同じcellを複数Lampが照らしてもstackしない。
- SuppliedでないLamp、non-Lamp `PowerConsumer`、Wall / Closed Door越しの暗いcellは効果0。
- colorは初期gameplayへ影響させず、P03でUNORM16 linear RGBから確定計算した`LightLevel` scalarだけを使う。
- proportional curveや明るさ別bonusは新しいbalance判断として別proposal / test更新を要求する。

### 3.2 sampling timing

既存`SlowSimulationClock`のtickだけで全対象Soulを1回走査する。

```text
Door / movement / power settlement
-> IndoorLightingRebuildSet
-> SoulLightRecoverySet（unpausedかつslow tick時だけ）
-> Visual consumer
```

- Soulの移動後grid cellをsampleする。
- field rebuild自体はpause中のmanual Doorへ追従するが、Soul回復effectはsimulation pause gate内に置きpause中には進めない。
- load wake直後でfield未構築 / epoch不一致ならdarkとして効果0にし、旧fieldへfallbackしない。
- field revision不変でもslow tickならSoulの現在位置が変わり得るためsampleする。field自体は再構築しない。
- queryは`Soul`と必要なenergy / recovery contextをcrate-owned context型へ集約し、Lamp entityとのN×M走査を廃止する。

## 4. Room summary契約

`RoomIlluminationState`はRoom entityへ付くruntime derived componentとし、保存しない。

| field | 意味 |
| --- | --- |
| `field_revision` | どのCPU fieldから計算したか |
| `room_topology_revision` | どのRoom tile集合か |
| `sample_count` | 有効tile数 |
| `mean_light` | scalar平均 |
| `min_light` | scalar最小 |
| `dark_ratio` | gameplay threshold以下のtile比率。v1 threshold 0ではscalar `== 0`の比率 |

- Room membershipは既存Room topologyを正本とし、Door Open / ClosedでRoomを分割・結合しない。
- summaryはfield revisionまたはRoom topology revision変更時だけ更新する。
- Room検出はentityを再生成し得るため、cache / dirty stateへRoom Entityを保存しない。stable room tile signatureまたは当該frame queryから再付与する。
- Room summaryはLOS入力ではない。照度→summaryの一方向consumerとする。
- UI / AIのconsumer追加は別計画まで行わないが、public read-only APIとtestsを用意する。

## 5. マイルストーン

## M1: balanceと単位を固定する

### 変更内容

1. P00のcurrent rate、tick、radius mismatch、stack挙動を記録する。
2. binary threshold、`LightRadiusTiles(5)`、non-stack契約を恒久gameplay docsへ追加する。
3. fixed field sampleからeffect有無を返すpure decision functionを作る。

### 主な変更ファイル

- `crates/hw_infra/src/lighting/`のread API
- `crates/bevy_app/src/systems/soul_ai/`またはenergy gameplay owner
- `docs/soul-energy.md`

### 完了条件

- [ ] rate / tick / thresholdに`TBD`がない
- [ ] tileとworld unitを混用するAPIがない
- [ ] 複数emitterでもdecisionは1回

## M2: Soul effectをfield consumerへ置換する

### 変更内容

1. `lamp_buff_system`を`apply_light_recovery_effect_system`相当へ置換する。
2. Lamp query / per-Lamp stacking / raw distance計算を削除する。
3. P04 named set後、slow tickだけにscheduleする。
4. move、Door、power、load epoch別のtable testを追加する。

### 主な変更ファイル

- `crates/bevy_app/src/systems/soul_ai/`のLamp effect owner
- `crates/bevy_app/src/plugins/{game.rs,logic.rs}`
- `crates/bevy_app/src/entities/damned_soul/mod.rs`
- `crates/hw_core`またはSoul energy context owner

### 完了条件

- [ ] Soul 1体あたりfield sample 1回 / slow step
- [ ] emitter数に応じたN×M queryがない
- [ ] Wall / Door / powerとeffectが同じfield expected値になる
- [ ] invalid / stale fieldではeffect 0

## M3: Room summaryを追加する

### 変更内容

1. `RoomIlluminationState`とpure aggregationを`hw_infra`へ追加する。
2. root adapterがRoom tile集合とfield snapshotからcomponentを付与 / 更新する。
3. Room再検出 / despawn / split相当のfixtureでstale component cleanupを検証する。
4. summary recompute / sampled cells metricを追加する。
5. P05のderived consumer reset hookへRoom summary cleanupを登録し、old epoch componentをwake前に除去する。

### 主な変更ファイル

- `crates/hw_infra/src/lighting/{components.rs,room.rs,systems.rs}`
- `crates/bevy_app/src/systems/lighting/room_summary.rs`
- `crates/bevy_app/src/plugins/logic.rs`

### 完了条件

- [ ] unchanged field / Room revisionのsummary再計算が0
- [ ] Room entity再生成後に旧summary / Entity参照が残らない
- [ ] mean / min / dark ratioがfixed field vectorと一致する
- [ ] summaryがsave payloadにない

## M4: soak・性能・文書を閉じる

### 変更内容

1. P00 `consumer-core` laneへproviderを接続し、large相当の500 Soul / 16 Room / 576 cellを32 warmup + 256 measured call × 3 runで測る。
2. Door連続開閉、power churn、Room再検出、loadを組み合わせたsoak testを実行する。
3. P00 `stage=p07`の全required legを採取し、behavior 6 load caseのeffect / Room old-epoch read `0`とconsumer-coreを検証する。
4. gameplay docs、Room docs、architectureを更新する。

### 完了条件

- [ ] `RLV1-P07-CPU-CONSUMERS`と`RLV1-P07-CONSUMER-CPU`を満たす
- [ ] effect / summary / field revision mismatchが0
- [ ] load / rollbackで旧worldの回復効果が1tickも発生しない

## 6. 検証計画

- pure threshold / unit / non-stack tests。
- one Soul / many Lamp、many Soul / one Lamp query-count tests。
- Door / Wall / power / movement / stale epoch table tests。
- Room mean / min / dark ratio golden vectors。
- Room entity recreation / unchanged revision tests。
- deterministic fixed-step audit。
- `indoor_light_consumers.csv`の256 row × 3 run、sample / summary exact count、p95 / p99、allocation gate検証。
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `python3 scripts/dev.py verify`
- native visualとのspot-check（P06完成後）。
- Help impact review。

## 7. リスクと対策

| リスク | 対策 |
| --- | --- |
| current bugを仕様として維持する | current stack / unit mismatchは観測値、targetはbinary non-stackと明記 |
| proportional明度でbalanceが大きく変わる | 初期はthreshold binary、curve変更を別判断にする |
| Room entityをcacheしてstale参照になる | revision + tile signatureを使いEntityをdirty stateへ保持しない |
| rendererとgameplayが別式になる | GPU readbackもshader再計算もせずCPU field revisionを唯一の正本にする |
| actor移動前のcellをsampleする | P04 named set後へscheduleしframe-order testを持つ |

## 8. ロールバック方針

- gameplay consumerだけを無効化してもP03〜P06 field / visualは保持できる。
- 旧Lamp effectへ一時的に戻す場合も、全`PowerConsumer`をLamp扱いするqueryとper-Lamp stackはcorrectness fixとして戻さない。
- `RoomIlluminationState`はderivedなのでregistrationを外せば安全に削除でき、save migrationは不要。

## 9. AI引継ぎメモ

### 現在地

- 進捗: `0%`
- 完了済み: gameplay / Room consumer契約
- 未着手: M1〜M4

### 次のAIが最初にやること

1. P03 / P04 / P05のfield revisionとepoch APIを確認する。
2. current `lamp_buff_system`のrate / tick / query ownerをP00 artifactとコードから記録する。
3. M1のpure binary decision testから着手する。

### ブロッカー/注意点

- P06未完でもheadless gameplay実装は可能だが、最終一致確認はP06後に行う。
- Room summaryを保存しない。
- 新しいcomfort / health効果へscopeを広げない。

### 最終確認ログ

- Rust gates: `2026-08-04` / `not run (plan-only update)`
- native acceptance: `2026-08-04` / `not run (plan-only update)`
- docs gate: `2026-08-04` / `pass (docs --write / --check, check_docs, diff --check)`

### Definition of Done

- [ ] M1〜M4が完了
- [ ] Soul effectがbinary non-stackのfield consumerになっている
- [ ] Room summaryがfield / topology revision追跡を持つ
- [ ] `stage=p07`のexact gate ID集合とload correctnessを満たす
- [ ] Help impact reviewとgameplay / Room docs更新が完了

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-08-04` | `Codex` | P00の10 Hz / rate / threshold、consumer-core数値gate、P05 reset hook ownershipへ同期 |
| `2026-08-03` | `Codex` | GPU表示から独立したCPU field consumerとしてgameplay / Room計画を具体化 |
