# エンティティリストUI仕様

## 概要
画面左側のパネルで、使い魔とソウルの一覧を表示します。  
UIは `EntityListViewModel` を経由して 100ms 間隔で同期されます。

## 構成

### 使い魔セクション
- ヘッダー左: 折りたたみボタン（▼/▶）
- ヘッダー右: 使い魔選択ボタン
- 表示形式: `{名前} ({現在/最大}) [{AIステート}]`
- 展開時: 配下ソウルの行を表示

### ソウル行
- 性別アイコン（`male.png` / `female.png`）
- 名前（ストレス度で色変化）
- 疲労アイコン + 疲労%
- ストレスアイコン + ストレス%
- タスクアイコン（Idle/Chop/Mine/Haul/Build 等）

### 未所属ソウルセクション
- ヘッダー: `Unassigned Souls` + 折りたたみアイコン
- ヘッダー直下の領域のみスクロール対象
- `Scroll: Mouse Wheel` のヒントを表示

## インタラクション

### マウス
- 行ホバー: 背景色ハイライト
- 行クリック: 選択 + カメラフォーカス
- 選択中行: 左ボーダー + 選択色で強調
- 未所属ソウル領域でホイール: リストを縦スクロール

### キーボード
- `Tab`: 次の候補を選択
- `Shift + Tab`: 前の候補を選択

## 入力ガード
- `UiInputBlocker` と `UiInputState` により、UI上ホバー中のワールド操作を抑止
- スクロール領域上でのホイール入力はリスト優先（ワールドズーム抑止）

## 主な関連ファイル
- `src/interface/ui/list/view_model.rs` - ビューモデル構築
- `src/interface/ui/list/sync.rs` - ビューモデル同期
- `src/interface/ui/list/interaction.rs` - クリック/ホバー/スクロール/Tabフォーカス
- `src/interface/ui/setup/entity_list.rs` - パネル初期生成
- `src/interface/ui/components.rs` - UIコンポーネント定義
- `src/interface/selection.rs` - 選択状態管理
