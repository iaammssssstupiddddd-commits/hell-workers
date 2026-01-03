# ファイルウォッチャー最適化ガイド

## 問題: エージェントが頻繁にクラッシュする

**原因**: ファイルウォッチャーが成果物ディレクトリ（`target/`, `dist/`, `.trunk/`, `logs/`）を監視しているため、大量のファイル変更によってメモリ不足やクラッシュが発生

## 実施した対策

### 1. VS Code/Cursor設定 (`.vscode/settings.json`)

成果物ディレクトリをファイルウォッチャーから除外：

- ✅ `target/` - Rustのビルド成果物（数千ファイル）
- ✅ `dist/` - 配布用成果物
- ✅ `.trunk/` - Trunkのキャッシュ
- ✅ `logs/` - ログファイル
- ✅ `*.log`, `*.txt` - ログファイル

### 2. Rust Analyzer設定 (`rust-analyzer.toml`)

Rust Analyzerが監視するディレクトリを制限：

```toml
[files]
excludeDirs = ["target", "dist", ".trunk", "logs"]
```

### 3. Cursor設定 (`.cursorignore`)

Cursor IDEの監視から成果物を除外

### 4. Trunk設定 (`Trunk.toml`)

Trunkのファイルウォッチャーを最適化：

```toml
[serve]
watch = ["src/**", "assets/**", "index.html"]
ignore = ["target/**", "dist/**", "logs/**"]
```

## 確認方法

設定が正しく適用されているか確認：

```powershell
.\scripts\check-watchers.ps1
```

## 効果

### Before（対策前）
- ❌ `target/`内の数千ファイルを監視
- ❌ ビルド時に大量のファイル変更イベント
- ❌ メモリ使用量が増加
- ❌ エージェントがクラッシュ

### After（対策後）
- ✅ ソースコードのみを監視（`src/`, `assets/`）
- ✅ ビルド時のファイル変更イベントを無視
- ✅ メモリ使用量が削減
- ✅ エージェントが安定

## 監視対象

### 監視する（変更を検知すべき）もの
- `src/**/*.rs` - Rustソースコード
- `Cargo.toml` - 依存関係
- `assets/**` - ゲームアセット
- `index.html` - Webページ

### 監視しない（除外すべき）もの
- `target/**` - ビルド成果物
- `dist/**` - 配布用成果物
- `.trunk/**` - Trunkキャッシュ
- `logs/**` - ログファイル
- `*.log`, `*.txt` - ログファイル

## トラブルシューティング

### エージェントがまだクラッシュする場合

1. **IDEの再起動**
   ```
   Cursor/VS Codeを完全に再起動
   ```

2. **Rust Analyzerの再起動**
   ```
   Cursor: Ctrl+Shift+P → "Rust Analyzer: Restart server"
   ```

3. **設定の確認**
   ```powershell
   .\scripts\check-watchers.ps1
   ```

4. **手動でウォッチャーをリセット**
   - IDEを完全に終了
   - `.vscode/`フォルダが存在するか確認
   - 再度IDEを起動

### ファイル変更が検知されない場合

監視除外設定が厳しすぎる可能性があります。`.vscode/settings.json`の`files.watcherInclude`を確認してください。

## 追加の最適化

### Windowsのファイルウォッチャー制限

Windowsでは、1つのプロセスが監視できるファイル数の上限があります（通常8192ファイル）。`target/`には数千ファイルがあるため、除外しないとすぐに上限に達します。

### メモリ使用量

ファイルウォッチャーは各ファイルに対してメモリを使用します。数千ファイルを監視すると、数百MB以上のメモリを使用する可能性があります。

## 参考

- [VS Code File Watchers](https://code.visualstudio.com/docs/getstarted/settings#_files-watcher-exclude)
- [Rust Analyzer Configuration](https://rust-analyzer.github.io/manual.html#configuration)
- [Trunk Watch Configuration](https://trunkrs.dev/configuration/)




