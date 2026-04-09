# 夢の泡パーティクル描画負荷最適化

作成日: 2026-04-09

---

## 問題

### 現状（確認済みコード）

`dream_particle_spawn_system`（particle.rs:122–128）と
`rest_area_dream_particle_spawn_system`（particle.rs:244–250）は、
粒子を 1 つスポーンするたびに新しい `Mesh` / `DreamBubbleMaterial` を生成している。

```rust
// particle.rs:122-128  (dream_particle_spawn_system)
let mesh = meshes.add(Circle::new(0.5));
let material = materials.add(DreamBubbleMaterial {
    color,
    time: 0.0,
    alpha: 0.85,
    mass: 1.0,
});
```

さらに `dream_particle_update_system`（particle.rs:319–322）では
毎フレーム各粒子の material を `Assets::get_mut` し、
`time` と `alpha` を per-particle 更新している。

```rust
// particle.rs:319-322
if let Some(material) = materials.get_mut(&material_handle.0) {
    material.time = time.elapsed_secs();
    material.alpha = life_ratio * 0.85;
}
```

### ファイル一覧

| ファイル | 現状 |
|---|---|
| `crates/hw_visual/src/dream/particle.rs` | spawn × 2 + update で全問題が集中 |
| `crates/hw_visual/src/dream/dream_bubble_material.rs` | `DreamBubbleMaterial` に `time: f32` が残っている |
| `assets/shaders/dream_bubble.wgsl` | `material.time` を 6 箇所で参照（L57, L58, L74, L78, L89, L117） |
| `crates/hw_visual/src/dream/components.rs` | `DreamParticle` に bucket フィールドなし |
| `crates/hw_visual/src/lib.rs` | Startup system なし、`DreamBubbleHandles` Resource なし |
| `crates/hw_visual/src/dream/mod.rs` | `handles` モジュールなし |

### 何が重いか

- `Circle::new(0.5)` が粒子数ぶん asset 化される（mesh dedupe なし）
- `DreamBubbleMaterial` が粒子数ぶん asset 化される（material dedupe なし）
- `Assets<DreamBubbleMaterial>::get_mut` を毎フレーム × 粒子数で実行する
- transparent 2D mesh なので、draw call は粒子数に近い規模まで増えうる

### UI 側は別のボトルネック

`DreamBubbleUiMaterial` も per-particle material を毎フレーム mutate しているが、
`UiMaterial` パイプライン側に batching がないため本計画では扱わない。

---

## 前提整理

### 1. Bevy 0.18 の globals uniform で time を参照できる

`bevy_sprite_render-0.18.0/src/mesh2d/mesh2d_view_bindings.wgsl` より:

```wgsl
#define_import_path bevy_sprite::mesh2d_view_bindings

#import bevy_render::view::View
#import bevy_render::globals::Globals

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<uniform> globals: Globals;
```

`Globals.time` = 起動からの経過秒（1 時間で wrap）。
Material2d フラグメントシェーダーで `#import bevy_sprite::mesh2d_view_bindings` すれば
`globals.time` が使える。これで `material.time` の per-frame CPU 更新を根絶できる。

### 2. shared mesh / shared material は batching に有効

Bevy 0.18 の `Transparent2d` sorted phase では、
`(material_bind_group_id, mesh_asset_id)` が一致する隣接エントリをバッチ化する。
同じ handle を共有する粒子が Z 順で連続していれば draw call を削減できる。

ただし sorted phase の batching は「隣接前提」なので、
**material 種類数 = draw call 上限** にはならない（上限保証は目標外）。

### 3. `mass` は全粒子で `1.0` 固定

particle.rs:128 より。shader の `deform_strength = 0.15 + clamp(mass/12.0)*0.10` も
mass=1.0 固定なら実質定数（0.158）なので、pool 化の障害にならない。

---

## 目標

- world-space 泡の `Mesh` 重複生成を排除する
- world-space 泡の per-frame `Assets::get_mut` を 0 回にする
- world-space 泡の material asset 数を 24 本（quality 3 × alpha_bucket 8）に固定する
- 見た目（フェードアウト・呼吸・揺らぎ）を崩さない

**保証しないもの**: draw call 固定上限、UI 泡の削減

---

## 改善方針

### A. Circle mesh 共有

`Circle::new(0.5)` を Startup で 1 回だけ asset 化し `DreamBubbleHandles` Resource に保持。
spawn 時はハンドルを clone するだけ。

### B. `material.time` → `globals.time`（WGSL 側）

