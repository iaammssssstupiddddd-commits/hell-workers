# Plans Index

`docs/plans` の文書ステータス一覧（更新日: 2026-08-08）。

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
| [3d-rtt/asset-milestones-2026-03-17.md](3d-rtt/asset-milestones-2026-03-17.md) | 進行中（建築・terrain track継続、Soul GLB runtime trackはSuperseded） | アセット作成マイルストーン |
| [3d-rtt/lighting-visual-plan-2026-04-04.md](3d-rtt/lighting-visual-plan-2026-04-04.md) | Superseded | Outdoor Lamp のローカル照明で Soul / 建物に落ちる影を追加する計画 |
| [3d-rtt/milestone-roadmap.md](3d-rtt/milestone-roadmap.md) | Superseded（完了済み実装の履歴。未完項目は凍結） | 3D-RtT 移行ロードマップ |
| [3d-rtt/single-scene-light-field/01-single-scene-rtt-plan-2026-08-03.md](3d-rtt/single-scene-light-field/01-single-scene-rtt-plan-2026-08-03.md) | Draft | Soul mask target／camera／proxyを撤去し、Scene RtT 1枚へ移行するP01。 |
| [3d-rtt/single-scene-light-field/02-topdown-presentation-plan-2026-08-03.md](3d-rtt/single-scene-light-field/02-topdown-presentation-plan-2026-08-03.md) | Draft | Door実経路、TopDown camera、Building分類、Soul billboard／Familiar前景を統合するP02。 |
| [3d-rtt/single-scene-light-field/03-indoor-light-domain-core-plan-2026-08-03.md](3d-rtt/single-scene-light-field/03-indoor-light-domain-core-plan-2026-08-03.md) | Draft | 固定精度のradial field、遮光grid、supercover LOSを`hw_infra`へ実装するP03。 |
| [3d-rtt/single-scene-light-field/04-indoor-light-runtime-integration-plan-2026-08-03.md](3d-rtt/single-scene-light-field/04-indoor-light-runtime-integration-plan-2026-08-03.md) | Draft | Wall／Door／給電／Roomのsnapshot、dirty管理、更新順を接続するP04。 |
| [3d-rtt/single-scene-light-field/05-indoor-light-save-lifecycle-plan-2026-08-03.md](3d-rtt/single-scene-light-field/05-indoor-light-save-lifecycle-plan-2026-08-03.md) | Blocked by coordination | FixtureMount保存、named rehydrate step、load／rollback fail-darkを導入するP05。 |
| [3d-rtt/single-scene-light-field/06-indoor-light-rendering-plan-2026-08-03.md](3d-rtt/single-scene-light-field/06-indoor-light-rendering-plan-2026-08-03.md) | Draft | 100×100共有textureとTerrain／構造物receiverへLight Fieldを表示するP06。 |
| [3d-rtt/single-scene-light-field/07-indoor-light-gameplay-room-plan-2026-08-03.md](3d-rtt/single-scene-light-field/07-indoor-light-gameplay-room-plan-2026-08-03.md) | Draft | Soul回復とRoom照度summaryを同じCPU field revisionへ統合するP07。 |
| [3d-rtt/single-scene-light-field/08-legacy-cleanup-release-plan-2026-08-03.md](3d-rtt/single-scene-light-field/08-legacy-cleanup-release-plan-2026-08-03.md) | Draft | Soul projector／section／legacy mirrorを撤去し、最終性能・Help gateを閉じるP08。 |
| [3d-rtt/single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md](3d-rtt/single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md) | In Progress | Scene RtT 1枚、TopDown表示、放射状Indoor Light Fieldへの9分割親ロードマップ。 |
| [3d-rtt/terrain-lod-switch-flicker-plan-2026-04-17.md](3d-rtt/terrain-lod-switch-flicker-plan-2026-04-17.md) | Draft | 地形 LOD の単発切替ポップを観測し、短い dither 遷移で抑える計画 |
| [building-deconstruction-plan-2026-08-03.md](building-deconstruction-plan-2026-08-03.md) | Draft | 完成建物の安全な撤去、固定資源回収、owner別cleanupを導入するTrack C1計画。 |
| [development-workstation-blender-migration-plan-2026-07-29.md](development-workstation-blender-migration-plan-2026-07-29.md) | In Progress | 新PCへの開発環境移行と、現PCでのBlender／asset新規構築計画。 |
| [hvac-plumbing-plan-2026-07-13.md](hvac-plumbing-plan-2026-07-13.md) | Draft | 地獄のインフラ（換気・導水・部屋認可）実装計画 |
| [player-facing-result-notifications-plan-2026-07-18.md](player-facing-result-notifications-plan-2026-07-18.md) | In Progress | 配置不能理由とセーブ/ロードの終端結果をゲーム画面から確実に確認できないの計画。 |
| [save-catalog-autosave-plan-2026-08-03.md](save-catalog-autosave-plan-2026-08-03.md) | Draft | 手動slot、bounded catalog、世代autosaveを段階導入するTrack C2計画。 |

