# Cursor/Antigravity との共存ガイド

## 結論：問題なく共存できます ✅

Rust Analyzer、Cursor、Antigravityは**互いに競合せず、共存可能**です。

## 各ツールの役割

### 1. Rust Analyzer
- **役割**: Rustコードの解析・補完・エラー検出
- **設定場所**: `.vscode/settings.json`（`rust-analyzer.*`）
- **ファイル監視**: ワークスペース設定で `target/` 等を除外（下記）

### 2. Cursor IDE
- **役割**: VS Codeベースのエディタ（Rust Analyzerのホスト）
- **設定場所**: `.vscode/settings.json`（VS Code互換）
- **ファイル監視**: `files.watcherExclude`で成果物を除外

### 3. Antigravity（Cursor AI Agent）
- **役割**: AIアシスタント（コード生成・リファクタリング）
- **設定場所**: `.cursorignore`（監視から除外するファイル）
- **ファイル監視**: プロジェクト全体をスキャン（設定で最適化可能）

## 現在の設定状況（リポジトリにコミットされている内容）

### `.vscode/settings.json`

次を含みます（実ファイルと一致させること）。

- **`terminal.integrated.env.linux`**: Linux 統合ターミナルで `CARGO_TARGET_DIR=target`（ビルド成果物の場所を統一）
- **`files.watcherExclude`**: `target/`, `dist/`, `.trunk/`, `logs/` を IDE のファイルウォッチャーから除外
- **`rust-analyzer.files.excludeDirs`**: 上記と同様のディレクトリを rust-analyzer の監視から除外

### `.cursorignore`

`target/`, `dist/`, `.trunk/`, `logs/` に加え、ログやビルド出力系のパターンなどが記載されています。エージェントが巨大な成果物を読み込みすぎないための設定です。

## 共存の仕組み

### ファイル監視の階層

```
Cursor IDE
├── ファイルウォッチャー（.vscode/settings.json）
│   └── target/ 等を除外
├── Rust Analyzer（.vscode/settings.json）
│   └── target/ 等を除外
└── Cursor エージェント（.cursorignore）
    └── 成果物・ログ等を除外
```

**ビルド成果物を各レイヤで除外**しているため、ビルド時の大量イベントによる負荷を抑えられます。

## パフォーマンスへの影響

### 最適化前（想定）
- 各ツールが `target/` などを広く監視すると、ビルド時に大量のファイル変更イベントが発生しうる

### 最適化後（本リポジトリの方針）
- `files.watcherExclude` と `rust-analyzer.files.excludeDirs` で成果物を除外
- `.cursorignore` でエージェント側のスキャンを抑制

## 設定の確認

Windows での動作確認用スクリプト（任意）:

```powershell
# ファイルウォッチャー設定を確認
.\scripts\check-watchers.ps1

# Rust Analyzer設定を確認
.\scripts\check-rust-analyzer.ps1
```

## トラブルシューティング

### 問題: エージェントや IDE が遅い

**原因**: 大量のファイルを監視・スキャンしている可能性

**解決策**:
1. `.cursorignore` と `.vscode/settings.json` が意図どおりか確認
2. Cursor を再起動して設定を再読み込み

### 問題: Rust Analyzerが動作しない

**原因**: CursorとRust Analyzerの競合ではなく、拡張機能の問題であることが多い

**解決策**:
1. Rust Analyzer拡張機能がインストールされているか確認
2. `Ctrl+Shift+P` → "Rust Analyzer: Restart server"
3. `.vscode/settings.json`の設定を確認

### 問題: ファイル変更が検知されない

**原因**: `files.watcherExclude` が広すぎる、または別の設定と競合している可能性

**解決策**:
1. 意図したソースパスが除外リストに入っていないか確認
2. 必要なら `files.watcherInclude` で明示的に含める（チームで合意のうえ）

## 推奨（再確認用チェックリスト）

### `.vscode/settings.json`
- `files.watcherExclude`: 成果物ディレクトリを除外
- `rust-analyzer.files.excludeDirs`: Rust Analyzer の監視から同様のディレクトリを除外
- Linux では `terminal.integrated.env.linux` で `CARGO_TARGET_DIR` を統一（任意だが本リポジトリでは設定済み）

### `.cursorignore`
- `target/`, `dist/`, `.trunk/`, `logs/` 等の除外

## まとめ

- **Rust Analyzer**: コード補完・エラー検出
- **Cursor IDE**: エディタ機能
- **Cursor エージェント**: AI アシスタント（`.cursor/rules/*.mdc` と `.cursorignore` で運用ルール・監視範囲を制御）

これらは**共存可能**で、上記ファイルにより**リポジトリ単位で**最適化されています。ローカルで上書きしているユーザー設定がある場合は、競合がないかだけ確認してください。
