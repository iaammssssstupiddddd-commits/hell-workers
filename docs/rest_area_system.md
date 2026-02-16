# 休憩所システム (Rest Area System)

このドキュメントでは、Hell-Workers におけるワーカー（Damned Soul）の休息を管理する「休憩所システム」について解説します。

## 1. 概要
休憩所（Rest Area）は、過酷な労働環境にあるワーカーたちが疲労やストレスを回復するための施設です。建設された休憩所が存在する場合、ワーカーは自律的に判断して休憩所へ向かい、一定時間滞在することでバイタルを回復させます。

## 2. 主要なコンポーネント

Bevy 0.18 の **ECS Relationships** を活用し、休憩所の定員管理や予約状態を型安全に管理しています。

### 休憩所側 (RestArea)
- `RestArea`: 休憩所であることを示すコンポーネント。
  - `capacity`: 同時に休憩できるワーカーの最大数。
- **`RestAreaOccupants(Vec<Entity>)`** (Target): 現在休憩中のワーカー一覧。
- **`RestAreaReservations(Vec<Entity>)`** (Target): 現在この休憩所へ向かっている（予約済み）ワーカー一覧。

### ワーカー側 (Damned Soul)
- **`RestingIn(Entity)`** (Relationship): 現在どの休憩所で休憩しているか。
- **`RestAreaReservedFor(Entity)`** (Relationship): どの休憩所を予約して移動中か。
- `RestAreaCooldown`: 休憩終了後、一定時間（リクルート不可など）の状態を管理するタイマー。

## 3. 休憩の発生条件

ワーカーは `Decide` フェーズの `idle_behavior_decision_system` において、以下のいずれかの条件を満たした場合に休憩を検討します。

1. **高い怠惰 (Laziness)**: `soul.laziness > LAZINESS_THRESHOLD_MID`
2. **中程度の疲労 (Fatigue)**: `soul.fatigue > FATIGUE_IDLE_THRESHOLD * 0.5`
3. **ストレス (Stress)**: `soul.stress > ESCAPE_STRESS_THRESHOLD`
4. **長い待機時間**: 累積アイドル時間が `IDLE_TIME_TO_GATHERING * 0.3` を超過

※ 条件を満たしても、最寄りの休憩所に空き（`capacity > 現在の利用者 + 予約者`）がない場合は休憩を行いません。

## 4. 休憩のライフサイクル

休憩行動は以下の 4 つのステップで進行します。

### 1. 予約 (Reservation)
条件を満たしたワーカーは、最も近い利用可能な休憩所を検索し、`RestAreaReservedFor` を付与して予約します。
- この時点から、休憩所の `capacity` を 1 枠消費します。
- 状態は `IdleBehavior::GoingToRest` に遷移します。

### 2. 移動 (GoingToRest)
休憩所の入口（または隣接マス）へ移動します。
- **移動の特徴**: 他のワーカーとの重なり回避（`gathering_separation_system`）をスキップし、効率的に目的地へ向かいます。
- **割り込み防止**: 移動中は使い魔によるリクルートの対象外となります。

### 3. 休憩 (Resting)
休憩所に到着すると、ワーカーは `RestingIn` 状態になります。
- **非表示化**: ワーカーのスプライトは非表示（`Visibility::Hidden`）になります。
- **回復**: 滞在中、疲労とストレスが継続的に減少します。
- **固定時間**: 3分間（`REST_AREA_RESTING_DURATION` 秒）、滞在が維持されます。

### 4. 退出とクールダウン (Exit & Cooldown)
所定の時間が経過すると休憩所から退出します。
- **位置の復帰**: 休憩所の中心から、歩行可能な隣接マスへ再配置されます。
- **クールダウン**: 退出直後に再び休憩に入ったり、即座に過酷な労働に投入されたりするのを防ぐため、`RestAreaCooldown` が付与されます。

## 5. 回復効果

休憩所での滞在は、通常の待機（Idle）よりも高い回復効果をもたらします。

| パラメータ | 回復速度（対通常待機） |
| :--- | :--- |
| **疲労 (Fatigue)** | 大幅に減少 |
| **ストレス (Stress)** | 大幅に減少 |

詳細な係数は `src/systems/soul_ai/update/vitals.rs` または関連するバイタル更新ロジックで定義されています。

## 6. 実装上の注意点

### 集会システムとの分離
かつて集会（Gathering）と休憩への移動が競合し、目的地が上書きされる問題がありましたが、現在は `GoingToRest` 状態を独立させ、移動フェーズでの排他制御を行うことで解決されています。

### 定員管理の整合性
ECS Relationships を使用しているため、ワーカーが削除（デスポーン）された場合や、休憩所が破壊された場合、予約や入所状態は Bevy のエンジンレベルで自動的にクリーンアップされます。これにより、定員（Capacity）の計算が狂う「ゴースト予約」の問題が防がれています。

## 7. 関連ファイル
- `src/systems/soul_ai/decide/idle_behavior.rs`: 休憩の意思決定、予約、到着判定。
- `src/systems/soul_ai/execute/idle_behavior.rs`: 休憩所への入退所処理、非表示化。
- `src/relationships.rs`: `RestingIn`, `RestAreaOccupants` 等の Relationship 定義。
- `src/systems/jobs/mod.rs`: `RestArea` コンポーネントの定義。
