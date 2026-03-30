# Soul Energy システム

Soul が Soul Spa で瞑想することで電力を生成し、Outdoor Lamp 等の消費設備に供給するシステムです。
供給が需要を下回ると **停電（Blackout）** が発生し、全消費設備が機能停止します。

## 1. 概要

- 電力は **リアルタイム供給/需要バランス**（蓄電なし）
- グリッドは **Yard 単位**（1 Yard = 1 PowerGrid）
- 発電: Soul が SoulSpaTile 上で GeneratePower タスクを実行 → Dream を消費して発電
- 消費: Outdoor Lamp 等が常時需要を持つ
- 停電: `generation < consumption` で全 consumer に `Unpowered` マーカーが付与される
- 型・定数・Relationship はすべて `crates/hw_energy` に集約

## 2. ECS 接続マップ

### Relationship（Source 操作のみ — Target は Bevy 自動管理）

| Source（手動操作） | Target（Bevy 自動） | 書き込み元 | 削除元 |
|:---|:---|:---|:---|
| `GeneratesFor(grid)` on SoulSpaSite | `GridGenerators` on PowerGrid | `soul_spa_place/input.rs` (配置時) | Entity despawn 時 Bevy 自動 |
| `ConsumesFrom(grid)` on OutdoorLamp | `GridConsumers` on PowerGrid | `on_power_consumer_added` Observer | Entity despawn 時 Bevy 自動 |

### コンポーネント付与チェーン

```
SoulSpaSite spawn
  → #[require(PowerGenerator)] → PowerGenerator 自動付与
  → GeneratesFor(grid) 手動 insert → GridGenerators 自動更新

OutdoorLamp 建設完了 (post_process)
  → PowerConsumer insert
    → #[require(Unpowered)] → Unpowered 自動付与（初期停電状態）
    → on_power_consumer_added Observer → ConsumesFrom(grid) insert → GridConsumers 自動更新
    → on_power_consumer_visual_added Observer → PoweredVisualState { is_powered: false }
  → grid_recalc_system が powered 判定 → Unpowered 除去 → on_unpowered_removed → is_powered = true
```

## 3. PowerGrid ライフサイクル

PowerGrid エンティティは Yard と 1 対 1 で存在する。

| イベント | 処理 | 実装 |
|:---|:---|:---|
| Yard 追加 | `PowerGrid::default()` + `YardPowerGrid(yard)` をスポーン | `on_yard_added` Observer |
| Yard 削除 | 対応 PowerGrid を despawn（Relationship 自動クリーンアップ） | `on_yard_removed` Observer |

初期状態: `generation=0, consumption=0, powered=true`（消費者なし = 停電ではない）

## 4. 発電: Soul Spa

### 4.1 施設構造

- **SoulSpaSite**: 2x2 ルートエンティティ。`#[require(PowerGenerator)]` で自動付与
- **SoulSpaTile**: 4 枚の子エンティティ。Operational 時に `Designation(GeneratePower)` + `TaskSlots{max:1}` が付与される

### 4.2 建設フロー

1. プレイヤーが Yard 内 walkable タイルにクリック配置（`soul_spa_place/input.rs`）
2. `SoulSpaSite { phase: Constructing }` + 4 × `SoulSpaTile` をスポーン
3. Familiar が Bone を搬送（`soul_spa_auto_haul_system`）
4. `bones_delivered >= bones_required` (12) で `soul_spa_tile_activate_system` が Operational に遷移
5. 各タイルに `Designation(GeneratePower)` + `TaskSlots{max:1}` を付与

### 4.3 発電出力の計算

`soul_spa_power_output_system`（Update, GameSystemSet::Logic）:

```
active_count = タイルのうち TaskWorkers が非空のもの数
current_output = active_count × output_per_soul (1.0W)
```

### 4.4 GeneratePower タスク実行

Soul が SoulSpaTile に到着後:
- Dream を `DREAM_CONSUME_RATE_GENERATING` (0.5/s) で消費
- 疲労を `FATIGUE_RATE_GENERATING` (0.005/s) で蓄積
- `soul.dream < DREAM_GENERATE_FLOOR` (10.0) でタスク自動完了

### 4.5 active_slots ゲート

`SoulSpaSite.has_available_slot(occupied)`: `phase == Operational && occupied < active_slots`

