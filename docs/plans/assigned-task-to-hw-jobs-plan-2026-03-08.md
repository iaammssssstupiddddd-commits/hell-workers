# AssignedTask を hw_core → hw_jobs へ移動する計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `assigned-task-to-hw-jobs-plan-2026-03-08` |
| ステータス | `Draft` |
| 作成日 | `2026-03-08` |
| 最終更新日 | `2026-03-08` |
| 作成者 | `AI (Claude)` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

---

## 1. 目的

- **解決したい課題**: `AssignedTask`（ワーカーに割り当てられたジョブの実行状態）が `hw_core` に置かれている。hw_core はエンジン全体の基盤だが、`AssignedTask` はジョブ実行に特化した型であり、意味的には `hw_jobs` に属する。
- **到達したい状態**: `AssignedTask` と付随するフェーズ型が `hw_jobs` に移り、`hw_core` が純粋な「基盤層（コンポーネント基礎型・定数・システムセット・ECSイベント基盤）」に絞られている。
- **成功指標**: `cargo check` エラーゼロ。`hw_core::assigned_task` モジュールが消え、`hw_jobs::assigned_task` に存在する。

---

## 2. スコープ

### 対象（In Scope）

- `hw_core::assigned_task`（`AssignedTask` enum + 全フェーズ型・データ型）→ `hw_jobs::assigned_task` へ移動
- `hw_core::events::TaskAssignmentRequest` → `hw_jobs::events` へ移動（循環依存回避のため）
- 上記変更に伴う全 import パスの更新（hw_ai / root）

### 非対象（Out of Scope）

- `hw_core` の他モジュールの移動（soul.rs, familiar.rs, relationships.rs 等）
- logistics や visual の変更

---

## 3. 現状とギャップ

### 現在の依存ツリー

```
hw_core::assigned_task  ← hw_core::events::TaskAssignmentRequest が内部参照
  ↑
  hw_ai (9ファイルが hw_core::assigned_task を直接参照)
  ↑
  root/src/systems/soul_ai/execute/task_execution/types.rs
    pub use hw_core::assigned_task::*;  ← root 全体への配布点
```

### 循環依存の問題

`AssignedTask` を `hw_jobs` へ単純に移すと `hw_core::events` が壊れる：

```
hw_core::events::TaskAssignmentRequest { assigned_task: AssignedTask }
  ↓ AssignedTask が hw_jobs に移ると...
hw_core が hw_jobs に依存 → hw_jobs が hw_core に依存 → 循環
```

**解決策**: `TaskAssignmentRequest` を `hw_jobs::events` に移し、hw_core::events から切り離す。

### TaskAssignmentRequest の影響範囲

```
hw_core::events::TaskAssignmentRequest を参照:
  root/src/events.rs (re-export)
    → root 全体が TaskAssignmentRequest を消費

crate 側での TaskAssignmentRequest 参照: なし（hw_core::events のみ定義）
```

`hw_ai` は `TaskAssignmentRequest` を直接使用しておらず、移動コストは低い。

---

## 4. 実装方針（高レベル）

- 移動は **型定義のコピー → import パス更新 → 元ファイルを pub use に置き換え → 最終的に元ファイル削除** の順で行う。
- `hw_core::events` から `TaskAssignmentRequest` を先に分離し、hw_core 内の循環を解消してから `AssignedTask` を移動する。
- root の `src/events.rs` と `src/systems/soul_ai/execute/task_execution/types.rs` は最後に再エクスポートを差し替える。

**移動後の依存グラフ:**

```
hw_core (assigned_task なし)
  ↑
  hw_jobs (assigned_task, events::TaskAssignmentRequest)
    ↑
    hw_ai (hw_jobs から AssignedTask を参照)
    ↑
    root (hw_jobs から re-export)
```

---

## 5. マイルストーン

### M1: hw_jobs に assigned_task モジュールを追加

