# Track B1 Stockpile ポリシー 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `stockpile-policy-plan-2026-07-20` |
| ステータス | `Completed` |
| 作成日 | `2026-07-20` |
| 最終更新日 | `2026-07-22` |
| 作成者 | `Codex` |
| 関連提案 | `docs/proposals/gameplay-management-improvements-proposal-2026-07-17.md`（Track B1） |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: 現在在庫と受入方針が未分離で、搬送経路ごとの判定も統一されていない。
  - `Stockpile.resource_type` は現在内容を表す値であり、空になっても残る受入方針として使えない。
  - 搬入先・搬出元の判定が producer、arbitration、Familiar の validator、Soul の実行系へ分散しており、
    方針を一部だけへ足すと予約済み容量、手搬送、猫車搬送、集約搬送の判定が食い違う。
  - Yard 由来の `StockpileGroup` は重複し得る派生集合なので、group 単位の設定では同じセルへ矛盾する正本が生まれる。
- 到達したい状態:
  - 通常の Stockpile セルごとに、受入資源、搬入優先度、目標量、搬出許可を永続設定できる。
  - 既存在庫と新方針が不一致でも内容を破壊せず、新規搬入を止めて安全に draining する。
  - 単一セルと矩形範囲の UI が同じ Message/Intent 経路を通り、物流の全経路が共通の適格性判定を使う。
- 成功指標:
  - 旧セーブは現行挙動と等価な既定方針へ移行し、新セーブはセル別方針を往復保持する。
  - 目標量、物理容量、`IncomingDeliveries`、同一 cycle 内の予約影を含め、過剰搬入と二重割当が 0 件である。
  - 重複 Yard、手搬送、通常搬送、猫車搬送、集約搬送で受入・搬出判定が一致する。

## 2. スコープ

### 対象（In Scope）

- 通常の Stockpile セル Entity を正本とする永続 `StockpilePolicy`。
- 初版の受入指定 `Any` / `Only(ResourceType)`、搬入優先度、目標量、搬出許可。
- 現在内容・物理容量・予約量・方針を統合する `hw_logistics` 所有の純粋判定 API。
- `DepositToStockpile`、`ConsolidateStockpile`、手搬送の搬入先選択、猫車搬送、割当後再検証、荷下ろし完了の整合。
- 選択セルの inspection/editor と、矩形範囲へ同一 patch を適用する操作。
- 旧 v0/v1 セーブへの既定値補完、新形式の round-trip、runtime cache の再構築。
- 専用 `StockpilePolicyChangeOutcome` Message と、A2 で確立した `UserFacingNotification` adapter を使う
  成功・一部適用・失敗表示。

### 非対象（Out of Scope）

- 資源カテゴリ、複数資源の許可集合、比率、最小在庫、物流ルートなどの高度なルール。
- 方針変更時の強制排出、自動再配置、自動在庫平準化。
- Stockpile 内容を直接 source 指定する新しい manual haul UI。初版の手搬送は現行どおり地面上 item の搬入である。
- Tank、Mud Mixer、`BucketStorage` など、`Stockpile` を内部容量表現として再利用する特殊設備への方針付与。
- Yard / `StockpileGroup` を方針の正本にすること。
- 新しい全体在庫画面、物流統計、セーブスロット管理。

## 3. 現状とギャップ

- `crates/hw_logistics/src/zone.rs` の `Stockpile` は `capacity` と現在内容の
  `resource_type: Option<ResourceType>` だけを持つ。空になれば資源種別は消える。
- `StockpileGroup` と `CachedStockpileGroups` は Yard とセルから作る派生データである。同じセルは重複 Yard の
  複数 group に現れ得るため、group 側へ durable policy を置けない。
- 搬入可否と残容量の計算は `transport_request/producer/task_area.rs`、`arbitration/candidates.rs`、
  `familiar_ai/.../validator/resolver.rs`、Soul の haul / wheelbarrow 荷下ろしなどへ分散している。
- `IncomingDeliveries` と producer cycle 内の reservation shadow を無視すると、同じ tick で複数要求が
  同じ最後の空きへ割り当てられる。
- `Stockpile` は通常セル以外にも使われる。既存 Entity へ一律に `StockpilePolicy` を補完すると Tank や Mixer の
  専用需要ロジックへプレイヤー設定が混入する。
- `BucketStorage` は現行 save schema の persisted component ではないため、`Without<BucketStorage>` は旧セーブ上の
  通常セル識別に使えない。通常 zone の durable な正の条件は `BelongsTo(owner)` の owner が `Yard` であること。
