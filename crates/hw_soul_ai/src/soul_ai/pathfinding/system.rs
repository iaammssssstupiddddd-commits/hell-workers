use bevy::prelude::*;
use hw_core::constants::MAX_PATHFINDS_PER_FRAME;
use hw_core::relationships::RestAreaReservedFor;
use hw_core::soul::{DamnedSoul, Destination, IdleBehavior, IdleState, Path};
use hw_world::{PathfindingContext, WorldMap, WorldMapRead, find_path_world_waypoints};

use crate::soul_ai::execute::task_execution::AssignedTask;

use super::{PathCooldown, fallback, reuse};

/// フェーズ予算上限を返す。
/// task フェーズは idle 探索用スロットを確保するため上限を絞る。
fn phase_budget_limit(prioritize_tasks: bool) -> usize {
    const RESERVED_IDLE_PATHFINDS_PER_FRAME: usize = 2;
    if prioritize_tasks {
        MAX_PATHFINDS_PER_FRAME.saturating_sub(RESERVED_IDLE_PATHFINDS_PER_FRAME)
    } else {
        MAX_PATHFINDS_PER_FRAME
    }
}

/// 障害物に埋まったソウルを最寄りの歩行可能タイルへ逃がす。
/// 建築物の配置や障害物の追加で現在位置が通行不可になった場合に実行される。
pub fn soul_stuck_escape_system(
    world_map: WorldMapRead,
    mut query: Query<(&mut Transform, &mut Path), With<DamnedSoul>>,
) {
    for (mut transform, mut path) in query.iter_mut() {
        let current_pos = transform.translation.truncate();
        if world_map.is_walkable_world(current_pos) {
            continue;
        }
        if let Some((gx, gy)) = world_map.get_nearest_walkable_grid(current_pos) {
            let escape_pos = WorldMap::grid_to_world(gx, gy);
            transform.translation.x = escape_pos.x;
            transform.translation.y = escape_pos.y;
            path.waypoints.clear();
            path.current_index = 0;
            path.planned_destination = None;
            path.validated_obstacle_version = 0;
            debug!(
                "SOUL_STUCK_ESCAPE: moved soul from {:?} to walkable {:?}",
                current_pos, escape_pos
            );
        }
    }
}

/// `process_worker_pathfinding` に渡す Soul の移動状態・アイドル情報。
struct SoulPfState<'a> {
    destination: &'a mut Destination,
    path: &'a mut Path,
    task: &'a mut AssignedTask,
    idle: &'a mut IdleState,
    rest_reserved_for: Option<&'a RestAreaReservedFor>,
    inventory_opt: Option<&'a mut hw_logistics::Inventory>,
}

/// `process_worker_pathfinding` に渡すワールド・パス探索コンテキスト。
struct WorldPfCtx<'a> {
    world_map: &'a WorldMap,
    pf_context: &'a mut PathfindingContext,
    pathfind_count: &'a mut usize,
    budget: usize,
}

fn record_path_plan(path: &mut Path, destination: Vec2, obstacle_version: u64) {
    path.planned_destination = Some(destination);
    path.validated_obstacle_version = obstacle_version;
}

/// クールダウン中でなく、有効なパス追従中なら当フレームの探索処理を省略できる。
fn can_skip_pathfinding_tick(
    has_task: bool,
    idle_can_move: bool,
    cooldown: Option<&PathCooldown>,
    path: &Path,
    destination: Vec2,
    obstacle_version: u64,
) -> bool {
    if !has_task && !idle_can_move {
        return true;
    }
    if cooldown.is_some() {
        return false;
    }
    if path.waypoints.is_empty() || path.current_index >= path.waypoints.len() {
        return false;
    }
    if path.planned_destination != Some(destination) {
        return false;
    }
    path.validated_obstacle_version == obstacle_version
}

