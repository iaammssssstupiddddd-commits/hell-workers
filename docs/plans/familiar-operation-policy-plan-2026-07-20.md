# Track B2 Familiar 運用ポリシー・永続化 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `familiar-operation-policy-plan-2026-07-20` |
| ステータス | `Draft` |
| 作成日 | `2026-07-20` |
| 最終更新日 | `2026-07-21` |
| 作成者 | `Codex` |
| 関連提案 | `docs/proposals/gameplay-management-improvements-proposal-2026-07-17.md`（Track B2） |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: Familiar の運用値がロード時に失われ、WorkType ごとの持続的な役割方針もない。
  - `FamiliarOperation` の疲労閾値と最大管理 Soul 数はプレイヤーが変更できるが save schema 外で、
    `attach_familiar_shell_with_voice` がロード時にも `default()` を挿入するため保存値を保持できない。
  - Familiar は `WorkType` ごとの許可・優先度を持たず、プレイヤーが役割分担を持続的な方針として表現できない。
  - UI 独自に候補可否を推測すると、A3 の停止理由と実際の AI 判断がずれる。
- 到達したい状態:
  - `FamiliarOperation` と新しい `FamiliarPolicy` が durable simulation state として保存・復元される。
  - Familiar ごとに WorkType の許可と優先度を設定でき、既存 `TaskArea` を活動範囲の唯一の正本として使う。
  - 全 WorkType 禁止を明示的な待機方針として許可し、実際の候補評価から A3 の停止理由を導出する。
  - 方針変更で実行中タスクを強制破棄せず、次回の通常判断から反映する。
- 成功指標:
  - 旧 v0/v1 セーブは operation / policy の欠落時だけ既定値を補い、新セーブは編集値を往復保持する。
  - 保存済み値が shell rehydrate や spawn helper で上書きされる経路が 0 件である。
  - 同じ task priority 内では Familiar の work priority が決定的に反映され、禁止 WorkType は新規割当されない。
  - policy UI を開いても候補評価や経路探索回数が増えない。

## 2. スコープ

### 対象（In Scope）

- 現行 `FamiliarOperation` の Reflect / save schema 対応と shell / durable spawn 責務の分離。
- `hw_core` 所有の永続 `FamiliarPolicy`、既定 rule、WorkType ごとの override。
- WorkType の許可 filter と、既存 composite score を保つ Familiar-local policy score offset。
- `FamiliarDelegationContext` / `FamiliarSearchContext` を通じた policy の集約。
- 全禁止の明示的 idle、operation dialog の警告、A3 の `PolicyDisabled` 停止理由。
- policy 変更時の task diagnostic revision 更新、次回判断反映、save/load/reset 回帰。
- 既存 operation dialog における疲労閾値、最大管理 Soul、WorkType rule の編集。

### 非対象（Out of Scope）

- 新しい距離・円・ポリゴン型の活動範囲。現行 `TaskArea` と AreaEdit UI を再利用する。
- 実行中 task の強制 cancel、Soul の即時再割当、relationship の直接解除。
- Soul の生命維持、休息、stress breakdown、Familiar の recruitment / supervision を WorkType policy で禁止すること。
- Familiar rank / 昇格、Contract、全 Familiar 共通 template、役職プリセット。
- WorkType ごとの数値効率、作業速度、複雑な weight tuning。
- A3 dashboard 全体の再設計や UI からの候補再評価。

## 3. 現状とギャップ

- `FamiliarOperation` は `Component + Debug + Clone` で Reflect を持たず、save schema に登録されていない。
- 新規 spawn と load rehydrate が共用する `attach_familiar_shell_with_voice` は、runtime shell と一緒に
  `FamiliarOperation::default()` を無条件挿入する。このまま schema へ加えるだけでは保存値を上書きする。