- 通常 Familiar の候補 score は `hw_jobs::Priority`、猫車 arbitration は `TransportRequest.priority` を読む。
  request field だけを更新しても搬入優先度が全搬送経路へ反映されない。
- 現行 save header は v1 で body schema と独立している。初版は additive component migration とし、
  旧セーブに component がない場合だけ rehydrate で既定値を挿入する。新型を知らない旧実行ファイルからの
  forward load は保証せず、未知型は旧 registry の deserialize / `InvalidData` として live apply 前に安全に失敗させる。

## 4. 実装方針（高レベル）

### 4.1 固定するデータ契約

```rust
pub enum StockpileAcceptance {
    Any,
    Only(ResourceType),
}

pub struct StockpilePolicy {
    pub acceptance: StockpileAcceptance,
    pub inbound_priority: TransportPriority,
    pub target_amount: usize,
    pub allow_export: bool,
}
```

- `Stockpile.resource_type` は引き続き現在内容の正本とし、policy へ統合・削除しない。
- 通常セルでの既定値は `Any`、`Normal`、`target_amount = capacity`、`allow_export = true` とし、
  既存 gameplay を変えない。
- `target_amount` は適用時と読込補完時に `0..=capacity` へ clamp する。0 は「新規搬入しない」であり、
  在庫の即時削除を意味しない。
- `StockpilePolicy` の存在を「player-managed stockpile cell」の明示境界にする。通常 zone 配置時に追加し、
  Tank / Mixer / `BucketStorage` の spawn には追加しない。
- 旧セーブ補完は `Stockpile + Without<StockpilePolicy> + BelongsTo(owner)` を起点にし、owner Entity が durable な
  `Yard` である場合だけ追加する。`Without<BucketStorage>` や表示名など欠落し得る負の marker へ依存しない。
  owner 不在・owner 非 Yard の Stockpile へ推測で追加しない。

### 4.2 共通判定と状態遷移

- `hw_logistics` に純粋な policy evaluator を置き、`NewInbound`、`CommittedInbound`、`NewOutbound` の phase を
  明示入力にする。少なくとも次を同じ入力から返す。
  - 資源互換性
  - 物理残容量
  - 目標量までの残容量
  - `IncomingDeliveries` と cycle-local reservation shadow 控除後の予約可能量
  - `Accepting` / `TargetReached` / `Draining` の導出状態と拒否理由
- `Draining` は「現在内容が `Only` と不一致」の導出状態とする。永続 component にはしない。
- `NewInbound` は acceptance、target、physical capacity、incoming、cycle shadow を全て検証する。
  `CommittedInbound` は reservation token / relationship で自分の確保枠を識別し、変更後の acceptance と target を
  grandfather して物理容量と現在内容の互換性だけを再検証する。これにより方針変更前の割当済み・運搬中搬入を
  既存 lifecycle で完了させる。物理的に置けない場合は既存 retry / cancellation disposition を通し、item を失わない。
- 新方針は新規 request / grant から適用し、途中の関係 component を UI や policy system が剥がさない。
- `allow_export = false` は方針適合中の在庫に対する新しい搬出元選択と consolidation donor 選択を禁止する。
  ただし受入資源と現在内容が不一致の `Draining` では提案仕様を優先して実効搬出を許可する。既に持ち上げた品や
  実行中タスクは常に安全な完了・既存 cleanup を優先する。
- 目標量を下げても自動搬出要求は作らない。現在量が目標以下になるまで新規搬入だけを止める。

### 4.3 group、優先度、cache

- group membership は Yard と `With<StockpilePolicy>` のセルから導出するだけに留め、特殊 storage を
  `BucketStorage` marker の有無だけで識別しない。policy や動的な stored / incoming 合計を stale cache の正本にしない。
- `DepositToStockpile` の group 需要は、その cycle の live snapshot と reservation shadow から求める。
  cache へ動的集計を残す場合は `Changed<StockpilePolicy>`、`StoredItems`、`IncomingDeliveries` の
  invalidation を必須とし、steady state で再構築しないテストを置く。
- group の受入可能セルは `inbound_priority` ごとに partition し、request identity を
  `(group owner, resource type, priority tier)` とする。各 `DepositToStockpile` request の `stockpile_group` は
  その tier の適格セルだけを持ち、需要量も tier 内の live capacity / reservation shadow だけから求める。
  group 最大値を異なる tier のセルへ共有しない。
