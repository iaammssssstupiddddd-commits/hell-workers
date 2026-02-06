use bevy::prelude::*;
use std::collections::HashMap;
use crate::events::ResourceReservationOp;
use crate::events::ResourceReservationRequest;
use crate::relationships::TaskWorkers;
use crate::systems::jobs::{Designation, TargetBlueprint, TargetMixer, WorkType};
use crate::systems::logistics::{BelongsTo, ResourceItem, ResourceType};
use crate::systems::soul_ai::task_execution::AssignedTask;

/// システム全体で共有されるリソース予約キャッシュ
///
/// タスク間（運搬、建築、採集など）でのリソース競合を防ぐために使用される。
/// Senseフェーズで再構築され、Thinkフェーズで仮予約、Actフェーズで確定更新される。
#[derive(Resource, Default, Debug)]
pub struct SharedResourceCache {
    /// 搬送先（Stockpile/Tank）への予約数 (Destination Reservation)
    /// Entity -> 搬送予定数
    destination_reservations: HashMap<Entity, usize>,

    /// ミキサー等への予約数 (Destination Reservation)
    /// (Entity, ResourceType) -> 搬送予定数
    mixer_dest_reservations: HashMap<(Entity, ResourceType), usize>,

    /// リソース/タンクからの取り出し予約数 (Source Reservation)
    /// Entity -> 取り出し予定数
    source_reservations: HashMap<Entity, usize>,

    /// このフレームで格納された数（コンポーネント未反映分）
    /// Entity -> 格納数
    frame_stored_count: HashMap<Entity, usize>,

    /// このフレームで取り出された数（コンポーネント未反映分）
    /// Entity -> 取り出し数
    frame_picked_count: HashMap<Entity, usize>,
}

impl SharedResourceCache {
    /// 外部から予約状況を再構築する（Senseフェーズ用）
    pub fn reset(
        &mut self,
        dest_reservations: HashMap<Entity, usize>,
        mixer_dest_reservations: HashMap<(Entity, ResourceType), usize>,
        source_reservations: HashMap<Entity, usize>,
    ) {
        self.destination_reservations = dest_reservations;
        self.mixer_dest_reservations = mixer_dest_reservations;
        self.source_reservations = source_reservations;
        self.frame_stored_count.clear();
        self.frame_picked_count.clear();
    }

    /// 搬送先（Stockpile/Tank）への予約を追加 (Destination Reservation)
    pub fn reserve_destination(&mut self, target: Entity) {
        *self.destination_reservations.entry(target).or_insert(0) += 1;
    }

