# GitHub 認証ガイド

このリポジトリでは、token を remote URL、`.env`、またはGit credentialの平文
保存helperに保存しない。ブラウザ認証を使う GitHub CLI、または
SSH を利用する。

## GitHub CLI（推奨）

GitHub CLI を公式パッケージでインストールした後、次を実行する。

```bash
gh auth login
gh auth setup-git
gh auth status
```

リポジトリ付属の薄いラッパーも利用できる。

```bash
./scripts/update-github-token.sh
```

`gh auth login` がOSのcredential storeを選べる環境では、その選択に従う。
CIではGitHub Actionsが提供する短命な `GITHUB_TOKEN` をworkflow内で参照し、
値をファイルやremote URLへ書き込まない。

## SSH

既にGitHubへ公開鍵を登録済みなら、remoteをSSH形式へ変更できる。

```bash
git remote set-url origin git@github.com:<owner>/<repository>.git
ssh -T git@github.com
```

## 確認

```bash
git remote -v
git ls-remote origin HEAD
gh auth status
```

remote URLにcredentialらしき文字列が含まれていた場合は、安全なURLへ戻し、
漏れたtokenをGitHub上で失効させる。