- 既存 tier request も producer cycle ごとに desired slots / subset / priority を更新し、適格セル 0 なら新規割当を
  停止する。resolver は request の subset 内で同順位の既存 packing 規則、最後に安定したセル順を使う。
  High の最後の枠を同 cycle shadow が埋めた後は同 request を拒否し、assignment loop が Low / Normal の別 request を
  次候補として評価する。committed worker を持つ旧 tier request は既存 lifecycle が終わるまで保持する。
- 猫車 arbitration は引き続き tier 別 `TransportRequest.priority` を読む。通常 Familiar task finder は既存 i32 candidate priority と
  `score_for_worker` を一切変更せず、算出済み worker score へ request の元になった receiver policy tier を named
  transport policy score offset として加える。最終 rank score は
  `base_worker_score + transport_policy_offset + familiar_policy_offset` とし、最後に clamp しない。
  共有 `POLICY_SCORE_UNIT` は現行 priority slope の `WORKER_PRIORITY_WEIGHT / 40.0` とし、transport contribution は
  Low=-10、Normal=0、High=+10、Critical=+20 unit とする。これにより Normal の現行順位を保ち、上限到達後も
  tier差を現行priority slopeの線形延長として維持する。
  shared helper は enum ではなく scalar の transport / familiar contribution を受け取り、各トラックが自分の enum だけを
  named constant へ変換する。B1 単独時の familiar offset、B2 単独時の transport offset は 0 とし、どちらを先に実装しても
  同じ helper を使う。両方を実装した場合も Low-to-High/Critical の合算 span は最大40 unit、すなわち現行
  `WORKER_PRIORITY_WEIGHT` 以内に収める。transport の隣接 tier 差10 unitは0.1625で、現行の全距離 span 0.35より小さいため
  1段階の policy 差を lexicographic hard tier にはしない。一方、複数 policy の同時指定や最小〜最大の累積差が距離差を
  上回り得ることは、明示された複数方針を合成した結果として許容する。
  `policy_score.rs` を `WORKER_PRIORITY_WEIGHT`、`WORKER_DISTANCE_WEIGHT`、`POLICY_SCORE_UNIT`、scalar contribution struct、
  composition helper の単一所有者にし、`assignment_loop.rs` の base scorer もそこから weight を読む。0.65 / 0.35 literal を
  二重定義しない。
  `ScoredDelegationCandidate` は transport / familiar の scalar unit を保持し、candidate collection 時に B1 は receiver tier、
  B2 は Familiar policy と `WorkType` から各 unit を解決する。worker ごとの距離 score を算出した後、Top-K 選択の直前に
  helper で一度だけ合成する。
  `hw_jobs::Priority` へコピーせず、A3 の task priority、Stockpile 搬入優先度、B2 の Familiar 方針を別の操作として保つ。
- `ConsolidateStockpile` は通常 Familiar の現行 `Priority(0)` base score と、猫車 arbitration の現行 maintenance 用
  `TransportPriority::Low` をそれぞれ維持し、receiver policy を Normal=0 の transport policy score offset / modifier として
  適用する。default policy で両経路の現行値を変えず、High / Critical receiver だけを段階的に上げる。
  raw `TransportRequest.priority` を通常 Familiar offset へ直接変換すると maintenance Low が二重適用されるため、通常経路は
  receiver policy tier を読み、猫車経路だけが既存 Low base を含む effective request priority を読む。
  明示的な manual haul は player action の既存 priority を保持し、policy は destination eligibility にだけ適用する。
- 重複 Yard から同じセルを評価しても、grant 時の共通 evaluator と reservation shadow で一度だけ容量を消費する。

### 4.4 UI と所有境界

- `hw_ui` は editor の ViewModel と `UiIntent` だけを所有し、ECS component を直接変更しない。
- root adapter は `StockpilePolicyChangeRequest { targets, patch }` へ変換する。domain handler は Entity の生存、
  policy-managed cell か、値の範囲を再検証し、専用 `StockpilePolicyChangeOutcome` Message で
  適用件数・skip 件数・clamp を返す。root notification adapter が player-safe な文言へ変換する。
- 単一セル editor と矩形編集は同じ patch 型を使う。範囲確定時に `StockpileSpatialGrid` から対象を列挙し、
  重複を除去した安定順の Entity 配列を一要求として渡す。
- 新しい物理キーは追加せず、A1 の既存 `TaskMode` / input capture 規約へ policy edit mode を接続する。

