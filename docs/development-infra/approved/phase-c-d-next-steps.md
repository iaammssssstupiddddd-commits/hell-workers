# Phase A 完了後の次の手順

Phase A（gcloud・IAP・SSH config・Cursor 接続）が完了したあとの進め方です。

---

## いまできていること

- Fedora から gcloud で GCP プロジェクトを操作できる
- パブリック IP なしの VM（trellis-dev）に IAP 経由で SSH できる
- Cursor の Remote-SSH で `gcp-trellis` に接続できる

---

## 次の目標（2 パターン）

### パターン 1: まず L4 GPU で TRELLIS 2 を動かす（推奨）

TRELLIS 2 は 24GB VRAM が必要なため、**L4 搭載の Spot VM を 1 台作り**、その上で推論まで検証します。

#### Step 1: L4 Spot VM を 1 台作成（Phase C1）

> **L4 が枯渇している場合**: [gpu-alternatives-l4.md](./gpu-alternatives-l4.md) に A100・V100・T4 などの代替 GPU とコマンド例をまとめています。

**ローカル（Fedora）のターミナル**で実行。プロジェクト・ゾーンは既に設定済みなら省略可。

```bash
# ゾーン（L4 が利用可能なゾーンを指定。例: us-central1-a）
gcloud config set compute/zone us-central1-a

# L4 GPU 1 枚・Spot・パブリック IP なし
gcloud compute instances create trellis-l4 \
  --no-address \
  --machine-type=g2-standard-8 \
  --accelerator=type=nvidia-l4,count=1 \
  --provisioning-model=SPOT \
  --image-family=ubuntu-2204-lts \
  --image-project=ubuntu-os-cloud \
  --maintenance-policy=TERMINATE \
  --boot-disk-size=200GB
```

- **--no-address**: IAP のみで接続（既存のファイアウォール・IAM がそのまま使える）
- **--boot-disk-size=200GB**: GCP は 200GB 未満で I/O 性能低下の警告を出す。PoC なら 100GB でも可だが、大量バッチや警告を避けたい場合は 200GB 推奨。
- **--provisioning-model=SPOT**: Spot 料金（プリエンプトあり）
- **--maintenance-policy=TERMINATE**: Spot 終了時にディスクは残す場合に合わせた設定。必要なら STOP にする場合は別オプションを確認

作成後、**同じ IAP 設定**で SSH できるので、`~/.ssh/config` に **trellis-l4 用の Host** を追加する（下記「Step 2」）。

#### Step 2: trellis-l4 用 SSH config を追加

`~/.ssh/config` に以下を追記（gcp-trellis の下でよい）。

```ini
Host gcp-trellis-l4
  HostName trellis-l4
  User satotakumi
  IdentityFile ~/.ssh/google_compute_engine
  ProxyCommand /home/satotakumi/google-cloud-sdk/bin/gcloud compute start-iap-tunnel trellis-l4 22 --listen-on-stdin --zone=asia-east1-a --project=hell-workers
  LocalForward 8080 localhost:7860
```
（VM を別ゾーンに作成した場合は `--zone=` をそのゾーンに合わせる）

接続確認:

```bash
ssh gcp-trellis-l4
```

Cursor からも「Remote-SSH: Connect to Host」→ **gcp-trellis-l4** で接続できます。

#### Step 2.5: apt が「connection timed out」になる場合（Cloud NAT）

VM に**パブリック IP を付けていない**（`--no-address`）場合、そのままではインターネットに出られず、`apt-get update` がタイムアウトします。**Cloud NAT** を有効にすると、VM に外部 IP を付けずに外向き通信だけ許可できます。

**ローカル（Fedora）のターミナル**で、VM がある**リージョン**（例: asia-east1）に対して 1 回だけ実行します。VPC が `default` の場合:

```bash
# リージョンは VM のゾーンに合わせる（asia-east1-a なら asia-east1）
gcloud compute routers create nat-router \
  --network=default \
  --region=asia-east1

gcloud compute routers nats create nat-config \
  --router=nat-router \
  --region=asia-east1 \
  --auto-allocate-nat-external-ips \
  --nat-all-subnet-ip-ranges
```

数分待ってから、VM 内で再度 `sudo apt-get update` を試してください。別リージョンに VM がある場合は、上記の `--region=` をそのリージョンに変更し、同じ VPC（通常は default）でルーターと NAT を作成します。

#### Step 3: VM 内で NVIDIA ドライバ・CUDA を入れる（初回のみ）

**gcp-trellis-l4 に SSH した状態**で、NVIDIA ドライバと CUDA をインストールします。

