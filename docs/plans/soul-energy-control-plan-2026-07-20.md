# Track B3 Soul Energy 制御 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `soul-energy-control-plan-2026-07-20` |
| ステータス | `Draft` |
| 作成日 | `2026-07-20` |
| 最終更新日 | `2026-07-21` |
| 作成者 | `Codex` |
| 関連提案 | `docs/proposals/gameplay-management-improvements-proposal-2026-07-17.md`（Track B3） |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: Soul Spa の稼働枠を操作できず、供給不足時の grid が全設備を一律停止する。
  - `SoulSpaSite.active_slots` は保存され AI の新規割当 gate に使われるが、プレイヤーが変更する UI と
    同一 decision cycle 内の pending assignment 控除がない。
  - 現行 Power grid は総需要が供給を超えると全 Consumer を一律 `Unpowered` にし、重要設備を優先できない。
  - 発電量、総需要、配線、個別の給電状態、遮断理由を同じ inspection 経路から説明できない。
  - `PowerGrid` の集計値と `Unpowered` は load 後に再計算される導出状態だが、現行 schema では保存対象に含まれる。
- 到達したい状態:
  - Soul Spa の稼働枠を 0〜4 で設定でき、減少時に作業中 Soul を追い出さず新規割当だけを止める。
  - 供給不足時は consumer priority と安定キーに従う決定的な prefix allocation を行い、個別設備だけを遮断する。
  - 不足時は即時遮断し、復旧時は hysteresis margin を満たすまで再投入しないため境界付近で反転しない。
  - dirty 時だけ再配分し、UI は runtime read model を読むだけで simulation work を増やさない。
- 成功指標:
  - `active_slots` を超える新規 `GeneratePower` assignment が同じ cycle 内にも発生しない。
  - 同じ入力・座標・policy では save/load 後も同じ consumer 順で給電される。
  - grid が部分供給でき、各 consumer の `Supplied` / `Shed` / `Disconnected` と理由を UI で識別できる。
  - new spawn / load / rollback 後も Yard ごとの canonical PowerGrid が厳密に1件で、全 connection がそこを参照する。
  - energy input が不変な tick では allocation system の実行回数が増えない。

## 2. スコープ

### 対象（In Scope）

- 既存 `SoulSpaSite.active_slots` の 0〜4 clamp、info panel 操作、保存往復。
- 稼働中数と設定枠の表示、枠減少時の no-kick / draining semantics。
- 同一 Familiar assignment cycle の pending Soul Spa slot shadow。
- 永続 `PowerConsumerPolicy` と Low / Normal / High の優先度。
- user-local `GameSettings.power_priority_enabled` と、無効時に現行 all-or-none 配電を再現する
  `PowerAllocationMode::LegacyAllOrNone`。
- priority、安定した空間 key、需要による純粋な prefix allocator。
- 不足時の即時 shed と、復旧時の named hysteresis margin。
- runtime `PowerSupplyState`、`PowerGridAllocationSummary`、`Unpowered` mirror の同期。
- grid / generator / consumer / relationship / policy change と removal による dirty-driven 再計算。
- Yard / PowerGrid の一対一 lifecycle reconciliation、duplicate / orphan cleanup、connection repair。
- 発電、総需要、供給済み需要、配線、priority、遮断理由の共通 inspection。
- 旧セーブの consumer policy 補完と runtime-derived state の load rebuild。
- `SoulSpaTile.parent_site` を正本とする占有数・発電出力集計。表示用 `ChildOf` には依存しない。

### 非対象（Out of Scope）

- Battery、蓄電、充放電優先度、複数 grid 間の融通。
- HVAC / Plumbing 自体の実装、Room / FluidGrid / paired site の設計変更。
- 発電 Soul の直接選択、Soul Spa タイルごとの個別予約 UI。
- 需要予測、時間帯 schedule、割合給電、consumer の部分出力。
- 電線 topology の再設計、Yard を跨ぐ grid merge、任意の手動配線。
- priority による物理 demand 値の書き換え。
- PowerGrid / Yard Entity 自体の world selection 導線。初版は選択済み Soul Spa / consumer から所属 grid を表示する。

## 3. 現状とギャップ

- `SoulSpaSite.active_slots` は default 4 で persisted component に含まれる。AI は現在の `TaskWorkers` 数を数えるが、
  同じ delegation cycle で送信済み・未適用の `TaskAssignmentRequest` を数えないため、複数 tile へ過剰割当し得る。
- `has_available_slot` は新規 assignment だけを gate する。この性質を保ち、枠を下げても現在 worker を剥がさない。
- `grid_recalc_system` は generation と全 consumer demand を合計し、`generation >= consumption` の真偽を
  全 consumer の `Unpowered` へ一律同期する。
