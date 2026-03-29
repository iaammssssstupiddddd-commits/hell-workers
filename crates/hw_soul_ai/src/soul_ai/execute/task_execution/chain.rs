//! タスクチェーンシステム
//!
//! 運搬タスク完了直後に、搬入先で作業タスクが開始できる場合は
//! 一旦 None に戻さず同一 Soul がそのまま作業に移行するロジックを集約する。

use bevy::prelude::*;
use hw_core::relationships::WorkingOn;
use hw_jobs::construction::{FloorTileState, WallTileState};
use hw_logistics::ResourceType;

use super::{
    context::TaskExecutionContext,
    types::{
        AssignedTask, BuildData, BuildPhase, CoatWallData, CoatWallPhase, FrameWallPhase,
        FrameWallTileData, PourFloorPhase, PourFloorTileData, ReinforceFloorPhase,
        ReinforceFloorTileData,
    },
};

/// 運搬完了後にチェーン移行できる作業タスクの種別
pub(super) enum ChainOpportunity {
    /// Blueprint の全素材が揃い Build スロットに空きがある
    Build { blueprint: Entity },
    /// FloorSite に Bone が搬入済みで WaitingBones タイルがある
    ReinforceFloor { tile: Entity, site: Entity },
    /// FloorSite に StasisMud が搬入済みで WaitingMud タイルがある
    PourFloor { tile: Entity, site: Entity },
    /// WallSite に Wood が搬入済みで WaitingWood タイルがある
    FrameWall { tile: Entity, site: Entity },
    /// WallSite に StasisMud が搬入済みで WaitingMud タイルがある（spawned_wall 確定済み）
    CoatWall {
        tile: Entity,
        site: Entity,
        wall: Entity,
    },
}

/// 運搬完了後にチェーン移行できる作業タスクを探す。
///
/// - `destination`: 搬入先エンティティ（Blueprint / FloorSite / WallSite）
/// - `resource_type`: 搬入したリソースタイプ
/// - `materials_complete`: Blueprint への搬入時に呼び出し元が計算済みの `bp.materials_complete()`。
///   `Some(true)` の場合のみ Build チェーンを試みる。`None` の場合は Blueprint チェックをスキップ。
pub(super) fn find_chain_opportunity(
    destination: Entity,
    resource_type: ResourceType,
    materials_complete: Option<bool>,
    ctx: &TaskExecutionContext,
) -> Option<ChainOpportunity> {
    // 1. Blueprint チェーン（materials_complete == Some(true) のときのみ）
    if materials_complete == Some(true) {
        // DesignationAccess.designations は TaskSlots / TaskWorkers を持つ
        if let Ok((_, _, _, _, task_slots_opt, task_workers_opt, _, _)) =
            ctx.queries.designation.designations.get(destination)
        {
            let max = task_slots_opt.map_or(1, |s| s.max);
            let used = task_workers_opt.map_or(0, |w| w.len());
            if used < max as usize {
                return Some(ChainOpportunity::Build {
                    blueprint: destination,
                });
            }
            // スロット満杯 → チェーン不可
            return None;
        }
    }

    // 2. FloorConstructionSite チェーン
    if ctx.queries.storage.floor_sites.get(destination).is_ok() {
        let opp = match resource_type {
            ResourceType::Bone => ctx
                .queries
                .storage
                .floor_tiles
                .iter()
                .find_map(|(tile_e, tile, workers)| {
                    if tile.parent_site == destination
                        && tile.state == FloorTileState::WaitingBones
                        && workers.is_none_or(|w| w.is_empty())
                    {
                        Some(ChainOpportunity::ReinforceFloor {
                            tile: tile_e,
                            site: destination,
                        })
                    } else {
                        None
                    }
                }),
            ResourceType::StasisMud => ctx
                .queries
                .storage
                .floor_tiles
                .iter()
                .find_map(|(tile_e, tile, workers)| {
                    if tile.parent_site == destination
                        && tile.state == FloorTileState::WaitingMud
                        && workers.is_none_or(|w| w.is_empty())
                    {
                        Some(ChainOpportunity::PourFloor {
                            tile: tile_e,
                            site: destination,
                        })
                    } else {
                        None
                    }
                }),
            _ => None,
        };
        return opp;
    }

    // 3. WallConstructionSite チェーン
    if ctx.queries.storage.wall_sites.get(destination).is_ok() {
        let opp = match resource_type {
            ResourceType::Wood => ctx
                .queries
                .storage
                .wall_tiles
                .iter()
                .find_map(|(tile_e, tile, workers)| {
                    if tile.parent_site == destination
                        && tile.state == WallTileState::WaitingWood
                        && workers.is_none_or(|w| w.is_empty())
                    {
                        Some(ChainOpportunity::FrameWall {
                            tile: tile_e,
                            site: destination,
                        })
                    } else {
                        None
                    }
                }),
            ResourceType::StasisMud => ctx
                .queries
                .storage
                .wall_tiles
                .iter()
                .find_map(|(tile_e, tile, workers)| {
                    let wall = tile.spawned_wall?;
                    if tile.parent_site == destination
                        && tile.state == WallTileState::WaitingMud
                        && workers.is_none_or(|w| w.is_empty())
                    {
                        Some(ChainOpportunity::CoatWall {
                            tile: tile_e,
                            site: destination,
                            wall,
                        })
                    } else {
                        None
                    }
                }),
            _ => None,
        };
        return opp;
    }

    None
}

