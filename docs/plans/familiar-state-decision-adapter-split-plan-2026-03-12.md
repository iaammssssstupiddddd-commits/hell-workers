# Familiar State Decision Adapter Split Plan

root `state_decision.rs` に残る Decide orchestration を、`hw_ai` の pure outcome core と root の message adapter に分離するための計画。

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `familiar-state-decision-adapter-split-plan-2026-03-12` |
| ステータス | `Done` |
| 作成日 | `2026-03-12` |
| 最終更新日 | `2026-03-12` |
| 作成者 | `AI (Codex)` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

> コードサーベイ基準日: `2026-03-12`

## 1. 目的

- 解決したい課題: `src/systems/familiar_ai/decide/state_decision.rs` が `SpatialGrid` / full-fat query / `MessageWriter` を同時に抱え、使い魔 1 体分の状態判断と request 発行が 1 ファイルに混在している。
- 到達したい状態: `hw_ai` 側に「使い魔 1 体分の状態判断の分岐制御と結果集約 core」を置き、root 側 `state_decision.rs` は query/resource 取得・レンズ分割・message 変換だけを担う thin adapter に縮退する。
- 成功指標:
  - `crates/hw_ai/src/familiar_ai/decide/state_decision.rs` が新設され、`FamiliarDecisionPath` / `DecisionEmitOp` / `FamiliarStateDecisionResult` と `determine_decision_path` を所有する
  - root `state_decision.rs` の `familiar_ai_state_system` から pure branching と inline message 構築が除去される
  - `FamiliarDecideOutput` への書き込みが root adapter の専用ヘルパーに集約される
  - `cargo check -p hw_ai` と `cargo check --workspace` が成功する

## 2. スコープ

### 対象（In Scope）

- `state_decision.rs` の分岐制御 core / message adapter 分離
- `hw_ai::familiar_ai::decide::state_decision` への `FamiliarDecisionPath` / `DecisionEmitOp` / `FamiliarStateDecisionResult` 追加
- root `state_decision.rs` の message 変換を専用ヘルパーに集約
- `docs/familiar_ai.md` / `docs/cargo_workspace.md` / `src/systems/familiar_ai/README.md` の境界説明更新

### 非対象（Out of Scope）

- `task_delegation.rs` / `familiar_processor.rs` の crate 移設
- `task_management` / pathfinding / `WorldMapRead` 周りの再設計
- `encouragement.rs` / `auto_gather_for_blueprint.rs` の同時整理
- gameplay アルゴリズム変更
- `FamiliarSoulQuery` / `transmute_lens_filtered` の構造変更

## 3. 現状とギャップ

### 現状のファイル構成（コードサーベイ済み）

| ファイル | 役割 | 所有場所 |
| --- | --- | --- |
| `src/systems/familiar_ai/decide/state_decision.rs` | `familiar_ai_state_system` 本体（約382行）。分岐制御・lens 構築・context 構築・message 発行を全て担う | root |
| `crates/hw_ai/src/familiar_ai/decide/recruitment.rs` | `process_recruitment` / `FamiliarRecruitmentContext` / `RecruitmentOutcome` | hw_ai ✅ |
| `crates/hw_ai/src/familiar_ai/decide/scouting.rs` | `scouting_logic` / `FamiliarScoutingContext` | hw_ai ✅ |
| `crates/hw_ai/src/familiar_ai/decide/state_handlers/` | `handle_idle_state` / `handle_scouting_state` / `StateTransitionResult` | hw_ai ✅ |
| `crates/hw_ai/src/familiar_ai/decide/helpers.rs` | `process_squad_management` / `finalize_state_transitions` | hw_ai ✅ |
| `crates/hw_ai/src/familiar_ai/decide/query_types.rs` | `SoulSquadQuery` / `SoulRecruitmentQuery` / `SoulScoutingQuery` / `SoulSupervisingQuery` | hw_ai ✅ |
| `src/systems/familiar_ai/helpers/query_types.rs` | `FamiliarSoulQuery`（10フィールド full-fat）/ `FamiliarStateQuery` / `FamiliarTaskQuery` | root |