### 4.5 設計判断

| ID | 判断 |
| --- | --- |
| B1-D01 | 初版の受入指定は `Any` / `Only(ResourceType)` に限定する |
| B1-D02 | policy の正本は通常 Stockpile セル Entity。group と draining は導出値 |
| B1-D03 | 変更前の搬送は強制 cancel せず、新規割当から新方針を適用する |
| B1-D04 | 搬入量は `min(physical capacity, target)` から stored、incoming、cycle shadow を控除する |
| B1-D05 | additive component のため container header v1 は維持し、missing component だけ既定補完する |
| B1-D06 | 特殊設備は `StockpilePolicy` 非保持のまま既存専用ロジックを使う |
| B1-D07 | `Draining` は `allow_export = false` を上書きして不一致在庫の搬出を許可する |
| B1-D08 | 旧セーブの通常セルは `BelongsTo` target が `Yard` である正の条件だけで識別する |
| B1-D09 | group を priority tier 別 request に分割し、通常経路と猫車 arbitration は同じ receiver policy tier から各経路の effective priority を導出する |
| B1-D10 | evaluator は新規搬入と committed 搬入を区別し、後者は変更後 policy を grandfather する |
| B1-D11 | consolidation は通常 `Priority(0)` と猫車 Low baseを維持したままreceiver policy modifierを加え、manual haulの明示priorityは上書きしない |
| B1-D12 | B1/B2 の方針値は i32 candidate priority へ畳み込まず、`WORKER_PRIORITY_WEIGHT / 40.0` unitの共有 additive offsetとして合成し、最終clampを行わない |

- Bevy 0.19 APIでの注意点:
  - component 追加・変更の可視化は既存 observer / `Changed<T>` の project 内実例を使う。
  - UI、Message、Query の新 API を導入する場合は Bevy 0.19 の一次資料またはローカル crate source で確認する。

## 5. マイルストーン

## M1: ポリシーモデル、共通 evaluator、旧セーブ移行

- 変更内容:
  - `StockpileAcceptance`、`StockpilePolicy`、patch、導出状態、拒否理由を `hw_logistics` に追加する。
  - evaluator に `NewInbound` / `CommittedInbound` / `NewOutbound` の phase と reservation ownership を追加する。
  - 通常 zone spawn と旧セーブ rehydrate に既定値を追加し、Yard owner の正の識別で特殊設備を除外する。
  - `schema.rs` の persisted / reflect inventory と schema tests を更新する。
- 変更ファイル:
  - `crates/hw_logistics/src/zone.rs`
  - `crates/hw_logistics/src/stockpile_policy.rs`
  - `crates/hw_logistics/src/lib.rs`
  - `crates/bevy_app/src/systems/command/zone_placement/placement.rs`
  - `crates/bevy_app/src/systems/save/schema.rs`
  - `crates/bevy_app/src/systems/save/rehydrate.rs`
  - `crates/bevy_app/src/systems/save/load.rs`
  - `crates/bevy_app/src/systems/save/rehydrate/tests/stockpile_policy.rs`
  - `crates/bevy_app/src/systems/save/schema/tests.rs`
- 完了条件:
  - [x] evaluator の境界値、異種在庫、目標 0 / capacity 超過、予約控除、committed reservation を unit test で固定した。
  - [x] old v0/v1 fixture は Yard-owned 通常セルだけを既定補完し、`BucketStorage` marker が欠落した
    Tank companion、Tank / Mixer root を変更しない。
  - [x] 新形式のセル別 policy が RON / Entity remap 後も往復する。
- 検証:
  - `cargo test -p hw_logistics stockpile_policy`
  - `cargo test -p bevy_app@0.1.0 --lib systems::save`

## M2: 搬入 producer・group・arbitration の統合

- 変更内容:
  - task-area producer の適格セルを priority tier 別に partition し、tier ごとの需要量と request を live policy snapshot から生成する。
  - request identity / upsert を priority tier 対応にし、既存 request の subset / priority / demand を再同期する。
  - 通常 task finder に共有 composition helper と Normal=0 の transport policy score offset を追加する。
  - manual haul の destination selector を `NewInbound` evaluator へ接続する。
  - arbitration の候補収集・grant 直前再検証を共通 evaluator へ寄せる。
  - mixed-owner group では通常 Familiar と猫車 grant の両方で、実 destination cell と source owner を結合する。
  - 猫車荷下ろしで owner 互換性を再確認し、owner 未設定 item を owner 付き通常セルへ格納した時点で ownership を確定する。
  - 重複 Yard と同一 cycle 内 reservation shadow を含む決定的な選択順を実装する。
