# 地形 LOD 切替ちらつき改善計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `terrain-lod-switch-flicker-plan-2026-04-17` |
| ステータス | `Draft` |
| 作成日 | `2026-04-17` |
| 最終更新日 | `2026-07-13` |
| 作成者 | `Codex` |
| 関連計画 | `docs/plans/3d-rtt/milestone-roadmap.md` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題:
  **ヒステリシスは既に適正で、不要な往復切替は起きていない** ものとして扱う。その前提でも、実際に `Lod1` / `Lod1Lite` / `Lod2` が切り替わる瞬間に、49 chunk が同フレームで別 shader に飛ぶため、ちらつきやポップとして知覚される。
- 到達したい状態:
  正当な LOD 切替が 1 回発生したときでも、画面全体が一瞬で別 shader に飛んだように見えない状態にする。
- 成功指標:
  - `Lod1 ↔ Lod1Lite`、`Lod1Lite ↔ Lod2` の切替時に「全地形が一斉に点滅する」印象がなくなる
  - 切替は 1 回で完了し、遷移中に深度欠けや境界破綻が出ない
  - 実装後に `cargo check --workspace` が通る

## 2. スコープ

### 対象（In Scope）

- 地形 chunk の LOD 切替時の視覚遷移改善
- 切替瞬間を確認するための最小限の DevPanel / debug 表示追加
- 実装後に必要な `docs/world_layout.md` / `docs/rendering-performance.md` / `docs/debug-features.md` の更新

### 非対象（Out of Scope）

- LOD 閾値そのものの大幅な再設計
- hysteresis 幅、平滑化、minimum hold time の調整
- `LOD1` / `LOD1Lite` / `LOD2` shader の品質内容そのものの再設計
- chunk サイズ (`CHUNK_TILES`) や worldgen の変更
- 建築物、Soul、UI 側の LOD / visibility 切替

## 3. 現状とギャップ

### 現状

- `terrain_lod_switch_system` は `level != applied_level` になった瞬間に、49 個の `TerrainChunk` の `MeshMaterial3d<T>` を同フレームで一括差し替える。
- `Lod1` と `Lod1Lite` / `Lod2` は shader の省略内容が大きく異なるため、切替自体が安定していても視覚差が「瞬間的な明滅」に見えやすい。
- DevPanel は `LOD:X rtt:Y.Ypx` しか出しておらず、切替要求が出た瞬間と、実際の遷移完了タイミングを分けて追えない。

### 問題

本件では **見た目の急変** だけを扱う。  
たとえ切替が 1 回だけで正しく完了していても、49 chunk が同フレームで別 material / 別 shader へ飛ぶため、ポップが強い。

### 本計画で埋めるギャップ

- material 差し替えを即時完了ではなく短い遷移フェーズに変える
- 切替要求と遷移完了を debug 表示で区別できるようにする

## 4. 実装方針（高レベル）

- 方針:
  1. まず切替要求と適用完了を見える化し、単発切替の見た目だけを観測できる状態にする
  2. 次に `terrain_lod_switch_system` の即時差し替えを、短い遷移フェーズへ置き換える
  3. 遷移方式は opaque を維持できる dither discard を本線とし、alpha blend は採らない
- 設計上の前提:
  - 地形 LOD は全 chunk で同一レベルを共有する
  - `Terrain3dHandles` は LOD ごとの共有 material handle を 1 本ずつ持っている
  - `MeshMaterial3d<T>` は型ごとの別 component なので、異なる LOD material を同じ entity に同時保持して lerp する構成は取れない
  - そのため視覚遷移は「一時的に複数の chunk view を重ねる」か「material を単一型へ再統合する」かの二択になる
- Bevy 0.19 API での注意点:
  - `MeshMaterial3d<T>` の差し替えだけでは cross-fade にならない
  - prepass と main pass の discard 条件を揃えないと深度や輪郭が一瞬破綻する
  - 共有 material handle を使う都合上、遷移パラメータは per-chunk ではなく global uniform / resource で持つのが自然

## 5. マイルストーン

## M1: 切替瞬間の観測を最小追加

- 変更内容:
  - `TerrainLodState` に `requested_level` と `transition_progress` など、遷移観測用の最小状態を追加する
  - DevPanel に `target / applied / transition` を表示する
  - `docs/rendering-performance.md` に「単発 LOD 切替確認シナリオ」を追加する
- 変更ファイル:
  - `crates/bevy_app/src/systems/visual/terrain_lod.rs`
  - `crates/bevy_app/src/interface/ui/dev_panel.rs`
  - `docs/rendering-performance.md`
  - `docs/debug-features.md`
- 完了条件:
  - [ ] 遷移要求と遷移完了を DevPanel で区別できる
  - [ ] 単発切替時のポップを観測する再現手順が docs に残っている
