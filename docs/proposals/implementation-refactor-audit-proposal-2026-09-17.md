# 実装横断レビューとリファクタリング提案

## メタ情報

| 項目 | 値 |
| --- | --- |
| ドキュメントID | `implementation-refactor-audit-proposal-2026-09-17` |
| ステータス | Complete |
| 作成日 / 最終更新日 | 2026-09-17 / 2026-09-19 |
| 作成者 | Codex |
| 調査対象 | primary、`52913b61ca524f1cd429d3d73a0468faf3d0999b` |
| 関連計画 | §4.1の14件の個別計画（全件実装・必要受入成功・完了archive） |
| 関連Issue/PR | N/A |

## 1. 背景と問題

全13 crateの責務・依存・登録入口と、主要な実行経路を横断した静的レビュー。
**優先すべきは、到達判定・予約・所有者確認・表示分類の契約を各入口で揃えること。**
crate分割、pure policy、typed terminal、UIのViewModel/Intent分離は既に進んでいる。
新たな全面分割より、整理後に残る別経路と暗黙の前提を減らす効果が大きい。

2026-09-17の調査時点では提案と文書のみを作成した。2026-09-18の実装依頼に基づき、
R01〜R14の本体変更とfocused回帰を実装した。2026-09-19に`dev.py verify`が成功し、
通常／profilingのworkspace test、Clippy、feature別compile、Python、Help、文書とstorage gateが通過した。
R01〜R14は必要な受入と終了整理を完了し、全14計画をarchiveした。
Task Dashboardのaudit／Capture／Memoryと、地形3 LOD・診断prepass・native DPIリサイズの受入は成功した。
R13/R14のP08 closureも最終候補で成功し、Light Fieldの実descriptor・pixelとScene画像1920×1080を確認した。
R04/R08/R12はLoad後の警告も修正し、suite-iで全18 checkpoint・独立verify・全画像・WARN／ERRORなしを確認した。終了した検証jobは整理済みで、最終候補とbuild cacheはレビュー用に保持する。
ソースで確定した処理の差と、実行再現が必要な故障リスクを区別する。
全ファイルの全行精読、全状態の組合せ検証、実機・GPU性能監査を完了したという意味ではない。
以下の行番号は調査commit時点のもの。

## 2. 目的と調査範囲

| 領域 | 確認した経路・契約 | 評価 |
| --- | --- | --- |
| `hw_core` | 共有型、Relationship、state、epoch | 基本構成を維持 |
| `hw_jobs` | payload、entity参照、lifecycle、construction rule | R07で参照列挙を揃える |
| `hw_soul_ai` | assignment→execution→terminal、navigation、chain、解除context | R01/R02/R07/R09 |
| `hw_familiar_ai` | delegation、builder、validator、reservation shadow | 既存割当経路をR02/R06の基準にする |
| `hw_energy` とroot energy | pure allocation、topology、dirty→reconcile→allocation | model/pure/runtime境界を維持 |
| `hw_world` | map更新、path budget、Room、mapgen構成 | R03。生成アルゴリズム置換は提案しない |
| `hw_spatial` | typed index、差分更新、generation consumer | R10 |
| `hw_logistics` | producer→arbitration→AI、Stockpile受理・予約 | R05/R06 |
| `hw_ui` とroot UI/input | spawn/sync、inspection、Intent、入力競合・capture | R04/R08/R11 |
| `hw_visual`、root visual、WGSL | owner/revision、bar、material/import/binding | R12/R13。既存描画方式を維持 |
| `hw_infra` とroot lighting | pure field、snapshot/epoch、load reset、consumer | 現行pure coreとadapter境界を維持 |
| `bevy_app` save/settings/startup | schema、preflight、rollback、rehydrate registry、配線 | 既存保存基盤を維持。設定保存は既存提案を継続 |
| `visual_test` | Scene-only構成、productionとの分離、resize契約 | 独立性を維持 |
| 開発基盤 | `dev.py`、dependency policy、validation storage、perf config/validator | R14。既存guard/独立検証を維持 |

Rustのファイル構成・Cargo依存を棚卸しし、上表の主要経路、変更境界、関連テストと仕様を精読した。
rust-analyzer参照検索を利用し、不完全な応答はソース検索で補完した。
アセット原本の美術レビュー、Blender制作処理の全分岐、全OS・旧save全世代は対象外。

## 3. 非目的

- gameplay追加、承認済みUIレイアウトの再設計、Bevy更新。
- ECS/WorldMapの二重保持廃止、AI/物流/入力frameworkの全面置換。
- 行数だけを理由としたファイル分割、測定前の高速化断定。
- independent verifierを実装と同じ判定関数へ統合すること。

## 4. 提案一覧

P1は正しさに関わる入口差を優先して解消する項目、P2は保守負担の削減、P3は関連変更時に扱う項目。
規模は相対評価であり日数見積りではない。P1の挙動修正と、その後の構造整理は別の変更単位にする。

| ID | 優先 | 提案 | 規模 | 根拠の性質 |
| --- | --- | --- | --- | --- |
| R01 | P1 | 建設タスクの到達結果を網羅する共通navigation契約 | 中 | 到達不能でも到着判定へ進む分岐を確認 |
| R02 | P1 | 採集後チェーンの予約・受理・delivery適用を通常割当と揃える | 大 | 入口差を確認。二重運搬の実行再現は未実施 |
| R03 | P1 | owner確認付き解放と仮予約解除を分離する | 中 | owner不一致でも障害物を消すAPIを確認 |
| R04 | P1 | 一覧・詳細・tooltipのtask表示分類を共有する | 小〜中 | Deconstructの誤分類を確認 |
| R05 | P2 | TransportRequestの重複選択・更新・worker保持を共通化 | 中 | producer間で異なる実装を確認 |
| R06 | P2 | Stockpile受理判定の入力snapshotをlogisticsへ集約 | 中 | evaluator以前の入力構築が重複 |
| R07 | P2 | taskのentity参照列挙を取消・割当拒否で共有 | 中 | secondary参照の扱いが経路で異なる |
| R08 | P2 | Soul一覧のtyped node参照とwidget所有の値同期 | 中 | 別crateのChildren位置への依存 |
| R09 | P2 | 解除専用経路を最小Query contextへ移す | 小〜中 | 既存の最小contextが利用可能 |
| R10 | P2・後順位 | SpatialIndexのraw mutationを閉じる | 小〜中 | generationを迂回できる公開API |
| R11 | P2・低工数 | module treeから外れた旧Tooltipと古い所有説明を整理 | 小 | 未参照ソース・実装と異なる案内 |
| R12 | P3 | ProgressBarの親接続と床・壁のライフサイクルを共通化 | 中 | 親引数未使用・生成破棄の重複 |
| R13 | P3 | Terrain materialのABI宣言を共通化 | 中〜大 | WGSL/Rust間の複数宣言 |
| R14 | P3 | perf設定をworkload別の検証済み型へ整理 | 中〜大 | 無効な組合せを表せる共通設定と分散validation |

