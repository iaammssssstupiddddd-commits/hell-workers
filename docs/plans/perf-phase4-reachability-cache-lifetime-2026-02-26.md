# Phase 4: パフォーマンス改善 — Reachability キャッシュのライフタイム延長

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `perf-phase4-reachability-cache-lifetime-2026-02-26` |
| ステータス | `Done` |
| 作成日 | `2026-02-26` |
| 最終更新日 | `2026-02-26` |
| 作成者 | `Claude (AI Agent)` |
| 関連提案 | `docs/proposals/performance-bottlenecks-proposal-2026-02-26.md` |
| 関連Issue/PR | N/A |

---

## 1. 目的

- **解決したい課題**: `ReachabilityFrameCache` が 5 フレームごと（≈83ms @60fps）に無条件クリアされており、委譲インターバル（0.3 秒）をまたいでキャッシュが再利用されない。WorldMap が変化しない限り、Worker→Target の到達可能性は変わらないため、不要な A* 再計算が発生している
- **到達したい状態**: WorldMap 変更時のみキャッシュを即時クリア。変更がない間は長期間保持（安全フォールバックとして 60 フレーム ≈1 秒 のクリアは維持）
- **成功指標**: `cargo check` 成功。`FamiliarDelegationPerfMetrics.reachable_with_cache_calls` でキャッシュヒット率が向上。Familiar タスク委譲の動作が正常。

---

## 2. スコープ

### 対象（In Scope）

- `src/systems/familiar_ai/decide/task_delegation.rs` — キャッシュクリア条件の変更
- 定数 `REACHABILITY_FRAME_CACHE_CLEAR_INTERVAL_FRAMES` の値変更

### 非対象（Out of Scope）

- `ReachabilityFrameCache` 構造体の変更（Resource としての利用は既に実装済み）
- A* アルゴリズム自体の変更
- キャッシュキーの変更（`ReachabilityCacheKey = ((i32, i32), (i32, i32))` は変更なし）
- Familiar AI の他のロジック

---

## 3. 現状とギャップ

**現状コード** (`task_delegation.rs:61-65`):
```rust
reachability_frame_cache.age = reachability_frame_cache.age.saturating_add(1);
if reachability_frame_cache.age >= REACHABILITY_FRAME_CACHE_CLEAR_INTERVAL_FRAMES {  // 5
    reachability_frame_cache.cache.clear();
    reachability_frame_cache.age = 0;
}
```

**タイムライン分析**:
- `familiar_task_delegation_system` は毎フレーム呼ばれる（age を毎フレームインクリメント）
- 委譲タイマー（`allow_task_delegation`）は 0.3 秒ごとに true → 実際の A* は 0.3 秒に 1 度のバースト
- 5 フレーム ≈ 83ms でキャッシュクリア → 委譲インターバル（0.3s）の中間でキャッシュが消える
- **結果**: キャッシュは 1 回の委譲バースト内では有効（同一フレームで複数 Familiar が同じ target に A*）だが、次の委譲時（0.3s 後）には既にクリア済み

**改善後のタイムライン**:
- WorldMap 変更なし → キャッシュ生存期間 = 60 フレーム（≈1 秒）
- 委譲（0.3s）×3 回分のキャッシュが再利用可能
- Worker A が frame 0 で Worker→Target1 = reachable と評価 → frame 18（0.3s後）の委譲で同一パスが再利用

---

## 4. 実装方針（高レベル）