- energy pipeline は `EnergyUpdateDirty` と明示 ordering を既に持ち、steady state zero-work test がある。
  B3 はこの wake-up 境界を拡張し、output / allocation を毎 tick 実行へ戻さない。一方 `lamp_buff_system` は
  `SlowSimulationClock` step ごとに継続効果を更新するため、その candidate work は zero にならない。
- `ConsumesFrom` が外れた consumer は、grid iteration の外へ出るため `Unpowered` を再付与されない可能性がある。
  disconnected normalization を allocation と同じ dirty transaction に含める必要がある。
- `PowerGrid` は relationship target を保持する durable entity の root markerでもある。初版では component 自体と既存3 fieldを
  compatibility mirror として残すが、値は load 後の full rebuild まで信頼しない。詳細な割当正本は非保存 summary とする。
- 現行 `on_yard_added` は `Add<Yard>` ごとに grid を無条件 spawn する。DynamicWorld load / rollback でも Yard の Add observer が
  発火し、保存済み `PowerGrid` と observer 生成 grid が二重化し得るため、即時生成を一対一 lifecycle reconciliation へ置き換える。
- `Unpowered` は効果 system が読む runtime mirror であり、旧 body からは legacy runtime-derived component として除去し、
  full rebuild で復元する。未知 schema を黙って受理するための header 緩和は行わない。
- `SoulSpaTile` の durable `parent_site` は保存されるが、表示 hierarchy の `ChildOf` は復元されない。
  現行 `soul_spa_power_output_system` の `&Children` 必須 query は load 後に site を取りこぼし、保存済み output を
  stale のまま残すため、B3 で `parent_site` 集計へ置き換える。
- `WorkingOn` / `TaskWorkers` は保存される一方、load 後の `AssignedTask` は `None` から始まる。state-sanity の stale relationship
  removal は Commands であり、現行 energy chain と Update → Decide 間の flush は相互順序がない。最初の Logic で旧 worker を
  発電数へ含めないよう、cleanup flush を名前付き境界にして energy pipeline をその後へ固定する。

## 4. 実装方針（高レベル）

### 4.1 Soul Spa 稼働枠

- `SOUL_SPA_MAX_ACTIVE_SLOTS = 4` を `hw_energy` に置き、新規値、UI intent、load normalization を
  `0..=SOUL_SPA_MAX_ACTIVE_SLOTS` へ clamp する。
- 稼働表示は `occupied / configured` を分ける。枠減少で occupied が上回る間は
  `Draining (4 active / 2 configured)` と表示する。
- 枠減少は `TaskWorkers`、designation、`AssignedTask` を変更しない。各 Soul は既存の Dream / fatigue / task completion
  条件で終了し、occupied が枠未満になってから新規 assignment を許す。
- `ReservationShadow` に site ごとの pending generate-power count を追加し、
  `occupied + pending < active_slots` のときだけ request を submit する。submit 成功時だけ shadow を増やす。
- occupied と発電出力は `SoulSpaTile.parent_site` で一走査集計し、`Children` の有無を論理条件にしない。
  load / rollback fixture は hierarchy なし・worker 0 の operational site で output が 0 へ再計算されることを固定する。

### 4.2 固定する consumer policy と runtime state

```rust
pub enum PowerPriority {
    Low,
    Normal,
    High,
}

pub struct PowerConsumerPolicy {
    pub priority: PowerPriority,
}

pub enum PowerSupplyState {
    Supplied,
    Shed { reason: PowerShedReason },
    Disconnected,
    InvalidDemand,
}

pub enum PowerAllocationMode {
    LegacyAllOrNone,
    PriorityPrefix,
}
```

- `PowerConsumerPolicy` だけを persisted component とし、旧 consumer には `Normal` を補う。
- `PowerSupplyState`、`PowerGridAllocationSummary`、`Unpowered` は runtime-derived とし保存しない。
- Outdoor Lamp の初期 priority は `Normal`。将来の HVAC consumer は同じ型と allocator を使い、設備ごとの default は
  HVAC 側で選ぶ。
- demand が負または非有限なら合計を汚染せず `InvalidDemand + Unpowered` とし、debug/test で検出する。
- `PowerAllocationMode` は runtime resource とし、world save には含めない。`GameSettings.power_priority_enabled` から同期し、
  `true` は `PriorityPrefix`、`false` は `LegacyAllOrNone` とする。既存 `settings.ron` に field がない場合は `true` を補う。
  `GameSettingsFile.power_priority_enabled` には field-level の `serde` default=true を置き、旧 file の UI scale 等の非既定値を
  保持したまま新 field だけを補完する。file 全体の parse failure / `GameSettings::default()` fallback を migration に使わない。
  mode resource は値が実際に変わる場合だけ更新し、他の settings 変更で energy dirty を起こさない。

### 4.3 決定的な prefix allocation

