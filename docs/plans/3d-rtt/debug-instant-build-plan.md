# Debug Instant Build ボタン 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `debug-instant-build-plan` |
| ステータス | `Ready` |
| 作成日 | `2026-03-18` |
| 目的 | MS-P3-Pre-C Camera3d 角度目視確認のための壁即時完成デバッグ機能 |

---

## 1. 目的

タスクシステム（ワーカーの作業キュー）を経由せずに壁建築物を即時完成させるデバッグトグルを追加する。

**課題**: Camera3d 角度の目視確認に壁の 3D ビジュアルが必要だが、
現状は Blueprint 配置後にワーカーがフレーミング → コーティングを完了するまで
完成済み壁 (`wall_material`) が表示されない。

**到達状態**: DevPanel の "IBuild: ON" トグルを有効にして壁を配置すると、
次フレームには完成済み `Building(Wall)` + `Building3dVisual(wall_material)` が表示される。
既に配置済みの provisional 壁も即時昇格する。

---

## 2. 現状フロー（変更前）

```
WallPlace ドラッグ
  → apply_wall_placement()
      → WallConstructionSite + WallTileBlueprint spawn（Blueprint 状態）
          ↓（Logic フェーズ、.chain() 順）
          wall_construction_cancellation_system
          wall_framed_tile_spawn_system
              → WallTileState::FramedProvisional のタイルに Building(provisional) + Building3dVisual(provisional_material) spawn
          wall_construction_phase_transition_system
              → 全タイルが FramedProvisional になったら Coating フェーズへ
          wall_construction_completion_system
              → WallTileState::Complete かつ phase == Coating → ProvisionalWall 除去・site/tile despawn
```

---

## 3. 追加するもの

### 3-1. Resource（`main.rs`）

`DebugVisible` / `Render3dVisible` が定義されている箇所に追加：

```rust
#[derive(Resource, Default)]
pub struct DebugInstantBuild(pub bool);
```

`init_resource::<DebugVisible>()` / `init_resource::<Render3dVisible>()` と並べて追加：

```rust
.init_resource::<DebugInstantBuild>()
```

### 3-2. UI ボタン（`dev_panel.rs`）

既存の `ToggleRender3dButton` と同パターンで追加：

- マーカー: `InstantBuildButton`
- ボタンテキスト: `"IBuild: OFF"` / `"IBuild: ON"`
- 色: OFF = 暗グレー（`srgb(0.25, 0.25, 0.25)`）、ON = 暗橙（`srgb(0.35, 0.20, 0.05)`）
- ボーダー: OFF = `srgb(0.45, 0.45, 0.45)`、ON = `srgb(0.60, 0.35, 0.10)`
- 追加する関数:
  - `toggle_instant_build_button_system` — `Interaction::Pressed` で `DebugInstantBuild.0` トグル
  - `update_instant_build_button_visual_system` — `is_changed()` で色・テキスト更新

> `spawn_dev_panel_system` の `.with_children` ブロックで、既存の `ToggleRender3dButton` の後ろに追加。

### 3-3. Logic システム（`interface_debug.rs` に追記）

**ファイル**: `crates/bevy_app/src/plugins/interface_debug.rs`
（`debug_spawn_system` と同ファイル。分離するほどの量でないため）

**必要な use 文**（既存インポートに追加）：

```rust
use crate::plugins::startup::Building3dHandles;
use crate::systems::jobs::wall_construction::components::{
    WallConstructionPhase, WallConstructionSite, WallTileBlueprint, WallTileState,
};
use crate::systems::jobs::{Building, BuildingType, ProvisionalWall};
use crate::world::map::{WorldMap, WorldMapWrite};
use hw_core::constants::{TILE_SIZE, Z_MAP};
use hw_visual::visual3d::Building3dVisual;
```

**システム本体**：

