# シャドウマップサイズ削減計画

## 問題

`startup_systems.rs` で `DirectionalLightShadowMap` が 4096 に固定されている。

```rust
commands.insert_resource(DirectionalLightShadowMap { size: 4096 });
```

Bevy 0.18 では `DirectionalLightShadowMap.size` は **各 cascade の幅・高さ**であり、
native 環境の `CascadeShadowConfigBuilder::default()` は通常 **4 cascade** を使う。
shadow depth texture は `Depth32Float` なので、4096 は VRAM 使用量・shadow pass コストの両面で重い。

一方で、現時点では「4096 shadow map が 4〜6 FPS の主因」とまでは断定しない。
既存の `docs/rendering-performance.md` では地形 LOD1 の fragment cost も支配的候補として整理されているため、
この計画では **shadow map size は有力な高コスト要素の 1 つ** として扱い、実測で改善幅を確認する。

## 解決方針

今回は **最小変更で startup 時の shadow map size を 4096 → 2048 に下げる**。

- `QualitySettings.rtt` とは連動させない
- F4 の RtT 品質切り替えは現状どおり RtT 解像度のみを対象にする
- shadow 品質プリセットや runtime 切り替えは別タスクに分離する

理由:

- 現在の `QualitySettings` は RtT 品質専用で、F4 による runtime 切り替え経路を持つ
- ここに shadow size を安易に結び付けると、「品質表示は変わるが shadow map は起動時固定」という契約ずれが起きる
- まずは 4096 固定値を下げて、改善幅を独立に評価する方が安全

## 変更ファイル

| ファイル | 変更内容 |
|---|---|
| `crates/bevy_app/src/plugins/startup/startup_systems.rs` | `DirectionalLightShadowMap { size: 4096 }` を `2048` に変更し、必要なら短い理由コメントを追加 |
| `docs/architecture.md` | 3D RtT 用 DirectionalLight の shadow map size 記述を 2048 に同期 |

## 実装ステップ

1. `startup_systems.rs` の hard-coded な `4096` を `2048` に変更する
2. `docs/architecture.md` の関連記述を実装に合わせて更新する
3. `cargo check --workspace` を実行してコンパイルを確認する

## 検証方法

### 機能確認

- `cargo check --workspace` が通る
- 起動後も壁・Soul の shadow が引き続き描画される
- 目視で shadow quality が許容範囲内である

### 性能確認

同一条件で before / after を比較する。

- 同じ seed・同じカメラ位置・同じズームで比較する
- 可能なら `--perf-scenario` または `HW_PERF_SCENARIO=1` を使って負荷条件を固定する
- DevPanel の FPS 表示を使って `4096` と `2048` を比較する
- 可能なら `DirectionalLight.shadows_enabled = false` を一時比較対象にして、shadow 自体の寄与を切り分ける

確認したいこと:

- `4096 -> 2048` で FPS が有意に改善するか
- 改善が小さい場合、主要因は shadow ではなく地形 RtT / terrain LOD1 側にある可能性が高い

## スコープ外

- `QualitySettings` に shadow 品質設定を追加する対応
- runtime 中に shadow map size を切り替える UI / 入力対応
- `4096 / 2048 / 1024` の複数プリセット化
- terrain LOD1 / RtT 側の別ボトルネック修正

## 補足

将来 shadow 品質を設定項目として公開する場合は、`QualitySettings.rtt` に便乗させず、
shadow 専用の設定値または `ShadowQualityPreset` を別に持たせる。
そのうえで runtime 変更時の UX と texture 再生成コストを別途検証する。
