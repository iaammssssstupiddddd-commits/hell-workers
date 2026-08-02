# Track B2 Familiar 運用ポリシー 実機・性能検証フォローアップ計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `familiar-operation-policy-validation-plan-2026-07-26` |
| ステータス | `Completed` |
| 作成日 | `2026-07-26` |
| 最終更新日 | `2026-08-02` |
| 作成者 | `Codex` |
| 関連提案 | `docs/proposals/gameplay-management-improvements-proposal-2026-07-17.md`（Track B2） |
| 実装完了記録 | `docs/plans/archive/familiar-operation-policy-plan-2026-07-20.md` |
| 関連計画 | `docs/plans/archive/task-dashboard-performance-validation-plan-2026-07-20.md` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: B2の受入項目が、既存自動テストで確定済みの契約、追加harnessが必要な客観検証、
  実renderer / pointerでしか判断できないUX確認を1行へ混在させていた。
- 到達したい状態:
  - 自動テストで確定した項目は、正確なtest名と結果を記録して実機checklistから外す。
  - 実機checklistは、可読性、実pointer hit-test、player-visibleなF5/F9、実gameplay観察へ限定する。
  - default / disabled policyを同一fixtureで比較し、disabled gateが後段workを減らし、
    dialog / dashboard表示がsimulationやAI workを増やさないことを有効なartifactで確認する。
- 成功指標:
  - M1-Aの自動correctness表が全項目`PASS`である。
  - M1-Bの実機checklistが全項目合格し、失敗時は観測事実と再現手順を残す。
  - M2のfixed-step checksumと対象counterが契約どおり一致／減少し、無効runを結果から除外しない。

## 2. スコープ

### 対象（In Scope）

- Familiar A/BのOperation dialog、exact target、scroll、overlay foreground。
- fatigue / max / allowed / priorityの表示とF5/F9 round-trip。
- policy変更後の新規割当、実行中task継続、all-disabled、`PolicyDisabled`表示。
- 最大数減少時の一回限りのrelease、task cleanup、通知、Entity List表示。
- unowned Blueprint Buildの複数producer表示。
- fixed-step determinism schema v3とcontrolled policy fixture。
- dialog hidden/open、dashboard hidden/visibleのAI work同一性。dashboard専用harnessの実装は
  関連するA3性能計画を正本とする。

### 非対象（Out of Scope）

- B2 domain / UI仕様の再設計。
- 手動確認を自動correctness testの代わりにすること。
- 異なるseed、fixture、build、schema、GPU/backend間の性能比較。
- A3 dashboard性能harnessとの重複実装。
- raw perf artifactのcommit。

## 3. 現状とギャップ

### 3.1 判定区分

| 区分 | 意味 | 完了の根拠 |
| --- | --- | --- |
| `AUTO PASS` | rendererを使わず客観的に確定済み | focused testまたはfull gateの成功 |
| `RUNTIME PASS` | 実windowは起動するが目視判断を使わない | runnerが生成したvalid artifact |
| `MANUAL PENDING` | 実renderer、実pointer、可読性、実gameplay観察が必要 | M1-Bへ日時・環境・結果を記録 |
| `HARNESS PENDING` | 客観検証可能だが現行fixture / counterに入口がない | harness実装後のvalid artifact |

`AUTO PASS`はplayer-visibleな見た目まで保証しない。反対に、`MANUAL PENDING`をdomain correctnessの
代替にはしない。

### 3.2 2026-07-27時点

- durable model、atomic settings apply、AI policy gate、score、diagnostics、Help、scroll構造、
  exact target、foreground rejectionは自動テスト済み。
- A/BのOperation表示値、許可色、次のtoggle / priority actionはfocused ECS UI testを追加して固定した。
- save schemaは`FamiliarOperation` / `FamiliarPolicy`だけでなく、`TaskArea`の矩形値もround-tripで固定した。
- 現行sourceからprofiling binaryをbuildし、`gather-small-cpu`固定step監査を2反復実行した。
  2 runともvalidで、determinism signatureは`e2eb7c58b2714667`。