- `hw_energy` に ECS 非依存の pure allocator を置き、`PowerAllocationMode` を明示入力にする。
- `LegacyAllOrNone` は有効な全 consumer demand が generation 以下なら全件 `Supplied`、超えるなら全件を
  `Shed { reason: LegacyGlobalDeficit }` とし、現行 all-or-none 挙動を個別 runtime state 上で再現する。
  invalid / disconnected の正規化は mode にかかわらず共通とする。
- `PriorityPrefix` は入力を次の順に安定 sort する。
  1. `PowerPriority`: High、Normal、Low
  2. consumer の grid 座標: y、x
  3. Entity bits は同一セル重複に対する最終 fallback のみ
- 初版 consumer は tile 上に一意配置される Outdoor Lamp を対象とし、同一 grid cell の複数 consumer を許可しない
  placement invariant をテストする。将来これを許可する場合は durable order key を別計画で追加する。
- sort 後の先頭から cumulative demand を足し、収まる連続 prefix だけを供給する。途中の大型 consumer を飛ばして
  後続の小型 consumer を点灯する bin-packing は行わない。
- `f32::EPSILON` を直接 gameplay margin にせず、比較用 epsilon と復旧用 `POWER_RESTORE_MARGIN` を分ける。
- 供給低下で現在 prefix が収まらなければ即時に末尾から shed する。以前 shed だった consumer を prefix へ戻すのは、
  その consumer までの cumulative demand と restore margin を generation が満たしたときだけにする。
- `PowerSupplyState` がない cold start（new spawn、load、reconnect）は hysteresis 履歴なしとして raw capacity prefix を
  即時供給し、exact capacity も `Supplied` とする。runtime latch は保存しないため load は hysteresis wait を解除し得るが、
  同じ durable input から毎回同じ順序・state を再構築する。既知の `Shed` からの復帰時だけ margin を要求する。
- policy / topology 変更でも最終 state は常に strict prefix とする。priority 上昇で上位へ移った設備を供給できない場合、
  旧下位設備を先に shed し、hysteresis 条件を満たすまで両方を shed し得ることを仕様として表示する。
- mode 切替は energy dirty を1回立て、変更を観測した最初の Logic frame の effect より前に全 consumer state と summary を
  新 mode へ再構築する。
  `LegacyAllOrNone` では priority と hysteresis latch を配電判断に使わず、再度 `PriorityPrefix` へ戻した時は cold start として
  raw prefix を再構築する。

### 4.4 ECS pipeline と互換 mirror

```text
Soul Update / state-sanity cleanup
  -> named state-sanity ApplyDeferred
  -> sync PowerAllocationMode from GameSettings (実値変更時だけ dirty)
  -> detect energy/topology dirty
  -> Yard <-> PowerGrid one-to-one reconciliation + connection repair
  -> topology ApplyDeferred
  -> Soul Spa output
  -> grid allocation + disconnected normalization
  -> supply-state ApplyDeferred
  -> lamp and future consumer effects
```

- dirty source に `Added/Changed/Removed<PowerConsumerPolicy>`、runtime state 欠落、relationship removal、実値が変わった
  `PowerAllocationMode` を加える。
- Yard / Grid / generator / consumer observer は topology dirty を立てるだけにし、observer callback から grid を無条件 spawn しない。
  lifecycle reconciler は `YardPowerGrid(yard)` を durable key として Yard ごとに grid を厳密に1件へする。0件なら作成し、1件なら再利用する。
  複数なら既存 `GeneratesFor` / `ConsumesFrom` の参照を持つ grid を優先し、最後は安定 Entity key で canonical を選び、全 connection を
  canonical へ付け替えて duplicate を despawn する。orphan grid も同じ transaction で除去する。
- lifecycle reconciliation 後の flush で grid / relationship を可視化してから output / allocation を実行する。これにより通常の Yard / consumer
  同時追加と DynamicWorld load / rollback が同じ経路を通り、consumer observer の実行順に接続結果を依存させない。
- grid allocator は consumer ごとに `PowerSupplyState` を同期し、`Supplied` のときだけ `Unpowered` を外す。
  `Shed` / `Disconnected` / `InvalidDemand` では `Unpowered` を挿入する。
- `ConsumesFrom` がない consumer と、参照 grid が消えた consumer は allocation と同じ dirty transaction で
  `Disconnected` へ正規化し、両 system の Commands を共通 `ApplyDeferred` より前に enqueue する。
  `PowerGridAllocationSummary` の direct mutation / publish もこの flush 前に完了させ、effect は flush 後だけに置く。
- `PowerGrid.powered` は互換上「全需要が供給済み」を意味し、部分供給中は false とする。UI 表示は BLACKOUT ではなく
  `Fully supplied` / `Load shedding` を使い、runtime summary の served demand と shed count を併記する。
- load / rollback 後は `EnergyUpdateDirty::request_full_rebuild()` を呼び、effect system より前に runtime state を完成させる。
- Soul state-sanity が stale `WorkingOn` を除去する Commands は名前付き Update → Decide 間 barrier で flush し、energy pipeline 全体を
  その barrier より後へ置く。load fixture では `AssignedTask::None` + 保存済み `WorkingOn/TaskWorkers` が output 集計前に消えることを固定する。