### root `state_decision.rs` 残存内容の分析

現在のファイルは `familiar_ai_state_system` の1関数で以下を全て行っている（行番号は現コード基準）：

| コード区間 | 役割 | 移設可否 |
| --- | --- | --- |
| L19-28: `write_add_member_request` | `SquadManagementRequest::AddMember` の MessageWriter ラッパー | **root 残留**（`MessageWriter` 依存）|
| L30-44: `write_release_requests` | `SquadManagementRequest::ReleaseMember` の MessageWriter ラッパー | **root 残留**（`MessageWriter` 依存）|
| L46-56: `FamiliarAiStateDecisionParams` | `SystemParam`（`SpatialGrid`・query 群・`FamiliarDecideOutput`）| **root 残留**（Bevy 具体型 / root 所有型）|
| L96-99: `needs_recruitment` 計算 | `max_workers > 0 && current_count < max_workers` | **hw_ai へ移設**（pure bool）|
| L105-221: Idle path 分岐制御 | `Scouting` 継続 vs 新規招募 vs 満員 Idle の3分岐 | **hw_ai へ移設**（純 enum 返し）|
| L115-140: lens 構築（Scouting 用） | `transmute_lens_filtered` で `SoulScoutingQuery` を取得 | **root 残留**（`FamiliarSoulQuery` は root 型）|
| L157-167: lens 構築（Recruit 用） | `transmute_lens_filtered` で `SoulRecruitmentQuery` を取得 | **root 残留**（同上）|
| L248-265: lens 構築（Squad 用） | `transmute_lens_filtered` で `SoulSquadQuery` を取得 | **root 残留**（同上）|
| L249-270: non-Idle squad 管理 + release 発行 | `process_squad_management` 呼び出し + release write | 呼び出し側の **dispatching を hw_ai へ**、message write は root 残留 |
| L272-358: non-Idle 分岐制御 | `Scouting` 継続 vs その他 の2分岐 | **hw_ai へ移設**（純 enum 返し）|
| L360-378: `state_changed` 時の event 発行 | `FamiliarStateRequest` + `FamiliarAiStateChangedEvent` 発行 | **root 残留**（`MessageWriter` 依存）|

### `transmute_lens_filtered` 正確なタプル型（M2 実装用）

root adapter が各分岐で使う lens のタプル型を現行コードから正確に転記する：

```rust
// Scouting 用（L115-122 / L277-283 と同型）
q_souls.transmute_lens_filtered::<
    (Entity, &Transform, &DamnedSoul, &AssignedTask, Option<&CommandedBy>),
    Without<crate::entities::familiar::Familiar>,
>()

// Recruitment 用（L157-166 / L313-321 と同型）
q_souls.transmute_lens_filtered::<
    (Entity, &Transform, &DamnedSoul, &AssignedTask, &IdleState, Option<&CommandedBy>),
    Without<crate::entities::familiar::Familiar>,
>()

// Squad 用（L253-257 と同型）
q_souls.transmute_lens_filtered::<
    (Entity, &DamnedSoul, &IdleState, Option<&CommandedBy>),
    Without<crate::entities::familiar::Familiar>,
>()
```

これらのタプル型は `hw_ai::familiar_ai::decide::query_types` の `SoulScoutingQuery` / `SoulRecruitmentQuery` / `SoulSquadQuery` に対応している。ただし **root の `query_types.rs`** がこれらを `pub use hw_ai::familiar_ai::decide::query_types::...` として再 export しており（`src/systems/familiar_ai/helpers/query_types.rs` L9-12）、lens の結果型はこれと一致する。

### メッセージ発行順序（パス別）

`emit_state_decision_messages` の実装時に維持すべき順序：