- sandbox内の最初の監査は`XOpenDisplayFailed`で0 validとなった。無効runを除外せず、
  `/tmp/hw-b2-objective-audit`へ残したうえで、X11へ接続できる同一binaryを
  `/tmp/hw-b2-objective-audit-x11`で再実行した。
- player-visibleなF5/F9、実pointer hit-test、文字の可読性、実gameplayの役割分担／自己維持は未確認。
- 現CLIにはdefault / disabled policy、dialog hidden / openを切り替えるcase dimensionがない。
  したがってB2 controlled comparisonは`HARNESS PENDING`であり、通常`gather`監査の成功で代用しない。

### 3.3 2026-07-31引継ぎ結果

- `scripts/perf.py audit`へdefault / disabled policyとdialog hidden / openのcase dimensionを追加した。
- determinism schema v3はpolicyを除いたstructural checksum、policyを含むstate checksum、
  candidate gate以後のsnapshot / score / worker / source selector / connectivity counterを記録する。
- controlled fixtureはmanual HaulとChopを1件ずつ用意し、同じrosterでsource selectorと
  connectivity cacheをそれぞれ正規経路から通す。
- 4 caseを各2反復したX11 artifactは8 runすべてvalidで、controlled comparisonは`PASS`だった。

### 3.4 2026-08-02実機確認結果

- player-visible確認は`V01` / `V02` / `V04`〜`V08`が実機`Pass`。`V03`はplayer inputから成立しない重ね表示を
  問う項目で、foreground遮断は`A07`で自動固定済みのためmanual checklistから削除した。
- `V04`で採掘等を拾わず、複数のRest Area Blueprintへ1体しか動かない症状を確認した。
  F9後にdurableな`TaskArea`が復元されてもruntime-onlyの`ActiveCommand`が常に`Idle`へ戻り、
  Blueprint auto-gather / auto-build等のarea producerを停止する不具合を修正した。複数producerの
  同時要求を確定する際の`TaskSlots`再検証も追加し、Mine / Rest Area供給 / Buildの2件並列経路を
  focused testで固定した。修正版の実機再確認は合格した。
- 全項目確認後のfollow-upとして、Blueprintが資材不足でも運搬が割り当てられない場合があるとの報告を受けた。
  accepted haulがpickup前に解除された際のghost `IncomingDeliveries`と、pickup先`Unreachable`時にtaskが
  残留する2経路をそれぞれ失敗する回帰testで再現し、source予約、搬入relationship、worker枠を
  retryable cleanupで同時解放するよう修正した。元checklist全体は再実施せず、狭い`V09`実機再確認で
  pickup前解除後に同じ不足分が再割り当てされることを確認した。
- dashboard表示比較はA3性能計画のfixed / Capture / Memory artifactで完了した。

## 4. 実装方針（高レベル）

- M1-Aは自動証拠の台帳、M1-Bは実機でしか判断できない最小checklistとする。
- M1-Bは同一save slotと明示したFamiliar A/Bだけを使い、各項目の観測結果を表へ追記する。
- 失敗時は値調整をせず、exact target、foreground、request、Logic commit、AI evidenceの順に経路を切り分ける。
- M2は既存`scripts/perf.py`の有効性契約を使い、必要なcounter/harnessが不足する場合だけ
  profiling feature限定で追加する。
- fixed-step correctness、AI work量、Capture frame-timeを別判定にする。
- dashboard mode比較のcase dimension / counter / reportはA3性能計画へ実装し、本計画はB2 policy条件と
  受入結果だけを所有する。

## 5. マイルストーン

## M1-A: 自動correctness台帳

完了条件は、各主張が単独の自動証拠を持ち、実機checklistへ重複して残っていないこと。

