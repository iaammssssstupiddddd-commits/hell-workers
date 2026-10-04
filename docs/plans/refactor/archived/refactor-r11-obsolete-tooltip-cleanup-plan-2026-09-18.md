# R11: 未参照Tooltip実装と古い所有説明の整理 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `refactor-r11-obsolete-tooltip-cleanup-plan-2026-09-18` |
| ステータス | Complete |
| 作成日 | 2026-09-18 |
| 最終更新日 | 2026-09-19 |
| 作成者 | Codex |
| 関連提案 | [実装横断レビュー](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md) §5 R11 |
| 関連Issue/PR | N/A |
| 優先度 / 相対規模 | P2・低工数 / 小 |
| 調査基準 | `52913b61ca524f1cd429d3d73a0468faf3d0999b` |
| 計画レビュー | 2026-09-18 / [全体レビュー結果](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md#42-計画レビュー2026-09-18)参照。実装受入ではない |
| 計画具体化 | 2026-09-18 / API案・変更単位・fixture/期待値・着手手順を追加。新規名は未実装の設計案 |
| 実装状態 | 実装・必要な回帰・仕様／Help同期・品質gateを完了。専用native受入は不要。設計経緯としてarchiveし、現行仕様は関連docsを正本とする。 |

## 1. 目的

- 解決したい課題: 移設後にmodule treeから外れた旧Tooltipソースと、現行所有先に合わない案内を除去する。
- 到達したい状態・成功指標: 現行Tooltip経路だけが参照可能な正本として残り、visual/UIのrules・READMEが現在の依存と登録先を説明する。

## 2. スコープ

### 対象（In Scope）

- root tooltipの旧fade/layout/targetの参照確認と削除
- hw_visual rules、root visual/UI READMEの修正

### 非対象（Out of Scope）

- 現行hw_ui Tooltipの挙動/見た目変更
- 一般的な未参照ファイル検出framework、全repository cleanup

### 主な変更対象（現行ファイル）

- [crates/bevy_app/src/interface/ui/interaction/tooltip/mod.rs](../../../../crates/bevy_app/src/interface/ui/interaction/tooltip/mod.rs)
- `52913b6:crates/bevy_app/src/interface/ui/interaction/tooltip/fade.rs`（削除済み）
- `52913b6:crates/bevy_app/src/interface/ui/interaction/tooltip/layout.rs`（削除済み）
- `52913b6:crates/bevy_app/src/interface/ui/interaction/tooltip/target.rs`（削除済み）
- [crates/hw_visual/_rules.md](../../../../crates/hw_visual/_rules.md)
- [crates/bevy_app/src/systems/visual/README.md](../../../../crates/bevy_app/src/systems/visual/README.md)
- [crates/bevy_app/src/interface/ui/README.md](../../../../crates/bevy_app/src/interface/ui/README.md)

新規moduleやtestは上記ownerの配下に置く。新しいcrateを既定案にせず、定義→生成→使用の順に変更する。

## 3. 現状とギャップ

root tooltip/mod.rsはhw_uiへ委譲する。追跡済みfade.rs/layout.rs/target.rsはmodule宣言・include参照を持たず、旧内容が現行と乖離する。hw_visual rulesの依存/Speech登録先とroot visual READMEの廃止ファイル案内も古い。

提案時点の静的な根拠と実行再現を区別し、M1で現HEADとの差・実際のcaller・既存testを再確認する。
仕様を直す前に実行経路・前提条件を確認し、再現できない仮説への値調整を繰り返さない。

### 既存テスト・検査の入口

- [scripts/tests/test_check_agent_rules.py](../../../../scripts/tests/test_check_agent_rules.py)
- [scripts/tests/test_update_docs_index.py](../../../../scripts/tests/test_update_docs_index.py)
- [crates/hw_ui/src/interaction/tooltip/system.rs](../../../../crates/hw_ui/src/interaction/tooltip/system.rs)

これらは再利用する入口であり、本計画の全条件を既に保証しているという意味ではない。
不足する受入ケースを下記milestoneで追加する。削除・文書整理だけの変更には実装をなぞる新規testを作らない。

## 4. 実装方針と具体設計

- 削除前にmod宣言、#[path]、include!、script/testのファイル読込み、docsリンク、feature条件を再検索する。前回レビューの未参照判断だけで削除しない。
- 現行mod.rs/hw_uiの実行経路は変更せず、未参照3ファイルだけを削除する。並行差分とHEADとの差を読み、他sessionの変更がないことを確認する。
- rulesは正本_rules.mdを変更し、AGENTS.md/CLAUDE.mdのsymlinkを保持する。SpeechVisualIngressSet/Visual等の実登録に合わせる。

### 具体設計と変更境界

新APIは追加しない。削除と案内の訂正を次の範囲に固定する。

| 対象 | 実施内容 |
| --- | --- |
| root interface/ui/interaction/tooltip/{fade.rs,layout.rs,target.rs} | 未参照を再確認して3ファイルだけ削除。現行mod.rsのInspectionSource/Renderer adapterは保持。 |
| root UI README | tooltipの案内を「rootはゲーム側inspection/asset adapter、target/layout/fade/systemはhw_ui」へ訂正。 |
| root visual README | 廃止済みcharacter_proxy_3d.rs / elevation_view.rsの案内を現行module一覧へ合わせる。 |
| TaskAreaMaterialの説明 | 定義はhw_visual/src/task_area_visual.rs、Material2dPlugin登録はhw_visual/src/lib.rsのHwVisualPlugin。rootはTaskContext依存更新とre-exportを所有する。 |
| hw_visual/_rules.md | 現行Cargo.tomlにないhw_jobs/hw_logistics依存記述を除き、SpeechPluginのSpeechVisualIngressSet→Visual登録へ合わせる。 |
| 本計画/親提案の旧ファイル参照 | 削除後にリンクを切らさない。歴史的根拠は `52913b6:<旧path>` のcommit付きコード表記へ変える。 |

`AGENTS.md` / `CLAUDE.md` は `_rules.md` へのsymlinkを保つ。新しい未参照検出frameworkや挙動testは作らない。

### 実装単位と移行順

1. **R11-A / M1:** mod / path / include / script読込み / feature / 文書参照を再検索し、3ファイルの実行経路がないことを記録する。
2. **R11-B / M2:** 3ファイル削除と所有説明・歴史参照の訂正を同じ変更単位にする。
3. **R11-C / M3:** 既存Tooltip回帰、agent-rules、docs索引/リンクを確認する。runtime経路不変ならnative追加は不要。

### 依存関係と着手順

他計画に先行条件なし。R12/R13後にvisual所有説明が変わった場合は、その計画が更新を引き継ぐ。共通docs索引とrulesは主担当が直列更新する。

関連計画へのリンクと全体の着手順は[親提案の計画一覧](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md)を参照。
依存は実装上の前提と共有ファイルの編集競合を区別する。全14件の完了を本計画の終了条件にしない。

### Bevy 0.19と変更の所有者

- 既存の正しい0.19実装を基準とし、新しいAPIはrust-analyzer/docsrs MCPまたはlocal registryで確認してから使う。
- Queryはcrate-owned contextへまとめ、rootの配線・leafの実装所有を維持する。lint抑制で構造問題を隠さない。
- コードと文書の編集は主担当が行う。subagentは読み取り専用の調査/レビューに限定する。
- 共通Help snapshot・docs索引・凍結native subjectは並行編集しない。

## 5. マイルストーン

## M1: 未参照と所有先の再確認

- 変更内容:
  - 3旧ファイルの全参照、feature条件、外部tool読込みを調査し、削除可能な根拠を記録する。
  - rules/READMEの各記述を現在のCargoとPlugin登録へ照合する。
- 変更ファイル: R11-A: 対象3ファイル・tooltip/mod.rs・hw_ui実装・Cargo feature・script/docsの読取調査。まだ削除しない。
- 完了条件:
  - [x] 削除対象がruntime/tooling両方から未参照と確認できる。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M2: 旧ソースと誤案内を整理

- 変更内容:
  - 3ファイルだけを削除し、hw_visual rulesとroot visual/UI READMEの古い説明を修正する。
  - current Tooltipや表示の値調整は混ぜない。
- 変更ファイル: R11-B: root tooltip旧3ファイル、root UI/visual README、hw_visual/_rules.md、本計画と親提案の歴史参照。
- 完了条件:
  - [x] 検索で旧実装を正本と誤認する経路がなく、現行runtimeは不変。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M3: 参照・rules・Helpを確認

- 変更内容:
  - リンク/索引・symlink・rules同期を検査する。
  - 既存Tooltip回帰と通常gateを通し、実経路不変のHelp No impact判断を残す。
- 変更ファイル: R11-C: 既存Tooltip testとagent-rules/docs gate。索引生成以外の新しい検査コードは追加しない。
- 完了条件:
  - [x] 未参照削除の根拠、文書整合、runtime非変更が説明できる。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M4: 仕様・Help・品質ゲートと終了整理

- 変更内容:
  - 下記仕様へ最終的な所有者・順序・失敗条件を反映する。計画の検討経緯をそのまま恒久仕様へ転記しない。
  - [Help影響Skill](../../../../.cursor/skills/hell-workers-review-help-impact/SKILL.md)で全実装差分のplayer-visible経路を確認する。
  - 実行されない旧ソースと開発者案内だけの整理で、現在のTooltip入力/文言/状態/表示は不変であることを確認してNo impactとする。
  - 受入と品質ゲートの結果を記録し、専用出力を整理する。
- 仕様更新対象（該当する契約を変更した文書だけを更新）:
  - [docs/cargo_workspace.md](../../../../docs/cargo_workspace.md)
  - [docs/speech_system.md](../../../../docs/speech_system.md)
  - [docs/DEVELOPMENT.md](../../../../docs/DEVELOPMENT.md)
- 完了条件:
  - [x] §7のケースと必要なnative受入が完了し、未検証範囲を明記した。
  - [x] HelpがUpdate required / No impactのいずれかに確定し、対応する更新または根拠を記録した。
  - [x] `check`・Clippy・`verify`が成功し、rust-analyzerの取得可能な診断にerrorがない。
  - [x] primary validation storage checkが成功し、不要出力の整理/継続用途を記録した。
- 検証: §7の完了gate。品質gateのpassだけをHelpの意味レビューやnative受入の代替にしない。

## 6. リスクと対策

| リスク | 対策 |
| --- | --- |
| 未宣言でも外部toolが読むsourceを削除する | Rust参照だけでなくscript/testの読込みを調べる。 |
| symlinkを実ファイルに置き換える | 正本_rules.mdのみ編集しsymlinkを確認する。 |
| 共有ファイルの並行変更を上書きする | 作業開始/各取り込み前にstatusと対象diffを確認し、変更所有を確認する。 |
| 過去のpassを新subjectへ流用する | 変更後の入力を固定して必要な検査を実施し、未実施は未実施と記録する。 |

## 7. 検証計画

### ケースと判定条件

| ケース | 期待する結果 |
| --- | --- |
| mod/path/include/script参照 | 削除後に参照残存なし。 |
| 現行Tooltip test | 位置・fade・Popoverなど既存挙動が継続。 |
| rules symlink/依存・Speech登録先 | 正本と実装が一致しadapter契約を壊さない。 |

### 回帰fixtureの作り方（新規test不要）

| ID / 検査 | 操作 | 合格条件 |
| --- | --- | --- |
| R11-T1 削除前参照調査 | 3pathと定義symbolをmodule宣言、#[path]、include、script/test入力、feature条件から検索。 | 実行/生成に使うconsumerなし。見つかれば削除前提を修正する。 |
| R11-T2 削除後参照調査 | 同じ検索とdocs link検査を実施。 | 残る参照は明示した歴史記録のみで、存在しないpathへのリンクなし。 |
| R11-T3 既存回帰 | 現行hw_ui Tooltipのpause/speed等のtestと通常gateを実行。 | 現行挙動の回帰なし。旧ソース向けの新testを追加しない。 |
| R11-T4 rules/symlink | _rules.mdとCargo/Speech登録を照合し、AGENTS.md/CLAUDE.mdのlink先を確認。 | 依存/登録説明が一致し、symlinkを通常ファイルへ置換していない。 |

### focused command（実装時に実行）

```bash
python3 scripts/check_agent_rules.py
python3 -m unittest scripts.tests.test_check_agent_rules scripts.tests.test_update_docs_index
python3 scripts/dev.py cargo -- test -p hw_ui interaction::tooltip
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

実行経路を変更しないためnative/GPU受入は不要。新規test frameworkや画像撮影を追加せず、既存回帰と参照/文書検査を使う。

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

必要な外部consumerが判明したファイルだけを、そのconsumerの所有先を明示して復元する。広範囲のcheckout/cleanは行わない。

戻す前に直近5commit、対象のHEAD差分、他sessionの所有を確認する。
広範囲のcheckout/cleanや他計画の変更破棄を行わず、レビュー可能な限定逆patchを作る。
保存型・asset formatを変える必要が生じた場合は、互換/復旧条件をこの計画へ追記してから変更する。

## 9. AI引継ぎメモ（完了記録）

### 現在地

- 2026-09-19完了。未コンパイルの旧Tooltip 3 fileを削除し、現行ownerの案内を更新。通常workspace Clippyは警告0。
- `python3 scripts/dev.py verify`が成功。通常／profilingのworkspace test、通常Clippy警告0、feature別compile、Python、Help、文書・storage gateを確認した。
- Helpは全体差分でUpdate required。Move失敗時の保持、タスク表示、建設工程の3 entryとexact snapshotを更新し、実際のplayer-visible経路と差分をレビュー済み。
- 本計画単体の専用native job／binary copy／worktreeは作成していない。通常primary cacheは継続開発用に保持する。R13/R14用candidateと未終了native確認は各計画が所有し、本計画の完了と分離する。

### 次のAIが最初にやること

現行の関連仕様と実装を参照する。本書の設計案・着手手順を未実装の作業として再実行しない。
新しい変更ではその差分に応じた回帰、Help判断、storage確認を行う。

### 最終確認ログ

| 検査 | 結果 |
| --- | --- |
| focused tests / 新規回帰 | 未コンパイルの旧Tooltip 3 fileを削除し、現行ownerの案内を更新。通常workspace Clippyは警告0。 |
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
| 2026-09-18 | Codex | R11の独立計画をテンプレートから作成。依存、milestone、受入、Help、検証データ管理を具体化 |
| 2026-09-18 | Codex | 計画レビュー: 削除前の全参照確認、symlink維持、現行Tooltip不変とnative不要の範囲を確認。追加修正不要。 |
| 2026-09-18 | Codex | 全計画ブラッシュアップ: 削除対象3pathとsymbolのmod/path/include/script/feature参照を再検索し、他session差分と歴史リンクの置換対象を記録する。 実装は未着手。 |
| 2026-09-19 | Codex | 実装・回帰・Help／仕様同期と品質gateを完了しarchive。 |
