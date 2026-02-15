//! ECS Relationships モジュール
//!
//! エンティティ間の関係を Bevy 0.18 の Relationship 機能で管理します。

use bevy::prelude::*;

// ============================================================
// 使い魔 ⇔ ソウル 関係
// ============================================================

/// ソウルが使い魔に使役されていることを示す Relationship
/// ソウル側に付与される（ソウル → 使い魔への参照）
///
/// # 使用例
/// ```ignore
/// // ソウルを使い魔の部下にする
/// commands.entity(soul_entity).insert(CommandedBy(familiar_entity));
///
/// // 使役を解除する
/// commands.entity(soul_entity).remove::<CommandedBy>();
/// ```
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
#[relationship(relationship_target = Commanding)]
pub struct CommandedBy(pub Entity);

impl Default for CommandedBy {
    fn default() -> Self {
        Self(Entity::PLACEHOLDER)
    }
}

/// 使い魔が使役しているソウルの一覧を保持する RelationshipTarget
/// 使い魔側に自動的に付与・維持される
#[derive(Component, Reflect, Debug, Default)]
#[reflect(Component)]
#[relationship_target(relationship = CommandedBy)]
pub struct Commanding(Vec<Entity>);

impl Commanding {
    /// 使役中のソウル一覧をイテレータで取得
    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        self.0.iter()
    }
}

// ============================================================
// ソウル ⇔ タスク 関係
// ============================================================

/// ソウルがタスク/アイテムに取り組んでいることを示す Relationship
/// **ソウル側**に付与される（ソウル → タスク/アイテムへの参照）
///
/// # 使用例
/// ```ignore
/// // ソウルをタスクに割り当てる
/// commands.entity(soul_entity).insert(WorkingOn(task_entity));
///
/// // タスク完了時にソウルから解除
/// commands.entity(soul_entity).remove::<WorkingOn>();
/// ```
///
/// # 自動管理
/// - タスク/アイテム側には `TaskWorkers` が自動的に付与・維持される
/// - 複数のソウルが同じタスクに取り組める（TaskWorkers で追跡）
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
#[relationship(relationship_target = TaskWorkers)]
pub struct WorkingOn(pub Entity);

impl Default for WorkingOn {
    fn default() -> Self {
        Self(Entity::PLACEHOLDER)
    }
}

/// タスク/アイテムに割り当てられている作業者（ソウル）の一覧
/// タスク側に自動的に付与・維持される RelationshipTarget
///
/// # 注意
/// このコンポーネントは Bevy の Relationship 機能により自動管理される。
/// 手動で追加・削除しないこと。
#[derive(Component, Reflect, Debug, Default)]
#[reflect(Component)]
#[relationship_target(relationship = WorkingOn)]
pub struct TaskWorkers(Vec<Entity>);

impl TaskWorkers {
    /// 作業中のソウル一覧をイテレータで取得
    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        self.0.iter()
    }

    /// 作業者の人数を取得
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

// ============================================================
// 使い魔 ⇔ タスク 関係
// ============================================================

/// タスクがどの使い魔に管理されているかを示す Relationship
/// タスク側に付与される（タスク → 使い魔への参照）
///
/// # 使用例
/// ```ignore
/// // タスクを使い魔に割り当てる
/// commands.entity(task_entity).insert(ManagedBy(familiar_entity));
///
/// // 管理を解除
/// commands.entity(task_entity).remove::<ManagedBy>();
/// ```
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
#[relationship(relationship_target = ManagedTasks)]
pub struct ManagedBy(pub Entity);

impl Default for ManagedBy {
    fn default() -> Self {
        Self(Entity::PLACEHOLDER)
    }
}

#[derive(Component, Reflect, Debug, Default)]
#[reflect(Component)]
#[relationship_target(relationship = ManagedBy)]
pub struct ManagedTasks(Vec<Entity>);

impl ManagedTasks {
    /// 管理中のタスク一覧をイテレータで取得
    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        self.0.iter()
    }

    /// 特定のタスクが含まれているかチェック
    pub fn contains(&self, entity: Entity) -> bool {
        self.0.contains(&entity)
    }
}
// ============================================================
// アイテム ⇔ ストックパイル 関係
// ============================================================

/// アイテムがどの備蓄場所に格納されているかを示す Relationship
/// アイテム側に付与される（アイテム → 備蓄場所への参照）
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
#[relationship(relationship_target = StoredItems)]
pub struct StoredIn(pub Entity);

impl Default for StoredIn {
    fn default() -> Self {
        Self(Entity::PLACEHOLDER)
    }
}

/// 備蓄場所に格納されているアイテムの一覧
/// 備蓄場所側に自動的に付与・維持される RelationshipTarget
#[derive(Component, Reflect, Debug, Default)]
#[reflect(Component)]
#[relationship_target(relationship = StoredIn)]
pub struct StoredItems(Vec<Entity>);

