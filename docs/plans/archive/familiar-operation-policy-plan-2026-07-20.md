# Track B2 Familiar 運用ポリシー・永続化 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `familiar-operation-policy-plan-2026-07-20` |
| ステータス | `Completed` |
| 作成日 | `2026-07-20` |
| 最終更新日 | `2026-07-26` |
| 作成者 | `Codex` |
| 関連提案 | `docs/proposals/gameplay-management-improvements-proposal-2026-07-17.md`（Track B2） |
| 前提 | Track B1 Stockpile policy と shared policy score composition は実装済み |
| 関連Issue/PR | `N/A` |

### クローズ判断

M1〜M6のproduction実装、自動correctness、Help、恒久docs、workspace gateを完了した。
実renderer / pointer受入と性能artifact採取は機能実装から分離し、
`docs/plans/familiar-operation-policy-validation-plan-2026-07-26.md`へ移管したため、本計画を完了・archiveする。

## 1. 目的

### 解決する課題

- `FamiliarOperation` の疲労閾値と最大管理 Soul 数はプレイヤーが編集できるが、現在は save schema 外である。
- `attach_familiar_shell_with_voice` が runtime shell と一緒に `FamiliarOperation::default()` を挿入するため、
  schema 登録だけではロード時に保存値を上書きする。
- operation dialog の handler が `GameSystemSet::Interface` で simulation component を直接変更し、
  最大管理 Soul の超過解放だけを次フレームの `FamiliarAiSystemSet::Execute` へ送っている。
  この分割は、`Last::SaveLoadApplySet` が両処理の間の状態を保存し得る。
- Familiar は `WorkType` ごとの許可・優先度を持たず、プレイヤーが役割分担を持続的な方針として表現できない。
- UI 独自の候補可否推測では、実際の Familiar AI と A3 task blocker が乖離する。

### 到達状態

- `FamiliarOperation` と新しい `FamiliarPolicy` を durable simulation state として保存・復元する。
- UI は simulation component を直接変更せず、typed request を発行する。
- request consumer は Logic 内で operation / policy の更新と、最大数減少時の超過 Soul 解放を一つの
  commit として処理する。
- Familiar ごとに WorkType の許可と Low / Normal / High を設定できる。活動範囲は既存 `TaskArea` を唯一の正本とする。
- 全 WorkType 禁止を有効な待機方針として許可し、通常の候補評価 evidence から `PolicyDisabled` を導出する。
- 方針変更は実行中 task を破棄せず、次回の通常 delegation cycle から未割当候補へ反映する。

### 成功指標

- 旧 v0/v1 セーブは欠落 component だけを補完し、新セーブは operation / policy を往復保持する。
- runtime shell が durable operation / policy を挿入または上書きする経路が 0 件になる。
- UI からの設定変更後に `FamiliarOperation.max_controlled_soul < roster_len` の保存可能な中間状態を作らない。
- default policy は現行候補集合、base score、shared policy score、Top-K、tie-break を変えない。
- disabled WorkType は `candidate_snapshot` と後段の source / reachability / score 評価より前に除外される。
- task dashboard と Help の表示有無が AI の候補評価回数を増やさない。

## 2. レビュー結果と計画修正

| 論点 | 現行実装の事実 | 本計画での修正 |
| --- | --- | --- |
| 最大管理数の整合 | UI が operation を変更し、次フレームの `max_soul_logic_system` が超過 Soul を解放する | typed request consumer へ統合し、値変更と超過解放を同じ Logic tick で行う |
| ロード後 reconciliation | operation は現在保存されていないため、旧 save に「保存済み max と roster の不一致」は存在しない | missing operation は roster-aware に補完し、runtime pending / 複数フレーム gate は追加しない |
| B1 score | `policy_score.rs`、`PolicyScoreContributions`、transport unit は実装済み | 新 helper を作らず、既存 `familiar_units` を -5 / 0 / +5 で埋める |
| policy gate | task finder は candidate membership を記録後、`candidate_snapshot` で詳細検証する | `observe_applicable` の後、`candidate_snapshot` の前を固定位置とする |
| diagnostics revision | Familiar evaluator の構成・適格性は既存 `roster` revision が表す | 新 revision domain を作らず、`Changed/Removed<FamiliarPolicy>` を `roster` revision source に加える |
| Build diagnostics | unowned Blueprint Build は Familiar と Blueprint auto-build の複数 producer を持つ | `PolicyDisabled` は Familiar の一票だけとし、他 producer の成功・未完了を上書きしない |
| operation dialog target | 表示更新と編集対象が live `SelectedEntity` に依存する | accepted open 時に対象 Entity を latch し、close / world replace で同期的に消去する |
| UI 境界 | handler が `FamiliarOperation` と entity-list text を直接変更する | root は request 変換だけを行い、表示は durable state から再構築する |
| WorkType 列挙 | task list に 16 variant のローカル配列がある | `WorkType::ALL` と exhaustive stable index を `hw_core` の正本にする |
| Help 影響 | 旧計画に Help catalog / coverage の更新条件がない | provider、exhaustive coverage、exact approval snapshot を M5 の必須成果物にする |

### 削除する旧設計

以下は実装しない。

- `FamiliarRosterReconcileRequest`
- `FamiliarRosterReconcilePending`
- ロード直後だけ recruitment / delegation を止める gate
- Soul Perceive 後に pending を外す root completion system
- frame 1 cleanup / frame 2 resume を前提とする受入条件
- B1 未実装時の synthetic transport contribution

UI 変更を原子的な domain commit に直すことで、上記の状態機械を追加せずに保存整合を保証する。

## 3. スコープ

### 対象（In Scope）

- `FamiliarOperation` の Reflect / save schema 対応。
- durable root spawn、runtime shell、load migration の責務分離。
- `hw_core` 所有の永続 `FamiliarPolicy`、既定 rule、WorkType override、正規化 API。
- `WorkType::ALL` と exhaustive stable index。
- operation / policy を変更する typed patch、request、terminal outcome。
- 最大管理 Soul 数減少時の既存 unassign / Bevy Relationship cleanup の同一 Logic tick 適用。
- Familiar task finder の allowed gate と、既存 shared score helper への Familiar priority 接続。
- `PolicyDisabled` rejection evidence、diagnostic class、revision、task dashboard 表示。
- operation dialog の対象固定、scrollable rule editor、一括許可/禁止、全禁止 warning。
- world replacement 時の dialog target / presentation / scroll state reset。
- player-facing Help provider / coverage / approval snapshot。
- save/load、実行中変更、all-disabled、複数 producer、性能の回帰検証。

### 非対象（Out of Scope）