- 変更ファイル:
  - `crates/hw_logistics/src/stockpile_policy.rs`
  - `crates/hw_logistics/src/transport_request/components.rs`
  - `crates/hw_logistics/src/transport_request/producer/task_area.rs`
  - `crates/hw_logistics/src/transport_request/producer/stockpile_group.rs`
  - `crates/hw_logistics/src/transport_request/producer/active_unit_cache.rs`
  - `crates/hw_logistics/src/transport_request/arbitration/mod.rs`
  - `crates/hw_logistics/src/transport_request/arbitration/candidates.rs`
  - `crates/hw_logistics/src/transport_request/arbitration/grants.rs`
  - `crates/hw_logistics/src/transport_request/arbitration/collection.rs`
  - `crates/hw_logistics/src/transport_request/arbitration/lease_state.rs`
  - `crates/hw_logistics/src/transport_request/arbitration/system.rs`
  - `crates/hw_logistics/src/transport_request/arbitration/types.rs`
  - `crates/hw_logistics/src/manual_haul_selector.rs`
  - `crates/bevy_app/src/systems/command/area_selection/apply.rs`
  - `crates/bevy_app/src/systems/command/area_selection/manual_haul.rs`
  - `crates/bevy_app/src/systems/command/area_selection/queries.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/policy_score.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/context.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/mod.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/task_finder/mod.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/delegation/assignment_loop.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/validator/resolver.rs`
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/haul_with_wheelbarrow/phases/unloading.rs`
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/haul_with_wheelbarrow/phases/unloading/item_ops.rs`
- 完了条件:
  - [x] 不適合、目標到達、物理満杯のセルへ request / grant が発生しない。
  - [x] 重複 Yard でも同じ空き枠を二度予約しない。
  - [x] High 1 枠 + Low 複数枠では、High 枠を埋めた後の搬送が High bias を引き継がず Low request として評価される。
  - [x] Normal policy の通常候補 score / Top-K は現行値と一致し、同条件では既存 priority が上限20へ達した候補でも
    Low < Normal < High < Critical になる。
  - [x] transport unit mapping は -10 / 0 / +10 / +20 と現行 priority slope の積に一致する。
  - [x] shared helper は `base + transport + familiar` の加算順に依存せず、各 Normal=0 と最終 no-clamp を守る。
    B2 未実装時は synthetic familiar contribution、B2 実装後は実 enum mapping との統合テストで固定する。
  - [x] transport と familiar の最小・最大を合算した score span が `WORKER_PRIORITY_WEIGHT` を超えない。
  - [x] transport の隣接 tier 差は `WORKER_DISTANCE_WEIGHT` より小さく、距離の最大差を常に上書きする hard tier にならない。
  - [x] candidate に保持した contribution が Top-K と fallback の両方へ同じ一回だけ適用され、base scorer の 0.65 / 0.35
    合成と tie-break は変わらない。
  - [x] 通常搬送と猫車搬送が同じ request priority の単調順序を守る。
  - [x] mixed-owner group でも、通常搬送と猫車 grant は選択 source を別 owner の destination cell へ再ルーティングしない。
  - [x] 猫車で owner 未設定 item を owner 付き通常セルへ荷下ろしすると `BelongsTo` が確定し、別 owner は拒否される。
  - [x] policy / contents / incoming が不変な tick で group cache と arbitration index を再構築しない。
- 検証:
  - `cargo test -p hw_logistics transport_request`
  - `cargo test -p hw_logistics stockpile_group`
  - `cargo test -p hw_familiar_ai task_management`
  - `cargo test -p hw_soul_ai unloading`
  - `cargo test -p bevy_app@0.1.0 area_selection`

## M3: AI validator、実行系、搬出・draining の統合

- 変更内容:
  - Familiar の request resolver と Soul の通常 haul / wheelbarrow 実行系を phase-aware evaluator へ接続する。
  - consolidation producer は donor を `NewOutbound`、receiver を `NewInbound` で評価する。receiver の acceptance、target、
    physical capacity、incoming / shadow のいずれかが不適格なら request を作らず、transfer amount も evaluator 上限へ clamp する。
  - donor の `allow_export` と `Draining` override を同じ evaluator で共有し、request priority は既存 Low base へ
    receiver policy の Normal=0 modifier を適用する。
  - assignment → policy change → unload を通し、committed delivery は完了、新規 assignment は停止する lifecycle test を追加する。