- `Familiar`、`Commanding`、`ManagedTasks`、`Transform` は durable 側、`ActiveCommand`、`FamiliarAiState`、
  presentation は runtime shell 側という既存境界がある。operation / policy は durable 側へ移す必要がある。
- `WorkType` は `hw_core::jobs` が所有し、`hw_ui` と `hw_familiar_ai` は既に `hw_core` に依存している。
  したがって policy 型も `hw_core::familiar` に置けば dependency 逆流を作らない。
- Familiar の空間制約は既に persistent な `TaskArea` と AreaEdit で表現され、delegation / recruitment /
  supervision / task search が参照している。別の activity range を追加すると正本が二重化する。
- A3 diagnostics は latest-only snapshot と input revision を持つ。policy を変更検知へ含め、実候補 path が
  記録した rejection evidence から理由を作る必要がある。
- 現行 candidate ranking は task `Priority` と種類別補正を i32 score にした後、worker 距離と 0.65 / 0.35 で
  合成する。これを lexicographic tuple へ置換すると default / Normal policy でも既存割当順が変わる。
- `WorkType` は列挙用 `ALL` や安定 rank を持たない。override 正規化、全行 UI、all-disabled 判定に共通の
  exhaustive な列挙契約が必要である。
- 旧セーブは `FamiliarOperation` を持たなくても durable `Commanding` を保持し得る。欠落 operation を単純に
  max 2 へすると、最大8体の既存 roster と設定が矛盾し、UI change event もないため解消されない。

## 4. 実装方針（高レベル）

### 4.1 固定するデータ契約

```rust
pub enum FamiliarWorkPriority {
    Low,
    Normal,
    High,
}

pub struct FamiliarWorkRule {
    pub allowed: bool,
    pub priority: FamiliarWorkPriority,
}

pub struct FamiliarWorkRuleOverride {
    pub work_type: WorkType,
    pub rule: FamiliarWorkRule,
}

pub struct FamiliarPolicy {
    pub default_rule: FamiliarWorkRule,
    pub overrides: Vec<FamiliarWorkRuleOverride>,
}
```

- 既定 policy は全 WorkType `allowed = true / Normal` とし、現行 gameplay と等価にする。
- lookup は `default_rule` を基準に override を適用する。新しい `WorkType` 追加時も default の意図を保つ。
- `WorkType::ALL` を UI 表示順兼 stable rank の正本として `hw_core::jobs` に追加する。variant 追加時に `ALL` の
  exhaustiveness test が落ちる契約にし、UI、normalization、all-disabled 判定が同じ列挙を使う。
- override は WorkType ごとに最大 1 件とし、更新時に重複を除去して `WorkType::ALL` 順へ正規化する。
  永続状態へ `HashMap` や Entity key を入れない。
- 「全禁止」は `default_rule.allowed = false` かつ allow override なしで表現できる。UI の一括禁止は
  override の大量生成ではなく default と例外の正規化を行う。
- `FamiliarOperation` と `FamiliarPolicy` は Reflect component として `schema.rs` の persisted component に加える。

### 4.2 durable spawn と rehydrate

- 新規 Familiar の root spawn が `FamiliarOperation::default()` と `FamiliarPolicy::default()` を挿入する。
- `attach_familiar_shell_with_voice` から `FamiliarOperation` を除き、同 helper は runtime shell だけを扱う。
- load 後は `Without<FamiliarOperation>` / `Without<FamiliarPolicy>` の Familiar だけへ値を補う。
  欠落 operation の threshold は default、`max_controlled_soul` は `max(default, Commanding.len())` とし、
  旧セーブの既存 roster を推測で解雇しない。保存済み operation / policy の値は上書きしない。
- 保存済み max より roster が多い場合は、UI event と speech を発火させず、専用の一回限り
  `FamiliarRosterReconcileRequest` から既存 `SoulTaskUnassignRequest` / `CommandedBy` cleanup を通して超過だけを解放する。
  対象 Familiar には `hw_familiar_ai` 所有の runtime-only `FamiliarRosterReconcilePending` と解放対象を記録し、pending 中はその Familiar の
  recruitment / delegation だけを gate する。自己維持 state と現在 task の通常実行は止めない。