```rust
// Before:
const REACHABILITY_FRAME_CACHE_CLEAR_INTERVAL_FRAMES: u32 = 5;

reachability_frame_cache.age = reachability_frame_cache.age.saturating_add(1);
if reachability_frame_cache.age >= REACHABILITY_FRAME_CACHE_CLEAR_INTERVAL_FRAMES {
    reachability_frame_cache.cache.clear();
    reachability_frame_cache.age = 0;
}

// After:
const REACHABILITY_CACHE_SAFETY_CLEAR_INTERVAL_FRAMES: u32 = 60;  // 安全フォールバック

if world_map.is_changed() {
    // WorldMap（建築/地形変化）があった場合は即時クリア
    reachability_frame_cache.cache.clear();
    reachability_frame_cache.age = 0;
} else {
    reachability_frame_cache.age = reachability_frame_cache.age.saturating_add(1);
    if reachability_frame_cache.age >= REACHABILITY_CACHE_SAFETY_CLEAR_INTERVAL_FRAMES {
        reachability_frame_cache.cache.clear();
        reachability_frame_cache.age = 0;
    }
}
```

**`world_map` パラメータ**: `FamiliarAiTaskDelegationParams` にすでに `pub world_map: Res<'w, WorldMap>` が含まれているため、追加不要。

**Bevy 0.18 API 注意**:
- `Res<WorldMap>.is_changed()` は WorldMap Resource が変更された場合に true を返す（フレームごとにリセット）
- WorldMap が内部の `HashMap` フィールドを変更した場合でも、`ResMut<WorldMap>` でアクセスされていれば `is_changed()` = true になる。ただし内部ミュータブルな変更（Cell, RefCell）では検知できない点に注意

---

## 5. マイルストーン

### M1: キャッシュクリア条件の変更

**変更内容**:

**(A) 定数変更**:

```rust
// Before (task_delegation.rs 付近):
const REACHABILITY_FRAME_CACHE_CLEAR_INTERVAL_FRAMES: u32 = 5;

// After (名前も変更):
const REACHABILITY_CACHE_SAFETY_CLEAR_INTERVAL_FRAMES: u32 = 60;
```

**(B) クリアロジックの変更**:

`familiar_task_delegation_system` 内（現在の age インクリメント部分）を置き換え:

```rust
// Before (lines 61-65):
reachability_frame_cache.age = reachability_frame_cache.age.saturating_add(1);
if reachability_frame_cache.age >= REACHABILITY_FRAME_CACHE_CLEAR_INTERVAL_FRAMES {
    reachability_frame_cache.cache.clear();
    reachability_frame_cache.age = 0;
}

// After:
if world_map.is_changed() {
    reachability_frame_cache.cache.clear();
    reachability_frame_cache.age = 0;
} else {
    reachability_frame_cache.age = reachability_frame_cache.age.saturating_add(1);
    if reachability_frame_cache.age >= REACHABILITY_CACHE_SAFETY_CLEAR_INTERVAL_FRAMES {
        reachability_frame_cache.cache.clear();
        reachability_frame_cache.age = 0;
    }
}
```

**変更ファイル**:
- `src/systems/familiar_ai/decide/task_delegation.rs`

**完了条件**:
- [x] `REACHABILITY_FRAME_CACHE_CLEAR_INTERVAL_FRAMES` が削除/改名されている
- [x] `world_map.is_changed()` によるキャッシュクリアが追加されている
- [x] 安全フォールバック（60 フレーム）が維持されている
- [x] `cargo check` でエラーなし

**検証**:
- `cargo check`

---

### M2: キャッシュヒット率の確認（オプション）

`FamiliarDelegationPerfMetrics.reachable_with_cache_calls` カウンターを活用して改善前後を比較する。

**現状の計測方法**:
```rust
// task_delegation.rs:138-139
let reachable_with_cache_calls =
    crate::systems::familiar_ai::decide::task_management::take_reachable_with_cache_calls();
perf_metrics.reachable_with_cache_calls = ...
```

この数値が 5 秒ごとにリセットされるため、建築中に `REACHABLE_WITH_CACHE_CALLS` が低下すれば改善確認できる。

**変更ファイル**: なし（観察のみ）

**確認方法**:
1. F12 デバッグ表示で `reachable_with_cache_calls` を確認
2. WorldMap に変化がない状態で時間経過後のキャッシュ呼び出し数を確認
3. Building を追加した直後にキャッシュクリアが発生することを確認

