#!/bin/bash
# GitHubトークン更新スクリプト

echo "=== GitHubトークン更新スクリプト ==="
echo ""

# GitHub CLIがインストールされているか確認
if ! command -v gh &> /dev/null; then
    echo "GitHub CLI (gh) がインストールされていません。"
    echo ""
    echo "インストール方法:"
    echo "  sudo apt update && sudo apt install -y gh"
    echo ""
    echo "または、以下のコマンドでインストールできます:"
    echo "  curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | sudo dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg"
    echo "  echo \"deb [arch=\$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main\" | sudo tee /etc/apt/sources.list.d/github-cli.list > /dev/null"
    echo "  sudo apt update && sudo apt install -y gh"
    echo ""
    read -p "GitHub CLIをインストールしますか？ (y/n): " install_gh
    if [ "$install_gh" = "y" ]; then
        echo "インストールコマンドを実行してください（sudoパスワードが必要です）"
        exit 1
    fi
fi

# GitHub CLIで認証
echo "GitHub CLIで認証を開始します..."
echo "ブラウザが開き、GitHubで認証を完了してください。"
gh auth login

# 認証状態を確認
echo ""
echo "=== 認証状態の確認 ==="
gh auth status

echo ""
echo "=== 完了 ==="
echo "GitHubトークンが更新されました。"

