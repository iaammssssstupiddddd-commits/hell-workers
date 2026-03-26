//! タスク実行の共通処理

use crate::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::soul_ai::execute::task_execution::types::AssignedTask;
use bevy::prelude::*;
use hw_core::soul::Path;
use hw_jobs::Designation;
use hw_logistics::{Inventory, ReservedForTask, Stockpile};

use hw_world::WorldMap;

// pure helper を hw_ai から re-export
pub use crate::soul_ai::helpers::navigation::{
    can_pickup_item, is_adjacent_grid, is_near_blueprint, is_near_target, is_near_target_or_dest,
    update_destination_if_needed,
};

// パスキャッシュ・経路設定ヘルパーを re-export
pub use super::path_cache::{update_destination_to_adjacent, update_destination_to_blueprint};

/// タスクとパスをクリア
pub fn clear_task_and_path(task: &mut AssignedTask, path: &mut Path) {
    *task = AssignedTask::None;
    path.waypoints.clear();
}

/// 指定が解除されていたらタスクをキャンセル
///
/// 指定が解除されていた場合、タスクとパスをクリアして`true`を返します。
/// 指定が存在する場合、`false`を返します。
pub fn cancel_task_if_designation_missing(
    des_opt: Option<&Designation>,
    task: &mut AssignedTask,
    path: &mut Path,
) -> bool {
    if des_opt.is_none() {
        clear_task_and_path(task, path);
        return true;
    }
    false
}

/// アイテムをピックアップ
///
/// 魂にアイテムを持たせ、アイテムを非表示にします。
pub fn pickup_item(
    commands: &mut Commands,
    _soul_entity: Entity,
    item_entity: Entity,
    inventory: &mut Inventory,
) {
    inventory.0 = Some(item_entity);
    commands.entity(item_entity).try_insert(Visibility::Hidden);

    // タスク指定・備蓄状態を削除
    //
    // 重要: `IssuedBy(=ManagedBy)` はここでは削除しない。
    // タスク実行中にアイテムを一時的に拾っている間も「どの使い魔が管理していたか」を保持しておくことで、
    // タスク放棄などでドロップされた場合でも ManagedTasks 経由で再検知できる。
    commands
        .entity(item_entity)
        .remove::<hw_jobs::Designation>()
        .remove::<hw_jobs::TaskSlots>()
        .remove::<hw_jobs::Priority>()
        .remove::<ReservedForTask>()
        .remove::<hw_core::relationships::StoredIn>();
}

/// アイテムを地面に落とす
pub fn drop_item(commands: &mut Commands, _soul_entity: Entity, item_entity: Entity, pos: Vec2) {
    commands
        .entity(item_entity)
        .try_insert((Visibility::Visible, Transform::from_xyz(pos.x, pos.y, 0.6)));
}

/// ストックパイルからアイテムが取り出された際の更新処理
///
/// ストックパイルが空になった場合、リソースタイプをリセットします。
pub fn update_stockpile_on_item_removal(
    stock_entity: Entity,
    q_stockpiles: &mut Query<(
        Entity,
        &Transform,
        &mut Stockpile,
        Option<&hw_core::relationships::StoredItems>,
    )>,
) {
    if let Ok((_, _, mut stock_comp, Some(stored_items))) = q_stockpiles.get_mut(stock_entity) {
        // 自分を引いて 0 個になるなら None に戻す
        if stored_items.len() <= 1 {
            stock_comp.resource_type = None;
            info!(
                "STOCKPILE: Stockpile {:?} became empty. Resetting resource type.",
                stock_entity
            );
        }
    }
}

/// MudMixer 在庫として保持していた mud アイテムが持ち出された際の在庫解放
pub fn release_mixer_mud_storage_for_item(
    ctx: &mut crate::soul_ai::execute::task_execution::context::TaskExecutionContext,
    item_entity: Entity,
    commands: &mut Commands,
) {
    let Ok(stored_by_mixer) = ctx.queries.mixer_stored_mud.get(item_entity) else {
        return;
    };

    if let Ok((_, mut mixer_storage, _)) = ctx.queries.storage.mixers.get_mut(stored_by_mixer.0) {
        mixer_storage.mud = mixer_storage.mud.saturating_sub(1);
    }

    commands
        .entity(item_entity)
        .remove::<hw_jobs::StoredByMixer>();
}

/// `try_pickup_item` のロケーション引数をまとめた構造体。
pub struct PickupLocations {
    pub soul_entity: Entity,
    pub item_entity: Entity,
    pub soul_pos: Vec2,
    pub item_pos: Vec2,
}