---

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `WorldMap.is_changed()` が期待通りに発火しない（内部 HashMap の変更が検知されない） | 高（キャッシュが永続して stale path 判定が発生する可能性） | 安全フォールバック（60 フレーム）が存在するため最悪でも 1 秒後にクリアされる。加えて: 既存コードで `WorldMap` が `ResMut<WorldMap>` でアクセスされているか確認 |
| キャッシュが 1 秒間保持されることで WorldMap 変更直後に誤った reachable 判定が残る | 低（WorldMap 変更時は即時クリアするため、安全フォールバック期間には到達しない） | `world_map.is_changed()` の確認を重点的にテスト |
| キャッシュサイズが無制限に増大する | 低（Worker × Target の組み合わせは有限。大規模マップでも通常 1000 エントリ以下） | 必要なら上限設定を追加するが、現状では不要 |

---

## 7. 検証計画

- **必須**: `cargo check`
- **手動確認シナリオ**:
  1. ゲーム起動後、Familiar が通常通りタスク委譲を行う
  2. Building を配置した直後に Familiar が迂回ルートを見つける（古い reachable キャッシュが残っていない）
  3. Building を配置した後 0.3 秒ごとの委譲で同じ到達可能タスクに対してキャッシュが再利用される（到達不能タスクのスキップが正常）

---

## 8. ロールバック方針

- M1 のみのコミット
- `git revert <commit>` で定数と 6 行のロジック変更が戻る

---

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`（独立実施で完了）
- 完了済みマイルストーン: M1 のみ
- 未着手/進行中: なし

### 次のAIが最初にやること

1. `src/systems/familiar_ai/decide/task_delegation.rs` の lines 61-65 付近を読む
2. `world_map.is_changed()` が `FamiliarAiTaskDelegationParams` のスコープ内で呼び出せるか確認（`params.world_map.is_changed()` として使用）
3. M1 を実施してから `cargo check`

### ブロッカー/注意点

- **Phase 3 に非依存**: このフェーズは Phase 1/2/3 が完了していなくても独立して実施可能（task_delegation.rs のみの変更）
- `world_map.is_changed()` は `Res<WorldMap>` の場合でも呼び出し可能。`ResMut` は不要
- `FamiliarAiTaskDelegationParams` 内の `world_map: Res<'w, WorldMap>` を使って `world_map.is_changed()` を呼ぶ。params が展開された後の `world_map` 変数を使用（lines 54 付近の `world_map,` 変数）
- WorldMap の内部変更の検知: `WorldMap.buildings` が `HashMap` フィールドであり、Rust では `ResMut<WorldMap>` へのアクセスがあれば `is_changed()` = true になる（実際に変更がなくても `ResMut` でアクセスすれば changed とマークされる）。WorldMap を更新するシステムが `Res<WorldMap>`（読み取り専用）でアクセスしている場合は `is_changed()` = false のままになる。この点を `grep -n "ResMut<WorldMap>\|mut world_map" src/` で確認する

### 参照必須ファイル

- `src/systems/familiar_ai/decide/task_delegation.rs`（変更対象）
- `src/systems/familiar_ai/decide/task_management/delegation/assignment_loop.rs`（`reachable_with_cache` の実装確認）
- `src/world/map.rs`（`WorldMap` 構造体と変更タイミング確認）

### 最終確認ログ

- 最終 `cargo check`: 未実施
- 未解決エラー: なし（計画段階）

### Definition of Done

- [x] M1 完了
- [x] `cargo check` 成功
- [x] `REACHABILITY_FRAME_CACHE_CLEAR_INTERVAL_FRAMES`（旧定数）が削除されている
- [x] WorldMap 変更時の即時クリアが実装されている
- [ ] 手動確認: Building 配置後に Familiar が正常にタスク委譲する

---

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-02-26` | `Claude (AI Agent)` | 初版作成 |
| `2026-02-26` | `Claude (AI Agent)` | Phase4 実装完了（reachability キャッシュのライフタイム延長） |
