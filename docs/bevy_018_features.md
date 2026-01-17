# Bevy 0.18 新機能導入提案

本ドキュメントは、Bevy 0.18で追加された新機能のうち、本プロジェクトに導入を推奨するものをまとめた提案資料である。

---

## 既に導入済みの機能

| 機能 | 使用箇所 |
|:--|:--|
| `2d` Cargo Feature Collection | `Cargo.toml` |
| `PanCamera` (First-Party Camera Controller) | `interface/camera.rs` |
| `BackgroundGradient` (UIグラデーション) | `panels.rs`, `entity_list.rs`, `bottom_bar.rs` |

---

## 新規拡張（現在ない機能の追加）

### 1. Popover / Menu ウィジェット ⭐⭐⭐ 高

**分類**: 🆕 **新規拡張**（現在ツールチップやメニュー機能は未実装）

**概要**: ポップアップUIの自動配置を提供するウィジェット。ウィンドウ端でも見切れないよう自動で位置調整される。

**用途例**:
- ツールチップ（アイテム名やステータス詳細の表示）
- 右クリックコンテキストメニュー
- ドロップダウンメニュー

**コード例**:
```rust
commands.spawn((
    Node { /* ... */ },
    Popover {
        anchor: anchor_entity,
        placement: PopoverPlacement::Bottom,
        ..default()
    },
));
```

---

### 2. Font Variations (下線・取り消し線・フォントウェイト) ⭐⭐ 中

**分類**: 🆕 **新規拡張**（現在テキスト装飾機能は未使用）

**概要**: テキストに下線、取り消し線、可変フォントウェイトを適用可能。

**用途例**:
- 重要なステータス値を**太字**で強調
- 無効化された選択肢に~~取り消し線~~を表示
- クリック可能なテキストに<u>下線</u>を表示

**コード例**:
```rust
commands.spawn((
    Text::new("クリック可能なテキスト"),
    Underline,
    UnderlineColor(Color::srgba(0.3, 0.6, 1.0, 1.0)),
));
```

---

### 3. IgnoreScroll (スクロール無視) ⭐⭐ 中

**分類**: 🆕 **新規拡張**（現在エンティティリストにヘッダー固定機能はない）

**概要**: Bevy UIノード階層内で、親の`ScrollPosition`を子が無視できる機能。

> [!NOTE]
> **UI専用機能**: ワールドマップのカメラスクロール（`PanCamera`）とは無関係。
> UIパネルは既にスクリーンスペースで描画されるため、カメラ移動の影響を受けない。

**用途例**:
エンティティリストパネル**内部**でリストをスクロールしたとき、ヘッダー行を固定表示：

```
┌─────────────────────────┐
│ 名前         ステータス │ ← IgnoreScrollでヘッダー固定
├─────────────────────────┤
│ Soul 1       採掘中     │ ↑
│ Soul 2       待機中     │ │ スクロール領域
│ Soul 3       休息中     │ ↓
└─────────────────────────┘
```

**コード例**:
```rust
// リストコンテナ
commands.spawn((
    Node { overflow: Overflow::scroll_y(), ..default() },
)).with_children(|parent| {
    // 固定ヘッダー
    parent.spawn((
        Node { /* ヘッダースタイル */ },
        Text::new("名前"),
        IgnoreScroll::default(), // 親のScrollPositionを無視
    ));
    // スクロールするリスト項目
    for soul in souls {
        parent.spawn((Node::default(), Text::new(&soul.name)));
    }
});
```

---

### 4. Automatic Directional Navigation ⭐⭐ 中

**分類**: 🆕 **新規拡張**（現在コントローラー/キーボードUI操作は未実装）

**概要**: UI要素間の方向ナビゲーションを自動計算。ゲームパッドや矢印キーでのUI操作が可能になる。

**用途例**:
- 矢印キーでボタン間を移動
- ゲームパッドのDパッドでメニュー操作

---

### 5. TryStableInterpolate (色・レイアウト補間) ⭐ 低

**分類**: 🆕 **新規拡張**（現在UIアニメーションは未実装）

**概要**: UIの`Val`型や`Color`のスムーズな補間が可能。

**用途例**:
- パネルのスライドインアニメーション
- 背景色のフェード効果

---

### 6. remove_systems_in_set (システム削除) ⭐ 低

**分類**: 🆕 **新規拡張**（現在デバッグシステムの条件付き除去は未実装）

**概要**: スケジュールからシステムセットごと削除可能。

**用途例**:
- リリースビルドでデバッグシステムを完全削除

---

## 置き換え候補

> [!NOTE]
> 現時点で「置き換え」を推奨する機能はありません。
> 既存の自作実装はBevy 0.18でも引き続き有効であり、0.18の新機能で代替すべきものは見当たりませんでした。

---

## 導入優先順位まとめ

| 順位 | 機能 | 分類 | 理由 |
|:--:|:--|:--:|:--|
| 1 | **Popover / Menu** | 🆕 新規 | UIの利便性向上、メニュー拡張の基盤 |
| 2 | **Font Variations** | 🆕 新規 | 導入コスト低、視覚的改善効果大 |
| 3 | **IgnoreScroll** | 🆕 新規 | エンティティリストの使い勝手向上 |
| 4 | **Directional Navigation** | 🆕 新規 | コントローラー対応の準備 |
| 5 | **TryStableInterpolate** | 🆕 新規 | UIアニメーション追加時に検討 |
| 6 | **remove_systems_in_set** | 🆕 新規 | リリース最適化時に検討 |

---

## 2Dゲームに関係の薄い機能（導入不要）

以下の機能は3D向けのため、本プロジェクトでは導入不要：

- Atmosphere Occlusion / PBR Shading Fixes
- Solari (レイトレーシング)
- Fullscreen Material
- glTF Extensions
- Portals / Mirrors
- Clustered Decals