**apt が IPv6 で失敗する場合**（`Network is unreachable` で `2600:...` のようなアドレスが出る）は、先に IPv4 を強制してからインストールしてください。

```bash
# apt を IPv4 のみ使うようにする（GCP VM で IPv6 が無い場合の対処）
echo 'Acquire::ForceIPv4 "true";' | sudo tee /etc/apt/apt.conf.d/99force-ipv4

# 例: Ubuntu 22.04 の場合
sudo apt-get update && sudo apt-get install -y nvidia-driver-535
# 再起動が必要な場合
sudo reboot
```

再起動後、再度 SSH して:

```bash
nvidia-smi
```

で L4 が表示されれば OK です。

#### Step 4: TRELLIS 2 の環境を用意（Phase D）

**利用可能なコンテナイメージ（2026年3月時点）**

| イメージ | 内容 | 備考 |
|----------|------|------|
| **camenduru/tostui-trellis2** | TRELLIS.2（4B）＋ TostUI の Web UI | 約 28GB。RTX 3090/4090/5090 で動作報告あり。L4 24GB でも利用可。[Docker Hub](https://hub.docker.com/r/camenduru/tostui-trellis2) |
| **cassidybridges/trellis-box:latest** | TRELLIS ベース・FP16 最適化（VRAM 削減） | 8〜16GB VRAM 向け。TRELLIS.2 ではなく旧 TRELLIS 系の可能性あり。[Docker Hub](https://hub.docker.com/r/cassidybridges/trellis-box) |

**TRELLIS.2（4B）をコンテナで動かす場合の推奨**: `camenduru/tostui-trellis2` を使用。VM 内で:

```bash
# Docker の場合（NVIDIA Container Toolkit 導入済み前提）
docker pull camenduru/tostui-trellis2

# 実行例（GPU 渡し・ポート・ボリューム）
docker run --gpus all -it -p 8501:8501 \
  -v $(pwd)/outputs:/output \
  -e PYTORCH_CUDA_ALLOC_CONF="expandable_segments:True" \
  camenduru/tostui-trellis2
```

- Podman の場合は `--gpus all` の代わりに `--device nvidia.com/gpu=all` など、環境に合わせて [NVIDIA Container Toolkit 相当](https://docs.nvidia.com/datacenter/cloud-native/container-toolkit/) の設定を行う。
- ボリュームは必要に応じて `:z`（共有）または `:Z`（専用）を付与（[Phase B](implementation-checklist.md#phase-b-podman--selinux)）。
- **バッチ再開**: 出力 GLB が既にある場合はスキップするロジックをパイプラインに組み込む（[実装チェックリスト Phase D2](implementation-checklist.md#phase-d-trellis-2-推論パイプライン)）。

ここまでで「L4 上で TRELLIS 2 が 1 枚の画像から GLB を生成できる」状態を目指します。

#### Step 5 以降（Phase E・F）

- **Phase E**: Headless Blender で PBR→Unlit・ポスタライズ・原点正規化（Bevy / world_lore 用）。
- **Phase F**: gltf-validator で CI 品質ゲート。

これらは TRELLIS 2 の出力が安定してからパイプラインに組み込むとよいです。

---

### パターン 2: いまの trellis-dev で軽い準備だけする

L4 はまだ使わず、**trellis-dev（e2-medium）** のまま:

- Podman や Docker のインストール
- コンテナの `:z` / `:Z` マウントの確認
- （GPU なしなので TRELLIS 2 の推論はできないが）Blender headless や gltf-validator の導入だけ先に試す

という進め方もできます。TRELLIS 2 をすぐ試したいなら **パターン 1** を優先してください。

---

## チェックリストでの位置づけ

| Phase | 内容           | 次の手順での対応 |
|-------|----------------|------------------|
| **B** | Podman :z/:Z   | L4 VM でコンテナを動かすときに適用 |
| **C** | L4 Spot 作成   | Step 1〜2 で実施 |
| **D** | TRELLIS 2 推論 | Step 3〜4 で実施 |
| **E** | Blender 変換   | Step 5 以降      |
| **F** | gltf-validator | Step 5 以降      |

---

## まとめ

1. **次にやること**: ローカルで **L4 Spot VM（trellis-l4）を 1 台作成**（Step 1）。
2. **~/.ssh/config** に **gcp-trellis-l4** を追加し、`ssh gcp-trellis-l4` で接続（Step 2）。
3. VM 内で **NVIDIA ドライバ** を入れ、**TRELLIS 2** のコンテナまたは環境を用意して推論まで検証（Step 3〜4）。

L4 の作成でクォータエラーが出る場合は、GCP コンソールの「IAM と管理」→「クォータ」で「NVIDIA L4 GPU」の割り当てを確認してください。
