# RtT 解像度スケーリング設計提案

## メタ情報

| 項目 | 値 |
| --- | --- |
| 提案ID | `rtt-resolution-scaling-proposal-2026-03-16` |
| ステータス | `Accepted` |
| 作成日 | `2026-03-16` |
| 最終更新日 | `2026-03-29` |
| 作成者 | Claude Sonnet 4.6 |
| 関連ロードマップ | `docs/plans/3d-rtt/milestone-roadmap.md` |
| 関連提案 | `docs/proposals/3d-rtt/phase2-hybrid-rtt-plan-2026-03-15.md` |
| 関連提案 | `docs/proposals/3d-rtt/20260316/section-material-proposal-2026-03-16.md` |
| 依存完了済み | Phase 2 全MS（MS-2A〜MS-2D, MS-Elev） |
| 実装対象フェーズ | 基盤：Phase 3 着手前 / 本実装：Phase 3 序盤 |

### 実装メモ（2026-03-29）

- 現行コードではオフスクリーン RtT の runtime 状態は **`RttRuntime`** Resource（`crates/bevy_app/src/plugins/startup/rtt_setup.rs`）に集約されている。`.scene` / `.soul_mask` が当時の `RttTextures.texture_3d` / `texture_soul_mask` に相当する。
- 本文およびサンプルに残る `RttTextures` は**提案当時の名称**。現行の型名・システム配線は `docs/architecture.md` の「RtT（Render-to-Texture）インフラ」節を正とする。

---

## 1. 目的

### 解決したい課題

提案当時、RtT テクスチャは `RttTextures` リソースで 1280×720 固定として生成されていた。ウィンドウサイズとテクスチャサイズが一致している前提で設計されているため、ウィンドウが変化すると以下の問題が生じる。

```
ウィンドウが 1280×720 より大きい場合
  → テクスチャが画面全体を覆えず端に未描画領域が生じる
  → 4K モニターで起動すると建物・キャラクターが画面の一部にしか表示されない

ウィンドウが 1280×720 より小さい場合
  → テクスチャがウィンドウからはみ出しクリッピングされる

アスペクト比が変わった場合
  → RtT の内容が引き伸ばされて歪む
```

Phase 3 で全 BuildingType が RtT に依存するため、未対応のまま配布すると環境依存の表示崩壊が発生する。

### 到達したい状態

- ウィンドウリサイズ時に RtT テクスチャが自動的に再生成される
- テクスチャ解像度に品質設定のスケール係数を適用できる
- Phase 3 で参照箇所が増えても RtT runtime（現行は `RttRuntime`）の更新が1箇所で完結する

---

## 2. スコープ

### 対象（In Scope）

- `create_rtt_texture` 関数の切り出し（Phase 3 着手前・基盤整備）
- `RttCompositeSprite` のサイズ更新の一元管理（Phase 3 着手前・基盤整備）
- `WindowResized` イベントハンドラの実装（Phase 3 序盤）
- 品質設定スケール係数の追加（Phase 3 序盤）

### 非対象（Out of Scope）

- レターボックス表示（方針Aは採用しない）
- HiDPI / Retina ディスプレイへの対応（別途検討）

---

## 3. 技術設計

### 3.1 採用方針

方針Bを基本とし、方針Cのスケール係数を品質設定に組み込む。

| 方針 | 内容 | 採否 |
| --- | --- | --- |
| A：固定解像度（1280×720）でレターボックス | 実装コストゼロ。4K で粗い | ❌ 配布を考えると許容しづらい |
| B：`WindowResized` 時にテクスチャ再生成 | ウィンドウ追従 | ✅ 採用 |
| C：解像度スケール係数を品質設定に組み込む | 低スペックPC対応 | ✅ 採用（Bと組み合わせ） |

品質設定とスケール係数の対応は以下の通り。

| 品質設定 | スケール係数 | 1280×720 換算 |
| --- | --- | --- |
| 高 | 1.0 | ウィンドウ解像度そのまま |
| 中 | 0.75 | 960×540 相当 |
| 低 | 0.5 | 640×360 相当 |