- request は最初の post-load Familiar Perceive で発行し、`CommandedBy` removal を同 phase 後の `ApplyDeferred` で反映する。
  `SoulTaskUnassignRequest` の実 cleanup は現行 schedule どおり、その frame 後半の `SoulAiSystemSet::Perceive` で処理する。
  その後の Perceive → Update 間の既存 `ApplyDeferred` を通し、`SoulAiSystemSet::Update` 冒頭の root completion system が
  解放対象の task / relationship cleanup を確認して pending を外す。
  Familiar Decide は既に通過済みなので、recruitment / delegation の再開は早くても次 frame とする。
- stale / despawn 済み Soul は解決済みとして扱い、未完対象がある間は pending を保持する。UI handler と load reconciliation は
  同じ deterministic な超過選択 helper を使い、表示 event と整合処理を分離する。
- additive component migration として container header v1 は維持する。v0/v1 fixture で missing 値の補完と、
  新 save の round-trip を固定する。

### 4.3 AI への適用順

- `FamiliarDelegationContext` / `FamiliarSearchContext` へ `&FamiliarPolicy` を集約し、個別 system parameter を増殖させない。
- 候補が `WorkType` を特定した直後、source scan、pathfinding、詳細 score の前に `allowed` filter を適用する。
- `FamiliarWorkPriority` は既存 i32 candidate priority へ加えない。既存の `score_for_worker`
  （priority 0.65 / distance 0.35）を変更せず、その算出後に named bounded familiar policy score offset を加える。
  最終 rank score は `base_worker_score + transport_policy_offset + familiar_policy_offset` とし、最後に clamp しない。
  共有 `POLICY_SCORE_UNIT` は現行 priority slope の `WORKER_PRIORITY_WEIGHT / 40.0` とし、familiar contribution は
  Low=-5、Normal=0、High=+5 unit とする。default policy の candidate score と Top-K を byte-for-byte等価に保ち、
  同条件では base priority が現行上限20へ達していても High > Normal > Low になるが、task の既存補正や距離を
  絶対に上書きする hard tierではない。Low-to-High の全 span 10 unitは0.1625で、現行の全距離 span 0.35より小さい。
  shared helper は scalar の transport / familiar contribution だけを受け取り、B2 は
  Familiar enum から named constant への変換だけを所有する。B1 が未実装なら transport offset は 0、実装済みなら同じ helper で加算する。
  両方を実装した場合も Low-to-High/Critical の合算 span は最大40 unit、すなわち現行
  `WORKER_PRIORITY_WEIGHT` 以内に収める。
  `policy_score.rs` を `WORKER_PRIORITY_WEIGHT`、`WORKER_DISTANCE_WEIGHT`、`POLICY_SCORE_UNIT`、scalar contribution struct、
  composition helper の単一所有者にし、`assignment_loop.rs` の base scorer もそこから weight を読む。0.65 / 0.35 literal を
  二重定義しない。
  `ScoredDelegationCandidate` は transport / familiar の scalar unit を保持し、candidate collection 時に B1 は receiver tier、
  B2 は `FamiliarPolicy` と `WorkType` から各 unit を解決する。worker ごとの距離 score を算出した後、Top-K 選択の直前に
  helper で一度だけ合成する。
- policy を変えても、既に `AssignedTask` / `ManagedTasks` に入った task は cancel しない。
  追加の即時 delegation cycle も起こさず、次の通常 cycle で未割当候補へ反映する。
- recruitment、supervision、休息、stress / escape など WorkType 外の自己維持 state は policy filter を通さない。
- load reconciliation pending の Familiar は candidate policy と独立に recruitment / delegation entry を skip し、
  Soul Perceive の cleanup 完了確認後の次 frame からだけ通常判断へ戻す。