| ID | 自動で確定する主張 | 根拠 | 状態 |
| --- | --- | --- | --- |
| `A01` | 同一targetの設定batchはFIFO適用され、最終maxだけで一度releaseする | `same_target_batch_replays_fifo_and_releases_only_for_final_max` | `AUTO PASS` |
| `A02` | all-disabled遷移とpolicy値はatomic commitされる | `settings_batch_updates_policy_and_reports_all_disabled_transition` | `AUTO PASS` |
| `A03` | policy変更は実行中taskと管理Relationshipを直接解除しない | `policy_change_preserves_the_current_task_and_management_relationships` | `AUTO PASS` |
| `A04` | A/Bそれぞれのname、threshold、max、allowed、priority、warning、次actionをexact targetから表示する | `operation_dialog_binds_each_exact_target_value_and_next_action` | `AUTO PASS` |
| `A05` | dialog openは選択変更後もopenerが認証したexact targetを使う | `operation_capture_authenticates_the_exact_opener_and_target`、`operation_capture_latches_exact_target_independent_of_live_selection` | `AUTO PASS` |
| `A06` | world replace / stale targetでdialog target、root、scrollを同期resetする | `world_replace_reset_clears_operation_target_root_and_scroll`、`stale_operation_target_closes_without_retargeting_and_resets_scroll` | `AUTO PASS` |
| `A07` | Pause / LoadConfirm / Settingsなどのforeground中は背景設定requestを発行しない | `paused_or_captured_explicit_settings_intents_emit_rejection_without_request`、`foreground_gate_blocks_background_menu_action` | `AUTO PASS` |
| `A08` | operation、policy、`TaskArea`矩形、Familiar rosterはsave schemaでround-tripする | `root_marker_matrix_collects_extracts_and_round_trips_durable_entities` | `AUTO PASS` |
| `A09` | v0/v1のmissing familiar settingsはroster-awareに移行し、保存済み値を上書きしない | `missing_familiar_settings_migrate_from_serialized_v0_and_v1_rosters`、`saved_operation_is_preserved_and_policy_is_canonicalized` | `AUTO PASS` |
| `A10` | Disabledはsnapshot/source/connectivity/scoreより前に候補をrejectし、priorityは共有scoreへ一度だけ合成する | `disabled_malformed_candidate_records_policy_before_snapshot_validation`、`familiar_priority_changes_final_score_and_normal_keeps_exact_bits` | `AUTO PASS` |
| `A11` | policy-only rejectionは`PolicyDisabled`へ集約され、idle workerがない場合は`NoEligibleFamiliar`を優先する | `policy_disabled_is_terminal_only_when_idle_worker_can_evaluate_it`、`zero_worker_normalizes_policy_disabled_to_no_eligible` | `AUTO PASS` |
| `A12` | max減少は決定的な対象を一度だけreleaseし、同Updateで主要task stateをcleanupする | `max_decrease_releases_reverse_roster_once_and_commits_before_outcome`、`settings_release_cleans_relationship_and_task_in_the_same_update` | `AUTO PASS` |
| `A13` | `FamiliarOperation`変更はEntity Listのvalue syncを要求し、labelは現在数／maxを表示する | `familiar_operation_change_requests_entity_list_value_sync`、`familiar_label_reflects_current_roster_and_operation_max` | `AUTO PASS` |
| `A14` | all-disabled / released rosterを通知へ変換し、all-disabled warning adapterは1件だけ発行する | `familiar_settings_warn_for_all_disabled_and_report_released_roster_once`、`familiar_all_disabled_outcome_becomes_one_warning_in_the_same_update` | `AUTO PASS` |
| `A15` | unowned Blueprintはauto-build producerを考慮し、Familiar policyだけを最終blockerにしない | `unowned_build_never_uses_familiar_policy_as_its_only_final_reason` | `AUTO PASS` |
| `A16` | dialogはbounded scroll、標準Scrollbar、`WorkType::ALL`全行を持つ | `operation_dialog_uses_bounded_scroll_and_every_work_type_row` | `AUTO PASS` |
| `A17` | Help entryの全文、shortcut、surface coverageがexact snapshotと一致する | `exact_snapshot_approves_all_player_visible_help_copy_and_coverage` | `AUTO PASS` |
| `A18` | effective policyが同じcanonical / noncanonical表現は同じaudit bytesになる | `familiar_audit_encodes_effective_policy_not_raw_override_shape` | `AUTO PASS` |
| `A19` | F9後、保存済み`TaskArea`を持つFamiliarは`Patrol`、持たないFamiliarは`Idle`で再開し、Rest Area 2件分のarea producerが継続する | `familiar_shell_rehydrate_restores_patrol_from_durable_task_area`、`familiar_rehydrate_keeps_two_rest_area_supply_sources_active` | `AUTO PASS` |
| `A20` | Rest Area 2件分のWood需要と到達可能Tree 2本から、同じFamiliar所有のChopを2件発行する | `two_rest_area_demands_designate_two_trees_for_one_familiar` | `AUTO PASS` |
| `A21` | blocked tileの所有Mine 2件を、1 cycleで別々のidle Soul 2体へ要求する | `one_delegation_cycle_submits_two_owned_mines_to_two_idle_souls` | `AUTO PASS` |
| `A22` | 完成済みRest Area 2件を別々のSoulへ要求し、1件目への競合を棄却したSoulが2件目を受理しつつ`TaskSlots(1)`を超えない | `one_auto_build_cycle_submits_two_blueprints_to_two_idle_souls`、`assignment_apply_keeps_one_slot_task_at_one_worker_across_competing_requests`、`rejected_slot_competitor_can_take_the_next_open_task` | `AUTO PASS` |
| `A23` | accepted Blueprint haulをpickup前に解除してもtask payload itemの`DeliveringTo`とtargetの`IncomingDeliveries`を残さない | `prepick_blueprint_haul_unassign_releases_incoming_delivery` | `AUTO PASS` |
| `A24` | Blueprint haulのpickup先が到達不能ならtask、worker枠、source予約、搬入relationshipをretryable cleanupで解放する | `unreachable_blueprint_haul_source_retryably_releases_assignment_and_delivery` | `AUTO PASS` |

