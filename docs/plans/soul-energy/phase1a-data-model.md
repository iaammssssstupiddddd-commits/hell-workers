# Phase 1a: Data Model + Grid Infrastructure

| Item | Value |
|:---|:---|
| Status | Done |
| Depends on | Nothing |
| Blocks | Phase 1b, Phase 1c |

## Goal

Soul Energy 全データ型・ECS Relationship・定数の定義と PowerGrid エンティティ基盤の構築。
ゲームプレイ変更・UI 変更なし — 純粋な基盤実装。

---

## 変更ファイル一覧

| 操作 | ファイル |
|:---|:---|
| **新規作成** | `crates/hw_core/src/constants/energy.rs` |
| **修正** | `crates/hw_core/src/constants/mod.rs` |
| **新規作成** | `crates/hw_core/src/energy.rs` |
| **修正** | `crates/hw_core/src/lib.rs` |
| **修正** | `crates/hw_core/src/relationships.rs` |
| **新規作成** | `crates/bevy_app/src/systems/energy/mod.rs` |
| **新規作成** | `crates/bevy_app/src/systems/energy/grid_lifecycle.rs` |
| **修正** | `crates/bevy_app/src/systems/mod.rs` |
| **修正** | `crates/bevy_app/src/plugins/logic.rs` |

---

## 実装手順

### Step 1: エネルギー定数

**新規作成** `crates/hw_core/src/constants/energy.rs`
（スタイル参考: `logistics.rs` の日本語コメント + フラットな `pub const` 並び）

```rust
//! Soul Energy・発電・消費の定数

/// Soul 1 体が 1 秒間に生成する発電量（基準値）
pub const OUTPUT_PER_SOUL: f32 = 1.0;

/// 発電中の Soul が 1 秒間に消費する Dream 量
pub const DREAM_CONSUME_RATE_GENERATING: f32 = 0.5;

/// この値を下回ったら GeneratePower タスクを自動終了
/// （参考: logistics.rs の REFINE 系終了閾値パターン）
pub const DREAM_GENERATE_FLOOR: f32 = 10.0;

/// 屋外ランプ 1 基の電力需要。1 Soul = 5 基まで点灯
pub const OUTDOOR_LAMP_DEMAND: f32 = OUTPUT_PER_SOUL * 0.2;

/// 屋外ランプの照明効果半径（タイル単位）
pub const OUTDOOR_LAMP_EFFECT_RADIUS: f32 = 5.0;

/// Soul Spa のタイル 1 枚あたり建設コスト（Bone）。2×2 = 合計 12
pub const SOUL_SPA_BONE_COST_PER_TILE: u32 = 3;

/// 発電中の疲労蓄積レート（/秒）
/// 参考: ai.rs の FATIGUE_WORK_RATE = 0.01。瞑想的な行為のため半分程度
pub const FATIGUE_RATE_GENERATING: f32 = 0.005;
```

**修正** `crates/hw_core/src/constants/mod.rs` — 既存の最後の `mod`/`pub use` ペアの直後に追記:

```rust
mod energy;
pub use energy::*;
```

---

### Step 2: エネルギーコンポーネント

**新規作成** `crates/hw_core/src/energy.rs`

```rust
use bevy::prelude::*;

/// Yard の電力網エンティティ。定期的に再計算される。
/// Yard 追加 Observer によって 1 対 1 で自動生成される。
/// 初期状態: generation=0, consumption=0, powered=true（消費者なし＝停電ではない）
#[derive(Component, Reflect, Debug, Clone)]
#[reflect(Component)]
pub struct PowerGrid {
    /// 接続全 PowerGenerator の current_output 合計
    pub generation: f32,
    /// 接続全 PowerConsumer の demand 合計
    pub consumption: f32,
    /// generation >= consumption のとき true
    pub powered: bool,
}

impl Default for PowerGrid {
    fn default() -> Self {
        Self {
            generation: 0.0,
            consumption: 0.0,
            powered: true, // 空グリッドは powered（消費者がいない＝停電ではない）
        }
    }
}

/// SoulSpaSite に付与。サイト単位の発電集計。
/// Phase 1b で SoulSpaSite スポーン時に追加される（ここでは型定義のみ）。
#[derive(Component, Reflect, Debug, Default, Clone)]
#[reflect(Component)]
pub struct PowerGenerator {
    /// 実際の出力: 占有スロット数 × output_per_soul
    pub current_output: f32,
    /// Soul 1 体あたりの発電量。通常は OUTPUT_PER_SOUL 定数と同値。
    /// フィールドとして保持する理由: 将来の上位施設（効率の良い Soul Spa 等）で
    /// 施設ごとに異なる値を設定可能にするため。
    pub output_per_soul: f32,
}

/// 電力消費建物（OutdoorLamp 等）に付与。
/// `#[require(Unpowered)]` により、グリッド接続前はデフォルトで停電状態になる。
#[derive(Component, Reflect, Debug, Clone)]
#[reflect(Component)]
#[require(Unpowered)]
pub struct PowerConsumer {
    /// 稼働時の消費電力（/秒）
    pub demand: f32,
}

