# Documentation Index

本プロジェクトの各機能や仕様に関する詳細ドキュメントです。

## 魂と使い魔 (Entities & AI)
- [soul_ai.md](soul_ai.md): 魂（Damned Soul）の自律行動、疲労、ストレスに関する仕様。
- [familiar_ai.md](familiar_ai.md): 使い魔（Familiar）の指揮、リクルート、タスク管理。
- [ai-system-phases.md](ai-system-phases.md): AI システムの4フェーズ設計（Perceive / Decide / Execute / Update）。

## ゲームシステム (Core Systems)
- [tasks.md](tasks.md): タスクの発行、割り当て、ECS Relationships による参照管理。
- [logistics.md](logistics.md): 資源の搬送、備蓄場所、オートホールの仕組み。
- [building.md](building.md): 建築プロセス、設計図、必要な材料。
- [gathering.md](gathering.md): 動的集会システム（自然発生・拡大・統合・消滅）。
- [rest_area_system.md](rest_area_system.md): 休憩所（Rest Area）の定員管理、予約、バイタル回復の仕組み。
- [population_system.md](population_system.md): Soul人口（初期/定期スポーン、人口上限、漂流デスポーン）の仕様。
- [save_load.md](save_load.md): シミュレーション状態の RON セーブ/ロード（F5/F9、allow-list、rehydrate、seed ガード）。
- [room_detection.md](room_detection.md): Room 検出システム（壁・扉・床で囲まれた空間の自動認識・オーバーレイ表示）。
- [dream.md](dream.md): Dreamシステム。睡眠中の夢による通貨獲得メカニクス。
- [state.md](state.md): ゲームの進行状態、プレイモードの遷移。
- [settings.md](settings.md): GameSettings と settings.ron 永続化、設定画面 UI。
- [soul_energy.md](soul_energy.md): Soul Energy システム（発電・消費・停電サイクル、Soul Spa、Outdoor Lamp）。

## UI & Visuals
- [ui-world-first.md](ui-world-first.md): 地図中心のHUD、管理/詳細の共通枠、要対応、建築カタログと補助表示。
- [design/ui-world-first/README.md](design/ui-world-first/README.md): 地図中心UIの設計理由、他ゲームの参考事例、実装前のラフと実装後の代表画面。
- [proposals/ui-usability-audit-proposal-2026-09-13.md](proposals/ui-usability-audit-proposal-2026-09-13.md): UI全領域の実装レビュー、操作性向上32項目、優先順位と受入条件。
- [help-screen.md](help-screen.md): F1/ボタンで開くプレイヤーHelp、catalog ownership、可逆pause、継続更新gate。
- [world-selection.md](world-selection.md): screen-spaceオブジェクトスナップ、クリック／ドラッグ所有、右クリック、選択表示の契約。
- [notifications.md](notifications.md): 有界なトースト／重要履歴、配置不能理由、セーブ／ロード終端結果、タスク操作・Stockpile方針変更結果の仕様。
- [entity_list_ui.md](entity_list_ui.md): エンティティリストのフィルタリングと操作。
- [task_list_ui.md](task_list_ui.md): タスクリストの表示・タブ切替・クリック操作。
- [info_panel_ui.md](info_panel_ui.md): 選択されたエンティティの詳細情報表示。
- [gather_haul_visual.md](gather_haul_visual.md): 採取や搬送の視覚的なフィードバック。
- [dream-visual.md](dream-visual.md): Dream システムの視覚的フィードバック実装。
- [speech_system.md](speech_system.md): 吹き出しと Soul 画像イベントの仕様。
- [fonts.md](fonts.md): フォントシステムの実装詳細。