### 4.1 個別実装計画（2026-09-18作成）

ユーザーの計画書作成依頼に基づき、全14項目を独立した計画へ展開し、レビュー・具体化後に実装へ進んだ。
各計画は対象ファイル、milestone、回帰ケース、Help判断、必要なnative受入、保存管理と引継ぎを持つ。

| ID | 個別計画 | 優先 | 状態 |
| --- | --- | --- | --- |
| R01 | [建設タスクの到達結果と中断契約の統一](../plans/refactor/archived/refactor-r01-construction-navigation-plan-2026-09-18.md) | P1 | 完了・archive |
| R02 | [採集後チェーンの受理・予約・segment適用の統一](../plans/refactor/archived/refactor-r02-gather-haul-chain-plan-2026-09-18.md) | P1 | 完了・archive |
| R03 | [WorldMapのowner付き解放と仮予約解除の分離](../plans/refactor/archived/refactor-r03-worldmap-owner-release-plan-2026-09-18.md) | P1 | 完了・archive |
| R04 | [一覧・詳細・Tooltipのタスク表示分類統一](../plans/refactor/archived/refactor-r04-task-presentation-plan-2026-09-18.md) | P1 | 完了・archive |
| R05 | [TransportRequest producerの重複選択と更新処理の共通化](../plans/refactor/archived/refactor-r05-transport-request-reconcile-plan-2026-09-18.md) | P2 | 完了・archive |
| R06 | [Stockpile受理判定のsnapshotと予約入力の集約](../plans/refactor/archived/refactor-r06-stockpile-inbound-snapshot-plan-2026-09-18.md) | P2 | 完了・archive |
| R07 | [タスク参照entityとowner取消判定の共有](../plans/refactor/archived/refactor-r07-task-entity-references-plan-2026-09-18.md) | P2 | 完了・archive |
| R08 | [Soul一覧のtyped node参照とwidget値同期の移管](../plans/refactor/archived/refactor-r08-soul-row-node-ownership-plan-2026-09-18.md) | P2 | 完了・archive |
| R09 | [解除専用経路の最小Query context化](../plans/refactor/archived/refactor-r09-task-unassign-context-plan-2026-09-18.md) | P2 | 完了・archive |
| R10 | [SpatialIndexのmutation APIとgeneration契約の保護](../plans/refactor/archived/refactor-r10-spatial-index-mutation-plan-2026-09-18.md) | P2・後順位 | 完了・archive |
| R11 | [未参照Tooltip実装と古い所有説明の整理](../plans/refactor/archived/refactor-r11-obsolete-tooltip-cleanup-plan-2026-09-18.md) | P2・低工数 | 完了・archive |
| R12 | [ProgressBarの親所有APIと床・壁のライフサイクル共有](../plans/refactor/archived/refactor-r12-progress-bar-lifecycle-plan-2026-09-18.md) | P3 | 完了・archive |
| R13 | [Terrain materialのuniform・binding宣言の共有](../plans/refactor/archived/refactor-r13-terrain-material-abi-plan-2026-09-18.md) | P3 | 完了・archive |
| R14 | [perf設定のworkload別型とvalidation境界の整理](../plans/refactor/archived/refactor-r14-perf-workload-config-plan-2026-09-18.md) | P3 | 完了・archive |

推奨順序は次のとおり。共有ファイルの競合は実装依存とは区別し、主担当が順次変更する。

- R01/R04を先行し、R03とR02の限定修正で正しさの契約を固定する。
- R02は既存evaluatorで限定修正を完結できる。R06はR02に依存せず既存callerを移行し、R02が後から共通型へ追従する。相互の完了待ちを作らない。
- R07の参照整理、R09のcontext整理、R11の旧ソース撤去は独立着手可能。R02/R07/R09の共有contextや取消callerは同時編集しない。
- R05/R06はproducer群の共有ファイルを順次移行する。R08はR04の表示分類を先に取り込むことを推奨する。
- R04/R08/R12はNativeUi fixture・UI helper・Python verifier testを共有する。caseを順次追加し、受入中のsource/harnessを編集しない。
- R10は公開APIのconsumerを確認して実施。R12〜R14は後順位とし、R13/R14が使うconfig・RenderDoc契約を同時編集しない。
- Help snapshot・docs索引は主担当が直列更新する。native batchの凍結中は関連source/asset/harnessを編集しない。

### 4.2 計画レビュー（2026-09-18）

計画書作成後のレビュー依頼に対応し、全14件を同じ基準commitの現行ソース・テスト・検証helperと再照合した。
9件の内容を修正し、5件は追加修正不要と判断した。各計画へレビュー日と履歴を記録している。
この判断は計画の整合性に対するもので、実装完了・専用回帰やnative受入の成功を意味しない。全件Draftを維持する。