    /// 搬送先（Stockpile/Tank）への予約を解除
    pub fn release_destination(&mut self, target: Entity) {
        if let Some(count) = self.destination_reservations.get_mut(&target) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.destination_reservations.remove(&target);
            }
        }
    }

    /// 指定された搬送先の現在の予約数を取得
    pub fn get_destination_reservation(&self, target: Entity) -> usize {
        self.destination_reservations.get(&target).cloned().unwrap_or(0)
    }

    /// ミキサーへの予約を追加
    pub fn reserve_mixer_destination(&mut self, target: Entity, resource_type: ResourceType) {
        *self.mixer_dest_reservations.entry((target, resource_type)).or_insert(0) += 1;
    }

    /// ミキサーへの予約を解除
    pub fn release_mixer_destination(&mut self, target: Entity, resource_type: ResourceType) {
        let key = (target, resource_type);
        if let Some(count) = self.mixer_dest_reservations.get_mut(&key) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.mixer_dest_reservations.remove(&key);
            }
        }
    }

    /// ミキサーへの予約数を取得
    pub fn get_mixer_destination_reservation(&self, target: Entity, resource_type: ResourceType) -> usize {
        self.mixer_dest_reservations.get(&(target, resource_type)).cloned().unwrap_or(0)
    }

    /// リソース取り出し予約を追加 (Source Reservation)
    pub fn reserve_source(&mut self, source: Entity, amount: usize) {
        *self.source_reservations.entry(source).or_insert(0) += amount;
    }

    /// リソース取り出し予約を解除
    pub fn release_source(&mut self, source: Entity, amount: usize) {
        if let Some(count) = self.source_reservations.get_mut(&source) {
            *count = count.saturating_sub(amount);
            if *count == 0 {
                self.source_reservations.remove(&source);
            }
        }
    }

    /// リソース取り出し予約数を取得（予約済み + このフレームで取得済み）
    pub fn get_source_reservation(&self, source: Entity) -> usize {
        let reserved = self.source_reservations.get(&source).cloned().unwrap_or(0);
        let picked = self.frame_picked_count.get(&source).cloned().unwrap_or(0);
        reserved + picked
    }

    /// アトミックに予約を試みる (Thinkフェーズ用)
    /// 
    /// 現在の予約数 + 要求量が capacity を超えなければ予約を確定し true を返す。
    /// capacity は外部（呼び出し元）が現在の実在庫や容量を確認して計算した「受け入れ可能残量」などを想定するが、
    /// ここでは単純に「コンポーネントの状態とは独立した純粋な予約管理」を行うため、
    /// 「予約可能か？」の判定ロジックは呼び出し元（TaskAssigner）が持つべき責務とする。
    /// このメソッドは単に予約を追加する。
    /// 
    /// check_and_reserve のようなヘルパーメソッドを作ることも可能だが、
    /// 判定条件が対象（Stockpile, Tank, Mixer）によって異なるため、プリミティブな予約機能を提供するに留める。


    /// 収納アクション成功を記録 (Delta Update)
    /// 予約を減らし、フレーム内格納数を増やす
    pub fn record_stored_destination(&mut self, target: Entity) {
        self.release_destination(target);
        *self.frame_stored_count.entry(target).or_insert(0) += 1;
    }

    /// 取得アクション成功を記録 (Delta Update)
    /// ソース予約を減らし、フレーム内取得数を増やす（論理在庫減少）
    pub fn record_picked_source(&mut self, source: Entity, amount: usize) {
        self.release_source(source, amount);
        *self.frame_picked_count.entry(source).or_insert(0) += amount;
    }

    /// 論理的な格納済み数（クエリ値 + フレーム内増加分）を取得
    pub fn get_logical_stored_count(&self, target: Entity, current_from_query: usize) -> usize {
        let frame_added = self.frame_stored_count.get(&target).cloned().unwrap_or(0);
        current_from_query + frame_added
    }
    
    /// 論理的な予約済み合計（実在庫 + 予約 + フレーム内増加）を取得
    /// これが capacity を超えてはいけない
    pub fn get_total_anticipated_count(&self, target: Entity, current_from_query: usize) -> usize {
        let stored = self.get_logical_stored_count(target, current_from_query);
        let reserved = self.get_destination_reservation(target);
        stored + reserved
    }
}

/// 予約操作をキャッシュに反映する
pub fn apply_reservation_op(cache: &mut SharedResourceCache, op: &ResourceReservationOp) {
    match *op {
        ResourceReservationOp::ReserveDestination { target } => {
            cache.reserve_destination(target);
        }
        ResourceReservationOp::ReleaseDestination { target } => {
            cache.release_destination(target);
        }
        ResourceReservationOp::ReserveMixerDestination { target, resource_type } => {
            cache.reserve_mixer_destination(target, resource_type);
        }
        ResourceReservationOp::ReleaseMixerDestination { target, resource_type } => {
            cache.release_mixer_destination(target, resource_type);
        }
        ResourceReservationOp::ReserveSource { source, amount } => {
            cache.reserve_source(source, amount);
        }
        ResourceReservationOp::ReleaseSource { source, amount } => {
            cache.release_source(source, amount);
        }
        ResourceReservationOp::RecordStoredDestination { target } => {
            cache.record_stored_destination(target);
        }
        ResourceReservationOp::RecordPickedSource { source, amount } => {
            cache.record_picked_source(source, amount);
        }
    }
}

/// 予約更新リクエストを反映するシステム
pub fn apply_reservation_requests_system(
    mut cache: ResMut<SharedResourceCache>,
    mut requests: MessageReader<ResourceReservationRequest>,
) {
    for request in requests.read() {
        apply_reservation_op(&mut cache, &request.op);
    }
}