- 新しい距離・円・ポリゴン型の活動範囲。現行 `TaskArea` と AreaEdit UI を再利用する。
- policy 変更時の実行中 task 強制 cancel、即時再割当、予約・relationship の直接破棄。
- recruitment、supervision、休息、stress / escape など WorkType 外の自己維持処理の禁止。
- Familiar rank、昇格、Contract、全 Familiar 共通 template、役職 preset。
- WorkType ごとの作業速度・効率値、任意数値 weight、drag-and-drop priority。
- A3 dashboard 全体の再設計、UI 専用の候補探索・到達判定。
- save container header の新 version。

## 4. 固定する実装契約

### 4.1 durable data model

`crates/hw_core/src/familiar.rs` に次の概念を置く。名前は実装時にこの契約を保つ範囲で調整できる。

```rust
#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FamiliarWorkPriority {
    Low,
    Normal,
    High,
}

#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FamiliarWorkRule {
    pub allowed: bool,
    pub priority: FamiliarWorkPriority,
}

#[derive(Reflect, Debug, Clone, PartialEq, Eq)]
pub struct FamiliarWorkRuleOverride {
    pub work_type: WorkType,
    pub rule: FamiliarWorkRule,
}

#[derive(Component, Reflect, Debug, Clone, PartialEq, Eq)]
#[reflect(Component)]
pub struct FamiliarPolicy {
    pub default_rule: FamiliarWorkRule,
    pub overrides: Vec<FamiliarWorkRuleOverride>,
}
```

- `FamiliarOperation` に `Reflect` と `#[reflect(Component)]` を追加する。
- default policy は全 WorkType `allowed = true / Normal` とし、B2 導入前と等価にする。
- lookup は `rule_for(work_type)` の一箇所に集約する。
- mutation は `set_rule` / `set_all_allowed` / `normalize` を経由する。
- override は WorkType ごとに最大 1 件、`WorkType::ALL` 順、default と同値の entry は保持しない。
- duplicate を受け取った場合の正規化は「後の entry が勝つ」で決定的にする。
- 一括許可/禁止は全 effective rule の `allowed` を指定値へ揃え、各 WorkType の effective priority は保持する。
  その上で `default_rule.allowed` を更新し、default と同値になった override を除去する。
  将来追加された WorkType は、この一括操作後の default を継承する。
- 個別行の変更は default を暗黙に変えない。
- priority は禁止中も保持し、再許可時に以前の Low / Normal / High を復元する。
- policy に Entity、座標、`TaskArea` の複製、`HashMap` を保存しない。

`crates/hw_core/src/jobs.rs` には次を追加する。

- `pub const ALL: [WorkType; N]`
- `pub const fn stable_index(self) -> usize` の exhaustive match
- `ALL` の長さ、一意性、`stable_index` 順を固定する test

`hw_ui` の task list にあるローカル `WORK_TYPES` は削除し、rule editor、task list、normalization、
all-disabled 判定、Help coverage が同じ列挙契約を使う。表示名は既存
`hw_ui::panels::task_list::work_type_label` を再利用する。

### 4.2 typed settings change