| 計画 | 結果 | 確認・修正した要点 |
| --- | --- | --- |
| R01 | 追加修正不要 | Found後の到着判定、Deferred維持、Unreachable終端と限定修正→共通化の順序が一致。 |
| R02 | 修正済み | sourceだけでなく資材別receiver容量も一体でclaim。owner/pending拒否と異種・残り1枠競合を追加。 |
| R03 | 修正済み | map拒否時にTransform/完成entityだけ進まないよう、footprint全体のlive検証と部分適用禁止を追加。 |
| R04 | 修正済み | task語彙の専用fixture/helper/verifier入口、可視文字と対象identityの独立検証を具体化。 |
| R05 | 修正済み | 需要0/duplicateのworker保持とanchor/issuer消失のcloseを分離。既存schedule/flushと終端回帰を追加。 |
| R06 | 追加修正不要 | phase別入力・自己予約・unknown予約・live再検証を保持し、R02との循環依存がない。 |
| R07 | 修正済み | payload参照とidentity/WorkingOnを含むshell参照を区別。preflightと適用時の参照漏れを防ぐ。 |
| R08 | 修正済み | row保持を非構造更新に限定。dirty橋渡しを維持し、専用caseを既存smokeの1画面で受入。 |
| R09 | 追加修正不要 | consumer調査→最小context移行→未参照撤去の順序とobserver/system smokeが妥当。 |
| R10 | 追加修正不要 | no-op/rebuild時のgenerationとworld replacementの明示reset、raw reader移行が区別されている。 |
| R11 | 追加修正不要 | 未参照の再確認、symlink維持、現行Tooltip不変の範囲が明確。新しいnative検証は不要。 |
| R12 | 修正済み | 比率端値はECS、nativeは中間表示/追従/終了時消滅。既存UI入口へworld Sprite観測を追加する。 |
| R13 | 修正済み | prepassへ本pass資源宣言を持ち込まない。新importのRenderDoc snapshot受渡しと既存LOD probe再利用を明記。 |
| R14 | 修正済み | 無効時Defaultとfeature別拒否を固定。明示入力によるparseテスト、validated payloadの変更制限、consumer移行順を追加。 |

UI計画は新しいviewport選択機構を追加せず、既存smoke（1920×1080/UI scale 1）を基本とする。
layout差が生じた場合のみ既存6組合せへ拡大する。新しい専用caseは既存fixtureの単なるpassで代替しない。
R12も既存NativeUi経路を拡張し、新しいperf workloadは作らない。

### 4.3 全計画の具体化（2026-09-18）

追加のブラッシュアップ依頼に対応し、全14件へAPI/型の案、処理順と拒否時の扱い、変更単位、fixtureの入力・操作・assert、次担当の最初の作業を追記した。
新規API・module・test名は設計案であり、存在する実装や実行済みtestとしては扱わない。各計画の§4・§5・§7・§9に詳細を置く。

| 計画 | 具体化した実装判断 | 最初に固定する検証 |
| --- | --- | --- |
| R01 | 専用navigation入口、材料はsite Transform、Found後だけ従来到着policyを評価 | 3 task×2移動phaseの隔絶mapと予算0 |
| R02 | 空のcycle shadow、prepare→commit、既存Soul巡回順を維持 | 2 Soul・2 source・残り1枠のStockpile/Mixer |
| R03 | footprint全体の検証後に適用。移設拒否は元位置で取消、床完成拒否はCuring保留 | 準備後のowner/移設先競合とmap/ECS部分適用なし |
| R04 | allocation不要の表示分類とphase整形を別APIにする | 全17 variantと表示固有の分類差、専用native case |
| R05 | worker優先＋stable ID、TotalSlots/AdditionalSlotsを分離、需要0をkind別policyで保持 | duplicate順序反転、総枠3/追加枠3、Decide/Maintain別の結果 |
| R06 | 在庫snapshotと予約snapshotを分け、ownedをdestinationに限定 | 数値fixture、未知資源、committed/new混在batch |
| R07 | payload参照とtask shell参照を分け、取消入口ごとにroleを選ぶ | secondary搬入先取消とidentity-onlyの一括terminal |
| R08 | row Componentに7 node参照、leafへ値同期を移す | 異なるicon handle、同値非書込み、行/epoch寿命 |
| R09 | 5入口を既存TaskUnassignQueriesへ移し、TaskReservationAccessは残す | 割当用Messageなしのsystem/observer実行 |
| R10 | upsert、clear、replace_positionsと読取APIでmutationを閉じる | 再insert時の一意membershipとno-op/rebuild generation |
| R11 | 旧3ファイルだけ削除、所有説明とcommit付き歴史参照を同期 | module/script/feature参照、既存Tooltip、symlink |
| R12 | 同じ親の兄弟pairを生成、床/壁だけlifecycleを共有 | 4 callerの端値/孤児、world Sprite専用native観測 |
| R13 | WGSL型/本pass bindingを2moduleへ分け、RenderDoc転送まで同時移行 | ABI固定表、不正binding拒否、既存LOD probe/P08 |
| R14 | 注入可能な入力とprivateな検証済み設定、Python正規化も互換維持 | 無効時Default、feature/precedence、11 workloadの出力値 |

最初の実装候補は引き続きR01/R04。R02-AはR06を待たず既存evaluatorで受理・claim修正を完了でき、共通snapshotへの追従だけをR02-Bに分ける。
R05の選択規則統一は採用案を確定したが、各producerの需要式を変更するものではない。identity全体の新しい総枠policyは別の挙動変更として扱う。
R04/R08/R12のnativeは同じ既存3入口を順次拡張し、R13/R14も既存recipeと独立verifierを利用する。

## 5. 詳細設計

### R01 — 建設タスクのnavigation契約

根拠は [reinforce_floor.rs](../../crates/hw_soul_ai/src/soul_ai/execute/task_execution/reinforce_floor.rs) L39/L117、
[pour_floor.rs](../../crates/hw_soul_ai/src/soul_ai/execute/task_execution/pour_floor.rs) L39/L113、
[frame_wall.rs](../../crates/hw_soul_ai/src/soul_ai/execute/task_execution/frame_wall.rs) L31/L76。
これらは`Deferred`だけでreturnし、`Unreachable`でも`is_near_target_or_dest`を評価する。
[path_cache.rs](../../crates/hw_soul_ai/src/soul_ai/execute/task_execution/path_cache.rs) L224では
到達不能時に旧Destinationを変更せず、[navigation.rs](../../crates/hw_soul_ai/src/soul_ai/helpers/navigation.rs) L31は旧目的地への近接も認める。
材料中心に到着した後、隔絶されたタイルへ向かう条件では、旧目的地近傍から作業phaseへ進める分岐が成立する。
実機での発生頻度は未確認。

最初にこの条件の回帰テストと限定修正を行い、その後既存`NavOutcome`へ寄せる。
`Found`の後だけ到着判定、`Deferred`は無変更、`Unreachable`は明示した中断方針とする。
[coat_wall.rs](../../crates/hw_soul_ai/src/soul_ai/execute/task_execution/coat_wall.rs) L238の網羅的な分岐が基準。
距離閾値・探索budget・床壁の状態機械そのものは変更対象に含めない。

