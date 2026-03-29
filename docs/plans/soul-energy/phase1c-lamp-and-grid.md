# Phase 1c: Outdoor Lamp + Grid Integration + Visual

| Item | Value |
|:---|:---|
| Status | ✅ Complete |
| Depends on | Phase 1a (data model), Phase 1b (Soul Spa + GeneratePower) |
| Blocks | Phase 2 |

## Goal

Complete the power loop: Outdoor Lamp as consumer, grid recalculation, powered/unpowered cycle, and visual feedback. At the end of this phase, the full "generate → consume → blackout → recover" cycle works in-game.

---

## Architecture Decisions

| Decision | Choice | Reason |
|:---|:---|:---|
| OutdoorLamp 配置フロー | 標準 Blueprint `SelectBuild` | 1x1 単タイル。SoulSpa 専用フロー不要 |
| ConsumesFrom 付与タイミング | Observer `On<Add, PowerConsumer>` | post_process.rs のシグネチャ変更不要。soul_spa_place と同様の Yard lookup |
| Grid 再計算トリガ | Update、`soul_spa_power_output_system` の後 | PowerGenerator の変化をすぐ反映。タイマー不要 |
| ランプバフ実装 | `DamnedSoul.stress/fatigue` を毎フレーム直接加減算 | "buff コンポーネント" 不要。範囲内にいる間だけ効果が続く自然な実装 |
| PoweredVisualState | `hw_core::visual_mirror::energy` に新設 | 既存 VisualMirror パターン踏襲 |
| 視覚フィードバック | `sprite.color` dimming のみ | 実装コスト最小。powered=false → `Color::srgba(0.4,0.4,0.4,1.0)` |

---

## Step 1: データ型 — OutdoorLamp

### `crates/hw_jobs/src/model.rs`

```rust
pub enum BuildingType {
    // ... 既存 ...
    OutdoorLamp,   // 追加
}
```

- `category()`: `BuildingType::OutdoorLamp => BuildingCategory::Temporary`
- `required_materials()`:
  ```rust
  BuildingType::OutdoorLamp => {
      materials.insert(ResourceType::Bone, 2);
  }
  ```

### `crates/hw_core/src/visual_mirror/building.rs`

```rust
pub enum BuildingTypeVisual {
    // ... 既存 ...
    OutdoorLamp,   // 追加
}
```

### `crates/hw_jobs/src/visual_sync/mod.rs`

```rust
BuildingType::OutdoorLamp => BuildingTypeVisual::OutdoorLamp,
```

### `crates/hw_energy/src/constants.rs` — 新定数

```rust
/// 点灯中のランプがソウルに与えるストレス軽減速度（/秒）
/// STRESS_WORK_RATE = 0.005 の 80% 相当
pub const LAMP_STRESS_REDUCTION_RATE: f32 = 0.004;

/// 点灯中のランプがソウルに与える疲労回復ボーナス（/秒）
/// FATIGUE_WORK_RATE = 0.01 の 30% 相当
pub const LAMP_FATIGUE_RECOVERY_BONUS: f32 = 0.003;
```

---

## Step 2: 建設フロー — Blueprint → 完成

### `crates/bevy_app/src/systems/jobs/building_completion/spawn.rs`

`SandPile`/`BonePile`（1x1）を参考に追加:

- Blueprint スプライト: `BuildingType::OutdoorLamp => (game_assets.bone_pile.clone(), Vec2::splat(TILE_SIZE))`
- 完成スプライト（同様に 1x1）
- 3D メッシュ（`WheelbarrowParking` パターン）:
  ```rust
  BuildingType::OutdoorLamp => (
      handles_3d.equipment_1x1_mesh.clone(),
      handles_3d.equipment_material.clone(),
      TILE_SIZE * 0.6,
  ),
  ```

### `crates/bevy_app/src/systems/jobs/building_completion/post_process.rs`

`apply_building_specific_post_process` に追加:

```rust
if bp.kind == BuildingType::OutdoorLamp {
    setup_outdoor_lamp(commands, building_entity);
}
```