### 3.2 参照箇所の問題

RtT テクスチャのハンドルは Phase 3 完了時点で以下の箇所から参照される。

```
RttTextures リソース
  └─ Handle<Image>
       ├─ Camera3d の RenderTarget
       ├─ Camera2d の合成スプライト（RttCompositeSprite）
       ├─ SectionMaterial の入力テクスチャ
       └─ Phase 3 で追加される各 BuildingType の visual system
```

Phase 3 完了後にリサイズ対応を追加しようとすると、全参照箇所を洗い出して更新する必要が生じる。Phase 3 着手前に `create_rtt_texture` を関数として切り出しておけば、Phase 3 でどれだけ参照箇所が増えても関数を呼び直すだけで全て更新される。

### 3.3 基盤設計（Phase 3 着手前に実装）

**`create_rtt_texture` の切り出し**

現在 `rtt_setup.rs` にインライン記述されているテクスチャ生成処理を独立した関数として切り出す。

```rust
/// RtT テクスチャを生成して Assets に登録し、ハンドルを返す。
/// ウィンドウリサイズ時に呼び直すことで全参照箇所が追従する。
pub fn create_rtt_texture(
    width: u32,
    height: u32,
    images: &mut Assets<Image>,
) -> Handle<Image> {
    let size = Extent3d { width, height, ..default() };
    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            size,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::RENDER_ATTACHMENT,
            format: TextureFormat::Bgra8UnormSrgb,
            ..default()
        },
        ..default()
    };
    image.resize(size);
    images.add(image)
}
```

**`RttCompositeSprite` のサイズ一元管理**

`RttTextures` のハンドルが変化したとき、合成スプライトのサイズを自動更新するシステムを追加する。

```rust
fn sync_rtt_composite_sprite(
    rtt: Res<RttTextures>,
    images: Res<Assets<Image>>,
    mut sprites: Query<(&mut Sprite, &mut Transform), With<RttCompositeSprite>>,
) {
    if !rtt.is_changed() { return; }
    let Some(image) = images.get(&rtt.handle) else { return };
    let size = image.size_f32();
    for (mut sprite, mut tf) in sprites.iter_mut() {
        sprite.custom_size = Some(size);
        tf.translation.z = Z_RTT_COMPOSITE; // 定数で管理
    }
}
```

### 3.4 本実装（Phase 3 序盤）

**`WindowResized` イベントハンドラ**

`WindowResized` は連続フレームで複数発火するため、最後のイベントのみ処理してフレームごとの再生成を防ぐ。

```rust
fn on_window_resized(
    mut events: EventReader<WindowResized>,
    mut rtt: ResMut<RttTextures>,
    mut images: ResMut<Assets<Image>>,
    quality: Res<QualitySettings>,
    mut cam3d: Query<&mut Camera, With<Camera3dRtt>>,
) {
    // 最後のイベントのみ処理（連続リサイズ時の毎フレーム再生成を防ぐ）
    let Some(event) = events.read().last() else { return };

    let scale = quality.rtt_scale();  // 0.5 / 0.75 / 1.0
    let w = (event.width  * scale) as u32;
    let h = (event.height * scale) as u32;

    // 古いテクスチャを削除
    images.remove(&rtt.handle);

    // 新しいテクスチャを生成（RttCompositeSprite は自動追従）
    rtt.handle = create_rtt_texture(w, h, &mut images);

    // Camera3d の RenderTarget を差し替え
    // ※ Bevy 0.18 の RenderTarget::Image が受け取る型を実装前に docsrs-mcp / cargo check で確認すること
    for mut cam in cam3d.iter_mut() {
        cam.target = RenderTarget::Image(rtt.handle.clone());
    }
}
```

**品質設定変更時の再生成**

`QualitySettings` が変更されたフレームにもテクスチャを再生成する。`on_window_resized` と同じ処理を共通関数に抽出して呼び出す。