受入: 3タスクの材料中心/タイル移動で正常到達、旧目的地＝現在地、到達不能、予算切れを確認。
中断時の予約解放、`WorkingOn`除去、完了イベント非発火を検査する。

### R02 — 採集後チェーンと通常割当の共通契約

[gather.rs](../../crates/hw_soul_ai/src/soul_ai/execute/task_execution/gather.rs) L183のDoneから
[chain.rs](../../crates/hw_soul_ai/src/soul_ai/execute/task_execution/chain.rs) L185を呼び、
gather L194以降でpayload・identity・`WorkingOn`を直接更新する。
候補は可視性、`StoredIn`、`LoadedIn`を確認するが、予約・`TaskWorkers`・`DeliveringTo`・手動固定sourceを確認しない。
新segmentのsource/mixer予約と`DeliveringTo`付与も通常割当と異なる。
chain L299のStockpile fallbackは内容種別と物理容量を見ており、acceptance/targetを評価しない。

通常経路はFamiliar submitのshadowとSoul assignment applyで予約・deliveryを反映する。
共通化するのは候補受理とclaim・予約・delivery適用部分で、chainの元assignment identityは維持する。
Soul→Familiarの依存は追加せず、共有判定を`hw_jobs`/`hw_logistics`、反映を`hw_soul_ai`に置く。
同じframeの候補選択に効くclaimと、初回assignment/segment遷移の違いを明示する。

ここから「禁止資材が格納される」とは結論しない。
[haul/dropping.rs](../../crates/hw_soul_ai/src/soul_ai/execute/task_execution/haul/dropping.rs) L166で
live policyを再検証し、L225で拒否・cleanupする。確認できたのは、拒否先へ運び始め得る入口と予約契約の差。
同時workerのsource競合は再現テストで確定する。

受入: 2 Soulの同時Done、通常割当との競合、手動固定source、acceptance拒否、target到達、Mixer残容量、
中断後予約、chain全体でのidentity維持と最終segmentだけの完了通知。
R06の共通入力型は利用できるが、全producer移行をR02の着手条件にしない。

### R03 — WorldMapのowner付き解放

[buildings.rs](../../crates/hw_world/src/map/buildings.rs) L64の`clear_building_occupancy_if_owned`は
owner不一致なら無変更だが、L72の`release_building_grid_if_owned`は不一致でも`remove_grid_obstacle`を呼ぶ。
`release_building_grid_if_matches`にも同じfallbackがある。
実callerは [move_plant.rs](../../crates/hw_soul_ai/src/soul_ai/execute/task_execution/move_plant.rs) L144と
[壁完成](../../crates/bevy_app/src/systems/jobs/wall_construction/completion.rs) L118。

owner確認付きAPIは不一致で無変更に統一し、ownerのない仮予約を消す操作を別APIとして明示する。
移設・完成の旧owner→新owner移管も同じ境界で表現する。
先に各callerで仮予約だけのセルが生じる条件を確認する。単純にfallbackを削除すると障害物が残る可能性がある。
通常プレイで他ownerが破壊されたという観測はなく、APIの保証と実装の差が根拠。

受入: owner一致/不一致/なし、複数footprint、door/bridge重複、旧建築物の遅延cleanupで
map各layerと`obstacle_version`を検査。床完成のownerなし一括解除も同時にcaller監査する。

### R04 — taskの表示分類

[presentation/mod.rs](../../crates/bevy_app/src/interface/ui/presentation/mod.rs) L277の`format_task_str`は
`Deconstruct`を列挙せず、L298の`_ => "BucketTransport"`へ落とす。
[builders.rs](../../crates/bevy_app/src/interface/ui/presentation/builders.rs) L40から詳細/tooltip用modelへ渡る。
一方 [list/view_model.rs](../../crates/bevy_app/src/interface/ui/list/view_model.rs) L81の一覧分類は解体を明示する。
ソース上確定した表示不整合であり、今回の実画面撮影は行っていない。

rootの共通presentation adapterで作業分類を共有する。
`AssignedTask::work_type()`を利用できる部分と、HaulToBlueprint等の表示固有区別・phase表記を分け、
task variantを隠す包括fallbackをなくす。
UI改善提案U14で直した一覧とは別経路に残った問題として扱う。

受入: 全variantの分類、解体・発電・BucketTransportの一覧/詳細/tooltip一致。
player-facing labelの変更としてHelp Skillで影響を判定する。

### R05 — TransportRequest producerのreconcile

[producer/upsert.rs](../../crates/hw_logistics/src/transport_request/producer/upsert.rs) L18の旧重複処理は
先に走査したrequestを採用する。一方L215のStockpile用は実行中requestを優先し、stable IDで選び、
非canonicalの新規枠を閉じて既存workerを残す。Blueprint/Tank/Mixer等には旧方式が残る。

identity定義・需要計算は各producerに残し、canonical選択、worker保護、spawn、同値更新回避を共有する。
需要0で休止かdespawnか等の意味のある差はpolicyに残す。
共通spawn specはpriorityと追加component群を表せる形を検討し、巨大な汎用producerは作らない。

受入: query順を変えたduplicate、workerあり/なし、需要0、anchor消失、再有効化、同値更新時にChangedなし。
`desired_slots`は総枠のまま保持し、producerの総枠入力と追加枠入力を区別する。選択規則の変更は挙動変更として明示する。

### R06 — Stockpile入力snapshot

pure evaluatorは既に共通だが、入力生成が
[task_area.rs](../../crates/hw_logistics/src/transport_request/producer/task_area.rs) L145、
[consolidation.rs](../../crates/hw_logistics/src/transport_request/producer/consolidation.rs) L272、
[arbitration/candidates.rs](../../crates/hw_logistics/src/transport_request/arbitration/candidates.rs) L113、
[Familiar capacity_helpers.rs](../../crates/hw_familiar_ai/src/familiar_ai/decide/task_management/validator/capacity_helpers.rs) L17、
[Soul stockpile_policy.rs](../../crates/hw_soul_ai/src/soul_ai/execute/task_execution/stockpile_policy.rs) L24に分散する。

内容snapshot、資源別予約数、policy input constructorを`hw_logistics`の値型に集約する。
`NewInbound`と自己予約を含む`CommittedInbound`を区別する。
各phaseのQueryと取得時点、grant直前/unload時のlive再検証は維持する。長寿命cache化は別課題。

