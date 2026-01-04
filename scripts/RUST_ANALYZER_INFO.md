# Rust Analyzerについて

## Rust Analyzerとは

Rust Analyzerは、Rustコードの**補完、エラー検出、リファクタリング**などを提供する**Language Server Protocol (LSP)**実装です。

## 必須かどうか

### ❌ 必須ではない
- RustコードはRust Analyzerなしでも**コンパイル・実行可能**
- `cargo build`や`cargo run`はRust Analyzerとは無関係に動作
- エディタがなくてもコマンドラインで開発可能

### ✅ しかし、非常に便利
Rust Analyzerがあると：

1. **コード補完**
   - 変数名、関数名、メソッドの自動補完
   - 型情報の表示

2. **エラー検出**
   - コンパイル前にエラーを発見
   - リアルタイムでエラー表示

3. **定義へのジャンプ**
   - `F12`で関数定義へ移動
   - `Shift+F12`で使用箇所を検索

4. **リファクタリング**
   - 変数名の一括変更
   - 関数の抽出

5. **ドキュメント表示**
   - ホバーでドキュメント表示
   - 型情報の確認

## エージェントのクラッシュ問題との関係

### Rust Analyzerとファイルウォッチャーの関係

- **Rust Analyzerはファイルウォッチャーを使用**
  - コード変更を監視してリアルタイムに解析
  - `target/`ディレクトリも監視してしまうことがある

- **最適化設定の目的**
  - Rust Analyzerが`target/`を監視しないように設定
  - 大量のファイル変更によるクラッシュを防止
  - **Rust Analyzerを無効化する必要はない**

### 推奨設定

`.vscode/settings.json`に以下の設定を追加しました：

```json
"rust-analyzer.files.excludeDirs": [
  "target",  // ビルド成果物を監視しない
  "dist",
  ".trunk",
  "logs"
]
```

これにより：
- ✅ Rust Analyzerは動作し続ける
- ✅ `target/`ディレクトリは監視しない
- ✅ エージェントのクラッシュを防止

## 結論

### Rust Analyzerなしでも開発可能
- コマンドラインで`cargo build`を実行
- エラーはコンパイル時に確認
- 補完なしでコーディング

### ただし、開発効率は大幅に低下
- コード補完がない
- エラーをすぐに確認できない
- 定義へのジャンプができない
- リファクタリングが困難

### 推奨
**Rust Analyzerはインストールして使用することを強く推奨します。**

エージェントのクラッシュ問題は、Rust Analyzerを無効化するのではなく、**適切に設定することで解決**できます。

## 設定の確認

Rust Analyzerが正しく設定されているか確認：

```powershell
.\scripts\check-rust-analyzer.ps1
```

## 参考

- [Rust Analyzer公式ドキュメント](https://rust-analyzer.github.io/)
- [VS Code Rust Analyzer拡張機能](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)



