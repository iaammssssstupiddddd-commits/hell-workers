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
- [entity_list_ui.md](entity_list_ui.md): エンティティリストのフィルタリングと操作。
- [task_list_ui.md](task_list_ui.md): タスクリストの表示・タブ切替・クリック操作。
- [info_panel_ui.md](info_panel_ui.md): 選択されたエンティティの詳細情報表示。
- [gather_haul_visual.md](gather_haul_visual.md): 採取や搬送の視覚的なフィードバック。
- [dream-visual.md](dream-visual.md): Dream システムの視覚的フィードバック実装。
- [speech_system.md](speech_system.md): 吹き出しと Soul 画像イベントの仕様。
- [fonts.md](fonts.md): フォントシステムの実装詳細。

## 世界観・アセット
- [world_lore.md](world_lore.md): 世界観設定書。アセットデザインのための世界観・視覚指針（アートスタイル含む）。
- [assets_workflow.md](assets_workflow.md): `Syncthing` を前提にした原本共有、`exports/` 運用、`assets/` 反映手順。

## 不変条件 & イベント（AI 必読）
- [invariants.md](invariants.md): **ゲーム不変条件**。コード変更前に必ず確認すること（Soul/Familiar/タスク/Logistics 各不変条件）。
- [events.md](events.md): **イベントカタログ**。全イベントの Producer / Consumer / Timing 一覧。イベント追加時は必ず更新。

## 開発ガイド
- [architecture.md](architecture.md): 全体構造、システム依存関係、GameTime、空間グリッド一覧。
- [cargo_workspace.md](cargo_workspace.md): Cargo workspace の crate 責務、依存方向、分割ルール（hw_core / hw_world / hw_logistics / hw_jobs / hw_familiar_ai / hw_soul_ai / hw_spatial / hw_ui / hw_visual）。
- [map_generation.md](map_generation.md): `generate_world_layout` を中心にしたマップ生成パイプラインの仕様。seed、WFC、validate、resource 配置、retry/fallback、startup 受け渡しの契約を扱う。
- [world_layout.md](world_layout.md): マップ仕様、地形タイプ、固定アンカー、資源配置の意味、**座標変換関数**（`world_to_grid` 等）。生成パイプライン自体は `map_generation.md` を参照。
- [state.md](state.md): PlayMode、**TaskMode全バリアント一覧**（指定・ゾーン・建築モード等）。
- [debug-features.md](debug-features.md): DevPanel・IBuild など**デバッグ専用機能**の一覧・実装箇所。
- [rendering-performance.md](rendering-performance.md): 描画パイプライン別の draw call 構造、バジェット、最適化方針。
- [performance-profiling.md](performance-profiling.md): 決定的なランタイム計測シナリオ、CSV、Tracy、RenderDocの採取手順。
- [visual_test.md](visual_test.md): Soul GLB Visual Test Scene の操作・アーキテクチャ・UV 計算。
- [DEVELOPMENT.md](DEVELOPMENT.md): AIエージェントおよび開発者向けガイドライン（コーディング規約・MCP活用）。
- [linux-setup.md](linux-setup.md): Linux ネイティブ環境でのビルド・実行セットアップ手順。
- [plans/README.md](plans/README.md): フェーズ分割した実装計画ドキュメント。
- [plans/hvac-plumbing-plan-2026-07-13.md](plans/hvac-plumbing-plan-2026-07-13.md): 換気・導水・Room 認可を M0〜M4 で導入する実装計画。
- [plans/input-action-context-resolver-plan-2026-07-17.md](plans/input-action-context-resolver-plan-2026-07-17.md): 離散キーボード入力の競合解決と Modal/Pause の背景入力遮断を行う Track A1 実装計画。
- [proposals/README.md](proposals/README.md): 提案書一覧とテンプレート。
- [proposals/gameplay-management-improvements-proposal-2026-07-17.md](proposals/gameplay-management-improvements-proposal-2026-07-17.md): 操作、運営ポリシー、復旧・永続化、進行要素を 4 トラックで整理した総合改善提案。
- [proposals/hvac-plumbing-proposal.md](proposals/hvac-plumbing-proposal.md): 採用済みの空調・衛生インフラ提案（世界観・採否理由）。
- `architecture.md` / `cargo_workspace.md` / `familiar_ai.md` / `soul_ai.md`: crate 境界と `root shell` 方針（thin shell、root adapter、leaf plugin の登録責務）を同期済み。