### 4.4 A3 diagnostics

- `TaskDiagnosticClass` と UI の `TaskBlockerReason` に `PolicyDisabled` を追加する。
- policy rejection は通常の candidate evaluation と同じ cycle の bounded counter / evidence に記録する。
  UI や diagnostics 専用 systemから候補収集・A*を再実行しない。
- 対象 task について、policy gate まで到達した全 Familiar が拒否され、許可した Familiar が 0 の場合だけ
  `PolicyDisabled` とする。policy gate へ到達する Familiar 自体がいない場合は既存 `NoEligibleFamiliar` を維持する。
- `Changed<FamiliarPolicy>` と removal を `TaskDiagnosticInputRevisions` の semantic change source に加え、
  latest snapshot を次の producer cycle で更新する。

### 4.5 UI と Message 境界

- operation dialog に WorkType ごとの Enabled / Disabled と Low / Normal / High を追加し、既存の疲労閾値、
  最大管理 Soul 設定と同じ Familiar Entity を編集する。
- `UiIntent` は `familiar Entity + WorkType + expected/current change` を明示する。root handler は Entity 生存、
  Familiar / policy の存在、priority 値を再検証して domain patch を適用する。
- 全禁止は正当な成功結果として適用し、dialog 内に「新しい作業は割り当てない。現在作業と自己維持は継続する」
  警告を表示する。専用 `FamiliarPolicyChangeOutcome` Message を A2 の notification adapter pattern へ接続し、
  全禁止の成功だけ warning severity とする。
- `hw_ui` は ViewModel と intent だけを所有し、Familiar component を直接 mutate しない。

### 4.6 設計判断

| ID | 判断 |
| --- | --- |
| B2-D01 | `FamiliarOperation` と `FamiliarPolicy` は durable、`ActiveCommand` / AI state / visual は runtime |
| B2-D02 | 活動範囲は既存 `TaskArea` を唯一の正本とし、新型を追加しない |
| B2-D03 | policy は `hw_core::familiar` が所有し、`WorkType` は `hw_core::jobs` を参照する |
| B2-D04 | work priority は既存worker score後の -5 / 0 / +5 policy unit。Normal=0、最終no-clampでdefault順位と上限到達時の単調性を両立する |
| B2-D05 | 方針変更は現在 task を cancel せず、次回の通常判断から反映する |
| B2-D06 | all-disabled は有効な idle policy。自己維持挙動は policy 対象外 |
| B2-D07 | missing component のみ既定補完し、container header v1 は維持する |
| B2-D08 | UI・正規化・all-disabled 判定は exhaustive な `WorkType::ALL` を共有する |
| B2-D09 | 旧 save の missing max は roster 数を下回らせず、保存済み max 超過 roster は無演出の domain reconciliation を通す |
| B2-D10 | load roster reconciliation 中は recruitment / delegation を gate し、Soul Perceive cleanup 完了後の次 frame に再開する |

- Bevy 0.19 APIでの注意点:
  - Reflect component の登録・DynamicScene round-trip は既存 save schema macro と tests を拡張する。
  - Message / Query / scrollable UI は既存 project 内実装を優先し、新 API は Bevy 0.19 の一次資料で確認する。

## 5. マイルストーン

## M1: `FamiliarOperation` の durable 化と非上書き移行

- 変更内容:
  - `FamiliarOperation` を Reflect component にし、save schema へ登録する。
  - root spawn と runtime shell を分離し、load では欠落時だけ既定値を補う。
  - old-save missing max の roster-aware migration と、保存済み max に対する無演出 roster reconciliation を追加する。
  - runtime pending による Familiar recruitment / delegation gate と、Soul Perceive cleanup 後の completion system を追加する。
  - operation の old fixture / round-trip / shell idempotence tests を追加する。