| path | 順序 |
|---|---|
| `IdleScoutingContinue` | `write_add_member_request`（recruited があれば）→ `state_requests.write` → `state_changed_events.write` |
| `IdleRecruitSearch` | `write_add_member_request`（ImmediateRecruit）→ `state_requests.write` → `state_changed_events.write` |
| `IdleSquadFull` | `idle_visual_requests.write` → `state_requests.write` → `state_changed_events.write` |
| `NonIdleScoutingContinue` | `write_release_requests` → `write_add_member_request`（recruited があれば）→ `state_requests.write` → `state_changed_events.write` |
| `NonIdleRecruitOrTransition` | `write_release_requests` → `write_add_member_request`（ImmediateRecruit）→ `state_requests.write` → `state_changed_events.write` |

### なぜ `FamiliarSoulQuery` の lens が root に残るか

`FamiliarSoulQuery` には `crate::systems::logistics::Inventory`（root 型）と `crate::entities::familiar::Familiar`（root 型）が含まれるため、hw_ai に持ち込めない。`transmute_lens_filtered` は root でのみ呼べる。

### 移設できる「分岐制御」の正体

hw_ai に移せる部分は **lens の中身を呼ぶ前の「どの lens を使ってどの hw_ai 関数を呼ぶか」の決定**：

```
Idle + needs_recruitment + Scouting中 → handle_scouting_state
Idle + needs_recruitment + 非Scouting → process_recruitment
Idle + 満員              → handle_idle_state
非Idle + Scouting中      → process_squad_management → handle_scouting_state
非Idle + その他          → process_squad_management → process_recruitment → finalize_state_transitions
```

この dispatch テーブルと「結果を束ねる型」を hw_ai に置くことで、root は `match decision_path { ... }` で lens 構築と hw_ai 関数呼び出しだけを担える。

### ギャップ

- hw_ai に dispatch 判定関数がない（`determine_decision_path` 相当が存在しない）
- outcome を束ねる型がない（各 `RecruitmentOutcome` / `ScoutingStateTransition` を root がバラバラに解釈している）
- `FamiliarDecideOutput` への書き込みが system 本体に散在している（専用 helper に集約されていない）
- `FamiliarAiState::Idle` の `emit_idle_visual` 判定が hw_ai 関数の外にある

## 4. 実装方針（高レベル）

### 方針

1. **hw_ai に `determine_decision_path` を追加する**。  
   `FamiliarCommand` / `FamiliarAiState` / `max_workers` / `current_count` だけを取り、`FamiliarDecisionPath` enum を返す pure function。Bevy resource / MessageWriter 不要。

2. **hw_ai に ordered emit plan を追加する**。  
   `DecisionEmitOp` と `FamiliarStateDecisionResult` を導入し、各パスが「どの message 相当操作をどの順番で発行すべきか」を順序付きで返す。  
   root はこれを受け取って MessageWriter に変換する。

3. **root に `emit_state_decision_messages` を追加する**。  
   `FamiliarStateDecisionResult` + 旧状態 → `FamiliarDecideOutput` 書き込みに特化した helper。  
   `determine_transition_reason` 呼び出しもここに集約する。

4. **`familiar_ai_state_system` をリファクタする**。  
   loop 本体が `determine_decision_path` → `match path { ... }` → lens 構築 + hw_ai 呼び出し → `emit_state_decision_messages` の形になる。

### 設計上の前提

- `FamiliarSoulQuery` の lens は引き続き root が担う。hw_ai に `transmute_lens_filtered` は持ち込まない。
- `FamiliarDecideOutput` は root 所有のままとし、hw_ai には渡さない。
- `determine_transition_reason` は root adapter で旧状態・新状態が確定した後に呼ぶ（現状維持）。
- `MessageWriter` の書き込み順（state_request → state_changed_event）を維持する。
- Bevy 0.18: `QueryLens::transmute_lens_filtered` の借用寿命を伸ばしすぎず、1 分岐ごとに作成・消費する（現状と同じパターン）。

## 5. マイルストーン

## M1: `hw_ai` に dispatch core と result 型を追加する

### 変更内容

新規ファイル `crates/hw_ai/src/familiar_ai/decide/state_decision.rs` を作成する。

#### 追加する型と関数

