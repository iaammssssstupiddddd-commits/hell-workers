# Hell Workers

地獄の住民たち（魂）をこき使い、世界を構築する自動化・基地建設系リアルタイムストラテジー（RTS）ゲームです。

## 技術スタック

- **Engine**: Bevy 0.17
- **Language**: Rust (Edition 2024)
- **UI System**: Bevy UI + `bevy-inspector-egui` (debug)
- **Features**:
    - **ECS Relationships**: エンティティ間の複雑な関係（指揮、タスク、所持、格納）を効率的に管理。
    - **Optimized Task System**: 空間グリッドを活用した高速なタスク検索と割り当て。
    - **Soul AI**: 疲労、ストレス、やる気を持つ自律的な作業ユニット。
    - **Familiar AI**: 魂を指揮し、効率的な物流を構築するマネジメントユニット。

## ディレクトリ構成

- `src/`: Rust ソースコード
    - `entities/`: 主要なエンティティ（魂、使い魔、建物など）の定義
    - `systems/`: ゲームロジック
        - `familiar_ai/`: 使い魔の管理・指揮ロジック
        - `soul_ai/`: 魂の作業・生命維持ロジック
        - `visual/`: プログレスバー、エフェクト等の視覚演出
        - `jobs/`: タスク発行・管理
    - `interface/`: UI コンポーネント
    - `plugins/`: Bevy プラグイン構成
- `docs/`: 技術仕様書、要件ドキュメント
- `proposals/`: 機能追加やリファクタリングの提案書
- `assets/`: スプライト、フォントなどのリソース
- `scripts/`: ユーティリティスクリプト（画像変換等）

## 開発の始め方

### ビルドと実行
```powershell
cargo run
```

### デバッグ
- `F12`: ワールドインスペクターのトグル
- `Space`: ポーズ / 再開（Virtual Time）

## 関連ドキュメント
詳細は [docs/README.md](docs/README.md) を参照してください。