新関数:
```rust
fn setup_outdoor_lamp(commands: &mut Commands, building_entity: Entity) {
    commands.entity(building_entity).insert(PowerConsumer {
        demand: hw_energy::OUTDOOR_LAMP_DEMAND,
    });
    // ConsumesFrom は on_power_consumer_added Observer が付与する
}
```

### `crates/bevy_app/src/systems/jobs/building_completion/world_update.rs`

`OutdoorLamp` は obstacle なし — `is_obstacle` リストに追加しない。

---

## Step 3: ConsumesFrom 付与 — Observer

### `crates/bevy_app/src/systems/energy/grid_lifecycle.rs`

既存の `on_yard_added` / `on_yard_removed` と同ファイルに追加:

```rust
/// PowerConsumer が追加されたとき、包含 Yard の PowerGrid に ConsumesFrom を付与する。
/// soul_spa_place/input.rs と同じ Yard lookup パターン（yard.contains(pos)）。
pub fn on_power_consumer_added(
    on: On<Add, PowerConsumer>,
    mut commands: Commands,
    q_transform: Query<&Transform>,
    q_yards: Query<(Entity, &Yard)>,
    q_grids: Query<(Entity, &YardPowerGrid)>,
) {
    let entity = on.entity;
    let Ok(transform) = q_transform.get(entity) else { return };
    let pos = transform.translation.xy();
    let Some(yard_entity) = q_yards.iter().find(|(_, y)| y.contains(pos)).map(|(e, _)| e)
    else {
        // Yard 外のランプは ConsumesFrom なし → 常時 Unpowered
        return;
    };
    let Some(grid_entity) = q_grids.iter().find(|(_, ypg)| ypg.0 == yard_entity).map(|(e, _)| e)
    else {
        return;
    };
    commands.entity(entity).insert(ConsumesFrom(grid_entity));
}
```

---

## Step 4: Grid 再計算システム

### `crates/bevy_app/src/systems/energy/grid_recalc.rs` (新規)

```rust
use bevy::prelude::*;
use hw_energy::{GridConsumers, GridGenerators, PowerConsumer, PowerGenerator, PowerGrid, Unpowered};

pub fn grid_recalc_system(
    mut q_grids: Query<(&mut PowerGrid, Option<&GridGenerators>, Option<&GridConsumers>)>,
    q_generators: Query<&PowerGenerator>,
    q_consumers: Query<&PowerConsumer>,
    mut commands: Commands,
) {
    for (mut grid, generators_opt, consumers_opt) in q_grids.iter_mut() {
        let new_gen: f32 = generators_opt
            .map(|generators| {
                generators
                    .iter()
                    .filter_map(|e| q_generators.get(*e).ok())
                    .map(|g| g.current_output)
                    .sum()
            })
            .unwrap_or(0.0);
        let new_cons: f32 = consumers_opt
            .map(|consumers| {
                consumers
                    .iter()
                    .filter_map(|e| q_consumers.get(*e).ok())
                    .map(|c| c.demand)
                    .sum()
            })
            .unwrap_or(0.0);
        // consumers == 0 は停電なし (PowerGrid::default() の仕様に合わせる)
        let new_powered = new_cons == 0.0 || new_gen >= new_cons;

        let gen_changed = (grid.generation - new_gen).abs() > f32::EPSILON;
        let cons_changed = (grid.consumption - new_cons).abs() > f32::EPSILON;
        let powered_changed = grid.powered != new_powered;

        if gen_changed { grid.generation = new_gen; }
        if cons_changed { grid.consumption = new_cons; }

        if powered_changed {
            grid.powered = new_powered;
            if let Some(consumers) = consumers_opt {
                for &consumer in consumers.iter() {
                    if new_powered {
                        commands.entity(consumer).remove::<Unpowered>();
                    } else {
                        commands.entity(consumer).try_insert(Unpowered);
                    }
                }
            }
            info!(
                "[Energy] Grid {} (gen={:.2}W, cons={:.2}W)",
                if new_powered { "POWERED" } else { "BLACKOUT" },
                new_gen, new_cons
            );
        }
    }
}
```