```rust
// ─── dispatch 判定 ───────────────────────────────────────────────────────────

/// 使い魔の状態判断パス（lens 構築とサブ関数の呼び方を決める enum）
pub enum FamiliarDecisionPath {
    /// Idle command + 招募必要 + 既に Scouting 中
    IdleScoutingContinue { target_soul: Entity },
    /// Idle command + 招募必要 + 非 Scouting
    IdleRecruitSearch,
    /// Idle command + 分隊満員（招募不要）
    IdleSquadFull,
    /// 非 Idle command + 既に Scouting 中
    NonIdleScoutingContinue { target_soul: Entity },
    /// 非 Idle command + その他（SearchingTask / Supervising etc.）
    NonIdleRecruitOrTransition,
}

/// Idle command + needs_recruitment + 分隊満員 かどうかを判定する
///
/// # 引数
/// - `command`: ActiveCommand.command
/// - `current_state`: 現在の FamiliarAiState
/// - `max_workers`: FamiliarOperation.max_controlled_soul
/// - `current_squad_count`: Commanding コンポーネントが持つメンバー数
pub fn determine_decision_path(
    command: &FamiliarCommand,
    current_state: &FamiliarAiState,
    max_workers: usize,
    current_squad_count: usize,
) -> FamiliarDecisionPath

// ─── 結果型 ──────────────────────────────────────────────────────────────────

/// root adapter が順番どおりに MessageWriter へ変換する emit 操作
pub enum DecisionEmitOp {
    AddMember(Entity),
    ReleaseMembers(Vec<Entity>),
    EmitIdleVisual,
    EmitStateChange,
}

/// per-familiar state decision の実行結果
///
/// root adapter はこれを受け取り MessageWriter へ変換する。
pub struct FamiliarStateDecisionResult {
    /// path ごとの既存順序を維持する emit 操作列
    pub emit_ops: Vec<DecisionEmitOp>,
}
```

#### `determine_decision_path` の実装概要

```
if FamiliarCommand::Idle:
  needs_recruitment = max_workers > 0 && current_squad_count < max_workers
  if needs_recruitment:
    if current_state == Scouting { target_soul } → IdleScoutingContinue { target_soul }
    else                                          → IdleRecruitSearch
  else:
    → IdleSquadFull
else:
  if current_state == Scouting { target_soul } → NonIdleScoutingContinue { target_soul }
  else                                          → NonIdleRecruitOrTransition
```

#### `FamiliarStateDecisionResult` ビルダー群（任意）

各 path の hw_ai 関数呼び出し結果を `FamiliarStateDecisionResult` に変換する helper（モジュール内 private）:

```rust
fn result_from_recruitment_outcome(outcome: RecruitmentOutcome) -> FamiliarStateDecisionResult
fn result_from_scouting_transition(t: ScoutingStateTransition) -> FamiliarStateDecisionResult
fn result_from_idle_transition(t: StateTransitionResult) -> FamiliarStateDecisionResult
fn result_from_non_idle_scouting(
    released: Vec<Entity>,
    t: ScoutingStateTransition,
) -> FamiliarStateDecisionResult
fn result_from_non_idle_recruit_or_transition(
    released: Vec<Entity>,
    outcome: RecruitmentOutcome,
    finalized_state_changed: bool,
) -> FamiliarStateDecisionResult
```

### 変更ファイル

- **新規** `crates/hw_ai/src/familiar_ai/decide/state_decision.rs`
- `crates/hw_ai/src/familiar_ai/decide/mod.rs` に `pub mod state_decision;` を追加

### 完了条件

- [ ] `FamiliarDecisionPath` と `determine_decision_path` が hw_ai に存在する
- [ ] `DecisionEmitOp` と `FamiliarStateDecisionResult` が hw_ai に存在する
- [ ] hw_ai 側が `MessageWriter` に依存しない
- [ ] `cargo check -p hw_ai` が通る

