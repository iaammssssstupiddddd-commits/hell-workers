# MS-3-Char-B 実装計画（2026-03-29）

## 問題

`MS-3-Char-A` は完了し、Soul の 3D 表示は以下まで到達している。

- `SoulAnimationLibrary` が `soul.glb` の `Idle / Walk / Work / Carry / Fear / Exhausted / WalkLeft / WalkRight` を読み込む
- `SoulAnimVisualState { body, face }` が導入済み
- `sync_soul_body_animation_system` が `AnimationGraph` を切り替える
- `sync_soul_face_expression_system` が per-instance face material の atlas offset を更新する
- `Normal / Sleep / Fear / Exhausted / Focused / Happy` の face atlas 連動は目視確認済み

その一方で、計画書上の `MS-3-Char-B` はまだ「未着手」扱いだった。実際には、基盤と初期写像の大半が `MS-3-Char-A` の中で先行実装されており、残りは以下の整理と確認だった。

- `Work / Carry` の body clip 切り替えを、実際の task phase と照合して確認する
- `Fear / Exhausted` の body / face 写像が、既存 state（breakdown / exhausted gathering / 会話表情）から妥当に導かれているか確認する
- `MS-3-Char-B` を「新規構築」ではなく「既存状態からの写像確認 + 目視検証 + 必要最小限の補正」として捉え直す

## 現行実装の棚卸し

### すでに実装済みのもの

- `soul_animation.rs`
  - `desired_body_state(...)` が `Walk / Work / Carry / Fear / Exhausted` を返す
  - `desired_face_state(...)` が `Fear / Exhausted / Focused / Happy / Sleep / Normal` を返す
  - `SoulAnimationLibrary::node_for(...)` が `Work / Carry / Fear / Exhausted` の clip node を解決する
- `hw_jobs::visual_sync::sync_soul_task_visual_system`
  - `AssignedTask` から `SoulTaskPhaseVisual` への mirror は既に存在する
- `expression_events.rs`
  - `ConversationExpressionKind::{Positive, Negative, Exhausted, GatheringWine, GatheringTrump}` は既に Soul へ付与される
- `soul_face_atlas_layout.md`
  - `Fear / Exhausted / Focused / Happy / Sleep` の atlas セル定義は既に存在する

### 今回確認したもの

- `Work` と `Carry` をどの task phase 群で使うかの写像
- `StressBreakdown` / `ExhaustedGathering` / 会話表情が body clip と face のどちらへどう反映されるか
- `Carry` 中の横移動で `WalkLeft / WalkRight` を使う現在の暫定仕様を維持するか
- これらの写像を再現可能に確認する手段

## 方針

### 1. `MS-3-Char-B` は「既存状態からの写像確認マイルストーン」として扱う

今回の主作業は、新しい描画経路を増やすことではなく、既存の state から `SoulAnimVisualState` への写像が妥当か確認し、不足だけを補うことにある。

やるべきことは次の 3 つに絞る。

1. body state 写像を明文化する
2. face state 写像を明文化する
3. その写像どおりに再現・確認できる経路を整える

### 2. body / face の写像をテーブル化する

現在の `desired_body_state` / `desired_face_state` は if 文の積み重ねで表現されているが、`MS-3-Char-B` では「どの既存 state の組み合わせがどの 3D 表示 state に写るか」を仕様として明文化する。

#### body 側の写像候補

1. `StressBreakdown`
   - `is_frozen = true`: body = `Idle`
   - `is_frozen = false`: body = `Fear`
2. `IdleBehavior::ExhaustedGathering`
   - body = `Exhausted`
3. moving 中
   - carry phase: `Carry`
   - それ以外: `Walk`
4. 静止中
   - carry phase: `Carry`
   - work phase: `Work`
   - それ以外: `Idle`

#### face 側の写像候補

1. `StressBreakdown` または negative conversation
   - face = `Fear`
2. positive conversation / gathering expression
   - face = `Happy`
3. sleeping / resting かつ非 busy
   - face = `Sleep`
4. fatigue しきい値超過 / exhausted gathering / exhausted expression
   - face = `Exhausted`
5. work phase
   - face = `Focused`
6. それ以外
   - face = `Normal`

`MS-3-Char-B` では、この写像をコードと docs の両方で一致させる。

### 3. task phase の分類を棚卸しする

`is_work_phase(...)` / `is_carry_phase(...)` が `SoulTaskPhaseVisual` 全体を適切に覆っているか確認する。

監査対象:

- `Haul`
- `HaulToBlueprint`
- `HaulToMixer`
- `HaulWithWheelbarrow`
- `BucketTransport`
- `Build`
- `ReinforceFloor`
- `PourFloor`
- `FrameWall`
- `CoatWall`
- `Refine`
- `CollectSand`
- `CollectBone`
- `MovePlant`

ここで不足があれば `hw_jobs::visual_sync::sync_soul_task_visual_system` か `soul_animation.rs` の分類関数を修正する。

### 4. 「確認できること」を先に作る