受入: 未知資材の物理予約、異種/自己予約、mixed batch、target/acceptance変更、重複Yardのshadow、
owner不一致、特殊storage。効果は判断基準の保守性であり、速度向上は未測定。

### R07 — taskのentity edgeとowner取消

[tasks/mod.rs](../../crates/hw_jobs/src/tasks/mod.rs) L182には`references_entity`があるが、
[task_assignment_apply.rs](../../crates/hw_soul_ai/src/soul_ai/execute/task_assignment_apply.rs) L255はvariantを再列挙し、
[床取消](../../crates/bevy_app/src/systems/jobs/floor_construction/cancellation.rs) L25と
[壁取消](../../crates/bevy_app/src/systems/jobs/wall_construction/cancellation.rs) L105は
payloadの一部/primaryと`WorkingOn`中心でworkerを選ぶ。
チェーンの`WorkingOn(item)`とpayload搬入先siteのようなsecondary参照の扱いが揃っていない。
後段の対象消失で回復する余地と、取消時に抽出されるかどうかを区別する。

まず既存`references_entity`で揃うcallerを移し、必要ならrole付きedge列挙から参照判定を導出する。
order/task_entity/搬入先を無条件に同一視せず、取消対象ごとのroleを定義する。
既存のexternal terminal preflightを活用する。

受入: source/destination/site/tile/wheelbarrow/order、WorkingOn欠落、chain搬送中の取消、解体中ownerへの割当。
予約二重解放・refund二重発生を防ぐ。

### R08 — Soul一覧のtyped node参照

[hw_ui/list/spawn.rs](../../crates/hw_ui/src/list/spawn.rs) L406が行構造を作る一方、
[root list/sync.rs](../../crates/bevy_app/src/interface/ui/list/sync.rs) L193は
`Children[0,1,3,5,6,7,8]`の順序を前提に更新する。表示色/iconの対応も両側に重複する。

既存`InfoPanelNodes`/`FamiliarSectionNodes`同様に`SoulRowNodes`を生成し、
値同期を`UiAssets`注入付きの`hw_ui` APIへ移す。rootはゲームQuery→ViewModelを保持する。
承認済みの列幅や配置を変えずに、レイアウト変更時の同期漏れを防ぐ。

受入: 初回spawnと同じrow Entityの値更新の一致、改名・所属/task変更、検索、折りたたみ。
表示回帰は実装後にnative Skillで確認する。

### R09 — 解除専用Query context

[context/queries.rs](../../crates/hw_soul_ai/src/soul_ai/execute/task_execution/context/queries.rs) L138の
`TaskAssignmentQueries`はstorage、assignment writer、多数のread Queryを含む。
cleanup、pathfinding fallback、rootの疲労/stress observerは、最終的に解除用traitへ渡すだけの用途がある。
L162には既に小さい`TaskUnassignQueries`があり、同じreservation accessを実装する。

解除専用callerを既存最小contextに移し、使用がなくなった旧contextと互換re-exportを撤去する。
crate-owned contextの分離をやり直す作業ではなく、残った不要な依存面の整理。

受入: 参照検索と全workspace compile、pathfinding failure、missing Familiar、疲労/stress、wheelbarrow cleanup。
不要Query削減による実速度は別途測定する。

### R10 — SpatialIndexのmutation境界

[grid.rs](../../crates/hw_spatial/src/grid.rs) L329の`data_mut`とL9のpublic storageは、
semantic generationを進めず内部を変更できる。
L37の`GridData::insert`は再insert時に旧bucketを消さない。
[Familiar diagnostics.rs](../../crates/bevy_app/src/systems/familiar_ai/diagnostics.rs) L671はgenerationをcache失効に利用する。
現状の`data_mut`利用はテスト中心であり、runtime障害を観測したとの主張ではない。

raw fieldsを非公開にし、`clear`/`rebuild`と設定用constructorを用意する。
`insert`はupsertか未登録限定かを明示する。typed tag・差分updater・現在のcell sizeは維持する。

受入: 同値update、remove、別cell再insert、clear/rebuildでmembership一意性とgenerationを検査。

### R11 — 移設後の旧ソースと案内

rootの [tooltip/mod.rs](../../crates/bevy_app/src/interface/ui/interaction/tooltip/mod.rs) は
`hw_ui`へ委譲するが、同directoryにmodule宣言・include参照のない追跡済み
`fade.rs`、`layout.rs`、`target.rs`が残る（合計326行）。現行実装と既に差がある。
削除前にmodule treeと参照を再確認し、旧ファイルを撤去する。

同時に [hw_visual/_rules.md](../../crates/hw_visual/_rules.md) の残存依存/Speech登録先の説明と、
[root visual README](../../crates/bevy_app/src/systems/visual/README.md) の廃止ファイル案内を同期する。
現行Speech登録は`SpeechVisualIngressSet`/Visualである。

受入: 未参照確認、docs/rules gate、通常品質ゲート。実行経路が変わらなければ実画面の再撮影は不要。

### R12 — ProgressBarの親所有とライフサイクル

[progress_bar.rs](../../crates/hw_visual/src/progress_bar.rs) L54は親と親Transformを受けるが使用せず、
local座標の2entityを返す。ChildOf付与はSoul/Blueprint/Floor/Wallのcaller側へ分散する。
床・壁には親収集→可視判定→生成→不要bar破棄の同形処理もある。

親接続済みのbarを返すAPIに揃え、不要なTransform引数をなくす。
その後、床/壁は`visible/ratio/color/config`の導出を残し、生成・破棄を共有する。
Z値、完成時消滅、world replacementを保持する。実装時の表示確認にはnative Skillを使う。

### R13 — Terrain materialのABI

`TerrainSurfaceUniforms`がWGSLのfull/lod1_lite/lod2/prepassへ重複し、
[terrain_surface_material.rs](../../crates/hw_visual/src/material/terrain_surface_material.rs) の3 extensionにも
同じfield/binding群がある。uniform/binding宣言だけを共有し、LOD固有のsamplingとblendは各shaderに残す。
大きなmacroやshader全統合は不要。

受入は3 LOD、prepass、shader import、binding対応、DPI/resize、productionのnative/GPU証拠。
**現行のIndoor Lightはbinding維持・fragment sampling停止という明示契約**を持つ。
未使用に見えるbindingの削除やsampling再有効化はこのリファクタに含めない。