今回追加したfocused test:

```bash
cargo test -p bevy_app@0.1.0 \
  operation_dialog_binds_each_exact_target_value_and_next_action -- --nocapture
cargo test -p bevy_app@0.1.0 \
  root_marker_matrix_collects_extracts_and_round_trips_durable_entities -- --nocapture
cargo test -p bevy_app@0.1.0 \
  familiar_operation_change_requests_entity_list_value_sync -- --nocapture
cargo test -p bevy_app@0.1.0 \
  familiar_label_reflects_current_roster_and_operation_max -- --nocapture
```

4件とも`1 passed; 0 failed`。

## M1-B: 実機でのみ残すplayer-visible checklist

### 共通fixture

- window: まずdefaultサイズ、`V08`だけ幅を小さくする。UiScaleを変更した場合は値を記録する。
- save slot: 同じslotを全項目で使い、`V02`開始前に一度だけF5保存する。
- Familiar A: threshold 70%、max 3、Haul High、Build Disabled。
- Familiar B: Build High、Haul Disabled。A/Bには識別できる名前を付ける。
- Aには3 Soul、Bには1 Soulを所属させる。Aの1 Soulは実行中taskを持たせる。
- 各項目は`Pass / Fail / Blocked`、日時、window/backend、短い観測事実を結果表へ記録する。

### 手順

