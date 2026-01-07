# GitHubトークン更新ガイド

このドキュメントでは、GitHubトークンを更新する方法を説明します。

## 方法1: GitHub CLIを使用（推奨）

GitHub CLIを使用すると、ブラウザ経由で簡単に認証できます。

### 手順

1. **GitHub CLIをインストール**（まだインストールしていない場合）
   ```bash
   sudo apt update && sudo apt install -y gh
   ```

2. **認証スクリプトを実行**
   ```bash
   ./scripts/update-github-token.sh
   ```

   または、直接コマンドを実行：
   ```bash
   gh auth login
   ```

3. **認証状態を確認**
   ```bash
   gh auth status
   ```

### メリット
- ブラウザ経由で簡単に認証できる
- トークンの管理が自動化される
- セキュリティが高い

---

## 方法2: Personal Access Token (PAT) を使用

GitHub Personal Access Tokenを使用してGit認証情報を更新します。

### 手順

1. **Personal Access Tokenを作成**
   - GitHubにログイン
   - Settings > Developer settings > Personal access tokens > Tokens (classic)
   - "Generate new token (classic)" をクリック
   - 必要な権限を選択（最低限: `repo`）
   - トークンを生成してコピー（一度しか表示されません）

2. **認証スクリプトを実行**
   ```bash
   ./scripts/update-git-credentials.sh
   ```

   または、手動で設定：
   ```bash
   # Git credential helperを設定
   git config --global credential.helper store
   
   # リモートURLを更新（トークンを含む）
   git remote set-url origin https://YOUR_TOKEN@github.com/USERNAME/REPO.git
   ```

### 注意事項
- トークンは機密情報です。Gitリポジトリにコミットしないでください
- トークンは定期的に更新することを推奨します

---

## 方法3: 環境変数に設定

CI/CD環境や特定のスクリプトで使用する場合、環境変数に設定できます。

### 手順

1. **`.env`ファイルを作成**（プロジェクトルートに）
   ```bash
   echo "GITHUB_TOKEN=your_token_here" > .env
   ```

2. **`.gitignore`に追加**（既に追加されている場合もあります）
   ```bash
   echo ".env" >> .gitignore
   ```

3. **環境変数を読み込む**
   ```bash
   source .env
   export GITHUB_TOKEN
   ```

### 使用例
```bash
# 環境変数を使用してGit操作
git push https://$GITHUB_TOKEN@github.com/USERNAME/REPO.git
```

---

## 認証の確認

どの方法を使用した場合でも、以下のコマンドで認証が成功しているか確認できます：

```bash
# リモートリポジトリへのアクセステスト
git ls-remote origin HEAD

# GitHub CLIを使用している場合
gh auth status
```

---

## トラブルシューティング

### 認証エラーが発生する場合

1. **トークンの有効期限を確認**
   - GitHubのSettings > Developer settings > Personal access tokensで確認

2. **トークンの権限を確認**
   - `repo`権限が必要です

3. **リモートURLを確認**
   ```bash
   git remote -v
   ```

4. **認証情報をクリアして再設定**
   ```bash
   # Git credential helperの認証情報をクリア
   git credential-cache exit  # キャッシュを使用している場合
   # または
   rm ~/.git-credentials  # storeを使用している場合
   ```

### GitHub CLIがインストールできない場合

WSL環境では、以下の方法でインストールできます：

```bash
# 方法1: apt経由
sudo apt update && sudo apt install -y gh

# 方法2: 公式リポジトリを追加
curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | sudo dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg
echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | sudo tee /etc/apt/sources.list.d/github-cli.list > /dev/null
sudo apt update && sudo apt install -y gh
```

---

## セキュリティのベストプラクティス

1. **トークンは定期的に更新する**
2. **最小限の権限のみを付与する**
3. **トークンをGitリポジトリにコミットしない**
4. **`.env`ファイルは`.gitignore`に追加する**
5. **不要になったトークンは削除する**

