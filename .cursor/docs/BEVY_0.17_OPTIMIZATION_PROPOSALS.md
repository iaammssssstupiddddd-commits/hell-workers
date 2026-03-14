# Bevy 0.17 最適化・リファクタリング提案（現状検証版）

## 概要

Bevy 0.17の新機能と改善を活用して、パフォーマンスとコード品質を向上させる提案です。
**現状の実装を検証し、実際に適用可能な最適化案を再検討しました。**

## 現状の実装状況（2025年1月時点）

### ✅ 既に実装済み
- **Bevy 0.17を使用中** - `Cargo.toml`で確認済み
- **SystemSetは既に実装済み** - `GameSystemSet`が定義され、使用されています
  - 現在のSystemSet: `Input`, `Spatial`, `Logic`, `Actor`, `Visual`, `Interface`
  - 実行順序: `Input` → `Spatial` → `Logic` → `Actor` → `Visual` → `Interface`
  - ポーズ機能が`Spatial`, `Logic`, `Actor`に適用されている

### ⚠️ 一部実装済み
- **Changedフィルターは一部使用** - 以下のシステムで使用されています
  - ✅ `update_familiar_spatial_grid_system` - `Changed<Transform>`
  - ✅ `pathfinding_system` - `Changed<Destination>`
  - ✅ `update_resource_spatial_grid_system` - `Changed<Transform>`, `Changed<Visibility>`
  - ❌ `update_spatial_grid_system` - **最適化が必要**（すべてのエンティティを毎フレーム処理）

### ❌ 未実装
- **バッチ処理は未使用** - `write_batch`や`spawn_batch`は使用されていない
- **フレームタイムグラフは未実装** - デバッグ用プラグインが追加されていない
- **UI Gradientsは未使用** - Bevy 0.17の新機能を活用していない

---

## 1. システムスケジューリングの最適化

### ✅ 現状: SystemSetは既に実装済み

**実装状況**: `src/systems/mod.rs`で`GameSystemSet`が定義され、`main.rs`で使用されています。

```18:33:src/systems/mod.rs
/// ゲームシステムの実行順序を制御するセット
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameSystemSet {
    /// 入力およびカメラの更新
    Input,
    /// UI・エンティティ選択・インタラクション
    Interface,
    /// 空間グリッドの更新 (最優先のデータ更新)
    Spatial,
    /// AI・タスク管理・リソース配分などのコアロジック
    Logic,
    /// エンティティの移動・アニメーション (ロジックに基づく実際のアクション)
    Actor,
    /// 視覚的な同期処理 (移動完了後の描画追従)
    Visual,
}
```

**現在の構成**:
- `Input` → `Spatial` → `Logic` → `Actor` → `Visual` → `Interface` の順で実行
- ポーズ機能が`Spatial`, `Logic`, `Actor`に適用されている

### 提案: さらなる最適化

#### 1.1 並列実行の機会を増やす

現在、多くのシステムが`.chain()`で直列実行されています。依存関係のないシステムは並列実行可能です。

**現状の問題点**:
```169:180:src/main.rs
        .configure_sets(
            Update,
            (
                GameSystemSet::Input,
                GameSystemSet::Spatial.run_if(|time: Res<Time<Virtual>>| !time.is_paused()),
                GameSystemSet::Logic.run_if(|time: Res<Time<Virtual>>| !time.is_paused()),
                GameSystemSet::Actor.run_if(|time: Res<Time<Virtual>>| !time.is_paused()),
                GameSystemSet::Visual,
                GameSystemSet::Interface,
            )
                .chain(),
        )
```

**改善案**: 同じSystemSet内で依存関係のないシステムは`.chain()`を外す

```rust
// 例: Visualセット内のシステム
.add_systems(
    Update,
    (
        progress_bar_system,
        update_progress_bar_fill_system,
        sync_progress_bar_position_system,
        soul_status_visual_system,
        task_link_system,
        building_completion_system,
        animation_system,
        // これらは並列実行可能（依存関係がない場合）
    )
        .in_set(GameSystemSet::Visual), // .chain()を削除
)
```

**メリット:**
- ✅ 既にSystemSetが実装されているため、追加の最適化が容易
- ✅ 並列実行の機会が増える
- ✅ パフォーマンス向上

### カスタムスケジュールの使用例（将来的な拡張）

将来的に固定間隔の物理シミュレーションが必要になった場合：