/// 1 worker のパス探索処理（cooldown 処理後に呼ぶ）。
/// pathfind_count は再利用時の部分再計算・新規計算それぞれで内部更新される。
fn process_worker_pathfinding(
    commands: &mut Commands,
    entity: Entity,
    transform: &Transform,
    soul: SoulPfState<'_>,
    world_pf: WorldPfCtx<'_>,
    q_rest_areas: &Query<&Transform, With<hw_jobs::RestArea>>,
    queries: &mut crate::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
) {
    let has_task = !matches!(*soul.task, AssignedTask::None);
    let current_pos = transform.translation.truncate();
    let start_grid = WorldMap::world_to_grid(current_pos);
    let goal_grid = WorldMap::world_to_grid(soul.destination.0);
    let obstacle_version = world_pf.world_map.obstacle_version;

    // --- 再利用フェーズ: 既存パスが有効なら A* コストなしで続行 ---
    if reuse::try_reuse_existing_path(
        commands,
        reuse::PathBudgetInfo {
            entity,
            pathfind_count: world_pf.pathfind_count,
            phase_budget_limit: world_pf.budget,
        },
        soul.path,
        soul.destination.0,
        goal_grid,
        world_pf.world_map,
        world_pf.pf_context,
    ) {
        return;
    }

    // 同グリッド: A* 不要、1ステップで到達可能
    if start_grid == goal_grid {
        soul.path.waypoints = vec![soul.destination.0];
        soul.path.current_index = 0;
        record_path_plan(soul.path, soul.destination.0, obstacle_version);
        commands.entity(entity).remove::<PathCooldown>();
        return;
    }

    // --- 探索フェーズ: 予算スロットを 1 消費して A* を実行 ---
    if *world_pf.pathfind_count >= world_pf.budget {
        return;
    }
    *world_pf.pathfind_count += 1;

    if let Some(world_path) = find_path_world_waypoints(
        world_pf.world_map,
        world_pf.pf_context,
        start_grid,
        goal_grid,
    ) {
        soul.path.waypoints = world_path;
        soul.path.current_index = 0;
        record_path_plan(soul.path, soul.destination.0, obstacle_version);
        commands.entity(entity).remove::<PathCooldown>();
        debug!("PATH: Soul {:?} found new path", entity);
    } else {
        debug!("PATH: Soul {:?} failed to find path", entity);

        // --- fallback フェーズ: 休憩所への代替タイルを探す（idle の GoingToRest のみ）---
        if !has_task
            && soul.idle.behavior == IdleBehavior::GoingToRest
            && fallback::try_rest_area_fallback_path(
                commands,
                soul.destination,
                soul.path,
                soul.rest_reserved_for,
                q_rest_areas,
                fallback::SoulGridPos {
                    entity,
                    current_pos,
                    start_grid,
                    goal_grid,
                },
                fallback::FallbackPfState {
                    world_map: world_pf.world_map,
                    pf_context: world_pf.pf_context,
                },
            )
        {
            record_path_plan(soul.path, soul.destination.0, obstacle_version);
            return;
        }

        // --- cleanup フェーズ: 到達不能の destination を破棄し、冷却期間を付与 ---
        fallback::cleanup_unreachable_destination(
            commands,
            fallback::SoulEntityCtx {
                entity,
                transform,
                current_pos,
                has_task,
            },
            fallback::SoulMoveState {
                idle: soul.idle,
                destination: soul.destination,
                task: soul.task,
                path: soul.path,
            },
            soul.inventory_opt,
            queries,
            world_pf.world_map,
        );
    }
}

type PathfindingQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static mut Destination,
        &'static mut Path,
        &'static mut AssignedTask,
        &'static mut IdleState,
        Option<&'static hw_core::relationships::RestingIn>,
        Option<&'static RestAreaReservedFor>,
        Option<&'static mut PathCooldown>,
        Option<&'static mut hw_logistics::Inventory>,
    ),
    With<DamnedSoul>,
>;

