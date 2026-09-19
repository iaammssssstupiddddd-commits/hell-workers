# R03: WorldMapのowner付き解放と仮予約解除の分離 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `refactor-r03-worldmap-owner-release-plan-2026-09-18` |
| ステータス | Complete |
| 作成日 | 2026-09-18 |
| 最終更新日 | 2026-09-19 |
| 作成者 | Codex |
| 関連提案 | [実装横断レビュー](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md) §5 R03 |
| 関連Issue/PR | N/A |
| 優先度 / 相対規模 | P1 / 中 |
| 調査基準 | `52913b61ca524f1cd429d3d73a0468faf3d0999b` |
| 計画レビュー | 2026-09-18 / [全体レビュー結果](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md#42-計画レビュー2026-09-18)参照。実装受入ではない |
| 計画具体化 | 2026-09-18 / API案・変更単位・fixture/期待値・着手手順を追加。新規名は未実装の設計案 |
| 実装状態 | 実装・必要な回帰・仕様／Help同期・品質gateを完了。専用native受入は不要。設計経緯としてarchiveし、現行仕様は関連docsを正本とする。 |

## 1. 目的

- 解決したい課題: owner不一致でもraw障害物を消すrelease APIと、無変更を保証するclear APIの契約差を解消する。
- 到達したい状態・成功指標: owner付き解除は不一致で全layerとrevisionが無変更となり、ownerなし仮予約は専用の明示操作で解除でき、完成・移設後に旧予約由来の障害物が残らない。

## 2. スコープ

### 対象（In Scope）

- WorldMap building release/clear/transfer APIと実caller
- 移設、床/壁完成、重複ownerとraw予約の回帰

### 非対象（Out of Scope）

- WorldMap/ECSの二重保持廃止
- door/bridgeの通行ルール変更、mapgenやpathfindingの変更

### 主な変更対象（現行ファイル）

- [crates/hw_world/src/map/buildings.rs](../../../../crates/hw_world/src/map/buildings.rs)
- [crates/hw_soul_ai/src/soul_ai/execute/task_execution/move_plant.rs](../../../../crates/hw_soul_ai/src/soul_ai/execute/task_execution/move_plant.rs)
- [crates/hw_jobs/src/tasks/move_plant.rs](../../../../crates/hw_jobs/src/tasks/move_plant.rs)（PendingBuildingMoveの適用入力を変更する場合）
- [crates/bevy_app/src/systems/jobs/wall_construction/completion.rs](../../../../crates/bevy_app/src/systems/jobs/wall_construction/completion.rs)
- [crates/bevy_app/src/systems/jobs/floor_construction/completion.rs](../../../../crates/bevy_app/src/systems/jobs/floor_construction/completion.rs)

新規moduleやtestは上記ownerの配下に置く。新しいcrateを既定案にせず、定義→生成→使用の順に変更する。

## 3. 現状とギャップ

release_building_grid_if_owned/if_matchesはowner照合失敗でもremove_grid_obstacleへ進む。clear_building_occupancy_if_ownedは不一致で無変更。移設/壁完成が前者を使用し、床完成にもownerを渡さない一括解除がある。通常プレイでの他owner破壊は未再現。

移設はTransform更新を先に積み、後続systemでmapを更新する。床完成も完成entity生成/旧tile削除を積んでからmapを登録する。map APIだけを拒否可能にするとECSだけ進むため、callerの適用順も見直す必要がある。

提案時点の静的な根拠と実行再現を区別し、M1で現HEADとの差・実際のcaller・既存testを再確認する。
仕様を直す前に実行経路・前提条件を確認し、再現できない仮説への値調整を繰り返さない。

### 既存テスト・検査の入口

- [crates/hw_world/src/map/doors.rs](../../../../crates/hw_world/src/map/doors.rs)
- [crates/hw_world/src/map/bridges.rs](../../../../crates/hw_world/src/map/bridges.rs)
- [crates/bevy_app/src/systems/jobs/deconstruction/tests.rs](../../../../crates/bevy_app/src/systems/jobs/deconstruction/tests.rs)

これらは再利用する入口であり、本計画の全条件を既に保証しているという意味ではない。
不足する受入ケースを下記milestoneで追加する。削除・文書整理だけの変更には実装をなぞる新規testを作らない。

## 4. 実装方針と具体設計

- 全callerについてbuilding layerのowner、raw obstacleの発行元、ownerなし予約が残る条件を先に整理する。名前だけで全releaseを置換しない。
- owner付きAPIは不一致で無変更とし、ownerなし仮予約を解除するAPIはownerなし/発行元の前提を明示して検査する。他ownerの障害物を強制解除する汎用escape hatchにしない。
- 旧ownerから新ownerへの移管をexplicit operationまたは検証済み手順にまとめ、door/bridge/passable facilityのlayer差を保持する。
- 移設・完成は変更対象footprint全体と移管先をlive再検証してから、Transform・完成entity・旧tile/siteを変更する。拒否時はWorldMapとECSを部分適用せず、移設は元位置を保持してabort_closed、床完成はCuringで保留する。map APIの戻り値を無視して完了phase/通知へ進めない。新しい永続owner台帳は導入しない。
- obstacle_versionは既存のwalkability変更契約に従う。no-opで増やさず、既存map property/terrain syncを壊さない。

### 具体設計と変更境界

WorldMap側のAPI案は `try_release_owned_footprint(owner, grids) -> Result<(), OccupancyConflict>`。全セル照合が成功するまでどのlayerも書き換えない。移管も旧footprintと移管先の両方を先に検証する。

raw obstacle bitだけでは発行元を識別できない。床のcallerはliveな `FloorTileBlueprint.parent_site/grid_pos`、`ObstaclePosition`、`ConstructionProtection`、siteの `CuringFootprint` を照合する。Protectionのownerはtile entityでありsiteと同一視しない。`ObstaclePositionIndex` は差分反映前の可能性があるため単独の許可根拠にしない。対象markerを除いた後も存在するmarker/direct blockerを保存する解除手順を用意する。

| 操作 | commit境界 | 拒否時 |
| --- | --- | --- |
| owner付き解除 | 全footprintのowner一致後にmapを更新 | 無変更で競合を返す。ownerなしfallbackへ進まない。 |
| raw予約解除 | callerが発行元と残存sourceをlive検証し、対象予約だけを解除 | 証明できなければ無変更。「building ownerなし」だけでは許可しない。 |
| 植物移設 | PendingBuildingMoveに提案Transform・task/worker識別を保持。適用systemが旧占有/移設先/taskを再検証し、成功時だけmapとECSを変更してDoneへ進める。 | 元位置を保持して既存abort_closedへ戻す。task取消の予約解放と移設の部分適用を区別する。 |
| 床完成 | 予約と全footprintの検証後に完成entity生成・旧tile/site削除・map移管 | Curingで保留し、完成entity/削除/完了通知を出さない。 |

準備段階ではTransformを移動しない。最終検証からmap/ECS適用までを同じ適用境界へ閉じ、検証後に状態が変わり得るdeferred処理を挟まない。成功結果を確認する前のDone遷移を撤去する。

### 実装単位と移行順

1. **R03-A / M1:** caller別の発行元表とmap全layer/構造状態の比較fixtureを作る。
2. **R03-B / M2:** owner付き全体検証APIとraw予約解除の前提を分離し、単体testを通す。
3. **R03-C / M3:** PendingBuildingMoveの準備/適用を移行してから床・壁完成/取消を移す。mapだけの修正を部分適用の解消と報告しない。

### 依存関係と着手順

必須先行なし。R01と独立。move_plantのQuery整理を行う場合はR09と順次取り込み、取消workerの抽出はR07の範囲へ任せる。

関連計画へのリンクと全体の着手順は[親提案の計画一覧](../../../proposals/implementation-refactor-audit-proposal-2026-09-17.md)を参照。
依存は実装上の前提と共有ファイルの編集競合を区別する。全14件の完了を本計画の終了条件にしない。

### Bevy 0.19と変更の所有者

- 既存の正しい0.19実装を基準とし、新しいAPIはrust-analyzer/docsrs MCPまたはlocal registryで確認してから使う。
- Queryはcrate-owned contextへまとめ、rootの配線・leafの実装所有を維持する。lint抑制で構造問題を隠さない。
- コードと文書の編集は主担当が行う。subagentは読み取り専用の調査/レビューに限定する。
- 共通Help snapshot・docs索引・凍結native subjectは並行編集しない。

## 5. マイルストーン

## M1: callerごとの予約所有を棚卸し

- 変更内容:
  - release/clear/footprint全callerを参照検索し、owner一致/他owner/なしの期待値表を作る。
  - stale owner、raw予約のみ、door/bridge重複の回帰テストを作る。
  - 専用testがまだないmove_plant.rs配下に、移設元/先のownerと予約解除を検証する回帰testを追加する。
  - map適用前後のCommands反映順を確認し、途中でownerが変わった場合の拒否とcleanup方針を固定する。
- 変更ファイル: R03-A: hw_world map tests、floor_construction/completion.rs、move_plantの新規適用回帰testとcaller台帳。
- 完了条件:
  - [x] 必要な仮予約解除と誤った他owner解除をテストで区別できる。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M2: 解放と移管のAPIを分離

- 変更内容:
  - owner不一致は無変更へ統一し、仮予約解除とowner移管の前提を表すAPIを導入する。
  - mapレベルのno-op/revision/property testを通す。
- 変更ファイル: R03-B: hw_world/src/map/buildings.rs、terrain_visual.rs の既存source契約を参照するcaller側検証。
- 完了条件:
  - [x] 各APIの保証が名前/戻り値/テストで一致する。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M3: 完成・移設callerを移行

- 変更内容:
  - 壁・床完成と移設を所有者表に従って移行し、旧APIの残存callerをなくす。
  - 正常完成、途中取消、遅延cleanup、複数footprintをECSで確認する。
  - PendingBuildingMoveの準備と適用を分け、成功前のTransform/子障害物/付属storage更新や完了通知を防ぐ。床・壁も全対象検証後に完成entityとmapを適用する。
- 変更ファイル: R03-C: hw_jobs/src/tasks/move_plant.rs、Soul task_execution/move_plant.rs の準備とapply_pending_building_move_system、同crate soul_ai/mod.rs の配線、root床/壁 completion・cancellation。Pendingを直接構築する既存testも追従。
- 完了条件:
  - [x] 他ownerを保持しつつ、正常完了後に古い占有/障害物が残らない。
- 検証: §7の該当するfocused commandとケース表。再現用testの初回失敗は記録し、修正後は成功を必須にする。

## M4: 仕様・Help・品質ゲートと終了整理

- 変更内容:
  - 下記仕様へ最終的な所有者・順序・失敗条件を反映する。計画の検討経緯をそのまま恒久仕様へ転記しない。
  - [Help影響Skill](../../../../.cursor/skills/hell-workers-review-help-impact/SKILL.md)で全実装差分のplayer-visible経路を確認する。
  - 完成/移設/取消のplayer-visible結果を追い、既存建築Helpと整合するか判断する。
  - 受入と品質ゲートの結果を記録し、専用出力を整理する。
- 仕様更新対象（該当する契約を変更した文書だけを更新）:
  - [docs/building.md](../../../../docs/building.md)
  - [docs/invariants.md](../../../../docs/invariants.md)
  - [docs/architecture.md](../../../../docs/architecture.md)
- 完了条件:
  - [x] §7のケースと必要なnative受入が完了し、未検証範囲を明記した。
  - [x] HelpがUpdate required / No impactのいずれかに確定し、対応する更新または根拠を記録した。
  - [x] `check`・Clippy・`verify`が成功し、rust-analyzerの取得可能な診断にerrorがない。
  - [x] primary validation storage checkが成功し、不要出力の整理/継続用途を記録した。
- 検証: §7の完了gate。品質gateのpassだけをHelpの意味レビューやnative受入の代替にしない。

## 6. リスクと対策

| リスク | 対策 |
| --- | --- |
| fallback削除で予約障害物が残る | ownerなしケースと正当な解除callerを先に固定。 |
| owner移管でdoor/bridge layerが消える | layer別期待値を持ち、既存特殊APIを尊重する。 |
| 共有ファイルの並行変更を上書きする | 作業開始/各取り込み前にstatusと対象diffを確認し、変更所有を確認する。 |
| 過去のpassを新subjectへ流用する | 変更後の入力を固定して必要な検査を実施し、未実施は未実施と記録する。 |

## 7. 検証計画

### ケースと判定条件

| ケース | 期待する結果 |
| --- | --- |
| owner一致/他owner/なし | building/raw/door/bridge各layerとobstacle_versionが契約どおり。 |
| raw予約だけのセル | 正当な発行元のraw予約だけを解除し、最終walkabilityは地形・door・bridgeの既存規則に従う。 |
| 移設後に旧owner cleanup | 新ownerと障害物を保持。 |
| 床・壁完成/取消と重複footprint | 解除漏れ・他owner消去なし。revisionはno-opで不変、最終walkabilityの有効な変更に応じて既存契約どおり更新。 |
| footprintの一部だけowner不一致・準備後に移管先が他ownerへ変更 | map各layer、Transform、子障害物、付属storage、旧tile/siteを部分更新しない。拒否した操作の完了通知はなく、§4の取消/保留方針へ進む。 |

### 回帰fixtureの作り方（新規test案）

| ID / test案 | 準備・操作 | 必須assert |
| --- | --- | --- |
| R03-T1 `owned_footprint_release_is_atomic_test` | 2セルのowner Aを作り、片方だけBへ置換してAの解除を実行。 | 全map layer・obstacle_versionが呼出前と同じ。Aの一致セルだけ消さない。 |
| R03-T2 `move_commit_revalidates_destination_test` | 移設準備後、旧footprintの一部/移設先をBへ変更して適用。 | 建物Transform・子marker・付属storage・占有は適用前と同じ。Done/完了通知なし。取消cleanupは既存契約どおり一度。 |
| R03-T3 `floor_completion_preserves_other_obstacle_sources_test` | 正当な床予約、別source重複、source欠落を別fixtureにし完成を試す。 | 正当時だけ移管。別sourceを保持。拒否時はCuring・旧tile/site保持、完成spawn/通知0。 |
| R03-T4 `release_keeps_passability_contract_test` | door/bridge/地形の組合せと、移管後の旧owner遅延cleanup。 | 旧予約由来の障害物だけを除き、新ownerと既存walkability規則を維持。no-op revision不変。 |

比較snapshotはmap layerに加えて建物/予約marker/site/tileのentity一覧を含める。移設拒否時の正当なtask解除まで「全World無変更」と要求しない。

### focused command（実装時に実行）

```bash
python3 scripts/dev.py cargo -- test -p hw_world
python3 scripts/dev.py cargo -- test -p hw_soul_ai move_plant
python3 scripts/dev.py cargo -- test -p bevy_app@0.1.0 construction
```

`move_plant`はM1で専用回帰testを追加した後に実行する。現時点の既存testによる保証として扱わない。

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

map/ECSテストを必須とする。完成後の通行・移設結果の補助実画面確認を行う場合はnative Skillを使う。描画変更や性能改善の証明は本計画の対象にしない。

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

map API導入とcaller移行を小さく分ける。戻す場合もowner不一致無変更の回帰を維持し、caller側の不足を修正する。save型は変更しない。

戻す前に直近5commit、対象のHEAD差分、他sessionの所有を確認する。
広範囲のcheckout/cleanや他計画の変更破棄を行わず、レビュー可能な限定逆patchを作る。
保存型・asset formatを変える必要が生じた場合は、互換/復旧条件をこの計画へ追記してから変更する。

## 9. AI引継ぎメモ（完了記録）

### 現在地

- 2026-09-19完了。WorldMapのowner-safe APIと完成／移設のexclusive commitを導入。移設6件、Blueprint／床／壁の競合拒否回帰が成功。
- `python3 scripts/dev.py verify`が成功。通常／profilingのworkspace test、通常Clippy警告0、feature別compile、Python、Help、文書・storage gateを確認した。
- Helpは全体差分でUpdate required。Move失敗時の保持、タスク表示、建設工程の3 entryとexact snapshotを更新し、実際のplayer-visible経路と差分をレビュー済み。
- 本計画単体の専用native job／binary copy／worktreeは作成していない。通常primary cacheは継続開発用に保持する。R13/R14用candidateと未終了native確認は各計画が所有し、本計画の完了と分離する。

### 次のAIが最初にやること

現行の関連仕様と実装を参照する。本書の設計案・着手手順を未実装の作業として再実行しない。
新しい変更ではその差分に応じた回帰、Help判断、storage確認を行う。

### 最終確認ログ

| 検査 | 結果 |
| --- | --- |
| focused tests / 新規回帰 | WorldMapのowner-safe APIと完成／移設のexclusive commitを導入。移設6件、Blueprint／床／壁の競合拒否回帰が成功。 |
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
| 2026-09-18 | Codex | R03の独立計画をテンプレートから作成。依存、milestone、受入、Help、検証データ管理を具体化 |
| 2026-09-18 | Codex | 計画レビュー: footprint全体のlive検証とWorldMap/ECSの部分適用禁止、PendingBuildingMoveと拒否回帰を補足。 |
| 2026-09-18 | Codex | 全計画ブラッシュアップ: 2セルowner Aの一部をBへ変更するR03-T1と、Pending作成後に競合させるR03-T2を用意し、mapとECSの適用時点を記録する。 実装は未着手。 |
| 2026-09-19 | Codex | 実装・回帰・Help／仕様同期と品質gateを完了しarchive。 |