**スケジュール**: `Update`、`.after(soul_spa_power_output_system).in_set(GameSystemSet::Logic)`

---

## Step 5: ランプバフシステム

### `crates/bevy_app/src/systems/energy/lamp_buff.rs` (新規)

```rust
use bevy::prelude::*;
use hw_core::soul::DamnedSoul;
use hw_energy::{LAMP_FATIGUE_RECOVERY_BONUS, LAMP_STRESS_REDUCTION_RATE, OUTDOOR_LAMP_EFFECT_RADIUS, PowerConsumer, Unpowered};

type PoweredLampQuery<'w, 's> =
    Query<'w, 's, &'static Transform, (With<PowerConsumer>, Without<Unpowered>)>;

pub fn lamp_buff_system(
    q_lamps: PoweredLampQuery,
    mut q_souls: Query<(&Transform, &mut DamnedSoul)>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let r2 = OUTDOOR_LAMP_EFFECT_RADIUS * OUTDOOR_LAMP_EFFECT_RADIUS;

    for lamp_tf in q_lamps.iter() {
        let lamp_pos = lamp_tf.translation.truncate();
        for (soul_tf, mut soul) in q_souls.iter_mut() {
            if soul_tf.translation.truncate().distance_squared(lamp_pos) <= r2 {
                soul.stress = (soul.stress - LAMP_STRESS_REDUCTION_RATE * dt).max(0.0);
                soul.fatigue = (soul.fatigue - LAMP_FATIGUE_RECOVERY_BONUS * dt).max(0.0);
            }
        }
    }
}
```

**スケジュール**: `GameSystemSet::Logic`（Update）

---

## Step 6: 視覚フィードバック

### A. `crates/hw_core/src/visual_mirror/energy.rs` (新規)

```rust
use bevy::prelude::*;

/// Outdoor Lamp の電源状態を hw_visual に伝える VisualMirror。
/// Observer が powered/unpowered 遷移に合わせて更新する。
#[derive(Component, Default)]
pub struct PoweredVisualState {
    pub is_powered: bool,
}
```

`crates/hw_core/src/visual_mirror/mod.rs` に追加:
```rust
pub mod energy;
pub use energy::PoweredVisualState;
```

### B. `crates/hw_jobs/src/visual_sync/observers.rs` に追加

```rust
use hw_core::visual_mirror::PoweredVisualState;
use hw_energy::Unpowered;

pub fn on_power_consumer_visual_added(on: On<Add, PowerConsumer>, mut commands: Commands) {
    // PowerConsumer 追加時: 初期値 is_powered=false（Unpowered が付いているため）
    commands.entity(on.entity).try_insert(PoweredVisualState { is_powered: false });
}

pub fn on_unpowered_added(on: On<Add, Unpowered>, mut q: Query<&mut PoweredVisualState>) {
    if let Ok(mut vis) = q.get_mut(on.entity) {
        vis.is_powered = false;
    }
}

pub fn on_unpowered_removed(on: On<Remove, Unpowered>, mut q: Query<&mut PoweredVisualState>) {
    if let Ok(mut vis) = q.get_mut(on.entity) {
        vis.is_powered = true;
    }
}
```

### C. `crates/hw_visual/src/power.rs` (新規)

```rust
use bevy::prelude::*;
use hw_core::visual_mirror::PoweredVisualState;

const COLOR_POWERED: Color = Color::WHITE;
const COLOR_UNPOWERED: Color = Color::srgba(0.4, 0.4, 0.4, 1.0);

/// PoweredVisualState が変化したとき、エンティティ自身および子 Sprite のカラーを更新。
pub fn sync_powered_visual_system(
    q: Query<(Entity, &PoweredVisualState), Changed<PoweredVisualState>>,
    q_children: Query<&Children>,
    mut q_sprites: Query<&mut Sprite>,
) {
    for (entity, vis) in q.iter() {
        let color = if vis.is_powered { COLOR_POWERED } else { COLOR_UNPOWERED };
        if let Ok(mut sprite) = q_sprites.get_mut(entity) {
            sprite.color = color;
        }
        if let Ok(children) = q_children.get(entity) {
            for &child in children.iter() {
                if let Ok(mut sprite) = q_sprites.get_mut(child) {
                    sprite.color = color;
                }
            }
        }
    }
}
```

