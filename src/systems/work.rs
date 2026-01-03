use crate::assets::GameAssets;
use crate::constants::*;
use crate::entities::damned_soul::{DamnedSoul, Destination, Path};
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand, UnderCommand};
use crate::systems::command::TaskArea;
use crate::systems::jobs::{
    Designation, DesignationCreatedEvent, IssuedBy, Rock, TaskCompletedEvent, TaskSlots, Tree,
    WorkType,
};
use crate::systems::logistics::{ClaimedBy, InStockpile, Inventory, ResourceItem, Stockpile};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use std::collections::HashMap;

/// 空間グリッド - Soul位置の高速検索用
#[derive(Resource, Default)]
pub struct SpatialGrid {
    cells: HashMap<(i32, i32), Vec<Entity>>,
    cell_size: f32,
    // 差分更新用: 各エンティティがどのセルにいるかを記録
    entity_cells: HashMap<Entity, (i32, i32)>,
}

impl SpatialGrid {
    #[allow(dead_code)]
    pub fn new(cell_size: f32) -> Self {
        Self {
            cells: HashMap::new(),
            cell_size,
            entity_cells: HashMap::new(),
        }
    }

    fn pos_to_cell(&self, pos: Vec2) -> (i32, i32) {
        let cell_size = if self.cell_size > 0.0 {
            self.cell_size
        } else {
            TILE_SIZE * 4.0
        };
        (
            (pos.x / cell_size).floor() as i32,
            (pos.y / cell_size).floor() as i32,
        )
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.cells.clear();
        self.entity_cells.clear();
    }

    pub fn insert(&mut self, entity: Entity, pos: Vec2) {
        let cell = self.pos_to_cell(pos);
        
        // 既に登録されている場合は古いセルから削除
        if let Some(old_cell) = self.entity_cells.get(&entity) {
            if *old_cell != cell {
                if let Some(entities) = self.cells.get_mut(old_cell) {
                    entities.retain(|&e| e != entity);
                }
            } else {
                // 同じセルにいる場合は何もしない
                return;
            }
        }
        
        self.cells.entry(cell).or_default().push(entity);
        self.entity_cells.insert(entity, cell);
    }

    pub fn remove(&mut self, entity: Entity) {
        if let Some(old_cell) = self.entity_cells.remove(&entity) {
            if let Some(entities) = self.cells.get_mut(&old_cell) {
                entities.retain(|&e| e != entity);
            }
        }
    }

    /// 指定位置周辺の9セルにいるエンティティを返す
    pub fn get_nearby(&self, pos: Vec2) -> Vec<Entity> {
        let (cx, cy) = self.pos_to_cell(pos);
        let mut result = Vec::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(entities) = self.cells.get(&(cx + dx, cy + dy)) {
                    result.extend(entities.iter().copied());
                }
            }
        }
        result
    }
}

/// 使い魔用の空間グリッド - モチベーション計算の高速化用
#[derive(Resource, Default)]
pub struct FamiliarSpatialGrid {
    cells: HashMap<(i32, i32), Vec<Entity>>,
    cell_size: f32,
    // 差分更新用
    entity_cells: HashMap<Entity, (i32, i32)>,
}

impl FamiliarSpatialGrid {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cells: HashMap::new(),
            cell_size,
            entity_cells: HashMap::new(),
        }
    }

    fn pos_to_cell(&self, pos: Vec2) -> (i32, i32) {
        (
            (pos.x / self.cell_size).floor() as i32,
            (pos.y / self.cell_size).floor() as i32,
        )
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.cells.clear();
        self.entity_cells.clear();
    }

    pub fn insert(&mut self, entity: Entity, pos: Vec2) {
        let cell = self.pos_to_cell(pos);
        
        // 既に登録されている場合は古いセルから削除
        if let Some(old_cell) = self.entity_cells.get(&entity) {
            if *old_cell != cell {
                if let Some(entities) = self.cells.get_mut(old_cell) {
                    entities.retain(|&e| e != entity);
                }
            } else {
                // 同じセルにいる場合は何もしない
                return;
            }
        }
        
        self.cells.entry(cell).or_default().push(entity);
        self.entity_cells.insert(entity, cell);
    }

    #[allow(dead_code)]
    pub fn remove(&mut self, entity: Entity) {
        if let Some(old_cell) = self.entity_cells.remove(&entity) {
            if let Some(entities) = self.cells.get_mut(&old_cell) {
                entities.retain(|&e| e != entity);
            }
        }
    }

    /// 指定位置周辺のセルにいるエンティティを返す（検索半径を考慮）
    pub fn get_nearby_in_radius(&self, pos: Vec2, radius: f32) -> Vec<Entity> {
        let (cx, cy) = self.pos_to_cell(pos);
        // 半径を考慮して必要なセル数を計算
        let cells_needed = (radius / self.cell_size).ceil() as i32 + 1;
        let mut result = Vec::new();
        for dx in -cells_needed..=cells_needed {
            for dy in -cells_needed..=cells_needed {
                if let Some(entities) = self.cells.get(&(cx + dx, cy + dy)) {
                    result.extend(entities.iter().copied());
                }
            }
        }
        result
    }
}

