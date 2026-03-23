# 計画書：Soul 経路探索ローテーション（P1-b）

## 問題

`pathfinding_system` はフレームごとに全 Soul を `query.iter_mut()` の反復順で走査する。  
`MAX_PATHFINDS_PER_FRAME = 8` の予算を超えると `process_worker_pathfinding` が早期 return するが、
**毎フレーム同じ Soul が先頭に来る**ため、後続 Soul は経路更新スロットをほぼ得られない。

### 現在のループ構造（変更前）

```rust
// pathfinding_system（system.rs L217〜）
for prioritize_tasks in [true, false] {
    let budget = phase_budget_limit(prioritize_tasks);  // task: 6, idle: 8
    if pathfind_count >= budget { continue; }

    for (entity, transform, ...) in query.iter_mut() {
        // フェーズ違いはスキップ（クールダウンも含めて）
        if has_task != prioritize_tasks { continue; }
        // クールダウンカウントダウン（全 Soul）
        if let Some(cooldown) = ... { cooldown.remaining_frames -= 1; continue; }
        // A* 実行（budget を超えたら内部で早期 return）
        process_worker_pathfinding(..., &mut pathfind_count, budget);
    }
}
```

**問題の核心**: Bevy 0.18 の `Query` は反復順を API として保証しないが、現状は同じ Soul 群が継続して先頭側に来やすく、先頭の Soul が毎フレーム A* 予算を独占している。

---

## 解決方針

task / idle それぞれに独立した開始インデックスを持つ `Resource` を追加し、  
`query.iter()` で各 phase のエンティティ一覧を収集→phase ごとにローテーション→`query.get_mut` でアクセスすることで、
**各 phase の予算枠の中で round-robin に近い公平な走査**にする。

アロケーション削減のため Entity バッファは `Local<Vec<Entity>>` で再利用する。

---

## 実装詳細

### Step 1 — `SoulPathfindingRotationOffsets` Resource の追加

**ファイル**: `crates/hw_soul_ai/src/soul_ai/pathfinding/mod.rs`

```rust
/// Soul 経路探索の phase 別走査開始オフセット。
/// task / idle で独立して進めることで、各予算枠の公平性を保つ。
#[derive(Resource, Default)]
pub struct SoulPathfindingRotationOffsets {
    pub task: usize,
    pub idle: usize,
}
```

`pub use` にも追加して `pathfinding_system` 側から参照できるようにする。

### Step 2 — `SoulAiCorePlugin` での Resource 登録

**ファイル**: `crates/hw_soul_ai/src/soul_ai/mod.rs`

```rust
// 既存の init_resource 群に 1 行追加
app.init_resource::<pathfinding::SoulPathfindingRotationOffsets>()
```

### Step 3 — `pathfinding_system` のシグネチャ変更

**ファイル**: `crates/hw_soul_ai/src/soul_ai/pathfinding/system.rs`

```rust
pub fn pathfinding_system(
    mut commands: Commands,
    world_map: WorldMapRead,
    mut pf_context: Local<PathfindingContext>,
    mut query: Query<(...)>,
    q_rest_areas: Query<&Transform, With<hw_jobs::RestArea>>,
    mut queries: crate::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    // 追加 ↓
    mut rotation_offsets: ResMut<super::SoulPathfindingRotationOffsets>,
    mut task_entities: Local<Vec<Entity>>,
    mut idle_entities: Local<Vec<Entity>>,
) {
```

### Step 4 — ループ本体の書き換え

フェーズループの直前で phase 別エンティティ一覧を収集し、  
各 phase の内側ループを `phase_buf` + `query.get_mut` に置き換える。

```rust
// --- phase 別エンティティ収集 ---
task_entities.clear();
idle_entities.clear();

for (entity, _, _, _, task, idle, resting_in, _, _, _) in query.iter() {
    let has_task = !matches!(*task, AssignedTask::None);
    if has_task {
        task_entities.push(entity);
        continue;
    }

    let idle_can_move = match idle.behavior {
        IdleBehavior::Sitting | IdleBehavior::Sleeping => false,
        IdleBehavior::Resting => resting_in.is_none(),
        IdleBehavior::GoingToRest => true,
        _ => true,
    };

    if idle_can_move {
        idle_entities.push(entity);
    }
}

let mut pathfind_count = 0usize;

for prioritize_tasks in [true, false] {
    let budget = phase_budget_limit(prioritize_tasks);
    if pathfind_count >= budget {
        continue;
    }

    let (phase_entities, phase_offset) = if prioritize_tasks {
        (&mut *task_entities, &mut rotation_offsets.task)
    } else {
        (&mut *idle_entities, &mut rotation_offsets.idle)
    };

    if !phase_entities.is_empty() {
        let offset = *phase_offset % phase_entities.len();
        phase_entities.rotate_left(offset);
    }

    for &soul_entity in phase_entities.iter() {
        let Ok((
            entity,
            transform,
            mut destination,
            mut path,
            mut task,
            mut idle,
            resting_in,
            rest_reserved_for,
            mut cooldown_opt,
            mut inventory_opt,
        )) = query.get_mut(soul_entity) else {
            continue;
        };

        // 以降は現行コードとほぼ同一
        // task_entities / idle_entities は事前に phase 別収集済みなので
        // has_task != prioritize_tasks 判定は不要。
        // ... クールダウン処理 ...
        // ... process_worker_pathfinding(...) ...
    }

    *phase_offset = phase_offset.wrapping_add(1);
}
```

