# 肥大化防止ガイド

## 現在の状況

✅ **削除完了**: `x86_64-pc-windows-msvc/`ディレクトリ（約7.37 GB）を削除しました
✅ **現在のサイズ**: 約1.1 GB（適切な範囲内）

## 肥大化防止策

### 1. 自動クリーンアップ（推奨）

ビルドスクリプト（`scripts/build.ps1`、`scripts/check.ps1`）を使用すると、自動的に不要なファイルを削除します：

```powershell
# 自動クリーンアップ付きでビルド
.\scripts\build.ps1

# 自動クリーンアップ付きでチェック
.\scripts\check.ps1
```

### 2. 定期メンテナンス

週に1回程度、以下を実行してください：

```powershell
# サイズ確認
.\scripts\check-size.ps1

# 自動クリーンアップ（3GBを超える場合）
.\scripts\prevent-bloat.ps1 -AutoClean

# 手動クリーンアップ（対話式）
.\scripts\prevent-bloat.ps1
```

### 3. 設定の確認

`.cargo/config.toml`で不要なターゲットが生成されないように設定されています：

- ✅ `[build] target`はコメントアウト（デフォルトターゲットを使用）
- ✅ クロスコンパイル設定もコメントアウト

**重要**: クロスコンパイルが必要な場合のみ、該当部分のコメントを外してください。

### 4. 手動クリーンアップ

必要な場合のみ：

```powershell
# 安全な最適化（ビルド速度への影響最小）
.\scripts\optimize-target.ps1

# 完全クリーン（すべて削除、次回ビルドは遅い）
.\scripts\clean-target.ps1 -Deep
```

## 推奨ワークフロー

### 日常開発
1. `scripts/build.ps1`または`scripts/check.ps1`を使用（自動クリーンアップ付き）
2. 週に1回`scripts/prevent-bloat.ps1`を実行

### 定期的なメンテナンス
```powershell
# 1. サイズ確認
.\scripts\check-size.ps1

# 2. 必要に応じてクリーンアップ
.\scripts\prevent-bloat.ps1

# 3. 詳細分析が必要な場合
.\scripts\analyze-target.ps1
```

## トラブルシューティング

### サイズが3GBを超えた場合

```powershell
# 自動クリーンアップを実行
.\scripts\prevent-bloat.ps1 -AutoClean -MaxSizeGB 3
```

### x86_64-pc-windows-msvcが再生成された場合

1. `.cargo/config.toml`を確認
2. `[build] target`がコメントアウトされているか確認
3. 不要であれば削除：
   ```powershell
   .\scripts\delete-x86.ps1
   ```

## 注意事項

⚠️ **削除してはいけないもの**:
- `target/debug/deps/` - 依存関係のコンパイル済みコード（ビルド速度に重要）
- `target/debug/incremental/` - 最新のインクリメンタルキャッシュ（ビルド速度に重要）

✅ **削除しても問題ないもの**:
- `target/x86_64-pc-windows-msvc/` - クロスコンパイル用（通常不要）
- `target/debug/build/` - 再生成可能（削除すると次回ビルドが少し遅い）

## 設定の初期化

初回セットアップ：

```powershell
.\scripts\setup-prevention.ps1
```

これで設定を確認し、最適化のアドバイスを表示します。


