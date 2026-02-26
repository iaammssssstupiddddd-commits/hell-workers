# Dream UI 表示 — 実装計画書
作成日: 2026-02-26  
更新日: 2026-02-26

## 目的
`DamnedSoul.dream`（0.0–100.0）を、以下の2箇所にテキスト表示する。

- インスペクションパネル（右パネル）
- エンティティリストの Soul 行（左パネル）

## 対象外
- dream 専用アイコンアセットの新規追加
- 左パネル行の大幅な横幅再設計（今回は既存行内の最小追加で対応）

---

## アーキテクチャ概要

### 1) インスペクションパネル

```text
DamnedSoul
  ↓ EntityInspectionQuery::build_soul_model() [builders.rs]
SoulInspectionFields { motivation, stress, fatigue, dream, ... }
  ↓ to_view_model() [model.rs]
SoulInfoViewModel { motivation, stress, fatigue, dream, ... }
  ↓ info_panel_system() [update.rs]
UiSlot::StatDream → Text エンティティを更新
```

### 2) エンティティリスト

```text
DamnedSoul
  ↓ build_entity_list_view_model_system() [src/interface/ui/list/view_model.rs]
SoulRowViewModel { fatigue_text, stress_text, dream_text, dream_empty, task_visual, ... }
  ↓ spawn/sync [src/interface/ui/list/spawn/soul_row.rs, src/interface/ui/list/sync.rs]
左パネルの Soul 行テキスト/色を更新
```

dream の値ソースは両方とも `DamnedSoul.dream` に統一する。

---

## 実装ステップ

### M1: データ層 — SoulInspectionFields に dream 追加
**ファイル:** `src/interface/ui/presentation/mod.rs`

`SoulInspectionFields` に `dream: String` を追加する。

**ファイル:** `src/interface/ui/presentation/builders.rs`

`build_soul_model()` で dream 表示文字列を構築し、`SoulInspectionFields` へ設定する。  
併せて tooltip へも追加する。

---

### M2: ViewModel 層（Info Panel）
**ファイル:** `src/interface/ui/panels/info_panel/model.rs`

`SoulInfoViewModel` に `dream: String` を追加し、`to_view_model()` で受け渡す。

---

### M3: コンポーネント層（Info Panel）
**ファイル:** `src/interface/ui/components.rs`

1. `UiSlot` に `StatDream` を追加  
2. `InfoPanelNodes` に `dream: Option<Entity>` を追加

---

### M4: レイアウト層（Info Panel）
**ファイル:** `src/interface/ui/panels/info_panel/layout.rs`

Fatigue 行の直後（`Current Task` divider の前）に Dream 行を追加し、`UiSlot::StatDream` を割り当てる。

---

### M5: 更新システム（Info Panel）
**ファイル:** `src/interface/ui/panels/info_panel/update.rs`

1. `entity_for_slot()` に `UiSlot::StatDream` を追加  
2. `InfoPanelViewModel::Soul` で `soul.dream` を反映  
3. `InfoPanelViewModel::Simple` で Dream スロットを空文字クリア

---

### M6: エンティティリスト反映

**ファイル:** `src/interface/ui/list/mod.rs`  
`SoulRowViewModel` に `dream_text` / `dream_empty` を保持する（未追加なら追加、既存なら定義維持）。

**ファイル:** `src/interface/ui/list/view_model.rs`  
`build_soul_view_model()` で `dream_text` と `dream_empty` を構築する。

```rust
dream_text: format!("{:.0}", soul.dream),
dream_empty: soul.dream <= 0.0,
```

**ファイル:** `src/interface/ui/list/spawn/soul_row.rs`  
Stress の後、Task アイコンの前に dream テキストノードを配置する。  
`dream_empty` で色を切り替える（例: 0 のとき警告寄り色）。

**ファイル:** `src/interface/ui/list/sync.rs`  
値更新時に dream ノードも更新対象へ含める。  
特に children index を dream 追加後の並びに合わせる（Task アイコンの index ずれ回避）。

---

### M7: 仕様ドキュメント同期
**ファイル:** `docs/entity_list_ui.md`

Soul 行の表示項目に Dream 値テキストを追記し、表示順と色ルール（`dream == 0`）を明記する。

---

## 表示フォーマット

| 表示箇所 | 表示例 |
|:---|:---|
| インスペクションパネル | `Dream: 47/100` |
| エンティティリスト行 | `47` |
| エンティティリスト行（枯渇） | `0`（枯渇色） |

---

## 変更ファイル一覧

| ファイル | 変更内容 |
|:---|:---|
| `src/interface/ui/presentation/mod.rs` | `SoulInspectionFields` に `dream: String` 追加 |
| `src/interface/ui/presentation/builders.rs` | dream フォーマット・tooltip・fields 設定 |
| `src/interface/ui/panels/info_panel/model.rs` | `SoulInfoViewModel` に `dream` 追加、`to_view_model()` 更新 |
| `src/interface/ui/components.rs` | `UiSlot::StatDream` 追加、`InfoPanelNodes.dream` 追加 |
| `src/interface/ui/panels/info_panel/layout.rs` | Dream 行 spawn、スロット登録 |
| `src/interface/ui/panels/info_panel/update.rs` | `entity_for_slot()` / `set_text_slot` の Dream 対応 |
| `src/interface/ui/list/mod.rs` | `SoulRowViewModel` の dream フィールド定義 |
| `src/interface/ui/list/view_model.rs` | `build_soul_view_model()` の dream 値構築 |
| `src/interface/ui/list/spawn/soul_row.rs` | dream テキストノードの追加/維持 |
| `src/interface/ui/list/sync.rs` | dream テキスト同期、children index 整合 |
| `docs/entity_list_ui.md` | Soul 行表示仕様に Dream 値を追記 |

---

## 検証

- `cargo check` でエラー・警告なし
- Soul 選択時、右パネルに `Dream: XX/100` が表示される
- 左エンティティリストの Soul 行に dream 値が表示される
- 睡眠・休憩で dream が減少したとき、右/左の表示がともに更新される
- `dream == 0` で左行が枯渇色へ切り替わる
- 左行のタスクアイコンが正しいまま（dream 追加による index ずれなし）

---

## オプション拡張（スコープ外）

- dream 専用アイコン追加（右/左共通）
- 左リストで `47/100` 表示に拡張（必要時に列幅再設計）
- dream 閾値による段階色（例: 0 / 1–30 / 31–100）
