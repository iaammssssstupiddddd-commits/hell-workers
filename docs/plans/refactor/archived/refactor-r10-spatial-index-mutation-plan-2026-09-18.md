# R10: SpatialIndexのmutation APIとgeneration契約の保護 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `refactor-r10-spatial-index-mutation-plan-2026-09-18` |
| ステータス | Complete |
| 作成日 | 2026-09-18 |
| 最終更新日 | 2026-09-19 |
| 作成者 | Codex |
| 関連提案 | [実装横断レビュー](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md) §5 R10 |
| 関連Issue/PR | N/A |
| 優先度 / 相対規模 | P2・後順位 / 小〜中 |
| 調査基準 | `52913b61ca524f1cd429d3d73a0468faf3d0999b` |
| 計画レビュー | 2026-09-18 / [全体レビュー結果](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md#42-計画レビュー2026-09-18)参照。実装受入ではない |
| 計画具体化 | 2026-09-18 / API案・変更単位・fixture/期待値・着手手順を追加。新規名は未実装の設計案 |
| 実装状態 | 実装・必要な回帰・仕様／Help同期・品質gateを完了。専用native受入は不要。設計経緯としてarchiveし、現行仕様は関連docsを正本とする。 |

## 1. 目的

- 解決したい課題: public raw storageとdata_mutでgenerationを迂回でき、再insertで旧bucketが残り得るAPI面を閉じる。
- 到達したい状態・成功指標: membership/positionの意味ある変更だけがgenerationを更新し、同Entity再insertでも重複hitせず、全callerが管理されたAPIを使う。

## 2. スコープ

### 対象（In Scope）

- GridData/SpatialIndexのread/clear/rebuild/insert API
- raw storageの現行reader/test移行、cache失効回帰

### 非対象（Out of Scope）

- cell size調整、index algorithmやtyped tagの置換
- 性能最適化、WorldMap/ECS統合

### 主な変更対象（現行ファイル）

- [crates/hw_spatial/src/grid.rs](../../../../crates/hw_spatial/src/grid.rs)
- [crates/bevy_app/src/plugins/spatial.rs](../../../../crates/bevy_app/src/plugins/spatial.rs)
- [crates/bevy_app/src/systems/command/stockpile_policy.rs](../../../../crates/bevy_app/src/systems/command/stockpile_policy.rs)
- [crates/bevy_app/src/systems/familiar_ai/diagnostics.rs](../../../../crates/bevy_app/src/systems/familiar_ai/diagnostics.rs)
- [crates/hw_familiar_ai/src/familiar_ai/decide/task_management/task_finder/mod.rs](../../../../crates/hw_familiar_ai/src/familiar_ai/decide/task_management/task_finder/mod.rs)

新規moduleやtestは上記ownerの配下に置く。新しいcrateを既定案にせず、定義→生成→使用の順に変更する。

## 3. 現状とギャップ

GridDataのfieldはpublicでdata_mutはgenerationを更新しない。insertは既存Entityの旧bucketを除去しない。generationはFamiliar diagnosticsのcache失効に使われる。data_mut利用はテスト中心だがrootにpositions直接参照もある。

提案時点の静的な根拠と実行再現を区別し、M1で現HEADとの差・実際のcaller・既存testを再確認する。
仕様を直す前に実行経路・前提条件を確認し、再現できない仮説への値調整を繰り返さない。

### 既存テスト・検査の入口

- [crates/hw_spatial/src/grid.rs](../../../../crates/hw_spatial/src/grid.rs)
- [crates/bevy_app/src/plugins/spatial.rs](../../../../crates/bevy_app/src/plugins/spatial.rs)
- [crates/bevy_app/src/systems/command/stockpile_policy.rs](../../../../crates/bevy_app/src/systems/command/stockpile_policy.rs)

これらは再利用する入口であり、本計画の全条件を既に保証しているという意味ではない。
不足する受入ケースを下記milestoneで追加する。削除・文書整理だけの変更には実装をなぞる新規testを作らない。

## 4. 実装方針と具体設計

- 既存raw readerとwriterを先に棚卸しし、必要なread-only iterator/position lookup、clear/rebuild、cell size constructorを定義する。
- insertは既存memberを安全に更新するupsertへ統一する方針を基本とし、現callerの初回限定前提を確認する。同値操作はgeneration不変、membership/position変更は更新する。
- raw fieldをprivateにしdata_mutを撤去する。cell size変更は空index constructorまたは明示rebuildに限定し、既定値・差分updater・typed tagを維持する。

### 具体設計と変更境界

| API（案） | mutation / generationの契約 |
| --- | --- |
| GridData::upsert(entity, pos) -> bool | 新規登録か既存位置変更を一つの入口にする。移動時は旧bucketから除去後に新bucketへ追加。同位置はfalse。 |
| SpatialIndex::insert / update | 上記upsertへ委譲し、trueのときだけgenerationをwrapping_add(1)。現在のupdate→insert新規枝を先にprivate helperへ分け、相互再帰を作らない。 |
| SpatialIndex::remove / clear | 不在remove・空clearはno-op。実変更だけgenerationを進める。 |
| SpatialIndex::replace_positions | 一意なEntity→Vec2 mapを入力とし、同内容なら無変更。異なる内容なら一括再構築後にgenerationを一度進める。通常rebuildで0へ戻さない。 |
| position / positions / cell_size | 読取専用lookup/iterator/getter。cell sizeはconstructorで決定し、可変fieldを公開しない。 |

world replacementによるresource再初期化は別epochの既存契約として維持する。座標妥当性の新policyやindex algorithm変更は含めない。

### 実装単位と移行順

1. **R10-A / M1〜M2:** grid.rsのupsertとmembership/generation testを先に完成させる。
2. **R10-B / M2:** clear/replace_positions/読取APIを追加し、再構築の失効回数を固定。
3. **R10-C / M3:** root stockpile_policyのpositions読取、Familiar task_finderとroot deconstruction testのclearを移す。その後GridData fieldをprivate化しdata_mutを削除する。

### 依存関係と着手順

単独着手可能。Familiar task_finder/deconstruction testのdata_mut移行はR07のtest変更と直列化する。R05/R06に新しいmutable raw APIを使わせない。

関連計画へのリンクと全体の着手順は[親提案の計画一覧](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md)を参照。
依存は実装上の前提と共有ファイルの編集競合を区別する。全14件の完了を本計画の終了条件にしない。

### Bevy 0.19と変更の所有者

- 既存の正しい0.19実装を基準とし、新しいAPIはrust-analyzer/docsrs MCPまたはlocal registryで確認してから使う。
- Queryはcrate-owned contextへまとめ、rootの配線・leafの実装所有を維持する。lint抑制で構造問題を隠さない。
- コードと文書の編集は主担当が行う。subagentは読み取り専用の調査/レビューに限定する。
- 共通Help snapshot・docs索引・凍結native subjectは並行編集しない。

## 5. マイルストーン

## M1: 公開利用面と契約表を固定

- 変更内容:
  - data_mut/raw fieldの全参照を調べ、read APIだけで置換できるconsumerを分類する。
  - insert/update/remove/clear/rebuildのgenerationとhit数の期待値を表にする。
- 変更ファイル: R10-A: hw_spatial/src/grid.rs と同moduleの既存test。全data/data_mut/公開field利用箇所の台帳。
- 完了条件:
  - [x] 現行readerを損なわずraw mutationを閉じるAPI案がある。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M2: 管理APIとgeneration回帰

- 変更内容:
  - read API、clear/rebuild、upsertを追加し、同値/変更/再insertを検証する。
  - diagnostic cache失効とcustom cell sizeの現行testを拡張する。
- 変更ファイル: R10-A/B: grid.rs のupsert・clear・replace_positions・read-only APIとgeneration test。
- 完了条件:
  - [x] 旧bucket残留なし、semantic generationが期待どおり。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M3: 全caller移行と非公開化

- 変更内容:
  - root positions reader、Familiar/解体test等を移し、raw field/data_mutを非公開化または削除する。
  - pause/load時のindex再構築とselectionの回帰を確認する。
- 変更ファイル: R10-C: root systems/command/stockpile_policy.rs、Familiar task_finder/mod.rs、root deconstruction test、他の実在reader/writer。
- 完了条件:
  - [x] 外部callerがgenerationを迂回してmutationできない。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M4: 仕様・Help・品質ゲートと終了整理

- 変更内容:
  - 下記仕様へ最終的な所有者・順序・失敗条件を反映する。計画の検討経緯をそのまま恒久仕様へ転記しない。
  - [Help影響Skill](../../../../.cursor/skills/hell-workers-review-help-impact/SKILL.md)で全実装差分のplayer-visible経路を確認する。
  - 検索結果・選択・実行可能性が不変かconsumerまで確認し、API保護だけなら具体的なNo impactを記録する。
  - 受入と品質ゲートの結果を記録し、専用出力を整理する。
- 仕様更新対象（該当する契約を変更した文書だけを更新）:
  - [docs/architecture.md](../../../../docs/architecture.md)
  - [docs/cargo_workspace.md](../../../../docs/cargo_workspace.md)
  - [docs/invariants.md](../../../../docs/invariants.md)
- 完了条件:
  - [x] §7のケースと必要なnative受入が完了し、未検証範囲を明記した。
  - [x] HelpがUpdate required / No impactのいずれかに確定し、対応する更新または根拠を記録した。
  - [x] `check`・Clippy・`verify`が成功し、rust-analyzerの取得可能な診断にerrorがない。
  - [x] primary validation storage checkが成功し、不要出力の整理/継続用途を記録した。
- 検証: §7の完了gate。品質gateのpassだけをHelpの意味レビューやnative受入の代替にしない。

## 6. リスクと対策

| リスク | 対策 |
| --- | --- |
| read consumerをraw writerと同時に破壊する | read-only代替APIを先に追加する。 |
| no-opでもgenerationが増えcache無効化過多 | 意味変化を表にして検査する。 |
| 共有ファイルの並行変更を上書きする | 作業開始/各取り込み前にstatusと対象diffを確認し、変更所有を確認する。 |
| 過去のpassを新subjectへ流用する | 変更後の入力を固定して必要な検査を実施し、未実施は未実施と記録する。 |

## 7. 検証計画

### ケースと判定条件

| ケース | 期待する結果 |
| --- | --- |
| 同Entityを別cellへ再insert | 旧bucketが消え、radius結果に重複なし。 |
| 同値update/存在しないremove/空clear | semantic no-opでgeneration不変。 |
| 実move/remove/clear/rebuild | consumerのcacheが適切に失効。 |
| 同じmembership/positionでrebuild | generation不変。 |
| 異なる内容へのrebuild | 既存generationから進め、constructor/defaultへの置換で0へ戻さない。world replacement時の明示resetとは区別する。 |
| custom cell size・pause/load | 既定挙動とtyped updater/再構築を維持。 |

### 回帰fixtureの作り方（新規test案）

| ID / test案 | 準備・操作 | 必須assert |
| --- | --- | --- |
| R10-T1 `reinsert_moves_unique_bucket_membership_test` | cell_size64、Eを(8,8)へinsert、同位置へ再insert、(136,8)へ再insert。 | 初回と移動だけgeneration+1。同位置不変。旧bucketにEなし、広域検索でEが1回だけ返る。 |
| R10-T2 `spatial_noops_keep_generation_test` | 同値update、不在remove、空clearを各実行。 | generation不変。実remove/非空clearはそれぞれ一度だけ増える。 |
| R10-T3 `replace_positions_preserves_generation_history_test` | 非0 generationのindexを同内容/異内容mapで再構築。 | 同内容不変、異内容は旧値+1。0へ戻らずmembership/positionが入力と一致。 |
| R10-T4 consumer invalidation fixture | diagnostics cacheと既存typed updaterのpause/load経路を実行。 | 実変更時だけ失効し、別epochでは既存reset契約どおり再構築。 |

T1は範囲検索の結果だけでなくtest moduleからbucket membershipも確認する。公開APIへ診断用可変escapeを残さない。

### focused command（実装時に実行）

```bash
python3 scripts/dev.py cargo -- test -p hw_spatial
python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 spatial_plugin
python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 stockpile_policy_area_targets
python3 scripts/dev.py cargo -- test -p hw_familiar_ai diagnostic_membership
```

test filterは実装時に実際の出力件数を確認し、0件実行を成功根拠にしない。
既存filterに入らない新testは正確なmodule/test名をここへ追加する。

### 完了gate

```bash
python3 scripts/dev.py docs --write
python3 scripts/dev.py docs --check
python3 scripts/check_help_impact.py
python3 scripts/dev.py check
python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings
python3 scripts/dev.py verify
python3 scripts/dev.py validation check
git diff --check
```

`verify`がworkspace testを含むため、成功後に理由なく同じ全testを繰り返さない。
上のコマンドは実装完了時の要件であり、計画作成時の実行済み記録ではない。
Help No impactのローカルoverrideはSkillに従い具体的理由を設定し、将来の差分を事前承認しない。
Help更新時はmanifest/provider/coverageと生成したexact snapshotを同じbatchに含め、全差分を読む。

### 実画面・性能確認

indexのmembership/generationと実consumerのECS testで判定する。性能を主張しないため新しいnative/allocator比較は不要。

実機確認を実施する場合は[Native Acceptance Skill](../../../../.cursor/skills/hell-workers-run-native-acceptance/SKILL.md)を読み直す。
primary `dev.py validation plan --spec ... -- <既存helperのplan>`へ登録し、返されたno-prompt kitty launcherだけを使う。
fixtureが対象を覆わなければ先に専用caseと独立verifierを用意する。汎用smokeの成功を個別要件の証明にしない。
Capture/Memoryを含むrecipeは逐次実行し、read-only observer・source/asset/binary一致・timeoutを検証する。
通常の修正feedbackでは同じdev cacheを再利用し、headless・feedback・正式受入・性能結果を区別する。

### 検証データ管理（各バッチの開始前・報告前に更新）

正本は[検証データ管理](../../../development-infra/validation-storage-workflow.md)。実行開始/再開時に必ず読み直す。
通常quality gateは既存primary cacheで行い、専用native/performance出力を作る場合にretain/plan/executeを登録する。

| 項目 | 完了時点 |
| --- | --- |
| batch / owner | 専用native batch不要。主担当Codexがprimaryで実装・通常品質gateを実行 |
| workspace / subject | primaryの作業差分。基準HEADはメタ情報の調査基準を参照 |
| artifact / 開始・終了bytes | 本計画専用のartifact・binary copy・worktreeなし |
| 採用成果と最終結果 | codeと関連仕様はprimary。本書§9に最終検証結果を記録 |
| 削除path / 容量差 | 本計画固有の削除なし、専用出力0 bytes |
| 継続保持 | primary通常Cargo cacheは継続開発用。R13/R14用candidateは各計画・primary保持台帳に用途を記録 |
| 整理状態 | 本計画の受入を完了。未終了の別計画を理由に専用jobを保持しない |

成功は元依存が残る間に独立verifyしてsealする。失敗/中断も理由・未検証範囲をsealし、不要jobを整理してfinalize/checkを通す。
job終了とcandidate/cache解放は分ける。review-activeの無応答・中間passを終了と解釈しない。
保持にはowner・consumer・bytes・次作業・release_whenが必要で、親track未完だけを保持理由にしない。
全jobのcapsule保存、一律日数/容量上限を新設しない。primaryの通常Cargo cacheは別の保守寿命を持つ。

## 8. ロールバック方針

read API導入、caller移行、非公開化を分ける。consumer側を戻す際も旧bucket不整合の回帰修正を維持する。

戻す前に直近5commit、対象のHEAD差分、他sessionの所有を確認する。
広範囲のcheckout/cleanや他計画の変更破棄を行わず、レビュー可能な限定逆patchを作る。
保存型・asset formatを変える必要が生じた場合は、互換/復旧条件をこの計画へ追記してから変更する。

## 9. AI引継ぎメモ（完了記録）

### 現在地

- 2026-09-19完了。SpatialIndexのmutation契約18件に加え、実generation変更だけがdiagnostics availabilityを進めblockerを解除するconsumer回帰が成功。
- `python3 scripts/dev.py verify`が成功。通常／profilingのworkspace test、通常Clippy警告0、feature別compile、Python、Help、文書・storage gateを確認した。
- Helpは全体差分でUpdate required。Move失敗時の保持、タスク表示、建設工程の3 entryとexact snapshotを更新し、実際のplayer-visible経路と差分をレビュー済み。
- 本計画単体の専用native job／binary copy／worktreeは作成していない。通常primary cacheは継続開発用に保持する。R13/R14用candidateと未終了native確認は各計画が所有し、本計画の完了と分離する。

### 次のAIが最初にやること

現行の関連仕様と実装を参照する。本書の設計案・着手手順を未実装の作業として再実行しない。
新しい変更ではその差分に応じた回帰、Help判断、storage確認を行う。

### 最終確認ログ

| 検査 | 結果 |
| --- | --- |
| focused tests / 新規回帰 | SpatialIndexのmutation契約18件に加え、実generation変更だけがdiagnostics availabilityを進めblockerを解除するconsumer回帰が成功。 |
| check / rust-analyzer | workspace compile成功。workspace rust-analyzer診断はMCPのnull応答により取得不可で、compileとClippyで代替確認 |
| Clippy / verify | 通常Clippy警告0、verify成功 |
| Help判断 / 仕様同期 | Update requiredとして3 entry・exact snapshot・関連仕様を同期 |
| native / GPU / 性能 | 本計画では不要。性能改善の定量主張なし |
| validation storage | primary `validation check`成功。専用出力なし、他計画の用途を持つcacheを保持 |
| 未解決エラー | なし。上記MCPの取得制約は残る |

### Definition of Done

- [x] 実装と必要な回帰、仕様・Help同期を完了
- [x] workspace compile・Clippy警告0・verify・storage checkが成功
- [x] 本計画固有の不要job／専用環境なし。関連計画の現用途を持つcacheを保持
- [x] primaryのarchiveへ移動し、計画・提案索引を再生成

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-09-18 | Codex | R10の独立計画をテンプレートから作成。依存、milestone、受入、Help、検証データ管理を具体化 |
| 2026-09-18 | Codex | 計画レビュー: no-op/rebuild/generation継続、world replacementのreset、raw reader移行を確認。追加修正不要。 |
| 2026-09-18 | Codex | 全計画ブラッシュアップ: cell_size64の再insert fixtureを作り、旧bucket membership・検索重複・generationを同時に観測するR10-T1を追加する。 実装は未着手。 |
| 2026-09-19 | Codex | 実装・回帰・Help／仕様同期と品質gateを完了しarchive。 |
