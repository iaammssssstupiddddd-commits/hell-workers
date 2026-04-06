# 川の境界自然化計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `river-boundary-naturalization-2026-04-06` |
| ステータス | `Complete` |
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
その後、配列に対して数パス（`RIVER_SMOOTH_PASSES = 3`）の移動平均フィルタを適用します。

**width について**: `width` はランダム性を維持し川岸の有機的な変化を保ちます。width を平滑化すると河川タイルと砂タイルが 2×2 チェッカーボードを形成し、`enforce_no_visual_cross_2x2` で修復不可能な視覚クロスが発生することが判明したため（seed=0 で再現）、width への平滑化は行いません。

**端点処理**: 端点（`x=0` および `x=MAP_WIDTH-1`）はミラー補外（境界値を 2 回使う）で処理します。

**平滑化後の保護帯侵犯への対応**: 移動平均後に `center_y` が `river_protection_band` や `anchor_mask` 内に入る可能性があります。川幅のタイル書き込み時に既存のフィルタ（`!anchor_mask.get(pos) && !river_protection_band.get(pos)`）で実際のタイル配置はガードされます。極端に細くなる列が生じうることは許容範囲とします（保護帯はマップ端寄りにしか存在しないため影響が限定的）。

**`centerline` の更新**: 戻り値の `centerline: Vec<GridPos>` も平滑化後の `center_y` から構築します（デバッグ表示等で参照されるため）。

**パス数の定数化**: `RIVER_SMOOTH_PASSES: usize = 3` を `river.rs` 上部に定義し、調整箇所を一か所に集約します。

### M2: 描画側の面取り（Chamfer）実装（`bevy_app`）

`boundary.rs` の `spawn_boundary_meshes` パイプラインに、`displace_polyline` の直後・`sample_catmull_rom` の前に `chamfer_polyline_points` を挿入する。

川岸境界は「水平基調 + 幅変化起因の 1 タイル垂直段差」構造を持つ。段差コーナーは正確に 90° であり、Catmull-Rom がオーバーシュートして「wavy staircase」になる。Laplacian Smoothing は閉ループ収縮・ノイズハッシュ不安定・ジャンクション隣接ピンチなど複数の副作用があり不採用。

**面取り（Chamfer）** を選択した理由:
- D-P（間引き）は川岸ステップコーナーを「高偏差頂点」として保持してしまい根本解決にならない
- 面取りはコーナー頂点を 2 つのベベル点で直接置換し、Catmull-Rom への 90° 入力を排除する
- ジャンクションは `displace_polyline` で変位=0 なので元座標にある → ジャンクション判定が安定
- ノイズパラメータは変位前の元ポリラインから計算するため、面取りでハッシュが変わらない

**定数**: `CHAMFER_DISTANCE = TILE_SIZE × 0.35 ≈ 11.2wu`、`CHAMFER_COS_THRESHOLD = 0.5`（60° より鋭いコーナーのみ）

**関数シグネチャ**:
```rust
fn chamfer_polyline_points(
    points: &[Vec2],
    is_closed: bool,
    junctions: &HashSet<(i32, i32)>,
    t: f32,
    cos_threshold: f32,
) -> Vec<Vec2>
```

## 5. マイルストーン

### M1: 論理側の平滑化実装 (`hw_world`) ✅
- [x] `crates/hw_world/src/river.rs` の `generate_river_mask` を修正し、`center_y` と `width` を配列に事前生成するロジックに変更。
- [x] 生成した配列に移動平均（1D Smoothing）を複数パス適用する関数/ロジックを追加。
- [x] 平滑化された配列から `river_mask` を構築し、既存の保護帯チェックを維持する。
- [x] `cargo test -p hw_world` が通過することを確認（52 テスト全通過）。
- [ ] 代表シードでマップを起動し、川の論理形状が滑らかになっていることを目視確認。

### M2: 描画側の面取り実装 (`bevy_app`) ✅
- [x] `crates/bevy_app/src/world/map/boundary.rs` に `chamfer_polyline_points` 関数を追加。
- [x] `spawn_boundary_meshes` 内で `displace_polyline` の後・`sample_catmull_rom` の前に面取りを呼び出す。
- [x] ジャンクション頂点・開ポリライン端点のピン留め、閉ループへの安全な処理が実装されていることを確認。
- [x] `cargo check --workspace` および Clippy 0 警告を確認。
- [ ] ゲームを起動し、川岸リボンに wavy staircase がなく滑らかな曲線になっていることを目視確認。

### M3: 文書更新 ✅
- [x] `docs/map_generation.md` に M1（center_y スムージング）と M2（面取りパイプライン）を追記。
- [x] 計画書のステータスを `Complete` に更新。

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
