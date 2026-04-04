//! タスクチェーンシステム
//!
//! 運搬タスク完了直後に、搬入先で作業タスクが開始できる場合は
//! 一旦 None に戻さず同一 Soul がそのまま作業に移行するロジックを集約する。

use bevy::prelude::*;
use hw_core::relationships::WorkingOn;
use hw_jobs::construction::{FloorTileState, WallTileState};
use hw_logistics::{
    ResourceType,
    transport_request::{TransportRequestKind, TransportRequestState},
};

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
            ResourceType::Bone => {
                ctx.queries
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
                    })
            }
            ResourceType::StasisMud => {
                ctx.queries
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
                    })
            }
            _ => None,
        };
        return opp;
    }

    // 3. WallConstructionSite チェーン
    if ctx.queries.storage.wall_sites.get(destination).is_ok() {
        let opp = match resource_type {
            ResourceType::Wood => {
                ctx.queries
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
                    })
            }
            ResourceType::StasisMud => {
                ctx.queries
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
                    })
            }
            _ => None,
        };
        return opp;
    }

    None
}

/// 採集タスク完了後のチェーン先種別
pub(super) enum GatherHaulChain {
    /// WallSite / FloorSite / Stockpile への運搬（AssignedTask::Haul で処理）
    Storage { item: Entity, destination: Entity },
    /// Blueprint への運搬（AssignedTask::HaulToBlueprint で処理）
    Blueprint { item: Entity, blueprint: Entity },
    /// MudMixer への運搬（AssignedTask::HaulToMixer で処理）
    Mixer { item: Entity, mixer: Entity },
}

/// 採集タスク完了後に即時運搬チェーンが開始できる先を探す。
///
/// 優先順: WallSite/FloorSite (建築) > Blueprint > MudMixer (精製) > Stockpile (フォールバック)  
/// `resource_type`: 採集したリソース種別（Wood / Rock）  
/// `soul_pos`: Soul の現在位置  
/// 採集直後アイテムは Soul の隣接タイルに散らばっているため、4タイル以内を検索する。
pub(super) fn find_haul_chain_after_gather(
    resource_type: ResourceType,
    soul_pos: Vec2,
    ctx: &TaskExecutionContext,
) -> Option<GatherHaulChain> {
    const MAX_RADIUS_SQ: f32 =
        (hw_core::constants::TILE_SIZE * 4.0) * (hw_core::constants::TILE_SIZE * 4.0);

    // 1. 近傍の空きアイテムを探す（StoredIn なし・LoadedIn なし・Visible）
    let (item, _) = ctx
        .queries
        .resource_items
        .iter()
        .filter(|(_, _, vis, ri, stored_in, loaded_in)| {
            *vis != Visibility::Hidden
                && ri.0 == resource_type
                && stored_in.is_none()
                && loaded_in.is_none()
        })
        .map(|(e, t, _, _, _, _)| (e, t.translation.truncate()))
        .filter(|(_, pos)| pos.distance_squared(soul_pos) <= MAX_RADIUS_SQ)
        .min_by(|(_, a), (_, b)| {
            a.distance_squared(soul_pos)
                .total_cmp(&b.distance_squared(soul_pos))
        })?;

    // 2. pending な TransportRequest から建築/精製先を探す
    // priority: 建築サイト(0) > Blueprint(1) > Mixer(2)
    let mut best_construction: Option<(Entity, f32)> = None;
    let mut best_blueprint: Option<(Entity, f32)> = None;
    let mut best_mixer: Option<(Entity, f32)> = None;

    for (tr, demand, state, wheelbarrow_lease, _) in ctx.queries.transport_request_status.iter() {
        if tr.resource_type != resource_type {
            continue;
        }
        if *state != TransportRequestState::Pending {
            continue;
        }
        if wheelbarrow_lease.is_some() {
            continue;
        }
        if demand.remaining() == 0 {
            continue;
        }

        let anchor_pos: Option<Vec2> = match tr.kind {
            TransportRequestKind::DeliverToWallConstruction => ctx
                .queries
                .storage
                .wall_sites
                .get(tr.anchor)
                .ok()
                .map(|(t, _, _)| t.translation.truncate()),
            TransportRequestKind::DeliverToFloorConstruction => ctx
                .queries
                .storage
                .floor_sites
                .get(tr.anchor)
                .ok()
                .map(|(t, _, _)| t.translation.truncate()),
            TransportRequestKind::DeliverToBlueprint => ctx
                .queries
                .storage
                .blueprints
                .get(tr.anchor)
                .ok()
                .map(|(t, _, _)| t.translation.truncate()),
            TransportRequestKind::DeliverToMixerSolid => ctx
                .queries
                .storage
                .mixers
                .get(tr.anchor)
                .ok()
                .map(|(t, _, _)| t.translation.truncate()),
            _ => None,
        };
        let Some(anchor_pos) = anchor_pos else {
            continue;
        };
        let dist_sq = anchor_pos.distance_squared(soul_pos);

        match tr.kind {
            TransportRequestKind::DeliverToWallConstruction
            | TransportRequestKind::DeliverToFloorConstruction => {
                if best_construction.is_none_or(|x| dist_sq < x.1) {
                    best_construction = Some((tr.anchor, dist_sq));
                }
            }
            TransportRequestKind::DeliverToBlueprint => {
                if best_blueprint.is_none_or(|x| dist_sq < x.1) {
                    best_blueprint = Some((tr.anchor, dist_sq));
                }
            }
            TransportRequestKind::DeliverToMixerSolid => {
                if best_mixer.is_none_or(|x| dist_sq < x.1) {
                    best_mixer = Some((tr.anchor, dist_sq));
                }
            }
            _ => {}
        }
    }

    if let Some((destination, _)) = best_construction {
        return Some(GatherHaulChain::Storage { item, destination });
    }
    if let Some((blueprint, _)) = best_blueprint {
        return Some(GatherHaulChain::Blueprint { item, blueprint });
    }
    if let Some((mixer, _)) = best_mixer {
        return Some(GatherHaulChain::Mixer { item, mixer });
    }

    // フォールバック: 最近傍ストックパイル
    let (stockpile, _) = ctx
        .queries
        .storage
        .stockpiles
        .iter()
        .filter(|(stock_e, _, stock, stored_items_opt)| {
            if ctx.queries.storage.bucket_storages.get(*stock_e).is_ok() {
                return false;
            }
            let accepts_type =
                stock.resource_type.is_none() || stock.resource_type == Some(resource_type);
            if !accepts_type {
                return false;
            }
            let current = stored_items_opt.map_or(0, |si| si.len());
            let incoming = ctx
                .queries
                .reservation
                .incoming_deliveries_query
                .get(*stock_e)
                .ok()
                .map_or(0, |(_, inc)| inc.len());
            current + incoming < stock.capacity
        })
        .map(|(e, t, _, _)| (e, t.translation.truncate()))
        .min_by(|(_, a), (_, b)| {
            a.distance_squared(soul_pos)
                .total_cmp(&b.distance_squared(soul_pos))
        })?;

    Some(GatherHaulChain::Storage {
        item,
        destination: stockpile,
    })
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
