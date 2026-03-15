# Phase A 詳細: gcloud 初期設定（Fedora）

いま gcloud のインストーラで「パス」などを聞かれている場合の、**プロンプトごとの答え**とその後の手順をまとめます。

---

## 1. インストーラで聞かれる項目と推奨の答え

`curl https://sdk.cloud.google.com | bash` を実行すると、おおよそ次の順で聞かれます。

### ① インストール先ディレクトリ（「パス」と表示されることが多い）

- **表示例**: 「インストール先を指定」「Path to install」「Choose a location for the google-cloud-sdk subdirectory」など。
- **意味**: `google-cloud-sdk` フォルダをどの親ディレクトリに作るか。
- **推奨**: **そのまま Enter**（デフォルト = ホームディレクトリ = `$HOME`）。
  - 結果として `~/google-cloud-sdk` にインストールされます。
  - 特別な理由がなければデフォルトで問題ありません。

### ② PATH への追加

- **表示例**: 「Add the gcloud CLI to your PATH?」「Enable command completion?」など。
- **推奨**: **Y**（Yes）。
  - これで `gcloud` をどのディレクトリからでも使えるようになり、タブ補完も有効になります。
  - 設定は `~/.bashrc`（または使用中のシェルの設定ファイル）に追記されます。

### ③ 利用統計の送信（オプション）

- **表示例**: 「Send anonymous usage statistics?」など。
- **推奨**: 好みで **Y** または **N**。どちらでも動作には影響しません。

---

## 2. インストール直後にやること

シェルに PATH を反映させます。

```bash
exec -l $SHELL
```

または

```bash
source ~/.bashrc
```

その後、次でバージョンが表示されれば OK です。

```bash
gcloud version
```

---

## 3. 認証とプロジェクト設定（A1 の続き）

### ログイン（OAuth 2.0）

```bash
gcloud auth login
```

- ブラウザが開くので、使う Google アカウントでログインし、「許可」まで進めます。
- コンソールに「You are now logged in.」と出れば成功です。

### デフォルトプロジェクトの設定

```bash
gcloud config set project YOUR_PROJECT_ID
```

- **YOUR_PROJECT_ID**: GCP のプロジェクト ID（コンソールの「プロジェクトを選択」で表示される ID）。
- プロジェクト ID がわからない場合:
  ```bash
  gcloud projects list
  ```
  で一覧表示されます。

### 設定確認

```bash
gcloud config list
```

- `project` が意図したプロジェクト ID になっていれば A1 は完了です。

---

## 4. 非対話インストール（やり直す場合・自動化用）

「パス」などの質問を出さずにインストールする例です。

```bash
curl https://sdk.cloud.google.com > /tmp/install.sh
bash /tmp/install.sh --disable-prompts
```

- インストール先はデフォルトで `$HOME`（= `~/google-cloud-sdk`）です。
- 別の場所にしたい場合:
  ```bash
  bash /tmp/install.sh --disable-prompts --install-dir=/home/あなたのユーザー名
  ```
  この場合、gcloud は `/home/あなたのユーザー名/google-cloud-sdk` に入ります。

インストール後は同じく:

```bash
source ~/.bashrc   # または exec -l $SHELL
gcloud version
```

---

## 5. この後の Phase A（A2・A3）— 概要

- **A2**: IAP で SSH するための、ファイアウォール・IAM・`gcloud compute ssh --tunnel-through-iap` の設定。
- **A3**: `~/.ssh/config` に IAP 用エントリを追加し、VS Code の Remote-SSH で接続。

---

## 6. A1 完了後の次: A2・A3 の具体的な手順

プロジェクト設定までできたら、**IAP 経由で VM に SSH できるようにする**のが次の目標です。

### 6.1 VM の有無を確認

```bash
gcloud compute instances list
```

- **一覧に VM がある場合** → 6.2 へ（IAP 用のファイアウォールと IAM を設定してから接続テスト）。
- **VM がない場合** → まず 1 台作成する（6.1 の下記「VM を新規作成する場合」を実行してから 6.2 へ）。

#### VM を新規作成する場合（オプション）

IAP 用に**パブリック IP なし**で作成する例です。ゾーンとマシンタイプは必要に応じて変更してください。

```bash
# ゾーンを指定（例: us-central1-a）
gcloud config set compute/zone us-central1-a

# 小さい VM で IAP SSH の動作確認用（L4 は後で Phase C で作成してよい）
gcloud compute instances create trellis-dev \
  --no-address \
  --machine-type=e2-medium \
  --image-family=ubuntu-2204-lts \
  --image-project=ubuntu-os-cloud
```

