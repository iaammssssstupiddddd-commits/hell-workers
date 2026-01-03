# Cursor/Antigravity との共存ガイド

## 結論：問題なく共存できます ✅

Rust Analyzer、Cursor、Antigravityは**互いに競合せず、共存可能**です。

## 各ツールの役割

### 1. Rust Analyzer
- **役割**: Rustコードの解析・補完・エラー検出
- **設定場所**: `.vscode/settings.json`（`rust-analyzer.*`）
- **ファイル監視**: `src/**`のみを監視（`target/`は除外）

### 2. Cursor IDE
- **役割**: VS Codeベースのエディタ（Rust Analyzerのホスト）
- **設定場所**: `.vscode/settings.json`（VS Code互換）
- **ファイル監視**: `files.watcherExclude`で成果物を除外

### 3. Antigravity（Cursor AI Agent）
- **役割**: AIアシスタント（コード生成・リファクタリング）
- **設定場所**: `.cursorignore`（監視から除外するファイル）
- **ファイル監視**: プロジェクト全体をスキャン（設定で最適化可能）

## 現在の設定状況

### ✅ 既に最適化済み

1. **`.vscode/settings.json`**
   ```json
   {
     "files.watcherExclude": {
       "**/target/**": true,
       // ... 成果物を除外
     },
     "rust-analyzer.files.excludeDirs": [
       "target", "dist", ".trunk", "logs"
     ]
   }
   ```
   - CursorとRust Analyzerの両方に適用
   - ファイルウォッチャーを最適化

2. **`.cursorignore`**
   ```
   target/
   dist/
   .trunk/
   logs/
   ```
   - Antigravityエージェントが監視しないファイルを指定
   - エージェントのクラッシュを防止

## 共存の仕組み

### ファイル監視の階層

```
Cursor IDE
├── ファイルウォッチャー（.vscode/settings.json）
│   └── target/ を除外 ✅
├── Rust Analyzer（.vscode/settings.json）
│   └── target/ を除外 ✅
└── Antigravity（.cursorignore）
    └── target/ を除外 ✅
```

**すべてのツールが同じ方針で`target/`を除外**しているため、競合しません。

## パフォーマンスへの影響

### Before（最適化前）
- ❌ 3つのツールすべてが`target/`を監視
- ❌ ビルド時に大量のファイル変更イベント
- ❌ メモリ使用量が3倍
- ❌ エージェントがクラッシュ

### After（最適化後）
- ✅ すべてのツールが`target/`を除外
- ✅ ソースコードのみを監視
- ✅ メモリ使用量が削減
- ✅ 安定して動作

## 設定の確認

現在の設定が正しく適用されているか確認：

```powershell
# ファイルウォッチャー設定を確認
.\scripts\check-watchers.ps1

# Rust Analyzer設定を確認
.\scripts\check-rust-analyzer.ps1
```

## トラブルシューティング

### 問題: Antigravityが遅い

**原因**: 大量のファイルをスキャンしている可能性

**解決策**:
1. `.cursorignore`が正しく設定されているか確認
2. Cursorを再起動して設定を再読み込み

### 問題: Rust Analyzerが動作しない

**原因**: CursorとRust Analyzerの競合ではなく、拡張機能の問題

**解決策**:
1. Rust Analyzer拡張機能がインストールされているか確認
2. `Ctrl+Shift+P` → "Rust Analyzer: Restart server"
3. `.vscode/settings.json`の設定を確認

### 問題: ファイル変更が検知されない

**原因**: `files.watcherExclude`が厳しすぎる可能性

**解決策**:
1. `.vscode/settings.json`の`files.watcherInclude`を確認
2. 必要なファイルパターンが含まれているか確認

## 推奨設定（再確認）

### `.vscode/settings.json`
- ✅ `files.watcherExclude`: 成果物を除外
- ✅ `rust-analyzer.files.excludeDirs`: Rust Analyzerの監視を最適化
- ✅ `files.watcherInclude`: ソースコードのみを監視

### `.cursorignore`
- ✅ `target/`, `dist/`, `.trunk/`, `logs/`を除外

## まとめ

- ✅ **Rust Analyzer**: コード補完・エラー検出
- ✅ **Cursor IDE**: エディタ機能
- ✅ **Antigravity**: AIアシスタント

これらは**すべて共存可能**で、現在の設定により**最適化されています**。

設定を変更する必要はありません。現在の構成のまま使用できます。