- `Unpowered` を新 save allow-list から外す操作は、親提案で許可する「legacy runtime-derived state の v1 body normalization」
  だけを適用対象とする。`schema.rs` では `Unpowered` を `for_each_persisted_component!` から既存の
  `for_each_runtime_derived_component!` へ移し、loader registry と `discard_runtime_derived_components()` の validation 前 strip
  経路を再利用する。durable field は削除・変換せず、effect より前の full rebuild と、既存 v0/v1 body を schema reject
  させず新 v1 resave からは除外する fixture を追加する。

### 4.5 UI と inspection

- Soul Spa info panel は Operational site だけで `active_slots` の -/+ または明示 set intent を有効にし、root handler が
  entity / phase / clamp を再検証する。Constructing site への stale / forged intent は `PhaseUnavailable` として変更しない。
  handler は必ず `SoulSpaSlotsChangeOutcome` を1件返し、`Applied { requested, applied, clamped }`、`StaleTarget`、
  `UnsupportedTarget`、`PhaseUnavailable` を区別する。clamp は適用成功の warning、failure は非変更の warning とし、
  A2 notification adapter が同じ Update で player-safe な通知へ変換する。
- consumer info panel は demand、priority、connection、supply state、shed reason を表示し、priority intent を出す。
- 選択済み Soul Spa または consumer building の inspection は `GeneratesFor` / `ConsumesFrom` から所属 grid summary を引き、
  generation、total demand、served demand、reserve/deficit、consumer 数、shed 順を表示する。
- widget は ECS を直接 mutate せず、既存 `UiIntent -> root handler -> domain state -> ViewModel` 境界を使う。
- UI は `PowerGridAllocationSummary` を読むだけとし、panel open 時に grid 探索・allocation を実行しない。
- Settings は `Power priority allocation` checkbox から `GameSettings.power_priority_enabled` を変更し、inspection は現在の
  `PriorityPrefix` / `Legacy all-or-none` mode を表示する。widget は allocator を直接呼ばない。

### 4.6 設計判断

| ID | 判断 |
| --- | --- |
| B3-D01 | `active_slots` 減少は no-kick。新規割当だけを gate する |
| B3-D02 | 同一 cycle の pending assignment を site 単位 shadow で数える |
| B3-D03 | allocator は priority + spatial stable key の strict prefix。bin-packing しない |
| B3-D04 | deficit は即時 shed、restore は hysteresis margin 後 |
| B3-D05 | consumer policy は durable、個別 supply state / grid summary / `Unpowered` は runtime-derived |
| B3-D06 | `PowerGrid.powered` は互換上 all-served を表し、部分給電の正本は runtime summary |
| B3-D07 | Battery は B3 と HVAC consumer の運用確認後まで導入しない |
| B3-D08 | Soul Spa occupancy / output は durable `SoulSpaTile.parent_site` で集計し、`ChildOf` を要求しない |
| B3-D09 | state 欠落の cold start は margin なしで raw prefix を供給し、既知の shed 復旧だけ hysteresis を使う |
| B3-D10 | direct Grid/Yard selection は追加せず、選択可能な site / consumer から所属 grid summary を表示する |
| B3-D11 | Yard/Grid observer は dirty 通知だけを行い、lifecycle reconciler が Yard 1件 : PowerGrid 1件を保証する |
| B3-D12 | energy pipeline は state-sanity Commands の名前付き flush 後に始め、load 直後の stale `TaskWorkers` を数えない |
| B3-D13 | Power priority は user-local setting で無効化でき、無効時は個別 state 上で現行 all-or-none 配電を再現する |
| B3-D14 | v1 内で除去できるのは明示登録・pre-validation strip・effect前再構築を満たす legacy runtime-derived state だけで、durable state は対象外 |

- Bevy 0.19 APIでの注意点:
  - relationship removal、required component、`ApplyDeferred` の可視化順は既存 0.19 実装を基準にテストする。
  - observer / Query / Message の新 API を使う場合は Bevy 0.19 の docs.rs またはローカル source を確認する。

## 5. マイルストーン

## M1: Soul Spa 稼働枠 UI と assignment 上限

- 変更内容:
  - active slot 定数、clamp / normalization、UiIntent、handler、ViewModel、専用 `SoulSpaSlotsChangeOutcome` と
    `UserFacingNotification` adapter を追加する。
  - outcome は applied / clamped / stale / unsupported / phase unavailable を区別し、失敗時に ECS を変更しない。
  - occupied / configured / draining 表示と no-kick contract を固定する。
  - `ReservationShadow` に site 単位 pending assignment を追加する。
  - occupied / power output の集計を `SoulSpaTile.parent_site` 正本へ統一し、`Children` dependency を外す。