- 変更ファイル:
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/validator/resolver.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/validator/capacity_helpers.rs`
  - `crates/hw_logistics/src/transport_request/producer/consolidation.rs`
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/stockpile_policy.rs`
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/context/queries.rs`
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/haul/`
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution/haul_with_wheelbarrow/`
  - `crates/hw_soul_ai/src/soul_ai/execute/task_execution_system/tests/stockpile_policy.rs`
- 完了条件:
  - [x] assignment 後の policy / target 変更は committed delivery を拒否せず、満杯・削除・内容競合だけを
    既存 disposition で安全に処理する。
  - [x] draining 中の既存在庫は失われず、設定値にかかわらず搬出元へ選べる。適合在庫は搬出許可を尊重する。
  - [x] consolidation は target 到達・受入不一致の receiver へ request / grant を作らない。
  - [x] consolidation は default policy で現行 Low を維持し、同条件では receiver policy の上昇を単調に反映する。
  - [x] 通常搬送と猫車搬送の policy 結果が一致する。
- 検証:
  - `cargo test -p hw_familiar_ai task_management`
  - `cargo test -p hw_soul_ai task_execution`
  - `cargo test -p hw_logistics consolidation`

## M4: 単一セル・矩形一括編集 UI

- 変更内容:
  - inspection ViewModel、editor、範囲 mode、UiIntent、root handler、`StockpilePolicyChangeOutcome` を追加する。
  - stale Entity、特殊設備、混在 selection、clamp の部分適用結果を A2 通知へ接続する。
- 変更ファイル:
  - `crates/hw_logistics/src/stockpile_policy_change.rs`
  - `crates/hw_ui/src/intents.rs`
  - `crates/hw_ui/src/panels/info_panel/`
  - `crates/bevy_app/src/interface/selection/`
  - `crates/bevy_app/src/interface/ui/presentation/`
  - `crates/bevy_app/src/interface/ui/interaction/`
  - `crates/bevy_app/src/interface/ui/plugins/notifications.rs`
  - `crates/bevy_app/src/plugins/messages.rs`
  - `crates/bevy_app/src/systems/command/`
- 完了条件:
  - [x] widget は domain component を直接 mutate しない。
  - [x] 単一編集と範囲編集が同じ validation / outcome 経路を通る。
  - [x] draining、現在量、目標量、受入、搬入優先度、搬出許可を画面から識別できる。
- 検証:
  - `cargo test -p hw_ui stockpile`
  - `cargo test -p bevy_app@0.1.0 stockpile_policy`

## M5: 横断回帰、性能、恒久ドキュメント

- 変更内容:
  - save/load、overlapping Yard、in-flight、manual destination / normal / wheelbarrow の固定シナリオを統合する。
  - `docs/logistics.md`、`docs/info_panel_ui.md`、`docs/save_load.md`、`docs/invariants.md`、
    必要なら `docs/architecture.md` / `docs/cargo_workspace.md` を実装へ同期する。
- 変更ファイル:
  - `crates/*/src/**/tests.rs`
  - `docs/logistics.md`
  - `docs/info_panel_ui.md`
  - `docs/save_load.md`
  - `docs/invariants.md`
- 完了条件:
  - [x] Track B1 の受入シナリオが固定 tick で成功する。
  - [x] UI を開かない steady state の producer / arbitration work が増えない。
  - [x] 恒久 docs と生成索引が最新で、本計画を archive できる。
- 検証:
  - `python3 scripts/dev.py docs --write`
  - `python3 scripts/dev.py verify`
  - `git diff --check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 判定を一部経路だけへ追加する | UI 表示と実搬送、通常搬送と猫車搬送が食い違う | 共通 evaluator を唯一の policy 判定にし、producer・grant・execution の三段階を横断テストする |
