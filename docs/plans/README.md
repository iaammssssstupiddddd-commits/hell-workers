# Plans Index

`docs/plans` の文書ステータス一覧（更新日: 2026-03-22）。

## 新規計画書の作り方

1. テンプレートをコピーする。  
   `cp docs/plans/plan-template.md docs/plans/<topic>-plan-YYYY-MM-DD.md`
2. `メタ情報`、`目的`、`マイルストーン`、`AI引継ぎメモ` を最低限埋める。
3. 進捗に応じて `ステータス` と `更新履歴` を更新する。

## テンプレート

| Document | Status | Notes |
|---|---|---|
| [plan-template.md](plan-template.md) | テンプレート | AIが引継ぎしやすい実装計画テンプレート。 |

## 現行計画書

| Document | Status | Notes |
|---|---|---|
| [visual-sync-source-selector-haul-refactor-2026-03-22.md](visual-sync-source-selector-haul-refactor-2026-03-22.md) | Completed | Observer/System 分離・計測コード分離・運搬 builder 分割の 3 フェーズリファクタリング。 |

## アーカイブ計画書一覧 (`docs/plans/archive`)

| Document | Status | Notes |
|---|---|---|
| [archive/ai-phase-refactor-implementation-plan.md](archive/ai-phase-refactor-implementation-plan.md) | アーカイブ | AIフェーズリファクタ実装計画。 |
| [archive/ai-phase-refactor.md](archive/ai-phase-refactor.md) | アーカイブ | AIフェーズリファクタの全体設計メモ。 |
| [archive/architecture-safe-crate-extraction-plan-2026-03-12.md](archive/architecture-safe-crate-extraction-plan-2026-03-12.md) | アーカイブ | アーキテクチャ維持前提の追加クレート化計画 |
| [archive/area-selection-input-refactor-plan-2026-03-19.md](archive/area-selection-input-refactor-plan-2026-03-19.md) | アーカイブ | area_selectionの計画。 |
| [archive/assigned-task-to-hw-jobs-plan-2026-03-08.md](archive/assigned-task-to-hw-jobs-plan-2026-03-08.md) | アーカイブ | AssignedTask を hw_core → hw_jobs へ移動する計画 |
| [archive/assignment-builder-unification-plan-2026-03-01.md](archive/assignment-builder-unification-plan-2026-03-01.md) | アーカイブ | `task_management/builders` の重複削減と割り当て生成経路の共通化計画。 |
| [archive/auto-gather-for-blueprint.md](archive/auto-gather-for-blueprint.md) | アーカイブ | Blueprint不足資材の自動伐採/採掘計画。 |
| [archive/bridge-building.md](archive/bridge-building.md) | アーカイブ | 橋（Bridge）建築物の実装計画。 |
| [archive/bucket-return-rebuild-plan.md](archive/bucket-return-rebuild-plan.md) | アーカイブ | バケツ返却仕様の再構築計画。 |
| [archive/cargo-workspace-migration-plan.md](archive/cargo-workspace-migration-plan.md) | アーカイブ | Cargo Workspace 移行計画 |
| [archive/codebase-quality-refactor.md](archive/codebase-quality-refactor.md) | アーカイブ | リファクタリング計画: コードベース全体の整理・品質向上 |
| [archive/command-crate-extraction-plan-2026-03-12.md](archive/command-crate-extraction-plan-2026-03-12.md) | アーカイブ | src/systems/command/の計画。 |
| [archive/crate-boundary-alignment-plan-2026-03-18.md](archive/crate-boundary-alignment-plan-2026-03-18.md) | アーカイブ | docs/crate-boundaries.mdの計画。 |
| [archive/debug-instant-build-plan.md](archive/debug-instant-build-plan.md) | アーカイブ | Debug Instant Build ボタン 実装計画 |
| [archive/destination-validation-unification-plan-2026-03-07.md](archive/destination-validation-unification-plan-2026-03-07.md) | アーカイブ | FloorConstruction / WallConstruction / ProvisionalWall の搬入先需要計算と実行時受入判定が、割り当て時・手運搬 dropping 時・猫車 unloading 時の 3 系統に分散し、同一ロジックを複数箇所で維持しているの計画。 |
| [archive/docs-index-automation-plan-2026-03-05.md](archive/docs-index-automation-plan-2026-03-05.md) | アーカイブ | docs/plans/README.mdの計画。 |
| [archive/door-implementation-2026-02-22.md](archive/door-implementation-2026-02-22.md) | アーカイブ | 壁で囲まれた空間への出入りを制御する手段がない。現状は壁に穴を開けるか、壁を完全に閉じるかの二択しかないの計画。 |
| [archive/dream-bubble-physics.md](archive/dream-bubble-physics.md) | アーカイブ | Dreamバブル物理挙動の実装計画。 |
| [archive/dream-per-soul-storage-plan-2026-02-26.md](archive/dream-per-soul-storage-plan-2026-02-26.md) | アーカイブ | dreamがグローバルプール直接加算でsoul個別の管理軸がないの計画。 |
| [archive/dream-system.md](archive/dream-system.md) | アーカイブ | Dreamシステム提案。 |
| [archive/dream-ui-display-plan-2026-02-26.md](archive/dream-ui-display-plan-2026-02-26.md) | アーカイブ | Dream UI 表示 — 実装計画書 |
| [archive/dream-ui-particle-update-refactor-plan-2026-03-05.md](archive/dream-ui-particle-update-refactor-plan-2026-03-05.md) | アーカイブ | visual/dream/ui_particle/update.rsの計画。 |
| [archive/dream-visual-update.md](archive/dream-visual-update.md) | アーカイブ | Dreamビジュアル更新計画。 |
| [archive/dream_per_soul_storage.md](archive/dream_per_soul_storage.md) | アーカイブ | 現状: Dreamはグローバルプール(`DreamPool.points`)に直接加算される。soulの睡眠レートで`DreamPool`に即時反映され、soul個別のdream貯蔵量という概念がないの提案。 |
| [archive/familiar-ai-root-slim-plan-2026-03-13.md](archive/familiar-ai-root-slim-plan-2026-03-13.md) | アーカイブ | bevy_app/familiar_ai ルート薄型化計画（adapter 維持版） |
| [archive/familiar-ai-root-thinning-plan-2026-03-09.md](archive/familiar-ai-root-thinning-plan-2026-03-09.md) | アーカイブ | familiar_ai を hw_ai へ寄せて root を薄くする計画 |
| [archive/familiar-state-decision-adapter-split-plan-2026-03-12.md](archive/familiar-state-decision-adapter-split-plan-2026-03-12.md) | アーカイブ | src/systems/familiar_ai/decide/state_decision.rsの計画。 |
| [archive/familiar-task-management-hw-ai-extraction-plan-2026-03-11.md](archive/familiar-task-management-hw-ai-extraction-plan-2026-03-11.md) | アーカイブ | Familiar Task Management `hw_ai` 抽出 実装計画 |
| [archive/familiar-ui-hw-ui-migration-plan-2026-03-11.md](archive/familiar-ui-hw-ui-migration-plan-2026-03-11.md) | アーカイブ | -の計画。 |
| [archive/floor-construction.md](archive/floor-construction.md) | アーカイブ | 床建築システムの実装計画。 |
| [archive/global-transport-request-plan.md](archive/global-transport-request-plan.md) | アーカイブ | 運搬系のグローバル request 化計画。 |
| [archive/hover-action-button.md](archive/hover-action-button.md) | アーカイブ | Plant ホバーアクションボタン（プレースホルダー）実装計画 |
| [archive/hw-ai-boundary-cleanup-plan-2026-03-12.md](archive/hw-ai-boundary-cleanup-plan-2026-03-12.md) | アーカイブ | -の計画。 |
| [archive/hw-ai-crate-phase2-2026-03-08 copy 1.md](archive/hw-ai-crate-phase2-2026-03-08 copy 1.md) | アーカイブ | Phase 1 時点ではの計画。 |
| [archive/hw-ai-crate-phase2-2026-03-08 copy 2.md](archive/hw-ai-crate-phase2-2026-03-08 copy 2.md) | アーカイブ | Phase 1 時点ではの計画。 |
| [archive/hw-ai-crate-phase2-2026-03-08 copy.md](archive/hw-ai-crate-phase2-2026-03-08 copy.md) | アーカイブ | Phase 1 時点ではの計画。 |
| [archive/hw-ai-crate-phase2-2026-03-08.md](archive/hw-ai-crate-phase2-2026-03-08.md) | アーカイブ | Phase 1 時点ではの計画。 |
| [archive/hw-ai-crate-plan-2026-03-08 copy 1.md](archive/hw-ai-crate-plan-2026-03-08 copy 1.md) | アーカイブ | src/systems/soul_ai/の計画。 |
| [archive/hw-ai-crate-plan-2026-03-08 copy 2.md](archive/hw-ai-crate-plan-2026-03-08 copy 2.md) | アーカイブ | src/systems/soul_ai/の計画。 |
| [archive/hw-ai-crate-plan-2026-03-08 copy.md](archive/hw-ai-crate-plan-2026-03-08 copy.md) | アーカイブ | src/systems/soul_ai/の計画。 |
| [archive/hw-ai-crate-plan-2026-03-08.md](archive/hw-ai-crate-plan-2026-03-08.md) | アーカイブ | src/systems/soul_ai/の計画。 |
| [archive/hw-spatial-crate copy 1.md](archive/hw-spatial-crate copy 1.md) | アーカイブ | WorldMapの計画。 |
| [archive/hw-spatial-crate copy.md](archive/hw-spatial-crate copy.md) | アーカイブ | WorldMapの計画。 |
| [archive/hw-spatial-crate.md](archive/hw-spatial-crate.md) | アーカイブ | WorldMapの計画。 |
| [archive/hw-ui-crate-extraction copy 1.md](archive/hw-ui-crate-extraction copy 1.md) | アーカイブ | src/interface/ui/の計画。 |
| [archive/hw-ui-crate-extraction copy 2.md](archive/hw-ui-crate-extraction copy 2.md) | アーカイブ | src/interface/ui/の計画。 |
| [archive/hw-ui-crate-extraction copy.md](archive/hw-ui-crate-extraction copy.md) | アーカイブ | src/interface/ui/の計画。 |
| [archive/hw-ui-crate-extraction.md](archive/hw-ui-crate-extraction.md) | アーカイブ | src/interface/ui/の計画。 |
| [archive/hw-ui-crate-plan-2026-03-08.md](archive/hw-ui-crate-plan-2026-03-08.md) | アーカイブ | hw_ui crate 分離 実装計画 |
| [archive/hw-ui-review-fixes-plan-2026-03-08.md](archive/hw-ui-review-fixes-plan-2026-03-08.md) | アーカイブ | hw_uiの計画。 |
| [archive/hw-visual-crate-extraction.md](archive/hw-visual-crate-extraction.md) | アーカイブ | hw_visual クレート化 実装計画 |
| [archive/hw-visual-domain-decoupling.md](archive/hw-visual-domain-decoupling.md) | アーカイブ | hw_visual ドメイン分離：ミラーコンポーネント実装計画 |
| [archive/initial-resource-bootstrap-split-plan-2026-03-12.md](archive/initial-resource-bootstrap-split-plan-2026-03-12.md) | アーカイブ | src/systems/logistics/initial_spawn.rsの計画。 |
| [archive/large-files-refactor-2026-02-16.md](archive/large-files-refactor-2026-02-16.md) | アーカイブ | 大規模ファイル分割リファクタ計画。 |
| [archive/logistics-to-hw-logistics-plan-2026-03-08.md](archive/logistics-to-hw-logistics-plan-2026-03-08.md) | アーカイブ | logistics 実行ロジックを hw_logistics へ移植する計画。M1〜M8 完了。 |
| [archive/mixer-producer-phase-separation-plan-2026-03-05.md](archive/mixer-producer-phase-separation-plan-2026-03-05.md) | アーカイブ | producer/mixer.rsの計画。 |
| [archive/move-plant-building.md](archive/move-plant-building.md) | アーカイブ | Plant 建物移動タスク 実装計画（詳細版） |
| [archive/multi-tool-ai-rules-plan.md](archive/multi-tool-ai-rules-plan.md) | アーカイブ | マルチツール AI ルール体系の構築 |
| [archive/participating-in-relationship.md](archive/participating-in-relationship.md) | アーカイブ | Relationship参加設計に関する計画。 |
| [archive/pathfinding-core-unification-plan-2026-03-01.md](archive/pathfinding-core-unification-plan-2026-03-01.md) | アーカイブ | find_pathの計画。 |
| [archive/pathfinding-executor-split-plan-2026-03-05.md](archive/pathfinding-executor-split-plan-2026-03-05.md) | アーカイブ | entities/damned_soul/movement/pathfinding.rsの計画。 |
| [archive/perf-phase1-quick-wins-2026-02-26.md](archive/perf-phase1-quick-wins-2026-02-26.md) | アーカイブ | パフォーマンス改善 Phase 1: idle HashMap→Local、get_nearby_into API、5グリッド Change Detection 化。 |
| [archive/perf-phase2-spatial-grid-change-detection-2026-02-26.md](archive/perf-phase2-spatial-grid-change-detection-2026-02-26.md) | アーカイブ | パフォーマンス改善 Phase 2: Designation/TransportRequest Change Detection 化 + sync 基盤削除。 |
| [archive/perf-phase3-room-detection-and-ui-2026-02-26.md](archive/perf-phase3-room-detection-and-ui-2026-02-26.md) | アーカイブ | パフォーマンス改善 Phase 3: Room Detection HashMap clone 削除 + UI ViewModel dirty ゲート化。 |
| [archive/perf-phase4-reachability-cache-lifetime-2026-02-26.md](archive/perf-phase4-reachability-cache-lifetime-2026-02-26.md) | アーカイブ | パフォーマンス改善 Phase 4: Reachability キャッシュを WorldMap 変更時のみクリア。 |
| [archive/perf-review-followups-plan-2026-03-06.md](archive/perf-review-followups-plan-2026-03-06.md) | アーカイブ | スケール時に効きやすい全件走査・全UI再構築・線形の計画。 |
| [archive/perf-top3-implementation-plan-2026-02-22.md](archive/perf-top3-implementation-plan-2026-02-22.md) | アーカイブ | 直近Perf上位3件の最適化計画。 |
| [archive/phase1-file-split-detail.md](archive/phase1-file-split-detail.md) | アーカイブ | Phase 1 詳細実装計画: ファイル分割・構造整理 |
| [archive/phase12-leftover-migration.md](archive/phase12-leftover-migration.md) | アーカイブ | フェーズ 3 調査の過程で、`GameAssets` 等の Root 固有型に依存せず、の提案。 |
| [archive/phase2-facade-cleanup-detail.md](archive/phase2-facade-cleanup-detail.md) | アーカイブ | Phase 2 詳細実装計画: bevy_app ファサード整理 |
| [archive/phase3-gameassets-abstraction.md](archive/phase3-gameassets-abstraction.md) | アーカイブ | `bevy_app` に残存するシステムのうち、`GameAssets`（Root 固有リソース）の **一部フィールド** のみに依存するものをの提案。 |
| [archive/phase3-naming-gridpos-detail.md](archive/phase3-naming-gridpos-detail.md) | アーカイブ | Phase 1・2 で構造整理・ファサード削除が完了したの提案。 |
| [archive/phase4-bevy-app-slim-detail.md](archive/phase4-bevy-app-slim-detail.md) | アーカイブ | Phase 4: `bevy_app` スリム化 — 詳細実装計画 |
| [archive/plant-trees-visuals-plan-2026-02-22.md](archive/plant-trees-visuals-plan-2026-02-22.md) | アーカイブ | `Plant Trees` の3フェーズ演出およびドラッグ時プレビュー追加の実装計画。 |
| [archive/re-export-consolidation-plan-2026-03-12.md](archive/re-export-consolidation-plan-2026-03-12.md) | アーカイブ | `pub use` の多段中継と wildcard 再公開を削減し、正規 public path を整理する計画。 |
| [archive/reexport-reduction-plan-2026-03-19.md](archive/reexport-reduction-plan-2026-03-19.md) | アーカイブ | bevy_appの計画。 |
| [archive/refactor-500plus-files-phase-plan-2026-02-14.md](archive/refactor-500plus-files-phase-plan-2026-02-14.md) | アーカイブ | 500行超ファイルの段階的リファクタ計画。 |
| [archive/refactor-500plus-rs-files-plan-2026-03-18.md](archive/refactor-500plus-rs-files-plan-2026-03-18.md) | アーカイブ | 500行超 Rust ソースファイルの責務分割計画 |
| [archive/refactor-implementation-order-2026-02-20.md](archive/refactor-implementation-order-2026-02-20.md) | アーカイブ | リファクタ実装順のガイド。 |
| [archive/refactor-phase-plan-2026-02.md](archive/refactor-phase-plan-2026-02.md) | アーカイブ | フェーズ分割リファクタ計画。 |
| [archive/refactor-roadmap-2026-02-22.md](archive/refactor-roadmap-2026-02-22.md) | アーカイブ | 現行実装を前提にした全体リファクタ実行計画（回帰テスト追加はスコープ外）。 |
| [archive/refactor-top5-followups-plan-2026-03-02.md](archive/refactor-top5-followups-plan-2026-03-02.md) | アーカイブ | 直近レビューで抽出した5件（運搬request重複、UI粒子更新肥大、assignment builder重複、pathfinding隠れ状態、冗長分岐）を段階的に解消するの計画。 |
| [archive/refactor-types-migration-plan copy.md](archive/refactor-types-migration-plan copy.md) | アーカイブ | 型・ドメインモデルのクレート境界リファクタリング計画 |
| [archive/refactor-types-migration-plan.md](archive/refactor-types-migration-plan.md) | アーカイブ | 型・ドメインモデルのクレート境界リファクタリング計画 |
| [archive/remove-instockpile-claimedby.md](archive/remove-instockpile-claimedby.md) | アーカイブ | `InStockpile`/`ClaimedBy` 削除統合計画。 |
| [archive/remove-reexport-indirections-plan.md](archive/remove-reexport-indirections-plan.md) | アーカイブ | bevy_appの計画。 |
| [archive/request-unification-plan-2026-02-14.md](archive/request-unification-plan-2026-02-14.md) | アーカイブ | Request方式一本化計画。 |
| [archive/rest-area-system.md](archive/rest-area-system.md) | アーカイブ | 休憩所（Rest Area）システム提案。 |
| [archive/room-detection-hw-world-extraction-plan-2026-03-11.md](archive/room-detection-hw-world-extraction-plan-2026-03-11.md) | アーカイブ | src/systems/room/detection.rsの計画。 |
| [archive/room-detection-plan-2026-02-23.md](archive/room-detection-plan-2026-02-23.md) | アーカイブ | 壁・扉・床の閉領域をRoomとして検出し、オーバーレイ表示する実装計画。 |
| [archive/scaling-performance-bottlenecks-plan.md](archive/scaling-performance-bottlenecks-plan.md) | アーカイブ | スケール時ボトルネック最適化計画。 |
| [archive/selection-placement-refactor-plan-2026-02-25.md](archive/selection-placement-refactor-plan-2026-02-25.md) | アーカイブ | `interface/selection` の配置処理を責務分離するリファクタ計画。 |
| [archive/selection-separation-plan-2026-03-08.md](archive/selection-separation-plan-2026-03-08.md) | アーカイブ | -の計画。 |
| [archive/site-yard-system.md](archive/site-yard-system.md) | アーカイブ | 実装計画: Site / Yard システム |
| [archive/soul-ai-crate-extraction-plan-2026-03-12.md](archive/soul-ai-crate-extraction-plan-2026-03-12.md) | アーカイブ | src/systems/soul_ai/の計画。 |
| [archive/soul-ai-root-thinning-plan-2026-03-09.md](archive/soul-ai-root-thinning-plan-2026-03-09.md) | アーカイブ | soul_ai を段階的に crate へ寄せて root を薄くする計画 |
| [archive/soul-ai-root-thinning-plan-2026-03-11 copy 1.md](archive/soul-ai-root-thinning-plan-2026-03-11 copy 1.md) | アーカイブ | src/systems/soul_aiの計画。 |
| [archive/soul-ai-root-thinning-plan-2026-03-11 copy 2.md](archive/soul-ai-root-thinning-plan-2026-03-11 copy 2.md) | アーカイブ | src/systems/soul_aiの計画。 |
| [archive/soul-ai-root-thinning-plan-2026-03-11 copy.md](archive/soul-ai-root-thinning-plan-2026-03-11 copy.md) | アーカイブ | src/systems/soul_aiの計画。 |
| [archive/soul-ai-root-thinning-plan-2026-03-11.md](archive/soul-ai-root-thinning-plan-2026-03-11.md) | アーカイブ | src/systems/soul_aiの計画。 |
| [archive/soul-spawn-despawn.md](archive/soul-spawn-despawn.md) | アーカイブ | Soul Spawn/Despawn設計計画。 |
| [archive/task-execution-hw-ai-extraction-plan-2026-03-12.md](archive/task-execution-hw-ai-extraction-plan-2026-03-12.md) | アーカイブ | src/systems/soul_ai/execute/task_execution/の計画。 |
| [archive/task-list-left-panel-toggle.md](archive/task-list-left-panel-toggle.md) | アーカイブ | タスクリスト左パネル操作改善計画。 |
| [archive/taskexecution-systemparam-refactor-plan-2026-03-05.md](archive/taskexecution-systemparam-refactor-plan-2026-03-05.md) | アーカイブ | task_execution/context.rsの計画。 |
| [archive/think-phase-iteration-optimization-plan-2026-03-07.md](archive/think-phase-iteration-optimization-plan-2026-03-07.md) | アーカイブ | -の計画。 |
| [archive/transport-overdelivery-fix-plan-2026-03-07.md](archive/transport-overdelivery-fix-plan-2026-03-07.md) | アーカイブ | 設計図搬入と補充系 request で、必要量を超える資材が搬送・消費・地面残留するの計画。 |
| [archive/ui-menu-action-boundary-plan-2026-03-01.md](archive/ui-menu-action-boundary-plan-2026-03-01.md) | アーカイブ | `MenuAction` 処理の責務境界整理と no-op 分岐解消の計画。 |
| [archive/ui-selection-boundary-thinning-plan-2026-03-12.md](archive/ui-selection-boundary-thinning-plan-2026-03-12.md) | アーカイブ | UI / Selection Boundary Thinning Plan |
| [archive/ui-submenu-spec-driven-plan-2026-03-01.md](archive/ui-submenu-spec-driven-plan-2026-03-01.md) | アーカイブ | サブメニュー生成を Spec 駆動へ移行し重複を削減する計画。 |
| [archive/wall-construction-phase-split-plan-2026-02-19.md](archive/wall-construction-phase-split-plan-2026-02-19.md) | アーカイブ | 壁建築フェーズ分割計画。 |
| [archive/wall-stasis-mud.md](archive/wall-stasis-mud.md) | アーカイブ | 壁材（stasis mud）関連計画。 |
| [archive/wheelbarrow-arbitration-plan.md](archive/wheelbarrow-arbitration-plan.md) | アーカイブ | 猫車利用仲裁ロジックの実装計画。 |
| [archive/workspace-area-bounds-extraction.md](archive/workspace-area-bounds-extraction.md) | アーカイブ | AreaBoundsの計画。 |
| [archive/workspace-construction-phase-extraction.md](archive/workspace-construction-phase-extraction.md) | アーカイブ | Floor/Wall 建設で使うフェーズ・状態型がの計画。 |
| [archive/zone-placement-refactor-plan-2026-03-05.md](archive/zone-placement-refactor-plan-2026-03-05.md) | アーカイブ | zone_placement.rsの計画。 |
| [archive/zone-removal-preview-diff-plan-2026-03-01.md](archive/zone-removal-preview-diff-plan-2026-03-01.md) | アーカイブ | Zone removal preview の全件更新を差分更新へ置換する計画。 |

