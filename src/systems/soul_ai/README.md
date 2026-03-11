# soul_ai — Soul（魂）AI 意思決定システム

## 役割

`DamnedSoul` エンティティの自律的な意思決定・行動を実装する。
基本的な AI ロジックは `hw_ai::soul_ai` に定義されており、このディレクトリは**ゲーム固有の拡張**（タスク実行・ドリフト・集会スポーン等）を担う。

## ディレクトリ構成

| ディレクトリ | フェーズ | 内容 |
|---|---|---|
| `perceive/` | Perceive | 環境情報の読み取り（`hw_ai` から re-export） |
| `update/` | Update | 時間経過によるバイタル・状態更新（`hw_ai` から re-export） |
| `decide/` | Decide | 次行動の選択・リクエスト生成 |
| `execute/` | Execute | 決定された行動の実行・ECS 変更 |
| `helpers/` | 共通 | 共有ヘルパー（`hw_ai` から re-export） |
| `visual/` | Visual | Soul 固有のビジュアル同期 |

## decide/ ディレクトリ

| ファイル/ディレクトリ | 内容 |
|---|---|
| `idle_behavior/` | `hw_ai` 実装を公開する薄い re-export（`mod.rs` のみ） |
| `work/` | `hw_ai` 実装を公開する薄い re-export（`auto_build.rs`, `auto_refine.rs`） |
| `drifting.rs` | 漂流（自然脱走）行動決定 |
| `escaping.rs` | 脱走行動決定 |
| `gathering_mgmt.rs` | 集会行動管理 |

## execute/ ディレクトリ

| ファイル/ディレクトリ | 内容 |
|---|---|
| `task_execution/` | タスク実行コア（下表参照） |
| `drifting.rs` | 漂流実行 |
| `gathering_apply.rs` | 集会場所への移動実行 |
| `gathering_spawn.rs` | `GatheringSpawnRequest` を消費する visual adapter |
| `cleanup.rs` | タスク完了後のクリーンアップ |
| `escaping_apply` (inline) | `hw_ai` から re-export（`execute/mod.rs` 内 inline module） |
| `idle_behavior_apply` (inline) | `hw_ai` から re-export（`execute/mod.rs` 内 inline module） |

## execute/task_execution/ ディレクトリ

タスク実行のコアサブシステム。

| ファイル/ディレクトリ | 内容 |
|---|---|
| `types.rs` | `AssignedTask` 実行バリアント定義 |
| `context/` | `TaskExecutionContext`, `TaskQueries`, `TaskAssignmentQueries` |
| `handler/` | タスク種別ごとのハンドラ |
| `haul/` | 運搬（基本） |
| `haul_with_wheelbarrow/` | 手押し車運搬 |
| `bucket_transport/` | バケツ輸送 |
| `transport_common/` | 輸送共通ロジック |
| `build.rs` | 建設タスク |
| `collect_sand.rs` | 砂採集 |
| `collect_bone.rs` | 骨採集 |
| `coat_wall.rs` | 壁コーティング |
| `frame_wall.rs` | 壁フレーミング |
| `gather.rs` | 採取 |
| `haul.rs` | 運搬エントリポイント |
| `haul_to_blueprint.rs` | ブループリントへの運搬 |
| `haul_to_mixer.rs` | ミキサーへの運搬 |
| `move_plant.rs` | 植物移植 |
| `pour_floor.rs` | 床注入 |
| `refine.rs` | 素材精製 |
| `reinforce_floor.rs` | 床補強 |

## 新しいタスクを追加する場合

1. `types.rs` に struct variant を追加
2. `context/queries.rs` の `TaskQueries` にクエリを追加
3. `handler/` に対応するハンドラを実装
4. `mod.rs` でハンドラを登録

---

## root-only 契約

`src/systems/soul_ai` にファイルを残してよい条件は、以下のいずれかを満たすものだけ。
**条件を満たさない新規ロジックは `hw_ai::soul_ai` に置くこと。**