/// マーカー: この Consumer は電力供給を受けていない。
/// `#[require(Unpowered)]` によりデフォルトで付与。
/// グリッド再計算で供給が確認されると除去され、停電時に再挿入される。
#[derive(Component, Reflect, Debug, Default, Clone, Copy)]
#[reflect(Component)]
pub struct Unpowered;

/// PowerGrid エンティティ上に付与。所属する Yard への逆参照。
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
pub struct YardPowerGrid(pub Entity);

impl Default for YardPowerGrid {
    fn default() -> Self {
        Self(Entity::PLACEHOLDER)
    }
}
```

**設計メモ**:
- `PowerConsumer` は `#[require(Unpowered)]` — 安全なデフォルト（未接続 = 停電）
- `SoulSpaSite` への `#[require(PowerGenerator)]` は **Phase 1b** で追加
- Relationship (`GeneratesFor`/`ConsumesFrom`) は `#[require(...)]` に含めない — 接続時に明示的 `insert` することで `Entity::PLACEHOLDER` が実データに混入するのを防ぐ

**修正** `crates/hw_core/src/lib.rs` — `events` と `familiar` の間に追加（アルファベット順）:

```rust
pub mod energy;      // ← ここに追加
```

---

### Step 3: ECS Relationships

**修正** `crates/hw_core/src/relationships.rs` — ファイル末尾に追記
（パターン参考: `RestingIn`/`RestAreaOccupants` の derive + `Default` + ヘルパーメソッド構成）

```rust
// ----- Soul Energy: GeneratesFor / GridGenerators -----

/// SoulSpaSite → PowerGrid。発電機としてグリッドに登録する。
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
#[relationship(relationship_target = GridGenerators)]
pub struct GeneratesFor(pub Entity);

impl Default for GeneratesFor {
    fn default() -> Self {
        Self(Entity::PLACEHOLDER)
    }
}

/// GeneratesFor の自動管理逆参照。PowerGrid エンティティ上に付与される。
#[derive(Component, Reflect, Debug, Default)]
#[reflect(Component)]
#[relationship_target(relationship = GeneratesFor)]
pub struct GridGenerators(Vec<Entity>);

impl GridGenerators {
    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        self.0.iter()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

// ----- Soul Energy: ConsumesFrom / GridConsumers -----

/// OutdoorLamp 等 → PowerGrid。消費者としてグリッドに登録する。
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
#[relationship(relationship_target = GridConsumers)]
pub struct ConsumesFrom(pub Entity);

impl Default for ConsumesFrom {
    fn default() -> Self {
        Self(Entity::PLACEHOLDER)
    }
}

/// ConsumesFrom の自動管理逆参照。PowerGrid エンティティ上に付与される。
#[derive(Component, Reflect, Debug, Default)]
#[reflect(Component)]
#[relationship_target(relationship = ConsumesFrom)]
pub struct GridConsumers(Vec<Entity>);

impl GridConsumers {
    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        self.0.iter()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
```

---

### Step 4: PowerGrid ライフサイクル Observer

**新規作成** `crates/bevy_app/src/systems/energy/mod.rs`:

```rust
pub mod grid_lifecycle;
```

**新規作成** `crates/bevy_app/src/systems/energy/grid_lifecycle.rs`:
（パターン参考: `hw_world/src/room_systems.rs` の `on_building_added` / `on_building_removed`）