### 検証

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check -p hw_ai
```

---

## M2: root `state_decision.rs` を thin adapter に縮退する

### 変更内容

#### 追加: `emit_state_decision_messages` helper

```rust
fn emit_state_decision_messages(
    fam_entity: Entity,
    old_state: &FamiliarAiState,
    next_state: &FamiliarAiState,
    result: &FamiliarStateDecisionResult,
    decide_output: &mut FamiliarDecideOutput,
) {
    for op in &result.emit_ops {
        match op {
            DecisionEmitOp::AddMember(soul_entity) => {
                write_add_member_request(&mut decide_output.squad_requests, fam_entity, *soul_entity);
            }
            DecisionEmitOp::ReleaseMembers(released) => {
                write_release_requests(&mut decide_output.squad_requests, fam_entity, released);
            }
            DecisionEmitOp::EmitIdleVisual => {
                decide_output.idle_visual_requests.write(FamiliarIdleVisualRequest { familiar_entity: fam_entity });
            }
            DecisionEmitOp::EmitStateChange => {
                decide_output.state_requests.write(FamiliarStateRequest { familiar_entity: fam_entity, new_state: next_state.clone() });
                decide_output.state_changed_events.write(FamiliarAiStateChangedEvent {
                    familiar_entity: fam_entity,
                    from: old_state.clone(),
                    to: next_state.clone(),
                    reason: determine_transition_reason(old_state, next_state),
                });
            }
        }
    }
}
```

#### リファクタ後の `familiar_ai_state_system` 構造

```rust
pub fn familiar_ai_state_system(params: FamiliarAiStateDecisionParams) {
    let mut recruitment_reservations: HashSet<Entity> = HashSet::new();

    for (...) in q_familiars.iter_mut() {
        let old_state = ai_state.clone();
        let mut next_state = old_state.clone();
        let current_count = commanding.map(|c| c.len()).unwrap_or(0);

        let path = determine_decision_path(
            &active_command.command,
            &old_state,
            familiar_op.max_controlled_soul,
            current_count,
        );

        let result = match path {
            FamiliarDecisionPath::IdleScoutingContinue { target_soul } => {
                // lens 構築 → handle_scouting_state 呼び出し → FamiliarStateDecisionResult
                let mut q_lens = q_souls.transmute_lens_filtered::<SoulScoutingQuery, ...>();
                let q = q_lens.query();
                let mut ctx = FamiliarScoutingContext { ..., ai_state: &mut next_state, ... };
                let t = handle_scouting_state(&mut ctx);
                result_from_scouting(&t)
            }
            FamiliarDecisionPath::IdleRecruitSearch => {
                // lens 構築 → process_recruitment
                let mut q_lens = q_souls.transmute_lens_filtered::<SoulRecruitmentQuery, ...>();
                let q = q_lens.query();
                let mut ctx = FamiliarRecruitmentContext { ..., ai_state: &mut next_state, ... };
                let outcome = process_recruitment(&mut ctx);
                result_from_recruitment_outcome(outcome)
            }
            FamiliarDecisionPath::IdleSquadFull => {
                let t = handle_idle_state(active_command, &next_state, fam_pos, &mut fam_dest, &mut fam_path);
                result_from_idle_transition(t)
            }
            FamiliarDecisionPath::NonIdleScoutingContinue { target_soul } => {
                // squad 管理 → lens 構築 → scouting
                let (squad_release, squad_entities) = run_squad_management(...);
                let mut q_lens = ...;
                let t = handle_scouting_state(...);
                aggregate_non_idle_scouting(squad_release, t)
            }
            FamiliarDecisionPath::NonIdleRecruitOrTransition => {
                // squad 管理 → lens 構築 → recruitment → finalize
                let (squad_release, mut squad_entities) = run_squad_management(...);
                let mut q_lens = ...;
                let outcome = process_recruitment(...);
                let finalized = finalize_state_transitions(...);
                aggregate_non_idle_recruit(squad_release, outcome, finalized)
            }
        };

        emit_state_decision_messages(fam_entity, &old_state, &next_state, &result, &mut decide_output);
    }
}
```

### 変更ファイル

- `src/systems/familiar_ai/decide/state_decision.rs`（メイン変更対象）
- `src/systems/familiar_ai/decide/mod.rs`（`hw_ai::familiar_ai::decide::state_decision` の re-export 追加）

### 完了条件

- [ ] root `state_decision.rs` の `familiar_ai_state_system` が `determine_decision_path` を呼んでいる
- [ ] `FamiliarDecideOutput` への write が `emit_state_decision_messages` だけから行われている
- [ ] inline で `FamiliarStateRequest` を直接構築している箇所が system 本体から除去されている
- [ ] §3 の「メッセージ発行順序（パス別）」表にある順序が `emit_ops` の並びとして維持されている
- [ ] `determine_transition_reason` が `emit_state_decision_messages` 内でのみ呼ばれている

### 検証

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
```