```rust
fn on_quality_changed(
    window: Query<&Window, With<PrimaryWindow>>,
    quality: Res<QualitySettings>,
    rtt: ResMut<RttTextures>,
    images: ResMut<Assets<Image>>,
    cam3d: Query<&mut Camera, With<Camera3dRtt>>,
) {
    if !quality.is_changed() { return; }
    let Ok(win) = window.get_single() else { return };
    recreate_rtt(win.width(), win.height(), quality.rtt_scale(), rtt, images, cam3d);
}
```

`recreate_rtt` は `on_window_resized` と共有するヘルパー関数として `rtt_resize.rs` に定義する。

**`QualitySettings` への追加**

```rust
impl QualitySettings {
    pub fn rtt_scale(&self) -> f32 {
        match self {
            QualitySettings::High   => 1.0,
            QualitySettings::Medium => 0.75,
            QualitySettings::Low    => 0.5,
        }
    }
}
```

品質設定が変更されたときも同様に `on_window_resized` と同じ処理を走らせる。

---

## 4. 実装計画

### Phase 3 着手前（基盤整備）

> **依存**: Phase 2 全MS 完了

**やること**:
1. `rtt_setup.rs` の `create_rtt_texture` 関数を切り出し
2. `sync_rtt_composite_sprite` システムを実装・登録
3. `RttTextures.handle` の変更で合成スプライトが自動追従することを目視確認

**完了条件**:
- `cargo check` ゼロエラー
- `RttTextures.handle` を手動で差し替えたとき合成スプライトのサイズが追従すること（目視確認）

### Phase 3 序盤（本実装）

> **依存**: 基盤整備完了・`QualitySettings` リソース存在

**やること**:
1. `WindowResized` イベントハンドラの実装
2. `QualitySettings` への `rtt_scale()` 追加
3. 品質設定変更時の再生成処理追加

**完了条件**:
- `cargo check` ゼロエラー
- ウィンドウリサイズ時に建物・キャラクターの描画が追従すること（目視確認）
- 品質設定を変更したとき RtT 解像度が変わること（目視確認）
- 低品質設定で GTX 1650 相当の統合グラフィックスでも 60fps を維持すること

---

## 5. 影響ファイル一覧

| ファイル | 変更種別 | タイミング | 内容 |
| --- | --- | --- | --- |
| `plugins/startup/rtt_setup.rs` | 変更 | Phase 3 着手前 | `create_rtt_texture` 関数の切り出し |
| `systems/visual/rtt_composite.rs` | 新規 | Phase 3 着手前 | `sync_rtt_composite_sprite` システム |
| `hw_core/src/constants/render.rs` | 変更 | Phase 3 着手前 | `Z_RTT_COMPOSITE` 定数の追加 |
| `systems/visual/rtt_resize.rs` | 新規 | Phase 3 序盤 | `on_window_resized` ハンドラ |
| `hw_core/src/quality.rs` | 変更 | Phase 3 序盤 | `rtt_scale()` メソッド追加 |

---

## 6. 未解決事項（Pending）

| 項目 | 優先度 | タイミング |
| --- | --- | --- |
| `RenderTarget::Image` の受け取り型確認（Bevy 0.18 では `Handle<Image>` 直接か wrapping 型か） | P0 | MS 着手前。`docsrs-mcp` または `~/.cargo/registry/src/` で確認 |
| HiDPI / Retina ディスプレイでの `scale_factor` 対応 | P2 | Phase 3 序盤の本実装時に判断 |
| 品質設定変更時のフレームドロップ許容範囲の確認 | P2 | Phase 3 序盤の PoC で計測 |

---

## 7. 決定事項サマリ

| 決定内容 | 日付 |
| --- | --- |
| 方針B（WindowResized 時に再生成）＋方針C（品質スケール係数）を採用する | 2026-03-16 |
| 方針A（1280×720 固定レターボックス）は採用しない | 2026-03-16 |
| `create_rtt_texture` の関数切り出しと `RttCompositeSprite` の一元管理は Phase 3 着手前に実施する | 2026-03-16 |
| `WindowResized` ハンドラと品質スケール係数は Phase 3 序盤に実装する | 2026-03-16 |
| 品質設定スケール係数：高=1.0・中=0.75・低=0.5 | 2026-03-16 |
