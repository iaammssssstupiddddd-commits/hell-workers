# Architecture

## システム全体俯瞰 (System Overview)

本プロジェクトは Bevy 0.17 のプラグインアーキテクチャに基づき、関心事ごとに分離されています。

```mermaid
graph TD
    subgraph Engine["Bevy Engine (0.17)"]
        ECS["ECS (Entities, Components, Systems)"]
        Rel["ECS Relationships"]
        Obs["Observers"]
    end

    subgraph Core["Core Logic"]
        Jobs["Job System (tasks.md)"]
        F_AI["Familiar AI (familiar_ai.md)"]
        S_AI["Soul AI (soul_ai.md)"]
    end

    subgraph Data["Data Structures"]
        Grid["Spatial Grid (Optimized Search)"]
        Inventory["Inventory (Relational)"]
    end

    subgraph View["Visual & UI"]
        Visual["Visual Systems"]
        UI["Bevy UI Nodes"]
    end

    %% Relationships
    F_AI -->|指揮/命令| S_AI
    F_AI -->|タスク割り当て| Jobs
    S_AI -->|タスク実行| Jobs
    Jobs -->|空間検索| Grid
    S_AI -->|アイテム保持| Inventory
    
    %% Engine Integration
    Core --> ECS
    Core --> Rel
    Core --> Obs
```

## 主要なデータフロー: タスク割り当て
1.  **Designation**: `Player` または `Auto-Haul` システムが `WorkType` を持つ `Designation` を実体に付与。
2.  **Notification**: `OnAdd<Designation>` を検知し、空間グリッド (`DesignationSpatialGrid`) に登録。
3.  **Assignment**: `Familiar AI` が定期的に周辺のタスクをスキャンし、配下の `魂` に `WorkingOn` 関係を結ぶ。
4.  **Execution**: `Soul AI` が `WorkingOn` を通じて目的地を特定し、移動・作業を開始。
5.  **Completion**: 資源が尽きると実体が消滅。`Observer` が検知し、`魂` のタスクを解除。

## システムセットの実行順序
`src/main.rs` で定義されている `GameSystemSet` は以下の順序でチェーンされています：
`Input` → `Spatial` → `Logic` → `Actor` → `Visual` → `Interface`