WGSL 冒頭の `#import` に `bevy_sprite::mesh2d_view_bindings` を追加し、
全 `material.time` 参照を `globals.time` に置換する。
Rust 側の `DreamBubbleMaterial` から `time: f32` フィールドを削除する。
これで `dream_particle_update_system` から `ResMut<Assets<DreamBubbleMaterial>>` が不要になる。

### C. material プール（quality × alpha_bucket）

spawn 時の `alpha` は常に `0.85`、フェードは update で算出している。
この alpha 値を 8 段階バケットに離散化し、`3 quality × 8 bucket = 24` 本の
shared material を Startup で作成して `DreamBubbleHandles` に保持する。

alpha bucket の定義:
```
bucket b (0–7) の alpha = b as f32 / (8.0 - 1.0) * 0.85
                        ≈ [0.000, 0.121, 0.243, 0.364, 0.486, 0.607, 0.729, 0.850]
```

この定義にすると:

- `bucket = 7` で現行どおり `alpha = 0.85`
- `bucket = 0` で `alpha = 0.0`

spawn 時: `life_ratio = 1.0` → `bucket = 7`（alpha = 0.85）
update 時: `life_ratio` が下がるにつれ bucket が 7→1 に変化するタイミングのみ
`MeshMaterial2d` ハンドルを差し替える。`bucket = 0` に入った粒子は不可視なので、
slot を消費し続けないようその時点で早期 despawn する（毎フレーム `get_mut` しない）。

---

## 実装ステップ（具体的変更）

### Step 1 — `handles.rs` 新規作成

**対象ファイル**: `crates/hw_visual/src/dream/handles.rs`（新規）

```rust
use bevy::prelude::*;
use super::dream_bubble_material::DreamBubbleMaterial;
use hw_core::soul::DreamQuality;

pub const ALPHA_BUCKETS: usize = 8;

#[derive(Resource)]
pub struct DreamBubbleHandles {
    pub circle_mesh: Handle<Mesh>,
    /// materials[quality_index][alpha_bucket]
    /// quality_index: 0=VividDream, 1=NormalDream, 2=NightTerror
    pub materials: [[Handle<DreamBubbleMaterial>; ALPHA_BUCKETS]; 3],
}

pub fn init_dream_bubble_handles(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<DreamBubbleMaterial>>,
) {
    let circle_mesh = meshes.add(Circle::new(0.5));

    const QUALITY_COLORS: [(DreamQuality, LinearRgba); 3] = [
        (DreamQuality::VividDream,   LinearRgba::new(0.55, 0.80, 1.00, 1.0)),
        (DreamQuality::NormalDream,  LinearRgba::new(0.55, 0.65, 0.95, 1.0)),
        (DreamQuality::NightTerror,  LinearRgba::new(0.95, 0.45, 0.55, 1.0)),
    ];

    let pool = std::array::from_fn(|qi| {
        let color = QUALITY_COLORS[qi].1;
        std::array::from_fn(|b| {
            let alpha = b as f32 / (ALPHA_BUCKETS as f32 - 1.0) * 0.85;
            materials.add(DreamBubbleMaterial {
                color,
                alpha,
                mass: 1.0,
            })
        })
    });

    commands.insert_resource(DreamBubbleHandles {
        circle_mesh,
        materials: pool,
    });
}

/// DreamQuality を quality_index (0–2) に変換する
pub fn quality_index(q: DreamQuality) -> usize {
    match q {
        DreamQuality::VividDream  => 0,
        DreamQuality::NormalDream => 1,
        DreamQuality::NightTerror => 2,
        DreamQuality::Awake       => 0, // 呼ばれないはずだが安全側
    }
}

/// life_ratio (0.0–1.0) から alpha_bucket (0–7) を算出する
pub fn life_ratio_to_bucket(life_ratio: f32) -> usize {
    ((life_ratio * ALPHA_BUCKETS as f32).floor() as usize).min(ALPHA_BUCKETS - 1)
}
```

### Step 2 — `DreamBubbleMaterial` から `time` を削除

**対象ファイル**: `crates/hw_visual/src/dream/dream_bubble_material.rs`

- `pub time: f32,`（9 行目付近）を削除する
- `DreamBubbleUiMaterial` の `time` フィールドは **残す**（UI 側は別計画）

変更後:
```rust
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct DreamBubbleMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    #[uniform(0)]
    pub alpha: f32,
    #[uniform(0)]
    pub mass: f32,
}
```

### Step 3 — WGSL シェーダーを `globals.time` に移行

