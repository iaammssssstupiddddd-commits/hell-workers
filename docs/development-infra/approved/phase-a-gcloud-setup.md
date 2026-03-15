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

## 5. この後の Phase A（A2・A3）

- **A2**: IAP で SSH するための、ファイアウォール・IAM・`gcloud compute ssh --tunnel-through-iap` の設定（[実装チェックリスト](./implementation-checklist.md) の A2）。
- **A3**: `~/.ssh/config` に IAP 用エントリを追加し、VS Code の Remote-SSH で接続（[config-templates.md](./config-templates.md) の SSH Config をコピーして利用）。

gcloud の「パス」は **① インストール先は Enter でデフォルト、② PATH 追加は Y** で進めれば大丈夫です。
