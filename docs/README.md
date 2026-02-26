# Documentation Index

本プロジェクトの各機能や仕様に関する詳細ドキュメントです。

## 魂と使い魔 (Entities & AI)
- [soul_ai.md](soul_ai.md): 魂（Damned Soul）の自律行動、疲労、ストレスに関する仕様。
- [familiar_ai.md](familiar_ai.md): 使い魔（Familiar）の指揮、リクルート、タスク管理。

## ゲームシステム (Core Systems)
- [tasks.md](tasks.md): タスクの発行、割り当て、ECS Relationships による参照管理。
- [rest_area_system.md](rest_area_system.md): 休憩所（Rest Area）の定員管理、予約、バイタル回復の仕組み。
- [population_system.md](population_system.md): Soul人口（初期/定期スポーン、人口上限、漂流デスポーン）の仕様。
- [logistics.md](logistics.md): 資源の搬送、備蓄場所、オートホールの仕組み。
- [building.md](building.md): 建築プロセス、設計図、必要な材料。
- [room_detection.md](room_detection.md): Room 検出システム（壁・扉・床で囲まれた空間の自動認識・オーバーレイ表示）。
- [dream.md](dream.md): Dreamシステム。睡眠中の夢による通貨獲得メカニクス。
- [state.md](state.md): ゲームの進行状態、プレイモードの遷移。

## UI & Visuals
- [entity_list_ui.md](entity_list_ui.md): エンティティリストのフィルタリングと操作。
- [task_list_ui.md](task_list_ui.md): タスクリストの表示・タブ切替・クリック操作。
- [info_panel_ui.md](info_panel_ui.md): 選択されたエンティティの詳細情報表示。
- [gather_haul_visual.md](gather_haul_visual.md): 採取や搬送の視覚的なフィードバック。
- [speech_system.md](speech_system.md): 吹き出しと Soul 画像イベントの仕様。

## 開発ガイド
- [architecture.md](architecture.md): 全体構造、システム依存関係、GameTime、空間グリッド一覧。
- [world_layout.md](world_layout.md): マップ仕様、地形、**座標変換関数**（`world_to_grid` 等）。
- [state.md](state.md): PlayMode、**TaskMode全バリアント一覧**（指定・ゾーン・建築モード等）。
- [DEVELOPMENT.md](DEVELOPMENT.md): AIエージェントおよび開発者向けガイドライン（コーディング規約・MCP活用）。
- [plans/README.md](plans/README.md): フェーズ分割した実装計画ドキュメント。
- [proposals/README.md](proposals/README.md): 提案書一覧とテンプレート。