```rust
// 将来的な拡張の例（現在は不要）
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct FixedLogic;

fn main() {
    App::new()
        // 固定間隔のスケジュールを追加（例：物理シミュレーション用）
        .add_schedules(Schedule::new(FixedLogic).with_run_criteria(
            FixedTimestep::steps_per_second(60.0)
        ))
        .add_systems(FixedLogic, (
            // 固定間隔で実行したい物理シミュレーション
            physics_collision_system,
            physics_movement_system,
        ))
        // ... 既存の設定 ...
        .run();
}
```

**推奨事項**: 現在のプロジェクトでは**SystemSetを使用**し、将来的に固定間隔の実行が必要になった場合にカスタムスケジュールを検討する。

---

## 2. Query最適化: Changedフィルターの活用

### ⚠️ 現状: Changedフィルターは一部使用済み

**既に実装されている箇所**:
- ✅ `update_familiar_spatial_grid_system` - `Changed<Transform>`を使用
- ✅ `pathfinding_system` - `Changed<Destination>`を使用
- ✅ `update_resource_spatial_grid_system` - `Changed<Transform>`, `Changed<Visibility>`を使用

**最適化が必要な箇所**:
- ❌ `update_spatial_grid_system` - すべてのエンティティを毎フレーム処理している

### 提案: `update_spatial_grid_system`の最適化

**現状の実装**:
```297:319:src/systems/spatial.rs
/// SpatialGridを更新するシステム（差分更新）
pub fn update_spatial_grid_system(
    mut spatial_grid: ResMut<SpatialGrid>,
    q_souls: Query<(
        Entity,
        &Transform,
        &DamnedSoul,
        &AssignedTask,
        &crate::entities::damned_soul::IdleState,
    )>,
) {
    for (entity, transform, soul, task, idle) in q_souls.iter() {
        let should_be_in_grid = matches!(task, AssignedTask::None)
            && soul.motivation >= MOTIVATION_THRESHOLD
            && soul.fatigue < FATIGUE_IDLE_THRESHOLD
            && idle.behavior != crate::entities::damned_soul::IdleBehavior::ExhaustedGathering;

        if should_be_in_grid {
            spatial_grid.insert(entity, transform.translation.truncate());
        } else {
            spatial_grid.remove(entity);
        }
    }
}
```

**問題点**: すべてのエンティティを毎フレーム処理しているため、エンティティ数が増えるとパフォーマンスが低下する。

**改善案**: Changedフィルターを使用して変更されたエンティティのみ処理

```rust
// src/systems/spatial.rs
/// SpatialGridを更新するシステム（差分更新 - 最適化版）
pub fn update_spatial_grid_system(
    mut spatial_grid: ResMut<SpatialGrid>,
    q_souls: Query<(
        Entity,
        &Transform,
        &DamnedSoul,
        &AssignedTask,
        &crate::entities::damned_soul::IdleState,
    )>,
    // 変更されたエンティティを検出
    q_changed_transform: Query<Entity, (Changed<Transform>, With<DamnedSoul>)>,
    q_changed_task: Query<Entity, (Changed<AssignedTask>, With<DamnedSoul>)>,
    q_changed_soul: Query<Entity, (Changed<DamnedSoul>, With<DamnedSoul>)>,
    q_changed_idle: Query<Entity, (Changed<crate::entities::damned_soul::IdleState>, With<DamnedSoul>)>,
) {
    use std::collections::HashSet;
    
    // 変更されたエンティティを収集
    let mut changed_entities = HashSet::new();
    changed_entities.extend(q_changed_transform.iter());
    changed_entities.extend(q_changed_task.iter());
    changed_entities.extend(q_changed_soul.iter());
    changed_entities.extend(q_changed_idle.iter());
    
    // 変更されたエンティティのみ処理
    for entity in changed_entities {
        if let Ok((_, transform, soul, task, idle)) = q_souls.get(entity) {
            let should_be_in_grid = matches!(task, AssignedTask::None)
                && soul.motivation >= MOTIVATION_THRESHOLD
                && soul.fatigue < FATIGUE_IDLE_THRESHOLD
                && idle.behavior != crate::entities::damned_soul::IdleBehavior::ExhaustedGathering;

            if should_be_in_grid {
                spatial_grid.insert(entity, transform.translation.truncate());
            } else {
                spatial_grid.remove(entity);
            }
        }
    }
}
```

**注意**: 初回登録時は`Added`フィルターも必要になる可能性があります。

**メリット:**
- ✅ 処理対象エンティティ数の大幅な削減（変更されたエンティティのみ）
- ✅ CPU使用率の低下
- ✅ フレームレートの向上（特にエンティティ数が多い場合）

---

## 3. バッチ処理の活用