- 変更ファイル:
  - `crates/hw_core/src/familiar.rs`
  - `crates/hw_core/src/events.rs`
  - `crates/bevy_app/src/entities/familiar/spawn.rs`
  - `crates/bevy_app/src/plugins/messages.rs`
  - `crates/bevy_app/src/systems/save/schema.rs`
  - `crates/bevy_app/src/systems/save/rehydrate.rs`
  - `crates/bevy_app/src/systems/save/schema/tests.rs`
  - `crates/bevy_app/src/systems/save/rehydrate/tests/`
  - `crates/hw_familiar_ai/src/familiar_ai/execute/max_soul_logic.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/roster_reconciliation.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/mod.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/recruitment.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/state_decision/system.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_delegation.rs`
  - `crates/bevy_app/src/systems/familiar_ai/roster_reconciliation.rs`
  - `crates/bevy_app/src/systems/familiar_ai/mod.rs`
  - `crates/bevy_app/src/systems/soul_ai/mod.rs`
- 完了条件:
  - [ ] 新規 spawn は operation default を持ち、shell helper は durable state を挿入しない。
  - [ ] 旧セーブの missing max は既存 roster 数以上になり、保存済み threshold / max soul は上書きされない。
  - [ ] 保存済み max を超える roster は frame 1 の Familiar recruitment / delegation を gate し、同 frame の Soul Perceive で
    既存 unassign / relationship cleanup を一回だけ完了して、最短でも frame 2 から通常判断へ戻る。
  - [ ] load / rollback 後も UI / speech event を偽発火せず、diagnostic revision は正しく再構築される。
- 検証:
  - `cargo test -p hw_core familiar`
  - `cargo test -p hw_familiar_ai roster_reconciliation`
  - `cargo test -p hw_soul_ai task_unassign`
  - `cargo test -p bevy_app@0.1.0 --lib systems::save`

## M2: `FamiliarPolicy` モデルと AI filter / priority

- 変更内容:
  - policy 型、`WorkType::ALL`、default / override normalization、lookup を追加する。
  - Familiar root spawn、save schema、missing-policy migration を接続する。
  - delegation / search context に policy を集約し、allowed filter と共有 composition helper上の Normal=0 bounded policy offset を実装する。
- 変更ファイル:
  - `crates/hw_core/src/familiar.rs`
  - `crates/hw_core/src/jobs.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/delegation_context.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_delegation.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/policy_score.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/task_finder/mod.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/delegation/assignment_loop.rs`
  - `crates/bevy_app/src/entities/familiar/spawn.rs`
  - `crates/bevy_app/src/systems/save/`
- 完了条件:
  - [ ] `WorkType::ALL` が全 variant を一度ずつ含み、UI / normalization / all-disabled が共有する。
  - [ ] default policy は全既存 WorkType で現行候補集合、i32 priority、worker composite score、Top-K が同じである。
  - [ ] disabled WorkType は expensive scan / pathfinding 前に除外される。
  - [ ] 同条件で既存 priority が上限20へ達した候補でも High > Normal > Low になり、既存 task 補正と距離の
    0.65 / 0.35 合成は変わらない。
  - [ ] familiar unit mapping は -5 / 0 / +5 と現行 priority slope の積に一致する。
  - [ ] shared helper は `base + transport + familiar` の加算順に依存せず、各 Normal=0 と最終 no-clamp を守る。
    B1 未実装時は synthetic transport contribution、B1 実装後は実 enum mapping との統合テストで固定する。
  - [ ] transport と familiar の最小・最大を合算した score span が `WORKER_PRIORITY_WEIGHT` を超えない。
  - [ ] Familiar の Low-to-High span は `WORKER_DISTANCE_WEIGHT` より小さく、距離の最大差を常に上書きする hard tier にならない。
  - [ ] candidate に保持した contribution が Top-K と fallback の両方へ同じ一回だけ適用され、base scorer の 0.65 / 0.35
    合成と tie-break は変わらない。
  - [ ] all-disabled でも現在 task と自己維持 state は破壊されない。
