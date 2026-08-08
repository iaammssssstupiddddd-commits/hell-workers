# Soul Energy システム

Soul が Soul Spa で瞑想することで電力を生成し、Outdoor Lamp 等の消費設備に供給するシステムです。
供給が需要を下回ると、設備ごとの優先度と安定した位置順に従って個別に負荷遮断します。

## 1. 概要

- 電力は **リアルタイム供給/需要バランス**（蓄電なし）
- グリッドは **Yard 単位**（1 Yard = 1 PowerGrid）
- 発電: Soul が SoulSpaTile 上で GeneratePower タスクを実行 → Dream を消費して発電
- 消費: Outdoor Lamp 等が常時需要を持つ
- 配電: High → Normal → Low、同順位は grid 座標 `(y, x)` の strict prefix。小需要を後から詰める bin-packing はしない
- 互換モード: Settings で優先配電を無効にすると、需要超過時に全 consumer を停止する旧 all-or-none 動作
- 型・定数・RelationshipとECS非依存のpure allocatorは`crates/hw_energy`に集約し、world Query / runtime同期はrootが所有

## 2. ECS 接続マップ

### Relationship（Source 操作のみ — Target は Bevy 自動管理）

| Source（手動操作） | Target（Bevy 自動） | 書き込み元 | 削除元 |
|:---|:---|:---|:---|
| `GeneratesFor(grid)` on SoulSpaSite | `GridGenerators` on PowerGrid | topology reconciler（Yard内の位置から再構築） | topology reconciler / Entity despawn |
| `ConsumesFrom(grid)` on OutdoorLamp | `GridConsumers` on PowerGrid | topology reconciler（Yard内の位置から再構築） | topology reconciler / Entity despawn |

### コンポーネント付与チェーン

```
SoulSpaSite spawn
  → #[require(PowerGenerator)] → PowerGenerator 自動付与
  → GeneratesFor(grid) 手動 insert → GridGenerators 自動更新

OutdoorLamp 建設完了 (post_process)
  → PowerConsumer insert
    → #[require(Unpowered, PowerConsumerPolicy)]
      → 初期状態をfail-closedにし、priorityはNormal
    → observerはtopology dirtyだけを通知
    → reconcilerがConsumesFrom(grid)をinsert → GridConsumers自動更新
    → on_power_consumer_visual_added Observer → PoweredVisualState { is_powered: false }
  → grid_recalc_system が個別PowerSupplyStateを確定
    → SuppliedだけUnpoweredを除去 → on_unpowered_removed → is_powered = true
```

## 3. PowerGrid ライフサイクル

PowerGrid エンティティは Yard と厳密に 1 対 1 で存在する。Observerはworldを直接編集せずdirty通知だけを行い、
`reconcile_power_grid_topology_system`が通常spawn・load・rollbackを同じ経路で正規化する。

| イベント | 処理 | 実装 |
|:---|:---|:---|
| Yard 追加 / Grid 欠落 | canonical `PowerGrid + YardPowerGrid` を1件生成 | topology reconciler |
| 重複 Grid | relationship数が多いGrid、同数ならEntity IDが小さいGridを残して接続を付け替える | topology reconciler |
| Yard 削除 / orphan Grid | Gridをdespawnし、対象外consumerを`Disconnected`へ正規化 | topology reconciler |
| consumer / generator移動 | Yard包含を再評価し、Relationshipの欠落・不整合を修復 | topology reconciler |

初期状態: `generation=0, consumption=0, powered=true`（消費者なし = 停電ではない）

## 4. 発電: Soul Spa

### 4.1 施設構造

- **SoulSpaSite**: 2x2 ルートエンティティ。`#[require(PowerGenerator)]` で自動付与
- **SoulSpaTile**: 通常spawnでは4枚の子エンティティ。保存対象の`parent_site`が論理ownerであり、`ChildOf` / `Children`はload時に復元しない。Operational 時に `Designation(GeneratePower)` + `TaskSlots{max:1}` が付与される