### R14 — perf設定のworkload別構造

[perf_scenario/config.rs](../../crates/bevy_app/src/plugins/startup/perf_scenario/config.rs) L541は
共通resourceにworkload固有のOption/flagを保持し、L759以降等で組合せを検査する。
[arguments.py](../../scripts/perf_tool/arguments.py) L673以降にもworkload別の制約、
[artifacts.py](../../scripts/perf_tool/artifacts.py) L1574以降にはartifact別分岐がある。
同じworkloadの変更が複数の大きな関数を横断する構造が保守課題。現行誤受理を確認したわけではない。

生CLI/env入力と検証済み設定を分け、Rustでは共通設定＋workload別payload、Pythonでもworkload別validatorに整理する。
既存のartifact reader分割を延長し、汎用plugin frameworkは導入しない。
Rust側は単発実行、Python側はmatrix/instrumentationの制約を持つため、双方の条件を機械的に同一化しない。
独立verifierの期待値・hash・拒否判定は独立のまま保持する。

受入: 全既存CLI/envの正常・拒否組合せ、source/asset契約、Capture→Memory順序、元artifact必須、
save/settings隔離、storage coordinator。既存self-testを維持しworkloadごとに段階移行する。

## 6. 代替案と既存提案

| 案 | 判断 | 理由 |
| --- | --- | --- |
| root/leafをもう一度全面分割 | 見送り | 現行境界とordering facadeは機能している |
| `hw_energy`へroot energy systemを一括移動 | 不採用 | 同crateはmodel/pure専用・他hw依存禁止という明示契約 |
| 大ファイルを一定行数で分割 | 見送り | 例: wall_presentationは約260行の本体に厚いテストが付く |
| 全taskを単一の汎用状態機械へ変更 | 見送り | terminal/nav/edgeの局所契約を先に揃えられる |
| WorldMapとECSの二重保持を廃止 | 再提案しない | 過去のspatial提案で不採用。今回は所有権APIを改善 |
| visual_testをproduction fixtureへ統合 | 見送り | Scene-only検証の独立性に意味がある |
| perf実装とverifierを同じrule関数へ統合 | 不採用 | 同じ誤りで合格する構造を作らない |

[ライブラリ・開発ツール評価](library-tooling-evaluation-proposal-2026-09-13.md) の
R02「設定保存への既存atomic I/O再利用」は引き続き有力。
settingsの`std::fs::write`とsaveのatomic writerの差は現行にも残るが、新規提案として重複計上しない。
保存先移行、PRNG、rstar、SmallVec等も同提案の採用条件・測定に従う。

## 7. 影響範囲

- 調査・計画作成の変更: この提案、14件の個別計画と索引。gameplay、runtime data、save schema、Help本文の変更なし。
- R01〜R04: 限定的な挙動/表示修正を含む。契約の回帰例を先に固定する。
- R05〜R10: 主にRustの内部APIと所有境界。保存型のpath/Reflect名を不用意に変えない。
- R12: 親接続・位置・消滅のnative表示確認が必要。R13はshader/bindingを含むGPU証拠も必要。
- 性能: 改善率・allocation削減は未測定。必要なら同じcandidate/cacheを保った比較を別途計画する。
- 更新する仕様: `tasks.md`、`invariants.md`、`logistics.md`、`building.md`、`entity_list_ui.md`、
  `info_panel_ui.md`、`cargo_workspace.md`等を採用項目に合わせて選ぶ。

## 8. リスクと対策

| リスク | 対策 |
| --- | --- |
| 共通化でpolicy差やphase順序まで変える | 既存挙動をケース表にし、意味のある差を入力/結果に残す |
| deferred Commandsのため同frame予約が見えない | R02でshadow/claimの所有者と反映時点を明記する |
| cleanupの共通化で二重解放・refund | identity/owner一致を保ち、取消・完了・abortの各終端を検査 |
| snapshot共通化でlive再検証を落とす | 型だけを共有し、各phaseの取得時点を維持する |
| 良好な保存・電力・照明構造を崩す | registry、pure allocator、epoch境界を変更理由なしに移設しない |
| 未再現のリスクを確定バグと扱う | ソース事実、成立条件、再現結果を分けて記録する |

## 9. 検証計画と今回の結果

2026-09-17、上記commitの実装に対して `python3 scripts/dev.py verify` は成功。
fmt、workspace check、profiling関連gate、Clippy `-D warnings`、Rust/Python test、perf self-test、
依存監査、docs/rules/Help/storage/diff検査を通過した。
cargo-denyは既存依存の複数version警告を出したが、監査は成功した。これはClippy警告ではない。
ignored指定の外部candidate資産テスト等は実行成功に含めない。

rust-analyzerのworkspace一括診断は予期しない応答形式で取得できなかった。
`bevy_app/src/lib.rs`の個別診断はerror/warning 0、無効featureのhint 1。
workspaceのコンパイル成立は上記品質ゲートで確認した。
実機、GPU、性能比較、R01〜R03の専用再現テストは今回未実施。

提案/索引編集後のdocs生成・freshness・リンク・Help・storage・diff検査も成功。
通常Cargo cacheを再利用し、新しいnative job、binary copy、worktreeは作成していない。
review-active cacheの削除・変更も行っていない。

2026-09-18、14件のDraft計画作成後にも `python3 scripts/dev.py verify` が成功した。
計画の全305件のローカルリンクと末尾空白を確認し、読み取り専用レビューで移行対象・既存契約・test filterを補正した。
これは文書追加時点の既存実装に対する品質ゲートであり、各計画の将来の実装・専用回帰・native受入は未実施。
新しいnative job・binary copy・専用worktreeは作成していない。

同日の計画再レビュー・修正後も `python3 scripts/dev.py verify` が成功し、Clippy警告0を含む全品質ゲートを通過した。
全14計画のレビュー記録、339件のローカルリンク、docs索引/リンク、Help（production変更なし）、storage、diffを確認した。
rust-analyzerのworkspace診断は今回も予期しない応答形式で取得できず、コンパイル成立はverifyで確認している。
今回の修正は計画文書のみで、各計画の新しい受入ケースは実装時に実施する。

