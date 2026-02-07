# UI ビジュアル再設計 & 操作感改善ドキュメント

> 本ドキュメントは `../proposals/ui-improvement-proposals.md` の延長として、UIパネルとツールチップの**根本的なビジュアル変更**および**操作感の抜本改善**に焦点を当てる。

---

## 目次

1. [デザイン方針](#1-デザイン方針)
2. [パネルシステムの再設計](#2-パネルシステムの再設計)
3. [ツールチップの根本改善](#3-ツールチップの根本改善)
4. [カラーシステムの再構築](#4-カラーシステムの再構築)
5. [タイポグラフィの体系化](#5-タイポグラフィの体系化)
6. [インタラクションデザインの刷新](#6-インタラクションデザインの刷新)
7. [レイアウトシステムの再設計](#7-レイアウトシステムの再設計)
8. [実装ロードマップ](#8-実装ロードマップ)

---

## 1. デザイン方針

### 1-1. ビジュアルコンセプト: 「地獄の管理デスク」

Hell Workers は地獄の管理シミュレーションであり、UIもその世界観を反映すべきである。

**目指すビジュアル:**
- **素材感**: 焦げた羊皮紙、溶岩で光る金属枠、硫黄で曇ったガラス
- **基調色**: 深い暗赤〜暗紫のグラデーション（現在のパネルごとのバラつきを統一）
- **アクセント**: 溶岩オレンジ、魂の青白い光、硫黄の黄
- **質感**: 微かなノイズテクスチャ、パネル端のぼんやりした発光（glow）

**現状との差分:**
- 現在は各パネルが青・紫・赤とバラバラのグラデーション → 統一した暗い色調に
- 半透明の黒背景がフラットすぎる → 微かなテクスチャと枠線で地獄感を演出
- ツールチップが汎用的すぎる → エンティティタイプごとに視覚的特徴を持たせる

### 1-2. デザイン原則

| 原則 | 説明 |
|------|------|
| **一貫性** | すべてのパネルが同じデザイン言語を共有する |
| **階層性** | 視覚的な重み付けで情報の優先度を伝える |
| **即応性** | ホバー・クリック・選択への即座のフィードバック |
| **読みやすさ** | 暗い背景でも十分なコントラストを確保する |
| **世界観** | ゲームテーマに沿ったビジュアル表現 |

---

## 2. パネルシステムの再設計

### 2-1. 共通パネルフレーム

現在各パネルが個別にスタイルを定義している問題を解決するため、共通の「パネルフレーム」コンポーネントを導入する。

**構造:**
```
┌─ 外枠 (border: 1px, 微光色) ─────────────────┐
│ ┌─ ヘッダー (グラデーション背景) ───────────┐ │
│ │  アイコン  タイトル        ミニマイズ [_] │ │
│ └──────────────────────────────────────────┘ │
│ ┌─ コンテンツ (半透明背景 + 微ノイズ) ─────┐ │
│ │                                          │ │
│ │  (パネル固有の内容)                       │ │
│ │                                          │ │
│ └──────────────────────────────────────────┘ │
│ ── 内側シャドウ (inset shadow 風) ──────────── │
└───────────────────────────────────────────────┘
```

**パネルフレームの共通プロパティ (theme.rs への追加案):**
```rust
// パネル共通
pub const PANEL_BORDER_WIDTH: f32 = 1.0;
pub const PANEL_BORDER_COLOR: Color = Color::srgba(0.6, 0.3, 0.2, 0.5);   // 暗い銅色の縁
pub const PANEL_BG_PRIMARY: Color = Color::srgba(0.08, 0.05, 0.1, 0.92);  // 深い暗紫
pub const PANEL_BG_SECONDARY: Color = Color::srgba(0.05, 0.03, 0.07, 0.95);
pub const PANEL_HEADER_BG: Color = Color::srgba(0.15, 0.08, 0.12, 0.95);
pub const PANEL_CORNER_RADIUS: f32 = 4.0;

// パネルごとのアクセント色（小さなヘッダーラインやアイコン色で差別化）
pub const ACCENT_ENTITY_LIST: Color = Color::srgb(0.3, 0.5, 0.8);   // 青
pub const ACCENT_INFO_PANEL: Color = Color::srgb(0.7, 0.3, 0.6);    // 紫
pub const ACCENT_CONTROL_BAR: Color = Color::srgb(0.8, 0.3, 0.2);   // 赤
pub const ACCENT_TIME_CONTROL: Color = Color::srgb(0.8, 0.7, 0.3);  // 金
```

### 2-2. 左パネル（エンティティリスト）の再設計

**現状の問題:**
- 300px固定幅、最大70%高さで応答性がない
- 折りたたみ状態の視認性が弱い
- ソウル行のステータスアイコンが小さく密集

**改善案:**

#### ヘッダー部
```
┌─────────────────────────────────┐
│ 👥 Squad Overview     [−] [📌] │  ← ミニマイズ & ピン固定
├─────────────────────────────────┤
│ [🔍 フィルタ...]    [ソート ▼]  │  ← 検索 & ソート機能
└─────────────────────────────────┘
```

- ヘッダーに検索/フィルタバーを追加（ソウル数が増えた際の対応）
- ソート機能: 名前順、ストレス順、タスク種別順

#### 使い魔セクション
```
▼ Familiar: Cerberus (3/5) [巡回中]
  ├─ ステータスバー ████████░░ HP
  │
  │  ♂ Marcus    [😰32%] [💤18%]  ⛏ Mining
  │  ♀ Elena     [😰 5%] [💤45%]  🪓 Chopping
  │  ♂ Brutus    [😰78%] [💤62%]  💤 Idle
  │
  └─ + ソウルをドラッグして配属 ─────
```

- 使い魔セクション自体にステータスサマリーバーを追加
- 配属ドロップゾーンの視覚的ヒント
- ソウル行にミニプログレスバー（テキスト%の代わり）

#### ソウル行の再設計

**現在:**
```
♂ SoulName  ⚡32%  💤18%  ⛏
```

**改善後:**
```
┌──────────────────────────────────┐
│ ♂ SoulName                   ⛏  │
│ █████░░░░░ Stress  ████░░ Fatigue│
└──────────────────────────────────┘
```

- 2行構成: 上段に名前+タスクアイコン、下段にミニバー
- ストレスバーの色がリアルタイムで変化（緑→黄→赤）
- ホバー時に行全体がハイライト + 詳細ツールチップ表示

### 2-3. 右パネル（情報パネル）の再設計

**現状の問題:**
- 200px固定幅で内容が窮屈
- テキストのみの表示で情報の優先度が不明確
- エンティティタイプごとのレイアウト差がない

**改善案:**

#### エンティティヘッダー
```
┌─────────────────────────────┐
│  [アイコン]                 │
│  Soul: Marcus               │
│  ♂ Male | Squad: Cerberus   │
├─────────────────────────────┤
```

- エンティティタイプに応じた大きめのアイコン/ポートレート領域
- サブ情報（性別、所属）を副テキストとして表示

#### ステータスセクション
```
├─ Status ─────────────────────┤
│                              │
│  Motivation                  │
│  ██████████░░░░ 72%          │
│                              │
│  Stress         ⚠ Rising     │
│  ████░░░░░░░░░░ 32%          │
│                              │
│  Fatigue                     │
│  ██░░░░░░░░░░░░ 18%          │
│                              │
├─ Current Task ───────────────┤
│  ⛏ Mining - Iron Ore        │
│  Progress: ████████░░ 78%    │
│                              │
├─ Inventory ──────────────────┤
│  🪨 Iron Ore x3             │
│  🪵 Wood x1                 │
└──────────────────────────────┘
```

- プログレスバー付きステータス表示
- ステータスのトレンド表示（上昇中 ⚠、安定 ─、下降中 ▽）
- セクション区切りのラベル付きディバイダー
- 幅を 200px → 260px に拡張（または自動調整）

### 2-4. 下部バーの再設計

**現状の問題:**
- 50px高さの単純なボタン列
- モード表示がテキストのみで目立たない
- サブメニューとの連携が視覚的に弱い

**改善案:**

```
┌─────────────────────────────────────────────────────────┐
│  [🏗 Architect]  [📐 Zones]  [📋 Orders]  │  ▶ Normal  │
│       ▲                                    │            │
│   選択中はアンダーライン + グロー          │  現在モード │
└─────────────────────────────────────────────────────────┘
                      ▼
        ┌──────────────────────────┐
        │  サブメニュー（展開時）   │
        │  アイテムグリッド表示    │
        └──────────────────────────┘
```

- 選択中のモードボタンにアンダーライン+微かなグロー効果
- サブメニューがスライドアップで展開（アニメーション）
- モード表示エリアを右端に分離
- ボタンにアイコンを追加してテキストだけでなく視覚的に識別

---

## 3. ツールチップの根本改善

### 3-1. 現状の問題

- 単一の黒背景ボックスにテキストを流し込むだけ
- エンティティタイプに関係なく同じ見た目
- ワールドエンティティのみ対応（UIボタンにツールチップがない）
- 位置がマウス追従のみで、画面端での切れを考慮していない

### 3-2. リッチツールチップシステム

#### エンティティタイプ別テンプレート

**ソウルツールチップ:**
```
┌─ Soul ─────────────────────┐
│ ♂ Marcus                   │
│ Squad: Cerberus            │
│──────────────────────────  │
│ Motivation ████████░░ 72%  │
│ Stress     ████░░░░░░ 32%  │
│ Fatigue    ██░░░░░░░░ 18%  │
│──────────────────────────  │
│ 🔨 Mining - Iron Ore      │
│ Click to select            │
└────────────────────────────┘
```

**建物ツールチップ:**
```
┌─ Building ─────────────────┐
│ 🏠 Stockpile              │
│──────────────────────────  │
│ Storage: 12/20             │
│ ████████████░░░░░░░░ 60%  │
│──────────────────────────  │
│ Contents:                  │
│  🪵 Wood x5               │
│  🪨 Stone x7              │
└────────────────────────────┘
```

**リソースツールチップ:**
```
┌─ Resource ──────┐
│ 🪨 Iron Ore     │
│ Harvestable     │
│ Click to select │
└─────────────────┘
```

**UIボタンツールチップ:**
```
┌───────────────────────────┐
│ Architect Mode            │
│ 建築物の設計・配置 [B]    │
└───────────────────────────┘
```

#### ビジュアルスタイル
```rust
// ツールチップ共通
pub const TOOLTIP_BG: Color = Color::srgba(0.06, 0.04, 0.08, 0.95);
pub const TOOLTIP_BORDER: Color = Color::srgba(0.5, 0.3, 0.2, 0.6);
pub const TOOLTIP_BORDER_WIDTH: f32 = 1.0;
pub const TOOLTIP_CORNER_RADIUS: f32 = 3.0;
pub const TOOLTIP_PADDING: f32 = 8.0;
pub const TOOLTIP_MAX_WIDTH: f32 = 280.0;
pub const TOOLTIP_DELAY_MS: u64 = 300;       // 表示までの遅延
pub const TOOLTIP_FADE_DURATION_MS: u64 = 100; // フェードイン時間

// タイプ別アクセント色（ヘッダーラインの色）
pub const TOOLTIP_ACCENT_SOUL: Color = Color::srgb(0.5, 0.7, 1.0);
pub const TOOLTIP_ACCENT_BUILDING: Color = Color::srgb(0.8, 0.6, 0.2);
pub const TOOLTIP_ACCENT_RESOURCE: Color = Color::srgb(0.5, 0.8, 0.4);
pub const TOOLTIP_ACCENT_UI: Color = Color::srgb(0.7, 0.7, 0.7);
```

### 3-3. ツールチップの振る舞い改善

| 機能 | 現状 | 改善後 |
|------|------|--------|
| 表示タイミング | 即座 | 300ms遅延 + フェードイン |
| 位置制御 | マウス直下 | マウス右下 + 画面端自動補正 |
| 内容 | テキストのみ | テンプレート + プログレスバー |
| 対象 | ワールドエンティティのみ | ワールド + UI要素すべて |
| 消失 | 即座に消える | 短いフェードアウト (50ms) |
| 固定表示 | 不可 | Alt+クリックで固定 (将来) |

### 3-4. ツールチップのポジショニングロジック

```
マウス位置を基準に:
1. デフォルト: 右下にオフセット (+12px, +16px)
2. 右端に近い場合: 左側に反転
3. 下端に近い場合: 上側に反転
4. 角の場合: 対角に配置
5. 他のパネルとの重なり回避
```

実装案:
```rust
fn calculate_tooltip_position(
    mouse_pos: Vec2,
    tooltip_size: Vec2,
    viewport_size: Vec2,
) -> Vec2 {
    let offset = Vec2::new(12.0, 16.0);
    let mut pos = mouse_pos + offset;

    // 右端チェック
    if pos.x + tooltip_size.x > viewport_size.x {
        pos.x = mouse_pos.x - tooltip_size.x - offset.x;
    }
    // 下端チェック
    if pos.y + tooltip_size.y > viewport_size.y {
        pos.y = mouse_pos.y - tooltip_size.y - offset.y;
    }
    pos
}
```

---

## 4. カラーシステムの再構築

### 4-1. セマンティックカラーの導入

現在 `theme.rs` のカラー定数は用途ベースで名付けられているが、体系的でない。セマンティック（意味的）カラーシステムを導入する。

```rust
// === ベースカラーパレット ===
// 地獄のテーマに基づく5色系統

// Primary: 暗い赤紫（地獄の基調色）
pub const BASE_PRIMARY_900: Color = Color::srgb(0.08, 0.03, 0.06);
pub const BASE_PRIMARY_800: Color = Color::srgb(0.12, 0.05, 0.10);
pub const BASE_PRIMARY_700: Color = Color::srgb(0.18, 0.08, 0.14);
pub const BASE_PRIMARY_500: Color = Color::srgb(0.35, 0.15, 0.28);
pub const BASE_PRIMARY_300: Color = Color::srgb(0.55, 0.30, 0.45);

// Ember: 溶岩オレンジ（アクセント・警告）
pub const BASE_EMBER_700: Color = Color::srgb(0.5, 0.2, 0.05);
pub const BASE_EMBER_500: Color = Color::srgb(0.8, 0.4, 0.1);
pub const BASE_EMBER_300: Color = Color::srgb(1.0, 0.6, 0.2);

// Soul: 青白い光（ソウル関連）
pub const BASE_SOUL_700: Color = Color::srgb(0.15, 0.25, 0.45);
pub const BASE_SOUL_500: Color = Color::srgb(0.3, 0.5, 0.8);
pub const BASE_SOUL_300: Color = Color::srgb(0.5, 0.7, 1.0);

// Sulfur: 硫黄の黄（ストレス・注意）
pub const BASE_SULFUR_500: Color = Color::srgb(0.8, 0.7, 0.2);
pub const BASE_SULFUR_300: Color = Color::srgb(1.0, 0.9, 0.3);

// Neutral: グレー系（テキスト・ボーダー）
pub const BASE_NEUTRAL_900: Color = Color::srgb(0.1, 0.1, 0.12);
pub const BASE_NEUTRAL_700: Color = Color::srgb(0.25, 0.25, 0.3);
pub const BASE_NEUTRAL_500: Color = Color::srgb(0.5, 0.5, 0.55);
pub const BASE_NEUTRAL_300: Color = Color::srgb(0.75, 0.75, 0.8);
pub const BASE_NEUTRAL_100: Color = Color::srgb(0.9, 0.9, 0.92);

// === セマンティックカラー ===
pub const COLOR_BG_SURFACE: Color = BASE_PRIMARY_900;     // パネル背景
pub const COLOR_BG_ELEVATED: Color = BASE_PRIMARY_800;     // ヘッダー・ホバー
pub const COLOR_BG_OVERLAY: Color = BASE_PRIMARY_700;      // ツールチップ・ダイアログ
pub const COLOR_TEXT_PRIMARY: Color = BASE_NEUTRAL_100;     // 主要テキスト
pub const COLOR_TEXT_SECONDARY: Color = BASE_NEUTRAL_500;   // 副次テキスト
pub const COLOR_TEXT_ACCENT: Color = BASE_EMBER_300;        // 強調テキスト
pub const COLOR_BORDER_DEFAULT: Color = BASE_NEUTRAL_700;   // 通常ボーダー
pub const COLOR_BORDER_ACCENT: Color = BASE_EMBER_500;      // 強調ボーダー
pub const COLOR_INTERACTIVE_DEFAULT: Color = BASE_NEUTRAL_700;
pub const COLOR_INTERACTIVE_HOVER: Color = BASE_PRIMARY_500;
pub const COLOR_INTERACTIVE_ACTIVE: Color = BASE_EMBER_500;

// === ステータスカラー（既存の置き換え） ===
pub const COLOR_STATUS_HEALTHY: Color = Color::srgb(0.3, 0.8, 0.4);
pub const COLOR_STATUS_WARNING: Color = BASE_SULFUR_500;
pub const COLOR_STATUS_DANGER: Color = Color::srgb(0.9, 0.2, 0.1);
pub const COLOR_STATUS_INFO: Color = BASE_SOUL_500;
```

### 4-2. 既存カラー定数との移行方針

| 既存定数 | 新しいセマンティックカラー |
|---------|----------------------|
| `COLOR_HEADER_TEXT` | `COLOR_TEXT_PRIMARY` |
| `COLOR_EMPTY_TEXT` | `COLOR_TEXT_SECONDARY` |
| `COLOR_FOLD_BUTTON_BG` | `COLOR_INTERACTIVE_DEFAULT` |
| `COLOR_STRESS_HIGH` | `COLOR_STATUS_DANGER` |
| `COLOR_STRESS_MEDIUM` | `COLOR_STATUS_WARNING` |
| `COLOR_STRESS_ICON` | `BASE_SULFUR_300` |
| `COLOR_FATIGUE_ICON` | `BASE_SOUL_500` |

タスク色（CHOP, MINE, HAUL等）は用途固有であり、セマンティック化の必要はない。そのまま維持。

---

## 5. タイポグラフィの体系化

### 5-1. フォントスケールの定義

```rust
// フォントサイズスケール（4pxベース、1.33倍のモジュラースケール）
pub const FONT_SIZE_XS: f32 = 9.0;    // 補足・注記
pub const FONT_SIZE_SM: f32 = 11.0;   // ステータス値・サブテキスト
pub const FONT_SIZE_BASE: f32 = 13.0; // 本文・リスト項目
pub const FONT_SIZE_MD: f32 = 15.0;   // セクションヘッダー
pub const FONT_SIZE_LG: f32 = 18.0;   // パネルタイトル
pub const FONT_SIZE_XL: f32 = 22.0;   // ダイアログタイトル
```

### 5-2. テキストスタイルの組み合わせ

| 用途 | サイズ | 色 | 太さ |
|------|--------|-----|------|
| パネルタイトル | `LG` | `TEXT_PRIMARY` | Bold |
| セクションヘッダー | `MD` | `TEXT_PRIMARY` | SemiBold |
| リスト項目名 | `BASE` | `TEXT_PRIMARY` | Regular |
| ステータス値 | `SM` | `TEXT_SECONDARY` | Regular |
| ツールチップ本文 | `SM` | `TEXT_PRIMARY` | Regular |
| 注記・ヒント | `XS` | `TEXT_SECONDARY` | Regular |

---

## 6. インタラクションデザインの刷新

### 6-1. ホバーステートの体系化

すべてのインタラクティブ要素に3段階のステートを持たせる:

```
Default → Hover → Active (Press)
```

**ソウルリスト行:**
| ステート | 背景 | ボーダー | その他 |
|---------|------|---------|--------|
| Default | 透明 | なし | — |
| Hover | `BG_ELEVATED` (半透明) | なし | カーソル変化 |
| Selected | `BG_ELEVATED` | 左2px `BORDER_ACCENT` | 微グロー |
| Hover+Selected | `BG_ELEVATED` (やや明) | 左2px `BORDER_ACCENT` | グロー強化 |

**ボタン:**
| ステート | 背景 | テキスト色 | その他 |
|---------|------|-----------|--------|
| Default | `INTERACTIVE_DEFAULT` | `TEXT_PRIMARY` | — |
| Hover | `INTERACTIVE_HOVER` | `TEXT_PRIMARY` | 微かなスケールアップ (1.02x) |
| Active | `INTERACTIVE_ACTIVE` | `TEXT_PRIMARY` | スケールダウン (0.98x) |
| Disabled | `NEUTRAL_900` | `TEXT_SECONDARY` | opacity 50% |

### 6-2. 選択フィードバックの強化

**ワールド上のエンティティ選択時:**
1. 選択エンティティの周囲にパルスするアウトライン
2. 情報パネルがスライドインで表示
3. エンティティリスト上の対応行がハイライト+自動スクロール

**エンティティリスト上の選択時:**
1. 行にアクセントボーダー+背景色の変化
2. ワールド上のエンティティにカメラがスムーズにパン（オプション）
3. 情報パネルが更新される

### 6-3. コンテキストメニューの再設計

**現状:** 使い魔のみ右クリック対応。左クリックで閉じるだけ。

**改善案:**
```
┌─────────────────────────┐
│ 👁 情報を見る           │
│ ── 区切り ───────────── │
│ 📋 タスクを割り当て   ▶ │  ← サブメニュー
│ 🔄 使い魔を変更       ▶ │
│ ── 区切り ───────────── │
│ ⚡ 優先度: 通常       ▶ │
└─────────────────────────┘
```

- 対象: ソウル、使い魔、建物、リソース（エンティティタイプごとにメニュー内容が変わる）
- サブメニュー対応
- キーボードショートカットの表示
- アイコン付きメニュー項目

### 6-4. ドラッグ操作の導入

**ソウルの配属変更:**
1. ソウル行を長押し（200ms）でドラッグモード開始
2. ドラッグ中はソウル行の半透明コピーがマウスに追従
3. 使い魔セクションにホバーするとドロップゾーンがハイライト
4. ドロップで配属変更を実行

**パネルのリサイズ:**
1. パネル端にリサイズハンドル（マウスカーソルが⟺に変化）
2. ドラッグでパネル幅を変更
3. ダブルクリックでデフォルトサイズに復帰
4. 最小幅/最大幅の制限あり

---

## 7. レイアウトシステムの再設計

### 7-1. 現状の問題

- 全パネルが絶対位置（`Position::Absolute` + ピクセル指定）
- ウィンドウリサイズに非対応
- パネル間の位置関係がハードコードで壊れやすい

### 7-2. レイアウト構造の提案

```
┌──────────────────────────────────────────────────────┐
│                    Time Control (右上固定)             │
│                                                       │
│ ┌─────────┐                          ┌──────────┐   │
│ │         │                          │          │   │
│ │ Entity  │      Game World          │  Info    │   │
│ │ List    │      (中央メイン)        │  Panel   │   │
│ │         │                          │          │   │
│ │ (左)    │                          │ (右)     │   │
│ │         │                          │          │   │
│ └─────────┘                          └──────────┘   │
│                                                       │
│ ┌──────────────────────────────────────────────────┐ │
│ │              Bottom Bar (下部固定)                │ │
│ └──────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────┘
```

**ポイント:**
- 左右パネルは `top/bottom` をパーセントベースで指定し、下部バーと重ならないようにする
- `min-width` / `max-width` を設定し、極端なリサイズでもレイアウトが崩れないように
- 中央のゲームワールド領域は左右パネルに応じて自動調整

### 7-3. パネル位置定数の整理

```rust
// パネルレイアウト定数
pub const ENTITY_LIST_WIDTH: f32 = 300.0;
pub const ENTITY_LIST_MIN_WIDTH: f32 = 200.0;
pub const ENTITY_LIST_MAX_WIDTH: f32 = 450.0;
pub const ENTITY_LIST_MARGIN: f32 = 10.0;

pub const INFO_PANEL_WIDTH: f32 = 260.0;
pub const INFO_PANEL_MIN_WIDTH: f32 = 200.0;
pub const INFO_PANEL_MAX_WIDTH: f32 = 400.0;
pub const INFO_PANEL_MARGIN: f32 = 10.0;

pub const BOTTOM_BAR_HEIGHT: f32 = 50.0;
pub const TIME_CONTROL_MARGIN_TOP: f32 = 10.0;
pub const TIME_CONTROL_MARGIN_RIGHT: f32 = 10.0;
```

---

## 8. 実装ロードマップ

### Phase 1: 基盤整備（カラーシステム & テーマ統一）

| 項目 | 内容 | 影響ファイル |
|------|------|------------|
| 1-a | セマンティックカラー定数を `theme.rs` に追加 | `theme.rs` |
| 1-b | フォントサイズスケールを `theme.rs` に追加 | `theme.rs` |
| 1-c | パネルレイアウト定数を `theme.rs` に集約 | `theme.rs`, `setup/*.rs` |
| 1-d | 既存パネルの背景色を新定数に差し替え | `setup/entity_list.rs`, `setup/panels.rs`, `setup/bottom_bar.rs` |

### Phase 2: ツールチップの根本改善

| 項目 | 内容 | 影響ファイル |
|------|------|------------|
| 2-a | `HoverTooltip` をテンプレートベースに再設計 | `components.rs`, `interaction/mod.rs` |
| 2-b | エンティティタイプ別のツールチップ生成 | `interaction/mod.rs` |
| 2-c | ツールチップポジショニングロジックの実装 | `interaction/mod.rs` |
| 2-d | UIボタンへのツールチップ追加 | `setup/bottom_bar.rs`, `setup/time_control.rs` |
| 2-e | 表示遅延 & フェードイン/アウトの実装 | `interaction/mod.rs` |

### Phase 3: インタラクション改善

| 項目 | 内容 | 影響ファイル |
|------|------|------------|
| 3-a | ソウル行のホバーハイライト実装 | `list/interaction.rs` |
| 3-b | 選択状態の視覚フィードバック強化 | `list/interaction.rs`, `list/sync.rs` |
| 3-c | ボタンのステート管理統一 | `interaction/common.rs` |
| 3-d | キーボードショートカットの実装 | 新規システム or `interaction/` |

### Phase 4: パネルレイアウト改善

| 項目 | 内容 | 影響ファイル |
|------|------|------------|
| 4-a | エンティティリストのスクロール対応 | `setup/entity_list.rs` |
| 4-b | 情報パネルの幅拡張と自動高さ調整 | `setup/panels.rs` |
| 4-c | パネルリサイズハンドルの実装 | 新規コンポーネント |

### Phase 5: 高度なインタラクション

| 項目 | 内容 | 影響ファイル |
|------|------|------------|
| 5-a | コンテキストメニューの拡充 | `panels/context_menu.rs` |
| 5-b | ドラッグ＆ドロップによるソウル配属 | `list/interaction.rs`, 新規 |
| 5-c | フェードアニメーションの実装 | `interaction/mod.rs` |

---

## 関連ドキュメント

- `../proposals/ui-improvement-proposals.md` - インクリメンタルな改善案（本ドキュメントの前提）
- `docs/entity_list_ui.md` - エンティティリストの現行仕様
- `docs/info_panel_ui.md` - 情報パネルの現行仕様
- `src/interface/ui/theme.rs` - 現行テーマ定数

---

## 変更履歴

- 2026-02-06: 初版作成 - パネル再設計、ツールチップ根本改善、カラーシステム、操作感改善を網羅