- 検証:
  - `cargo check --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - 手動: 同一 seed / 同一 camera direction で単発の閾値通過を確認する

## M2: 切替時の視覚遷移導入

- 変更内容:
  - `terrain_lod_switch_system` の「即 remove/insert」から、短い transition state へ変更する
  - 地形 chunk は LOD 別の sibling view を持てる構成にし、遷移中だけ source / target の 2 系統を同時表示する
  - 遷移は alpha blend ではなく dither discard を使う
    - 理由: 地形は opaque 前提で、sorted transparent に落としたくないため
    - source / target は同じ `transition_progress` を逆位相の weight として使い、両方が同時に欠けたり重なり続けたりしないようにする
    - dither 座標は chunk ローカルではなく world-space で安定させ、chunk 境界とカメラ移動で模様が跳ねないようにする
  - `TerrainSurfaceUniform` にグローバルな transition パラメータを追加し、main / prepass の両 shader で同じ dither 判定を使う
  - 遷移完了後に source view を非表示化し、通常時は従来どおり単一 view のみ表示する
- 変更ファイル:
  - `crates/bevy_app/src/world/map/spawn.rs`
  - `crates/bevy_app/src/systems/visual/terrain_lod.rs`
  - `crates/bevy_app/src/plugins/startup/visual_handles.rs`
  - `crates/hw_visual/src/material/terrain_surface_material.rs`
  - `assets/shaders/terrain_surface_material.wgsl`
  - `assets/shaders/terrain_surface_material_lod1_lite.wgsl`
  - `assets/shaders/terrain_surface_material_lod2.wgsl`
  - `assets/shaders/terrain_surface_material_prepass.wgsl`
  - `docs/world_layout.md`
  - `docs/architecture.md`
- 完了条件:
  - [ ] LOD 切替時に全地形が一瞬で飛び変わったように見えない
  - [ ] 遷移中に深度欠け、section cut 欠け、境界の破綻が出ない
  - [ ] 通常時は 1 LOD view のみが有効で、常時 2 倍描画にならない
- 検証:
  - `cargo check --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - 手動: TopDown / elevation view の両方で閾値通過を確認
  - 手動: section cut 有効時、river/sand 境界、map 端で切替確認

## M3: 定数調整とドキュメント確定

- 変更内容:
  - M2 で入れた transition 時間と dither パターンを実機で微調整する
  - デバッグ表示、切替契約、遷移コストを docs に反映する
- 変更ファイル:
  - `docs/world_layout.md`
  - `docs/rendering-performance.md`
  - `docs/debug-features.md`
  - 必要なら `docs/architecture.md`
- 完了条件:
  - [ ] transition 時間と遷移方式の根拠が docs に残っている
  - [ ] デバッグ手順なしでも挙動を理解できる状態になっている
- 検証:
  - `cargo check --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - 手動: 低速ズーム、急速ズーム、カメラ方向変更後の再確認

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 遷移中の 2 系統描画で一時的に GPU コストが増える | 低スペック環境で一瞬重くなる | transition 時間を短く固定し、通常時は単一 view のみ有効にする |
| main pass と prepass の discard 条件がずれる | 輪郭欠け、深度破綻、shadow/outline 不整合 | dither 判定ヘルパーを共通化し、prepass shader にも同じ分岐を追加する |
| sibling view 化で spawn / cleanup が複雑化する | メンテナンス負荷が上がる | `TerrainChunkRoot` と `TerrainChunkView { level }` の責務を分け、LOD 管理を `terrain_lod.rs` に集約する |

## 7. 検証計画

- 必須:
  - `cargo check --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 手動確認シナリオ:
  - TopDown で `LOD1 ↔ Lod1Lite` の単発切替を確認する
  - TopDown で `Lod1Lite ↔ Lod2` の単発切替を確認する
  - North/East/South/West の elevation view でも同様に確認する
  - river/sand 境界、zone tone 境界、section cut 有効時に切替を確認する
- パフォーマンス確認:
  - DevPanel の LOD 表示で target / applied / transition を観測する
  - transition 中だけ draw cost が一時的に増えることを許容し、通常時に戻ることを確認する

## 8. ロールバック方針

- どの単位で戻せるか:
  - M2 は sibling view と dither 遷移だけを切り戻せる
- 戻す時の手順:
  1. `terrain_lod_switch_system` を即時差し替え方式に戻す
  2. `spawn.rs` の追加 view entity を削除する
  3. shader の transition uniform / discard を削除する

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン:
  - なし
- 未着手/進行中:
  - M1 から着手

### 次のAIが最初にやること

1. `terrain_lod.rs` と DevPanel に target / applied / transition の表示を追加する
2. 単発切替を観測し、どの LOD 組み合わせでポップが強いか確認する
3. sibling view + dither 遷移を M2 として実装する

### ブロッカー/注意点

- `MeshMaterial3d<T>` の型差をまたぐ単一 entity 上の cross-fade は現構成では取れない
- 遷移を shader 側で入れる場合、prepass も同時に更新しないと破綻しやすい

### 参照必須ファイル

- `crates/bevy_app/src/systems/visual/terrain_lod.rs`
- `crates/bevy_app/src/world/map/spawn.rs`
- `crates/bevy_app/src/plugins/visual.rs`
- `crates/hw_visual/src/material/terrain_surface_material.rs`
- `assets/shaders/terrain_surface_material.wgsl`
- `assets/shaders/terrain_surface_material_lod1_lite.wgsl`
- `assets/shaders/terrain_surface_material_lod2.wgsl`
- `assets/shaders/terrain_surface_material_prepass.wgsl`
- `docs/world_layout.md`
- `docs/rendering-performance.md`

### 最終確認ログ

- 最終 `cargo check --workspace`: `2026-07-13` / `not run (plan only)`
- 未解決エラー:
  - なし

### Definition of Done

- [ ] M1 で単発切替の再現条件と観測値が固定されている
- [ ] M2 で単発切替の視覚ポップが許容範囲まで下がっている
- [ ] 影響ドキュメントが更新済み
- [ ] `cargo check --workspace` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-17` | `Codex` | 初版作成 |
| `2026-04-18` | `Codex` | ヒステリシスは適正と仮定し、切替瞬間の視覚ポップ対策にスコープを絞って再構成 |
| `2026-07-13` | `Codex` | Bevy 0.19、workspace clippy、逆位相 weight と world-space dither の契約を反映 |
