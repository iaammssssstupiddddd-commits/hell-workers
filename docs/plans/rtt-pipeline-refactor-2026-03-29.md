# RtT Pipeline Refactor Plan

## Problem

現在の RtT 実装は機能自体は成立しているが、責務が複数ファイルにまたがって分散している。

- `startup_systems.rs`
  - RtT texture 初期生成
  - `Camera3dRtt` / `Camera3dSoulMaskRtt` spawn
  - Overlay camera spawn
- `rtt_setup.rs`
  - viewport size 計算
  - texture 再生成
- `rtt_composite.rs`
  - composite material 初期化
  - camera `RenderTarget` 差し替え
  - material texture binding / `pixel_size` 更新

この構成には次の問題がある。

- 初期生成と再生成の責務が分かれており、RtT の構成要素を追いにくい
- `setup()` が大きく、RtT の知識を startup 全体へ漏らしている
- texture / camera target / composite binding の同期が別々の場所にあり、拡張時に差分漏れを起こしやすい
- scene RtT と soul mask RtT が「2 系統ある」ことを明示する中間表現が弱い

現状では動いているが、今後 `SectionMaterial` / 追加 pass / 品質設定拡張を入れる前に整理しておく価値がある。

## Goal

RtT を「初期化」「viewport 決定」「texture 再生成」「camera target 更新」「composite binding 更新」の一連の pipeline として扱える状態にする。

到達目標:

- RtT の初期化と再生成が同じデータモデルで動く
- `startup_systems.rs` から RtT の詳細を減らす
- scene RtT / soul mask RtT を 1 つの runtime resource で追える
- 再生成時に更新すべき対象が 1 箇所にまとまる

## Non-Goals

- RtT の機能追加
- 新しい texture pass の導入
- composite shader の見た目変更
- camera/light の構成変更

## Refactor Direction

### 1. RtT runtime resource を明示化する

`RttTextures` と `RttViewportSize` を別 resource のまま扱うのではなく、次のような RtT runtime 構造へまとめる。

- viewport size
- scene texture handle
- soul mask texture handle

候補:

```rust
#[derive(Resource)]
pub struct RttRuntime {
    pub viewport: RttViewportSize,
    pub scene: Handle<Image>,
    pub soul_mask: Handle<Image>,
}
```

これにより、初期化と再生成の両方が同じ resource を更新する形にできる。

### 2. 初期生成を `rtt_setup.rs` 側へ寄せる

いまの `startup_systems::setup()` にある:

- fallback viewport 決定
- window + quality からの viewport 算出
- scene / soul mask texture 初期生成
- resource insert

を `rtt_setup.rs` の helper へ寄せる。

候補:

- `initialize_rtt_runtime(...) -> RttRuntime`
- `spawn_rtt_cameras(...)`

こうすると `setup()` は「RtT を使う startup wiring」だけに薄くできる。

### 3. 再生成後の反映を 1 系統にまとめる

現在は:

- `rtt_setup::sync_rtt_texture_size_to_window_and_quality`
  - texture の再生成
- `rtt_composite::sync_rtt_output_bindings`
  - camera target / composite material を同期

の 2 段階になっている。

これ自体は悪くないが、更新対象が増えると追跡が難しい。

次の形に整理する。

- `rtt_setup`
  - viewport 判定
  - resource 更新
- `rtt_bindings`
  - runtime resource から camera / material / mesh scale へ反映

少なくとも「RtT runtime をどこに反映するか」を 1 system に集約する。

### 4. camera spawn helper を分離する

`startup_systems.rs` の `setup()` には Camera2d / OverlayCamera / Camera3dRtt / Camera3dSoulMaskRtt が混在している。

RtT に関係するのは:

- OverlayCamera
- Camera3dRtt
- Camera3dSoulMaskRtt

これらは `rtt_setup.rs` か新規 `rtt_spawn.rs` に切り出した方が読みやすい。

## Proposed Steps

1. `RttRuntime` resource を導入し、`RttTextures` / `RttViewportSize` の利用箇所を洗い出す
2. 初期 texture 生成を `rtt_setup.rs` helper に移す
3. `startup_systems::setup()` から RtT resource 初期化コードを除去する
4. `sync_rtt_output_bindings` が `RttRuntime` を単一入力として受けるようにする
5. 必要なら RtT camera spawn を helper 化する
6. docs を新しい責務分割に同期する

## Files To Modify

- `crates/bevy_app/src/plugins/startup/rtt_setup.rs`
- `crates/bevy_app/src/plugins/startup/rtt_composite.rs`
- `crates/bevy_app/src/plugins/startup/startup_systems.rs`
- `crates/bevy_app/src/plugins/startup/mod.rs`
- `docs/architecture.md`
- `docs/plans/3d-rtt/phase3-implementation-plan-2026-03-16.md` if milestone wording changes

## Verification

- `cargo check --workspace`
- `cargo clippy --workspace -- -D warnings`
- 起動時に RtT が初期化される
- window resize で scene / soul mask の両方が追従する
- `F4` の品質切り替えで RtT 解像度だけが変わる
- composite 表示と Familiar 2D 前面表示に退行がない

## Asset Check

このリファクタに追加アセットは不要。

- 既存 shader / texture / GLB をそのまま使う
- 必要なのは runtime resource と startup wiring の整理のみ