| ID | 手順 | player-visible期待結果 | 状態 |
| --- | --- | --- | --- |
| `V01` | A/Bのcontext menuからOperationを交互に開き、Aの4設定を編集する | 対象名・値・Enabled/Disabled・Low/Normal/High・all-disabled warningが読み取れ、A/Bの値が混ざらず、commit後すぐ表示が追従する | `MANUAL PASS` |
| `V02` | Aの`TaskArea`とA/B設定をF5保存し、すべて別値へ変更してからF9 confirmする | 保存時のoperation / policy / `TaskArea`へ戻る。F9直後はdialogが閉じて旧target/scrollが残らず、A/Bを開き直すと各自の値になる | `MANUAL PASS` |
| `V04` | F5時点で`TaskArea`、全作業許可、idle Soul 2体以上を持つFamiliarを用意する。F9後にIdle/Patrolを操作せず、(a) 隣接可能なRockへMineを2件指定する。(b) 到達可能なTreeが2本以上ある状態でRest Area Blueprintを2件置き、資材採取から完成後Buildまで観察する | (a) Mine 2件が別々のSoulへ割り当てられる。(b) 必要なChop/Mineが複数発行され、資材完了後は2件のBlueprintへ別々のSoulがBuildに入る。各1枠taskへ2体が重複確定しない | `MANUAL PASS` |
| `V05` | AをDisable allにし、idle Soul、未割当task、疲労または巡回／監督状態を観察する | warningは一度だけ見え、新規割当は止まり、Tasksは条件成立時に`Blocked: Disabled by familiar policy`を表示する。実行中taskと自己維持は継続する | `MANUAL PASS` |
| `V06` | Aのmaxを3から1へ一度で下げる | 2 Soulだけが所属解除され、task表示もcleanupされ、Entity Listの所属／`current/max`と通知・吹き出しが一度だけ更新される | `MANUAL PASS` |
| `V07` | 全FamiliarのBuildを禁止し、auto-build可能なunowned Blueprintを置く | Familiar policyだけを理由にBlockedにならず、auto-build producerが成功可能なら建築が進む | `MANUAL PASS` |
| `V08` | 小さいwindowでOperationを開き、wheelとscrollbar thumbを使って最終WorkTypeへ移動する。F1 Helpの該当entryも開く | clipping / overlapがなく全行を操作でき、scroll操作感に問題がなく、Help本文とshortcutが読める | `MANUAL PASS` |
| `V09` | 資材不足のBlueprintで運搬開始後、pickup前に担当Soulをmax減少などで解放し、別の作業可能Soulがいる状態で待つ | 元の担当と搬入予定が解放され、同じ不足分の運搬が再割り当てされる。Tasksのworker枠や搬入予定が残留しない | `MANUAL PASS` |

### 結果記録

| ID | 結果 | 日時・環境 | 観測事実 / artifact |
| --- | --- | --- | --- |
| `V01` | `Pass` | `2026-08-02`、window / backend未記録 | A/BのOperation表示と4設定編集を実機確認し、問題なしとの報告 |
| `V02` | `Pass` | `2026-08-02`、window / backend未記録 | F5/F9 round-tripを実機確認し、問題なしとの報告 |
| `V04` | `Pass` | `2026-08-02`、window / backend未記録 | F9後のMine 2件、Rest Area 2件の採取・搬入・Buildを修正版で確認し、基本動作は合格との報告 |
| `V05`〜`V08` | `Pass` | `2026-08-02`、window / backend未記録 | 残るplayer-visible項目をすべて確認し、基本的に合格との報告 |
| `V09` | `Pass` | `2026-08-02`、window / backend未記録 | pickup前解除後、同じ不足分の運搬が別の作業可能Soulへ再割り当てされ、問題なしとの報告 |

## M2: determinism・work counter検証

### 2026-07-27客観スモーク

実行条件:

```bash
python3 scripts/perf.py audit \
  --workload gather --sizes small --renders cpu \
  --repeat 2 --preflight-runs 1 --seed 20260727 \
  --backend auto --window-backend x11 --present-mode novsync \
  --fixed-hz 64 --warmup-ticks 129 --audit-ticks 16 \
  --skip-build --binary target/profiling/bevy_app \
  --output /tmp/hw-b2-objective-audit-x11
```

