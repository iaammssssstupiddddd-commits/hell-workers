#!/bin/bash
# Git認証情報更新スクリプト（Personal Access Token使用）

echo "=== Git認証情報更新スクリプト ==="
echo ""
echo "このスクリプトは、GitHub Personal Access Token (PAT) を使用して"
echo "Git認証情報を更新します。"
echo ""
echo "Personal Access Tokenの作成方法:"
echo "1. GitHubにログイン"
echo "2. Settings > Developer settings > Personal access tokens > Tokens (classic)"
echo "3. \"Generate new token (classic)\" をクリック"
echo "4. 必要な権限を選択（最低限: repo）"
echo "5. トークンを生成してコピー"
echo ""

read -p "GitHub Personal Access Tokenを入力してください: " token

if [ -z "$token" ]; then
    echo "エラー: トークンが入力されていません。"
    exit 1
fi

# Git credential helperを設定（まだ設定されていない場合）
if ! git config --global credential.helper &> /dev/null; then
    echo ""
    echo "Git credential helperを設定します..."
    git config --global credential.helper store
fi

# リモートURLを確認
echo ""
echo "現在のリモートURL:"
git remote -v

echo ""
read -p "リモートURLを更新しますか？ (y/n): " update_url

if [ "$update_url" = "y" ]; then
    # HTTPS URLに変更（トークンを含む）
    repo_url=$(git remote get-url origin)
    # 既存のトークン部分を削除
    clean_url=$(echo "$repo_url" | sed 's|https://[^@]*@|https://|')
    # 新しいトークンを含むURLに更新
    new_url="https://${token}@${clean_url#https://}"
    git remote set-url origin "$new_url"
    echo "リモートURLを更新しました。"
fi

# テスト: 認証が成功するか確認
echo ""
echo "認証をテストしています..."
if git ls-remote origin HEAD &> /dev/null; then
    echo "✅ 認証成功！"
else
    echo "❌ 認証に失敗しました。トークンを確認してください。"
    exit 1
fi

echo ""
echo "=== 完了 ==="
echo "Git認証情報が更新されました。"