---

## M3: 境界ドキュメントを同期する

### 変更内容

- `docs/familiar_ai.md` に `state_decision` の責務分担（`FamiliarDecisionPath` / `DecisionEmitOp` は hw_ai、MessageWriter は root）を追記する
- `docs/cargo_workspace.md` の Familiar AI 境界説明に `hw_ai::familiar_ai::decide::state_decision` への言及を追加する
- `src/systems/familiar_ai/README.md` の root / `hw_ai` 分担表に `state_decision` の新境界を反映する

### 変更ファイル

- `docs/familiar_ai.md`
- `docs/cargo_workspace.md`
- `src/systems/familiar_ai/README.md`

### 完了条件

- [ ] `state_decision` の所有境界が docs 上で一貫している
- [ ] `determine_decision_path` が pure function である理由が説明されている

### 検証

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace
```

---

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `FamiliarDecisionPath` が細かすぎて root adapter が逆に複雑化する | 責務分離しても見通しが悪い | 5 パスに限定し、各パスは 1 つの hw_ai サブ関数呼び出しに対応するよう設計する |
| `transmute_lens_filtered` の借用範囲が変わり Query 競合を起こす | `cargo check` 失敗 | lens は match アーム内でのみ作成・使用・drop する（現状パターン維持）|
| request 発行順や state reason 計算順が変わる | runtime 挙動差分 | `DecisionEmitOp` を順序付き `Vec` とし、§3 のパス別順序表を golden path として実装コメントで明記する |
| `FamiliarStateDecisionResult` が `Vec<DecisionEmitOp>` を使いアロケーションが増える | フレームレート微小低下 | op 数は高々数件に限定し、`released_entities` は既存 `Vec<Entity>` を再利用して追加コピーを避ける |
| M1 を先に入れると `cargo check` が一時的に壊れる | CI / 開発フロー停止 | M1（hw_ai 追加のみ）は root を触らないので安全。M2 の一括変更でのみ root が壊れ得る |

## 7. 検証計画

- 必須:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check -p hw_ai`
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`
- 手動確認シナリオ:
  - `Idle` command の Familiar が近傍 Soul を即時リクルートできる（`IdleRecruitSearch` path → `ImmediateRecruit`）
  - 遠方 Soul に対して `Scouting` へ遷移し、到達後に AddMember request を発行できる（`IdleRecruitSearch` → `ScoutingStarted` → `IdleScoutingContinue`）
  - `Idle` command かつ分隊十分のとき、使い魔が停止し `Idle` へ遷移または維持する（`IdleSquadFull` → `handle_idle_state`）
  - 疲労解放で `ReleaseMember { reason: Fatigued }` が従来どおり発行される（`NonIdle*` path → squad_release）
- パフォーマンス確認（必要時）:
  - `familiar_ai_state_system` の 1 フレームあたり処理量に明確な増加がないこと

## 8. ロールバック方針

- M1: `crates/hw_ai/src/familiar_ai/decide/state_decision.rs` 削除 + `mod.rs` revert のみ
- M2: root `state_decision.rs` の revert（hw_ai 追加分は残してよい）
- M3: docs revert のみ
- 問題が出た場合は `transmute_lens_filtered` の借用範囲と match アームの drop タイミングを最初に確認する

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`（M1/M2/M3 全完了）
- 完了済みマイルストーン: コードサーベイ・ブラッシュアップ・M1（hw_ai core）・M2（root thin adapter）・M3（docs 同期）
- 未着手/進行中: なし