- 変更ファイル:
  - `crates/hw_energy/src/constants.rs`
  - `crates/hw_energy/src/soul_spa.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/context.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/task_assigner.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/policy/basic.rs`
  - `crates/bevy_app/src/systems/energy/power_output.rs`
  - `crates/hw_ui/src/intents.rs`
  - `crates/hw_ui/src/components.rs`
  - `crates/hw_ui/src/setup/panels.rs`
  - `crates/hw_ui/src/panels/info_panel/`
  - `crates/bevy_app/src/interface/ui/presentation/builders.rs`
  - `crates/bevy_app/src/interface/ui/interaction/handlers/`
  - `crates/bevy_app/src/interface/ui/plugins/notifications.rs`
  - `crates/bevy_app/src/plugins/messages.rs`
- 完了条件:
  - [ ] active slots は UI / load / domain の全入口で 0〜4 に収まる。
  - [ ] 同一 cycle に空き 1 枠へ 2 Soul を submit しない。
  - [ ] 枠減少で現在 worker を外さず、occupied が減るまで新規 assignment を止める。
  - [ ] `ChildOf` なしの load / rollback でも `parent_site` から occupied / output を再計算する。
  - [ ] worker 0 の operational site は stale な保存 output を 0 に戻し、active slots と表示も保存値に一致する。
  - [ ] slot intent 1件につき outcome 1件を返し、exact apply / clamp / stale / unsupported / phase unavailable が
    同じ Update の notification adapter を通る。failure では `active_slots` を変更しない。
- 検証:
  - `cargo test -p hw_energy soul_spa`
  - `cargo test -p hw_familiar_ai generate_power`
  - `cargo test -p bevy_app@0.1.0 soul_spa`

## M2: consumer policy と pure allocator

- 変更内容:
  - `PowerPriority`、`PowerConsumerPolicy`、`PowerAllocationMode`、allocation input/output、state / reason を追加する。
  - legacy all-or-none、strict prefix、stable ordering、epsilon、restore hysteresis を ECS 非依存テストで固定する。
  - default policy と schema registration、old-save missing-policy migration を追加する。
- 変更ファイル:
  - `crates/hw_energy/src/components.rs`
  - `crates/hw_energy/src/allocation.rs`
  - `crates/hw_energy/src/constants.rs`
  - `crates/hw_energy/src/lib.rs`
  - `crates/bevy_app/src/systems/save/schema.rs`
  - `crates/bevy_app/src/systems/save/rehydrate.rs`
- 完了条件:
  - [ ] High > Normal > Low と y/x tie-break が入力順に依存しない。
  - [ ] supply deficit、exact boundary、invalid demand、empty grid の結果が固定される。
  - [ ] generation が境界を往復しても restore margin 内では state が反転しない。
  - [ ] cold start / reconnect の exact capacity は margin 待ちせず供給し、既知の shed 復旧だけ margin を要求する。
  - [ ] `LegacyAllOrNone` は同じ finite input に対して現行の全件供給 / 全件遮断境界と一致し、priority 順を参照しない。
  - [ ] 旧 consumer は Normal を得て、新 save は priority を往復する。
- 検証:
  - `cargo test -p hw_energy allocation`
  - `cargo test -p bevy_app@0.1.0 --lib systems::save`

## M3: dirty-driven grid 統合と runtime state 再構築

- 変更内容:
  - all-or-none grid recalc を個別 allocation へ置き換える。
  - Yard/Grid/consumer observer を dirty notification 化し、Yard↔Grid の一対一 reconciliation と duplicate / orphan cleanup、connection repair を追加する。
  - Soul state-sanity 後の `ApplyDeferred` を名前付き ordering 境界にし、energy pipeline を必ずその後へ置く。
  - dirty source、individual state / `Unpowered` mirror、disconnected normalization、summary publish を
    一つの `ApplyDeferred` より前に順序付ける。
  - `Unpowered` の legacy stripping と load / rollback full rebuild を追加する。
  - `GameSettings.power_priority_enabled` と old settings default を追加し、実値変更だけを runtime mode と energy dirty へ同期する。
  - steady-state zero-work、relationship removal、grid despawn、reconnect tests を追加する。
- 変更ファイル:
  - `crates/bevy_app/src/systems/energy/grid_recalc.rs`
  - `crates/bevy_app/src/systems/energy/grid_lifecycle.rs`
  - `crates/bevy_app/src/systems/energy/lamp_buff.rs`
  - `crates/bevy_app/src/plugins/logic.rs`
  - `crates/bevy_app/src/systems/soul_ai/mod.rs`
  - `crates/bevy_app/src/systems/save/reset.rs`
  - `crates/bevy_app/src/systems/save/rehydrate.rs`
  - `crates/bevy_app/src/systems/save/schema.rs`
  - `crates/hw_core/src/settings.rs`
  - `crates/bevy_app/src/systems/settings/`
