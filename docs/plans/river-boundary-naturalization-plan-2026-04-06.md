# 川の境界自然化計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `river-boundary-naturalization-2026-04-06` |
| ステータス | `Draft` |
| 作成日 | `2026-04-06` |
| 最終更新日 | `2026-04-06` |
| 作成者 | `Gemini CLI` |
| 関連提案 | `N/A` |

## 1. 目的

- **解決したい課題**: 
  現在の `generate_river_mask` において、X座標ごとに独立した乱数でY座標と川幅を決定しているため、論理的な川の形が1マス単位で激しく凹凸し、階段状の不自然な境界になっている。また、描画側（`boundary.rs`）のスプライン補間がこの階段状の頂点をすべて通過しようとするため、川岸に不自然な波打ち（Wavy staircase）が発生している。
- **到達したい状態**: 
  論理的な川の蛇行が滑らかな曲線（帯）になり、描画される川岸の境界線からも階段状の波打ちが消え、自然で美しい川の輪郭になる。
- **成功指標**:
  代表的なシード値において、川の輪郭が滑らかになっていること。
  `cargo check --workspace` および各種テスト（`generated_layouts_have_no_visual_cross` 等）が通ること。
  既存の `anchor_mask` や各種保護帯（`river_protection_band`）との契約を壊さず、決定論的な生成を維持すること。

## 2. スコープ

### 対象（In Scope）

- `hw_world::river::generate_river_mask` における、中心Y座標および川幅配列の 1D 平滑化（移動平均）処理の追加。
- `bevy_app::world::map::boundary::spawn_boundary_meshes` 等における、境界ポリライン頂点の Laplacian Smoothing 処理の追加。
- 実装方針に応じた `docs/map_generation.md` 等の文書更新（必要時）。

### 非対象（Out of Scope）

- 川の生成アルゴリズムの根本的な再設計（例：ノイズ関数や流体シミュレーションへの全面移行）。
- 境界メッシュ（`boundary.rs`）のマテリアルやシェーダーの変更。
- 川以外の地形（岩、砂など）の生成アルゴリズム変更（ただし、川の形状変化に伴う自然な波及は許容）。

## 3. 現状とギャップ

### レイヤー1: 論理形状（`hw_world`）
`generate_river_mask` は、列（X）ごとに `-1, 0, 1` のステップで `current_y` を更新し、さらに毎回独立した乱数で `width` を決定しています。これらが平滑化されていないため、1マス単位での急な凹凸が発生しています。

### レイヤー2: 描画メッシュ（`bevy_app`）
`boundary.rs` では、論理グリッドの境界エッジから抽出したポリラインに対して、直接 Catmull-Rom スプラインを適用しています。論理上の「階段」の角をすべて正確に通過しようとするため、斜め方向の境界が不要にうねってしまいます。

## 4. 実装方針（ハイブリッド・アプローチ）

論理側と描画側の両方からアプローチすることで、相乗効果を狙います。

### M1: 論理側の 1D 平滑化（`hw_world`）

`generate_river_mask` 内で、即座に `river_mask` に書き込むのではなく、一旦 `center_y` と `width` の配列を X座標 0〜MAP_WIDTH まで生成します。
その後、配列に対して数パス（例：3パス程度）の移動平均フィルタ（`(val[x-1] + val[x] + val[x+1]) / 3`）を適用します。
最後に平滑化された配列を用いて `river_mask` を塗ります。

**注意点**:
- 端点（`x=0` および `x=MAP_WIDTH-1`）の平滑化処理。
- 平滑化によって川が `anchor_mask` や `river_protection_band` に侵食しないよう、書き込み時のフィルタリング（`!anchor_mask.get(pos) && !river_protection_band.get(pos)`）は維持します。

### M2: 描画側の Laplacian Smoothing（`bevy_app`）

`boundary.rs` で論理グリッドからポリライン（`BoundaryPolyline`）を構築した直後（ノイズやスプライン適用**前**）に、頂点座標に対して数パス（例：3パス程度）の Laplacian Smoothing を適用します。

```rust
// 概念コード
for _ in 0..smoothing_passes {
    let mut new_points = points.clone();
    for i in 1..(n - 1) {
        if !junctions.contains(&world_to_corner_key(points[i])) {
            new_points[i] = (points[i - 1] + points[i] + points[i + 1]) / 3.0;
        }
    }
    points = new_points;
}
```

**注意点**:
- 端点および「三叉路以上のジャンクション（`junctions` に含まれる頂点）」は位置を固定（ピン留め）し、他の地形境界との隙間が生じないようにします。
- ポリラインが閉ループ（`is_closed`）の場合は、インデックスのラップアラウンドを考慮して平滑化します。

## 5. マイルストーン

### M1: 論理側の平滑化実装 (`hw_world`)
- [ ] `crates/hw_world/src/river.rs` の `generate_river_mask` を修正し、`center_y` と `width` を配列に事前生成するロジックに変更。
- [ ] 生成した配列に移動平均（1D Smoothing）を複数パス適用する関数/ロジックを追加。
- [ ] 平滑化された配列から `river_mask` を構築し、既存の保護帯チェックを維持する。
- [ ] `cargo test -p hw_world` が通過することを確認。
- [ ] 代表シードでマップを起動し、川の論理形状が滑らかになっていることを目視確認。

### M2: 描画側の平滑化実装 (`bevy_app`)
- [ ] `crates/bevy_app/src/world/map/boundary.rs` に、ポリライン頂点の Laplacian Smoothing 関数を追加。
- [ ] `spawn_boundary_meshes` 内で、スプラインやノイズを適用する前のポリラインに対して平滑化関数を呼び出す。
- [ ] 端点および `junctions` のピン留め、閉ループ時の処理が正しく実装されていることを確認。
- [ ] `cargo check --workspace` が通過することを確認。
- [ ] ゲームを起動し、境界リボンに不自然な波打ちがなく、滑らかな曲線になっていることを目視確認。

### M3: 最終調整・文書更新
- [ ] 平滑化のパス数（強さ）を調整し、最も自然な見た目になる設定を見つける。
- [ ] 変更内容に合わせて、必要であれば `docs/map_generation.md` に追記。
- [ ] 計画書のステータスを `Complete` に更新。

## 6. リスクと対策

| リスク | 対策 |
| --- | --- |
| M1の平滑化により、川が極端に細くなる箇所が発生する | 最小川幅（`RIVER_MIN_WIDTH`）の制限を平滑化後にも適用（`clamp` 等）するか、平滑化パス数を調整する。 |
| M2の平滑化が強すぎて、境界メッシュが本来の地形タイルから大きくズレる | 境界リボンの幅（48wu）に対して十分に小さい範囲に収まるよう、平滑化の強さ（パス数）を2〜3程度に留める。 |
| M2で他の境界線との隙間が発生する | ジャンクション頂点（`junctions`）は移動させない（ピン留めする）条件を厳密に実装する。 |

## 7. 検証計画

- 必須:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check --workspace`
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test -p hw_world generated_layouts_have_no_visual_cross -- --nocapture`
- 手動確認シナリオ:
  - `cargo run` でゲームを起動し、川の形が滑らかで階段状の波打ちがないことを確認する。
  - 川が `Site` や `Yard`（中心の施設エリア）に侵食していないことを確認する。

## 8. ロールバック方針

- M1 と M2 は独立して変更可能。
- 問題が発生した場合は、Git の変更を取り消すことで容易に以前の状態（階段状の境界）に復帰できる。
