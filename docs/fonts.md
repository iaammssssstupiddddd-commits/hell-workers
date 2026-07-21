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

[assets.rs](../crates/bevy_app/src/assets.rs) の `GameAssets` で管理:

```rust
pub font_ui: Handle<Font>,         // UI全般
pub font_familiar: Handle<Font>,   // Familiar吹き出し
pub font_soul_name: Handle<Font>,  // Soul名
pub font_soul_emoji: Handle<Font>, // Soulセリフ（絵文字）
```

---

## フォントサイズ

Bevy UIの正本は[theme.rs](../crates/hw_ui/src/theme.rs)の`UiTheme.typography`です。
基本のmodular scaleは`xs/sm/base/md/lg/xl = 9/11/13/15/18/22px`で、widgetはResourceから値を読みます。
`title/header/item/small/clock/status/dialog_*`は既存画面向けのtheme aliasであり、グローバルな
`FONT_SIZE_TITLE`等の定数ではありません。

world-space visualはUI themeとは別責務です。Soul status iconは
`hw_core::constants::animation::FONT_SIZE_BODY = 16px`、speech bubbleは
`hw_visual::speech::spawn`が発話priority別のサイズを決めます。

---

## 適用箇所

| ファイル | 適用内容 |
|:--|:--|
| [entity_list.rs](../crates/hw_ui/src/setup/entity_list.rs) | タイトル、セクションヘッダー |
| `crates/bevy_app/src/interface/ui/list/` | ソウル名、使い魔名、空欄テキスト（view_model, spawn 等） |
| [panels.rs](../crates/hw_ui/src/setup/panels.rs) | InfoPanel、HoverTooltip |
| [dialogs.rs](../crates/hw_ui/src/setup/dialogs.rs) | 操作ダイアログ |
| [bottom_bar.rs](../crates/hw_ui/src/setup/bottom_bar.rs) | メニューボタン、モード表示 |
| [submenus.rs](../crates/hw_ui/src/setup/submenus.rs) | サブメニュー項目 |
| [time_control.rs](../crates/hw_ui/src/setup/time_control.rs) | 時計、速度ボタン、タスクサマリー |
| [soul/systems.rs](../crates/hw_visual/src/soul/systems.rs) | ステータスアイコン |
| [effects.rs](../crates/hw_visual/src/blueprint/effects.rs) | 搬入ポップアップ |
| [building_completion/post_process.rs](../crates/bevy_app/src/systems/jobs/building_completion/post_process.rs) | 建物完成テキスト |
| [speech/update.rs](../crates/hw_visual/src/speech/update.rs) | 吹き出しシステム（Soul/Familiar） |