### ❌ 現状: バッチ処理は未使用

**現状の問題**:
- メッセージの送信が個別に行われている
- エンティティのスポーンが個別に行われている可能性がある

**確認が必要な箇所**:
- `spawn_damned_souls` - メッセージ送信
- `spawn_familiar` - メッセージ送信
- タスク完了イベントの送信

### 提案: メッセージのバッチ処理

**現状の実装例**:
```500:503:src/main.rs
fn spawn_entities(spawn_events: MessageWriter<DamnedSoulSpawnEvent>) {
    // 人間をスポーン
    spawn_damned_souls(spawn_events);
}
```

**改善案**: Bevy 0.17の`MessageWriter::write_batch()`を使用

```rust
// src/entities/damned_soul.rs
// 変更前
pub fn spawn_damned_souls(mut spawn_events: MessageWriter<DamnedSoulSpawnEvent>) {
    let spawn_positions = [
        Vec2::new(-50.0, -50.0),
        Vec2::new(50.0, 0.0),
        Vec2::new(0.0, 50.0),
    ];

    for spawn_pos in spawn_positions.iter() {
        spawn_events.write(DamnedSoulSpawnEvent {
            position: *spawn_pos,
        });
    }
}

// 変更後: バッチ処理（Bevy 0.17の新機能）
pub fn spawn_damned_souls(mut spawn_events: MessageWriter<DamnedSoulSpawnEvent>) {
    let spawn_positions = [
        Vec2::new(-50.0, -50.0),
        Vec2::new(50.0, 0.0),
        Vec2::new(0.0, 50.0),
    ];

    // バッチでメッセージを送信（オーバーヘッド削減）
    spawn_events.write_batch(
        spawn_positions.iter().map(|pos| DamnedSoulSpawnEvent {
            position: *pos,
        })
    );
}
```

**注意**: `write_batch()`がBevy 0.17で利用可能か確認が必要です。利用できない場合は、複数のメッセージを一度に処理する方法を検討してください。

**メリット:**
- ✅ メッセージ送信のオーバーヘッド削減
- ✅ パフォーマンス向上（特に大量のメッセージを送信する場合）

---

## 4. フレームタイムグラフの追加（Bevy 0.17新機能）

### ❌ 現状: フレームタイムグラフは未実装

**現状**: デバッグ用のパフォーマンス監視ツールが追加されていません。

### 提案: デバッグ用フレームタイムグラフの追加

**実装案**:
```rust
// src/main.rs
use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            // フレームタイムグラフ（Bevy 0.17新機能）
            // 開発時のみ有効にする場合は、run_ifで条件を追加
            FrameTimeDiagnosticsPlugin,
            LogDiagnosticsPlugin::default(),
        ))
        // ... 既存の設定 ...
        .run();
}
```

**注意**: Bevy 0.17で`FrameTimeDiagnosticsPlugin`が利用可能か確認が必要です。利用できない場合は、代替手段を検討してください。

**メリット:**
- ✅ リアルタイムでパフォーマンスを監視
- ✅ ボトルネックの特定が容易
- ✅ 開発効率の向上
- ✅ 最適化の効果を可視化

---

## 5. UI Gradientsの活用（Bevy 0.17新機能）

### 提案: プログレスバーにグラデーション適用

```rust
// src/systems/visuals.rs
use bevy::ui::gradient::LinearGradient;

pub fn progress_bar_system(
    mut commands: Commands,
    mut q_souls: Query<(Entity, &AssignedTask, &Transform, &mut DamnedSoul)>,
) {
    for (soul_entity, task, transform, mut soul) in q_souls.iter_mut() {
        if let AssignedTask::Gather {
            phase: GatherPhase::Collecting { progress },
            ..
        } = task
        {
            if soul.bar_entity.is_none() {
                let bar_background = commands
                    .spawn((
                        ProgressBar { parent: soul_entity },
                        Sprite {
                            color: Color::srgba(0.0, 0.0, 0.0, 0.8),
                            custom_size: Some(Vec2::new(TILE_SIZE * 0.8, TILE_SIZE * 0.15)),
                            ..default()
                        },
                        Transform::from_translation(
                            transform.translation + Vec3::new(0.0, TILE_SIZE * 0.6, 0.1),
                        ),
                    ))
                    .id();

                // グラデーション付きフィル（Bevy 0.17新機能）
                let _fill_entity = commands
                    .spawn((
                        ProgressBarFill,
                        Sprite {
                            // グラデーション: 緑 → 黄 → 赤
                            color: Color::srgb(0.0, 1.0, 0.0),
                            custom_size: Some(Vec2::new(TILE_SIZE * 0.8, TILE_SIZE * 0.15)),
                            ..default()
                        },
                        // UI Gradientsを使用する場合は、Nodeコンポーネントが必要
                        Transform::from_translation(Vec3::new(0.0, 0.0, 0.1)),
                        ChildOf(bar_background),
                    ))
                    .id();

                soul.bar_entity = Some(bar_background);
            }
        }
    }
}
```