### 次のAIが最初にやること

1. `crates/hw_ai/src/familiar_ai/decide/mod.rs` に `pub mod state_decision;` を追加し、`state_decision.rs` を新規作成する
2. `FamiliarDecisionPath` enum と `determine_decision_path` 関数を実装する（hw_core の `FamiliarCommand` / `FamiliarAiState` を使う。bevy::prelude::* 以外の追加依存は不要）
3. `DecisionEmitOp` enum と `FamiliarStateDecisionResult { emit_ops: Vec<DecisionEmitOp> }` を実装する
4. `cargo check -p hw_ai` を通してから M2 へ進む

### ブロッカー/注意点

- `FamiliarSoulQuery` は root 型（`Inventory` 依存）なので hw_ai に持ち込めない。`transmute_lens_filtered` は root 側に残す。
- `determine_decision_path` は `FamiliarCommand` と `FamiliarAiState`（どちらも `hw_core` 型）のみを使うので hw_ai に置ける。
- M2 で `emit_state_decision_messages` を実装する際、**パスごとに発行順が異なる**（§3 の「メッセージ発行順序（パス別）」表を参照）。Idle パスでは `add` のみ、非 Idle パスでは `release` → `add` の順。
- `transmute_lens_filtered` の正確なタプル型は §3 の「`transmute_lens_filtered` 正確なタプル型」を参照すること（型ズレは即コンパイルエラーになる）。
- `SoulRecruitmentQuery` / `SoulScoutingQuery` / `SoulSquadQuery` は `hw_ai::familiar_ai::decide::query_types` で定義されており、root の `src/systems/familiar_ai/helpers/query_types.rs` が `pub use hw_ai::familiar_ai::decide::query_types::...` で再 export している。M2 の import では root 側の `query_types` モジュールから参照するか直接 `hw_ai::...` を使うかを統一すること。
- `task_delegation` は今回の計画では触れない。`familiar_processor.rs` も同様。

### 参照必須ファイル

- `src/systems/familiar_ai/decide/state_decision.rs`（リファクタ対象・382行）
- `crates/hw_ai/src/familiar_ai/decide/recruitment.rs`（`RecruitmentOutcome` 確認）
- `crates/hw_ai/src/familiar_ai/decide/state_handlers/scouting.rs`（`ScoutingStateTransition` 確認）
- `crates/hw_ai/src/familiar_ai/decide/state_handlers/idle.rs`（`StateTransitionResult` 確認）
- `crates/hw_ai/src/familiar_ai/decide/helpers.rs`（`SquadManagementOutcome` 確認）
- `src/systems/familiar_ai/helpers/query_types.rs`（`FamiliarSoulQuery` / `FamiliarStateQuery` 確認）
- `src/systems/familiar_ai/README.md`（境界 docs の同期対象）

### 最終確認ログ

- 最終 `cargo check`: `2026-03-12 / not run (planning only)`
- 未解決エラー: なし（計画作成のみ）

### Definition of Done

- [ ] M1-M3 が完了している
- [ ] `familiar_ai_state_system` の本体が `determine_decision_path` + `match` + `emit_state_decision_messages` の構造になっている
- [ ] `state_decision` の core と adapter の責務がコードと docs の両方で一致している
- [ ] `cargo check -p hw_ai` と `cargo check --workspace` が成功している

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-12` | `AI (Codex)` | 初版作成 |
| `2026-03-12` | `AI (GitHub Copilot)` | コードサーベイに基づきブラッシュアップ。型名・関数名・行番号を具体化、lens 制約の説明追加、`FamiliarDecisionPath` 5パス設計を明記 |
| `2026-03-12` | `AI (GitHub Copilot)` | 再ブラッシュアップ。`transmute_lens_filtered` の正確なタプル型、パス別メッセージ発行順序表、`SoulQuery` の hw_ai→root re-export 構造を追記 |
| `2026-03-12` | `AI (Codex)` | レビュー指摘を反映。ordered emit op 設計へ修正し、Idle 検証シナリオと docs 更新対象を是正 |