## アーカイブ計画書一覧 (`archive/` / `**/archived/`)

| Document | Status | Notes |
|---|---|---|
| [3d-rtt/archived/00-baseline-gates-plan-2026-08-03.md](3d-rtt/archived/00-baseline-gates-plan-2026-08-03.md) | Archived | 実装後の結果を見て性能閾値、fixture、表示分類、光の意味論を都合よく変えられる状態をなくすの計画。 |
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
| [archive/actionable-task-dashboard-plan-2026-07-19.md](archive/actionable-task-dashboard-plan-2026-07-19.md) | Archived | 停滞タスクの理由を安全に可視化し、絞り込み・優先度変更・owner別キャンセルを提供するの計画。 |
| [archive/bevy-0-19-migration-plan-2026-07-05.md](archive/bevy-0-19-migration-plan-2026-07-05.md) | Archived | Bevy 0.18 のまま留まると、今後のエコシステム追随・バグ修正・パフォーマンス改善（render graph as systems, Parley テキスト等）を受けられないの計画。 |
| [archive/dev-tools-debug-overlay-plan-2026-07-05.md](archive/dev-tools-debug-overlay-plan-2026-07-05.md) | Archived | Soul / Familiar の AI 状態（AssignedTask・フェーズ・Squad 状態）をワールド内で直接確認できず、デバッグがログ頼み。フレームスパイクの可視化手段がないの計画。 |
| [archive/familiar-operation-policy-plan-2026-07-20.md](archive/familiar-operation-policy-plan-2026-07-20.md) | Archived | Track B2 Familiar 運用ポリシー・永続化 実装計画 |
| [archive/familiar-operation-policy-validation-plan-2026-07-26.md](archive/familiar-operation-policy-validation-plan-2026-07-26.md) | Archived | B2の受入項目が、既存自動テストで確定済みの契約、追加harnessが必要な客観検証、の計画。 |
| [archive/familiar-task-management-hw-ai-extraction-plan-2026-03-11.md](archive/familiar-task-management-hw-ai-extraction-plan-2026-03-11.md) | Archived | Familiar Task Management `hw_ai` 抽出 実装計画 |
| [archive/implementation-spec-alignment-plan-2026-07-20.md](archive/implementation-spec-alignment-plan-2026-07-20.md) | Archived | - 現行実装と仕様文書の比較で、実装バグ、意図的な実装変更に追従していない文書、未登録の重複system、の計画。 |
| [archive/input-action-context-resolver-plan-2026-07-17.md](archive/input-action-context-resolver-plan-2026-07-17.md) | Archived | Track A1: 離散キーボード競合解決と Modal/Pause の背景入力遮断計画 |
| [archive/large-source-file-split-plan-2026-07-17.md](archive/large-source-file-split-plan-2026-07-17.md) | Archived | 500行以上の実装ファイル分割計画 |
| [archive/runtime-correctness-contracts-plan-2026-07-12.md](archive/runtime-correctness-contracts-plan-2026-07-12.md) | Archived | 実行時正しさ契約リファクタリング計画 |
| [archive/save-load-hardening-plan-2026-07-12.md](archive/save-load-hardening-plan-2026-07-12.md) | Archived | Save/Load境界強化・互換性リファクタリング計画 |
| [archive/save-rehydration-registry-plan-2026-08-03.md](archive/save-rehydration-registry-plan-2026-08-03.md) | Archived | ロード後再構築の暗黙順序と通常ロード／rollbackの追従漏れを機械的に防げないの計画。 |
| [archive/soul-energy-control-plan-2026-07-20.md](archive/soul-energy-control-plan-2026-07-20.md) | Archived | Soul Spa の稼働枠を操作できず、供給不足時の grid が全設備を一律停止するの計画。 |
| [archive/stockpile-policy-manual-acceptance-plan-2026-07-23.md](archive/stockpile-policy-manual-acceptance-plan-2026-07-23.md) | Archived | B1実装完了後の実機受入結果と、B1-R05修正後の再受入完了記録。 |
| [archive/stockpile-policy-plan-2026-07-20.md](archive/stockpile-policy-plan-2026-07-20.md) | Archived | 現在在庫と受入方針が未分離で、搬送経路ごとの判定も統一されていないの計画。 |
| [archive/stockpile-resource-checklist-plan-2026-07-24.md](archive/stockpile-resource-checklist-plan-2026-07-24.md) | Archived | Stockpile受入資材をチェックリスト化する計画。 |
| [archive/structural-maintainability-followups-plan-2026-07-12.md](archive/structural-maintainability-followups-plan-2026-07-12.md) | Archived | 構造・保守性・品質ゲート フォローアップ計画 |
| [archive/system-wide-performance-followups-plan-2026-07-07.md](archive/system-wide-performance-followups-plan-2026-07-07.md) | Archived | 全体パフォーマンス改善フォローアップ計画書 |
| [archive/system-wide-runtime-performance-plan-2026-07-12.md](archive/system-wide-runtime-performance-plan-2026-07-12.md) | Archived | 全体ランタイム・ホットパス性能改善計画書 |
| [archive/task-dashboard-performance-validation-plan-2026-07-20.md](archive/task-dashboard-performance-validation-plan-2026-07-20.md) | Archived | A3で未整備のdashboard mode別AI work counterと実renderer / allocator計測を、再現可能なperf harnessへ載せるの計画。 |
| [archive/task-execution-refactor-plan-2026-07-07.md](archive/task-execution-refactor-plan-2026-07-07.md) | Archived | task_execution リファクタリング計画（コンテキスト集約・完了/中断区別・ログ降格・boundary.rs 分割） |
| [archive/text-input-ui-plan-2026-07-05.md](archive/text-input-ui-plan-2026-07-05.md) | Archived | テキスト入力 UI — EditableText + clipboard 実装計画 |
| [soul-energy/archived/milestone-roadmap.md](soul-energy/archived/milestone-roadmap.md) | Archived | Soul Energy System — Milestone Roadmap |
| [soul-energy/archived/phase1a-data-model.md](soul-energy/archived/phase1a-data-model.md) | Archived | Phase 1a: Data Model + Grid Infrastructure |
| [soul-energy/archived/phase1b-soul-spa.md](soul-energy/archived/phase1b-soul-spa.md) | Archived | Phase 1b: Soul Spa + GeneratePower Task |
| [soul-energy/archived/phase1c-lamp-and-grid.md](soul-energy/archived/phase1c-lamp-and-grid.md) | Archived | Phase 1c: Outdoor Lamp + Grid Integration + Visual |
