# フォントシステム

本ドキュメントは、フォント機能の実装詳細を記載する。

---

## 導入済みフォント

| カテゴリ | フォント | ファイル名 | 用途 |
|:--|:--|:--|:--|
| **UI全般** | Noto Sans JP VF | `NotoSansJP-VF.ttf` | パネル、ボタン、ラベル |
| **Familiar** | Shantell Sans VF | `ShantellSans-VF.ttf` | ラテン語の吹き出しセリフ |
| **Soul (名前)** | Source Serif 4 VF | `SourceSerif4-VF.ttf` | UIリスト上のソウル名 |
| **Soul (セリフ)** | Noto Emoji VF | `NotoEmoji-VF.ttf` | モノクロ絵文字の吹き出し |

---

## フォントハンドル

[assets.rs](src/assets.rs) の `GameAssets` で管理:

```rust
pub font_ui: Handle<Font>,         // UI全般
pub font_familiar: Handle<Font>,   // Familiar吹き出し
pub font_soul_name: Handle<Font>,  // Soul名
pub font_soul_emoji: Handle<Font>, // Soulセリフ（絵文字）
```

---

## フォントサイズ定数

[constants.rs](src/constants.rs) で定義:

```rust
pub const FONT_SIZE_TITLE: f32 = 24.0;
pub const FONT_SIZE_HEADER: f32 = 20.0;
pub const FONT_SIZE_BODY: f32 = 16.0;
pub const FONT_SIZE_SMALL: f32 = 14.0;
pub const FONT_SIZE_TINY: f32 = 10.0;

// 吹き出し用
pub const FONT_SIZE_BUBBLE_SOUL: f32 = 24.0;
pub const FONT_SIZE_BUBBLE_FAMILIAR: f32 = 12.0;
```

---

## 適用箇所

| ファイル | 適用内容 |
|:--|:--|
| [entity_list.rs](src/interface/ui/setup/entity_list.rs) | タイトル、セクションヘッダー |
| `src/interface/ui/list/` | ソウル名、使い魔名、空欄テキスト（view_model, spawn 等） |
| [panels.rs](src/interface/ui/setup/panels.rs) | InfoPanel、HoverTooltip |
| [dialogs.rs](src/interface/ui/setup/dialogs.rs) | 操作ダイアログ |
| [bottom_bar.rs](src/interface/ui/setup/bottom_bar.rs) | メニューボタン、モード表示 |
| [submenus.rs](src/interface/ui/setup/submenus.rs) | サブメニュー項目 |
| [time_control.rs](src/interface/ui/setup/time_control.rs) | 時計、速度ボタン、タスクサマリー |
| [soul.rs](src/systems/visual/soul.rs) | ステータスアイコン |
| [effects.rs](src/systems/visual/blueprint/effects.rs) | 搬入ポップアップ |
| [jobs.rs](src/systems/jobs.rs) | 建物完成テキスト |
| [speech/update.rs](src/systems/visual/speech/update.rs) | 吹き出しシステム（Soul/Familiar） |