```rust
pub fn debug_instant_complete_walls_system(
    debug: Res<crate::DebugInstantBuild>,
    mut q_sites: Query<(Entity, &mut WallConstructionSite)>,
    mut q_tiles: Query<(Entity, &mut WallTileBlueprint)>,
    mut q_buildings: Query<&mut Building>,
    handles_3d: Res<Building3dHandles>,
    mut world_map: WorldMapWrite,
    mut commands: Commands,
) {
    if !debug.0 {
        return;
    }

    for (site_entity, mut site) in q_sites.iter_mut() {
        let mut framed_count = 0u32;

        for (_, mut tile) in q_tiles
            .iter_mut()
            .filter(|(_, t)| t.parent_site == site_entity)
        {
            if tile.spawned_wall.is_none() {
                // 未 spawn（フレーミング前） → 完成済み Building + 3D visual を直接 spawn
                let world_pos = WorldMap::grid_to_world(tile.grid_pos.0, tile.grid_pos.1);
                let wall_entity = commands
                    .spawn((
                        Building {
                            kind: BuildingType::Wall,
                            is_provisional: false,
                        },
                        Transform::from_translation(world_pos.extend(Z_MAP + 0.01)),
                        Visibility::default(),
                        Name::new("Building (Wall)"),
                    ))
                    .id();
                commands.spawn((
                    Mesh3d(handles_3d.wall_mesh.clone()),
                    MeshMaterial3d(handles_3d.wall_material.clone()),
                    Transform::from_xyz(world_pos.x, TILE_SIZE / 2.0, -world_pos.y),
                    handles_3d.render_layers.clone(),
                    Building3dVisual { owner: wall_entity },
                    Name::new("Building3dVisual (Wall)"),
                ));
                world_map.reserve_building_footprint(
                    BuildingType::Wall,
                    wall_entity,
                    std::iter::once(tile.grid_pos),
                );
                tile.spawned_wall = Some(wall_entity);
            } else if let Some(wall_entity) = tile.spawned_wall {
                // 既存 provisional 壁 → 完成済みに昇格
                if let Ok(mut building) = q_buildings.get_mut(wall_entity) {
                    building.is_provisional = false;
                }
                commands.entity(wall_entity).remove::<ProvisionalWall>();
            }

            tile.state = WallTileState::Complete;
            framed_count += 1;
        }

        // カウンタを揃えて completion_system のログを正確にする
        site.tiles_framed = framed_count;
        site.tiles_coated = framed_count;
        // Coating フェーズに強制移行 → wall_construction_completion_system が同フレームで cleanup
        site.phase = WallConstructionPhase::Coating;
    }
}
```

### 3-4. システム登録

#### `interface.rs` — UI ボタン systems

```rust
use crate::interface::ui::dev_panel::{
    toggle_instant_build_button_system,
    toggle_render3d_button_system,
    update_instant_build_button_visual_system,
    update_render3d_button_visual_system,
};
// ...
(
    toggle_render3d_button_system,
    update_render3d_button_visual_system,
    toggle_instant_build_button_system,
    update_instant_build_button_visual_system,
)
    .in_set(GameSystemSet::Interface),
```

#### `logic.rs` — wall construction chain への挿入

現在のチェーン（`logic.rs`）：

```rust
wall_construction_cancellation_system,
wall_framed_tile_spawn_system,
wall_construction_phase_transition_system,
wall_construction_completion_system,
```

`debug_instant_complete_walls_system` を `wall_framed_tile_spawn_system` の **直前** に挿入する
（全状態のタイルを Complete にしてから framed_spawn が空振りするように）：

```rust
use crate::plugins::interface_debug::debug_instant_complete_walls_system;
// ...
wall_construction_cancellation_system,
debug_instant_complete_walls_system
    .run_if(|d: Res<crate::DebugInstantBuild>| d.0),
wall_framed_tile_spawn_system,
wall_construction_phase_transition_system,
wall_construction_completion_system,
```

> `.chain()` は維持する。`debug_instant_complete_walls_system` が全タイルを `Complete` かつ
> `phase = Coating` にセットした後:
> - `wall_framed_tile_spawn_system`: `state != FramedProvisional || spawned_wall.is_some()` でスキップ ✓
> - `wall_construction_phase_transition_system`: `phase != Framing` でスキップ ✓
> - `wall_construction_completion_system`: `all_complete && phase == Coating` で cleanup 実行 ✓

---

## 4. 変更ファイル一覧

| ファイル | 変更内容 |
| --- | --- |
| `crates/bevy_app/src/main.rs` | `DebugInstantBuild` struct + `init_resource` を `DebugVisible` 近傍に追加 |
| `crates/bevy_app/src/interface/ui/dev_panel.rs` | `InstantBuildButton` マーカー + spawn + toggle/visual 関数追加 |
| `crates/bevy_app/src/plugins/interface.rs` | `toggle_instant_build_button_system` / `update_instant_build_button_visual_system` 登録 |
| `crates/bevy_app/src/plugins/interface_debug.rs` | `debug_instant_complete_walls_system` 追加（use 文含む） |
| `crates/bevy_app/src/plugins/logic.rs` | chain に `debug_instant_complete_walls_system` を `wall_framed_tile_spawn_system` 直前に挿入 |

---

## 5. 完了条件

- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` ゼロエラー
- [ ] "IBuild: OFF" 状態で壁配置 → 通常フロー（ワーカーが作業）
- [ ] "IBuild: ON" 状態で壁配置 → 次フレームで完成済み壁（`wall_material` 色）が表示される
- [ ] "IBuild: ON" 中に既存 provisional 壁（フレーミング済み）が存在する場合も即時昇格する
- [ ] 3D ビジュアルが Camera3d の RTT に正しく描画される

---

## 6. 非対象

- Floor 建築の instant complete（必要になれば同パターンで追加）
- ワーカーが進行中タスクを持っている場合のキャンセル処理
  （ワーカーは次フレームでタイルが消えたことを検知し、タスクを自然解放する）
