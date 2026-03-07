# Proposals Index

`docs/proposals` は、機能追加・改善・リファクタリング案をまとめる提案書ディレクトリです。

## 新規提案書の作り方

1. テンプレートをコピーする。  
   `cp docs/proposals/proposal-template.md docs/proposals/<topic>-proposal-YYYY-MM-DD.md`
2. `メタ情報`、`背景と問題`、`提案内容`、`AI引継ぎメモ` を最低限埋める。
3. 進捗に応じて `ステータス` と `更新履歴` を更新する。

## テンプレート

- [proposal-template.md](proposal-template.md): AIが引継ぎしやすい提案書テンプレート。

## 現在の提案書

| Document | Notes |
| --- | --- |
| [08_visual_update_prompts.md](08_visual_update_prompts.md) | ビジュアル更新プロンプト集 |
| [destination-validation-unification-proposal-2026-03-07.md](destination-validation-unification-proposal-2026-03-07.md) | 搬入先バリデーション（Floor/Wall/ProvisionalWall）の一元化。3箇所の重複 ~250行を logistics/ 共通モジュールに集約 |
| [soul_spawn_despawn_optimization.md](soul_spawn_despawn_optimization.md) | Soul Spawn/Despawn 最適化提案 |
| [speech_optimization.md](speech_optimization.md) | スピーチシステム最適化提案 |
| [think-phase-iteration-optimization-proposal-2026-03-07.md](think-phase-iteration-optimization-proposal-2026-03-07.md) | Think フェーズのタイル O(n) スキャン + IncomingDeliveries O(m) ルックアップの最適化 |
| [water-transport-consolidation-proposal-2026-03-07.md](water-transport-consolidation-proposal-2026-03-07.md) | GatherWater / HaulWaterToMixer の統合（20ファイル → 10ファイル、~1000行削減） |

## アーカイブ提案書一覧 (`docs/proposals/archive`)

| Document | Notes |
| --- | --- |
| [archive/01-event-driven-ui.md](archive/01-event-driven-ui.md) | 提案01: ポーリング廃止 — イベント駆動UIアーキテクチャへの全面移行 |
| [archive/05-unified-interaction-layer.md](archive/05-unified-interaction-layer.md) | 提案05: インタラクション層の統一 — 全UI操作を単一の入力パイプラインに集約 |
| [archive/09-large-file-refactor.md](archive/09-large-file-refactor.md) | 提案09: 500行超ファイルの段階的リファクタリング計画 |
| [archive/ai-scalability-optimization.md](archive/ai-scalability-optimization.md) | AIシステム スケーラビリティ最適化提案 (Scale: Familiar 30, Soul 500) |
| [archive/bevy_018_features.md](archive/bevy_018_features.md) | Bevy 0.18 新機能導入提案 |
| [archive/dream_general_visuals.md](archive/dream_general_visuals.md) | Dreamシステム全体 ビジュアルアップデート提案 |
| [archive/dream_tree_planting_proposal.md](archive/dream_tree_planting_proposal.md) | Dream を使った植林システム提案 |
| [archive/high_priority_performance_plan.md](archive/high_priority_performance_plan.md) | `try_assign_for_workers` でワーカーごとに候補収集と評価を実行しているの提案。 |
| [archive/pathfinding-optimization.md](archive/pathfinding-optimization.md) | 経路探索システムの最適化提案 |
| [archive/performance-bottlenecks-proposal-2026-02-26.md](archive/performance-bottlenecks-proposal-2026-02-26.md) | **現状**: Soul 数が増加するにつれてフレームレートが低下する傾向がある。Space/Spatial グリッドの同期、Room 検出、Soul AI の決定処理など複数の領域で毎フレーム・定期的な全件処理が行われているの提案。 |
| [archive/plant_trees_visuals.md](archive/plant_trees_visuals.md) | Plant Trees機能 個別ビジュアルアップデート提案 |
| [archive/recruit-and-task-assignment-algorithm.md](archive/recruit-and-task-assignment-algorithm.md) | リクルート及びタスクアサインの選定アルゴリズム改善提案 |
| [archive/room_detection.md](archive/room_detection.md) | 現状: 壁・ドア・床は個別のエンティティとして管理されており、囲まれた空間を論理的に認識する仕組みがないの提案。 |
| [archive/scaling_performance_bottlenecks.md](archive/scaling_performance_bottlenecks.md) | スケール時パフォーマンス・ボトルネック再評価（2026-02-17 更新） |
| [archive/site-yard-system.md](archive/site-yard-system.md) | **現状**: Familiar ごとに 1 つの `TaskArea`（矩形）が全活動範囲を担っている。建築現場・設備・Stockpile すべてが同じ TaskArea 内に配置され、1 Familiar = 1 エリアの 1:1 対応の提案。 |
| [archive/task_delegation_implementation_plan.md](archive/task_delegation_implementation_plan.md) | タスク移譲最適化 実装計画 |
| [archive/transport-task-refactor.md](archive/transport-task-refactor.md) | 提案10: 運搬タスクの責務分離と予約処理の統合リファクタ（Phase 2） |
| [archive/ui-visual-redesign.md](archive/ui-visual-redesign.md) | UI ビジュアル再設計 & 操作感改善ドキュメント |
| [archive/wheelbarraw_sand_stasis_mud_implementation_plan.md](archive/wheelbarraw_sand_stasis_mud_implementation_plan.md) | 実装計画書: Sand / StasisMud の猫車専用運搬化 |
| [archive/wheelbarrow_only_for_sand_and_stasis_mud.md](archive/wheelbarrow_only_for_sand_and_stasis_mud.md) | 提案: Sand / StasisMud の猫車専用運搬化 |

