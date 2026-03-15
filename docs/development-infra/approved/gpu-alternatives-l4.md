# L4 が枯渇しているときの代替 GPU 候補

TRELLIS 2 は **24GB VRAM** を推奨（FP16 最適化でやや下回る運用も可能）。L4 が使えない場合の候補を優先度順にまとめます。

---

## 候補一覧

| 優先 | GPU | VRAM | マシンタイプ | 備考 |
|------|-----|------|--------------|------|
| **1** | **A100 40GB** | 40GB | `a2-highgpu-1g` | 24GB を十分満たす。L4 より高単価だが枯渇しにくいことが多い。Spot 対応リージョンあり。 |
| **2** | **V100 ×2** | 32GB | N1 + 2 GPU | 2 枚で 32GB。L4 より古いが推論は可能。ゾーン限定。 |
| **3** | **T4** | 16GB | N1 + T4 | 16GB のため解像度を下げるか FP16+空きキャッシュ最適化が必要。OOM リスクあり。 |
| **4** | **G4 (RTX PRO 6000)** | 96GB | `g4-standard-48` | 96GB で余裕あり。単価・提供ゾーン要確認。 |

---

## 1. A100 40GB（推奨）

**1 枚で 40GB** あるため、TRELLIS 2 の 24GB 要件を満たします。

```bash
gcloud config set compute/zone asia-southeast1-a   # または A100 が使えるゾーン

gcloud compute instances create trellis-a100 \
  --no-address \
  --machine-type=a2-highgpu-1g \
  --accelerator=type=nvidia-tesla-a100,count=1 \
  --provisioning-model=SPOT \
  --image-family=ubuntu-2204-lts \
  --image-project=ubuntu-os-cloud \
  --maintenance-policy=TERMINATE \
  --boot-disk-size=200GB
```

- **注意**: A2 が Spot に対応しているかはリージョンにより異なります。Spot 不可の場合は `--provisioning-model=SPOT` を外すとオンデマンドになります。
- **ゾーン**: [GPU のリージョン一覧](https://cloud.google.com/compute/docs/gpus/gpu-regions-zones) で `a2-highgpu-1g` または `nvidia-tesla-a100` が利用可能なゾーンを確認してください（例: asia-northeast1-a, us-central1-a など）。

---

## 2. V100 ×2（32GB）

**2 枚で 32GB**。24GB を満たします。N1 に V100 を 2 枚付ける形です。

```bash
gcloud compute instances create trellis-v100 \
  --no-address \
  --machine-type=n1-standard-16 \
  --accelerator=type=nvidia-tesla-v100,count=2 \
  --provisioning-model=SPOT \
  --image-family=ubuntu-2204-lts \
  --image-project=ubuntu-os-cloud \
  --maintenance-policy=TERMINATE \
  --boot-disk-size=200GB
```

- TRELLIS 2 が 1 GPU 前提なら、環境変数 `CUDA_VISIBLE_DEVICES=0` で 1 枚だけ使う運用にするとよいです。
- **ゾーン**: V100 は提供ゾーンが限られます。[GPU リージョン一覧](https://cloud.google.com/compute/docs/gpus/gpu-regions-zones) で `nvidia-tesla-v100` を確認してください。

---

## 3. T4（16GB）— 要・解像度／メモリ最適化

**16GB** のため、最大解像度では OOM になりやすいです。解像度を下げる・FP16・`PYTORCH_CUDA_ALLOC_CONF="expandable_segments:True"` などを組み合わせれば動かせる可能性はあります。

```bash
gcloud compute instances create trellis-t4 \
  --no-address \
  --machine-type=n1-standard-8 \
  --accelerator=type=nvidia-tesla-t4,count=1 \
  --provisioning-model=SPOT \
  --image-family=ubuntu-2204-lts \
  --image-project=ubuntu-os-cloud \
  --maintenance-policy=TERMINATE \
  --boot-disk-size=200GB
```

- コミュニティでは 8GB で動かした例もあるため、試す価値はありますが、安定性は L4/A100 より落ちます。

---

## 4. G4（RTX PRO 6000）— 96GB

**96GB** と十分な VRAM。提供ゾーン・価格は要確認です。

```bash
gcloud compute instances create trellis-g4 \
  --no-address \
  --machine-type=g4-standard-48 \
  --provisioning-model=SPOT \
  --image-family=ubuntu-2204-lts \
  --image-project=ubuntu-os-cloud \
  --maintenance-policy=TERMINATE \
  --boot-disk-size=200GB
```

- G4 は GPU がマシンに固定のため `--accelerator` は不要です。

---

## ゾーン確認のしかた

1. [GPU のリージョンとゾーン](https://cloud.google.com/compute/docs/gpus/gpu-regions-zones) で、使いたい GPU（例: A100, V100, T4）の「利用可能なリージョン/ゾーン」を確認する。
2. ホーチミンから遅延を抑えたい場合は **asia-southeast1**（シンガポール）や **asia-northeast1**（東京）を優先し、その中で該当 GPU が有効なゾーンを選ぶ。
3. コマンド実行時に「このゾーンではその GPU が使えません」と出た場合は、上記ページで別ゾーンを選び直す。

---

## SSH config の Host 名

上記で VM 名を `trellis-a100` / `trellis-v100` / `trellis-t4` などにしている場合、`~/.ssh/config` の `HostName` と `ProxyCommand` 内のインスタンス名を、作成した VM 名に合わせてください（[phase-c-d-next-steps.md](./phase-c-d-next-steps.md) の Step 2 を参照）。
