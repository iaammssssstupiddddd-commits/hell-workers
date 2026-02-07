# 情報パネルUI仕様

## 概要
画面右側に表示されるパネルで、`SelectedEntity` の詳細情報を表示します。  
未選択時は非表示です。

## 表示対象

### ソウル選択時
- ヘッダー: 名前（未設定時は `Damned Soul`）
- 性別アイコン: `male.png` / `female.png`
- ステータス:
  - `Motivation: xx%`
  - `Stress: xx%`
  - `Fatigue: xx%`
- タスク: `Task: ...`
- 所持品: `Carrying: ...`
- 補助情報: Idle状態・Escape判定のデバッグ情報

### 使い魔選択時
- ヘッダー: 使い魔名
- 共通テキスト: タイプ、指揮半径、疲労閾値

### その他のエンティティ
- Blueprint: 種類、進捗
- Resource Item: 種類
- Tree/Rock: 自然資源情報

## 仕様メモ
- 情報パネルの背景・テキスト色・サイズは `theme.rs` の定数を使用
- ステータス表示は内部的に `SoulStatDisplay` を経由して整形し、将来のバー表示追加に備える

## 関連ファイル
- `src/interface/ui/panels/info_panel.rs` - 情報更新ロジック
- `src/interface/ui/setup/panels.rs` - 初期UI構築
- `src/interface/ui/components.rs` - 情報パネル用コンポーネント
