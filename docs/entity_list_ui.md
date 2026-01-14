# エンティティリストUI仕様

## 概要
画面左側に表示されるパネルで、使い魔とソウルの一覧を表示します。

## 構成

### 使い魔セクション
各使い魔ごとにヘッダーと配下のソウルリストが表示されます。

**ヘッダー構成:**
- **折りたたみアイコン** (左): クリックでセクションを展開/折りたたみ
  - ▼ (`arrow_down.jpg`): 展開状態
  - ▶ (`arrow_right.jpg`): 折りたたみ状態
- **名前ボタン** (右): クリックで使い魔を選択し、カメラをフォーカス
  - 表示: `{名前} ({現在/最大}) [{AIステート}]`

**ソウルリストアイテム:**
- 性別アイコン (`male.jpg` / `female.jpg`)
- 名前
- 疲労アイコン (`fatigue.jpg`) + パーセンテージ
- ストレスアイコン (`stress.jpg`) + パーセンテージ
- タスクアイコン (`idle.jpg` / `pick.jpg` / `haul.jpg`)

### 未所属ソウルセクション
使い魔に配属されていないソウルの一覧です。

**ヘッダー:** 「Unassigned Souls」+ 折りたたみアイコン

## アセット
すべてのアイコンは `assets/textures/ui/` に配置（JPEG形式）:
- `male.jpg`, `female.jpg` - 性別
- `fatigue.jpg`, `stress.jpg` - ステータス
- `idle.jpg`, `pick.jpg`, `haul.jpg` - タスク状態
- `arrow_down.jpg`, `arrow_right.jpg` - 折りたたみ

## 更新頻度
100msごとにリストが再構築されます（`interface.rs` の `on_timer`）。

## 関連ファイル
- `src/interface/ui/list.rs` - リスト構築・インタラクション
- `src/interface/ui/setup/entity_list.rs` - 初期UIセットアップ
- `src/interface/ui/components.rs` - UIコンポーネント定義
