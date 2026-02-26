# Dream Per-Soul Storage 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `dream-per-soul-storage-plan-2026-02-26` |
| ステータス | `Draft` |
| 作成日 | `2026-02-26` |
| 最終更新日 | `2026-02-26` |
| 作成者 | `AI (Claude)` |
| 関連提案 | `docs/proposals/dream_per_soul_storage.md` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: dreamがグローバルプール直接加算でsoul個別の管理軸がない
- 到達したい状態: soulごとにdreamが蓄積し、睡眠/休憩で放出→DreamPool変換、dream=0では眠れない
- 成功指標: `cargo check` 成功、dream蓄積→放出サイクルが動作、dream=0で睡眠/休憩ブロック

## 2. スコープ

### 対象（In Scope）

- `DamnedSoul`にdreamフィールド追加
- dream蓄積ロジック（行動状態別レート）
- dream放出ロジック（睡眠/休憩→DreamPool変換）
- dream=0時の睡眠/休憩禁止＋強制起床
- dreamによるストレス蓄積乗算
- 休憩所のDreamPool直接加算の廃止

### 非対象（Out of Scope）

- UIへのdreamバー表示
- DreamPool消費先の変更
- DreamQualityの役割変更
- ビジュアルエフェクトの大幅変更

## 3. 現状とギャップ

### 現状

- `DamnedSoul` は laziness/motivation/fatigue/stress の4バイタル
- `dream_update_system`: 睡眠中soulのDreamQualityに応じたレートでDreamPoolに直接加算
- `rest_area_update_system`: `occupant_count × REST_AREA_DREAM_RATE × dt` をDreamPoolに直接加算
- `vitals_influence.rs:75-85`: ストレスはタスク/近接/集会/アイドルの状態で増減（dreamの影響なし）
- `transitions.rs:46-71`: `select_next_behavior()` はlazinessのみでSleepingを判定（dreamガードなし）
- `transitions.rs:9-17`: `random_gathering_behavior()` は4択均等ランダム（dreamガードなし）
- `idle_behavior/mod.rs:177-180`: `wants_rest_area` はlaziness/fatigue/stress/idle_time条件（dreamガードなし）

### 本計画で埋めるギャップ

- soul個別のdream貯蔵量の導入
- グローバルプール直接加算 → soul蓄積＋放出変換への移行
- dream=0での睡眠/休憩抑止
- dreamのストレスへの乗算影響

## 4. 実装方針（高レベル）

- 方針: 既存のバイタル管理パターン（fatigue/stress）に合わせ、dreamを`DamnedSoul`のフィールドとして追加。蓄積はUpdate phase、行動ガードはDecide phaseで処理。
- 設計上の前提:
  - dreamは0.0–100.0の範囲（他バイタルの0.0–1.0とは異なるスケール）
  - DreamQualityは放出レートに影響せず、ビジュアル専用として維持
  - 放出はsleeping/restingの両方で同じメカニズム（毎フレームdrain）
- Bevy 0.18 APIでの注意点:
  - `IdleDecisionSoulQuery`は`&DamnedSoul`（読み取り専用）でdream値にアクセス可能
  - `dream_update_system`のQueryに`AssignedTask`を追加して行動判定可能に

## 5. マイルストーン

## M1: データモデル＋定数追加

`DamnedSoul`にdreamフィールドを追加し、新定数を定義する。

- 変更内容:
  - `DamnedSoul`に`dream: f32`追加（default: 0.0）
  - dream関連の新定数を追加
- 変更ファイル:
  - `src/entities/damned_soul/mod.rs` — dreamフィールド追加
  - `src/constants/dream.rs` — 蓄積/放出/ストレス乗算定数追加
- 詳細:

**`src/entities/damned_soul/mod.rs`** (L75-90):
```rust
pub struct DamnedSoul {
    pub laziness: f32,
    pub motivation: f32,
    pub fatigue: f32,
    pub stress: f32,
    pub dream: f32,     // 追加: 夢の貯蔵量 (0.0-100.0)
}

impl Default for DamnedSoul {
    fn default() -> Self {
        Self {
            laziness: 0.7,
            motivation: 0.1,
            fatigue: 0.0,
            stress: 0.0,
            dream: 0.0,        // 追加
        }
    }
}
```