同日の全14計画ブラッシュアップ後も `python3 scripts/dev.py verify` が成功した。
各計画の具体設計・変更単位・着手手順、計57件の名前付き検証ケース、339件のローカルリンクと空白を確認し、docs索引とprimary storage checkも通過した。
読み取り専用レビューで、Gather競合敗者の正常完了、Mixer候補のrequest前提、slot入力の違い、終端条件と所有pathを補正した。
文書のみの更新であり、ここに追加した回帰test・native/GPU受入は未実施。専用job・binary copy・worktreeは作成せず、既存cacheを保持した。

実装時は各項目の受入条件に加え、`python3 scripts/dev.py check`、
`python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`、`python3 scripts/dev.py verify`を通す。
Help Skillによる実経路レビューを行い、実画面が必要な項目はnative Skillとprimary validation coordinatorを利用する。

## 10. 段階導入と戻し方

1. R01/R04の小さい回帰修正から開始。確認した問題と修正を独立してレビューする。
2. R03のowner契約、R02のchain契約をそれぞれ別batchで固定する。
3. R06/R07を利用してR02の共有部分を整理し、R05へ展開する。全物流一括改修は避ける。
4. R08/R09/R11は別領域として独立着手可能。R10は利用面を閉じられる時に実施する。
5. R12〜R14は該当機能の次の変更に合わせ、受入費用と釣り合う場合に実施する。

挙動修正を残したまま構造整理だけを戻せる変更単位にする。
review中のcandidateとCargo targetは維持し、採用/終了後の出力整理はstorage workflowに従う。

## 11. 受入範囲と未検証範囲

R01〜R14の必須実装・回帰・必要なnative受入は完了した。
最終suite-iは1920×1080／UI scale 1、X11 under Waylandのportal実入力で一覧13件と進捗バー5件を確認した。
全OS・GPU・DPI、IME composition、6 viewport入力、全UIシナリオ、性能改善率は今回の受入範囲外。
これらを今回の完了結果へ含めず、新たに必要となった場合は対象を限定して検証する。

## 12. AI引継ぎメモ

R01〜R14の実装、専用回帰、仕様とHelpの同期、必要なnative受入と終了整理が完了し、全14計画をarchiveした。受入時の基準HEADは`52913b61ca524f1cd429d3d73a0468faf3d0999b`で、その時点の未commit差分を含むsourceを検証した。以下のhashはその受入subjectの履歴である。

最新sourceは`dd7f0245ee3ea5040a7b726d298d0311d5704b97504ed7e16e930ebf657620c1`、harnessは`0ce94e6ad10aeec5056d3581f4e5fffb8c33597a3198ffeaa606d0362a1d53da`。`dev.py check`／`verify`、通常・profilingのworkspace all-targets Clippy警告0、入力関連45件とRtT関連8件、Skill全自己テストが成功。`construction_shells.rs`の最新rust-analyzer個別診断はerror／warning 0。一括診断はMCPのnull応答のため、workspace compile／Clippyで補完した。

最終batch `refactor-suite-portal-20260919-i`はRAM 11.21 GiBで開始し、2026-09-19 12:16 UTCに全18 checkpointが成功した。独立verifierもvalid、全18画像を主担当と読み取り専用reviewerが確認した。両game logはWARN／ERROR 0、Intel Arc Graphics (MTL)／Vulkan／Mesa 26.1.8を各1件記録。binary SHA-256は`83c1a2813070beaa0f127e38752718bd021137b7112b444ce0b6bb859ad17aed`。元依存がある間にpass sealし、jobの削除／finalize／storage checkを完了した。

追加で実測・修正した問題は、折りたたみマーカー2種類の削除検知漏れ、長いASCII Soul名と疲労値の重なり、Load時の床・壁site Visibility欠落。削除reader追加、名前TextLayoutの文字単位折返し、欠けたVisibilityだけの補完で直した。既存Hiddenを保持し、mirror／Nameが既存でも補完する。最後の修正は6件のconstruction回帰でRED→GREENと実Visual生成まで確認した。

検証helperはrows→barsを一つのportal sessionで実行する。key-upを描画ACK待ち前に送って低FPSでのautorepeatを防ぐ。bar位置はparentの完全な行列で独立検証し、fixtureのTreeVariantと壁tileの実際の占有形態を保存validatorへ合わせた。validator・許可・owner／focus／nonce・WARN／ERRORの拒否条件は維持する。suite-fで改名／検索／再展開、通常simulationによる疲労25→26%と同じrow、4 callerの中間bar、Save/Load・完了・取消を確認した。suite-fはinvalidの履歴として維持し、最終合格は修正後の新しいsuite-iに基づく。

許可画面は主担当がAT-SPI上のportal PID、RemoteDesktopDialog、実際のGtkSwitchと共有buttonを照合して操作した。suite-d/e/fでdevices=3と後続入力を確認済み。e/fは設定変更なし。dで一時変更したanimation設定は元の値へ復元した。suite-iでは主担当の操作スクリプト実行前に許可応答と入力が進み、同じportal session・devices=3を両gameで確認した。この回を主担当のbutton操作証拠とは扱わない。許可は非永続mode=0でsuite終了時に閉じる。過去のtimeoutは未表示・拒否の証拠に読み替えない。ユーザーへ反復する手動操作を要求しない。

先行nativeはIntel Arc Graphics (MTL)／Vulkan／X11／Mesa 26.1.8でTask Dashboardのaudit・Capture・Memory、地形3 LODと診断prepass・リサイズ、最終P08 closureとScene extentを確認済み。詳細は[描画の受入結果](../rendering-performance.md)と[性能計測の受入結果](../performance-profiling.md)。今回のUI変更を過去のP08候補へ反映済みとは扱わない。

Helpは実際の表示経路から全体を**Update required**と判断し、Move失敗時の保持、タスク表示、建設工程の3 entryと承認snapshotを更新・レビューした。追加の表示復元修正は既存操作・意味・文言を維持する。fixture／入力／行列verifierは開発用経路だけで、追加Help影響はNo impact。根拠は[Help仕様](../help-screen.md)へ記録した。

最終結果は[一覧UI](../entity_list_ui.md)と[建設表示](../building.md)へ反映し、両索引・Help・storage・diff gateを確認した。今後の要求や回帰にはprimaryの現行仕様を使い、影響する受入だけ再実行する。コード編集は主担当、subagentは読み取り専用に限る。

### 検証データの最終状態

