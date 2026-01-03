# Bevyプロジェクトでの最適化

## ✅ Bevyプロジェクトでも完全に有効です

現在のプロジェクトは**Bevy 0.15**を使用しており、実施した最適化は**Bevyプロジェクトに特化して設計**されています。

## Bevyプロジェクトでの重要性

### なぜBevyプロジェクトで特に重要か

1. **大量の依存関係**
   - Bevyは多くのサブクレート（bevy_render, bevy_sprite, bevy_winit等）を含む
   - `target/debug/deps/`には数百のコンパイル済みライブラリが含まれる
   - **約841 MB**の依存関係キャッシュ（削除すると再コンパイルに数十分）

2. **頻繁なビルド**
   - ゲーム開発では頻繁にビルド・実行する
   - ビルドのたびに`target/`内で大量のファイルが更新される
   - ファイルウォッチャーが監視していると、毎回クラッシュのリスク

3. **大きなビルド成果物**
   - Bevyアプリは比較的大きなバイナリを生成
   - `target/debug/`には実行ファイル、DLL、PDBファイルなどが含まれる
   - インクリメンタルビルドキャッシュも大きくなる

## 現在の設定がBevyに最適化されている理由

### 1. 依存関係の保護

```json
"rust-analyzer.files.excludeDirs": [
  "target",  // Bevyの依存関係（deps/）を保護
  ...
]
```

- ✅ `target/debug/deps/`は監視されない
- ✅ Bevyの再コンパイルを防止
- ✅ ビルド速度を維持

### 2. 動的リンクのサポート

`Cargo.toml`で`dynamic_linking`を使用している場合：

```toml
[features]
default = ["dynamic_linking"]
dynamic_linking = ["bevy/dynamic_linking"]
```

- ✅ `target/debug/bevy_dylib.dll`などのDLLファイルも除外
- ✅ ビルド時のDLL更新を監視しない
- ✅ パフォーマンスに影響なし

### 3. アセットファイルの監視

```json
"files.watcherInclude": [
  "**/assets/**",  // Bevyのアセットを監視
  ...
]
```

- ✅ `assets/`ディレクトリは監視対象
- ✅ テクスチャ、サウンドなどの変更を検知
- ✅ ゲーム開発に必要なファイル変更を検知

### 4. Trunkとの統合（WebAssemblyビルド）

`Trunk.toml`でWebAssemblyビルドも最適化：

```toml
[serve]
watch = ["src/**", "assets/**", "index.html"]
ignore = ["target/**", "dist/**", "logs/**"]
```

- ✅ Bevy Webアプリの開発でも最適化
- ✅ `trunk serve`時のファイルウォッチャーも最適化

## Bevy特有のディレクトリ構造

### 監視から除外（成果物）

```
target/
├── debug/
│   ├── deps/          ← Bevy依存関係（841 MB）- 除外 ✅
│   ├── build/         ← ビルドスクリプト成果物 - 除外 ✅
│   ├── incremental/   ← インクリメンタルキャッシュ - 除外 ✅
│   └── bevy_app.exe   ← 実行ファイル - 除外 ✅
└── x86_64-pc-windows-msvc/  ← クロスコンパイル - 除外 ✅
```

### 監視対象（ソースコード）

```
src/                   ← Rustソースコード - 監視 ✅
assets/                ← ゲームアセット - 監視 ✅
Cargo.toml             ← 依存関係設定 - 監視 ✅
```

## Bevy開発での推奨ワークフロー

### 1. 開発時のビルド

```powershell
# 自動クリーンアップ付きでビルド
.\scripts\build.ps1

# または通常のcargo build
cargo build
```

### 2. リリースビルド

```powershell
.\scripts\build.ps1 -Release
```

### 3. 定期的なクリーンアップ

```powershell
# 週に1回程度
.\scripts\prevent-bloat.ps1
```

## Bevyプロジェクトでの効果

### Before（最適化前）
- ❌ Bevyの依存関係（数百ファイル）を監視
- ❌ ビルドのたびに大量のファイル変更イベント
- ❌ メモリ使用量が増加（Bevyは大きいため特に顕著）
- ❌ エージェントが頻繁にクラッシュ

### After（最適化後）
- ✅ ソースコードとアセットのみを監視
- ✅ Bevyの依存関係は監視しない（保護）
- ✅ メモリ使用量が削減
- ✅ エージェントが安定

## Bevy特有の注意事項

### 1. 依存関係の再コンパイル

⚠️ **`target/debug/deps/`を削除しないでください**

Bevyの依存関係を再コンパイルすると：
- 初回ビルド: **10-30分**かかる場合がある
- メモリ使用量: **数GB**を使用
- CPU使用率: **100%**に達する

### 2. 動的リンクの使用

`dynamic_linking`を使用している場合：
- `bevy_dylib.dll`が`target/debug/`に生成される
- このファイルも監視から除外されている
- ビルド時の更新を監視しない

### 3. WebAssemblyビルド

Trunkを使用してWebAssemblyにビルドする場合：
- `dist/`ディレクトリに成果物が生成される
- これも監視から除外されている
- `trunk serve`時のパフォーマンスも最適化

## 確認方法

Bevyプロジェクトでの設定を確認：

```powershell
# ファイルウォッチャー設定
.\scripts\check-watchers.ps1

# Rust Analyzer設定
.\scripts\check-rust-analyzer.ps1

# サイズ確認
.\scripts\check-size.ps1
```

## まとめ

✅ **現在の設定はBevyプロジェクトに最適化されています**

- Bevyの依存関係を保護
- ビルド成果物を監視しない
- ソースコードとアセットのみを監視
- エージェントのクラッシュを防止

**追加の設定変更は不要です。** 現在の構成のまま、Bevyプロジェクトを効率的に開発できます。