/// 拾い判定が満たされない場合はタスクをクリアする
pub fn try_pickup_item(
    commands: &mut Commands,
    locations: PickupLocations,
    inventory: &mut Inventory,
    task: &mut AssignedTask,
    path: &mut Path,
) -> bool {
    let PickupLocations {
        soul_entity,
        item_entity,
        soul_pos,
        item_pos,
    } = locations;
    if !can_pickup_item(soul_pos, item_pos) {
        clear_task_and_path(task, path);
        return false;
    }
    pickup_item(commands, soul_entity, item_entity, inventory);
    true
}

// ---------------------------------------------------------------------------
// 移動フェーズ共通ヘルパー
// ---------------------------------------------------------------------------

/// 指定への隣接移動フェーズの処理結果
#[derive(Debug, PartialEq, Eq)]
pub enum NavOutcome {
    /// 移動中: 特別な処理不要
    Moving,
    /// 到達済み: 次フェーズへ遷移可能
    Arrived,
    /// 指定が解除された: task/path はすでにクリア済み
    Cancelled,
    /// 到達不能: task/path はまだ残っている（呼び出し元でクリーンアップ）
    Unreachable,
}

/// 指定チェック → 隣接ナビゲーション → 到達判定をまとめたヘルパー。
///
/// - 指定が存在しない (`designation_present: false`) なら `Cancelled`（task+path はすでにクリア済み）
/// - 到達不能なら `Unreachable`
/// - 到達済みなら `Arrived`
/// - 移動中なら `Moving`
///
/// # Note
/// `designation_present` は呼び出し元で `des_opt.is_some()` に評価してから渡すこと。
/// これにより `ctx` に対するイミュータブル借用（クエリ結果）を事前に解放できる。
pub fn navigate_to_adjacent(
    ctx: &mut TaskExecutionContext,
    designation_present: bool,
    target_pos: Vec2,
    soul_pos: Vec2,
    world_map: &WorldMap,
) -> NavOutcome {
    if !designation_present {
        clear_task_and_path(ctx.task, ctx.path);
        return NavOutcome::Cancelled;
    }
    let reachable = update_destination_to_adjacent(
        ctx.dest,
        target_pos,
        ctx.path,
        soul_pos,
        world_map,
        ctx.pf_context,
    );
    if !reachable {
        return NavOutcome::Unreachable;
    }
    if is_near_target(soul_pos, target_pos) {
        NavOutcome::Arrived
    } else {
        NavOutcome::Moving
    }
}

/// 指定チェックなしの隣接移動フェーズヘルパー。
///
/// 指定（Designation）が存在しない hauling など、キャンセル条件が呼び出し元固有の
/// フェーズで使用する。`navigate_to_adjacent` と異なり `Cancelled` を返さない。
pub fn navigate_to_pos(
    ctx: &mut TaskExecutionContext,
    target_pos: Vec2,
    soul_pos: Vec2,
    world_map: &WorldMap,
) -> NavOutcome {
    let reachable = update_destination_to_adjacent(
        ctx.dest,
        target_pos,
        ctx.path,
        soul_pos,
        world_map,
        ctx.pf_context,
    );
    if !reachable {
        return NavOutcome::Unreachable;
    }
    if is_near_target(soul_pos, target_pos) {
        NavOutcome::Arrived
    } else {
        NavOutcome::Moving
    }
}

/// 収集対象が消えた・到達不能のときの共通クリーンアップ。
///
/// Designation / TaskSlots を削除し、予約を解放してからタスクをクリアする。
pub fn cleanup_collect_target(
    ctx: &mut TaskExecutionContext,
    target: Entity,
    commands: &mut Commands,
) {
    cleanup_collect_common(ctx, target, commands, false);
}

fn cleanup_collect_common(
    ctx: &mut TaskExecutionContext,
    target: Entity,
    commands: &mut Commands,
    remove_issued_by: bool,
) {
    let mut entity_cmd = commands.entity(target);
    entity_cmd
        .remove::<hw_jobs::Designation>()
        .remove::<hw_jobs::TaskSlots>();
    if remove_issued_by {
        entity_cmd.remove::<hw_jobs::IssuedBy>();
    }
    ctx.queue_reservation(hw_core::events::ResourceReservationOp::ReleaseSource {
        source: target,
        amount: 1,
    });
    clear_task_and_path(ctx.task, ctx.path);
}

/// 収集タスク Done フェーズの共通クリーンアップ。
///
/// Designation / TaskSlots / IssuedBy を削除し、予約を解放してからタスクをクリアする。
pub fn finalize_collect_task(
    ctx: &mut TaskExecutionContext,
    target: Entity,
    commands: &mut Commands,
) {
    cleanup_collect_common(ctx, target, commands, true);
}
