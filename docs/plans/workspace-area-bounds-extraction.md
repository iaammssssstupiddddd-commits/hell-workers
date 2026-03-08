# AreaBounds を hw_core へ抽出

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `workspace-area-bounds-2026-03-08` |
| ステータス | `Implemented` |
| 作成日 | `2026-03-08` |
| 最終更新日 | `2026-03-08` |
| 作成者 | `AI` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: `AreaBounds`（矩形領域の共通型）が `src/systems/world/zones.rs` にあり、crate 境界を越えて共有できない。`TaskArea`（`src/systems/command/mod.rs`）と `AreaBounds` が同一概念の重複実装になっている。
- 到達したい状態: `AreaBounds` が `hw_core` に定義され、`TaskArea` が `AreaBounds` の newtype / Component ラッパーとなる。建設サイト等の crate 化の前提条件が整う。
- 成功指標: `cargo check` 成功、`AreaBounds` の定義が `hw_core` に一本化

## 2. スコープ

### 対象（In Scope）

- `AreaBounds` struct を `hw_core` へ移動
- `TaskArea` を `AreaBounds` ベースに統合（Component 属性は維持）
- `Site` / `Yard` の `bounds()` メソッドが `hw_core::AreaBounds` を返すように更新
- `From` impl の整理

### 非対象（Out of Scope）

- `TaskArea` の Component としての削除（UI/Visual が依存しているため維持）
- `Site` / `Yard` 自体の crate 移動（`WorldMap` に依存するため root に残す）
- `FloorConstructionSite` / `WallConstructionSite` の移動（次の計画で対応）

## 3. 現状とギャップ

- 現状:
  - `AreaBounds` → `src/systems/world/zones.rs`（plain struct、Component ではない）
  - `TaskArea` → `src/systems/command/mod.rs`（Component、min/max Vec2 で同一構造）
  - 相互 `From` impl が存在するが、両方とも root crate 内
- 問題: crate レベルで矩形型を共有できず、建設フェーズ enum の `hw_jobs` 移動がブロックされる
- 本計画で埋めるギャップ: `AreaBounds` を crate 境界で利用可能にする

## 4. 実装方針（高レベル）

- 方針: `AreaBounds` を `hw_core` に移動し、`TaskArea` はフィールドを `AreaBounds` に委譲する newtype パターンに変更
- 設計上の前提: `AreaBounds` は `Reflect` derive 不要（Component ではないため）。`bevy::prelude::Vec2` のみに依存。
- Bevy 0.18 APIでの注意点: なし（純粋なデータ型）

## 5. マイルストーン

## M1: AreaBounds を hw_core へ移動

- 変更内容:
  - `hw_core/src/lib.rs` に `pub mod area;` 追加
  - `hw_core/src/area.rs` に `AreaBounds` struct を定義（`from_points`, `center`, `size`, `contains`, `contains_with_margin`）
  - `src/systems/world/zones.rs` の `AreaBounds` 定義を削除し、`pub use hw_core::area::AreaBounds;` に置換
- 変更ファイル:
  - `crates/hw_core/src/lib.rs`
  - `crates/hw_core/src/area.rs`（新規）
  - `src/systems/world/zones.rs`
- 完了条件:
  - [x] `AreaBounds` が `hw_core::area::AreaBounds` から利用可能
  - [x] 既存の `use crate::systems::world::zones::AreaBounds` が変更なしで動作（re-export）
- 検証:
  - `cargo check`

## M2: TaskArea を AreaBounds ベースに統合

- 変更内容:
  - `TaskArea` のフィールド `min`/`max` を維持しつつ、`bounds()` が `hw_core::area::AreaBounds` を返すよう更新（既存の `From` impl を整理）
  - `TaskArea::from_points` 等のメソッドが `AreaBounds` に委譲する形にリファクタ
  - 重複メソッド（`contains`, `contains_with_margin`, `center`, `size`）を `Deref<Target=AreaBounds>` または明示的委譲で解消
- 変更ファイル:
  - `src/systems/command/mod.rs`
  - `src/systems/world/zones.rs`（`From` impl 更新）
- 完了条件:
  - [x] `TaskArea` と `AreaBounds` 間の重複メソッドが解消
  - [x] 既存の `TaskArea` 利用箇所がコンパイル通過
- 検証:
  - `cargo check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `TaskArea` の `Deref` 導入で意図しないメソッド解決 | 低 | `Deref` ではなく明示的委譲メソッドを採用する選択肢を残す |
| `AreaBounds` の `PartialEq` derive が外れる | 低 | hw_core 側でも `#[derive(Clone, Debug, PartialEq)]` を維持 |

## 7. 検証計画

- 必須:
  - `cargo check`
- 手動確認シナリオ:
  - TaskArea を使うエリア選択・建設サイト配置が正常に動作すること
- パフォーマンス確認（必要時）: 不要（データ型変更のみ）

## 8. ロールバック方針

- どの単位で戻せるか: M1, M2 それぞれ独立して revert 可能
- 戻す時の手順: git revert

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`
- 完了済みマイルストーン: なし
- 未着手/進行中: なし（M1, M2 完了）

### 次のAIが最初にやること

1. `src/systems/world/zones.rs` の `AreaBounds` 定義を確認
2. `crates/hw_core/src/area.rs` を作成して移動
3. `TaskArea` を `bounds()/min()/max()` 経由で既存コードへ統一
4. re-export を設定し `cargo check`

### ブロッカー/注意点

- `Yard::width_tiles()` / `height_tiles()` が `WorldMap::world_to_grid` を呼んでいるため、`Yard` 自体は root に残す必要がある

### 参照必須ファイル

- `src/systems/world/zones.rs` — 現在の AreaBounds 定義
- `src/systems/command/mod.rs` — TaskArea 定義
- `crates/hw_core/src/lib.rs` — hw_core モジュール構成

### 最終確認ログ

- 最終 `cargo check`: 2026-03-08
- 結果: `cargo check` 成功
- 未解決エラー: N/A

### Definition of Done

- [x] `AreaBounds` が `hw_core` で定義されている
- [x] `TaskArea` の重複メソッドが解消されている
- [x] `cargo check` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-08` | `AI` | 初版作成 |