- 検証:
  - `cargo test -p hw_core familiar_policy`
  - `cargo test -p hw_familiar_ai task_management`

## M3: A3 停止理由と revision 統合

- 変更内容:
  - policy rejection evidence、`PolicyDisabled`、UI mapping を追加する。
  - policy change / removal を diagnostic revision source に含める。
  - no roster、全拒否、一部許可、別理由優先の分類テストを追加する。
- 変更ファイル:
  - `crates/hw_jobs/src/diagnostics.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/diagnostics.rs`
  - `crates/bevy_app/src/systems/familiar_ai/diagnostics.rs`
  - `crates/bevy_app/src/interface/ui/panels/task_list/view_model.rs`
  - `crates/hw_ui/src/panels/task_list/`
- 完了条件:
  - [ ] 全候補が policy で拒否された場合だけ `PolicyDisabled` になる。
  - [ ] policy を許可へ戻すと次の通常 cycle で blocker が消える。
  - [ ] dashboard 表示の有無で候補評価・pathfinding 回数が変わらない。
- 検証:
  - `cargo test -p hw_familiar_ai diagnostics`
  - `cargo test -p bevy_app@0.1.0 task_list`
  - `cargo test -p hw_ui task_list`

## M4: operation dialog と typed outcome

- 変更内容:
  - 既存 dialog を scrollable な WorkType rule editor へ拡張する。
  - policy UiIntent、root revalidation、normalization、`FamiliarPolicyChangeOutcome`、全禁止 warning を追加する。
  - `TaskArea` は既存 AreaEdit への導線だけを示し、dialog 内に別 range 値を作らない。
- 変更ファイル:
  - `crates/hw_ui/src/intents.rs`
  - `crates/hw_ui/src/setup/dialogs.rs`
  - `crates/hw_ui/src/components.rs`
  - `crates/bevy_app/src/interface/ui/interaction/handlers/familiar_settings.rs`
  - `crates/bevy_app/src/interface/ui/interaction/intent_context.rs`
  - `crates/bevy_app/src/interface/ui/interaction/systems.rs`
  - `crates/bevy_app/src/interface/ui/plugins/notifications.rs`
  - `crates/bevy_app/src/plugins/messages.rs`
- 完了条件:
  - [ ] stale selection と欠落 component は安全な failure outcome になる。
  - [ ] 全 WorkType の rule と all-disabled warning を dialog から識別できる。
  - [ ] load 後の表示が保存済み operation / policy と一致する。
- 検証:
  - `cargo test -p hw_ui familiar`
  - `cargo test -p bevy_app@0.1.0 familiar_settings`

## M5: 横断回帰、性能、恒久ドキュメント

- 変更内容:
  - save/load、実行中変更、all-disabled、mixed priorities、diagnostics の固定シナリオを統合する。
  - `docs/familiar_ai.md`、`docs/info_panel_ui.md`、`docs/task_list_ui.md`、`docs/save_load.md`、
    `docs/invariants.md`、必要なら `docs/architecture.md` / `docs/cargo_workspace.md` を同期する。
- 変更ファイル:
  - `crates/*/src/**/tests.rs`
  - `docs/familiar_ai.md`
  - `docs/info_panel_ui.md`
  - `docs/task_list_ui.md`
  - `docs/save_load.md`
  - `docs/invariants.md`
- 完了条件:
  - [ ] Track B2 の受入シナリオと workspace gate が成功する。
  - [ ] UI を開かない steady state の候補収集 work が増えない。
  - [ ] 恒久 docs と生成索引が最新で、本計画を archive できる。