impl StoredItems {
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

// Item -> Stockpile 関係のみを維持します。
// Familiar -> Stockpile の管理は空間検索 (TaskArea) で行います。

// ============================================================
// 手押し車 ⇔ 積載アイテム 関係
// ============================================================

/// アイテムが手押し車に積載されていることを示す Relationship
/// アイテム側に付与される（アイテム → 手押し車への参照）
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
#[relationship(relationship_target = LoadedItems)]
pub struct LoadedIn(pub Entity);

impl Default for LoadedIn {
    fn default() -> Self {
        Self(Entity::PLACEHOLDER)
    }
}

/// 手押し車に積載されているアイテムの一覧
/// 手押し車側に自動的に付与・維持される RelationshipTarget
#[derive(Component, Reflect, Debug, Default)]
#[reflect(Component)]
#[relationship_target(relationship = LoadedIn)]
pub struct LoadedItems(Vec<Entity>);

impl LoadedItems {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

// ============================================================
// 手押し車 ⇔ 駐車エリア 関係
// ============================================================

/// 手押し車が駐車エリアに駐車されていることを示す Relationship
/// 手押し車側に付与される（手押し車 → 駐車エリアへの参照）
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
#[relationship(relationship_target = ParkedWheelbarrows)]
pub struct ParkedAt(pub Entity);

impl Default for ParkedAt {
    fn default() -> Self {
        Self(Entity::PLACEHOLDER)
    }
}

/// 駐車エリアに駐車されている手押し車の一覧
/// 駐車エリア側に自動的に付与・維持される RelationshipTarget
#[derive(Component, Reflect, Debug, Default)]
#[reflect(Component)]
#[relationship_target(relationship = ParkedAt)]
pub struct ParkedWheelbarrows(Vec<Entity>);

impl ParkedWheelbarrows {}

// ============================================================
// 手押し車 ⇔ ソウル（使用中）関係
// ============================================================

/// 手押し車が特定のソウルに押されていることを示す Relationship
/// 手押し車側に付与される（手押し車 → ソウルへの参照）
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
#[relationship(relationship_target = PushingWheelbarrow)]
pub struct PushedBy(pub Entity);

impl Default for PushedBy {
    fn default() -> Self {
        Self(Entity::PLACEHOLDER)
    }
}

/// ソウルが押している手押し車の一覧
/// ソウル側に自動的に付与・維持される RelationshipTarget
#[derive(Component, Reflect, Debug, Default)]
#[reflect(Component)]
#[relationship_target(relationship = PushedBy)]
pub struct PushingWheelbarrow(Vec<Entity>);

impl PushingWheelbarrow {
    pub fn get(&self) -> Option<Entity> {
        self.0.first().copied()
    }
}
// ============================================================
// アイテム ⇔ 搬入先 予約関係
// ============================================================

/// アイテムが搬入先へ向かっていることを示す Relationship
/// アイテム側に付与（アイテム → 宛先への参照）
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
#[relationship(relationship_target = IncomingDeliveries)]
pub struct DeliveringTo(pub Entity);

impl Default for DeliveringTo {
    fn default() -> Self {
        Self(Entity::PLACEHOLDER)
    }
}

/// 搬入先に向かっているアイテムの一覧
/// 宛先側に自動的に付与・維持される RelationshipTarget
#[derive(Component, Reflect, Debug, Default)]
#[reflect(Component)]
#[relationship_target(relationship = DeliveringTo)]
pub struct IncomingDeliveries(Vec<Entity>);

impl IncomingDeliveries {
    /// 搬入予定のアイテム数を取得
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        self.0.iter()
    }
}

// ============================================================
// ソウル ⇔ 集会スポット 関係
// ============================================================

/// ソウルが集会に参加していることを示す Relationship
/// ソウル側に付与される（ソウル → 集会スポットへの参照）
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
#[relationship(relationship_target = GatheringParticipants)]
pub struct ParticipatingIn(pub Entity);

impl Default for ParticipatingIn {
    fn default() -> Self {
        Self(Entity::PLACEHOLDER)
    }
}

/// 集会スポットに参加しているソウルの一覧
/// 集会スポット側に自動的に付与・維持される RelationshipTarget
#[derive(Component, Reflect, Debug, Default)]
#[reflect(Component)]
#[relationship_target(relationship = ParticipatingIn)]
pub struct GatheringParticipants(Vec<Entity>);

impl GatheringParticipants {
    /// 参加中のソウル一覧をイテレータで取得
    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        self.0.iter()
    }

    /// 参加人数を取得
    pub fn len(&self) -> usize {
        self.0.len()
    }
}