pub fn pathfinding_system(
    mut commands: Commands,
    world_map: WorldMapRead,
    mut pf_context: Local<PathfindingContext>,
    mut query: PathfindingQuery,
    q_rest_areas: Query<&Transform, With<hw_jobs::RestArea>>,
    mut queries: crate::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
) {
    let mut pathfind_count = 0usize;
    let obstacle_version = world_map.obstacle_version;

    // task フェーズ → idle フェーズの順に処理
    for prioritize_tasks in [true, false] {
        // task フェーズは idle 探索用に枠を確保するため上限を絞る
        let budget = phase_budget_limit(prioritize_tasks);
        if pathfind_count >= budget {
            continue;
        }

        for (
            entity,
            transform,
            mut destination,
            mut path,
            mut task,
            mut idle,
            resting_in,
            rest_reserved_for,
            mut cooldown_opt,
            mut inventory_opt,
        ) in query.iter_mut()
        {
            let has_task = !matches!(*task, AssignedTask::None);
            // task フェーズは task あり worker のみ、idle フェーズは task なし worker のみ処理
            if has_task != prioritize_tasks {
                continue;
            }

            let idle_can_move = match idle.behavior {
                IdleBehavior::Sitting | IdleBehavior::Sleeping => false,
                IdleBehavior::Resting => resting_in.is_none(),
                IdleBehavior::GoingToRest => true,
                _ => true,
            };

            if can_skip_pathfinding_tick(
                has_task,
                idle_can_move,
                cooldown_opt.as_deref(),
                &path,
                destination.0,
                obstacle_version,
            ) {
                continue;
            }

            // --- クールダウン処理: 残フレームを消費し、切れたらコンポーネントを除去 ---
            if let Some(cooldown) = cooldown_opt.as_mut() {
                if cooldown.remaining_frames > 0 {
                    cooldown.remaining_frames -= 1;
                    continue;
                }
                commands.entity(entity).remove::<PathCooldown>();
            }

            process_worker_pathfinding(
                &mut commands,
                entity,
                transform,
                SoulPfState {
                    destination: &mut destination,
                    path: &mut path,
                    task: &mut task,
                    idle: &mut idle,
                    rest_reserved_for,
                    inventory_opt: inventory_opt.as_deref_mut(),
                },
                WorldPfCtx {
                    world_map: world_map.as_ref(),
                    pf_context: &mut pf_context,
                    pathfind_count: &mut pathfind_count,
                    budget,
                },
                &q_rest_areas,
                &mut queries,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::schedule::ApplyDeferred;
    use hw_core::events::{ResourceReservationOp, ResourceReservationRequest};
    use hw_core::relationships::WorkingOn;
    use hw_jobs::events::TaskAssignmentRequest;
    use hw_jobs::{ActiveTaskIdentity, GeneratePowerData, GeneratePowerPhase, WorkType};
    use hw_logistics::SharedResourceCache;

    #[derive(Resource, Default)]
    struct ReservationReceipts(Vec<ResourceReservationOp>);

    fn collect_reservations(
        mut reservations: MessageReader<ResourceReservationRequest>,
        mut receipts: ResMut<ReservationReceipts>,
    ) {
        receipts
            .0
            .extend(reservations.read().map(|request| request.op.clone()));
    }

    #[test]
    fn unreachable_task_destination_unassigns_and_releases_its_reservation() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(WorldMap::default())
            .init_resource::<SharedResourceCache>()
            .init_resource::<ReservationReceipts>()
            .add_message::<ResourceReservationRequest>()
            .add_message::<TaskAssignmentRequest>()
            .add_systems(
                Update,
                (pathfinding_system, ApplyDeferred, collect_reservations).chain(),
            );

        let target = app.world_mut().spawn_empty().id();
        let start = WorldMap::grid_to_world(10, 12);
        let soul = app
            .world_mut()
            .spawn((
                Transform::from_translation(start.extend(0.0)),
                DamnedSoul::default(),
                Destination(WorldMap::grid_to_world(-1, 12)),
                Path::default(),
                AssignedTask::GeneratePower(GeneratePowerData {
                    tile: target,
                    tile_pos: start,
                    phase: GeneratePowerPhase::Generating,
                }),
                IdleState::default(),
                ActiveTaskIdentity::new(target, target, WorkType::GeneratePower),
                WorkingOn(target),
            ))
            .id();

        app.update();

        assert!(matches!(
            app.world().get::<AssignedTask>(soul),
            Some(AssignedTask::None)
        ));
        assert!(app.world().get::<ActiveTaskIdentity>(soul).is_none());
        assert!(app.world().get::<WorkingOn>(soul).is_none());
        assert!(app.world().get::<PathCooldown>(soul).is_some());
        assert_eq!(
            app.world().resource::<ReservationReceipts>().0,
            vec![ResourceReservationOp::ReleaseSource {
                source: target,
                amount: 1,
            }]
        );
    }
}