- 完了条件:
  - [ ] 供給不足時に priority prefix だけが稼働し、lamp effect が同 frame の状態を読む。
  - [ ] relationship / grid removal 後に orphan consumer が `Disconnected + Unpowered` になる。
  - [ ] new spawn / load / rollback / duplicate legacy fixture の全てで Yard 1件につき canonical PowerGrid が厳密に1件だけ残る。
  - [ ] load 後の `AssignedTask::None` と stale `WorkingOn/TaskWorkers` は output より先に cleanup され、旧 worker を発電へ数えない。
  - [ ] load と rollback 後の最初の Logic で全 runtime state が再構築される。
  - [ ] energy input が不変な tick で output / allocation run は増えない。lamp buff は slow-step ごとの継続 work を維持する。
  - [ ] old `settings.ron` は priority mode を有効として補完し、設定切替は1回だけ allocation を起動して、変更を観測した最初の
    Logic frame の effect 前に反映される。
  - [ ] priority field を持たない old `settings.ron` の非既定 UI scale / camera / debug 値を保持し、新 field だけ `true` で補完する。
  - [ ] UI scale 等、energy と無関係な settings 変更では allocation run が増えない。
- 検証:
  - `cargo test -p bevy_app@0.1.0 energy`
  - `cargo test -p bevy_app@0.1.0 settings`
  - `cargo check -p bevy_app@0.1.0 --lib --no-default-features --features profiling`

## M4: consumer priority 操作と共通 inspection

- 変更内容:
  - Soul Spa / consumer と、その所属 grid summary の ViewModel / inspection section を追加する。
  - consumer priority intent、root revalidation、専用 `PowerConsumerPolicyChangeOutcome` と
    `UserFacingNotification` adapter を接続する。
  - Settings checkbox、UiIntent、root settings handler を `GameSettings.power_priority_enabled` へ接続し、現在 mode を inspection に表示する。
  - disconnected、invalid demand、load shedding、restore hysteresis 待ちを別理由で表示する。
- 変更ファイル:
  - `crates/hw_ui/src/intents.rs`
  - `crates/hw_ui/src/models/inspection.rs`
  - `crates/hw_ui/src/panels/info_panel/`
  - `crates/hw_ui/src/setup/settings_panel.rs`
  - `crates/bevy_app/src/interface/ui/presentation/`
  - `crates/bevy_app/src/interface/ui/interaction/`
  - `crates/bevy_app/src/interface/ui/interaction/handlers/settings.rs`
  - `crates/bevy_app/src/interface/ui/plugins/notifications.rs`
  - `crates/bevy_app/src/plugins/messages.rs`
- 完了条件:
  - [ ] 選択可能な Soul Spa / consumer から発電、需要、供給済み需要、配線、priority、遮断理由を説明できる。
  - [ ] stale target や policy 欠落は安全な outcome になり、ECS を部分更新しない。
  - [ ] settings checkbox から mode を往復でき、Legacy mode では全consumerが現行 all-or-none境界に従う。
  - [ ] panel open / close が allocation work と state を変えない。
- 検証:
  - `cargo test -p hw_ui energy`
  - `cargo test -p bevy_app@0.1.0 energy_ui`
  - `cargo test -p bevy_app@0.1.0 settings`

## M5: HVAC 接続契約、横断回帰、恒久ドキュメント

- 変更内容:
  - HVAC M2/M3 が `PowerConsumer + PowerConsumerPolicy` と `Unpowered` / `PowerSupplyState` を利用する
    consumer contract を文書化する。HVAC の Room / FluidGrid 実装は行わない。
  - active slots、partial supply、hysteresis、save/load、orphan cleanup の固定シナリオを統合する。
  - `docs/soul_energy.md`、`docs/info_panel_ui.md`、`docs/settings.md`、`docs/save_load.md`、`docs/invariants.md`、
    `docs/architecture.md`、必要なら `docs/cargo_workspace.md` と HVAC 計画を同期する。
- 変更ファイル:
  - `crates/*/src/**/tests.rs`
  - `docs/soul_energy.md`
  - `docs/info_panel_ui.md`
  - `docs/settings.md`
  - `docs/save_load.md`
  - `docs/invariants.md`
  - `docs/architecture.md`
  - `docs/plans/hvac-plumbing-plan-2026-07-13.md`
- 完了条件:
  - [ ] Track B3 の受入シナリオと workspace gate が成功する。
  - [ ] HVAC が energy internal を複製せず共通 consumer contract を利用できる。
  - [ ] Battery の着手条件が恒久 docs に明記され、本計画を archive できる。