結果:

- `RUNTIME PASS`: valid runs `2`、invalid runs `0`、determinism signature
  `e2eb7c58b2714667`、post-capture teardown warning `0;0`。
- adapter: `NVIDIA GeForce MX250 (NVK GP107)`、Vulkan / NVK。
- `/tmp/hw-b2-objective-audit-x11/report.md`はraw artifactでありcommitしない。
- 先行するsandbox run `/tmp/hw-b2-objective-audit`は`XOpenDisplayFailed`でinvalid。
  runnerのfail-closed動作を確認した記録として残し、有効runへ数えない。
- このスモークが証明するのは通常`gather-small-cpu`のschema v2反復決定性だけである。
  B2 policy比較やdialog mode同一性の証拠にはしない。

### 2026-07-31 controlled policy / dialog監査

実行条件:

```bash
PYTHONDONTWRITEBYTECODE=1 python3 scripts/perf.py audit \
  --workload gather --sizes small --renders cpu \
  --familiar-policies default,disabled \
  --operation-dialog-modes hidden,open \
  --repeat 2 --preflight-runs 1 --seed 20260731 \
  --backend auto --window-backend x11 --present-mode novsync \
  --fixed-hz 64 --warmup-ticks 129 --audit-ticks 16 \
  --skip-build --binary target/profiling/bevy_app \
  --output /tmp/hw-b2-controlled-policy-audit-20260731-x11-run9
```

結果:

- `RUNTIME PASS`: valid runs `8`、invalid runs `0`、post-capture teardown warning `0`。
- dialog hidden / openのdeterminism signatureはpolicyごとに完全一致した。
  defaultは`349ffb0359204270`、disabledは`3fdfa98db14450f1`。
- `post-warmup`のdefaultはmembership `24`、snapshot `24`、score `24`、worker score `23`、
  source selector `1`、source scan `2`、connectivity `21`。
- disabledはmembership `24`、policy rejection `24`で、snapshot / score / worker / source /
  connectivityはすべて`0`。
- fixture初期のstructural checksumはdefault / disabledで一致し、policyを含むstate checksumは異なる。
- `/tmp/hw-b2-controlled-policy-audit-20260731-x11-run9/report.md`と
  `familiar_policy_comparison.json`はraw artifactでありcommitしない。
- controlled rosterによるEntity List再構築で判明した二重despawn commandは、
  Bevy 0.19の再帰despawn契約に合わせてroot childだけをdespawnするよう修正し、
  `clears_nested_children_without_queueing_duplicate_descendant_despawns`で固定した。

### 残作業

- [x] determinism schema v2で通常`gather-small-cpu`同一caseの反復checkpoint checksumが一致する。
- [x] effective policyが同じcanonical / noncanonical表現は同じactor record bytesになる。
- [x] default policyのscore mapping、Top-K、tie-break契約はfocused testで維持される。
- [x] default / disabled policyを選べるcontrolled fixtureを追加する。
- [x] disabled policyがcandidate snapshot / source / connectivity / score workを減らし、
      代替の全件scanを追加しないことをcounterで確認する。
- [x] dialog hidden / openでsimulation checksumとAI work counterが一致する。
- [x] dashboard hidden / visible / active-filterはA3性能計画のartifactを参照する。
- [x] invalid run、schema不一致、warning / errorを黙って除外しない。

B2所有のcontrolled policy / dialog検証に加え、A3所有dashboard artifactの参照も完了した。
`/tmp/hw-task-dashboard-audit-20260802-headless-v5`は3 modeのAI work同一性、
`/tmp/hw-task-dashboard-capture-20260802-x11-native-v2`と
`/tmp/hw-task-dashboard-memory-20260802-x11-native-v2`はIntel Vulkan / X11の各9 valid runと
mode cost comparison PASSを記録する。raw artifactはcommitしない。