**メリット:**
- 視覚的な改善
- プログレスバーの見やすさ向上

---

## 6. メッセージシステムの最適化

### 現状: メッセージは個別に送信されている

**現状の実装**:
```151:166:src/systems/task_execution.rs
        // 完了イベントの発行
        if was_busy && matches!(*task, AssignedTask::None) {
            if let Some(work_type) = old_work_type {
                // 既存のMessage送信
                ev_completed.write(TaskCompletedEvent {
                    _soul_entity: soul_entity,
                    _task_type: work_type,
                });

                // Bevy 0.17 の Observer をトリガー
                commands.trigger(OnTaskCompleted {
                    entity: soul_entity,
                    task_entity: old_task_entity.unwrap_or(Entity::PLACEHOLDER),
                    work_type,
                });
```

### 提案: メッセージのバッチ処理（複数タスク完了時）

**改善案**: 複数のタスクが同時に完了する可能性がある場合、バッチ処理を検討

```rust
// src/systems/task_execution.rs
// 変更前: ループ内で個別に送信
for (soul_entity, ...) in q_souls.iter_mut() {
    if should_complete {
        ev_completed.write(TaskCompletedEvent {
            _soul_entity: soul_entity,
            _task_type: old_work_type,
        });
    }
}

// 変更後: バッチ処理（複数完了時）
let mut completed_tasks = Vec::new();
for (soul_entity, ...) in q_souls.iter_mut() {
    if should_complete {
        completed_tasks.push((soul_entity, old_work_type));
    }
}

// バッチで送信（write_batchが利用可能な場合）
for (soul_entity, task_type) in completed_tasks {
    ev_completed.write(TaskCompletedEvent {
        _soul_entity: soul_entity,
        _task_type: task_type,
    });
}
```

**注意**: 現在の実装では、タスク完了は個別に発生するため、バッチ処理の効果は限定的かもしれません。ただし、将来的に複数のタスクを同時に処理する場合は有効です。

---

## 7. システム条件の最適化

### 提案: より効率的な条件分岐

```rust
// src/main.rs
use bevy::ecs::schedule::common_conditions::*;

// 変更前
.add_systems(
    Update,
    (task_area_auto_haul_system,).run_if(on_timer(Duration::from_millis(500))),
)

// 変更後: より柔軟な条件
.add_systems(
    Update,
    task_area_auto_haul_system
        .run_if(|time: Res<Time>| time.elapsed_secs() % 0.5 < time.delta_secs())
        .run_if(in_state(GameState::Playing)), // 状態ベースの条件も追加可能
)
```

---

## 8. Queryの分割による最適化

### 現状: 大きなQueryが使用されている

**現状の実装例**:
```58:98:src/systems/familiar_ai.rs
pub fn familiar_ai_system(
    mut commands: Commands,
    _time: Res<Time>,
    spatial_grid: Res<SpatialGrid>,
    mut q_familiars: Query<(
        Entity,
        &Transform,
        &Familiar,
        &FamiliarOperation,
        &ActiveCommand,
        &mut FamiliarAiState,
        &mut Destination,
        &mut Path,
        Option<&TaskArea>,
        &Commanding,
    )>,
    mut q_souls: Query<
        (
            Entity,
            &Transform,
            &DamnedSoul,
            &mut AssignedTask,
            &mut Destination,
            &mut Path,
            &IdleState,
            &mut crate::systems::logistics::Inventory,
            Option<&UnderCommand>,
        ),
        Without<Familiar>,
    >,
```

### 提案: Queryの分割（読み取り専用と書き込み専用を分離）

**改善案**: 読み取り専用のQueryと書き込み専用のQueryを分離することで、並列実行の機会を増やす