**変更内容**:
- `hw_core/src/assigned_task.rs` の内容を `hw_jobs/src/assigned_task.rs` にコピー
- `hw_jobs/src/lib.rs` に `pub mod assigned_task;` と `pub use assigned_task::*;` を追加
- import パスを `crate::jobs::WorkType` → `hw_core::jobs::WorkType` 等に修正（hw_jobs 内の相対参照を絶対参照に変換）

**変更ファイル**:
- `crates/hw_jobs/src/assigned_task.rs` (新規作成)
- `crates/hw_jobs/src/lib.rs` (pub mod 追加)

**注意点**:
- `hw_core/src/assigned_task.rs` はこの時点では **削除しない**。hw_core 側の消費者が壊れるため。
- `hw_jobs::assigned_task` の依存: `hw_core::jobs::WorkType`, `hw_core::logistics::{ResourceType, WheelbarrowDestination}`, `bevy::prelude::*`

**完了条件**:
- [ ] `cargo check` が通る（hw_jobs に AssignedTask が追加された状態）

---

### M2: TaskAssignmentRequest を hw_jobs::events へ移動

**変更内容**:
- `hw_jobs/src/events.rs` を新規作成し、`TaskAssignmentRequest` を定義
- `hw_core/src/events.rs` から `TaskAssignmentRequest` の定義を削除、`use crate::assigned_task::AssignedTask` 行も削除

**変更ファイル**:
- `crates/hw_jobs/src/events.rs` (新規作成)
- `crates/hw_jobs/src/lib.rs` (`pub mod events; pub use events::*;` 追加)
- `crates/hw_core/src/events.rs` (`TaskAssignmentRequest` 定義と `use crate::assigned_task` 行を削除)

**hw_jobs::events::TaskAssignmentRequest の内容**:
```rust
use hw_core::events::ResourceReservationOp;
use hw_core::jobs::WorkType;
use crate::assigned_task::AssignedTask;
use bevy::prelude::*;

#[derive(Message, Debug, Clone)]
pub struct TaskAssignmentRequest {
    pub familiar_entity: Entity,
    pub worker_entity: Entity,
    pub task_entity: Entity,
    pub work_type: WorkType,
    pub task_pos: Vec2,
    pub assigned_task: AssignedTask,
    pub reservation_ops: Vec<ResourceReservationOp>,
    pub already_commanded: bool,
}
```

**注意点**:
- hw_core::events の re-export 先（root の `src/events.rs`）が `TaskAssignmentRequest` を期待している。M4 で修正するまで一時的にコンパイルエラーになることを許容してよい（M1→M4 を一括コミットする場合）。
- あるいは hw_core::events に一時的に `pub use hw_jobs::events::TaskAssignmentRequest;` を追加して段階的に進める方が安全。

**完了条件**:
- [ ] `cargo check` が通る

---

### M3: hw_ai の import パスを hw_core → hw_jobs に変更

**変更対象ファイル** (9ファイル):

| ファイル | 変更内容 |
|:--|:--|
| `crates/hw_ai/src/soul_ai/update/vitals_update.rs` | `hw_core::assigned_task::AssignedTask` → `hw_jobs::AssignedTask` |
| `crates/hw_ai/src/soul_ai/update/vitals.rs` | `hw_core::jobs::WorkType` は変更不要（WorkType は hw_core に残る） |
| `crates/hw_ai/src/soul_ai/update/dream_update.rs` | 同上 |
| `crates/hw_ai/src/soul_ai/update/state_sanity.rs` | 同上 |
| `crates/hw_ai/src/soul_ai/update/vitals_influence.rs` | 同上 |
| `crates/hw_ai/src/soul_ai/helpers/work.rs` | 同上 |
| `crates/hw_ai/src/soul_ai/helpers/query_types.rs` | 同上 |
| `crates/hw_ai/src/soul_ai/decide/idle_behavior/task_override.rs` | 同上 |
| `crates/hw_ai/src/soul_ai/decide/gathering_mgmt.rs` | 同上 |
| `crates/hw_ai/src/soul_ai/decide/separation.rs` | 同上 |
| `crates/hw_ai/src/familiar_ai/decide/following.rs` | 同上 |