- 検証:
  - `python3 scripts/dev.py docs --write`
  - `python3 scripts/dev.py verify`
  - `git diff --check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| shell helper が保存値を再び上書きする | プレイヤー設定が load ごとに消える | durable spawn と runtime shell を分離し、非上書き / idempotence test を置く |
| future WorkType が暗黙に有効化される | all-disabled の意図が破れる | default rule + normalized override で将来 variant の挙動を明示する |
| policy priority 導入で default 順位が変わる | 既存 assignment / Top-K が回帰する | Normal=0 の bounded offset を既存 worker score 後に加え、default score equality を固定する |
| B1/B2 の offset を既存 i32 priority へ加える | priority 20 の clamp で High と Normal が同点になる | 共有 additive composition を worker score 後へ置き、最終 no-clamp と組合せ単調性を固定する |
| 旧 save の roster と missing max が矛盾する | 8/2 等の状態が残るか、予期せず Soul を解雇する | missing 値は roster-aware に補い、保存済み値の超過だけ無演出 domain reconciliation する |
| unassign consumer より先に Familiar Decide が走る | cleanup 前に再募集・再委譲して roster / task が再び競合する | runtime pending で対象 Familiar だけを gate し、Soul Perceive 後の cleanup 確認から次 frame に再開する |
| `WorkType::ALL` が新 variant に追従しない | UI 行・all-disabled 判定が欠落する | exhaustive test と variant 追加チェックリストを `hw_core::jobs` に置く |
| 方針変更で現在 task を解除する | relationship・予約・担当が壊れる | 新規候補だけへ適用し、現在 task は既存終了経路へ任せる |
| `PolicyDisabled` が UI 推測になる | 実 AI と blocker が食い違う | candidate path の bounded rejection evidence だけを集約する |
| policy を Query へ個別追加し続ける | `SystemParam` 制限と保守性が悪化する | crate-owned delegation/search context に集約する |
| rule editor が長大になる | dialog が画面外へ溢れる | 既存 UI 規約に沿う scroll / section と一括 default 操作を使う |

## 7. 検証計画

- 必須:
  - `FamiliarOperation` / `FamiliarPolicy` の old-save migration と new-save round-trip。
  - default / override normalization と将来 WorkType を想定した default semantics。
  - disabled filter の pre-pathfinding 適用、Normal=0 score equality、上限 base score を含む Low / High offset の単調性。
  - A3 Critical + Build 補正、B1 Critical transport、B2 High Familiar を組み合わせた shared offset の加算順非依存。
  - old-save roster-aware max、saved max 超過時の一回限り unassign / relationship cleanup、frame 1 gate / frame 2 resume。
  - all-disabled、no roster、一部許可、現在 task 継続、自己維持継続。
  - diagnostic latest-only / revision / reset 回帰。
  - `cargo check --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 計画完了時:
  - `cargo test --workspace`
  - `python3 scripts/dev.py docs --write`
  - `python3 scripts/dev.py verify`
  - `git diff --check`
- 手動確認シナリオ:
  - Familiar A を Haul only、B を Build only にし、同じ task priority 内で役割分担することを確認する。
  - 実行中に該当 WorkType を禁止し、その task は安全に終わり、次の同種 task を選ばないことを確認する。
  - 全禁止で警告と task blocker を確認し、休息・recruitment / supervision が壊れないことを確認する。
  - 保存後に値を変更して load し、保存時の threshold / max soul / rules / TaskArea へ戻ることを確認する。
- パフォーマンス確認:
  - policy gate で拒否された候補は source scan / A* 前に除外される。
  - dialog hidden / visible で AI work counter と simulation checksum が一致する。

## 8. ロールバック方針

- M1 の operation 永続化、M2 の policy / AI、M3 の diagnostics、M4 の UI を別変更単位にする。
- UI を戻しても durable policy は既定動作を継続できる。AI 接続を戻す場合は policy を無視する default behavior にし、
  保存 component を破壊的に削除しない。