### 4.2 建設フロー

1. プレイヤーが Yard 内 walkable タイルにクリック配置（`soul_spa_place/input.rs`）
2. `SoulSpaSite { phase: Constructing }` + 4 × `SoulSpaTile` をスポーン
3. Familiar が Bone を搬送（`soul_spa_auto_haul_system`）
4. `bones_delivered >= bones_required` (12) で `soul_spa_tile_activate_system` が Operational に遷移
5. 各タイルに `Designation(GeneratePower)` + `TaskSlots{max:1}` を付与

Constructing中は情報パネル、またはTask Dashboardのwell-formedな`DeliverToSoulSpa`行からsite単位でcancelできる。
owner consumerは2×2 footprint、4 tile、関連worker/request、power shapeを事前検証し、成功時だけ全owner状態を
一括除去して`bones_delivered`の実数をBoneとして100%返す。pause中の情報パネル操作は停止中のLogicへrequestを
残さず`Paused` outcomeへ即時終端し、resume後に遅延適用しない。

### 4.3 発電出力の計算

`soul_spa_power_output_system`（Update, GameSystemSet::Logic）:

```
active_count = タイルのうち TaskWorkers が非空のもの数
current_output = active_count × output_per_soul (1.0W)
```

`active_count` は表示用 `Children` ではなく、保存される `SoulSpaTile.parent_site` と各tileの
`TaskWorkers`を一走査して集計する。ロード後に表示階層が未再構築でも出力は正しく0へ戻る。
計算結果がNaN/infまたは負値になる不正な発電設定は0Wへ正規化し、保存済み出力を残さずfail-closedにする。

### 4.4 GeneratePower タスク実行

Soul が SoulSpaTile に到着後:
- Dream を `DREAM_CONSUME_RATE_GENERATING` (0.5/s) で消費
- 疲労を `FATIGUE_RATE_GENERATING` (0.005/s) で蓄積
- `soul.dream < DREAM_GENERATE_FLOOR` (10.0) でタスク自動完了

### 4.5 active_slots ゲート

`active_slots` は情報パネルから 0〜`SOUL_SPA_MAX_ACTIVE_SLOTS`（4）で変更する。
`SoulSpaSite.has_available_slot(occupied)`: `phase == Operational && occupied < active_slots`

Familiar AI の `assign_generate_power` は、既存 `TaskWorkers` と同じ割当cycle内にsubmit済みのpending reservationを
サイト単位で合算し、同時に各tileの`TaskSlots` / source reservationもshadowで検証する。これにより、同じtileへの
二重submitでsite枠だけを消費して別の空きtileを取り逃すことも、複数Familiarが枠上限を超えることも防ぐ。
デフォルトは4。稼働中数より小さく変更しても既存worker・task・relationshipは解除しない（no-kick）。
表示は `Draining (N active / M configured)` となり、既存作業の終了後から新規割当だけをM体まで止める。

### 4.6 解体時のowner lifecycle

Operational Soul Spaの一般解体はsite rootを正本に、4 tileの`GeneratePower` task/worker、
`DeliverToSoulSpa` request、`GeneratesFor` / `GridGenerators`、2D child / 3D proxy、WorldMapの2×2 ownerを
同じexclusive commitで閉じる。Outdoor Lampは`ConsumesFrom` / `GridConsumers`とpassable building ownerを閉じる。
どちらもowner/reverse relationship shapeが一致しなければ非変更でfail-closeし、targetだけを先にdespawnしない。

commitは`EnergyUpdateDirty::request_full_rebuild()`を発行し、production energy transactionが同じUpdateで
topology、Soul Spa output、allocation、`Unpowered`を再計算する。続くVisualで`PoweredVisualState`とsprite色まで
同期するため、Soul Spa撤去後の負荷遮断とLamp撤去後の残存consumer復旧は最初の有効frameで完結する。