変更パターン:
```rust
// Before
use hw_core::assigned_task::AssignedTask;
// After
use hw_jobs::AssignedTask;
```

**完了条件**:
- [ ] `cargo check` が通る

---

### M4: root の re-export パスを hw_jobs に差し替え

**変更ファイル**:
- `src/systems/soul_ai/execute/task_execution/types.rs`
  ```rust
  // Before
  pub use hw_core::assigned_task::*;
  // After
  pub use hw_jobs::assigned_task::*;
  ```
- `src/events.rs`
  ```rust
  // Before: TaskAssignmentRequest は hw_core::events から
  // After: hw_jobs::events::TaskAssignmentRequest を追加
  pub use hw_jobs::events::TaskAssignmentRequest;
  ```

**完了条件**:
- [ ] `cargo check` が通る

---

### M5: hw_core から assigned_task モジュールを削除

**変更ファイル**:
- `crates/hw_core/src/assigned_task.rs` → **削除**
- `crates/hw_core/src/lib.rs` → `pub mod assigned_task;` を削除

**完了条件**:
- [ ] `cargo check` が通る
- [ ] `hw_core::assigned_task` が存在しない

---

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| hw_core::events の `TaskAssignmentRequest` 削除で root が壊れる | M2〜M4 | M2 で hw_core::events に一時的な `pub use hw_jobs::events::TaskAssignmentRequest` を追加し、M4 完了後に削除する |
| hw_ai が hw_jobs に既に依存しているため import 変更は単純 | 低 | `cargo check` で確認 |
| `AssignedTask` を参照するファイルが root に多数（`crate::systems::soul_ai::execute::task_execution` 経由の re-export で全て解決済み） | 低 | types.rs の pub use 差し替えのみで波及なし |

---

## 7. 検証計画

- 必須: 各マイルストーン完了後に `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
- 最終確認: M5 完了後、`grep -r "hw_core::assigned_task" crates/ src/` でヒットなしを確認

---

## 8. ロールバック方針

- M1〜M5 を個別コミットとする
- M5（元ファイル削除）前の状態に戻せば完全ロールバック可能
- hw_core::events への一時的な `pub use` を挟むことで各ステップを独立してコミットできる

---

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手: M1〜M5

### 次のAIが最初にやること

1. `crates/hw_jobs/src/` を確認して `assigned_task.rs` がまだないことを確認
2. `crates/hw_core/src/assigned_task.rs` を読んで内容を把握
3. M1 から着手（hw_jobs/src/assigned_task.rs の作成）

### ブロッカー/注意点

- `hw_core::events::TaskAssignmentRequest` を削除する前に hw_jobs::events に定義を追加すること。順序を間違えると広範囲のコンパイルエラーが発生する。
- root の `src/events.rs` は hw_core::events を一括 re-export しているため、TaskAssignmentRequest が hw_core::events から消えた後は hw_jobs::events から個別に re-export する必要がある（M4）。
- `WorkType` は hw_core::jobs に残す（移動しない）。

### 参照必須ファイル

- `crates/hw_core/src/assigned_task.rs`（移動元）
- `crates/hw_core/src/events.rs`（TaskAssignmentRequest の現在地）
- `crates/hw_jobs/src/lib.rs`（追加先）
- `src/systems/soul_ai/execute/task_execution/types.rs`（root の re-export 点）
- `src/events.rs`（root の re-export 点）

### 最終確認ログ

- 最終 `cargo check`: `N/A`（未着手）
- 未解決エラー: N/A

### Definition of Done

- [ ] M1〜M5 完了
- [ ] `hw_core::assigned_task` モジュールが存在しない
- [ ] `hw_jobs` が `AssignedTask` と付随型を公開している
- [ ] `cargo check` が成功

---

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-08` | `AI (Claude)` | 初版作成 |