/// タスク状態から予約を同期するシステム (Sense Phase)
///
/// 以下の2種類のソースから予約を再構築する:
/// 1. `AssignedTask` - 既にSoulに割り当てられているタスク
/// 2. `Designation` (Without<TaskWorkers>) - まだ割り当て待ちのタスク候補
///
/// これにより、自動発行システムが複数フレームにわたって過剰にタスクを発行することを防ぐ。
pub fn sync_reservations_system(
    q_souls: Query<&AssignedTask>,
    // まだ割り当てられていない（TaskWorkers がない）タスク候補を汎用的にスキャン
    q_pending_tasks: Query<
        (
            &Designation,
            Option<&TargetMixer>,
            Option<&TargetBlueprint>,
            Option<&BelongsTo>,
            Option<&ResourceItem>,
        ),
        Without<TaskWorkers>,
    >,
    mut cache: ResMut<SharedResourceCache>,
) {
    let mut dest_res = HashMap::new();
    let mut mixer_dest_res = HashMap::new();
    let mut source_res = HashMap::new();

    // Designation（タスク割り当て待ち）のアイテムも予約としてカウント
    // Without<TaskWorkers> により、既に割り当て済みのものは除外（AssignedTask 側でカウント）
    for (designation, target_mixer, target_blueprint, belongs_to, resource_item) in q_pending_tasks.iter() {
        match designation.work_type {
            WorkType::Haul => {
                // ブループリント向け運搬
                if let Some(blueprint) = target_blueprint {
                    *dest_res.entry(blueprint.0).or_insert(0) += 1;
                }
                // 通常のStockpile向けHaulは、どのStockpileか不明のためカウントしない
                // （task_area_auto_haul は Without<Designation> でフィルタしているので問題なし）
            }
            WorkType::HaulToMixer => {
                // 固体原料（Sand/Rock）のミキサー向け運搬
                if let (Some(mixer), Some(res)) = (target_mixer, resource_item) {
                    *mixer_dest_res.entry((mixer.0, res.0)).or_insert(0) += 1;
                }
            }
            WorkType::HaulWaterToMixer => {
                // ミキサー向け水運搬（バケツ1杯 = 1予約）
                if let Some(mixer) = target_mixer {
                    *mixer_dest_res.entry((mixer.0, ResourceType::Water)).or_insert(0) += 1;
                }
            }
            WorkType::GatherWater => {
                // 水汲みタスク（BelongsTo はタンクを指す）
                if let Some(belongs) = belongs_to {
                    *dest_res.entry(belongs.0).or_insert(0) += 1;
                }
            }
            // 他の WorkType は現状予約カウント不要
            // CollectSand, Refine, Build 等は source_res で管理されるが、
            // Designation 段階では不要（AssignedTask になってからカウント）
            _ => {}
        }
    }

    for task in q_souls.iter() {
        match task {
            AssignedTask::Haul(data) => {
                // ストックパイルへの搬送予約
                *dest_res.entry(data.stockpile).or_insert(0) += 1;
                // アイテム（ソース）への取り出し予約（まだ持っていない場合）
                use crate::systems::soul_ai::task_execution::types::HaulPhase;
                if matches!(data.phase, HaulPhase::GoingToItem) {
                    *source_res.entry(data.item).or_insert(0) += 1;
                }
            }
            AssignedTask::GatherWater(data) => {
                // タンクへの搬送（返却）予約... はGatherWaterだと「バケツを取りに行く」フェーズか「水を汲む」フェーズかによる
                // GatherWaterタスクの意味合い定義によるが、通常は水を汲んで戻ってくるまでを含む？
                // 既存実装: GatherWaterは「バケツを持ってタンクに行き、水を汲む」まで。
                // 水を汲んだ後は HaulWaterToMixer などに派生しそうだが、ここでは「タンクそのもの」への予約（他人が使わないように？）
                // いや、HaulReservationCacheの実装を見ると `AssignedTask::GatherWater(data) => reserves(data.tank)` となっていた。
                // これは「タンクのキャパシティ（バケツを戻す場所としての？）」を予約しているのか？
                // コードを読むと `best_tank` 選定時に `has_capacity` をチェックしている。
                // つまり「水入りバケツを作るための空き容量」ではなく「バケツ（アイテム）をタンクエリアに置くための容量」か？
                // だとすると Destination Reservation で正しい。
                *dest_res.entry(data.tank).or_insert(0) += 1;

                use crate::systems::soul_ai::task_execution::types::GatherWaterPhase;
                if matches!(data.phase, GatherWaterPhase::GoingToBucket) {
                    *source_res.entry(data.bucket).or_insert(0) += 1;
                }
            }
            AssignedTask::HaulToMixer(data) => {
                *mixer_dest_res.entry((data.mixer, data.resource_type)).or_insert(0) += 1;
                
                use crate::systems::soul_ai::task_execution::types::HaulToMixerPhase;
                if matches!(data.phase, HaulToMixerPhase::GoingToItem) {
                    *source_res.entry(data.item).or_insert(0) += 1;
                }
            }
            AssignedTask::HaulWaterToMixer(data) => {
                *mixer_dest_res.entry((data.mixer, ResourceType::Water)).or_insert(0) += 1; // ここは水量単位ではなく「作業員数単位」で予約しているかもしれない（既存実装依存）
                // 既存実装では `*mixer_reservations.entry((data.mixer, ResourceType::Water)).or_insert(0) += 1;` となっていた。
                // つまり、1人の作業員が向かっている＝1予約、としている。水量は考慮されていない（あるいはバケツ1杯分と仮定？）
                // ここでは既存のロジックを踏襲する。
                
                use crate::systems::soul_ai::task_execution::types::HaulWaterToMixerPhase;
                if matches!(data.phase, HaulWaterToMixerPhase::GoingToBucket) {
                    *source_res.entry(data.bucket).or_insert(0) += 1;
                } else if matches!(data.phase, HaulWaterToMixerPhase::FillingFromTank) {
                     // タンクから水を汲む予約。ここでタンクをSourceとして予約すべきか？
                     // 水リソースそのものを予約するのは難しい（水アイテムは動的に生成/消滅したりする？）
                     // タンクロジック依存だが、ここではタンクEntityをSourceとして予約数に入れておく
                     // amountはとりあえず1（作業員一人分）とする
                     *source_res.entry(data.tank).or_insert(0) += 1;
                }
            }
            // 他のタスク（Build, CollectSand, Refine）も必要に応じて追加
            AssignedTask::Build(data) => {
                // 建材を持っていくソース予約が必要かもしれないが、
                // Buildタスク自体は「資材を持って現地に行く」のか「現地で建築する」のか。
                // 既存実装では `GoingToBlueprint` で移動。
                // 手持ちがない場合は別途HaulToBlueprintタスクになるはず（TaskAssignerが切り替える）。
                // よって Build タスク中は既にアイテムを持っているか、あるいは不要。
                // ... と思われたが、Cycle Framework移行によりSource Reservationが必要になったため追記
                use crate::systems::soul_ai::task_execution::types::BuildPhase;
                if matches!(data.phase, BuildPhase::GoingToBlueprint | BuildPhase::Building { .. }) {
                    *source_res.entry(data.blueprint).or_insert(0) += 1;
                }
            }
            AssignedTask::HaulToBlueprint(data) => {
                // ブループリントへの搬送予約（正しくはBlueprintが必要とする資材枠への予約）
                // 現状のSharedResourceCacheはEntity単位。Blueprint EntityへのDestination Reservationとする。
                 *dest_res.entry(data.blueprint).or_insert(0) += 1;

                use crate::systems::soul_ai::task_execution::types::HaulToBpPhase;
                if matches!(data.phase, HaulToBpPhase::GoingToItem) {
                    *source_res.entry(data.item).or_insert(0) += 1;
                }
            }
            AssignedTask::Gather(data) => {
                // 木や岩への予約（複数人同時作業の制御用）
                use crate::systems::soul_ai::task_execution::types::GatherPhase;
                if matches!(data.phase, GatherPhase::GoingToResource | GatherPhase::Collecting { .. }) {
                    *source_res.entry(data.target).or_insert(0) += 1;
                }
            }
            AssignedTask::CollectSand(data) => {
                // SandPileへの予約
                use crate::systems::soul_ai::task_execution::types::CollectSandPhase;
                if matches!(data.phase, CollectSandPhase::GoingToSand | CollectSandPhase::Collecting { .. }) {
                    *source_res.entry(data.target).or_insert(0) += 1;
                }
            }
            AssignedTask::Refine(data) => {
                // Mixerへの予約（精製作業の排他制御）
                use crate::systems::soul_ai::task_execution::types::RefinePhase;
                if matches!(data.phase, RefinePhase::GoingToMixer | RefinePhase::Refining { .. }) {
                    *source_res.entry(data.mixer).or_insert(0) += 1;
                }
            }
            _ => {}
        }
    }

    cache.reset(dest_res, mixer_dest_res, source_res);
}
