# GCP L4 TRELLIS 活用 — 設定テンプレート

[実装チェックリスト](./implementation-checklist.md) で参照する設定のコピペ用です。`YOUR_*` を実際の値に置き換えてください。

---

## 1. SSH Config（IAP 経由・VS Code 用）

`~/.ssh/config` に追加。`gcloud compute ssh` で初回接続すると `~/.ssh/google_compute_engine` が作成されるので、そのパスを IdentityFile に指定する。

```ini
Host gcp-trellis
  User YOUR_GCP_OS_LOGIN_USER
  IdentityFile ~/.ssh/google_compute_engine
  ProxyCommand gcloud compute start-iap-tunnel %h 22 --listen-on-stdin --zone=YOUR_ZONE --project=YOUR_PROJECT_ID
  LocalForward 8080 localhost:7860
```

- **YOUR_ZONE**: 例 `us-central1-a`
- **YOUR_PROJECT_ID**: GCP プロジェクト ID
- **YOUR_GCP_OS_LOGIN_USER**: 例 `satotakumi@gmail.com`（OS Login のユーザー名）
- **LocalForward**: Gradio をローカル `http://localhost:8080` で見る場合のみ。不要なら行ごと削除してよい。

接続例:

```bash
ssh gcp-trellis
```

VS Code: 「Remote-SSH: Connect to Host」→ `gcp-trellis` を選択。

---

## 2. TRELLIS 2 推論用環境変数

コンテナやシェルで TRELLIS 2 を実行する前に設定。

```bash
export PYTORCH_CUDA_ALLOC_CONF="expandable_segments:True"
# 必要に応じて
# export CUDA_VISIBLE_DEVICES=0
```

---

## 3. Podman ボリュームマウント例

```bash
# 共有ディレクトリ（入力画像・中間データ）: :z
podman run -v /home/ubuntu/input:/workspace/input:z ...

# TRELLIS 専用キャッシュ: :Z
podman run -v /home/ubuntu/trellis_cache:/workspace/cache:Z ...
```

---

## 4. gcloud コマンド早見

```bash
# プロジェクト・ゾーン設定
gcloud config set project YOUR_PROJECT_ID
gcloud config set compute/zone YOUR_ZONE

# IAP 経由 SSH
gcloud compute ssh INSTANCE_NAME --tunnel-through-iap --zone=YOUR_ZONE

# IAP TCP トンネルのみ（別ツールで SSH する場合）
gcloud compute start-iap-tunnel INSTANCE_NAME 22 --local-host-port=localhost:2222 --zone=YOUR_ZONE
```

---

## 5. プリエンプション検知（VM 内）

シャットダウンスクリプトや監視スクリプトで使用。

```bash
curl -s -H "Metadata-Flavor: Google" \
  http://metadata.google.internal/computeMetadata/v1/instance/preempted
# TRUE ならまもなく終了
```

---

## 6. バッチスキップロジック（疑似コード）

```python
import os

OUTPUT_DIR = "/path/to/output"  # または GCS のローカルマウント

for image_path in input_list:
    base = os.path.splitext(os.path.basename(image_path))[0]
    glb_path = os.path.join(OUTPUT_DIR, f"{base}.glb")
    if os.path.exists(glb_path):
        print(f"Skip (exists): {glb_path}")
        continue
    # ここで TRELLIS 2 推論 + Blender 処理
```

---

## 7. 参照リンク（本ドキュメント本文より）

- [Connect to Linux VMs using IAP](https://docs.cloud.google.com/compute/docs/connect/ssh-using-iap)
- [Using IAP for TCP forwarding](https://docs.cloud.google.com/iap/docs/using-tcp-forwarding)
- [Create and use Spot VMs](https://docs.cloud.google.com/compute/docs/instances/create-use-spot)
- [Run shutdown scripts](https://docs.cloud.google.com/compute/docs/shutdownscript)
- [TRELLIS.2 (Microsoft)](https://microsoft.github.io/TRELLIS.2/)
- [glTF Validator](https://github.com/KhronosGroup/glTF-Validator)