| 目標量と物理容量を混同する | 予約超過、または容量があるのに永久停止する | 両方の残容量と incoming / shadow を別値で計算し、最小値を割当上限にする |
| group cache が contents / policy 変更後に stale になる | 誤った需要と優先度が残る | membership と live demand を分離し、必要な変更検知と zero-work test を固定する |
| 旧セーブ補完が特殊設備へ付く | Tank / Mixer の専用搬送が壊れる | 欠落し得る marker ではなく Yard owner の正の識別を使い、旧 save fixture を作る |
| 方針変更後の evaluator が committed 搬送も拒否する | 品物消失、幽霊予約、二重搬送 | phase と reservation ownership を渡し、committed 搬入を grandfather する |
| 搬入優先度が猫車だけへ反映される | 通常搬送と猫車の順序が食い違う | 同じ receiver policy tier から通常 offset と猫車 effective priority を導出し、Normal=0 回帰を固定する |
| group 最大優先度を request へ畳み込む | High 枠消費後も Low セルが High bias を引き継ぐ | tier 別 request / subset に分け、shadow で tier 容量を使い切る回帰を固定する |
| B1/B2 の offset を既存 i32 priority へ加える | priority 20 の clamp で High / Critical が同点になる | 既存 worker score 後の共有 additive offset とし、組合せテストで no-clamp と単調性を固定する |
| Entity ID tie-break が load 後に変わる | 同条件で搬入順が変わる | grid 座標等の durable key を優先し、Entity は最終 fallback のみにする |

## 7. 検証計画

- 必須:
  - evaluator の pure unit tests。
  - v0/v1 missing-policy migration と新 save round-trip。
  - 重複 Yard、異種在庫、目標減少、export 禁止、in-flight 変更、同一 cycle 多重 request。
  - assignment → policy change → committed unload と、物理競合時の retry / cancellation。
  - 通常搬送 / manual destination / wheelbarrow / consolidation の整合、priority 単調性、High 枠消費後の tier 降格。
  - A3 Critical + Build 補正相当の base score 上限、B1 Critical、B2 High を組み合わせた shared offset の単調性と加算順非依存。
  - `cargo check --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 計画完了時:
  - `cargo test --workspace`
  - `python3 scripts/dev.py docs --write`
  - `python3 scripts/dev.py verify`
  - `git diff --check`
- 手動確認シナリオ:
  - Bone 入りセルを Wood only に変え、Bone が消えず搬入だけ止まり、搬出後に Wood を受けることを確認する。
  - 重複 Yard 内で範囲設定し、セルが一度だけ更新され、通知件数と実セル数が一致することを確認する。
  - 目標量を現在量より下げ、実行中搬送は安全に完了し、その後の新規搬入が停止することを確認する。
- パフォーマンス確認:
  - policy UI の開閉で producer / arbitration cycle 数が変わらない。
  - 変更なし tick で policy 起因の全 Stockpile 再走査が発生しない。

## 8. ロールバック方針

- M1 の additive component、M2/M3 の evaluator 接続、M4 の UI を別変更単位に保つ。
- UI だけ戻す場合も policy と既定値は残せる。allocator 接続を戻す場合は全 call site を同時に旧判定へ戻し、
  混在状態を作らない。
- 新セーブを旧 executable が読めない場合は registry deserialize / `InvalidData` で live apply 前に拒否し、
  未知 component を黙って捨てない。
- rollback 時も在庫や relationship を一括削除せず、既存 lifecycle で world を保持する。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `計画 100% / 実装 100%`
- 完了済みマイルストーン: `M1`、`M2`、`M3`、`M4`、`M5`
- 未着手/進行中: なし

### 次のAIが最初にやること

1. B1 は完了済み。追加変更では本書のセル正本・共通evaluator・committed搬送契約を維持する。
2. B2 を進める場合は、B1で追加したshared policy score compositionを再利用する。

### ブロッカー/注意点

- `Stockpile.resource_type` は現在内容であり、policy に置換しない。
- `StockpileGroup` は重複可能な派生集合であり、policy の正本にしない。
- Tank、Mixer、`BucketStorage` へ通常 Stockpile policy を自動付与しない。旧 save では `BucketStorage` がない前提で Yard owner を見る。
- manual haul は現行では地面上 item の搬入操作であり、Stockpile 内容の搬出 UI と誤認しない。
- execution では `CommittedInbound` を使い、新 policy を新規搬入と同じ条件で再適用しない。
- transport / Familiar policy を既存 i32 candidate priority へ加えない。既存 worker score 後の共有 offset を使い、最終 score を clamp しない。
- consolidation の通常 score に raw request Low を直接変換せず、receiver policy modifier と既存 maintenance base を分離する。
- 方針変更時に `StoredIn`、`DeliveringTo`、`IncomingDeliveries` を UI handler から直接変更しない。
- 既存の未コミット変更がある場合は、その所有者の変更を保持して狭い差分で実装する。

### 参照必須ファイル

- `docs/proposals/gameplay-management-improvements-proposal-2026-07-17.md`
- `docs/logistics.md`
- `docs/save_load.md`
- `docs/invariants.md`
- `docs/info_panel_ui.md`
- `crates/hw_logistics/src/zone.rs`
- `crates/hw_logistics/src/transport_request/producer/stockpile_group.rs`
- `crates/hw_logistics/src/transport_request/producer/task_area.rs`
- `crates/hw_logistics/src/transport_request/arbitration/`
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/validator/resolver.rs`
- `crates/hw_soul_ai/src/soul_ai/execute/task_execution/`
- `crates/bevy_app/src/systems/save/schema.rs`