`crates/hw_visual/src/lib.rs` に `pub mod power;` を追加。

---

## Step 7: Power Status UI

### `crates/bevy_app/src/interface/ui/presentation/builders.rs`

`EntityInspectionQuery` に追加クエリを追加:
- `q_power_consumers: Query<(&PowerConsumer, Option<&Unpowered>, Option<&ConsumesFrom>)>`
- `q_power_grids: Query<&PowerGrid>`

`append_building_model` の末尾に追加:
```rust
if let Ok((consumer, _, consumes_from_opt)) = self.q_power_consumers.get(entity) {
    let grid_line = consumes_from_opt
        .and_then(|cf| self.q_power_grids.get(cf.0).ok())
        .map(|grid| format!(
            "Power: {:.1}W / {:.1}W [{}]",
            grid.generation, grid.consumption,
            if grid.powered { "POWERED" } else { "BLACKOUT" }
        ))
        .unwrap_or_else(|| format!("Power: {:.1}W demand [no grid]", consumer.demand));
    model.push_tooltip(grid_line);
}
```

**注意**: `q_buildings` tuple が長くなる場合は型エイリアスを定義して clippy `type_complexity` 警告を回避すること。

---

## Step 8: メニュー・タスクリスト登録

### `crates/hw_ui/src/setup/submenus.rs`

Temporary サブメニュー（`SandPile` の近傍）に追加:
```rust
MenuEntrySpec::new(
    "Outdoor Lamp",
    MenuAction::SelectBuild(BuildingType::OutdoorLamp),
    button_color,
),
```

### `crates/bevy_app/src/interface/ui/panels/task_list/presenter.rs`

```rust
BuildingType::OutdoorLamp => "Construct Outdoor Lamp".to_string(),
```

---

## Step 9: Plugin 登録

### `crates/bevy_app/src/plugins/logic.rs`

```rust
// Observer 登録
app.add_observer(on_power_consumer_added)         // grid_lifecycle.rs
   .add_observer(on_power_consumer_visual_added)  // visual_sync/observers.rs
   .add_observer(on_unpowered_added)              // visual_sync/observers.rs
   .add_observer(on_unpowered_removed);           // visual_sync/observers.rs

// Update (soul_spa_power_output_system の後)
app.add_systems(Update,
    grid_recalc_system
        .after(soul_spa_power_output_system)
        .in_set(GameSystemSet::Logic),
);

// Logic (Update)
app.add_systems(Update, lamp_buff_system.in_set(GameSystemSet::Logic));

// Visual (Update)
app.add_systems(Update, sync_powered_visual_system.in_set(GameSystemSet::Visual));
```

---

## 変更ファイル一覧

| ファイル | 変更種別 |
|:---|:---|
| `crates/hw_jobs/Cargo.toml` | `hw_energy` 依存追加 |
| `crates/hw_jobs/src/model.rs` | BuildingType::OutdoorLamp 追加 |
| `crates/hw_core/src/visual_mirror/building.rs` | BuildingTypeVisual::OutdoorLamp 追加 |
| `crates/hw_core/src/visual_mirror/energy.rs` | **新規** PoweredVisualState |
| `crates/hw_core/src/visual_mirror/mod.rs` | energy モジュール追加 |
| `crates/hw_jobs/src/visual_sync/mod.rs` | OutdoorLamp → BuildingTypeVisual マップ |
| `crates/hw_jobs/src/visual_sync/observers.rs` | on_power_consumer_visual_added, on_unpowered_added/removed |
| `crates/hw_energy/src/constants.rs` | LAMP_STRESS_REDUCTION_RATE, LAMP_FATIGUE_RECOVERY_BONUS |
| `crates/bevy_app/src/systems/jobs/building_completion/spawn.rs` | OutdoorLamp スプライト + 3D メッシュ |
| `crates/bevy_app/src/systems/jobs/building_completion/post_process.rs` | setup_outdoor_lamp |
| `crates/bevy_app/src/systems/energy/grid_lifecycle.rs` | on_power_consumer_added Observer |
| `crates/bevy_app/src/systems/energy/grid_recalc.rs` | **新規** grid_recalc_system |
| `crates/bevy_app/src/systems/energy/lamp_buff.rs` | **新規** lamp_buff_system |
| `crates/bevy_app/src/systems/energy/mod.rs` | grid_recalc, lamp_buff モジュール追加 |
| `crates/hw_ui/src/setup/submenus.rs` | Outdoor Lamp メニューボタン |
| `crates/bevy_app/src/interface/ui/panels/task_list/presenter.rs` | OutdoorLamp ラベル |
| `crates/bevy_app/src/interface/ui/presentation/builders.rs` | power status UI |
| `crates/hw_visual/src/power.rs` | **新規** sync_powered_visual_system |
| `crates/hw_visual/src/lib.rs` | power モジュール追加 |
| `crates/bevy_app/src/plugins/logic.rs` | Observer + システム登録 |