- 検証:
  - `python3 scripts/dev.py docs --write`
  - `python3 scripts/dev.py verify`
  - `git diff --check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| pending assignment を数えない | active slot を同一 cycle で超える | site 単位 shadow を submit 成功時に更新する |
| 枠減少で worker を即解除する | task / relationship / Dream 消費が壊れる | no-kick を domain test にし、新規 assignment だけを gate する |
| priority allocator が bin-packing 化する | 遮断順を説明できず小需要が高優先度を飛び越す | strict prefix を純粋関数と test で固定する |
| f32 境界で毎回反転する | Lamp / HVAC effect と UI がちらつく | compare epsilon と restore margin を分け、以前の state を入力にする |
| relationship removal 後に給電状態が残る | Yard 外設備が効果を出し続ける | dirty transaction に disconnected normalization を含める |
| runtime state を保存して load 直後に使う | stale Entity / topology の状態を表示する | legacy state を strip し、full rebuild 前は runtime summary を未確定扱いにする |
| load 後に `ChildOf` がなく site output を再計算できない | 保存済み発電量が stale のまま配分される | `SoulSpaTile.parent_site` を論理正本にして一走査集計する |
| `Add<Yard>` observer が load 中にも grid を spawn する | 保存 grid と二重化し接続先が非決定になる | observer は dirty 化だけを行い、reconciler が一対一化・rewire・duplicate cleanup を行う |
| state-sanity の deferred cleanup と energy が未順序 | load 直後に stale `TaskWorkers` を発電へ数える | cleanup flush を名前付き set にし、energy pipeline をその後へ順序付ける |
| cold start に以前の shed state がない | exact capacity の復旧結果が未定義になる | state 欠落は raw prefix 即時供給、既知 shed だけ margin 待ちと固定する |
| normalization の Commands が effect 後まで遅延する | orphan consumer が1 frame効果を出す | allocation / normalization を同じ flush 前に置き、その後だけ effect を実行する |
| Entity ID を主 tie-break にする | load 後に遮断順が変わる | grid 座標を主キーにし、同一セル重複を禁止する |
| hysteresis のため毎 tick timer を進める | zero-work 契約を失う | 時間保持ではなく generation margin を採用し、dirty event 時だけ再計算する |
| `SoulSpaSlotsChangeOutcome` を登録するだけで consumer を置かない | clamp / stale / phase failure がplayerへ届かない | M1でA2 notification adapterと1 intent : 1 outcome回帰を同時に追加する |
| priority UIだけを隠して無効化扱いにする | Normal同士でもprefix配電が残り現行all-or-noneへ戻せない | user-local mode settingからpure allocator modeを切り替え、Legacy経路の等価性を固定する |
| `Unpowered` 除去を通常のadditive migrationとして扱う | v1 schema方針と実装が矛盾する | runtime-derived限定のv1 normalization条件を親提案と共有し、durable stateをstripしない |

## 7. 検証計画

- 必須:
  - active slots 0 / 1 / 4、same-cycle multi-assignment、枠減少中の no-kick、`ChildOf` なし output rebuild。
  - priority / stable key / exact capacity / strict prefix / hysteresis / cold start / invalid demand の pure tests。
  - Legacy all-or-none の現行境界等価性、mode 往復、old settings default、無関係 settings 変更の zero-work。
  - consumer add/remove、`ConsumesFrom` removal、grid despawn、reconnect exact-boundary、generation change。
  - new/load/rollback/legacy duplicate の Yard↔Grid 一対一化と、canonical grid への relationship rewire。
  - load fixture の `AssignedTask::None` + stale `WorkingOn/TaskWorkers` cleanup-before-output ordering。
  - old v0/v1 runtime-state stripping、policy default、new save round-trip、rollback rebuild。
  - active slot outcome の exact / clamp / stale / unsupported / phase unavailable と同 Update notification。
  - effect system が同 frame の個別 supply state を読む ordering test。
  - `cargo check --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 計画完了時:
  - `cargo test --workspace`
  - `python3 scripts/dev.py docs --write`
  - `python3 scripts/dev.py verify`
  - `git diff --check`
- 手動確認シナリオ:
  - Soul Spa を 4/4 稼働中に 2 枠へ下げ、4体が継続し、終了後は2体までしか再割当されないことを確認する。
  - 供給不足の Lamp 群で High、Normal、Low の順に点灯し、同順位は同じ座標順になることを確認する。
  - generation を境界付近で上下させ、shed は即時、restore は margin 後であることを確認する。
  - Yard / grid を失った consumer が Disconnected と表示され、effect を出さないことを確認する。
  - Power priority setting を無効化し、需要超過時に全consumerが停止する現行all-or-none挙動へ戻ることを確認する。
- パフォーマンス確認:
  - profiling counter で dirty 1 回につき output / allocation 1 回、energy steady state 0 回を確認する。
    lamp candidate counter は `SlowSimulationClock` step に応じて増える既存契約を維持する。
  - inspection hidden / visible で allocation run 数と simulation checksum が一致する。

## 8. ロールバック方針