### 最終確認ログ

- 最終 `cargo check --workspace --locked`: 成功（`python3 scripts/dev.py verify` 内）
- 最終 `cargo clippy --workspace --all-targets --locked -- -D warnings`: 成功（警告 0 件）
- 最終 `cargo test --workspace --locked`: 成功（`python3 scripts/dev.py verify` 内）
- M1 focused tests: `cargo test -p hw_logistics stockpile_policy --locked`、
  `cargo test -p bevy_app@0.1.0 --lib systems::save --locked` 成功
- M2 focused tests: `cargo test -p hw_logistics transport_request --locked`、
  `cargo test -p hw_familiar_ai task_management --locked`、
  `cargo test -p hw_soul_ai unloading --locked`、
  `cargo test -p bevy_app@0.1.0 area_selection --locked` 成功
- M3 focused tests: `cargo test -p hw_logistics consolidation --locked`、
  `cargo test -p hw_familiar_ai task_management --locked`、
  `cargo test -p hw_soul_ai task_execution --locked` 成功
- M4 focused tests: `cargo test -p hw_ui stockpile --locked`、
  `cargo test -p hw_logistics stockpile_policy_change --locked`、
  `cargo test -p bevy_app@0.1.0 stockpile_policy --locked` 成功
- M4 app regression: `cargo test -p bevy_app@0.1.0 --lib --locked` 成功（235件）
- M5 focused tests: manual area request生成、重複Yard live需要、猫車grant時容量shadow、
  consolidation割当前再検証、typed policy変更後のcommitted/unreserved搬送、UI開閉の60Hz・40 tick比較が成功
- M5 performance regression: 同一fixtureのcontrolとpanel開閉＋no-op編集でproducer / arbitration整数work量、
  group rebuild回数、arbitration generation増分が一致
- 最終 `python3 scripts/dev.py verify`: M1〜M5 反映後に成功（2026-07-22、bevy_app 237件を含むworkspace全test）
- 未解決エラー: なし

### Definition of Done

- [x] M1〜M5 が完了
- [x] 通常セルと特殊設備の policy 境界を自動テストで保証
- [x] 全搬送経路が共通 evaluator を使用
- [x] priority tier 別 request が High 枠消費後に Low / Normal へ値を漏らさず、B1/B2 shared offset が上限 score でも単調
- [x] 旧セーブ移行と新セーブ往復が成功
- [x] UI と `StockpilePolicyChangeOutcome` notification が単一・範囲編集を説明
- [x] `python3 scripts/dev.py verify` が成功
- [x] 恒久 docs 更新後に本計画を archive

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-07-20` | `Codex` | Track B1 のセル正本、初版 policy、draining、予約・搬送横断、旧セーブ移行、UI 一括編集を計画化 |
| `2026-07-21` | `Codex` | B1/B2 の方針優先度を既存 i32 priority から分離し、既存 worker score 後の共有 no-clamp offset と組合せ回帰を固定 |
| `2026-07-22` | `Codex` | M1 の durable policy、共通 evaluator、通常セル spawn、v0/v1 補完、新形式 round-trip を実装 |
| `2026-07-22` | `Codex` | M2 の tier 別 request、manual / wheelbarrow evaluator、owner fallback、mixed-owner destination/source 結合、荷下ろし owner 確定、semantic-diff upsert、決定的生成、共有 no-clamp score offset を実装 |
| `2026-07-22` | `Codex` | M3 の Familiar live resolver、通常 / wheelbarrow committed execution、NewOutbound / draining 対応 consolidation、lifecycle 回帰を実装 |
| `2026-07-22` | `Codex` | M4 の managed cell選択・inspection/editor、単一/矩形共通typed request、安定対象解決、部分適用outcome、ToastOnly通知、world replacement reset回帰を実装 |
| `2026-07-22` | `Codex` | M5 の固定tick横断回帰、同一fixture性能比較、恒久docs同期、全workspace検証を完了し、本計画をarchive |