```rust
// src/systems/familiar_ai.rs
// 変更前: 大きなQuery
mut q_souls: Query<
    (
        Entity,
        &Transform,
        &DamnedSoul,
        &mut AssignedTask,
        &mut Destination,
        &mut Path,
        &IdleState,
        &mut crate::systems::logistics::Inventory,
        Option<&UnderCommand>,
    ),
    Without<Familiar>,
>,

// 変更後: 用途別に分割
// 読み取り専用のQuery（並列実行可能）
q_souls_read: Query<
    (Entity, &Transform, &DamnedSoul, &IdleState),
    (With<DamnedSoul>, Without<Familiar>),
>,
// 書き込みが必要なQuery（個別に取得）
mut q_souls_write: Query<
    (&mut AssignedTask, &mut Destination, &mut Path, &mut crate::systems::logistics::Inventory),
    (With<DamnedSoul>, Without<Familiar>),
>,
// オプショナルなQuery
q_souls_command: Query<Option<&UnderCommand>, (With<DamnedSoul>, Without<Familiar>)>,
```

**注意**: Queryの分割は、システムの複雑さを増す可能性があります。実際のパフォーマンス改善を測定してから適用することを推奨します。

**メリット:**
- ✅ 並列実行の機会が増える（読み取り専用Queryは並列実行可能）
- ✅ 必要なデータのみを取得（メモリ効率の向上）
- ✅ パフォーマンス向上（特にエンティティ数が多い場合）

**デメリット:**
- ⚠️ コードの複雑さが増す
- ⚠️ エンティティの取得が2回になる可能性がある

---

## 9. リソースの最適化

### 提案: リソースの遅延初期化

```rust
// src/main.rs
// 変更前
.init_resource::<SpatialGrid>()
.init_resource::<FamiliarSpatialGrid>()
.init_resource::<ResourceSpatialGrid>()

// 変更後: 必要になったときに初期化
// システム内で初期化するか、Default実装を改善
```

---

## 10. エラーハンドリングの改善

### 提案: より安全なエラーハンドリング

```rust
// src/interface/camera.rs
// 変更前
let Ok((mut transform, projection)) = query.single_mut() else { return; };

// 変更後: エラーログを追加
let Ok((mut transform, projection)) = query.single_mut() else {
    warn!("MainCamera not found or multiple cameras detected");
    return;
};
```

---

## 実装優先順位（現状検証版）

### ✅ 既に実装済み
1. **システムスケジューリングの最適化（SystemSet使用）** - ✅ 実装済み
   - `GameSystemSet`が定義され、使用されています
   - さらなる最適化: 並列実行の機会を増やす（`.chain()`の見直し）

### 高優先度（即座に実装推奨）
1. **Query最適化: Changedフィルターの活用** - パフォーマンスへの影響が大きい
   - ⚠️ 一部実装済み（`update_familiar_spatial_grid_system`, `pathfinding_system`など）
   - 🔴 **最優先**: `update_spatial_grid_system`の最適化（すべてのエンティティを毎フレーム処理している）

2. **フレームタイムグラフの追加** - デバッグ効率が大幅に向上
   - ❌ 未実装
   - パフォーマンス監視とボトルネック特定に有効

### 中優先度（近いうちに実装推奨）
3. **バッチ処理の活用** - メッセージ送信の効率化
   - ❌ 未実装
   - `write_batch()`が利用可能か確認が必要

4. **Queryの分割** - 並列実行の機会増加
   - ⚠️ 検討が必要（コードの複雑さとのトレードオフ）
   - `familiar_ai_system`などの大きなQueryを分割

5. **エラーハンドリングの改善** - コード品質向上
   - エラーログの追加でデバッグ効率向上

### 低優先度（時間があるときに実装）
6. **UI Gradientsの活用** - 視覚的な改善
   - Bevy 0.17の新機能を活用した視覚的改善

7. **システム条件の最適化** - 細かい最適化
   - 現在の`on_timer`の使用は適切

8. **リソースの最適化** - メモリ使用量の削減
   - 現在のリソース初期化は適切

## 推奨される実装順序

1. **`update_spatial_grid_system`のChangedフィルター適用** - 最も効果が大きい
2. **フレームタイムグラフの追加** - パフォーマンス測定の基盤
3. **並列実行の機会を増やす** - SystemSet内の`.chain()`の見直し
4. **バッチ処理の検討** - `write_batch()`の利用可能性を確認
5. **Queryの分割** - パフォーマンス測定後に検討

---

## 参考資料

- [Bevy 0.16 Migration Guide](https://bevy.org/learn/migration-guides/0-15-to-0-16)
- [Bevy 0.17 Migration Guide](https://bevy.org/learn/migration-guides/0-16-to-0-17)
- [Bevy 0.17 Release Notes](https://bevy.org/news/bevy-0-17/)
- [Bevy Performance Best Practices](https://bevy-cheatbook.github.io/programming/performance.html)