- M1 の Soul Spa control、M2/M3 の allocator、M4 の UI を別変更単位にする。
- priority UI に問題がある場合は `GameSettings.power_priority_enabled = false` で allocator を
  `LegacyAllOrNone` へ切り替え、individual state / summary / `Unpowered` mirror は同じpipelineで維持する。
  code rollback が必要な場合だけ allocator、individual state、`Unpowered` 同期を一括して旧実装へ戻し、混在させない。
- new save の policy component は破壊的に削除しない。未知型を理解しない旧 executable は registry deserialize 時の
  `InvalidData` として live world apply 前に拒否し、forward compatibility は保証しない。
- Battery / HVAC を本計画へ先行結合しないため、B3 単独で rollback 可能にする。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `計画 100% / 実装 0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: `M1`〜`M5` 未着手

### 次のAIが最初にやること

1. `assign_generate_power` の same-cycle over-assignment、`ChildOf` なし output rebuild、no-kick の failing tests を先に作る。
2. ECS system を変更する前に、Legacy all-or-none、pure prefix allocator、hysteresis の表形式 test を `hw_energy` に作る。
3. `Unpowered` / `PowerGrid` の既存 v0/v1 fixture を確認し、Yard/Grid 一対一化、stale worker cleanup ordering、
   legacy stripping、full rebuild を同じ load 回帰へ入れる。

### ブロッカー/注意点

- `active_slots` は既に保存対象。新しい値を重複 component へ移さない。
- Soul Spa の論理集計で表示用 `Children` を要求せず、durable `SoulSpaTile.parent_site` を使う。
- 枠減少で現在 worker、designation、task を剥がさない。
- `PowerGrid.powered` は部分給電を表現できないため、runtime summary を inspection の正本にする。
- `Unpowered` は downstream effect 互換 marker であり、policy や遮断理由の正本にしない。
- Battery は B3 と HVAC consumer の実運用が安定するまで非対象。
- allocation / disconnected normalization の後、effect の前に Commands を flush する。
- `SoulSpaSlotsChangeOutcome` は登録だけで終わらせず、M1でnotification adapterと失敗時非変更を接続する。
- priority無効化はUI非表示ではなく `LegacyAllOrNone` allocator modeへの実切替とし、他settings変更でenergyをwakeしない。
- Yard の Add observer から grid を即時 spawn せず、通常 spawn と world replacement の両方を一対一 reconciler へ通す。
- energy output を Soul state-sanity cleanup の named flush より前へ置かない。
- output / allocation の zero-work と、slow-step ごとに必要な lamp buff work を混同しない。

### 参照必須ファイル

- `docs/proposals/gameplay-management-improvements-proposal-2026-07-17.md`
- `docs/soul_energy.md`
- `docs/info_panel_ui.md`
- `docs/save_load.md`
- `docs/invariants.md`
- `docs/proposals/hvac-plumbing-proposal.md`
- `docs/plans/hvac-plumbing-plan-2026-07-13.md`
- `crates/hw_energy/src/components.rs`
- `crates/hw_energy/src/soul_spa.rs`
- `crates/bevy_app/src/systems/energy/grid_recalc.rs`
- `crates/bevy_app/src/systems/energy/grid_lifecycle.rs`
- `crates/bevy_app/src/plugins/logic.rs`
- `crates/bevy_app/src/systems/soul_ai/mod.rs`
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/policy/basic.rs`
- `crates/bevy_app/src/systems/save/`

### 最終確認ログ

- 最終 `cargo check --workspace`: 未実施（計画作成のみ）
- 最終 `cargo clippy --workspace --all-targets -- -D warnings`: 未実施（計画作成のみ）
- 最終 `cargo test --workspace`: 未実施（計画作成のみ）
- 未解決エラー: なし（未着手）

### Definition of Done

- [ ] M1〜M5 が完了
- [ ] active slots の同一 cycle 上限と no-kick を自動テスト済み
- [ ] `ChildOf` なし output rebuild と worker 0 stale-output reset が自動テスト済み
- [ ] priority prefix / stable order / hysteresis / cold start が自動テスト済み
- [ ] Legacy all-or-none mode が現行境界と等価で、setting切替とold settings補完が自動テスト済み
- [ ] runtime supply state と legacy save rebuild が正しい
- [ ] new/load/rollback の Yard↔PowerGrid 一対一化と stale worker cleanup-before-output が自動テスト済み
- [ ] 共通 inspection で発電・需要・接続・遮断理由を説明可能
- [ ] Soul Spa slot outcome がclamp / stale / phase failureを通知し、失敗時に状態を変更しない
- [ ] `python3 scripts/dev.py verify` が成功
- [ ] 恒久 docs 更新後に本計画を archive

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-07-20` | `Codex` | Track B3 の Soul Spa 枠、pending shadow、priority prefix、hysteresis、runtime inspection、HVAC 接続境界を計画化 |
| `2026-07-21` | `Codex` | Soul Spa outcome adapter、Power priorityのLegacy切替設定、runtime-derived state strippingのv1例外条件をレビューから反映 |