## 世界観・アセット
- [art-style-criteria.md](art-style-criteria.md): アートスタイルの受入基準と検証観点。
- [building-art-direction.md](building-art-direction.md): 建築物の形状・手描きテクスチャ・UV・光の役割分担、素材表現、制作順と採用条件。
- [building-asset-sets.md](building-asset-sets.md): 設備manifest・authority・依存バイト検証、presentation/residency/pool接続と段階受入。全種の正式releaseは未完。
- [building-art-static-reference.md](building-art-static-reference.md): Bridgeを除く9種の静止性能参照、実地形の配置・状態検査、Capture/Memoryの逐次計測と未測定範囲。
- [plans/3d-rtt/non-wall-floor-building-art-migration-plan-2026-09-19.md](plans/3d-rtt/non-wall-floor-building-art-migration-plan-2026-09-19.md): 壁・床を除く10種のアート移行、モデルとpreviewの接続、状態表示、段階導入と受入の計画。
- [world_lore.md](world_lore.md): 世界観設定書。アセットデザインのための世界観・視覚指針（アートスタイル含む）。
- [assets_workflow.md](assets_workflow.md): `Syncthing` を前提にした原本共有、`exports/` 運用、`assets/` 反映手順。
- [blender-setup.md](blender-setup.md): AI支援Blender編集、MCP安全境界、staging品質gate、GLB検証の手順。

## 不変条件 & イベント（AI 必読）
- [invariants.md](invariants.md): **ゲーム不変条件**。コード変更前に必ず確認すること（Soul/Familiar/タスク/Logistics 各不変条件）。
- [events.md](events.md): **イベントカタログ**。全イベントの Producer / Consumer / Timing 一覧。イベント追加時は必ず更新。

## 開発ガイド
- [plans/orca-normalization-and-coordinator-extraction-plan-2026-10-03.md](plans/orca-normalization-and-coordinator-extraction-plan-2026-10-03.md): 通常版復帰の6工程と実施記録。独自統括を撤去し、公式package・標準設定・同保存会話の通常turnを受入確認。外付け実装は別工程。
- [development-infra/orca-conductor-migration.md](development-infra/orca-conductor-migration.md): 外付け統括構想を完全別プロジェクトOrca Conductorへ移管した案内。設計と今後の実装はhell-workers外で管理。

旧統括の運用・受入ガイドと旧Orca計画は歴史資料です。以下で歴史資料とした文書のcommandや
未チェック項目は現在の作業指示ではなく、旧復旧/自動継続を再実行しません。
必須規則は通常運用へ変更済みです。旧制御実装はprimaryへ取り込んでいません。
TAK-14の製品機能と受入ツールは別対象で、[統合PRの対象・検証境界](development-infra/tak14-pr-preparation.md)を参照します。
統合実装のPR準備と全10種の正式release完了を区別し、原移行計画の未達条件を維持します。