**`src/constants/dream.rs`** に追加:
```rust
// Dream per-soul storage
pub const DREAM_MAX: f32 = 100.0;
pub const DREAM_ACCUMULATE_RATE_WORKING: f32 = 0.5;   // 労働中 (要調整)
pub const DREAM_ACCUMULATE_RATE_IDLE: f32 = 0.1;      // アイドル中 (要調整)
pub const DREAM_ACCUMULATE_RATE_GATHERING: f32 = 0.3;  // 集会中 (要調整)
pub const DREAM_ACCUMULATE_RATE_ESCAPING: f32 = 0.5;   // 逃走中 (要調整)
pub const DREAM_DRAIN_RATE: f32 = 1.0;                 // 睡眠/休憩中の放出レート (要調整)
pub const DREAM_STRESS_MULTIPLIER: f32 = 0.005;        // ストレス乗算係数 (要調整)
```

- 完了条件:
  - [ ] `cargo check` 成功
  - [ ] 未使用警告はM2以降で解消されるため一時的に許容
- 検証:
  - `cargo check`

## M2: dream蓄積ロジック（Update phase）

起きている間にdreamが行動状態に応じて蓄積する。

- 変更内容:
  - `dream_update_system`を改修し、非睡眠・非休憩soulのdream蓄積を追加
  - 蓄積レートは `AssignedTask`/`IdleState` で判定
- 変更ファイル:
  - `src/systems/soul_ai/update/dream_update.rs` — 蓄積ロジック追加
- 詳細:

**`dream_update_system`** のQueryに`AssignedTask`と`RestingIn`を追加:
```rust
pub fn dream_update_system(
    time: Res<Time>,
    mut dream_pool: ResMut<DreamPool>,
    mut q_souls: Query<(
        &mut DamnedSoul,          // &→&mut (dream書き込み用)
        &IdleState,
        &mut DreamState,
        &AssignedTask,             // 追加
        Option<&ParticipatingIn>,
        Option<&RestingIn>,        // 追加
    )>,
) {
```

蓄積ロジック（非睡眠・非休憩soul）:
```rust
// 1) 睡眠中でも休憩中でもないsoulのdream蓄積
if !is_sleeping && !is_resting {
    let rate = if has_task {
        DREAM_ACCUMULATE_RATE_WORKING
    } else {
        match idle.behavior {
            IdleBehavior::Gathering | IdleBehavior::ExhaustedGathering => DREAM_ACCUMULATE_RATE_GATHERING,
            IdleBehavior::Escaping => DREAM_ACCUMULATE_RATE_ESCAPING,
            _ => DREAM_ACCUMULATE_RATE_IDLE,
        }
    };
    soul.dream = (soul.dream + rate * dt).min(DREAM_MAX);
    // DreamQuality判定: Awakeにリセット
    if dream.quality != DreamQuality::Awake {
        dream.quality = DreamQuality::Awake;
    }
    continue;
}
```

- 完了条件:
  - [ ] `cargo check` 成功
  - [ ] 労働中/アイドル中/集会中にsoul.dreamが増加すること
- 検証:
  - `cargo check`

## M3: dream放出ロジック（睡眠時）

睡眠中soulのdreamをDreamPoolへ変換する。旧ロジック（DreamQualityレートでの直接加算）を廃止。

- 変更内容:
  - 睡眠中: `soul.dream`から毎フレームdrainし`DreamPool`に加算
  - 旧: `dream_pool.points += rate * dt`（DreamQuality依存）
  - 新: `drain = min(soul.dream, DREAM_DRAIN_RATE * dt); soul.dream -= drain; dream_pool.points += drain`
  - DreamQuality判定はビジュアル用に維持（放出レートに影響しない）
- 変更ファイル:
  - `src/systems/soul_ai/update/dream_update.rs` — 放出ロジック置き換え
- 詳細:

```rust
// 2) 睡眠中のdream放出
// DreamQuality判定（ビジュアル用、放出レートには影響しない）
if dream.quality == DreamQuality::Awake {
    dream.quality = determine_dream_quality(soul, participating_in.is_some());
}

// 一律レートで放出
let drain = (DREAM_DRAIN_RATE * dt).min(soul.dream);
if drain > 0.0 {
    soul.dream -= drain;
    dream_pool.points += drain;
}
```

- 完了条件:
  - [ ] `cargo check` 成功
  - [ ] 睡眠中にsoul.dreamが減少しDreamPoolが増加すること
  - [ ] DreamQualityがビジュアルに正しく反映されること
- 検証:
  - `cargo check`

## M4: 休憩所の放出統一

休憩所のDreamPool直接加算を廃止し、soul個別のdream放出に統一する。

- 変更内容:
  - `rest_area_update_system` から `dream_pool.points += occupant_count * REST_AREA_DREAM_RATE * dt` を削除
  - 代わりに各resting soulのdreamをdrainしてDreamPoolに加算
  - dream=0到達時に `LeaveRestArea` を発行
- 変更ファイル:
  - `src/systems/soul_ai/update/rest_area_update.rs` — DreamPool直接加算削除、per-soul drain追加
- 詳細:

**削除** (現行L26-33):
```rust
// 削除: 固定レートのDreamPool加算
// for (rest_area, occupants_opt) in q_rest_areas.iter() { ... }
```

**追加** resting soul loopの中:
```rust
for (entity, mut soul, mut idle) in q_resting_souls.iter_mut() {
    if idle.behavior != IdleBehavior::Resting { continue; }
    // バイタル回復（既存）
    soul.fatigue = (soul.fatigue - dt * REST_AREA_FATIGUE_RECOVERY_RATE).max(0.0);
    soul.stress = (soul.stress - dt * REST_AREA_STRESS_RECOVERY_RATE).max(0.0);

    // dream放出（新規）
    let drain = (DREAM_DRAIN_RATE * dt).min(soul.dream);
    if drain > 0.0 {
        soul.dream -= drain;
        dream_pool.points += drain;
    }

    // 時間経過による退出（既存）
    idle.idle_timer += dt;
    if idle.idle_timer >= REST_AREA_RESTING_DURATION {
        request_writer.write(IdleBehaviorRequest {
            entity,
            operation: IdleBehaviorOperation::LeaveRestArea,
        });
    }
    // dream=0による退出（新規）
    else if soul.dream <= 0.0 {
        request_writer.write(IdleBehaviorRequest {
            entity,
            operation: IdleBehaviorOperation::LeaveRestArea,
        });
    }
}
```

Queryの変更: `&DamnedSoul` → `&mut DamnedSoul` (dream書き込み用)

注意: `REST_AREA_DREAM_RATE` 定数は削除または休憩所放出ボーナス用途で残す。初期実装では統一レート（`DREAM_DRAIN_RATE`）を使用。

- 完了条件:
  - [ ] `cargo check` 成功
  - [ ] 休憩所でsoul.dreamが減少しDreamPoolが増加すること
  - [ ] 休憩所でdream=0到達時に退出すること
  - [ ] 休憩所の既存バイタル回復（fatigue/stress）は維持
- 検証:
  - `cargo check`

## M5: ストレス乗算

dreamの蓄積量がストレス蓄積レートに乗算ペナルティを与える。

- 変更内容:
  - `familiar_influence_unified_system`内のストレス蓄積箇所にdream乗算を適用
- 変更ファイル:
  - `src/systems/soul_ai/update/vitals_influence.rs` — L75-85
- 詳細:

**現行** (L75-85):
```rust
if has_task {
    soul.stress = (soul.stress + dt * STRESS_WORK_RATE).min(1.0);
} else if under_command.is_some() {
    // 待機中（使役下）ではストレス変化なし
} else if is_influence_close {
    soul.stress = (soul.stress + dt * ESCAPE_PROXIMITY_STRESS_RATE).min(1.0);
} else if is_gathering {
    soul.stress = (soul.stress - dt * STRESS_RECOVERY_RATE_GATHERING).max(0.0);
} else {
    soul.stress = (soul.stress - dt * STRESS_RECOVERY_RATE_IDLE).max(0.0);
}
```