---

## Completion Criteria

- [ ] `cargo check --workspace --exclude visual_test` エラーなし
- [ ] `cargo clippy --workspace --exclude visual_test` 警告ゼロ
- [ ] Outdoor Lamp が Temporary メニューに表示される
- [ ] Lamp を建設できる（1x1、Bone x2）
- [ ] Lamp 完成時、同一 Yard の PowerGrid に ConsumesFrom が付与される
- [ ] Yard 外のランプは ConsumesFrom なし → 常時 Unpowered（スプライト暗）
- [ ] Grid 再計算: generation/consumption が正しく集計される
- [ ] Blackout: consumption > generation → 全 consumer が Unpowered、スプライト暗
- [ ] Recovery: generation 回復 → Unpowered 除去、スプライト明
- [ ] ランプバフ: 点灯中のランプ半径内 Soul に stress/fatigue 軽減が適用される
- [ ] 停電時はバフ停止（Without<Unpowered> でランプがスキップされる）
- [ ] ランプ撤去 → ConsumesFrom 自動クリーンアップ、consumption 減少、grid 再計算
- [ ] 建物選択パネルに power status 表示

---

## Verification Scenarios

1. **基本**: Soul Spa（Soul 2 体）+ Lamp 3 基 → 全灯 (2.0W gen, 0.6W cons)
2. **Soul 1 体離脱**: 1.0W gen > 0.6W cons → 引き続き全灯
3. **全 Soul 離脱**: 0.0W gen < 0.6W cons → BLACKOUT、スプライト暗、バフ消滅
4. **Soul 復帰**: POWERED 回復 → スプライト明、バフ再開
5. **過負荷**: 2 Soul = 2.0W; 11 Lamp = 2.2W cons → BLACKOUT
6. **Lamp 撤去**: despawn → ConsumesFrom 自動削除 → consumption 減少 → POWERED 復帰
7. **Yard 外 Lamp**: ConsumesFrom なし → 常時暗

---

## AI Handoff

### 前提確認
1. `phase1a-data-model.md` と `phase1b-soul-spa.md` の実装済み内容を把握する
2. `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace --exclude visual_test` がグリーン
3. Soul Spa が Operational になり `PowerGenerator.current_output` が更新されることを確認

### 実装順序
Step 1（型定義）→ Step 2（建設フロー）→ Step 3（Observer）→ Step 4（Grid 再計算）→ Step 5（バフ）→ Step 6（視覚）→ Step 7（UI）→ Step 8-9（メニュー・Plugin 登録）

各 Step 完了時に `cargo check` でグリーンを確認すること。

### 注意事項
- `on_unpowered_removed` は `bevy::ecs::lifecycle::Remove` を use すること（Bevy 0.18）
- `grid_recalc_system` の `consumers == 0` → `powered = true`（空グリッドは停電でない）
- ランプ撤去は既存の despawn フローで動作する。追加の despawn hook は不要
- `q_buildings` tuple が長い場合は型エイリアスを定義する（clippy `type_complexity` 回避）
