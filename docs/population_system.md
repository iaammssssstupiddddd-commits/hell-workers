# Population System

Soul の流入（スポーン）と流出（漂流デスポーン）を管理するシステムです。  
関連実装は主に `src/entities/damned_soul/spawn.rs` と `src/systems/soul_ai/decide|execute/drifting.rs` にあります。

## 1. スポーン

### 1.1 初期スポーン
- 起動時の初期 Soul 数は `SOUL_SPAWN_INITIAL`（デフォルト 10）
- `--spawn-souls` / `HW_SPAWN_SOULS` で上書き可能
- **初期スポーン位置は川の南側のみ**（`RIVER_Y_MIN - 1` 以南）

### 1.2 定期スポーン
- 間隔: `SOUL_SPAWN_INTERVAL`（60秒）
- 1回の出現数: `SOUL_SPAWN_COUNT_MIN..=SOUL_SPAWN_COUNT_MAX`（1〜2）
- 現在人口が上限の 50% 以下なら +1 体
- 人口が 0 の場合は緊急スポーン（`SOUL_SPAWN_INITIAL`）

## 2. 人口上限

人口上限は `PopulationManager` で毎フレーム更新されます。

```text
population_cap = SOUL_POPULATION_BASE_CAP + RestArea数 * SOUL_POPULATION_PER_REST_AREA
```

- `SOUL_POPULATION_BASE_CAP = 10`
- `SOUL_POPULATION_PER_REST_AREA = 5`

## 3. 脱走（Drifting）

未管理状態が続いた Soul は、確率で `IdleBehavior::Drifting` に遷移してマップ端へ漂流します。

### 3.1 開始条件
- `CommandedBy` なし
- `AssignedTask::None`
- `RestingIn` なし
- `IdleState.total_idle_time >= SOUL_ESCAPE_UNMANAGED_TIME`（120秒）
- 判定間隔 `SOUL_ESCAPE_CHECK_INTERVAL`（10秒）
- 判定確率 `SOUL_ESCAPE_CHANCE_PER_CHECK`（0.3）
- グローバルクールダウン `SOUL_ESCAPE_GLOBAL_COOLDOWN`（30秒）中は開始しない

### 3.2 行動
- `DriftPhase::Wandering`（5〜10秒）と `DriftPhase::Moving` を交互に実行
- `Moving` では最寄りのマップ端へ 3〜6 タイルずつ進行（横ブレあり）

### 3.3 終了
- 端から `SOUL_DESPAWN_EDGE_MARGIN_TILES`（2タイル）以内でデスポーン
- 累計脱走数 `PopulationManager.total_escaped` を加算
- リクルート・タスク再割り当て時は Drifting を解除し通常状態へ復帰

## 4. 主なリソース/型

- `PopulationManager`（Resource）
  - 現在人口、人口上限、累計スポーン/脱走、脱走クールダウンを保持
- `DriftingState`（Component）
  - 目標端、現在フェーズ、フェーズタイマーを保持
