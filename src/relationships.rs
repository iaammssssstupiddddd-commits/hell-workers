//! ECS Relationships モジュール
//!
//! エンティティ間の関係を Bevy 0.17 の Relationship 機能で管理します。

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