**対象ファイル**: `assets/shaders/dream_bubble.wgsl`

変更箇所 1: import 追加（L4–L6 の `#import` ブロックに追加）

```wgsl
#import bevy_sprite::{
    mesh2d_vertex_output::VertexOutput,
}
#import bevy_sprite::mesh2d_view_bindings::globals
```

変更箇所 2: WGSL struct から `time` フィールド削除（L42–L46）

```wgsl
struct DreamBubbleMaterial {
    color: vec4<f32>,  // offset 0:  ベース色 (16 bytes)
    alpha: f32,        // offset 16: 透明度
    mass: f32,         // offset 20: 質量
    _pad0: f32,        // offset 24: パディング
    _pad1: f32,        // offset 28: パディング
}
```

変更箇所 3: `material.time` 参照をすべて `globals.time` に置換

| 行 | 変更前 | 変更後 |
|---|---|---|
| L57 | `let t = material.time * 0.8;` | `let t = globals.time * 0.8;` |
| L74 | `let breath = 0.85 + 0.15 * sin(material.time * 1.5);` | `let breath = 0.85 + 0.15 * sin(globals.time * 1.5);` |
| L78 | `fbm(p * 2.0 + vec2<f32>(t * 0.8, t * -0.5))` | 変更不要（`t` 経由） |
| L89 | `let iridescent_phase = angle + material.time * 0.6;` | `let iridescent_phase = angle + globals.time * 0.6;` |
| L117 | `let edge_noise = fbm(p * 8.0 - t);` | 変更不要（`t` 経由） |

> `t` に代入している箇所（L57）を直せば `t` 経由の参照は自動的に修正される。
> 直接参照は L74 と L89 の 2 行のみ。

### Step 4 — `DreamParticle` に `alpha_bucket` を追加

**対象ファイル**: `crates/hw_visual/src/dream/components.rs`

```rust
#[derive(Component)]
pub struct DreamParticle {
    pub owner: Entity,
    pub quality: DreamQuality,
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub velocity: Vec2,
    pub phase: f32,
    pub alpha_bucket: usize,   // ← 追加
}
```

### Step 5 — spawn 系を handles 参照に変更

**対象ファイル**: `crates/hw_visual/src/dream/particle.rs`

#### `dream_particle_spawn_system` の変更

パラメータから `ResMut<Assets<Mesh>>` と `ResMut<Assets<DreamBubbleMaterial>>` を削除し、
`handles: Res<DreamBubbleHandles>` を追加する。

```rust
pub fn dream_particle_spawn_system(
    mut commands: Commands,
    time: Res<Time>,
    handles: Res<DreamBubbleHandles>,   // ← 追加
    mut q_souls: SleepingSoulsQuery,
) {
    // ...（既存ロジック）

    // 変更前: let mesh = meshes.add(Circle::new(0.5));
    // 変更前: let material = materials.add(DreamBubbleMaterial { ... });
    let initial_bucket = handles::life_ratio_to_bucket(1.0); // = 7
    let mesh     = handles.circle_mesh.clone();
    let material = handles.materials[handles::quality_index(dream.quality)][initial_bucket].clone();

    commands.spawn((
        DreamParticle {
            owner: soul_entity,
            quality: dream.quality,
            lifetime: particle_lifetime,
            max_lifetime: particle_lifetime,
            velocity,
            phase: rng.gen_range(0.0..=std::f32::consts::TAU),
            alpha_bucket: initial_bucket,  // ← 追加
        },
        Mesh2d(mesh),
        MeshMaterial2d(material),
        // ...
    ));
```

#### `RestAreaDreamParams` の変更

`meshes` と `materials` フィールドを削除し、`handles` フィールドを追加する。

```rust
#[derive(bevy::ecs::system::SystemParam)]
pub struct RestAreaDreamParams<'w, 's> {
    commands: Commands<'w, 's>,
    time: Res<'w, Time>,
    handles: Res<'w, DreamBubbleHandles>,    // ← 追加（meshes/materials を置換）
    q_rest_areas: Query<...>,
    // ...（他フィールドは変更なし）
}
```

spawn 部分（particle.rs:244–250）を同様に handles 参照へ置換する。

### Step 6 — `dream_particle_update_system` の material mutation を削除

**対象ファイル**: `crates/hw_visual/src/dream/particle.rs`

#### パラメータ変更

- `ResMut<Assets<DreamBubbleMaterial>>` を削除
- `&MeshMaterial2d<DreamBubbleMaterial>` を `&mut MeshMaterial2d<DreamBubbleMaterial>` に変更
- `handles: Res<DreamBubbleHandles>` を追加