- [orca-quickstart.md](orca-quickstart.md): 通常Orcaのworktree・terminal・providerを使う現行ガイド。独自統括は必須にしない。
- [development-infra/orca-supervised-quickstart-history.md](development-infra/orca-supervised-quickstart-history.md): 撤去した独自統括の旧操作・受入記録。実行手順ではない。
- [architecture.md](architecture.md): 全体構造、システム依存関係、GameTime、空間グリッド一覧。
- [crate-boundaries.md](crate-boundaries.md): crate 間の依存方向とコアロジック分離の原則。
- [cargo_workspace.md](cargo_workspace.md): Cargo workspace の crate 責務、依存方向、分割ルール（hw_core / hw_energy / hw_infra / hw_world / hw_logistics / hw_jobs / hw_familiar_ai / hw_soul_ai / hw_spatial / hw_ui / hw_visual）。
- [indoor_lighting.md](indoor_lighting.md): P03室内Light Fieldのpure core、遮光/LOS、fixed-point field、revision、field-core evidence契約。
- [map_generation.md](map_generation.md): `generate_world_layout` を中心にしたマップ生成パイプラインの仕様。seed、WFC、validate、resource 配置、retry/fallback、startup 受け渡しの契約を扱う。
- [world_layout.md](world_layout.md): マップ仕様、地形タイプ、固定アンカー、資源配置の意味、**座標変換関数**（`world_to_grid` 等）。生成パイプライン自体は `map_generation.md` を参照。
- [state.md](state.md): PlayMode、**TaskMode全バリアント一覧**（指定・ゾーン・建築モード等）。
- [debug-features.md](debug-features.md): DevPanel・IBuild など**デバッグ専用機能**の一覧・実装箇所。
- [rendering-performance.md](rendering-performance.md): 描画パイプライン別の draw call 構造、バジェット、最適化方針。
- [performance-profiling.md](performance-profiling.md): 決定的なランタイム計測シナリオ、CSV、native allocation / RSS、Tracy、RenderDocの採取手順。
- [visual_test.md](visual_test.md): productionとは独立したTopDown建物・地形visual testの操作とScene RtT構造。
- [DEVELOPMENT.md](DEVELOPMENT.md): 開発規約・MCP活用、固定品質ツール、依存監査・Dependabot更新、property testの再現手順。
- [development-infra/rust-analyzer-mcp.md](development-infra/rust-analyzer-mcp.md): 複数エージェントでrust-analyzer MCP backendを共有するadapter、idle解放、IDE側の常駐コスト削減設定。
- [development-infra/orca-development.md](development-infra/orca-development.md): 歴史資料。撤去した独自統括・分離開発の仕様。通常運用の実行手順ではない。
- [development-infra/orca-native-host-execution.md](development-infra/orca-native-host-execution.md): 旧統括の歴史資料。host実行・依存source固定・表示サービスの旧契約。
- [development-infra/orca-request-lifecycle.md](development-infra/orca-request-lifecycle.md): 旧統括の歴史資料。工程承認と全体受入、scope改訂、固定review/終了gateの旧候補実装。
- [development-infra/orca-linear-read-recovery.md](development-infra/orca-linear-read-recovery.md): 旧統括の歴史資料。通信待機と同一Run復旧。旧操作を再実行しない。
- [development-infra/orca-coordinator-continuation.md](development-infra/orca-coordinator-continuation.md): 旧統括の歴史資料。統合指摘と受入工程の継続設計。
- [development-infra/orca-github-linear-acceptance-2026-09-22.md](development-infra/orca-github-linear-acceptance-2026-09-22.md): TAK-7 / PR #26の隔離試験。標準PR link、Draft解除・mergeのLinear反映を実測。全体自動ループの受入とは区別。
- [development-infra/validation-storage-audit-2026-09-13.md](development-infra/validation-storage-audit-2026-09-13.md): 検証データの容量実測、track close時の撤去規則と実装の差、旧checkout・共有worktreeの残存調査。
- [development-infra/validation-storage-workflow.md](development-infra/validation-storage-workflow.md): 終了データの整理、現在の用途による保持、フィードバック中の差分ビルド保全。全job保存・固定日数の義務は設けない。
- [development-infra/validation-storage-workflow-review-2026-09-13.md](development-infra/validation-storage-workflow-review-2026-09-13.md): 保存規則の自己レビューと修正結果。差分ビルド保全、primary coordinator・台帳・整理gateの適用範囲と検証記録。
- [linux-setup.md](linux-setup.md): Linux ネイティブ環境でのビルド・実行セットアップ手順。
- [plans/README.md](plans/README.md): フェーズ分割した実装計画ドキュメント。
- [development-infra/change-aware-ci-acceptance-2026-09-20.md](development-infra/change-aware-ci-acceptance-2026-09-20.md): 変更内容に応じたCI・開発ルール更新の完了記録。受入証拠と既存の日次監査の位置づけ。
- [plans/archive/priority-development-tools-plan-2026-09-13.md](plans/archive/priority-development-tools-plan-2026-09-13.md): cargo-deny、Dependabot、Ruff、actionlint、proptestの導入・GitHub受入完了記録。
- [plans/3d-rtt/provisional-wall-formwork-plan-2026-09-05.md](plans/3d-rtt/provisional-wall-formwork-plan-2026-09-05.md): 木材を使う仮設壁の型枠表現、段階別mesh切替、混在接続・実機受入の計画。
- [plans/3d-rtt/production-door-art-plan-2026-09-05.md](plans/3d-rtt/production-door-art-plan-2026-09-05.md): 木・骨の両開きドア、固定枠と開閉／施錠の3状態、2軸preview・壁との接続・実機受入の計画。
- [plans/archive/save-rehydration-registry-plan-2026-08-03.md](plans/archive/save-rehydration-registry-plan-2026-08-03.md): Track C3 のロード前検証、phase-aware 再構築 registry、通常ロード／rollback共通化の完了記録。
- [plans/archive/building-deconstruction-plan-2026-08-03.md](plans/archive/building-deconstruction-plan-2026-08-03.md): Track C1 の一般建築物解体、固定資源回収、owner-safe cleanup の完了記録。
- [plans/archive/save-catalog-autosave-plan-2026-08-03.md](plans/archive/save-catalog-autosave-plan-2026-08-03.md): Track C2 の手動セーブスロット、catalog、世代オートセーブ計画（archive）。
- [plans/archive/implementation-spec-alignment-plan-2026-07-20.md](plans/archive/implementation-spec-alignment-plan-2026-07-20.md): Dream質量、Familiar疲労閾値、production経路、文書台帳の実装・仕様整合性回復計画。
- [plans/archive/soul-energy-control-plan-2026-07-20.md](plans/archive/soul-energy-control-plan-2026-07-20.md): Track B3のSoul Spa稼働枠、優先配電、保存互換、実機受入の完了記録。
- [plans/archive/player-facing-result-notifications-plan-2026-07-18.md](plans/archive/player-facing-result-notifications-plan-2026-07-18.md): Track A2 の有界な通知センター、配置不能理由、セーブ/ロード終端結果、actual-window受入の完了記録。
- [plans/hvac-plumbing-plan-2026-07-13.md](plans/hvac-plumbing-plan-2026-07-13.md): 換気・導水・Room 認可を M0〜M4 で導入する実装計画。
- [plans/3d-rtt/single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md](plans/3d-rtt/single-scene-rtt-indoor-light-field-migration-plan-2026-08-03.md): Scene RtT 1枚、TopDown表示、Wall / Door遮光の放射状Indoor Light Fieldへ移行する9分割計画の親ロードマップ。
- [proposals/README.md](proposals/README.md): 提案書一覧とテンプレート。
- [proposals/implementation-refactor-audit-proposal-2026-09-17.md](proposals/implementation-refactor-audit-proposal-2026-09-17.md): 全13 crateの実装横断レビューとR01〜R14の個別実装計画への入口。優先順位・依存・根拠・検証条件。
- [proposals/library-tooling-evaluation-proposal-2026-09-13.md](proposals/library-tooling-evaluation-proposal-2026-09-13.md): ライブラリ・開発ツールの導入／置換候補、現行構成との重複、優先順位と採用条件。
- [proposals/orca-parallel-development-proposal-2026-09-20.md](proposals/orca-parallel-development-proposal-2026-09-20.md): 歴史資料。旧A/B・専任review・統括の運用素案。継続指示ではない。
- [plans/orca-parallel-development-plan-2026-09-20.md](plans/orca-parallel-development-plan-2026-09-20.md): 歴史資料。旧固定受付・A/B・統括の受入記録。通常化方針で置換済み。
- [plans/orca-git-review-loop-plan-2026-09-22.md](plans/orca-git-review-loop-plan-2026-09-22.md): 歴史資料。旧review/統合ループの受入記録。自動同期を継続しない。
- [plans/orca-ui-lifecycle-plan-2026-09-23.md](plans/orca-ui-lifecycle-plan-2026-09-23.md): 歴史資料。旧受付・役割・終了/再開・整理の受入記録。
- [plans/orca-abcd-workflow-expansion-plan-2026-09-25.md](plans/orca-abcd-workflow-expansion-plan-2026-09-25.md): 歴史資料。撤去した統括のA〜D roadmap。旧実装/復旧を再開しない。
- [plans/orca-request-lifecycle-correction-plan-2026-09-26.md](plans/orca-request-lifecycle-correction-plan-2026-09-26.md): 歴史資料。旧全体受入・継続・同期・終了の是正計画。
- [development-infra/orca-ui-extension.md](development-infra/orca-ui-extension.md): 歴史資料。旧本体UI/APIとcontroller契約。現在の配備ではない。
- [proposals/progression-and-choice-proposal-2026-08-09.md](proposals/progression-and-choice-proposal-2026-08-09.md): Track D の Dream Edict、Contract、Familiar 昇格を扱う進行・選択提案。
- [proposals/archive/gameplay-management-improvements-proposal-2026-07-17.md](proposals/archive/gameplay-management-improvements-proposal-2026-07-17.md): Track A〜C のロードマップと完了履歴を保持するアーカイブ提案。
- [proposals/hvac-plumbing-proposal.md](proposals/hvac-plumbing-proposal.md): 採用済みの空調・衛生インフラ提案（世界観・採否理由）。
- `architecture.md` / `cargo_workspace.md` / `familiar_ai.md` / `soul_ai.md`: crate 境界と `root shell` 方針（thin shell、root adapter、leaf plugin の登録責務）を同期済み。