Familiar AI の `assign_generate_power` がタスク割当前にチェック。`active_slots` のデフォルトは 4（= タイル数）。

## 5. 消費: Outdoor Lamp

### 5.1 建設

- `BuildingType::OutdoorLamp`（Temporary カテゴリ、1x1）
- 素材: Bone × 2
- 標準 Blueprint `SelectBuild` フローで建設

### 5.2 PowerConsumer 付与

建設完了時 `setup_outdoor_lamp` が `PowerConsumer { demand: OUTDOOR_LAMP_DEMAND }` を insert。
`#[require(Unpowered)]` により初期状態は停電。

### 5.3 ConsumesFrom 自動付与

`on_power_consumer_added` Observer が Yard lookup を行い `ConsumesFrom(grid)` を付与。
Yard 外のランプは ConsumesFrom なし → 常時 `Unpowered`。

### 5.4 ランプバフ

`lamp_buff_system`（Update, GameSystemSet::Logic）:

- 対象: `With<PowerConsumer>, Without<Unpowered>` のランプ（= 通電中のみ）
- 半径 `OUTDOOR_LAMP_EFFECT_RADIUS` (5.0 タイル) 内の Soul に:
  - stress を `LAMP_STRESS_REDUCTION_RATE` (0.004/s) で軽減
  - fatigue を `LAMP_FATIGUE_RECOVERY_BONUS` (0.003/s) で軽減
- 停電時は `Without<Unpowered>` フィルタでスキップ → バフ自動停止

## 6. Grid 再計算

`grid_recalc_system`（Update, `.after(soul_spa_power_output_system)`, GameSystemSet::Logic）:

1. 全 `PowerGrid` を走査
2. `GridGenerators` から `generation` を合計、`GridConsumers` から `consumption` を合計
3. `powered = consumption == 0 || generation >= consumption`
4. powered 状態変化時:
   - **POWERED**: 全 consumer から `Unpowered` を除去
   - **BLACKOUT**: 全 consumer に `Unpowered` を挿入
5. 通電中グリッドに新規 consumer 追加時も `Unpowered` を同期（`#[require(Unpowered)]` 対策）

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

### 7.3 Power Status UI

建物選択パネル（`append_building_model`）に PowerConsumer を持つ建物の電力情報を表示:
- グリッド接続あり: `"Power: {gen}W / {cons}W [POWERED/BLACKOUT]"`
- グリッド接続なし: `"Power: {demand}W demand [no grid]"`

## 8. サイレント失敗トラップ

| 状況 | 症状 | 原因 |
|:---|:---|:---|
| ランプ建設しても常時暗い | `Unpowered` が除去されない | Yard 外配置 → ConsumesFrom なし → grid_recalc が Unpowered を操作しない |
| Soul Spa Operational なのに発電 0 | TaskWorkers が空 | Familiar が GeneratePower をアサインしていない。Dream 閾値 (`DREAM_GENERATE_ASSIGN_THRESHOLD` = 30.0) 未満の Soul しかいない |
| 新規ランプが通電グリッドなのに一瞬暗い | 1 フレーム遅延 | `#[require(Unpowered)]` で初期 Unpowered → 次の grid_recalc で除去される |

## 9. 定数一覧

| 定数 | 値 | 用途 |
|:---|:---|:---|
| `OUTPUT_PER_SOUL` | 1.0 | Soul 1 体の発電量（W） |
| `DREAM_CONSUME_RATE_GENERATING` | 0.5 | 発電中の Dream 消費速度（/s） |
| `DREAM_GENERATE_FLOOR` | 10.0 | Dream がこの値を下回ったらタスク自動終了 |
| `DREAM_GENERATE_ASSIGN_THRESHOLD` | 30.0 | この値以上でないとタスクをアサインしない |
| `OUTDOOR_LAMP_DEMAND` | 0.2 | ランプ 1 基の電力需要（W） |
| `OUTDOOR_LAMP_EFFECT_RADIUS` | 5.0 | ランプバフ半径（タイル） |
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

- active_slots UI（Phase 1c ではバックエンドのみ）
- Room 接続（Phase 2: 壁隣接による Room → Grid 接続）
- Battery（蓄電建物）
- 追加消費設備（電動ミキサー等）
- Power line（遠距離グリッド接続）