## 5. 消費: Outdoor Lamp

### 5.1 建設

- `BuildingType::OutdoorLamp`（Temporary カテゴリ、1x1）
- 素材: Bone × 2
- 標準 Blueprint `SelectBuild` フローで建設

### 5.2 PowerConsumer 付与

建設完了時 `setup_outdoor_lamp` が `PowerConsumer { demand: OUTDOOR_LAMP_DEMAND }` を insert。
`#[require(Unpowered, PowerConsumerPolicy)]` により初期状態はfail-closed、priorityはNormal。

### 5.3 接続と配電方針

topology reconcilerがTransform位置を含むYardへ `ConsumesFrom(grid)` を付与する。
Yard 外または有効なGridがないconsumerは `PowerSupplyState::Disconnected + Unpowered`。

各consumerは保存対象の `PowerConsumerPolicy { priority }` を持つ。既存saveで欠ける場合は
`Normal`を補完する。情報パネルのPriorityボタンは `Low → Normal → High → Low` を循環し、root handlerが
対象の生存・`PowerConsumer`・policy存在を再検証してから変更する。

### 5.4 ランプバフ

`lamp_buff_system`（Update, GameSystemSet::Logic）:

- 対象: `With<PowerConsumer>, Without<Unpowered>` のランプ（= 通電中のみ）
- `SlowSimulationClock` の 100 ms step を共有し、render delta を別に積算しない
- Soul 全件との直積は作らず、Soul 用 `SpatialGrid` から半径内候補だけを取得してから正確な距離を判定する
- 半径 `OUTDOOR_LAMP_EFFECT_RADIUS` (5.0 タイル) 内の Soul に:
  - stress を `LAMP_STRESS_REDUCTION_RATE` (0.004/s) で軽減
  - fatigue を `LAMP_FATIGUE_RECOVERY_BONUS` (0.003/s) で軽減
- 停電時は `Without<Unpowered>` フィルタでスキップ → バフ自動停止

## 6. Grid 再計算

energy pipelineはSoul AI state-sanityの名前付きflush後に、次の順で1 transactionとして実行する。

```text
Deconstruction finalizer / construction owner cancel
→ SoulSpa construction → ApplyDeferred
→ SettingsからPowerAllocationMode同期（実値変更時だけdirty）
→ dirty検出
→ topology reconciliation → ApplyDeferred
→ SoulSpa output
→ individual allocation / disconnected normalization → ApplyDeferred
→ lamp effect
```

output/allocationはsteady-stateでは実行せず、次の変更でdirtyになる。

- SoulSpaSite / SoulSpaTile / SoulSpaTile上の`TaskWorkers`、SoulSpa の `PowerGenerator` 設定
- `PowerGrid` / generator / consumer の Added・Changed・Removed
- `PowerConsumerPolicy`、`GeneratesFor` / `ConsumesFrom` とtarget relationshipの変更
- `PowerSupplyState` / `PowerGridAllocationSummary` の欠落（runtime stateのfail-closed再構築）
- Yard境界、consumer/generator位置、load/rollback後の最初の完全再構築
- Operational Soul Spa / Outdoor Lampのowner-safe解体、Constructing Soul Spaのcancel

Soul Spa出力の`current_output`は同じtransaction内でallocation dirtyへ明示伝播し、そのderived writeだけ
change detectionを抑制する。したがって翌frameの自己再起動は起こさない。一方、外部からの`PowerGenerator`
（`output_per_soul`を含む）直接変更は通常の`Changed<PowerGenerator>`としてoutput/allocationを起動する。

`grid_recalc_system`（dirty時のみ、GameSystemSet::Logic）:

1. `GridGenerators`からfiniteかつ正のgenerationを合計する。
2. consumerをpriority、grid座標 `(y, x)`、同一座標時だけEntity IDで安定sortする。
3. `PriorityPrefix`では先頭から累積需要を加え、最初にcapacityを超えたconsumer以降をすべてShedにする。
4. 既知のpriority-mode Shedを復旧する時だけ`POWER_RESTORE_MARGIN`を要求する。cold start/reconnectと
   `LegacyAllOrNone`からの復帰はexact capacityで供給する。
5. `LegacyAllOrNone`ではpriorityとhysteresisを無視し、全需要が収まれば全件Supplied、超えれば全件Shedにする。
6. NaN/inf/負需要は該当consumerだけ`InvalidDemand + Unpowered`にし、他consumerの配電は継続する。
7. consumerごとの`PowerSupplyState`と互換`Unpowered`、Gridごとの`PowerGridAllocationSummary`を同時に確定する。

`PowerGrid.powered`は互換値であり、全consumerがSuppliedの時だけtrue。部分給電の正本は
`PowerGridAllocationSummary`と各consumerの`PowerSupplyState`である。

| `PowerSupplyState` | 意味 |
|:---|:---|
| `Supplied` | 給電中。`Unpowered`なし |
| `Shed::InsufficientGeneration` | 需要超過で即時遮断 |
| `Shed::RestoreMargin` | 復旧余裕が不足し、ちらつき防止のため待機 |
| `Shed::LegacyGlobalDeficit` | Legacy all-or-noneでGrid全体が不足 |
| `Disconnected` | 有効なYard/Grid接続なし |
| `InvalidDemand` | consumer demandが非finiteまたは負値 |

## 7. 視覚フィードバック

### 7.1 PoweredVisualState（VisualMirror パターン）

`hw_core::visual_mirror::energy::PoweredVisualState { is_powered: bool }`

| Observer | トリガー | 処理 |
|:---|:---|:---|
| `on_power_consumer_visual_added` | `Add<PowerConsumer>` | `PoweredVisualState { is_powered: false }` を付与 |
| `on_unpowered_added` | `Add<Unpowered>` | `is_powered = false` |
| `on_unpowered_removed` | `Remove<Unpowered>` | `is_powered = true` |

### 7.2 スプライト反映

`sync_powered_visual_system`（Update, GameSystemSet::Visual）:

- `Changed<PoweredVisualState>` を検知
- `is_powered=true` → `Color::WHITE`、`false` → `Color::srgba(0.4, 0.4, 0.4, 1.0)`
- エンティティ自身 + 子 Sprite のカラーを更新
- Deconstruction finalizerはLogic中にenergy dirtyを立て、energy transactionとVisualが後続するため、撤去による
  残存consumerの色変更も同じ`Update`で反映する

### 7.3 Power Status UI

Soul Spa / PowerConsumer の共通 Power Grid section は次を表示する。

- Connected / Disconnected
- generation、total demand、served demand
- reserve / deficit、consumer / supplied / shed / invalid件数、座標で表すshed順
- `Priority prefix` / `Legacy all-or-none`
- consumerのdemand、priority、`Supplied`または個別の遮断理由
- consumerではPriority循環ボタン、Soul Spaでは0〜4のActive slotsボタン

表示は`PowerGrid.powered`や`Unpowered`だけから推測せず、typed `PowerInspectionFields`へ写した
runtime summary/stateを正本にする。Generator / Consumerのroleを分けるため、正常なSoul Spaを
`Generator / rebuilding`と誤表示しない。Constructing中は骨材進捗と`Cancel Construction`だけを表示し、
output・grid・枠操作は隠す。Operationalではconstruction cancelを隠す。
stale/unsupported/missing policyやslot clamp/phase failureは専用outcomeから
同じUpdateのtoastへ変換する。

## 8. サイレント失敗トラップ