**変更後**:
```rust
let dream_stress_factor = 1.0 + soul.dream * DREAM_STRESS_MULTIPLIER;
if has_task {
    soul.stress = (soul.stress + dt * STRESS_WORK_RATE * dream_stress_factor).min(1.0);
} else if under_command.is_some() {
    // 待機中（使役下）ではストレス変化なし
} else if is_influence_close {
    soul.stress = (soul.stress + dt * ESCAPE_PROXIMITY_STRESS_RATE * dream_stress_factor).min(1.0);
} else if is_gathering {
    soul.stress = (soul.stress - dt * STRESS_RECOVERY_RATE_GATHERING).max(0.0);
} else {
    soul.stress = (soul.stress - dt * STRESS_RECOVERY_RATE_IDLE).max(0.0);
}
```

注意: ストレス回復（gathering/idle）には乗算を適用しない（提案仕様: 蓄積レートのみ）

L87-89の監視ストレスにも適用:
```rust
if has_task && best_influence > 0.0 {
    let supervision_stress = best_influence * dt * SUPERVISION_STRESS_SCALE * dream_stress_factor;
    soul.stress = (soul.stress + supervision_stress).min(1.0);
}
```

- 完了条件:
  - [ ] `cargo check` 成功
  - [ ] dream高蓄積soulのストレス増加速度が上がること
- 検証:
  - `cargo check`

## M6: dream=0睡眠/休憩ガード（Decide phase）

dream=0のsoulが睡眠/休憩に入れないようガードし、睡眠中のdream=0で強制起床する。

- 変更内容:
  1. `select_next_behavior()`: dreamパラメータ追加、dream<=0でSleeping除外
  2. `random_gathering_behavior()`: dreamパラメータ追加、dream<=0でSleeping除外
  3. `wants_rest_area`条件: `soul.dream > 0.0` 追加
  4. sleeping中のdream=0チェック: 強制的にWandering/次サブ行動へ遷移
  5. callers更新（mod.rs、motion_dispatch.rs）
- 変更ファイル:
  - `src/systems/soul_ai/decide/idle_behavior/transitions.rs`
  - `src/systems/soul_ai/decide/idle_behavior/mod.rs`
  - `src/systems/soul_ai/decide/idle_behavior/motion_dispatch.rs`
- 詳細:

### transitions.rs

**`select_next_behavior`** (L46-71):
```rust
// dream引数を追加
pub fn select_next_behavior(laziness: f32, _fatigue: f32, _total_idle_time: f32, dream: f32) -> IdleBehavior {
    let can_sleep = dream > 0.0;
    let mut rng = rand::thread_rng();
    let roll: f32 = rng.gen_range(0.0..1.0);

    if laziness > LAZINESS_THRESHOLD_HIGH {
        if roll < 0.6 && can_sleep {
            IdleBehavior::Sleeping
        } else if roll < 0.6 || roll < 0.9 {
            // Sleepingが選べない場合はSittingにフォールバック
            IdleBehavior::Sitting
        } else {
            IdleBehavior::Wandering
        }
    } else if laziness > LAZINESS_THRESHOLD_MID {
        if roll < 0.3 && can_sleep {
            IdleBehavior::Sleeping
        } else if roll < 0.3 || roll < 0.6 {
            IdleBehavior::Sitting
        } else {
            IdleBehavior::Wandering
        }
    } else if roll < 0.7 {
        IdleBehavior::Wandering
    } else {
        IdleBehavior::Sitting
    }
}
```

**`random_gathering_behavior`** (L9-17):
```rust
pub fn random_gathering_behavior(dream: f32) -> GatheringBehavior {
    let mut rng = rand::thread_rng();
    if dream > 0.0 {
        match rng.gen_range(0..4) {
            0 => GatheringBehavior::Wandering,
            1 => GatheringBehavior::Sleeping,
            2 => GatheringBehavior::Standing,
            _ => GatheringBehavior::Dancing,
        }
    } else {
        // dream=0: Sleeping除外、3択
        match rng.gen_range(0..3) {
            0 => GatheringBehavior::Wandering,
            1 => GatheringBehavior::Standing,
            _ => GatheringBehavior::Dancing,
        }
    }
}
```