/// チェーン機会に応じてタスクを移行する。
///
/// `WorkingOn` Relationship を新しい対象に付け替え、`AssignedTask` を更新する。
/// インベントリのクリアおよび予約解放は呼び出し元が行う。
pub(super) fn execute_chain(
    opportunity: ChainOpportunity,
    ctx: &mut TaskExecutionContext,
    commands: &mut Commands,
) {
    // 既存 WorkingOn を外してから新しいターゲットに付け替える
    commands.entity(ctx.soul_entity).remove::<WorkingOn>();

    match opportunity {
        ChainOpportunity::Build { blueprint } => {
            commands
                .entity(ctx.soul_entity)
                .insert(WorkingOn(blueprint));
            *ctx.task = AssignedTask::Build(BuildData {
                blueprint,
                phase: BuildPhase::GoingToBlueprint,
            });
            ctx.path.waypoints.clear();
            ctx.soul.fatigue = (ctx.soul.fatigue + 0.05).min(1.0);
            info!(
                "CHAIN: Soul {:?} chained to Build {:?}",
                ctx.soul_entity, blueprint
            );
        }
        ChainOpportunity::ReinforceFloor { tile, site } => {
            commands.entity(ctx.soul_entity).insert(WorkingOn(tile));
            *ctx.task = AssignedTask::ReinforceFloorTile(ReinforceFloorTileData {
                tile,
                site,
                phase: ReinforceFloorPhase::PickingUpBones,
            });
            ctx.path.waypoints.clear();
            ctx.soul.fatigue = (ctx.soul.fatigue + 0.05).min(1.0);
            info!(
                "CHAIN: Soul {:?} chained to ReinforceFloor tile {:?}",
                ctx.soul_entity, tile
            );
        }
        ChainOpportunity::PourFloor { tile, site } => {
            commands.entity(ctx.soul_entity).insert(WorkingOn(tile));
            *ctx.task = AssignedTask::PourFloorTile(PourFloorTileData {
                tile,
                site,
                phase: PourFloorPhase::PickingUpMud,
            });
            ctx.path.waypoints.clear();
            ctx.soul.fatigue = (ctx.soul.fatigue + 0.05).min(1.0);
            info!(
                "CHAIN: Soul {:?} chained to PourFloor tile {:?}",
                ctx.soul_entity, tile
            );
        }
        ChainOpportunity::FrameWall { tile, site } => {
            commands.entity(ctx.soul_entity).insert(WorkingOn(tile));
            *ctx.task = AssignedTask::FrameWallTile(FrameWallTileData {
                tile,
                site,
                phase: FrameWallPhase::PickingUpWood,
            });
            ctx.path.waypoints.clear();
            ctx.soul.fatigue = (ctx.soul.fatigue + 0.05).min(1.0);
            info!(
                "CHAIN: Soul {:?} chained to FrameWall tile {:?}",
                ctx.soul_entity, tile
            );
        }
        ChainOpportunity::CoatWall { tile, site, wall } => {
            commands.entity(ctx.soul_entity).insert(WorkingOn(tile));
            *ctx.task = AssignedTask::CoatWall(CoatWallData {
                tile,
                site,
                wall,
                phase: CoatWallPhase::PickingUpMud,
            });
            ctx.path.waypoints.clear();
            ctx.soul.fatigue = (ctx.soul.fatigue + 0.05).min(1.0);
            info!(
                "CHAIN: Soul {:?} chained to CoatWall tile {:?}",
                ctx.soul_entity, tile
            );
        }
    }
}