| 残留条件 | 代表例 |
|---|---|
| `GameAssets` / sprite / entity spawn に直接依存する | `execute/gathering_spawn.rs` |
| UI / camera / gizmo 依存 | `visual/gathering.rs`, `visual/vitals.rs` |
| `PopulationManager` など root 固有リソースを直接読む | `decide/drifting.rs`, `execute/drifting.rs` |
| `task_execution` の full-fat query / `unassign_task` 副作用を持つ | `execute/task_execution/**`, `helpers/work.rs` |
| Plugin wiring（system 登録・`MessagesPlugin` 連携）| `mod.rs` |

**逆に以下のものは `hw_ai` または `hw_spatial`・`hw_core` へ移設する**:
- shared model・shared events・`hw_world::WorldMap`・`hw_spatial` の resource だけで閉じるロジック
- 純粋な Decide メッセージ生成（`GameAssets` / `WorldMapRead` wrapper 非依存のもの）
- 空間グリッド定義（対応 Component が `hw_core` / `hw_jobs` にある場合）

> **ファイル vs inline module の使い分け**:  
> `hw_ai` 側に正式な実装がある場合、root では **ファイルを置かず** `mod.rs` の inline module で re-export する。  
> ファイルと inline module が両方あると、Rust はファイルを **無視** するため stale file になる。

---

## hw_ai との境界

Soul AI は `hw_ai::soul_ai` と `src/systems/soul_ai` に分割されている。

### hw_ai に置かれているもの（純粋ロジック）

| モジュール | 内容 |
|---|---|
| `update/` | バイタル更新・状態整合・夢更新・集会タイマー |
| `decide/idle_behavior/mod.rs` | `idle_behavior_decision_system` 本体 |
| `decide/work/auto_refine.rs` | MudMixer の自動精製指定発行 |
| `decide/work/auto_build.rs` | 資材完了 Blueprint への自動割り当て |
| `decide/separation.rs` | 分離行動（純粋空間計算） |
| `execute/escaping_apply.rs` | 脱走移動実行 |
| `execute/idle_behavior_apply.rs` | アイドル行動実行 |
| `execute/designation_apply.rs` | 指定適用 |
| `execute/gathering_apply.rs` | 集会移動実行 |
| `execute/gathering_spawn.rs` | 集会発生判定と `GatheringSpawnRequest` 発行 |
| `helpers/gathering.rs` | `hw_core::gathering` の互換 re-export |
| `helpers/work.rs` の `is_soul_available_for_work` | 純粋可否判定 |

### src/ に置かれているもの（ゲーム固有・副作用あり）

| モジュール | 理由 |
|---|---|
| `execute/task_execution/` (全23ファイル) | `WorldMap`・`Transform`・`Visibility`・ECS Relationship に依存 |
| `execute/drifting.rs` | `Path` / `DriftingState` 書き換え + 境界経路探索 |
| `execute/gathering_spawn.rs` | `GameAssets` を使う visual spawn と request 消費時の状態再検証を行う |
| `helpers/work.rs` の `unassign_task` | `WorldMap`・`Visibility`・`Transform` を変更 |

### 典型的な拡張パターン

```rust
// src/systems/soul_ai/execute/mod.rs
// hw_ai の純粋な Apply をそのまま公開
pub mod escaping_apply {
    pub use hw_ai::soul_ai::execute::escaping_apply::*;
}
// ゲーム固有のタスク実行は src/ に実装
pub mod task_execution;  // 全て src/ 独自
```

```rust
// src/systems/soul_ai/helpers/work.rs
// 純粋関数は hw_ai から、副作用関数は src/ に
pub use hw_ai::soul_ai::helpers::work::is_soul_available_for_work;
pub fn unassign_task(..., world_map: &WorldMap) { ... }  // WorldMap 参照が必要
```

`decide/work/*.rs` と `decide/idle_behavior/mod.rs` は互換パス維持用の thin re-export で、実装本体と system 登録は `hw_ai::soul_ai::*` と `hw_ai::SoulAiCorePlugin` が担当する。`execute/task_execution::apply_task_assignment_requests_system` も同様に root 側は re-export のみを持ち、system 登録は `hw_ai::SoulAiCorePlugin` に一本化する。`src/systems/soul_ai/mod.rs` はこの system を `.after(...)` / `.before(...)` の ordering 参照にのみ使う。