- `--no-address`: パブリック IP を付けない（IAP 経由でのみ接続）。
- 本番の L4 GPU は Phase C で Spot として別途作成します。

### 6.2 ファイアウォール: IAP 用ルールを作成

IAP が使う Google の IP レンジ（`35.235.240.0/20`）から TCP 22 を許可するルールを追加します。

```bash
gcloud compute firewall-rules create allow-iap-ssh \
  --direction=INGRESS \
  --priority=1000 \
  --network=default \
  --action=ALLOW \
  --rules=tcp:22 \
  --source-ranges=35.235.240.0/20
```

- 既に同名のルールがある場合は `create` が失敗するので、そのときは「ルールは既に存在する」と解釈して 6.3 へ進んでください。

### 6.3 IAM: IAP トンネル権限を付与

**自分（現在の gcloud のログインアカウント）**に、IAP で VM に接続する権限を付けます。

```bash
# 自分のメールを確認（gcloud auth list の ACTIVE の行）
gcloud auth list

# プロジェクト単位で IAP トンネルユーザーを付与（YOUR_EMAIL を上で表示されたメールに置き換え）
# 注意: member は必ず "user:メール" の形式。メールだけではエラーになる。
gcloud projects add-iam-policy-binding $(gcloud config get-value project) \
  --member="user:YOUR_EMAIL" \
  --role="roles/iap.tunnelResourceAccessor"
```

- 例: `--member="user:satotakumi@gmail.com"`（**user:** を付け忘れないこと）

### 6.4 接続テスト（A2 の確認）

VM 名とゾーンを指定して、IAP 経由で SSH します。

```bash
gcloud compute ssh VM名 --tunnel-through-iap --zone=ゾーン名
```

- 例: VM 名が `trellis-dev`、ゾーンが `us-central1-a` の場合  
  `gcloud compute ssh trellis-dev --tunnel-through-iap --zone=us-central1-a`
- 初回は SSH 鍵の作成を聞かれるので **Y**。鍵は `~/.ssh/google_compute_engine` に保存されます。
- ログインできたら A2 は完了です。`exit` で抜けてください。

### 6.5 SSH Config を追加（A3）

`gcloud compute ssh` の代わりに、通常の `ssh` や VS Code から接続できるようにします。

1. `~/.ssh/config` を開く（なければ作成）。
2. 以下を追加（**VM 名・ゾーン・プロジェクト ID・ユーザー名**を実際の値に置き換え）。

```ini
Host gcp-trellis
  HostName VM名
  User YOUR_GCP_OS_LOGIN_USER
  IdentityFile ~/.ssh/google_compute_engine
  ProxyCommand gcloud compute start-iap-tunnel VM名 22 --listen-on-stdin --zone=ゾーン名 --project=プロジェクトID
```

- **HostName** と **ProxyCommand 内の VM 名**は、`gcloud compute instances list` の NAME と同じにします。
- **User**: OS Login を使う場合は `gcloud auth list` のメール（例: `satotakumi@gmail.com`）。デフォルトの Debian/Ubuntu イメージでは多くの場合 `username` やメール形式です。接続エラーで「Permission denied」になる場合は、VM 上で `whoami` で確認したユーザー名に合わせてください。
- Gradio をローカルから見る場合は、次の 1 行を追加:  
  `LocalForward 8080 localhost:7860`

**例**（VM 名 `trellis-dev`、ゾーン `us-central1-a`、プロジェクト `my-project-123`、ユーザー `satotakumi@gmail.com`）:

```ini
Host gcp-trellis
  HostName trellis-dev
  User satotakumi@gmail.com
  IdentityFile ~/.ssh/google_compute_engine
  ProxyCommand gcloud compute start-iap-tunnel trellis-dev 22 --listen-on-stdin --zone=us-central1-a --project=my-project-123
```

3. 接続テスト:

```bash
ssh gcp-trellis
```

### 6.6 VS Code でリモート接続（A3 の確認）

1. VS Code で拡張機能 **Remote - SSH** を入れておく。
2. `F1` または `Ctrl+Shift+P` → 「Remote-SSH: Connect to Host」を選択。
3. 一覧から **gcp-trellis** を選ぶ（またはホスト名を入力）。
4. 新しいウィンドウで VM 上に接続されれば A3 完了です。

---

## まとめ

- **次にやること**: 6.1 で VM を確認 → 6.2 ファイアウォール → 6.3 IAM → 6.4 で `gcloud compute ssh --tunnel-through-iap` を試す → 6.5 で `~/.ssh/config` を書く → 6.6 で VS Code から `ssh gcp-trellis` 接続。
- より詳しい設定例は [config-templates.md](./config-templates.md) を参照。
