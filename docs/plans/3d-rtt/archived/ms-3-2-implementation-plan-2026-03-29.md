# MS-3-2 実装計画

## 目的

`MS-3-2` は RtT の固定解像度前提を外し、ウィンドウサイズ変更と品質設定変更の両方に対して scene RtT / soul mask RtT / composite の整合を維持するための基盤整備である。

Phase 3 の現在地では Soul・建築物・Soul silhouette mask がすべて RtT に依存しているため、ここが不安定なままだと表示確認自体が環境依存になる。

## 現状確認

- `create_rtt_texture` は `rtt_setup.rs` に切り出し済み
- **`RttRuntime`**（Resource）が `.scene` / `.soul_mask` の 2 系統ハンドルと `.viewport`（`RttViewportSize`）をまとめて保持する（2026-03-29 の pipeline 整理で `RttTextures` から統合）
- `RttViewportSize` は standalone Resource ではなく `RttRuntime.viewport` のフィールド型
- `sync_rtt_texture_size_to_window_and_quality` で PrimaryWindow の物理解像度および品質変更の追従が入っている
- `sync_rtt_output_bindings` で camera target と composite material 差し替えも既に入っている
- proposal が前提にしていた `QualitySettings` / `hw_core/src/quality.rs` は現行 repo に未実装

したがって、このマイルストーンの本体は「resize 対応の新規導入」ではなく、以下の 2 点になる。

1. 既存の window-follow 実装を恒久仕様として整理する
2. 未実装の品質スケール導線を 2 系統 RtT に安全に追加する

## 到達状態

- window size change 時に `RttRuntime.scene` / `RttRuntime.soul_mask` が同時に再生成される
- `Camera3dRtt` / `Camera3dSoulMaskRtt` / `RttCompositeMaterial` が同フレームで新 handle を参照する
- 品質設定 change 時も同じ再生成経路を通る
- `logical size` と `physical size` の使い分けが明文化される

## 不足アセット確認

新規アセットは不要。

理由:
- 対象は runtime texture の管理と quality resource の整備であり、GLB や画像の追加を伴わない
- scene texture / soul mask texture はいずれも runtime 生成である
- 品質スケールは係数変更で成立するため、品質別アセット差分も不要

不足しているのはアセットではなく、`QualitySettings` 相当の resource と変更経路である。

## 論点

### 1. 品質リソースの不在

proposal は `QualitySettings` の存在を前提にしているが、現行 repo にその型はない。

最初に決めること:
- `hw_core` に最小 enum を置くか
- 既存 state/resource に統合するか

このマイルストーンでは、RtT 解像度係数だけを持つ最小 resource から始める。

### 2. 2 系統 RtT の同期

scene RtT だけを更新すると、soul mask 合成が壊れる。

再生成は常に以下を同時更新する必要がある。
- `RttRuntime.scene`（旧 `RttTextures.texture_3d`）
- `RttRuntime.soul_mask`（旧 `RttTextures.texture_soul_mask`）
- `Camera3dRtt.target`
- `Camera3dSoulMaskRtt.target`
- `RttCompositeMaterial.scene_texture`
- `RttCompositeMaterial.soul_mask_texture`
- `RttCompositeMaterial.params.pixel_size`

### 3. logical / physical size の使い分け

現在のコードでは
- texture 生成は `physical_width / physical_height`
- composite 表示サイズは `Window::size()` の logical size
になっている。

この方針自体は妥当だが、品質係数を入れると `pixel_size` が texture 実サイズ基準で再計算されることを保証する必要がある。

## 実装方針

### 方針 A: 再生成責務の一本化

`rtt_setup.rs` に、次をまとめた共通ヘルパーを置く。

- window から viewport size を決める
- 品質係数を適用する
- 2 系統 texture を再生成する
- `RttViewportSize` を更新する

window change / quality change は、そのヘルパーを呼ぶ薄い wrapper にする。

### 方針 B: 品質設定は最小導入

proposal の full quality system を一度に入れず、まずは
- `High / Medium / Low`
- `rtt_scale()`
だけを持つ最小 resource を導入する。

UI が無くても dev / debug 経路から値を変えられれば `MS-3-2` の検証は成立する。

### 方針 C: 既存実装を壊さない

既に動いている
- window follow
- RtT handle 差し替え
- logical composite size
は活かし、責務整理と品質係数追加に集中する。

## 実装ステップ

1. [x] `QualitySettings` 相当の最小 resource を追加する
2. [x] `rtt_scale()` を実装する
3. [x] viewport size 算出を helper 化する
4. [x] 2 系統 RtT の再生成処理を helper 化する
5. [x] window change 系システムを helper 利用へ整理する
6. [x] quality change 系システムを追加する
7. [x] `RttCompositeMaterial.params.pixel_size` が texture 実サイズ準拠で更新されるよう確認する
8. [x] docs を同期する

## 変更対象候補

- `crates/bevy_app/src/plugins/startup/rtt_setup.rs`
- `crates/bevy_app/src/plugins/startup/rtt_composite.rs`
- `crates/bevy_app/src/plugins/startup/mod.rs`
- `crates/bevy_app/src/plugins/input.rs`
- `crates/hw_core/src/quality.rs` または同等の新規ファイル
- `docs/architecture.md`
- `docs/plans/3d-rtt/phase3-implementation-plan-2026-03-16.md`
- `docs/plans/3d-rtt/milestone-roadmap.md`

## 検証

- [x] `cargo check --workspace`
- [x] `cargo clippy --workspace -- -D warnings`
- [x] window resize 時に scene RtT / soul mask RtT が両方追従する
- [x] 品質設定変更時に RtT 解像度が変わる
- [x] Soul silhouette 丸めが崩れない
- [x] Familiar の 2D 前面表示に退行がない

## リスク

- quality resource の置き場を誤ると、後続の品質系提案と責務衝突する
- physical / logical size の混同で composite 表示が再びずれる
- scene / soul mask の片方だけ更新されると silhouette 合成が壊れる

## 完了条件

- window resize / quality change の両方で 2 系統 RtT が同期して再生成される
- `RttCompositeMaterial` の参照と `pixel_size` が新サイズへ追従する
- `cargo check --workspace` と `cargo clippy --workspace -- -D warnings` が通る
- 目視で scene / soul mask / Familiar 2D 前面表示に退行がない