- new save を旧 executable が理解できない場合は registry deserialize / `InvalidData` で live apply 前に拒否し、
  未知 component を黙って落とさない。
- rollback で実行中 task や `Commanding` / `ManagedTasks` を直接解除しない。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `計画 100% / 実装 0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: `M1`〜`M5` 未着手

### 次のAIが最初にやること

1. `attach_familiar_shell_with_voice` の operation 挿入を failing regression test で固定してから責務を分離する。
2. `FamiliarOperation` 単体の old/new save tests を通し、その後 `FamiliarPolicy` の型と normalization を追加する。
3. task finder の既存 priority / worker score baseline を固定し、shared no-clamp offset、Normal=0、`WorkType::ALL` を
   test-first で接続する。

### ブロッカー/注意点

- `FamiliarOperation` を schema に足すだけでは shell が保存値を上書きするため不十分。
- 旧セーブの missing max を単純に2へ戻さず durable roster 数を考慮する。保存済み max は上書きせず domain cleanup で整合する。
- `SoulTaskUnassignRequest` consumer は Familiar Execute 後の Soul Perceive にある。load frame の Familiar Decide 前に
  cleanup 済みと仮定せず、runtime pending で recruitment / delegation を次 frame まで gate する。
- 活動範囲は `TaskArea` を再利用し、policy に座標や radius を重複保存しない。
- all-disabled は valid state であり、自動で既定値へ戻さない。
- Familiar / transport policy を既存 i32 candidate priority へ加えない。既存 worker score 後の共有 offset を使い、最終 score を clamp しない。
- policy 変更を理由に現在の task、予約、relationship を直接剥がさない。
- A3 blocker は実候補評価の evidence を使い、UI から候補評価や A* を呼ばない。

### 参照必須ファイル

- `docs/proposals/gameplay-management-improvements-proposal-2026-07-17.md`
- `docs/familiar_ai.md`
- `docs/task_list_ui.md`
- `docs/save_load.md`
- `docs/invariants.md`
- `crates/hw_core/src/familiar.rs`
- `crates/hw_core/src/jobs.rs`
- `crates/bevy_app/src/entities/familiar/spawn.rs`
- `crates/hw_familiar_ai/src/familiar_ai/decide/delegation_context.rs`
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/`
- `crates/hw_soul_ai/src/soul_ai/execute/task_unassign_apply.rs`
- `crates/bevy_app/src/systems/soul_ai/mod.rs`
- `crates/bevy_app/src/systems/familiar_ai/diagnostics.rs`
- `crates/bevy_app/src/systems/save/`

### 最終確認ログ

- 最終 `cargo check --workspace`: 未実施（計画作成のみ）
- 最終 `cargo clippy --workspace --all-targets -- -D warnings`: 未実施（計画作成のみ）
- 最終 `cargo test --workspace`: 未実施（計画作成のみ）
- 未解決エラー: なし（未着手）

### Definition of Done

- [ ] M1〜M5 が完了
- [ ] operation / policy の旧セーブ移行と新セーブ往復が成功
- [ ] roster reconciliation は frame 1 を gate し、Soul cleanup 完了後の frame 2 以降にだけ通常判断を再開
- [ ] `WorkType::ALL`、filter、Normal=0 shared policy offset、上限 score の単調性、現在 task 継続が自動テスト済み
- [ ] all-disabled warning と A3 blocker が実 AI と一致
- [ ] `TaskArea` が活動範囲の唯一の正本
- [ ] `python3 scripts/dev.py verify` が成功
- [ ] 恒久 docs 更新後に本計画を archive

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-07-20` | `Codex` | Track B2 の operation 非上書き永続化、WorkType policy、TaskArea 再利用、A3 blocker、UI を計画化 |
| `2026-07-21` | `Codex` | B1/B2 の方針優先度を既存 i32 priority から分離し、既存 worker score 後の共有 no-clamp offset と組合せ回帰を固定 |
