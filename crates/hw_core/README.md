# hw_core — コア型定義・システム基盤

## 役割

ゲーム全体で共有される**基盤型・定数・システム実行順序**を提供するクレート。
他のすべての hw_* クレートがこのクレートに依存する。UI/アニメーション/スポーン等の実装は含まない。

## 主要モジュール

| ファイル | 内容 |
|---|---|
| `system_sets.rs` | システム実行順序 (`GameSystemSet`, `FamiliarAiSystemSet`, `SoulAiSystemSet`) |
| `soul.rs` | `DamnedSoul` バイタル・`IdleState` など Soul コンポーネント群 |
| `familiar.rs` | `Familiar` コンポーネント (name, efficiency, command_radius) |
| `gathering.rs` | 集会 shared model (`GatheringSpot`, `GatheringObjectType`, timer/readiness) |
| `events.rs` | クレート間通信用メッセージ・Observer イベント定義 |
| `relationships.rs` | ECS Relationship 定義 (`ManagedBy`, `CommandedBy` 等) |
| `area.rs` | 共有矩形抽象型 `AreaBounds` と `TaskArea`（Familiar 担当エリア） |
| `jobs.rs` | `WorkType` 関連 Relationship 定義 |
| `logistics.rs` | ロジスティクス共通型 (Stockpile 等) |
| `game_state.rs` | ゲーム状態管理 |
| `world.rs` | ワールドコンテキスト型 |
| `constants/` | ドメイン別定数 (下表参照) |

### constants/ ディレクトリ

| ファイル | 内容 |
|---|---|
| `ai.rs` | AI 閾値 (疲労・動機・ストレス) |
| `animation.rs` | アニメーション速度・タイミング |
| `building.rs` | 建物種別・素材 |
| `conversation.rs` | 会話システム定数 |
| `dream.rs` | 夢システムパラメータ |
| `logistics.rs` | 輸送・リソース定数 |
| `render.rs` | Z軸レイヤー・表示定数 |
| `speech.rs` | 発話システムパラメータ |
| `world.rs` | ワールド固有定数 |
| `world_zones.rs` | ゾーン設定定数 |

## システム実行順序

```
GameSystemSet:
  Input → Spatial → Logic → Actor → Visual → Interface

FamiliarAiSystemSet / SoulAiSystemSet (Logic フェーズ内):
  Perceive → Update → Decide → Execute
```

- `Spatial` / `Logic` / `Actor` は仮想時間ゲートにより一括停止可能
- AI フェーズ間には `ApplyDeferred` が挿入される

## Soul コンポーネント主要型

```rust
DamnedSoul { laziness, motivation, fatigue, stress, dream }
IdleState   { behavior: IdleBehavior, gathering_behavior, ... }
StressBreakdown { is_frozen, remaining_freeze_secs }
DriftingState   { target_edge: DriftEdge, phase: DriftPhase, ... }
```

`IdleBehavior` の種類: `Wandering / Sitting / Sleeping / Gathering / ExhaustedGathering / Resting / GoingToRest / Escaping / Drifting`

## 依存クレート

- `bevy` (ECS・数学型)
- `rand` (乱数)
- 他の hw_* クレートには **依存しない**（最下層）

---

## src/ との境界

hw_core は**型定義・定数・システムセット定義のみ**を提供する。
Bevy への登録（`app.register_type()`・`app.add_systems()` 等）は src/ 側で行う。

| hw_core に置くもの | src/ に置くもの |
|---|---|
| コンポーネント型定義 (`DamnedSoul`, `IdleState`, `TaskArea`, `GatheringSpot` 等) | コンポーネントの `#[reflect]` 登録 |
| `GameSystemSet` / `FamiliarAiSystemSet` 等の定義 | `.configure_sets()` による実行順序配線 |
| 定数値 (`constants/`) | 定数を使うシステム実装 |
| `Message` 型の定義（`events.rs`, `GatheringSpawnRequest` など） | `MessagesPlugin` での `app.add_message::<T>()` 登録 |
| ECS Relationship 型定義 (`relationships.rs`) | Relationship を生成・削除するシステム |
