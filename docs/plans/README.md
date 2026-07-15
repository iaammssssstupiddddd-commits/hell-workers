# Plans Index

`docs/plans` の文書ステータス一覧（更新日: 2026-07-15）。

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
| [3d-rtt/asset-milestones-2026-03-17.md](3d-rtt/asset-milestones-2026-03-17.md) | 進行中（Soul・terrain・shader 完了、建築 GLB pipeline 未着手） | アセット作成マイルストーン |
| [3d-rtt/lighting-visual-plan-2026-04-04.md](3d-rtt/lighting-visual-plan-2026-04-04.md) | Draft | Outdoor Lamp のローカル照明で Soul / 建物に落ちる影を追加する計画 |
| [3d-rtt/milestone-roadmap.md](3d-rtt/milestone-roadmap.md) | Phase 3 進行中（未完: MS-3-5 / 7 / 8 / 9、受入残件: MS-3-6 / 10） | 3D-RtT 移行ロードマップ |
| [3d-rtt/terrain-lod-switch-flicker-plan-2026-04-17.md](3d-rtt/terrain-lod-switch-flicker-plan-2026-04-17.md) | Draft | 地形 LOD の単発切替ポップを観測し、短い dither 遷移で抑える計画 |
| [hvac-plumbing-plan-2026-07-13.md](hvac-plumbing-plan-2026-07-13.md) | Draft | 地獄のインフラ（換気・導水・部屋認可）実装計画 |
| [performance-cpu-2026-04-16.md](performance-cpu-2026-04-16.md) | Superseded | CPU パフォーマンス改善計画書 |
| [system-wide-correctness-refactoring-plan-2026-07-12.md](system-wide-correctness-refactoring-plan-2026-07-12.md) | In Progress | 全体実装レビュー追補: 横断リファクタリングロードマップ |
| [system-wide-runtime-performance-plan-2026-07-12.md](system-wide-runtime-performance-plan-2026-07-12.md) | In Progress | 全体ランタイム・ホットパス性能改善計画書 |

## アーカイブ計画書一覧 (`archive/` / `**/archived/`)