> **phase 別バッファにする理由**  
> 1 本の `entity_buf` を task / idle で共有すると、phase 先頭に不一致エンティティが並んだフレームで
> wrap 後の同じ task 群だけが毎回予算を取りやすくなり、phase 内公平性が崩れる。  
> そのため、task / idle は別バッファ・別 offset にする。

> **クールダウン処理の正確性について**  
> クールダウンは現行でも `has_task == prioritize_tasks` かつ `idle_can_move == true` の Soul に対してのみ消費される。  
> phase 別バッファはこの条件を事前に反映するだけなので、カウントダウン対象は変わらない。

---

## 変更対象ファイル

| ファイル | 変更内容 |
|---|---|
| `crates/hw_soul_ai/src/soul_ai/pathfinding/mod.rs` | `SoulPathfindingRotationOffsets` Resource 定義・pub use 追加 |
| `crates/hw_soul_ai/src/soul_ai/mod.rs` | `init_resource::<SoulPathfindingRotationOffsets>()` 登録 |
| `crates/hw_soul_ai/src/soul_ai/pathfinding/system.rs` | phase 別バッファ収集・phase 別ローテーション・システム引数追加 |

変更なし（影響を与えないことを確認）:

| ファイル | 確認内容 |
|---|---|
| `crates/hw_soul_ai/src/soul_ai/pathfinding/reuse.rs` | `try_reuse_existing_path` は `entity` 単位処理のため影響なし |
| `crates/hw_soul_ai/src/soul_ai/pathfinding/fallback.rs` | 同上 |
| `crates/hw_core/src/constants/ai.rs` | `MAX_PATHFINDS_PER_FRAME` の値は変えない |

---

## リスクと対策

| リスク | 評価 | 対策 |
|---|---|---|
| `query.iter()` + `query.get_mut` の 2 パスによるオーバーヘッド | 小：Entity 収集は軽量な read-only イテレーション | `Local<Vec<Entity>>` で Vec 再利用しアロケーション 0 に抑える |
| `wrapping_add` によるオフセット巻き戻し | 無害：`% phase_entities.len()` で正規化されるため問題なし | — |
| task / idle バッファを毎フレーム組み立てるコスト | 小：`Entity` の push のみで軽量 | `Local<Vec<Entity>>` を再利用し、phase ごとに必要な候補だけを格納する |
| phase ごとの公平性が崩れる | 高：単一 offset だと phase 先頭の不一致エンティティを毎回飛ばした後、同じ先頭群が予算を取り続ける | 初期実装から task / idle を別 offset・別バッファに分離する |
| クールダウン処理の取りこぼし | なし：phase 対象 Soul を全て走査するため現行と同一 | — |
| `query.get_mut` パニック | なし：`let Ok(...) = ... else { continue; }` で安全に処理 | — |

---

## 期待される効果

- task Soul 数 `Nt`・task 予算 6、idle Soul 数 `Ni`・idle 予算 2 のとき、各 phase 内で概ね round-robin に A* スロットを取得できる。
- 大量 Soul スポーン（`--spawn-souls 500`）時に後方 Soul の経路更新遅延が均一化される。
- ゲームプレイへの副作用: 各 Soul の出発タイミングが微妙にずれるが、これは公平性向上の副産物であり問題なし。

---

## 検証方法

1. **コンパイル確認**  
   `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`

2. **高負荷シナリオ目視確認**  
   `cargo run -- --spawn-souls 500 --spawn-familiars 30 --perf-scenario --perf-log-fps`  
   → 全 Soul が徐々に経路を取得して動き出すことを確認（先頭 Soul だけ先行して後続が止まり続けないこと）

3. **通路ボトルネックシナリオ**  
   マップ上に建築物で通路を絞り多数 Soul が目的地へ向かう状況を作り、後続 Soul にも数フレーム以内に経路が割り当てられることを確認

4. **クールダウン正常動作確認**  
   到達不能タスクを与えた Soul に `PathCooldown`（`PATHFINDING_RETRY_COOLDOWN_FRAMES = 10`）が正しく付与・カウントダウンされることを確認

---

## 完了条件

- [ ] `cargo check --workspace` がエラーなし
- [ ] 高負荷シナリオで後続 Soul が経路を得られることを目視確認
- [ ] クールダウン処理が現行と同一挙動であることを確認

---

## 出典・関連

- 出典: `docs/proposals/high-priority-performance-proposal-2026-03-23.md` §5.1 施策 (b)
- P1-a（早期 break）・P2・P3 の見送り理由は提案書参照