## M3: 記録・完了

- 変更内容:
  - M1-B / M2残作業の環境、case、結果、artifact pathを本計画へ記録する。
  - 回帰があれば症状ごとの修正計画へ切り出し、機能仕様を無断で変えない。
  - 全項目成功後に本計画をarchiveし、生成indexを更新する。
- 完了条件:
  - [x] 自動correctnessの根拠と今回の客観スモーク結果が記録済み。
  - [x] M1-Bに残る各IDにpass / failと根拠がある。
  - [x] M2のcontrolled policy / dialog UI mode artifactがvalidである。
  - [x] 恒久仕様の変更が発生した場合だけ関連docs / Helpを同期している。
  - [x] `python3 scripts/dev.py verify`が成功する。
  - [x] 本計画を`docs/plans/archive/`へ移し、indexがfreshである。
- 検証:
  - `python3 scripts/dev.py docs --write`
  - `python3 scripts/dev.py verify`
  - `git diff --check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 自動testと目視結果を同じ`Pass`で扱う | 証明範囲を誤る | `AUTO` / `RUNTIME` / `MANUAL` / `HARNESS`を明示する |
| 手動fixtureが曖昧 | 別条件の結果を比較する | A/B、task、save時点をM1-Bの各手順へ固定する |
| UI確認でAIを再評価する | 表示コストとsimulation workを混同する | producer counterとchecksumを別artifactで比較する |
| counter不足を0扱いする | work reductionを誤判定する | 不足は未計測とし、profiling feature限定で追加する |
| A3計画とharnessが重複する | schema/case identityが分岐する | dashboard modeはA3性能計画を正本として参照する |
| invalid runを除外する | 良いrunだけを選ぶ | session全体をinvalidとし原因を記録する |

## 7. 検証計画

- 必須:
  - M1-Aの自動証拠。
  - M1-Bに残るplayer-visible項目。
  - determinism schema v3 / same-schema / same-fixture / same-build。
  - policy gateの後段work削減とUI mode間AI work同一性。
- 計画完了時:
  - `python3 scripts/dev.py docs --write`
  - `python3 scripts/dev.py verify`
  - `git diff --check`
- 手動確認シナリオ: M1-Bを正本とする。
- パフォーマンス確認: M2を正本とする。正式比較artifactは`target/perf-runs/`、
  今回の短縮スモークはcommit対象外の`/tmp`へ置く。

## 8. ロールバック方針

- 本計画は検証だけならproduction stateを変更しない。
- harness / counterを追加した場合はprofiling featureの変更単位で戻し、Rust writerとPython validatorの
  schemaを必ず同時に戻す。
- 回帰修正を戻す場合は該当修正計画のrollbackを使い、B2 durable dataを推測で削除しない。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`。自動correctness、B2 controlled policy / dialog監査、実機`V01` / `V02` / `V04`〜`V09`、A3 dashboard artifact参照、`M3`を完了。
- 完了済み: `M1-A`〜`M3`。
- 未着手/進行中: なし。

### 次のAIが最初にやること

1. B2仕様を変更する場合はarchive済み実装計画と本受入記録を参照する。
2. 新しい性能比較はA3性能artifactと同じschema / fixture / matrixで採る。

### ブロッカー/注意点

- B2実装は自動correctness gateまで完了済み。本計画を理由に仕様を再設計しない。
- `/tmp/hw-b2-objective-audit-x11`は通常`gather`の短縮スモークであり、
  controlled B2 policy artifactとして引用しない。
- controlled B2 policy artifactは`/tmp/hw-b2-controlled-policy-audit-20260731-x11-run9`。
  8 valid / 0 invalidとcomparison `PASS`を正本とし、先行するrun2〜run6のinvalid診断を成功値へ混ぜない。