UI と domain の間に typed patch を置く。

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FamiliarSettingsPatch {
    AdjustFatigueThreshold { steps: i8 },
    AdjustMaxControlledSoul { delta: i8 },
    SetWorkAllowed { work_type: WorkType, allowed: bool },
    SetWorkPriority { work_type: WorkType, priority: FamiliarWorkPriority },
    SetAllWorkAllowed { allowed: bool },
}
```

- fatigue は `steps * 0.1`、範囲 `0.0..=1.0`、小数 1 桁へ正規化する。
- max soul は UI 契約 `1..=8` へ clamp する。既存 profiling fixture が使用する範囲外値を
  component 型全体の不正値とはみなさず、UI patch のみ clamp する。
- patch value は相対差分または enum で表し、任意 `f32` / `isize` を UI 境界から流さない。

`hw_familiar_ai` は次を公開し、domain ownership を持つ。

```rust
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FamiliarSettingsChangeRequest {
    pub target: Entity,
    pub patch: FamiliarSettingsPatch,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FamiliarSettingsChangeOutcome {
    pub target: Entity,
    pub status: FamiliarSettingsChangeStatus,
}
```

terminal status は最低限、次を区別する。

- `Applied { requested_patches, released_souls, entered_all_work_disabled }`
- `Unchanged { requested_patches }`
- `Rejected { requested_patches, reason: StaleTarget }`
- `Rejected { requested_patches, reason: MissingOperation }`
- `Rejected { requested_patches, reason: MissingPolicy }`
- `Rejected { requested_patches, reason: PausedOrModal }`

root UI adapter は simulation component を触らず、accepted intent を
`FamiliarSettingsChangeRequest` Message へ変換する。Pause / Pause への遷移が pending の frame /
別 modal foreground では request を発行せず、synthetic intent には `Rejected(PausedOrModal)` outcome を返す。
通常 pointer action は `ForegroundUiGate` で source 自体を抑止する。

request consumer は root が配線する `FamiliarSettingsApplySet` で動かす。この set は
`GameSystemSet::Logic` 内の `FamiliarAiSystemSet::Perceive` より前に置き、
`(apply_familiar_settings_change_requests_system, ApplyDeferred).chain()` とする。
Familiar Perceive / Decide / Execute は必ず commit 済み operation / policy / roster を見る。

consumer は一回の read batch を次の順で処理する。

1. request を target ごとにまとめる。target は Entity key の安定順、同 target 内の patch は Message 順を保つ。
2. live `Familiar` と operation / policy を一度だけ再検証する。
3. current operation / policy の local copy へ全 patch を FIFO replay し、target の final state を求める。
4. final max と current roster だけを比較し、現行と同じ reverse roster order で超過 Soul を一度だけ決める。
   例: 同 batch の max `4 → 2 → 4` は final max 4 なので Soul を解放しない。
5. final state が current と異なる component だけを書き戻す。
6. 超過 Soul ごとに `SoulTaskUnassignRequest { emit_abandoned: false }` を発行し、
   `CommandedBy` removal を queue する。
7. target batch ごとに terminal outcome を一件だけ発行する。

直後の専用 `ApplyDeferred` が Relationship target を更新し、その後の Soul Perceive / ApplyDeferred が
task assignment cleanup を同じ Update 内で完了する。この間に別の Familiar simulation system を挟まない。
これにより同 frame の recruitment request と競合せず、Last の save は整合済み state だけを見る。

これにより `FamiliarOperationMaxSoulChangedEvent` と `max_soul_logic_system` は置換する。
operation を先に直接変更して副作用だけを後で処理する二重 writer は残さない。

UI intent は `GameSystemSet::Interface` で request Message へ変換されるため、domain commit は次の Logic tick になる。
同じ frame に F5 が要求された場合は「変更前の最後に commit 済みの状態」を保存し、途中状態は保存しない。
前 frame に accepted request があり、次 frame に Pause へ遷移する場合は、Logic 冒頭の settings commit が先、
Interface の Pause 遷移が後になる。同 frame の settings + Pause は pending foreground capture が Pause を優先して
settings を拒否する。

F1 Help open は例外で、現行 `apply_accepted_help_open_system` が Input 内で Logic より先に
`Time<Virtual>` を pause する。同 system に専用 `MessageReader<FamiliarSettingsChangeRequest>` を持たせ、
毎 frame unread request を最後まで drain する。Help capture が accepted かつ unread request が1件以上ある場合だけ、
この early Help open / pause を見送る。domain consumer の別 reader には影響しない。
`PendingWorldInputCapture` は同 frame 中有効なため背景入力は抑止されたまま、
Logic 冒頭で settings を commit し、その後の通常 `handle_help_intent` が Interface で Help を開いて pause する。
永続 queue や次 frame への Help request 持越しは作らない。この順序、Message reset、save / Help 境界を
integration test で固定する。

### 4.3 spawn / save / load migration

- 新規 Familiar の durable root spawn が `FamiliarOperation::default()` と `FamiliarPolicy::default()` を挿入する。
- `attach_familiar_shell_with_voice` から `FamiliarOperation` の挿入を除く。policy も shell では扱わない。
- `FamiliarOperation` / `FamiliarPolicy` と nested Reflect 型を save registry へ登録する。
- load finalize は runtime shell を付ける前に、欠落 component だけを補う。
  `roster_len` は現行 API の `Commanding.iter().count()` で求める。
  - missing operation:
    - `fatigue_threshold = FamiliarOperation::default().fatigue_threshold`
    - `max_controlled_soul = max(FamiliarOperation::default().max_controlled_soul, roster_len)`
  - missing policy: `FamiliarPolicy::default()`
- 保存済み operation の値は clamp / default 置換しない。
- 保存済み policy は effective semantics を変えずに canonical form へ正規化してよい。
- 補完後も保存済み operation が `roster_len` 未満なら、B2 対応 executable が生成しない破損状態として
  rehydrate finalize を失敗させ、既存 transaction rollback を使う。load 時に Soul を推測で解雇しない。
- migration 中に settings request、notification、speech、task abandonment を発行しない。
- world replacement / rollback reset は未消費 `FamiliarSettingsChangeRequest` と古い
  `FamiliarSettingsChangeOutcome` を消去する。
- 破損 save の rollback test は pre-load の operation / policy、`Commanding` / `CommandedBy`、
  `ManagedTasks`、Soul task / reservation、runtime shell を比較し、`ApplyRecovered` outcome 後も
  元 world が操作可能であることを確認する。
- additive component migration として legacy v0 / header v1 を維持し、header version は上げない。
- deterministic profiling audit は `FamiliarPolicy` の effective rules を checkpoint encoding へ加え、
  必要な audit schema / baseline を B2 後の形式へ更新する。record bytes が増えるため旧 checksum との一致は要求しない。
  B2 前との behavior equality は candidate / score / Top-K の focused test で別に証明する。

### 4.4 task finder と score

policy は Familiar query の crate-owned context に集約し、各 helper が ECS Query を追加取得しない。
`task_finder::find_all_candidates` の順序を次で固定する。

1. 既存の global candidate universe と Familiar / Yard owner 規則を適用する。
2. candidate の `Designation.work_type` を取得する。
3. diagnostics の `observe_applicable` を呼び、candidate membership を記録する。
4. `policy.rule_for(work_type).allowed` を確認する。
5. 禁止なら既存 `CandidateRejectReason::PolicyDisabled` を記録して終了する。
6. 許可された候補だけ `candidate_snapshot`、source / slot / TaskArea / topology / score 評価へ進める。

この位置により、policy は task の存在を隠さず、禁止候補の高コストな後段評価を避ける。
Build / Yard-owned designation の既存 candidate universe は変更しない。

priority は実装済み `policy_score.rs` の seam を使う。

- `PolicyScoreContributions.familiar_units`:
  - Low = `-5`
  - Normal = `0`
  - High = `+5`
- `transport_units` と `familiar_units` は worker ごとの base score 算出後、Top-K 選択前に一度だけ合成する。
- `WORKER_PRIORITY_WEIGHT = 0.65`、`WORKER_DISTANCE_WEIGHT = 0.35`、
  `POLICY_SCORE_UNIT = WORKER_PRIORITY_WEIGHT / 40.0` の既存所有者を変えない。
- 最終 score を clamp しない。
- default Normal は score bit pattern、Top-K、fallback、tie-break を B2 前と同じにする。
- transport の -10..+20 と Familiar の -5..+5 を合わせた最大 span は 40 unit とし、
  `WORKER_PRIORITY_WEIGHT` を超えない。
- policy 変更で `AssignedTask` / `ManagedTasks` / reservation を剥がさず、
  次の通常 delegation cycle の未割当候補だけへ反映する。
- recruitment、supervision、休息、stress / escape は WorkType policy を参照しない。

### 4.5 diagnostics

`TaskDiagnosticClass` と `TaskBlockerReason` に `PolicyDisabled` を追加する。

- `CandidateRejectReason::PolicyDisabled` を candidate path で記録する。
- idle worker を持つ applicable Familiar evaluator が policy gate に到達し、その evaluator の terminal reason が
  policy 拒否だけの場合に `PolicyDisabled` vote を作る。
- policy gate へ到達する Familiar が 0 の場合は既存 `NoEligibleFamiliar` を維持する。
- idle worker が 0 の evaluator は、policy より先に既存 `NoEligibleFamiliar` へ正規化する。
- Familiar producer 全体では、`PolicyDisabled` terminal vote 数が applicable Familiar evaluator 数と一致する
  complete rejection のときだけ policy-only とする。一体でも許可後の別 terminal reason、submit、partial、
  stale、coverage 不足があれば policy-only にしない。
- 既存 discriminant を動かさず `PolicyDisabled = 5`、`COUNT = 6` とする。
- representative order は既存5分類の相対順を変えず、`PolicyDisabled` を末尾へ加える。
  policy-only の成立は順位ではなく上記の全票条件で保証する。
- fixed discriminant / index、固定長 counter、snapshot、exhaustive mapping test を同時に更新する。
- `TaskDiagnosticDomainMask::for_class(PolicyDisabled)` は `TASK | ROSTER` とする。
- `Changed<FamiliarPolicy>` と removal を既存 Familiar eligibility bridge に加え、`roster` revision を bump する。
  新しい revision field は追加しない。
- diagnostics 専用の候補探索、source scan、pathfinding は追加しない。

root `view_model::producer_evidence` / `derive_task_status` 相当の集約は次で固定する。

```text
if taskにworkerがいる:
    Working

各applicable producerについて:
    header欠落/stale/completed_evaluators != eligible_evaluators:
        PendingEvaluation
    header.eligible_evaluators == 0:
        record無しでcompleteなNoEligibleFamiliar evidence
    eligible_evaluators > 0 かつ
      record欠落/stale/submitted/!record.coverage.is_complete_rejection():
        PendingEvaluation

familiar.policy_only =
    record.coverage.is_complete_rejection()
    && terminal_votes > 0
    && policy_count == terminal_votes

if !familiar.policy_only:
    representative選択前にPolicyDisabled countを0として扱う

if auto-buildが非applicable && familiar.policy_only:
    Blocked(PolicyDisabled)
else:
    current/complete producerのnon-policy countersだけをmergeして代表理由を選ぶ
```

unowned Blueprint `Build` は Familiar delegation と Blueprint auto-build の両 producer を持つ。
`PolicyDisabled` は Familiar producer の terminal vote にすぎず、次を守る。

- task に worker がいれば `Working` を最優先する。
- Blueprint auto-build が submit した cycle は Pending、worker 反映後は Working とする。
- Blueprint auto-build record が欠落、partial、stale なら root reducer は Pending を維持する。
- 両 producer が current / complete rejection の場合、Familiar の policy voteを代表理由候補から除外し、
  policy 対象外の auto-build 側を含む non-policy reason を選ぶ。
- したがって auto-build が applicable な unowned Blueprint Build の最終 blocker を
  `PolicyDisabled` 単独にはしない。
- `ManagedBy` 付き Blueprint / non-Blueprint Build は既存どおり Familiar producer のみで判定する。

### 4.6 operation dialog、notification、Help

`hw_ui` に runtime-only `OperationDialogState { target: Option<Entity> }` を置く。

- `MenuAction` は `UiIntent` の alias なので、単一 variant
  `UiIntent::OpenOperationDialog { opener: Option<Entity>, target: Entity }` を使う。
- dynamic context menu の button は同 variantを `opener: None` で保持し、pressed adapter が
  `opener: Some(button_entity)` に差し替えて書き出す。
- `PendingWorldInputCapture::accepts(InputOverlay::OperationDialog, opener)` が成立し、target が同じ live Familiar
  であると root が再検証した時点だけ target を latch する。rejected open は state を変更しない。
- dialog 表示中に `SelectedEntity` が変わっても編集対象を切り替えない。
- target が despawn、Familiar でなくなる、必要 component を失う場合は dialog を閉じて state を消す。
- close、Escape、F9 world replacement、rollback recovery は target を消し、root `Node.display` を同期的に
  `Display::None` にし、scroll position を 0 に戻す。
- dynamic Entity を埋めた static button data は reset 後に残さない。

dialog 内 action は target を含めず `FamiliarSettingsPatch` だけを表し、root adapter が latched target と組にして
domain request を発行する。entity-list の max soul actionは明示 target と同じ request pathへ変換する。
root handler は `FamiliarOperation`、`FamiliarPolicy`、entity-list `Text` を直接変更しない。
Pause / higher-priority modal の foreground では操作 button を受理せず、synthetic intent も
`Rejected(PausedOrModal)` として state を変更しない。

表示は次を満たす。

- current operation / effective policy から毎回 view を更新する。
- 16 WorkType 行で Enabled / Disabled と Low / Normal / High を識別できる。
- Enable all / Disable all を提供する。
- all-disabled のとき「新しい作業は割り当てない。現在作業と自己維持は継続する」を常時表示する。
- 通常 Applied / Unchanged は inline表示とdirty rebuildだけにし、+/-ごとのtoastを出さない。
- all-disabled へ遷移した Applied は warning、Soul解放を伴うAppliedは解放数付きinfo、
  Rejectedは warning/error 相当のToastOnly notificationへ変換する。
- entity-list header は `Changed<FamiliarOperation>` / outcome による通常 dirty rebuild で更新し、
  handler から文字列を直接書き換えない。

Bevy 0.19 の UI は既存 Help / entity-list pattern を再利用する。

- bounded dialog body
- `Overflow::scroll_y()`
- `ScrollPosition`
- `bevy_ui_widgets::ScrollArea`
- standard `Scrollbar` / `ScrollbarThumb`

Help は player-facing impact ありと判定する。M5 で最低限、次を更新する。

- `interface/ui/help_content/providers/familiars.rs` の既存 Familiar topicへ stable entry
  `familiar-operation-policy` を追加する
- entry は疲労閾値、最大使役数、WorkType 許可、Low / Normal / High、全禁止、現在 task / 自己維持継続、
  Familiar ごとの保存を説明する
- operation open / close、settings patch、`FamiliarWorkPriority` の exhaustive `coverage.rs`
- `coverage_approval.snap`
- 新 topic を追加する場合のみ `manifest.rs`
- Help ownership / workflow 自体を変える場合のみ `docs/help-screen.md`

### 4.7 設計判断

| ID | 判断 |
| --- | --- |
| B2-D01 | `FamiliarOperation` / `FamiliarPolicy` は durable、AI state / visual / dialog target は runtime |
| B2-D02 | 活動範囲は既存 `TaskArea` を唯一の正本とし、新型を追加しない |
| B2-D03 | policy と patch value は `hw_core::familiar`、request / outcome consumer は `hw_familiar_ai` が所有する |
| B2-D04 | UI は request adapter と presentation に限定し、simulation component を直接 mutate しない |
| B2-D05 | settings apply + flush を Familiar Perceive より前へ置き、operation 更新と超過 roster 解放を一つの Logic commit にする |
| B2-D06 | old-save missing max は roster-aware に補完し、既存 Soul を load migration で解雇しない |
| B2-D07 | policy priority は既存 shared score の `familiar_units = -5 / 0 / +5` を使う |
| B2-D08 | all-disabled は有効な idle policy。実行中 task と自己維持挙動は継続する |
| B2-D09 | policy gate は diagnostics membership 記録後、`candidate_snapshot` 前に置く |
| B2-D10 | `PolicyDisabled` は全 applicable Familiar evaluator が policy-only の場合だけ成立する |
| B2-D11 | diagnostics は既存 `roster` revision と複数 producer reducer を使い、unowned Build では policy vote を最終理由にしない |
| B2-D12 | 同 target の同 batch patch は FIFO replay後の final stateへ一度だけ commitし、副作用をcoalesceする |
| B2-D13 | Pause / higher modal 中は settings intentを抑止またはtyped rejectionにし、Messageを保留しない |
| B2-D14 | 未処理request直後のF1はearly pauseだけを同frame Logic後へ遅らせ、Help capture自体は維持する |
| B2-D15 | dialog target は accepted opener + exact target で固定し、world replacement で同期 reset する |
| B2-D16 | save container は additive v1 のまま維持する |

## 5. マイルストーン

### M1: durable model、spawn、save migration

#### 変更内容

- `FamiliarOperation` を Reflect component にする。
- `FamiliarPolicy`、rule、priority、override、normalization API を追加する。
- `WorkType::ALL` / `stable_index` を追加し、task-list の重複配列を置換する。
- durable root spawn と runtime shell を分離する。
- save registry、old-save missing migration、round-trip、audit checkpoint encoding / schema を接続する。

#### 主な変更ファイル

- `crates/hw_core/src/familiar.rs`
- `crates/hw_core/src/jobs.rs`
- `crates/bevy_app/src/entities/familiar/spawn.rs`
- `crates/bevy_app/src/systems/save/schema.rs`
- `crates/bevy_app/src/systems/save/schema/tests.rs`
- `crates/bevy_app/src/systems/save/rehydrate.rs`
- `crates/bevy_app/src/systems/save/rehydrate/tests/`
- `crates/bevy_app/src/plugins/startup/perf_scenario/audit_encoding.rs`
- `crates/hw_ui/src/panels/task_list/types.rs`

#### 完了条件

- [x] new spawn は operation / policy default を持つ。
- [x] shell attach を複数回行っても durable values は変わらない。
- [x] legacy v0 / v1 fixture の欠落 operation は roster 数以上の max で補完される。
- [x] missing policy は全許可 / Normal になる。
- [x] 保存済み operation は上書きされず、policy は effective semantics を保って正規化される。
- [x] new save round-trip 後の threshold / max / effective rules / canonical overrides が一致する。
- [x] app が生成しない saved max < roster は `ApplyRecovered` になり、pre-load の durable relationship /
  task / reservation、runtime shell、UI reset 後の操作性が復元される。
- [x] `WorkType::ALL` が全 variant を一度ずつ安定順で含む。

#### focused verification

- `cargo test -p hw_core familiar`
- `cargo test -p hw_core work_type`
- `cargo test -p bevy_app@0.1.0 --lib systems::save`
- `cargo test -p hw_ui task_list`

### M2: atomic settings request / outcome

#### 変更内容

- `FamiliarSettingsPatch`、request / terminal outcome Message を追加する。
- `FamiliarSettingsApplySet` を Familiar Perceive 前へ登録し、直後に `ApplyDeferred` を置く。
- 同 target の patch を FIFO replayして final stateへcoalesceする。
- final max 減少時の deterministic excess selection、task unassign、Relationship cleanup を統合する。
- `FamiliarOperationMaxSoulChangedEvent` と旧 `max_soul_logic_system` を削除する。
- Pause / modal rejection、same-frame save、world-replace Message reset、stale / unchanged / failure をテストする。

#### 主な変更ファイル

- `crates/hw_core/src/familiar.rs`
- `crates/hw_core/src/events.rs`
- `crates/hw_familiar_ai/src/familiar_ai/settings.rs`
- `crates/hw_familiar_ai/src/familiar_ai/execute/mod.rs`
- `crates/hw_familiar_ai/src/familiar_ai/execute/max_soul_logic.rs`（置換後に削除）
- `crates/hw_familiar_ai/src/familiar_ai/mod.rs`
- `crates/bevy_app/src/plugins/messages.rs`
- `crates/bevy_app/src/systems/familiar_ai/mod.rs`
- `crates/bevy_app/src/interface/ui/interaction/intent_context.rs`
- `crates/bevy_app/src/interface/ui/help_controller.rs`
- `crates/bevy_app/src/systems/soul_ai/mod.rs`

#### 完了条件

- [x] operation / policy の domain writer が一箇所になる。
- [x] settings apply / flush は Familiar Perceive / Decide / Execute より前に完了する。
- [x] max 4 → 2 は current reverse roster order の2体だけを一回解放する。
- [x] 同 batch の max 4 → 2 → 4 は final stateへcoalesceし、Soulを解放しない。
- [x] released Soul の `CommandedBy` と task assignment cleanup が同じ Update 内に反映される。
- [x] commit 後の保存対象 state は `roster_len <= max_controlled_soul` を満たす。
- [x] target batch ごとに Applied / Unchanged / stale / missing component の terminal outcome が一件になる。
- [x] Pause / higher modal の通常 action は抑止され、synthetic intent は一件の rejection outcome になり state を変えない。
- [x] accepted request の次 frame に Pause へ遷移しても settings commit が先に完了する。
- [x] accepted request の次 frameにF1を押すと、early Help pauseを同frameのLogic後まで遅延し、
  settings commit後にHelpが開く。HelpをMessage retention期間より長く表示して閉じても設定は維持され、
  Help pause guardは通常どおり時刻を復元し、背景inputも漏れない。
- [x] UI request と F5 が同 frame の場合、最後に commit 済みの整合 state を保存する。
- [x] F9 / rollback は未消費 request / 古い outcome を消し、新 world へ stale target を流さない。
- [x] migration / load は settings outcome や task abandonment を偽発火しない。

#### focused verification

- `cargo test -p hw_familiar_ai familiar_settings`
- `cargo test -p hw_soul_ai task_unassign`
- `cargo test -p bevy_app@0.1.0 save_load_apply`
- `cargo test -p bevy_app@0.1.0 help_controller`

### M3: policy filter と既存 shared score への接続

#### 変更内容

- Familiar Decide query/context に `&FamiliarPolicy` を集約する。
- `observe_applicable` 後 / `candidate_snapshot` 前に allowed gate を追加する。
- existing `PolicyScoreContributions.familiar_units` を effective priority から設定する。
- default equality、mixed B1/B2 contribution、Top-K / fallback、現在 task 継続を固定する。

#### 主な変更ファイル

- `crates/hw_familiar_ai/src/familiar_ai/decide/query_types.rs`
- `crates/hw_familiar_ai/src/familiar_ai/decide/delegation_context.rs`
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_delegation.rs`
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/task_finder/mod.rs`
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/policy_score.rs`
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/delegation/assignment_loop.rs`

#### 完了条件

- [x] default policy の candidate IDs、base score、final score、Top-K、tie-break が baseline と一致する。
- [x] disabled candidate は applicable evidence を残し、`candidate_snapshot` 以降へ進まない。
- [x] Low / Normal / High は同条件で -5 / 0 / +5 unit になる。
- [x] B1 transport と B2 Familiar の contribution は一度だけ加算され、順序に依存しない。
- [x] 合算最大 span は 40 unit、最終 score は no-clamp である。
- [x] policy 変更時も現在 task、reservation、`ManagedTasks` は変更されない。
- [x] all-disabled でも recruitment / supervision / rest / stress path は継続する。

#### focused verification

- `cargo test -p hw_familiar_ai policy_score`
- `cargo test -p hw_familiar_ai task_finder`
- `cargo test -p hw_familiar_ai task_management`

### M4: diagnostics と task dashboard

#### 変更内容

- `PolicyDisabled` reject reason / diagnostic class / blocker label を追加する。
- fixed counter、representative order、domain mask を更新する。
- policy change / removal を既存 `roster` revision bridge に接続する。
- Familiar と Blueprint auto-build の複数 producer 集約を固定する。

#### 主な変更ファイル

- `crates/hw_jobs/src/diagnostics.rs`
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/diagnostics.rs`
- `crates/bevy_app/src/systems/familiar_ai/diagnostics.rs`
- `crates/bevy_app/src/interface/ui/panels/task_list/view_model.rs`
- `crates/hw_ui/src/panels/task_list/types.rs`

#### 完了条件

- [x] idle worker を持つ applicable Familiar evaluator が全て policy-only のときだけ
  Familiar producer が `PolicyDisabled` になる。
- [x] policy gate に到達する Familiar がない場合は `NoEligibleFamiliar` のままである。
- [x] 多数の Familiar が禁止でも、一体が許可後に別 terminal reason を出せばその non-policy reason を使う。
- [x] submit / partial / stale / coverage 不足の混在は `PendingEvaluation` を維持する。
- [x] policy を許可へ戻すと roster revision が進み、次の current cycle で blocker が消える。
- [x] unowned Blueprint Build は auto-build record の欠落 / stale / submit で Pending/Working を維持し、
  complete rejection では auto-build 側の non-policy reason を選ぶ。
- [x] unowned Blueprint Build で Familiar が policy-only、auto-build header が current / complete /
  eligible 0 の場合は、record 無しでも auto-build 側 `NoEligibleFamiliar` を代表候補にする。
- [x] managed Blueprint / non-Blueprint Build の全 Familiar 禁止は `PolicyDisabled` になる。
- [x] `COUNT`、counter、mapping、label、snapshot の exhaustive tests が揃う。
- [ ] dashboard hidden / visible の定量counter比較は
  `familiar-operation-policy-validation-plan-2026-07-26.md` とA3性能計画へ移管した。

#### focused verification

- `cargo test -p hw_jobs diagnostics`
- `cargo test -p hw_familiar_ai diagnostics`
- `cargo test -p bevy_app@0.1.0 task_list`
- `cargo test -p hw_ui task_list`

### M5: operation dialog、notification、Help

#### 変更内容

- latched `OperationDialogState` と world-replace reset を追加する。
- operation open intent に capture `opener` と exact Familiar `target` を持たせる。
- operation dialog を bounded scrollable WorkType editor にする。
- dialog / entity-list intent を同じ domain request path へ変換する。
- direct component / text mutation を削除し、outcome notification と dirty rebuild を接続する。
- all-disabled warning、Help provider / coverage / approval snapshot を更新する。

#### 主な変更ファイル

- `crates/hw_ui/src/intents.rs`
- `crates/hw_ui/src/components.rs`
- `crates/hw_ui/src/setup/dialogs.rs`
- `crates/hw_ui/src/interaction/dialog.rs`
- `crates/hw_ui/src/lib.rs`
- `crates/bevy_app/src/interface/ui/interaction/menu_actions.rs`
- `crates/bevy_app/src/interface/ui/interaction/intent_context.rs`
- `crates/bevy_app/src/interface/ui/interaction/intent_handler.rs`
- `crates/bevy_app/src/interface/ui/interaction/handlers/general.rs`
- `crates/bevy_app/src/interface/ui/interaction/handlers/familiar_settings.rs`
- `crates/bevy_app/src/interface/ui/interaction/systems.rs`
- `crates/bevy_app/src/interface/ui/notifications.rs`
- `crates/bevy_app/src/interface/ui/plugins/notifications.rs`
- `crates/bevy_app/src/interface/ui/help_content/providers/familiars.rs`
- `crates/bevy_app/src/interface/ui/help_content/coverage.rs`
- `crates/bevy_app/src/interface/ui/help_content/coverage_approval.snap`

#### 完了条件

- [x] 自動テストで dialog を Familiar A で開いた後に `SelectedEntity` を B へ変更しても edit は A を対象にする。
- [x] accepted opener + exact target だけが state を latch し、rejected capture は dialog を開かない。
- [x] stale / despawn target は別 Familiar へ retarget せず、dialog を閉じる。
- [x] F9 / rollback 後は dialog、target、button target、scroll position が旧 Entity を保持しない。
- [x] Pause / higher modal 中は settings action を発行せず、synthetic intent も state を変えない。
- [x] 全 WorkType の effective allowed / priority と all-disabled warning を認識できる。
- [x] dialog body は小さい window でも画面外へ溢れず、wheel / scrollbar で最終行へ到達できる。
- [x] entity-list action と dialog action が同じ request / outcome semantics を持つ。
- [x] Help の `familiar-operation-policy` entry から operation / role policy / all-disabled の意味を確認できる。
- [x] 新しい player-facing action が exhaustive coverage と exact approval snapshot に分類される。

#### focused verification

- `cargo test -p hw_ui operation_dialog`
- `cargo test -p bevy_app@0.1.0 familiar_settings`
- `cargo test -p bevy_app@0.1.0 help_content`

### M6: 横断回帰、性能、恒久ドキュメント、plan 完了

#### 変更内容

- save/load、AI、diagnostics、UI、Help の統合シナリオを通す。
- determinism schema v2のencodingとdisabled gateの後段call suppressionを自動回帰で固定する。
- 恒久ドキュメントと生成 index を同期する。
- code /自動回帰/docs gateの完了後、実renderer受入と計測artifactは独立follow-upへ移し、
  本実装計画をarchiveする。

#### 更新対象ドキュメント

- `docs/familiar_ai.md`
- `docs/tasks.md`
- `docs/events.md`
- `docs/entity_list_ui.md`
- `docs/info_panel_ui.md`
- `docs/task_list_ui.md`
- `docs/save_load.md`
- `docs/invariants.md`
- `docs/cargo_workspace.md`
- Help contract 自体を変えた場合のみ `docs/help-screen.md`

#### 完了条件

- [x] code /自動回帰としてsave/load、AI、diagnostics、UI、Helpの横断ケースが成功する。
- [x] determinism schema v2がoperationと全effective policyを安定encodingし、Rust/Pythonのversionが一致する。
- [x] disabled candidateはapplicable evidenceを残し、candidate snapshot / source / reachability / scoreへ進まない。
- [x] 未実施のplayer-visible受入、fixed-step artifact、UI mode counterを
  `familiar-operation-policy-validation-plan-2026-07-26.md`へ移管する。
- [x] 恒久 docs と generated indexes が current implementation と一致する。
- [x] `python3 scripts/perf.py self-test` が成功する。
- [x] `python3 scripts/dev.py verify` が成功する。
- [x] 計画を `docs/plans/archive/` へ移し、index を更新する。

#### verification

- `python3 scripts/perf.py self-test`
- `python3 scripts/dev.py docs --write`
- `python3 scripts/dev.py verify`
- `git diff --check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| shell helper が durable 値を再挿入する | load ごとに設定が失われる | root/shell boundary と idempotence test を固定する |
| UI と side effect が別 tick で変更される | max < roster の中間 save ができる | domain consumer に値変更・解放を統合し、旧 event writer を削除する |
| UI request と同 frame の F5 が期待と違う | クリック直後の値が保存されない | next Logic commit / last committed save の契約を UI test と docs に明記する |
| settings apply 前に Familiar Decide が走る | 旧 max で recruit し、flush 後に上限超過する | 専用 set を Familiar Perceive 前へ置き、apply + flush を先に完了する |
| 同 target の max request を個別処理する | deferred roster を再読して同じ Soul を二重解放する | patch batch を final state へcoalesceし、副作用を一度だけ計算する |
| Pause / modal 中に request を保留する | Message retention や stale target に依存する | foreground gate と typed rejection で受理せず、保留 queue を作らない |
| request直後にF1 Helpがearly pauseする | Logic consumerが止まりMessageが失効する | request bufferが非空ならearly pauseだけを見送り、同frame Logic後のInterfaceでHelpを開く |
| 旧 save の roster を default max 2 へ縮める | 意図しない Soul 解放 | missing max は `max(default, roster_len)` で補完し、migration で解放しない |
| 不整合な新 save を load-time cleanup する | task / relationship を推測で破壊する | rehydrate を失敗させ transaction rollback する |
| default policy で割当順が変わる | gameplay regression | Normal=0 と既存 `PolicyScoreContributions` の bit-level equality test を置く |
| gate が早すぎる | applicable evidence が消え `PolicyDisabled` を出せない | `observe_applicable` 後 / `candidate_snapshot` 前を固定する |
| gate が遅すぎる | 禁止候補へ source / reachability work を行う | downstream call counter test を置く |
| policy reject と許可後の別 reject が混在する | 多数決で誤った `PolicyDisabled` になる | 全 applicable evaluator が policy-only の場合だけ成立させる |
| `PolicyDisabled` が他 producer を上書きする | Build blocker が誤る | unowned Build では policy vote を除外し、producer completion / current-cycle reducer をテストする |
| policy change が revision に反映されない | blocker snapshot が stale のまま | `Changed` と removal を existing roster revision bridge に追加する |
| dialog が live selection を参照する | 意図しない Familiar を編集する | accepted open target を latch し、表示・intent が同じ state を読む |
| F9 後に static UI が旧 Entity を保持する | stale entity 操作・誤表示 | state、button target、root display、scroll を synchronous reset する |
| WorkType 追加時に一部 UI が欠落する | policy / Help / task list が不一致 | `WorkType::ALL`、stable index、coverage exhaustive test を正本にする |
| rule editor が画面外へ溢れる | 最終行を操作できない | Bevy 0.19 の既存 ScrollArea / Scrollbar pattern を再利用する |

## 7. 検証計画

### 自動検証マトリクス

| 領域 | 必須ケース |
| --- | --- |
| model | default、lookup、個別 override、重複 last-wins、default 同値削除、set-all、priority 保持、all-disabled |
| WorkType | `ALL` の全 variant / 一意性 / stable order、task-list と rule editor の共有 |
| spawn | new root の durable defaults、shell attach の非上書き・idempotence |
| save | v0/v1 missing migration、roster-aware max、new round-trip、policy canonicalization、破損 state rollback |
| settings | fatigue/max clamp、unchanged、stale、missing component、batch coalesce、deterministic release、targetごとのoutcome |
| schedule | pre-Perceive apply/flush、request frameのF5は旧commit、次frame Pause/F1よりsettings先、Pause/modal reject、F9 Message clear |
| AI filter | applicable 記録後の reject、candidate snapshot / downstream work 0、default candidate equality |
| score | -5/0/+5、B1+B2 span 40、no-clamp、Top-K / fallback / tie-break equality |
| task continuity | policy 禁止後も current task / reservation / relationship を維持し、次候補だけ拒否 |
| self-maintenance | all-disabled でも recruitment / supervision / rest / stress path を壊さない |
| diagnostics | zero idle、全 policy-only、許可後の別 reject、submit/partial/stale、revision、reset、class discriminant |
| multiple producer | unowned Build の auto-build record欠落/stale/submitted/complete reject、managed Build、Working優先 |
| UI | accepted opener + exact target、rejected capture、test-only selection change、stale close、scroll、notification |
| reset | F9 / rollback で dialog target / display / scroll / request/outcome Message を消去 |
| Help | provider content、manifest ownership、UiIntent coverage、exact approval snapshot |

### workspace gate

- focused tests は各 milestone で実行する。
- 実装完了時:
  - `cargo check --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `python3 scripts/dev.py docs --check`
  - `python3 scripts/dev.py verify`
  - `git diff --check`
- Rust-analyzer diagnostics を確認し、error / unresolved import を残さない。

### player-visible 手動確認

以下の実renderer / pointer確認は実装完了条件から分離し、
`familiar-operation-policy-validation-plan-2026-07-26.md` M1へ未実施のまま移管した。
Operation dialog foreground では現行 binding 上 F5 / F9 が抑止されるため、手動 save/load は dialog を閉じて行う。
visible dialog のまま強制 world replacement する reset ケースは自動テストで固定する。

1. Familiar A を threshold 70%、max 3、Chop Disabled、Haul High にし、Applied outcome または
   committed 表示への反映を確認してから dialog を閉じ、F5 で保存する。
2. A を別値へ変更して committed 表示を確認後に dialog を閉じ、F9 confirm 後に保存時の
   operation / policy / `TaskArea` が戻ることを確認する。
3. load 直後に operation root / target / scroll が残っておらず、A を再選択して開くと保存値、B で開くと B の値を表示することを確認する。
4. Pause / LoadConfirm / Settings など高優先 overlay から背景の operation / entity-list 設定を操作できないことを確認する。
5. A を Haul High / Build Disabled、B を Build High / Haul Disabled にし、同じ task priority 内で役割分担を確認する。
6. 実行中 task の WorkType を禁止し、その task は通常終了し、次の同種 task は選ばないことを確認する。
7. idle な commanded Soul が1体以上いる Familiar と Familiar-only の applicable Chop task を用意し、
   全禁止の warning と `PolicyDisabled` blockerを確認する。休息・recruitment・supervision は継続する。
8. max を roster 未満へ下げ、通知数、解放対象、task cleanup、entity-list 表示が一回だけ更新されることを確認する。
9. unowned Blueprint Build で Familiar を禁止しても、auto-build が成功可能なら誤った Blocked 表示にならないことを確認する。
10. 小さい window で WorkType 最終行まで scroll でき、Help の `familiar-operation-policy` entry を確認する。

### performance / determinism

実artifact採取とUI mode counter比較は
`familiar-operation-policy-validation-plan-2026-07-26.md` M2へ未実施のまま移管した。

- `gather` の同一 seed / fixture / build を更新後 audit schema で反復し、既存名称の state checkpoint checksum が一致することを確認する。
- B2 前との default behavior equality は別の focused testで candidate IDs / score / Top-K / tie-break を比較する。
- profiling counter で、policy reject candidate が `candidate_snapshot`、source selector、
  `reachable_with_cache`、worker scoringへ進まず、該当 counter を増やさないことを確認する。
- dialog hidden / open、task dashboard hidden / visible で simulation checksum と AI counter が一致する。
- 性能 artifact は通常の docs / git 対象へ追加しない。

## 8. ロールバック方針

- M1 durable schema、M2 request path、M3 AI、M4 diagnostics、M5 UI/Help を独立した変更単位にする。
- UI を戻しても durable policy は default behavior で安全に保持できる。
- AI 接続を戻す場合は policy component を削除せず、全候補を default allow / Normal 相当として扱う。
- new component を含む save を読む executable 互換性を検証し、未知 component を黙って捨てない。
- rollback で実行中 task、`Commanding`、`ManagedTasks`、reservation を直接解除しない。
- load migration が失敗した場合は既存 transaction rollback を使い、部分的に置換された world を継続しない。

## 9. AI 引継ぎメモ

### 現在地

- 進捗: `実装・自動回帰・Help・恒久docs 100%`
- 完了済み: M1〜M6のproduction code、自動correctness、workspace gate、plan archive
- 独立移管: 実renderer / pointer受入、fixed-step / profiling artifact採取

### 最初に行うこと

1. player-visible / performance検証を続ける場合は
   `docs/plans/familiar-operation-policy-validation-plan-2026-07-26.md`を正本にする。
2. dashboard mode harnessはA3性能計画のownershipを確認し、重複実装しない。
3. B2 production仕様を変更する場合は本書のB2-D01〜D16と恒久docsを先に照合する。

### ブロッカー / 注意点

- `FamiliarOperation` / `FamiliarPolicy` はdurableであり、runtime shellへ戻してはならない。
- UIはtyped request adapterであり、domain componentを直接変更してはならない。
- settings requestはPause / higher modalでは発行・保留しない。
- settings apply / `ApplyDeferred` は Familiar Perceive より前に置き、同 target batch を final stateへcoalesceする。
- load 後 roster reconciliation を追加しない。旧 save は operation 自体を持たないため roster-aware default で足りる。
- policy gate を `observe_applicable` より前へ移すと diagnostics evidence が消える。
- B1/B2は同じ`policy_score.rs`と`PolicyScoreContributions`を使う。別constant所有を作らない。
- `PolicyDisabled` は多数決にしない。許可後の別 reject が一票でもあれば policy-only ではない。
- unowned Blueprint Build は複数 producer 集約であり、Familiar の policy vote を最終 blocker にしない。
- operation dialog は live `SelectedEntity` ではなく latched target を読む。
- all-disabled は valid state であり、自動で default へ戻さない。
- Help provider / coverage / exact approval snapshotをplayer-facing変更から外さない。
- Bevy UI は 0.19 の `ScrollArea` / `ScrollPosition` / `Scrollbar` を既存コードと一次資料に合わせる。

### 参照必須ファイル

- `docs/proposals/gameplay-management-improvements-proposal-2026-07-17.md`
- `docs/invariants.md`
- `docs/familiar_ai.md`
- `docs/tasks.md`
- `docs/task_list_ui.md`
- `docs/save_load.md`
- `docs/help-screen.md`
- `crates/hw_core/src/familiar.rs`
- `crates/hw_core/src/jobs.rs`
- `crates/bevy_app/src/entities/familiar/spawn.rs`
- `crates/bevy_app/src/systems/save/`
- `crates/hw_familiar_ai/src/familiar_ai/settings.rs`
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/`
- `crates/bevy_app/src/systems/familiar_ai/diagnostics.rs`
- `crates/hw_ui/src/setup/dialogs.rs`
- `crates/bevy_app/src/interface/ui/interaction/handlers/familiar_settings.rs`
- `crates/bevy_app/src/interface/ui/help_content/`

### Definition of Done

- [x] M1〜M6 のproduction実装と自動correctnessが完了
- [x] old-save migration と new-save round-trip が成功
- [x] operation / roster が atomic domain commit 後に整合する
- [x] `WorkType::ALL`、allowed gate、-5/0/+5 score、default equality が自動テスト済み
- [x] current task 継続、all-disabled、self-maintenance、multiple producer diagnostics が自動テスト済み
- [x] dialog target / F9 reset / Help / notification がheadless integrationで自動テスト済み
- [x] 未実施のplayer-visible / performance検証が独立follow-upへ移管済み
- [x] `TaskArea` が活動範囲の唯一の正本
- [x] `python3 scripts/dev.py verify` が成功
- [x] 恒久 docs 更新後に本計画を archive

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-07-20` | `Codex` | Track B2 の operation 非上書き永続化、WorkType policy、TaskArea 再利用、A3 blocker、UI を計画化 |
| `2026-07-21` | `Codex` | B1/B2 priority を既存 worker score 後の shared no-clamp offset として具体化 |
| `2026-07-26` | `Codex` | 現行実装を再監査。B1 実装済み seam を前提化し、UI direct mutation を atomic domain request へ置換。不要な load roster reconciliation を削除し、gate位置、複数producer診断、dialog target、Help、F5/F9受入を具体化 |
| `2026-07-26` | `Codex` | M1〜M6のproduction実装、自動回帰、Help、恒久docs、workspace gateを完了。未実施の実機受入と性能artifactを独立検証計画へ移管してarchive |