| 状況 | 症状 | 原因 |
|:---|:---|:---|
| ランプ建設しても常時暗い | `Disconnected` と表示される | Yard 外配置またはYard/Grid対応の欠落。reconcilerが有効な接続だけを張る |
| Soul Spa Operational なのに発電 0 | TaskWorkers が空 | Familiar が GeneratePower をアサインしていない。Dream 閾値 (`DREAM_GENERATE_ASSIGN_THRESHOLD` = 30.0) 未満の Soul しかいない |
| 稼働枠を下げてもSoulがすぐ退出しない | `Draining` | no-kick仕様。現在作業は完了まで継続し、新規割当だけを止める |
| 発電量が需要と一致してもShedが復旧しない | `RestoreMargin` | 既知Shedの復旧だけ余裕を要求する。発電量をmargin以上へ増やす |
| ランプ追加/出力変更後に通電状態が古い | energy pipeline の順序が崩れている | topology/output/allocationのdeferred境界とeffect前flushを保つ |
| pause中のSoul Spa cancelがresume後に突然適用される | 停止中のLogicへrequestを残している | root adapterで`Paused` outcomeへ即時終端し、requestを発行しない |

## 9. 定数一覧

| 定数 | 値 | 用途 |
|:---|:---|:---|
| `OUTPUT_PER_SOUL` | 1.0 | Soul 1 体の発電量（W） |
| `DREAM_CONSUME_RATE_GENERATING` | 0.5 | 発電中の Dream 消費速度（/s） |
| `DREAM_GENERATE_FLOOR` | 10.0 | Dream がこの値を下回ったらタスク自動終了 |
| `DREAM_GENERATE_ASSIGN_THRESHOLD` | 30.0 | この値以上でないとタスクをアサインしない |
| `OUTDOOR_LAMP_DEMAND` | 0.2 | ランプ 1 基の電力需要（W） |
| `OUTDOOR_LAMP_EFFECT_RADIUS` | 5.0 | ランプバフ半径（タイル） |
| `SOUL_SPA_MAX_ACTIVE_SLOTS` | 4 | Soul Spaで設定可能な最大稼働枠 |
| `POWER_ALLOCATION_EPSILON` | 0.0001 | capacity比較の浮動小数点許容差 |
| `POWER_RESTORE_MARGIN` | 0.05 | 既知Shedを復旧するための追加余裕 |
| `SOUL_SPA_BONE_COST_PER_TILE` | 3 | タイルあたり建設 Bone 数 |
| `FATIGUE_RATE_GENERATING` | 0.005 | 発電中の疲労蓄積速度（/s） |
| `LAMP_STRESS_REDUCTION_RATE` | 0.004 | ランプバフ ストレス軽減（/s） |
| `LAMP_FATIGUE_RECOVERY_BONUS` | 0.003 | ランプバフ 疲労回復（/s） |

定数はすべて `crates/hw_energy/src/constants.rs` に定義。

## 10. ゲームデザイン上の意図

### Dream トレードオフ三角形

- **労働力**: Soul を作業に割り当てる → 物理リソース生産
- **Soul Energy**: Soul を発電に割り当てる → 電力供給（ランプバフ等）、ただし Dream を消費
- **Dream**: Soul を休息させる → DreamPool 蓄積

同一 Soul は同時に 1 つの役割しか果たせないため、三者間の配分がマクロ管理の判断軸になる。

### 停電圧力

- ランプを増やすほど消費が増加 → より多くの Soul を発電に回す必要
- 発電 Soul を増やすと労働力・Dream 蓄積が減少
- active_slots で発電枠を絞ることで意図的に停電を許容する選択肢もある

## 11. 未実装（将来拡張）

- Room 接続（Phase 2: 壁隣接による Room → Grid 接続）
- Battery（蓄電建物）
- HVAC consumer実装。追加設備は`PowerConsumer + PowerConsumerPolicy`を付与し、効果判定は
  `Unpowered`または`PowerSupplyState::Supplied`を利用する。独自allocatorや全体blackout判定を複製しない
- Power line（遠距離グリッド接続）

BatteryはB3とHVAC consumerの運用が安定した後に別計画で着手する。