```rust
use bevy::prelude::*;
use hw_core::energy::{PowerGrid, YardPowerGrid};
use hw_world::zones::Yard;

/// Yard が追加されたとき PowerGrid エンティティをスポーン。
pub fn on_yard_added(on: On<Add, Yard>, mut commands: Commands) {
    let yard_entity = on.entity;
    commands.spawn((
        Name::new("PowerGrid"),
        PowerGrid::default(),
        YardPowerGrid(yard_entity),
    ));
    info!("[Energy] PowerGrid spawned for Yard {:?}", yard_entity);
}

/// Yard が削除されたとき対応する PowerGrid をデスポーン。
/// PowerGrid despawn 時、Bevy が `GeneratesFor`/`ConsumesFrom` Source を
/// 参照元エンティティから自動削除する。明示的なクリーンアップは不要。
pub fn on_yard_removed(
    on: On<Remove, Yard>,
    q_grids: Query<(Entity, &YardPowerGrid)>,
    mut commands: Commands,
) {
    let yard_entity = on.entity;
    for (grid_entity, yard_ref) in &q_grids {
        if yard_ref.0 == yard_entity {
            commands.entity(grid_entity).despawn();
            break;
        }
    }
}
```

**修正** `crates/bevy_app/src/systems/mod.rs` — 既存 `pub mod` 一覧に追加:

```rust
pub mod energy;
```

**修正** `crates/bevy_app/src/plugins/logic.rs`

`use` ブロックに追加:
```rust
use crate::systems::energy::grid_lifecycle::{on_yard_added, on_yard_removed};
```

`add_observer` ブロック（151 行目付近）に追加:
```rust
.add_observer(on_yard_added)
.add_observer(on_yard_removed)
```

---

### Step 5: Reflect 型登録

`crates/bevy_app/src/entities/damned_soul/mod.rs` の `register_type` ブロック（82 行目付近）を参考に、
energy 専用の登録を **energy plugin** または `logic.rs` 内に追加:

```rust
app.register_type::<PowerGrid>()
    .register_type::<PowerGenerator>()
    .register_type::<PowerConsumer>()
    .register_type::<Unpowered>()
    .register_type::<YardPowerGrid>()
    .register_type::<GeneratesFor>()
    .register_type::<GridGenerators>()
    .register_type::<ConsumesFrom>()
    .register_type::<GridConsumers>();
```

---

## 完了基準

- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` がエラーなしで通る
- [ ] `cargo clippy --workspace` で警告 0 件
- [ ] Yard 生成時に PowerGrid エンティティがスポーンされる（`on_yard_added` の `info!` ログで確認）
- [ ] 既存ゲームプレイへの影響なし（`cargo run` で通常起動・動作確認）

---

## AI Handoff

### 読む順番
1. このファイル + `milestone-roadmap.md`
2. `crates/hw_core/src/constants/logistics.rs` — 定数ファイルのスタイル（コメント・命名）
3. `crates/hw_core/src/relationships.rs` — derive パターン・`Default` 実装・ヘルパーメソッド
4. `crates/bevy_app/src/plugins/logic.rs` — `add_observer` ブロックの位置（150 行目付近）
5. `crates/bevy_app/src/systems/mod.rs` — `pub mod` 一覧（`energy` 追加箇所の確認）

### 既に決定済みの設計
| 決定事項 | 内容 |
|:---|:---|
| `PowerConsumer` のデフォルト | `#[require(Unpowered)]` で未接続 = 停電（安全側デフォルト） |
| `SoulSpaSite` への `#[require(PowerGenerator)]` | Phase 1b で追加（Phase 1a では型定義のみ） |
| Relationship は `#[require]` に含めない | 接続時に明示的 `insert` → `Entity::PLACEHOLDER` の実データ混入を防ぐ |
| Yard → PowerGrid 逆引き | `Query<(Entity, &YardPowerGrid)>` をフィルタリング（逆参照コンポーネントは不要） |
| PowerGrid の存在タイミング | Yard 存在中は常に存在（generation=0 / consumption=0 の空グリッドも有効） |

### 注意点
- `hw_core/src/lib.rs` への `pub mod energy;` はアルファベット順（`events` と `familiar` の間）
- Observer API: `On<Add, T>` の entity 取得は `on.entity`（フィールド、メソッドではない）。参照: `hw_jobs/src/visual_sync/observers.rs`
- `register_type` の追加場所は `entities/damned_soul/mod.rs` 82 行目付近を参考に決定する
- `hw_world` は `bevy_app` の依存クレートに既に含まれているため `Cargo.toml` 修正は不要
