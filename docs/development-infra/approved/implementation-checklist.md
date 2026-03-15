# GCP L4 TRELLIS 活用 — 実装チェックリスト

本チェックリストは [FedoraローカルからGCP L4 TRELLIS活用.md](./FedoraローカルからGCP%20L4%20TRELLIS活用.md) に沿って、パイプラインを段階的に構築するための実行用メモです。

---

## Phase A: Fedora ローカル環境と GCP 接続

> **詳細**: gcloud のインストールで「パス」などを聞かれたときの答え方は [phase-a-gcloud-setup.md](./phase-a-gcloud-setup.md) を参照。

### A1. Google Cloud CLI の導入

- [ ] 公式スクリプトでサンドボックスインストール（dnf は使わない）
  ```bash
  curl https://sdk.cloud.google.com | bash
  exec -l $SHELL   # または source ~/.bashrc
  ```
- [ ] 認証とプロジェクト設定
  ```bash
  gcloud auth login
  gcloud config set project YOUR_PROJECT_ID
  ```
- [ ] `gcloud version` で動作確認

### A2. IAP による SSH（パブリック IP なし）

- [ ] 対象 VM は **パブリック IP なし** で作成（`--no-address`）
- [ ] ファイアウォール: 送信元 `35.235.240.0/20` から TCP 22（SSH）を許可
- [ ] IAM: 自分のアカウントに **IAP-Secured Tunnel User**（`roles/iap.tunnelResourceAccessor`）を付与
- [ ] 接続テスト
  ```bash
  gcloud compute ssh INSTANCE_NAME --tunnel-through-iap --zone=ZONE
  ```

### A3. VS Code Remote-SSH + SSH Config

- [ ] `~/.ssh/config` に IAP 経由のエントリを追加（下記テンプレート参照）
- [ ] VS Code で「Remote-SSH: Connect to Host」→ 設定した Host 名で接続
- [ ] （必要なら）Gradio 用ポートフォワード: `-L 8080:localhost:7860`

---

## Phase B: Podman / SELinux

### B1. ボリュームマウント

- [ ] **共有ディレクトリ**（入力画像・複数コンテナで使う中間データ）→ `-v /host/path:/container/path:z`
- [ ] **TRELLIS 専用**（キャッシュ・一時ファイル）→ `-v /host/path:/container/path:Z`
- [ ] `setenforce 0` は使わない

---

## Phase C: GCP L4 Spot インスタンス

### C1. インスタンス作成

- [ ] マシンタイプ: L4 GPU 1 枚（24GB VRAM）
- [ ] Spot（プリエンプティブル）で作成
- [ ] パブリック IP なし（IAP 前提）

### C2. プリエンプション対策

- [ ] メタデータでプリエンプション検知: `http://metadata.google.internal/computeMetadata/v1/instance/preempted`
- [ ] **シャットダウンスクリプト**を VM メタデータに登録し、終了前に GCS へチェックポイント同期（`gsutil cp` または GCS クライアント）
- [ ] バッチ再開用: 出力 GLB が既に GCS/ローカルに存在すれば **スキップ**（冪等）

### C3. 復旧（オプション・量産時）

- [ ] マネージドインスタンスグループ（MIG）で Spot を管理
- [ ] 起動スクリプトで GCS から最新チェックポイントを取得し、残タスクから再開

---

## Phase D: TRELLIS 2 推論パイプライン

### D1. VRAM 最適化

- [ ] モデル・演算を **FP16 または BF16** に
- [ ] 環境変数: `PYTORCH_CUDA_ALLOC_CONF="expandable_segments:True"`
- [ ] 1 アセット処理ごとに `torch.cuda.empty_cache()` を呼ぶ

### D2. バッチ再開ロジック

- [ ] 入力画像リストをループし、`[image_name].glb` が出力先に既にある場合はスキップ
- [ ] スキップ時はログに記録

---

## Phase E: Headless Blender（アートスタイル・Bevy 向け）

### E1. PBR → Unlit

- [ ] 全マテリアルの Principled BSDF を削除/切り離し
- [ ] 不透明: 画像テクスチャ → Background（または Emission 強さ 1）→ Material Output
- [ ] 透過あり: Mix Shader（Fac=アルファ）、Transparent BSDF + カラー → Material Output
- [ ] エクスポートで `KHR_materials_unlit` が付与されることを確認

### E2. ポスタライズ（手描き感）

- [ ] Compositor で Posterize（steps 例: 8）を適用
- [ ] Bake で新テクスチャに焼き直し → glTF エクスポート

### E3. 原点・スケール（world_lore / Bevy 用）

- [ ] 原点を **底面中心** に移動（頂点の min Z + XY 中心）
- [ ] 最大寸法で `TILE_SIZE` に合わせてスケール正規化

---

## Phase F: CI 品質ゲート

- [ ] Headless Blender 出力後に **gltf-validator** を実行
- [ ] ポリゴン数・UV・トポロジ・`KHR_materials_unlit` をチェック
- [ ] 失敗時は Reject し、ログに詳細を記録
- [ ] 合格 GLB のみ GCS の Production-Ready バケットへ

---

## Phase G: コスト・法務

- [ ] コスト式の確認: (L4 Spot 単価 × 処理秒数) + API 料金
- [ ] TRELLIS 2 は MIT 相当で商用・地域制限なしであることを確認
- [ ] 依存ライブラリの GPL 混入に注意

---

## 次のアクション

- **まず試す**: A1 → A2 → A3 で Fedora から IAP SSH と VS Code 接続まで完了させる。
- **次**: C1 で L4 Spot を 1 台立ち上げ、D1 まで（TRELLIS 2 コンテナ + VRAM 最適化）を検証。
- **その後**: E1〜E3 の Blender スクリプトと F の gltf-validator をパイプラインに組み込む。

設定テンプレートは [config-templates.md](./config-templates.md) を参照。