#### ループ内の変更

削除:
```rust
// particle.rs:319-322（削除）
if let Some(material) = materials.get_mut(&material_handle.0) {
    material.time = time.elapsed_secs();
    material.alpha = life_ratio * 0.85;
}
```

追加（bucket 変化時のみ handle を差し替え）:
```rust
let new_bucket = handles::life_ratio_to_bucket(life_ratio);
if new_bucket == 0 {
    commands.entity(entity).try_despawn();
    if let Ok(mut visual_state) = q_visual_state.get_mut(particle.owner) {
        visual_state.active_particles = visual_state.active_particles.saturating_sub(1);
    }
    continue;
}

if new_bucket != particle.alpha_bucket {
    particle.alpha_bucket = new_bucket;
    let qi = handles::quality_index(particle.quality);
    *material_handle = MeshMaterial2d(handles.materials[qi][new_bucket].clone());
}
```

`time` 更新が消えるため、`materials.get_mut` は毎フレーム 0 回になる。
Handle 差し替えは life_ratio が 8 分の 1 刻みで変わるタイミングのみ（1 粒子あたり最大 6 回）。

### Step 7 — `mod.rs` / `lib.rs` の配線

#### `crates/hw_visual/src/dream/mod.rs` に追加

```rust
mod handles;
pub use handles::{DreamBubbleHandles, ALPHA_BUCKETS};
```

#### `crates/hw_visual/src/lib.rs` に追加

`HwVisualPlugin::build` に Startup system を追加:

```rust
app.add_systems(Startup, dream::init_dream_bubble_handles);
```

`pub use` の追加:

```rust
pub use dream::DreamBubbleHandles;
```

---

## 変更ファイル一覧

| ファイル | 変更内容 |
|---|---|
| `crates/hw_visual/src/dream/handles.rs` | **新規作成**（`DreamBubbleHandles` Resource + 初期化 system + ヘルパー関数） |
| `crates/hw_visual/src/dream/mod.rs` | `handles` モジュール追加・`DreamBubbleHandles` re-export |
| `crates/hw_visual/src/dream/components.rs` | `DreamParticle.alpha_bucket: usize` フィールド追加 |
| `crates/hw_visual/src/dream/dream_bubble_material.rs` | `DreamBubbleMaterial` から `time: f32` 削除 |
| `crates/hw_visual/src/dream/particle.rs` | spawn × 2 を handles 参照へ置換、update から `get_mut` + `time` 更新を削除 |
| `assets/shaders/dream_bubble.wgsl` | `globals` import 追加、`material.time` → `globals.time` 置換（3 箇所）、struct から `time` 削除 |
| `crates/hw_visual/src/lib.rs` | `Startup` に `dream::init_dream_bubble_handles` を追加 |

---

## 期待効果

| 指標 | 改善前 | 改善後 |
|---|---|---|
| world-space mesh asset 数 | 粒子数ぶん | **1 個** |
| world-space material asset 数 | 粒子数ぶん | **24 本固定** |
| world-space per-frame `get_mut` | 粒子数ぶん | **0 回** |
| MeshMaterial2d 差し替え | 毎フレーム（実質 `get_mut` 経由） | **bucket 変化時のみ**（1 粒子あたり最大 7 回） |
| batchability | 低い | **同 quality × 同 bucket の粒子でバッチ候補** |

---

## 検証方法

1. `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`
2. `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace 2>&1 | grep "^warning:" | grep -v generated`
3. 50 体以上を同時睡眠させ、world-space 泡のフェードアウト・呼吸・揺らぎが維持されていることを目視確認
4. alpha bucket 8 段階のフェードが視覚的に許容範囲か確認（目安: 1 段階の alpha 差 ≈ 0.121）
5. RenderDoc または `wgpu-profiler` でスポーン前後の draw call 数を比較
6. RestArea 集中時に UI 泡と world-space 泡が独立して観察できることを確認

---

## 追加候補（本計画の後続）

低コスト順:

1. **offscreen カリング**: viewport 外の Soul / RestArea では泡を spawn しない
2. **ズームアウト間引き**: zoom ratio に応じて emit 間隔を延長する
3. **高密度集約**: RestArea 収容人数が閾値を超えたら大きい 1 粒子へ束ねる
4. **instanced 2D 描画**: `alpha` と `quality` を instance data に載せ、draw call を粒子数 0 依存に

1–3 は数十行で実装可能。4 は伸びしろ最大だが実装コスト高。
