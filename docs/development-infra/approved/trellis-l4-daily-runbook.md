# TRELLIS.2（trellis-l4）日常運用 Runbook

VM は**作業時のみ起動**する前提。初回セットアップが済んでいる場合の、再現しやすいフローです。

---

## 前提（初回のみ済ませておくこと）

- [phase-c-d-next-steps.md](./phase-c-d-next-steps.md) に従い、以下が完了していること:
  - GCP プロジェクト・gcloud 認証・IAP ファイアウォール・IAM
  - **trellis-l4**（L4 Spot, asia-east1-a, パブリック IP なし）
  - Cloud NAT（asia-east1）
  - VM 内: NVIDIA ドライバ 535、Docker、NVIDIA Container Toolkit、apt IPv4 強制
  - ローカル `~/.ssh/config` に **gcp-trellis-l4**（3000/8000/9000 フォワード済み）

---

## 作業開始フロー（VM を起こして生成まで）

### 1. VM を起動する（ローカル・Fedora）

```bash
gcloud compute instances start trellis-l4 --zone=asia-east1-a
```

**1〜2 分**待つ（ステータスが RUNNING になるまで）。  
コンテナに `--restart=unless-stopped` を付けていれば、**VM 起動後に Docker が自動で tostui を起動**します。

### 2. SSH で接続する（ポートフォワード付き）

```bash
ssh gcp-trellis-l4
```

接続したままにしておく（ブラウザ用に 3000/8000/9000 がフォワードされる）。

### 3. コンテナを起動する（VM 内で 1 回）

コンテナがまだ無い、または停止している場合:

```bash
# 既存コンテナがあるか確認
docker ps -a | grep tostui

# 無い場合: 新規作成（バックグラウンド）。--restart=unless-stopped で VM 起動時に自動起動。
docker run -d --restart=unless-stopped --gpus all -p 3000:3000 -p 8000:8000 -p 9000:9000 \
  -v $(pwd)/outputs:/output \
  -e PYTORCH_CUDA_ALLOC_CONF="expandable_segments:True" \
  --name tostui \
  camenduru/tostui-trellis2

# ある場合: 起動だけ（すでに --restart 付きなら VM 起動時に自動起動済み）
docker start tostui
```

**既存の tostui を「VM 起動時に自動起動」にしたい場合**（1 回だけ）:

```bash
docker update --restart=unless-stopped tostui
```

**2〜3 分**待つ（ログで `Uvicorn running on http://0.0.0.0:8000` が出るまで）。

### 4. ブラウザで TostUI を開く（ローカル）

SSH 接続したまま、**Fedora のブラウザ**で:

- **TostUI**: http://localhost:3000  
- 画像をアップロードして「生成」を実行

解像度は **高くしすぎると CuMesh で OOM** になることがあるので、**512** など低めから試す。

### 5. 出力 GLB の取り出し

- **UI のダウンロードリンク**が出たらそのまま保存。
- または VM 内で MinIO からマウント先へコピーしてから scp:

```bash
# VM 内
docker exec tostui mc ls local/tost/output/
docker exec tostui mc cp --recursive local/tost/output/ /output/
ls ~/outputs/
```

```bash
# ローカル（別ターミナル）
scp gcp-trellis-l4:~/outputs/*.glb ./
```

---

## 作業終了フロー（VM を止める）

### 6. コンテナを止める（任意・VM 内）

```bash
docker stop tostui
```

### 7. VM を停止する（ローカル）

```bash
gcloud compute instances stop trellis-l4 --zone=asia-east1-a
```

Spot なので「停止」でインスタンスは残り、次回 `start` で再開できます。完全に削除する場合は `delete` を使用。

---

## コマンド一覧（コピペ用）

| 作業 | 場所 | コマンド |
|------|------|----------|
| VM 起動 | ローカル | `gcloud compute instances start trellis-l4 --zone=asia-east1-a` |
| SSH 接続 | ローカル | `ssh gcp-trellis-l4` |
| コンテナ起動 | VM 内 | `docker start tostui` または上記 `docker run ...`（`--restart=unless-stopped` で VM 起動時に自動起動） |
| ログ確認 | VM 内 | `docker logs -f tostui` |
| GLB を VM にコピー | VM 内 | `docker exec tostui mc cp --recursive local/tost/output/ /output/` |
| GLB をローカルに取得 | ローカル | `scp gcp-trellis-l4:~/outputs/*.glb ./` |
| コンテナ停止 | VM 内 | `docker stop tostui` |
| VM 停止 | ローカル | `gcloud compute instances stop trellis-l4 --zone=asia-east1-a` |

---

## トラブルシュート簡易メモ

| 現象 | 対処 |
|------|------|
| apt / 接続タイムアウト | Cloud NAT が有効か確認。IPv4 強制: `echo 'Acquire::ForceIPv4 "true";' \| sudo tee /etc/apt/apt.conf.d/99force-ipv4` |
| Failed to connect to local API | API が 8000 で立ち上がるまで 2〜3 分待つ。`docker logs tostui` で `Uvicorn running on http://0.0.0.0:8000` を確認。 |
| ECONNREFUSED 127.0.0.1:8000 | SSH で 8000 をフォワードしているか確認。`~/.ssh/config` に `LocalForward 8000 localhost:8000` があること。 |
| CuMesh out of memory | 解像度を下げる（512 等）。必要なら `docker restart tostui` で VRAM を空けてから再生成。 |
| 出力 GLB が無い | `docker exec tostui mc ls local/tost/output/` で MinIO を確認。あれば `mc cp ... /output/` でコピー。 |
| SSH が遅い / タイムアウト | コンテナが重いと VM が応答しにくい。`docker stop tostui` してから SSH し直す。 |
| Spot で VM が消えた | `gcloud compute instances list` で確認。無ければ [phase-c-d-next-steps.md](./phase-c-d-next-steps.md) の Step 1 で再作成（NAT・ドライバ・Docker は再設定が必要）。 |

---

## 参照

- 初回セットアップ・詳細: [phase-c-d-next-steps.md](./phase-c-d-next-steps.md)
- L4 が使えない場合: [gpu-alternatives-l4.md](./gpu-alternatives-l4.md)
- 実装チェックリスト: [implementation-checklist.md](./implementation-checklist.md)