- 手動観測で変化がない仮説は一度で打ち切り、経路・前提条件を先に確認する。
- fixed-step auditと実時間frame-timeを混ぜない。
- artifactがない性能主張を「確認済み」と書かない。

### 参照必須ファイル

- `docs/plans/archive/familiar-operation-policy-plan-2026-07-20.md`
- `docs/plans/archive/task-dashboard-performance-validation-plan-2026-07-20.md`
- `docs/familiar_ai.md`
- `docs/task_list_ui.md`
- `docs/save_load.md`
- `docs/performance-profiling.md`
- `crates/hw_familiar_ai/src/familiar_ai/settings.rs`
- `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/`
- `crates/bevy_app/src/interface/ui/interaction/systems.rs`
- `crates/bevy_app/src/plugins/startup/perf_scenario/`

### 最終確認ログ

- focused UI / save / Entity List tests: 4件すべて`1 passed; 0 failed`
- `V04` focused / connected regression: F9 rehydrate、Rest Area供給2件、Mine 2件、Build 2件、
  slot競合後の別task受理がすべて`PASS`
- follow-up運搬回帰: pickup前の汎用unassignとpickup先`Unreachable`の2件が、修正前`FAIL`・修正後`PASS`
- fixed-step audit: `2 valid; 0 invalid`（有効session）
- controlled fixed-step audit: `8 valid; 0 invalid`、comparison `PASS`
- 最終 `python3 scripts/dev.py verify`: PASS（2026-08-02、全quality gate成功）
- A3 dashboard artifact: fixed 3 valid / 0 invalid、Capture 9 valid / 0 invalid、Memory 9 valid / 0 invalid、各comparison `PASS`
- 未解決エラー: なし

### Definition of Done

- [x] M1-Aの自動correctness台帳が全項目合格
- [x] M1-Bのplayer-visible checklistが全項目合格
- [x] M2のcontrolled policy / dialog UI mode artifactと判定根拠が記録済み
- [x] A3性能計画とのownership重複がない
- [x] 影響ドキュメントが更新済み
- [x] `python3 scripts/dev.py verify`が成功
- [x] 完了後に本計画をarchive

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-07-26` | `Codex` | B2実装完了時に、未実施のplayer-visible受入と性能artifact採取を独立移管 |
| `2026-07-27` | `Codex` | 自動／実window客観／実機手動／harness待ちを分離し、focused回帰2件と通常fixed-step監査結果を記録 |
| `2026-07-31` | `Codex` | schema v3 controlled policy / dialog harness、後段counter、8-run X11 artifactを追加し、Help影響レビューと完全検証を通してM2のB2所有範囲を完了 |
| `2026-08-02` | `Codex` | 実機`V01` / `V02` Pass、`V03`到達不能、`V04`採掘割当失敗を記録し、V03を到達可能なoverlay遮断確認、V04を基礎割当・役割分担・実行中継続へ分割 |
| `2026-08-02` | `Codex` | 意味のない`V03`手動項目を削除。F9後のTaskArea→Patrol復元と割当確定時のTaskSlots競合を修正し、Mine / Rest Area供給 / Buildの2件並列回帰を追加。`V04`を修正版の実機再確認待ちへ更新 |
| `2026-08-02` | `Codex` | 元の実機`V04`〜`V08` Passを記録。follow-upの運搬停止をpickup前解除時のghost搬入予定と到達不能pickup task残留に分離し、2件の自動回帰と狭い`V09`へ整理 |
| `2026-08-02` | `Codex` | follow-up修正版の`V09`実機再確認Passを記録し、M1-Bのplayer-visible checklistを完了。残件をA3所有のdashboard性能artifact参照に限定 |
| `2026-08-02` | `Codex` | A3のfixed / Intel Vulkan X11 Capture / native Memory artifact参照を完了し、B2実機・性能フォローアップをarchive |
