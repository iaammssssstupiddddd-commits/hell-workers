# Linux 開発環境セットアップガイド

Linux ネイティブ環境で `Hell Workers` をビルド・実行するための手順です。

## 1. 依存パッケージのインストール

Bevy 0.18 のビルドには、以下のシステムライブラリが必要です。Ubuntu/Debian 系のディストリビューションを使用している場合は、以下のコマンドを実行してください。

```bash
sudo apt-get update
sudo apt-get install -y \
    g++ \
    pkg-config \
    libx11-dev \
    libasound2-dev \
    libudev-dev \
    libxkbcommon-dev \
    libwayland-dev \
    libvulkan-dev
```

## 2. 高速リンカの導入 (推奨)

Linux での Rust のリンク時間は非常に長くなることがありますが、`mold` や `lld` を使用することで大幅に短縮できます。

### mold のインストール
```bash
sudo apt-get install -y mold
```

## 3. Rust/Cargo の設定

`.cargo/config.toml` を更新することで、Linux ネイティブビルドがデフォルトになります。

### ビルドと実行
```bash
# 通常の実行
cargo run

# 動的リンクを有効にして高速にビルド・実行
cargo run --features dynamic_linking
```

## 4. トラブルシューティング

### Vulkan ドライバー
Vulkan が正しく動作しない場合は、グラフィックスドライバー（NVIDIA/AMD/Intel）が最新であることを確認してください。

### オーディオ
オーディオの初期化に失敗する場合は、`libasound2-dev` がインストールされていることを確認してください。