### mod.rs (idle_behavior_decision_system)

**`wants_rest_area`** (L177-180):
```rust
let wants_rest_area = soul.dream > 0.0  // 追加: dream=0では休憩不可
    && (soul.laziness > LAZINESS_THRESHOLD_MID
        || soul.fatigue > FATIGUE_IDLE_THRESHOLD * 0.5
        || soul.stress > ESCAPE_STRESS_THRESHOLD
        || idle.total_idle_time > IDLE_TIME_TO_GATHERING * 0.3);
```

**sleeping中のdream=0チェック** (L241付近、idle_timer判定の前に追加):
```rust
// dream=0で睡眠中なら強制起床
if soul.dream <= 0.0 && idle.behavior == IdleBehavior::Sleeping {
    idle.behavior = IdleBehavior::Wandering;
    idle.idle_timer = 0.0;
    idle.behavior_duration = transitions::behavior_duration_for(IdleBehavior::Wandering);
    path.waypoints.clear();
    path.current_index = 0;
    dest.0 = current_pos;
}
```

**`select_next_behavior`呼び出し** (L262-266):
```rust
idle.behavior = transitions::select_next_behavior(
    soul.laziness,
    soul.fatigue,
    idle.total_idle_time,
    soul.dream,             // 追加
);
```

**gathering遷移時の`random_gathering_behavior`呼び出し** (L251):
```rust
idle.gathering_behavior = transitions::random_gathering_behavior(soul.dream);  // dream引数追加
```

### motion_dispatch.rs

**`random_gathering_behavior`呼び出し** (L55):

`update_motion_destinations`にdream情報を渡す必要がある。

方法: 引数に`dream: f32`を追加。

```rust
pub fn update_motion_destinations(
    // ... 既存引数 ...
    dt: f32,
    dream: f32,    // 追加
) {
    // ...
    // L55:
    idle.gathering_behavior = transitions::random_gathering_behavior(dream);
    // ...
}
```

呼び出し元の`idle_behavior_decision_system` (L283-296):
```rust
motion_dispatch::update_motion_destinations(
    // ... 既存引数 ...
    dt,
    soul.dream,    // 追加
);
```

### gathering中のdream=0サブ行動チェック

`motion_dispatch.rs` のGathering処理内で、現在Sleepingサブ行動中にdream=0になった場合の強制切り替えを追加:

```rust
// gathering_behavior_timer更新後、Sleeping中にdream=0なら切り替え
if idle.gathering_behavior == GatheringBehavior::Sleeping && dream <= 0.0 {
    idle.gathering_behavior = transitions::random_gathering_behavior(dream); // dreamは0なのでSleeping以外が選ばれる
    idle.gathering_behavior_timer = 0.0;
    idle.gathering_behavior_duration = transitions::random_gathering_duration();
    idle.needs_separation = true;
}
```

- 完了条件:
  - [ ] `cargo check` 成功
  - [ ] dream=0のsoulがSleeping/Restingに遷移しないこと
  - [ ] sleeping中のdream=0でWanderingに遷移すること
  - [ ] gathering中のSleepingサブ行動がdream=0で切り替わること
- 検証:
  - `cargo check`

## M7: ドキュメント更新

- 変更内容:
  - `docs/dream.md`を改訂し、per-soul storage仕様を反映
  - 提案書のステータス更新
- 変更ファイル:
  - `docs/dream.md`
  - `docs/proposals/dream_per_soul_storage.md`
- 完了条件:
  - [ ] ドキュメントが実装と一致
  - [ ] 提案書ステータスが`Approved`に更新
- 検証:
  - ドキュメント内容の正確性確認

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| QueryのMutability競合 | `dream_update_system`が`&mut DamnedSoul`を要求するとUpdate phase内の他システムと競合する可能性 | Bevy 0.18のスケジューラが自動検出。同一set内の並行実行は`DamnedSoul`アクセスの排他性で順序付けされる |
| DreamPool収入のバランス変化 | 旧ロジックと蓄積→放出サイクルで収入が大幅に変わる | `DREAM_DRAIN_RATE`と蓄積レートを調整して移行前後のDreamPool収入を近似させる |
| dream=0+高疲労のデッドロック | soulが永遠に休めない | 蓄積レートが常に正なのでアイドル中も微量蓄積。完全なデッドロックは発生しない |
| `motion_dispatch::update_motion_destinations`の引数増加 | 関数シグネチャが肥大化 | dream 1引数の追加で収まる。将来的にコンテキスト構造体化を検討 |

