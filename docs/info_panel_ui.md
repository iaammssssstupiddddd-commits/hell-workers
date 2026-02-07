# 情報パネルUI仕様

## 概要
画面右側に表示されるパネルで、`SelectedEntity` の詳細情報を表示します。  
UIノードは Startup 時に常駐生成され、選択変更時は **差分更新のみ** を行います。  
未選択時は `display: none` で非表示にします。

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
- Building/Designation: 建物状態、担当者、発行者など（該当時）

## 実装アーキテクチャ
- UI更新は `UiNodeRegistry`（`UiSlot -> Entity`）経由で行う
- `UiSlot` の全走査は行わず、`Query::get_mut(entity)` で直接更新する
- 表示データは `presentation` 層で組み立てる
  - `EntityInspectionModel` を生成
  - InfoPanel は描画責務に限定

## 仕様メモ
- 情報パネルの背景・テキスト色・サイズは `theme.rs` の定数を使用
- ルートノードは `UiRoot` 配下の `RightPanel` スロットにマウントされる
- ソウル向けセクション (`StatMotivation/Stress/Fatigue/Task/Inventory`) は対象がソウルでない場合に非表示化

## 関連ファイル
- `src/interface/ui/panels/info_panel.rs` - 常駐パネル生成と差分更新
- `src/interface/ui/presentation.rs` - 表示モデル生成 (`EntityInspectionModel`)
- `src/interface/ui/setup/panels.rs` - Startup時のパネル初期構築
- `src/interface/ui/components.rs` - `UiSlot` / `UiNodeRegistry` / UIコンポーネント