/// タスクキュー - 保留中の仕事を管理
#[derive(Resource, Default)]
pub struct TaskQueue {
    pub by_familiar: HashMap<Entity, Vec<PendingTask>>,
}

/// 未アサインタスクキュー - 使い魔に割り当てられていないタスク
#[derive(Resource, Default)]
pub struct GlobalTaskQueue {
    pub unassigned: Vec<PendingTask>,
}

#[derive(Clone, Copy, Debug)]
pub struct PendingTask {
    pub entity: Entity,
    pub work_type: WorkType,
    pub priority: u32, // 0: Normal, 1: High, etc.
}

impl TaskQueue {
    pub fn add(&mut self, familiar: Entity, task: PendingTask) {
        let tasks = self.by_familiar.entry(familiar).or_default();
        tasks.push(task);
        // 優先度でソート (降順)
        tasks.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    pub fn get_for_familiar(&self, familiar: Entity) -> Option<&Vec<PendingTask>> {
        self.by_familiar.get(&familiar)
    }

    pub fn remove(&mut self, familiar: Entity, task_entity: Entity) {
        if let Some(tasks) = self.by_familiar.get_mut(&familiar) {
            tasks.retain(|t| t.entity != task_entity);
        }
    }
}

impl GlobalTaskQueue {
    pub fn add(&mut self, task: PendingTask) {
        self.unassigned.push(task);
        self.unassigned.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    pub fn remove(&mut self, task_entity: Entity) {
        self.unassigned.retain(|t| t.entity != task_entity);
    }
}

/// DesignationCreatedEventを受けてキューに追加するシステム
pub fn queue_management_system(
    mut queue: ResMut<TaskQueue>,
    mut global_queue: ResMut<GlobalTaskQueue>,
    mut ev_created: EventReader<DesignationCreatedEvent>,
) {
    for ev in ev_created.read() {
        let task = PendingTask {
            entity: ev.entity,
            work_type: ev.work_type,
            priority: ev.priority,
        };

        if let Some(issued_by) = ev.issued_by {
            queue.add(issued_by, task);
            if ev.priority > 0 {
                info!(
                    "QUEUE: High Priority Task added for Familiar {:?}",
                    issued_by
                );
            }
        } else {
            global_queue.add(task);
            info!("QUEUE: Unassigned Task added (entity: {:?})", ev.entity);
        }
    }
}

/// 人間に割り当てられたタスク
#[derive(Component, Clone, Debug)]
pub enum AssignedTask {
    None,
    /// リソースを収集する（簡略版）
    Gather {
        target: Entity,
        work_type: WorkType,
        phase: GatherPhase,
    },
    /// リソースを運搬する
    Haul {
        item: Entity,
        stockpile: Entity,
        phase: HaulPhase,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HaulPhase {
    GoingToItem,
    GoingToStockpile,
    Dropping,
}

impl Default for AssignedTask {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GatherPhase {
    GoingToResource,
    Collecting { progress: f32 },
    Done,
}

impl AssignedTask {
    pub fn work_type(&self) -> Option<WorkType> {
        match self {
            AssignedTask::Gather { work_type, .. } => Some(*work_type),
            AssignedTask::Haul { .. } => Some(WorkType::Haul),
            AssignedTask::None => None,
        }
    }
}

/// SpatialGridを更新するシステム（差分更新）
pub fn update_spatial_grid_system(
    mut spatial_grid: ResMut<SpatialGrid>,
    q_souls_transform: Query<(Entity, &Transform, &DamnedSoul, &AssignedTask), Changed<Transform>>,
    q_souls_soul: Query<(Entity, &Transform, &DamnedSoul, &AssignedTask), Changed<DamnedSoul>>,
    q_souls_task: Query<(Entity, &Transform, &DamnedSoul, &AssignedTask), Changed<AssignedTask>>,
) {
    // 位置が変更されたsoulを更新
    for (entity, transform, soul, task) in q_souls_transform.iter() {
        let should_be_in_grid = matches!(task, AssignedTask::None)
            && soul.motivation >= MOTIVATION_THRESHOLD
            && soul.fatigue < FATIGUE_THRESHOLD;
        
        if should_be_in_grid {
            spatial_grid.insert(entity, transform.translation.truncate());
        } else {
            spatial_grid.remove(entity);
        }
    }
    
    // やる気・疲労が変更されたsoulを更新
    for (entity, transform, soul, task) in q_souls_soul.iter() {
        let should_be_in_grid = matches!(task, AssignedTask::None)
            && soul.motivation >= MOTIVATION_THRESHOLD
            && soul.fatigue < FATIGUE_THRESHOLD;
        
        if should_be_in_grid {
            spatial_grid.insert(entity, transform.translation.truncate());
        } else {
            spatial_grid.remove(entity);
        }
    }
    
    // タスクが変更されたsoulを更新
    for (entity, transform, soul, task) in q_souls_task.iter() {
        let should_be_in_grid = matches!(task, AssignedTask::None)
            && soul.motivation >= MOTIVATION_THRESHOLD
            && soul.fatigue < FATIGUE_THRESHOLD;
        
        if should_be_in_grid {
            spatial_grid.insert(entity, transform.translation.truncate());
        } else {
            spatial_grid.remove(entity);
        }
    }
}

/// FamiliarSpatialGridを更新するシステム（差分更新）
pub fn update_familiar_spatial_grid_system(
    mut familiar_grid: ResMut<FamiliarSpatialGrid>,
    q_familiars: Query<(Entity, &Transform, &Familiar), Changed<Transform>>,
) {
    // 変更された使い魔のみ更新
    for (entity, transform, _) in q_familiars.iter() {
        familiar_grid.insert(entity, transform.translation.truncate());
    }
}

pub fn task_delegation_system(
    mut commands: Commands,
    mut q_familiars: Query<(Entity, &Transform, &mut ActiveCommand)>,
    mut q_souls: Query<(
        Entity,
        &Transform,
        &DamnedSoul,
        &mut AssignedTask,
        &mut Destination,
        &mut Path,
        &mut Inventory,
    )>,
    q_stockpiles: Query<(Entity, &Transform), With<Stockpile>>,
    mut q_designations: Query<(&Transform, &Designation, &mut TaskSlots)>,
    mut queue: ResMut<TaskQueue>,
    spatial_grid: Res<SpatialGrid>,
    mut ev_created: EventReader<DesignationCreatedEvent>,
    mut ev_completed: EventReader<TaskCompletedEvent>,
) {
    // イベントがあるか、キューが空でない場合のみ実行
    if ev_created.is_empty()
        && ev_completed.is_empty()
        && queue.by_familiar.values().all(|v| v.is_empty())
    {
        return;
    }

    // イベントを読み飛ばしてフラグにする（実際にはqueue_management_systemが既に処理している想定）
    ev_created.clear();
    ev_completed.clear();
    for (fam_entity, fam_transform, mut active_command) in q_familiars.iter_mut() {
        let fam_pos = fam_transform.translation.truncate();

        // 使役枠の空きを確認 (最大2名)
        let current_count = active_command.assigned_souls.len();
        if current_count >= 2 {
            continue;
        }
        let slots_available = 2 - current_count;

        // キューからこの使い魔のタスクを取得
        let Some(tasks) = queue.get_for_familiar(fam_entity) else {
            continue;
        };
        if tasks.is_empty() {
            continue;
        }

        // 優先度と距離でソート（既に優先度でソート済みなので、距離のみ計算）
        // 優先度が同じタスクのみ距離でソート
        let mut sorted_tasks: Vec<_> = tasks.iter().copied().collect();
        
        // 優先度グループごとに距離でソート
        // まず優先度でグループ化し、各グループ内で距離ソート
        sorted_tasks.sort_by(|t1, t2| {
            // 1. 優先度 (降順)
            match t2.priority.cmp(&t1.priority) {
                std::cmp::Ordering::Equal => {
                    // 優先度が同じ場合のみ距離を計算
                    let p1 = q_designations
                        .get(t1.entity)
                        .map(|(t, _, _)| t.translation.truncate())
                        .unwrap_or(Vec2::ZERO);
                    let p2 = q_designations
                        .get(t2.entity)
                        .map(|(t, _, _)| t.translation.truncate())
                        .unwrap_or(Vec2::ZERO);
                    let d1 = p1.distance_squared(fam_pos);
                    let d2 = p2.distance_squared(fam_pos);
                    d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
                }
                other => other,
            }
        });

        let mut assigned_this_tick = 0;
        let mut to_remove = Vec::new();

        for task_info in sorted_tasks.iter() {
            let des_entity = task_info.entity;
            let work_type = task_info.work_type;

            let Ok((des_transform, _, mut slots)) = q_designations.get_mut(des_entity) else {
                to_remove.push(des_entity); // 存在しないタスクは削除対象
                continue;
            };

            // スロットの空きがない場合はスキップ
            if !slots.has_slot() {
                to_remove.push(des_entity); // キューからも消す（既に埋まっている）
                continue;
            }

            let des_pos = des_transform.translation.truncate();

            if assigned_this_tick >= slots_available {
                break;
            }

            // SpatialGridで近くのSoulを高速検索（フォールバック付き）
            let nearby_souls = spatial_grid.get_nearby(des_pos);

            // 最も近いSoulを見つける（近傍検索）
            let best_soul = if !nearby_souls.is_empty() {
                nearby_souls
                    .iter()
                    .filter_map(|&e| q_souls.get(e).ok())
                    .filter(|(_, _, soul, current_task, _, _, _)| {
                        matches!(*current_task, AssignedTask::None)
                            && soul.motivation >= MOTIVATION_THRESHOLD
                            && soul.fatigue < FATIGUE_THRESHOLD
                    })
                    .min_by(|(_, t1, _, _, _, _, _), (_, t2, _, _, _, _, _)| {
                        let d1 = t1.translation.truncate().distance_squared(des_pos);
                        let d2 = t2.translation.truncate().distance_squared(des_pos);
                        d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(e, _, _, _, _, _, _)| e)
            } else {
                // フォールバック: 全Soulから検索
                q_souls
                    .iter()
                    .filter(|(_, _, soul, current_task, _, _, _)| {
                        matches!(*current_task, AssignedTask::None)
                            && soul.motivation >= MOTIVATION_THRESHOLD
                            && soul.fatigue < FATIGUE_THRESHOLD
                    })
                    .min_by(|(_, t1, _, _, _, _, _), (_, t2, _, _, _, _, _)| {
                        let d1 = t1.translation.truncate().distance_squared(des_pos);
                        let d2 = t2.translation.truncate().distance_squared(des_pos);
                        d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(e, _, _, _, _, _, _)| e)
            };

            if let Some(soul_entity) = best_soul {
                match work_type {
                    WorkType::Chop | WorkType::Mine => {
                        if let Ok((mut soul_task, mut dest, mut path)) = q_souls
                            .get_mut(soul_entity)
                            .map(|(_, _, _, t, d, p, _)| (t, d, p))
                        {
                            *soul_task = AssignedTask::Gather {
                                target: des_entity,
                                work_type: work_type,
                                phase: GatherPhase::GoingToResource,
                            };
                            dest.0 = des_pos;
                            path.waypoints.clear();

                            // リンクの作成
                            commands.entity(des_entity).insert(ClaimedBy(soul_entity));
                            slots.current += 1; // スロットを埋める
                            commands
                                .entity(soul_entity)
                                .insert(UnderCommand(fam_entity));
                            active_command.assigned_souls.push(soul_entity);

                            assigned_this_tick += 1;
                            to_remove.push(des_entity); // キューから削除
                            info!(
                                "DELEGATION: Soul {:?} assigned to GATHER target {:?} by Familiar {:?}",
                                soul_entity, des_entity, fam_entity
                            );
                        }
                    }
                    WorkType::Haul => {
                        let best_stockpile = q_stockpiles
                            .iter()
                            .min_by(|(_, t1), (_, t2)| {
                                let d1 = t1.translation.truncate().distance_squared(des_pos);
                                let d2 = t2.translation.truncate().distance_squared(des_pos);
                                d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
                            })
                            .map(|(e, _)| e);

                        if let Some(stock_entity) = best_stockpile {
                            if let Ok((mut soul_task, mut dest, mut path)) = q_souls
                                .get_mut(soul_entity)
                                .map(|(_, _, _, t, d, p, _)| (t, d, p))
                            {
                                *soul_task = AssignedTask::Haul {
                                    item: des_entity,
                                    stockpile: stock_entity,
                                    phase: HaulPhase::GoingToItem,
                                };
                                dest.0 = des_pos;
                                path.waypoints.clear();

                                // リンクの作成
                                commands.entity(des_entity).insert(ClaimedBy(soul_entity));
                                slots.current += 1; // スロットを埋める
                                commands
                                    .entity(soul_entity)
                                    .insert(UnderCommand(fam_entity));
                                active_command.assigned_souls.push(soul_entity);

                                assigned_this_tick += 1;
                                to_remove.push(des_entity);
                                info!(
                                    "DELEGATION: Soul {:?} assigned HAUL item {:?} by Familiar {:?}",
                                    soul_entity, des_entity, fam_entity
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // 割り当てられたタスクをキューから削除
        for entity in to_remove {
            queue.remove(fam_entity, entity);
        }
    }
}

/// タスクが完了した魂を使役から解放する
pub fn cleanup_commanded_souls_system(
    mut commands: Commands,
    mut q_familiars: Query<&mut ActiveCommand>,
    q_souls: Query<(Entity, &AssignedTask, &UnderCommand)>,
) {
    for (soul_entity, task, under_command) in q_souls.iter() {
        if matches!(task, AssignedTask::None) {
            let fam_entity = under_command.0;
            if let Ok(mut active_command) = q_familiars.get_mut(fam_entity) {
                // 使い魔のリストから削除
                active_command.assigned_souls.retain(|&e| e != soul_entity);
            }
            // コンポーネントを削除して解放
            commands.entity(soul_entity).remove::<UnderCommand>();
            info!(
                "RELEASE: Soul {:?} released from Familiar {:?}",
                soul_entity, fam_entity
            );
        }
    }
}

pub fn task_execution_system(
    mut commands: Commands,
    mut q_souls: Query<(
        Entity,
        &Transform,
        &mut DamnedSoul,
        &mut AssignedTask,
        &mut Destination,
        &mut Path,
        &mut Inventory,
    )>,
    q_targets: Query<(
        &Transform,
        Option<&Tree>,
        Option<&Rock>,
        Option<&ResourceItem>,
        Option<&Designation>,
    )>,
    q_stockpiles: Query<&Transform, With<Stockpile>>,
    game_assets: Res<GameAssets>,
    mut ev_completed: EventWriter<TaskCompletedEvent>,
    time: Res<Time>,
) {
    for (soul_entity, soul_transform, mut soul, mut task, mut dest, mut path, mut inventory) in
        q_souls.iter_mut()
    {
        let was_busy = !matches!(*task, AssignedTask::None);
        let old_work_type = task.work_type();

        match *task {
            AssignedTask::Gather {
                target,
                work_type,
                phase,
            } => {
                handle_gather_task(
                    soul_entity,
                    soul_transform,
                    &mut soul,
                    &mut task,
                    &mut dest,
                    &mut path,
                    target,
                    &work_type,
                    phase,
                    &q_targets,
                    &mut commands,
                    &game_assets,
                    &time,
                );
            }
            AssignedTask::Haul {
                item,
                stockpile,
                phase,
            } => {
                handle_haul_task(
                    soul_entity,
                    soul_transform,
                    &mut soul,
                    &mut task,
                    &mut dest,
                    &mut path,
                    &mut inventory,
                    item,
                    stockpile,
                    phase,
                    &q_targets,
                    &q_stockpiles,
                    &mut commands,
                );
            }
            AssignedTask::None => {}
        }

        // 完了イベントの発行
        if was_busy && matches!(*task, AssignedTask::None) {
            if let Some(work_type) = old_work_type {
                ev_completed.send(TaskCompletedEvent {
                    _soul_entity: soul_entity,
                    _task_type: work_type,
                });
                info!("EVENT: TaskCompletedEvent sent for Soul {:?}", soul_entity);
            }
        }
    }
}

fn handle_gather_task(
    soul_entity: Entity,
    soul_transform: &Transform,
    soul: &mut DamnedSoul,
    task: &mut AssignedTask,
    dest: &mut Destination,
    path: &mut Path,
    target: Entity,
    work_type: &WorkType,
    phase: GatherPhase,
    q_targets: &Query<(
        &Transform,
        Option<&Tree>,
        Option<&Rock>,
        Option<&ResourceItem>,
        Option<&Designation>,
    )>,
    commands: &mut Commands,
    game_assets: &Res<GameAssets>,
    time: &Res<Time>,
) {
    let soul_pos = soul_transform.translation.truncate();
    match phase {
        GatherPhase::GoingToResource => {
            if let Ok((res_transform, _, _, _, des_opt)) = q_targets.get(target) {
                // 指定が解除されていたら中止
                if des_opt.is_none() {
                    *task = AssignedTask::None;
                    path.waypoints.clear();
                    return;
                }
                let res_pos = res_transform.translation.truncate();
                if dest.0.distance_squared(res_pos) > 2.0 {
                    dest.0 = res_pos;
                    path.waypoints.clear();
                }

                let dist = soul_pos.distance(res_pos);
                if dist < TILE_SIZE * 1.2 {
                    *task = AssignedTask::Gather {
                        target,
                        work_type: *work_type,
                        phase: GatherPhase::Collecting { progress: 0.0 },
                    };
                    path.waypoints.clear();
                }
            } else {
                *task = AssignedTask::None;
                path.waypoints.clear();
            }
        }
        GatherPhase::Collecting { mut progress } => {
            if let Ok((res_transform, tree, rock, _, des_opt)) = q_targets.get(target) {
                // 指定が解除されていたら中止
                if des_opt.is_none() {
                    *task = AssignedTask::None;
                    path.waypoints.clear();
                    return;
                }
                let pos = res_transform.translation;

                // 進行度を更新 (仮に 2秒で完了とする)
                progress += time.delta_secs() * 0.5;

                if progress >= 1.0 {
                    if tree.is_some() {
                        commands.spawn((
                            ResourceItem(crate::systems::logistics::ResourceType::Wood),
                            Sprite {
                                image: game_assets.wood.clone(),
                                custom_size: Some(Vec2::splat(TILE_SIZE * 0.5)),
                                color: Color::srgb(0.5, 0.35, 0.05),
                                ..default()
                            },
                            Transform::from_translation(pos),
                        ));
                        info!("TASK_EXEC: Soul {:?} chopped a tree", soul_entity);
                    } else if rock.is_some() {
                        commands.spawn((
                            ResourceItem(crate::systems::logistics::ResourceType::Stone), // 修正: Stone
                            Sprite {
                                image: game_assets.stone.clone(),
                                custom_size: Some(Vec2::splat(TILE_SIZE * 0.5)),
                                ..default()
                            },
                            Transform::from_translation(pos),
                        ));
                        info!("TASK_EXEC: Soul {:?} mined a rock", soul_entity);
                    }
                    commands.entity(target).despawn();

                    *task = AssignedTask::Gather {
                        target,
                        work_type: *work_type,
                        phase: GatherPhase::Done,
                    };
                    soul.fatigue = (soul.fatigue + 0.1).min(1.0);
                } else {
                    // 進捗を保存
                    *task = AssignedTask::Gather {
                        target,
                        work_type: *work_type,
                        phase: GatherPhase::Collecting { progress },
                    };
                }
            } else {
                *task = AssignedTask::None;
            }
        }
        GatherPhase::Done => {
            *task = AssignedTask::None;
            path.waypoints.clear();
        }
    }
}

fn handle_haul_task(
    soul_entity: Entity,
    soul_transform: &Transform,
    soul: &mut DamnedSoul,
    task: &mut AssignedTask,
    dest: &mut Destination,
    path: &mut Path,
    inventory: &mut Inventory,
    item: Entity,
    stockpile: Entity,
    phase: HaulPhase,
    q_targets: &Query<(
        &Transform,
        Option<&Tree>,
        Option<&Rock>,
        Option<&ResourceItem>,
        Option<&Designation>,
    )>,
    q_stockpiles: &Query<&Transform, With<Stockpile>>,
    commands: &mut Commands,
) {
    let soul_pos = soul_transform.translation.truncate();
    match phase {
        HaulPhase::GoingToItem => {
            if let Ok((res_transform, _, _, _, des_opt)) = q_targets.get(item) {
                // 指定が解除されていたら中止（運搬中のものは既に Designation が無いので、GoingToItem フェーズのみチェック）
                if des_opt.is_none() {
                    *task = AssignedTask::None;
                    path.waypoints.clear();
                    return;
                }
                let res_pos = res_transform.translation.truncate();
                if dest.0.distance_squared(res_pos) > 2.0 {
                    dest.0 = res_pos;
                    path.waypoints.clear();
                }

                if soul_pos.distance(res_pos) < TILE_SIZE * 1.2 {
                    inventory.0 = Some(item);
                    commands.entity(item).insert(Visibility::Hidden);
                    // タスク指定を削除（地面の色を消し、他の使い魔が割り当てるのを防ぐ）
                    commands.entity(item).remove::<Designation>();
                    commands.entity(item).remove::<IssuedBy>();
                    commands.entity(item).remove::<TaskSlots>();

                    *task = AssignedTask::Haul {
                        item,
                        stockpile,
                        phase: HaulPhase::GoingToStockpile,
                    };
                    path.waypoints.clear();
                    info!("HAUL: Soul {:?} picked up item", soul_entity);
                }
            } else {
                *task = AssignedTask::None;
                path.waypoints.clear();
            }
        }
        HaulPhase::GoingToStockpile => {
            if let Ok(stock_transform) = q_stockpiles.get(stockpile) {
                let stock_pos = stock_transform.translation.truncate();
                if dest.0.distance_squared(stock_pos) > 2.0 {
                    dest.0 = stock_pos;
                    path.waypoints.clear();
                }

                if soul_pos.distance(stock_pos) < TILE_SIZE * 1.2 {
                    *task = AssignedTask::Haul {
                        item,
                        stockpile,
                        phase: HaulPhase::Dropping,
                    };
                    path.waypoints.clear();
                }
            } else {
                warn!(
                    "HAUL: Soul {:?} stockpile {:?} not found",
                    soul_entity, stockpile
                );
                if let Some(item_entity) = inventory.0.take() {
                    commands.entity(item_entity).insert(Visibility::Visible);
                    commands.entity(item_entity).remove::<ClaimedBy>();
                }
                *task = AssignedTask::None;
                path.waypoints.clear();
            }
        }
        HaulPhase::Dropping => {
            if let Ok(stock_transform) = q_stockpiles.get(stockpile) {
                let stock_pos = stock_transform.translation.truncate();
                if let Some(item_entity) = inventory.0.take() {
                    commands.entity(item_entity).insert((
                        Visibility::Visible,
                        Transform::from_xyz(stock_pos.x, stock_pos.y, 0.6),
                        InStockpile,
                    ));
                    commands.entity(item_entity).remove::<ClaimedBy>();
                    info!(
                        "TASK_EXEC: Soul {:?} dropped item at stockpile",
                        soul_entity
                    );
                }
            }
            *task = AssignedTask::None;
            path.waypoints.clear();
            soul.fatigue = (soul.fatigue + 0.05).min(1.0);
        }
    }
}

/// リソースアイテム用の空間グリッド
#[derive(Resource, Default)]
pub struct ResourceSpatialGrid {
    cells: HashMap<(i32, i32), Vec<Entity>>,
    cell_size: f32,
    entity_cells: HashMap<Entity, (i32, i32)>,
}

impl ResourceSpatialGrid {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cells: HashMap::new(),
            cell_size,
            entity_cells: HashMap::new(),
        }
    }

    fn pos_to_cell(&self, pos: Vec2) -> (i32, i32) {
        (
            (pos.x / self.cell_size).floor() as i32,
            (pos.y / self.cell_size).floor() as i32,
        )
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.cells.clear();
        self.entity_cells.clear();
    }

    pub fn insert(&mut self, entity: Entity, pos: Vec2) {
        let cell = self.pos_to_cell(pos);
        
        if let Some(old_cell) = self.entity_cells.get(&entity) {
            if *old_cell != cell {
                if let Some(entities) = self.cells.get_mut(old_cell) {
                    entities.retain(|&e| e != entity);
                }
            } else {
                return;
            }
        }
        
        self.cells.entry(cell).or_default().push(entity);
        self.entity_cells.insert(entity, cell);
    }

    pub fn remove(&mut self, entity: Entity) {
        if let Some(old_cell) = self.entity_cells.remove(&entity) {
            if let Some(entities) = self.cells.get_mut(&old_cell) {
                entities.retain(|&e| e != entity);
            }
        }
    }

    pub fn get_nearby_in_radius(&self, pos: Vec2, radius: f32) -> Vec<Entity> {
        let (cx, cy) = self.pos_to_cell(pos);
        let cells_needed = (radius / self.cell_size).ceil() as i32 + 1;
        let mut result = Vec::new();
        for dx in -cells_needed..=cells_needed {
            for dy in -cells_needed..=cells_needed {
                if let Some(entities) = self.cells.get(&(cx + dx, cy + dy)) {
                    result.extend(entities.iter().copied());
                }
            }
        }
        result
    }
}

/// リソースグリッドを更新するシステム（差分更新）
pub fn update_resource_spatial_grid_system(
    mut resource_grid: ResMut<ResourceSpatialGrid>,
    q_resources_added: Query<(Entity, &Transform, Option<&Visibility>), (With<ResourceItem>, Added<ResourceItem>)>,
    q_resources: Query<(Entity, &Transform, Option<&Visibility>), (With<ResourceItem>, Changed<Transform>)>,
    q_visibility_changed: Query<(Entity, &Transform, Option<&Visibility>), (With<ResourceItem>, Changed<Visibility>)>,
) {
    // 新しく追加されたリソースを登録
    // Visibility::Hiddenのリソース（拾われている）は除外、それ以外は登録
    for (entity, transform, visibility) in q_resources_added.iter() {
        let should_register = visibility.map(|v| *v != Visibility::Hidden).unwrap_or(true);
        if should_register {
            resource_grid.insert(entity, transform.translation.truncate());
            info!("RESOURCE_GRID: Added resource {:?} at {:?}", entity, transform.translation.truncate());
        }
    }
    
    // 位置が変更されたリソースを更新
    for (entity, transform, visibility) in q_resources.iter() {
        let should_register = visibility.map(|v| *v != Visibility::Hidden).unwrap_or(true);
        if should_register {
            resource_grid.insert(entity, transform.translation.truncate());
        } else {
            resource_grid.remove(entity);
        }
    }
    
    // 可視性が変更されたリソースを更新
    for (entity, transform, visibility) in q_visibility_changed.iter() {
        let should_register = visibility.map(|v| *v != Visibility::Hidden).unwrap_or(true);
        if should_register {
            resource_grid.insert(entity, transform.translation.truncate());
        } else {
            resource_grid.remove(entity);
        }
    }
}

/// 実行頻度を制御するためのカウンター
#[derive(Resource, Default)]
pub struct AutoHaulCounter {
    frame_count: u32,
}

pub fn task_area_auto_haul_system(
    mut commands: Commands,
    mut counter: ResMut<AutoHaulCounter>,
    resource_grid: Res<ResourceSpatialGrid>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_stockpiles: Query<(&Transform, &Stockpile)>,
    q_items_in_stock: Query<&Transform, With<InStockpile>>,
    q_resources: Query<
        (Entity, &Transform, &Visibility),
        (
            With<ResourceItem>,
            Without<InStockpile>,
            Without<Designation>,
            Without<ClaimedBy>,
        ),
    >,
    mut ev_created: EventWriter<DesignationCreatedEvent>,
) {
    // 10フレームに1回実行（約0.17秒@60FPS）
    counter.frame_count += 1;
    if counter.frame_count < 10 {
        return;
    }
    counter.frame_count = 0;

    let familiar_count = q_familiars.iter().count();
    let stockpile_count = q_stockpiles.iter().count();
    
    // リソースの総数を確認
    let all_resources: Vec<_> = q_resources.iter().collect();
    info!("AUTO_HAUL: System running - {} familiars with TaskArea, {} stockpiles, {} total resources in query", 
        familiar_count, stockpile_count, all_resources.len());

    for (fam_entity, active_command, task_area) in q_familiars.iter() {
        // タスクエリアが設定されている場合は、Idleでも自動運搬を実行
        // タスクエリアが設定されていない場合は、Idleの場合はスキップ
        if matches!(active_command.command, FamiliarCommand::Idle) {
            // TaskAreaが設定されている場合は続行（自動運搬を実行）
            info!("AUTO_HAUL: Familiar {:?} is Idle but has TaskArea, continuing", fam_entity);
        } else {
            info!("AUTO_HAUL: Checking familiar {:?} with command {:?}", fam_entity, active_command.command);
        }

        for (stock_transform, stockpile) in q_stockpiles.iter() {
            let stock_pos = stock_transform.translation.truncate();
            if !task_area.contains(stock_pos) {
                info!("AUTO_HAUL: Stockpile at {:?} is outside task area (min: {:?}, max: {:?})", stock_pos, task_area.min, task_area.max);
                continue;
            }

            info!("AUTO_HAUL: Found stockpile at {:?} in task area", stock_pos);

            let current_count = q_items_in_stock
                .iter()
                .filter(|t| {
                    WorldMap::world_to_grid(t.translation.truncate())
                        == WorldMap::world_to_grid(stock_pos)
                })
                .count();

            if current_count >= stockpile.capacity {
                info!("AUTO_HAUL: Stockpile at {:?} is full ({}/{})", stock_pos, current_count, stockpile.capacity);
                continue;
            }

            // 空間グリッドを使用して近傍のリソースのみを検索
            let search_radius = TILE_SIZE * 15.0;
            let nearby_resources = resource_grid.get_nearby_in_radius(stock_pos, search_radius);
            
            // タスクエリア内のすべてのリソースを確認（デバッグ用）
            let resources_in_area: Vec<_> = all_resources.iter()
                .filter(|(_, transform, _)| task_area.contains(transform.translation.truncate()))
                .collect();
            
            info!("AUTO_HAUL: Found {} nearby resources in grid (search radius: {}), {} resources in task area, {} total resources in query", 
                nearby_resources.len(), search_radius, resources_in_area.len(), all_resources.len());

            let nearest_resource = nearby_resources
                .iter()
                .filter_map(|&entity| {
                    let Ok((_, transform, visibility)) = q_resources.get(entity) else {
                        // リソースがクエリに含まれない理由（Designation/ClaimedBy/InStockpileが付いている）
                        return None;
                    };
                    // 非表示（拾われている）アイテムは除外
                    if *visibility == Visibility::Hidden {
                        return None;
                    }
                    let dist_sq = transform.translation.truncate().distance_squared(stock_pos);
                    if dist_sq < search_radius * search_radius {
                        Some((entity, dist_sq))
                    } else {
                        None
                    }
                })
                .min_by(|(_, d1): &(Entity, f32), (_, d2): &(Entity, f32)| {
                    d1.partial_cmp(d2).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(entity, _)| entity);

            if let Some(item_entity) = nearest_resource {
                info!("AUTO_HAUL: Found nearest resource {:?} for stockpile at {:?}", item_entity, stock_pos);
                commands.entity(item_entity).insert((
                    Designation {
                        work_type: WorkType::Haul,
                    },
                    IssuedBy(fam_entity),
                    TaskSlots::new(1),
                ));
                ev_created.send(DesignationCreatedEvent {
                    entity: item_entity,
                    work_type: WorkType::Haul,
                    issued_by: Some(fam_entity),
                    priority: 0,
                });
                info!(
                    "AUTO_HAUL: Designated item {:?} for stockpile (IssuedBy: {:?})",
                    item_entity, fam_entity
                );
            } else {
                let available_count = nearby_resources.iter().filter(|&&e| q_resources.get(e).is_ok()).count();
                info!("AUTO_HAUL: No suitable resource found for stockpile at {:?} (nearby in grid: {}, available in query: {})", 
                    stock_pos, nearby_resources.len(), available_count);
            }
        }
    }
}