| Document | Status | Notes |
|---|---|---|
| [3d-rtt/archived/blob-shadow-tim-burton-2026-04-12.md](3d-rtt/archived/blob-shadow-tim-burton-2026-04-12.md) | Archived | 影スタイル 2D 化計画（床・壁接続維持） 2026-04-12 |
| [3d-rtt/archived/blueprint-terrain-surface-material.md](3d-rtt/archived/blueprint-terrain-surface-material.md) | Archived | TerrainSurfaceMaterial 統合（MS-3-6 Phase 3 ブループリント） |
| [3d-rtt/archived/building-visual-layer-implementation-plan-2026-03-15.md](3d-rtt/archived/building-visual-layer-implementation-plan-2026-03-15.md) | Archived | - 建築物が1エンティティ=1スプライトに固定されており、床・壁・配線などの重層的な表現ができないの計画。 |
| [3d-rtt/archived/ms-3-2-implementation-plan-2026-03-29.md](3d-rtt/archived/ms-3-2-implementation-plan-2026-03-29.md) | Archived | MS-3-2 実装計画 |
| [3d-rtt/archived/ms-3-4-terrain-3d-plan-2026-03-29.md](3d-rtt/archived/ms-3-4-terrain-3d-plan-2026-03-29.md) | Archived | MS-3-4 テレイン 3D 化 実装計画 |
| [3d-rtt/archived/ms-3-5-building-section-material-plan-2026-03-31.md](3d-rtt/archived/ms-3-5-building-section-material-plan-2026-03-31.md) | Archived | MS-3-5 Building3dHandles の SectionMaterial 移行（MS-Section-B）実装計画 |
| [3d-rtt/archived/ms-3-6-ad-implementation-plan-2026-04-01.md](3d-rtt/archived/ms-3-6-ad-implementation-plan-2026-04-01.md) | Archived | MS-3-6 A/D 実装計画（現行アセット限定） |
| [3d-rtt/archived/ms-3-6-terrain-surface-plan-2026-03-31.md](3d-rtt/archived/ms-3-6-terrain-surface-plan-2026-03-31.md) | Archived | MS-3-6 テレイン表面表現改善（旧 MS-3B）実装計画 |
| [3d-rtt/archived/ms-3-char-a-implementation-plan-2026-03-28.md](3d-rtt/archived/ms-3-char-a-implementation-plan-2026-03-28.md) | Archived | MS-3-Char-A 実装計画（2026-03-28） |
| [3d-rtt/archived/ms-3-char-b-implementation-plan-2026-03-29.md](3d-rtt/archived/ms-3-char-b-implementation-plan-2026-03-29.md) | Archived | MS-3-Char-B 実装計画（2026-03-29） |
| [3d-rtt/archived/ms-asset-shader-plan.md](3d-rtt/archived/ms-asset-shader-plan.md) | Archived | MS-Asset-Shader 実装計画：section_material.wgsl 事前作成 |
| [3d-rtt/archived/phase1-rtt-infrastructure-plan-2026-03-15.md](3d-rtt/archived/phase1-rtt-infrastructure-plan-2026-03-15.md) | Archived | 3D-RtT フェーズ1: RtTインフラ実装計画 |
| [3d-rtt/archived/phase2-hybrid-rtt-plan-2026-03-15.md](3d-rtt/archived/phase2-hybrid-rtt-plan-2026-03-15.md) | Archived | 3D-RtT フェーズ2: ハイブリッドRtT 実装計画 |
| [3d-rtt/archived/phase2-implementation-review.md](3d-rtt/archived/phase2-implementation-review.md) | Archived | Phase 2 実装計画 レビュー |
| [3d-rtt/archived/phase3-implementation-plan-2026-03-16.md](3d-rtt/archived/phase3-implementation-plan-2026-03-16.md) | Archived | Phase 3 実装計画 |
| [3d-rtt/archived/phase3-ms-p3-pre-c-plan.md](3d-rtt/archived/phase3-ms-p3-pre-c-plan.md) | Archived | Phase 3 着手前基盤整備計画 (MS-2C〜MS-P3-Pre-C) |
| [3d-rtt/archived/terrain-visual-reassessment-2026-04-05.md](3d-rtt/archived/terrain-visual-reassessment-2026-04-05.md) | Archived | 地形ビジュアル再検討メモ（2026-04-05） |
| [3d-rtt/archived/wfc-ms0-invariant-spec.md](3d-rtt/archived/wfc-ms0-invariant-spec.md) | Archived | MS-WFC-0: 生成 invariant 仕様化 |
| [3d-rtt/archived/wfc-ms1-anchor-data-model.md](3d-rtt/archived/wfc-ms1-anchor-data-model.md) | Archived | MS-WFC-1: 固定アンカー定義と生成結果モデル化 |
| [3d-rtt/archived/wfc-ms2-5-terrain-zone-mask.md](3d-rtt/archived/wfc-ms2-5-terrain-zone-mask.md) | Archived | 現行の WFC は全セル共通の重み（WEIGHT_GRASS=5, WEIGHT_DIRT=2）で動作するため、Grass/Dirt の分布がマップ全域でほぼ均一になるの提案。 |
| [3d-rtt/archived/wfc-ms2a-crate-adapter-river-mask.md](3d-rtt/archived/wfc-ms2a-crate-adapter-river-mask.md) | Archived | MS-WFC-2a: 外部 WFC crate 選定・アダプタ骨格・川マスク生成 |
| [3d-rtt/archived/wfc-ms2b-wfc-solver-constraints.md](3d-rtt/archived/wfc-ms2b-wfc-solver-constraints.md) | Archived | MS-WFC-2b: WFC ソルバー統合と制約マスキング |
| [3d-rtt/archived/wfc-ms2c-validator.md](3d-rtt/archived/wfc-ms2c-validator.md) | Archived | MS-WFC-2c: 生成後バリデータ（lightweight + debug） |
| [3d-rtt/archived/wfc-ms2d-river-driven-sand-mask.md](3d-rtt/archived/wfc-ms2d-river-driven-sand-mask.md) | Archived | 現状の 2b 実装では、`Sand` は WFC 結果から選ばれ、`post_process_tiles()` がの提案。 |
| [3d-rtt/archived/wfc-ms2e-sand-shore-shape.md](3d-rtt/archived/wfc-ms2e-sand-shore-shape.md) | Archived | MS-WFC-2d により、`Sand` は WFC 出力ではなく `river_mask` 由来の deterministic mask になった。これは責務分離として正しいが、現行実装の候補生成は次の性質を持つの提案。 |
| [3d-rtt/archived/wfc-ms3-procedural-resources.md](3d-rtt/archived/wfc-ms3-procedural-resources.md) | Archived | MS-WFC-3: 木・岩の procedural 配置 |
| [3d-rtt/archived/wfc-ms4-startup-integration.md](3d-rtt/archived/wfc-ms4-startup-integration.md) | Archived | MS-WFC-4: Startup 統合と Yard 内固定資源の移行 |
| [3d-rtt/archived/wfc-ms45-docs-tests.md](3d-rtt/archived/wfc-ms45-docs-tests.md) | Archived | MS-WFC-4.5: ドキュメントと検証整備 |
| [3d-rtt/archived/wfc-refactor-plan-2026-04-04.md](3d-rtt/archived/wfc-refactor-plan-2026-04-04.md) | Archived | WFC 関連リファクタ計画 |
| [3d-rtt/archived/wfc-terrain-generation-plan-2026-04-01.md](3d-rtt/archived/wfc-terrain-generation-plan-2026-04-01.md) | Archived | - 現状は [の計画。 |
| [3d-rtt/archived/world-map-lod1-performance-plan-2026-04-09.md](3d-rtt/archived/world-map-lod1-performance-plan-2026-04-09.md) | Archived | ワールドマップの近景表示で使うの計画。 |
| [archive/bevy-0-19-migration-plan-2026-07-05.md](archive/bevy-0-19-migration-plan-2026-07-05.md) | Archived | Bevy 0.18 のまま留まると、今後のエコシステム追随・バグ修正・パフォーマンス改善（render graph as systems, Parley テキスト等）を受けられないの計画。 |
| [archive/dev-tools-debug-overlay-plan-2026-07-05.md](archive/dev-tools-debug-overlay-plan-2026-07-05.md) | Archived | Soul / Familiar の AI 状態（AssignedTask・フェーズ・Squad 状態）をワールド内で直接確認できず、デバッグがログ頼み。フレームスパイクの可視化手段がないの計画。 |
| [archive/familiar-task-management-hw-ai-extraction-plan-2026-03-11.md](archive/familiar-task-management-hw-ai-extraction-plan-2026-03-11.md) | Archived | Familiar Task Management `hw_ai` 抽出 実装計画 |
| [archive/runtime-correctness-contracts-plan-2026-07-12.md](archive/runtime-correctness-contracts-plan-2026-07-12.md) | Archived | 実行時正しさ契約リファクタリング計画 |
| [archive/save-load-hardening-plan-2026-07-12.md](archive/save-load-hardening-plan-2026-07-12.md) | Archived | Save/Load境界強化・互換性リファクタリング計画 |
| [archive/structural-maintainability-followups-plan-2026-07-12.md](archive/structural-maintainability-followups-plan-2026-07-12.md) | Archived | 構造・保守性・品質ゲート フォローアップ計画 |
| [archive/system-wide-performance-followups-plan-2026-07-07.md](archive/system-wide-performance-followups-plan-2026-07-07.md) | Archived | 全体パフォーマンス改善フォローアップ計画書 |
| [archive/task-execution-refactor-plan-2026-07-07.md](archive/task-execution-refactor-plan-2026-07-07.md) | Archived | task_execution リファクタリング計画（コンテキスト集約・完了/中断区別・ログ降格・boundary.rs 分割） |
| [archive/text-input-ui-plan-2026-07-05.md](archive/text-input-ui-plan-2026-07-05.md) | Archived | テキスト入力 UI — EditableText + clipboard 実装計画 |
| [soul-energy/archived/milestone-roadmap.md](soul-energy/archived/milestone-roadmap.md) | Archived | Soul Energy System — Milestone Roadmap |
| [soul-energy/archived/phase1a-data-model.md](soul-energy/archived/phase1a-data-model.md) | Archived | Phase 1a: Data Model + Grid Infrastructure |
| [soul-energy/archived/phase1b-soul-spa.md](soul-energy/archived/phase1b-soul-spa.md) | Archived | Phase 1b: Soul Spa + GeneratePower Task |
| [soul-energy/archived/phase1c-lamp-and-grid.md](soul-energy/archived/phase1c-lamp-and-grid.md) | Archived | Phase 1c: Outdoor Lamp + Grid Integration + Visual |