## 7. 検証計画

- 必須:
  - `cargo check` 各マイルストーン完了時
- 手動確認シナリオ:
  - soulが労働中にdreamが増加 → 睡眠中にdreamが減少しDreamPoolが増加
  - dream=0のsoulがSleepingを選択しない
  - dream=0のsoulが休憩所へ行かない
  - 睡眠中にdream=0到達でWanderingに遷移
  - 休憩所でdream=0到達で退出
  - gathering中のSleepingサブ行動がdream=0で切り替わる
  - dream高蓄積soulのストレスが速く増加
  - DreamPool消費（植林）が正常動作
- パフォーマンス確認: 不要（per-soul演算は既存バイタルと同等の計算量）

## 8. ロールバック方針

- どの単位で戻せるか: マイルストーン単位でgit revert可能
- 戻す時の手順:
  1. dream関連の変更をrevert
  2. `DamnedSoul`からdreamフィールドを削除
  3. `dream_update_system`と`rest_area_update_system`を旧ロジックに戻す
  4. `cargo check` で確認

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`（計画書作成完了、実装未着手）
- 完了済みマイルストーン: なし
- 未着手/進行中: M1〜M7すべて未着手

### 次のAIが最初にやること

1. 本計画書を読み、提案書 (`docs/proposals/dream_per_soul_storage.md`) と照合
2. M1（データモデル+定数追加）から着手
3. 各マイルストーン完了時に`cargo check`を実行

### ブロッカー/注意点

- `dream_update_system`のQuery変更で`&DamnedSoul`→`&mut DamnedSoul`にする際、他のUpdate phaseシステムとの排他性に注意
- `rest_area_update_system`でも`&DamnedSoul`→`&mut DamnedSoul`に変更が必要
- `motion_dispatch::update_motion_destinations`は多数の引数を持つ関数。dream引数追加時に呼び出し元も忘れずに更新
- `IdleDecisionSoulQuery`は`&DamnedSoul`（読み取り専用）。Decide phaseではdream値を読むだけなのでQuery型変更は不要
- 蓄積/放出レートの具体値は仮値。ゲームプレイテストで調整が必要

### 参照必須ファイル

- `docs/proposals/dream_per_soul_storage.md` — 仕様
- `src/entities/damned_soul/mod.rs` — DamnedSoul定義
- `src/systems/soul_ai/update/dream_update.rs` — 現行dreamロジック
- `src/systems/soul_ai/update/rest_area_update.rs` — 休憩所ロジック
- `src/systems/soul_ai/update/vitals_influence.rs` — ストレス蓄積ロジック（L75-90）
- `src/systems/soul_ai/decide/idle_behavior/transitions.rs` — 行動選択関数
- `src/systems/soul_ai/decide/idle_behavior/mod.rs` — idle_behavior_decision_system（L177:wants_rest_area, L241-280:行動遷移, L283:motion_dispatch呼出）
- `src/systems/soul_ai/decide/idle_behavior/motion_dispatch.rs` — 集会サブ行動更新（L55:random_gathering_behavior呼出）
- `src/systems/soul_ai/helpers/query_types.rs` — Query型定義
- `src/systems/soul_ai/mod.rs` — システム登録・順序
- `src/constants/dream.rs` — Dream定数
- `src/constants/ai.rs` — AI定数

### 最終確認ログ

- 最終 `cargo check`: 未実施
- 未解決エラー: なし

### Definition of Done

- [ ] 目的に対応するマイルストーンが全て完了（M1〜M7）
- [ ] 影響ドキュメントが更新済み（docs/dream.md）
- [ ] `cargo check` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-02-26` | `AI (Claude)` | 初版作成 |