終了した本作業の全jobは結果をsealし、原本を削除してfinalizeした。削除したjobのallocated bytes合計は**5,297,901,568 → 0**。各削除直後の`df`空き増加は0 bytesで、共有extentを含むため`du`減少量を空き増加量とは扱わない。exact pathと各結果は個別計画・primary台帳に記録済み。診断用の一時スクリプトも撤去した。portal bは`target/native-acceptance/ui-usability-feedback-20260919T020651Z-1cfbd858`（4,042,752→0）、cは同prefix `20260919T032042Z-c7831a11`（30,412,800→0）、dは`20260919T032635Z-aa28e256`（4,042,752→0 bytes）。各失敗理由をsealして整理済み。以下のexact jobはa〜hがinvalid、iがpassとしてsealし、削除／finalize済み（各df空き差0）。g/hはjob未作成・0 bytesで終了した。

| suite | `target/native-acceptance/`以下の削除path | allocated bytes → 0 | 結果 |
| --- | --- | ---: | --- |
| a | `ui-usability-feedback-20260919T095236Z-64b700d8` | 49,074,176 | 11 row成功、再展開の削除検知漏れ |
| b | `ui-usability-feedback-20260919T100629Z-67fdaf60` | 4,046,848 | 許可前のfocus条件不成立、入力なし |
| c | `ui-usability-feedback-20260919T101018Z-cf1f5b0d` | 57,131,008 | 13 row成功、bar位置verifierの親scale欠落 |
| d | `ui-usability-feedback-20260919T101831Z-5f216a58` | 65,347,584 | 13 row・3 bar成功、fixtureのTreeVariant欠落でLoad拒否 |
| e | `ui-usability-feedback-20260919T103041Z-97b23241` | 41,545,728 | 9 row成功、遅いACK待ちで検索key autorepeat |
| f | `ui-usability-feedback-20260919T110517Z-dbdb0c28` | 69,304,320 | 全18件成立、Load後のInheritedVisibility欠落警告 |
| g | `ui-usability-feedback-20260919T114440Z-3a597468` | 0 | 起動前終了、native未実施 |
| h | `ui-usability-feedback-20260919T120113Z-37a807c5` | 0 | 起動前終了、native未実施 |
| i | `ui-usability-feedback-20260919T121448Z-3cc51761` | 69,300,224 | 全18件・独立verify・全画像・無警告成功 |

| 保持対象 | 現在の用途 |
| --- | --- |
| exact path | `/home/satotakumi/projects/hell-workers-validation/refactor-r01-r14-92a7d87235e6` |
| subject | clean HEAD `a0487a35c09ed444dc588902ed198c9532f4c685`。最終P08受入候補 |
| owner / consumer | `refactor-r01-r14` / `refactor-r01-r14-review` |
| allocated bytes | 40,952,061,952（約38.14 GiB、全job整理後） |
| 次の作業 | primaryの現行R01〜R14実装へのレビューを継続する。P08へ影響する要求には同じcandidate／Cargo targetを再利用する。最新UI修正とsuite-iはprimaryが正本であり、保持中の旧P08候補へ反映済みとは扱わない |
| release_when | 実装レビューと要求された修正が最終受入または明示終了し、このcandidateの利用者がなくなった時 |

この保持は最終候補と差分build cacheに限り、終了済みjobの保持を含まない。無応答や途中のpassでreview-activeを解除しない。primaryの通常Cargo cacheと他ownerの資産は別管理のまま維持する。報告前のprimary storage checkは成功した。

参照必須: [開発ガイド](../DEVELOPMENT.md)、[crate境界](../crate-boundaries.md)、
[不変条件](../invariants.md)、[検証データ管理](../development-infra/validation-storage-workflow.md)、各対象仕様。

- [x] 根拠・優先順位・維持する設計を記載
- [x] リスク・影響・検証範囲を記載
- [x] 実装への引継ぎとplan作成条件を記載

## 13. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-09-17 | Codex | 全13 crateと関連基盤を横断し、14件のリファクタ候補・根拠・段階導入・検証結果を記録 |
| 2026-09-18 | Codex | R01〜R14それぞれの独立計画を作成し、依存・編集競合・着手順を接続。実装は未着手 |
| 2026-09-18 | Codex | ユーザー依頼で全14計画を再レビュー。9件の設計/受入条件を修正し、5件は追加修正不要と判断。§4.2へ結果を集約 |
| 2026-09-18 | Codex | 全14計画を具体化。API案、処理順、拒否時policy、変更単位、fixture/期待値と最初の着手手順を追加。§4.3へ要点を集約 |
| 2026-09-19 | Codex | 全14件を実装し、Help・仕様を同期。全体gateと必要なGPU受入を通過して11件をarchive。R04/R08/R12は実入力許可への対応待ち。終了jobを整理し、最終候補とcacheをレビュー用に保持 |
| 2026-09-19 | Codex | 対応可能との回答を受けportal入力受入を再開したが、Startの許可応答がtimeout。入力送信なし・job整理済み。ダイアログ表示・操作状況の確認を次の作業へ更新 |
| 2026-09-19 | Codex | ダイアログ未表示を確認。sessionと公式APIを照合し、親の前面化を先行する切り分けを準備。38回帰成功。nativeはRAM開始条件未達で起動前停止、原因・実機効果は未確定 |
| 2026-09-19 | Codex | ユーザーは直近dの画面を未確認で、反復するOS許可操作の負担を指摘。同一条件の再試行・表示確認依頼を止め、入力許可を毎回取り直す実行方法を見直す状態へ更新 |
| 2026-09-19 | Codex | ユーザーが手動操作必須の仕様指定を否定。Skillの一律手動ルールを4配布先で訂正し、現行実装の非永続設定と区別。該当文言は開始時HEADにも存在し、今回追加したとの説明も訂正。コード・OS設定・許可状態は変更していない |
| 2026-09-19 | Codex | 主担当の許可UI操作とsuite-f全18 checkpointが成立。再展開・長い名前・Load後site Visibilityを修正し、最新verify／check／通常・profiling Clippy／個別RAが成功。修正後の再nativeはRAM開始条件未達として未完を維持。終了jobを整理し、文書・Help・storage gateを再確認 |
| 2026-09-19 | Codex | RAM開始条件を満たしてsuite-iを実行。全18件・独立verify・全画像・無警告が成功し、R04/R08/R12を完了archive。全14計画の必須受入を完了、job整理／storage check成功 |