`Fear / Exhausted / Work / Carry` は通常プレイでも再現できるが、確認コストが高い可能性がある。必要なら `visual_test` に最小限の direct state preview を追加する。

候補:

- `SoulBodyAnimState` を直接選ぶデバッグ切替
- `SoulFaceState` を直接選ぶデバッグ切替
- `task_visual phase` / `fatigue` / `conversation expression` の preset 注入

ただし、本編で十分再現可能なら debug UI は増やさない。まずは既存経路で確認可能かを優先する。

## 実装ステップ

### Step 1. 現行写像の明文化

- `soul_animation.rs` の body / face 判定を読み直し、既存 state から表示 state への写像として整理する
- `MS-3-Char-B` の plan / roadmap / architecture に写像規則を言語化する

### Step 2. task phase 分類の監査

- `SoulTaskPhaseVisual` の全 variant を洗い出す
- `is_work_phase(...)` / `is_carry_phase(...)` が意図どおりか確認する
- `Work` / `Carry` に入るべき task が漏れていれば補正する

### Step 3. body clip の確定

- `Work / Carry / Fear / Exhausted` の clip 切り替えを gameplay で再現する
- body `Exhausted` は `IdleBehavior::ExhaustedGathering` にのみ結び付くことを確認する
- carry 中横移動で `WalkLeft / WalkRight` を使う現仕様を維持するか判断する

### Step 4. face atlas 写像の確認

- `Fear / Exhausted / Focused / Happy / Sleep / Normal` の写像が既存 state と整合しているか確認する
- positive / negative / exhausted expression と idle / fatigue / task phase の競合時挙動を確認する
- 必要があれば `desired_face_state(...)` の写像条件を補正する

### Step 5. 検証経路の補強

- 本編だけで十分に再現できない状態がある場合のみ `visual_test` へ preview を追加する
- preview を入れる場合も、本編ロジックを bypass せず、最終的には同じ state 更新経路へ流す

### Step 6. ドキュメント同期

- `phase3-implementation-plan-2026-03-16.md`
- `milestone-roadmap.md`
- `asset-milestones-2026-03-17.md`
- `architecture.md`

を現行仕様へ揃える。

## 変更ファイル候補

- `crates/bevy_app/src/systems/visual/soul_animation.rs`
- `crates/hw_jobs/src/visual_sync/sync.rs`
- `crates/bevy_app/examples/visual_test.rs`（必要な場合のみ）
- `docs/architecture.md`
- `docs/plans/3d-rtt/phase3-implementation-plan-2026-03-16.md`
- `docs/plans/3d-rtt/milestone-roadmap.md`
- `docs/plans/3d-rtt/asset-milestones-2026-03-17.md`

## 期待効果

- `MS-3-Char-B` の未達が「実装不足」なのか「既存写像の確認不足」なのかが明確になる
- Soul の body / face 状態写像が docs とコードで一致する
- `Work / Carry / Fear / Exhausted` の見え方確認が済み、次段のマイルストーンに迷いが残らない

性能影響は軽微で、主に状態分岐整理と検証経路整備が中心になる見込み。

## 検証

### コード

- `cargo check --workspace`
- `cargo clippy --workspace -- -D warnings`

### 目視

- `Work` が作業フェーズで再生される
- `Carry` が搬送フェーズで再生される
- breakdown freeze 中は body が `Idle` のままである
- `Fear` body が breakdown freeze 明けで再生される
- `Exhausted` が `IdleBehavior::ExhaustedGathering` で再生される
- face atlas が `Fear / Exhausted / Focused / Happy / Sleep / Normal` へ期待どおりに切り替わる
- 既存の `Idle / Walk / WalkLeft / WalkRight` に退行がない
- Soul mask prepass / shadow proxy / Familiar 表示に退行がない

## リスク

- `Work` と `Carry` の境界が `SoulTaskPhaseVisual` 定義と完全には一致していない可能性がある
- `ExhaustedGathering` と body `Exhausted` の対応が、今後の idle state 拡張時にずれる可能性がある
- debug preview を足しすぎると visual_test が本編ロジックから乖離する

## 結論

`MS-3-Char-B` は greenfield 実装ではなく、既に存在する `Work / Carry / Fear / Exhausted` の body/face 経路が既存 state から妥当に導かれているか確認し、必要最小限の補正を行う段階である。したがって次は、

1. state 写像の明文化
2. task phase 分類の監査
3. 目視確認と必要最小限の補正

の順で進めるのが最短である。

今回の確認で、上記 3 点は完了した。`MS-3-Char-B` は以下をもって完了扱いとする。

- `Work / Carry` の P1 clip が task phase と連動することを目視確認
- face atlas の状態連動を目視確認
- body `Fear` は `StressBreakdown` にのみ結び付き、freeze 中は `Idle` のままであることを確認
- body `Exhausted` は `IdleBehavior::ExhaustedGathering` にのみ結び付き、通常 fatigue / exhausted expression は face 側で扱う写像へ補正
